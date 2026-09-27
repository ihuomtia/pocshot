//! Logging setup for the application.
//!
//! Uses the [`log`] facade with an [`env_logger`] backend. By default our
//! crates log at **debug** and the common runtime crates (`winit`, `wgpu`) are
//! dropped to `warn` so the console isn't flooded with their debug trace.
//!
//! Filter resolution, highest priority first:
//! 1. the `--log <FILTER>` command-line flag
//! 2. the `POCSHOT_LOG` environment variable
//! 3. [`DEFAULT_FILTER`]
//!
//! Output goes to stderr, and additionally to a file when `--log-file <PATH>`
//! is given. The file sink matters on Windows: release builds are built with
//! `windows_subsystem = "windows"` (no console), so a redirect like `2>log`
//! captures nothing there.

use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, Once};

static INIT: Once = Once::new();

/// Default filter when neither `--log` nor `POCSHOT_LOG` is set: our crates at
/// `debug`, runtime backends at `warn`, everything else at `info`.
const DEFAULT_FILTER: &str =
    "pocshot=debug,pocshot_core=debug,pocshot_gui=debug,pocshot_ocr=debug,pocshot_snap=debug,\
     winit=warn,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,info";

/// A sink that mirrors every log record to stderr and, when configured, to a
/// file. `Mutex` because the `log` facade is called from worker threads too
/// (capture, snap detection, OCR, effects).
struct TeeWriter {
    file: Option<Mutex<std::fs::File>>,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Some(file) = &self.file {
            if let Ok(mut file) = file.lock() {
                // A failed file write must not lose the log line entirely, so
                // stderr above is the source of truth and file errors are
                // reported there rather than propagated.
                if let Err(e) = file.write_all(buf) {
                    let _ = writeln!(std::io::stderr(), "[pocshot] log file write failed: {e}");
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        if let Some(file) = &self.file {
            if let Ok(mut file) = file.lock() {
                let _ = file.flush();
            }
        }
        Ok(())
    }
}

/// Resolve the effective filter. Pure, so the precedence is testable.
pub(crate) fn resolve_filter(cli: Option<&str>, env: Option<&str>) -> String {
    cli.map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| env.map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or(DEFAULT_FILTER)
        .to_string()
}

/// Initialise the process-wide logger once. Safe to call multiple times; only
/// the first call takes effect.
///
/// `log_file` adds a file sink (created/truncated) in addition to stderr.
/// `log_filter` overrides `POCSHOT_LOG`; see [`resolve_filter`].
pub fn init(log_file: Option<&Path>, log_filter: Option<&str>) {
    INIT.call_once(|| {
        let filter = resolve_filter(log_filter, std::env::var("POCSHOT_LOG").ok().as_deref());

        let file = log_file.and_then(|path| match std::fs::File::create(path) {
            Ok(file) => Some(Mutex::new(file)),
            Err(e) => {
                // Non-fatal: stderr still carries the logs.
                let _ = writeln!(
                    std::io::stderr(),
                    "[pocshot] cannot open log file {}: {e}",
                    path.display()
                );
                None
            }
        });

        let mut builder = env_logger::Builder::new();
        builder.parse_filters(&filter);
        builder.format_timestamp_millis();
        builder.target(env_logger::Target::Pipe(Box::new(TeeWriter { file })));
        let _ = builder.try_init();
    });
}

/// Initialise with defaults only (stderr, `POCSHOT_LOG`). Convenience for the
/// GUI entry points and tests that do not parse the CLI.
pub fn init_default() {
    init(None, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_filter_wins_over_env_and_default() {
        assert_eq!(resolve_filter(Some("trace"), Some("info")), "trace");
        assert_eq!(resolve_filter(Some("trace"), None), "trace");
    }

    #[test]
    fn env_filter_used_when_no_cli_flag() {
        assert_eq!(resolve_filter(None, Some("info")), "info");
    }

    #[test]
    fn blank_values_fall_through_to_the_default() {
        // An empty flag must not mean "log nothing" — it means "unspecified".
        assert_eq!(resolve_filter(Some(""), Some("warn")), "warn");
        assert_eq!(resolve_filter(Some("   "), Some("")), DEFAULT_FILTER);
        assert_eq!(resolve_filter(None, None), DEFAULT_FILTER);
    }
}
