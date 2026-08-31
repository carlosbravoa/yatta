//! The vault: a plain folder of markdown files that is the single source of
//! truth. Every read re-scans from disk, so an edit made in a text editor or by
//! an agent is indistinguishable from one made in the app.

use crate::task::{parse_task, render_task, slugify, Status, Task};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRS: [&str; 4] = [".git", "node_modules", ".obsidian", ".trash"];
pub const ARCHIVE_DIR: &str = "archive";

/// Create the vault and, if it is pristine, seed it with a README explaining
/// the format plus a couple of example tasks so the app never opens on a blank
/// wall.
///
/// "Pristine" means the folder contains no markdown at all -- not merely that
/// the directory was absent. Picking (or creating) an empty folder in the file
/// picker is the ordinary path through onboarding, and such a vault still needs
/// its README: that file is the format contract for whoever, or whatever, opens
/// the folder next. Keying off markdown rather than directory existence also
/// means an existing notes folder is adopted untouched, and that emptying the
/// vault by hand does not resurrect the examples.
pub fn ensure_vault(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| format!("could not create {}: {e}", root.display()))?;

    let mut existing = Vec::new();
    collect(root, root, &mut existing, 0);
    if !existing.is_empty() || root.join("README.md").exists() {
        return Ok(());
    }

    let _ = fs::write(
        root.join("README.md"),
        include_str!("../assets/vault_readme.md"),
    );

    let welcome = Task {
        tags: vec!["yatta".into()],
        priority: crate::task::Priority::Medium,
        description: "Every task in this list is a markdown file in this folder.\n\n\
            Open the folder in your editor and change something -- the app updates as you save. \
            Point an AI agent at it and it can add or complete tasks by writing plain files.\n\n\
            Delete this task whenever you like."
            .into(),
        ..Task::new("Welcome — your tasks are just markdown files".into())
    };
    let _ = save_task(root, &welcome);

    let quickadd = Task {
        tags: vec!["yatta".into()],
        description: "Try typing this into the box at the top:\n\n\
            `Send the quarterly report tomorrow !high #work`\n\n\
            The date, priority and tag are parsed out of the sentence as you type."
            .into(),
        ..Task::new("Try the quick-add box".into())
    };
    let _ = save_task(root, &quickadd);

    Ok(())
}

fn should_skip(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if should_skip(&name) {
                continue;
            }
            collect(&path, root, out, depth + 1);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            // The README documents the format; it isn't a task.
            if path.parent() == Some(root) && name.eq_ignore_ascii_case("readme.md") {
                continue;
            }
            out.push(path);
        }
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn list_tasks(root: &Path) -> Vec<Task> {
    let mut files = Vec::new();
    collect(root, root, &mut files, 0);

    let mut tasks: Vec<Task> = files
        .iter()
        .filter_map(|path| {
            let content = fs::read_to_string(path).ok()?;
            Some(parse_task(&content, &rel(root, path)))
        })
        .collect();

    // A duplicated id (an agent copying a file, a careless cp) would make the
    // UI address the wrong file, so fall back to the path, which is unique.
    let mut seen = std::collections::HashSet::new();
    for task in &mut tasks {
        if !seen.insert(task.id.clone()) {
            task.id = task.path.clone();
        }
    }
    tasks
}

fn unique_path(root: &Path, stem: &str) -> PathBuf {
    let candidate = root.join(format!("{stem}.md"));
    if !candidate.exists() {
        return candidate;
    }
    for n in 2..1000 {
        let candidate = root.join(format!("{stem}-{n}.md"));
        if !candidate.exists() {
            return candidate;
        }
    }
    root.join(format!("{stem}-{}.md", crate::task::new_id()))
}

/// Write a task, atomically. Returns the vault-relative path it landed on.
///
/// The filename is chosen once, at creation, and then left alone: renaming a
/// file when the title changes would break any path an agent or a link is
/// holding. The title in the frontmatter is the thing that's authoritative.
pub fn save_task(root: &Path, task: &Task) -> Result<String, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;

    let path = if task.path.trim().is_empty() {
        unique_path(root, &slugify(&task.title))
    } else {
        let candidate = root.join(&task.path);
        // Refuse to follow a path that escapes the vault.
        if !candidate.starts_with(root) {
            return Err("refusing to write outside the vault".into());
        }
        candidate
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let contents = render_task(task);
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, contents).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("could not save {}: {e}", path.display()))?;

    Ok(rel(root, &path))
}

pub fn delete_task(root: &Path, rel_path: &str) -> Result<(), String> {
    let path = root.join(rel_path);
    if !path.starts_with(root) {
        return Err("refusing to delete outside the vault".into());
    }
    fs::remove_file(&path).map_err(|e| format!("could not delete {}: {e}", path.display()))
}

/// Move every completed task into `archive/`. They stay real markdown files --
/// archiving is a move, never a delete.
pub fn archive_done(root: &Path) -> Result<usize, String> {
    let archive = root.join(ARCHIVE_DIR);
    fs::create_dir_all(&archive).map_err(|e| e.to_string())?;

    let mut moved = 0;
    for task in list_tasks(root) {
        if task.status != Status::Done || task.archived {
            continue;
        }
        let from = root.join(&task.path);
        let stem = Path::new(&task.path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| slugify(&task.title));
        let to = unique_path(&archive, &stem);
        if fs::rename(&from, &to).is_ok() {
            moved += 1;
        }
    }
    Ok(moved)
}

/// Move one task back out of `archive/` into the vault root. Archiving is a
/// file move, so undoing it is a file move too.
pub fn restore_task(root: &Path, rel_path: &str) -> Result<String, String> {
    let from = root.join(rel_path);
    if !from.starts_with(root) {
        return Err("refusing to move a file outside the vault".into());
    }
    let stem = Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or("not a file")?;
    let to = unique_path(root, &stem);
    fs::rename(&from, &to).map_err(|e| format!("could not restore {rel_path}: {e}"))?;
    Ok(rel(root, &to))
}

/// Stamp or clear the completion date as status changes, so the file records
/// when something actually got done.
pub fn apply_status(task: &mut Task, status: Status) {
    task.status = status;
    task.completed = match status {
        Status::Done => Some(Local::now().format("%Y-%m-%d").to_string()),
        _ => None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yatta-test-{name}-{}", crate::task::new_id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn seeds_a_folder_that_does_not_exist_yet() {
        let root = temp_dir("absent");
        ensure_vault(&root).unwrap();
        assert!(root.join("README.md").exists());
        assert!(!list_tasks(&root).is_empty(), "example tasks are seeded");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn seeds_an_existing_but_empty_folder() {
        // The ordinary path through the first-run picker: the user creates or
        // selects an empty folder. It still needs its README.
        let root = temp_dir("empty");
        fs::create_dir_all(&root).unwrap();
        ensure_vault(&root).unwrap();
        assert!(root.join("README.md").exists(), "an existing empty folder still gets the README");
        assert!(!list_tasks(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adopts_a_folder_that_already_has_notes_without_touching_it() {
        let root = temp_dir("adopt");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("my-note.md"), "# Something I already had\n").unwrap();

        ensure_vault(&root).unwrap();

        assert!(!root.join("README.md").exists(), "we do not litter in someone's notes folder");
        let tasks = list_tasks(&root);
        assert_eq!(tasks.len(), 1, "no examples added alongside existing notes");
        assert_eq!(tasks[0].title, "Something I already had");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_resurrect_examples_after_the_user_clears_the_vault() {
        let root = temp_dir("cleared");
        ensure_vault(&root).unwrap();
        for task in list_tasks(&root) {
            fs::remove_file(root.join(&task.path)).unwrap();
        }
        assert!(list_tasks(&root).is_empty());

        ensure_vault(&root).unwrap();
        assert!(list_tasks(&root).is_empty(), "an intentionally emptied vault stays empty");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn archiving_moves_the_file_and_restoring_moves_it_back() {
        let root = temp_dir("archive");
        fs::create_dir_all(&root).unwrap();

        let mut task = Task::new("Finish the thing".into());
        apply_status(&mut task, Status::Done);
        let path = save_task(&root, &task).unwrap();
        assert!(root.join(&path).exists());

        assert_eq!(archive_done(&root).unwrap(), 1);
        assert!(!root.join(&path).exists(), "the original file is gone");

        let archived: Vec<_> = list_tasks(&root).into_iter().filter(|t| t.archived).collect();
        assert_eq!(archived.len(), 1);
        assert!(archived[0].path.starts_with("archive/"));

        let restored = restore_task(&root, &archived[0].path).unwrap();
        assert!(!restored.starts_with("archive/"), "restored back to the vault root");
        assert!(root.join(&restored).exists());
        let _ = fs::remove_dir_all(&root);
    }
}
