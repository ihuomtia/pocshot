use std::{thread, time::Duration};

use image::RgbaImage;
use xcap::{Monitor, Window};

use crate::error::{capture_err, PocshotError, Result};
use crate::types::{CaptureMode, CaptureOptions, MonitorInfo, WindowInfo};

pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::all().map_err(capture_err)?;
    monitors.iter().map(monitor_info).collect()
}

fn monitor_info(monitor: &Monitor) -> Result<MonitorInfo> {
    Ok(MonitorInfo {
        id: monitor.id().map_err(capture_err)?,
        name: monitor.name().map_err(capture_err)?,
        friendly_name: monitor.friendly_name().map_err(capture_err)?,
        x: monitor.x().map_err(capture_err)?,
        y: monitor.y().map_err(capture_err)?,
        width: monitor.width().map_err(capture_err)?,
        height: monitor.height().map_err(capture_err)?,
        scale_factor: monitor.scale_factor().map_err(capture_err)?,
        is_primary: monitor.is_primary().map_err(capture_err)?,
    })
}

/// Capture the primary monitor (the same monitor
/// `CaptureMode::Screen { monitor_id: None }` selects) together with its
/// geometry, so callers can convert global window coordinates into image
/// coordinates (window positions are in the virtual-desktop space; the image
/// origin is the monitor's top-left).
pub fn capture_screen_with_monitor() -> Result<(RgbaImage, MonitorInfo)> {
    let monitor = select_monitor(None)?;
    let info = monitor_info(&monitor)?;
    let image = monitor.capture_image().map_err(capture_err)?;
    Ok((image, info))
}

pub fn list_windows() -> Result<Vec<WindowInfo>> {
    let windows = Window::all().map_err(capture_err)?;
    windows
        .iter()
        .filter_map(|window| match window.is_minimized() {
            Ok(true) => None,
            _ => Some(window),
        })
        .map(|window| {
            Ok(WindowInfo {
                id: window.id().map_err(capture_err)?,
                pid: window.pid().map_err(capture_err)?,
                app_name: window.app_name().map_err(capture_err)?,
                title: window.title().map_err(capture_err)?,
                x: window.x().map_err(capture_err)?,
                y: window.y().map_err(capture_err)?,
                width: window.width().map_err(capture_err)?,
                height: window.height().map_err(capture_err)?,
                is_minimized: window.is_minimized().map_err(capture_err)?,
                is_focused: window.is_focused().map_err(capture_err)?,
            })
        })
        .collect()
}

pub fn capture_rgba(mode: &CaptureMode, delay_ms: u64) -> Result<RgbaImage> {
    let options = CaptureOptions {
        mode: mode.clone(),
        delay_ms,
        ..CaptureOptions::default()
    };
    validate_options(&options)?;

    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    capture_image(mode)
}

pub fn validate_options(options: &CaptureOptions) -> Result<()> {
    if !(1..=100).contains(&options.quality) {
        return Err(PocshotError::InvalidOptions(
            "quality must be between 1 and 100".to_string(),
        ));
    }

    if let CaptureMode::Region { width, height, .. } = options.mode {
        if width == 0 || height == 0 {
            return Err(PocshotError::InvalidOptions(
                "region width and height must be greater than zero".to_string(),
            ));
        }
    }

    Ok(())
}

fn capture_image(mode: &CaptureMode) -> Result<RgbaImage> {
    match mode {
        CaptureMode::Screen { monitor_id } => {
            let monitor = select_monitor(*monitor_id)?;
            monitor.capture_image().map_err(capture_err)
        }
        CaptureMode::Region {
            monitor_id,
            x,
            y,
            width,
            height,
        } => {
            let monitor = select_monitor(*monitor_id)?;
            monitor
                .capture_region(*x, *y, *width, *height)
                .map_err(capture_err)
        }
        CaptureMode::Window { window_id } => {
            let window = select_window(*window_id)?;
            if window.is_minimized().map_err(capture_err)? {
                return Err(PocshotError::InvalidOptions(format!(
                    "window {window_id} is minimized and cannot be captured"
                )));
            }
            window.capture_image().map_err(capture_err)
        }
    }
}

fn select_monitor(monitor_id: Option<u32>) -> Result<Monitor> {
    let monitors = Monitor::all().map_err(capture_err)?;
    if let Some(id) = monitor_id {
        return monitors
            .into_iter()
            .find(|monitor| monitor.id().ok() == Some(id))
            .ok_or_else(|| PocshotError::InvalidOptions(format!("monitor {id} was not found")));
    }

    monitors
        .iter()
        .position(|monitor| monitor.is_primary().unwrap_or(false))
        .and_then(|index| monitors.get(index).cloned())
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| PocshotError::Capture("no monitors found".to_string()))
}

fn select_window(window_id: u32) -> Result<Window> {
    Window::all()
        .map_err(capture_err)?
        .into_iter()
        .find(|window| window.id().ok() == Some(window_id))
        .ok_or_else(|| PocshotError::InvalidOptions(format!("window {window_id} was not found")))
}
