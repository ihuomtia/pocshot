mod algorithm;

use image::{imageops, imageops::FilterType, DynamicImage, Rgba, RgbaImage};

const DOWNSCALE_FACTOR: u32 = 2;

#[derive(Debug, Clone)]
pub struct SnapLines {
    pub horizontal: Vec<f32>,
    pub vertical: Vec<f32>,
}

impl SnapLines {
    pub fn nearest_horizontal(&self, y: f32, threshold: f32) -> Option<f32> {
        self.horizontal
            .iter()
            .map(|&line| (line, (line - y).abs()))
            .filter(|&(_, dist)| dist <= threshold)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(line, _)| line)
    }

    pub fn nearest_vertical(&self, x: f32, threshold: f32) -> Option<f32> {
        self.vertical
            .iter()
            .map(|&line| (line, (line - x).abs()))
            .filter(|&(_, dist)| dist <= threshold)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(line, _)| line)
    }
}

pub struct SnapConfig {
    pub gradient_threshold: u8,
    pub min_line_ratio: f32,
    pub min_line_length_ratio: f32,
    pub merge_distance: f32,
    pub dilate_radius: u8,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            gradient_threshold: 25,
            min_line_ratio: 0.30,
            min_line_length_ratio: 0.30,
            merge_distance: 4.0,
            dilate_radius: 7,
        }
    }
}

pub fn detect_snap_lines(image: &RgbaImage, config: &SnapConfig) -> SnapLines {
    let (w, h) = (image.width() as usize, image.height() as usize);
    if w < 10 || h < 10 {
        return SnapLines {
            horizontal: Vec::new(),
            vertical: Vec::new(),
        };
    }

    let scale = DOWNSCALE_FACTOR as f32;

    let small_w = (w / DOWNSCALE_FACTOR as usize).max(1);
    let small_h = (h / DOWNSCALE_FACTOR as usize).max(1);

    let small_rgba = imageops::resize(image, small_w as u32, small_h as u32, FilterType::Nearest);
    let gray = DynamicImage::ImageRgba8(small_rgba).to_luma8();

    let (sw, sh) = (gray.width() as usize, gray.height() as usize);

    let horiz_edges_i16 = imageproc::gradients::vertical_sobel(&gray);
    let vert_edges_i16 = imageproc::gradients::horizontal_sobel(&gray);

    let horiz_edges = algorithm::map_i16_to_u8(&horiz_edges_i16);
    let vert_edges = algorithm::map_i16_to_u8(&vert_edges_i16);

    let horiz_bin = algorithm::threshold_image(&horiz_edges, config.gradient_threshold);
    let vert_bin = algorithm::threshold_image(&vert_edges, config.gradient_threshold);

    let horiz_clean = algorithm::dilate_horizontal(&horiz_bin, config.dilate_radius);

    let vert_transposed = algorithm::transpose(&vert_bin);
    let vert_clean_t = algorithm::dilate_horizontal(&vert_transposed, config.dilate_radius);

    let horiz_count_px = (sw as f32 * config.min_line_ratio) as usize;
    let horiz_run_px = (sw as f32 * config.min_line_length_ratio) as usize;
    let vert_count_px = (sh as f32 * config.min_line_ratio) as usize;
    let vert_run_px = (sh as f32 * config.min_line_length_ratio) as usize;

    let horizontal_raw = algorithm::detect_projection_lines_rows(
        &horiz_clean,
        horiz_count_px,
        horiz_run_px,
        config.merge_distance,
    );
    let vertical_raw = algorithm::detect_projection_lines_rows(
        &vert_clean_t,
        vert_count_px,
        vert_run_px,
        config.merge_distance,
    );

    let horizontal: Vec<f32> = horizontal_raw.iter().map(|&y| y * scale).collect();
    let vertical: Vec<f32> = vertical_raw.iter().map(|&x| x * scale).collect();

    log::debug!(
        "snap: {}x{} → {}x{}, {} horiz lines, {} vert lines (thresh={}, min_ratio={}, min_len_ratio={})",
        w, h, sw, sh,
        horizontal.len(),
        vertical.len(),
        config.gradient_threshold,
        config.min_line_ratio,
        config.min_line_length_ratio,
    );
    if !horizontal.is_empty() {
        log::debug!(
            "snap:   horiz @ y: {:?}",
            &horizontal[..horizontal.len().min(10)]
        );
    }
    if !vertical.is_empty() {
        log::debug!(
            "snap:   vert  @ x: {:?}",
            &vertical[..vertical.len().min(10)]
        );
    }

    SnapLines {
        horizontal,
        vertical,
    }
}

/// Debug helper: write the original image with detected snap lines overlaid
/// (green for horizontal, red for vertical) to `snap_debug.png` in CWD.
pub fn debug_write_overlay(image: &RgbaImage, lines: &SnapLines) {
    let mut overlay: RgbaImage = image.clone();
    let green = Rgba([0, 255, 0, 255]);
    let red = Rgba([255, 0, 0, 255]);

    for &y in &lines.horizontal {
        let y0 = y.round() as u32;
        for x in 0..overlay.width() {
            if let Some(px) = overlay.get_pixel_mut_checked(x, y0) {
                *px = green;
            }
        }
    }
    for &x in &lines.vertical {
        let x0 = x.round() as u32;
        for y in 0..overlay.height() {
            if let Some(px) = overlay.get_pixel_mut_checked(x0, y) {
                *px = red;
            }
        }
    }

    match DynamicImage::ImageRgba8(overlay).save("snap_debug.png") {
        Ok(()) => log::info!("snap debug overlay written to snap_debug.png"),
        Err(e) => log::error!("failed to write snap debug overlay: {e}"),
    }
}
