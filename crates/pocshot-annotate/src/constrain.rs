//! Drag constraints: Ctrl snaps rectangles/circles to square/circle and
//! lines/arrows to 45° steps. The pen/highlighter lock onto a fixed angle for
//! the whole drag once Ctrl is held.

use emath::{Pos2, Vec2};
use std::f32::consts::FRAC_PI_4;

use crate::ToolKind;

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
    tool: ToolKind,
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
        ToolKind::Rectangle | ToolKind::FilledRectangle => constrain_to_square(anchor, current),
        ToolKind::Circle | ToolKind::FilledCircle => {
            let r = constrain_to_circle(anchor, current);
            Pos2::new(anchor.x + r.x, anchor.y + r.y)
        }
        ToolKind::Line | ToolKind::Arrow => snap_point_to_45(anchor, current),
        ToolKind::Pen | ToolKind::Highlighter => {
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
    use emath::pos2;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn square_all_four_directions() {
        let a = pos2(100.0, 100.0);
        for (cx, cy) in [(150.0, 130.0), (150.0, 70.0), (50.0, 130.0), (50.0, 70.0)] {
            let end = constrain_to_square(a, pos2(cx, cy));
            assert!(approx((end.x - a.x).abs(), (end.y - a.y).abs()));
            assert_eq!((end.x - a.x).is_sign_positive(), cx >= 100.0);
            assert_eq!((end.y - a.y).is_sign_positive(), cy >= 100.0);
        }
    }

    #[test]
    fn line_snaps_to_45_steps() {
        let a = pos2(0.0, 0.0);
        let end = snap_point_to_45(a, pos2(10.0, 10.5));
        let d = end - a;
        assert!(approx(d.y.atan2(d.x).to_degrees(), 45.0));
        assert!(approx(d.length(), (pos2(10.0, 10.5) - a).length()));
    }

    #[test]
    fn no_ctrl_leaves_geometry_unchanged() {
        let a = pos2(5.0, 7.0);
        let cur = pos2(23.0, 19.0);
        for tool in [
            ToolKind::Rectangle,
            ToolKind::Circle,
            ToolKind::Line,
            ToolKind::Arrow,
            ToolKind::Pen,
            ToolKind::Highlighter,
        ] {
            let r = apply_constraint(tool, a, cur, false, None);
            assert_eq!(r.point, cur);
            assert_eq!(r.locked_angle, None);
        }
    }

    #[test]
    fn pen_locks_angle_until_released() {
        let a = pos2(0.0, 0.0);
        let first = apply_constraint(ToolKind::Pen, a, pos2(8.0, 2.0), true, None);
        let angle = first.locked_angle.expect("should lock");
        let second = apply_constraint(ToolKind::Pen, a, pos2(9.0, 7.0), true, Some(angle));
        assert_eq!(second.locked_angle, Some(angle));
        let d = second.point - a;
        assert!(approx(d.y.atan2(d.x), angle));
        let released = apply_constraint(ToolKind::Pen, a, pos2(12.0, 3.0), false, Some(angle));
        assert_eq!(released.locked_angle, None);
    }
}
