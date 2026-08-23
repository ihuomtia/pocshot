//! Text annotation tool: a click places an anchored, multi-line text label.
//! Text entry happens through an interactive `egui::TextEdit` at the app level
//! (see `AnnotationState::begin_text_edit`); this struct holds the *committed*
//! result — anchor, string, size and its measured bounding box — and knows how
//! to draw it on the canvas and stamp it onto an exported image.

use eframe::egui::{self, pos2, vec2, Align2, Color32, Painter, Pos2, Rect};
use image::RgbaImage;

use super::counter_font::{draw_text_on_image, halo_color, measure_text, SAMPLE_OFFSETS};

#[derive(Debug, Clone)]
pub struct TextTool {
    /// Top-left anchor of the text, in image coordinates.
    anchor: Pos2,
    text: String,
    /// Font size in points/pixels (snapshotted at commit time).
    size: f32,
    /// Measured `(width, height)` at `size` (via the bundled font), used for
    /// hit-testing and snap lines.
    bbox: (f32, f32),
}

impl TextTool {
    pub fn new(anchor: Pos2, text: String, size: f32) -> Self {
        let bbox = measure_text(&text, size);
        Self {
            anchor,
            text,
            size,
            bbox,
        }
    }
}

impl super::Tool for TextTool {
    fn start(&mut self, pos: Pos2) {
        self.anchor = pos;
    }

    fn update(&mut self, _pos: Pos2, _ctrl: bool, _locked_angle: Option<f32>) -> Option<f32> {
        None
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, _width: f32) {
        let anchor = map(self.anchor);
        let font = egui::FontId::proportional(self.size);
        let halo = halo_color(color);
        // Halo underneath, then main color on top (halo only shows at the rim).
        for (sx, sy) in SAMPLE_OFFSETS {
            painter.text(
                pos2(anchor.x + sx, anchor.y + sy),
                Align2::LEFT_TOP,
                &self.text,
                font.clone(),
                halo,
            );
        }
        painter.text(anchor, Align2::LEFT_TOP, &self.text, font, color);
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, _width: f32, offset: Pos2) {
        let anchor = self.anchor - offset.to_vec2();
        draw_text_on_image(
            image,
            anchor,
            &self.text,
            self.size,
            color,
            halo_color(color),
        );
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        let rect = Rect::from_min_max(self.anchor, self.anchor + vec2(self.bbox.0, self.bbox.1));
        rect.expand(threshold).contains(pos)
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![self.anchor.y, self.anchor.y + self.bbox.1],
            vec![self.anchor.x, self.anchor.x + self.bbox.0],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::Tool;

    fn pos2(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    fn make(text: &str, size: f32) -> TextTool {
        TextTool::new(pos2(100.0, 100.0), text.to_string(), size)
    }

    #[test]
    fn new_measures_bbox() {
        let tool = make("Hello", 18.0);
        assert!(
            tool.bbox.0 > 10.0,
            "width should be > 10, got {}",
            tool.bbox.0
        );
        assert!(
            tool.bbox.1 > 5.0,
            "height should be > 5, got {}",
            tool.bbox.1
        );
    }

    #[test]
    fn multiline_bbox_is_taller() {
        let single = make("Hello", 18.0);
        let multi = make("Hello\nWorld", 18.0);
        assert!(multi.bbox.1 > single.bbox.1);
        // Two lines never shrink the measured height.
        assert!((multi.bbox.1 - single.bbox.1) >= 1.0);
    }

    #[test]
    fn hit_test_within_bbox() {
        let tool = make("Hello World", 18.0);
        assert!(tool.hit_test(tool.anchor + vec2(1.0, 1.0), 0.0, 0.0));
        // Below the measured height should miss.
        assert!(!tool.hit_test(tool.anchor + vec2(1.0, tool.bbox.1 + 5.0), 0.0, 0.0));
        // Within threshold still hits.
        assert!(tool.hit_test(tool.anchor + vec2(-2.0, -2.0), 6.0, 0.0));
    }

    #[test]
    fn snap_lines_span_bbox_edges() {
        let tool = make("Abc", 18.0);
        let (h, v) = tool.snap_lines();
        assert_eq!(h, vec![100.0, 100.0 + tool.bbox.1]);
        assert_eq!(v, vec![100.0, 100.0 + tool.bbox.0]);
    }

    #[test]
    fn render_stamps_pixels() {
        let mut img = RgbaImage::new(300, 300);
        let tool = make("Text", 24.0);
        tool.render(
            &mut img,
            Color32::from_rgb(239, 68, 68),
            3.0,
            pos2(0.0, 0.0),
        );
        // Some non-background pixel should appear inside the text bbox.
        let mut colored = 0u32;
        for y in 100..140 {
            for x in 100..180 {
                let p = img.get_pixel(x, y);
                if p.0[0] > 30 || p.0[1] > 30 || p.0[2] > 30 {
                    colored += 1;
                }
            }
        }
        assert!(colored > 0, "expected stamped glyph pixels, got {colored}");
    }
}
