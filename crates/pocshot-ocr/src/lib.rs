//! Text detection + recognition for Pocshot, backed by the [`ocrs`] engine.
//!
//! Kept independent of egui on purpose: the GUI crate converts `TextRegion`
//! coordinates into its own image-space `egui::Rect` when consuming results,
//! and feeds the region edges into the existing snapping-line engine.
//!
//! The pipeline (all inside [`crate::detect::OcrsDetector`]):
//!
//! ```text
//! screenshot (RgbaImage)
//!   → ocrs prepare_input (greyscale, range [-0.5, 0.5])
//!   → detect_words → oriented word rectangles
//!   → find_text_lines (reading order)
//!   → recognize_text → line strings
//!   → TextRegion per line (axis-aligned, text populated)
//!       → text_regions_to_guides → existing SnapEngine
//! ```
//!
//! Models (`text-detection.rten`, `text-recognition.rten`) are fetched on
//! first use via [`crate::models::ensure_models`].

pub mod detect;
pub mod models;
pub mod postprocess;
pub mod snap;

use image::DynamicImage;

/// Axis-aligned rectangle in some coordinate space (usually image pixels,
/// fractional). The GUI maps this to its own rect type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF32 {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl RectF32 {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn center_x(&self) -> f32 {
        (self.x0 + self.x1) * 0.5
    }

    pub fn center_y(&self) -> f32 {
        (self.y0 + self.y1) * 0.5
    }

    /// Vertical overlap as a fraction of the smaller box height.
    /// Used for line grouping: `1.0` = fully overlapping, `<= 0` = disjoint.
    pub fn vertical_overlap(&self, other: &RectF32) -> f32 {
        let h = self.height().min(other.height());
        if h <= 0.0 {
            return 0.0;
        }
        let lo = self.y0.max(other.y0);
        let hi = self.y1.min(other.y1);
        (hi - lo).max(0.0) / h
    }

    pub fn intersects(&self, other: &RectF32) -> bool {
        self.x0 < other.x1 && self.x1 > other.x0 && self.y0 < other.y1 && self.y1 > other.y0
    }
}

/// A detected text region, in a coordinate space that callers choose
/// (the detector promises *original screenshot* coordinates after scaling).
#[derive(Debug, Clone, PartialEq)]
pub struct TextRegion {
    pub rect: RectF32,
    pub confidence: f32,
    /// Recognition text, when a recognition model is attached. Detection-only
    /// pipelines leave this `None`.
    pub text: Option<String>,
}

impl TextRegion {
    pub fn new(rect: RectF32, confidence: f32) -> Self {
        Self {
            rect,
            confidence,
            text: None,
        }
    }
}

/// Image container used by the detector trait. Either the caller supplies the
/// source screenshot directly, or a pre-resized copy for processing at a
/// reduced resolution (see `should_downscale`/detector config).
pub enum SourceImage<'a> {
    Borrowed(&'a DynamicImage),
    Owned(DynamicImage),
}

impl SourceImage<'_> {
    pub fn as_image(&self) -> &DynamicImage {
        match self {
            SourceImage::Borrowed(img) => img,
            SourceImage::Owned(img) => img,
        }
    }
}
