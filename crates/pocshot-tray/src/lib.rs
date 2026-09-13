//! System-tray daemon: `pocshot tray`.
//!
//! The daemon hosts a status icon and stays alive in the background. A left
//! click on the icon (or the "Take screenshot" menu item) launches a fresh GUI
//! process, detached so it outlives the daemon; "Exit" stops the daemon.
//!
//! Backends come from tray-rs (`tray-menu`): the native X11 system-tray
//! protocol on Linux (with a GTK popup menu) and Shell_NotifyIconW +
//! TrackPopupMenu on Windows. The crate pumps its platform event loop on an
//! internal worker thread, so this module only polls the event channel.

use anyhow::Result;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use tray_menu::{
    Divider, Icon, MouseButton, MouseButtonState, PopupMenu, TextEntry, TrayIconBuilder,
    TrayIconEvent,
};

const TOOLTIP: &str = "Pocshot — click to take a screenshot";
const ICON_SIZE: u32 = 64;
const MENU_SCREENSHOT: &str = "screenshot";
const MENU_EXIT: &str = "exit";

/// Run the tray daemon until the user chooses "Exit".
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn run() -> Result<()> {
    // Bound to `_tray` so the icon lives for the whole loop.
    let _tray = TrayIconBuilder::new()
        .with_tooltip(TOOLTIP)
        .with_icon(default_icon()?)
        .build()?;

    log::info!("pocshot tray running (left click to screenshot, right click for menu)");

    loop {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => spawn_gui(),
                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } => {
                    // Rebuild the menu on each open: it is tiny, and this keeps
                    // no state to sync with the backend.
                    let mut menu = PopupMenu::new();
                    menu.add(&TextEntry::of(MENU_SCREENSHOT, "Take screenshot"));
                    menu.add(&Divider);
                    menu.add(&TextEntry::of(MENU_EXIT, "Exit"));

                    match menu.popup(position) {
                        Some(id) if id.0 == MENU_EXIT => {
                            log::info!("pocshot tray exiting");
                            break;
                        }
                        Some(id) if id.0 == MENU_SCREENSHOT => spawn_gui(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn run() -> Result<()> {
    anyhow::bail!("the tray is only supported on Windows and Linux")
}

/// Launch a detached GUI process (`pocshot gui`).
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn spawn_gui() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            log::error!("could not resolve executable: {e}");
            return;
        }
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("gui");
    pocshot_core::quiet_io(&mut cmd);
    pocshot_core::detach(&mut cmd);
    match cmd.spawn() {
        Ok(_) => log::info!("launched screenshot GUI"),
        Err(e) => log::error!("failed to launch GUI: {e}"),
    }
}

/// A small procedural tray icon: rounded accent square with a camera lens.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn default_icon() -> std::result::Result<Icon, tray_menu::BadIcon> {
    const ACCENT: [u8; 3] = [58, 120, 255];
    const LENS: [u8; 3] = [245, 247, 252];
    const PUPIL: [u8; 3] = [24, 28, 38];

    let s = ICON_SIZE as f32;
    let center = s / 2.0 - 0.5;
    let corner_radius = s * 0.22;
    let lens_outer = s * 0.26;
    let lens_inner = s * 0.15;

    let mut rgba = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let px = x as f32;
            let py = y as f32;
            let dx = (px - center).abs() - (s / 2.0 - corner_radius);
            let dy = (py - center).abs() - (s / 2.0 - corner_radius);
            let inside_square =
                dx <= 0.0 || dy <= 0.0 || (dx * dx + dy * dy) <= corner_radius * corner_radius;
            if !inside_square {
                continue;
            }

            let dist = ((px - center).powi(2) + (py - center).powi(2)).sqrt();
            let color = if dist <= lens_inner {
                PUPIL
            } else if dist <= lens_outer {
                LENS
            } else {
                ACCENT
            };

            let i = ((y * ICON_SIZE + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }

    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
}
