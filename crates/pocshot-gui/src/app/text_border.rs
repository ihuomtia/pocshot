//! Text-region tools: drag over detected text and commit a rectangle fitted to
//! it — an outline (Text border) or a solid fill (Redact).
//!
//! The gesture reuses the pixelate/blur drag fields (`effect_start` /
//! `effect_current`) because the tools are mutually exclusive; on release the
//! drag rect is expanded to the union of every OCR region it touches, padded,
//! clamped to the selection (when one exists) and committed as an ordinary
//! rectangle annotation (undoable).

use eframe::egui::{self, pos2, Pos2, Rect, Vec2};
use pocshot_ocr::TextRegion;

use crate::app::PocshotApp;
use crate::canvas::screen_to_image;

/// Empty space left between the text and its border, in image pixels. Only
/// used by the Text border tool — Redact fills the exact text bounding box
/// (plus [`REDACT_VERTICAL_PADDING`]).
pub const TEXT_BORDER_PADDING: f32 = 6.0;

/// Extra vertical space for Redact: the filled rectangle is the exact text
/// bounding box widened by one pixel top and bottom (and nothing sideways), so
/// antialiased glyph edges are covered without a visible margin.
pub const REDACT_VERTICAL_PADDING: f32 = 1.0;

/// Union of the OCR regions that intersect `drag`, grown by `padding`, clamped
/// to the image and — when `selection` is given — restricted to it, so a text
/// box extending past the selected region never produces a border outside it.
/// `None` when no region was touched or the result is empty.
pub fn text_border_rect(
    drag: Rect,
    regions: &[TextRegion],
    image_size: Vec2,
    padding: Vec2,
    selection: Option<Rect>,
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

    let mut border = Rect::from_min_max(
        pos2(
            (min.x - padding.x).clamp(0.0, image_size.x),
            (min.y - padding.y).clamp(0.0, image_size.y),
        ),
        pos2(
            (max.x + padding.x).clamp(0.0, image_size.x),
            (max.y + padding.y).clamp(0.0, image_size.y),
        ),
    );

    if let Some(sel) = selection {
        border = border.intersect(sel);
        if border.width() < 1.0 || border.height() < 1.0 {
            return None;
        }
    }

    Some(border)
}

impl PocshotApp {
    /// `filled` = true for the Redact tool (solid rectangle), false for the
    /// Text border tool (outline). Snapping is deliberately not applied: the
    /// result is fitted to OCR regions, so guide snapping would only fight it.
    pub(crate) fn process_text_region_drag(
        &mut self,
        response: &egui::Response,
        draw_rect: Rect,
        image_size: Vec2,
        filled: bool,
    ) {
        let screen_to_img = |pos: Pos2| screen_to_image(pos, draw_rect, image_size);

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.effect_start = Some(screen_to_img(pos));
                self.effect_current = Some(screen_to_img(pos));
            }
        } else if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.effect_current = Some(screen_to_img(pos));
            }
        } else if response.drag_stopped() {
            let (Some(start), Some(cur)) = (self.effect_start, self.effect_current) else {
                self.reset_effect();
                return;
            };
            let drag = Rect::from_two_pos(start, cur);
            if drag.width() >= 2.0 && drag.height() >= 2.0 {
                let selection = self.current_selection_region();
                if !self.ocr_enabled {
                    self.status = "Enable text detection in Settings to frame text".to_string();
                } else if let Some(border) = text_border_rect(
                    drag,
                    &self.ocr_regions,
                    image_size,
                    if filled {
                        Vec2::new(0.0, REDACT_VERTICAL_PADDING)
                    } else {
                        Vec2::splat(TEXT_BORDER_PADDING)
                    },
                    selection,
                ) {
                    self.annotations.push_rect(border, filled);
                    self.status = if filled {
                        "Text redacted".to_string()
                    } else {
                        "Text border added".to_string()
                    };
                } else if selection.is_some() {
                    self.status = "No text detected inside the selection".to_string();
                } else {
                    self.status = "No text detected in that area".to_string();
                }
            }
            self.reset_effect();
        }
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
        let border = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            None,
        )
        .unwrap();
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
        let border = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            None,
        )
        .unwrap();
        assert_eq!(border.min, pos2(4.0, 4.0));
        assert_eq!(border.max, pos2(46.0, 36.0));
    }

    #[test]
    fn no_regions_returns_none() {
        let regions = [region(150.0, 150.0, 190.0, 180.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 50.0));
        assert!(text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            None
        )
        .is_none());
    }

    #[test]
    fn padding_is_clamped_to_the_image() {
        let regions = [region(0.0, 0.0, 5.0, 5.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let border = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            None,
        )
        .unwrap();
        assert_eq!(border.min, pos2(0.0, 0.0));
        assert_eq!(border.max, pos2(11.0, 11.0));
    }

    #[test]
    fn border_is_clamped_to_the_selection() {
        // Text extends past the selection on every side; the border is cropped
        // to the selection instead of spilling outside it.
        let regions = [region(0.0, 0.0, 100.0, 100.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 200.0));
        let selection = Rect::from_min_max(pos2(20.0, 30.0), pos2(80.0, 90.0));
        let border = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            Some(selection),
        )
        .unwrap();
        assert_eq!(border, selection);
    }

    #[test]
    fn empty_intersection_with_selection_returns_none() {
        let regions = [region(0.0, 0.0, 10.0, 10.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 200.0));
        let selection = Rect::from_min_max(pos2(100.0, 100.0), pos2(150.0, 150.0));
        assert!(text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::splat(6.0),
            Some(selection)
        )
        .is_none());
    }

    #[test]
    fn redact_pads_one_pixel_top_and_bottom_only() {
        let regions = [region(10.0, 20.0, 90.0, 40.0)];
        let drag = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let redact = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::new(0.0, REDACT_VERTICAL_PADDING),
            None,
        )
        .unwrap();
        assert_eq!(
            redact,
            Rect::from_min_max(pos2(10.0, 19.0), pos2(90.0, 41.0))
        );
    }
}
