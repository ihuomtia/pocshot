//! Region-selection UI: drag-to-select and resize-handle interaction plus the
//! selection chrome (shaded exterior, border, size readout, resize handles).

use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, Vec2,
};

use crate::app::PocshotApp;
use crate::canvas::{
    clamp_rect_to_size, image_to_screen_rect, normalized_uv, screen_to_image,
    translate_rect_clamped,
};
use crate::selection::{handle_rects, HandleType, ImageSelection};
use crate::snap::{snap_pos, SNAP_DISTANCE};

impl PocshotApp {
    pub(crate) fn process_resize_handles(
        &mut self,
        ui: &egui::Ui,
        _painter: &egui::Painter,
        (horiz_snaps, vert_snaps): (Vec<f32>, Vec<f32>),
        _dragging_handle: Option<HandleType>,
        draw_rect: Rect,
        image_size: Vec2,
        snap_ok: bool,
    ) {
        let Some(selection) = self.selection else {
            return;
        };
        let image_rect =
            clamp_rect_to_size(selection.rect(), image_size).unwrap_or_else(|| selection.rect());
        let screen_rect = image_to_screen_rect(image_rect, draw_rect, image_size);

        for (ht, hr) in handle_rects(screen_rect) {
            let handle_resp = ui.interact(hr, Id::new(("resize", ht as u8)), Sense::drag());

            if handle_resp.drag_started() {
                self.dragging_handle = Some(ht);
            }

            if self.dragging_handle == Some(ht) && handle_resp.dragged() {
                if let Some(pos) = handle_resp.interact_pointer_pos() {
                    let mut ip = screen_to_image(pos, draw_rect, image_size);
                    ip.x = ip.x.clamp(0.0, image_size.x);
                    ip.y = ip.y.clamp(0.0, image_size.y);

                    let sx = if snap_ok {
                        snap_pos(ip.x, &vert_snaps, SNAP_DISTANCE)
                    } else {
                        ip.x
                    };
                    let sy = if snap_ok {
                        snap_pos(ip.y, &horiz_snaps, SNAP_DISTANCE)
                    } else {
                        ip.y
                    };

                    let mut r = image_rect;
                    match ht {
                        HandleType::TopLeft => {
                            r.min.x = sx.min(r.max.x - 1.0);
                            r.min.y = sy.min(r.max.y - 1.0);
                        }
                        HandleType::TopRight => {
                            r.max.x = sx.max(r.min.x + 1.0);
                            r.min.y = sy.min(r.max.y - 1.0);
                        }
                        HandleType::BottomLeft => {
                            r.min.x = sx.min(r.max.x - 1.0);
                            r.max.y = sy.max(r.min.y + 1.0);
                        }
                        HandleType::BottomRight => {
                            r.max.x = sx.max(r.min.x + 1.0);
                            r.max.y = sy.max(r.min.y + 1.0);
                        }
                        HandleType::Top => {
                            r.min.y = sy.min(r.max.y - 1.0);
                        }
                        HandleType::Bottom => {
                            r.max.y = sy.max(r.min.y + 1.0);
                        }
                        HandleType::Left => {
                            r.min.x = sx.min(r.max.x - 1.0);
                        }
                        HandleType::Right => {
                            r.max.x = sx.max(r.min.x + 1.0);
                        }
                    }
                    self.selection = Some(ImageSelection {
                        start: r.min,
                        end: r.max,
                    });
                }
            }

            if handle_resp.drag_stopped() {
                self.dragging_handle = None;
            }
        }
    }

    pub(crate) fn process_canvas_drag(
        &mut self,
        ui: &egui::Ui,
        _painter: &egui::Painter,
        (horiz_snaps, vert_snaps): (Vec<f32>, Vec<f32>),
        draw_rect: Rect,
        image_size: Vec2,
        snap_ok: bool,
    ) {
        let response = ui.interact(draw_rect, Id::new("capture-canvas"), Sense::drag());

        // Hovering inside an existing selection means "drag to move it"; outside
        // means "drag a new region".
        let hover_img = response
            .hover_pos()
            .map(|p| screen_to_image(p, draw_rect, image_size));
        let inside_selection = hover_img
            .zip(self.selection)
            .map(|(p, s)| s.rect().contains(p))
            .unwrap_or(false);
        if self.dragging_handle.is_none() && self.moving_selection.is_none() {
            response.clone().on_hover_cursor(if inside_selection {
                CursorIcon::Move
            } else {
                CursorIcon::Crosshair
            });
        }

        if self.dragging_handle.is_none() {
            if response.drag_started() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let image_pos = screen_to_image(pointer, draw_rect, image_size);
                    let existing = self.selection.map(|s| {
                        clamp_rect_to_size(s.rect(), image_size).unwrap_or_else(|| s.rect())
                    });
                    match existing {
                        // Press inside the current region: start moving it.
                        Some(sel_rect) if sel_rect.contains(image_pos) => {
                            self.moving_selection = Some((image_pos, sel_rect));
                            self.drag_start = None;
                        }
                        // Press elsewhere: start a new region.
                        _ => {
                            self.moving_selection = None;
                            self.drag_start = Some(image_pos);
                        }
                    }
                }
            }

            if response.dragged() {
                if let (Some((anchor, original)), Some(pointer)) =
                    (self.moving_selection, response.interact_pointer_pos())
                {
                    let cur = screen_to_image(pointer, draw_rect, image_size);
                    let moved = translate_rect_clamped(original, cur - anchor, image_size);
                    self.selection = Some(ImageSelection {
                        start: moved.min,
                        end: moved.max,
                    });
                } else if let (Some(start), Some(pointer)) =
                    (self.drag_start, response.interact_pointer_pos())
                {
                    let end = screen_to_image(pointer, draw_rect, image_size);
                    let snapped = if snap_ok {
                        pos2(
                            snap_pos(end.x, &vert_snaps, SNAP_DISTANCE),
                            snap_pos(end.y, &horiz_snaps, SNAP_DISTANCE),
                        )
                    } else {
                        end
                    };
                    self.selection = Some(ImageSelection {
                        start,
                        end: snapped,
                    });
                }
            }

            if response.drag_stopped() {
                self.drag_start = None;
                self.moving_selection = None;
            }
        }
    }

    pub(crate) fn draw_selection_base(
        &self,
        _ui: &egui::Ui,
        painter: &egui::Painter,
        texture: &TextureHandle,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let Some(selection) = self.selection else {
            if self.show_snap_lines {
                self.draw_snap_overlay(painter, draw_rect);
            }
            return;
        };
        let image_rect =
            clamp_rect_to_size(selection.rect(), image_size).unwrap_or_else(|| selection.rect());
        let screen_rect = image_to_screen_rect(image_rect, draw_rect, image_size);

        painter.image(
            texture.id(),
            screen_rect,
            normalized_uv(image_rect, image_size),
            Color32::WHITE,
        );

        if self.show_snap_lines {
            self.draw_snap_overlay(painter, draw_rect);
        }
    }

    pub(crate) fn draw_selection_top(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        _texture: &TextureHandle,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let Some(selection) = self.selection else {
            return;
        };
        let image_rect =
            clamp_rect_to_size(selection.rect(), image_size).unwrap_or_else(|| selection.rect());
        let screen_rect = image_to_screen_rect(image_rect, draw_rect, image_size);

        let r = screen_rect;
        let outer = draw_rect;
        let dim = self.theme.colors.overlay_dim;
        painter.rect_filled(
            Rect::from_min_max(outer.min, pos2(outer.max.x, r.min.y)),
            0.0,
            dim,
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(outer.min.x, r.max.y), outer.max),
            0.0,
            dim,
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(outer.min.x, r.min.y), pos2(r.min.x, r.max.y)),
            0.0,
            dim,
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(r.max.x, r.min.y), pos2(outer.max.x, r.max.y)),
            0.0,
            dim,
        );

        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(2.0_f32, self.theme.colors.selection_border),
            StrokeKind::Outside,
        );
        painter.text(
            r.min + vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{} x {}",
                image_rect.width().round() as u32,
                image_rect.height().round() as u32
            ),
            egui::FontId::proportional(self.theme.fonts.size_readout),
            self.theme.colors.text_primary,
        );

        self.draw_resize_handles(ui, painter, r);
    }

    pub(crate) fn draw_resize_handles(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        screen_rect: Rect,
    ) {
        for (ht, hr) in handle_rects(screen_rect) {
            let is_dragging = self.dragging_handle == Some(ht);
            let is_hovered = hr.contains(ui.ctx().pointer_latest_pos().unwrap_or(Pos2::ZERO));
            let fill = if is_dragging || is_hovered {
                self.theme.colors.handle_active
            } else {
                self.theme.colors.handle_idle
            };
            painter.rect_filled(hr, 2.0, fill);
            painter.rect_stroke(
                hr,
                1.0_f32,
                Stroke::new(1.0_f32, self.theme.colors.text_primary),
                StrokeKind::Inside,
            );

            if is_hovered || is_dragging {
                let cursor = match ht {
                    HandleType::TopLeft | HandleType::BottomRight => CursorIcon::ResizeNwSe,
                    HandleType::TopRight | HandleType::BottomLeft => CursorIcon::ResizeNeSw,
                    HandleType::Top | HandleType::Bottom => CursorIcon::ResizeVertical,
                    HandleType::Left | HandleType::Right => CursorIcon::ResizeHorizontal,
                };
                ui.ctx().set_cursor_icon(cursor);
            }
        }
    }
}
