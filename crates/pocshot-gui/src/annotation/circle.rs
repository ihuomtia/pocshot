//! Circle and Filled-circle tools. Share a `CircleTool` whose `filled` flag
//! selects between an outline and a solid fill.

use eframe::egui::{pos2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use image::RgbaImage;

use super::raster::{draw_line_on_image, fill_ellipse_on_image};
use super::AnnotationTool;
use crate::constrain::apply_constraint;

#[derive(Debug, Clone, Default)]
pub struct CircleTool {
    center: Pos2,
    radius: Vec2,
    filled: bool,
}

impl CircleTool {
    pub fn new(filled: bool) -> Self {
        Self {
            center: Pos2::ZERO,
            radius: Vec2::ZERO,
            filled,
        }
    }
}

impl super::Tool for CircleTool {
    fn start(&mut self, pos: Pos2) {
        self.center = pos;
        self.radius = Vec2::ZERO;
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let r = apply_constraint(AnnotationTool::Circle, self.center, pos, ctrl, locked_angle);
        self.radius = Vec2::new(
            (r.point.x - self.center.x).abs(),
            (r.point.y - self.center.y).abs(),
        );
        r.locked_angle
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        let rect = Rect::from_center_size(map(self.center), self.radius * 2.0);
        if self.filled {
            painter.rect_filled(rect, self.radius.x, color);
        } else {
            painter.rect_stroke(
                rect,
                self.radius.x,
                Stroke::new(width, color),
                StrokeKind::Middle,
            );
        }
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        let c = self.center - offset.to_vec2();
        let rx = self.radius.x;
        let ry = self.radius.y;
        if self.filled {
            fill_ellipse_on_image(image, c, rx, ry, color);
        } else {
            let steps = 64;
            for i in 0..steps {
                let a0 = i as f32 / steps as f32 * std::f32::consts::TAU;
                let a1 = (i + 1) as f32 / steps as f32 * std::f32::consts::TAU;
                let p0 = pos2(c.x + a0.cos() * rx, c.y + a0.sin() * ry);
                let p1 = pos2(c.x + a1.cos() * rx, c.y + a1.sin() * ry);
                draw_line_on_image(image, p0, p1, color, width);
            }
        }
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        let dx = (pos.x - self.center.x) / self.radius.x.max(1.0);
        let dy = (pos.y - self.center.y) / self.radius.y.max(1.0);
        let d = (dx * dx + dy * dy).sqrt();
        if self.filled {
            d <= 1.0
        } else {
            (d - 1.0).abs() * self.radius.x.min(self.radius.y) <= threshold
        }
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (
            vec![
                self.center.y,
                self.center.y - self.radius.y,
                self.center.y + self.radius.y,
            ],
            vec![
                self.center.x,
                self.center.x - self.radius.x,
                self.center.x + self.radius.x,
            ],
        )
    }
}
