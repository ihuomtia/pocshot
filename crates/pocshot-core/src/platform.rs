//! Platform conventions: the directory Pocshot uses for config and OCR models.
//!
//! Kept in core so the GUI, CLI, and any future front-end agree on where
//! persistent state lives instead of re-implementing the lookup independently
//! (and drifting). Resolutions follow each OS's normal convention:
//!
//! - Windows: `%APPDATA%/pocshot` (with the standard fallbacks below)
//! - macOS: `$HOME/Library/Application Support/pocshot`
//! - Linux/other Unix: `$XDG_CONFIG_HOME/pocshot`, else `$HOME/.config/pocshot`

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Base directory for Pocshot config and (optionally) models.
pub fn config_dir() -> PathBuf {
    app_data_dir().join("pocshot")
}

/// OS-native app-data root for the current user, independent of CWD, so config
/// survives an install anywhere. Falls back to a CWD-relative directory rather
/// than erroring when the OS can't tell us where user data belongs.
fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // Prefer the roaming APPDATA, then LOCALAPPDATA, then the user profile
        // directory. Windows always provides at least USERPROFILE.
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(target_os = "macos")]
    {
        // $HOME/Library/Application Support. macOS always sets HOME.
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Directory holding the ocrs `.rten` models. No override -> `config_dir/models`.
pub fn models_dir(configured: Option<&str>) -> PathBuf {
    match configured.map(str::trim).filter(|s| !s.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => config_dir().join("models"),
    }
}

/// Make a spawned child run detached from this process (its own process group
/// on Unix, detached-process flags on Windows) so it outlives the parent.
pub fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
}

/// Discard a child's stdio (it may not have a console).
pub fn quiet_io(cmd: &mut Command) {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
}

/// Show a fatal error to the user.
///
/// On Windows the GUI is built without a console (release), so a raw
/// `anyhow`/panic message on stderr is invisible and the app appears to exit
/// silently. A native message box guarantees the user sees *something* even
/// when the windowing/GPU stack failed before any window existed.
///
/// On other platforms (where the app always has a terminal or a system log)
/// this just prints to stderr.
pub fn show_error_dialog(message: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let title: Vec<u16> = std::ffi::OsStr::new("Pocshot")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let text: Vec<u16> = std::ffi::OsStr::new(message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        const MB_OK: u32 = 0x0000_0000;
        const MB_ICONERROR: u32 = 0x0000_0010;
        const MB_SETFOREGROUND: u32 = 0x0001_0000;

        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(
                hwnd: *mut core::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                u_type: u32,
            ) -> i32;
        }

        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("{message}");
    }
}

/// Route panics through [`show_error_dialog`] on Windows so a crash during
/// startup (GPU init, window creation) is not silent. Other platforms keep the
/// default hook.
pub fn install_panic_dialog_hook() {
    #[cfg(target_os = "windows")]
    {
        std::panic::set_hook(Box::new(|info| {
            let message = format!("Pocshot crashed:\n\n{info}");
            log::error!("{message}");
            show_error_dialog(&message);
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_always_absolute_or_dot() {
        // It must never panic, and must resolve under a known user dir when the
        // env has one. On Windows/Linux/macOS CI (whichever this runs on) one
        // of these is set.
        let dir = config_dir();
        assert!(dir.has_root() || dir == PathBuf::from("."));
    }

    #[test]
    fn models_dir_defaults_under_config() {
        assert_eq!(models_dir(None), config_dir().join("models"));
        assert_eq!(models_dir(Some("")), config_dir().join("models"));
        assert_eq!(models_dir(Some("   ")), config_dir().join("models"));
    }

    #[test]
    fn models_dir_honors_override() {
        assert_eq!(models_dir(Some("/tmp/m")), PathBuf::from("/tmp/m"));
    }
}
