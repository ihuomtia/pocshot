//! OS-level region detection: turn top-level window rectangles into snap
//! guides.
//!
//! The edge detector in `pocshot-snap` finds *inner* panels (borders, toolbars,
//! separators) but is blind to the window frame itself. Enumerating windows
//! through the OS gives those outer frames for free, so the two approaches are
//! combined: window edges for the outer regions, Sobel edges for what's inside
//! them. Window coordinates are in the global virtual-desktop space; the image
//! origin is the captured monitor's top-left, so the monitor origin is
//! subtracted before clamping to the image.

use pocshot_core::WindowInfo;
use pocshot_snap::SnapLines;

/// Windows smaller than this (image px, either axis) are ignored — tooltips,
/// popup menus and other transient chrome add noisy guides.
const MIN_WINDOW_EXTENT: f32 = 24.0;

/// Build snap guides from top-level windows that intersect the captured
/// monitor. `exclude_pid` drops windows owned by this process (the fullscreen
/// GUI window itself would otherwise frame the whole capture).
///
/// Each surviving window contributes its four outer edges: left/right to
/// `vertical`, top/bottom to `horizontal`.
pub fn window_snap_lines(
    windows: &[WindowInfo],
    origin_x: i32,
    origin_y: i32,
    image_w: u32,
    image_h: u32,
    exclude_pid: u32,
) -> SnapLines {
    let (w, h) = (image_w as f32, image_h as f32);
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();

    for win in windows {
        if win.pid == exclude_pid || win.is_minimized {
            continue;
        }
        let x0 = (win.x - origin_x) as f32;
        let y0 = (win.y - origin_y) as f32;
        let x1 = x0 + win.width as f32;
        let y1 = y0 + win.height as f32;

        // Keep only windows that actually appear in the capture.
        if x1 <= 0.0 || y1 <= 0.0 || x0 >= w || y0 >= h {
            continue;
        }
        if (x1 - x0) < MIN_WINDOW_EXTENT || (y1 - y0) < MIN_WINDOW_EXTENT {
            continue;
        }

        let cx0 = x0.clamp(0.0, w);
        let cy0 = y0.clamp(0.0, h);
        let cx1 = x1.clamp(0.0, w);
        let cy1 = y1.clamp(0.0, h);

        vertical.push(cx0);
        vertical.push(cx1);
        horizontal.push(cy0);
        horizontal.push(cy1);
    }

    SnapLines {
        horizontal,
        vertical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u32, pid: u32, x: i32, y: i32, w: u32, h: u32, minimized: bool) -> WindowInfo {
        WindowInfo {
            id,
            pid,
            app_name: "app".to_string(),
            title: "title".to_string(),
            x,
            y,
            width: w,
            height: h,
            is_minimized: minimized,
            is_focused: false,
        }
    }

    #[test]
    fn translates_global_coords_to_image_space() {
        // Monitor origin (100, 50): a window at global (150, 90) 200x100 lands
        // at image (50, 40)-(250, 140).
        let windows = [win(1, 42, 150, 90, 200, 100, false)];
        let lines = window_snap_lines(&windows, 100, 50, 1000, 800, 999);
        assert_eq!(lines.vertical, vec![50.0, 250.0]);
        assert_eq!(lines.horizontal, vec![40.0, 140.0]);
    }

    #[test]
    fn drops_own_process_and_minimized_windows() {
        let windows = [
            win(1, 7, 0, 0, 500, 500, false),   // our GUI window
            win(2, 8, 0, 0, 500, 500, true),    // minimized
            win(3, 9, 10, 10, 100, 100, false), // kept
        ];
        let lines = window_snap_lines(&windows, 0, 0, 1920, 1080, 7);
        assert_eq!(lines.vertical, vec![10.0, 110.0]);
        assert_eq!(lines.horizontal, vec![10.0, 110.0]);
    }

    #[test]
    fn clamps_partially_offscreen_windows() {
        // Window starts left/above the monitor; edges clamp to the image.
        let windows = [win(1, 8, -50, -20, 100, 100, false)];
        let lines = window_snap_lines(&windows, 0, 0, 1920, 1080, 999);
        assert_eq!(lines.vertical, vec![0.0, 50.0]);
        assert_eq!(lines.horizontal, vec![0.0, 80.0]);
    }

    #[test]
    fn skips_windows_outside_the_capture() {
        let windows = [win(1, 8, 3000, 3000, 200, 200, false)];
        let lines = window_snap_lines(&windows, 0, 0, 1920, 1080, 999);
        assert!(lines.vertical.is_empty());
        assert!(lines.horizontal.is_empty());
    }

    #[test]
    fn skips_tiny_windows() {
        let windows = [win(1, 8, 10, 10, 10, 10, false)];
        let lines = window_snap_lines(&windows, 0, 0, 1920, 1080, 999);
        assert!(lines.vertical.is_empty());
        assert!(lines.horizontal.is_empty());
    }
}
