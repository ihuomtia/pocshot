//! The selectable annotation tools. Mirrors `pocshot_gui::annotation::AnnotationTool`
//! without any egui dependency.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
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
    HighlightText,
    Redact,
    Eraser,
}

impl ToolKind {
    /// Whether Ctrl constrains a drag of this tool to a square/circle/45°.
    pub fn constrained_by_ctrl(self) -> bool {
        matches!(
            self,
            ToolKind::Rectangle
                | ToolKind::FilledRectangle
                | ToolKind::Circle
                | ToolKind::FilledCircle
                | ToolKind::Line
                | ToolKind::Arrow
                | ToolKind::Pen
                | ToolKind::Highlighter
        )
    }
}
