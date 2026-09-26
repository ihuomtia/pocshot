//! Slint-based pocshot editor.
//!
//! Same facade as `pocshot-gui`: `run()` captures the screen, `run_edit()`
//! edits a caller-supplied image. The window is created only after the capture
//! is in hand, so the opaque fullscreen overlay is never part of the
//! screenshot.

use anyhow::{Context, Result};
use image::RgbaImage;

slint::include_modules!();

mod editor;
mod platform;

/// Capture the screen and open the editor on it.
pub fn run() -> Result<()> {
    let (image, monitor) = pocshot_core::capture_screen_with_monitor()
        .context("failed to capture the screen")?;
    log::info!(
        "captured {}x{} from monitor {}",
        image.width(),
        image.height(),
        monitor.name
    );
    show_editor(image)
}

/// Open the editor on an image supplied by the caller (edit-clipboard flow).
pub fn run_edit(image: RgbaImage) -> Result<()> {
    show_editor(image)
}

fn show_editor(image: RgbaImage) -> Result<()> {
    let ui = EditorWindow::new().context("failed to create the Slint window")?;
    ui.set_shot(editor::to_slint_image(&image));

    // Keep the editor alive for as long as the event loop runs: the Slint
    // callbacks only hold a weak reference to it.
    let editor = editor::Editor::new(ui, image);
    editor.borrow().show().context("failed to show the window")?;
    slint::run_event_loop().context("Slint event loop failed")
}

/// Ask the running event loop to stop.
pub(crate) fn quit_event_loop() {
    let _ = slint::quit_event_loop();
}

/// Quit shortly after an action. On X11 the clipboard contents are served by
/// this process, and arboard's handover to a clipboard manager times out when
/// none is running, so give it a moment before exiting.
pub(crate) fn quit_after_grace() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let _ = slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        });
    });
}
