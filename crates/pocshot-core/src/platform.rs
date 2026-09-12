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
