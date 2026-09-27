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

/// Present mode requested via `--present-mode`. `Auto` keeps eframe's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentMode {
    Vsync,
    Immediate,
    Mailbox,
}

#[cfg(feature = "software")]
static PRESENT_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Frame latency requested via `--frame-latency`. `None` keeps the default (1).
/// Only read by the software renderer, so it is gated to keep GPU-only builds
/// warning-free.
#[cfg(feature = "software")]
static FRAME_LATENCY: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(i64::MIN);
#[cfg(feature = "software")]
const FRAME_LATENCY_UNSET: i64 = i64::MIN;

/// `--repaint-probe`: request a repaint every frame so frame timings contain no
/// idle wait. A diagnostic, not a preference.
static REPAINT_PROBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Enable the repaint probe (`--repaint-probe`).
pub fn set_repaint_probe(enabled: bool) {
    REPAINT_PROBE.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the repaint probe was requested via `--repaint-probe`.
pub(crate) fn repaint_probe_requested() -> bool {
    REPAINT_PROBE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Set the wgpu present mode for the software renderer (the `--present-mode`
/// flag). Overrides eframe's `AutoVsync` default.
///
/// A no-op in builds without the software renderer, where the flag cannot
/// affect anything.
pub fn set_present_mode(mode: PresentMode) {
    #[cfg(feature = "software")]
    {
        let value = match mode {
            PresentMode::Vsync => 1,
            PresentMode::Immediate => 2,
            PresentMode::Mailbox => 3,
        };
        PRESENT_MODE.store(value, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(not(feature = "software"))]
    let _ = mode;
}

#[cfg(feature = "software")]
fn requested_present_mode() -> Option<PresentMode> {
    match PRESENT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Some(PresentMode::Vsync),
        2 => Some(PresentMode::Immediate),
        3 => Some(PresentMode::Mailbox),
        _ => None,
    }
}

/// Set how many frames the presentation engine may queue (the
/// `--frame-latency` flag). `0` disables the cap; the default of 1 favours
/// latency over throughput.
///
/// A no-op in builds without the software renderer.
pub fn set_frame_latency(frames: u32) {
    #[cfg(feature = "software")]
    {
        FRAME_LATENCY.store(i64::from(frames), std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(not(feature = "software"))]
    let _ = frames;
}

/// The configured frame-latency cap, or the default of 1 when unset.
#[cfg(feature = "software")]
fn requested_frame_latency() -> Option<u32> {
    match FRAME_LATENCY.load(std::sync::atomic::Ordering::Relaxed) {
        FRAME_LATENCY_UNSET => Some(1), // existing default behaviour
        0 => None,                      // explicitly uncapped
        n => Some(n as u32),
    }
}

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

/// True when at least one enumerated wgpu adapter is not a CPU rasterizer.
/// RDP sessions and GPU-less VMs expose only WARP (`DeviceType::Cpu`).
/// Only called when the `software` feature is compiled in (the `wgpu`
/// dep exists only there); runs once, before `run_native`.
#[cfg(feature = "software")]
fn has_hardware_adapter() -> bool {
    let instance = eframe::wgpu::Instance::new(&eframe::wgpu::InstanceDescriptor::default());
    instance
        .enumerate_adapters(eframe::wgpu::Backends::all())
        .iter()
        .any(|a| a.get_info().device_type != eframe::wgpu::DeviceType::Cpu)
}

/// `auto` + no hardware adapter (RDP, GPU-less VM) → software directly,
/// skipping the glow attempt that panics there. Explicit settings win.
fn effective_with_hardware(setting: RendererSetting, has_hardware: bool) -> EffectiveRenderer {
    match setting {
        _ if !cfg!(feature = "gpu") => EffectiveRenderer::Software,
        RendererSetting::Software if cfg!(feature = "software") => EffectiveRenderer::Software,
        RendererSetting::Gpu => EffectiveRenderer::Gpu,
        _ if !has_hardware && cfg!(feature = "software") => EffectiveRenderer::Software,
        _ => EffectiveRenderer::Gpu,
    }
}

/// The renderer actually chosen at startup, latched by [`apply`]. This is the
/// authoritative answer — `apply` is the only place that knows the adapter
/// probe result, so the HUD reads this rather than re-deriving the decision.
static ACTIVE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(ACTIVE_UNKNOWN);

const ACTIVE_UNKNOWN: u8 = 0;
const ACTIVE_OPENGL: u8 = 1;
const ACTIVE_SOFTWARE: u8 = 2;

/// Human-readable name of the renderer this process is actually using, for the
/// performance HUD and logs. `"unknown"` until [`apply`] has run.
pub(crate) fn active_label() -> &'static str {
    match ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        ACTIVE_OPENGL => "opengl",
        ACTIVE_SOFTWARE => "software",
        _ => "unknown",
    }
}

/// Point `options` at the renderer to use, honoring `POCSHOT_RENDERER`.
pub(crate) fn apply(options: &mut eframe::NativeOptions) {
    let setting = current_setting();
    // Probe only when it can change the outcome: `auto` in a build that
    // carries the software renderer (the `wgpu` dep exists only there).
    // Explicit settings ignore the probe; gpu-only builds have no dep to
    // probe with.
    let probe = setting == RendererSetting::Auto && cfg!(feature = "software") && cfg!(feature = "gpu");
    let has_hardware = if probe {
        #[cfg(feature = "software")]
        { has_hardware_adapter() }
        #[cfg(not(feature = "software"))]
        { true }
    } else {
        true // probe result irrelevant — explicit setting or gpu-only build
    };
    if probe && !has_hardware {
        log::info!(
            "auto renderer: no hardware adapter (RDP / GPU-less) → software, skipping the glow attempt"
        );
    }
    match effective_with_hardware(setting, has_hardware) {
        EffectiveRenderer::Gpu => {
            ACTIVE.store(ACTIVE_OPENGL, std::sync::atomic::Ordering::Relaxed);
            #[cfg(feature = "gpu")]
            apply_gpu(options);
            #[cfg(not(feature = "gpu"))]
            {
                let _ = options;
                unreachable!("gpu renderer not compiled in");
            }
        }
        EffectiveRenderer::Software => {
            ACTIVE.store(ACTIVE_SOFTWARE, std::sync::atomic::Ordering::Relaxed);
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

    // Frames the presentation engine may queue. The default of 1 was added to
    // stop WARP queueing frames ahead of the display, but it also forbids
    // pipelining — every present must finish before the next frame starts —
    // which is a suspect for the ~250ms/frame measured on a VM.
    options.wgpu_options.desired_maximum_frame_latency = requested_frame_latency();
    log::info!(
        "software renderer: desired_maximum_frame_latency = {:?}",
        options.wgpu_options.desired_maximum_frame_latency
    );

    // Present mode. eframe defaults to AutoVsync, which on a CPU adapter with
    // no GPU flip path can mean a synchronous full-surface copy to the
    // compositor every frame — a cost that does not shrink when the app draws
    // less. `--present-mode` allows A/B testing that on the target machine.
    if let Some(mode) = requested_present_mode() {
        let (wgpu_mode, label) = match mode {
            PresentMode::Vsync => (eframe::wgpu::PresentMode::AutoVsync, "vsync"),
            PresentMode::Immediate => (eframe::wgpu::PresentMode::Immediate, "immediate"),
            PresentMode::Mailbox => (eframe::wgpu::PresentMode::Mailbox, "mailbox"),
        };
        log::info!("software renderer: present mode forced to {label} ({wgpu_mode:?})");
        options.wgpu_options.present_mode = wgpu_mode;
    }

    let setup = eframe::egui_wgpu::WgpuSetupCreateNew {
        native_adapter_selector: Some(Arc::new(select_software_adapter)),
        ..Default::default()
    };
    options.wgpu_options.wgpu_setup = setup.into();

    log::info!(
        "software renderer: selecting a CPU wgpu adapter (WARP on Windows, lavapipe on Linux)"
    );
    log::info!(
        "software renderer: transparent window = {:?} (per-pixel alpha can force a \
         full-surface composite instead of a flip)",
        options.viewport.transparent
    );
}

/// Prefer a CPU rasterizer (WARP / lavapipe); fall back to any adapter that can
/// present to the surface, so the software build still starts on a GPU machine.
///
/// Logs the full adapter/surface capability picture at info level: on a
/// GPU-less VM the present path is the suspect for frame-time problems, and
/// whether a fast present mode or an opaque alpha mode is even *available* is
/// decided here.
#[cfg(feature = "software")]
fn select_software_adapter(
    adapters: &[eframe::wgpu::Adapter],
    compatible_surface: Option<&eframe::wgpu::Surface<'_>>,
) -> Result<eframe::wgpu::Adapter, String> {
    use eframe::wgpu::{Adapter, DeviceType};

    let presentable = |adapter: &Adapter| {
        compatible_surface.is_none_or(|surface| adapter.is_surface_supported(surface))
    };

    log::info!("wgpu adapter enumeration ({} found):", adapters.len());
    for (i, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        log::info!(
            "  [{i}] {} | type={:?} backend={:?} driver={} {} | surface_supported={}",
            info.name,
            info.device_type,
            info.backend,
            info.driver,
            info.driver_info,
            presentable(adapter)
        );
    }
    if let Some(surface) = compatible_surface {
        let caps = surface.get_capabilities(
            adapters
                .iter()
                .find(|a| presentable(a))
                .expect("a presentable adapter must exist if a surface was provided"),
        );
        log::info!("  surface: formats={:?}", caps.formats);
        log::info!("  surface: present_modes={:?}", caps.present_modes);
        log::info!("  surface: alpha_modes={:?}", caps.alpha_modes);
        log::info!(
            "  surface: supported_present_modes -> immediate={} mailbox={} fifo={}",
            caps.present_modes.contains(&eframe::wgpu::PresentMode::Immediate),
            caps.present_modes.contains(&eframe::wgpu::PresentMode::Mailbox),
            caps.present_modes.contains(&eframe::wgpu::PresentMode::Fifo),
        );
    } else {
        log::info!("  surface: none provided (no present-capability info)");
    }

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

    #[cfg(feature = "software")]
    #[test]
    fn frame_latency_defaults_to_one_and_zero_means_uncapped() {
        // The pre-flag default must be preserved.
        FRAME_LATENCY.store(FRAME_LATENCY_UNSET, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(requested_frame_latency(), Some(1));

        // 0 is the documented "no cap".
        set_frame_latency(0);
        assert_eq!(requested_frame_latency(), None);

        // Anything else passes through.
        set_frame_latency(3);
        assert_eq!(requested_frame_latency(), Some(3));

        // Restore, so test order cannot leak state into other tests.
        FRAME_LATENCY.store(FRAME_LATENCY_UNSET, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(feature = "software")]
    #[test]
    fn present_mode_is_unset_by_default() {
        PRESENT_MODE.store(0, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(requested_present_mode(), None, "eframe default must stand");

        for mode in [PresentMode::Vsync, PresentMode::Immediate, PresentMode::Mailbox] {
            set_present_mode(mode);
            assert_eq!(requested_present_mode(), Some(mode));
        }

        PRESENT_MODE.store(0, std::sync::atomic::Ordering::Relaxed);
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

    #[cfg(feature = "gpu")]
    #[test]
    fn auto_with_hardware_prefers_glow() {
        assert!(matches!(
            effective_with_hardware(RendererSetting::Auto, true),
            EffectiveRenderer::Gpu
        ));
    }

    #[cfg(feature = "software")]
    #[test]
    fn auto_without_hardware_goes_software() {
        assert!(matches!(
            effective_with_hardware(RendererSetting::Auto, false),
            EffectiveRenderer::Software
        ));
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn explicit_gpu_ignores_hardware_probe() {
        assert!(matches!(
            effective_with_hardware(RendererSetting::Gpu, false),
            EffectiveRenderer::Gpu
        ));
    }

    #[cfg(feature = "software")]
    #[test]
    fn explicit_software_ignores_hardware_probe() {
        assert!(matches!(
            effective_with_hardware(RendererSetting::Software, true),
            EffectiveRenderer::Software
        ));
    }
}
