//! Launch-at-login, written as an XDG autostart entry.
//!
//! Deliberately hand-rolled rather than using tauri-plugin-autostart, because
//! the plugin writes to `~/.config/autostart`, which a confined snap cannot
//! reach: the `home` interface excludes dot-directories. snapd's user session
//! agent instead starts entries from `$SNAP_USER_DATA/.config/autostart`, so
//! the correct destination depends on how the app was installed.

use std::path::PathBuf;

const FILE: &str = "yatta.desktop";

/// Where the autostart entry belongs for this installation.
fn autostart_dir() -> Option<PathBuf> {
    // Inside a snap, $SNAP_USER_DATA is the writable per-user root that
    // snapd's session agent scans.
    if let Some(snap_data) = std::env::var_os("SNAP_USER_DATA") {
        return Some(PathBuf::from(snap_data).join(".config/autostart"));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart"))
}

/// The command the entry should run. Inside a snap the wrapper on PATH is the
/// only sane choice; elsewhere it is this binary.
fn exec_command() -> String {
    if std::env::var_os("SNAP").is_some() {
        return "/snap/bin/yatta".into();
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "yatta".into())
}

/// Create or remove the entry so it matches `enabled`.
pub fn apply(enabled: bool) -> Result<(), String> {
    let Some(dir) = autostart_dir() else {
        return Err("could not work out where autostart entries belong".into());
    };
    let path = dir.join(FILE);

    if !enabled {
        // Absent is the desired state; a missing file is success, not failure.
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("could not remove {}: {e}", path.display())),
        };
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=yatta\n\
         Comment=Yet Another Text-based TODO App\n\
         Exec={}\n\
         Icon=yatta\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exec_command()
    );
    std::fs::write(&path, entry).map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_the_snap_location_when_confined() {
        // Not asserting on the live environment: just that the snap variable is
        // what decides, since getting this wrong means autostart silently fails
        // under confinement.
        let snap_data = std::env::var_os("SNAP_USER_DATA");
        let dir = autostart_dir().expect("a location is always derivable");
        match snap_data {
            Some(root) => assert!(dir.starts_with(PathBuf::from(root))),
            None => assert!(dir.ends_with("autostart")),
        }
    }

    #[test]
    fn disabling_a_missing_entry_is_not_an_error() {
        // apply(false) runs the remove path; with no entry present it must
        // still report success rather than surfacing a NotFound to the user.
        assert!(apply(false).is_ok());
    }
}
