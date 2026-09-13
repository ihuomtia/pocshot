pub mod capture;
pub mod error;
pub mod notification;
pub mod platform;
pub mod save;
pub mod types;

pub use error::{PocshotError, Result};
pub use notification::send_notification;
pub use platform::{config_dir, models_dir};
pub use types::{
    CaptureMode, CaptureOptions, CaptureResult, MonitorInfo, OutputFormat, WindowInfo,
};

pub use capture::{
    capture_rgba, capture_screen_with_monitor, list_monitors, list_windows, validate_options,
};
pub use save::{capture, copy_rgba_to_clipboard, default_output_path, resolve_format, save_rgba};
