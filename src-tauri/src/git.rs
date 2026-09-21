//! Talking to the `git` binary.
//!
//! This shells out to `git` rather than linking libgit2 on purpose: the
//! feature stays identical on Linux, macOS and Windows, it adds nothing to the
//! build, and -- the part that matters for syncing -- the credential helper,
//! SSH agent and config that apply are the user's own. The snap bundles the
//! binary, which is what keeps that confinement strict.
//!
//! Nothing here decides policy. Every function is a thin, timed,
//! non-interactive wrapper around one git command; when to run them, and what
//! to do when two devices disagree, lives in `sync` and `merge`.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Anything purely local: staging, committing, reading blobs out of the index.
pub const LOCAL: Duration = Duration::from_secs(30);

/// Anything that touches the network. Generous, because a cold SSH handshake
/// on a bad connection is slow; bounded, because a hung fetch must never wedge
/// the sync thread for the life of the app.
pub const NETWORK: Duration = Duration::from_secs(120);

pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    pub fn line(&self) -> String {
        self.stdout.trim().to_string()
    }

    /// The first meaningful line of a failure -- what the user should be told,
    /// rather than the whole of git's advice block.
    pub fn why(&self) -> String {
        let text = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        text.lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with("hint:"))
            .unwrap_or("git failed")
            .to_string()
    }
}

/// A git invocation that can never stop and wait for a human.
///
/// A sync runs on a background thread with no terminal attached, so an
/// authentication prompt would not be answered -- it would simply hang until
/// the timeout, every time. `GIT_TERMINAL_PROMPT=0` and SSH's `BatchMode` turn
/// that into an immediate, reportable error instead. Credential *helpers* are
/// deliberately left alone: those answer without asking, and are how a working
/// HTTPS remote keeps working.
fn base(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        // Parsed output must not follow the user's locale.
        .env("LC_ALL", "C");

    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        cmd.env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=15",
        );
    }
    // Stops OpenSSH reaching for a GUI passphrase dialog nobody is watching.
    cmd.env("SSH_ASKPASS_REQUIRE", "never");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).into_owned()
    })
}

/// Run a git command to completion, or kill it.
///
/// `std::process` has no waiting-with-a-deadline, so the pipes are drained on
/// their own threads (a full pipe buffer would deadlock a naive wait) while
/// this one polls. Something that outlives its timeout is a hung network call
/// or a prompt we failed to suppress; either way the right answer is to kill
/// it and report, not to wait forever.
pub fn run(root: &Path, args: &[&str], timeout: Duration) -> Result<Out, String> {
    let mut child = base(root)
        .args(args)
        .spawn()
        .map_err(|e| format!("could not run git: {e}"))?;

    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("git {} timed out", args.first().unwrap_or(&"")));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(format!("could not wait for git: {e}")),
        }
    };

    Ok(Out {
        code,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

fn quiet(root: &Path, args: &[&str]) -> Option<Out> {
    run(root, args, LOCAL).ok()
}

// ---------------------------------------------------------------------------
// Asking the repository about itself
// ---------------------------------------------------------------------------

pub fn is_repo(root: &Path) -> bool {
    quiet(root, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|o| o.ok())
}

/// The checked-out branch, or `None` on a detached HEAD. A repository with no
/// commits yet still has one: `symbolic-ref` reports the branch HEAD points
/// at, which `rev-parse HEAD` cannot.
pub fn branch(root: &Path) -> Option<String> {
    quiet(root, &["symbolic-ref", "--short", "HEAD"])
        .filter(|o| o.ok())
        .map(|o| o.line())
        .filter(|b| !b.is_empty())
}

pub fn remotes(root: &Path) -> Vec<String> {
    quiet(root, &["remote"])
        .filter(|o| o.ok())
        .map(|o| o.stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

/// Where a `git push` from this branch would go, and what it should be
/// compared against.
///
/// A tracking branch (`@{u}`) is the ordinary case and needs no guessing. A
/// remote added by hand without a first `push -u` has none, so fall back to
/// the branch of the same name on the branch's configured remote, or on
/// `origin`; `set_upstream` then tells the pusher to record it. Without a
/// remote at all there is nothing to sync with, and this returns `None`.
pub struct Target {
    pub remote: String,
    pub branch: String,
    /// `origin/main` -- the ref to merge from. May not exist yet.
    pub remote_ref: String,
    pub set_upstream: bool,
}

pub fn target(root: &Path) -> Option<Target> {
    let branch = branch(root)?;
    let remotes = remotes(root);
    if remotes.is_empty() {
        return None;
    }

    let tracked = quiet(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .filter(|o| o.ok())
        .map(|o| o.line())
        .filter(|u| !u.is_empty());

    if let Some(remote_ref) = tracked {
        // "origin/main" -> ("origin", "main"). A remote name cannot contain a
        // slash, so the first one splits it.
        let (remote, upstream_branch) = match remote_ref.split_once('/') {
            Some((r, b)) if remotes.iter().any(|known| known == r) => (r.to_string(), b.to_string()),
            _ => (remotes[0].clone(), branch.clone()),
        };
        return Some(Target {
            remote,
            branch: upstream_branch,
            remote_ref,
            set_upstream: false,
        });
    }

    let configured = quiet(root, &["config", "--get", &format!("branch.{branch}.remote")])
        .filter(|o| o.ok())
        .map(|o| o.line())
        .filter(|r| !r.is_empty());

    let remote = configured
        .or_else(|| remotes.iter().find(|r| *r == "origin").cloned())
        .unwrap_or_else(|| remotes[0].clone());

    Some(Target {
        remote_ref: format!("{remote}/{branch}"),
        remote,
        branch,
        set_upstream: true,
    })
}

pub fn ref_exists(root: &Path, name: &str) -> bool {
    quiet(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")])
        .is_some_and(|o| o.ok())
}

/// Commits on each side of the fork: `(ahead, behind)`.
pub fn divergence(root: &Path, remote_ref: &str) -> (u32, u32) {
    let range = format!("HEAD...{remote_ref}");
    let Some(out) = quiet(root, &["rev-list", "--left-right", "--count", &range]) else {
        return (0, 0);
    };
    if !out.ok() {
        return (0, 0);
    }
    let mut parts = out.stdout.split_whitespace();
    let ahead = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

/// A merge, rebase or cherry-pick the user left half-finished. Touching the
/// repository in that state would make a mess of their resolution, so sync
/// stands back and says so.
pub fn interrupted(root: &Path) -> Option<&'static str> {
    let dir = quiet(root, &["rev-parse", "--git-dir"])
        .filter(|o| o.ok())
        .map(|o| o.line())?;
    let dir = root.join(dir);
    for (file, label) in [
        ("MERGE_HEAD", "a merge"),
        ("rebase-merge", "a rebase"),
        ("rebase-apply", "a rebase"),
        ("CHERRY_PICK_HEAD", "a cherry-pick"),
        ("REVERT_HEAD", "a revert"),
    ] {
        if dir.join(file).exists() {
            return Some(label);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Changing it
// ---------------------------------------------------------------------------

/// `-c` flags that give git someone to commit as, on a machine that has not
/// told it.
///
/// A machine with no `user.email` cannot commit at all, and telling someone to
/// configure git before their task list will save is a poor trade. Only used
/// as a fallback: a configured identity always wins. Empty when one exists.
///
/// Every command that writes a commit needs this, not just `commit`: a merge
/// that is not a fast-forward makes a commit of its own, and refuses for the
/// same reason. Inside the snap `HOME` is `$SNAP_USER_DATA`, so the user's
/// real `~/.gitconfig` is never read and this fallback is the ordinary case,
/// not the odd one.
fn identity(root: &Path) -> Vec<String> {
    let has_identity = quiet(root, &["config", "--get", "user.email"])
        .is_some_and(|o| o.ok() && !o.line().is_empty());
    if has_identity {
        return Vec::new();
    }
    vec![
        "-c".into(),
        "user.name=yatta".into(),
        "-c".into(),
        "user.email=yatta@localhost".into(),
    ]
}

/// Commit everything in the vault. `Ok(false)` means there was nothing to
/// commit, which is the common case and not an error.
///
/// The pathspec keeps a vault that lives inside a larger repository honest:
/// only the task folder is ever staged, never the rest of someone's project.
pub fn commit_all(root: &Path, message: &str) -> Result<bool, String> {
    let add = run(root, &["add", "-A", "--", "."], LOCAL)?;
    if !add.ok() {
        return Err(add.why());
    }

    let mut args = identity(root);
    args.extend(["commit".into(), "-m".into(), message.into()]);

    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run(root, &refs, LOCAL)?;
    if out.ok() {
        return Ok(true);
    }
    // `git commit` exits non-zero with an empty index. That is a no-op, not a
    // failure -- every other non-zero exit is.
    let text = format!("{}{}", out.stdout, out.stderr);
    if text.contains("nothing to commit") || text.contains("no changes added") {
        return Ok(false);
    }
    Err(out.why())
}

pub fn fetch(root: &Path, remote: &str) -> Result<(), String> {
    let out = run(root, &["fetch", "--prune", remote], NETWORK)?;
    if out.ok() {
        Ok(())
    } else {
        Err(out.why())
    }
}

pub fn push(root: &Path, t: &Target) -> Result<(), String> {
    let refspec = format!("HEAD:refs/heads/{}", t.branch);
    let mut args = vec!["push"];
    if t.set_upstream {
        args.push("--set-upstream");
    }
    args.extend([t.remote.as_str(), refspec.as_str()]);

    let out = run(root, &args, NETWORK)?;
    if out.ok() {
        Ok(())
    } else {
        Err(out.why())
    }
}

pub enum Merged {
    /// Nothing to do, or the merge went through on its own.
    Clean,
    /// Paths git could not merge by itself, vault-relative.
    Conflicts(Vec<String>),
    /// The merge never started: a dirty tree, unrelated histories, no ref.
    Failed(String),
}

pub fn merge(root: &Path, remote_ref: &str) -> Merged {
    let message = format!("yatta: merge {remote_ref}");
    let mut args = identity(root);
    args.extend(["merge".into(), "--no-edit".into(), "-m".into(), message, remote_ref.into()]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = match run(root, &refs, LOCAL) {
        Ok(out) => out,
        Err(e) => return Merged::Failed(e),
    };
    if out.ok() {
        return Merged::Clean;
    }

    let conflicts = unmerged(root);
    if conflicts.is_empty() {
        // Nothing to resolve means the merge was refused outright rather than
        // started and stopped. Leave no half-merge behind.
        let _ = quiet(root, &["merge", "--abort"]);
        return Merged::Failed(out.why());
    }
    Merged::Conflicts(conflicts)
}

/// Paths still in conflict, relative to the vault (which may be a subdirectory
/// of the repository -- hence `--relative`, which also scopes the answer to
/// the vault).
pub fn unmerged(root: &Path) -> Vec<String> {
    quiet(root, &["diff", "--name-only", "--diff-filter=U", "--relative"])
        .filter(|o| o.ok())
        .map(|o| o.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// One side of a conflict, straight out of the index: stage 1 is the common
/// ancestor, 2 is ours, 3 is theirs. `None` means that stage does not exist,
/// which is how a delete on one side shows up.
///
/// The `./` prefix is load-bearing: without it git resolves the path against
/// the top of the repository rather than the vault directory.
pub fn stage(root: &Path, n: u8, path: &str) -> Option<String> {
    let spec = format!(":{n}:./{path}");
    quiet(root, &["show", &spec])
        .filter(|o| o.ok())
        .map(|o| o.stdout)
}

pub fn add(root: &Path, path: &str) -> bool {
    quiet(root, &["add", "--", path]).is_some_and(|o| o.ok())
}

pub fn remove(root: &Path, path: &str) -> bool {
    quiet(root, &["rm", "-q", "-f", "--ignore-unmatch", "--", path]).is_some_and(|o| o.ok())
}

/// Finish a merge whose conflicts have been resolved in the working tree.
pub fn commit_merge(root: &Path, message: &str) -> Result<(), String> {
    let mut args = identity(root);
    args.extend(["commit".into(), "--no-edit".into(), "-m".into(), message.into()]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run(root, &refs, LOCAL)?;
    if out.ok() {
        Ok(())
    } else {
        Err(out.why())
    }
}

pub fn abort_merge(root: &Path) {
    let _ = quiet(root, &["merge", "--abort"]);
}

/// Three-way merge of plain text, with git's own line-level merge.
///
/// Reusing `git merge-file` rather than hand-rolling a diff is the whole
/// reason two devices editing *different* paragraphs of the same note merge
/// silently: only genuinely overlapping edits come back marked. Returns the
/// merged text and whether it contains conflict markers.
pub fn merge_text(
    ours: &str,
    base: &str,
    theirs: &str,
    ours_label: &str,
    theirs_label: &str,
) -> (String, bool) {
    let dir = std::env::temp_dir().join(format!("yatta-merge-{}", crate::task::new_id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return (ours.to_string(), false);
    }

    let write = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).ok().map(|_| path)
    };
    let (Some(o), Some(b), Some(t)) = (write("ours", ours), write("base", base), write("theirs", theirs))
    else {
        let _ = std::fs::remove_dir_all(&dir);
        return (ours.to_string(), false);
    };

    let args = vec![
        "merge-file",
        "-p",
        "-L",
        ours_label,
        "-L",
        "common ancestor",
        "-L",
        theirs_label,
        o.to_str().unwrap_or_default(),
        b.to_str().unwrap_or_default(),
        t.to_str().unwrap_or_default(),
    ];
    // `merge-file` needs no repository, but `run` still wants a directory to
    // start in, and the temp dir is one that certainly exists.
    let result = run(&dir, &args, LOCAL);
    let _ = std::fs::remove_dir_all(&dir);

    match result {
        // Exit code is the number of conflicts; negative means git itself
        // failed, in which case keeping our text unchanged is the safe answer.
        Ok(out) if out.code == 0 => (out.stdout, false),
        Ok(out) if out.code > 0 => (out.stdout, true),
        _ => (ours.to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_edits_to_different_lines_without_a_conflict() {
        let base = "one\ntwo\nthree\n";
        let ours = "ONE\ntwo\nthree\n";
        let theirs = "one\ntwo\nTHREE\n";
        let (merged, conflicted) = merge_text(ours, base, theirs, "here", "there");
        assert!(!conflicted, "separate lines merge cleanly");
        assert_eq!(merged, "ONE\ntwo\nTHREE\n");
    }

    #[test]
    fn marks_overlapping_edits() {
        let base = "one\n";
        let (merged, conflicted) = merge_text("ours\n", base, "theirs\n", "here", "there");
        assert!(conflicted);
        assert!(merged.contains("<<<<<<< here"));
        assert!(merged.contains(">>>>>>> there"));
    }
}
