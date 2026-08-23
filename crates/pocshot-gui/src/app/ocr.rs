//! OCR / text-detection integration: model loading, the background detection
//! worker and the on-canvas overlay of detected regions.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use image::DynamicImage;
use pocshot_ocr::detect::OcrsDetector;
use pocshot_ocr::models;
use pocshot_ocr::postprocess::{filter_by_confidence, merge_text_regions, MergeConfig};
use pocshot_ocr::{RectF32 as OcrRectF32, SourceImage};
use std::sync::Arc;

use crate::app::PocshotApp;
use crate::config;

impl PocshotApp {
    /// Lazily load the ocrs engine once and keep it for reuse (auto-downloading
    /// the `.rten` models on first use). On failure we log, disable detection
    /// gracefully and leave the rest of the app usable.
    pub(crate) fn ensure_ocr_detector(&mut self) {
        if self.ocr_detector.is_some() || !self.ocr_enabled {
            return;
        }
        let dir = config::ocr_models_dir(&self.ocr_models_dir);
        match models::ensure_models(&dir) {
            Ok((detection, recognition)) => {
                self.status = "Loading OCR models…".to_string();
                match OcrsDetector::new(&detection, &recognition) {
                    Ok(detector) => {
                        log::info!(
                            "loaded ocrs engine (detection {}, recognition {})",
                            detection.display(),
                            recognition.display()
                        );
                        self.ocr_detector = Some(Arc::new(detector));
                    }
                    Err(e) => {
                        log::warn!("text detection disabled: {e:#}");
                        self.status = format!("Text detection disabled: {e}");
                        self.ocr_enabled = false;
                    }
                }
            }
            Err(e) => {
                log::warn!("text detection disabled (model download failed): {e:#}");
                self.status = format!("Text detection disabled: {e}");
                self.ocr_enabled = false;
            }
        }
    }

    /// The currently-selected image region (clamped to the capture), if any.
    /// Used by the region-only setting and by the "OCR this region" toolbar
    /// button.
    fn current_selection_region(&self) -> Option<Rect> {
        let capture = self.capture.as_ref()?;
        let rect = self.selection.map(|s| s.rect())?;
        crate::canvas::clamp_image_rect(rect, capture)
    }

    /// The image region OCR should detect, when the "detect text only in
    /// selection" debug setting is on and a selection exists. `None` means
    /// the whole capture.
    fn ocr_target_region(&self) -> Option<Rect> {
        if !self.ocr_region_only {
            return None;
        }
        self.current_selection_region()
    }

    /// Spawn the async OCR worker for `image`, optionally restricted to
    /// `region` (image coords); the crop origin is sent back so results can be
    /// re-anchored to full-capture coordinates. Returns false when there is
    /// nothing to run (disabled or model unavailable).
    fn spawn_ocr_job(&mut self, image: image::RgbaImage, region: Option<Rect>) -> bool {
        if !config::should_run_detection(self.ocr_enabled, true) {
            return false;
        }
        self.ensure_ocr_detector();
        let Some(detector) = self.ocr_detector.clone() else {
            return false;
        };
        let version = self.capture_version;
        let detect_image = match region {
            Some(r) => {
                let (x, y) = (r.min.x as u32, r.min.y as u32);
                let (w, h) = (r.width().max(1.0) as u32, r.height().max(1.0) as u32);
                image::imageops::crop_imm(&image, x, y, w, h).to_image()
            }
            None => image,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.ocr_rx = Some(rx);
        self.ocr_running = true;
        std::thread::spawn(move || {
            let result = detector
                .detect(&SourceImage::Owned(DynamicImage::ImageRgba8(detect_image)))
                .map_err(|e| e.to_string());
            let _ = tx.send((version, result, region));
        });
        true
    }

    /// Kick off an async detection for the given capture at the current
    /// version. When the "detect text only in selection" debug setting is on,
    /// detection runs on a crop of the selection.
    pub(crate) fn run_ocr_detection(&mut self, image: image::RgbaImage) -> bool {
        let region = if self.ocr_region_only {
            self.ocr_target_region()
        } else {
            None
        };
        self.spawn_ocr_job(image, region)
    }

    /// Force an async detection on the currently-selected region, regardless
    /// of the "detect text only in selection" setting. This is the toolbar
    /// "OCR this region" button.
    pub(crate) fn run_ocr_on_selection(&mut self) -> bool {
        if !config::should_run_detection(self.ocr_enabled, true) {
            self.status = "OCR is disabled".to_string();
            return false;
        }
        let Some(region) = self.current_selection_region() else {
            self.status = "Select a region to OCR".to_string();
            return false;
        };
        let Some(image) = self.capture.clone() else {
            return false;
        };
        if self.spawn_ocr_job(image, Some(region)) {
            // Copy the recognized text to the clipboard once it's detected.
            self.ocr_copy_on_done = true;
            // poll_ocr overwrites this with the real outcome once the worker
            // returns ("OCR: no text detected" / "OCR failed: …").
            self.status = "OCR running…".to_string();
            true
        } else {
            // spawn_ocr_job already set a status (e.g. model load failure).
            false
        }
    }

    /// Detect once per capture: run only when enabled, a capture exists, the
    /// newest version (or, in region-only mode, the selected region) has not
    /// been processed yet and no job is in flight.
    pub(crate) fn maybe_ocr_rerun(&mut self) {
        let target_region = self.ocr_target_region();
        let region_changed = self.ocr_region_only && self.ocr_ready_region != target_region;
        if !self.ocr_running
            && config::should_run_detection(self.ocr_enabled, self.capture.is_some())
            && (self.ocr_ready_version != self.capture_version || region_changed)
        {
            if let Some(img) = self.capture.clone() {
                self.run_ocr_detection(img);
            }
        }
    }

    /// Keep the last valid detection result available while a newer detection
    /// runs; ignore results tagged with a stale capture version.
    pub(crate) fn poll_ocr(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.ocr_rx else {
            return;
        };
        match rx.try_recv() {
            Ok((version, result, region)) => {
                self.ocr_running = false;
                if version == self.capture_version {
                    match result {
                        Ok(raw) => {
                            let min_conf = self.ocr_confidence;
                            let mut filtered = filter_by_confidence(raw, min_conf);
                            // Re-anchor region-only results back to full-capture
                            // coordinates so the overlay and snap lines line up.
                            if let Some(region) = region {
                                for r in &mut filtered {
                                    r.rect.x0 += region.min.x;
                                    r.rect.y0 += region.min.y;
                                    r.rect.x1 += region.min.x;
                                    r.rect.y1 += region.min.y;
                                }
                            }
                            self.ocr_raw_regions = filtered.clone();
                            self.ocr_regions = merge_text_regions(filtered, MergeConfig::default());
                            self.ocr_ready_version = version;
                            self.ocr_ready_region = region;
                            if self.ocr_raw_regions.is_empty() {
                                // Distinguish "detection ran but found nothing"
                                // from an outright failure.
                                self.ocr_copy_on_done = false;
                                self.status = "OCR: no text detected".to_string();
                            } else {
                                log::info!(
                                    "text detection: {} boxes, {} merged regions{}",
                                    self.ocr_raw_regions.len(),
                                    self.ocr_regions.len(),
                                    if region.is_some() {
                                        " (region-only)"
                                    } else {
                                        ""
                                    }
                                );
                                // The "OCR this region" button copies the
                                // recognized text to the clipboard.
                                if std::mem::take(&mut self.ocr_copy_on_done) {
                                    let copied: Vec<&str> = self
                                        .ocr_regions
                                        .iter()
                                        .filter_map(|r| r.text.as_deref())
                                        .collect();
                                    if copied.is_empty() {
                                        self.status = "OCR done, no text to copy".to_string();
                                    } else {
                                        let text = copied.join("\n");
                                        ctx.copy_text(text.clone());
                                        self.status = format!("Copied {} characters", text.len());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // Keep previous valid regions; surface the failure so
                            // the user can see why nothing shows, and advance the
                            // ready-version so we don't relaunch the worker every
                            // frame on a persistent error.
                            self.ocr_copy_on_done = false;
                            self.status = format!("OCR failed: {e}");
                            self.ocr_ready_version = version;
                            self.ocr_ready_region = region;
                            log::error!("text detection failed: {e}");
                        }
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.ocr_rx = None;
                self.ocr_running = false;
            }
        }
    }

    /// Optional overlay showing detected text regions. Reuses the same
    /// image→screen mapping as annotations; never touches the captured image.
    pub(crate) fn draw_ocr_overlay(
        &self,
        painter: &egui::Painter,
        draw_rect: Rect,
        image_size: Vec2,
    ) {
        if !self.ocr_enabled {
            return;
        }
        let to_screen = |p: Pos2| -> Pos2 {
            pos2(
                draw_rect.min.x + p.x / image_size.x * draw_rect.width(),
                draw_rect.min.y + p.y / image_size.y * draw_rect.height(),
            )
        };
        let screen_rect = |r: OcrRectF32| -> Rect {
            Rect::from_min_max(to_screen(pos2(r.x0, r.y0)), to_screen(pos2(r.x1, r.y1)))
        };

        if self.show_ocr_debug {
            let fill = self.theme.colors.ocr_zone;
            for r in &self.ocr_raw_regions {
                let rr = screen_rect(r.rect);
                painter.rect_filled(rr, 0.0, fill);
                painter.text(
                    rr.min + vec2(2.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.2}", r.confidence),
                    egui::FontId::proportional(self.theme.fonts.ocr_label_small),
                    self.theme.colors.ocr_label,
                );
            }
        }

        if self.show_text_boxes {
            let stroke = Stroke::new(1.5_f32, self.theme.colors.ocr_box);
            for r in &self.ocr_regions {
                let rr = screen_rect(r.rect);
                painter.rect_stroke(rr, 0.0, stroke, StrokeKind::Outside);
                let label = match &r.text {
                    Some(t) => format!("{} · {:.2}", t, r.confidence),
                    None => format!("{:.2}", r.confidence),
                };
                painter.text(
                    rr.left_top() + vec2(2.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(self.theme.fonts.ocr_label),
                    self.theme.colors.ocr_box,
                );
            }
        }

        // Highlight the detected text box under the cursor: a plain soft fill
        // with no border or label, so it reads as a hover highlight rather than
        // a debug zone.
        if let Some(pointer) = painter.ctx().pointer_latest_pos() {
            for r in &self.ocr_regions {
                let rr = screen_rect(r.rect);
                if !rr.intersects(Rect::from_center_size(pointer, vec2(1.0, 1.0))) {
                    continue;
                }
                let col = self.theme.colors.ocr_box;
                painter.rect_filled(
                    rr,
                    0.0,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 40),
                );
                break;
            }
        }
    }
}
