//! Logging setup for the application.
//!
//! Uses the [`log`] facade with an [`env_logger`] backend. By default our
//! crates log at **debug** and the common runtime crates (`winit`, `wgpu`)
//! are dropped to `warn` so the console isn't flooded with their debug trace.
//! Override everything with the `POCSHOT_LOG` environment variable using
//! env_logger/RUST_LOG syntax, e.g. `POCSHOT_LOG=info` or
//! `POCSHOT_LOG=pocshot_gui=warn,winit=debug`.

use std::sync::Once;

static INIT: Once = Once::new();

/// Default filter when `POCSHOT_LOG` is unset: our crates at `debug`, runtime
/// backends at `warn`, everything else at `info`.
const DEFAULT_FILTER: &str =
    "pocshot=debug,pocshot_core=debug,pocshot_gui=debug,pocshot_ocr=debug,pocshot_snap=debug,\
     winit=warn,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,info";

/// Initialise the process-wide logger once. Safe to call multiple times; only
/// the first call takes effect.
pub fn init() {
    INIT.call_once(|| {
        let mut builder = env_logger::Builder::from_env(
            env_logger::Env::default().filter_or("POCSHOT_LOG", DEFAULT_FILTER),
        );
        builder.format_timestamp_millis();
        let _ = builder.try_init();
    });
}
