// Release builds on Windows must not spawn a console window: pocshot is a GUI
// screenshot tool, and the console that the default subsystem opens is both
// ugly and slow to appear. Debug builds keep the console so logs are visible.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};
use pocshot_core::{CaptureMode, CaptureOptions, OutputFormat};
use pocshot_ocr::detect::{OcrsDetector, TextDetector};
use pocshot_ocr::models;
use pocshot_ocr::SourceImage;

#[derive(Debug, Parser)]
#[command(name = "pocshot")]
#[command(about = "Cross-platform screenshots — CLI and interactive GUI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the interactive fullscreen selection GUI (default when no subcommand is given)
    Gui,
    /// Capture a screenshot from the command line
    Capture(CaptureArgs),
    /// Show a pinned image snippet in a borderless window
    Pin(PinArgs),
    /// Edit the image currently on the clipboard in the interactive GUI
    Edit,
    /// Run a background tray icon (left click starts a screenshot, right click
    /// menu has Take screenshot / Edit clipboard image / Exit)
    Tray,
    /// List available monitors or windows as JSON
    List {
        #[command(subcommand)]
        target: ListTarget,
    },
    /// Diagnostics helpers for manual testing
    Debug {
        #[command(subcommand)]
        target: DebugTarget,
    },
}

#[derive(Debug, Subcommand)]
enum ListTarget {
    Monitors,
    Windows,
}

#[derive(Debug, Subcommand)]
enum DebugTarget {
    /// Run ocrs text detection + recognition on an image and save it with the
    /// detected boxes and recognized text drawn on
    Ocr(OcrArgs),
}

#[derive(Debug, Args)]
struct OcrArgs {
    /// Image to run detection on (PNG or JPEG)
    image: PathBuf,
    /// Where to save the annotated image (default: overwrite the input)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Directory holding the ocrs .rten models (auto-downloaded if missing).
    /// Defaults to ~/.config/pocshot/models.
    #[arg(long)]
    models_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CaptureArgs {
    #[arg(long, value_enum, default_value_t = CaptureModeArg::Screen)]
    mode: CaptureModeArg,
    #[arg(long)]
    monitor_id: Option<u32>,
    #[arg(long)]
    window_id: Option<u32>,
    #[arg(long)]
    x: Option<u32>,
    #[arg(long)]
    y: Option<u32>,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum)]
    format: Option<FormatArg>,
    #[arg(long, default_value_t = 90)]
    quality: u8,
    #[arg(long, default_value_t = 0.0)]
    delay: f64,
    #[arg(long)]
    clipboard: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CaptureModeArg {
    Screen,
    Region,
    Window,
}

#[derive(Debug, Args)]
struct PinArgs {
    /// Path to the PNG to display
    image: PathBuf,
    /// Window top-left x (logical points, pre-converted from screen pixels)
    #[arg(default_value_t = 0)]
    x: i32,
    /// Window top-left y (logical points, pre-converted from screen pixels)
    #[arg(default_value_t = 0)]
    y: i32,
    /// Window width (logical points)
    #[arg(default_value_t = 0)]
    width: u32,
    /// Window height (logical points)
    #[arg(default_value_t = 0)]
    height: u32,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Png,
    Jpg,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Png => OutputFormat::Png,
            FormatArg::Jpg => OutputFormat::Jpg,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    pocshot_gui::init_logging();
    pocshot_core::install_panic_dialog_hook();

    match cli.command {
        None | Some(Command::Gui) => {
            if let Err(e) = pocshot_gui::run() {
                let message = format!("Pocshot failed to start: {e}");
                log::error!("{message}");
                pocshot_core::show_error_dialog(&message);
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Pin(args)) => pin(args),
        Some(Command::Edit) => {
            let image = match pocshot_core::read_clipboard_image() {
                Ok(image) => image,
                Err(e) => {
                    let message = format!("Clipboard does not contain an image: {e}");
                    log::error!("{message}");
                    pocshot_core::show_error_dialog(&message);
                    std::process::exit(1);
                }
            };
            if let Err(e) = pocshot_gui::run_edit(image) {
                let message = format!("Pocshot failed to start: {e}");
                log::error!("{message}");
                pocshot_core::show_error_dialog(&message);
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::Tray) => {
            if let Err(e) = pocshot_tray::run() {
                let message = format!("Pocshot tray failed: {e}");
                log::error!("{message}");
                pocshot_core::show_error_dialog(&message);
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::List { target }) => list(target),
        Some(Command::Debug { target }) => match target {
            DebugTarget::Ocr(args) => debug_ocr(args),
        },
    }
}

fn pin(args: PinArgs) -> anyhow::Result<()> {
    pocshot_gui::run_pin(args.image, args.x, args.y, args.width, args.height)
        .map_err(|e| anyhow::anyhow!("pin exited with error: {e}"))
}

fn capture(args: CaptureArgs) -> anyhow::Result<()> {
    if args.delay.is_sign_negative() {
        anyhow::bail!("--delay must be greater than or equal to zero");
    }

    let mode = match args.mode {
        CaptureModeArg::Screen => CaptureMode::Screen {
            monitor_id: args.monitor_id,
        },
        CaptureModeArg::Region => CaptureMode::Region {
            monitor_id: args.monitor_id,
            x: required(args.x, "--x")?,
            y: required(args.y, "--y")?,
            width: required(args.width, "--width")?,
            height: required(args.height, "--height")?,
        },
        CaptureModeArg::Window => CaptureMode::Window {
            window_id: required(args.window_id, "--window-id")?,
        },
    };

    let result = pocshot_core::capture(&CaptureOptions {
        mode,
        output: args.output,
        format: args.format.map(Into::into),
        quality: args.quality,
        delay_ms: (args.delay * 1000.0).round() as u64,
        clipboard: args.clipboard,
    })
    .context("failed to capture screenshot")?;

    println!(
        "saved {} ({}x{}, {:?}, clipboard: {})",
        result.path.display(),
        result.width,
        result.height,
        result.format,
        result.clipboard_copied
    );

    Ok(())
}

fn list(target: ListTarget) -> anyhow::Result<()> {
    match target {
        ListTarget::Monitors => {
            let monitors = pocshot_core::list_monitors().context("failed to list monitors")?;
            println!("{}", serde_json::to_string_pretty(&monitors)?);
        }
        ListTarget::Windows => {
            let windows = pocshot_core::list_windows().context("failed to list windows")?;
            println!("{}", serde_json::to_string_pretty(&windows)?);
        }
    }

    Ok(())
}

fn required<T>(value: Option<T>, name: &str) -> anyhow::Result<T> {
    value.with_context(|| format!("{name} is required for this capture mode"))
}

/// Default directory for the ocrs `.rten` models, resolved by
/// `pocshot_core::platform` (APPDATA on Windows, `~/Library/Application
/// Support` on macOS, XDG on Linux).
fn default_models_dir() -> PathBuf {
    pocshot_core::config_dir().join("models")
}

/// `pocshot debug ocr <image>` — run the ocrs detection + recognition pipeline
/// on an image and save it with the detected boxes and recognized text drawn
/// on. Intended for manual tuning of the OCR backend without launching the
/// GUI.
fn debug_ocr(args: OcrArgs) -> anyhow::Result<()> {
    let img = image::open(&args.image)
        .with_context(|| format!("failed to open image: {}", args.image.display()))?
        .into_rgb8();

    let dir = args.models_dir.unwrap_or_else(default_models_dir);
    log::info!("ensuring models in {}", dir.display());
    let (detection, recognition) = models::ensure_models(&dir)
        .with_context(|| format!("failed to ensure models in {}", dir.display()))?;
    log::info!("loading ocrs engine…");
    let detector = OcrsDetector::new(&detection, &recognition)
        .with_context(|| "failed to build ocrs engine")?;

    let start = std::time::Instant::now();
    let dynamic = image::DynamicImage::ImageRgb8(img.clone());
    log::info!("running OCR (detection + recognition)…");
    let regions = detector
        .detect(&SourceImage::Borrowed(&dynamic))
        .context("text detection failed")?;
    let elapsed = start.elapsed().as_secs_f32();

    println!("detected {} regions in {elapsed:.2}s", regions.len());
    for r in &regions {
        println!(
            "  box=({:.1},{:.1},{:.1},{:.1}) conf={:.3} text={:?}",
            r.rect.x0, r.rect.y0, r.rect.x1, r.rect.y1, r.confidence, r.text
        );
    }

    // Draw the detected boxes + text labels onto a copy.
    let mut annotated = img.clone();
    for r in &regions {
        let (x0, y0, x1, y1) = (
            r.rect.x0.round().max(0.0) as u32,
            r.rect.y0.round().max(0.0) as u32,
            r.rect.x1.round() as u32,
            r.rect.y1.round() as u32,
        );
        draw_hollow_rect(&mut annotated, x0, y0, x1, y1, image::Rgb([0, 230, 255]));
        if let Some(text) = &r.text {
            // Tag pixels at the top-left corner so each box is associated with
            // its recognized text in the output image.
            let max_y = annotated.height() - 1;
            let (tw, th) = ((x1 - x0).min(text.len() as u32 * 2).max(1), 2);
            draw_filled_rect(
                &mut annotated,
                x0,
                y0.min(max_y),
                x0 + tw,
                y0 + th,
                image::Rgb([255, 200, 0]),
            );
        }
    }

    let output = args.output.unwrap_or_else(|| args.image.clone());
    annotated
        .save(&output)
        .with_context(|| format!("failed to save annotated image to {}", output.display()))?;
    println!("annotated image saved to {}", output.display());

    Ok(())
}

/// Draw a 1px hollow rectangle outline (clipped to image bounds).
fn draw_hollow_rect(
    img: &mut image::RgbImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    color: image::Rgb<u8>,
) {
    let (w, h) = (img.width(), img.height());
    for x in x0..x1.min(w) {
        if y0 < h {
            img.put_pixel(x, y0, color);
        }
        if y1 > y0 && y1 - 1 < h {
            img.put_pixel(x, y1 - 1, color);
        }
    }
    for y in y0..y1.min(h) {
        if x0 < w {
            img.put_pixel(x0, y, color);
        }
        if x1 > x0 && x1 - 1 < w {
            img.put_pixel(x1 - 1, y, color);
        }
    }
}

/// Draw a filled rectangle (clipped to image bounds).
fn draw_filled_rect(
    img: &mut image::RgbImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    color: image::Rgb<u8>,
) {
    let (w, h) = (img.width(), img.height());
    for y in y0..y1.min(h) {
        for x in x0..x1.min(w) {
            img.put_pixel(x, y, color);
        }
    }
}
