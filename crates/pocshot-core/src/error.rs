#[derive(Debug, thiserror::Error)]
pub enum PocshotError {
    #[error("capture failed: {0}")]
    Capture(String),
    #[error("clipboard failed: {0}")]
    Clipboard(String),
    #[error("invalid options: {0}")]
    InvalidOptions(String),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("image encoding failed: {0}")]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, PocshotError>;

pub fn capture_err(error: impl std::fmt::Display) -> PocshotError {
    let message = error.to_string();
    if cfg!(target_os = "macos") && message.to_ascii_lowercase().contains("permission") {
        return PocshotError::Capture(
            "screen capture permission is required in macOS System Settings".to_string(),
        );
    }

    if cfg!(target_os = "linux") && std::env::var("WAYLAND_DISPLAY").is_ok() {
        return PocshotError::Capture(format!(
            "{message}. Linux Wayland capture support depends on compositor and portal availability"
        ));
    }

    PocshotError::Capture(message)
}
