//! Filesystem watching, so an edit made in a text editor or by an agent shows
//! up in the window without anyone pressing refresh.

use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
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

/// Whether an event means the vault's contents may have changed.
///
/// Reads must not count. The backend reports a file being *opened* as an
/// event too, and reloading reads every task file -- so treating an open as a
/// change makes each reload announce another one, and the app reloads the
/// whole vault several times a second for as long as it runs.
fn is_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

fn is_interesting(event: &notify::Event) -> bool {
    is_change(&event.kind)
        && event.paths.iter().any(|p| {
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
            let mut relevant = is_interesting(&first);
            // Drain the rest of the burst.
            while let Ok(event) = rx.recv_timeout(QUIET) {
                relevant |= is_interesting(&event);
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

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, ModifyKind};
    use notify::Event;

    fn event(kind: EventKind, path: &str) -> Event {
        Event::new(kind).add_path(path.into())
    }

    #[test]
    fn reading_a_task_is_not_a_change() {
        // The regression: a reload opens every file, and an open that counts
        // as a change triggers the next reload.
        for kind in [
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
            EventKind::Access(AccessKind::Read),
        ] {
            assert!(!is_interesting(&event(kind, "/vault/task.md")), "{kind:?}");
        }
    }

    #[test]
    fn writing_a_task_is_a_change() {
        for kind in [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            EventKind::Access(AccessKind::Close(AccessMode::Write)),
            EventKind::Remove(notify::event::RemoveKind::File),
        ] {
            assert!(is_interesting(&event(kind, "/vault/task.md")), "{kind:?}");
        }
    }

    #[test]
    fn only_markdown_counts() {
        let write = EventKind::Modify(ModifyKind::Data(DataChange::Any));
        assert!(!is_interesting(&event(write, "/vault/task.md.tmp")));
        assert!(!is_interesting(&event(write, "/vault/.hidden.md")));
        assert!(!is_interesting(&event(write, "/vault/notes.txt")));
    }
}
