//! Export actions: save / copy / pin the selection, and the toolbar action
//! dispatcher that ties them together with undo/redo and refresh.

use eframe::egui::{self, Rect, Vec2};
use image::DynamicImage;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::PocshotApp;
use crate::canvas::image_to_screen_rect;
use crate::toolbar::Action;
use pocshot_core::{default_output_path, OutputFormat};

impl PocshotApp {
    pub(crate) fn save_selection(&mut self) {
        let Some(mut image) = self.selected_image() else {
            self.status = "Nothing to save".to_string();
            return;
        };

        let offset = self.selection_offset();
        self.annotations.render_to_image(&mut image, offset);

        let path = default_output_path(OutputFormat::Png);
        match pocshot_core::save_rgba(&image, &path, OutputFormat::Png, 90) {
            Ok(()) => {
                if self.clipboard {
                    match pocshot_core::copy_rgba_to_clipboard(&image) {
                        Ok(_) => self.status = format!("Saved and copied {}", path.display()),
                        Err(error) => self.status = format!("Saved {}; {}", path.display(), error),
                    }
                } else {
                    self.status = format!("Saved {}", path.display());
                }
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    pub(crate) fn copy_selection(&mut self, ctx: &egui::Context) {
        let Some(mut image) = self.selected_image() else {
            self.status = "Nothing to copy".to_string();
            return;
        };

        let offset = self.selection_offset();
        self.annotations.render_to_image(&mut image, offset);

        match pocshot_core::copy_rgba_to_clipboard(&image) {
            Ok(true) => {
                self.status = "Copied selection".to_string();
                pocshot_core::send_notification("Pocshot", "Screenshot copied to clipboard");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Ok(false) => self.status = "Clipboard copy was skipped".to_string(),
            Err(error) => self.status = error.to_string(),
        }
    }

    pub(crate) fn handle_action(
        &mut self,
        ctx: &egui::Context,
        draw_rect: Rect,
        image_size: Vec2,
        action: Action,
    ) {
        match action {
            Action::Refresh => self.refresh_source(ctx),
            Action::Save => self.save_selection(),
            Action::Copy => self.copy_selection(ctx),
            Action::Undo => {
                // Undo an annotation if any, else an image-effect edit.
                if self.annotations.can_undo() {
                    self.annotations.undo();
                } else if let Some(prev) = self.capture_undo.pop() {
                    if let Some(cur) = self.capture.clone() {
                        self.capture_redo.push(cur);
                    }
                    self.apply_capture(prev, &ctx.clone());
                }
            }
            Action::Redo => {
                if self.annotations.can_redo() {
                    self.annotations.redo();
                } else if let Some(next) = self.capture_redo.pop() {
                    if let Some(cur) = self.capture.clone() {
                        self.capture_undo.push(cur);
                    }
                    self.apply_capture(next, &ctx.clone());
                }
            }
            Action::Pin => self.pin_selection(ctx, draw_rect, image_size),
            Action::ClearAnnotations => self.annotations.clear(),
            Action::ClearSelection => self.selection = None,
            Action::ToggleSettings => self.show_settings = !self.show_settings,
            Action::OcrRegion => {
                self.run_ocr_on_selection(ctx);
            }
        }
    }

    pub(crate) fn pin_selection(&mut self, ctx: &egui::Context, draw_rect: Rect, image_size: Vec2) {
        let Some(mut image) = self.selected_image() else {
            self.status = "Nothing to pin".to_string();
            return;
        };
        let offset = self.selection_offset();
        self.annotations.render_to_image(&mut image, offset);

        let Some(selection) = self.selection else {
            self.status = "Nothing to pin".to_string();
            return;
        };
        let screen_rect = image_to_screen_rect(selection.rect(), draw_rect, image_size);

        let (width, height) = (image.width(), image.height());
        let mut cursor = std::io::Cursor::new(Vec::new());
        let write_result =
            DynamicImage::ImageRgba8(image).write_to(&mut cursor, image::ImageFormat::Png);
        let png = match write_result {
            Ok(()) => cursor.into_inner(),
            Err(error) => {
                self.status = format!("Pin failed: {}", error);
                return;
            }
        };

        let dir = std::env::temp_dir().join("pocshot-pins");
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.status = format!("Pin failed: {}", error);
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = dir.join(format!("pin-{}-{}.png", std::process::id(), stamp));
        if let Err(error) = std::fs::write(&path, &png) {
            self.status = format!("Pin failed: {}", error);
            return;
        }

        let Ok(exe) = std::env::current_exe() else {
            self.status = "Pin failed: could not resolve executable".to_string();
            return;
        };

        let x = screen_rect.min.x.round() as i32;
        let y = screen_rect.min.y.round() as i32;
        let w = screen_rect.width().round().max(1.0) as u32;
        let h = screen_rect.height().round().max(1.0) as u32;

        let mut cmd = std::process::Command::new(exe);
        cmd.arg("pin")
            .arg(&path)
            .arg(x.to_string())
            .arg(y.to_string())
            .arg(w.to_string())
            .arg(h.to_string());
        pocshot_core::quiet_io(&mut cmd);
        pocshot_core::detach(&mut cmd);

        match cmd.spawn() {
            Ok(_) => {
                self.status = format!("Pinned {} x {}", width, height);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(error) => {
                self.status = format!("Pin failed: {}", error);
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}
