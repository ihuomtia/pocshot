//! Lightweight, persistent application settings.
//!
//! The app historically kept all state in memory. This module introduces a
//! single JSON file on disk (`$XDG_CONFIG_HOME/pocshot/config.json`, falling
//! back to `~/.config`) so the machine-level "on/off" knobs for excess
//! features (OCR, snap display) survive restarts without a second settings
//! store or a config framework.
//!
//! The actual directory is resolved by `pocshot_core::config_dir`, which
//! follows the platform convention (APPDATA on Windows, `~/Library/Application
//! Support` on macOS, XDG on Linux).

use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The subset of preferences that make sense to persist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub ocr_enabled: bool,
    pub show_text_boxes: bool,
    pub show_ocr_debug: bool,
    pub ocr_confidence: f32,
    /// Directory holding the ocrs `.rten` models (text-detection.rten +
    /// text-recognition.rten). Empty means the default location
    /// (`$CONFIG_DIR/models`); models are auto-downloaded on first use.
    #[serde(default)]
    pub ocr_models_dir: String,
    pub snap_enabled: bool,
    pub show_snap_lines: bool,
    /// Slint UI / shared: copy the selection to the clipboard as well when
    /// saving. Absent in older configs -> false.
    #[serde(default)]
    pub copy_on_save: bool,
    /// Stroke width for new annotations (shared by both UIs).
    #[serde(default = "default_stroke_width")]
    pub annotation_stroke_width: f32,
    /// Debug: restrict OCR text detection to the currently-selected region
    /// rather than the whole capture. Results are still shown at full-capture
    /// coordinates.
    #[serde(default)]
    pub ocr_region_only: bool,
    /// Visual theme (colors, font sizes, geometry). Absent in legacy configs
    /// -> built-in look, which equals the app's original hardcoded values.
    #[serde(default = "Theme::builtin")]
    pub theme: Theme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ocr_enabled: false,
            show_text_boxes: false,
            show_ocr_debug: false,
            ocr_confidence: 0.5,
            ocr_models_dir: String::new(),
            snap_enabled: true,
            show_snap_lines: true,
            copy_on_save: false,
            annotation_stroke_width: default_stroke_width(),
            ocr_region_only: false,
            theme: Theme::builtin(),
        }
    }
}

fn default_stroke_width() -> f32 {
    3.0
}

/// Directory used for config and (optionally) the bundled model file.
pub fn config_dir() -> PathBuf {
    pocshot_core::config_dir()
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// Resolve the OCR models directory: an explicit override first, otherwise the
/// conventional `$CONFIG_DIR/models` location (models auto-download there).
pub fn ocr_models_dir(configured: &str) -> PathBuf {
    pocshot_core::models_dir(Some(configured))
}

/// Load persisted settings, falling back to defaults when the file is missing
/// or corrupt. The app degrades gracefully (feature just starts disabled).
///
/// Loading is also the migration/ensure path: when no config exists the file
/// is created, and when an older config without the `theme` field is read the
/// merged defaults are written back — so a fresh run always leaves a config on
/// disk that carries the current schema (including `theme`).
pub fn load() -> AppSettings {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        // No config file yet: materialize the defaults so the file exists and
        // carries the full schema on first run.
        let settings = AppSettings::default();
        save(&settings);
        return settings;
    };
    match serde_json::from_str::<AppSettings>(&text) {
        Ok(s) => {
            // Detect a pre-theme config (valid JSON but no `theme` key) and
            // write the de-serialized defaults back so the attribute is
            // persisted rather than living only in this process.
            let value = serde_json::from_str::<serde_json::Value>(&text).unwrap_or_default();
            let has_theme = value
                .as_object()
                .map(|o| o.contains_key("theme"))
                .unwrap_or(false);
            if !has_theme {
                save(&s);
            }
            s
        }
        Err(e) => {
            log::warn!("ignoring corrupt config {}: {e}", path.display());
            AppSettings::default()
        }
    }
}

/// Persist settings to disk. Failures are logged but never fatal — the app
/// keeps running with the in-memory values.
pub fn save(settings: &AppSettings) {
    let dir = config_dir();
    if std::fs::create_dir_all(&dir).is_err() && !dir.is_dir() {
        return;
    }
    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            let path = config_path();
            if std::fs::write(&path, json).is_err() {
                log::warn!("failed to write config to {}", path.display());
            }
        }
        Err(e) => log::warn!("failed to serialize config: {e}"),
    }
}

/// Whether a detection run is warranted at all. Central gate used to keep the
/// app inert when OCR is disabled or there is nothing to process yet — also
/// the unit-testable oracle for "disabled OCR does nothing".
pub fn should_run_detection(ocr_enabled: bool, has_capture: bool) -> bool {
    ocr_enabled && has_capture
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env vars are process-global; serialize env-mutating tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // Point the platform's config-dir env var at `dir` so the tests never touch
    // the developer's real config.
    fn with_tmp_config(dir: &std::path::Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap();
        let (key, prev) = set_config_env(dir);
        f();
        match prev {
            Some(p) => unsafe { std::env::set_var(&key, p) },
            None => unsafe { std::env::remove_var(&key) },
        }
    }

    #[cfg(target_os = "windows")]
    fn set_config_env(dir: &std::path::Path) -> (String, Option<std::ffi::OsString>) {
        let key = "APPDATA".to_string();
        let prev = std::env::var_os(&key);
        unsafe { std::env::set_var(&key, dir) };
        (key, prev)
    }

    #[cfg(target_os = "linux")]
    fn set_config_env(dir: &std::path::Path) -> (String, Option<std::ffi::OsString>) {
        let key = "XDG_CONFIG_HOME".to_string();
        let prev = std::env::var_os(&key);
        unsafe { std::env::set_var(&key, dir) };
        (key, prev)
    }

    #[cfg(target_os = "macos")]
    fn set_config_env(dir: &std::path::Path) -> (String, Option<std::ffi::OsString>) {
        let key = "HOME".to_string();
        let prev = std::env::var_os(&key);
        unsafe { std::env::set_var(&key, dir) };
        (key, prev)
    }

    #[test]
    fn roundtrip_save_and_load() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-test-{}", std::process::id()));
        with_tmp_config(&dir, || {
            let mut s = AppSettings::default();
            s.ocr_enabled = true;
            s.show_text_boxes = true;
            s.ocr_confidence = 0.7;
            s.ocr_models_dir = std::env::temp_dir()
                .join("models")
                .to_string_lossy()
                .into_owned();
            s.snap_enabled = false;
            save(&s);

            let loaded = load();
            assert_eq!(loaded, s);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_defaults() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-missing-{}", std::process::id()));
        with_tmp_config(&dir, || {
            assert_eq!(load(), AppSettings::default());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_config_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-corrupt-{}", std::process::id()));
        with_tmp_config(&dir, || {
            std::fs::create_dir_all(config_dir()).unwrap();
            std::fs::write(config_path(), "{ not json").unwrap();
            assert_eq!(load(), AppSettings::default());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn models_dir_resolution() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-path-{}", std::process::id()));
        with_tmp_config(&dir, || {
            let over = std::env::temp_dir()
                .join("custom-models")
                .to_string_lossy()
                .into_owned();
            assert_eq!(ocr_models_dir(&over), PathBuf::from(&over));
            assert_eq!(ocr_models_dir(""), config_dir().join("models"));
        });
    }

    #[test]
    fn legacy_config_without_models_dir_defaults() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-preset-{}", std::process::id()));
        with_tmp_config(&dir, || {
            std::fs::create_dir_all(config_dir()).unwrap();
            // Old config files (written before ocr_models_dir existed) must keep
            // loading instead of being discarded as corrupt.
            let legacy = r#"{
                "ocr_enabled": true,
                "show_text_boxes": true,
                "show_ocr_debug": true,
                "ocr_confidence": 0.5,
                "snap_enabled": true,
                "show_snap_lines": true
            }"#;
            std::fs::write(config_path(), legacy).unwrap();
            let s = load();
            assert!(s.ocr_enabled);
            assert_eq!(s.ocr_models_dir, String::new());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detection_gate_requires_enabled_and_capture() {
        assert!(!should_run_detection(false, true));
        assert!(!should_run_detection(false, false));
        assert!(!should_run_detection(true, false));
        assert!(should_run_detection(true, true));
    }

    #[test]
    fn legacy_config_without_theme_uses_builtin_and_is_migrated() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-theme-{}", std::process::id()));
        with_tmp_config(&dir, || {
            let legacy = r#"{
                "ocr_enabled": true,
                "show_text_boxes": false,
                "show_ocr_debug": false,
                "ocr_confidence": 0.5,
                "ocr_model_path": "",
                "snap_enabled": true,
                "show_snap_lines": true
            }"#;
            std::fs::create_dir_all(config_dir()).unwrap();
            std::fs::write(config_path(), legacy).unwrap();
            let s = load();
            // No `theme` key -> must resolve to the built-in theme, not panic
            // or zero-fill.
            assert_eq!(s.theme, crate::theme::Theme::builtin());
            // Loading migrated the on-disk file to include the `theme` key.
            let raw = std::fs::read_to_string(config_path()).unwrap();
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(
                value.get("theme").is_some(),
                "expected migrated config to carry a `theme` key"
            );
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_creates_missing_config_with_theme() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-create-{}", std::process::id()));
        with_tmp_config(&dir, || {
            assert!(!config_path().exists());
            let s = load();
            assert_eq!(s, AppSettings::default());
            // The file now exists and includes the `theme` attribute.
            assert!(config_path().exists());
            let raw = std::fs::read_to_string(config_path()).unwrap();
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(
                value.get("theme").is_some(),
                "freshly-created config must carry a `theme` key"
            );
            // And it round-trips.
            assert_eq!(load(), s);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ocr_region_only_defaults_off_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-ocri-{}", std::process::id()));
        with_tmp_config(&dir, || {
            // Fresh configs default the region-only debug flag to off.
            assert!(!AppSettings::default().ocr_region_only);
            let mut s = AppSettings::default();
            s.ocr_region_only = true;
            save(&s);
            assert!(load().ocr_region_only);

            // A legacy config (pre-flag) still loads with the flag off.
            let legacy = r#"{
                "ocr_enabled": true,
                "show_text_boxes": false,
                "show_ocr_debug": false,
                "ocr_confidence": 0.5,
                "ocr_model_path": "",
                "snap_enabled": true,
                "show_snap_lines": true
            }"#;
            std::fs::create_dir_all(config_dir()).unwrap();
            std::fs::write(config_path(), legacy).unwrap();
            assert!(!load().ocr_region_only);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn theme_roundtrips_through_config() {
        let dir = std::env::temp_dir().join(format!("pocshot-cfg-theme2-{}", std::process::id()));
        with_tmp_config(&dir, || {
            let mut s = AppSettings::default();
            s.theme.colors.accent = crate::theme::color32_from_hex(
                "#00ff00",
                crate::theme::Theme::builtin().colors.accent,
            );
            save(&s);
            let loaded = load();
            assert_eq!(loaded.theme.colors.accent, s.theme.colors.accent);
            assert_eq!(loaded, s);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
