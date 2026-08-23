//! Select tool: a no-op annotation. Selecting a region is handled at the app
//! level (see `app::selection_ui`); the tool exists only as the default active
//! mode with no shape of its own.

#[derive(Debug, Clone, Default)]
pub struct SelectTool;

impl super::Tool for SelectTool {
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
