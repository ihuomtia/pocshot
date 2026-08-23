use eframe::egui::{pos2, vec2, Pos2, Rect};

#[derive(Debug, Clone, Copy)]
pub struct ImageSelection {
    pub start: Pos2,
    pub end: Pos2,
}

impl ImageSelection {
    pub fn rect(self) -> Rect {
        Rect::from_two_pos(self.start, self.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
