//! On-canvas performance HUD, drawn in the top-right corner when enabled in
//! the debug settings.
//!
//! Kept deliberately cheap: it must not become part of the problem it exists
//! to measure. Text is laid out per frame (a handful of short strings), so the
//! HUD is opt-in and off by default.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Color32, Rect, Stroke, StrokeKind};

use crate::app::PocshotApp;

/// Rolling window for the averaged timing figures. Long enough to be stable,
/// short enough to react to a change within a second at 60fps.
const WINDOW: usize = 60;

/// Timing samples for the HUD, as a ring buffer of recent frames.
#[derive(Debug, Default)]
pub(crate) struct PerfStats {
    gaps: VecDeque<Duration>,
    builds: VecDeque<Duration>,
    /// Total frames observed since startup (not windowed).
    pub(crate) frames: u64,
    /// Worst `gap` seen since the stats were last reset.
    worst_gap: Duration,
    /// When sampling started, for the average frame rate.
    started: Option<Instant>,
}

impl PerfStats {
    /// Record one frame. `gap` is the wall-clock distance from the previous
    /// frame (includes raster/present and idle wait); `build` is the time spent
    /// inside `update` building the frame.
    pub(crate) fn record(&mut self, gap: Duration, build: Duration) {
        debug_assert!(gap >= build, "build time cannot exceed the frame gap");
        push(&mut self.gaps, gap);
        push(&mut self.builds, build);
        self.frames += 1;
        if gap > self.worst_gap {
            self.worst_gap = gap;
        }
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
    }

    pub(crate) fn reset_window(&mut self) {
        self.gaps.clear();
        self.builds.clear();
        self.worst_gap = Duration::ZERO;
    }

    fn avg_gap(&self) -> Option<Duration> {
        average(&self.gaps)
    }

    fn avg_build(&self) -> Option<Duration> {
        average(&self.builds)
    }

    /// Average frames per second over the rolling window, from the mean gap.
    /// `None` until at least one frame is recorded.
    pub(crate) fn fps(&self) -> Option<f32> {
        self.avg_gap()
            .filter(|d| !d.is_zero())
            .map(|d| 1.0 / d.as_secs_f32())
    }

    /// Milliseconds of the mean inter-frame gap.
    pub(crate) fn gap_ms(&self) -> Option<f32> {
        self.avg_gap().map(|d| d.as_secs_f32() * 1000.0)
    }

    /// Milliseconds of the mean `update` build time.
    pub(crate) fn build_ms(&self) -> Option<f32> {
        self.avg_build().map(|d| d.as_secs_f32() * 1000.0)
    }

    /// Mean time per frame that is NOT our own build work: raster, present and
    /// any idle wait. With the forced-repaint probe on there is no idle wait,
    /// so this is the raster+present cost. Saturates at zero so a stale pair
    /// never reports a negative.
    pub(crate) fn rest_ms(&self) -> Option<f32> {
        match (self.avg_gap(), self.avg_build()) {
            (Some(gap), Some(build)) => {
                Some((gap.saturating_sub(build)).as_secs_f32() * 1000.0)
            }
            _ => None,
        }
    }

    pub(crate) fn worst_gap_ms(&self) -> f32 {
        self.worst_gap.as_secs_f32() * 1000.0
    }
}

fn push(buf: &mut VecDeque<Duration>, value: Duration) {
    if buf.len() == WINDOW {
        buf.pop_front();
    }
    buf.push_back(value);
}

fn average(buf: &VecDeque<Duration>) -> Option<Duration> {
    if buf.is_empty() {
        return None;
    }
    let total: Duration = buf.iter().sum();
    Some(total / buf.len() as u32)
}

/// The lines rendered in the HUD, in order. Pure so the content is testable
/// without a window.
pub(crate) fn hud_lines(
    stats: &PerfStats,
    renderer: &str,
    pixels: [usize; 2],
    pixels_per_point: f32,
) -> Vec<String> {
    let mut lines = vec![
        format!("renderer  {renderer}"),
        format!("canvas    {}x{}", pixels[0], pixels[1]),
        format!("dpi       {pixels_per_point:.2}"),
        format!("frames    {}", stats.frames),
    ];
    match (stats.fps(), stats.gap_ms(), stats.build_ms()) {
        (Some(fps), Some(gap), Some(build)) => {
            lines.push(format!("fps       {fps:.1}"));
            lines.push(format!("gap       {gap:.1} ms"));
            lines.push(format!("build     {build:.1} ms"));
            if let Some(rest) = stats.rest_ms() {
                lines.push(format!("raster    {rest:.1} ms"));
            }
        }
        _ => lines.push("fps       -".to_string()),
    }
    lines.push(format!("worst     {:.1} ms", stats.worst_gap_ms()));
    lines
}

impl PocshotApp {
    /// Draw the performance HUD in the top-right corner of the canvas. No-op
    /// unless the debug setting is on. Clicking the panel resets the rolling
    /// window and the worst-gap figure, so a single interaction (one drag, one
    /// blur) can be measured in isolation.
    pub(crate) fn draw_perf_hud(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        draw_rect: Rect,
        renderer: &str,
    ) {
        if !self.show_perf_hud {
            return;
        }
        let ctx = painter.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let lines = hud_lines(
            &self.perf,
            renderer,
            [self.canvas_px[0] as usize, self.canvas_px[1] as usize],
            ppp,
        );

        let font = egui::FontId::monospace(11.0);
        let line_h = 14.0_f32;
        let pad = 6.0_f32;
        // Size from the widest line so the panel hugs its content.
        let max_w = lines.iter().fold(0.0_f32, |acc, l| {
            let w = ctx
                .fonts(|f| f.layout_no_wrap(l.clone(), font.clone(), Color32::WHITE).size().x);
            acc.max(w)
        });
        let box_size = vec2(max_w + pad * 2.0, lines.len() as f32 * line_h + pad * 2.0);
        let box_rect = Rect::from_min_size(
            pos2(draw_rect.max.x - box_size.x - 8.0, draw_rect.min.y + 8.0),
            box_size,
        );

        painter.rect_filled(
            box_rect,
            self.theme.geometry.help_radius,
            Color32::from_rgba_unmultiplied(8, 8, 12, 200),
        );
        painter.rect_stroke(
            box_rect,
            self.theme.geometry.help_radius,
            Stroke::new(1.0_f32, self.theme.colors.box_border),
            StrokeKind::Inside,
        );

        let mut y = box_rect.min.y + pad;
        for line in &lines {
            painter.text(
                pos2(box_rect.min.x + pad, y),
                egui::Align2::LEFT_TOP,
                line,
                font.clone(),
                self.theme.colors.text_primary,
            );
            y += line_h;
        }

        // Click to reset the rolling window (measured interactions in
        // isolation). Registered after painting so it never blocks the canvas.
        let resp = ui.interact(
            box_rect,
            eframe::egui::Id::new("perf-hud"),
            eframe::egui::Sense::click(),
        );
        if resp.clicked() {
            self.perf.reset_window();
            self.last_frame = None;
        }
        if resp.hovered() {
            ctx.set_cursor_icon(eframe::egui::CursorIcon::PointingHand);
            resp.on_hover_text("Click to reset the perf window");
        }
    }
}

/// Per-phase timings of one `update()`, for locating where build time goes.
///
/// `build` in the HUD is the total; these split it so a regression can be
/// attributed rather than just observed. Cheap: a handful of `Instant`s per
/// frame, and only formatted when a debug log line is actually emitted.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct BuildPhases {
    /// Worker polling, capture/snap/OCR/effect bookkeeping.
    pub(crate) polls: Duration,
    /// Drawing the screenshot texture (the fullscreen blit).
    pub(crate) canvas_blit: Duration,
    /// Toolbar, help, settings panel, status line, HUD.
    pub(crate) chrome: Duration,
    /// Selection chrome, annotations, effect/OCR overlays.
    pub(crate) overlays: Duration,
}

impl BuildPhases {
    /// The three phases other than `polls`, as one figure.
    pub(crate) fn draw(&self) -> Duration {
        self.canvas_blit + self.chrome + self.overlays
    }

    /// Compact one-line form for the frame debug log.
    pub(crate) fn summary(&self) -> String {
        format!(
            "polls={:?} blit={:?} overlays={:?} chrome={:?}",
            self.polls, self.canvas_blit, self.overlays, self.chrome
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_excludes_polling() {
        let p = BuildPhases {
            polls: Duration::from_millis(5),
            canvas_blit: Duration::from_millis(2),
            chrome: Duration::from_millis(1),
            overlays: Duration::from_millis(3),
        };
        assert_eq!(p.draw(), Duration::from_millis(6));
    }

    #[test]
    fn phase_summary_names_every_phase() {
        let p = BuildPhases::default();
        let s = p.summary();
        for field in ["polls=", "blit=", "overlays=", "chrome="] {
            assert!(s.contains(field), "{s:?} missing {field:?}");
        }
    }
    use super::*;

    fn stats_from(gaps_ms: &[u64], builds_ms: &[u64]) -> PerfStats {
        let mut s = PerfStats::default();
        for (g, b) in gaps_ms.iter().zip(builds_ms) {
            s.record(Duration::from_millis(*g), Duration::from_millis(*b));
        }
        s
    }

    #[test]
    fn averages_over_the_recording_window() {
        let s = stats_from(&[10, 20, 30], &[2, 4, 6]);
        assert_eq!(s.gap_ms(), Some(20.0));
        assert_eq!(s.build_ms(), Some(4.0));
        assert_eq!(s.frames, 3);
    }

    #[test]
    fn fps_derives_from_the_mean_gap() {
        // 16.666ms mean gap == ~60fps.
        let s = stats_from(&[16, 17, 17], &[1, 1, 1]);
        let fps = s.fps().unwrap();
        assert!((fps - 60.0).abs() < 1.0, "expected ~60fps, got {fps}");
    }

    #[test]
    fn window_is_bounded_and_drops_the_oldest() {
        let mut s = PerfStats::default();
        // 10 slow frames then WINDOW fast ones: the slow ones must age out.
        for _ in 0..10 {
            s.record(Duration::from_millis(500), Duration::from_millis(500));
        }
        for _ in 0..WINDOW {
            s.record(Duration::from_millis(10), Duration::from_millis(1));
        }
        assert_eq!(s.gap_ms(), Some(10.0));
        assert_eq!(s.frames, 10 + WINDOW as u64);
        // `worst` is since reset, so it still remembers the slow frames.
        assert_eq!(s.worst_gap_ms(), 500.0);
    }

    #[test]
    fn worst_gap_resets_with_the_window() {
        let mut s = stats_from(&[500], &[1]);
        s.reset_window();
        assert_eq!(s.worst_gap_ms(), 0.0);
        assert_eq!(s.gap_ms(), None);
        assert_eq!(s.fps(), None, "no samples means no fps");
    }

    #[test]
    fn hud_lines_report_placeholders_before_any_frame() {
        let s = PerfStats::default();
        let lines = hud_lines(&s, "software", [1920, 1080], 1.0);
        assert!(lines.iter().any(|l| l.contains("software")));
        assert!(lines.iter().any(|l| l.contains("1920x1080")));
        assert!(lines.iter().any(|l| l.starts_with("fps") && l.contains('-')));
    }

    /// The raster figure is the whole diagnostic: it must be gap minus build,
    /// and must never go negative on a stale pairing.
    #[test]
    fn rest_time_is_gap_minus_build_and_never_negative() {
        let s = stats_from(&[300, 300], &[20, 20]);
        assert_eq!(s.rest_ms(), Some(280.0));

        // Build longer than the gap cannot happen, but the figure must still
        // saturate rather than report a negative.
        let mut odd = PerfStats::default();
        odd.record(Duration::from_millis(10), Duration::from_millis(10));
        assert_eq!(odd.rest_ms(), Some(0.0));
    }

    #[test]
    fn hud_lines_report_measurements_once_sampled() {
        let s = stats_from(&[16, 17, 17], &[3, 3, 3]);
        let lines = hud_lines(&s, "software", [1920, 1080], 1.25);
        let joined = lines.join("\n");
        assert!(joined.contains("dpi       1.25"), "{joined}");
        assert!(joined.contains("build     3.0 ms"), "{joined}");
        assert!(joined.contains("gap       16.7 ms"), "{joined}");
    }
}
