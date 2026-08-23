//! Smoke test: ensure the ocrs `.rten` models (auto-downloaded on first use)
//! and run a synthetic image through the full detect() pipeline.
//!
//! ```sh
//! # uses the default model cache dir; pass a custom one as the first arg
//! cargo run -p pocshot-ocr --example ocr_smoke -- [models-dir] [image.png]
//! ```

use image::RgbaImage;
use pocshot_ocr::detect::{OcrsDetector, TextDetector};
use pocshot_ocr::{models, SourceImage};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let models_dir: Option<PathBuf> = args.next().map(PathBuf::from);
    let image_path = args.next().map(PathBuf::from);

    // A quiet temp dir for tests is nicer than writing into the user cache.
    let dir = models_dir.unwrap_or_else(|| std::env::temp_dir().join("pocshot-ocr-models"));
    let (detection_path, recognition_path) =
        models::ensure_models(&dir).expect("failed to ensure models");

    let image: RgbaImage = match image_path {
        Some(path) => image::open(&path).expect("open image").to_rgba8(),
        None => {
            let (w, h) = (1280, 720);
            let mut img = RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let band = (x as f32 / w as f32 * 255.0) as u8;
                    img.put_pixel(x, y, image::Rgba([band, band / 2, 255 - band, 255]));
                }
            }
            img
        }
    };

    let detector = OcrsDetector::new(&detection_path, &recognition_path)
        .unwrap_or_else(|e| panic!("model load failed: {e:#}"));
    let dynamic = image::DynamicImage::ImageRgba8(image);
    let regions = detector
        .detect(&SourceImage::Borrowed(&dynamic))
        .expect("detection failed");
    let _ = pocshot_ocr::postprocess::filter_by_confidence(
        pocshot_ocr::postprocess::merge_text_regions(regions.clone(), Default::default()),
        0.5,
    );
    eprintln!("detected {} regions", regions.len());
    for r in regions {
        eprintln!(
            "  box=({:.0},{:.0},{:.0},{:.0}) conf={:.3} text={:?}",
            r.rect.x0, r.rect.y0, r.rect.x1, r.rect.y1, r.confidence, r.text
        );
    }
}
