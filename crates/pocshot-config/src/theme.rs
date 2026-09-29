//! Theming: a hand-editable set of named colors, font sizes and geometry.
//!
//! A [`Theme`] is embedded in `config.json` (see [`crate::config::AppSettings`])
//! and defaults to the built-in look. Users may override individual fields;
//! unspecified fields fall back to the built-in values via serde defaults. The
//! "Reload theme" button in the settings panel re-reads it live.

use ecolor::Color32;
use serde::{Deserialize, Serialize};

/// The visual theme. Embedded in `config.json`; every field defaults to the
/// built-in look so editing one value leaves the rest untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default)]
    pub colors: Colors,
    #[serde(default)]
    pub fonts: Fonts,
    #[serde(default)]
    pub geometry: Geometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Colors {
    #[serde(default = "hx::accent", with = "serde_color")]
    pub accent: Color32,
    #[serde(default = "hx::canvas_bg", with = "serde_color")]
    pub canvas_bg: Color32,
    #[serde(default = "hx::overlay_dim", with = "serde_color")]
    pub overlay_dim: Color32,
    #[serde(default = "hx::hint_text", with = "serde_color")]
    pub hint_text: Color32,
    #[serde(default = "hx::toolbar_bg", with = "serde_color")]
    pub toolbar_bg: Color32,
    #[serde(default = "hx::button_bg_disabled", with = "serde_color")]
    pub button_bg_disabled: Color32,
    #[serde(default = "hx::button_bg_hover", with = "serde_color")]
    pub button_bg_hover: Color32,
    #[serde(default = "hx::button_bg_idle", with = "serde_color")]
    pub button_bg_idle: Color32,
    #[serde(default = "hx::icon_fg_active", with = "serde_color")]
    pub icon_fg_active: Color32,
    #[serde(default = "hx::icon_fg_idle", with = "serde_color")]
    pub icon_fg_idle: Color32,
    #[serde(default = "hx::icon_fg_disabled", with = "serde_color")]
    pub icon_fg_disabled: Color32,
    #[serde(default = "hx::text_primary", with = "serde_color")]
    pub text_primary: Color32,
    #[serde(default = "hx::text_muted", with = "serde_color")]
    pub text_muted: Color32,
    #[serde(default = "hx::text_faint", with = "serde_color")]
    pub text_faint: Color32,
    #[serde(default = "hx::settings_bg", with = "serde_color")]
    pub settings_bg: Color32,
    #[serde(default = "hx::selection_border", with = "serde_color")]
    pub selection_border: Color32,
    #[serde(default = "hx::handle_idle", with = "serde_color")]
    pub handle_idle: Color32,
    #[serde(default = "hx::handle_active", with = "serde_color")]
    pub handle_active: Color32,
    #[serde(default = "hx::snap_line", with = "serde_color")]
    pub snap_line: Color32,
    #[serde(default = "hx::snap_line_near", with = "serde_color")]
    pub snap_line_near: Color32,
    #[serde(default = "hx::snap_line_faint", with = "serde_color")]
    pub snap_line_faint: Color32,
    #[serde(default = "hx::ocr_zone", with = "serde_color")]
    pub ocr_zone: Color32,
    #[serde(default = "hx::ocr_label", with = "serde_color")]
    pub ocr_label: Color32,
    #[serde(default = "hx::ocr_box", with = "serde_color")]
    pub ocr_box: Color32,
    #[serde(default = "hx::effect_fill", with = "serde_color")]
    pub effect_fill: Color32,
    #[serde(default = "hx::effect_border", with = "serde_color")]
    pub effect_border: Color32,
    #[serde(default = "hx::tooltip_bg", with = "serde_color")]
    pub tooltip_bg: Color32,
    #[serde(default = "hx::tooltip_border", with = "serde_color")]
    pub tooltip_border: Color32,
    #[serde(default = "hx::default_annotation_color", with = "serde_color")]
    pub default_annotation_color: Color32,
    #[serde(default = "hx::counter_rim", with = "serde_color")]
    pub counter_rim: Color32,
    #[serde(default = "hx::counter_edge", with = "serde_color")]
    pub counter_edge: Color32,
    /// Quick-pick swatches shown in the toolbar group 3 and settings panel.
    #[serde(default = "hx::color_swatches", with = "serde_color_vec")]
    pub color_swatches: Vec<Color32>,
    /// Border/box stroke color for panels (settings, help, tooltip borders).
    #[serde(default = "hx::box_border", with = "serde_color")]
    pub box_border: Color32,
    /// Card background for the settings/help/pin surfaces.
    #[serde(default = "hx::panel_bg", with = "serde_color")]
    pub panel_bg: Color32,
    /// Border of card surfaces.
    #[serde(default = "hx::panel_border", with = "serde_color")]
    pub panel_border: Color32,
    /// Switch/slider track when off.
    #[serde(default = "hx::control_track", with = "serde_color")]
    pub control_track: Color32,
}

impl Default for Colors {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Colors {
    pub fn builtin() -> Self {
        Self {
            accent: hx::accent(),
            canvas_bg: hx::canvas_bg(),
            overlay_dim: hx::overlay_dim(),
            hint_text: hx::hint_text(),
            toolbar_bg: hx::toolbar_bg(),
            button_bg_disabled: hx::button_bg_disabled(),
            button_bg_hover: hx::button_bg_hover(),
            button_bg_idle: hx::button_bg_idle(),
            icon_fg_active: hx::icon_fg_active(),
            icon_fg_idle: hx::icon_fg_idle(),
            icon_fg_disabled: hx::icon_fg_disabled(),
            text_primary: hx::text_primary(),
            text_muted: hx::text_muted(),
            text_faint: hx::text_faint(),
            settings_bg: hx::settings_bg(),
            selection_border: hx::selection_border(),
            handle_idle: hx::handle_idle(),
            handle_active: hx::handle_active(),
            snap_line: hx::snap_line(),
            snap_line_near: hx::snap_line_near(),
            snap_line_faint: hx::snap_line_faint(),
            ocr_zone: hx::ocr_zone(),
            ocr_label: hx::ocr_label(),
            ocr_box: hx::ocr_box(),
            effect_fill: hx::effect_fill(),
            effect_border: hx::effect_border(),
            tooltip_bg: hx::tooltip_bg(),
            tooltip_border: hx::tooltip_border(),
            default_annotation_color: hx::default_annotation_color(),
            counter_rim: hx::counter_rim(),
            counter_edge: hx::counter_edge(),
            color_swatches: hx::color_swatches(),
            box_border: hx::box_border(),
            panel_bg: hx::panel_bg(),
            panel_border: hx::panel_border(),
            control_track: hx::control_track(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fonts {
    #[serde(default)]
    pub hint: f32,
    #[serde(default)]
    pub icon: f32,
    #[serde(default)]
    pub help_title: f32,
    #[serde(default)]
    pub help_row: f32,
    #[serde(default)]
    pub help_footer: f32,
    #[serde(default)]
    pub status: f32,
    #[serde(default)]
    pub size_readout: f32,
    #[serde(default)]
    pub ocr_label_small: f32,
    #[serde(default)]
    pub ocr_label: f32,
    #[serde(default)]
    pub slider_value: f32,
    #[serde(default)]
    pub annotation_text: f32,
    #[serde(default)]
    pub settings_heading: f32,
    #[serde(default)]
    pub settings_section: f32,
    #[serde(default)]
    pub settings_label: f32,
    #[serde(default)]
    pub settings_helper: f32,
    #[serde(default)]
    pub panel_title: f32,
    #[serde(default)]
    pub section: f32,
    #[serde(default)]
    pub row: f32,
    #[serde(default)]
    pub shortcut: f32,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Fonts {
    pub fn builtin() -> Self {
        Self {
            hint: 18.0,
            icon: 15.0,
            help_title: 15.0,
            help_row: 13.0,
            help_footer: 12.0,
            status: 12.0,
            size_readout: 14.0,
            ocr_label_small: 9.0,
            ocr_label: 10.0,
            slider_value: 10.0,
            annotation_text: 18.0,
            settings_heading: 14.0,
            settings_section: 12.0,
            settings_label: 11.0,
            settings_helper: 10.0,
            panel_title: 15.0,
            section: 12.0,
            row: 13.0,
            shortcut: 11.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    #[serde(default)]
    pub toolbar_margin: f32,
    #[serde(default)]
    pub button_w: f32,
    #[serde(default)]
    pub button_h: f32,
    #[serde(default)]
    pub button_gap: f32,
    #[serde(default)]
    pub button_pad: f32,
    #[serde(default)]
    pub g1_w_h: f32,
    #[serde(default)]
    pub g1_h_v: f32,
    #[serde(default)]
    pub g2_w_h: f32,
    #[serde(default)]
    pub g2_h_v: f32,
    #[serde(default)]
    pub g3_w_h: f32,
    #[serde(default)]
    pub g3_h_v: f32,
    #[serde(default)]
    pub button_radius: f32,
    #[serde(default)]
    pub box_radius: f32,
    #[serde(default)]
    pub settings_radius: f32,
    #[serde(default)]
    pub help_radius: f32,
    #[serde(default)]
    pub swatch_size: f32,
    #[serde(default)]
    pub swatch_gap: f32,
    #[serde(default)]
    pub settings_swatch_gap: f32,
    #[serde(default)]
    pub counter_bubble_radius: f32,
    #[serde(default)]
    pub snap_distance: f32,
    #[serde(default)]
    pub default_stroke_width: f32,
    /// Lucide icon edge length in logical px.
    #[serde(default)]
    pub icon_size: f32,
    /// Lucide stroke width (24px grid units).
    #[serde(default)]
    pub icon_stroke: f32,
    /// Card corner radius.
    #[serde(default)]
    pub panel_radius: f32,
    /// Switch/slider corner radius.
    #[serde(default)]
    pub control_radius: f32,
    /// Toolbar inner padding.
    #[serde(default)]
    pub toolbar_padding: f32,
}

impl Default for Geometry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Geometry {
    pub fn builtin() -> Self {
        Self {
            toolbar_margin: 6.0,
            button_w: 34.0,
            button_h: 28.0,
            button_gap: 4.0,
            button_pad: 6.0,
            g1_w_h: 392.0,
            g1_h_v: 332.0,
            g2_w_h: 502.0,
            g2_h_v: 424.0,
            g3_w_h: 152.0,
            g3_h_v: 152.0,
            button_radius: 4.0,
            box_radius: 6.0,
            settings_radius: 8.0,
            help_radius: 10.0,
            swatch_size: 20.0,
            swatch_gap: 4.0,
            settings_swatch_gap: 6.0,
            counter_bubble_radius: 18.0,
            snap_distance: 8.0,
            default_stroke_width: 3.0,
            icon_size: 18.0,
            icon_stroke: 1.75,
            panel_radius: 12.0,
            control_radius: 8.0,
            toolbar_padding: 8.0,
        }
    }
}

impl Theme {
    /// The default look, matching the app's original hardcoded values.
    pub fn builtin() -> Self {
        Self {
            colors: Colors::builtin(),
            fonts: Fonts::builtin(),
            geometry: Geometry::builtin(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::builtin()
    }
}

/// The built-in color values as plain functions so they can serve as serde
/// `default = "..."` paths and as the source of truth for `Colors::builtin`.
mod hx {
    use super::*;

    pub fn accent() -> Color32 {
        Color32::from_rgb(66, 153, 225)
    }
    pub fn canvas_bg() -> Color32 {
        Color32::BLACK
    }
    pub fn overlay_dim() -> Color32 {
        Color32::from_rgba_unmultiplied(0, 0, 0, 80)
    }
    pub fn hint_text() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 200)
    }
    pub fn toolbar_bg() -> Color32 {
        Color32::from_black_alpha(190)
    }
    pub fn button_bg_disabled() -> Color32 {
        Color32::from_rgb(30, 30, 30)
    }
    pub fn button_bg_hover() -> Color32 {
        Color32::from_rgb(60, 60, 60)
    }
    pub fn button_bg_idle() -> Color32 {
        Color32::from_rgb(40, 40, 40)
    }
    pub fn icon_fg_active() -> Color32 {
        Color32::WHITE
    }
    pub fn icon_fg_idle() -> Color32 {
        Color32::LIGHT_GRAY
    }
    pub fn icon_fg_disabled() -> Color32 {
        Color32::from_rgb(80, 80, 80)
    }
    pub fn text_primary() -> Color32 {
        Color32::WHITE
    }
    pub fn text_muted() -> Color32 {
        Color32::LIGHT_GRAY
    }
    pub fn text_faint() -> Color32 {
        Color32::GRAY
    }
    pub fn settings_bg() -> Color32 {
        Color32::from_rgb(24, 24, 28)
    }
    pub fn selection_border() -> Color32 {
        Color32::from_rgb(66, 153, 225)
    }
    pub fn handle_idle() -> Color32 {
        Color32::from_rgb(66, 153, 225)
    }
    pub fn handle_active() -> Color32 {
        Color32::from_rgb(99, 179, 237)
    }
    pub fn snap_line() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 165, 0, 110)
    }
    pub fn snap_line_near() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 165, 0, 230)
    }
    pub fn snap_line_faint() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 165, 0, 80)
    }
    pub fn ocr_zone() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 200, 0, 40)
    }
    pub fn ocr_label() -> Color32 {
        Color32::from_rgb(255, 220, 100)
    }
    pub fn ocr_box() -> Color32 {
        Color32::from_rgb(34, 211, 238)
    }
    pub fn effect_fill() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 36)
    }
    pub fn effect_border() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 220)
    }
    pub fn tooltip_bg() -> Color32 {
        Color32::from_rgb(30, 30, 30)
    }
    pub fn tooltip_border() -> Color32 {
        Color32::from_rgb(80, 80, 80)
    }
    pub fn default_annotation_color() -> Color32 {
        Color32::from_rgb(239, 68, 68)
    }
    pub fn counter_rim() -> Color32 {
        Color32::WHITE
    }
    pub fn counter_edge() -> Color32 {
        Color32::from_rgb(20, 20, 20)
    }
    pub fn box_border() -> Color32 {
        Color32::from_rgb(70, 70, 70)
    }
    pub fn panel_bg() -> Color32 {
        Color32::from_rgb(0x1b, 0x1b, 0x1f)
    }
    pub fn panel_border() -> Color32 {
        Color32::from_rgb(0x3a, 0x3a, 0x42)
    }
    pub fn control_track() -> Color32 {
        Color32::from_rgb(0x3a, 0x3a, 0x42)
    }
    pub fn color_swatches() -> Vec<Color32> {
        vec![
            Color32::from_rgb(239, 68, 68),
            Color32::from_rgb(250, 204, 21),
            Color32::from_rgb(34, 197, 94),
            Color32::from_rgb(59, 130, 246),
            Color32::WHITE,
            Color32::from_rgb(15, 23, 42),
        ]
    }
}

/// Parse a `"#rrggbb"` / `"#rrggbbaa"` (or bare, no `#`) string into a
/// `Color32`, falling back to `default` on malformed input so a bad field
/// never breaks the app. Six digits imply fully opaque.
pub fn color32_from_hex(s: &str, default: Color32) -> Color32 {
    let s = s.trim().trim_start_matches('#');
    let hex = |r: &str| u8::from_str_radix(r, 16).ok();
    match s.len() {
        6 => match (hex(&s[0..2]), hex(&s[2..4]), hex(&s[4..6])) {
            (Some(r), Some(g), Some(b)) => Color32::from_rgb(r, g, b),
            _ => default,
        },
        8 => match (hex(&s[0..2]), hex(&s[2..4]), hex(&s[4..6]), hex(&s[6..8])) {
            (Some(r), Some(g), Some(b), Some(a)) => Color32::from_rgba_unmultiplied(r, g, b, a),
            _ => default,
        },
        _ => default,
    }
}

/// Format a `Color32` as a `"#rrggbb"` string (or `"#rrggbbaa"` when it has
/// transparency) for hand editing. `Color32` stores premultiplied channels,
/// so RGB is un-premultiplied by alpha to round-trip cleanly with
/// [`color32_from_hex`].
pub fn color32_to_hex(c: &Color32) -> String {
    let a = c.a();
    let unmult = |v: u8| {
        if a == 0 {
            0
        } else {
            ((v as u16 * 255) / a as u16).min(255) as u8
        }
    };
    if a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            unmult(c.r()),
            unmult(c.g()),
            unmult(c.b()),
            a
        )
    }
}

/// Serde adapter: a single `Color32` <-> `"#rrggbb"` hex string.
pub mod serde_color {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &Color32, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&color32_to_hex(c))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color32, D::Error> {
        let s = String::deserialize(d)
            .map_err(|_| serde::de::Error::custom("expected a color string"))?;
        Ok(color32_from_hex(&s, hx::accent()))
    }
}

/// Serde adapter: a `Vec<Color32>` <-> array of `"#rrggbb"` hex strings.
pub mod serde_color_vec {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(colors: &[Color32], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(colors.len()))?;
        for c in colors {
            seq.serialize_element(&color32_to_hex(c))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Color32>, D::Error> {
        let items = Vec::<String>::deserialize(d)?;
        Ok(items
            .into_iter()
            .map(|s| color32_from_hex(&s, hx::default_annotation_color()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_matches_default_and_is_stable() {
        assert_eq!(Theme::builtin(), Theme::default());
        assert_eq!(
            Theme::builtin().colors.accent,
            Color32::from_rgb(66, 153, 225)
        );
        assert_eq!(Theme::builtin().fonts.icon, 15.0);
        assert_eq!(Theme::builtin().geometry.g2_w_h, 502.0);
    }

    #[test]
    fn hex_roundtrip() {
        let c = Color32::from_rgb(0x42, 0x99, 0xe1);
        assert_eq!(color32_to_hex(&c), "#4299e1");
        assert_eq!(color32_from_hex("#4299e1", Color32::BLACK), c);
        assert_eq!(color32_from_hex("4299e1", Color32::BLACK), c);
        let t = Color32::from_rgba_unmultiplied(0, 0, 0, 80);
        assert_eq!(color32_to_hex(&t), "#00000050");
        assert_eq!(color32_from_hex("#00000050", Color32::BLACK), t);
    }

    #[test]
    fn invalid_hex_falls_back() {
        assert_eq!(
            color32_from_hex("nope", Color32::from_rgb(66, 153, 225)),
            Color32::from_rgb(66, 153, 225)
        );
    }

    #[test]
    fn partial_theme_fills_from_builtin() {
        let json = r##"{
            "colors": { "accent": "#00ff00" }
        }"##;
        let t: Theme = serde_json::from_str(json).unwrap();
        assert_eq!(t.colors.accent, Color32::from_rgb(0, 255, 0));
        assert_eq!(t.colors.canvas_bg, Theme::builtin().colors.canvas_bg);
        assert_eq!(t.fonts.hint, Theme::builtin().fonts.hint);
    }

    #[test]
    fn theme_serde_roundtrip() {
        let t = Theme::builtin();
        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(
            t,
            back,
            "theme roundtrip mismatch:\n{}",
            diff_theme(&t, &back)
        );
    }

    #[test]
    fn swatches_vec_serde_roundtrip() {
        let v = vec![Color32::RED, Color32::GREEN, Color32::BLUE];
        let json = serde_json::to_string(&SerdeColorVecHolder(v.clone())).unwrap();
        let back = serde_json::from_str::<SerdeColorVecHolder>(&json).unwrap();
        assert_eq!(back.0, v);
    }

    #[test]
    fn lucide_theme_fields_have_defaults_and_roundtrip() {
        let t = Theme::builtin();
        assert_eq!(t.geometry.icon_size, 18.0);
        assert_eq!(t.geometry.icon_stroke, 1.75);
        assert_eq!(t.geometry.panel_radius, 12.0);
        assert_eq!(t.geometry.control_radius, 8.0);
        assert_eq!(t.geometry.toolbar_padding, 8.0);
        assert_eq!(t.fonts.panel_title, 15.0);
        assert_eq!(t.fonts.section, 12.0);
        assert_eq!(t.fonts.row, 13.0);
        assert_eq!(t.fonts.shortcut, 11.0);
        assert_eq!(color32_to_hex(&t.colors.panel_bg), "#1b1b1f");
        assert_eq!(color32_to_hex(&t.colors.panel_border), "#3a3a42");
        assert_eq!(color32_to_hex(&t.colors.control_track), "#3a3a42");

        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);

        // A legacy theme JSON missing the new keys still loads with defaults.
        let legacy = r##"{ "colors": { "accent": "#00ff00" } }"##;
        let partial: Theme = serde_json::from_str(legacy).unwrap();
        assert_eq!(partial.geometry.icon_size, 18.0);
        assert_eq!(partial.colors.panel_border, Theme::builtin().colors.panel_border);
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct SerdeColorVecHolder(#[serde(with = "super::serde_color_vec")] Vec<Color32>);

    #[allow(clippy::type_complexity)]
    fn diff_theme(a: &Theme, b: &Theme) -> String {
        let mut out = String::new();
        let (ac, bc) = (&a.colors, &b.colors);
        let (af, bf) = (&a.fonts, &b.fonts);
        let (ag, bg) = (&a.geometry, &b.geometry);
        for (k, x, y) in [
            ("accent", ac.accent, bc.accent),
            ("canvas_bg", ac.canvas_bg, bc.canvas_bg),
            ("overlay_dim", ac.overlay_dim, bc.overlay_dim),
            ("snap_line", ac.snap_line, bc.snap_line),
        ] {
            if x != y {
                out.push_str(&format!("  colors.{k}: {x:?} != {y:?}\n"));
            }
        }
        if ac.color_swatches != bc.color_swatches {
            out.push_str(&format!(
                "  colors.color_swatches: {:?} != {:?}\n",
                ac.color_swatches, bc.color_swatches
            ));
        }
        for (k, x, y) in [
            ("fonts.hint", af.hint, bf.hint),
            ("fonts.icon", af.icon, bf.icon),
            ("geom.button_w", ag.button_w, bg.button_w),
            ("geom.g2_w_h", ag.g2_w_h, bg.g2_w_h),
        ] {
            if x != y {
                out.push_str(&format!("  {k}: {x} != {y}\n"));
            }
        }
        if out.is_empty() {
            out.push_str("  (no diff found for sampled fields)\n");
        }
        out
    }
}
