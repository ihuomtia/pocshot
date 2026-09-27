//! Destructive image effects (pixelate / blur): the drag gesture, the
//! off-thread processing, the live preview and the result polling.

use eframe::egui::{self, pos2, Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::sync::mpsc;

use crate::annotation::AnnotationTool;
use crate::app::PocshotApp;
use crate::canvas::screen_to_image;
use crate::effects;
use crate::snap::{snap_pos, SNAP_DISTANCE};

impl PocshotApp {
    /// Handle the drag gesture for the Pixelate / Blur tools: record the
    /// region while dragging and apply the effect on release, with an undo
    /// snapshot of the previous capture.
    pub(crate) fn process_effect_drag(
        &mut self,
        response: &egui::Response,
        ctx: &egui::Context,
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
            let rect = Rect::from_two_pos(start, cur);
            // Discard sub-pixel / empty regions.
            if rect.width() >= 2.0 && rect.height() >= 2.0 {
                let Some(image) = self.capture.clone() else {
                    self.reset_effect();
                    return;
                };
                let tool = self.annotations.active_tool;
                self.capture_undo.push(image.clone());
                self.capture_redo.clear();
                self.status = match tool {
                    AnnotationTool::Pixelate => "Pixelating…".to_string(),
                    AnnotationTool::Blur => "Blurring…".to_string(),
                    _ => self.status.clone(),
                };
                // Run the (potentially expensive) effect on a worker thread so
                // the UI stays responsive even for a large region.
                let (tx, rx) = mpsc::channel();
                self.effect_rx = Some(rx);
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let mut img = image;
                    match tool {
                        AnnotationTool::Pixelate => effects::pixelate_region(&mut img, rect, 16),
                        AnnotationTool::Blur => effects::blur_region(&mut img, rect, 12.0),
                        _ => {}
                    }
                    let _ = tx.send(img);
                    ctx.request_repaint();
                });
            }
            self.reset_effect();
        }
    }

    pub(crate) fn reset_effect(&mut self) {
        self.effect_start = None;
        self.effect_current = None;
    }

    /// Poll the in-flight pixelate/blur worker. When its result is ready,
    /// swap it in and refresh the texture; otherwise keep the UI repainting
    /// so the tool stays responsive while the heavy work runs off-thread.
    pub(crate) fn poll_effect(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.effect_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(image) => {
                self.effect_rx = None;
                self.capture = Some(image.clone());
                self.apply_capture_texture(ctx);
                self.status = "Effect applied".to_string();
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(super::WORKER_WATCHDOG_INTERVAL)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.effect_rx = None;
                self.status = "Effect failed".to_string();
            }
        }
    }

    /// Draw a translucent preview of the region being pixelated/blurred.
    pub(crate) fn draw_effect_preview(
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
        let a = to_screen(start);
        let b = to_screen(cur);
        let rect = Rect::from_two_pos(a, b);
        painter.rect_filled(rect, 0.0, self.theme.colors.effect_fill);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.5_f32, self.theme.colors.effect_border),
            StrokeKind::Middle,
        );
    }
}
