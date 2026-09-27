//! The settings side panel: export options, OCR config, annotation defaults
//! and debug toggles.

use eframe::egui::{self, vec2, Color32, Id, Rect, Sense, Stroke};

use crate::theme::Theme;
use crate::toolbar::options::COLOR_SWATCHES;
use egui_phosphor::regular as icons;

pub fn show_settings_panel(
    theme: &Theme,
    clipboard: &mut bool,
    color: &mut Color32,
    stroke_width: &mut f32,
    show_settings: &mut bool,
    show_snap_lines: &mut bool,
    snap_enabled: &mut bool,
    ocr_enabled: &mut bool,
    show_text_boxes: &mut bool,
    show_ocr_debug: &mut bool,
    ocr_region_only: &mut bool,
    ocr_confidence: &mut f32,
    ocr_models_dir: &mut String,
    reload_theme: &mut bool,
    download_models: &mut bool,
    text_size: &mut f32,
    settings_changed: &mut bool,
    ui: &mut egui::Ui,
    box_rect: Rect,
) {
    ui.painter().rect_filled(
        box_rect,
        theme.geometry.settings_radius,
        theme.colors.settings_bg,
    );
    ui.painter().rect_stroke(
        box_rect,
        theme.geometry.settings_radius,
        Stroke::new(1.0_f32, theme.colors.box_border),
        egui::StrokeKind::Outside,
    );

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(box_rect.shrink(10.0)));
    let ui = &mut child_ui;

    let c = &theme.colors;
    let f = &theme.fonts;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} Settings", icons::GEAR))
                .strong()
                .size(f.settings_heading)
                .color(c.text_primary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(icons::X).frame(false)).clicked() {
                *show_settings = false;
            }
        });
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(format!("{} Reload theme", icons::ARROW_CLOCKWISE))
                    .fill(c.button_bg_idle),
            )
            .clicked()
        {
            *reload_theme = true;
        }
        ui.label(
            egui::RichText::new("re-reads config.json theme")
                .size(f.settings_label)
                .color(c.text_faint),
        );
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Export Options")
            .strong()
            .size(f.settings_section)
            .color(c.accent),
    );
    ui.add_space(2.0);
    ui.checkbox(clipboard, "Copy to clipboard on Save");

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("OCR / Text Detection")
            .strong()
            .size(f.settings_section)
            .color(c.accent),
    );
    ui.add_space(2.0);
    *settings_changed |= ui.checkbox(ocr_enabled, "Enable text detection").changed();
    *settings_changed |= ui.checkbox(show_text_boxes, "Show text bounding boxes").changed();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Min confidence:")
                .size(f.settings_label)
                .color(c.text_muted),
        );
        *settings_changed |= ui
            .add(
                egui::DragValue::new(ocr_confidence)
                    .range(0.1..=0.95)
                    .speed(0.01),
            )
            .changed();
    });
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new("Models dir:")
            .size(f.settings_label)
            .color(c.text_muted),
    );
    *settings_changed |= ui
        .add(
            egui::TextEdit::singleline(ocr_models_dir)
                .hint_text("default: ~/.config/pocshot/models")
                .desired_width(panel_content_width(ui, box_rect)),
        )
        .changed();
    if ui
        .add(
            egui::Button::new(format!("{} Download models", icons::TRAY_ARROW_DOWN))
                .fill(c.button_bg_idle),
        )
        .clicked()
    {
        *download_models = true;
    }
    ui.label(
        egui::RichText::new("Models auto-download on first use (ocrs .rten)")
            .italics()
            .size(f.settings_helper)
            .color(c.text_faint),
    );

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Annotation Defaults")
            .strong()
            .size(f.settings_section)
            .color(c.accent),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Width:")
                .size(f.settings_label)
                .color(c.text_muted),
        );
        let slider_rect = Rect::from_min_size(ui.cursor().min, vec2(120.0, 18.0));
        draw_stroke_slider(stroke_width, ui, slider_rect, theme);
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Text size:")
                .size(f.settings_label)
                .color(c.text_muted),
        );
        *settings_changed |= ui
            .add(egui::DragValue::new(text_size).range(8.0..=72.0).speed(0.5))
            .changed();
    });

    ui.add_space(8.0);

    ui.label(
        egui::RichText::new("Color:")
            .size(f.settings_label)
            .color(c.text_muted),
    );
    ui.add_space(4.0);
    egui::color_picker::color_picker_color32(ui, color, egui::color_picker::Alpha::Opaque);

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Quick presets:")
            .size(f.settings_label)
            .color(c.text_muted),
    );
    ui.horizontal(|ui| {
        let swatch_size = theme.geometry.swatch_size;
        let swatch_gap = theme.geometry.settings_swatch_gap;
        let swatches: &[Color32] = if theme.colors.color_swatches.is_empty() {
            COLOR_SWATCHES
        } else {
            &theme.colors.color_swatches
        };
        for &swatch_color in swatches {
            let (rect, response) =
                ui.allocate_exact_size(vec2(swatch_size, swatch_size), Sense::click());
            let is_active = *color == swatch_color;
            ui.painter().rect_filled(rect, 3.0, swatch_color);
            if is_active {
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(2.0_f32, theme.colors.text_primary),
                    egui::StrokeKind::Outside,
                );
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 120)),
                    egui::StrokeKind::Outside,
                );
            }
            if response.clicked() {
                *color = swatch_color;
            }
            ui.add_space(swatch_gap);
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Debug Settings")
            .strong()
            .size(f.settings_section)
            .color(c.accent),
    );
    ui.add_space(2.0);
    *settings_changed |= ui.checkbox(show_snap_lines, "Show all snap lines").changed();
    *settings_changed |= ui.checkbox(snap_enabled, "Enable snapping").changed();
    *settings_changed |= ui.checkbox(show_ocr_debug, "Show OCR debug zones").changed();
    *settings_changed |= ui.checkbox(ocr_region_only, "Detect text only in selection").changed();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Hold Shift to temporarily disable snapping")
                .italics()
                .size(f.settings_helper)
                .color(c.text_faint),
        );
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Press Esc to cancel or close")
            .italics()
            .size(f.settings_helper)
            .color(c.text_faint),
    );
}

fn panel_content_width(ui: &egui::Ui, box_rect: Rect) -> f32 {
    (box_rect.width() - 20.0).clamp(60.0, ui.available_width())
}

fn draw_stroke_slider(stroke_width: &mut f32, ui: &mut egui::Ui, slider_rect: Rect, theme: &Theme) {
    let slider_id = Id::new("stroke-width-slider");
    let response = ui.interact(slider_rect, slider_id, Sense::drag());
    let slider_bg = Color32::from_rgb(60, 60, 60);
    ui.painter().rect_filled(slider_rect, 2.0, slider_bg);

    let min_w = 1.0f32;
    let max_w = 12.0f32;
    let frac = (*stroke_width - min_w) / (max_w - min_w);
    let fill_w = slider_rect.width() * frac.clamp(0.0, 1.0);
    let fill_rect = Rect::from_min_size(slider_rect.min, vec2(fill_w, slider_rect.height()));
    ui.painter()
        .rect_filled(fill_rect, 2.0, theme.colors.accent);

    if let Some(pos) = response.interact_pointer_pos() {
        if slider_rect.contains(pos) {
            let f = ((pos.x - slider_rect.min.x) / slider_rect.width()).clamp(0.0, 1.0);
            *stroke_width = min_w + f * (max_w - min_w);
        }
    }

    ui.painter().text(
        slider_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}", stroke_width),
        egui::FontId::proportional(theme.fonts.slider_value),
        theme.colors.text_primary,
    );
}
