//! Group 3 of the toolbar: the quick color swatches.

use eframe::egui::{self, vec2, Color32, Id, Pos2, Rect, Sense, Stroke};

use crate::theme::Theme;
use crate::toolbar::Orientation;

pub(crate) const COLOR_SWATCHES: &[Color32] = &[
    Color32::from_rgb(239, 68, 68),
    Color32::from_rgb(250, 204, 21),
    Color32::from_rgb(34, 197, 94),
    Color32::from_rgb(59, 130, 246),
    Color32::from_rgb(255, 255, 255),
    Color32::from_rgb(15, 23, 42),
];

pub fn draw_group_3_options(
    color: &mut Color32,
    ui: &mut egui::Ui,
    box_rect: Rect,
    orient: Orientation,
    theme: &Theme,
) {
    ui.painter()
        .rect_filled(box_rect, theme.geometry.box_radius, theme.colors.toolbar_bg);

    let g = &theme.geometry;
    let pad = g.button_pad;
    let swatch_size = g.swatch_size;
    let swatch_gap = g.swatch_gap;

    let swatches: &[Color32] = if theme.colors.color_swatches.is_empty() {
        COLOR_SWATCHES
    } else {
        &theme.colors.color_swatches
    };

    let start_x = box_rect.min.x + pad;
    let start_y = box_rect.min.y + pad;

    match orient {
        Orientation::Horizontal => {
            let swatch_y = start_y + (g.button_h - swatch_size) / 2.0;
            let x = start_x;
            for (i, &swatch_color) in swatches.iter().enumerate() {
                let sx = x + i as f32 * (swatch_size + swatch_gap);
                let swatch_rect =
                    Rect::from_min_size(Pos2::new(sx, swatch_y), vec2(swatch_size, swatch_size));
                let response = ui.interact(swatch_rect, Id::new(("color-h", i)), Sense::click());

                let is_active = *color == swatch_color;
                ui.painter().rect_filled(swatch_rect, 3.0, swatch_color);
                if is_active {
                    ui.painter().rect_stroke(
                        swatch_rect,
                        3.0,
                        Stroke::new(2.0_f32, theme.colors.text_primary),
                        egui::StrokeKind::Outside,
                    );
                } else if response.hovered() {
                    ui.painter().rect_stroke(
                        swatch_rect,
                        3.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 120)),
                        egui::StrokeKind::Outside,
                    );
                }

                if response.clicked() {
                    *color = swatch_color;
                }
            }
        }
        Orientation::Vertical => {
            let swatch_x = start_x + (g.button_h - swatch_size) / 2.0;
            let y = start_y;
            for (i, &swatch_color) in swatches.iter().enumerate() {
                let sy = y + i as f32 * (swatch_size + swatch_gap);
                let swatch_rect =
                    Rect::from_min_size(Pos2::new(swatch_x, sy), vec2(swatch_size, swatch_size));
                let response = ui.interact(swatch_rect, Id::new(("color-v", i)), Sense::click());

                let is_active = *color == swatch_color;
                ui.painter().rect_filled(swatch_rect, 3.0, swatch_color);
                if is_active {
                    ui.painter().rect_stroke(
                        swatch_rect,
                        3.0,
                        Stroke::new(2.0_f32, theme.colors.text_primary),
                        egui::StrokeKind::Outside,
                    );
                } else if response.hovered() {
                    ui.painter().rect_stroke(
                        swatch_rect,
                        3.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 120)),
                        egui::StrokeKind::Outside,
                    );
                }

                if response.clicked() {
                    *color = swatch_color;
                }
            }
        }
    }
}
