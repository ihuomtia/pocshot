//! Annotation tools and document state.
//!
//! Each tool lives in its own file and implements the [`Tool`] trait (see
//! [`tool`]). A [`ToolKind`] wraps the concrete tool structs so an
//! [`Annotation`] holds a uniform, cloneable handle (for undo/redo) without
//! dynamic dispatch. [`AnnotationState`] owns the annotation list, the undo/
//! redo stacks and the active-tool dispatch.

mod blur;
mod circle;
mod counter;
mod counter_font;
mod eraser;
mod highlighter;
mod line;
mod pen;
mod pixelate;
mod raster;
mod rect;
mod select;
mod text;
mod tool;

use eframe::egui::{self, vec2, Color32, Pos2, Rect};
use image::RgbaImage;

use crate::theme::Theme;

pub use counter::COUNTER_BUBBLE_RADIUS;
pub use tool::{Tool, ToolKind};

/// The selectable drawing/effect tools.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationTool {
    Select,
    Arrow,
    Line,
    Rectangle,
    Circle,
    FilledRectangle,
    FilledCircle,
    Pen,
    Highlighter,
    Counter,
    Pixelate,
    Blur,
    Text,
    /// Draw a rectangular border around detected text (OCR regions) under the
    /// drag. Handled at the app level because it needs the OCR results.
    HighlightText,
    /// Fill a solid rectangle over detected text (OCR regions) under the drag.
    /// Handled at the app level because it needs the OCR results.
    Redact,
    Eraser,
}

impl AnnotationTool {
    /// Tools that draw a persistent overlay shape (as opposed to the cursor,
    /// the counter, the image-effect tools or the eraser).
    pub fn is_draw_tool(self) -> bool {
        !matches!(
            self,
            AnnotationTool::Select
                | AnnotationTool::Counter
                | AnnotationTool::Pixelate
                | AnnotationTool::Blur
                | AnnotationTool::Text
                | AnnotationTool::HighlightText
                | AnnotationTool::Redact
                | AnnotationTool::Eraser
        )
    }

    /// Tools that reshape pixels of the capture region rather than adding an
    /// overlay annotation shape.
    pub fn is_image_effect(self) -> bool {
        matches!(self, AnnotationTool::Pixelate | AnnotationTool::Blur)
    }
}

/// A committed annotation: a tool's shape plus the style it was drawn in.
#[derive(Debug, Clone)]
pub struct Annotation {
    tool: ToolKind,
    color: Color32,
    stroke_width: f32,
}

/// Document state shared by the annotation canvas: the committed shapes, the
/// undo/redo stacks and the active tool.
pub struct AnnotationState {
    pub annotations: Vec<Annotation>,
    redo_stack: Vec<Annotation>,
    pub active_tool: AnnotationTool,
    pub color: Color32,
    pub stroke_width: f32,
    /// Next number the Counter tool will place. Auto-increments per placement
    /// and resets to 1 when annotations are cleared.
    pub counter: u32,
    pending: Option<ToolKind>,
    drag_start: Option<Pos2>,
    constraint_angle: Option<f32>,
    /// Explicit state machine for the Text tool. Global shortcuts are kept
    /// enabled in `Idle` and disabled while `Editing` (the TextEdit owns the
    /// keyboard then).
    text_tool: TextToolState,
    /// Monotonic counter used to give each placed text box a fresh, stable
    /// `egui::Id` (so egui's focus persists for the lifetime of that box).
    text_edit_seq: u64,
    /// Font size (points) for newly placed text annotations. Synced from
    /// `theme.fonts.annotation_text` each frame by the app.
    pub text_size: f32,
}

/// Explicit state machine for the Text annotation tool.
#[derive(Debug, Clone)]
pub enum TextToolState {
    /// No text box exists. Global shortcuts stay enabled.
    Idle,
    /// A text box exists and is being typed into. The TextEdit owns the
    /// keyboard until submit (`Enter`/click-away) or cancel (`Esc`).
    Editing {
        /// Stable `egui::Id` of the text box's `egui::Area`, created once at
        /// placement so the widget keeps its identity (and egui focus) across
        /// frames for the lifetime of this box.
        id: egui::Id,
        /// Image-coordinate box (top-left + size); resizable via handles.
        rect: Rect,
        /// Font size in points, snapshotted at placement.
        size: f32,
        /// The text being typed (may contain `\n`).
        text: String,
        /// True only on the frame the box was created. The app calls
        /// `request_focus()` exactly once and clears this immediately, because
        /// re-requesting focus every frame breaks egui's native TextEdit input
        /// (emilk/egui#5187).
        just_created: bool,
    },
}

/// Immutable snapshot of the editing text box, for the app's editor UI.
#[derive(Debug, Clone, Copy)]
pub struct TextBoxView {
    pub id: egui::Id,
    pub rect: Rect,
    pub size: f32,
    pub just_created: bool,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            annotations: Vec::new(),
            redo_stack: Vec::new(),
            active_tool: AnnotationTool::Select,
            color: Theme::builtin().colors.default_annotation_color,
            stroke_width: 3.0,
            counter: 1,
            pending: None,
            drag_start: None,
            constraint_angle: None,
            text_tool: TextToolState::Idle,
            text_edit_seq: 0,
            text_size: 18.0,
        }
    }
}

impl AnnotationState {
    pub fn clear(&mut self) {
        self.annotations.clear();
        self.redo_stack.clear();
        self.counter = 1;
        self.pending = None;
        self.drag_start = None;
        self.constraint_angle = None;
        self.text_tool = TextToolState::Idle;
    }

    pub fn undo(&mut self) {
        if let Some(a) = self.annotations.pop() {
            self.redo_stack.push(a);
        }
    }

    pub fn redo(&mut self) {
        if let Some(a) = self.redo_stack.pop() {
            self.annotations.push(a);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.annotations.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// True while a Counter bubble placement drag is in progress (so the
    /// caller can hide the idle cursor preview once the gesture starts).
    pub fn is_counter_dragging(&self) -> bool {
        matches!(self.pending, Some(ToolKind::Counter(_)))
            && self.active_tool == AnnotationTool::Counter
    }

    /// True while any shape-drawing gesture is in progress.
    pub fn is_drawing(&self) -> bool {
        self.pending.is_some()
    }

    /// True while a Text annotation is being typed (the app renders a
    /// `TextEdit` while this is true, and must disable its global shortcuts).
    pub fn is_text_editing(&self) -> bool {
        matches!(self.text_tool, TextToolState::Editing { .. })
    }

    /// Immutable snapshot of the editing text box, for the app's editor UI.
    pub fn text_box_view(&self) -> Option<TextBoxView> {
        match &self.text_tool {
            TextToolState::Editing {
                id,
                rect,
                size,
                just_created,
                ..
            } => Some(TextBoxView {
                id: *id,
                rect: *rect,
                size: *size,
                just_created: *just_created,
            }),
            TextToolState::Idle => None,
        }
    }

    /// Open a text box at `pos` (image coords) with font `size`, framed by a
    /// default-size resizable box. Gives the box a fresh, stable `egui::Id`.
    pub fn begin_text_edit(&mut self, pos: Pos2, size: f32) {
        self.text_edit_seq += 1;
        self.text_tool = TextToolState::Editing {
            id: egui::Id::new(("pocshot-text", self.text_edit_seq)),
            rect: Rect::from_min_size(pos, vec2(240.0, 48.0)),
            size,
            text: String::new(),
            just_created: true,
        };
    }

    /// Current editing box, in image coordinates.
    pub fn editing_rect(&self) -> Option<Rect> {
        match &self.text_tool {
            TextToolState::Editing { rect, .. } => Some(*rect),
            TextToolState::Idle => None,
        }
    }

    /// Mutate the editing box rect (used by the resize handles).
    pub fn resize_editing_rect(&mut self, f: impl FnOnce(&mut Rect)) {
        if let TextToolState::Editing { rect, .. } = &mut self.text_tool {
            f(rect);
        }
    }

    /// Access to the mutating edit buffer (used by the app's `TextEdit`).
    pub fn text_buffer_mut(&mut self) -> Option<&mut String> {
        match &mut self.text_tool {
            TextToolState::Editing { text, .. } => Some(text),
            TextToolState::Idle => None,
        }
    }

    /// Clear the one-shot focus-request flag (called right after
    /// `request_focus()`), per egui#5187.
    pub fn clear_just_created(&mut self) {
        if let TextToolState::Editing { just_created, .. } = &mut self.text_tool {
            *just_created = false;
        }
    }

    /// Whether `img_pos` is inside the currently-edited text box.
    pub fn editing_contains(&self, img_pos: Pos2) -> bool {
        matches!(
            self.text_tool,
            TextToolState::Editing { rect, .. } if rect.contains(img_pos)
        )
    }

    /// Submit the in-progress text as an annotation (non-empty commits, empty
    /// is discarded) and return to `Idle`. The text tool stays selected.
    pub fn commit_text(&mut self) {
        if let TextToolState::Editing {
            rect, size, text, ..
        } = &self.text_tool
        {
            let text = text.trim().to_string();
            if !text.is_empty() {
                self.annotations.push(Annotation {
                    tool: ToolKind::Text(text::TextTool::new(rect.left_top(), text, *size)),
                    color: self.color,
                    stroke_width: self.stroke_width,
                });
                self.redo_stack.clear();
            }
        }
        self.text_tool = TextToolState::Idle;
    }

    fn commit(&mut self) {
        if let Some(tool) = self.pending.take() {
            self.annotations.push(Annotation {
                tool,
                color: self.color,
                stroke_width: self.stroke_width,
            });
            self.redo_stack.clear();
        }
        self.drag_start = None;
        self.constraint_angle = None;
    }

    /// Push a ready-made rectangle (used by the Text-border and Redact tools,
    /// which compute their rect from OCR regions rather than a drag gesture).
    /// `filled` selects a solid rectangle; otherwise an outline. Clears the
    /// redo stack like any other committed annotation.
    pub fn push_rect(&mut self, rect: Rect, filled: bool) {
        let tool = if filled {
            ToolKind::FilledRectangle(rect::RectTool::with_filled_rect(rect))
        } else {
            ToolKind::Rectangle(rect::RectTool::with_rect(rect))
        };
        self.annotations.push(Annotation {
            tool,
            color: self.color,
            stroke_width: self.stroke_width,
        });
        self.redo_stack.clear();
    }

    pub fn process_drag(
        &mut self,
        response: &egui::Response,
        screen_to_image: impl Fn(Pos2) -> Pos2,
        snap_image_pos: impl Fn(Pos2) -> Pos2,
    ) {
        // Select mode is handled elsewhere; nothing to annotate.
        if self.active_tool == AnnotationTool::Select {
            return;
        }

        // Eraser: remove the top-most annotation under a click (or drag stop
        // held over the same point). No shape is committed. The canvas senses
        // `Sense::drag()`, so a clean click surfaces as `drag_stopped()` — the
        // `dragged()` flag lingers one frame and must NOT gate this.
        if self.active_tool == AnnotationTool::Eraser {
            if response.drag_stopped() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let img_pos = snap_image_pos(screen_to_image(pos));
                    if let Some(idx) = self.hit_test(img_pos) {
                        self.annotations.remove(idx);
                        self.redo_stack.clear();
                    }
                }
            }
            return;
        }

        // Image-effect tools (pixelate/blur) and the OCR-driven text-region
        // tools are handled at the app level.
        if self.active_tool.is_image_effect()
            || matches!(
                self.active_tool,
                AnnotationTool::HighlightText | AnnotationTool::Redact
            )
        {
            return;
        }

        if self.active_tool == AnnotationTool::Counter {
            self.process_counter_drag(response, &screen_to_image, &snap_image_pos);
            return;
        }

        // Text: a plain click places (or re-places) the interactive text box.
        // The `Sense::drag()` canvas reports a clean click via `drag_stopped()`
        // (the `dragged()` flag lingers one frame, so it must not gate this).
        if self.active_tool == AnnotationTool::Text {
            if response.drag_stopped() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let img_pos = snap_image_pos(screen_to_image(pos));
                    // Clicking inside the current box = editing it (the box's
                    // TextEdit owns that click; nothing to do here).
                    if self.editing_contains(img_pos) {
                        return;
                    }
                    // Clicking away submits any in-progress text (non-empty
                    // commits, empty discards) and immediately starts a new box
                    // at the click location.
                    self.commit_text();
                    self.begin_text_edit(img_pos, self.text_size);
                }
            }
            return;
        }

        // Remaining tools are draw tools producing a persistent shape.
        if !self.active_tool.is_draw_tool() {
            return;
        }

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let img_pos = snap_image_pos(screen_to_image(pos));
                self.drag_start = Some(img_pos);
                self.constraint_angle = None;
                let mut tool = ToolKind::new(self.active_tool);
                tool.start(img_pos);
                self.pending = Some(tool);
            }
        }

        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let img_pos = snap_image_pos(screen_to_image(pos));
                let ctrl = response
                    .ctx
                    .input(|i| i.modifiers.ctrl || i.modifiers.command);
                if let Some(tool) = &mut self.pending {
                    let locked = self.constraint_angle;
                    let r = tool.update(img_pos, ctrl, locked);
                    if r.is_some() {
                        self.constraint_angle = r;
                    }
                }
            }
        }

        if response.drag_stopped() {
            self.commit();
        }
    }

    fn process_counter_drag(
        &mut self,
        response: &egui::Response,
        screen_to_image: &impl Fn(Pos2) -> Pos2,
        snap_image_pos: &impl Fn(Pos2) -> Pos2,
    ) {
        // Placement starts identically for click and drag: press pins the
        // bubble center. A plain click leaves a bubble; dragging extends a
        // callout triangle from the bubble center to the release point.
        let mut placed = false;

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let img_pos = snap_image_pos(screen_to_image(pos));
                self.drag_start = Some(img_pos);
                let mut tool = ToolKind::Counter(counter::CounterTool::new(self.counter));
                tool.start(img_pos);
                self.pending = Some(tool);
            }
        }

        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let img_pos = snap_image_pos(screen_to_image(pos));
                if let Some(tool) = &mut self.pending {
                    tool.update(img_pos, false, None);
                }
            }
        }

        if response.drag_stopped() {
            self.commit();
            placed = true;
        }

        if response.clicked() && !response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let img_pos = snap_image_pos(screen_to_image(pos));
                let mut tool = ToolKind::Counter(counter::CounterTool::new(self.counter));
                tool.start(img_pos);
                self.annotations.push(Annotation {
                    tool,
                    color: self.color,
                    stroke_width: self.stroke_width,
                });
                self.redo_stack.clear();
                placed = true;
            }
        }

        if placed {
            self.counter += 1;
        }
    }

    pub fn draw(&self, painter: &egui::Painter, image_to_screen: impl Fn(Pos2) -> Pos2) {
        for a in &self.annotations {
            a.tool
                .draw(painter, &image_to_screen, a.color, a.stroke_width);
        }
        if let Some(tool) = &self.pending {
            tool.draw(painter, &image_to_screen, self.color, self.stroke_width);
        }
    }

    fn hit_test(&self, pos: Pos2) -> Option<usize> {
        let threshold = 6.0;
        for (i, a) in self.annotations.iter().enumerate().rev() {
            if a.tool.hit_test(pos, threshold, a.stroke_width) {
                return Some(i);
            }
        }
        None
    }

    pub fn render_to_image(&self, image: &mut RgbaImage, offset: Pos2) {
        for a in &self.annotations {
            a.tool.render(image, a.color, a.stroke_width, offset);
        }
    }

    pub fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        let mut horiz = Vec::new();
        let mut vert = Vec::new();
        for a in &self.annotations {
            let (h, v) = a.tool.snap_lines();
            horiz.extend(h);
            vert.extend(v);
        }
        (horiz, vert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_starts_at_one() {
        let state = AnnotationState::default();
        assert_eq!(state.counter, 1);
    }

    #[test]
    fn clear_resets_counter() {
        let mut state = AnnotationState::default();
        state.counter = 42;
        state.clear();
        assert_eq!(state.counter, 1);
    }

    #[test]
    fn text_edit_begin_commit_flow() {
        let mut state = AnnotationState::default();
        assert!(!state.is_text_editing());

        let anchor = Pos2::new(120.0, 60.0);
        state.begin_text_edit(anchor, 20.0);
        assert!(state.is_text_editing());
        let edit = state.text_box_view().unwrap();
        assert_eq!(edit.rect.left_top(), anchor);
        assert_eq!(edit.size, 20.0);
        // Freshly created box requests focus exactly once.
        assert!(state.text_box_view().unwrap().just_created);
        state.clear_just_created();
        assert!(!state.text_box_view().unwrap().just_created);

        // The editing box is a default size and resizable.
        assert!(edit.rect.width() >= 200.0);
        assert!(edit.rect.height() >= 40.0);

        // Simulate typing, resize, and commit.
        edit_buffer_set(&mut state, "Hello\nWorld");
        state.resize_editing_rect(|r| {
            r.max.x = 400.0;
        });
        state.commit_text();
        assert!(!state.is_text_editing());
        assert_eq!(state.annotations.len(), 1);
        // The text annotation's snap lines span its measured box.
        let (h, v) = state.annotations[0].tool.snap_lines();
        assert!(h.contains(&anchor.y));
        assert!(v.contains(&anchor.x));
    }

    #[test]
    fn text_edit_empty_buffer_does_not_commit_annotation() {
        let mut state = AnnotationState::default();
        state.begin_text_edit(Pos2::new(0.0, 0.0), 20.0);
        state.commit_text();
        assert!(!state.is_text_editing());
        assert!(state.annotations.is_empty());
    }

    #[test]
    fn text_edit_escape_submits_nonempty_and_discards_empty() {
        // Non-empty text on Esc-cancel behaves like commit.
        let mut state = AnnotationState::default();
        state.begin_text_edit(Pos2::new(0.0, 0.0), 20.0);
        edit_buffer_set(&mut state, "keep me");
        state.commit_text();
        assert_eq!(state.annotations.len(), 1);
        // Empty text on cancel discards.
        let mut state = AnnotationState::default();
        state.begin_text_edit(Pos2::new(0.0, 0.0), 20.0);
        state.commit_text();
        assert!(state.annotations.is_empty());
    }

    #[test]
    fn text_edit_click_away_commits_then_begins_new_box() {
        let mut state = AnnotationState::default();
        state.begin_text_edit(Pos2::new(10.0, 10.0), 20.0);
        edit_buffer_set(&mut state, "first");
        // Clicking outside the box: earlier box is committed via commit_text,
        // then a new box begins (mirrors the Text branch in process_drag).
        state.commit_text();
        assert_eq!(state.annotations.len(), 1);
        state.begin_text_edit(Pos2::new(300.0, 300.0), 20.0);
        assert!(state.is_text_editing());
        assert!(state.editing_contains(Pos2::new(310.0, 310.0)));
        assert!(!state.editing_contains(Pos2::new(10.0, 10.0)));
    }

    #[test]
    fn text_edit_is_cleared_by_clear() {
        let mut state = AnnotationState::default();
        state.begin_text_edit(Pos2::new(0.0, 0.0), 20.0);
        state.clear();
        assert!(!state.is_text_editing());
    }

    fn edit_buffer_set(state: &mut AnnotationState, text: &str) {
        let buf = state.text_buffer_mut().unwrap();
        *buf = text.to_string();
    }
}
