//! Renderer selection for the eframe window.
//!
//! The build compiles in both of eframe's backends and picks one at startup:
//!
//! - `gpu` — OpenGL via eframe's glow backend. Works on GPU-less Linux through
//!   Mesa's llvmpipe, but on Windows a machine with no real OpenGL 3.x driver
//!   (RDP sessions, GPU-less VMs) only exposes GDI's OpenGL 1.1, which glow
//!   cannot use.
//! - `software` — wgpu forced onto a CPU rasterizer (WARP on Windows, lavapipe
//!   on Linux). wgpu's DX12 backend picks WARP when no hardware adapter is
//!   present, and shader compilation falls back from DXC to FXC
//!   (`d3dcompiler_47.dll`, shipped with Windows), so no extra runtime DLLs are
//!   needed.
//!
//! `POCSHOT_RENDERER=gpu|software|auto` (default `auto`) forces the choice.
//! Under `auto` the renderer prefers OpenGL and, if it cannot initialize,
//! relaunches the same executable with `POCSHOT_RENDERER=software`. eframe's
//! glow backend *panics* (rather than returning an error) when no matching GL
//! config exists, so the fallback is triggered from a panic hook plus the
//! `Err` return path; the relaunch happens in a fresh process because eframe's
//! winit event loop cannot be restarted in-process.

use std::ffi::OsString;

#[cfg(not(any(feature = "gpu", feature = "software")))]
compile_error!("pocshot-gui needs at least one of the `gpu` or `software` features");

/// Which renderer the user asked for, before availability is considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RendererSetting {
    Auto,
    Gpu,
    Software,
}

/// The renderer actually used, after applying compiled-in features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// In a software-only build `Gpu` is never constructed (it is still a valid
// `effective` result in the `gpu`-capable builds).
#[cfg_attr(not(feature = "gpu"), allow(dead_code))]
enum EffectiveRenderer {
    Gpu,
    Software,
}

/// Parse a `POCSHOT_RENDERER` value. Unknown, empty and missing values mean
/// `Auto`.
fn parse_setting(value: Option<&str>) -> RendererSetting {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("gpu") | Some("glow") | Some("opengl") => RendererSetting::Gpu,
        Some("software") | Some("soft") | Some("wgpu") => RendererSetting::Software,
        _ => RendererSetting::Auto,
    }
}

/// A renderer forced programmatically (the `--software` flag), taking
/// precedence over `POCSHOT_RENDERER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForcedRenderer {
    None,
    Gpu,
    Software,
}

/// `--software` sets this; read on every `apply`/fallback decision.
static FORCED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn forced_renderer() -> ForcedRenderer {
    match FORCED.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ForcedRenderer::Gpu,
        2 => ForcedRenderer::Software,
        _ => ForcedRenderer::None,
    }
}

/// Combine the programmatic override with `POCSHOT_RENDERER`.
fn resolve_setting(forced: ForcedRenderer, env: Option<&str>) -> RendererSetting {
    match forced {
        ForcedRenderer::Gpu => RendererSetting::Gpu,
        ForcedRenderer::Software => RendererSetting::Software,
        ForcedRenderer::None => parse_setting(env),
    }
}

fn current_setting() -> RendererSetting {
    resolve_setting(
        forced_renderer(),
        std::env::var("POCSHOT_RENDERER").ok().as_deref(),
    )
}

/// Force the software (wgpu CPU) renderer for this process — the `--software`
/// command-line flag. Overrides `POCSHOT_RENDERER`.
pub fn set_software_renderer() {
    FORCED.store(2, std::sync::atomic::Ordering::Relaxed);
}

/// True when `reason` is an OpenGL/glutin initialization failure. Covers
/// eframe's glow config-picker panic ("...creating glutin config"), the
/// `NoGlutinConfigs` error, and the `Glutin`/`OpenGL` error variants
/// ("glutin error: ...", "egui_glow: ...").
fn is_gl_failure(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    lower.contains("glutin") || lower.contains("opengl") || lower.contains("glow")
}

/// Whether a failure with `reason` should relaunch in software mode.
///
/// Only under `auto`: an explicit `POCSHOT_RENDERER=gpu` is respected (the
/// error is surfaced instead of silently downgrading).
fn decide_fallback(reason: &str, setting: RendererSetting) -> bool {
    setting == RendererSetting::Auto && is_gl_failure(reason)
}

/// The command-line arguments for the relaunched process: everything after the
/// program name.
// `fallback_and_exit` only uses this when the `software` feature is compiled
// in, but the unit test exercises it in every build.
#[cfg_attr(not(feature = "software"), allow(dead_code))]
fn fallback_args(argv: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    argv.into_iter().skip(1).collect()
}

/// Resolve the requested setting against the renderers compiled into this
/// build. A renderer that is not compiled in can never be selected.
fn effective(setting: RendererSetting) -> EffectiveRenderer {
    #[cfg(feature = "gpu")]
    {
        match setting {
            RendererSetting::Software if cfg!(feature = "software") => {
                EffectiveRenderer::Software
            }
            _ => EffectiveRenderer::Gpu,
        }
    }
    #[cfg(not(feature = "gpu"))]
    {
        let _ = setting;
        EffectiveRenderer::Software
    }
}

/// Point `options` at the renderer to use, honoring `POCSHOT_RENDERER`.
pub(crate) fn apply(options: &mut eframe::NativeOptions) {
    match effective(current_setting()) {
        EffectiveRenderer::Gpu => {
            #[cfg(feature = "gpu")]
            apply_gpu(options);
            #[cfg(not(feature = "gpu"))]
            {
                let _ = options;
                unreachable!("gpu renderer not compiled in");
            }
        }
        EffectiveRenderer::Software => {
            #[cfg(feature = "software")]
            apply_software(options);
            #[cfg(not(feature = "software"))]
            {
                let _ = options;
                unreachable!("software renderer not compiled in");
            }
        }
    }
}

#[cfg(feature = "gpu")]
fn apply_gpu(options: &mut eframe::NativeOptions) {
    options.renderer = eframe::Renderer::Glow;
    log::info!("using OpenGL (glow) renderer");
}

/// Force wgpu onto a software (CPU) adapter.
#[cfg(feature = "software")]
fn apply_software(options: &mut eframe::NativeOptions) {
    use std::sync::Arc;

    options.renderer = eframe::Renderer::Wgpu;
    // Don't let the CPU adapter queue frames ahead of the display: on WARP /
    // lavapipe the default queue makes input feel laggy even when every frame
    // renders in reasonable time.
    options.wgpu_options.desired_maximum_frame_latency = Some(1);

    let setup = eframe::egui_wgpu::WgpuSetupCreateNew {
        native_adapter_selector: Some(Arc::new(select_software_adapter)),
        ..Default::default()
    };
    options.wgpu_options.wgpu_setup = setup.into();

    log::info!(
        "software renderer: selecting a CPU wgpu adapter (WARP on Windows, lavapipe on Linux)"
    );
}

/// Prefer a CPU rasterizer (WARP / lavapipe); fall back to any adapter that can
/// present to the surface, so the software build still starts on a GPU machine.
#[cfg(feature = "software")]
fn select_software_adapter(
    adapters: &[eframe::wgpu::Adapter],
    compatible_surface: Option<&eframe::wgpu::Surface<'_>>,
) -> Result<eframe::wgpu::Adapter, String> {
    use eframe::wgpu::{Adapter, DeviceType};

    let presentable = |adapter: &Adapter| {
        compatible_surface.is_none_or(|surface| adapter.is_surface_supported(surface))
    };

    let chosen = adapters
        .iter()
        .find(|a| a.get_info().device_type == DeviceType::Cpu && presentable(a))
        .or_else(|| adapters.iter().find(|a| presentable(a)));

    let adapter = chosen.ok_or_else(|| {
        let names: Vec<String> = adapters.iter().map(|a| a.get_info().name).collect();
        format!("no wgpu adapter can present to the window (enumerated: {names:?})")
    })?;

    let info = adapter.get_info();
    if info.device_type == DeviceType::Cpu {
        log::info!("software renderer using CPU wgpu adapter: {}", info.name);
    } else {
        log::warn!(
            "software renderer found no CPU wgpu adapter, falling back to {} ({:?})",
            info.name,
            info.device_type
        );
    }
    Ok(adapter.clone())
}

/// Whether a startup failure with `reason` should trigger the software
/// relaunch in this build.
pub(crate) fn should_fallback(reason: &str) -> bool {
    cfg!(feature = "software") && decide_fallback(reason, current_setting())
}

/// Extract the panic message, ignoring the location/backtrace wrapper.
fn panic_reason(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        String::new()
    }
}

/// Install a panic hook that turns an OpenGL initialization panic into a
/// relaunch with the software renderer. Must be installed *after* any other
/// panic hook (e.g. the Windows error-dialog hook): non-GL panics are delegated
/// to the previously installed hook.
pub fn install_fallback_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let reason = panic_reason(info);
        if should_fallback(&reason) {
            fallback_and_exit(&reason);
        }
        previous(info);
    }));
}

/// React to `run_native` returning an error: relaunch in software mode when the
/// error is a GL initialization failure.
pub(crate) fn fallback_on_error(err: &eframe::Error) {
    let reason = err.to_string();
    if should_fallback(&reason) {
        fallback_and_exit(&reason);
    }
    log::error!("renderer failed to start: {reason}");
}

/// Relaunch this executable with `POCSHOT_RENDERER=software`, inheriting the
/// original arguments, then exit with the child's status. Diverges: the current
/// process never continues past this point.
pub(crate) fn fallback_and_exit(reason: &str) -> ! {
    #[cfg(feature = "software")]
    {
        log::warn!("OpenGL unavailable ({reason}); relaunching with the software renderer");
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                log::error!("cannot determine current executable for software fallback: {e}");
                std::process::exit(1);
            }
        };
        let status = std::process::Command::new(exe)
            .args(fallback_args(std::env::args_os()))
            .env("POCSHOT_RENDERER", "software")
            .status();
        let code = match status {
            Ok(status) => status.code().unwrap_or(1),
            Err(e) => {
                log::error!("failed to relaunch with the software renderer: {e}");
                1
            }
        };
        std::process::exit(code);
    }
    #[cfg(not(feature = "software"))]
    {
        log::error!("OpenGL unavailable and the software renderer is not compiled in: {reason}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_defaults_to_auto() {
        assert_eq!(parse_setting(None), RendererSetting::Auto);
        assert_eq!(parse_setting(Some("")), RendererSetting::Auto);
        assert_eq!(parse_setting(Some("auto")), RendererSetting::Auto);
        assert_eq!(parse_setting(Some("bogus")), RendererSetting::Auto);
    }

    #[test]
    fn setting_parses_explicit_renderers() {
        assert_eq!(parse_setting(Some("gpu")), RendererSetting::Gpu);
        assert_eq!(parse_setting(Some("GPU")), RendererSetting::Gpu);
        assert_eq!(parse_setting(Some("software")), RendererSetting::Software);
        assert_eq!(parse_setting(Some("soft")), RendererSetting::Software);
        assert_eq!(parse_setting(Some("Software")), RendererSetting::Software);
    }

    #[test]
    fn recognizes_gl_failures() {
        assert!(is_gl_failure(
            "failed to find a matching configuration for creating glutin config"
        ));
        assert!(is_gl_failure(
            "Found no glutin configs matching the template: ConfigTemplate { .. }. Error: x"
        ));
        assert!(is_gl_failure("NoGlutinConfigs(ConfigTemplate { .. }, ..)"));
        // Context-creation failures surface as these eframe error variants.
        assert!(is_gl_failure("glutin error: Failed to create context"));
        assert!(is_gl_failure("egui_glow: Failed to create OpenGL context"));
        assert!(!is_gl_failure("index out of bounds"));
    }

    #[test]
    fn auto_falls_back_on_gl_failure() {
        let message = "failed to find a matching configuration for creating glutin config";
        assert!(decide_fallback(message, RendererSetting::Auto));
    }

    #[test]
    fn explicit_renderer_never_falls_back() {
        let message = "failed to find a matching configuration for creating glutin config";
        assert!(!decide_fallback(message, RendererSetting::Gpu));
        assert!(!decide_fallback(message, RendererSetting::Software));
    }

    #[test]
    fn unrelated_failure_does_not_fall_back() {
        assert!(!decide_fallback("boom", RendererSetting::Auto));
    }

    #[test]
    fn fallback_args_drop_program_name() {
        let argv: Vec<OsString> = ["pocshot", "edit", "--x"].iter().map(OsString::from).collect();
        let expected: Vec<OsString> = ["edit", "--x"].iter().map(OsString::from).collect();
        assert_eq!(fallback_args(argv), expected);
    }

    #[test]
    fn forced_renderer_overrides_env() {
        assert_eq!(
            resolve_setting(ForcedRenderer::Software, Some("gpu")),
            RendererSetting::Software
        );
        assert_eq!(
            resolve_setting(ForcedRenderer::Gpu, Some("software")),
            RendererSetting::Gpu
        );
    }

    #[test]
    fn env_used_when_not_forced() {
        assert_eq!(
            resolve_setting(ForcedRenderer::None, Some("software")),
            RendererSetting::Software
        );
        assert_eq!(resolve_setting(ForcedRenderer::None, None), RendererSetting::Auto);
    }
}
