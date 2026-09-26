//! Painter-free annotation shapes. Each shape knows how to update itself from
//! drag positions and how to stamp itself onto an `RgbaImage`. This is the
//! model the Slint UI drives.

use ecolor::Color32;
use emath::{pos2, Pos2, Rect, Vec2};
use image::RgbaImage;

use crate::constrain::apply_constraint;
use crate::counter_font::{
    contrast_fg, draw_number_on_image, draw_text_on_image, halo_color, measure_text,
};
use crate::raster::{
    dist_to_segment, draw_circle_on_image, draw_line_on_image, fill_circle_on_image,
    fill_ellipse_on_image, fill_rect_on_image, fill_triangle_on_image,
};
use crate::ToolKind;

/// Fixed radius of the numbered counter bubble (independent of stroke width).
pub const COUNTER_BUBBLE_RADIUS: f32 = 18.0;
/// Width of the callout triangle base, through the bubble center.
const COUNTER_CALLOUT_BASE: f32 = 28.0;
/// Minimum drag distance before a counter drag produces a callout triangle.
const COUNTER_CALLOUT_THRESHOLD: f32 = 8.0;

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
    Text {
        anchor: Pos2,
        text: String,
        size: f32,
    },
    Counter {
        center: Pos2,
        number: u32,
        target: Option<Pos2>,
        start: Pos2,
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

    /// Start a numbered counter bubble at `pos`.
    pub fn begin_counter(pos: Pos2, number: u32) -> Self {
        Shape::Counter {
            center: pos,
            number,
            target: None,
            start: pos,
        }
    }

    /// A text label anchored at `anchor` (top-left).
    pub fn text(anchor: Pos2, text: impl Into<String>, size: f32) -> Self {
        Shape::Text {
            anchor,
            text: text.into(),
            size,
        }
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
            Shape::Text { .. } => ToolKind::Text,
            Shape::Counter { .. } => ToolKind::Counter,
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
            Shape::Counter { start, target, .. } => {
                if pos.distance(*start) >= COUNTER_CALLOUT_THRESHOLD {
                    *target = Some(pos);
                }
                None
            }
            Shape::Text { .. } => None,
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
            Shape::Text { anchor, text, size } => {
                draw_text_on_image(image, *anchor - off, text, *size, color, halo_color(color));
            }
            Shape::Counter {
                center,
                number,
                target,
                ..
            } => {
                let c = *center - off;
                let radius = COUNTER_BUBBLE_RADIUS;
                if let Some(tip) = target {
                    let (a, b, t) = counter_callout_triangle(*center, *tip);
                    fill_triangle_on_image(image, a - off, b - off, t - off, color);
                }
                fill_circle_on_image(image, c, radius, color);
                // Two-tone rim mirroring the on-canvas look.
                draw_circle_on_image(image, c, radius, 2.0, Color32::WHITE);
                draw_circle_on_image(image, c, radius + 2.0, 1.0, Color32::from_rgb(20, 20, 20));
                draw_number_on_image(image, c, *number, radius, contrast_fg(color));
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
            Shape::Text { anchor, text, size } => {
                let (w, h) = measure_text(text, *size);
                Rect::from_min_size(*anchor, Vec2::new(w, h))
                    .expand(threshold)
                    .contains(pos)
            }
            Shape::Counter { center, .. } => {
                pos.distance(*center) <= COUNTER_BUBBLE_RADIUS + threshold
            }
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
            Shape::Text { anchor, text, size } => {
                let (w, h) = measure_text(text, *size);
                Rect::from_min_size(*anchor, Vec2::new(w, h)).expand(1.0)
            }
            Shape::Counter {
                center,
                target,
                ..
            } => {
                let mut rect = Rect::from_center_size(
                    *center,
                    Vec2::splat((COUNTER_BUBBLE_RADIUS + 3.0) * 2.0),
                );
                if let Some(tip) = target {
                    rect.extend_with(*tip);
                }
                rect.expand(1.0)
            }
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
            Shape::Text { anchor, text, size } => {
                let (w, h) = measure_text(text, *size);
                (vec![anchor.y, anchor.y + h], vec![anchor.x, anchor.x + w])
            }
            Shape::Counter {
                center,
                target,
                ..
            } => {
                let mut horiz = vec![center.y];
                let mut vert = vec![center.x];
                if let Some(tip) = target {
                    horiz.push(tip.y);
                    vert.push(tip.x);
                }
                (horiz, vert)
            }
        }
    }
}

/// Callout triangle for a counter bubble at `center` pointing at `tip`: the
/// base passes through the center perpendicular to the direction, apex is tip.
fn counter_callout_triangle(center: Pos2, tip: Pos2) -> (Pos2, Pos2, Pos2) {
    let d = tip - center;
    let len = d.length();
    let dir = if len > 1e-3 { d / len } else { Vec2::X };
    let perp = Vec2::new(-dir.y, dir.x);
    let half = COUNTER_CALLOUT_BASE * 0.5;
    (center + perp * half, center - perp * half, tip)
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
    fn counter_renders_bubble_and_callout() {
        let mut image = blank(300, 300);
        let mut shape = Shape::begin_counter(pos2(100.0, 100.0), 7);
        shape.update(pos2(250.0, 100.0), false, None);
        shape.render(&mut image, Color32::from_rgb(239, 68, 68), 5.0, Pos2::ZERO);
        let center = image.get_pixel(100, 100);
        assert!(center.0[0] > 0, "bubble center should be coloured");
        let mid = image.get_pixel(175, 100);
        assert!(mid.0[0] > 0, "callout interior should be coloured");
    }

    #[test]
    fn counter_number_increments_externally() {
        let shape = Shape::begin_counter(pos2(10.0, 10.0), 3);
        match shape {
            Shape::Counter { number, .. } => assert_eq!(number, 3),
            _ => panic!("expected counter"),
        }
    }

    #[test]
    fn text_renders_glyph_pixels() {
        let mut image = blank(300, 300);
        let shape = Shape::text(pos2(50.0, 50.0), "Text", 24.0);
        shape.render(&mut image, Color32::from_rgb(255, 60, 60), 3.0, Pos2::ZERO);
        let mut coloured = 0u32;
        for y in 50..100 {
            for x in 50..200 {
                let p = image.get_pixel(x, y);
                if p.0[0] > 30 || p.0[1] > 30 || p.0[2] > 30 {
                    coloured += 1;
                }
            }
        }
        assert!(coloured > 0, "expected stamped glyph pixels");
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
