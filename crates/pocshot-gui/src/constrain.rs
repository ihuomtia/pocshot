use eframe::egui::{Pos2, Vec2};
use std::f32::consts::FRAC_PI_4;

use crate::annotation::AnnotationTool;

const SNAP_STEP: f32 = FRAC_PI_4;

pub fn constrain_to_square(anchor: Pos2, current: Pos2) -> Pos2 {
    let d = current - anchor;
    let side = d.x.abs().max(d.y.abs());
    let sx = if d.x >= 0.0 { 1.0 } else { -1.0 };
    let sy = if d.y >= 0.0 { 1.0 } else { -1.0 };
    Pos2::new(anchor.x + sx * side, anchor.y + sy * side)
}

pub fn constrain_to_circle(anchor: Pos2, current: Pos2) -> Vec2 {
    let d = current - anchor;
    let r = d.x.abs().max(d.y.abs());
    Vec2::splat(r)
}

pub fn snap_angle(angle: f32) -> f32 {
    (angle / SNAP_STEP).round() * SNAP_STEP
}

pub fn snap_point_to_45(anchor: Pos2, current: Pos2) -> Pos2 {
    let d = current - anchor;
    if d.length_sq() < f32::EPSILON {
        return anchor;
    }
    let angle = snap_angle(d.y.atan2(d.x));
    let len = d.length();
    Pos2::new(anchor.x + angle.cos() * len, anchor.y + angle.sin() * len)
}

pub fn constrain_to_line(anchor: Pos2, current: Pos2, angle: f32) -> Pos2 {
    let dir = Vec2::new(angle.cos(), angle.sin());
    let proj_len = (current - anchor).dot(dir);
    Pos2::new(anchor.x + dir.x * proj_len, anchor.y + dir.y * proj_len)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintResult {
    pub point: Pos2,
    pub locked_angle: Option<f32>,
}

pub fn apply_constraint(
    tool: AnnotationTool,
    anchor: Pos2,
    current: Pos2,
    ctrl: bool,
    locked_angle: Option<f32>,
) -> ConstraintResult {
    if !ctrl {
        return ConstraintResult {
            point: current,
            locked_angle: None,
        };
    }

    let point = match tool {
        AnnotationTool::Rectangle | AnnotationTool::FilledRectangle => {
            constrain_to_square(anchor, current)
        }
        AnnotationTool::Circle | AnnotationTool::FilledCircle => {
            let r = constrain_to_circle(anchor, current);
            Pos2::new(anchor.x + r.x, anchor.y + r.y)
        }
        AnnotationTool::Line | AnnotationTool::Arrow => snap_point_to_45(anchor, current),
        AnnotationTool::Pen | AnnotationTool::Highlighter => {
            let angle = match locked_angle {
                Some(a) => a,
                None => snap_angle((current - anchor).y.atan2((current - anchor).x)),
            };
            return ConstraintResult {
                point: constrain_to_line(anchor, current, angle),
                locked_angle: Some(angle),
            };
        }
        _ => current,
    };

    ConstraintResult {
        point,
        locked_angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn assert_rect_square(anchor: Pos2, end: Pos2) {
        let dx = end.x - anchor.x;
        let dy = end.y - anchor.y;
        assert!(approx(dx.abs(), dy.abs()), "not square: dx={dx} dy={dy}");
    }

    #[test]
    fn square_all_four_directions() {
        let a = pos2(100.0, 100.0);
        for (cx, cy) in [(150.0, 130.0), (150.0, 70.0), (50.0, 130.0), (50.0, 70.0)] {
            let end = constrain_to_square(a, pos2(cx, cy));
            assert_rect_square(a, end);
            let dx = end.x - a.x;
            let dy = end.y - a.y;
            assert_eq!(dx.is_sign_positive(), cx >= 100.0);
            assert_eq!(dy.is_sign_positive(), cy >= 100.0);
        }
    }

    #[test]
    fn square_uses_dominant_axis() {
        let a = pos2(0.0, 0.0);
        let end = constrain_to_square(a, pos2(10.0, 30.0));
        assert!(approx(end.x, 30.0));
        assert!(approx(end.y, 30.0));
    }

    #[test]
    fn circle_all_four_directions() {
        let a = pos2(100.0, 100.0);
        for (cx, cy) in [(150.0, 130.0), (150.0, 70.0), (50.0, 130.0), (50.0, 70.0)] {
            let r = constrain_to_circle(a, pos2(cx, cy));
            assert!(approx(r.x, r.y), "not circle: rx={} ry={}", r.x, r.y);
            assert!(approx(r.x, 50.0));
        }
    }

    fn snapped_deg(anchor: Pos2, angle_deg: f32, len: f32) -> f32 {
        let rad = angle_deg.to_radians();
        let end = snap_point_to_45(
            anchor,
            pos2(anchor.x + rad.cos() * len, anchor.y + rad.sin() * len),
        );
        let d = end - anchor;
        let deg = d.y.atan2(d.x).to_degrees();
        (deg % 360.0 + 360.0) % 360.0
    }

    #[test]
    fn line_snaps_around_45_degree_boundaries() {
        let a = pos2(0.0, 0.0);
        for &(deg, expect) in &[
            (0.0, 0.0),
            (21.0, 0.0),
            (24.0, 45.0),
            (44.0, 45.0),
            (46.0, 45.0),
            (69.0, 90.0),
            (90.0, 90.0),
            (114.0, 135.0),
            (135.0, 135.0),
            (156.0, 135.0),
            (159.0, 180.0),
            (180.0, 180.0),
            (201.0, 180.0),
            (204.0, 225.0),
            (226.0, 225.0),
            (249.0, 270.0),
            (294.0, 315.0),
            (336.0, 315.0),
            (339.0, 0.0),
            (300.0, 315.0),
        ] {
            let got = snapped_deg(a, deg, 50.0);
            assert!(
                approx(got, expect),
                "angle {deg} deg snapped to {got}, expected {expect}"
            );
        }
    }

    #[test]
    fn ctrl_held_from_drag_start() {
        let a = pos2(0.0, 0.0);
        let cur = pos2(10.0, 10.5);
        let r = apply_constraint(AnnotationTool::Line, a, cur, true, None);
        let d = r.point - a;
        assert!(approx(d.y.atan2(d.x).to_degrees(), 45.0));
        assert!(approx(d.length(), (cur - a).length()));
    }

    #[test]
    fn no_ctrl_leaves_geometry_unchanged() {
        let a = pos2(5.0, 7.0);
        let cur = pos2(23.0, 19.0);
        for tool in [
            AnnotationTool::Rectangle,
            AnnotationTool::Circle,
            AnnotationTool::Line,
            AnnotationTool::Arrow,
            AnnotationTool::Pen,
            AnnotationTool::Highlighter,
        ] {
            let r = apply_constraint(tool, a, cur, false, None);
            assert_eq!(r.point, cur, "{tool:?} changed without ctrl");
            assert_eq!(r.locked_angle, None);
        }
    }

    #[test]
    fn ctrl_pressed_mid_drag_locks_angle() {
        let a = pos2(0.0, 0.0);
        let free = apply_constraint(AnnotationTool::Pen, a, pos2(8.0, 1.0), false, None);
        assert_eq!(free.point, pos2(8.0, 1.0));
        assert_eq!(free.locked_angle, None);

        let first = apply_constraint(AnnotationTool::Pen, a, pos2(8.0, 2.0), true, None);
        let angle = first.locked_angle.expect("should lock");
        assert!(approx(angle, 0.0) || approx(angle.abs(), FRAC_PI_4));

        let second = apply_constraint(AnnotationTool::Pen, a, pos2(9.0, 7.0), true, Some(angle));
        assert_eq!(second.locked_angle, Some(angle));
        let d = second.point - a;
        assert!(approx(d.y.atan2(d.x), angle), "locked angle drifted");
    }

    #[test]
    fn ctrl_released_mid_drag_restores_freehand() {
        let a = pos2(0.0, 0.0);
        let locked = apply_constraint(AnnotationTool::Pen, a, pos2(9.0, 9.0), true, None);
        let released = apply_constraint(
            AnnotationTool::Pen,
            a,
            pos2(12.0, 3.0),
            false,
            locked.locked_angle,
        );
        assert_eq!(released.point, pos2(12.0, 3.0));
        assert_eq!(released.locked_angle, None);
    }

    #[test]
    fn highlighter_uses_projection_onto_locked_line() {
        let a = pos2(0.0, 0.0);
        let r = apply_constraint(AnnotationTool::Highlighter, a, pos2(20.0, 20.0), true, None);
        let angle = r.locked_angle.unwrap();
        let d = r.point - a;
        assert!(approx(d.y.atan2(d.x), angle));
        assert!(approx(d.length(), 20.0 * 2.0f32.sqrt()));

        let drifted = apply_constraint(
            AnnotationTool::Highlighter,
            a,
            pos2(20.0, 5.0),
            true,
            Some(angle),
        );
        let dd = drifted.point - a;
        assert!(approx(dd.x, dd.y));
    }

    #[test]
    fn circle_constraint_preserves_anchor() {
        let a = pos2(50.0, 60.0);
        let r = apply_constraint(AnnotationTool::Circle, a, pos2(80.0, 90.0), true, None);
        let d = r.point - a;
        assert!(approx(d.x.abs(), 30.0));
        assert!(approx(d.y.abs(), 30.0));
    }

    #[test]
    fn zero_delta_does_not_nan() {
        let a = pos2(10.0, 10.0);
        let r = apply_constraint(AnnotationTool::Line, a, a, true, None);
        assert!(r.point.x.is_finite() && r.point.y.is_finite());
    }
}
