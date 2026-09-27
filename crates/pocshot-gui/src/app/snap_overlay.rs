//! Baked snap-guide overlay: one full-image texture rebuilt per capture,
//! drawn as a single quad each frame instead of N tessellated line segments.

use eframe::egui::{self, Color32, Pos2, Rect, TextureOptions};
use image::RgbaImage;

use crate::app::PocshotApp;

/// Bake snap guides into a transparent overlay at capture resolution,
/// one straight 1px line per guide (unmultiplied alpha). Pure function.
/// Stub for now — the real body arrives in Step 3.
pub(crate) fn bake_snap_overlay(
    width: u32,
    height: u32,
    horiz: &[f32],
    vert: &[f32],
    color: Color32,
) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);
    let [r, g, b, a] = color.to_array(); // unmultiplied source for ColorImage
    let px = image::Rgba([r, g, b, a]);
    if width > 0 && height > 0 {
        for &y in horiz {
            let y = y.round().clamp(0.0, (height - 1) as f32) as u32;
            for x in 0..width {
                img.put_pixel(x, y, px);
            }
        }
        for &x in vert {
            let x = x.round().clamp(0.0, (width - 1) as f32) as u32;
            for y in 0..height {
                img.put_pixel(x, y, px);
            }
        }
    }
    img
}

impl PocshotApp {
    /// Rebuild the baked snap overlay iff any input changed. One full-image
    /// alloc + one texture upload per capture (not per frame). `pub(crate)`
    /// because `update()` in the parent module (`app/mod.rs`) calls it.
    pub(crate) fn refresh_snap_overlay(&mut self, ctx: &egui::Context) {
        let Some(capture) = &self.capture else { return };
        let key: super::SnapOverlayKey = (
            self.capture_version,
            self.snap_lines_generation,
            self.ocr_ready_version,
            self.ocr_enabled,
            self.show_snap_lines,
            self.theme_generation,
        );
        if key == self.snap_overlay_key && self.snap_overlay.is_some() {
            return;
        }
        if !self.show_snap_lines {
            self.snap_overlay = None;
            self.snap_overlay_key = key;
            return;
        }
        let (horiz, vert) = self.snap_guide_lines();
        let baked = bake_snap_overlay(
            capture.width(),
            capture.height(),
            &horiz,
            &vert,
            self.theme.colors.snap_line,
        );
        let size = [capture.width() as usize, capture.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, baked.as_raw());
        self.snap_overlay =
            Some(ctx.load_texture("snap-overlay", color_image, TextureOptions::LINEAR));
        self.snap_overlay_key = key;
    }

    /// Draw the baked overlay: one textured quad over the whole image area.
    pub(crate) fn draw_snap_overlay(&self, painter: &egui::Painter, draw_rect: Rect) {
        if let Some(tex) = &self.snap_overlay {
            painter.image(
                tex.id(),
                draw_rect,
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bakes_rows_and_columns() {
        let img = bake_snap_overlay(50, 30, &[10.0], &[5.0], Color32::from_rgb(255, 0, 0));
        assert_eq!(*img.get_pixel(0, 10), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(49, 10), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(5, 0), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(5, 29), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(3, 3), image::Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn clamps_out_of_bounds_guides() {
        let img = bake_snap_overlay(10, 10, &[-5.0, 99.0], &[], Color32::from_rgb(0, 255, 0));
        assert_eq!(*img.get_pixel(0, 0), image::Rgba([0, 255, 0, 255]));
        assert_eq!(*img.get_pixel(9, 9), image::Rgba([0, 255, 0, 255]));
        assert_eq!(*img.get_pixel(4, 4), image::Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn empty_image_is_noop() {
        let img = bake_snap_overlay(0, 0, &[1.0], &[1.0], Color32::from_rgb(1, 2, 3));
        assert_eq!(img.dimensions(), (0, 0));
    }
}
