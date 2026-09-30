//! M0 go/no-go spike. Throwaway.
//!
//! Run with the software renderer:
//!   SLINT_BACKEND=winit-software cargo run
//!
//! Compares against `SLINT_BACKEND=winit-femtovg`.
//!
//! The image is RGBA with a transparent border band and an opaque centre
//! rectangle, so window transparency is observable: if the desktop shows
//! through the border band, transparent windows work; if it renders black,
//! they do not.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = SpikeWindow::new()?;
    ui.set_shot(test_image(1920, 1080));
    ui.on_report(|message| eprintln!("spike: {message}"));
    ui.on_quit(|| {
        eprintln!("spike: quit");
        let _ = slint::quit_event_loop();
    });
    ui.on_mod_ctrl(|| query_modifiers().0);
    ui.on_mod_shift(|| query_modifiers().1);
    ui.run()
}

/// Current Ctrl/Shift state straight from the OS. Slint's winit backend does
/// not track modifiers (PointerEvent/KeyEvent `.modifiers` stay false), so the
/// app has to ask the windowing system.
#[cfg(target_os = "linux")]
fn query_modifiers() -> (bool, bool) {
    use std::sync::OnceLock;
    use x11_dl::xlib::{ControlMask, Display, ShiftMask, Xlib};

    static DISPLAY: OnceLock<usize> = OnceLock::new();

    let xlib = match Xlib::open() {
        Ok(xlib) => xlib,
        Err(_) => return (false, false),
    };
    let display = *DISPLAY.get_or_init(|| unsafe { (xlib.XOpenDisplay)(std::ptr::null()) as usize });
    if display == 0 {
        return (false, false);
    }
    let display = display as *mut Display;

    unsafe {
        let root = (xlib.XDefaultRootWindow)(display);
        let (mut root_ret, mut child_ret) = (0, 0);
        let (mut root_x, mut root_y, mut win_x, mut win_y) = (0, 0, 0, 0);
        let mut mask: u32 = 0;
        (xlib.XQueryPointer)(
            display,
            root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );
        (mask & ControlMask != 0, mask & ShiftMask != 0)
    }
}

#[cfg(not(target_os = "linux"))]
fn query_modifiers() -> (bool, bool) {
    // Windows: GetAsyncKeyState(VK_CONTROL/VK_SHIFT); macOS: NSEvent.modifierFlags.
    (false, false)
}

/// RGBA image, premultiplied: opaque centre rectangle, transparent border band.
fn test_image(width: u32, height: u32) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let pixels = buffer.make_mut_slice();
    let (x0, x1) = (width / 4, width * 3 / 4);
    let (y0, y1) = (height / 4, height * 3 / 4);
    for y in 0..height {
        for x in 0..width {
            let inside = x >= x0 && x < x1 && y >= y0 && y < y1;
            let alpha = if inside { 255u8 } else { 0u8 };
            // Premultiplied: scale the colour by alpha.
            let scale = alpha as u32;
            pixels[(y * width + x) as usize] = Rgba8Pixel {
                r: ((x * 255 / width) as u32 * scale / 255) as u8,
                g: ((y * 255 / height) as u32 * scale / 255) as u8,
                b: (64 * scale / 255) as u8,
                a: alpha,
            };
        }
    }
    Image::from_rgba8_premultiplied(buffer)
}
