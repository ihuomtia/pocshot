//! Counter bubble tool: a numbered circle with an optional callout tip. A
//! plain click drops a bare bubble; dragging extends a callout triangle from
//! the bubble center to the release point.

use eframe::egui::{self, pos2, Align2, Color32, Painter, Pos2, Stroke, Vec2};
use image::RgbaImage;

use super::counter_font::{contrast_fg, draw_number_on_image};
use super::raster::{draw_circle_on_image, fill_circle_on_image, fill_triangle_on_image};

/// Fixed radius of the numbered counter bubble (independent of stroke width),
/// shared by the on-canvas preview, the committed shape and the image export.
pub const COUNTER_BUBBLE_RADIUS: f32 = 18.0;

/// Width of the callout triangle base, measured through the bubble center
/// perpendicular to the pointing direction.
const COUNTER_CALLOUT_BASE: f32 = 28.0;

/// Minimum drag distance (image px) before a Counter drag produces a callout
/// triangle rather than just a bubble.
const COUNTER_CALLOUT_THRESHOLD: f32 = 8.0;

#[derive(Debug, Clone, Default)]
pub struct CounterTool {
    center: Pos2,
    number: u32,
    target: Option<Pos2>,
    start: Option<Pos2>,
}

impl CounterTool {
    pub fn new(number: u32) -> Self {
        Self {
            center: Pos2::ZERO,
            number,
            target: None,
            start: None,
        }
    }
}

impl super::Tool for CounterTool {
    fn start(&mut self, pos: Pos2) {
        self.center = pos;
        self.start = Some(pos);
        self.target = None;
    }

    fn update(&mut self, pos: Pos2, _ctrl: bool, _locked_angle: Option<f32>) -> Option<f32> {
        if let Some(start) = self.start {
            if pos.distance(start) >= COUNTER_CALLOUT_THRESHOLD {
                self.target = Some(pos);
            }
        }
        None
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, _width: f32) {
        let center = map(self.center);
        let radius = COUNTER_BUBBLE_RADIUS;
        if let Some(tip) = self.target {
            let tri = counter_callout_triangle(center, map(tip));
            painter.add(egui::Shape::convex_polygon(
                vec![tri.0, tri.1, tri.2],
                color,
                egui::Stroke::NONE,
            ));
        }
        painter.circle_filled(center, radius, color);
        // Two-tone rim for contrast: a white band (2px) just outside the fill
        // with a thin black (1px) outer edge. Fill radius stays the same.
        painter.circle_stroke(center, radius + 1.0, Stroke::new(2.0_f32, Color32::WHITE));
        painter.circle_stroke(
            center,
            radius + 2.5,
            Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
        );
        let fg = contrast_fg(color);
        // Fake-bold: stamp the label at a few small offsets so the light font
        // reads heavier (matches the exported PNG).
        let label = self.number.to_string();
        let font = egui::FontId::proportional(radius * 0.95);
        for (sx, sy) in [
            (0.0f32, 0.0f32),
            (0.7, 0.7),
            (-0.7, 0.7),
            (0.7, -0.7),
            (-0.7, -0.7),
        ] {
            painter.text(
                pos2(center.x + sx, center.y + sy),
                Align2::CENTER_CENTER,
                &label,
                font.clone(),
                fg,
            );
        }
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, _width: f32, offset: Pos2) {
        let o = offset.to_vec2();
        let c = self.center - o;
        let radius = COUNTER_BUBBLE_RADIUS;
        if let Some(tip) = self.target {
            let tri = counter_callout_triangle(self.center, tip);
            fill_triangle_on_image(image, tri.0 - o, tri.1 - o, tri.2 - o, color);
        }
        fill_circle_on_image(image, c, radius, color);
        // Two-tone rim mirroring the on-canvas look.
        draw_circle_on_image(image, c, radius, 2.0, Color32::WHITE);
        draw_circle_on_image(image, c, radius + 2.0, 1.0, Color32::from_rgb(20, 20, 20));
        draw_number_on_image(image, c, self.number, radius, contrast_fg(color));
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, _width: f32) -> bool {
        pos.distance(self.center) <= COUNTER_BUBBLE_RADIUS + threshold
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        let mut horiz = vec![self.center.y];
        let mut vert = vec![self.center.x];
        if let Some(tip) = self.target {
            horiz.push(tip.y);
            vert.push(tip.x);
        }
        (horiz, vert)
    }
}

/// Compute the callout triangle for a counter bubble centered at `center`
/// pointing at `tip`: the base passes through the bubble center perpendicular
/// to the pointing direction, and the apex is the tip.
fn counter_callout_triangle(center: Pos2, tip: Pos2) -> (Pos2, Pos2, Pos2) {
    let d = tip - center;
    let len = d.length();
    let dir = if len > 1e-3 { d / len } else { Vec2::X };
    let perp = Vec2::new(-dir.y, dir.x);
    let half = COUNTER_CALLOUT_BASE * 0.5;
    let b1 = center + perp * half;
    let b2 = center - perp * half;
    (b1, b2, tip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::Tool;

    fn pos2(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    #[test]
    fn counter_radius_is_fixed() {
        assert_eq!(COUNTER_BUBBLE_RADIUS, 18.0);
    }

    #[test]
    fn counter_callout_triangle_has_apex_at_target() {
        let center = pos2(100.0, 100.0);
        let tip = pos2(200.0, 100.0);
        let (a, b, c) = counter_callout_triangle(center, tip);
        // Apex is the target.
        assert_eq!(c, tip);
        // Base is symmetric about the center, perpendicular to direction.
        let base_y = (a.y + b.y) * 0.5;
        assert!((base_y - center.y).abs() < 1.0);
        assert!(((a.x + b.x) * 0.5 - center.x).abs() < 1.0);
    }

    #[test]
    fn counter_hit_test_on_center() {
        let center = pos2(100.0, 100.0);
        let mut tool = CounterTool::new(5);
        tool.center = center;
        assert!(tool.hit_test(center, 4.0, 5.0));
        assert!(tool.hit_test(
            pos2(center.x + COUNTER_BUBBLE_RADIUS * 0.5, center.y),
            4.0,
            5.0
        ));
        assert!(!tool.hit_test(
            pos2(center.x + COUNTER_BUBBLE_RADIUS * 3.0, center.y),
            4.0,
            5.0
        ));
    }

    #[test]
    fn counter_snap_lines_expose_center_and_target() {
        let mut tool = CounterTool::new(3);
        tool.center = pos2(64.0, 32.0);
        tool.target = Some(pos2(200.0, 150.0));
        let (h, v) = tool.snap_lines();
        assert!(h.contains(&32.0));
        assert!(h.contains(&150.0));
        assert!(v.contains(&64.0));
        assert!(v.contains(&200.0));
    }

    #[test]
    fn contrast_fg_opposes_light_fill() {
        assert_eq!(contrast_fg(Color32::WHITE), Color32::from_rgb(20, 20, 20));
        assert_eq!(contrast_fg(Color32::BLACK), Color32::WHITE);
    }

    #[test]
    fn counter_renders_to_image_without_panic() {
        let mut img = RgbaImage::new(300, 300);
        let mut tool = CounterTool::new(42);
        tool.center = pos2(100.0, 100.0);
        tool.target = Some(pos2(250.0, 100.0));
        tool.render(
            &mut img,
            Color32::from_rgb(239, 68, 68),
            5.0,
            pos2(0.0, 0.0),
        );
        let px = img.get_pixel(100, 100);
        assert!(
            px.0[0] > 0 || px.0[1] > 0 || px.0[2] > 0,
            "bubble center should be colored, got {:?}",
            px.0
        );
        let mid = img.get_pixel(175, 100);
        assert!(
            mid.0[0] > 0 || mid.0[1] > 0 || mid.0[2] > 0,
            "triangle interior should be colored, got {:?}",
            mid.0
        );
    }

    #[test]
    fn counter_rim_white_then_black() {
        let mut img = RgbaImage::new(300, 80);
        let center = pos2(150.0, 40.0);
        let mut tool = CounterTool::new(1);
        tool.center = center;
        tool.render(
            &mut img,
            Color32::from_rgb(239, 68, 68),
            5.0,
            pos2(0.0, 0.0),
        );
        let r = COUNTER_BUBBLE_RADIUS as i32;
        let white = img.get_pixel((150 + r + 1) as u32, 40);
        assert!(
            white.0[0] > 200 && white.0[1] > 200 && white.0[2] > 200,
            "pixel just outside fill should be white rim, got {:?}",
            white.0
        );
        let black = img.get_pixel((150 + r + 2) as u32, 40);
        assert!(
            black.0[0] < 80 && black.0[1] < 80 && black.0[2] < 80,
            "outermost pixel should be black edge, got {:?}",
            black.0
        );
    }

    #[test]
    fn counter_digit_pixels_rendered_on_dark_bubble() {
        let mut img = RgbaImage::new(300, 300);
        let center = pos2(150.0, 150.0);
        let mut tool = CounterTool::new(7);
        tool.center = center;
        tool.render(&mut img, Color32::from_rgb(40, 40, 40), 5.0, pos2(0.0, 0.0));
        let inner = 8.0f32;
        let mut white = 0usize;
        for y in -(inner as i32)..=(inner as i32) {
            for x in -(inner as i32)..=(inner as i32) {
                if (x * x + y * y) as f32 <= inner * inner {
                    let p = img.get_pixel((150 + x) as u32, (150 + y) as u32);
                    if p.0[0] > 200 && p.0[1] > 200 && p.0[2] > 200 {
                        white += 1;
                    }
                }
            }
        }
        assert!(
            white > 0,
            "expected white digit pixels in bubble center, found 0"
        );
    }
}
