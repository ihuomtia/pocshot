//! Model acquisition: ensure the ocrs `.rten` models are present, downloading
//! them on first use if necessary.

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Filename of the text-detection model.
pub const DETECTION_MODEL: &str = "text-detection.rten";
/// Filename of the text-recognition model.
pub const RECOGNITION_MODEL: &str = "text-recognition.rten";

const DETECTION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

/// Path to a `.rten` model inside `dir`.
pub fn model_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

/// Ensure both models exist in `dir`, downloading them when missing. Returns
/// the detection and recognition paths.
pub fn ensure_models(dir: &Path) -> Result<(PathBuf, PathBuf)> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create models dir {}", dir.display()))?;
    }
    let detection = model_path(dir, DETECTION_MODEL);
    let recognition = model_path(dir, RECOGNITION_MODEL);

    if !detection.exists() {
        download_model(&detection, DETECTION_URL)?;
    }
    if !recognition.exists() {
        download_model(&recognition, RECOGNITION_URL)?;
    }
    Ok((detection, recognition))
}

fn download_model(path: &Path, url: &str) -> Result<()> {
    log::info!(
        "downloading {} from {} …",
        path.file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default(),
        url
    );
    let resp = ureq::get(url)
        // Overall cap so a stalled body read can't hang forever (ureq's
        // default agent only has a 30s connect timeout, no read timeout).
        .timeout(Duration::from_secs(180))
        .call()
        .with_context(|| format!("failed to download {url}"))?;
    let mut data: Vec<u8> = Vec::new();
    resp.into_reader()
        .read_to_end(&mut data)
        .with_context(|| format!("failed to read body from {url}"))?;
    if data.is_empty() {
        anyhow::bail!("downloaded model {url} is empty");
    }
    std::fs::write(path, &data).with_context(|| format!("failed to write {}", path.display()))?;
    let size_mb = data.len() as f64 / (1024.0 * 1024.0);
    log::info!("downloaded {} ({size_mb:.1} MiB)", path.display());
    Ok(())
}
