//! Line and Arrow tools. Both are a straight segment from start to end; the
//! Arrow additionally renders a head at the end point.

use eframe::egui::{Color32, Painter, Pos2, Stroke, Vec2};
use image::RgbaImage;

use super::raster::{dist_to_segment, draw_line_on_image};
use super::AnnotationTool;
use crate::constrain::apply_constraint;

#[derive(Debug, Clone, Default)]
pub struct LineTool {
    start: Pos2,
    end: Pos2,
}

impl super::Tool for LineTool {
    fn start(&mut self, pos: Pos2) {
        self.start = pos;
        self.end = pos;
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(AnnotationTool::Line, self.start, pos, ctrl, locked_angle);
        self.end = r.point;
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        painter.line_segment([map(self.start), map(self.end)], Stroke::new(width, color));
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        let off = offset.to_vec2();
        draw_line_on_image(image, self.start - off, self.end - off, color, width);
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        dist_to_segment(pos, self.start, self.end) <= threshold
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![self.start.y, self.end.y],
            vec![self.start.x, self.end.x],
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArrowTool {
    start: Pos2,
    end: Pos2,
}

impl ArrowTool {
    fn head_points(&self, width: f32) -> (Pos2, Pos2, Pos2) {
        let delta = self.end - self.start;
        let len = delta.length();
        let dir = if len > 0.001 { delta / len } else { Vec2::ZERO };
        let perp = Vec2::new(-dir.y, dir.x);
        let head_len = 12.0 + width * 1.5;
        let head_w = 6.0 + width;
        let p1 = self.end - dir * head_len + perp * head_w;
        let p2 = self.end - dir * head_len - perp * head_w;
        (self.end, p1, p2)
    }
}

impl super::Tool for ArrowTool {
    fn start(&mut self, pos: Pos2) {
        self.start = pos;
        self.end = pos;
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(AnnotationTool::Arrow, self.start, pos, ctrl, locked_angle);
        self.end = r.point;
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        let a = map(self.start);
        let b = map(self.end);
        painter.line_segment([a, b], Stroke::new(width, color));
        let delta = b - a;
        let len = delta.length();
        let dir = if len > 0.001 { delta / len } else { Vec2::ZERO };
        let perp = Vec2::new(-dir.y, dir.x);
        let head_len = 12.0 + width * 1.5;
        let head_w = 6.0 + width;
        let p1 = b - dir * head_len + perp * head_w;
        let p2 = b - dir * head_len - perp * head_w;
        painter.line_segment([b, p1], Stroke::new(width, color));
        painter.line_segment([b, p2], Stroke::new(width, color));
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        let off = offset.to_vec2();
        let (apex, p1, p2) = self.head_points(width);
        draw_line_on_image(image, self.start - off, self.end - off, color, width);
        draw_line_on_image(image, apex - off, p1 - off, color, width);
        draw_line_on_image(image, apex - off, p2 - off, color, width);
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        dist_to_segment(pos, self.start, self.end) <= threshold
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![self.start.y, self.end.y],
            vec![self.start.x, self.end.x],
        )
    }
}
