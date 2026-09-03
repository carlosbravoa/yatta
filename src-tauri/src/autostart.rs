//! Launch-at-login. Each platform stores this somewhere entirely different, so
//! the module is split rather than pretending one mechanism fits.
//!
//! Deliberately hand-rolled rather than using tauri-plugin-autostart, because
//! on Linux the plugin writes to `~/.config/autostart`, which a confined snap
//! cannot reach: the `home` interface excludes dot-directories. snapd's user
//! session agent reads `$SNAP_USER_DATA/.config/autostart` instead, so even the
//! Linux destination depends on how the app was installed.

#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
const FILE: &str = "yatta.desktop";

/// Where the autostart entry belongs for this installation.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn exec_command() -> String {
    if std::env::var_os("SNAP").is_some() {
        return "/snap/bin/yatta".into();
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "yatta".into())
}

/// Create or remove the entry so it matches `enabled`.
#[cfg(target_os = "linux")]
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

/// Windows stores this as a value under the per-user Run key.
///
/// Driven through `reg.exe` rather than by linking a registry crate: it is a
/// known system binary, the same reasoning that keeps git a subprocess. The
/// console window is suppressed so nothing flashes on screen.
#[cfg(windows)]
pub fn apply(enabled: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "yatta";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut cmd = Command::new("reg");
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
    cmd.creation_flags(CREATE_NO_WINDOW);

    if enabled {
        let exe = std::env::current_exe()
            .map_err(|e| format!("could not find this executable: {e}"))?;
        // Quoted: the path routinely contains spaces on Windows.
        let value = format!("\"{}\"", exe.display());
        cmd.args(["add", RUN_KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &value, "/f"]);
    } else {
        cmd.args(["delete", RUN_KEY, "/v", VALUE, "/f"]);
    }

    let status = cmd.status().map_err(|e| format!("could not run reg.exe: {e}"))?;

    // Deleting a value that is not there fails, and that is the desired state
    // already -- the same tolerance the Linux branch has for a missing file.
    if status.success() || !enabled {
        Ok(())
    } else {
        Err("reg.exe could not write the autostart entry".into())
    }
}

/// macOS would need a LaunchAgent plist; not implemented yet, and saying so is
/// better than a switch that silently does nothing.
#[cfg(not(any(target_os = "linux", windows)))]
pub fn apply(_enabled: bool) -> Result<(), String> {
    Err("launching at login is not supported on this platform yet".into())
}

#[cfg(all(test, target_os = "linux"))]
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
