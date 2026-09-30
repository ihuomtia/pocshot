//! Low-level raster helpers shared by the annotation tools when stamping a
//! shape onto the exported/pinned `RgbaImage`. All coordinates are in image
//! pixel space and all drawing blends with alpha.

use ecolor::Color32;
use emath::{pos2, Pos2};
use image::{Rgba, RgbaImage};

/// Sub-samples per axis used to estimate partial pixel coverage (4x4 = 16).
const AA_SAMPLES: u32 = 4;

pub fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    blend_coverage(image, x, y, color, 1.0);
}

/// Blend `color` into pixel `(x, y)` weighted by fractional `coverage` (0..1),
/// so edge pixels can be partially tinted. Coverage 1.0 is equivalent to
/// [`blend_pixel`].
pub fn blend_coverage(image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>, coverage: f32) {
    let coverage = coverage.clamp(0.0, 1.0);
    if coverage <= 0.0 {
        return;
    }
    let p = image.get_pixel_mut(x, y);
    let a = color.0[3] as f32 / 255.0 * coverage;
    p.0[0] = (color.0[0] as f32 * a + p.0[0] as f32 * (1.0 - a)) as u8;
    p.0[1] = (color.0[1] as f32 * a + p.0[1] as f32 * (1.0 - a)) as u8;
    p.0[2] = (color.0[2] as f32 * a + p.0[2] as f32 * (1.0 - a)) as u8;
    p.0[3] = 255;
}

/// Paint the pixel box `[x0..=x1] x [y0..=y1]` (index space, clipped to the
/// image), blending each pixel with the fraction of 4x4 sub-samples for which
/// `inside` is true. This is the antialiasing primitive the curved shapes use;
/// coordinates are pixel indices, so sub-samples straddle each pixel centre.
fn fill_box_aa<F: Fn(f32, f32) -> bool>(
    image: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
    inside: F,
) {
    let (iw, ih) = (image.width() as i32, image.height() as i32);
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(iw - 1);
    let y1 = y1.min(ih - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }
    let step = 1.0 / AA_SAMPLES as f32;
    let total = (AA_SAMPLES * AA_SAMPLES) as f32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let mut hits = 0u32;
            for sy in 0..AA_SAMPLES {
                let py = y as f32 - 0.5 + (sy as f32 + 0.5) * step;
                for sx in 0..AA_SAMPLES {
                    let px = x as f32 - 0.5 + (sx as f32 + 0.5) * step;
                    if inside(px, py) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                blend_coverage(image, x as u32, y as u32, color, hits as f32 / total);
            }
        }
    }
}

pub fn draw_line_on_image(
    image: &mut RgbaImage,
    start: Pos2,
    end: Pos2,
    color: Color32,
    width: f32,
) {
    // Analytic coverage: a pixel is tinted by how far its centre sits inside
    // the stroke, feathered over one pixel so diagonal edges stay smooth. The
    // half-width keeps the rendered run equal to `width` at integer
    // coordinates (1 -> 1 px, 3 -> 3 px, ...).
    let (iw, ih) = (image.width() as i32, image.height() as i32);
    let half = width.max(1.0) * 0.5;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let pad = half + 1.0;
    let x0 = (start.x.min(end.x) - pad).floor() as i32;
    let x1 = (start.x.max(end.x) + pad).ceil() as i32;
    let y0 = (start.y.min(end.y) - pad).floor() as i32;
    let y1 = (start.y.max(end.y) + pad).ceil() as i32;

    for y in y0.max(0)..=y1.min(ih - 1) {
        for x in x0.max(0)..=x1.min(iw - 1) {
            let dist = dist_to_segment(pos2(x as f32, y as f32), start, end);
            let coverage = (half + 0.5 - dist).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_coverage(image, x as u32, y as u32, rgba, coverage);
            }
        }
    }
}

pub fn fill_rect_on_image(image: &mut RgbaImage, min: Pos2, max: Pos2, color: Color32) {
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let x0 = min.x.floor() as i32;
    let y0 = min.y.floor() as i32;
    let x1 = max.x.ceil() as i32;
    let y1 = max.y.ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    for y in y0..y1 {
        for x in x0..x1 {
            if x >= 0 && y >= 0 && (x as f32) < iw && (y as f32) < ih {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
}

pub fn fill_ellipse_on_image(
    image: &mut RgbaImage,
    center: Pos2,
    rx: f32,
    ry: f32,
    color: Color32,
) {
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let (rxm, rym) = (rx, ry);
    fill_box_aa(
        image,
        (center.x - rx - 1.0).floor() as i32,
        (center.y - ry - 1.0).floor() as i32,
        (center.x + rx + 1.0).ceil() as i32,
        (center.y + ry + 1.0).ceil() as i32,
        rgba,
        move |px, py| {
            let dx = (px - center.x) / rxm;
            let dy = (py - center.y) / rym;
            dx * dx + dy * dy <= 1.0
        },
    );
}

pub fn fill_circle_on_image(image: &mut RgbaImage, center: Pos2, radius: f32, color: Color32) {
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    if radius <= 0.0 {
        return;
    }
    let r = radius;
    fill_box_aa(
        image,
        (center.x - radius - 1.0).floor() as i32,
        (center.y - radius - 1.0).floor() as i32,
        (center.x + radius + 1.0).ceil() as i32,
        (center.y + radius + 1.0).ceil() as i32,
        rgba,
        move |px, py| {
            let dx = px - center.x;
            let dy = py - center.y;
            dx * dx + dy * dy <= r * r
        },
    );
}

/// Draw a stroked (hollow) circle ring of the given width.
pub fn draw_circle_on_image(
    image: &mut RgbaImage,
    center: Pos2,
    radius: f32,
    width: f32,
    color: Color32,
) {
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let outer = radius + width;
    if outer <= 0.0 {
        return;
    }
    let (ri, ro) = (radius.max(0.0), outer);
    fill_box_aa(
        image,
        (center.x - outer - 1.0).floor() as i32,
        (center.y - outer - 1.0).floor() as i32,
        (center.x + outer + 1.0).ceil() as i32,
        (center.y + outer + 1.0).ceil() as i32,
        rgba,
        move |px, py| {
            let dx = px - center.x;
            let dy = py - center.y;
            let d2 = dx * dx + dy * dy;
            d2 <= ro * ro && d2 >= ri * ri
        },
    );
}

/// Fill an axis-aligned bounding-box of the given triangle, testing barycentric
/// containment so only the triangle interior is painted.
pub fn fill_triangle_on_image(image: &mut RgbaImage, a: Pos2, b: Pos2, c: Pos2, color: Color32) {
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let d1 = b - a;
    let d2 = c - a;
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-6 {
        return;
    }
    fill_box_aa(
        image,
        (a.x.min(b.x).min(c.x) - 1.0).floor() as i32,
        (a.y.min(b.y).min(c.y) - 1.0).floor() as i32,
        (a.x.max(b.x).max(c.x) + 1.0).ceil() as i32,
        (a.y.max(b.y).max(c.y) + 1.0).ceil() as i32,
        rgba,
        move |px, py| {
            let q = pos2(px, py) - a;
            let u = (q.x * d2.y - q.y * d2.x) / denom;
            let v = (d1.x * q.y - d1.y * q.x) / denom;
            u >= 0.0 && v >= 0.0 && u + v <= 1.0
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecolor::Color32;
    use emath::pos2;
    use image::Rgba;

    fn blank(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 255]))
    }

    fn run_width(image: &RgbaImage, y: u32) -> usize {
        let row: Vec<bool> = (0..image.width())
            .map(|x| image.get_pixel(x, y).0[0] != 0)
            .collect();
        let first = row.iter().position(|&c| c).unwrap();
        let last = row.iter().rposition(|&c| c).unwrap();
        last - first + 1
    }

    #[test]
    fn line_thickness_matches_width() {
        for width in [1.0f32, 2.0, 3.0, 5.0] {
            let mut image = blank(40, 40);
            draw_line_on_image(
                &mut image,
                pos2(20.0, 10.0),
                pos2(20.0, 30.0),
                Color32::WHITE,
                width,
            );
            let run = run_width(&image, 20);
            assert!(
                (run as f32 - width).abs() <= 1.0,
                "width {width} rendered {run} px"
            );
        }
    }

    /// True when `image` holds at least one pixel blended between the black
    /// background and the full white stroke — the signature of edge coverage.
    fn has_partial(image: &RgbaImage) -> bool {
        image
            .pixels()
            .any(|p| p.0[0] > 0 && p.0[0] < 255)
    }

    #[test]
    fn line_edges_are_antialiased() {
        let mut image = blank(40, 40);
        draw_line_on_image(
            &mut image,
            pos2(4.0, 4.0),
            pos2(36.0, 20.0),
            Color32::WHITE,
            3.0,
        );
        assert!(
            has_partial(&image),
            "line edge pixels should have partial coverage"
        );
    }

    #[test]
    fn filled_circle_edges_are_antialiased() {
        let mut image = blank(40, 40);
        fill_circle_on_image(&mut image, pos2(20.0, 20.0), 10.0, Color32::WHITE);
        assert!(
            has_partial(&image),
            "circle edge pixels should have partial coverage"
        );
    }

    #[test]
    fn triangle_edges_are_antialiased() {
        let mut image = blank(40, 40);
        fill_triangle_on_image(
            &mut image,
            pos2(4.0, 34.0),
            pos2(20.0, 4.0),
            pos2(36.0, 34.0),
            Color32::WHITE,
        );
        assert!(
            has_partial(&image),
            "triangle edge pixels should have partial coverage"
        );
    }

    #[test]
    fn filled_circle_interior_stays_opaque() {
        let mut image = blank(40, 40);
        fill_circle_on_image(&mut image, pos2(20.0, 20.0), 10.0, Color32::WHITE);
        assert_eq!(
            image.get_pixel(20, 20).0[0],
            255,
            "the circle interior must stay fully opaque"
        );
    }
}

/// Distance from `p` to the segment `[a, b]`.
pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}
