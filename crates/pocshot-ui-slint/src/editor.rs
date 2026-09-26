//! Editor state and input handling. All of this is toolkit-independent apart
//! from pushing properties/images into the Slint view, so the interesting logic
//! is testable.

use std::cell::RefCell;
use std::rc::Rc;

use image::{imageops, RgbaImage};
use pocshot_annotate::{Color32, Pos2, Rect, Shape, ToolKind, Vec2};
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

pub struct Annotation {
    pub shape: Shape,
    pub color: Color32,
    pub width: f32,
}

pub struct Editor {
    ui: EditorWindow,
    base: Rc<RgbaImage>,
    annotations: Vec<Annotation>,
    current: Option<Shape>,
    locked_angle: Option<f32>,
    tool: ToolKind,
    color: Color32,
    width: f32,
    selection: Rect,
    select_anchor: Pos2,
    selecting: bool,
}

impl Editor {
    pub fn new(ui: EditorWindow, base: RgbaImage) -> Rc<RefCell<Self>> {
        let size = Vec2::new(base.width() as f32, base.height() as f32);
        let editor = Rc::new(RefCell::new(Self {
            ui,
            base: Rc::new(base),
            annotations: Vec::new(),
            current: None,
            locked_angle: None,
            tool: ToolKind::Select,
            color: PALETTE[0],
            width: 3.0,
            selection: Rect::from_min_size(Pos2::ZERO, size),
            select_anchor: Pos2::ZERO,
            selecting: false,
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

        editor.borrow().refresh_overlay();
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

    fn on_pointer(&mut self, nx: f32, ny: f32, phase: i32) {
        let size = self.size();
        let pos = Pos2::new(nx.clamp(0.0, 1.0) * size.x, ny.clamp(0.0, 1.0) * size.y);
        let (ctrl, _shift) = platform::query_modifiers();

        match phase {
            0 => {
                self.locked_angle = None;
                if self.tool == ToolKind::Select {
                    self.selecting = true;
                    self.select_anchor = pos;
                    self.selection = Rect::from_two_pos(pos, pos);
                } else if let Some(shape) = Shape::begin(self.tool, pos) {
                    self.current = Some(shape);
                }
            }
            1 => {
                if self.selecting {
                    self.selection = Rect::from_two_pos(self.select_anchor, pos);
                } else if let Some(shape) = self.current.as_mut() {
                    self.locked_angle = shape.update(pos, ctrl, self.locked_angle);
                }
            }
            2 => {
                if self.selecting {
                    self.selecting = false;
                    self.selection = self.selection.intersect(self.image_rect());
                } else if let Some(shape) = self.current.take() {
                    self.annotations.push(Annotation {
                        shape,
                        color: self.color,
                        width: self.width,
                    });
                    self.refresh_overlay();
                }
            }
            _ => {}
        }

        self.refresh_selection();
        self.refresh_preview();
        self.refresh_hint();
    }

    fn on_key(&mut self, text: &str) {
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
            self.annotations.pop();
            self.refresh_overlay();
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

    fn set_tool(&mut self, tool: ToolKind) {
        self.tool = tool;
        self.refresh_hint();
    }

    fn refresh_overlay(&self) {
        let mut image = (*self.base).clone();
        render_annotations(&mut image, &self.annotations, Pos2::ZERO);
        self.ui.set_overlay(to_slint_image(&image));
    }

    fn refresh_preview(&self) {
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
        let mut image = imageops::crop_imm(self.base.as_ref(), x0, y0, x1 - x0, y1 - y0).to_image();
        shape.render(&mut image, self.color, self.width, Pos2::new(x0 as f32, y0 as f32));

        self.ui.set_preview(to_slint_image(&image));
        self.ui.set_preview_nx(x0 as f32 / size.x);
        self.ui.set_preview_ny(y0 as f32 / size.y);
        self.ui.set_preview_nw((x1 - x0) as f32 / size.x);
        self.ui.set_preview_nh((y1 - y0) as f32 / size.y);
        self.ui.set_preview_visible(true);
    }

    fn refresh_selection(&self) {
        let size = self.size();
        let r = self.selection;
        self.ui.set_sel_nx(r.min.x / size.x);
        self.ui.set_sel_ny(r.min.y / size.y);
        self.ui.set_sel_nw((r.max.x - r.min.x) / size.x);
        self.ui.set_sel_nh((r.max.y - r.min.y) / size.y);
        self.ui.set_sel_visible(true);
    }

    fn refresh_hint(&self) {
        self.ui.set_hint(
            format!(
                "{:?} · color {} · width {:.0} · R rect · Shift+R fill · L line · A arrow · C circle · P pen · H highlight · D redact · V select · Ctrl+Z undo · Enter copy · Ctrl+S save · Esc quit",
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
