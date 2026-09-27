//! Low-level raster helpers shared by the annotation tools when stamping a
//! shape onto the exported/pinned `RgbaImage`. All coordinates are in image
//! pixel space and all drawing blends with alpha.

use ecolor::Color32;
use emath::{pos2, Pos2};
use image::{Rgba, RgbaImage};

pub fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    let p = image.get_pixel_mut(x, y);
    let a = color.0[3] as f32 / 255.0;
    p.0[0] = (color.0[0] as f32 * a + p.0[0] as f32 * (1.0 - a)) as u8;
    p.0[1] = (color.0[1] as f32 * a + p.0[1] as f32 * (1.0 - a)) as u8;
    p.0[2] = (color.0[2] as f32 * a + p.0[2] as f32 * (1.0 - a)) as u8;
    p.0[3] = 255;
}

pub fn draw_line_on_image(
    image: &mut RgbaImage,
    start: Pos2,
    end: Pos2,
    color: Color32,
    width: f32,
) {
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = dx.hypot(dy).max(1.0);
    let steps = (len * 2.0).ceil() as i32;
    // Stamping radius: the run of covered pixels is 2w+1 wide, so choose w so
    // the thickness matches `width` (1 -> 1 px, 3 -> 3 px, ...).
    let w = ((width.max(1.0) - 1.0) / 2.0).ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = start.x + dx * t;
        let cy = start.y + dy * t;
        for oy in -w..=w {
            for ox in -w..=w {
                if ox * ox + oy * oy > w * w {
                    continue;
                }
                let x = (cx + ox as f32).round() as i32;
                let y = (cy + oy as f32).round() as i32;
                if x >= 0 && y >= 0 && (x as f32) < iw && (y as f32) < ih {
                    blend_pixel(image, x as u32, y as u32, rgba);
                }
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
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let x0 = (center.x - rx).floor() as i32;
    let y0 = (center.y - ry).floor() as i32;
    let x1 = (center.x + rx).ceil() as i32;
    let y1 = (center.y + ry).ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let rxm = rx.max(1.0);
    let rym = ry.max(1.0);
    for y in y0..y1 {
        for x in x0..x1 {
            if x < 0 || y < 0 || (x as f32) >= iw || (y as f32) >= ih {
                continue;
            }
            let dx = (x as f32 - center.x) / rxm;
            let dy = (y as f32 - center.y) / rym;
            if dx * dx + dy * dy <= 1.0 {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
}

pub fn fill_circle_on_image(image: &mut RgbaImage, center: Pos2, radius: f32, color: Color32) {
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let x0 = (center.x - radius).floor() as i32;
    let y0 = (center.y - radius).floor() as i32;
    let x1 = (center.x + radius).ceil() as i32;
    let y1 = (center.y + radius).ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let r2 = radius.max(1.0);
    for y in y0..y1 {
        for x in x0..x1 {
            if x < 0 || y < 0 || (x as f32) >= iw || (y as f32) >= ih {
                continue;
            }
            let dx = x as f32 - center.x;
            let dy = y as f32 - center.y;
            if dx * dx + dy * dy <= r2 * r2 {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
}

/// Draw a stroked (hollow) circle ring of the given width.
pub fn draw_circle_on_image(
    image: &mut RgbaImage,
    center: Pos2,
    radius: f32,
    width: f32,
    color: Color32,
) {
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let outer = radius + width;
    let x0 = (center.x - outer).floor() as i32;
    let y0 = (center.y - outer).floor() as i32;
    let x1 = (center.x + outer).ceil() as i32;
    let y1 = (center.y + outer).ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);
    let ro = outer.max(1.0);
    let ri = radius.max(0.0);
    for y in y0..y1 {
        for x in x0..x1 {
            if x < 0 || y < 0 || (x as f32) >= iw || (y as f32) >= ih {
                continue;
            }
            let dx = x as f32 - center.x;
            let dy = y as f32 - center.y;
            let d2 = dx * dx + dy * dy;
            if d2 <= ro * ro && d2 >= ri * ri {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
}

/// Fill an axis-aligned bounding-box of the given triangle, testing barycentric
/// containment so only the triangle interior is painted.
pub fn fill_triangle_on_image(image: &mut RgbaImage, a: Pos2, b: Pos2, c: Pos2, color: Color32) {
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let min_x = a.x.min(b.x).min(c.x).floor() as i32;
    let max_x = a.x.max(b.x).max(c.x).ceil() as i32;
    let min_y = a.y.min(b.y).min(c.y).floor() as i32;
    let max_y = a.y.max(b.y).max(c.y).ceil() as i32;
    let rgba = Rgba([color.r(), color.g(), color.b(), color.a()]);

    let d1 = b - a;
    let d2 = c - a;
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-6 {
        return;
    }

    for y in min_y..max_y {
        for x in min_x..max_x {
            if x < 0 || y < 0 || (x as f32) >= iw || (y as f32) >= ih {
                continue;
            }
            let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
            let q = p - a;
            let u = (q.x * d2.y - q.y * d2.x) / denom;
            let v = (d1.x * q.y - d1.y * q.x) / denom;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
                blend_pixel(image, x as u32, y as u32, rgba);
            }
        }
    }
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
