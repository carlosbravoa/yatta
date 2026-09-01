//! User settings, persisted as JSON next to the app config.
//!
//! Note that the *tasks* never live here -- settings are app preferences only.
//! Everything that is your data lives in the vault as markdown.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub vault_path: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "none" | "tag" | "priority" | "due"
    pub group_by: String,
    /// "list" | "board"
    pub layout: String,
    /// "manual" | "due" | "priority" | "created" | "title"
    pub sort_by: String,
    pub show_done: bool,
    /// Opt-in: shells out to the `git` binary, no-ops when it's absent or the
    /// vault isn't a repo. Never a build dependency.
    pub git_autocommit: bool,
    /// Only meaningful in builds compiled with the `desktop-integration` feature.
    pub tray_enabled: bool,
    pub hotkey: String,
    pub first_run_done: bool,

    /// Deadline reminders.
    pub reminders_enabled: bool,
    /// Local `HH:MM` times to check at. One entry means once a day, two means
    /// twice; keeping it a list is what makes both the same code path.
    pub reminder_times: Vec<String>,
    /// Keep running in the tray when the window is closed, instead of
    /// quitting. Only honoured when a tray icon actually exists -- otherwise
    /// closing the window would strand the app with no way back.
    pub close_to_tray: bool,
    /// Start yatta at login, via an XDG autostart entry.
    pub autostart: bool,

    /// Width of the task detail panel, in CSS pixels.
    pub detail_width: u32,

    /// The last slot that fired, as `YYYY-MM-DDTHH:MM`. Lexicographically
    /// sortable on purpose -- a string comparison is enough to know whether a
    /// slot is still owed, and it survives a restart.
    pub last_reminder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            vault_path: String::new(),
            theme: "system".into(),
            group_by: "none".into(),
            layout: "list".into(),
            sort_by: "due".into(),
            show_done: false,
            git_autocommit: false,
            tray_enabled: true,
            hotkey: "CmdOrCtrl+Shift+Space".into(),
            first_run_done: false,
            reminders_enabled: true,
            reminder_times: vec!["09:00".into()],
            close_to_tray: false,
            autostart: false,
            detail_width: 380,
            last_reminder: String::new(),
        }
    }
}

fn config_file(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    Some(dir.join("settings.json"))
}

/// The suggested vault location, offered to the user on first run.
///
/// A plainly visible folder, not a hidden app-data dir -- the user is meant to
/// open, edit and version-control this.
pub fn default_vault(app: &AppHandle) -> PathBuf {
    // Inside a strictly-confined snap, HOME points at $SNAP_USER_DATA, so the
    // normal XDG lookup would bury the vault under ~/snap/yatta/current where
    // nobody would ever find their own tasks. SNAP_REAL_HOME is the real one.
    if let Some(home) = std::env::var_os("SNAP_REAL_HOME").filter(|h| !h.is_empty()) {
        let home = PathBuf::from(home);
        let documents = home.join("Documents");
        return if documents.is_dir() {
            documents.join("yatta")
        } else {
            home.join("yatta")
        };
    }

    let base = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("yatta")
}

/// Whether a settings file already exists on disk.
///
/// This is the honest test for "has this user run yatta before". A fresh
/// install has no config at all, whereas a config lacking `first_run_done`
/// came from a version that predates the first-run picker.
pub fn config_exists(app: &AppHandle) -> bool {
    config_file(app).is_some_and(|p| p.exists())
}

pub fn load(app: &AppHandle) -> Settings {
    let mut settings = config_file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default();

    if settings.vault_path.trim().is_empty() {
        settings.vault_path = default_vault(app).to_string_lossy().to_string();
    }
    settings
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = config_file(app).ok_or("could not resolve the config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}
