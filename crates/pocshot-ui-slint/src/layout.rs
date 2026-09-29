//! Pure toolbar placement math, unit-tested without a Slint window.

/// Sizes and counts needed to measure and place the toolbar.
#[derive(Debug, Clone, Copy)]
pub struct ToolbarSpec {
    pub button: f32,
    pub button_gap: f32,
    pub swatch: f32,
    pub swatch_gap: f32,
    pub sep_w: f32,
    pub pad: f32,
    pub margin: f32,
    pub row_h: f32,
    pub hint_h: f32,
    pub n_actions: usize,
    pub n_tools: usize,
    pub n_swatches: usize,
}

/// Placement of the toolbar panel, in window pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub wrapped: bool,
    pub tooltips_below: bool,
}

fn row_width(spec: &ToolbarSpec, buttons: usize, swatches: usize) -> f32 {
    let n = buttons as f32;
    let s = swatches as f32;
    if n == 0.0 && s == 0.0 {
        return 0.0;
    }
    // buttons + swatches + three separators + the width-stepper allowance.
    n * spec.button
        + (n - 1.0).max(0.0) * spec.button_gap
        + s * spec.swatch
        + (s - 1.0).max(0.0) * spec.swatch_gap
        + 3.0 * (spec.sep_w + 2.0 * spec.button_gap)
        + 2.0 * (spec.button - spec.swatch)
}

/// Compute the toolbar rect. `sel` is (min_x, min_y, width, height) in window
/// pixels. Bounds are always the whole viewport.
pub fn toolbar_layout(win: (f32, f32), sel: (f32, f32, f32, f32), spec: &ToolbarSpec) -> ToolbarLayout {
    let (win_w, win_h) = win;
    let (sx, sy, sw, sh) = sel;
    let sel_bottom = sy + sh;

    let single = row_width(spec, spec.n_actions + spec.n_tools, spec.n_swatches) + 2.0 * spec.pad;
    let action_row = row_width(spec, spec.n_actions, spec.n_swatches) + 2.0 * spec.pad;
    let tool_row = row_width(spec, spec.n_tools, 0) + 2.0 * spec.pad;

    let wrapped = single + 2.0 * spec.margin > win_w;
    let avail = (win_w - 2.0 * spec.margin).max(1.0);
    let width = (if wrapped {
        action_row.max(tool_row)
    } else {
        single
    })
    .min(avail);
    let height = if wrapped { 2.0 * spec.row_h + 2.0 * spec.pad } else { spec.row_h };

    // Centre horizontally on the selection.
    let want_x = sx + sw / 2.0 - width / 2.0;
    let max_x = (win_w - width - spec.margin).max(spec.margin);
    let x = want_x.clamp(spec.margin, max_x);

    let max_y = (win_h - spec.hint_h - height - spec.margin).max(spec.margin);
    let below = sel_bottom + spec.margin;
    let above = sy - height - spec.margin;
    let y = if below <= max_y {
        below
    } else if above >= spec.margin {
        above
    } else {
        (sel_bottom - height - spec.margin).clamp(spec.margin, max_y)
    };
    let y = y.clamp(spec.margin, max_y);

    ToolbarLayout {
        x,
        y,
        width,
        height,
        wrapped,
        tooltips_below: y < 48.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ToolbarSpec {
        ToolbarSpec {
            button: 32.0,
            button_gap: 4.0,
            swatch: 26.0,
            swatch_gap: 4.0,
            sep_w: 1.0,
            pad: 8.0,
            margin: 8.0,
            row_h: 40.0,
            hint_h: 28.0,
            n_actions: 12,
            n_tools: 16,
            n_swatches: 6,
        }
    }

    #[test]
    fn places_below_when_room() {
        let l = toolbar_layout((1920.0, 1080.0), (100.0, 100.0, 400.0, 200.0), &spec());
        assert!(!l.wrapped);
        assert!(l.y >= 300.0 && l.y <= 316.0, "y={}", l.y);
        assert!(l.width > 600.0);
        assert!(l.height >= 40.0 && l.height <= 48.0);
        assert!(!l.tooltips_below);
    }

    #[test]
    fn flips_above_near_bottom() {
        let l = toolbar_layout((1920.0, 1080.0), (100.0, 900.0, 400.0, 150.0), &spec());
        assert!(l.y < 900.0, "toolbar should be above the selection, y={}", l.y);
        assert!(l.y + l.height <= 900.0);
    }

    #[test]
    fn clamps_into_window() {
        // Selection in the bottom-right corner with no room either side.
        let l = toolbar_layout((800.0, 600.0), (600.0, 560.0, 190.0, 30.0), &spec());
        assert!(l.x >= 0.0);
        assert!(l.x + l.width <= 800.0, "right overflow: {}", l.x + l.width);
        assert!(l.y >= 0.0);
        assert!(l.y + l.height <= 600.0 - 28.0, "bottom overflow: {}", l.y + l.height);
    }

    #[test]
    fn wraps_when_too_narrow() {
        let l = toolbar_layout((640.0, 900.0), (40.0, 100.0, 200.0, 150.0), &spec());
        assert!(l.wrapped, "expected wrap at 640px wide");
        assert!(l.height > 48.0, "wrapped height should exceed one row: {}", l.height);
        assert!(l.width <= 640.0);
        assert!(l.x + l.width <= 640.0, "wrapped toolbar must stay on-screen");
    }

    #[test]
    fn tooltips_flip_below_near_top() {
        // A shallow selection at the very top puts the toolbar within ~18px of
        // the top edge, so tooltips must open downwards.
        let l = toolbar_layout((1920.0, 1080.0), (200.0, 0.0, 300.0, 10.0), &spec());
        assert!(!l.wrapped);
        assert!(l.y < 48.0, "toolbar should be near the top, y={}", l.y);
        assert!(l.tooltips_below, "toolbar at the top needs tooltips below");
    }

    #[test]
    fn uses_viewport_bounds() {
        // A small clipboard image in a big window: toolbar must be allowed to
        // use the full viewport, not the image rect.
        let l = toolbar_layout((1920.0, 1080.0), (800.0, 400.0, 320.0, 200.0), &spec());
        assert!(l.width > 600.0);
        assert!(!l.wrapped);
    }
}
