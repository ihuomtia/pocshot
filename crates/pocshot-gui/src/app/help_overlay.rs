//! Baked help overlay: the static shortcut panel is rasterized once into a
//! texture and drawn as a single quad each frame, instead of laying out ~46
//! text runs every frame while it is visible.
//!
//! Drawing uses the same pure-raster helpers the annotation tools already use
//! to stamp shapes and text onto an `RgbaImage` (`annotation::raster`,
//! `annotation::counter_font`), so the panel needs no egui layout at all.

use eframe::egui::{Color32, Pos2, Rect, TextureOptions, Vec2};
use image::RgbaImage;

use crate::annotation::{blend_pixel, draw_text_on_image, fill_rect_on_image, measure_text};
use crate::app::PocshotApp;
use crate::theme::Theme;

/// The rows shown in the help panel: `(shortcut, description)`.
pub(crate) const HELP_ROWS: [(&str, &str); 22] = [
    ("Drag", "select region"),
    ("Enter / Space", "capture"),
    ("Esc", "cancel / close"),
    ("Cmd+S", "save"),
    ("Cmd+C", "copy"),
    ("Cmd+R", "recapture"),
    ("V", "Select"),
    ("A", "Arrow"),
    ("L", "Line"),
    ("R", "Rect"),
    ("C", "Circle"),
    ("P", "Pen"),
    ("H", "Highlight"),
    ("N", "Number bubble"),
    ("T", "Text border"),
    ("D", "Redact"),
    ("M", "Pixelate"),
    ("B", "Blur"),
    ("E", "Eraser"),
    ("Shift+R", "Filled rect"),
    ("Shift+C", "Filled circle"),
    ("Ctrl+drag", "constrain shape / angle"),
];

const LINE_H: f32 = 20.0;
const TEXT_W: f32 = 260.0;
const BOX_W: f32 = TEXT_W + 60.0;

/// Panel size in logical points, matching the previous on-screen geometry
/// (`box_h + line_h` where `box_h = 40 + rows * line_h + 20`).
pub(crate) fn help_box_size() -> Vec2 {
    let box_h = 40.0 + HELP_ROWS.len() as f32 * LINE_H + 20.0;
    Vec2::new(BOX_W, box_h + LINE_H)
}

/// Pixels-per-point used for the bake. Clamped so an extreme DPI cannot
/// allocate an absurd texture for a small panel.
fn bake_scale(pixels_per_point: f32) -> f32 {
    pixels_per_point.clamp(1.0, 2.0)
}

/// Draw a 1px-stroked axis-aligned rect (the panel border). The panel's corner
/// radius is small enough that a square border is visually equivalent, and
/// this avoids inventing a rounded-rect rasterizer.
fn stroke_rect_on_image(image: &mut RgbaImage, rect: Rect, color: Color32) {
    let x0 = rect.min.x.round() as i32;
    let y0 = rect.min.y.round() as i32;
    let x1 = rect.max.x.round() as i32;
    let y1 = rect.max.y.round() as i32;
    let rgba = image::Rgba([color.r(), color.g(), color.b(), color.a()]);
    let (w, h) = (image.width() as i32, image.height() as i32);
    for x in x0..=x1 {
        for y in [y0, y1] {
            if x >= 0 && y >= 0 && x < w && y < h {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
    for y in y0..=y1 {
        for x in [x0, x1] {
            if x >= 0 && y >= 0 && x < w && y < h {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
}

/// Rasterize the whole help panel to a fresh RGBA image. Pure function: same
/// inputs, same pixels — unit-testable without a window.
///
/// `scale` is the pixels-per-point of the target texture; all geometry is
/// authored in points and multiplied by it.
pub(crate) fn bake_help_panel(theme: &Theme, scale: f32) -> RgbaImage {
    let size = help_box_size();
    let px_w = (size.x * scale).ceil().max(1.0) as u32;
    let px_h = (size.y * scale).ceil().max(1.0) as u32;
    let mut img = RgbaImage::new(px_w, px_h);

    let to_px = |p: Pos2| Pos2::new(p.x * scale, p.y * scale);
    let box_rect = Rect::from_min_size(Pos2::ZERO, size);
    let box_rect_px = Rect::from_min_max(to_px(box_rect.min), to_px(box_rect.max));

    // Backdrop: same translucent near-black as the on-screen panel.
    fill_rect_on_image(
        &mut img,
        box_rect_px.min,
        box_rect_px.max,
        Color32::from_rgba_unmultiplied(10, 10, 14, 200),
    );
    stroke_rect_on_image(&mut img, box_rect_px, theme.colors.box_border);

    let center_x = box_rect.center().x;
    let mut y = box_rect.min.y + 16.0;

    draw_text_on_image(
        &mut img,
        to_px(Pos2::new(center_x, y)),
        "Shortcuts",
        theme.fonts.help_title * scale,
        theme.colors.text_primary,
        Color32::TRANSPARENT,
    );
    y += LINE_H + 6.0;

    for (shortcut, desc) in HELP_ROWS {
        draw_text_on_image(
            &mut img,
            to_px(Pos2::new(box_rect.min.x + 30.0, y)),
            shortcut,
            theme.fonts.help_row * scale,
            theme.colors.text_primary,
            Color32::TRANSPARENT,
        );
        // Right-aligned: offset by the measured width.
        let (w, _) = measure_text(desc, theme.fonts.help_row * scale);
        draw_text_on_image(
            &mut img,
            to_px(Pos2::new(box_rect.max.x - 30.0 - w, y)),
            desc,
            theme.fonts.help_row * scale,
            theme.colors.text_muted,
            Color32::TRANSPARENT,
        );
        y += LINE_H;
    }

    draw_text_on_image(
        &mut img,
        to_px(Pos2::new(center_x, box_rect.max.y - 14.0)),
        "Select a region to dismiss",
        theme.fonts.help_footer * scale,
        Color32::from_rgba_unmultiplied(180, 180, 180, 220),
        Color32::TRANSPARENT,
    );

    img
}

impl PocshotApp {
    /// Rebuild the cached help texture when the theme generation or the DPI
    /// scale changes. `pub(crate)` because `update()` calls it.
    pub(crate) fn refresh_help_overlay(&mut self, ctx: &eframe::egui::Context) {
        let scale_bits = bake_scale(ctx.pixels_per_point()).to_bits() as u64;
        let key = self
            .theme_generation
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ scale_bits;
        if self.help_overlay_key == Some(key) && self.help_overlay.is_some() {
            return;
        }
        let scale = bake_scale(ctx.pixels_per_point());
        let baked = bake_help_panel(&self.theme, scale);
        let size = [baked.width() as usize, baked.height() as usize];
        let color_image = eframe::egui::ColorImage::from_rgba_unmultiplied(size, baked.as_raw());
        self.help_overlay = Some(ctx.load_texture(
            "help-overlay",
            color_image,
            TextureOptions::LINEAR,
        ));
        self.help_overlay_key = Some(key);
    }

    /// Draw the cached help panel as one quad, and return its screen rect for
    /// the toolbar hit-testing.
    pub(crate) fn draw_help_overlay(
        &self,
        painter: &eframe::egui::Painter,
        draw_rect: Rect,
    ) -> Rect {
        let box_rect = Rect::from_center_size(draw_rect.center(), help_box_size());
        if let Some(tex) = &self.help_overlay {
            painter.image(
                tex.id(),
                box_rect,
                Rect::from_min_max(Pos2::ZERO, eframe::egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        box_rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn panel_size_matches_previous_geometry() {
        let size = help_box_size();
        // box_w = text_w + 60; box_h(+line) = 40 + 22*20 + 20 + 20
        assert_eq!(size.x, 320.0);
        assert_eq!(size.y, 520.0);
    }

    #[test]
    fn bake_draws_backdrop_and_text() {
        let theme = Theme::default();
        let img = bake_help_panel(&theme, 1.0);
        assert_eq!(img.dimensions(), (320, 520));
        // Interior of the backdrop is painted (translucent near-black under the
        // panel), and the panel is not fully transparent anywhere inside.
        let mid = img.get_pixel(160, 260);
        assert!(mid.0[3] > 0, "panel interior should be painted");
        // Some pixels differ from the plain backdrop => text was stamped.
        let mut text_px = 0u32;
        for y in 40..100 {
            for x in 20..300 {
                let p = img.get_pixel(x, y);
                if p.0[0] > 60 || p.0[1] > 60 || p.0[2] > 60 {
                    text_px += 1;
                }
            }
        }
        assert!(text_px > 0, "expected stamped title pixels, got {text_px}");
    }

    #[test]
    fn bake_scales_with_scale_factor() {
        let theme = Theme::default();
        let img = bake_help_panel(&theme, 2.0);
        assert_eq!(img.dimensions(), (640, 1040));
    }
}
