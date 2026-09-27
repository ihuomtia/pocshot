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
mod snap;

/// Capture the screen and open the editor on it.
pub fn run() -> Result<()> {
    init_backend().context("failed to select a Slint backend")?;
    let (image, monitor) = pocshot_core::capture_screen_with_monitor()
        .context("failed to capture the screen")?;
    log::info!(
        "captured {}x{} from monitor {}",
        image.width(),
        image.height(),
        monitor.name
    );

    let started = std::time::Instant::now();
    let mut lines = pocshot_snap::detect_snap_lines(&image, &pocshot_snap::SnapConfig::default());
    if let Ok(windows) = pocshot_core::list_windows() {
        let window_lines = snap::window_snap_lines(
            &windows,
            monitor.x,
            monitor.y,
            image.width(),
            image.height(),
            std::process::id(),
        );
        lines.horizontal.extend(window_lines.horizontal);
        lines.vertical.extend(window_lines.vertical);
    }
    log::info!(
        "snap guides: {} horizontal, {} vertical ({} ms)",
        lines.horizontal.len(),
        lines.vertical.len(),
        started.elapsed().as_millis()
    );
    show_editor(image, lines)
}

/// Open the editor on an image supplied by the caller (edit-clipboard flow).
pub fn run_edit(image: RgbaImage) -> Result<()> {
    init_backend().context("failed to select a Slint backend")?;
    let lines = pocshot_snap::detect_snap_lines(&image, &pocshot_snap::SnapConfig::default());
    show_editor(image, lines)
}

/// Force the software renderer for this process (`--software`).
pub fn set_software_renderer() {
    FORCE_SOFTWARE.store(true, std::sync::atomic::Ordering::Relaxed);
}

static FORCE_SOFTWARE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Pick a renderer, mirroring `POCSHOT_RENDERER` on the egui side: force
/// software when asked, otherwise prefer the GPU renderer and fall back to the
/// software one (GPU-less machines/RDP).
fn init_backend() -> Result<(), slint::PlatformError> {
    let forced = FORCE_SOFTWARE.load(std::sync::atomic::Ordering::Relaxed)
        || std::env::var("POCSHOT_RENDERER")
            .map(|value| value.eq_ignore_ascii_case("software"))
            .unwrap_or(false);

    if forced {
        log::info!("software renderer requested: using Slint's winit-software backend");
        return slint::BackendSelector::new()
            .backend_name("winit".to_string())
            .renderer_name("software".to_string())
            .select();
    }
    if std::env::var_os("SLINT_BACKEND").is_some() {
        // An explicit SLINT_BACKEND wins; Slint reads it itself.
        return Ok(());
    }

    match slint::BackendSelector::new()
        .backend_name("winit".to_string())
        .renderer_name("femtovg".to_string())
        .select()
    {
        Ok(()) => Ok(()),
        Err(error) => {
            log::warn!("femtovg renderer unavailable ({error}); using the software renderer");
            slint::BackendSelector::new()
                .backend_name("winit".to_string())
                .renderer_name("software".to_string())
                .select()
        }
    }
}

fn show_editor(image: RgbaImage, snap_lines: pocshot_snap::SnapLines) -> Result<()> {
    let ui = EditorWindow::new().context("failed to create the Slint window")?;
    ui.set_shot(editor::to_slint_image(&image));

    // Keep the editor alive for as long as the event loop runs: the Slint
    // callbacks only hold a weak reference to it.
    let editor = editor::Editor::new(ui, image, snap_lines);
    editor.borrow().show().context("failed to show the window")?;
    // The window size is only known once shown, so publish the image placement
    // after that.
    editor.borrow().refresh_layout();
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
