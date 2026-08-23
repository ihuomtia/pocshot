//! On-canvas overlays: committed annotations, the counter bubble cursor
//! preview, and the transient / always-visible snap-line feedback.

use eframe::egui::{self, pos2, vec2, Color32, Rect, Stroke, Vec2};

use crate::annotation::AnnotationTool;
use crate::annotation::COUNTER_BUBBLE_RADIUS;
use crate::app::PocshotApp;
use crate::canvas::screen_to_image;
use crate::snap::SNAP_DISTANCE;

impl PocshotApp {
    pub(crate) fn draw_annotations(
        &self,
        painter: &egui::Painter,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let dr = draw_rect;
        let isz = image_size;
        self.annotations.draw(painter, move |pos| {
            pos2(
                dr.min.x + pos.x / isz.x * dr.width(),
                dr.min.y + pos.y / isz.y * dr.height(),
            )
        });
    }

    /// Live transparent preview of the counter bubble that follows the cursor
    /// while the Counter tool is active. Only shown over the canvas, centered
    /// on the pointer, using the selected annotation color.
    pub(crate) fn draw_counter_preview(&self, painter: &egui::Painter, draw_rect: Rect) {
        if self.annotations.active_tool != AnnotationTool::Counter {
            return;
        }
        if self.annotations.is_counter_dragging() {
            return;
        }
        let Some(pointer) = painter.ctx().pointer_latest_pos() else {
            return;
        };
        if !draw_rect.intersects(Rect::from_center_size(pointer, vec2(1.0, 1.0))) {
            return;
        }
        let r = COUNTER_BUBBLE_RADIUS;
        let color = self.annotations.color;
        let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90);
        painter.circle_filled(pointer, r, fill);
        painter.circle_stroke(
            pointer,
            r,
            Stroke::new(
                1.5_f32,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200),
            ),
        );
    }

    /// Briefly highlight the snap lines that are currently within snap
    /// distance of the pointer, but only while a drag is in progress. This is
    /// the transient "near line" feedback, independent of the always-visible
    /// debug overlay (which is gated by `show_snap_lines`).
    pub(crate) fn draw_brief_snap_indicators(
        &self,
        painter: &egui::Painter,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let dragging = self.drag_start.is_some()
            || self.dragging_handle.is_some()
            || self.annotations.is_drawing()
            || self.effect_start.is_some();
        if !dragging {
            return;
        }
        let Some(pointer) = painter.ctx().pointer_latest_pos() else {
            return;
        };
        let img_pointer = screen_to_image(pointer, draw_rect, image_size);
        let (horiz_snaps, vert_snaps) = self.combined_snap_lines();
        let near = self.theme.colors.snap_line_near;
        let faint = self.theme.colors.snap_line_faint;

        for &line_y in &horiz_snaps {
            if (line_y - img_pointer.y).abs() <= SNAP_DISTANCE {
                let y_pix = draw_rect.min.y
                    + line_y.clamp(0.0, image_size.y) / image_size.y * draw_rect.height();
                let on = (line_y - img_pointer.y).abs() <= 2.0;
                painter.line_segment(
                    [pos2(draw_rect.min.x, y_pix), pos2(draw_rect.max.x, y_pix)],
                    Stroke::new(
                        if on { 2.0_f32 } else { 1.0_f32 },
                        if on { near } else { faint },
                    ),
                );
            }
        }
        for &line_x in &vert_snaps {
            if (line_x - img_pointer.x).abs() <= SNAP_DISTANCE {
                let x_pix = draw_rect.min.x
                    + line_x.clamp(0.0, image_size.x) / image_size.x * draw_rect.width();
                let on = (line_x - img_pointer.x).abs() <= 2.0;
                painter.line_segment(
                    [pos2(x_pix, draw_rect.min.y), pos2(x_pix, draw_rect.max.y)],
                    Stroke::new(
                        if on { 2.0_f32 } else { 1.0_f32 },
                        if on { near } else { faint },
                    ),
                );
            }
        }
    }

    pub(crate) fn draw_all_snap_indicators(
        &self,
        painter: &egui::Painter,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let snap_color = self.theme.colors.snap_line;
        let (horiz_snaps, vert_snaps) = self.combined_snap_lines();

        for &line_y in &horiz_snaps {
            let y_pix = draw_rect.min.y
                + line_y.clamp(0.0, image_size.y) / image_size.y * draw_rect.height();
            painter.line_segment(
                [pos2(draw_rect.min.x, y_pix), pos2(draw_rect.max.x, y_pix)],
                Stroke::new(1.0_f32, snap_color),
            );
        }
        for &line_x in &vert_snaps {
            let x_pix = draw_rect.min.x
                + line_x.clamp(0.0, image_size.x) / image_size.x * draw_rect.width();
            painter.line_segment(
                [pos2(x_pix, draw_rect.min.y), pos2(x_pix, draw_rect.max.y)],
                Stroke::new(1.0_f32, snap_color),
            );
        }
    }

    /// Persistent status line at the bottom of the canvas. Shown whenever there
    /// is an active capture so failures like "OCR failed: …" or transient
    /// messages are visible even with no region selected (the toolbar status is
    /// otherwise gated on a selection).
    pub(crate) fn draw_status_line(&self, painter: &egui::Painter, draw_rect: Rect, status: &str) {
        if status.is_empty() {
            return;
        }
        let max_w = (draw_rect.width() - 16.0).max(40.0);
        let layout_job = egui::text::LayoutJob::simple(
            status.to_string(),
            egui::FontId::proportional(self.theme.fonts.status),
            self.theme.colors.text_muted,
            f32::INFINITY,
        );
        let galley = painter.layout_job(layout_job);
        let g_w = galley.size().x.min(max_w);
        let g_h = galley.size().y;
        let margin = 8.0;
        let pill_w = g_w.min(max_w) + 16.0;
        let pill_h = g_h + 10.0;
        let pill_min = pos2(draw_rect.min.x + margin, draw_rect.max.y - margin - pill_h);
        let pill = Rect::from_min_size(pill_min, vec2(pill_w, pill_h));
        painter.rect_filled(pill, pill_h / 2.0, self.theme.colors.toolbar_bg);
        painter.rect_stroke(
            pill,
            pill_h / 2.0,
            Stroke::new(1.0_f32, self.theme.colors.box_border),
            egui::StrokeKind::Outside,
        );
        let text_pos = pos2(pill_min.x + 8.0, pill_min.y + (pill_h - g_h) / 2.0);
        painter.text(
            text_pos,
            egui::Align2::LEFT_TOP,
            status,
            egui::FontId::proportional(self.theme.fonts.status),
            self.theme.colors.text_muted,
        );
    }
}
