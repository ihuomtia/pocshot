//! Slint-based pocshot editor.
//!
//! Same facade as `pocshot-gui`: `run()` captures the screen, `run_edit()`
//! edits a caller-supplied image. The window is created only after the capture
//! is in hand, so the opaque fullscreen overlay is never part of the
//! screenshot.

use std::rc::Rc;

use anyhow::{Context, Result};
use image::RgbaImage;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

slint::include_modules!();

mod platform;

/// Capture the screen and open the editor on it.
pub fn run() -> Result<()> {
    let (image, monitor) = pocshot_core::capture_screen_with_monitor()
        .context("failed to capture the screen")?;
    log::info!(
        "captured {}x{} from monitor {}",
        image.width(),
        image.height(),
        monitor.name
    );
    show_editor(image)
}

/// Open the editor on an image supplied by the caller (edit-clipboard flow).
pub fn run_edit(image: RgbaImage) -> Result<()> {
    show_editor(image)
}

fn show_editor(image: RgbaImage) -> Result<()> {
    let ui = EditorWindow::new().context("failed to create the Slint window")?;

    ui.set_shot(to_slint_image(&image));
    ui.on_report(|message| log::debug!("ui: {message}"));
    ui.on_mod_ctrl(|| platform::query_modifiers().0);
    ui.on_mod_shift(|| platform::query_modifiers().1);
    ui.on_cancel(|| {
        log::info!("cancelled");
        let _ = slint::quit_event_loop();
    });

    let source = Rc::new(image);
    ui.on_commit(move |nx0, ny0, nx1, ny1| {
        log::info!("commit: normalised ({nx0},{ny0})-({nx1},{ny1})");
        match crop_normalized(&source, nx0, ny0, nx1, ny1) {
            Some(selection) => match pocshot_core::copy_rgba_to_clipboard(&selection) {
                Ok(true) => log::info!(
                    "copied {}x{} selection to clipboard",
                    selection.width(),
                    selection.height()
                ),
                Ok(false) => log::warn!("clipboard reported no image copied"),
                Err(error) => log::error!("failed to copy selection: {error}"),
            },
            None => log::warn!("empty selection, nothing copied"),
        }
        // X11 clipboard data is served by this process; without a clipboard
        // manager the selection dies with us. Keep the process alive briefly so
        // a manager (if any) can take ownership.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        });
    });

    ui.run().context("Slint event loop failed")
}

fn to_slint_image(image: &RgbaImage) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    );
    Image::from_rgba8(buffer)
}

/// Crop `image` by two normalised (0..1) corners. Returns `None` for a
/// degenerate selection.
fn crop_normalized(image: &RgbaImage, nx0: f32, ny0: f32, nx1: f32, ny1: f32) -> Option<RgbaImage> {
    let width = image.width() as f32;
    let height = image.height() as f32;
    let x0 = (nx0.min(nx1).clamp(0.0, 1.0) * width).round() as u32;
    let y0 = (ny0.min(ny1).clamp(0.0, 1.0) * height).round() as u32;
    let x1 = (nx0.max(nx1).clamp(0.0, 1.0) * width).round() as u32;
    let y1 = (ny0.max(ny1).clamp(0.0, 1.0) * height).round() as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(image::imageops::crop_imm(image, x0, y0, x1 - x0, y1 - y0).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn gradient(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| Rgba([x as u8, y as u8, 0, 255]))
    }

    #[test]
    fn crop_normalized_maps_corners_to_pixels() {
        let image = gradient(100, 50);
        let crop = crop_normalized(&image, 0.1, 0.2, 0.5, 0.8).unwrap();
        assert_eq!((crop.width(), crop.height()), (40, 30));
        assert_eq!(crop.get_pixel(0, 0), &Rgba([10, 10, 0, 255]));
    }

    #[test]
    fn crop_normalized_handles_reversed_and_clamped_corners() {
        let image = gradient(100, 50);
        // Reversed corners are normalised, overshoot is clamped.
        let crop = crop_normalized(&image, 1.5, 1.5, -0.5, -0.5).unwrap();
        assert_eq!((crop.width(), crop.height()), (100, 50));
    }

    #[test]
    fn crop_normalized_rejects_degenerate_selection() {
        let image = gradient(100, 50);
        assert!(crop_normalized(&image, 0.5, 0.5, 0.5, 0.5).is_none());
        assert!(crop_normalized(&image, 0.0, 0.0, 0.0, 1.0).is_none());
    }
}
