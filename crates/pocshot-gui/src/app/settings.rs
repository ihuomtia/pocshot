//! App settings <-> persistence: convert the running state into an
//! [`AppSettings`] snapshot and save it to disk.

use crate::app::PocshotApp;
use crate::config::{self, AppSettings};

impl PocshotApp {
    pub(crate) fn to_settings(&self) -> AppSettings {
        AppSettings {
            ocr_enabled: self.ocr_enabled,
            show_text_boxes: self.show_text_boxes,
            show_ocr_debug: self.show_ocr_debug,
            ocr_confidence: self.ocr_confidence,
            ocr_models_dir: self.ocr_models_dir.clone(),
            snap_enabled: self.snap_enabled,
            show_snap_lines: self.show_snap_lines,
            ocr_region_only: self.ocr_region_only,
            theme: self.theme.clone(),
        }
    }

    pub(crate) fn persist_settings(&self) {
        config::save(&self.to_settings());
    }
}
