//! Renderer selection for the eframe window.
//!
//! The normal build (`gpu` feature, the default) uses eframe's glow backend
//! (OpenGL). That keeps working on GPU-less Linux machines through Mesa's
//! llvmpipe software OpenGL, but on Windows a machine without a real OpenGL 3.x
//! driver (RDP sessions, GPU-less VMs) only exposes GDI's OpenGL 1.1, which glow
//! cannot use.
//!
//! The `software` feature builds `pocshot-soft` instead: wgpu only (no glutin),
//! with the adapter forced to a CPU rasterizer — WARP on Windows, lavapipe on
//! Linux. wgpu's DX12 backend picks WARP when no hardware adapter is present,
//! and shader compilation falls back from DXC to FXC (`d3dcompiler_47.dll`,
//! shipped with Windows), so no extra DLLs are needed at runtime.

#[cfg(not(any(feature = "gpu", feature = "software")))]
compile_error!("pocshot-gui needs either the `gpu` (default) or `software` feature");

/// Point `options` at the renderer this build flavor ships with.
#[cfg(all(not(feature = "software"), feature = "gpu"))]
pub(crate) fn apply(options: &mut eframe::NativeOptions) {
    options.renderer = eframe::Renderer::Glow;
}

/// Force wgpu onto a software (CPU) adapter.
#[cfg(feature = "software")]
pub(crate) fn apply(options: &mut eframe::NativeOptions) {
    use std::sync::Arc;

    options.renderer = eframe::Renderer::Wgpu;

    let setup = eframe::egui_wgpu::WgpuSetupCreateNew {
        native_adapter_selector: Some(Arc::new(select_software_adapter)),
        ..Default::default()
    };
    options.wgpu_options.wgpu_setup = setup.into();

    log::info!(
        "software renderer build: selecting a CPU wgpu adapter (WARP on Windows, lavapipe on Linux)"
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
