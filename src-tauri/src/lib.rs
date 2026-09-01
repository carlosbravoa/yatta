mod autostart;
mod git;
mod reminders;
mod settings;
mod task;
mod vault;
mod watcher;

#[cfg(feature = "desktop-integration")]
mod tray;

use settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use task::{Status, Task};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    // `settings` is read by the reminder scheduler as well as the commands.
    pub settings: Mutex<Settings>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    last_self_write: Arc<AtomicI64>,
    committer: Arc<git::Committer>,
}

impl AppState {
    fn vault(&self) -> Result<PathBuf, String> {
        let settings = self.settings.lock().map_err(|_| "settings lock poisoned")?;
        Ok(PathBuf::from(&settings.vault_path))
    }

    /// Timestamp our own writes so the filesystem watcher can tell them apart
    /// from somebody else's.
    fn mark_write(&self) {
        self.last_self_write
            .store(watcher::now_millis(), Ordering::Relaxed);
    }
}

/// Bring the main window up, rebuilding it if it has been closed.
///
/// Closing to the tray genuinely closes the window rather than hiding it. GTK
/// unmapping and remapping a toplevel leaves GNOME's server-side decorations
/// stale -- the minimise, maximise and close buttons stop responding until
/// something forces a reconfigure, such as maximising. A freshly built window
/// has no such history, and it also gets focus, which a re-shown one does not
/// reliably get on Wayland.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned();

    match config {
        Some(cfg) => match WebviewWindowBuilder::from_config(app, &cfg) {
            Ok(builder) => {
                if let Err(e) = builder.build() {
                    eprintln!("yatta: could not reopen the main window ({e})");
                }
            }
            Err(e) => eprintln!("yatta: could not reopen the main window ({e})"),
        },
        None => eprintln!("yatta: no main window configuration to reopen from"),
    }
}

/// Open the About window.
///
/// A window rather than a panel inside the main window for the same reason the
/// quick-add popup is one: reached from the tray, there is no reliable way to
/// bring the main window forward on Wayland.
pub fn open_about(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "about", WebviewUrl::App("index.html?window=about".into()))
        .title("About yatta")
        .inner_size(420.0, 460.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .center()
        .build();
    if let Err(e) = built {
        eprintln!("yatta: could not open the About window ({e})");
    }
}

/// Open the quick-add popup: a small always-on-top window with one field.
///
/// This exists as a separate window rather than as "focus the main window and
/// put the cursor in the box" because on Wayland a client cannot raise itself
/// without an activation token, so asking the main window to come forward is
/// simply ignored by the compositor. A newly mapped window does get focus, so
/// capture works from the tray or the hotkey whatever the main window is doing
/// -- minimised, on another workspace, or closed.
pub fn open_quick_add(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("quickadd") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let built = WebviewWindowBuilder::new(
        app,
        "quickadd",
        WebviewUrl::App("index.html?window=quickadd".into()),
    )
    .title("Quick add")
    .inner_size(620.0, 132.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .center()
    .build();

    if let Err(e) = built {
        eprintln!("yatta: could not open the quick-add window ({e})");
    }
}

/// Invoked by the popup once a task is saved, so an open main window refreshes.
/// The filesystem watcher deliberately ignores our own writes, which is exactly
/// what would make this save invisible to the other window.
#[tauri::command]
fn quick_add_done(app: AppHandle) {
    use tauri::Emitter;
    let _ = app.emit("vault-changed", ());
    if let Some(window) = app.get_webview_window("quickadd") {
        let _ = window.close();
    }
}

#[derive(serde::Serialize)]
pub struct AppInfo {
    version: String,
    /// Whether a tray icon is available, which is what makes closing to the
    /// tray a safe thing to offer.
    has_tray: bool,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        has_tray: cfg!(feature = "desktop-integration"),
    }
}

#[derive(serde::Serialize)]
pub struct VaultInfo {
    path: String,
    exists: bool,
    is_git_repo: bool,
    /// False in a build compiled without `desktop-integration`, so the UI can
    /// hide the tray and hotkey settings instead of offering dead switches.
    supports_tray: bool,
    /// Whether a settings file already existed. Distinguishes a returning user
    /// from a fresh install that merely happens to have a folder sitting at
    /// the default vault path.
    had_settings: bool,
}

/// Bring the vault, watcher, git committer and hotkey in line with the current
/// settings. Safe to call repeatedly; switching vaults goes through here.
fn apply_runtime(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings = {
        let guard = state.settings.lock().map_err(|_| "settings lock poisoned")?;
        guard.clone()
    };
    let root = PathBuf::from(&settings.vault_path);

    // Until the user has confirmed where their tasks should live, create
    // nothing: they may yet pick a different folder, and leaving an orphaned
    // directory behind in the meantime would be presumptuous.
    if settings.first_run_done {
        vault::ensure_vault(&root)?;

        // Dropping the previous watcher stops the old watch.
        if let Ok(mut guard) = state.watcher.lock() {
            *guard = None;
            *guard = watcher::start(app.clone(), &root, Arc::clone(&state.last_self_write));
        }
    }

    state
        .committer
        .configure(root.clone(), settings.git_autocommit);

    // Reconcile the autostart entry with the setting. A failure here is worth
    // reporting but must not stop the rest of startup.
    if let Err(e) = autostart::apply(settings.autostart) {
        eprintln!("yatta: autostart: {e}");
    }

    #[cfg(feature = "desktop-integration")]
    if settings.tray_enabled {
        tray::register_hotkey(app, &settings.hotkey);
    } else {
        tray::register_hotkey(app, "");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "settings lock poisoned".into())
}

#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    new_settings: Settings,
) -> Result<Settings, String> {
    {
        let mut guard = state.settings.lock().map_err(|_| "settings lock poisoned")?;
        *guard = new_settings;
        settings::save(&app, &guard)?;
    }
    apply_runtime(&app, &state)?;
    get_settings(state)
}

#[tauri::command]
fn vault_info(app: AppHandle, state: State<'_, AppState>) -> Result<VaultInfo, String> {
    let root = state.vault()?;
    Ok(VaultInfo {
        exists: root.exists(),
        is_git_repo: git::is_repo(&root),
        path: root.to_string_lossy().to_string(),
        supports_tray: cfg!(feature = "desktop-integration"),
        had_settings: settings::config_exists(&app),
    })
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    Ok(vault::list_tasks(&state.vault()?))
}

/// Create or update. An empty `path` means "new task": the vault picks a
/// filename from the title.
#[tauri::command]
fn save_task(state: State<'_, AppState>, mut task: Task) -> Result<Task, String> {
    let root = state.vault()?;
    state.mark_write();

    if task.id.trim().is_empty() {
        task.id = task::new_id();
    }
    task.tags = normalize_tags(task.tags);
    task.adopted = false;

    let is_new = task.path.trim().is_empty();
    task.path = vault::save_task(&root, &task)?;

    state.committer.request(if is_new {
        format!("add \"{}\"", task.title)
    } else {
        format!("update \"{}\"", task.title)
    });

    Ok(task)
}

#[tauri::command]
fn set_status(state: State<'_, AppState>, path: String, status: String) -> Result<Task, String> {
    let root = state.vault()?;
    let content = std::fs::read_to_string(root.join(&path))
        .map_err(|e| format!("could not read {path}: {e}"))?;

    let mut task = task::parse_task(&content, &path);
    let status = Status::parse(&status);
    vault::apply_status(&mut task, status);

    state.mark_write();
    task.path = vault::save_task(&root, &task)?;
    state
        .committer
        .request(format!("{} \"{}\"", status.as_str(), task.title));

    Ok(task)
}

#[tauri::command]
fn delete_task(state: State<'_, AppState>, path: String, title: String) -> Result<(), String> {
    let root = state.vault()?;
    state.mark_write();
    vault::delete_task(&root, &path)?;
    state.committer.request(format!("delete \"{title}\""));
    Ok(())
}

/// Bulk create, for the importer. One call, one git commit, one reload --
/// importing forty lines shouldn't mean forty round trips.
#[tauri::command]
fn create_tasks(state: State<'_, AppState>, tasks: Vec<Task>) -> Result<Vec<Task>, String> {
    let root = state.vault()?;
    state.mark_write();

    let mut created = Vec::with_capacity(tasks.len());
    for mut task in tasks {
        if task.title.trim().is_empty() {
            continue;
        }
        if task.id.trim().is_empty() {
            task.id = task::new_id();
        }
        task.tags = normalize_tags(task.tags);
        task.adopted = false;
        // Always a fresh file: the importer never overwrites existing tasks.
        task.path = String::new();
        task.path = vault::save_task(&root, &task)?;
        created.push(task);
    }

    if !created.is_empty() {
        state.committer.request(format!("import {} task(s)", created.len()));
    }
    Ok(created)
}

#[tauri::command]
fn restore_task(state: State<'_, AppState>, path: String) -> Result<Task, String> {
    let root = state.vault()?;
    state.mark_write();
    let new_path = vault::restore_task(&root, &path)?;

    let content = std::fs::read_to_string(root.join(&new_path))
        .map_err(|e| format!("could not read {new_path}: {e}"))?;
    let task = task::parse_task(&content, &new_path);
    state.committer.request(format!("restore \"{}\"", task.title));
    Ok(task)
}

#[tauri::command]
fn archive_done(state: State<'_, AppState>) -> Result<usize, String> {
    let root = state.vault()?;
    state.mark_write();
    let moved = vault::archive_done(&root)?;
    if moved > 0 {
        state.committer.request(format!("archive {moved} task(s)"));
    }
    Ok(moved)
}

/// Absolute path for a vault-relative one, so the frontend can hand it to the
/// opener plugin to reveal the file in an editor or file manager.
#[tauri::command]
fn absolute_path(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let root = state.vault()?;
    let full = root.join(&path);
    if !full.starts_with(&root) {
        return Err("path escapes the vault".into());
    }
    Ok(full.to_string_lossy().to_string())
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    tags.into_iter()
        .map(|t| t.trim().trim_start_matches('#').to_lowercase())
        .filter(|t| !t.is_empty())
        .filter(|t| seen.insert(t.clone()))
        .collect()
}

// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `mut` is only needed when the desktop-integration plugin is compiled in.
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());

    #[cfg(feature = "desktop-integration")]
    {
        builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            vault_info,
            list_tasks,
            save_task,
            create_tasks,
            restore_task,
            set_status,
            delete_task,
            archive_done,
            absolute_path,
            quick_add_done,
            app_info,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let loaded = settings::load(&handle);

            let state = AppState {
                committer: git::Committer::new(
                    PathBuf::from(&loaded.vault_path),
                    loaded.git_autocommit,
                ),
                settings: Mutex::new(loaded),
                watcher: Mutex::new(None),
                last_self_write: Arc::new(AtomicI64::new(0)),
            };
            app.manage(state);

            #[cfg(feature = "desktop-integration")]
            {
                let tray_enabled = app.state::<AppState>()
                    .settings
                    .lock()
                    .map(|s| s.tray_enabled)
                    .unwrap_or(true);
                if tray_enabled {
                    tray::setup_tray(&handle);
                }
            }

            // One scheduler for the life of the app; it re-reads settings each
            // tick, so changing reminder times needs no restart.
            reminders::start(handle.clone());

            // A vault that can't be created is worth surfacing, but the window
            // should still open so the user can point settings somewhere else.
            if let Err(e) = apply_runtime(&handle, &app.state::<AppState>()) {
                eprintln!("yatta: {e}");
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building yatta")
        .run(|app, event| {
            let tauri::RunEvent::ExitRequested { code, api, .. } = &event else { return };

            // `code` is set when something asked to quit outright -- the tray's
            // Quit item, or a signal. Only a window closing leaves it empty,
            // and only that case should be held back.
            if code.is_some() {
                return;
            }

            let keep_running = app
                .try_state::<AppState>()
                .and_then(|state| state.settings.lock().ok().map(|s| s.close_to_tray && s.tray_enabled))
                .unwrap_or(false);

            if keep_running && cfg!(feature = "desktop-integration") {
                api.prevent_exit();
            }
        });
}
