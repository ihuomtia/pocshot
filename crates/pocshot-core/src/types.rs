use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Png,
    Jpg,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpg => "jpg",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum CaptureMode {
    Screen {
        monitor_id: Option<u32>,
    },
    Region {
        monitor_id: Option<u32>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Window {
        window_id: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureOptions {
    pub mode: CaptureMode,
    pub output: Option<PathBuf>,
    pub format: Option<OutputFormat>,
    pub quality: u8,
    pub delay_ms: u64,
    pub clipboard: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            mode: CaptureMode::Screen { monitor_id: None },
            output: None,
            format: None,
            quality: 90,
            delay_ms: 0,
            clipboard: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: OutputFormat,
    pub clipboard_copied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub friendly_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u32,
    pub pid: u32,
    pub app_name: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
    pub is_focused: bool,
}
