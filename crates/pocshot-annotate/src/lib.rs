//! UI-toolkit-independent annotation drawing.
//!
//! Shapes are stamped onto an [`image::RgbaImage`] (the exported/pinned image
//! or the live overlay), so the same code backs both the egui build and the
//! Slint build. Geometry comes from `emath`, colours from `ecolor` — both are
//! standalone crates used by egui, not egui itself.

pub mod constrain;
pub mod counter_font;
pub mod effects;
pub mod raster;
pub mod selection;
pub mod shapes;
pub mod tool_kind;

pub use selection::{handle_at, handle_rects, resize, HandleType, HANDLE_SIZE};

pub use ecolor::Color32;
pub use emath::{pos2, vec2, Pos2, Rect, Vec2};
pub use shapes::Shape;
pub use tool_kind::ToolKind;
