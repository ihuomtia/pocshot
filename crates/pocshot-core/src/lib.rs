pub mod capture;
pub mod clipboard;
pub mod error;
pub mod notification;
pub mod platform;
pub mod save;
pub mod types;

pub use error::{PocshotError, Result};
pub use notification::send_notification;
pub use platform::{
    config_dir, detach, install_panic_dialog_hook, models_dir, quiet_io, show_error_dialog,
};
pub use types::{
    CaptureMode, CaptureOptions, CaptureResult, MonitorInfo, OutputFormat, WindowInfo,
};

pub use capture::{
    capture_rgba, capture_screen_with_monitor, list_monitors, list_windows, validate_options,
};
pub use clipboard::{clipboard_has_image, read_clipboard_image};
pub use save::{capture, copy_rgba_to_clipboard, default_output_path, resolve_format, save_rgba};
