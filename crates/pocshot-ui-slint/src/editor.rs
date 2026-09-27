//! Editor state and input handling. All of this is toolkit-independent apart
//! from pushing properties/images into the Slint view, so the interesting logic
//! is testable.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use image::{imageops, RgbaImage};
use pocshot_annotate::{effects, Color32, Pos2, Rect, Shape, ToolKind, Vec2};
use pocshot_ocr::TextRegion;
use pocshot_snap::SnapLines;
use slint::{ComponentHandle as _, Image, Rgba8Pixel, SharedPixelBuffer};

use crate::platform;
use crate::EditorWindow;

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
    counter: u32,
    /// True while a text annotation is being typed (key events go to the buffer).
    typing_text: bool,
    /// Undo snapshots (base + annotations), newest last.
    history: Vec<HistoryEntry>,
    /// In-progress blur/pixelate region (image pixels).
    effect_anchor: Pos2,
    effect_rect: Option<Rect>,
    snap_lines: SnapLines,
    snap_enabled: bool,
    ocr_rx: Option<mpsc::Receiver<Vec<TextRegion>>>,
    ocr_regions: Vec<TextRegion>,
    /// Image-space origin of the crop the OCR regions are relative to.
    ocr_origin: Pos2,
    show_ocr: bool,
    /// Keeps the OCR poll timer alive.
    _timer: slint::Timer,
}

const TEXT_SIZE: f32 = 24.0;

/// Toolbar order; index ↔ `ToolKind`.
const TOOL_ORDER: [ToolKind; 15] = [
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
];

impl Editor {
    pub fn new(ui: EditorWindow, base: RgbaImage, snap_lines: SnapLines) -> Rc<RefCell<Self>> {
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
            color: PALETTE[0],
            width: 3.0,
            selection: Rect::from_min_size(Pos2::ZERO, size),
            select_anchor: Pos2::ZERO,
            selecting: false,
            moving_selection: false,
            move_anchor: Pos2::ZERO,
            move_origin: Rect::NOTHING,
            counter: 1,
            typing_text: false,
            history: Vec::new(),
            effect_anchor: Pos2::ZERO,
            effect_rect: None,
            snap_lines,
            snap_enabled: true,
            ocr_rx: None,
            ocr_regions: Vec::new(),
            ocr_origin: Pos2::ZERO,
            show_ocr: false,
            _timer: slint::Timer::default(),
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
                    if let Some(color) = PALETTE.get(index.max(0) as usize) {
                        editor.color = *color;
                        editor.refresh_hint();
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
                std::time::Duration::from_millis(250),
                move || {
                    if let Some(editor) = weak.upgrade() {
                        editor.borrow_mut().poll_ocr();
                    }
                },
            );
            editor.borrow_mut()._timer = timer;
        }

        editor.borrow_mut().rebuild_composited();
        editor.borrow().refresh_selection();
        editor.borrow().refresh_preview();
        editor.borrow().refresh_hint();
        editor
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
        if self.pointer_in_toolbar(ny) {
            return;
        }
        let (ctrl, shift) = platform::query_modifiers();
        let raw = self.norm_to_image(nx, ny);
        self.refresh_layout();
        // Snap to image/window edges unless Shift is held.
        let pos = if self.snap_enabled && !shift {
            Pos2::new(
                crate::snap::snap_pos(
                    raw.x,
                    &self.snap_lines.vertical,
                    crate::snap::SNAP_DISTANCE,
                ),
                crate::snap::snap_pos(
                    raw.y,
                    &self.snap_lines.horizontal,
                    crate::snap::SNAP_DISTANCE,
                ),
            )
        } else {
            raw
        };

        match phase {
            0 => {
                self.locked_angle = None;
                if self.tool == ToolKind::Select {
                    // A full-image selection counts as "no selection": dragging
                    // anywhere starts a new one. Otherwise dragging inside the
                    // existing selection moves it.
                    if self.selection.contains(pos) && !self.selection_is_full() {
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
                } else if self.tool == ToolKind::Text {
                    self.current = Some(Shape::text(pos, "", TEXT_SIZE));
                    self.typing_text = true;
                } else if self.tool == ToolKind::Blur || self.tool == ToolKind::Pixelate {
                    self.effect_anchor = pos;
                    self.effect_rect = Some(Rect::from_two_pos(pos, pos));
                } else if let Some(shape) = Shape::begin(self.tool, pos) {
                    self.current = Some(shape);
                }
            }
            1 => {
                if self.moving_selection {
                    let delta = pos - self.move_anchor;
                    self.selection = translate_clamped(self.move_origin, delta, self.image_rect());
                } else if self.selecting {
                    self.selection = Rect::from_two_pos(self.select_anchor, pos);
                } else if self.effect_rect.is_some() {
                    self.effect_rect = Some(Rect::from_two_pos(self.effect_anchor, pos));
                } else if let Some(shape) = self.current.as_mut() {
                    self.locked_angle = shape.update(pos, ctrl, self.locked_angle);
                }
            }
            2 => {
                if self.moving_selection {
                    self.moving_selection = false;
                } else if self.selecting {
                    self.selecting = false;
                    self.selection = self.selection.intersect(self.image_rect());
                } else if let Some(rect) = self.effect_rect.take() {
                    self.apply_effect(rect);
                } else if self.tool == ToolKind::Text {
                    // Text stays editable until Enter/Esc; mouse-up commits nothing.
                } else if let Some(shape) = self.current.take() {
                    if shape.kind() == ToolKind::Counter {
                        self.counter += 1;
                    }
                    self.history.push(self.snapshot());
                    self.annotations.push(Annotation {
                        shape,
                        color: self.color,
                        width: self.width,
                    });
                    self.rebuild_composited();
                }
            }
            _ => {}
        }

        self.refresh_selection();
        self.refresh_preview();
        self.refresh_hint();
    }

    fn on_action(&mut self, action: &str) {
        match action {
            "width-" => {
                self.width = (self.width - 1.0).max(1.0);
                self.refresh_hint();
            }
            "width+" => {
                self.width = (self.width + 1.0).min(24.0);
                self.refresh_hint();
            }
            "undo" => self.undo(),
            "ocr" => self.toggle_ocr(),
            "copy" => self.copy_and_quit(),
            "save" => self.save_and_quit(),
            "quit" => crate::quit_event_loop(),
            _ => {}
        }
    }

    /// True when the normalised `ny` falls inside the toolbar band, where
    /// pointer events belong to buttons rather than the canvas.
    fn pointer_in_toolbar(&self, ny: f32) -> bool {
        let win = self.window_size();
        win.y > 0.0 && ny * win.y > win.y - 76.0
    }

    fn on_key(&mut self, text: &str) {
        if self.typing_text {
            self.on_text_key(text);
            return;
        }

        let (ctrl, shift) = platform::query_modifiers();
        let lower = text.to_ascii_lowercase();

        if text == "q" || text == "\u{1b}" {
            crate::quit_event_loop();
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
            self.undo();
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
            "n" => self.set_tool(ToolKind::Counter),
            "t" => self.set_tool(ToolKind::Text),
            "b" => self.set_tool(ToolKind::Blur),
            "m" => self.set_tool(ToolKind::Pixelate),
            "e" => self.set_tool(ToolKind::Eraser),
            "o" => self.toggle_ocr(),
            "[" => {
                self.width = (self.width - 1.0).max(1.0);
                self.refresh_hint();
            }
            "]" => {
                self.width = (self.width + 1.0).min(24.0);
                self.refresh_hint();
            }
            digit if digit.len() == 1 => {
                if let Some(index) = digit.chars().next().and_then(|c| c.to_digit(10)) {
                    if (1..=6).contains(&index) {
                        self.color = PALETTE[index as usize - 1];
                        self.refresh_hint();
                    }
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
                        self.history.push(self.snapshot());
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
        self.refresh_hint();
    }

    fn set_tool(&mut self, tool: ToolKind) {
        self.tool = tool;
        self.refresh_hint();
    }

    /// Repaint the committed overlay (base + annotations) and publish it.
    fn rebuild_composited(&mut self) {
        self.composited = (*self.base).clone();
        render_annotations(&mut self.composited, &self.annotations, Pos2::ZERO);
        if self.show_ocr {
            let color = Color32::from_rgb(0, 230, 255);
            let origin = self.ocr_origin;
            for region in &self.ocr_regions {
                let r = region.rect;
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
            self.rebuild_composited();
            return;
        }
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
    fn poll_ocr(&mut self) {
        let Some(rx) = &self.ocr_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(regions) => {
                self.ocr_rx = None;
                for region in &regions {
                    log::info!(
                        "ocr: {:.2} {:?}",
                        region.confidence,
                        region.text.as_deref().unwrap_or("")
                    );
                }
                self.ocr_regions = regions;
                self.show_ocr = true;
                self.rebuild_composited();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ocr_rx = None;
            }
        }
    }

    /// Remove the topmost committed annotation under `pos` (eraser).
    fn erase_at(&mut self, pos: Pos2) {
        let hit = self
            .annotations
            .iter()
            .rposition(|annotation| annotation.shape.hit_test(pos, 6.0));
        if let Some(index) = hit {
            self.history.push(self.snapshot());
            self.annotations.remove(index);
            self.rebuild_composited();
        }
    }

    /// Restore the previous base/annotations snapshot.
    fn undo(&mut self) {
        if let Some(entry) = self.history.pop() {
            self.base = entry.base;
            self.annotations = entry.annotations;
            self.rebuild_composited();
            self.refresh_preview();
        }
    }

    /// Apply the current destructive tool to `rect` (image pixels).
    fn apply_effect(&mut self, rect: Rect) {
        let rect = rect.intersect(self.image_rect());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let tool = self.tool;
        self.history.push(self.snapshot());
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
    }

    fn refresh_hint(&self) {
        self.ui
            .set_active_tool(self.tool_index());
        self.ui
            .set_active_color(PALETTE.iter().position(|c| *c == self.color).unwrap_or(0) as i32);
        if self.typing_text {
            self.ui.set_hint(
                "Type text · Enter commit · Esc cancel · Backspace delete".into(),
            );
            return;
        }
        self.ui.set_hint(
            format!(
                "{:?} · color {} · width {:.0} · R rect · Shift+R fill · L line · A arrow · C circle · P pen · H highlight · D redact · N counter · T text · B blur · M pixelate · E eraser · V select · O OCR · Ctrl+Z undo · Enter copy · Ctrl+S save · Esc quit",
                self.tool,
                self.color_index(),
                self.width
            )
            .into(),
        );
    }

    fn color_index(&self) -> usize {
        PALETTE
            .iter()
            .position(|c| *c == self.color)
            .map(|i| i + 1)
            .unwrap_or(0)
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
        crate::quit_event_loop();
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

    fn pos2(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }
}
