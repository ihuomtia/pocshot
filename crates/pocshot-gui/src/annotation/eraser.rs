//! Eraser tool: a click removes the top-most annotation under the cursor.
//! The actual removal is done at the state level (it needs the annotation
//! list); this struct is a stateless marker used for the active-tool match.

#[derive(Debug, Clone, Default)]
pub struct EraserTool;

impl super::Tool for EraserTool {
    fn start(&mut self, _pos: eframe::egui::Pos2) {}

    fn update(
        &mut self,
        _pos: eframe::egui::Pos2,
        _ctrl: bool,
        _locked_angle: Option<f32>,
    ) -> Option<f32> {
        None
    }

    fn draw(
        &self,
        _painter: &eframe::egui::Painter,
        _map: &dyn Fn(eframe::egui::Pos2) -> eframe::egui::Pos2,
        _color: eframe::egui::Color32,
        _width: f32,
    ) {
    }

    fn render(
        &self,
        _image: &mut image::RgbaImage,
        _color: eframe::egui::Color32,
        _width: f32,
        _offset: eframe::egui::Pos2,
    ) {
    }

    fn hit_test(&self, _pos: eframe::egui::Pos2, _threshold: f32, _width: f32) -> bool {
        false
    }

    fn snap_lines(&self) -> (Vec<f32>, Vec<f32>) {
        (Vec::new(), Vec::new())
    }
}
