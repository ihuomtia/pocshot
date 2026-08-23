use eframe::egui::Rect;
use image::{imageops, Rgba, RgbaImage};

/// Pixelate the rectangular region `[min, max]` of the image. Each
/// `block`×`block` cell is filled with the average colour of that cell,
/// producing the mosaic look.
pub fn pixelate_region(image: &mut RgbaImage, rect: Rect, block: u32) {
    let (iw, ih) = (image.width(), image.height());
    let x0 = rect.min.x.floor().max(0.0) as u32;
    let y0 = rect.min.y.floor().max(0.0) as u32;
    let x1 = rect.max.x.ceil().min(iw as f32) as u32;
    let y1 = rect.max.y.ceil().min(ih as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let block = block.max(1);

    let mut y = y0;
    while y < y1 {
        let cell_h = block.min(y1 - y);
        let mut x = x0;
        while x < x1 {
            let cell_w = block.min(x1 - x);
            // Average the block's pixels.
            let (mut r, mut g, mut b, mut a, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for py in y..y + cell_h {
                for px in x..x + cell_w {
                    let p = image.get_pixel(px, py).0;
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    a += p[3] as u64;
                    n += 1;
                }
            }
            if n == 0 {
                n = 1;
            }
            let avg = Rgba([(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]);
            for py in y..y + cell_h {
                for px in x..x + cell_w {
                    image.put_pixel(px, py, avg);
                }
            }
            x += cell_w;
        }
        y += cell_h;
    }
}

/// Gaussian-blur the rectangular region `[min, max]` of the image by `sigma`.
/// The region is clamped to the image bounds.
pub fn blur_region(image: &mut RgbaImage, rect: Rect, sigma: f32) {
    let x0 = rect.min.x.floor().max(0.0) as i64;
    let y0 = rect.min.y.floor().max(0.0) as i64;
    let x1 = rect.max.x.ceil().min(image.width() as f32) as i64;
    let y1 = rect.max.y.ceil().min(image.height() as f32) as i64;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let w = (x1 - x0) as u32;
    let h = (y1 - y0) as u32;
    let sub = imageops::crop_imm(image, x0 as u32, y0 as u32, w, h).to_image();
    let blurred = imageops::blur(&sub, sigma);
    imageops::replace(image, &blurred, x0 as i64, y0 as i64);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(eframe::egui::pos2(x0, y0), eframe::egui::pos2(x1, y1))
    }

    #[test]
    fn pixelate_region_mosaics_block() {
        // 64x8 image with a fine alternating black/white checkerboard. Pixelating
        // with a 16px block averages each cell to grey, so the output must no
        // longer be pure black/white.
        let img = RgbaImage::from_fn(64, 8, |x, y| {
            if (x + y as u32) % 2 == 0 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let mut p = img.clone();
        pixelate_region(&mut p, rect(0.0, 0.0, 64.0, 8.0), 16);
        assert_ne!(p, img, "pixelation should alter the region");
        // Each cell now averages a mix of black/white -> mid grey somewhere.
        let mid = p.get_pixel(8, 4).0;
        assert!(
            (mid[0] as i32 - 128).abs() < 70,
            "checkerboard block should average to grey, got {:?}",
            mid
        );
    }

    #[test]
    fn blur_region_smooths_extremes() {
        // Black field with a single bright white pixel in the middle.
        let mut img = RgbaImage::from_pixel(64, 8, Rgba([0, 0, 0, 255]));
        img.put_pixel(32, 4, Rgba([255, 255, 255, 255]));
        // Blur a region around the white pixel: neighbouring pixels gain
        // brightness, proving the blur spread.
        let mut b = img.clone();
        blur_region(&mut b, rect(26.0, 0.0, 38.0, 8.0), 6.0);
        let neighbour = b.get_pixel(30, 4).0[0];
        assert!(neighbour > 0, "blur should bleed white into neighbours");
        // Centred pixel dims (spread out).
        let centre = b.get_pixel(32, 4).0[0];
        assert!(centre < 255, "blur should diffuse the hot centre");
    }
}
