//! Highlighter tool: a translucent filled rectangle (marker).

use eframe::egui::{Color32, Painter, Pos2, Rect};
use image::RgbaImage;

use super::raster::fill_rect_on_image;
use super::AnnotationTool;
use crate::constrain::apply_constraint;

#[derive(Debug, Clone)]
pub struct HighlighterTool {
    /// The fixed drag origin (set once on `start`), kept so the highlight
    /// always anchors to where the drag began rather than drifting with the
    /// current pointer.
    start: Pos2,
    rect: Rect,
}

impl Default for HighlighterTool {
    fn default() -> Self {
        Self {
            start: Pos2::ZERO,
            rect: Rect::NOTHING,
        }
    }
}

impl super::Tool for HighlighterTool {
    fn start(&mut self, pos: Pos2) {
        self.start = pos;
        self.rect = Rect::from_two_pos(pos, pos);
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(
            AnnotationTool::Highlighter,
            self.start,
            pos,
            ctrl,
            locked_angle,
        );
        self.rect = Rect::from_two_pos(self.start, r.point);
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, _width: f32) {
        let r = Rect::from_two_pos(map(self.rect.min), map(self.rect.max));
        painter.rect_filled(
            r,
            0.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120),
        );
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, _width: f32, offset: Pos2) {
        let o = offset.to_vec2();
        let hc = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120);
        fill_rect_on_image(image, self.rect.min - o, self.rect.max - o, hc);
    }

    fn hit_test(&self, pos: Pos2, _threshold: f32, _width: f32) -> bool {
        self.rect.contains(pos)
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![self.rect.min.y, self.rect.max.y],
            vec![self.rect.min.x, self.rect.max.x],
        )
    }
}
