//! Editor state and input handling. All of this is toolkit-independent apart
//! from pushing properties/images into the Slint view, so the interesting logic
//! is testable.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use image::{imageops, RgbaImage};
use pocshot_annotate::{
    effects, handle_at, resize as resize_selection, Color32, HandleType, Pos2, Rect, Shape,
    ToolKind, Vec2,
};
use pocshot_ocr::TextRegion;
use pocshot_snap::SnapLines;
use slint::{
    ComponentHandle as _, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};
use slint::private_unstable_api::re_exports::MouseCursorInner;

use crate::platform;
use crate::{EditorWindow, HelpRow, Theme};

/// Annotation colour presets (keys 1..6).
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(0xff, 0x3b, 0x30),
    Color32::from_rgb(0xff, 0x95, 0x00),
    Color32::from_rgb(0xff, 0xd6, 0x0a),
    Color32::from_rgb(0x34, 0xc7, 0x59),
    Color32::from_rgb(0x0a, 0x84, 0xff),
    Color32::from_rgb(0xff, 0x2d, 0x95),
];

#[derive(Clone)]
pub struct Annotation {
    pub shape: Shape,
    pub color: Color32,
    pub width: f32,
}

/// Snapshot for undo: the base image plus the committed annotations.
struct HistoryEntry {
    base: Rc<RgbaImage>,
    annotations: Vec<Annotation>,
}

pub struct Editor {
    ui: EditorWindow,
    base: Rc<RgbaImage>,
    /// Base image with all committed annotations stamped on. Overlay shown to
    /// the UI and the source the live preview crops from (so annotations under
    /// the drag stay visible while dragging).
    composited: RgbaImage,
    annotations: Vec<Annotation>,
    current: Option<Shape>,
    locked_angle: Option<f32>,
    tool: ToolKind,
    color: Color32,
    width: f32,
    selection: Rect,
    select_anchor: Pos2,
    selecting: bool,
    /// Moving the existing selection (drag started inside it).
    moving_selection: bool,
    move_anchor: Pos2,
    move_origin: Rect,
    /// Resizing the selection via one of its handles.
    resizing: Option<HandleType>,
    /// True while a drag is building `current` (so hover previews are not
    /// committed on the next mouse-up).
    dragging_shape: bool,
    counter: u32,
    /// True while a text annotation is being typed (key events go to the buffer).
    typing_text: bool,
    /// Undo snapshots (base + annotations), newest last.
    history: Vec<HistoryEntry>,
    /// Snapshots undone and available to redo.
    redo_stack: Vec<HistoryEntry>,
    /// In-progress blur/pixelate region (image pixels).
    effect_anchor: Pos2,
    effect_rect: Option<Rect>,
    snap_lines: SnapLines,
    snap_enabled: bool,
    /// Active snap guide coordinates (image px) to draw while dragging.
    snap_x: Option<f32>,
    snap_y: Option<f32>,
    /// When the current snap guide was last hit, so it can flash and fade.
    snap_updated: Option<std::time::Instant>,
    ocr_rx: Option<mpsc::Receiver<Vec<TextRegion>>>,
    ocr_regions: Vec<TextRegion>,
    /// Merged paragraph-level regions (from `ocr_regions`), used for text-region
    /// fitting and the hover highlight, as egui does.
    ocr_merged: Vec<TextRegion>,
    /// Image-space origin of the crop the OCR regions are relative to.
    ocr_origin: Pos2,
    show_ocr: bool,
    /// Keeps the OCR poll timer alive.
    _timer: slint::Timer,
    // Settings.
    copy_on_save: bool,
    settings_visible: bool,
    text_size: f32,
    /// True when the image came from the clipboard (affects Refresh).
    source_is_clipboard: bool,
    settings: pocshot_config::AppSettings,
    /// Colour presets in use (theme swatches, falling back to the built-ins).
    palette: [Color32; 6],
    ocr_confidence: f32,
    help_visible: bool,
    /// Transient status message shown in the hint bar.
    status: Option<String>,
    /// Message shown in the settings panel's error/status line.
    error_text: String,
    /// Toolbar rect in window pixels (for hit-testing).
    toolbar_rect: Rect,
}

/// Padding around detected text regions for the highlight-text tool.
const TEXT_BORDER_PADDING: f32 = 6.0;
/// Redact boxes only pad vertically by one pixel.
const REDACT_VERTICAL_PADDING: f32 = 1.0;
/// A press-and-release that moved less than this (window px) in both axes is a
/// click (fit the one text block under the cursor); anything larger is a drag
/// (fit every block the drag touches).
const TEXT_CLICK_SLOP: f32 = 8.0;

/// Snap guide flash: the guide is drawn only while a drag is in progress, and
/// fades out this long after the last snap update.
const SNAP_GUIDE_FADE: std::time::Duration = std::time::Duration::from_millis(150);

/// Whether a text-region gesture is a click rather than a multi-block drag.
fn is_click_gesture(drag: Rect) -> bool {
    drag.width() <= TEXT_CLICK_SLOP && drag.height() <= TEXT_CLICK_SLOP
}

/// The pointer cursor for the active tool.
fn tool_cursor(tool: ToolKind) -> MouseCursorInner {
    use slint::private_unstable_api::re_exports::{BuiltInMouseCursor, MouseCursorInner};
    let cursor = match tool {
        // Select has its own per-region move/resize cursors.
        ToolKind::Select => BuiltInMouseCursor::Default,
        ToolKind::Text => BuiltInMouseCursor::Text,
        _ => BuiltInMouseCursor::Crosshair,
    };
    MouseCursorInner::BuiltIn(cursor)
}
/// Shortcut reference shown by the help overlay.
pub(crate) fn help_rows() -> Vec<HelpRow> {
    let row = |keys: &str, text: &str| HelpRow {
        keys: keys.into(),
        text: text.into(),
    };
    vec![
        row("V", "Select — drag to select, drag inside to move"),
        row("R / Shift+R", "Rectangle / filled rectangle"),
        row("L / A", "Line / arrow"),
        row("C / Shift+C", "Circle / filled circle"),
        row("P / H", "Pen / highlighter"),
        row("D / G", "Redact / text border (needs OCR)"),
        row("N / T", "Counter / text"),
        row("B / M", "Blur / pixelate"),
        row("E", "Eraser — click an annotation"),
        row("1–6", "Choose a colour"),
        row("[ / ]", "Stroke width"),
        row("Ctrl+drag", "Constrain to square / circle / 45°"),
        row("Enter / Ctrl+C", "Copy selection"),
        row("Ctrl+S", "Save selection"),
        row("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
        row("O", "OCR the selection"),
        row("Shift", "Temporarily disable snapping"),
        row("?", "Toggle this help"),
        row("Esc", "Clear selection, then quit"),
    ]
}

/// Human-readable name for the hint bar.
fn tool_label(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Select => "Select — drag to select, drag inside to move",
        ToolKind::Rectangle => "Rectangle",
        ToolKind::FilledRectangle => "Filled rectangle",
        ToolKind::Line => "Line",
        ToolKind::Arrow => "Arrow",
        ToolKind::Circle => "Circle",
        ToolKind::FilledCircle => "Filled circle",
        ToolKind::Pen => "Pen",
        ToolKind::Highlighter => "Highlighter",
        ToolKind::Redact => "Redact",
        ToolKind::Counter => "Counter",
        ToolKind::Text => "Text",
        ToolKind::Blur => "Blur",
        ToolKind::Pixelate => "Pixelate",
        ToolKind::Eraser => "Eraser",
        ToolKind::HighlightText => "Text border",
    }
}

/// Convert an `ecolor::Color32` to a Slint colour.
fn to_slint_color(color: Color32) -> slint::Color {
    slint::Color::from_argb_u8(color.a(), color.r(), color.g(), color.b())
}

/// Outer bounding box of a rectangle-like shape.
fn shape_bounds(shape: &Shape) -> Option<Rect> {
    match shape {
        Shape::Rectangle { rect, .. } => Some(*rect),
        Shape::Highlighter { rect } => Some(*rect),
        _ => None,
    }
}

/// Union of the text `regions` that intersect `drag`, grown by `padding`,
/// clamped to the image and — when `selection` is given — restricted to it, so
/// a text box extending past the selected region never produces a border
/// outside it. `None` when nothing was touched or the result is empty.
/// Ports egui's `app::text_border::text_border_rect`.
fn text_border_rect(
    drag: Rect,
    regions: &[Rect],
    image_size: Vec2,
    padding: Vec2,
    selection: Option<Rect>,
) -> Option<Rect> {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;

    for region in regions {
        if !region.intersects(drag) {
            continue;
        }
        any = true;
        min = min.min(region.min);
        max = max.max(region.max);
    }

    if !any {
        return None;
    }

    let mut border = Rect::from_min_max(
        Pos2::new(
            (min.x - padding.x).clamp(0.0, image_size.x),
            (min.y - padding.y).clamp(0.0, image_size.y),
        ),
        Pos2::new(
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

/// Toolbar order; index ↔ `ToolKind`.
const TOOL_ORDER: [ToolKind; 16] = [
    ToolKind::Select,
    ToolKind::Rectangle,
    ToolKind::FilledRectangle,
    ToolKind::Line,
    ToolKind::Arrow,
    ToolKind::Circle,
    ToolKind::FilledCircle,
    ToolKind::Pen,
    ToolKind::Highlighter,
    ToolKind::Redact,
    ToolKind::Counter,
    ToolKind::Text,
    ToolKind::Blur,
    ToolKind::Pixelate,
    ToolKind::Eraser,
    ToolKind::HighlightText,
];

impl Editor {
    pub fn new(
        ui: EditorWindow,
        base: RgbaImage,
        snap_lines: SnapLines,
        source_is_clipboard: bool,
        settings: pocshot_config::AppSettings,
    ) -> Rc<RefCell<Self>> {
        let text_size = settings.theme.fonts.annotation_text;
        let copy_on_save = settings.copy_on_save;
        let snap_enabled = settings.snap_enabled;
        let ocr_confidence = settings.ocr_confidence;
        let initial_width = settings.annotation_stroke_width.clamp(1.0, 24.0);
        let default_color = settings.theme.colors.default_annotation_color;
        let mut palette = PALETTE;
        for (index, color) in settings
            .theme
            .colors
            .color_swatches
            .iter()
            .take(6)
            .enumerate()
        {
            palette[index] = *color;
        }
        let size = Vec2::new(base.width() as f32, base.height() as f32);
        let composited = base.clone();
        let editor = Rc::new(RefCell::new(Self {
            ui,
            base: Rc::new(base),
            composited,
            annotations: Vec::new(),
            current: None,
            locked_angle: None,
            tool: ToolKind::Select,
            color: default_color,
            width: initial_width,
            selection: Rect::from_min_size(Pos2::ZERO, size),
            select_anchor: Pos2::ZERO,
            selecting: false,
            moving_selection: false,
            move_anchor: Pos2::ZERO,
            move_origin: Rect::NOTHING,
            resizing: None,
            dragging_shape: false,
            counter: 1,
            typing_text: false,
            history: Vec::new(),
            redo_stack: Vec::new(),
            effect_anchor: Pos2::ZERO,
            effect_rect: None,
            snap_lines,
            snap_enabled,
            snap_x: None,
            snap_y: None,
            snap_updated: None,
            ocr_rx: None,
            ocr_regions: Vec::new(),
            ocr_merged: Vec::new(),
            ocr_origin: Pos2::ZERO,
            show_ocr: false,
            _timer: slint::Timer::default(),
            copy_on_save,
            settings_visible: false,
            text_size,
            source_is_clipboard,
            settings,
            palette,
            ocr_confidence: ocr_confidence.clamp(0.1, 0.95),
            help_visible: false,
            status: None,
            error_text: String::new(),
            toolbar_rect: Rect::NOTHING,
        }));

        {
            let weak = Rc::downgrade(&editor);
            editor.borrow().ui.on_pointer(move |nx, ny, phase| {
                if let Some(editor) = weak.upgrade() {
                    editor.borrow_mut().on_pointer(nx, ny, phase);
                }
            });
        }
        {
            let weak = Rc::downgrade(&editor);
            editor.borrow().ui.on_key(move |text| {
                if let Some(editor) = weak.upgrade() {
                    editor.borrow_mut().on_key(&text);
                }
            });
        }
        {
            let weak = Rc::downgrade(&editor);
            editor.borrow().ui.on_tool(move |index| {
                if let Some(editor) = weak.upgrade() {
                    let mut editor = editor.borrow_mut();
                    if let Some(tool) = TOOL_ORDER.get(index.max(0) as usize) {
                        let tool = *tool;
                        editor.set_tool(tool);
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&editor);
            editor.borrow().ui.on_pick_color(move |index| {
                if let Some(editor) = weak.upgrade() {
                    let mut editor = editor.borrow_mut();
                    if let Some(color) = editor.palette.get(index.max(0) as usize) {
                        editor.color = *color;
                        editor.refresh_chrome();
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&editor);
            editor.borrow().ui.on_action(move |action| {
                if let Some(editor) = weak.upgrade() {
                    editor.borrow_mut().on_action(&action);
                }
            });
        }
        {
            let weak = Rc::downgrade(&editor);
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(50),
                move || {
                    if let Some(editor) = weak.upgrade() {
                        let mut editor = editor.borrow_mut();
                        editor.poll_ocr_work();
                        editor.refresh_snap_guide();
                    }
                },
            );
            editor.borrow_mut()._timer = timer;
        }

        editor.borrow_mut().rebuild_composited();
        editor.borrow().refresh_selection();
        editor.borrow().refresh_preview();
        editor.borrow().refresh_chrome();
        editor.borrow().apply_theme();
        editor.borrow().ui.set_help_rows(ModelRc::from(std::rc::Rc::new(
            VecModel::from(help_rows()),
        )));
        editor
            .borrow()
            .ui
            .set_hint_primary("Drag to select a region".into());
        if editor.borrow().settings.ocr_enabled {
            editor.borrow_mut().toggle_ocr();
        }
        editor
    }

    /// Push the theme colours into the Slint global.
    fn apply_theme(&self) {
        let theme = self.ui.global::<Theme>();
        let colors = &self.settings.theme.colors;
        theme.set_accent(to_slint_color(colors.accent));
        theme.set_chip_bg(to_slint_color(colors.button_bg_idle));
        theme.set_chip_hover(to_slint_color(colors.button_bg_hover));
        theme.set_chip_border(to_slint_color(colors.button_bg_disabled));
        theme.set_toolbar_bg(to_slint_color(colors.toolbar_bg));
        theme.set_hint_text(to_slint_color(colors.hint_text));
        theme.set_dim(to_slint_color(colors.overlay_dim));
        theme.set_sel_border(to_slint_color(colors.selection_border));
        theme.set_handle(to_slint_color(colors.handle_idle));
        theme.set_canvas_bg(to_slint_color(colors.canvas_bg));
        theme.set_tooltip_bg(to_slint_color(colors.tooltip_bg));
        theme.set_swatch0(to_slint_color(self.palette[0]));
        theme.set_swatch1(to_slint_color(self.palette[1]));
        theme.set_swatch2(to_slint_color(self.palette[2]));
        theme.set_swatch3(to_slint_color(self.palette[3]));
        theme.set_swatch4(to_slint_color(self.palette[4]));
        theme.set_swatch5(to_slint_color(self.palette[5]));
        theme.set_panel_bg(to_slint_color(colors.panel_bg));
        theme.set_panel_border(to_slint_color(colors.panel_border));
        theme.set_control_track(to_slint_color(colors.control_track));
        let ocr_box = colors.ocr_box;
        theme.set_ocr_hover(slint::Color::from_argb_u8(
            40,
            ocr_box.r(),
            ocr_box.g(),
            ocr_box.b(),
        ));
        theme.set_snap_line(to_slint_color(colors.snap_line_near));
        theme.set_icon_fg_idle(to_slint_color(colors.icon_fg_idle));
        theme.set_icon_fg_active(to_slint_color(colors.icon_fg_active));
        theme.set_icon_fg_disabled(to_slint_color(colors.icon_fg_disabled));
        theme.set_text_faint(to_slint_color(colors.text_faint));
        theme.set_tooltip_border(to_slint_color(colors.tooltip_border));
        let g = &self.settings.theme.geometry;
        theme.set_icon_size(g.icon_size);
        theme.set_icon_stroke(g.icon_stroke);
        let f = &self.settings.theme.fonts;
        theme.set_row_font(f.row);
        theme.set_section_font(f.section);
        theme.set_shortcut_font(f.shortcut);
        theme.set_panel_title_font(f.panel_title);
        theme.set_panel_radius(g.panel_radius);
        theme.set_control_radius(g.control_radius);
    }

    /// Write the current settings back to disk (best effort).
    fn persist_settings(&self) {
        pocshot_config::config::save(&self.settings);
    }

    /// Show the window (the editor must exist first so its properties are set).
    pub fn show(&self) -> Result<(), slint::PlatformError> {
        self.ui.show()
    }

    fn size(&self) -> Vec2 {
        Vec2::new(self.base.width() as f32, self.base.height() as f32)
    }

    fn image_rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size())
    }

    /// True when the selection covers the whole image (the initial state).
    fn selection_is_full(&self) -> bool {
        let image = self.image_rect();
        (self.selection.width() - image.width()).abs() < 1.0
            && (self.selection.height() - image.height()).abs() < 1.0
    }

    /// Clamp a selection to the image, keeping it at least 1 px in size.
    fn clamp_selection(&self, rect: Rect) -> Rect {
        let size = self.size();
        let mut r = rect;
        r.min.x = r.min.x.clamp(0.0, (size.x - 1.0).max(0.0));
        r.max.x = r.max.x.clamp(r.min.x + 1.0, size.x);
        r.min.y = r.min.y.clamp(0.0, (size.y - 1.0).max(0.0));
        r.max.y = r.max.y.clamp(r.min.y + 1.0, size.y);
        r
    }

    fn window_size(&self) -> Vec2 {
        let size = self.ui.window().size();
        Vec2::new(size.width as f32, size.height as f32)
    }

    /// Normalised (0..1) rect the image occupies in the window: native 1:1 when
    /// it fits, otherwise aspect-fit (centred). Screen captures match their
    /// monitor, so they land full-window.
    fn display_rect(&self) -> Rect {
        let win = self.window_size();
        let img = self.size();
        if win.x <= 0.0 || win.y <= 0.0 {
            return Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0));
        }
        let scale = if img.x <= win.x && img.y <= win.y {
            1.0
        } else {
            (win.x / img.x).min(win.y / img.y)
        };
        let size = img * scale;
        let min = Pos2::new((win.x - size.x) * 0.5, (win.y - size.y) * 0.5);
        Rect::from_min_size(
            Pos2::new(min.x / win.x, min.y / win.y),
            Vec2::new(size.x / win.x, size.y / win.y),
        )
    }

    /// Image-space rect → normalised window rect.
    fn image_to_norm(&self, rect: Rect) -> (f32, f32, f32, f32) {
        let display = self.display_rect();
        let img = self.size();
        (
            display.min.x + rect.min.x / img.x * display.width(),
            display.min.y + rect.min.y / img.y * display.height(),
            rect.width() / img.x * display.width(),
            rect.height() / img.y * display.height(),
        )
    }

    /// Normalised window coords → image-space point.
    fn norm_to_image(&self, nx: f32, ny: f32) -> Pos2 {
        let display = self.display_rect();
        if display.width() <= 0.0 || display.height() <= 0.0 {
            return Pos2::ZERO;
        }
        let img = self.size();
        Pos2::new(
            (nx - display.min.x) / display.width() * img.x,
            (ny - display.min.y) / display.height() * img.y,
        )
    }

    /// Publish the image placement (called after show and on every pointer
    /// event, so resizes are picked up).
    pub(crate) fn refresh_layout(&self) {
        let display = self.display_rect();
        self.ui.set_shot_nx(display.min.x);
        self.ui.set_shot_ny(display.min.y);
        self.ui.set_shot_nw(display.width());
        self.ui.set_shot_nh(display.height());
    }

    fn on_pointer(&mut self, nx: f32, ny: f32, phase: i32) {
        // Ignore presses that land on the toolbar so its buttons work, but keep
        // feeding move/release events to an in-progress gesture: a drag that
        // ends over the toolbar must still finish (otherwise the selection
        // never stops following the cursor).
        let dragging = self.selecting
            || self.moving_selection
            || self.resizing.is_some()
            || self.dragging_shape
            || self.effect_rect.is_some();
        if !dragging && self.pointer_in_toolbar(nx, ny) {
            self.hide_hover();
            return;
        }
        self.status = None;
        let (ctrl, shift) = platform::query_modifiers();
        let raw = self.norm_to_image(nx, ny);
        self.refresh_layout();
        // Snap to image/window edges unless Shift is held.
        let pos = if self.snap_enabled && !shift {
            let snapped_x = crate::snap::snap_pos(
                raw.x,
                &self.snap_lines.vertical,
                crate::snap::SNAP_DISTANCE,
            );
            let snapped_y = crate::snap::snap_pos(
                raw.y,
                &self.snap_lines.horizontal,
                crate::snap::SNAP_DISTANCE,
            );
            self.snap_x = ((snapped_x - raw.x).abs() > 0.01).then_some(snapped_x);
            self.snap_y = ((snapped_y - raw.y).abs() > 0.01).then_some(snapped_y);
            if self.snap_x.is_some() || self.snap_y.is_some() {
                self.snap_updated = Some(std::time::Instant::now());
            }
            Pos2::new(snapped_x, snapped_y)
        } else {
            self.snap_x = None;
            self.snap_y = None;
            self.snap_updated = None;
            raw
        };

        // The brief guide is shown only while a drag is actually in progress;
        // update_hover/refresh_chrome below would otherwise leave a stale one.
        if !dragging {
            self.snap_updated = None;
        }

        self.update_hover(pos);
        self.ui.set_tool_cursor(tool_cursor(self.tool));

        match phase {
            0 => {
                self.locked_angle = None;
                if self.tool == ToolKind::Select {
                    // Handles first, then move, then a fresh selection. A
                    // full-image selection counts as "no selection".
                    if let Some(handle) =
                        (!self.selection_is_full()).then(|| handle_at(self.selection, pos)).flatten()
                    {
                        self.resizing = Some(handle);
                    } else if self.selection.contains(pos) && !self.selection_is_full() {
                        self.moving_selection = true;
                        self.move_anchor = pos;
                        self.move_origin = self.selection;
                    } else {
                        self.selecting = true;
                        self.select_anchor = pos;
                        self.selection = Rect::from_two_pos(pos, pos);
                    }
                } else if self.tool == ToolKind::Eraser {
                    self.erase_at(pos);
                } else if self.tool == ToolKind::Counter {
                    self.current = Some(Shape::begin_counter(pos, self.counter));
                    self.dragging_shape = true;
                } else if self.tool == ToolKind::Text {
                    self.current = Some(Shape::text(pos, "", self.text_size));
                    self.typing_text = true;
                } else if self.tool == ToolKind::Blur || self.tool == ToolKind::Pixelate {
                    self.effect_anchor = pos;
                    self.effect_rect = Some(Rect::from_two_pos(pos, pos));
                } else if let Some(shape) = Shape::begin(self.tool, pos) {
                    self.effect_anchor = pos;
                    self.current = Some(shape);
                    self.dragging_shape = true;
                }
            }
            1 => {
                if let Some(handle) = self.resizing {
                    self.selection = self
                        .clamp_selection(resize_selection(self.selection, handle, pos));
                } else if self.moving_selection {
                    let delta = pos - self.move_anchor;
                    self.selection = self.clamp_selection(translate_clamped(
                        self.move_origin,
                        delta,
                        self.image_rect(),
                    ));
                } else if self.selecting {
                    self.selection = Rect::from_two_pos(self.select_anchor, pos);
                } else if self.effect_rect.is_some() {
                    self.effect_rect = Some(Rect::from_two_pos(self.effect_anchor, pos));
                } else if self.dragging_shape {
                    if let Some(shape) = self.current.as_mut() {
                        self.locked_angle = shape.update(pos, ctrl, self.locked_angle);
                    }
                } else if self.tool == ToolKind::Counter {
                    // Hover preview: the bubble follows the cursor.
                    self.current = Some(Shape::begin_counter(pos, self.counter));
                }
            }
            2 => {
                if self.resizing.is_some() {
                    self.resizing = None;
                } else if self.moving_selection {
                    self.moving_selection = false;
                } else if self.selecting {
                    self.selecting = false;
                    self.selection = self.selection.intersect(self.image_rect());
                } else if let Some(rect) = self.effect_rect.take() {
                    self.apply_effect(rect);
                } else if self.tool == ToolKind::Text {
                    // Text stays editable until Enter/Esc; mouse-up commits nothing.
                } else if self.dragging_shape {
                    // A short press-and-release is a click: fit the one block
                    // under the cursor. A real drag fits every block it covers.
                    let click_pos = (self.tool == ToolKind::Redact
                        || self.tool == ToolKind::HighlightText)
                        .then_some(self.effect_anchor);
                    self.dragging_shape = false;
                    if let Some(shape) = self.current.take() {
                        if matches!(self.tool, ToolKind::Redact | ToolKind::HighlightText) {
                            // These tools are defined by the text they cover, so
                            // they never commit a freehand rectangle.
                            let drag = shape_bounds(&shape).unwrap_or(Rect::NOTHING);
                            let target = if is_click_gesture(drag) {
                                // Point rect at the anchor, so only the block
                                // under the cursor is picked.
                                click_pos.map(|p| Rect::from_two_pos(p, p))
                            } else {
                                Some(drag)
                            };
                            self.commit_text_region(shape, target);
                        } else {
                            if shape.kind() == ToolKind::Counter {
                                self.counter += 1;
                            }
                            self.checkpoint();
                            self.annotations.push(Annotation {
                                shape,
                                color: self.color,
                                width: self.width,
                            });
                            self.rebuild_composited();
                        }
                    }
                }
            }
            _ => {}
        }

        self.refresh_selection();
        self.refresh_preview();
        self.refresh_snap_guide();
        self.refresh_toolbar();
        self.refresh_chrome();
    }

    fn on_action(&mut self, action: &str) {
        // Live slider feedback: update state + the readout, but never persist.
        if let Some(v) = action.strip_prefix("width-live:") {
            if let Ok(v) = v.parse::<f32>() {
                self.width = v.clamp(1.0, 24.0);
                self.ui.set_stroke_width(self.width as i32);
            }
            return;
        }
        if let Some(v) = action.strip_prefix("text-size-live:") {
            if let Ok(v) = v.parse::<f32>() {
                self.text_size = v.clamp(8.0, 72.0);
                self.ui.set_text_size(self.text_size as i32);
            }
            return;
        }
        if let Some(v) = action.strip_prefix("ocr-confidence-live:") {
            if let Ok(v) = v.parse::<f32>() {
                self.ocr_confidence = v.clamp(0.1, 0.95);
                self.ui.set_ocr_confidence(self.ocr_confidence);
                self.ui
                    .set_ocr_confidence_value(format!("{:.2}", self.ocr_confidence).into());
            }
            return;
        }
        // Committed slider values: persist once, on release.
        if let Some(v) = action.strip_prefix("width-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.width = v.round().clamp(1.0, 24.0);
                self.settings.annotation_stroke_width = self.width;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
        if let Some(v) = action.strip_prefix("text-size-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.text_size = v.round().clamp(8.0, 72.0);
                self.settings.theme.fonts.annotation_text = self.text_size;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
        if let Some(v) = action.strip_prefix("ocr-confidence-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.ocr_confidence = v.clamp(0.1, 0.95);
                self.settings.ocr_confidence = self.ocr_confidence;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
        if let Some(v) = action.strip_prefix("models-dir:") {
            self.settings.ocr_models_dir = v.to_string();
            self.persist_settings();
            return;
        }
        match action {
            "width-" => {
                self.width = (self.width - 1.0).max(1.0);
                self.settings.annotation_stroke_width = self.width;
                self.persist_settings();
                self.refresh_chrome();
            }
            "width+" => {
                self.width = (self.width + 1.0).min(24.0);
                self.settings.annotation_stroke_width = self.width;
                self.persist_settings();
                self.refresh_chrome();
            }
            "text-size-" => {
                self.text_size = (self.text_size - 2.0).max(8.0);
                self.settings.theme.fonts.annotation_text = self.text_size;
                self.persist_settings();
                self.refresh_chrome();
            }
            "text-size+" => {
                self.text_size = (self.text_size + 2.0).min(72.0);
                self.settings.theme.fonts.annotation_text = self.text_size;
                self.persist_settings();
                self.refresh_chrome();
            }
            "undo" => self.undo(),
            "redo" => self.redo(),
            "ocr" => self.toggle_ocr(),
            "copy" => self.copy_and_quit(),
            "save" => self.save_and_quit(),
            "pin" => self.pin_and_quit(),
            "refresh" => self.refresh_and_quit(),
            "clear-annotations" => {
                if !self.annotations.is_empty() {
                    self.checkpoint();
                    self.annotations.clear();
                    self.rebuild_composited();
                    self.refresh_chrome();
                }
            }
            "clear-selection" => {
                self.selection = self.image_rect();
                self.refresh_selection();
                self.refresh_toolbar();
            }
            "settings" => {
                self.settings_visible = !self.settings_visible;
                self.ui.set_settings_visible(self.settings_visible);
            }
            "copy-save" => {
                self.copy_on_save = !self.copy_on_save;
                self.settings.copy_on_save = self.copy_on_save;
                self.persist_settings();
                self.refresh_chrome();
            }
            "snap" => {
                self.snap_enabled = !self.snap_enabled;
                self.settings.snap_enabled = self.snap_enabled;
                self.persist_settings();
                self.refresh_chrome();
            }
            "ocr-confidence-" => {
                self.ocr_confidence = (self.ocr_confidence - 0.05).max(0.1);
                self.settings.ocr_confidence = self.ocr_confidence;
                self.persist_settings();
                self.refresh_chrome();
            }
            "ocr-confidence+" => {
                self.ocr_confidence = (self.ocr_confidence + 0.05).min(0.95);
                self.settings.ocr_confidence = self.ocr_confidence;
                self.persist_settings();
                self.refresh_chrome();
            }
            "reload-theme" => {
                self.settings = pocshot_config::config::load();
                self.text_size = self.settings.theme.fonts.annotation_text;
                self.ocr_confidence = self.settings.ocr_confidence.clamp(0.1, 0.95);
                self.copy_on_save = self.settings.copy_on_save;
                self.snap_enabled = self.settings.snap_enabled;
                let mut palette = PALETTE;
                for (index, color) in self
                    .settings
                    .theme
                    .colors
                    .color_swatches
                    .iter()
                    .take(6)
                    .enumerate()
                {
                    palette[index] = *color;
                }
                self.palette = palette;
                self.apply_theme();
                self.rebuild_composited();
                self.refresh_chrome();
            }
            "help" => {
                self.help_visible = !self.help_visible;
                self.ui.set_help_visible(self.help_visible);
            }
            "guides" => {
                self.settings.show_snap_lines = !self.settings.show_snap_lines;
                self.persist_settings();
                self.rebuild_composited();
                self.refresh_chrome();
            }
            "auto-ocr" => {
                self.settings.ocr_enabled = !self.settings.ocr_enabled;
                self.persist_settings();
                self.refresh_chrome();
            }
            "ocr-boxes" => {
                self.settings.show_text_boxes = !self.settings.show_text_boxes;
                self.show_ocr = self.settings.show_text_boxes;
                self.persist_settings();
                self.rebuild_composited();
                self.refresh_chrome();
            }
            "ocr-debug" => {
                self.settings.show_ocr_debug = !self.settings.show_ocr_debug;
                self.persist_settings();
                self.rebuild_composited();
                self.refresh_chrome();
            }
            "ocr-region-only" => {
                self.settings.ocr_region_only = !self.settings.ocr_region_only;
                self.persist_settings();
                self.refresh_chrome();
            }
            "reload-models" => {
                self.settings.ocr_models_dir = String::new();
                self.persist_settings();
                self.refresh_chrome();
            }
            "download-models" => self.download_models(),
            "quit" => crate::quit_event_loop(),
            _ => {}
        }
    }

    /// Ensure the OCR models exist locally (downloading them when needed) and
    /// report the outcome in the panel's error line.
    fn download_models(&mut self) {
        let dir = pocshot_config::config::ocr_models_dir(&self.settings.ocr_models_dir);
        match pocshot_ocr::models::ensure_models(&dir) {
            Ok((detection, _)) => {
                self.error_text = format!("Models ready in {}", dir.display());
                log::info!("OCR models ready at {} ({})", dir.display(), detection.display());
            }
            Err(error) => {
                self.error_text = format!("Model download failed: {error}");
                log::warn!("model download failed: {error:#}");
            }
        }
        self.ui.set_error_text(self.error_text.clone().into());
    }

    /// Selection rect in window pixels.
    fn selection_window_px(&self) -> Rect {
        let display = self.display_rect();
        let win = self.window_size();
        let image = self.size();
        let to_win = |p: Pos2| {
            Pos2::new(
                display.min.x * win.x + p.x / image.x * display.width() * win.x,
                display.min.y * win.y + p.y / image.y * display.height() * win.y,
            )
        };
        Rect::from_min_max(to_win(self.selection.min), to_win(self.selection.max))
    }

    /// Place the toolbar near the selection using the pure `layout` math.
    pub(crate) fn refresh_toolbar(&mut self) {
        let win = self.window_size();
        if win.x <= 0.0 || win.y <= 0.0 {
            return;
        }
        let g = &self.settings.theme.geometry;
        let spec = crate::layout::ToolbarSpec {
            button: 32.0,
            button_gap: 4.0,
            swatch: 26.0,
            swatch_gap: 4.0,
            sep_w: 1.0,
            pad: g.toolbar_padding,
            margin: 8.0,
            row_h: 40.0,
            hint_h: 28.0,
            n_actions: 12,
            n_tools: 16,
            n_swatches: 6,
        };
        let sel = self.selection_window_px();
        let l = crate::layout::toolbar_layout(
            (win.x, win.y),
            (sel.min.x, sel.min.y, sel.width(), sel.height()),
            &spec,
        );

        self.ui.set_toolbar_x(l.x);
        self.ui.set_toolbar_y(l.y);
        self.ui.set_toolbar_width(l.width);
        self.ui.set_toolbar_height(l.height);
        self.ui.set_toolbar_wrapped(l.wrapped);
        self.ui.set_toolbar_tooltips_below(l.tooltips_below);
        self.toolbar_rect = Rect::from_min_size(Pos2::new(l.x, l.y), Vec2::new(l.width, l.height));
    }

    /// True when the normalised pointer position falls over the toolbar.
    fn pointer_in_toolbar(&self, nx: f32, ny: f32) -> bool {
        let win = self.window_size();
        if win.x <= 0.0 || win.y <= 0.0 {
            return false;
        }
        self.toolbar_rect
            .contains(Pos2::new(nx * win.x, ny * win.y))
    }

    /// Re-capture (or re-read the clipboard) from scratch: hide, respawn a
    /// fresh editor process, quit.
    fn refresh_and_quit(&mut self) {
        let args: Vec<String> = if self.source_is_clipboard {
            vec!["--ui".into(), "slint".into(), "edit".into()]
        } else {
            vec!["--ui".into(), "slint".into()]
        };
        // The window is opaque and would land in the new capture: hide first,
        // then give the new process a moment before it captures.
        let _ = self.ui.window().hide();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(exe) = std::env::current_exe() {
                let mut cmd = std::process::Command::new(exe);
                cmd.args(&args);
                pocshot_core::quiet_io(&mut cmd);
                pocshot_core::detach(&mut cmd);
                if let Err(error) = cmd.spawn() {
                    log::error!("failed to respawn: {error}");
                }
            }
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        });
    }

    fn on_key(&mut self, text: &str) {
        if self.typing_text {
            self.on_text_key(text);
            return;
        }
        if text == "?" || text == "/" {
            self.help_visible = !self.help_visible;
            self.ui.set_help_visible(self.help_visible);
            return;
        }

        let (ctrl, shift) = platform::query_modifiers();
        let lower = text.to_ascii_lowercase();

        if text == "q" {
            crate::quit_event_loop();
            return;
        }
        if text == "\u{1b}" {
            // Esc: first clears the selection, then quits.
            if self.selection_is_full() {
                crate::quit_event_loop();
            } else {
                self.selection = self.image_rect();
                self.refresh_selection();
                self.refresh_toolbar();
                self.refresh_chrome();
            }
            return;
        }
        if (ctrl && lower == "c") || text == "\n" || text == "\u{0d}" || text == " " {
            self.copy_and_quit();
            return;
        }
        if ctrl && lower == "s" {
            self.save_and_quit();
            return;
        }
        if ctrl && lower == "z" {
            if shift {
                self.redo();
            } else {
                self.undo();
            }
            return;
        }
        if ctrl && lower == "y" {
            self.redo();
            return;
        }
        if ctrl {
            return;
        }

        let shifted = shift || text.chars().any(|c| c.is_ascii_uppercase());
        match lower.as_str() {
            "v" => self.set_tool(ToolKind::Select),
            "r" => self.set_tool(if shifted {
                ToolKind::FilledRectangle
            } else {
                ToolKind::Rectangle
            }),
            "l" => self.set_tool(ToolKind::Line),
            "a" => self.set_tool(ToolKind::Arrow),
            "c" => self.set_tool(if shifted {
                ToolKind::FilledCircle
            } else {
                ToolKind::Circle
            }),
            "p" => self.set_tool(ToolKind::Pen),
            "h" => self.set_tool(ToolKind::Highlighter),
            "d" => self.set_tool(ToolKind::Redact),
            "g" => self.set_tool(ToolKind::HighlightText),
            "n" => self.set_tool(ToolKind::Counter),
            "t" => self.set_tool(ToolKind::Text),
            "b" => self.set_tool(ToolKind::Blur),
            "m" => self.set_tool(ToolKind::Pixelate),
            "e" => self.set_tool(ToolKind::Eraser),
            "o" => self.toggle_ocr(),
            "[" => {
                self.width = (self.width - 1.0).max(1.0);
                self.refresh_chrome();
            }
            "]" => {
                self.width = (self.width + 1.0).min(24.0);
                self.refresh_chrome();
            }
            digit if digit.len() == 1 => {
                if let Some(index) = digit
                    .chars()
                    .next()
                    .and_then(|c| c.to_digit(10))
                    .filter(|i| (1..=6).contains(i))
                {
                    self.color = PALETTE[index as usize - 1];
                    self.refresh_chrome();
                }
            }
            _ => {}
        }
    }

    /// Key events while typing a text annotation: Esc cancels, Enter commits,
    /// Backspace/Delete edits, printable characters append.
    fn on_text_key(&mut self, text: &str) {
        match text {
            "\u{1b}" => {
                self.current = None;
                self.typing_text = false;
            }
            "\n" | "\u{0d}" => {
                if let Some(shape) = self.current.take() {
                    let keep = matches!(&shape, Shape::Text { text, .. } if !text.trim().is_empty());
                    if keep {
                        self.checkpoint();
                        self.annotations.push(Annotation {
                            shape,
                            color: self.color,
                            width: self.width,
                        });
                    }
                    self.rebuild_composited();
                }
                self.typing_text = false;
            }
            "\u{8}" | "\u{7f}" => {
                if let Some(Shape::Text { text, .. }) = self.current.as_mut() {
                    text.pop();
                }
            }
            other if !other.is_empty() && other.chars().all(|c| !c.is_control()) => {
                if let Some(Shape::Text { text, .. }) = self.current.as_mut() {
                    text.push_str(other);
                }
            }
            _ => return,
        }
        self.refresh_preview();
        self.refresh_chrome();
    }

    fn set_tool(&mut self, tool: ToolKind) {
        // Drop a counter hover preview when switching to another tool.
        if tool != ToolKind::Counter && !self.dragging_shape && self.current.is_some() {
            self.current = None;
            self.refresh_preview();
        }
        self.tool = tool;
        self.refresh_chrome();
    }

    /// Repaint the committed overlay (base + annotations) and publish it.
    fn rebuild_composited(&mut self) {
        self.composited = (*self.base).clone();
        render_annotations(&mut self.composited, &self.annotations, Pos2::ZERO);
        if self.show_ocr {
            let color = Color32::from_rgb(0, 230, 255);
            let origin = self.ocr_origin;
            for region in &self.ocr_regions {                let r = region.rect;
                let min = Pos2::new(r.x0 + origin.x, r.y0 + origin.y);
                let max = Pos2::new(r.x1 + origin.x, r.y1 + origin.y);
                for (a, b) in [
                    (min, Pos2::new(max.x, min.y)),
                    (Pos2::new(max.x, min.y), max),
                    (max, Pos2::new(min.x, max.y)),
                    (Pos2::new(min.x, max.y), min),
                ] {
                    pocshot_annotate::raster::draw_line_on_image(&mut self.composited, a, b, color, 1.0);
                }
            }
        }
        if self.settings.show_snap_lines {
            let color = self.settings.theme.colors.snap_line_faint;
            let (w, h) = (self.size().x, self.size().y);
            for x in &self.snap_lines.vertical {
                pocshot_annotate::raster::draw_line_on_image(
                    &mut self.composited,
                    Pos2::new(*x, 0.0),
                    Pos2::new(*x, h),
                    color,
                    1.0,
                );
            }
            for y in &self.snap_lines.horizontal {
                pocshot_annotate::raster::draw_line_on_image(
                    &mut self.composited,
                    Pos2::new(0.0, *y),
                    Pos2::new(w, *y),
                    color,
                    1.0,
                );
            }
        }
        self.ui.set_overlay(to_slint_image(&self.composited));
    }

    fn snapshot(&self) -> HistoryEntry {
        HistoryEntry {
            base: self.base.clone(),
            annotations: self.annotations.clone(),
        }
    }

    /// Toggle OCR on the selection: start detection in the background, or hide
    /// existing regions.
    fn toggle_ocr(&mut self) {
        if self.show_ocr {
            self.show_ocr = false;
            self.ocr_regions.clear();
            self.ocr_merged.clear();
            self.hide_hover();
            self.settings.show_text_boxes = false;
            self.persist_settings();
            self.rebuild_composited();
            return;
        }
        self.settings.show_text_boxes = true;
        self.persist_settings();
        let rect = self.selection.intersect(self.image_rect());
        if rect.width() < 2.0 || rect.height() < 2.0 {
            log::warn!("selection too small for OCR");
            return;
        }
        let x0 = rect.min.x.floor().max(0.0) as u32;
        let y0 = rect.min.y.floor().max(0.0) as u32;
        let x1 = rect.max.x.ceil().min(self.size().x) as u32;
        let y1 = rect.max.y.ceil().min(self.size().y) as u32;
        let crop = imageops::crop_imm(self.base.as_ref(), x0, y0, x1 - x0, y1 - y0).to_image();
        self.ocr_origin = Pos2::new(x0 as f32, y0 as f32);
        let (tx, rx) = mpsc::channel();
        self.ocr_rx = Some(rx);
        log::info!("running OCR on {}x{} selection…", x1 - x0, y1 - y0);
        std::thread::spawn(move || {
            use pocshot_ocr::detect::{OcrsDetector, TextDetector};
            use pocshot_ocr::SourceImage;
            let dir = pocshot_core::models_dir(None);
            let result = (|| -> anyhow::Result<Vec<TextRegion>> {
                let (detection, recognition) = pocshot_ocr::models::ensure_models(&dir)?;
                let detector = OcrsDetector::new(&detection, &recognition)?;
                let dynamic = image::DynamicImage::ImageRgba8(crop);
                detector.detect(&SourceImage::Borrowed(&dynamic))
            })();
            match result {
                Ok(regions) => {
                    let _ = tx.send(regions);
                }
                Err(error) => log::error!("OCR failed: {error}"),
            }
        });
    }

    /// Poll for OCR results (called from a Slint timer).
    fn poll_ocr_work(&mut self) {
        let Some(rx) = &self.ocr_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(regions) => {
                self.ocr_rx = None;
                let regions = pocshot_ocr::postprocess::filter_by_confidence(
                    regions,
                    self.ocr_confidence,
                );
                for region in &regions {
                    log::info!(
                        "ocr: {:.2} {:?}",
                        region.confidence,
                        region.text.as_deref().unwrap_or("")
                    );
                }
                self.ocr_regions = regions.clone();
                self.ocr_merged = pocshot_ocr::postprocess::merge_text_regions(
                    regions,
                    pocshot_ocr::postprocess::MergeConfig::default(),
                );
                self.show_ocr = true;
                self.status = Some(format!(
                    "OCR: {} text regions",
                    self.ocr_merged.len()
                ));
                self.rebuild_composited();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ocr_rx = None;
            }
        }
    }

    /// OCR regions in image space (already offset by the crop origin), merged
    /// to paragraph level so a border/hover covers the whole text block.
    fn text_regions(&self) -> Vec<Rect> {
        let origin = self.ocr_origin;
        self.ocr_merged
            .iter()
            .map(|region| {
                let r = region.rect;
                Rect::from_min_max(
                    Pos2::new(r.x0 + origin.x, r.y0 + origin.y),
                    Pos2::new(r.x1 + origin.x, r.y1 + origin.y),
                )
            })
            .collect()
    }

    /// Fit a Redact/Text-border gesture to OCR regions and commit the result.
    /// `target` is the image-space rect to fit to: the drag rect for a
    /// multi-block drag, or a zero-size point at the cursor for a click. These
    /// tools never commit a freehand rectangle (mirrors egui).
    fn commit_text_region(&mut self, mut shape: Shape, target: Option<Rect>) {
        let Some(drag) = target else {
            return;
        };
        let image = self.image_rect();
        let selection = (!self.selection_is_full()).then(|| self.selection.intersect(image));
        let padding = if self.tool == ToolKind::Redact {
            Vec2::new(0.0, REDACT_VERTICAL_PADDING)
        } else {
            Vec2::splat(TEXT_BORDER_PADDING)
        };
        match text_border_rect(
            drag,
            &self.text_regions(),
            Vec2::new(image.width(), image.height()),
            padding,
            selection,
        ) {
            Some(border) => {
                shape.replace_rect(border);
                self.checkpoint();
                self.annotations.push(Annotation {
                    shape,
                    color: self.color,
                    width: self.width,
                });
                self.rebuild_composited();
                self.status = Some(
                    if self.tool == ToolKind::Redact {
                        "Text redacted"
                    } else {
                        "Text border added"
                    }
                    .into(),
                );
            }
            None if self.ocr_merged.is_empty() && !self.settings.ocr_enabled => {
                self.status = Some("Enable text detection in Settings to frame text".into());
            }
            None => {
                self.status = Some(
                    if selection.is_some() {
                        "No text detected inside the selection"
                    } else {
                        "No text detected in that area"
                    }
                    .into(),
                );
            }
        }
    }

    /// Highlight the detected text region under `pos` (image space). Mirrors
    /// egui's cursor hover fill.
    fn update_hover(&self, pos: Pos2) {
        for region in self.text_regions() {
            if region.contains(pos) {
                let (nx, ny, nw, nh) = self.image_to_norm(region);
                self.ui.set_hover_nx(nx);
                self.ui.set_hover_ny(ny);
                self.ui.set_hover_nw(nw);
                self.ui.set_hover_nh(nh);
                self.ui.set_hover_visible(true);
                return;
            }
        }
        self.ui.set_hover_visible(false);
    }

    fn hide_hover(&self) {
        self.ui.set_hover_visible(false);
    }

    /// Remove the topmost committed annotation under `pos` (eraser).
    fn erase_at(&mut self, pos: Pos2) {
        let hit = self
            .annotations
            .iter()
            .rposition(|annotation| annotation.shape.hit_test(pos, 6.0));
        if let Some(index) = hit {
            self.checkpoint();
            self.annotations.remove(index);
            self.rebuild_composited();
        }
    }

    /// Record the current state before a mutation; clears the redo stack.
    fn checkpoint(&mut self) {
        self.history.push(self.snapshot());
        self.redo_stack.clear();
    }

    /// Restore the previous base/annotations snapshot.
    fn undo(&mut self) {
        if let Some(entry) = self.history.pop() {
            let current = self.snapshot();
            self.redo_stack.push(current);
            self.restore(entry);
            self.refresh_chrome();
        }
    }

    fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            let current = self.snapshot();
            self.history.push(current);
            self.restore(entry);
            self.refresh_chrome();
        }
    }

    fn restore(&mut self, entry: HistoryEntry) {
        self.base = entry.base;
        self.annotations = entry.annotations;
        self.rebuild_composited();
        self.refresh_preview();
    }

    /// Apply the current destructive tool to `rect` (image pixels).
    fn apply_effect(&mut self, rect: Rect) {
        let rect = rect.intersect(self.image_rect());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let tool = self.tool;
        self.checkpoint();
        let base = Rc::make_mut(&mut self.base);
        match tool {
            ToolKind::Pixelate => effects::pixelate_region(base, rect, 16),
            ToolKind::Blur => effects::blur_region(base, rect, 12.0),
            _ => {}
        }
        self.rebuild_composited();
    }

    fn refresh_preview(&self) {
        if let Some(rect) = self.effect_rect {
            let size = self.size();
            let bbox = rect.intersect(self.image_rect());
            if bbox.width() < 1.0 || bbox.height() < 1.0 {
                self.ui.set_preview_visible(false);
                return;
            }
            let x0 = bbox.min.x.floor().max(0.0) as u32;
            let y0 = bbox.min.y.floor().max(0.0) as u32;
            let x1 = bbox.max.x.ceil().min(size.x) as u32;
            let y1 = bbox.max.y.ceil().min(size.y) as u32;
            let mut image =
                imageops::crop_imm(&self.composited, x0, y0, x1 - x0, y1 - y0).to_image();
            // Tint the region so the effect target is obvious.
            pocshot_annotate::raster::fill_rect_on_image(
                &mut image,
                Pos2::ZERO,
                Pos2::new((x1 - x0) as f32, (y1 - y0) as f32),
                Color32::from_rgba_unmultiplied(255, 51, 102, 70),
            );
            self.publish_preview(image, x0, y0, x1, y1);
            return;
        }

        let Some(shape) = self.current.as_ref() else {
            self.ui.set_preview_visible(false);
            return;
        };
        let size = self.size();
        let bbox = shape
            .bounds(self.width)
            .intersect(self.image_rect())
            .intersect(Rect::from_min_size(Pos2::ZERO, size));
        if bbox.width() < 1.0 || bbox.height() < 1.0 {
            self.ui.set_preview_visible(false);
            return;
        }
        let x0 = bbox.min.x.floor().max(0.0) as u32;
        let y0 = bbox.min.y.floor().max(0.0) as u32;
        let x1 = bbox.max.x.ceil().min(size.x) as u32;
        let y1 = bbox.max.y.ceil().min(size.y) as u32;
        let mut image = imageops::crop_imm(&self.composited, x0, y0, x1 - x0, y1 - y0).to_image();
        shape.render(&mut image, self.color, self.width, Pos2::new(x0 as f32, y0 as f32));

        self.publish_preview(image, x0, y0, x1, y1);
    }

    /// Briefly overlay the active snap guides as plain full-length lines, drawn
    /// by Slint (no image upload): egui's "brief snap indicators". They are
    /// shown only while a gesture is in progress and fade out shortly after the
    /// last snap update.
    fn refresh_snap_guide(&mut self) {
        let Some(updated) = self.snap_updated else {
            self.ui.set_snap_guide_visible(false);
            return;
        };
        let dragging = self.selecting
            || self.moving_selection
            || self.resizing.is_some()
            || self.dragging_shape
            || self.effect_rect.is_some();
        if !dragging {
            self.snap_updated = None;
            self.ui.set_snap_guide_visible(false);
            return;
        }
        let age = updated.elapsed();
        if age >= SNAP_GUIDE_FADE {
            self.snap_updated = None;
            self.ui.set_snap_guide_visible(false);
            return;
        }
        let (w, h) = (self.size().x, self.size().y);
        // One property update per visible line: no full-capture copy, no image
        // upload on every tick.
        match self.snap_x {
            Some(sx) => {
                self.ui.set_snap_guide_x(sx);
                self.ui.set_snap_guide_has_vertical(true);
            }
            None => self.ui.set_snap_guide_has_vertical(false),
        }
        match self.snap_y {
            Some(sy) => {
                self.ui.set_snap_guide_y(sy);
                self.ui.set_snap_guide_has_horizontal(true);
            }
            None => self.ui.set_snap_guide_has_horizontal(false),
        }
        self.ui.set_snap_guide_width(w);
        self.ui.set_snap_guide_height(h);
        self.ui.set_snap_guide_visible(true);
    }

    fn publish_preview(&self, image: RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) {
        let (nx, ny, nw, nh) = self.image_to_norm(Rect::from_min_max(
            Pos2::new(x0 as f32, y0 as f32),
            Pos2::new(x1 as f32, y1 as f32),
        ));
        self.ui.set_preview(to_slint_image(&image));
        self.ui.set_preview_nx(nx);
        self.ui.set_preview_ny(ny);
        self.ui.set_preview_nw(nw);
        self.ui.set_preview_nh(nh);
        self.ui.set_preview_visible(true);
    }

    fn refresh_selection(&self) {
        let (nx, ny, nw, nh) = self.image_to_norm(self.selection);
        self.ui.set_sel_nx(nx);
        self.ui.set_sel_ny(ny);
        self.ui.set_sel_nw(nw);
        self.ui.set_sel_nh(nh);
        self.ui.set_sel_visible(true);
        self.ui.set_sel_handles_visible(!self.selection_is_full());
    }

    fn refresh_chrome(&self) {
        self.ui
            .set_active_tool(self.tool_index());
        self.ui
            .set_active_color(self.palette.iter().position(|c| *c == self.color).unwrap_or(0) as i32);
        self.ui.set_stroke_width(self.width as i32);
        self.ui.set_text_size(self.text_size as i32);
        self.ui.set_can_undo(!self.history.is_empty());
        self.ui.set_can_redo(!self.redo_stack.is_empty());
        self.ui.set_ocr_confidence(self.ocr_confidence);
        self.ui
            .set_ocr_confidence_value(format!("{:.2}", self.ocr_confidence).into());
        self.ui
            .set_copy_save_value(if self.copy_on_save { "on" } else { "off" }.into());
        self.ui
            .set_snap_value(if self.snap_enabled { "on" } else { "off" }.into());
        self.ui.set_guides_value(
            if self.settings.show_snap_lines { "on" } else { "off" }.into(),
        );
        self.ui.set_auto_ocr_value(
            if self.settings.ocr_enabled { "on" } else { "off" }.into(),
        );
        self.ui.set_ocr_boxes_value(
            if self.settings.show_text_boxes { "on" } else { "off" }.into(),
        );
        self.ui.set_ocr_debug_value(
            if self.settings.show_ocr_debug { "on" } else { "off" }.into(),
        );
        self.ui.set_ocr_region_only_value(
            if self.settings.ocr_region_only { "on" } else { "off" }.into(),
        );
        self.ui
            .set_models_dir(self.settings.ocr_models_dir.clone().into());
        self.ui.set_error_text(self.error_text.clone().into());
        if self.typing_text {
            self.ui
                .set_status("Type text · Enter commit · Esc cancel".into());
            self.ui.set_hint_chips(ModelRc::from(std::rc::Rc::new(VecModel::from(vec![
                SharedString::from("Backspace Delete"),
            ]))));
            return;
        }
        self.ui.set_status(match &self.status {
            Some(status) => SharedString::from(status.as_str()),
            None => SharedString::from(tool_label(self.tool)),
        });
        self.ui.set_hint_chips(ModelRc::from(std::rc::Rc::new(VecModel::from(vec![
            SharedString::from("Enter Copy"),
            SharedString::from("Ctrl+S Save"),
            SharedString::from("Ctrl+Z Undo"),
            SharedString::from("Esc Cancel"),
        ]))));
    }

    fn tool_index(&self) -> i32 {
        TOOL_ORDER
            .iter()
            .position(|t| *t == self.tool)
            .unwrap_or(0) as i32
    }

    /// The selection with all annotations stamped on, ready to copy/save.
    fn selected_image(&self) -> Option<RgbaImage> {
        crop_and_render(self.base.as_ref(), &self.annotations, self.selection)
    }

    fn copy_and_quit(&self) {
        match self.selected_image() {
            Some(image) => match pocshot_core::copy_rgba_to_clipboard(&image) {
                Ok(true) => log::info!("copied {}x{} selection", image.width(), image.height()),
                Ok(false) => log::warn!("clipboard reported no image copied"),
                Err(error) => log::error!("failed to copy selection: {error}"),
            },
            None => log::warn!("empty selection, nothing copied"),
        }
        crate::quit_after_grace();
    }

    /// Save the selection to a temp PNG and spawn a pin window for it.
    fn pin_and_quit(&self) {
        let Some(image) = self.selected_image() else {
            log::warn!("empty selection, nothing pinned");
            return;
        };
        let path = std::env::temp_dir().join(format!("pocshot-pin-{}.png", std::process::id()));
        if let Err(error) = image.save(&path) {
            log::error!("failed to write pin image: {error}");
            return;
        }

        let win = self.window_size();
        let display = self.display_rect();
        let origin = self.ui.window().position();
        let scale_x = display.width() * win.x / self.size().x;
        let scale_y = display.height() * win.y / self.size().y;
        let x = origin.x + (display.min.x * win.x) as i32 + (self.selection.min.x * scale_x) as i32;
        let y = origin.y + (display.min.y * win.y) as i32 + (self.selection.min.y * scale_y) as i32;
        let width = (self.selection.width() * scale_x).max(1.0) as u32;
        let height = (self.selection.height() * scale_y).max(1.0) as u32;

        match std::env::current_exe() {
            Ok(exe) => {
                let mut cmd = std::process::Command::new(exe);
                cmd.args(["--ui", "slint", "pin"])
                    .arg(&path)
                    .args([
                        x.to_string(),
                        y.to_string(),
                        width.to_string(),
                        height.to_string(),
                    ]);
                pocshot_core::quiet_io(&mut cmd);
                pocshot_core::detach(&mut cmd);
                if let Err(error) = cmd.spawn() {
                    log::error!("failed to spawn pin process: {error}");
                }
            }
            Err(error) => log::error!("cannot find own executable: {error}"),
        }
        crate::quit_event_loop();
    }

    fn save_and_quit(&self) {
        let Some(image) = self.selected_image() else {
            log::warn!("empty selection, nothing saved");
            crate::quit_event_loop();
            return;
        };
        let path = pocshot_core::default_output_path(pocshot_core::OutputFormat::Png);
        match pocshot_core::save_rgba(&image, &path, pocshot_core::OutputFormat::Png, 90) {
            Ok(()) => log::info!("saved {} ({}x{})", path.display(), image.width(), image.height()),
            Err(error) => log::error!("failed to save {}: {error}", path.display()),
        }
        if self.copy_on_save {
            match pocshot_core::copy_rgba_to_clipboard(&image) {
                Ok(true) => log::info!("also copied to clipboard"),
                Ok(false) => log::warn!("clipboard reported no image copied"),
                Err(error) => log::error!("failed to copy: {error}"),
            }
            crate::quit_after_grace();
        } else {
            crate::quit_event_loop();
        }
    }
}

/// Translate `rect` by `delta`, clamping so it stays inside `bounds`.
fn translate_clamped(rect: Rect, delta: Vec2, bounds: Rect) -> Rect {
    let size = rect.size();
    let mut min = rect.min + delta;
    min.x = min
        .x
        .clamp(bounds.min.x, (bounds.max.x - size.x).max(bounds.min.x));
    min.y = min
        .y
        .clamp(bounds.min.y, (bounds.max.y - size.y).max(bounds.min.y));
    Rect::from_min_size(min, size)
}

/// Stamp every annotation onto `image`, offset by `offset` image pixels.
pub fn render_annotations(image: &mut RgbaImage, annotations: &[Annotation], offset: Pos2) {
    for annotation in annotations {
        annotation
            .shape
            .render(image, annotation.color, annotation.width, offset);
    }
}

/// Crop `base` to `selection` (clamped) and stamp the annotations on it.
pub fn crop_and_render(
    base: &RgbaImage,
    annotations: &[Annotation],
    selection: Rect,
) -> Option<RgbaImage> {
    let bounds = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(base.width() as f32, base.height() as f32),
    );
    let r = selection.intersect(bounds);
    if !r.is_positive() {
        return None;
    }
    let x0 = r.min.x.floor().max(0.0) as u32;
    let y0 = r.min.y.floor().max(0.0) as u32;
    let x1 = r.max.x.ceil().min(base.width() as f32) as u32;
    let y1 = r.max.y.ceil().min(base.height() as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut image = imageops::crop_imm(base, x0, y0, x1 - x0, y1 - y0).to_image();
    render_annotations(&mut image, annotations, Pos2::new(x0 as f32, y0 as f32));
    Some(image)
}

pub fn to_slint_image(image: &RgbaImage) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    );
    Image::from_rgba8(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pocshot_annotate::ToolKind;

    #[test]
    fn help_rows_cover_the_three_sections() {
        let rows = help_rows();
        assert!(rows.len() >= 10);
        let joined = rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("|");
        assert!(joined.contains("Select"));
        assert!(joined.contains("Undo"));
        assert!(joined.to_lowercase().contains("copy"));
        assert!(rows.iter().all(|r| !r.keys.is_empty() && !r.text.is_empty()));
    }

    fn gradient(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([x as u8, y as u8, 0, 255])
        })
    }

    #[test]
    fn crop_and_render_maps_selection_to_pixels() {
        let base = gradient(100, 50);
        let crop = crop_and_render(&base, &[], Rect::from_min_max(pos2(10.0, 10.0), pos2(50.0, 40.0)))
            .unwrap();
        assert_eq!((crop.width(), crop.height()), (40, 30));
        assert_eq!(crop.get_pixel(0, 0), &image::Rgba([10, 10, 0, 255]));
    }

    #[test]
    fn crop_and_render_stamps_annotations_at_selection_origin() {
        let base = gradient(100, 50);
        let mut shape = Shape::begin(ToolKind::FilledRectangle, pos2(20.0, 20.0)).unwrap();
        shape.update(pos2(40.0, 40.0), false, None);
        let annotations = vec![Annotation {
            shape,
            color: Color32::RED,
            width: 1.0,
        }];
        let crop =
            crop_and_render(&base, &annotations, Rect::from_min_max(pos2(10.0, 10.0), pos2(60.0, 50.0)))
                .unwrap();
        // Shape starts at (20,20) - crop origin (10,10) = (10,10) in the crop.
        assert_eq!(crop.get_pixel(15, 15), &image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn crop_and_render_rejects_degenerate_selection() {
        let base = gradient(100, 50);
        assert!(crop_and_render(&base, &[], Rect::NOTHING).is_none());
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn point_gesture_is_a_click_within_the_threshold() {
        assert!(is_click_gesture(Rect::from_two_pos(
            pos2(10.0, 10.0),
            pos2(13.0, 12.0)
        )));
        assert!(is_click_gesture(Rect::from_two_pos(
            pos2(10.0, 10.0),
            pos2(10.0, 10.0)
        )));
    }

    #[test]
    fn larger_gesture_is_a_drag() {
        assert!(!is_click_gesture(Rect::from_two_pos(
            pos2(10.0, 10.0),
            pos2(19.0, 10.0)
        )));
        assert!(!is_click_gesture(Rect::from_two_pos(
            pos2(10.0, 10.0),
            pos2(10.0, 40.0)
        )));
    }

    #[test]
    fn text_border_unions_regions_inside_the_drag() {
        let regions = [rect(10.0, 20.0, 50.0, 40.0), rect(60.0, 20.0, 90.0, 40.0)];
        let drag = rect(0.0, 0.0, 100.0, 100.0);
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
    fn text_border_only_includes_intersecting_regions() {
        let regions = [rect(10.0, 10.0, 40.0, 30.0), rect(150.0, 150.0, 190.0, 180.0)];
        let drag = rect(0.0, 0.0, 50.0, 50.0);
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
    fn text_border_none_without_intersection() {
        let regions = [rect(150.0, 150.0, 190.0, 180.0)];
        let drag = rect(0.0, 0.0, 50.0, 50.0);
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
    fn text_border_padding_is_clamped_to_the_image() {
        let regions = [rect(0.0, 0.0, 5.0, 5.0)];
        let drag = rect(0.0, 0.0, 10.0, 10.0);
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
    fn text_border_is_clamped_to_the_selection() {
        // Text extends past the selection on every side; the border is cropped
        // to the selection instead of spilling outside it.
        let regions = [rect(0.0, 0.0, 100.0, 100.0)];
        let drag = rect(0.0, 0.0, 200.0, 200.0);
        let selection = rect(20.0, 30.0, 80.0, 90.0);
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
    fn text_border_empty_intersection_with_selection_is_none() {
        let regions = [rect(0.0, 0.0, 10.0, 10.0)];
        let drag = rect(0.0, 0.0, 200.0, 200.0);
        let selection = rect(100.0, 100.0, 150.0, 150.0);
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
    fn text_border_redact_pads_one_pixel_top_and_bottom_only() {
        let regions = [rect(10.0, 20.0, 90.0, 40.0)];
        let drag = rect(0.0, 0.0, 100.0, 100.0);
        let redact = text_border_rect(
            drag,
            &regions,
            Vec2::new(200.0, 200.0),
            Vec2::new(0.0, REDACT_VERTICAL_PADDING),
            None,
        )
        .unwrap();
        assert_eq!(redact, rect(10.0, 19.0, 90.0, 41.0));
    }

    fn pos2(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }
}
