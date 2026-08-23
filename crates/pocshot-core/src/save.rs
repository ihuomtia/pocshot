use std::{
    borrow::Cow,
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

use chrono::Local;
use image::{DynamicImage, RgbaImage};

use crate::capture::{capture_rgba, validate_options};
use crate::error::{PocshotError, Result};
use crate::types::{CaptureOptions, CaptureResult, OutputFormat};

pub fn capture(options: &CaptureOptions) -> Result<CaptureResult> {
    validate_options(options)?;

    let image = capture_rgba(&options.mode, options.delay_ms)?;
    let format = resolve_format(options.output.as_deref(), options.format);
    let path = resolve_output_path(options.output.as_deref(), format);
    save_rgba(&image, &path, format, options.quality)?;

    let clipboard_copied = if options.clipboard {
        copy_rgba_to_clipboard(&image)?
    } else {
        false
    };

    Ok(CaptureResult {
        path,
        width: image.width(),
        height: image.height(),
        format,
        clipboard_copied,
    })
}

pub fn save_rgba(image: &RgbaImage, path: &Path, format: OutputFormat, quality: u8) -> Result<()> {
    if !(1..=100).contains(&quality) {
        return Err(PocshotError::InvalidOptions(
            "quality must be between 1 and 100".to_string(),
        ));
    }

    save_image(image, path, format, quality)
}

pub fn copy_rgba_to_clipboard(image: &RgbaImage) -> Result<bool> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| PocshotError::Clipboard(error.to_string()))?;
    clipboard
        .set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Borrowed(image.as_raw()),
        })
        .map_err(|error| PocshotError::Clipboard(error.to_string()))?;
    Ok(true)
}

pub fn default_output_path(format: OutputFormat) -> PathBuf {
    PathBuf::from(format!(
        "pocshot-{}.{}",
        Local::now().format("%Y%m%d-%H%M%S"),
        format.extension()
    ))
}

pub fn resolve_format(output: Option<&Path>, explicit: Option<OutputFormat>) -> OutputFormat {
    if let Some(format) = explicit {
        return format;
    }

    match output
        .and_then(Path::extension)
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => OutputFormat::Jpg,
        _ => OutputFormat::Png,
    }
}

fn resolve_output_path(output: Option<&Path>, format: OutputFormat) -> PathBuf {
    output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_output_path(format))
}

fn save_image(image: &RgbaImage, path: &Path, format: OutputFormat, quality: u8) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let dynamic = DynamicImage::ImageRgba8(image.clone());

    match format {
        OutputFormat::Png => dynamic.write_to(writer, image::ImageFormat::Png)?,
        OutputFormat::Jpg => {
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality);
            encoder.encode_image(&dynamic)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CaptureMode;
    use std::path::Path;

    #[test]
    fn resolves_explicit_format_first() {
        let path = Path::new("shot.png");
        assert_eq!(
            resolve_format(Some(path), Some(OutputFormat::Jpg)),
            OutputFormat::Jpg
        );
    }

    #[test]
    fn infers_jpeg_extensions() {
        assert_eq!(
            resolve_format(Some(Path::new("shot.jpeg")), None),
            OutputFormat::Jpg
        );
        assert_eq!(
            resolve_format(Some(Path::new("shot.jpg")), None),
            OutputFormat::Jpg
        );
    }

    #[test]
    fn defaults_unknown_extension_to_png() {
        assert_eq!(
            resolve_format(Some(Path::new("shot.webp")), None),
            OutputFormat::Png
        );
    }

    #[test]
    fn rejects_bad_quality() {
        let options = CaptureOptions {
            quality: 0,
            ..Default::default()
        };
        assert!(matches!(
            validate_options(&options),
            Err(PocshotError::InvalidOptions(_))
        ));
    }

    #[test]
    fn rejects_zero_sized_region() {
        let options = CaptureOptions {
            mode: CaptureMode::Region {
                monitor_id: None,
                x: 0,
                y: 0,
                width: 0,
                height: 200,
            },
            ..Default::default()
        };
        assert!(matches!(
            validate_options(&options),
            Err(PocshotError::InvalidOptions(_))
        ));
    }
}
