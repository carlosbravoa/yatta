//! Optional git auto-commit.
//!
//! This shells out to the `git` binary rather than linking libgit2 on purpose:
//! it keeps the feature identical on Linux, macOS and Windows and adds nothing
//! to the build. If git isn't installed, or the vault isn't a repo, or the user
//! hasn't switched it on, every function here quietly does nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Commits are coalesced: rapid edits produce one commit, not twenty.
const COALESCE: Duration = Duration::from_secs(4);

fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

pub fn is_repo(root: &Path) -> bool {
    git(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub struct Committer {
    root: Mutex<PathBuf>,
    enabled: AtomicBool,
    generation: AtomicU64,
}

impl Committer {
    pub fn new(root: PathBuf, enabled: bool) -> Arc<Self> {
        Arc::new(Committer {
            root: Mutex::new(root),
            enabled: AtomicBool::new(enabled),
            generation: AtomicU64::new(0),
        })
    }

    pub fn configure(&self, root: PathBuf, enabled: bool) {
        if let Ok(mut guard) = self.root.lock() {
            *guard = root;
        }
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Ask for a commit. Returns immediately; the commit happens on a
    /// background thread once edits have settled.
    pub fn request(self: &Arc<Self>, message: String) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let Ok(root) = self.root.lock().map(|r| r.clone()) else { return };

        // Only the newest request in a burst survives the sleep.
        let ticket = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let this = Arc::clone(self);

        std::thread::spawn(move || {
            std::thread::sleep(COALESCE);
            if this.generation.load(Ordering::SeqCst) != ticket {
                return;
            }
            if !this.enabled.load(Ordering::Relaxed) || !is_repo(&root) {
                return;
            }
            let _ = git(&root).args(["add", "-A"]).status();
            // `git commit` exits non-zero with nothing staged; that's a no-op,
            // not an error, so the result is deliberately discarded.
            let _ = git(&root)
                .args(["commit", "-m", &format!("yatta: {message}")])
                .status();
        });
    }
}
