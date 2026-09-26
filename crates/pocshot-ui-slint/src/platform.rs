//! OS-level modifier queries.
//!
//! Slint's winit backend does not track modifier state: `PointerEvent.modifiers`
//! and `KeyEvent.modifiers` are always false. The app has to ask the windowing
//! system for the current state instead.

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::OnceLock;
    use x11_dl::xlib::{ControlMask, Display, ShiftMask, Xlib};

    static DISPLAY: OnceLock<usize> = OnceLock::new();

    /// Current (Ctrl, Shift) state on X11.
    pub fn query_modifiers() -> (bool, bool) {
        let xlib = match Xlib::open() {
            Ok(xlib) => xlib,
            Err(_) => return (false, false),
        };
        let display =
            *DISPLAY.get_or_init(|| unsafe { (xlib.XOpenDisplay)(std::ptr::null()) as usize });
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
}

#[cfg(target_os = "windows")]
mod imp {
    /// Current (Ctrl, Shift) state via `GetAsyncKeyState`.
    pub fn query_modifiers() -> (bool, bool) {
        // TODO(M1): implement with GetAsyncKeyState(VK_CONTROL / VK_SHIFT).
        (false, false)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod imp {
    pub fn query_modifiers() -> (bool, bool) {
        (false, false)
    }
}

pub use imp::query_modifiers;
