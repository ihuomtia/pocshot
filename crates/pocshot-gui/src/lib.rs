mod annotation;
mod app;
mod canvas;
mod config;
mod constrain;
mod effects;
mod logging;
mod renderer;
mod selection;
mod snap;
mod theme;
mod toolbar;
mod window_snap;

pub use app::{run, run_edit, run_pin};
pub use renderer::{
    install_fallback_hook, set_frame_latency, set_present_mode, set_repaint_probe,
    set_software_renderer, PresentMode,
};

/// Initialize the process-wide logger (used by the `pocshot` CLI too, which
/// calls into the shared crates). `log_file` adds a file sink; `log_filter`
/// overrides `POCSHOT_LOG`.
pub use logging::init as init_logging;
/// Initialize logging with defaults (stderr only, honours `POCSHOT_LOG`).
pub use logging::init_default as init_logging_default;
