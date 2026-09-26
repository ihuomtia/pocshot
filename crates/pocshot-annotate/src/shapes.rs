//! Painter-free annotation shapes. Each shape knows how to update itself from
//! drag positions and how to stamp itself onto an `RgbaImage`. This is the
//! model the Slint UI drives.

use ecolor::Color32;
use emath::{pos2, Pos2, Rect, Vec2};
use image::RgbaImage;

use crate::constrain::apply_constraint;
use crate::raster::{dist_to_segment, draw_line_on_image, fill_ellipse_on_image, fill_rect_on_image};
use crate::ToolKind;

#[derive(Debug, Clone)]
pub enum Shape {
    Rectangle {
        start: Pos2,
        rect: Rect,
        filled: bool,
    },
    Line {
        start: Pos2,
        end: Pos2,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
    },
    Circle {
        center: Pos2,
        radius: Vec2,
        filled: bool,
    },
    Pen {
        points: Vec<Pos2>,
        anchor: Pos2,
    },
    Highlighter {
        rect: Rect,
    },
}

impl Shape {
    /// Start a shape for `kind` at `pos`, or `None` for tools that are not
    /// drag shapes (Select, Text, effects, …).
    pub fn begin(kind: ToolKind, pos: Pos2) -> Option<Self> {
        Some(match kind {
            ToolKind::Rectangle => Shape::Rectangle {
                start: pos,
                rect: Rect::from_two_pos(pos, pos),
                filled: false,
            },
            ToolKind::FilledRectangle | ToolKind::Redact => Shape::Rectangle {
                start: pos,
                rect: Rect::from_two_pos(pos, pos),
                filled: true,
            },
            ToolKind::Line => Shape::Line {
                start: pos,
                end: pos,
            },
            ToolKind::Arrow => Shape::Arrow {
                start: pos,
                end: pos,
            },
            ToolKind::Circle => Shape::Circle {
                center: pos,
                radius: Vec2::ZERO,
                filled: false,
            },
            ToolKind::FilledCircle => Shape::Circle {
                center: pos,
                radius: Vec2::ZERO,
                filled: true,
            },
            ToolKind::Pen => Shape::Pen {
                points: vec![pos],
                anchor: pos,
            },
            ToolKind::Highlighter | ToolKind::HighlightText => Shape::Highlighter {
                rect: Rect::from_two_pos(pos, pos),
            },
            _ => return None,
        })
    }

    pub fn kind(&self) -> ToolKind {
        match self {
            Shape::Rectangle { filled: false, .. } => ToolKind::Rectangle,
            Shape::Rectangle { filled: true, .. } => ToolKind::FilledRectangle,
            Shape::Line { .. } => ToolKind::Line,
            Shape::Arrow { .. } => ToolKind::Arrow,
            Shape::Circle { filled: false, .. } => ToolKind::Circle,
            Shape::Circle { filled: true, .. } => ToolKind::FilledCircle,
            Shape::Pen { .. } => ToolKind::Pen,
            Shape::Highlighter { .. } => ToolKind::Highlighter,
        }
    }

    /// Extend the shape to `pos`, applying the Ctrl constraint.
    pub fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        let kind = self.kind();
        match self {
            Shape::Rectangle { start, rect, .. } => {
                let r = apply_constraint(kind, *start, pos, ctrl, locked_angle);
                *rect = Rect::from_two_pos(*start, r.point);
                r.locked_angle
            }
            Shape::Line { start, end } | Shape::Arrow { start, end } => {
                let r = apply_constraint(kind, *start, pos, ctrl, locked_angle);
                *end = r.point;
                r.locked_angle
            }
            Shape::Circle { center, radius, .. } => {
                let r = apply_constraint(kind, *center, pos, ctrl, locked_angle);
                *radius = Vec2::new(
                    (r.point.x - center.x).abs(),
                    (r.point.y - center.y).abs(),
                );
                r.locked_angle
            }
            Shape::Pen { points, anchor } => {
                let r = apply_constraint(kind, *anchor, pos, ctrl, locked_angle);
                points.push(r.point);
                r.locked_angle
            }
            Shape::Highlighter { rect } => {
                let start = rect.min;
                let r = apply_constraint(kind, start, pos, ctrl, locked_angle);
                *rect = Rect::from_two_pos(start, r.point);
                r.locked_angle
            }
        }
    }

    /// Stamp the shape onto `image`. `offset` is subtracted from shape
    /// coordinates (the image crop origin).
    pub fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        let off = offset.to_vec2();
        match self {
            Shape::Rectangle { rect, filled, .. } => {
                if *filled {
                    fill_rect_on_image(image, rect.min - off, rect.max - off, color);
                } else {
                    let min = rect.min - off;
                    let max = rect.max - off;
                    draw_line_on_image(image, min, pos2(max.x, min.y), color, width);
                    draw_line_on_image(image, pos2(max.x, min.y), max, color, width);
                    draw_line_on_image(image, max, pos2(min.x, max.y), color, width);
                    draw_line_on_image(image, pos2(min.x, max.y), min, color, width);
                }
            }
            Shape::Line { start, end } => {
                draw_line_on_image(image, *start - off, *end - off, color, width);
            }
            Shape::Arrow { start, end } => {
                let (apex, p1, p2) = arrow_head(*start, *end, width);
                draw_line_on_image(image, *start - off, *end - off, color, width);
                draw_line_on_image(image, apex - off, p1 - off, color, width);
                draw_line_on_image(image, apex - off, p2 - off, color, width);
            }
            Shape::Circle {
                center,
                radius,
                filled,
            } => {
                let c = *center - off;
                let (rx, ry) = (radius.x, radius.y);
                if *filled {
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
            Shape::Pen { points, .. } => {
                for w in points.windows(2) {
                    draw_line_on_image(image, w[0] - off, w[1] - off, color, width);
                }
            }
            Shape::Highlighter { rect } => {
                let translucent =
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120);
                fill_rect_on_image(image, rect.min - off, rect.max - off, translucent);
            }
        }
    }

    pub fn hit_test(&self, pos: Pos2, threshold: f32) -> bool {
        match self {
            Shape::Rectangle { rect, filled, .. } => {
                if *filled {
                    rect.contains(pos)
                } else {
                    let edges = [
                        (rect.min, pos2(rect.max.x, rect.min.y)),
                        (pos2(rect.max.x, rect.min.y), rect.max),
                        (rect.max, pos2(rect.min.x, rect.max.y)),
                        (pos2(rect.min.x, rect.max.y), rect.min),
                    ];
                    edges
                        .iter()
                        .any(|&(a, b)| dist_to_segment(pos, a, b) <= threshold)
                }
            }
            Shape::Line { start, end } | Shape::Arrow { start, end } => {
                dist_to_segment(pos, *start, *end) <= threshold
            }
            Shape::Circle {
                center,
                radius,
                filled,
            } => {
                let dx = (pos.x - center.x) / radius.x.max(1.0);
                let dy = (pos.y - center.y) / radius.y.max(1.0);
                let d = (dx * dx + dy * dy).sqrt();
                if *filled {
                    d <= 1.0
                } else {
                    (d - 1.0).abs() * radius.x.min(radius.y) <= threshold
                }
            }
            Shape::Pen { points, .. } => points
                .windows(2)
                .any(|w| dist_to_segment(pos, w[0], w[1]) <= threshold),
            Shape::Highlighter { rect } => rect.contains(pos),
        }
    }

    /// Bounding box of the rendered shape, padded for stroke width (and the
    /// arrow head). Used to size the live-preview image.
    pub fn bounds(&self, width: f32) -> Rect {
        let pad = width + 1.0;
        match self {
            Shape::Rectangle { rect, .. } => rect.expand(pad),
            Shape::Line { start, end } => Rect::from_two_pos(*start, *end).expand(pad),
            Shape::Arrow { start, end } => {
                Rect::from_two_pos(*start, *end).expand(12.0 + width * 1.5 + 6.0 + width)
            }
            Shape::Circle { center, radius, .. } => {
                Rect::from_center_size(*center, *radius * 2.0).expand(pad)
            }
            Shape::Pen { points, .. } => {
                let mut rect = Rect::NOTHING;
                for p in points {
                    rect.extend_with(*p);
                }
                if rect == Rect::NOTHING {
                    rect = Rect::from_min_size(Pos2::ZERO, Vec2::ZERO);
                }
                rect.expand(pad)
            }
            Shape::Highlighter { rect } => rect.expand(1.0),
        }
    }

    pub fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        match self {
            Shape::Rectangle { rect, .. } => {
                (vec![rect.min.y, rect.max.y], vec![rect.min.x, rect.max.x])
            }
            Shape::Line { start, end } | Shape::Arrow { start, end } => {
                (vec![start.y, end.y], vec![start.x, end.x])
            }
            Shape::Circle { center, radius, .. } => (
                vec![
                    center.y,
                    center.y - radius.y,
                    center.y + radius.y,
                ],
                vec![
                    center.x,
                    center.x - radius.x,
                    center.x + radius.x,
                ],
            ),
            Shape::Pen { points, .. } => match (points.first(), points.last()) {
                (Some(first), Some(last)) => (vec![first.y, last.y], vec![first.x, last.x]),
                _ => (Vec::new(), Vec::new()),
            },
            Shape::Highlighter { rect } => {
                (vec![rect.min.y, rect.max.y], vec![rect.min.x, rect.max.x])
            }
        }
    }
}

fn arrow_head(start: Pos2, end: Pos2, width: f32) -> (Pos2, Pos2, Pos2) {
    let delta = end - start;
    let len = delta.length();
    let dir = if len > 0.001 { delta / len } else { Vec2::ZERO };
    let perp = Vec2::new(-dir.y, dir.x);
    let head_len = 12.0 + width * 1.5;
    let head_w = 6.0 + width;
    (
        end,
        end - dir * head_len + perp * head_w,
        end - dir * head_len - perp * head_w,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]))
    }

    #[test]
    fn rectangle_outline_stamps_edges_only() {
        let mut image = blank(40, 40);
        let mut shape = Shape::begin(ToolKind::Rectangle, pos2(5.0, 5.0)).unwrap();
        shape.update(pos2(35.0, 35.0), false, None);
        shape.render(&mut image, Color32::RED, 1.0, Pos2::ZERO);
        assert_ne!(image.get_pixel(5, 20), &image::Rgba([0, 0, 0, 255]));
        assert_eq!(image.get_pixel(20, 20), &image::Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn filled_rectangle_stamps_interior() {
        let mut image = blank(40, 40);
        let mut shape = Shape::begin(ToolKind::FilledRectangle, pos2(5.0, 5.0)).unwrap();
        shape.update(pos2(35.0, 35.0), false, None);
        shape.render(&mut image, Color32::RED, 1.0, Pos2::ZERO);
        assert_eq!(image.get_pixel(20, 20), &image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn ctrl_squares_a_rectangle() {
        let mut shape = Shape::begin(ToolKind::Rectangle, pos2(0.0, 0.0)).unwrap();
        shape.update(pos2(10.0, 30.0), true, None);
        match shape {
            Shape::Rectangle { rect, .. } => {
                assert!((rect.width() - rect.height()).abs() < 0.01);
                assert!((rect.width() - 30.0).abs() < 0.01);
            }
            _ => panic!("expected rectangle"),
        }
    }

    #[test]
    fn bounds_cover_the_shape_and_pad() {
        let mut shape = Shape::begin(ToolKind::Rectangle, pos2(10.0, 10.0)).unwrap();
        shape.update(pos2(30.0, 40.0), false, None);
        let b = shape.bounds(2.0);
        assert!(b.min.x <= 7.0 && b.min.y <= 7.0);
        assert!(b.max.x >= 33.0 && b.max.y >= 43.0);
    }

    #[test]
    fn offset_shifts_the_stamp() {
        let mut image = blank(20, 20);
        let mut shape = Shape::begin(ToolKind::FilledRectangle, pos2(10.0, 10.0)).unwrap();
        shape.update(pos2(18.0, 18.0), false, None);
        shape.render(&mut image, Color32::RED, 1.0, pos2(10.0, 10.0));
        assert_eq!(image.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]));
    }
}
