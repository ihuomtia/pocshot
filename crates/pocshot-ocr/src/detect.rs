//! Text detector backed by the [`ocrs`] OCR engine.
//!
//! [`ocrs`] is a purpose-built Rust OCR engine (DBNet-style detection plus a
//! recognition model), running its own `.rten` models on the [`rten`] runtime.
//! Unlike our previous tract-onnx DBNet path, it handles the modern PyTorch /
//! ONNX exports (including 8-bit quantized ones) natively and additionally
//! *recognizes* the text, populating `TextRegion.text`.
//!
//! Pipeline:
//! 1. `prepare_input` → greyscale, `[-0.5, 0.5]` (ocrs handles this).
//! 2. `detect_words` → oriented word rectangles.
//! 3. `find_text_lines` → group words into lines, reading order.
//! 4. `recognize_text` → recognized `String` per line.
//! 5. Emit one `TextRegion` per line: axis-aligned bounding box of the line's
//!    words, `confidence = 1.0` (ocrs gives no per-word confidence), and the
//!    recognized text.
//!
//! Regions are in *original image coordinates* — the same coordinate space as
//! every other stage (snapping, overlay).

use crate::{RectF32, SourceImage, TextRegion};
use anyhow::{Context, Result};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use rten_imageproc::BoundingRect;
use rten_imageproc::RotatedRect;

/// Detector abstraction. Implementations are replaceable without touching the
/// snapping or rendering layers.
pub trait TextDetector: Send + Sync {
    /// Human-readable model/backend name, used for logging.
    fn name(&self) -> &str;
    /// Run detection on `image` and return regions in *image coordinates*.
    fn detect(&self, image: &SourceImage) -> Result<Vec<TextRegion>>;
}

/// ocrs-backed detector: detection + recognition, one `TextRegion` per text
/// line.
pub struct OcrsDetector {
    engine: OcrEngine,
}

impl OcrsDetector {
    /// Load the detection and recognition `.rten` models and build the engine.
    ///
    /// Both paths must exist. The recognition model is optional for
    /// detection-only use, but we always load it (we want the recognized text).
    pub fn new(
        detection_path: &std::path::Path,
        recognition_path: &std::path::Path,
    ) -> Result<Self> {
        let detection_model = Model::load_file(detection_path).with_context(|| {
            format!(
                "failed to load detection model {}",
                detection_path.display()
            )
        })?;
        let recognition_model = Model::load_file(recognition_path).with_context(|| {
            format!(
                "failed to load recognition model {}",
                recognition_path.display()
            )
        })?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .context("failed to build OCR engine")?;
        Ok(Self { engine })
    }
}

impl TextDetector for OcrsDetector {
    fn name(&self) -> &str {
        "ocrs"
    }

    fn detect(&self, image: &SourceImage) -> Result<Vec<TextRegion>> {
        let img = image.as_image().to_rgb8();
        if img.width() == 0 || img.height() == 0 {
            return Ok(Vec::new());
        }
        log::debug!("ocrs: preparing input ({}x{})", img.width(), img.height());
        let source = ImageSource::from_bytes(img.as_raw(), img.dimensions())
            .context("ocrs could not ingest image bytes")?;
        let input = self
            .engine
            .prepare_input(source)
            .context("ocrs input preparation failed")?;
        log::debug!("ocrs: detecting words");
        let words = self
            .engine
            .detect_words(&input)
            .context("ocrs word detection failed")?;
        if words.is_empty() {
            return Ok(Vec::new());
        }
        log::debug!("ocrs: {} words, grouping into lines", words.len());
        let lines = self.engine.find_text_lines(&input, &words);
        log::debug!("ocrs: {} lines, recognizing text", lines.len());
        let line_texts = self
            .engine
            .recognize_text(&input, &lines)
            .context("ocrs text recognition failed")?;

        let mut regions = Vec::with_capacity(lines.len());
        for (i, line_words) in lines.iter().enumerate() {
            let rect = union_rect(line_words);
            let text = line_texts
                .get(i)
                .and_then(|t| t.as_ref().map(|l| l.to_string()));
            regions.push(TextRegion {
                rect,
                confidence: 1.0,
                text,
            });
        }
        Ok(regions)
    }
}

/// Axis-aligned bounding box of a list of (possibly oriented) word rects.
/// Returns a zero-size rect when the list is empty (callers guard separately).
fn union_rect(words: &[RotatedRect]) -> RectF32 {
    if words.is_empty() {
        return RectF32::new(0.0, 0.0, 0.0, 0.0);
    }
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for word in words {
        let r = word.bounding_rect();
        let tl = r.top_left();
        let br = r.bottom_right();
        x0 = x0.min(tl.x);
        y0 = y0.min(tl.y);
        x1 = x1.max(br.x);
        y1 = y1.max(br.y);
    }
    RectF32::new(x0, y0, x1.max(x0), y1.max(y0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rten_imageproc::{PointF, RotatedRect, Vec2};

    // Construct an axis-aligned box: up = (0,1) (vertical in image coords),
    // with `perpendicular (x,y) -> (y,-x)` this yields corners exactly matching
    // the (x0,y0)-(x1,y1) bounds.
    fn word(x0: f32, y0: f32, x1: f32, y1: f32) -> RotatedRect {
        RotatedRect::new(
            PointF::from_yx((y0 + y1) / 2.0, (x0 + x1) / 2.0),
            Vec2::from_xy(0.0, 1.0),
            x1 - x0,
            y1 - y0,
        )
    }

    #[test]
    fn union_of_words_spans_their_bounds() {
        let words = vec![word(0.0, 0.0, 30.0, 16.0), word(42.0, 0.0, 74.0, 16.0)];
        let rect = union_rect(&words);
        assert_eq!(rect.x0, 0.0);
        assert_eq!(rect.y0, 0.0);
        assert_eq!(rect.x1, 74.0);
        assert_eq!(rect.y1, 16.0);
    }

    #[test]
    fn union_of_empty_is_zero_rect() {
        let rect = union_rect(&[]);
        assert!(rect.width() <= 0.0);
        assert!(rect.height() <= 0.0);
    }
}
