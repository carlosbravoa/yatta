//! Keeping the vault and its git remote in step.
//!
//! Two machines share one vault by sharing one branch. The whole protocol is
//! the obvious one -- commit, fetch, merge, push -- and everything interesting
//! is in the timing and in what happens when the merge does not go cleanly
//! (see `merge`).
//!
//! Three things trigger a sync, and they answer different needs:
//!   * the Sync button, for when you are about to walk away from this machine;
//!   * a timer, so a laptop left open picks up what the desktop did;
//!   * a short delay after your own edits settle, so the other machine has
//!     something to pick up. Without that last one a task written here is
//!     invisible there until the next tick, which is the difference between a
//!     list you trust on two devices and one you don't.
//!
//! Local commits are a separate, always-available feature: auto-commit gives
//! you history in a folder with no remote at all, and sync is layered on top.

use crate::git::{self, Merged};
use crate::merge;
use crate::task::parse_task;
use crate::vault;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Commits are coalesced: rapid edits produce one commit, not twenty.
const COALESCE: Duration = Duration::from_secs(4);

/// How long after the last commit to push. Long enough that a burst of edits
/// is one push, short enough that picking up the other device works.
const AFTER_CHANGE: Duration = Duration::from_secs(20);

/// The scheduler wakes up this often and asks whether the interval has come
/// round. A coarse tick keeps an idle app genuinely idle.
const TICK: Duration = Duration::from_secs(60);

/// Startup sync, delayed so it never competes with the first paint.
const AT_LAUNCH: Duration = Duration::from_secs(6);

#[derive(Clone)]
pub struct Config {
    pub root: PathBuf,
    pub autocommit: bool,
    pub sync: bool,
    pub interval_mins: u32,
    pub on_change: bool,
}

impl Config {
    pub fn new(root: PathBuf) -> Self {
        Config {
            root,
            autocommit: false,
            sync: false,
            interval_mins: 60,
            on_change: true,
        }
    }
}

/// Everything the UI needs to show one button honestly.
#[derive(Clone, Serialize)]
pub struct SyncState {
    /// "off" | "idle" | "syncing" | "error"
    pub status: String,
    pub message: String,
    /// RFC 3339, or empty when this session has not synced yet.
    pub last_sync: String,
    /// Vault-relative paths whose notes came back with conflict markers.
    pub conflicts: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The vault is a repo with a remote, so syncing is possible at all.
    pub available: bool,
    pub repo: bool,
    pub branch: String,
    pub remote: String,
}

impl Default for SyncState {
    fn default() -> Self {
        SyncState {
            status: "off".into(),
            message: String::new(),
            last_sync: String::new(),
            conflicts: Vec::new(),
            ahead: 0,
            behind: 0,
            available: false,
            repo: false,
            branch: String::new(),
            remote: String::new(),
        }
    }
}

/// What this machine calls itself, for commit messages and conflict markers.
/// The whole point of a marker is to say which device wrote which line.
pub fn device_name() -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        for key in ["YATTA_DEVICE", "HOSTNAME", "COMPUTERNAME"] {
            if let Some(v) = std::env::var_os(key) {
                let v = v.to_string_lossy().trim().to_string();
                if !v.is_empty() {
                    return v;
                }
            }
        }
        if let Ok(v) = std::fs::read_to_string("/etc/hostname") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
        "this device".into()
    })
    .clone()
}

pub struct Engine {
    app: Mutex<Option<AppHandle>>,
    config: Mutex<Config>,
    state: Mutex<SyncState>,
    /// One sync at a time. A timer tick landing on a manual click must not
    /// start a second merge in the same repository.
    busy: AtomicBool,
    commit_ticket: AtomicU64,
    sync_ticket: AtomicU64,
    last_run: Mutex<Option<Instant>>,
}

impl Engine {
    pub fn new(root: PathBuf) -> Arc<Self> {
        Arc::new(Engine {
            app: Mutex::new(None),
            config: Mutex::new(Config::new(root)),
            state: Mutex::new(SyncState::default()),
            busy: AtomicBool::new(false),
            commit_ticket: AtomicU64::new(0),
            sync_ticket: AtomicU64::new(0),
            last_run: Mutex::new(None),
        })
    }

    pub fn state(&self) -> SyncState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn config(&self) -> Config {
        self.config
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }

    fn with_state(&self, edit: impl FnOnce(&mut SyncState)) {
        let snapshot = {
            let Ok(mut state) = self.state.lock() else { return };
            edit(&mut state);
            state.clone()
        };
        if let Ok(guard) = self.app.lock() {
            if let Some(app) = guard.as_ref() {
                let _ = app.emit("sync-state", snapshot);
            }
        }
    }

    /// Start the scheduler. Called once, after the app handle exists.
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        if let Ok(mut guard) = self.app.lock() {
            *guard = Some(app);
        }

        let this = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(AT_LAUNCH);
            this.run_now("launch");

            loop {
                std::thread::sleep(TICK);
                let config = this.config();
                if !config.sync || config.interval_mins == 0 {
                    continue;
                }
                let due = this
                    .last_run
                    .lock()
                    .ok()
                    .and_then(|t| *t)
                    .map(|t| t.elapsed() >= Duration::from_secs(config.interval_mins as u64 * 60))
                    .unwrap_or(true);
                if due {
                    this.run_now("timer");
                }
            }
        });
    }

    /// Bring the engine in line with the settings. Cheap and safe to call on
    /// every settings change; the repository probe it needs runs off-thread so
    /// switching vaults never blocks the UI.
    pub fn configure(self: &Arc<Self>, config: Config) {
        let changed_root = {
            let Ok(mut guard) = self.config.lock() else { return };
            let changed = guard.root != config.root;
            *guard = config;
            changed
        };
        if changed_root {
            // A vault that has not been probed yet must not report the last
            // one's branch.
            self.with_state(|s| *s = SyncState::default());
        }
        let this = Arc::clone(self);
        std::thread::spawn(move || this.probe());
    }

    /// Ask the repository what it is, and tell the UI.
    pub fn probe(&self) {
        let config = self.config();
        let root = &config.root;

        let repo = git::is_repo(root);
        let target = if repo { git::target(root) } else { None };
        let (branch, remote) = match &target {
            Some(t) => (t.branch.clone(), t.remote.clone()),
            None => (git::branch(root).unwrap_or_default(), String::new()),
        };
        let available = repo && target.is_some();
        let (ahead, behind) = match &target {
            Some(t) if git::ref_exists(root, &t.remote_ref) => git::divergence(root, &t.remote_ref),
            _ => (0, 0),
        };

        self.with_state(|s| {
            s.repo = repo;
            s.available = available;
            s.branch = branch;
            s.remote = remote;
            s.ahead = ahead;
            s.behind = behind;
            if !config.sync {
                s.status = "off".into();
                s.message = String::new();
            } else if s.status == "off" {
                s.status = "idle".into();
            }
        });
    }

    // -- Committing ---------------------------------------------------------

    /// Ask for a commit. Returns immediately; the commit happens on a
    /// background thread once edits have settled.
    pub fn request_commit(self: &Arc<Self>, message: String) {
        let config = self.config();
        if !config.autocommit {
            return;
        }

        // Only the newest request in a burst survives the sleep.
        let ticket = self.commit_ticket.fetch_add(1, Ordering::SeqCst) + 1;
        let this = Arc::clone(self);

        std::thread::spawn(move || {
            std::thread::sleep(COALESCE);
            if this.commit_ticket.load(Ordering::SeqCst) != ticket {
                return;
            }
            let config = this.config();
            if !config.autocommit || !git::is_repo(&config.root) {
                return;
            }

            let committed = {
                let _guard = vault::lock();
                git::commit_all(&config.root, &format!("yatta: {message}"))
            };
            match committed {
                Ok(true) => {
                    if config.sync && config.on_change {
                        this.request_sync_soon();
                    }
                }
                Ok(false) => {}
                Err(e) => eprintln!("yatta: git commit: {e}"),
            }
        });
    }

    /// Push what just changed, once the dust has settled.
    fn request_sync_soon(self: &Arc<Self>) {
        let ticket = self.sync_ticket.fetch_add(1, Ordering::SeqCst) + 1;
        let this = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(AFTER_CHANGE);
            if this.sync_ticket.load(Ordering::SeqCst) != ticket {
                return;
            }
            this.run_now("after changes");
        });
    }

    // -- Syncing ------------------------------------------------------------

    /// Sync on a background thread. The UI follows along through `sync-state`
    /// events rather than waiting for a return value.
    pub fn sync_now(self: &Arc<Self>) {
        let this = Arc::clone(self);
        std::thread::spawn(move || this.run_now("manual"));
    }

    fn run_now(self: &Arc<Self>, reason: &str) {
        let config = self.config();
        if !config.sync {
            return;
        }
        if self.busy.swap(true, Ordering::SeqCst) {
            return; // a sync is already in flight
        }

        let result = self.sync_once(&config, reason);

        // A push rejected because the remote moved between our fetch and our
        // push is not a failure, it is a race -- and the cure for it is to go
        // round once more.
        let result = match result {
            Err(e) if is_race(&e) => self.sync_once(&config, reason),
            other => other,
        };

        if let Ok(mut last) = self.last_run.lock() {
            *last = Some(Instant::now());
        }
        self.busy.store(false, Ordering::SeqCst);

        match result {
            Ok(outcome) => {
                let now = chrono::Local::now().to_rfc3339();
                self.with_state(|s| {
                    s.status = "idle".into();
                    s.message = outcome.message.clone();
                    s.last_sync = now;
                    s.conflicts = outcome.conflicts.clone();
                    s.ahead = 0;
                    s.behind = 0;
                });
                if outcome.changed {
                    if let Ok(guard) = self.app.lock() {
                        if let Some(app) = guard.as_ref() {
                            let _ = app.emit("vault-changed", ());
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("yatta: sync ({reason}): {e}");
                self.with_state(|s| {
                    s.status = "error".into();
                    s.message = explain(&e);
                });
            }
        }
        self.probe();
    }

    fn sync_once(&self, config: &Config, reason: &str) -> Result<Outcome, String> {
        let root = &config.root;
        if !git::is_repo(root) {
            return Err("this folder is not a git repository".into());
        }
        if let Some(what) = git::interrupted(root) {
            return Err(format!(
                "the vault is in the middle of {what}; finish it in git first"
            ));
        }
        let Some(target) = git::target(root) else {
            return Err("this repository has no remote to sync with".into());
        };

        self.with_state(|s| {
            s.status = "syncing".into();
            s.message = String::new();
        });

        // 1. Anything not yet committed is committed now, so the merge has
        //    something complete to work against and no edit is left behind.
        {
            let _guard = vault::lock();
            git::commit_all(root, &format!("yatta: sync from {} ({reason})", device_name()))?;
        }

        // 2. What has the other device done?
        git::fetch(root, &target.remote)?;

        let mut outcome = Outcome::default();

        // 3. Take it in. A remote branch that does not exist yet simply means
        //    we are the first to push.
        if git::ref_exists(root, &target.remote_ref) {
            let (_, behind) = git::divergence(root, &target.remote_ref);
            if behind > 0 {
                let _guard = vault::lock();
                match git::merge(root, &target.remote_ref) {
                    Merged::Clean => {}
                    Merged::Conflicts(paths) => {
                        let (conflicts, renamed) = self.resolve(root, &target.remote_ref, paths)?;
                        outcome.conflicts = conflicts;
                        outcome.renamed = renamed;
                    }
                    Merged::Failed(why) => return Err(why),
                }
                outcome.changed = true;
            }
        }

        // 4. Hand ours over.
        let (ahead, _) = if git::ref_exists(root, &target.remote_ref) {
            git::divergence(root, &target.remote_ref)
        } else {
            (1, 0) // nothing to compare against; the first push is always due
        };
        if ahead > 0 {
            git::push(root, &target)?;
            outcome.pushed = ahead;
        }

        outcome.message = outcome.summarise();
        Ok(outcome)
    }

    /// Resolve every conflicted path and finish the merge commit.
    ///
    /// The merge must end one way or another: leaving the repository half
    /// merged would break every later sync and confront the user with a state
    /// the app never explains. So anything unresolvable is committed *with*
    /// its markers and reported, rather than abandoned.
    fn resolve(
        &self,
        root: &Path,
        remote_ref: &str,
        paths: Vec<String>,
    ) -> Result<(Vec<String>, usize), String> {
        let here = format!("{} (this device)", device_name());
        let there = format!("{remote_ref} (the other device)");
        let mut conflicted = Vec::new();
        let mut renamed = 0usize;

        for path in paths {
            let base = git::stage(root, 1, &path);
            let ours = git::stage(root, 2, &path);
            let theirs = git::stage(root, 3, &path);
            let full = root.join(&path);

            match (ours, theirs) {
                // Both edited it. The interesting case, and the common one.
                (Some(ours), Some(theirs)) => {
                    if !is_task(&path) {
                        // Not a task -- a README, a .gitignore. git has
                        // already written its own merge, markers and all.
                        git::add(root, &path);
                        conflicted.push(path);
                        continue;
                    }

                    // Two devices can independently create `buy-milk.md` for
                    // two different errands. Different ids mean two tasks, not
                    // one disagreement, so both survive under separate names.
                    if base.is_none() {
                        let ours_id = parse_task(&ours, &path).id;
                        let theirs_id = parse_task(&theirs, &path).id;
                        if ours_id != theirs_id {
                            write(&full, &ours)?;
                            git::add(root, &path);
                            let other = vault::write_beside(root, &path, &theirs)?;
                            git::add(root, &other);
                            continue;
                        }
                    }

                    let resolved = merge::merge_task(
                        &path,
                        base.as_deref(),
                        &ours,
                        &theirs,
                        &|o, b, t| git::merge_text(o, b, t, &here, &there),
                    );
                    write(&full, &resolved.content)?;
                    git::add(root, &path);
                    if resolved.renamed {
                        renamed += 1;
                    }
                    if resolved.conflicted {
                        conflicted.push(path);
                    }
                }

                // One side deleted what the other was editing. Keep the
                // edit: deleting it again takes one click, and rewriting a
                // note from memory does not.
                (Some(ours), None) => {
                    write(&full, &ours)?;
                    git::add(root, &path);
                }
                (None, Some(theirs)) => {
                    write(&full, &theirs)?;
                    git::add(root, &path);
                }
                (None, None) => {
                    git::remove(root, &path);
                }
            }
        }

        let message = format!(
            "yatta: merge {remote_ref} into {}",
            git::branch(root).unwrap_or_else(|| "HEAD".into())
        );
        if let Err(e) = git::commit_merge(root, &message) {
            git::abort_merge(root);
            return Err(format!("could not complete the merge: {e}"));
        }
        Ok((conflicted, renamed))
    }
}

#[derive(Default)]
pub struct Outcome {
    pub changed: bool,
    pub pushed: u32,
    pub conflicts: Vec<String>,
    /// Tasks both devices retitled, where this one's name was kept. Counted
    /// rather than listed because the alternative is still in `git log` -- but
    /// reported, because it is the only merge decision that leaves no trace in
    /// the file it changed.
    pub renamed: usize,
    pub message: String,
}

impl Outcome {
    fn summarise(&self) -> String {
        if !self.conflicts.is_empty() {
            let n = self.conflicts.len();
            return format!("{n} task{} need{} a look", plural(n), if n == 1 { "s" } else { "" });
        }
        if self.renamed > 0 {
            let n = self.renamed;
            return format!("Merged — kept this device's title for {n} task{}", plural(n));
        }
        match (self.changed, self.pushed > 0) {
            (true, true) => "Merged and pushed".into(),
            (true, false) => "Merged".into(),
            (false, true) => "Pushed".into(),
            (false, false) => "Up to date".into(),
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, content).map_err(|e| format!("could not write {}: {e}", path.display()))
}

fn is_task(rel_path: &str) -> bool {
    rel_path.to_ascii_lowercase().ends_with(".md")
        && !rel_path.eq_ignore_ascii_case("readme.md")
}

/// A push refused because the remote moved on under us. Worth one retry.
fn is_race(error: &str) -> bool {
    let e = error.to_ascii_lowercase();
    e.contains("non-fast-forward") || e.contains("fetch first") || e.contains("rejected")
}

/// Turn git's own words into something worth putting in front of a user.
fn explain(error: &str) -> String {
    let e = error.to_ascii_lowercase();
    if e.contains("could not read from remote")
        || e.contains("authentication failed")
        || e.contains("permission denied")
        || e.contains("terminal prompts disabled")
    {
        return "Could not sign in to the remote. yatta uses your own git credentials: set up an \
                SSH key or a credential helper and try again."
            .into();
    }
    if e.contains("could not resolve host") || e.contains("network is unreachable") || e.contains("timed out")
    {
        return "Could not reach the remote. Check the connection and try again.".into();
    }
    if e.contains("refusing to merge unrelated histories") {
        return "The remote has a different history to this vault. Sort that out in git once, \
                then sync again."
            .into();
    }
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Two clones of one bare repository, driven through the real engine: the
    /// rules in `merge` are only worth anything if git, the index and the
    /// three stages behave as this module assumes they do.
    struct Pair {
        dir: PathBuf,
        a: PathBuf,
        b: PathBuf,
    }

    fn git_in(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@localhost")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@localhost")
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn set_up() -> Option<Pair> {
        if Command::new("git").arg("--version").output().is_err() {
            return None;
        }
        let dir = std::env::temp_dir().join(format!("yatta-sync-{}", crate::task::new_id()));
        std::fs::create_dir_all(&dir).ok()?;

        git_in(&dir, &["init", "--bare", "-b", "main", "remote.git"]);
        let remote = dir.join("remote.git");
        let remote = remote.to_string_lossy().to_string();
        // An empty remote has no branch to ask for by name; the clone gets its
        // branch from init.defaultBranch, which git may otherwise set to
        // anything.
        git_in(&dir, &["-c", "init.defaultBranch=main", "clone", &remote, "a"]);

        Some(Pair {
            a: dir.join("a"),
            b: dir.join("b"),
            dir,
        })
    }

    fn engine_for(root: &Path) -> (Arc<Engine>, Config) {
        let config = Config {
            root: root.to_path_buf(),
            autocommit: true,
            sync: true,
            interval_mins: 0,
            on_change: false,
        };
        (Engine::new(root.to_path_buf()), config)
    }

    fn sync(root: &Path) -> Outcome {
        let (engine, config) = engine_for(root);
        engine
            .sync_once(&config, "test")
            .unwrap_or_else(|e| panic!("sync in {}: {e}", root.display()))
    }

    fn task_file(status: &str, priority: &str, body: &str) -> String {
        format!("---\nid: shared01\ntitle: Send the report\nstatus: {status}\npriority: {priority}\ntags: [work]\ncreated: 2026-09-01\n---\n\n{body}\n")
    }

    #[test]
    fn two_devices_converge_on_one_vault() {
        let Some(pair) = set_up() else { return };

        // Device A writes a task and syncs it up.
        std::fs::write(pair.a.join("report.md"), task_file("todo", "low", "Draft it.")).unwrap();
        let first = sync(&pair.a);
        assert_eq!(first.pushed, 1, "the first sync pushes");

        // Device B is set up the way the docs say: clone, then point yatta at
        // the folder.
        let remote = pair.dir.join("remote.git").to_string_lossy().to_string();
        git_in(&pair.dir, &["clone", "-b", "main", &remote, "b"]);
        assert!(pair.b.join("report.md").exists());

        // Now both edit the same task, in ways that disagree in every field.
        std::fs::write(
            pair.a.join("report.md"),
            task_file("done", "low", "Draft it.\nAsk Ana to review."),
        )
        .unwrap();
        std::fs::write(
            pair.b.join("report.md"),
            task_file("doing", "urgent", "Draft it."),
        )
        .unwrap();

        sync(&pair.a);
        let outcome = sync(&pair.b);
        assert!(outcome.changed, "B merged something");
        assert!(
            outcome.conflicts.is_empty(),
            "an edit on one side only is not a conflict: {:?}",
            outcome.conflicts
        );

        let merged = crate::task::parse_task(
            &std::fs::read_to_string(pair.b.join("report.md")).unwrap(),
            "report.md",
        );
        assert_eq!(merged.status, crate::task::Status::Done, "furthest along");
        assert_eq!(merged.priority, crate::task::Priority::Urgent, "most urgent");
        assert!(merged.description.contains("Ask Ana to review."), "A's note survived");
        assert!(!merged.conflicted);

        // And A ends up with exactly what B resolved to.
        sync(&pair.a);
        assert_eq!(
            std::fs::read_to_string(pair.a.join("report.md")).unwrap(),
            std::fs::read_to_string(pair.b.join("report.md")).unwrap(),
            "both devices hold the same file"
        );

        let _ = std::fs::remove_dir_all(&pair.dir);
    }

    #[test]
    fn rival_notes_survive_as_a_marked_conflict() {
        let Some(pair) = set_up() else { return };
        std::fs::write(pair.a.join("report.md"), task_file("todo", "low", "Draft it.")).unwrap();
        sync(&pair.a);
        let remote = pair.dir.join("remote.git").to_string_lossy().to_string();
        git_in(&pair.dir, &["clone", "-b", "main", &remote, "b"]);

        std::fs::write(pair.a.join("report.md"), task_file("todo", "low", "Ask Ana first.")).unwrap();
        std::fs::write(pair.b.join("report.md"), task_file("todo", "low", "Numbers confirmed.")).unwrap();
        sync(&pair.a);
        let outcome = sync(&pair.b);

        assert_eq!(outcome.conflicts, vec!["report.md".to_string()]);
        let text = std::fs::read_to_string(pair.b.join("report.md")).unwrap();
        assert!(text.contains("Ask Ana first."), "neither side is dropped");
        assert!(text.contains("Numbers confirmed."));
        let task = crate::task::parse_task(&text, "report.md");
        assert!(task.conflicted, "the UI can see it needs a look");

        // Resolving keeps both notes and leaves a clean file that syncs on.
        let resolved = merge::resolve_markers(&task.description, merge::Side::Both);
        assert!(!merge::has_markers(&resolved));
        assert!(resolved.contains("Ask Ana first.") && resolved.contains("Numbers confirmed."));

        // The conflicted merge still committed, so the vault is never left
        // mid-merge for the next sync to trip over.
        assert!(git::interrupted(&pair.b).is_none());
        let _ = std::fs::remove_dir_all(&pair.dir);
    }

    #[test]
    fn an_edit_beats_a_delete_from_the_other_device() {
        let Some(pair) = set_up() else { return };
        std::fs::write(pair.a.join("report.md"), task_file("todo", "low", "Draft it.")).unwrap();
        sync(&pair.a);
        let remote = pair.dir.join("remote.git").to_string_lossy().to_string();
        git_in(&pair.dir, &["clone", "-b", "main", &remote, "b"]);

        // A deletes the task; B, not having seen that, edits it.
        std::fs::remove_file(pair.a.join("report.md")).unwrap();
        std::fs::write(pair.b.join("report.md"), task_file("doing", "high", "Started on it.")).unwrap();
        sync(&pair.a);
        sync(&pair.b);

        let text = std::fs::read_to_string(pair.b.join("report.md"))
            .expect("the edited task is not deleted out from under its author");
        assert!(text.contains("Started on it."));
        let _ = std::fs::remove_dir_all(&pair.dir);
    }

    #[test]
    fn same_filename_different_tasks_both_survive() {
        let Some(pair) = set_up() else { return };
        std::fs::write(pair.a.join("report.md"), task_file("todo", "low", "Draft it.")).unwrap();
        sync(&pair.a);
        let remote = pair.dir.join("remote.git").to_string_lossy().to_string();
        git_in(&pair.dir, &["clone", "-b", "main", &remote, "b"]);

        // Both devices now invent `errand.md` for two different errands, each
        // without having seen the other's. Same name, no common ancestor, two
        // genuinely different tasks.
        std::fs::write(
            pair.a.join("errand.md"),
            "---\nid: aaa111\ntitle: Buy milk\n---\n\nSemi-skimmed.\n",
        )
        .unwrap();
        std::fs::write(
            pair.b.join("errand.md"),
            "---\nid: bbb222\ntitle: Collect the parcel\n---\n\nBefore six.\n",
        )
        .unwrap();
        sync(&pair.a);
        sync(&pair.b);

        let titles: Vec<String> = crate::vault::list_tasks(&pair.b)
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert!(titles.contains(&"Buy milk".to_string()), "got {titles:?}");
        assert!(titles.contains(&"Collect the parcel".to_string()), "got {titles:?}");
        let _ = std::fs::remove_dir_all(&pair.dir);
    }

    #[test]
    fn a_rejected_push_is_treated_as_a_race() {
        assert!(is_race("! [rejected] main -> main (fetch first)"));
        assert!(!is_race("could not resolve host github.com"));
    }

    #[test]
    fn only_markdown_that_is_not_the_readme_merges_as_a_task() {
        assert!(is_task("work/report.md"));
        assert!(!is_task("README.md"));
        assert!(!is_task(".gitignore"));
    }

    #[test]
    fn outcomes_read_as_sentences() {
        let mut outcome = Outcome::default();
        assert_eq!(outcome.summarise(), "Up to date");
        outcome.pushed = 2;
        assert_eq!(outcome.summarise(), "Pushed");
        outcome.changed = true;
        assert_eq!(outcome.summarise(), "Merged and pushed");
        outcome.renamed = 1;
        assert_eq!(
            outcome.summarise(),
            "Merged — kept this device's title for 1 task",
            "a title kept over the other device's is said out loud"
        );
        outcome.conflicts = vec!["a.md".into()];
        assert_eq!(outcome.summarise(), "1 task needs a look", "conflicts come first");
    }
}
