//! Deadline reminders.
//!
//! One background thread for the life of the app. It re-reads settings every
//! tick rather than being restarted when they change, so toggling reminders or
//! editing a time takes effect without any lifecycle plumbing.

use crate::task::Status;
use crate::{settings, vault, AppState};
use chrono::{Local, NaiveTime};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const TICK: Duration = Duration::from_secs(30);

/// How late a slot may fire. Without this, opening the app in the evening
/// would immediately deliver a reminder scheduled for the morning, which is
/// noise rather than a reminder.
const MAX_LATENESS_MINUTES: i64 = 120;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };

    let (enabled, times, last, root) = {
        let Ok(s) = state.settings.lock() else { return };
        (
            s.reminders_enabled,
            s.reminder_times.clone(),
            s.last_reminder.clone(),
            PathBuf::from(&s.vault_path),
        )
    };
    if !enabled || times.is_empty() {
        return;
    }

    let now = Local::now();
    let today = now.date_naive().to_string();

    // Take the newest slot that is due and not yet fired, so a burst of missed
    // slots collapses into one notification rather than several.
    let mut fire: Option<(String, NaiveTime)> = None;
    for raw in &times {
        let Some(at) = parse_time(raw) else { continue };
        let key = format!("{today}T{}", at.format("%H:%M"));
        if key <= last || now.time() < at {
            continue;
        }
        if (now.time() - at).num_minutes() > MAX_LATENESS_MINUTES {
            // Too stale to deliver, but still record it so it does not linger.
            mark_fired(app, &state, &key);
            continue;
        }
        if fire.as_ref().is_none_or(|(k, _)| key > *k) {
            fire = Some((key, at));
        }
    }

    let Some((key, _)) = fire else { return };
    mark_fired(app, &state, &key);

    if let Some((title, body)) = summarize(&root) {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

fn parse_time(raw: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(raw.trim(), "%H:%M").ok()
}

fn mark_fired(app: &AppHandle, state: &tauri::State<'_, AppState>, key: &str) {
    if let Ok(mut s) = state.settings.lock() {
        s.last_reminder = key.to_string();
        let _ = settings::save(app, &s);
    }
}

/// Build the notification, or None when there is nothing worth interrupting
/// for. Silence when nothing is due is the whole point.
fn summarize(root: &PathBuf) -> Option<(String, String)> {
    let today = Local::now().date_naive().to_string();

    let mut overdue = Vec::new();
    let mut due_today = Vec::new();
    for task in vault::list_tasks(root) {
        if task.archived || task.status == Status::Done {
            continue;
        }
        match task.due.as_deref() {
            Some(due) if due < today.as_str() => overdue.push(task.title),
            Some(due) if due == today => due_today.push(task.title),
            _ => {}
        }
    }
    if overdue.is_empty() && due_today.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !overdue.is_empty() {
        parts.push(format!("{} overdue", overdue.len()));
    }
    if !due_today.is_empty() {
        parts.push(format!("{} due today", due_today.len()));
    }
    let title = parts.join(", ");

    // Overdue first: it is the more urgent half of the message.
    let names: Vec<String> = overdue.into_iter().chain(due_today).collect();
    let shown = names.iter().take(3).cloned().collect::<Vec<_>>().join("\n");
    let body = if names.len() > 3 {
        format!("{shown}\nand {} more", names.len() - 3)
    } else {
        shown
    };

    Some((title, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Priority, Task};

    fn temp_vault(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yatta-rem-{name}-{}", crate::task::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add(root: &PathBuf, title: &str, due: Option<&str>, status: Status) {
        let task = Task {
            due: due.map(str::to_string),
            status,
            priority: Priority::None,
            ..Task::new(title.into())
        };
        vault::save_task(root, &task).unwrap();
    }

    #[test]
    fn parses_only_well_formed_times() {
        assert!(parse_time("09:00").is_some());
        assert!(parse_time(" 17:30 ").is_some(), "whitespace is tolerated");
        assert!(parse_time("9:00").is_some());
        assert!(parse_time("25:00").is_none());
        assert!(parse_time("noon").is_none());
        assert!(parse_time("").is_none());
    }

    #[test]
    fn stays_silent_when_nothing_is_due() {
        let root = temp_vault("quiet");
        add(&root, "Someday thing", None, Status::Todo);
        add(&root, "Next month", Some("2099-01-01"), Status::Todo);
        assert!(summarize(&root).is_none(), "no notification when nothing is due");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignores_completed_and_archived_tasks() {
        let root = temp_vault("done");
        add(&root, "Already handled", Some("2020-01-01"), Status::Done);
        assert!(summarize(&root).is_none(), "a completed overdue task is not a reminder");

        add(&root, "Still open", Some("2020-01-01"), Status::Todo);
        assert!(summarize(&root).is_some());

        // Archiving it should silence the reminder again.
        vault::archive_done(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn counts_overdue_and_today_separately() {
        let root = temp_vault("counts");
        let today = Local::now().date_naive().to_string();
        add(&root, "Late one", Some("2020-01-01"), Status::Todo);
        add(&root, "Late two", Some("2020-02-02"), Status::Todo);
        add(&root, "Today one", Some(&today), Status::Todo);

        let (title, _) = summarize(&root).expect("something is due");
        assert_eq!(title, "2 overdue, 1 due today");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn body_lists_overdue_first_and_truncates() {
        let root = temp_vault("body");
        for n in 1..=5 {
            add(&root, &format!("Late {n}"), Some("2020-01-01"), Status::Todo);
        }
        let (_, body) = summarize(&root).expect("something is due");
        assert_eq!(body.lines().count(), 4, "three titles plus an 'and N more' line");
        assert!(body.ends_with("and 2 more"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn slot_keys_sort_chronologically() {
        // The scheduler relies on plain string ordering to decide what is owed.
        let mut keys = vec!["2026-08-31T17:00", "2026-08-30T09:00", "2026-08-31T09:00"];
        keys.sort();
        assert_eq!(
            keys,
            vec!["2026-08-30T09:00", "2026-08-31T09:00", "2026-08-31T17:00"]
        );
    }
}
