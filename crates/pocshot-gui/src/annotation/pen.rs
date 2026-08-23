//! Pen / freehand tool: a polyline of sampled points.

use eframe::egui::{Color32, Painter, Pos2, Stroke};
use image::RgbaImage;

use super::raster::{dist_to_segment, draw_line_on_image};
use super::AnnotationTool;
use crate::constrain::apply_constraint;

#[derive(Debug, Clone, Default)]
pub struct PenTool {
    points: Vec<Pos2>,
    anchor: Pos2,
}

impl super::Tool for PenTool {
    fn start(&mut self, pos: Pos2) {
        self.points.clear();
        self.anchor = pos;
        self.points.push(pos);
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(AnnotationTool::Pen, self.anchor, pos, ctrl, locked_angle);
        self.points.push(r.point);
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        for w in self.points.windows(2) {
            painter.line_segment([map(w[0]), map(w[1])], Stroke::new(width, color));
        }
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        let o = offset.to_vec2();
        for w in self.points.windows(2) {
            draw_line_on_image(image, w[0] - o, w[1] - o, color, width);
        }
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        self.points
            .windows(2)
            .any(|w| dist_to_segment(pos, w[0], w[1]) <= threshold)
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        if let (Some(first), Some(last)) = (self.points.first(), self.points.last()) {
            (vec![first.y, last.y], vec![first.x, last.x])
        } else {
            (Vec::new(), Vec::new())
        }
    }
}
