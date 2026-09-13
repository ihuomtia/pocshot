mod effects;
mod export;
mod ocr;
mod overlay;
mod selection_ui;
mod settings;
mod text_border;

use eframe::egui::{
    self, pos2, vec2, Color32, ColorImage, CursorIcon, Id, Key, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use image::{imageops, RgbaImage};
use pocshot_ocr::detect::TextDetector;
use pocshot_ocr::snap::text_regions_to_guides;
use pocshot_ocr::TextRegion;
use pocshot_snap::{detect_snap_lines, SnapConfig, SnapLines};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use crate::annotation::{AnnotationState, AnnotationTool};
use crate::canvas::{capture_size, clamp_image_rect, fit_rect, screen_to_image};
use crate::config;
use crate::selection::{handle_rects, HandleType, ImageSelection};
use crate::snap::{snap_pos, SNAP_DISTANCE};
use crate::toolbar::{show_settings_panel, show_toolbar, Action, ToolbarState};

/// Result of a background screen capture: the image plus its monitor geometry
/// (needed to map global window coordinates into image coordinates).
type CaptureResult = Result<(RgbaImage, pocshot_core::MonitorInfo), String>;

pub fn run() -> eframe::Result<()> {
    crate::logging::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pocshot")
            .with_fullscreen(true)
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Pocshot",
        options,
        Box::new(|cc| Ok(Box::new(PocshotApp::new(cc)))),
    )
}

pub fn run_pin(image_path: PathBuf, x: i32, y: i32, width: u32, height: u32) -> eframe::Result<()> {
    crate::logging::init();
    let size = vec2(width.max(1) as f32, height.max(1) as f32);
    let pos = pos2(x as f32, y as f32);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pocshot pin")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_inner_size(size)
            .with_position(pos),
        ..Default::default()
    };

    eframe::run_native(
        "Pocshot pin",
        options,
        Box::new(move |cc| Ok(Box::new(PinApp::new(cc, image_path, size, pos)))),
    )
}

// One eframe/wgpu (GL) process per pin. Fine for a handful (<= ~10-15);
// revisit a lighter renderer if 30+ simultaneous pins are ever needed.
pub struct PinApp {
    texture: Option<TextureHandle>,
    display_size: Vec2,
    expected_pos: Pos2,
    placement_checked: bool,
}

impl PinApp {
    fn new(cc: &eframe::CreationContext<'_>, image_path: PathBuf, size: Vec2, pos: Pos2) -> Self {
        let mut app = Self {
            texture: None,
            display_size: size,
            expected_pos: pos,
            placement_checked: false,
        };

        if let Ok(image) = image::open(&image_path) {
            let rgba = image.to_rgba8();
            let dims = [rgba.width() as usize, rgba.height() as usize];
            let color_image = ColorImage::from_rgba_unmultiplied(dims, rgba.as_raw());
            app.texture = Some(cc.egui_ctx.load_texture(
                "pin",
                color_image,
                TextureOptions::LINEAR,
            ));
            let _ = std::fs::remove_file(&image_path);
        }
        app
    }
}

impl eframe::App for PinApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.placement_checked {
            self.placement_checked = true;
            let actual = ctx.input(|i| i.viewport().outer_rect.map(|r| r.min));
            match actual {
                Some(actual) if (actual - self.expected_pos).length() > 4.0 => {
                    log::info!(
                        "[pocshot pin] position hint ignored ({:?} vs {actual:?}); relying on StartDrag",
                        self.expected_pos
                    );
                }
                Some(_) => {}
                None => self.placement_checked = false,
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (rect, resp) =
                    ui.allocate_exact_size(self.display_size, Sense::click_and_drag());
                if let Some(tex) = &self.texture {
                    ui.painter().image(
                        tex.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if resp.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if resp.secondary_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

struct PocshotApp {
    capture: Option<RgbaImage>,
    /// In-flight screen capture off the UI thread. Capturing (especially the
    /// first DXGI/X11 call on Windows) can take seconds; running it in the
    /// background lets the window paint immediately.
    capture_rx: Option<Receiver<CaptureResult>>,
    texture: Option<TextureHandle>,
    selection: Option<ImageSelection>,
    drag_start: Option<Pos2>,
    dragging_handle: Option<HandleType>,
    snap_lines: Option<SnapLines>,
    snap_lines_rx: Option<Receiver<SnapLines>>,
    annotations: AnnotationState,
    status: String,
    clipboard: bool,
    snap_enabled: bool,
    show_settings: bool,
    show_help: bool,
    show_snap_lines: bool,
    // Image-effect (pixelate/blur) drag gesture, in *image* coordinates.
    effect_start: Option<Pos2>,
    effect_current: Option<Pos2>,
    // In-flight pixelate/blur job off the UI thread; the result image arrives
    // here so a large region doesn't block the whole app while being processed.
    effect_rx: Option<Receiver<RgbaImage>>,
    // Undo/redo stacks for destructive pixelate/blur edits on the capture.
    capture_undo: Vec<RgbaImage>,
    capture_redo: Vec<RgbaImage>,
    // OCR / text detection (optional, off by default).
    ocr_enabled: bool,
    show_text_boxes: bool,
    show_ocr_debug: bool,
    ocr_region_only: bool,
    ocr_confidence: f32,
    ocr_models_dir: String,
    ocr_detector: Option<Arc<dyn TextDetector>>,
    ocr_raw_regions: Vec<TextRegion>,
    ocr_regions: Vec<TextRegion>,
    ocr_rx: Option<Receiver<(u64, Result<Vec<TextRegion>, String>, Option<Rect>)>>,
    ocr_running: bool,
    /// When the "OCR this region" button triggered the in-flight job, copy the
    /// recognized text to the clipboard once detection completes.
    ocr_copy_on_done: bool,
    ocr_ready_version: u64,
    /// The selection rect (image coords) that the ready OCR results correspond
    /// to, when "detect text only in selection" was used; `None` = full image.
    ocr_ready_region: Option<Rect>,
    capture_version: u64,
    /// Active visual theme (colors, font sizes, geometry). Loaded from config
    /// at startup and swapped live on "Reload theme".
    theme: crate::theme::Theme,
}

impl PocshotApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        // DPI normalization is applied in `update` (native_pixels_per_point is
        // only populated once frames begin on the desktop backend).
        cc.egui_ctx.set_fonts(fonts);

        let settings = config::load();

        let mut app = Self {
            capture: None,
            capture_rx: None,
            texture: None,
            selection: None,
            drag_start: None,
            dragging_handle: None,
            snap_lines: None,
            snap_lines_rx: None,
            annotations: AnnotationState::default(),
            status: String::new(),
            clipboard: false,
            snap_enabled: settings.snap_enabled,
            show_settings: false,
            show_help: true,
            show_snap_lines: settings.show_snap_lines,
            effect_start: None,
            effect_current: None,
            effect_rx: None,
            capture_undo: Vec::new(),
            capture_redo: Vec::new(),
            ocr_enabled: settings.ocr_enabled,
            show_text_boxes: settings.show_text_boxes,
            show_ocr_debug: settings.show_ocr_debug,
            ocr_region_only: settings.ocr_region_only,
            ocr_confidence: settings.ocr_confidence,
            ocr_models_dir: settings.ocr_models_dir,
            ocr_detector: None,
            ocr_raw_regions: Vec::new(),
            ocr_regions: Vec::new(),
            ocr_rx: None,
            ocr_running: false,
            ocr_copy_on_done: false,
            ocr_ready_version: 0,
            ocr_ready_region: None,
            capture_version: 0,
            theme: settings.theme,
        };
        app.begin_capture();
        app
    }

    /// Normalize the effective scale so the fixed-size UI (toolbar, overlay,
    /// text/icons) occupies the same logical space on every display. egui
    /// renders points at `pixels_per_point = zoom * native_pixels_per_point`;
    /// on a high-DPI panel (e.g. the 200% laptop) autoscaling inflates the UI
    /// so it overflows and fonts/icons look too large, while a low-DPI 1080p
    /// desktop at ppp≈1.0 looks right. We set the zoom factor so the net
    /// pixels-per-point matches the low-DPI desktop, making high-DPI screens
    /// render identically.
    fn normalize_dpi(&self, ctx: &egui::Context) {
        if let Some(native) = ctx.native_pixels_per_point() {
            // Target net pixels-per-point (matches a ~100% 1080p desktop).
            const TARGET_PPP: f32 = 1.0;
            if native > TARGET_PPP {
                ctx.set_zoom_factor(TARGET_PPP / native);
            }
        }
    }

    /// Kick off a fresh screen capture on a background thread. The result is
    /// picked up by [`Self::poll_capture`] on a later frame, so the window
    /// shows immediately instead of blocking the first frame on the (slow on
    /// Windows) backend initialization.
    fn begin_capture(&mut self) {
        if self.capture_rx.is_some() {
            return;
        }
        self.status = "Capturing screen…".to_string();
        let (tx, rx) = mpsc::channel();
        self.capture_rx = Some(rx);
        std::thread::spawn(move || {
            let result = pocshot_core::capture_screen_with_monitor().map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    /// Deliver a completed background capture. Builds the texture, resets the
    /// per-capture interaction state and launches the snap (window + edge) and
    /// OCR workers.
    fn poll_capture(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.capture_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok((image, monitor))) => {
                self.capture_rx = None;
                self.on_capture_ready(image, monitor, ctx);
            }
            Ok(Err(error)) => {
                self.capture_rx = None;
                self.status = error;
                self.capture = None;
                self.texture = None;
            }
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.capture_rx = None;
            }
        }
    }

    fn on_capture_ready(
        &mut self,
        image: RgbaImage,
        monitor: pocshot_core::MonitorInfo,
        ctx: &egui::Context,
    ) {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.texture =
            Some(ctx.load_texture("screen-capture", color_image, TextureOptions::LINEAR));
        self.capture = Some(image.clone());
        self.capture_undo.clear();
        self.capture_redo.clear();
        self.effect_start = None;
        self.effect_current = None;
        self.effect_rx = None;
        self.selection = None;
        self.drag_start = None;
        self.dragging_handle = None;
        self.show_settings = false;
        self.status.clear();
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        self.snap_lines = Some(pocshot_snap::SnapLines {
            horizontal: vec![0.0, ih],
            vertical: vec![0.0, iw],
        });

        let (tx, rx) = mpsc::channel();
        self.snap_lines_rx = Some(rx);

        let img_for_thread = image.clone();
        let (origin_x, origin_y) = (monitor.x, monitor.y);
        let own_pid = std::process::id();
        std::thread::spawn(move || {
            let mut result = detect_snap_lines(&img_for_thread, &SnapConfig::default());

            // OS-level outer regions: each top-level window that intersects the
            // captured monitor contributes its four frame edges. The Sobel pass
            // above covers the panels *inside* those windows.
            match pocshot_core::list_windows() {
                Ok(windows) => {
                    let window_lines = crate::window_snap::window_snap_lines(
                        &windows, origin_x, origin_y, iw as u32, ih as u32, own_pid,
                    );
                    result.horizontal.extend(window_lines.horizontal);
                    result.vertical.extend(window_lines.vertical);
                }
                Err(e) => log::warn!("window snap detection unavailable: {e:#}"),
            }

            result.horizontal.extend_from_slice(&[0.0, ih]);
            result.vertical.extend_from_slice(&[0.0, iw]);
            result.horizontal.sort_by(|a, b| a.partial_cmp(b).unwrap());
            result.vertical.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let _ = tx.send(result);
        });

        self.capture_version += 1;
        // Keep the last valid detection result visible while the new
        // capture's detection runs; poll_ocr swaps it when ready.
        if config::should_run_detection(self.ocr_enabled, true) {
            if self.run_ocr_detection(image.clone()) {
                log::info!(
                    "captured {}x{}, text detection running in background",
                    image.width(),
                    image.height(),
                );
            }
        }

        log::info!(
            "captured {}x{}, snap detection running in background",
            image.width(),
            image.height(),
        );
    }

    /// Lazily load the ONNX detector once and keep it for reuse. On failure we
    /// log, disable detection gracefully and leave the rest of the app usable.
    fn apply_capture(&mut self, image: RgbaImage, ctx: &egui::Context) {
        self.capture = Some(image.clone());
        self.apply_capture_texture(ctx);
    }

    /// Re-upload the current capture as the on-screen texture without touching
    /// the capture pixels (used right after mutating the capture in place).
    fn apply_capture_texture(&mut self, ctx: &egui::Context) {
        let Some(image) = &self.capture else {
            return;
        };
        let size = [image.width() as usize, image.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.texture =
            Some(ctx.load_texture("screen-capture", color_image, TextureOptions::LINEAR));
    }

    /// Handle the drag gesture for the Pixelate / Blur tools: record the
    /// region while dragging and apply the effect on release, with an undo
    /// snapshot of the previous capture.
    fn selected_image(&self) -> Option<RgbaImage> {
        let capture = self.capture.as_ref()?;
        let rect = self
            .selection
            .map(ImageSelection::rect)
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, capture_size(capture)));
        let crop = clamp_image_rect(rect, capture)?;

        Some(
            imageops::crop_imm(
                capture,
                crop.min.x as u32,
                crop.min.y as u32,
                crop.width() as u32,
                crop.height() as u32,
            )
            .to_image(),
        )
    }

    fn selection_offset(&self) -> Pos2 {
        self.selection
            .map(|s| s.rect())
            .map(|r| Pos2::new(r.min.x.min(r.max.x), r.min.y.min(r.max.y)))
            .unwrap_or(Pos2::ZERO)
    }

    fn shortcuts(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // While a text annotation is being typed, the editor owns the
        // keyboard: don't interpret any key (Esc, Cmd+C, bare letters, …) as
        // an app shortcut.
        if self.annotations.is_text_editing() {
            return;
        }
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            if self.show_settings {
                self.show_settings = false;
            } else if self.annotations.active_tool != AnnotationTool::Select {
                self.annotations.active_tool = AnnotationTool::Select;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input(|input| input.key_pressed(Key::S) && input.modifiers.command) {
            self.save_selection();
        }
        // Copy: Ctrl+C (Linux/Win) or Cmd+C (Mac). Handle both the raw key
        // press (nothing focused) and the Copy/Cut event (text field focused,
        // where egui suppresses the key press).
        let copy_key =
            ctx.input(|i| i.key_pressed(Key::C) && (i.modifiers.command || i.modifiers.ctrl));
        let copy_event = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Copy | egui::Event::Cut))
        });
        if copy_key || copy_event {
            self.copy_selection(ctx);
        }
        if ctx.input(|input| input.key_pressed(Key::R) && input.modifiers.command) {
            self.begin_capture();
        }
        let undo_pressed = ctx.input(|input| input.key_pressed(Key::Z) && input.modifiers.command);
        let shift = ctx.input(|input| input.modifiers.shift);
        if undo_pressed {
            if shift {
                self.annotations.redo();
            } else {
                self.annotations.undo();
            }
        }

        // Tool shortcuts (bare keys). Skip when a text field (e.g. the hex
        // input in Settings) has focus, and skip when a command/ctrl/alt
        // modifier is held so we don't steal Cmd+S / Ctrl+C / Cmd+R, etc.
        if ctx.memory(|m| m.focused().is_none())
            && ctx.input(|i| !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt)
        {
            let shift_down = ctx.input(|i| i.modifiers.shift);
            let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
            let tool = if pressed(Key::V) {
                Some(AnnotationTool::Select)
            } else if pressed(Key::A) {
                Some(AnnotationTool::Arrow)
            } else if pressed(Key::L) {
                Some(AnnotationTool::Line)
            } else if pressed(Key::R) {
                if shift_down {
                    Some(AnnotationTool::FilledRectangle)
                } else {
                    Some(AnnotationTool::Rectangle)
                }
            } else if pressed(Key::C) {
                if shift_down {
                    Some(AnnotationTool::FilledCircle)
                } else {
                    Some(AnnotationTool::Circle)
                }
            } else if pressed(Key::P) {
                Some(AnnotationTool::Pen)
            } else if pressed(Key::H) {
                Some(AnnotationTool::Highlighter)
            } else if pressed(Key::N) {
                Some(AnnotationTool::Counter)
            } else if pressed(Key::T) {
                Some(AnnotationTool::HighlightText)
            } else if pressed(Key::M) {
                Some(AnnotationTool::Pixelate)
            } else if pressed(Key::B) {
                Some(AnnotationTool::Blur)
            } else if pressed(Key::E) {
                Some(AnnotationTool::Eraser)
            } else {
                None
            };
            if let Some(t) = tool {
                if t != self.annotations.active_tool {
                    self.reset_effect();
                }
                self.annotations.active_tool = t;
            }
        }
    }

    fn combined_snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        let (mut horiz, mut vert) = match &self.snap_lines {
            Some(sl) => (sl.horizontal.clone(), sl.vertical.clone()),
            None => (Vec::new(), Vec::new()),
        };
        let (anno_h, anno_v) = self.annotations.snap_lines();
        horiz.extend(anno_h);
        vert.extend(anno_v);
        if self.ocr_enabled && !self.ocr_regions.is_empty() {
            if let Some(cap) = &self.capture {
                let guides = text_regions_to_guides(
                    &self.ocr_regions,
                    cap.width() as f32,
                    cap.height() as f32,
                );
                horiz.extend(guides.horizontal);
                vert.extend(guides.vertical);
            }
        }
        (horiz, vert)
    }

    /// Render the interactive `TextEdit` for a text annotation being typed,
    /// positioned at the anchor's screen location. Enter commits, Esc cancels,
    /// and Shift+Enter inserts a newline (the multiline editor's default
    /// return key).
    fn draw_text_editor(&mut self, ui: &egui::Ui, draw_rect: Rect, image_size: Vec2) {
        let to_screen = |p: Pos2| -> Pos2 {
            pos2(
                draw_rect.min.x + p.x / image_size.x * draw_rect.width(),
                draw_rect.min.y + p.y / image_size.y * draw_rect.height(),
            )
        };

        // Handle resize-drags on the box handles first (mutates the editing
        // rect in image coordinates) so this frame's drags apply.
        self.process_text_box_handles(ui, &to_screen, draw_rect, image_size);

        let Some(edit) = self.annotations.text_box_view() else {
            return;
        };
        let screen_rect = Rect::from_two_pos(to_screen(edit.rect.min), to_screen(edit.rect.max));

        // Transparent input: no box frame — the text itself shows in the
        // current annotation color. Resize handles remain as the affordance.
        let text_color = self.annotations.color;

        // Intercept Enter / Esc BEFORE the TextEdit processes them, so plain
        // Enter never inserts a newline and Esc is handled exactly once here
        // (egui#5609: don't double-handle Esc between the widget and us).
        let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

        let pad = 5.0;
        egui::Area::new(edit.id)
            .fixed_pos(screen_rect.min + vec2(pad, pad))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let Some(buf) = self.annotations.text_buffer_mut() else {
                    return;
                };
                let (w, h) = (
                    (screen_rect.width() - pad * 2.0).max(40.0),
                    (screen_rect.height() - pad * 2.0).max(20.0),
                );
                let rows = ((h / (edit.size * 1.5)).round() as usize).max(1);
                let response = ui.add(
                    // Shift+Enter inserts a literal newline (plain Enter was
                    // already consumed above → submits). Transparent background,
                    // text in the selected annotation color.
                    egui::TextEdit::multiline(buf)
                        .font(egui::FontId::proportional(edit.size))
                        .desired_width(w)
                        .desired_rows(rows)
                        .text_color(text_color)
                        .background_color(Color32::TRANSPARENT)
                        .return_key(Some(egui::KeyboardShortcut::new(
                            egui::Modifiers::SHIFT,
                            egui::Key::Enter,
                        ))),
                );
                // Request focus exactly once, on the frame the box was created
                // (egui#5187: re-requesting focus every frame breaks input).
                if edit.just_created {
                    response.request_focus();
                    self.annotations.clear_just_created();
                }
            });

        if enter {
            // Submit; keep the text tool selected for the next box.
            self.annotations.commit_text();
        } else if escape {
            // Esc cancels the *tool*: submit whatever text exists (non-empty
            // commits, empty discards) and deselect the text tool.
            self.annotations.commit_text();
            self.annotations.active_tool = AnnotationTool::Select;
        }

        // Resize handles on top of the box.
        self.draw_text_box_handles(ui, screen_rect);
    }

    /// Drag-resize the editing text box via its corner/edge handles. `to_screen`
    /// maps image → screen; pointer positions are mapped back to image space.
    fn process_text_box_handles(
        &mut self,
        ui: &egui::Ui,
        to_screen: &dyn Fn(Pos2) -> Pos2,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        let Some(r) = self.annotations.editing_rect() else {
            return;
        };
        let screen_rect = Rect::from_two_pos(to_screen(r.min), to_screen(r.max));
        for (ht, hr) in handle_rects(screen_rect) {
            let resp = ui.interact(hr, Id::new(("text-handle", ht as u8)), Sense::drag());
            if resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    self.annotations.resize_editing_rect(|edit| {
                        let mut ip = screen_to_image(pos, draw_rect, image_size);
                        ip.x = ip.x.clamp(0.0, image_size.x);
                        ip.y = ip.y.clamp(0.0, image_size.y);
                        match ht {
                            HandleType::TopLeft => {
                                edit.min.x = ip.x.min(edit.max.x - 30.0);
                                edit.min.y = ip.y.min(edit.max.y - 20.0);
                            }
                            HandleType::TopRight => {
                                edit.max.x = ip.x.max(edit.min.x + 30.0);
                                edit.min.y = ip.y.min(edit.max.y - 20.0);
                            }
                            HandleType::BottomLeft => {
                                edit.min.x = ip.x.min(edit.max.x - 30.0);
                                edit.max.y = ip.y.max(edit.min.y + 20.0);
                            }
                            HandleType::BottomRight => {
                                edit.max.x = ip.x.max(edit.min.x + 30.0);
                                edit.max.y = ip.y.max(edit.min.y + 20.0);
                            }
                            HandleType::Top => {
                                edit.min.y = ip.y.min(edit.max.y - 20.0);
                            }
                            HandleType::Bottom => {
                                edit.max.y = ip.y.max(edit.min.y + 20.0);
                            }
                            HandleType::Left => {
                                edit.min.x = ip.x.min(edit.max.x - 30.0);
                            }
                            HandleType::Right => {
                                edit.max.x = ip.x.max(edit.min.x + 30.0);
                            }
                        }
                    });
                }
            }
        }
    }

    /// Draw the resize handles on the text box.
    fn draw_text_box_handles(&self, ui: &egui::Ui, screen_rect: Rect) {
        let painter = ui.painter();
        for (ht, hr) in handle_rects(screen_rect) {
            let is_hovered = ui
                .ctx()
                .pointer_latest_pos()
                .map(|p| hr.contains(p))
                .unwrap_or(false);
            let fill = if is_hovered {
                self.theme.colors.handle_active
            } else {
                self.theme.colors.handle_idle
            };
            painter.rect_filled(hr, 2.0, fill);
            painter.rect_stroke(
                hr,
                1.0,
                egui::Stroke::new(1.0_f32, self.theme.colors.text_primary),
                egui::StrokeKind::Inside,
            );
            let _ = ht;
        }
    }
}

impl eframe::App for PocshotApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.normalize_dpi(ctx);
        self.shortcuts(ctx, frame);

        self.poll_capture(ctx);
        if let Some(rx) = &self.snap_lines_rx {
            if let Ok(sl) = rx.try_recv() {
                log::info!(
                    "snap lines ready: {} h-snaps, {} v-snaps",
                    sl.horizontal.len(),
                    sl.vertical.len()
                );
                self.snap_lines = Some(sl);
                self.snap_lines_rx = None;
            } else {
                ctx.request_repaint();
            }
        }

        self.poll_ocr(ctx);
        self.poll_effect(ctx);
        self.maybe_ocr_rerun();

        let snap_ok = self.snap_enabled && !ctx.input(|i| i.modifiers.shift);
        let settings_before = self.to_settings();

        // Clone the theme so the painting closures below can borrow it freely
        // while `self` is borrowed mutably (same pattern as `status`).
        let theme = self.theme.clone();
        // Keep the annotation text size in sync with the themed default.
        self.annotations.text_size = theme.fonts.annotation_text;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme.colors.canvas_bg))
            .show(ctx, |ui| {
                let Some(texture) = self.texture.clone() else {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(theme.colors.text_primary, &self.status);
                    });
                    return;
                };

                let available = ui.available_rect_before_wrap();
                let image_size = texture.size_vec2();
                let draw_rect = fit_rect(image_size, available);

                let painter = ui.painter_at(draw_rect);
                painter.image(
                    texture.id(),
                    draw_rect,
                    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                let no_selection = self.selection.is_none() && self.drag_start.is_none();
                if no_selection {
                    painter.rect_filled(draw_rect, 0.0, theme.colors.overlay_dim);
                    painter.text(
                        draw_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Drag to select a region",
                        egui::FontId::proportional(theme.fonts.hint),
                        theme.colors.hint_text,
                    );
                }

                let (horiz_snaps, vert_snaps) = self.combined_snap_lines();
                let dragging_handle = self.dragging_handle;
                let is_select_mode = self.annotations.active_tool == AnnotationTool::Select;

                let is_selecting = self.drag_start.is_some() || self.dragging_handle.is_some();
                let selection_for_toolbar = self.selection;
                let status = self.status.clone();
                let ctx_clone = ui.ctx().clone();
                let can_undo = self.annotations.can_undo() || !self.capture_undo.is_empty();
                let can_redo = self.annotations.can_redo() || !self.capture_redo.is_empty();
                let mut pending_action: Option<Action> = None;
                let prev_tool = self.annotations.active_tool;
                let toolbar_rect = egui::Area::new(Id::new("toolbar-area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::ZERO)
                    .interactable(true)
                    .show(ui.ctx(), |ui| {
                        show_toolbar(
                            ToolbarState {
                                status: &status,
                                active_tool: &mut self.annotations.active_tool,
                                color: &mut self.annotations.color,
                                can_undo,
                                can_redo,
                                theme: &theme,
                            },
                            ui,
                            draw_rect,
                            image_size,
                            selection_for_toolbar,
                            is_selecting,
                            &mut |action| pending_action = Some(action),
                        )
                    })
                    .response
                    .rect;
                if self.annotations.active_tool != prev_tool {
                    self.reset_effect();
                    // Switching away from the Text tool while a box is being
                    // edited submits it (non-empty commits, empty discards).
                    if prev_tool == AnnotationTool::Text {
                        self.annotations.commit_text();
                    }
                }

                let mut help_rect = Rect::NOTHING;
                if self.show_help && no_selection {
                    let rows = [
                        ("Drag", "select region"),
                        ("Enter / Space", "capture"),
                        ("Esc", "cancel / close"),
                        ("Cmd+S", "save"),
                        ("Cmd+C", "copy"),
                        ("Cmd+R", "recapture"),
                        ("V", "Select"),
                        ("A", "Arrow"),
                        ("L", "Line"),
                        ("R", "Rect"),
                        ("C", "Circle"),
                        ("P", "Pen"),
                        ("H", "Highlight"),
                        ("N", "Number bubble"),
                        ("T", "Text border"),
                        ("M", "Pixelate"),
                        ("B", "Blur"),
                        ("E", "Eraser"),
                        ("Shift+R", "Filled rect"),
                        ("Shift+C", "Filled circle"),
                        ("Ctrl+drag", "constrain shape / angle"),
                    ];
                    // Draw the instructions centered on the screen, styled like
                    // the "Drag to select a region" hint, with a subtle backdrop.
                    let line_h = 20.0_f32;
                    let text_w = 260.0_f32;
                    let title = "Shortcuts";
                    let box_w = text_w + 60.0;
                    let box_h = 40.0 + rows.len() as f32 * line_h + 20.0;
                    let center = draw_rect.center();
                    let box_rect = Rect::from_center_size(center, vec2(box_w, box_h + line_h));
                    help_rect = box_rect;
                    painter.rect_filled(
                        box_rect,
                        theme.geometry.help_radius,
                        Color32::from_rgba_unmultiplied(10, 10, 14, 200),
                    );
                    painter.rect_stroke(
                        box_rect,
                        theme.geometry.help_radius,
                        Stroke::new(1.0_f32, theme.colors.box_border),
                        StrokeKind::Middle,
                    );
                    let mut y = box_rect.min.y + 16.0;
                    painter.text(
                        pos2(center.x, y),
                        egui::Align2::CENTER_TOP,
                        title,
                        egui::FontId::proportional(theme.fonts.help_title),
                        theme.colors.text_primary,
                    );
                    y += line_h + 6.0;
                    for (shortcut, desc) in rows {
                        painter.text(
                            pos2(box_rect.min.x + 30.0, y),
                            egui::Align2::LEFT_CENTER,
                            shortcut,
                            egui::FontId::proportional(theme.fonts.help_row),
                            theme.colors.text_primary,
                        );
                        painter.text(
                            pos2(box_rect.max.x - 30.0, y),
                            egui::Align2::RIGHT_CENTER,
                            desc,
                            egui::FontId::proportional(theme.fonts.help_row),
                            theme.colors.text_muted,
                        );
                        y += line_h;
                    }
                    painter.text(
                        pos2(center.x, box_rect.max.y - 14.0),
                        egui::Align2::CENTER_BOTTOM,
                        "Select a region to dismiss",
                        egui::FontId::proportional(theme.fonts.help_footer),
                        Color32::from_rgba_unmultiplied(180, 180, 180, 220),
                    );
                }

                let mut settings_rect = Rect::NOTHING;
                let mut reload_theme = false;
                let mut download_models = false;
                if self.show_settings {
                    let panel_w = 300.0_f32;
                    settings_rect = Rect::from_min_max(
                        pos2(available.min.x, available.min.y),
                        pos2(
                            (available.min.x + panel_w).min(available.max.x),
                            available.max.y,
                        ),
                    );
                    egui::Area::new(Id::new("settings-panel"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(settings_rect.min)
                        .interactable(true)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::NONE
                                .fill(theme.colors.settings_bg)
                                .corner_radius(theme.geometry.settings_radius)
                                .inner_margin(10.0)
                                .stroke(Stroke::new(1.0_f32, theme.colors.box_border))
                                .show(ui, |ui| {
                                    show_settings_panel(
                                        &theme,
                                        &mut self.clipboard,
                                        &mut self.annotations.color,
                                        &mut self.annotations.stroke_width,
                                        &mut self.show_settings,
                                        &mut self.show_snap_lines,
                                        &mut self.snap_enabled,
                                        &mut self.ocr_enabled,
                                        &mut self.show_text_boxes,
                                        &mut self.show_ocr_debug,
                                        &mut self.ocr_region_only,
                                        &mut self.ocr_confidence,
                                        &mut self.ocr_models_dir,
                                        &mut reload_theme,
                                        &mut download_models,
                                        &mut self.theme.fonts.annotation_text,
                                        ui,
                                        settings_rect,
                                    );
                                });
                        });
                }

                if reload_theme {
                    let loaded = config::load();
                    self.theme = loaded.theme;
                    self.status = "Theme reloaded".to_string();
                    ui.ctx().request_repaint();
                }

                if download_models {
                    let dir = crate::config::ocr_models_dir(&self.ocr_models_dir);
                    self.status = "Downloading OCR models…".to_string();
                    if let Err(e) = pocshot_ocr::models::ensure_models(&dir) {
                        self.status = format!("Model download failed: {e}");
                    } else {
                        self.status = "OCR models ready".to_string();
                        self.ocr_detector = None;
                        ui.ctx().request_repaint();
                    }
                }

                if let Some(action) = pending_action {
                    self.handle_action(&ctx_clone, draw_rect, image_size, action);
                }

                let pointer = ui.ctx().pointer_latest_pos();
                let pointer_over_toolbar = pointer
                    .map(|p| {
                        toolbar_rect.contains(p)
                            || help_rect.contains(p)
                            || settings_rect.contains(p)
                    })
                    .unwrap_or(false);

                if is_select_mode {
                    if !pointer_over_toolbar {
                        self.process_resize_handles(
                            ui,
                            &painter,
                            (horiz_snaps.clone(), vert_snaps.clone()),
                            dragging_handle,
                            draw_rect,
                            image_size,
                            snap_ok,
                        );
                        self.process_canvas_drag(
                            ui,
                            &painter,
                            (horiz_snaps.clone(), vert_snaps.clone()),
                            draw_rect,
                            image_size,
                            snap_ok,
                        );
                    }
                } else {
                    if !pointer_over_toolbar {
                        let canvas_response =
                            ui.interact(draw_rect, Id::new("annotation-canvas"), Sense::drag());

                        if self.annotations.active_tool.is_image_effect() {
                            canvas_response
                                .clone()
                                .on_hover_cursor(CursorIcon::Crosshair);
                            let ctx_now = ui.ctx().clone();
                            self.process_effect_drag(
                                &canvas_response,
                                &ctx_now,
                                (horiz_snaps.clone(), vert_snaps.clone()),
                                draw_rect,
                                image_size,
                                snap_ok,
                            );
                        } else if self.annotations.active_tool == AnnotationTool::HighlightText {
                            canvas_response
                                .clone()
                                .on_hover_cursor(CursorIcon::Crosshair);
                            self.process_text_border_drag(
                                &canvas_response,
                                (horiz_snaps.clone(), vert_snaps.clone()),
                                draw_rect,
                                image_size,
                                snap_ok,
                            );
                        } else {
                            if self.annotations.active_tool.is_draw_tool() {
                                canvas_response
                                    .clone()
                                    .on_hover_cursor(CursorIcon::Crosshair);
                            } else if self.annotations.active_tool == AnnotationTool::Eraser {
                                canvas_response
                                    .clone()
                                    .on_hover_cursor(CursorIcon::PointingHand);
                            } else if self.annotations.active_tool == AnnotationTool::Counter {
                                canvas_response
                                    .clone()
                                    .on_hover_cursor(CursorIcon::PointingHand);
                            } else if self.annotations.active_tool == AnnotationTool::Text {
                                canvas_response.clone().on_hover_cursor(CursorIcon::Text);
                            }

                            let dr = draw_rect;
                            let isz = image_size;
                            let h_snaps = horiz_snaps.clone();
                            let v_snaps = vert_snaps.clone();
                            let do_snap = snap_ok;
                            self.annotations.process_drag(
                                &canvas_response,
                                move |pos| screen_to_image(pos, dr, isz),
                                move |img_pos| {
                                    if do_snap {
                                        pos2(
                                            snap_pos(img_pos.x, &v_snaps, SNAP_DISTANCE),
                                            snap_pos(img_pos.y, &h_snaps, SNAP_DISTANCE),
                                        )
                                    } else {
                                        img_pos
                                    }
                                },
                            );
                        }
                    }
                }

                self.draw_selection_base(ui, &painter, &texture, draw_rect, image_size);
                if snap_ok {
                    self.draw_brief_snap_indicators(&painter, draw_rect, image_size);
                }
                self.draw_annotations(&painter, draw_rect, image_size);
                self.draw_counter_preview(&painter, draw_rect);
                if self.annotations.active_tool == AnnotationTool::HighlightText {
                    self.draw_text_border_preview(&painter, draw_rect, image_size);
                } else {
                    self.draw_effect_preview(&painter, draw_rect, image_size);
                }
                self.draw_ocr_overlay(&painter, draw_rect, image_size);
                self.draw_selection_top(ui, &painter, &texture, draw_rect, image_size);
                self.draw_status_line(&painter, draw_rect, &status);
                if self.annotations.is_text_editing() {
                    self.draw_text_editor(ui, draw_rect, image_size);
                }
            });

        if settings_before != self.to_settings() {
            self.persist_settings();
            // Models-dir changed: drop the loaded engine so the next detection
            // round re-reads from the new directory.
            if settings_before.ocr_models_dir != self.ocr_models_dir {
                self.ocr_detector = None;
                self.ocr_regions.clear();
                self.ocr_raw_regions.clear();
                self.ocr_ready_version = 0;
                if self.capture.is_some() {
                    let _ = self.run_ocr_detection(self.capture.clone().unwrap());
                }
            }
        }
    }
}
