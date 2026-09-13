//! Text-border tool: drag over detected text and commit a rectangular border
//! around it.
//!
//! The gesture reuses the pixelate/blur drag fields (`effect_start` /
//! `effect_current`) because the tools are mutually exclusive; on release the
//! drag rect is expanded to the union of every OCR region it touches, padded,
//! and committed as an ordinary rectangle outline annotation (undoable).

use eframe::egui::{self, pos2, Pos2, Rect, Stroke, StrokeKind, Vec2};
use pocshot_ocr::TextRegion;

use crate::app::PocshotApp;
use crate::canvas::screen_to_image;
use crate::snap::{snap_pos, SNAP_DISTANCE};

/// Empty space left between the text and its border, in image pixels.
pub const TEXT_BORDER_PADDING: f32 = 6.0;

/// Union of the OCR regions that intersect `drag`, grown by `padding` and
/// clamped to the image. `None` when no region was touched.
pub fn text_border_rect(
    drag: Rect,
    regions: &[TextRegion],
    image_size: Vec2,
    padding: f32,
) -> Option<Rect> {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;

    for region in regions {
        let r = region.rect;
        let rr = Rect::from_min_max(pos2(r.x0, r.y0), pos2(r.x1, r.y1));
        if !rr.intersects(drag) {
            continue;
        }
        any = true;
        min = min.min(rr.min);
        max = max.max(rr.max);
    }

    if !any {
        return None;
    }

    Some(Rect::from_min_max(
        pos2(
            (min.x - padding).clamp(0.0, image_size.x),
            (min.y - padding).clamp(0.0, image_size.y),
        ),
        pos2(
            (max.x + padding).clamp(0.0, image_size.x),
            (max.y + padding).clamp(0.0, image_size.y),
        ),
    ))
}

impl PocshotApp {
    pub(crate) fn process_text_border_drag(
        &mut self,
        response: &egui::Response,
        snaps: (Vec<f32>, Vec<f32>),
        draw_rect: Rect,
        image_size: Vec2,
        snap_ok: bool,
    ) {
        let screen_to_img = |pos: Pos2| screen_to_image(pos, draw_rect, image_size);
        let snap = |img_pos: Pos2| -> Pos2 {
            if snap_ok {
                pos2(
                    snap_pos(img_pos.x, &snaps.1, SNAP_DISTANCE),
                    snap_pos(img_pos.y, &snaps.0, SNAP_DISTANCE),
                )
            } else {
                img_pos
            }
        };

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.effect_start = Some(snap(screen_to_img(pos)));
                self.effect_current = Some(snap(screen_to_img(pos)));
            }
        } else if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.effect_current = Some(snap(screen_to_img(pos)));
            }
        } else if response.drag_stopped() {
            let (Some(start), Some(cur)) = (self.effect_start, self.effect_current) else {
                self.reset_effect();
                return;
            };
            let drag = Rect::from_two_pos(start, cur);
            if drag.width() >= 2.0 && drag.height() >= 2.0 {
                if !self.ocr_enabled {
                    self.status = "Enable text detection in Settings to frame text".to_string();
                } else if let Some(border) =
                    text_border_rect(drag, &self.ocr_regions, image_size, TEXT_BORDER_PADDING)
                {
                    self.annotations.push_rect(border);
                    self.status = "Text border added".to_string();
                } else {
                    self.status = "No text detected in that area".to_string();
                }
            }
            self.reset_effect();
        }
    }

    /// Live preview of the drag rectangle while the text-border tool is active.
    pub(crate) fn draw_text_border_preview(
        &self,
        painter: &egui::Painter,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let (Some(start), Some(cur)) = (self.effect_start, self.effect_current) else {
            return;
        };
        let to_screen = |p: Pos2| -> Pos2 {
            pos2(
                draw_rect.min.x + p.x / image_size.x * draw_rect.width(),
                draw_rect.min.y + p.y / image_size.y * draw_rect.height(),
            )
        };
        let rect = Rect::from_two_pos(to_screen(start), to_screen(cur));
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.5_f32, self.annotations.color),
            StrokeKind::Middle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pocshot_ocr::RectF32;

    fn region(x0: f32, y0: f32, x1: f32, y1: f32) -> TextRegion {
        TextRegion::new(RectF32::new(x0, y0, x1, y1), 0.9)
    }

    #[test]
    fn unions_regions_inside_the_drag() {
        let regions = [
            region(10.0, 20.0, 50.0, 40.0),
            region(60.0, 20.0, 90.0, 40.0),
        ];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let border = text_border_rect(drag, &regions, Vec2::new(200.0, 200.0), 6.0).unwrap();
        assert_eq!(border.min, pos2(4.0, 14.0));
        assert_eq!(border.max, pos2(96.0, 46.0));
    }

    #[test]
    fn only_regions_intersecting_the_drag_are_included() {
        let regions = [
            region(10.0, 10.0, 40.0, 30.0),
            region(150.0, 150.0, 190.0, 180.0),
        ];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 50.0));
        let border = text_border_rect(drag, &regions, Vec2::new(200.0, 200.0), 6.0).unwrap();
        assert_eq!(border.min, pos2(4.0, 4.0));
        assert_eq!(border.max, pos2(46.0, 36.0));
    }

    #[test]
    fn no_regions_returns_none() {
        let regions = [region(150.0, 150.0, 190.0, 180.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 50.0));
        assert!(text_border_rect(drag, &regions, Vec2::new(200.0, 200.0), 6.0).is_none());
    }

    #[test]
    fn padding_is_clamped_to_the_image() {
        let regions = [region(0.0, 0.0, 5.0, 5.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let border = text_border_rect(drag, &regions, Vec2::new(200.0, 200.0), 6.0).unwrap();
        assert_eq!(border.min, pos2(0.0, 0.0));
        assert_eq!(border.max, pos2(11.0, 11.0));
    }
}
