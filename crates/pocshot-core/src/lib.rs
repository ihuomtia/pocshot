pub mod capture;
pub mod error;
pub mod notification;
pub mod save;
pub mod types;

pub use error::{PocshotError, Result};
pub use notification::send_notification;
pub use types::{
    CaptureMode, CaptureOptions, CaptureResult, MonitorInfo, OutputFormat, WindowInfo,
};

pub use capture::{capture_rgba, list_monitors, list_windows, validate_options};
pub use save::{capture, copy_rgba_to_clipboard, default_output_path, resolve_format, save_rgba};
