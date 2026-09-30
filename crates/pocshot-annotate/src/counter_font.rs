//! Text stamping for annotations: rasterizes text onto an image using the
//! embedded light font. The Counter bubble is one use; general text
//! annotations share the same machinery. Glyphs are "bolded"/haloed by
//! stamping each outline at a few sub-pixel offsets.

use ecolor::Color32;
use emath::Pos2;
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;

use crate::raster::blend_pixel;

/// Foreground color that contrasts with the (possibly light) bubble fill.
pub fn contrast_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}

/// Outline color that makes `text` read over arbitrary screenshot content:
/// dark halo for bright text, white halo for dark text.
pub fn halo_color(text: Color32) -> Color32 {
    let lum = 0.299 * text.r() as f32 + 0.587 * text.g() as f32 + 0.114 * text.b() as f32;
    if lum > 140.0 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}

/// Sub-pixel offsets used to fake-bold / outline glyphs.
pub const SAMPLE_OFFSETS: [(f32, f32); 5] = [
    (0.0, 0.0),
    (0.7, 0.7),
    (-0.7, 0.7),
    (0.7, -0.7),
    (-0.7, -0.7),
];

/// Rasterize `text` as multiple left-aligned lines starting at `anchor`, with
/// a contrasting halo and the main color. Used by text annotations.
pub fn draw_text_on_image(
    image: &mut RgbaImage,
    anchor: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    halo: Color32,
) {
    use ab_glyph::{Font, Glyph, ScaleFont};
    if text.is_empty() {
        return;
    }
    let font = bundled_font();
    let scale = ab_glyph::PxScale::from(size.max(1.0));
    let scaled = font.as_scaled(scale);
    let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();

    for (li, line) in text.split('\n').enumerate() {
        let baseline_y = scaled.ascent();
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut pen_x = 0.0f32;
        for c in line.chars() {
            let id = font.glyph_id(c);
            glyphs.push(id.with_scale_and_position(scale, ab_glyph::point(pen_x, baseline_y)));
            pen_x += scaled.h_advance(id);
        }
        let ox = anchor.x;
        let oy = anchor.y + li as f32 * line_height;
        // Halo first (underneath), then the main color on top.
        stamp_glyphs(image, scaled, &glyphs, ox, oy, halo);
        stamp_glyphs(image, scaled, &glyphs, ox, oy, color);
    }
}

/// Measure the bounding box `(width, height)` in pixels that
/// [`draw_text_on_image`] would produce for `text` at `size`.
pub fn measure_text(text: &str, size: f32) -> (f32, f32) {
    use ab_glyph::{Font, ScaleFont};
    let font = bundled_font();
    let scale = ab_glyph::PxScale::from(size.max(1.0));
    let scaled = font.as_scaled(scale);
    let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
    let mut max_w = 0.0f32;
    let mut lines = 0usize;
    for line in text.split('\n') {
        lines += 1;
        let mut w = 0.0f32;
        for c in line.chars() {
            w += scaled.h_advance(font.glyph_id(c));
        }
        max_w = max_w.max(w);
    }
    (max_w, line_height * lines as f32)
}

/// Stamp every glyph at `ox,oy` (after translating) using the 5-offset
/// sampling, blending each sample pixel with `color`.
fn stamp_glyphs(
    image: &mut RgbaImage,
    scaled: impl ab_glyph::ScaleFont<&'static ab_glyph::FontArc>,
    glyphs: &[ab_glyph::Glyph],
    ox: f32,
    oy: f32,
    color: Color32,
) {
    for mut g in glyphs.iter().cloned() {
        g.position.x += ox;
        g.position.y += oy;
        for (sx, sy) in SAMPLE_OFFSETS {
            let mut shifted = g.clone();
            shifted.position.x += sx;
            shifted.position.y += sy;
            if let Some(outline) = scaled.outline_glyph(shifted) {
                // The rasterizer yields glyph-local pixel coordinates; rebase
                // them onto the glyph's absolute pixel origin so the glyph
                // lands where the layout math placed it.
                let origin = outline.px_bounds().min;
                outline.draw(|gx, gy, cov| {
                    if cov <= 0.0 {
                        return;
                    }
                    let x = (origin.x + gx as f32) as i32;
                    let y = (origin.y + gy as f32) as i32;
                    if x < 0
                        || y < 0
                        || x as f32 >= image.width() as f32
                        || y as f32 >= image.height() as f32
                    {
                        return;
                    }
                    let edge = (color.a() as f32 * cov).round() as u8;
                    blend_pixel(
                        image,
                        x as u32,
                        y as u32,
                        Rgba([color.r(), color.g(), color.b(), edge]),
                    );
                });
            }
        }
    }
}

pub fn draw_number_on_image(
    image: &mut RgbaImage,
    center: Pos2,
    number: u32,
    radius: f32,
    color: Color32,
) {
    use ab_glyph::{Font, Glyph, ScaleFont};
    let text = number.to_string();
    let font = bundled_font();
    let scale = ab_glyph::PxScale::from(radius * 1.3);
    let scaled = font.as_scaled(scale);

    // Lay out digits left-to-right using advance widths.
    let baseline_y = scaled.ascent();
    let mut glyphs: Vec<Glyph> = Vec::new();
    let mut pen_x = 0.0f32;
    for c in text.chars() {
        let id = font.glyph_id(c);
        glyphs.push(id.with_scale_and_position(scale, ab_glyph::point(pen_x, baseline_y)));
        pen_x += scaled.h_advance(id);
    }

    // Center the whole run inside the bubble from the union of glyph bounds.
    let mut min_x: Option<f32> = None;
    let mut min_y: Option<f32> = None;
    let mut max_x: Option<f32> = None;
    let mut max_y: Option<f32> = None;
    for g in &glyphs {
        let b = scaled.glyph_bounds(g);
        min_x = Some(min_x.map_or(b.min.x, |v: f32| v.min(b.min.x)));
        min_y = Some(min_y.map_or(b.min.y, |v: f32| v.min(b.min.y)));
        max_x = Some(max_x.map_or(b.max.x, |v: f32| v.max(b.max.x)));
        max_y = Some(max_y.map_or(b.max.y, |v: f32| v.max(b.max.y)));
    }
    let (Some(mx), Some(my), Some(mxx), Some(myy)) = (min_x, min_y, max_x, max_y) else {
        return;
    };
    let w = mxx - mx;
    let h = myy - my;
    let ox = center.x - w * 0.5 - mx;
    // Optical correction: numerals fill the horizontal-middle band of the em
    // box rather than the full ascent/descent, so nudge up slightly.
    let oy = center.y - h * 0.5 - my - h * 0.06;

    stamp_glyphs(image, scaled, &glyphs, ox, oy, color);
}

fn bundled_font() -> &'static ab_glyph::FontArc {
    static FONT: OnceLock<ab_glyph::FontArc> = OnceLock::new();
    FONT.get_or_init(|| {
        ab_glyph::FontArc::try_from_slice(include_bytes!("../assets/Ubuntu-Light.ttf"))
            .expect("embedded Ubuntu-Light.ttf is a valid font")
    })
}
