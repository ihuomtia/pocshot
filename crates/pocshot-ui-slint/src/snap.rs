//! Snap guides: 1-D magnet snapping plus the OS-window edge detector.
//! Ported from the egui build; pure geometry, no toolkit types.

use pocshot_core::WindowInfo;
use pocshot_snap::SnapLines;

pub const SNAP_DISTANCE: f32 = 8.0;

/// Nearest line within `threshold`, else `pos` unchanged.
pub fn snap_pos(pos: f32, lines: &[f32], threshold: f32) -> f32 {
    lines
        .iter()
        .map(|&l| (l, (l - pos).abs()))
        .filter(|&(_, d)| d <= threshold)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(l, _)| l)
        .unwrap_or(pos)
}

/// Windows smaller than this (image px, either axis) are ignored.
const MIN_WINDOW_EXTENT: f32 = 24.0;

/// Build snap guides from top-level windows that intersect the captured
/// monitor. `exclude_pid` drops windows owned by this process.
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

        if x1 <= 0.0 || y1 <= 0.0 || x0 >= w || y0 >= h {
            continue;
        }
        if (x1 - x0) < MIN_WINDOW_EXTENT || (y1 - y0) < MIN_WINDOW_EXTENT {
            continue;
        }

        vertical.push(x0.clamp(0.0, w));
        vertical.push(x1.clamp(0.0, w));
        horizontal.push(y0.clamp(0.0, h));
        horizontal.push(y1.clamp(0.0, h));
    }

    SnapLines {
        horizontal,
        vertical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_pos_picks_nearest_within_threshold() {
        let lines = [10.0, 30.0, 100.0];
        assert_eq!(snap_pos(12.0, &lines, 8.0), 10.0);
        assert_eq!(snap_pos(28.0, &lines, 8.0), 30.0);
        // No line close enough: unchanged.
        assert_eq!(snap_pos(60.0, &lines, 8.0), 60.0);
    }

    fn win(pid: u32, x: i32, y: i32, w: u32, h: u32, minimized: bool) -> WindowInfo {
        WindowInfo {
            id: 1,
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
    fn window_lines_translate_and_filter() {
        let windows = [
            win(7, 0, 0, 500, 500, false),     // our own process
            win(8, 0, 0, 500, 500, true),      // minimized
            win(9, 150, 90, 200, 100, false),  // kept
        ];
        let lines = window_snap_lines(&windows, 100, 50, 1000, 800, 7);
        assert_eq!(lines.vertical, vec![50.0, 250.0]);
        assert_eq!(lines.horizontal, vec![40.0, 140.0]);
    }
}
