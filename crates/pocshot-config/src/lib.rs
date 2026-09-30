//! Persistent user settings and theming, shared by the egui and Slint UIs.
//!
//! Extracted from `pocshot-gui` so both UIs read and write the same
//! `config.json` and the Slint build gets real theming. `Theme`/`AppSettings`
//! only depend on `ecolor` and `pocshot-core`, so nothing egui-specific leaks
//! into this crate.

pub mod config;
pub mod theme;

pub use config::AppSettings;
pub use theme::{color32_from_hex, color32_to_hex, Theme};
