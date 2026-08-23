//! Group 1 of the toolbar: the document actions (refresh, save, copy, undo,
//! redo, pin, clear, settings).

use eframe::egui::{self, vec2, Align2, Id, Pos2, Rect, Sense};
use egui_phosphor::regular as icons;

use crate::theme::Theme;
use crate::toolbar::{show_tooltip, Action, Orientation};

pub(crate) struct ActionButton {
    pub(crate) icon: &'static str,
    pub(crate) action: Action,
    pub(crate) name: &'static str,
    pub(crate) shortcut: Option<&'static str>,
}

pub(crate) const ACTION_BUTTONS: &[ActionButton] = &[
    ActionButton {
        icon: icons::ARROW_CLOCKWISE,
        action: Action::Refresh,
        name: "Refresh",
        shortcut: Some("Cmd+R"),
    },
    ActionButton {
        icon: icons::TRAY_ARROW_DOWN,
        action: Action::Save,
        name: "Save",
        shortcut: Some("Cmd+S"),
    },
    ActionButton {
        icon: icons::COPY,
        action: Action::Copy,
        name: "Copy",
        shortcut: Some("Cmd+C"),
    },
    ActionButton {
        icon: icons::ARROW_COUNTER_CLOCKWISE,
        action: Action::Undo,
        name: "Undo",
        shortcut: Some("Cmd+Z"),
    },
    ActionButton {
        icon: icons::ARROW_CLOCKWISE,
        action: Action::Redo,
        name: "Redo",
        shortcut: Some("Cmd+Shift+Z"),
    },
    ActionButton {
        icon: icons::PUSH_PIN,
        action: Action::Pin,
        name: "Pin selection",
        shortcut: None,
    },
    ActionButton {
        icon: icons::TEXT_T,
        action: Action::OcrRegion,
        name: "OCR this region",
        shortcut: None,
    },
    ActionButton {
        icon: icons::ERASER,
        action: Action::ClearAnnotations,
        name: "Clear annotations",
        shortcut: None,
    },
    ActionButton {
        icon: icons::TRASH,
        action: Action::ClearSelection,
        name: "Clear selection",
        shortcut: None,
    },
    ActionButton {
        icon: icons::GEAR,
        action: Action::ToggleSettings,
        name: "Settings",
        shortcut: None,
    },
];

pub fn draw_group_1_actions(
    can_undo: bool,
    can_redo: bool,
    status: &str,
    ui: &mut egui::Ui,
    box_rect: Rect,
    orient: Orientation,
    theme: &Theme,
    on_action: &mut impl FnMut(Action),
) {
    ui.painter()
        .rect_filled(box_rect, theme.geometry.box_radius, theme.colors.toolbar_bg);

    let pad = theme.geometry.button_pad;
    let icon_w = theme.geometry.button_w;
    let btn_h = theme.geometry.button_h;
    let gap = theme.geometry.button_gap;
    let start_x = box_rect.min.x + pad;
    let start_y = box_rect.min.y + pad;

    match orient {
        Orientation::Horizontal => {
            draw_action_row_horizontal(
                can_undo, can_redo, ui, start_x, start_y, icon_w, btn_h, gap, theme, on_action,
            );

            if !status.is_empty() {
                let status_x = start_x + ACTION_BUTTONS.len() as f32 * (icon_w + gap) + 2.0;
                ui.painter().text(
                    Pos2::new(status_x, start_y + btn_h / 2.0),
                    Align2::LEFT_CENTER,
                    status,
                    egui::FontId::proportional(theme.fonts.status),
                    theme.colors.text_muted,
                );
            }
        }
        Orientation::Vertical => {
            let mut y = start_y;
            for btn in ACTION_BUTTONS {
                let btn_rect = Rect::from_min_size(Pos2::new(start_x, y), vec2(btn_h, btn_h));
                let enabled = match btn.action {
                    Action::Undo => can_undo,
                    Action::Redo => can_redo,
                    _ => true,
                };

                let id = Id::new(("action-btn-v", btn.name));
                let response = if enabled {
                    ui.interact(btn_rect, id, Sense::click())
                } else {
                    ui.interact(btn_rect, id, Sense::hover())
                };

                let bg = if !enabled {
                    theme.colors.button_bg_disabled
                } else if response.hovered() {
                    theme.colors.button_bg_hover
                } else {
                    theme.colors.button_bg_idle
                };
                ui.painter()
                    .rect_filled(btn_rect, theme.geometry.button_radius, bg);

                let fg = if enabled {
                    theme.colors.icon_fg_active
                } else {
                    theme.colors.icon_fg_disabled
                };
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    btn.icon,
                    egui::FontId::proportional(theme.fonts.icon),
                    fg,
                );

                if response.hovered() && enabled {
                    let tip = if let Some(sc) = btn.shortcut {
                        format!("{} ({})", btn.name, sc)
                    } else {
                        btn.name.to_string()
                    };
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    show_tooltip(
                        ui,
                        &tip,
                        Pos2::new(btn_rect.right() + 6.0, btn_rect.center().y - 10.0),
                        theme,
                    );
                }

                if response.clicked() && enabled {
                    on_action(btn.action);
                }

                y += btn_h + gap;
            }

            if !status.is_empty() {
                ui.painter().text(
                    Pos2::new(start_x + 14.0, y + 4.0),
                    Align2::CENTER_TOP,
                    status,
                    // Vertical status keeps its historical 11px; the themed
                    // `fonts.status` (12px) is used by the horizontal row.
                    egui::FontId::proportional(11.0),
                    theme.colors.text_muted,
                );
            }
        }
    }
}

fn draw_action_row_horizontal(
    can_undo: bool,
    can_redo: bool,
    ui: &mut egui::Ui,
    start_x: f32,
    y: f32,
    icon_w: f32,
    btn_h: f32,
    gap: f32,
    theme: &Theme,
    on_action: &mut impl FnMut(Action),
) {
    for (i, btn) in ACTION_BUTTONS.iter().enumerate() {
        let x = start_x + i as f32 * (icon_w + gap);
        let btn_rect = Rect::from_min_size(Pos2::new(x, y), vec2(icon_w, btn_h));

        let enabled = match btn.action {
            Action::Undo => can_undo,
            Action::Redo => can_redo,
            _ => true,
        };

        let id = Id::new(("action-btn-h", btn.name));
        let response = if enabled {
            ui.interact(btn_rect, id, Sense::click())
        } else {
            ui.interact(btn_rect, id, Sense::hover())
        };

        let bg = if !enabled {
            theme.colors.button_bg_disabled
        } else if response.hovered() {
            theme.colors.button_bg_hover
        } else {
            theme.colors.button_bg_idle
        };
        ui.painter()
            .rect_filled(btn_rect, theme.geometry.button_radius, bg);

        let fg = if enabled {
            theme.colors.icon_fg_active
        } else {
            theme.colors.icon_fg_disabled
        };
        ui.painter().text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            btn.icon,
            egui::FontId::proportional(theme.fonts.icon),
            fg,
        );

        if response.hovered() && enabled {
            let tip = if let Some(sc) = btn.shortcut {
                format!("{} ({})", btn.name, sc)
            } else {
                btn.name.to_string()
            };
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            show_tooltip(
                ui,
                &tip,
                Pos2::new(btn_rect.center().x - 20.0, btn_rect.top() - 24.0),
                theme,
            );
        }

        if response.clicked() && enabled {
            on_action(btn.action);
        }
    }
}
