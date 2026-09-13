use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

pub fn fit_rect(image_size: Vec2, bounds: Rect) -> Rect {
    let scale = (bounds.width() / image_size.x)
        .min(bounds.height() / image_size.y)
        .max(0.01);
    let size = image_size * scale;
    Rect::from_center_size(bounds.center(), size)
}

pub fn screen_to_image(position: Pos2, draw_rect: Rect, image_size: Vec2) -> Pos2 {
    let x = ((position.x - draw_rect.min.x) / draw_rect.width() * image_size.x)
        .clamp(0.0, image_size.x);
    let y = ((position.y - draw_rect.min.y) / draw_rect.height() * image_size.y)
        .clamp(0.0, image_size.y);
    pos2(x, y)
}

pub fn image_to_screen_rect(rect: Rect, draw_rect: Rect, image_size: Vec2) -> Rect {
    let min = pos2(
        draw_rect.min.x + rect.min.x / image_size.x * draw_rect.width(),
        draw_rect.min.y + rect.min.y / image_size.y * draw_rect.height(),
    );
    let max = pos2(
        draw_rect.min.x + rect.max.x / image_size.x * draw_rect.width(),
        draw_rect.min.y + rect.max.y / image_size.y * draw_rect.height(),
    );
    Rect::from_min_max(min, max)
}

pub fn normalized_uv(rect: Rect, image_size: Vec2) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x / image_size.x, rect.min.y / image_size.y),
        pos2(rect.max.x / image_size.x, rect.max.y / image_size.y),
    )
}

pub fn capture_size(image: &image::RgbaImage) -> Vec2 {
    vec2(image.width() as f32, image.height() as f32)
}

pub fn clamp_image_rect(rect: Rect, image: &image::RgbaImage) -> Option<Rect> {
    clamp_rect_to_size(rect, capture_size(image))
}

/// Translate `rect` by `delta` (image px), clamping so the result stays fully
/// inside the image. Size is preserved. Used when dragging a committed
/// selection around.
pub fn translate_rect_clamped(rect: Rect, delta: Vec2, image_size: Vec2) -> Rect {
    let size = rect.size();
    let bounds = Rect::from_min_size(Pos2::ZERO, image_size);
    let mut min = rect.min + delta;
    min.x = min
        .x
        .clamp(bounds.min.x, (bounds.max.x - size.x).max(bounds.min.x));
    min.y = min
        .y
        .clamp(bounds.min.y, (bounds.max.y - size.y).max(bounds.min.y));
    Rect::from_min_size(min, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_clamps_to_image_bounds() {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(100.0, 50.0));
        let size = vec2(200.0, 200.0);
        // Free movement within bounds.
        let moved = translate_rect_clamped(rect, vec2(20.0, 30.0), size);
        assert_eq!(moved.min, pos2(30.0, 40.0));
        assert_eq!(moved.size(), rect.size());
        // Overshooting left/top pins at the origin.
        let pinned = translate_rect_clamped(rect, vec2(-500.0, -500.0), size);
        assert_eq!(pinned.min, pos2(0.0, 0.0));
        // Overshooting right/bottom pins the far edge at the image edge.
        let pinned = translate_rect_clamped(rect, vec2(500.0, 500.0), size);
        assert_eq!(pinned.max, pos2(200.0, 200.0));
    }
}

pub fn clamp_rect_to_size(rect: Rect, image_size: Vec2) -> Option<Rect> {
    let bounds = Rect::from_min_size(Pos2::ZERO, image_size);
    let min = pos2(
        rect.min.x.min(rect.max.x).clamp(bounds.min.x, bounds.max.x),
        rect.min.y.min(rect.max.y).clamp(bounds.min.y, bounds.max.y),
    );
    let max = pos2(
        rect.min.x.max(rect.max.x).clamp(bounds.min.x, bounds.max.x),
        rect.min.y.max(rect.max.y).clamp(bounds.min.y, bounds.max.y),
    );

    let width = (max.x - min.x).round();
    let height = (max.y - min.y).round();
    if width < 1.0 || height < 1.0 {
        return None;
    }

    Some(Rect::from_min_max(min.round(), max.round()))
}
