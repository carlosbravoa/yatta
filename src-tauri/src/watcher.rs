//! Filesystem watching, so an edit made in a text editor or by an agent shows
//! up in the window without anyone pressing refresh.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Editors save in bursts (write, rename, chmod). Wait for quiet before
/// telling the UI anything.
const QUIET: Duration = Duration::from_millis(250);

/// Ignore filesystem noise for this long after the app's own write, so saving
/// a task doesn't bounce back as an "external change" and interrupt typing.
const SELF_WRITE_GRACE: i64 = 700;

pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn is_interesting(paths: &[std::path::PathBuf]) -> bool {
    paths.iter().any(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // Skip our own atomic-write temp files and editor swap files.
        !name.ends_with(".tmp")
            && !name.starts_with('.')
            && !name.ends_with('~')
            && p.extension().and_then(|e| e.to_str()) == Some("md")
    })
}

/// Start watching `root`. The returned watcher must be kept alive; dropping it
/// stops the watch, which is how switching vaults works.
pub fn start(
    app: AppHandle,
    root: &Path,
    last_self_write: Arc<AtomicI64>,
) -> Option<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Event>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })
    .ok()?;

    watcher.watch(root, RecursiveMode::Recursive).ok()?;

    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut relevant = is_interesting(&first.paths);
            // Drain the rest of the burst.
            while let Ok(event) = rx.recv_timeout(QUIET) {
                relevant |= is_interesting(&event.paths);
            }
            if !relevant {
                continue;
            }
            if now_millis() - last_self_write.load(Ordering::Relaxed) < SELF_WRITE_GRACE {
                continue;
            }
            let _ = app.emit("vault-changed", ());
        }
    });

    Some(watcher)
}
