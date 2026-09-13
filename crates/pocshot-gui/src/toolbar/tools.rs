//! Group 2 of the toolbar: the drawing / effect tool selection.

use eframe::egui::{self, vec2, Id, Pos2, Rect, Sense};
use egui_phosphor::fill as fill_icons;
use egui_phosphor::regular as icons;

use crate::annotation::AnnotationTool;
use crate::theme::Theme;
use crate::toolbar::{show_tooltip, Orientation};

pub(crate) struct ToolButtonDef {
    pub(crate) icon: &'static str,
    pub(crate) tool: AnnotationTool,
    pub(crate) name: &'static str,
}

pub(crate) const TOOL_BUTTONS: &[ToolButtonDef] = &[
    ToolButtonDef {
        icon: icons::CURSOR,
        tool: AnnotationTool::Select,
        name: "Select",
    },
    ToolButtonDef {
        icon: icons::ARROW_UP_RIGHT,
        tool: AnnotationTool::Arrow,
        name: "Arrow",
    },
    ToolButtonDef {
        icon: icons::LINE_SEGMENT,
        tool: AnnotationTool::Line,
        name: "Line",
    },
    ToolButtonDef {
        icon: icons::RECTANGLE,
        tool: AnnotationTool::Rectangle,
        name: "Rectangle",
    },
    ToolButtonDef {
        icon: icons::CIRCLE,
        tool: AnnotationTool::Circle,
        name: "Circle",
    },
    ToolButtonDef {
        icon: fill_icons::SQUARE,
        tool: AnnotationTool::FilledRectangle,
        name: "Filled rectangle",
    },
    ToolButtonDef {
        icon: fill_icons::CIRCLE,
        tool: AnnotationTool::FilledCircle,
        name: "Filled circle",
    },
    ToolButtonDef {
        icon: icons::PEN_NIB,
        tool: AnnotationTool::Pen,
        name: "Pen",
    },
    ToolButtonDef {
        icon: icons::HIGHLIGHTER,
        tool: AnnotationTool::Highlighter,
        name: "Highlighter",
    },
    ToolButtonDef {
        icon: icons::NUMBER_CIRCLE_ONE,
        tool: AnnotationTool::Counter,
        name: "Counter",
    },
    ToolButtonDef {
        icon: icons::GRID_NINE,
        tool: AnnotationTool::Pixelate,
        name: "Pixelate",
    },
    ToolButtonDef {
        icon: icons::WAVES,
        tool: AnnotationTool::Blur,
        name: "Blur",
    },
    ToolButtonDef {
        icon: icons::TEXT_T,
        tool: AnnotationTool::Text,
        name: "Text",
    },
    ToolButtonDef {
        icon: icons::BOUNDING_BOX,
        tool: AnnotationTool::HighlightText,
        name: "Text border",
    },
    ToolButtonDef {
        icon: fill_icons::EYE_SLASH,
        tool: AnnotationTool::Redact,
        name: "Redact",
    },
    ToolButtonDef {
        icon: icons::ERASER,
        tool: AnnotationTool::Eraser,
        name: "Eraser",
    },
];

pub fn draw_group_2_tools(
    active_tool: &mut AnnotationTool,
    ui: &mut egui::Ui,
    box_rect: Rect,
    orient: Orientation,
    theme: &Theme,
) {
    let c = &theme.colors;
    let g = &theme.geometry;
    ui.painter()
        .rect_filled(box_rect, g.box_radius, c.toolbar_bg);

    let pad = g.button_pad;
    let icon_w = g.button_w;
    let btn_h = g.button_h;
    let gap = g.button_gap;
    let start_x = box_rect.min.x + pad;
    let start_y = box_rect.min.y + pad;

    match orient {
        Orientation::Horizontal => {
            let mut x = start_x;
            for btn in TOOL_BUTTONS {
                let btn_rect = Rect::from_min_size(Pos2::new(x, start_y), vec2(icon_w, btn_h));
                let response =
                    ui.interact(btn_rect, Id::new(("tool-btn-h", btn.name)), Sense::click());

                let is_active = *active_tool == btn.tool;
                let bg = if is_active {
                    c.accent
                } else if response.hovered() {
                    c.button_bg_hover
                } else {
                    c.button_bg_idle
                };
                ui.painter().rect_filled(btn_rect, g.button_radius, bg);

                let fg = if is_active {
                    c.icon_fg_active
                } else {
                    c.icon_fg_idle
                };
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    btn.icon,
                    egui::FontId::proportional(theme.fonts.icon),
                    fg,
                );

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    show_tooltip(
                        ui,
                        btn.name,
                        Pos2::new(btn_rect.center().x - 20.0, btn_rect.top() - 24.0),
                        theme,
                    );
                }

                if response.clicked() {
                    *active_tool = btn.tool;
                }

                x += icon_w + gap;
            }
        }
        Orientation::Vertical => {
            let mut y = start_y;
            for btn in TOOL_BUTTONS {
                let btn_rect = Rect::from_min_size(Pos2::new(start_x, y), vec2(btn_h, btn_h));
                let response =
                    ui.interact(btn_rect, Id::new(("tool-btn-v", btn.name)), Sense::click());

                let is_active = *active_tool == btn.tool;
                let bg = if is_active {
                    c.accent
                } else if response.hovered() {
                    c.button_bg_hover
                } else {
                    c.button_bg_idle
                };
                ui.painter().rect_filled(btn_rect, g.button_radius, bg);

                let fg = if is_active {
                    c.icon_fg_active
                } else {
                    c.icon_fg_idle
                };
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    btn.icon,
                    egui::FontId::proportional(theme.fonts.icon),
                    fg,
                );

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    show_tooltip(
                        ui,
                        btn.name,
                        Pos2::new(btn_rect.right() + 6.0, btn_rect.center().y - 10.0),
                        theme,
                    );
                }

                if response.clicked() {
                    *active_tool = btn.tool;
                }

                y += btn_h + gap;
            }
        }
    }
}
