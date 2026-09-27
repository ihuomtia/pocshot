//! Selection handles: geometry, hit-testing and edge/corner resizing.

use crate::{pos2, vec2, Pos2, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleType {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

pub const HANDLE_SIZE: f32 = 8.0;

pub fn handle_rects(rect: Rect) -> [(HandleType, Rect); 8] {
    [
        (
            HandleType::TopLeft,
            Rect::from_center_size(rect.left_top(), vec2(HANDLE_SIZE, HANDLE_SIZE)),
        ),
        (
            HandleType::TopRight,
            Rect::from_center_size(rect.right_top(), vec2(HANDLE_SIZE, HANDLE_SIZE)),
        ),
        (
            HandleType::BottomLeft,
            Rect::from_center_size(rect.left_bottom(), vec2(HANDLE_SIZE, HANDLE_SIZE)),
        ),
        (
            HandleType::BottomRight,
            Rect::from_center_size(rect.right_bottom(), vec2(HANDLE_SIZE, HANDLE_SIZE)),
        ),
        (
            HandleType::Top,
            Rect::from_center_size(
                pos2(rect.center().x, rect.top()),
                vec2(HANDLE_SIZE, HANDLE_SIZE),
            ),
        ),
        (
            HandleType::Bottom,
            Rect::from_center_size(
                pos2(rect.center().x, rect.bottom()),
                vec2(HANDLE_SIZE, HANDLE_SIZE),
            ),
        ),
        (
            HandleType::Left,
            Rect::from_center_size(
                pos2(rect.left(), rect.center().y),
                vec2(HANDLE_SIZE, HANDLE_SIZE),
            ),
        ),
        (
            HandleType::Right,
            Rect::from_center_size(
                pos2(rect.right(), rect.center().y),
                vec2(HANDLE_SIZE, HANDLE_SIZE),
            ),
        ),
    ]
}

/// Handle under `pos`, if any.
pub fn handle_at(rect: Rect, pos: Pos2) -> Option<HandleType> {
    handle_rects(rect)
        .into_iter()
        .find(|(_, handle)| handle.contains(pos))
        .map(|(handle, _)| handle)
}

/// Resize `rect` by dragging `handle` to `pos`. Opposite edges stay put; the
/// selection never collapses below 1 px.
pub fn resize(rect: Rect, handle: HandleType, pos: Pos2) -> Rect {
    let mut r = rect;
    match handle {
        HandleType::TopLeft => {
            r.min.x = pos.x.min(r.max.x - 1.0);
            r.min.y = pos.y.min(r.max.y - 1.0);
        }
        HandleType::TopRight => {
            r.max.x = pos.x.max(r.min.x + 1.0);
            r.min.y = pos.y.min(r.max.y - 1.0);
        }
        HandleType::BottomLeft => {
            r.min.x = pos.x.min(r.max.x - 1.0);
            r.max.y = pos.y.max(r.min.y + 1.0);
        }
        HandleType::BottomRight => {
            r.max.x = pos.x.max(r.min.x + 1.0);
            r.max.y = pos.y.max(r.min.y + 1.0);
        }
        HandleType::Top => {
            r.min.y = pos.y.min(r.max.y - 1.0);
        }
        HandleType::Bottom => {
            r.max.y = pos.y.max(r.min.y + 1.0);
        }
        HandleType::Left => {
            r.min.x = pos.x.min(r.max.x - 1.0);
        }
        HandleType::Right => {
            r.max.x = pos.x.max(r.min.x + 1.0);
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_max(pos2(10.0, 10.0), pos2(50.0, 40.0))
    }

    #[test]
    fn handle_at_finds_corners_and_edges() {
        assert_eq!(handle_at(rect(), pos2(10.0, 10.0)), Some(HandleType::TopLeft));
        assert_eq!(
            handle_at(rect(), pos2(50.0, 40.0)),
            Some(HandleType::BottomRight)
        );
        assert_eq!(handle_at(rect(), pos2(30.0, 10.0)), Some(HandleType::Top));
        assert_eq!(
            handle_at(rect(), pos2(50.0, 25.0)),
            Some(HandleType::Right)
        );
        assert_eq!(handle_at(rect(), pos2(30.0, 25.0)), None);
    }

    #[test]
    fn resize_moves_only_the_dragged_edges() {
        let r = resize(rect(), HandleType::BottomRight, pos2(70.0, 60.0));
        assert_eq!(r.min, pos2(10.0, 10.0));
        assert_eq!(r.max, pos2(70.0, 60.0));

        let r = resize(rect(), HandleType::Left, pos2(2.0, 25.0));
        assert_eq!(r.min, pos2(2.0, 10.0));
        assert_eq!(r.max, pos2(50.0, 40.0));
    }

    #[test]
    fn resize_never_collapses_below_one_pixel() {
        let r = resize(rect(), HandleType::TopLeft, pos2(200.0, 200.0));
        assert!(r.width() >= 1.0);
        assert!(r.height() >= 1.0);
    }
}
