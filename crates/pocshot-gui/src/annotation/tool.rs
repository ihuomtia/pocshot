//! Per-tool abstraction. Every annotation tool is its own struct (in its own
//! file) implementing the [`Tool`] trait. A [`ToolKind`] enum wraps the
//! concrete structs so an [`Annotation`](super::Annotation) can hold a uniform,
//! cloneable handle without dynamic dispatch.

use eframe::egui::{Color32, Painter, Pos2};
use image::RgbaImage;

use super::blur::BlurTool;
use super::circle::CircleTool;
use super::counter::CounterTool;
use super::eraser::EraserTool;
use super::highlighter::HighlighterTool;
use super::line::ArrowTool;
use super::line::LineTool;
use super::pen::PenTool;
use super::pixelate::PixelateTool;
use super::rect::RectTool;
use super::select::SelectTool;
use super::text::TextTool;

/// Behavior shared by every annotation tool.
///
/// Tools operate in *image* coordinates; the caller maps screen → image before
/// invoking the trait methods, and maps image → screen when drawing.
pub trait Tool {
    /// A fresh starting state for a new drag/click gesture at `pos`.
    fn start(&mut self, pos: Pos2);

    /// Advance the gesture to `pos`. `ctrl` toggles shape/angle constraints.
    /// Returns the newly locked angle (Some) when a constraint took effect.
    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32>;

    /// Draw the on-canvas preview/committed shape, mapping image→screen via `map`.
    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32);

    /// Stamp the shape onto an exported image, offset by `offset` (image px).
    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2);

    /// True if `pos` (image coords) hits this shape within `threshold`.
    fn hit_test(&self, pos: Pos2, threshold: f32, width: f32) -> bool;

    /// Snap-guide coordinates this shape contributes.
    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>);
}

/// Uniform cloneable handle over a concrete tool. Mirrors `AnnotationTool` +
/// each tool's own data so `Annotation` can clone freely for undo/redo.
#[derive(Debug, Clone)]
pub enum ToolKind {
    Select(SelectTool),
    Line(LineTool),
    Arrow(ArrowTool),
    Rectangle(RectTool),
    FilledRectangle(RectTool),
    Circle(CircleTool),
    FilledCircle(CircleTool),
    Pen(PenTool),
    Highlighter(HighlighterTool),
    Counter(CounterTool),
    Pixelate(PixelateTool),
    Blur(BlurTool),
    Text(TextTool),
    Eraser(EraserTool),
}

impl ToolKind {
    pub fn new(tool: super::AnnotationTool) -> Self {
        match tool {
            super::AnnotationTool::Select => Self::Select(SelectTool::default()),
            super::AnnotationTool::Line => Self::Line(LineTool::default()),
            super::AnnotationTool::Arrow => Self::Arrow(ArrowTool::default()),
            super::AnnotationTool::Rectangle => Self::Rectangle(RectTool::new(false)),
            super::AnnotationTool::FilledRectangle => Self::FilledRectangle(RectTool::new(true)),
            super::AnnotationTool::Circle => Self::Circle(CircleTool::new(false)),
            super::AnnotationTool::FilledCircle => Self::FilledCircle(CircleTool::new(true)),
            super::AnnotationTool::Pen => Self::Pen(PenTool::default()),
            super::AnnotationTool::Highlighter => Self::Highlighter(HighlighterTool::default()),
            super::AnnotationTool::Counter => Self::Counter(CounterTool::default()),
            super::AnnotationTool::Pixelate => Self::Pixelate(PixelateTool::default()),
            super::AnnotationTool::Blur => Self::Blur(BlurTool::default()),
            super::AnnotationTool::Text => {
                Self::Text(TextTool::new(Pos2::ZERO, String::new(), 18.0))
            }
            // The text-border and redact tools are handled at the app level
            // (they need OCR results); shapes are committed through
            // `AnnotationState::push_rect`.
            super::AnnotationTool::HighlightText | super::AnnotationTool::Redact => {
                Self::Select(SelectTool::default())
            }
            super::AnnotationTool::Eraser => Self::Eraser(EraserTool::default()),
        }
    }
}

// Thin forwarding so callers can treat a `ToolKind` like a `Tool` without
// matching every time.
impl Tool for ToolKind {
    fn start(&mut self, pos: Pos2) {
        match self {
            Self::Select(t) => t.start(pos),
            Self::Line(t) => t.start(pos),
            Self::Arrow(t) => t.start(pos),
            Self::Rectangle(t) => t.start(pos),
            Self::FilledRectangle(t) => t.start(pos),
            Self::Circle(t) => t.start(pos),
            Self::FilledCircle(t) => t.start(pos),
            Self::Pen(t) => t.start(pos),
            Self::Highlighter(t) => t.start(pos),
            Self::Counter(t) => t.start(pos),
            Self::Pixelate(t) => t.start(pos),
            Self::Blur(t) => t.start(pos),
            Self::Text(t) => t.start(pos),
            Self::Eraser(t) => t.start(pos),
        }
    }

    fn update(&mut self, pos: Pos2, ctrl: bool, locked_angle: Option<f32>) -> Option<f32> {
        match self {
            Self::Select(t) => t.update(pos, ctrl, locked_angle),
            Self::Line(t) => t.update(pos, ctrl, locked_angle),
            Self::Arrow(t) => t.update(pos, ctrl, locked_angle),
            Self::Rectangle(t) => t.update(pos, ctrl, locked_angle),
            Self::FilledRectangle(t) => t.update(pos, ctrl, locked_angle),
            Self::Circle(t) => t.update(pos, ctrl, locked_angle),
            Self::FilledCircle(t) => t.update(pos, ctrl, locked_angle),
            Self::Pen(t) => t.update(pos, ctrl, locked_angle),
            Self::Highlighter(t) => t.update(pos, ctrl, locked_angle),
            Self::Counter(t) => t.update(pos, ctrl, locked_angle),
            Self::Pixelate(t) => t.update(pos, ctrl, locked_angle),
            Self::Blur(t) => t.update(pos, ctrl, locked_angle),
            Self::Text(t) => t.update(pos, ctrl, locked_angle),
            Self::Eraser(t) => t.update(pos, ctrl, locked_angle),
        }
    }

    fn draw(&self, painter: &Painter, map: &dyn Fn(Pos2) -> Pos2, color: Color32, width: f32) {
        match self {
            Self::Select(t) => t.draw(painter, map, color, width),
            Self::Line(t) => t.draw(painter, map, color, width),
            Self::Arrow(t) => t.draw(painter, map, color, width),
            Self::Rectangle(t) => t.draw(painter, map, color, width),
            Self::FilledRectangle(t) => t.draw(painter, map, color, width),
            Self::Circle(t) => t.draw(painter, map, color, width),
            Self::FilledCircle(t) => t.draw(painter, map, color, width),
            Self::Pen(t) => t.draw(painter, map, color, width),
            Self::Highlighter(t) => t.draw(painter, map, color, width),
            Self::Counter(t) => t.draw(painter, map, color, width),
            Self::Pixelate(t) => t.draw(painter, map, color, width),
            Self::Blur(t) => t.draw(painter, map, color, width),
            Self::Text(t) => t.draw(painter, map, color, width),
            Self::Eraser(t) => t.draw(painter, map, color, width),
        }
    }

    fn render(&self, image: &mut RgbaImage, color: Color32, width: f32, offset: Pos2) {
        match self {
            Self::Select(t) => t.render(image, color, width, offset),
            Self::Line(t) => t.render(image, color, width, offset),
            Self::Arrow(t) => t.render(image, color, width, offset),
            Self::Rectangle(t) => t.render(image, color, width, offset),
            Self::FilledRectangle(t) => t.render(image, color, width, offset),
            Self::Circle(t) => t.render(image, color, width, offset),
            Self::FilledCircle(t) => t.render(image, color, width, offset),
            Self::Pen(t) => t.render(image, color, width, offset),
            Self::Highlighter(t) => t.render(image, color, width, offset),
            Self::Counter(t) => t.render(image, color, width, offset),
            Self::Pixelate(t) => t.render(image, color, width, offset),
            Self::Blur(t) => t.render(image, color, width, offset),
            Self::Text(t) => t.render(image, color, width, offset),
            Self::Eraser(t) => t.render(image, color, width, offset),
        }
    }

    fn hit_test(&self, pos: Pos2, threshold: f32, width: f32) -> bool {
        match self {
            Self::Select(t) => t.hit_test(pos, threshold, width),
            Self::Line(t) => t.hit_test(pos, threshold, width),
            Self::Arrow(t) => t.hit_test(pos, threshold, width),
            Self::Rectangle(t) => t.hit_test(pos, threshold, width),
            Self::FilledRectangle(t) => t.hit_test(pos, threshold, width),
            Self::Circle(t) => t.hit_test(pos, threshold, width),
            Self::FilledCircle(t) => t.hit_test(pos, threshold, width),
            Self::Pen(t) => t.hit_test(pos, threshold, width),
            Self::Highlighter(t) => t.hit_test(pos, threshold, width),
            Self::Counter(t) => t.hit_test(pos, threshold, width),
            Self::Pixelate(t) => t.hit_test(pos, threshold, width),
            Self::Blur(t) => t.hit_test(pos, threshold, width),
            Self::Text(t) => t.hit_test(pos, threshold, width),
            Self::Eraser(t) => t.hit_test(pos, threshold, width),
        }
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        match self {
            Self::Select(t) => t.snap_lines(),
            Self::Line(t) => t.snap_lines(),
            Self::Arrow(t) => t.snap_lines(),
            Self::Rectangle(t) => t.snap_lines(),
            Self::FilledRectangle(t) => t.snap_lines(),
            Self::Circle(t) => t.snap_lines(),
            Self::FilledCircle(t) => t.snap_lines(),
            Self::Pen(t) => t.snap_lines(),
            Self::Highlighter(t) => t.snap_lines(),
            Self::Counter(t) => t.snap_lines(),
            Self::Pixelate(t) => t.snap_lines(),
            Self::Blur(t) => t.snap_lines(),
            Self::Text(t) => t.snap_lines(),
            Self::Eraser(t) => t.snap_lines(),
        }
    }
}
