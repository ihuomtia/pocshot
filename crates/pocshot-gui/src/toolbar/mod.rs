//! The in-canvas toolbar: positions three button groups (actions, tools,
//! color options) around the selection and routes clicks/state through
//! [`ToolbarState`].

mod actions;
mod options;
mod settings;
mod tools;

use eframe::egui::{self, vec2, Color32, Id, Pos2, Rect, Stroke, Vec2};

use crate::annotation::AnnotationTool;
use crate::canvas::image_to_screen_rect;
use crate::selection::ImageSelection;
use crate::theme::Theme;

use actions::{draw_group_1_actions, ACTION_BUTTONS};
use options::{draw_group_3_options, COLOR_SWATCHES};
use tools::{draw_group_2_tools, TOOL_BUTTONS};

pub use settings::show_settings_panel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Refresh,
    Save,
    Copy,
    Undo,
    Redo,
    Pin,
    ClearAnnotations,
    ClearSelection,
    ToggleSettings,
    OcrRegion,
}

pub struct ToolbarState<'a> {
    pub status: &'a str,
    pub active_tool: &'a mut AnnotationTool,
    pub color: &'a mut Color32,
    pub can_undo: bool,
    pub can_redo: bool,
    pub theme: &'a Theme,
}

pub fn show_toolbar(
    state: ToolbarState,
    ui: &mut egui::Ui,
    draw_rect: Rect,
    image_size: Vec2,
    selection: Option<ImageSelection>,
    is_selecting: bool,
    on_action: &mut impl FnMut(Action),
) -> Rect {
    let Some(sel) = selection else {
        return Rect::NOTHING;
    };
    if is_selecting {
        return Rect::NOTHING;
    }

    let sel_screen_rect = image_to_screen_rect(sel.rect(), draw_rect, image_size);

    let theme = state.theme;
    let margin = theme.geometry.toolbar_margin;
    let space_bottom = draw_rect.bottom() - sel_screen_rect.bottom();
    let space_right = draw_rect.right() - sel_screen_rect.right();
    let space_top = sel_screen_rect.top() - draw_rect.top();
    let space_left = sel_screen_rect.left() - draw_rect.left();

    // Group boxes are sized dynamically from the number of buttons so adding a
    // toolbar button can never overflow the box again.
    let g = &theme.geometry;
    let group_h_w = |n: usize| {
        g.button_pad * 2.0 + n as f32 * g.button_w + (n as f32 - 1.0).max(0.0) * g.button_gap
    };
    let group_v_h = |n: usize| {
        g.button_pad * 2.0 + n as f32 * g.button_h + (n as f32 - 1.0).max(0.0) * g.button_gap
    };

    let g1_w_h = group_h_w(ACTION_BUTTONS.len());
    let g1_h_v = group_v_h(ACTION_BUTTONS.len());
    let g2_w_h = group_h_w(TOOL_BUTTONS.len());
    let g2_h_v = group_v_h(TOOL_BUTTONS.len());

    let swatch_count = if theme.colors.color_swatches.is_empty() {
        COLOR_SWATCHES.len()
    } else {
        theme.colors.color_swatches.len()
    };
    let g3_w_h = g.button_pad * 2.0
        + swatch_count as f32 * g.swatch_size
        + (swatch_count as f32 - 1.0).max(0.0) * g.swatch_gap;
    let g3_h_v = g3_w_h;

    let single_row_w = g1_w_h + margin + g2_w_h + margin + g3_w_h;

    let (g1_info, g2_info, g3_info) =
        if space_bottom >= 46.0 && draw_rect.width() >= (single_row_w + 2.0 * margin) {
            // DEFAULT: All buttons in 1 single horizontal row at BOTTOM
            let block_w = single_row_w;
            let cx = sel_screen_rect.center().x;
            let start_x = (cx - block_w / 2.0).clamp(
                draw_rect.left() + margin,
                (draw_rect.right() - block_w - margin).max(draw_rect.left() + margin),
            );
            let y = sel_screen_rect.bottom() + margin;

            let g1 = (
                Rect::from_min_size(Pos2::new(start_x, y), vec2(g1_w_h, 40.0)),
                Orientation::Horizontal,
            );
            let g2 = (
                Rect::from_min_size(Pos2::new(start_x + g1_w_h + margin, y), vec2(g2_w_h, 40.0)),
                Orientation::Horizontal,
            );
            let g3 = (
                Rect::from_min_size(
                    Pos2::new(start_x + g1_w_h + margin + g2_w_h + margin, y),
                    vec2(g3_w_h, 40.0),
                ),
                Orientation::Horizontal,
            );
            (g1, g2, g3)
        } else if space_bottom >= 88.0 {
            // Fallback if screen width is narrow: 2 rows at BOTTOM
            let r1_w = g1_w_h;
            let r2_w = g2_w_h + margin + g3_w_h;
            let block_w = r1_w.max(r2_w);
            let cx = sel_screen_rect.center().x;
            let start_x = (cx - block_w / 2.0).clamp(
                draw_rect.left() + margin,
                (draw_rect.right() - block_w - margin).max(draw_rect.left() + margin),
            );
            let r1_y = sel_screen_rect.bottom() + margin;
            let r2_y = r1_y + 40.0 + margin;

            let g1 = (
                Rect::from_min_size(
                    Pos2::new(start_x + (block_w - r1_w) / 2.0, r1_y),
                    vec2(g1_w_h, 40.0),
                ),
                Orientation::Horizontal,
            );
            let g2 = (
                Rect::from_min_size(Pos2::new(start_x, r2_y), vec2(g2_w_h, 40.0)),
                Orientation::Horizontal,
            );
            let g3 = (
                Rect::from_min_size(
                    Pos2::new(start_x + g2_w_h + margin, r2_y),
                    vec2(g3_w_h, 40.0),
                ),
                Orientation::Horizontal,
            );
            (g1, g2, g3)
        } else if space_bottom >= 46.0 && space_right >= 46.0 {
            // Bottom (Horizontal) + Right (Vertical)
            let r1_w = g1_w_h.max(g2_w_h);
            let start_x = (sel_screen_rect.center().x - r1_w / 2.0).clamp(
                draw_rect.left() + margin,
                (draw_rect.right() - r1_w - margin).max(draw_rect.left() + margin),
            );
            let b_y = sel_screen_rect.bottom() + margin;

            let g1 = (
                Rect::from_min_size(Pos2::new(start_x, b_y), vec2(g1_w_h, 40.0)),
                Orientation::Horizontal,
            );
            let g2 = (
                Rect::from_min_size(Pos2::new(start_x, b_y + 40.0 + margin), vec2(g2_w_h, 40.0)),
                Orientation::Horizontal,
            );
            let rx = (sel_screen_rect.right() + margin).clamp(
                draw_rect.left() + margin,
                (draw_rect.right() - 40.0 - margin).max(draw_rect.left() + margin),
            );
            let ry = (sel_screen_rect.center().y - g3_h_v / 2.0).clamp(
                draw_rect.top() + margin,
                (draw_rect.bottom() - g3_h_v - margin).max(draw_rect.top() + margin),
            );
            let g3 = (
                Rect::from_min_size(Pos2::new(rx, ry), vec2(40.0, g3_h_v)),
                Orientation::Vertical,
            );
            (g1, g2, g3)
        } else {
            // Dynamic assignment based on adjacent border
            let mut sides = Vec::new();
            if space_bottom >= 46.0 {
                sides.push("bottom");
            }
            if space_right >= 46.0 {
                sides.push("right");
            }
            if space_top >= 46.0 {
                sides.push("top");
            }
            if space_left >= 46.0 {
                sides.push("left");
            }
            sides.push("inside_bottom");
            sides.push("inside_top");
            sides.push("inside_center");

            let get_group_info = |side: &str,
                                  w_h: f32,
                                  h_h: f32,
                                  w_v: f32,
                                  h_v: f32,
                                  idx: usize|
             -> (Rect, Orientation) {
                match side {
                    "bottom" | "top" | "inside_bottom" | "inside_top" => {
                        let orient = Orientation::Horizontal;
                        let w = w_h;
                        let h = h_h;
                        let y_base = if side == "bottom" {
                            sel_screen_rect.bottom() + margin + idx as f32 * (h + margin)
                        } else if side == "top" {
                            sel_screen_rect.top() - h - margin - idx as f32 * (h + margin)
                        } else if side == "inside_bottom" {
                            sel_screen_rect.bottom() - h - margin - idx as f32 * (h + margin)
                        } else {
                            sel_screen_rect.top() + margin + idx as f32 * (h + margin)
                        };
                        let cx = sel_screen_rect.center().x;
                        let x = (cx - w / 2.0).clamp(
                            draw_rect.left() + margin,
                            (draw_rect.right() - w - margin).max(draw_rect.left() + margin),
                        );
                        let y = y_base.clamp(
                            draw_rect.top() + margin,
                            (draw_rect.bottom() - h - margin).max(draw_rect.top() + margin),
                        );
                        (Rect::from_min_size(Pos2::new(x, y), vec2(w, h)), orient)
                    }
                    "right" | "left" => {
                        let orient = Orientation::Vertical;
                        let w = w_v;
                        let h = h_v;
                        let x_base = if side == "right" {
                            sel_screen_rect.right() + margin
                        } else {
                            sel_screen_rect.left() - w - margin
                        };
                        let cy = sel_screen_rect.center().y + (idx as f32) * (h + margin);
                        let x = x_base.clamp(
                            draw_rect.left() + margin,
                            (draw_rect.right() - w - margin).max(draw_rect.left() + margin),
                        );
                        let y = cy.clamp(
                            draw_rect.top() + margin,
                            (draw_rect.bottom() - h - margin).max(draw_rect.top() + margin),
                        );
                        (Rect::from_min_size(Pos2::new(x, y), vec2(w, h)), orient)
                    }
                    _ => {
                        let orient = Orientation::Horizontal;
                        let w = w_h;
                        let h = h_h;
                        let cx = sel_screen_rect.center().x;
                        let cy = sel_screen_rect.center().y;
                        let x = (cx - w / 2.0).clamp(
                            draw_rect.left() + margin,
                            (draw_rect.right() - w - margin).max(draw_rect.left() + margin),
                        );
                        let y = (cy - h / 2.0 + idx as f32 * (h + margin)).clamp(
                            draw_rect.top() + margin,
                            (draw_rect.bottom() - h - margin).max(draw_rect.top() + margin),
                        );
                        (Rect::from_min_size(Pos2::new(x, y), vec2(w, h)), orient)
                    }
                }
            };

            let side1 = sides.first().copied().unwrap_or("inside_bottom");
            let side2 = sides.get(1).copied().unwrap_or(side1);
            let side3 = sides.get(2).copied().unwrap_or(side2);

            let g1 = get_group_info(side1, g1_w_h, 40.0, 40.0, g1_h_v, 0);
            let g2 = get_group_info(
                side2,
                g2_w_h,
                40.0,
                40.0,
                g2_h_v,
                if side2 == side1 { 1 } else { 0 },
            );
            let g3 = get_group_info(
                side3,
                g3_w_h,
                40.0,
                40.0,
                g3_h_v,
                if side3 == side2 { 1 } else { 0 },
            );
            (g1, g2, g3)
        };

    draw_group_1_actions(
        state.can_undo,
        state.can_redo,
        state.status,
        ui,
        g1_info.0,
        g1_info.1,
        theme,
        on_action,
    );
    draw_group_2_tools(state.active_tool, ui, g2_info.0, g2_info.1, theme);
    draw_group_3_options(state.color, ui, g3_info.0, g3_info.1, theme);

    g1_info.0.union(g2_info.0).union(g3_info.0)
}

pub(crate) fn show_tooltip(ui: &mut egui::Ui, text: &str, pos: Pos2, theme: &Theme) {
    egui::Area::new(Id::new(("tooltip", text)))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(theme.colors.tooltip_bg)
                .inner_margin(4.0)
                .stroke(Stroke::new(1.0_f32, theme.colors.tooltip_border))
                .show(ui, |ui| {
                    ui.colored_label(theme.colors.text_primary, text);
                });
        });
}
