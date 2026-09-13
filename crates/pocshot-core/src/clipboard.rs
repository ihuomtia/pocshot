//! Clipboard image access: the "edit clipboard image" flow reads whatever
//! bitmap the system clipboard currently holds (next to the write path in
//! [`crate::save::copy_rgba_to_clipboard`]).

use image::RgbaImage;

use crate::error::{PocshotError, Result};

/// Read the image currently held on the system clipboard, if any.
///
/// `arboard` hands back straight RGBA8 bytes; they are validated before being
/// wrapped in an [`RgbaImage`] so a malformed clip reports a clean error
/// instead of panicking in the GUI.
pub fn read_clipboard_image() -> Result<RgbaImage> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| PocshotError::Clipboard(error.to_string()))?;
    let image = clipboard
        .get_image()
        .map_err(|error| PocshotError::Clipboard(error.to_string()))?;

    let (width, height) = (image.width, image.height);
    if width == 0 || height == 0 {
        return Err(PocshotError::Clipboard(
            "clipboard image is empty".to_string(),
        ));
    }
    if image.bytes.len() != width * height * 4 {
        return Err(PocshotError::Clipboard(format!(
            "clipboard image has {} bytes, expected {}",
            image.bytes.len(),
            width * height * 4
        )));
    }

    RgbaImage::from_raw(width as u32, height as u32, image.bytes.into_owned())
        .ok_or_else(|| PocshotError::Clipboard("clipboard image buffer is malformed".to_string()))
}

/// Cheap probe for whether the clipboard currently holds an image. Used by the
/// tray to decide whether to offer "Edit clipboard image".
pub fn clipboard_has_image() -> bool {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_image())
        .map(|image| image.width > 0 && image.height > 0)
        .unwrap_or(false)
}
