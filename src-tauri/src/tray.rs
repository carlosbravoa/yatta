//! Tray icon and global hotkey. Compiled only with the `desktop-integration`
//! feature, and every step degrades to a warning rather than an error: a
//! missing tray daemon or a compositor that refuses global shortcuts must not
//! stop the app from opening.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub fn focus_main(app: &AppHandle) {
    crate::show_main(app);
}

pub fn setup_tray(app: &AppHandle) {
    if let Err(e) = build_tray(app) {
        eprintln!("yatta: tray unavailable ({e}); continuing without it");
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open yatta", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "quickadd", "Quick add task…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let about = MenuItem::with_id(app, "about", "About yatta", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &add, &separator, &about, &quit])?;

    let mut builder = TrayIconBuilder::with_id("yatta-tray")
        .menu(&menu)
        .tooltip("yatta")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => focus_main(app),
            // A real popup, not "open the app and hope it comes forward".
            "quickadd" => crate::open_quick_add(app),
            "about" => crate::open_about(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                focus_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Register the quick-add hotkey, replacing any previously registered one.
///
/// Global shortcuts are an X11 feature; under a Wayland session the compositor
/// owns them and registration will fail. That's reported and then ignored.
pub fn register_hotkey(app: &AppHandle, accelerator: &str) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();

    if accelerator.trim().is_empty() {
        return;
    }

    let handle = app.clone();
    let result = shortcuts.on_shortcut(accelerator, move |_app, _shortcut, event| {
        // Fire on press only; otherwise the window toggles twice per keypress.
        if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            crate::open_quick_add(&handle);
        }
    });

    if let Err(e) = result {
        eprintln!(
            "yatta: could not register the global hotkey '{accelerator}' ({e}). \
             Wayland sessions reserve global shortcuts for the compositor; \
             bind one there to `yatta` instead."
        );
    }
}
