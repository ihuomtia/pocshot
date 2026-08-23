use image::GrayImage;

pub fn map_i16_to_u8(image: &image::ImageBuffer<image::Luma<i16>, Vec<i16>>) -> GrayImage {
    GrayImage::from_raw(
        image.width(),
        image.height(),
        image
            .iter()
            .map(|&p| p.unsigned_abs().min(255) as u8)
            .collect(),
    )
    .unwrap()
}

pub fn threshold_image(image: &GrayImage, threshold: u8) -> GrayImage {
    GrayImage::from_raw(
        image.width(),
        image.height(),
        image
            .iter()
            .map(|&p| if p > threshold { 255u8 } else { 0u8 })
            .collect(),
    )
    .unwrap()
}

/// Dilate only along the horizontal axis (left-right), O(n) per row.
/// Tracks the last non-zero pixel position; a pixel is set if it's within
/// `radius` of any non-zero pixel in the same row.
pub fn dilate_horizontal(image: &GrayImage, radius: u8) -> GrayImage {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let src = image.as_raw();
    let r = radius as usize;
    let mut dst = vec![0u8; src.len()];

    for y in 0..h {
        let row_start = y * w;
        let row = &src[row_start..row_start + w];

        // Forward pass: propagate hits rightward
        let mut last_hit: isize = -(r as isize) - 1;
        for x in 0..w {
            if row[x] > 0 {
                last_hit = x as isize;
            }
            if (x as isize) - last_hit <= r as isize {
                dst[row_start + x] = 255;
            }
        }
    }

    GrayImage::from_raw(image.width(), image.height(), dst).unwrap()
}

/// Transpose a GrayImage (swap rows and columns).
/// After transpose, what were columns become rows — enabling cache-friendly
/// row-major access for vertical line processing.
pub fn transpose(image: &GrayImage) -> GrayImage {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let src = image.as_raw();
    let mut dst = vec![0u8; w * h];

    for y in 0..h {
        for x in 0..w {
            dst[x * h + y] = src[y * w + x];
        }
    }

    GrayImage::from_raw(h as u32, w as u32, dst).unwrap()
}

/// Project binary image onto rows (sum each row).
/// A row qualifies as a line if:
///   1. Total non-zero pixel count >= `count_px`, AND
///   2. The longest contiguous run of non-zero pixels >= `run_px`.
pub fn detect_projection_lines_rows(
    image: &GrayImage,
    count_px: usize,
    run_px: usize,
    merge: f32,
) -> Vec<f32> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let data = image.as_raw();
    let mut candidates: Vec<f32> = Vec::new();

    for y in 0..h {
        let row = &data[y * w..(y + 1) * w];
        let mut count = 0usize;
        let mut longest = 0usize;
        let mut run = 0usize;
        for &p in row {
            if p > 0 {
                count += 1;
                run += 1;
                longest = longest.max(run);
            } else {
                run = 0;
            }
        }
        if count >= count_px && longest >= run_px {
            candidates.push(y as f32);
        }
    }

    merge_lines(candidates, merge)
}

fn merge_lines(mut lines: Vec<f32>, merge: f32) -> Vec<f32> {
    if lines.is_empty() {
        return lines;
    }
    lines.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut merged: Vec<f32> = Vec::new();
    let mut sum = lines[0];
    let mut count = 1usize;
    for &line in &lines[1..] {
        let avg = sum / count as f32;
        if (line - avg).abs() <= merge {
            sum += line;
            count += 1;
        } else {
            merged.push(avg);
            sum = line;
            count = 1;
        }
    }
    merged.push(sum / count as f32);
    merged
}
