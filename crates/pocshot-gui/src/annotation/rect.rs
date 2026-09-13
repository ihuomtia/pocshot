//! Rectangle and Filled-rectangle tools. Share a `RectTool` whose `filled`
//! flag selects between an outline and a solid fill.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind};
use image::RgbaImage;

use super::raster::{dist_to_segment, draw_line_on_image, fill_rect_on_image};
use super::AnnotationTool;
use crate::constrain::apply_constraint;

#[derive(Debug, Clone)]
pub struct RectTool {
    /// The fixed drag origin (set once on `start`), kept so the rectangle
    /// always anchors to where the drag began rather than drifting with the
    /// current pointer.
    start: Pos2,
    rect: Rect,
    filled: bool,
}

impl Default for RectTool {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RectTool {
    pub fn new(filled: bool) -> Self {
        Self {
            start: Pos2::ZERO,
            rect: Rect::NOTHING,
            filled,
        }
    }

    /// A rectangle outline already anchored to `rect` (used when a ready-made
    /// border is committed without a drag gesture).
    pub fn with_rect(rect: Rect) -> Self {
        Self {
            start: rect.min,
            rect,
            filled: false,
        }
    }

    /// A solid rectangle already anchored to `rect` (Redact tool).
    pub fn with_filled_rect(rect: Rect) -> Self {
        Self {
            start: rect.min,
            rect,
            filled: true,
        }
    }

    fn draw_outline(
        &self,
        painter: &Painter,
        map: &dyn Fn(Pos2) -> Pos2,
        color: Color32,
        width: f32,
    ) {
        let r = Rect::from_two_pos(map(self.rect.min), map(self.rect.max));
        painter.rect_stroke(r, 0.0, Stroke::new(width, color), StrokeKind::Middle);
    }

    fn draw_filled(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32) {
        let r = Rect::from_two_pos(map(self.rect.min), map(self.rect.max));
        painter.rect_filled(r, 0.0, color);
    }

    fn render_outline(&self, image: &mut RgbaImage, color: Color32, width: f32, off: Pos2) {
        let o = off.to_vec2();
        let min = self.rect.min - o;
        let max = self.rect.max - o;
        draw_line_on_image(image, min, Pos2::new(max.x, min.y), color, width);
        draw_line_on_image(image, Pos2::new(max.x, min.y), max, color, width);
        draw_line_on_image(image, max, Pos2::new(min.x, max.y), color, width);
        draw_line_on_image(image, Pos2::new(min.x, max.y), min, color, width);
    }

    fn render_filled(&self, image: &mut RgbaImage, color: Color32, off: Pos2) {
        let o = off.to_vec2();
        fill_rect_on_image(image, self.rect.min - o, self.rect.max - o, color);
    }
}

impl super::Tool for RectTool {
    fn start(&mut self, pos: Pos2) {
        self.start = pos;
        self.rect = Rect::from_two_pos(pos, pos);
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(
            AnnotationTool::Rectangle,
            self.start,
            pos,
            ctrl,
            locked_angle,
        );
        self.rect = Rect::from_two_pos(self.start, r.point);
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        if self.filled {
            self.draw_filled(painter, map, color);
        } else {
            self.draw_outline(painter, map, color, width);
        }
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        if self.filled {
            self.render_filled(image, color, offset);
        } else {
            self.render_outline(image, color, width, offset);
        }
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        if self.filled {
            self.rect.contains(pos)
        } else {
            let edges = [
                (self.rect.min, Pos2::new(self.rect.max.x, self.rect.min.y)),
                (Pos2::new(self.rect.max.x, self.rect.min.y), self.rect.max),
                (self.rect.max, Pos2::new(self.rect.min.x, self.rect.max.y)),
                (Pos2::new(self.rect.min.x, self.rect.max.y), self.rect.min),
            ];
            edges
                .iter()
                .any(|&(a, b)| dist_to_segment(pos, a, b) <= threshold)
        }
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![self.rect.min.y, self.rect.max.y],
            vec![self.rect.min.x, self.rect.max.x],
        )
    }
}
