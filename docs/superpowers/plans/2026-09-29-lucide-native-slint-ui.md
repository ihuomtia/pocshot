# Lucide-native Slint UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the Slint editor UI to and beyond egui's maturity, using vendored Lucide icons and a single adaptive floating toolbar, without regressing software-renderer performance.

**Architecture:** The Slint view (`ui/*.slint`) stays dumb and forwards events; `src/editor.rs` owns state and layout. A new vendored `ui/icons.slint` holds the Lucide icon data and `IconDisplay`; `ui/controls.slint` holds reusable `IconButton`/`Switch`/`ValueSlider` widgets; `main.slint` composes them. Toolbar geometry moves into a pure, unit-tested `src/layout.rs`. The shared `Theme` gains serde-defaulted fields in **both** copies (`pocshot-config` and `pocshot-gui`) so one `config.json` keeps driving both UIs.

**Tech Stack:** Rust 2024, Slint 1.18 (`renderer-software`), `pocshot-config` (`ecolor`), `pocshot-gui` (`eframe::egui`), Python 3 (one-off icon extraction).

**Spec:** `docs/superpowers/specs/2026-09-29-lucide-native-slint-ui-design.md`

## Global Constraints

- Slint `1.18`, Rust edition `2024`; no new runtime dependencies.
- Icons are **vendored** into `crates/pocshot-ui-slint/ui/icons.slint`; do **not** add `lucide-slint` as a dependency. Generated from lucide-slint `1.48.0`.
- Lucide artwork is ISC-licensed; keep the attribution header in `icons.slint`.
- Every new `Theme` field must be added to **both** `crates/pocshot-config/src/theme.rs` and `crates/pocshot-gui/src/theme.rs`, with a serde default, or egui's save will drop it.
- Do not change egui rendering behaviour.
- The software renderer (`POCSHOT_RENDERER=software`, Slint `winit-software`) must remain usable; icons are static `Path`s and must not be animated.
- Toolbar placement uses the **whole viewport** as bounds, not the letterboxed image rect (clipboard images are shown at native size).
- `cargo clippy -p pocshot-ui-slint -p pocshot-config` must be clean at the end.

## Review Focus

Uncovered input classes most likely to bite a user, each pinned to a test below:

1. **Narrow / portrait window** — toolbar must wrap to two rows and stay fully on-screen, never overlap the hint bar. → `layout.rs::wraps_when_too_narrow`, `never_off_screen`.
2. **Selection hugging the top edge** — toolbar flips below the selection and its tooltips must flip below too. → `layout.rs::tooltips_flip_below_near_top`.
3. **Selection hugged by the bottom edge / tiny 1px selection at a corner** — toolbar flips above and clamps. → `layout.rs::flips_above_near_bottom`, `clamps_into_window`.
4. **Clipboard image narrower than the window** (letterboxed, native size) — toolbar bounds are the viewport, so it must not be pushed into the letterbox. → manual checklist in Task 7; `layout.rs::uses_viewport_bounds`.
5. **Undo/Redo empty stacks** — buttons render disabled, ignore clicks, and do not panic. → Task 4 manual checklist + `ToolbarLayout` has no dependency on history.
6. **Pin resized below the minimum** — clamp to 16×16 rather than provoking a zero-size window. → `Task 2::clamp_pin_size`.
7. **Extraction script cannot find lucide-slint or an icon name** — fail loudly, never emit a partial file. → `Task 2` script assertion + explicit error.

---

### Task 1: Extend the shared theme (both copies)

**Files:**
- Modify: `crates/pocshot-config/src/theme.rs`
- Modify: `crates/pocshot-gui/src/theme.rs`
- Test: same files (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: new fields on `Colors` (`panel_bg`, `panel_border`, `control_track`), `Geometry` (`icon_size`, `icon_stroke`, `panel_radius`, `control_radius`, `toolbar_padding`), `Fonts` (`panel_title`, `section`, `row`, `shortcut`) in both crates.

- [ ] **Step 1: Write the failing test** in `crates/pocshot-config/src/theme.rs` tests module:

```rust
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
    assert_eq!(color32_to_hex(&t.colors.panel_bg), "#1b1b1ff2");
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
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p pocshot-config lucide_theme_fields_have_defaults_and_roundtrip`
Expected: FAIL — no field `icon_size`.

- [ ] **Step 3: Implement the fields in `crates/pocshot-config/src/theme.rs`**

Add to `Colors` (after `box_border`):

```rust
    /// Card background for the settings/help/pin surfaces.
    #[serde(default = "hx::panel_bg", with = "serde_color")]
    pub panel_bg: Color32,
    /// Border of card surfaces.
    #[serde(default = "hx::panel_border", with = "serde_color")]
    pub panel_border: Color32,
    /// Switch/slider track when off.
    #[serde(default = "hx::control_track", with = "serde_color")]
    pub control_track: Color32,
```

Add to `Colors::builtin()` before the closing brace:

```rust
            panel_bg: hx::panel_bg(),
            panel_border: hx::panel_border(),
            control_track: hx::control_track(),
```

Add to `mod hx`:

```rust
    pub fn panel_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(0x1b, 0x1b, 0x1f, 0xf2)
    }
    pub fn panel_border() -> Color32 {
        Color32::from_rgb(0x3a, 0x3a, 0x42)
    }
    pub fn control_track() -> Color32 {
        Color32::from_rgb(0x3a, 0x3a, 0x42)
    }
```

Add to `Geometry` (after `default_stroke_width`):

```rust
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
```

Add to `Geometry::builtin()` before the closing brace:

```rust
            icon_size: 18.0,
            icon_stroke: 1.75,
            panel_radius: 12.0,
            control_radius: 8.0,
            toolbar_padding: 8.0,
```

Add to `Fonts` (after `settings_helper`):

```rust
    #[serde(default)]
    pub panel_title: f32,
    #[serde(default)]
    pub section: f32,
    #[serde(default)]
    pub row: f32,
    #[serde(default)]
    pub shortcut: f32,
```

Add to `Fonts::builtin()` before the closing brace:

```rust
            panel_title: 15.0,
            section: 12.0,
            row: 13.0,
            shortcut: 11.0,
```

- [ ] **Step 4: Run the config test**

Run: `cargo test -p pocshot-config`
Expected: PASS (all existing theme tests still pass).

- [ ] **Step 5: Mirror the exact same additions in `crates/pocshot-gui/src/theme.rs`**

`pocshot-gui`'s copy uses `eframe::egui::Color32` instead of `ecolor::Color32`; the field names, defaults, doc comments and `hx` helpers are otherwise identical. Add the same three `Colors` fields, five `Geometry` fields and four `Fonts` fields, the same `builtin()` entries, and the same `hx::panel_bg`/`panel_border`/`control_track` functions (using `eframe::egui::Color32::from_rgba_unmultiplied` / `from_rgb`).

- [ ] **Step 6: Build both crates and commit**

Run: `cargo build -p pocshot-config -p pocshot-gui`
Expected: clean build.

```bash
git add crates/pocshot-config/src/theme.rs crates/pocshot-gui/src/theme.rs
git commit -m "feat(config): theme fields for the Lucide-native UI"
```

---

### Task 2: Vendor Lucide icons + controls, and rebuild the pin window

**Files:**
- Create: `scripts/extract_lucide.py`
- Create: `crates/pocshot-ui-slint/ui/icons.slint` (generated)
- Create: `crates/pocshot-ui-slint/ui/theme.slint`
- Create: `crates/pocshot-ui-slint/ui/controls.slint`
- Modify: `crates/pocshot-ui-slint/ui/main.slint` (drop its `Theme` global; PinWindow only)
- Modify: `crates/pocshot-ui-slint/src/lib.rs` (`run_pin`, new `clamp_pin_size`)
- Test: `crates/pocshot-ui-slint/src/lib.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: the new `Theme` fields from Task 1.
- Produces:
  - `Theme` global in `ui/theme.slint` (imported by `controls.slint` and `main.slint`; still reachable from Rust via `ui.global::<Theme>()`).
  - `Icons` global and `IconDisplay { icon: Icon; size: length; stroke: brush; stroke-width: float; }`.
  - `IconButton { icon; tooltip; shortcut; active; enabled; button-width; tooltip-below; callback clicked; }`.
  - `Switch { checked: bool; label: string; callback toggled; }`.
  - `ValueSlider { value: float; min: float; max: float; step: float; callback changed(float); }`.
  - `Sep { }` vertical separator.
  - `Swatch { fill: color; active: bool; callback clicked; }`.
  - `SectionLabel { icon: Icon; text: string; }`.
  - `export struct HelpRow { keys: string; text: string; }` in `controls.slint`.
  - `PinWindow { shot: image; callback dismiss; callback resize(length, length); }`.
  - `pub fn clamp_pin_size(width: f32, height: f32) -> (f32, f32)` in `lib.rs`.

**Ordering note:** this task creates `theme.slint` (Step 7) **before** `controls.slint` (Step 8), because `controls.slint` imports `Theme`. The Theme global is deliberately a separate file so no later task has to redefine it.

- [ ] **Step 1: Write the failing test** in `crates/pocshot-ui-slint/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_pin_size_enforces_minimum_and_integral() {
        assert_eq!(clamp_pin_size(-10.0, 0.0), (16.0, 16.0));
        assert_eq!(clamp_pin_size(120.4, 80.6), (120.0, 81.0));
        assert_eq!(clamp_pin_size(f32::INFINITY, f32::NAN), (16.0, 16.0));
    }
}
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p pocshot-ui-slint clamp_pin_size`
Expected: FAIL — `clamp_pin_size` not found.

- [ ] **Step 3: Implement `clamp_pin_size`** in `crates/pocshot-ui-slint/src/lib.rs`:

```rust
/// Clamp a requested pin-window size to a sane, integral pixel value.
pub fn clamp_pin_size(width: f32, height: f32) -> (f32, f32) {
    let fix = |v: f32| {
        if v.is_finite() {
            v.max(16.0).round()
        } else {
            16.0
        }
    };
    (fix(width), fix(height))
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p pocshot-ui-slint clamp_pin_size`
Expected: PASS.

- [ ] **Step 5: Write `scripts/extract_lucide.py`** (the validated extractor; it locates lucide-slint in the cargo registry, so it needs no network):

```python
#!/usr/bin/env python3
"""Generate crates/pocshot-ui-slint/ui/icons.slint from lucide-slint.

Run:  python3 scripts/extract_lucide.py
The lucide-slint crate must be present in the cargo registry (e.g. after
`cargo info lucide-slint` or `cargo add lucide-slint --build` followed by
`cargo fetch`). Fails loudly if the crate or any icon is missing; never emits
a partial file.
"""
import glob
import os
import re
import sys

WANTED = [
    "MousePointer2", "Square", "Slash", "ArrowUpRight", "Circle", "Pen",
    "Highlighter", "EyeOff", "Hash", "Type", "Waves", "Grid3x3", "Eraser",
    "BoxSelect", "RotateCw", "Download", "Copy", "Undo2", "Redo2", "Pin",
    "ScanText", "Trash2", "Maximize", "Settings", "CircleHelp", "X",
    "Minus", "Plus", "Check",
]

ICONDISPLAY = '''export component IconDisplay inherits Rectangle {
    in property <Icon> icon;
    in-out property <length> size: 24px;
    in-out property <brush> stroke: #fff;
    in-out property <brush> stroke-fill: transparent;
    in-out property <float> stroke-width: 2.0;
    in-out property <bool> absolute-stroke-width: false;
    out property <length> calculated-stroke-width: absolute-stroke-width ? (stroke-width * 1px) : (size / 24 * stroke-width);

    width: size;
    height: size;
    clip: false;

    for path in icon.paths: Path {
        x: 0;
        y: 0;
        viewbox-x: path.viewbox-x;
        viewbox-y: path.viewbox-y;
        viewbox-width: path.viewbox-width;
        viewbox-height: path.viewbox-height;
        commands: path.command;
        stroke: parent.stroke;
        fill: path.has_fill ? parent.stroke : parent.stroke-fill;
        stroke-width: parent.calculated-stroke-width;
        stroke-line-cap: round;
        stroke-line-join: round;
    }
}
'''


def find_source():
    home = os.path.expanduser("~")
    pattern = os.path.join(home, ".cargo", "registry", "src", "*", "lucide-slint-*", "lucide.slint")
    matches = sorted(glob.glob(pattern))
    if not matches:
        sys.exit(f"lucide-slint not found under {pattern}; run `cargo info lucide-slint` first")
    return matches[-1]


def block(text, name):
    m = re.search(r"out property <Icon> " + re.escape(name) + r": \{(.*?)\n    \};", text, re.S)
    if not m:
        sys.exit(f"icon not found in lucide-slint: {name}")
    return m.group(1).strip("\n")


def main():
    src = find_source()
    text = open(src, encoding="utf-8").read()
    out = [
        "// Generated by scripts/extract_lucide.py from lucide-slint 1.48.0.",
        "// Lucide icons are ISC licensed; lucide-slint is MIT OR Apache-2.0.",
        "// Do not edit by hand: re-run the script to update.",
        "",
        "export struct PathElem {",
        "    viewbox-x: float,",
        "    viewbox-y: float,",
        "    viewbox-width: float,",
        "    viewbox-height: float,",
        "",
        "    command: string,",
        "    has_fill: bool,",
        "}",
        "",
        "export struct Icon {",
        "    paths: [PathElem],",
        "}",
        "",
        "export global Icons {",
    ]
    for name in WANTED:
        out.append(f"    out property <Icon> {name}: {{")
        out.append("    " + block(text, name))
        out.append("    };")
        out.append("")
    out += [
        "    // Custom filled variants. Lucide's Square/Circle have has_fill:false,",
        "    // so IconDisplay cannot fill them via stroke-fill.",
        "    out property <Icon> SquareFill: {",
        "        paths: [",
        "            { viewbox-x: 0, viewbox-y: 0, viewbox-width: 24, viewbox-height: 24,",
        '              command: "M 4 4 L 20 4 L 20 20 L 4 20 Z", has_fill: true },',
        "        ],",
        "    };",
        "",
        "    out property <Icon> CircleFill: {",
        "        paths: [",
        "            { viewbox-x: 0, viewbox-y: 0, viewbox-width: 24, viewbox-height: 24,",
        '              command: "M 12 3 C 7.02944 3 3 7.02944 3 12 C 3 16.9706 7.02944 21 12 21 C 16.9706 21 21 16.9706 21 12 C 21 7.02944 16.9706 3 12 3 Z", has_fill: true },',
        "        ],",
        "    };",
        "}",
        "",
        ICONDISPLAY,
    ]
    dest = os.path.join(os.path.dirname(__file__), "..", "crates", "pocshot-ui-slint", "ui", "icons.slint")
    dest = os.path.normpath(dest)
    result = "\n".join(out)
    assert "out property <Icon> SquareFill:" in result
    with open(dest, "w", encoding="utf-8") as fh:
        fh.write(result)
    print(f"wrote {dest}: {len(result)} bytes")


if __name__ == "__main__":
    main()
```

- [ ] **Step 6: Generate the icon file**

Run: `python3 scripts/extract_lucide.py`
Expected: `wrote .../ui/icons.slint: ~21000 bytes`. Then confirm: `grep -c "out property <Icon>" crates/pocshot-ui-slint/ui/icons.slint` → `31`.

- [ ] **Step 7: Write `crates/pocshot-ui-slint/ui/theme.slint`**

The `Theme` global moves out of `main.slint` into its own file so `controls.slint` and `main.slint` share it without a definition-order problem. Values are pushed from Rust in `apply_theme`; the defaults here match `Theme::builtin`:

```slint
export global Theme {
    in-out property <color> accent: #4299e1;
    in-out property <color> chip-bg: #282828;
    in-out property <color> chip-hover: #3c3c3c;
    in-out property <color> chip-border: #1e1e1e;
    in-out property <color> icon-fg-idle: #d3d3d3;
    in-out property <color> icon-fg-active: #ffffff;
    in-out property <color> icon-fg-disabled: #505050;
    in-out property <color> toolbar-bg: #000000d8;
    in-out property <color> hint-text: #ffffffdd;
    in-out property <color> text-faint: #808080;
    in-out property <color> dim: #00000066;
    in-out property <color> sel-border: #4299e1;
    in-out property <color> handle: #4299e1;
    in-out property <color> canvas-bg: #101010;
    in-out property <color> tooltip-bg: #1e1e1e;
    in-out property <color> tooltip-border: #505050;
    in-out property <color> panel-bg: #1b1b1ff2;
    in-out property <color> panel-border: #3a3a42;
    in-out property <color> control-track: #3a3a42;
    in-out property <color> swatch0: #ff3b30;
    in-out property <color> swatch1: #ff9500;
    in-out property <color> swatch2: #ffd60a;
    in-out property <color> swatch3: #34c759;
    in-out property <color> swatch4: #0a84ff;
    in-out property <color> swatch5: #ff2d95;
    in-out property <length> icon-size: 18px;
    in-out property <float> icon-stroke: 1.75;
    in-out property <float> row-font: 13;
    in-out property <float> section-font: 12;
    in-out property <float> shortcut-font: 11;
}
```

- [ ] **Step 8: Write `crates/pocshot-ui-slint/ui/controls.slint`**

```slint
// Reusable Lucide-native controls. View-only: all state lives in Rust.
import { Icon, IconDisplay, Icons } from "icons.slint";
import { Theme } from "theme.slint";

/// A row of the help overlay.
export struct HelpRow {
    keys: string,
    text: string,
}

/// A square icon button with hover/active/disabled states and a tooltip.
export component IconButton inherits Rectangle {
    in property <Icon> icon;
    in property <string> tooltip: "";
    in property <string> shortcut: "";
    in property <bool> active: false;
    in property <bool> enabled: true;
    in property <length> button-width: 32px;
    in property <bool> tooltip-below: false;
    callback clicked;

    width: root.button-width;
    height: 32px;
    border-radius: 8px;
    background: !root.enabled ? Theme.control-track
        : root.active ? Theme.accent
        : ta.has-hover ? Theme.chip-hover
        : Theme.chip-bg;

    IconDisplay {
        icon: root.icon;
        width: 100%;
        height: 100%;
        size: Theme.icon-size;
        stroke-width: Theme.icon-stroke;
        stroke: !root.enabled ? Theme.icon-fg-disabled
            : (root.active || ta.has-hover) ? Theme.icon-fg-active
            : Theme.icon-fg-idle;
        horizontal-alignment: center;
        vertical-alignment: center;
    }

    if (ta.has-hover && root.enabled && root.tooltip != "") : Tooltip {
        label: root.tooltip;
        shortcut: root.shortcut;
        below: root.tooltip-below;
    }

    ta := TouchArea {
        mouse-cursor: root.enabled ? pointer : default;
        clicked => { if (root.enabled) { root.clicked(); } }
    }
}

/// A square icon button with hover/active/disabled states and a tooltip.
export component IconButton inherits Rectangle {
    in property <Icon> icon;
    in property <string> tooltip: "";
    in property <string> shortcut: "";
    in property <bool> active: false;
    in property <bool> enabled: true;
    in property <length> button-width: 32px;
    in property <bool> tooltip-below: false;
    callback clicked;

    width: root.button-width;
    height: 32px;
    border-radius: 8px;
    background: !root.enabled ? Theme.control-track
        : root.active ? Theme.accent
        : ta.has-hover ? Theme.chip-hover
        : Theme.chip-bg;

    IconDisplay {
        icon: root.icon;
        width: 100%;
        height: 100%;
        size: Theme.icon-size;
        stroke-width: Theme.icon-stroke;
        stroke: !root.enabled ? Theme.icon-fg-disabled
            : (root.active || ta.has-hover) ? Theme.icon-fg-active
            : Theme.icon-fg-idle;
        horizontal-alignment: center;
        vertical-alignment: center;
    }

    if (ta.has-hover && root.enabled && root.tooltip != "") : Tooltip {
        label: root.tooltip;
        shortcut: root.shortcut;
        below: root.tooltip-below;
    }

    ta := TouchArea {
        mouse-cursor: root.enabled ? pointer : default;
        clicked => { if (root.enabled) { root.clicked(); } }
    }
}

/// A tooltip that hangs above (or below) its parent.
export component Tooltip inherits Rectangle {
    in property <string> label;
    in property <string> shortcut: "";
    in property <bool> below: false;

    x: (parent.width - self.width) / 2;
    y: below ? parent.height + 6px : -self.height - 6px;
    width: Math.max(60px, label-text.preferred-width + 16px);
    height: (shortcut == "") ? 24px : 40px;
    background: Theme.tooltip-bg;
    border-radius: 6px;
    border-color: Theme.tooltip-border;
    border-width: 1px;

    label-text := Text {
        x: 8px;
        y: shortcut == "" ? 0 : 4px;
        width: parent.width - 16px;
        height: shortcut == "" ? parent.height : 18px;
        text: root.label;
        color: Theme.hint-text;
        font-size: 12px;
        vertical-alignment: center;
        overflow: elide;
    }

    if (root.shortcut != "") : Text {
        x: 8px;
        y: 20px;
        width: parent.width - 16px;
        height: 16px;
        text: root.shortcut;
        color: Theme.text-faint;
        font-size: 10px;
        vertical-alignment: center;
        overflow: elide;
    }
}

/// A labelled on/off switch.
export component Switch inherits HorizontalLayout {
    in property <bool> checked: false;
    in property <string> label;
    callback toggled;

    height: 32px;
    spacing: 8px;
    alignment: start;

    Text {
        text: root.label;
        color: Theme.hint-text;
        font-size: Theme.row-font;
        vertical-alignment: center;
        width: 150px;
    }

    Rectangle {
        width: 40px;
        height: 22px;
        border-radius: 11px;
        background: root.checked ? Theme.accent : Theme.control-track;

        Rectangle {
            width: 18px;
            height: 18px;
            y: 2px;
            x: root.checked ? parent.width - 20px : 2px;
            border-radius: 9px;
            background: #ffffff;
        }

        ta := TouchArea {
            mouse-cursor: pointer;
            clicked => { root.toggled(); }
        }
    }
}

/// A horizontal slider with an accent fill and a numeric readout. Emits the
/// continuous value; Rust clamps and quantises it.
export component ValueSlider inherits Rectangle {
    in property <float> value: 0;
    in property <float> min: 0;
    in property <float> max: 1;
    callback changed(float);

    height: 24px;
    border-radius: 4px;
    background: Theme.control-track;

    Rectangle {
        x: 0;
        y: 0;
        width: parent.width * (root.value - root.min) / (root.max - root.min);
        height: 100%;
        border-radius: 4px;
        background: Theme.accent;
    }

    Text {
        x: 0;
        width: 100%;
        height: 100%;
        text: root.value;
        color: #ffffff;
        font-size: 11px;
        horizontal-alignment: center;
        vertical-alignment: center;
    }

    ta := TouchArea {
        mouse-cursor: pointer;
        pointer-event(e) => {
            if (e.kind == PointerEventKind.down || (e.kind == PointerEventKind.move && ta.pressed)) {
                root.changed(root.min + ta.mouse-x / root.width * (root.max - root.min));
            }
        }
    }
}

/// A colour swatch for the toolbar and settings panel.
export component Swatch inherits Rectangle {
    in property <color> fill;
    in property <bool> active: false;
    callback clicked;

    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: root.fill;
    border-color: root.active ? #ffffff : (ta.has-hover ? #9a9a9a : transparent);
    border-width: root.active ? 2px : 1px;

    ta := TouchArea {
        mouse-cursor: pointer;
        clicked => { root.clicked(); }
    }
}

/// A vertical separator between toolbar groups.
export component Sep inherits Rectangle {
    width: 1px;
    height: 24px;
    background: Theme.chip-border;
}

/// An accent section heading with a leading icon.
export component SectionLabel inherits HorizontalLayout {
    in property <Icon> icon;
    in property <string> text;
    height: 24px;
    spacing: 6px;
    alignment: start;

    IconDisplay { icon: root.icon; size: 14px; stroke: Theme.accent; }
    Text {
        text: root.text;
        color: Theme.accent;
        font-size: Theme.section-font;
        vertical-alignment: center;
    }
}
```

> The plan's `Theme` global (Task 4) defines `icon-size`, `icon-stroke`, `icon-fg-*`, `chip-*`, `accent`, `hint-text`, `text-faint`, `tooltip-*`, `control-track`, `row-font`, `section-font`. Task 2's `PinWindow` only needs `panel-border`/`accent`; Task 4 owns the full Theme global. To keep Task 2 compiling on its own, Task 4's Theme additions must land before `controls.slint` is imported by `main.slint`. Order Task 4's Theme block first if executing out of order — or (simpler) do Task 4's Step "Theme global" before wiring `controls.slint` into `main.slint`. The recommended sequence below is 1 → 4 → 2 → 3 → 5 → 6 → 7, which avoids the issue.

- [ ] **Step 9: Rewrite `PinWindow` in `ui/main.slint`**

At the top of the file, delete the old `export global Theme { … }` block (it now lives in `theme.slint`) and add:

```slint
import { Icons } from "icons.slint";
import { IconButton, Swatch } from "controls.slint";
import { Theme } from "theme.slint";
```

Then replace the `PinWindow` component with:

```slint
export component PinWindow inherits Window {
    in property <image> shot;
    callback dismiss;
    callback resize(length, length);

    background: Theme.canvas-bg;
    no-frame: true;
    always-on-top: true;
    title: "Pocshot pin";

    Image {
        source: root.shot;
        width: 100%;
        height: 100%;
    }

    Rectangle {
        x: 0;
        y: 0;
        width: 100%;
        height: 100%;
        background: transparent;
        border-color: Theme.panel-border;
        border-width: 1px;
    }

    close := IconButton {
        x: parent.width - 34px;
        y: 6px;
        icon: Icons.X;
        tooltip: "Close";
        button-width: 28px;
        tooltip-below: true;
        clicked => { root.dismiss(); }
    }

    grip := TouchArea {
        x: parent.width - 16px;
        y: parent.height - 16px;
        width: 16px;
        height: 16px;
        mouse-cursor: nwse-resize;
        pointer-event(e) => {
            if (e.kind == PointerEventKind.move && grip.pressed) {
                root.resize(grip.mouse-x + grip.x, grip.mouse-y + grip.y);
            }
            if (e.kind == PointerEventKind.down && e.button == PointerEventButton.right) {
                root.dismiss();
            }
        }
    }

    TouchArea {
        pointer-event(e) => {
            if (e.kind == PointerEventKind.down && e.button == PointerEventButton.right) {
                root.dismiss();
            }
        }
    }

    WindowMoveArea { }

    keys := FocusScope {
        key-pressed(e) => {
            if (e.text == "\u{1b}" || e.text == "q") {
                root.dismiss();
            }
            accept
        }
    }
}
```

- [ ] **Step 10: Wire the resize callback in `run_pin`** (`crates/pocshot-ui-slint/src/lib.rs`):

```rust
    ui.on_resize(move |width, height| {
        let (w, h) = clamp_pin_size(width, height);
        if let Some(ui) = ui_weak.upgrade() {
            ui.window().set_size(slint::PhysicalSize::new(w as u32, h as u32));
        }
    });
```

Set up the weak reference before assigning the callback:

```rust
    let ui_weak = ui.as_weak();
```

- [ ] **Step 11: Build and smoke-test the pin**

Run: `cargo build -p pocshot-ui-slint`
Expected: clean build (icons.slint and controls.slint compile).

Manual: copy an image to the clipboard, then `cargo run -p pocshot -- --ui slint pin` or trigger Pin from the editor. Verify the border, hover `X`, drag to move, drag the bottom-right grip to resize (clamps at 16×16), right-click and Esc close.

- [ ] **Step 12: Commit**

```bash
git add scripts/extract_lucide.py crates/pocshot-ui-slint/ui/icons.slint \
        crates/pocshot-ui-slint/ui/theme.slint crates/pocshot-ui-slint/ui/controls.slint \
        crates/pocshot-ui-slint/ui/main.slint crates/pocshot-ui-slint/src/lib.rs
git commit -m "feat(slint): vendor Lucide icons, add controls, rebuild the pin window"
```

---

### Task 3: Pure toolbar layout math

**Files:**
- Create: `crates/pocshot-ui-slint/src/layout.rs`
- Modify: `crates/pocshot-ui-slint/src/lib.rs` (add `mod layout;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct ToolbarSpec { pub button: f32, pub button_gap: f32, pub swatch: f32, pub swatch_gap: f32, pub sep_w: f32, pub pad: f32, pub margin: f32, pub row_h: f32, pub hint_h: f32, pub n_actions: usize, pub n_tools: usize, pub n_swatches: usize }`
  - `pub struct ToolbarLayout { pub x: f32, pub y: f32, pub width: f32, pub height: f32, pub wrapped: bool, pub tooltips_below: bool }`
  - `pub fn toolbar_layout(win: (f32, f32), sel: (f32, f32, f32, f32), spec: &ToolbarSpec) -> ToolbarLayout`

- [ ] **Step 1: Write failing tests** in `crates/pocshot-ui-slint/src/layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ToolbarSpec {
        ToolbarSpec {
            button: 32.0,
            button_gap: 4.0,
            swatch: 20.0,
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
        // use the full viewport, not the 320x200 image rect.
        let l = toolbar_layout((1920.0, 1080.0), (800.0, 400.0, 320.0, 200.0), &spec());
        assert!(l.width > 600.0);
        assert!(!l.wrapped);
    }
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p pocshot-ui-slint layout`
Expected: FAIL — module `layout` not found / items missing.

- [ ] **Step 3: Implement `crates/pocshot-ui-slint/src/layout.rs`:**

```rust
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
    // buttons + (swatches) + two separators + trailing width stepper allowance.
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
```

- [ ] **Step 4: Add `mod layout;`** to `crates/pocshot-ui-slint/src/lib.rs` next to `mod editor;`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p pocshot-ui-slint layout`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/pocshot-ui-slint/src/layout.rs crates/pocshot-ui-slint/src/lib.rs
git commit -m "feat(slint): pure toolbar layout math"
```

---

### Task 4: Single floating toolbar

The `Theme` global already lives in `ui/theme.slint` (Task 2). This task adds the grouped button components and the panel, and rewires Rust.

**Files:**
- Modify: `crates/pocshot-ui-slint/ui/main.slint`
- Modify: `crates/pocshot-ui-slint/src/editor.rs`

**Interfaces:**
- Consumes: `Icons`, `IconButton`, `Swatch`, `Sep`, `Theme`, `ToolbarSpec`, `ToolbarLayout`, `toolbar_layout`.
- Produces (on `EditorWindow`): `toolbar-x`, `toolbar-y`, `toolbar-width`, `toolbar-height` (`length`), `toolbar-wrapped`, `toolbar-tooltips-below` (`bool`), `can-undo`, `can-redo` (`bool`); `Editor::refresh_chrome()`; `Editor::refresh_toolbar()` rewritten.

- [ ] **Step 1: Extend the imports** at the top of `ui/main.slint`. Task 2 added `icons.slint`, `controls.slint` and `theme.slint` imports; merge these into them (do not duplicate the `controls.slint` import line):

```slint
import { Icons, IconDisplay } from "icons.slint";
import { IconButton, Swatch, Sep, Switch, ValueSlider, SectionLabel, HelpRow } from "controls.slint";
```

- [ ] **Step 2: Replace the two toolbar `Rectangle`s + `HorizontalLayout`s** (currently lines ~295–380) with grouped components and a single panel. Add these three components before `EditorWindow`:

```slint
component ActionButtons inherits HorizontalLayout {
    in property <int> active-tool: 0;
    in property <bool> can-undo: false;
    in property <bool> can-redo: false;
    in property <bool> settings-visible: false;
    in property <bool> help-visible: false;
    in property <bool> tips-below: false;
    callback action(string);

    height: 32px;
    spacing: 4px;
    alignment: start;

    IconButton { icon: Icons.RotateCw; tooltip: "Refresh capture"; shortcut: "re-captures and resets"; tooltip-below: root.tips-below; clicked => { root.action("refresh"); } }
    IconButton { icon: Icons.Undo2; tooltip: "Undo"; shortcut: "Ctrl+Z"; enabled: root.can-undo; tooltip-below: root.tips-below; clicked => { root.action("undo"); } }
    IconButton { icon: Icons.Redo2; tooltip: "Redo"; shortcut: "Ctrl+Shift+Z"; enabled: root.can-redo; tooltip-below: root.tips-below; clicked => { root.action("redo"); } }
    IconButton { icon: Icons.Download; tooltip: "Save"; shortcut: "Ctrl+S"; tooltip-below: root.tips-below; clicked => { root.action("save"); } }
    IconButton { icon: Icons.Copy; tooltip: "Copy"; shortcut: "Enter / Ctrl+C"; active: true; tooltip-below: root.tips-below; clicked => { root.action("copy"); } }
    IconButton { icon: Icons.Pin; tooltip: "Pin selection"; tooltip-below: root.tips-below; clicked => { root.action("pin"); } }
    IconButton { icon: Icons.ScanText; tooltip: "OCR selection"; shortcut: "O"; tooltip-below: root.tips-below; clicked => { root.action("ocr"); } }
    IconButton { icon: Icons.Trash2; tooltip: "Clear annotations"; tooltip-below: root.tips-below; clicked => { root.action("clear-annotations"); } }
    IconButton { icon: Icons.Maximize; tooltip: "Reset selection"; tooltip-below: root.tips-below; clicked => { root.action("clear-selection"); } }
    IconButton { icon: Icons.Settings; tooltip: "Settings"; active: root.settings-visible; tooltip-below: root.tips-below; clicked => { root.action("settings"); } }
    IconButton { icon: Icons.CircleHelp; tooltip: "Help"; shortcut: "?"; active: root.help-visible; tooltip-below: root.tips-below; clicked => { root.action("help"); } }
    IconButton { icon: Icons.X; tooltip: "Close"; shortcut: "Esc"; tooltip-below: root.tips-below; clicked => { root.action("quit"); } }
}

component ToolButtons inherits HorizontalLayout {
    in property <int> active-tool: 0;
    in property <bool> tips-below: false;
    callback tool(int);

    height: 32px;
    spacing: 4px;
    alignment: start;

    IconButton { icon: Icons.MousePointer2; tooltip: "Select"; shortcut: "V"; active: root.active-tool == 0; tooltip-below: root.tips-below; clicked => { root.tool(0); } }
    IconButton { icon: Icons.Square; tooltip: "Rectangle"; shortcut: "R"; active: root.active-tool == 1; tooltip-below: root.tips-below; clicked => { root.tool(1); } }
    IconButton { icon: Icons.SquareFill; tooltip: "Filled rectangle"; shortcut: "Shift+R"; active: root.active-tool == 2; tooltip-below: root.tips-below; clicked => { root.tool(2); } }
    IconButton { icon: Icons.Slash; tooltip: "Line"; shortcut: "L"; active: root.active-tool == 3; tooltip-below: root.tips-below; clicked => { root.tool(3); } }
    IconButton { icon: Icons.ArrowUpRight; tooltip: "Arrow"; shortcut: "A"; active: root.active-tool == 4; tooltip-below: root.tips-below; clicked => { root.tool(4); } }
    IconButton { icon: Icons.Circle; tooltip: "Circle"; shortcut: "C"; active: root.active-tool == 5; tooltip-below: root.tips-below; clicked => { root.tool(5); } }
    IconButton { icon: Icons.CircleFill; tooltip: "Filled circle"; shortcut: "Shift+C"; active: root.active-tool == 6; tooltip-below: root.tips-below; clicked => { root.tool(6); } }
    IconButton { icon: Icons.Pen; tooltip: "Pen"; shortcut: "P"; active: root.active-tool == 7; tooltip-below: root.tips-below; clicked => { root.tool(7); } }
    IconButton { icon: Icons.Highlighter; tooltip: "Highlighter"; shortcut: "H"; active: root.active-tool == 8; tooltip-below: root.tips-below; clicked => { root.tool(8); } }
    IconButton { icon: Icons.EyeOff; tooltip: "Redact"; shortcut: "D"; active: root.active-tool == 9; tooltip-below: root.tips-below; clicked => { root.tool(9); } }
    IconButton { icon: Icons.Hash; tooltip: "Counter"; shortcut: "N"; active: root.active-tool == 10; tooltip-below: root.tips-below; clicked => { root.tool(10); } }
    IconButton { icon: Icons.Type; tooltip: "Text"; shortcut: "T"; active: root.active-tool == 11; tooltip-below: root.tips-below; clicked => { root.tool(11); } }
    IconButton { icon: Icons.Waves; tooltip: "Blur"; shortcut: "B"; active: root.active-tool == 12; tooltip-below: root.tips-below; clicked => { root.tool(12); } }
    IconButton { icon: Icons.Grid3x3; tooltip: "Pixelate"; shortcut: "M"; active: root.active-tool == 13; tooltip-below: root.tips-below; clicked => { root.tool(13); } }
    IconButton { icon: Icons.Eraser; tooltip: "Eraser"; shortcut: "E"; active: root.active-tool == 14; tooltip-below: root.tips-below; clicked => { root.tool(14); } }
    IconButton { icon: Icons.BoxSelect; tooltip: "Text border"; shortcut: "G"; active: root.active-tool == 15; tooltip-below: root.tips-below; clicked => { root.tool(15); } }
}

component OptionControls inherits HorizontalLayout {
    in property <int> active-color: 0;
    in property <int> stroke-width: 3;
    in property <bool> tips-below: false;
    callback pick-color(int);
    callback action(string);

    height: 32px;
    spacing: 4px;
    alignment: start;

    Swatch { fill: Theme.swatch0; active: root.active-color == 0; clicked => { root.pick-color(0); } }
    Swatch { fill: Theme.swatch1; active: root.active-color == 1; clicked => { root.pick-color(1); } }
    Swatch { fill: Theme.swatch2; active: root.active-color == 2; clicked => { root.pick-color(2); } }
    Swatch { fill: Theme.swatch3; active: root.active-color == 3; clicked => { root.pick-color(3); } }
    Swatch { fill: Theme.swatch4; active: root.active-color == 4; clicked => { root.pick-color(4); } }
    Swatch { fill: Theme.swatch5; active: root.active-color == 5; clicked => { root.pick-color(5); } }
    Sep { }
    IconButton { icon: Icons.Minus; tooltip: "Thinner stroke"; shortcut: "["; button-width: 28px; tooltip-below: root.tips-below; clicked => { root.action("width-"); } }
    Text { width: 30px; text: root.stroke-width; color: Theme.hint-text; font-size: Theme.row-font; vertical-alignment: center; horizontal-alignment: center; }
    IconButton { icon: Icons.Plus; tooltip: "Thicker stroke"; shortcut: "]"; button-width: 28px; tooltip-below: root.tips-below; clicked => { root.action("width+"); } }
}
```

> `Swatch` is defined and exported in `controls.slint` (Task 2, Step 8), so `OptionControls`, the settings panel and any other consumer share one implementation.

- [ ] **Step 3: Add the toolbar panel** inside `EditorWindow` (replace the old two rows and their layouts):

```slint
    // Single floating toolbar. `toolbar-wrapped` chooses one row or two.
    if (root.sel-handles-visible || root.sel-visible) : Rectangle {
        x: root.toolbar-x - 2px;
        y: root.toolbar-y - 2px;
        width: root.toolbar-width + 4px;
        height: root.toolbar-height + 4px;
        border-radius: 12px;
        background: #00000055;
        opacity: 0.35;
    }

    toolbar-panel := Rectangle {
        x: root.toolbar-x;
        y: root.toolbar-y;
        width: root.toolbar-width;
        height: root.toolbar-height;
        border-radius: 10px;
        background: Theme.toolbar-bg;
        border-color: Theme.chip-border;
        border-width: 1px;

        if (!root.toolbar-wrapped) : HorizontalLayout {
            x: Theme.toolbar-padding;
            y: (root.toolbar-height - 32px) / 2;
            spacing: 6px;
            alignment: start;
            ActionButtons { can-undo: root.can-undo; can-redo: root.can-redo; settings-visible: root.settings-visible; help-visible: root.help-visible; tips-below: root.toolbar-tooltips-below; action(a) => { root.action(a); } }
            Sep { }
            ToolButtons { active-tool: root.active-tool; tips-below: root.toolbar-tooltips-below; tool(i) => { root.tool(i); } }
            Sep { }
            OptionControls { active-color: root.active-color; stroke-width: root.stroke-width; tips-below: root.toolbar-tooltips-below; pick-color(i) => { root.pick-color(i); } action(a) => { root.action(a); } }
        }

        if (root.toolbar-wrapped) : VerticalLayout {
            x: Theme.toolbar-padding;
            y: Theme.toolbar-padding;
            spacing: 4px;
            alignment: start;
            HorizontalLayout {
                spacing: 6px;
                ActionButtons { can-undo: root.can-undo; can-redo: root.can-redo; settings-visible: root.settings-visible; help-visible: root.help-visible; tips-below: true; action(a) => { root.action(a); } }
                Sep { }
                OptionControls { active-color: root.active-color; stroke-width: root.stroke-width; tips-below: true; pick-color(i) => { root.pick-color(i); } action(a) => { root.action(a); } }
            }
            HorizontalLayout {
                spacing: 6px;
                ToolButtons { active-tool: root.active-tool; tips-below: true; tool(i) => { root.tool(i); } }
            }
        }
    }
```

Add these properties to `EditorWindow` (and delete the old `toolbar-width` default of 1060px):

```slint
    in property <length> toolbar-x: 8px;
    in property <length> toolbar-y: 0px;
    in property <length> toolbar-width: 900px;
    in property <length> toolbar-height: 40px;
    in property <bool> toolbar-wrapped: false;
    in property <bool> toolbar-tooltips-below: false;
    in property <bool> can-undo: false;
    in property <bool> can-redo: false;
```

- [ ] **Step 4: Rewrite `Editor::refresh_toolbar`** in `editor.rs` to use `layout::toolbar_layout`:

```rust
    pub(crate) fn refresh_toolbar(&mut self) {
        let win = self.window_size();
        if win.x <= 0.0 || win.y <= 0.0 {
            return;
        }
        let g = &self.settings.theme.geometry;
        let spec = crate::layout::ToolbarSpec {
            button: 32.0,
            button_gap: 4.0,
            swatch: 26.0,
            swatch_gap: 4.0,
            sep_w: 1.0,
            pad: g.toolbar_padding,
            margin: 8.0,
            row_h: 40.0,
            hint_h: 28.0,
            n_actions: 12,
            n_tools: 16,
            n_swatches: 6,
        };
        let sel = self.selection_window_px();
        let l = crate::layout::toolbar_layout(
            (win.x, win.y),
            (sel.min.x, sel.min.y, sel.width(), sel.height()),
            &spec,
        );
        self.ui.set_toolbar_x(l.x);
        self.ui.set_toolbar_y(l.y);
        self.ui.set_toolbar_width(l.width);
        self.ui.set_toolbar_height(l.height);
        self.ui.set_toolbar_wrapped(l.wrapped);
        self.ui.set_toolbar_tooltips_below(l.tooltips_below);
        self.toolbar_rect = Rect::from_min_size(Pos2::new(l.x, l.y), Vec2::new(l.width, l.height));
    }
```

Delete the `TOOLBAR_W` / `TOOLBAR_H` constants; keep `TOOLBAR_MARGIN` only if still referenced (remove it if not).

- [ ] **Step 5: Publish `can-undo`/`can-redo`.** Rename `refresh_hint` to `refresh_chrome` (update all call sites) and add:

```rust
        self.ui.set_can_undo(!self.history.is_empty());
        self.ui.set_can_redo(!self.redo_stack.is_empty());
```

- [ ] **Step 6: Build, test, lint**

Run: `cargo build -p pocshot-ui-slint && cargo test -p pocshot-ui-slint && cargo clippy -p pocshot-ui-slint`
Expected: clean; layout tests pass.

Manual: launch the editor. Verify icons render, hover/active/disabled states, tooltips with shortcuts, the toolbar flips above when the selection is near the bottom, wraps on a narrow window, and does not swallow canvas drags.

- [ ] **Step 7: Commit**

```bash
git add crates/pocshot-ui-slint/ui/main.slint crates/pocshot-ui-slint/src/editor.rs
git commit -m "feat(slint): single floating Lucide toolbar"
```

---

### Task 5: Hint bar + structured help overlay

**Files:**
- Modify: `crates/pocshot-ui-slint/ui/main.slint`
- Modify: `crates/pocshot-ui-slint/src/editor.rs`
- Modify: `crates/pocshot-ui-slint/src/lib.rs`
- Test: `crates/pocshot-ui-slint/src/editor.rs`

**Interfaces:**
- Consumes: `Icons`, `IconButton`, `HelpRow`.
- Produces: `EditorWindow` `status` (`string`), `hint-chips` (`[string]`), `help-rows` (`[HelpRow]`), `help-visible`; `pub(crate) fn help_rows() -> Vec<HelpRow>`.

- [ ] **Step 1: Write the failing test** in `editor.rs` tests:

```rust
#[test]
fn help_rows_cover_the_three_sections() {
    let rows = help_rows();
    assert!(rows.len() >= 10);
    let joined = rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join("|");
    assert!(joined.contains("Select"));
    assert!(joined.contains("Undo"));
    assert!(joined.contains("copy"));
    assert!(rows.iter().all(|r| !r.keys.is_empty() && !r.text.is_empty()));
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p pocshot-ui-slint help_rows`
Expected: FAIL — `help_rows` not found.

- [ ] **Step 3: Implement `help_rows()`** in `editor.rs` (replace `HELP_TEXT`):

```rust
/// Shortcut reference shown by the help overlay.
pub(crate) fn help_rows() -> Vec<HelpRow> {
    let row = |keys: &str, text: &str| HelpRow { keys: keys.into(), text: text.into() };
    vec![
        row("V", "Select — drag to select, drag inside to move"),
        row("R / Shift+R", "Rectangle / filled rectangle"),
        row("L / A", "Line / arrow"),
        row("C / Shift+C", "Circle / filled circle"),
        row("P / H", "Pen / highlighter"),
        row("D / G", "Redact / text border (needs OCR)"),
        row("N / T", "Counter / text"),
        row("B / M", "Blur / pixelate"),
        row("E", "Eraser — click an annotation"),
        row("1–6", "Choose a colour"),
        row("[ / ]", "Stroke width"),
        row("Ctrl+drag", "Constrain to square / circle / 45°"),
        row("Enter / Ctrl+C", "Copy selection"),
        row("Ctrl+S", "Save selection"),
        row("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
        row("O", "OCR the selection"),
        row("Shift", "Temporarily disable snapping"),
        row("?", "Toggle this help"),
        row("Esc", "Clear selection, then quit"),
    ]
}
```

- [ ] **Step 4: Replace the hint bar + help overlay** in `ui/main.slint` with:

```slint
    // Hint bar: status on the left, contextual shortcut chips on the right.
    Rectangle {
        x: 0;
        y: root.height - 30px;
        width: 100%;
        height: 30px;
        background: Theme.toolbar-bg;
    }

    Text {
        x: 12px;
        y: root.height - 30px;
        height: 30px;
        width: 55%;
        vertical-alignment: center;
        color: Theme.hint-text;
        font-size: Theme.row-font;
        text: root.status == "" ? root.hint-primary : root.status;
        overflow: elide;
    }

    HorizontalLayout {
        x: parent.width - self.width - 12px;
        y: root.height - 30px;
        height: 30px;
        spacing: 6px;
        alignment: end;
        for chip in root.hint-chips: Rectangle {
            height: 20px;
            width: chip-text.preferred-width + 14px;
            border-radius: 5px;
            background: Theme.chip-bg;
            chip-text := Text {
                x: 7px;
                width: 100%;
                height: 100%;
                text: chip;
                color: Theme.hint-text;
                font-size: Theme.shortcut-font;
                vertical-alignment: center;
            }
        }
    }

    // Help overlay.
    if (root.help-visible) : Rectangle {
        x: 0; y: 0; width: 100%; height: 100%;
        background: #00000000;
        catcher := TouchArea { clicked => { root.action("help"); } }
    }
    if (root.help-visible) : Rectangle {
        x: root.width / 2 - 340px;
        y: root.height / 2 - 260px;
        width: 680px;
        height: 520px;
        background: Theme.panel-bg;
        border-radius: 14px;
        border-color: Theme.panel-border;
        border-width: 1px;

        Text {
            x: 20px; y: 16px;
            text: "Shortcuts";
            color: #ffffff;
            font-size: Theme.panel-title;
        }
        IconButton {
            x: parent.width - 44px; y: 12px;
            icon: Icons.X; tooltip: "Close"; button-width: 28px; tooltip-below: true;
            clicked => { root.action("help"); }
        }
        Flickable {
            x: 20px; y: 52px;
            width: parent.width - 40px;
            height: parent.height - 68px;
            viewport-height: help-list.preferred-height;
            help-list := VerticalLayout {
                spacing: 6px;
                alignment: start;
                for row in root.help-rows: HorizontalLayout {
                    spacing: 12px;
                    alignment: start;
                    Text {
                        width: 150px;
                        text: row.keys;
                        color: Theme.accent;
                        font-size: Theme.row-font;
                        vertical-alignment: center;
                    }
                    Text {
                        width: 100%;
                        text: row.text;
                        color: Theme.hint-text;
                        font-size: Theme.row-font;
                        vertical-alignment: center;
                        wrap: word-wrap;
                    }
                }
            }
        }
    }
```

Add properties to `EditorWindow` and drop `hint`/`help-text`:

```slint
    in property <string> status: "";
    in property <string> hint-primary: "";
    in property <[string]> hint-chips;
    in property <[HelpRow]> help-rows;
    in property <bool> help-visible: false;
```

- [ ] **Step 5: Update `refresh_chrome`** in `editor.rs` to publish the compact status/chips instead of the giant string. Delete the `format!("{:?} · color ...")` block; replace with:

```rust
        if self.typing_text {
            self.ui.set_status("Type text · Enter commit · Esc cancel".into());
            self.ui.set_hint_chips(slint::ModelRc::from(std::rc::Rc::new(VecModel::from(vec![
                SharedString::from("Backspace Delete"),
            ]))));
            return;
        }
        self.ui.set_status(match &self.status {
            Some(status) => SharedString::from(status.as_str()),
            None => SharedString::from(tool_label(self.tool)),
        });
        self.ui.set_hint_chips(slint::ModelRc::from(std::rc::Rc::new(VecModel::from(vec![
            SharedString::from("Enter Copy"),
            SharedString::from("Ctrl+S Save"),
            SharedString::from("Ctrl+Z Undo"),
            SharedString::from("Esc Cancel"),
        ]))));
```

`refresh_chrome` needs `SharedString` and `VecModel`; add `use slint::{ModelRc, SharedString, VecModel};` to `editor.rs` (extend the existing `use slint::{...}`). Add this free function to `editor.rs` (do **not** modify `pocshot-annotate`):

```rust
/// Human-readable name for the hint bar.
fn tool_label(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Select => "Select — drag to select, drag inside to move",
        ToolKind::Rectangle => "Rectangle",
        ToolKind::FilledRectangle => "Filled rectangle",
        ToolKind::Line => "Line",
        ToolKind::Arrow => "Arrow",
        ToolKind::Circle => "Circle",
        ToolKind::FilledCircle => "Filled circle",
        ToolKind::Pen => "Pen",
        ToolKind::Highlighter => "Highlighter",
        ToolKind::Redact => "Redact",
        ToolKind::Counter => "Counter",
        ToolKind::Text => "Text",
        ToolKind::Blur => "Blur",
        ToolKind::Pixelate => "Pixelate",
        ToolKind::Eraser => "Eraser",
        ToolKind::HighlightText => "Text border",
    }
}
```

- [ ] **Step 5b: Set the idle hint text.** In `lib.rs::show_editor`, after `Editor::new(...)`, add:

```rust
    editor.borrow().ui.set_hint_primary("Drag to select a region".into());
```

- [ ] **Step 6: Set help rows in `lib.rs`**

Replace `editor.borrow().ui.set_help_text(HELP_TEXT.into());` with:

```rust
    let rows = editor::help_rows();
    let ui = editor.borrow().ui.clone_strong();
    ui.set_help_rows(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(rows))));
```

(Or expose `Editor::set_help_rows()` that does the same — pick one and use it consistently. `HelpRow` is generated by `slint::include_modules!` and is `Clone`.)

- [ ] **Step 7: Build, test, lint**

Run: `cargo build -p pocshot-ui-slint && cargo test -p pocshot-ui-slint && cargo clippy -p pocshot-ui-slint`
Expected: clean.

Manual: the hint bar shows a short status + chips; `?` opens the two-column help; click-outside, `X`, `?`, and `Esc` all close it.

- [ ] **Step 8: Commit**

```bash
git add crates/pocshot-ui-slint/ui/main.slint crates/pocshot-ui-slint/src/editor.rs crates/pocshot-ui-slint/src/lib.rs
git commit -m "feat(slint): compact hint bar and structured help overlay"
```

---

### Task 6: Settings panel rebuild + theme mapping

**Files:**
- Modify: `crates/pocshot-ui-slint/ui/main.slint`
- Modify: `crates/pocshot-ui-slint/src/editor.rs` (`apply_theme`)
- Test: `crates/pocshot-config/src/theme.rs` (already covers defaults)

**Interfaces:**
- Consumes: `Switch`, `ValueSlider`, `SectionLabel`, `Swatch`, `Icons`.
- Produces: `EditorWindow` gains `ocr-confidence` (`float`) alongside the existing settings properties; the three sliders emit `action("ocr-confidence-set:<f>")`, `action("width-set:<f>")`, `action("text-size-set:<f>")`; `Editor::apply_theme` maps the new fields.

- [ ] **Step 1: Replace the settings panel** in `ui/main.slint` with a scrollable card:

```slint
    if (root.settings-visible) : Rectangle {
        x: root.width - 328px;
        y: root.height - 520px;
        width: 316px;
        height: 500px;
        background: Theme.panel-bg;
        border-radius: 14px;
        border-color: Theme.panel-border;
        border-width: 1px;

        HorizontalLayout {
            x: 16px; y: 14px;
            height: 28px;
            spacing: 8px;
            IconDisplay { icon: Icons.Settings; size: 16px; stroke: #ffffff; }
            Text { text: "Settings"; color: #ffffff; font-size: Theme.panel-title; vertical-alignment: center; }
        }
        IconButton {
            x: parent.width - 44px; y: 12px;
            icon: Icons.X; tooltip: "Close"; button-width: 28px; tooltip-below: true;
            clicked => { root.action("settings"); }
        }

        Flickable {
            x: 16px; y: 52px;
            width: parent.width - 32px;
            height: parent.height - 68px;
            viewport-height: settings-list.preferred-height;
            settings-list := VerticalLayout {
                spacing: 8px;
                alignment: start;

                SectionLabel { icon: Icons.Download; text: "Export"; }
                Switch { label: "Copy on save"; checked: root.copy-save-value == "on"; toggled => { root.action("copy-save"); } }

                SectionLabel { icon: Icons.Maximize; text: "Snapping"; }
                Switch { label: "Snapping"; checked: root.snap-value == "on"; toggled => { root.action("snap"); } }
                Switch { label: "Show guides"; checked: root.guides-value == "on"; toggled => { root.action("guides"); } }

                SectionLabel { icon: Icons.ScanText; text: "OCR"; }
                Switch { label: "Auto OCR on capture"; checked: root.auto-ocr-value == "on"; toggled => { root.action("auto-ocr"); } }
                HorizontalLayout {
                    height: 28px; spacing: 8px;
                    Text { text: "Min confidence"; color: Theme.hint-text; font-size: Theme.row-font; width: 150px; vertical-alignment: center; }
                    ValueSlider {
                        width: 80px; height: 24px;
                        min: 0.10; max: 0.95;
                        value: root.ocr-confidence;
                        changed(v) => { root.action("ocr-confidence-set:" + v); }
                    }
                    Text { text: root.ocr-confidence-value; color: #ffffff; font-size: Theme.row-font; width: 40px; horizontal-alignment: center; vertical-alignment: center; }
                }

                SectionLabel { icon: Icons.Pen; text: "Annotations"; }
                HorizontalLayout {
                    height: 28px; spacing: 8px;
                    Text { text: "Stroke width"; color: Theme.hint-text; font-size: Theme.row-font; width: 150px; vertical-alignment: center; }
                    ValueSlider { width: 80px; height: 24px; min: 1; max: 24; value: root.stroke-width; changed(v) => { root.action("width-set:" + v); } }
                }
                HorizontalLayout {
                    height: 28px; spacing: 8px;
                    Text { text: "Text size"; color: Theme.hint-text; font-size: Theme.row-font; width: 150px; vertical-alignment: center; }
                    ValueSlider { width: 80px; height: 24px; min: 8; max: 72; value: root.text-size; changed(v) => { root.action("text-size-set:" + v); } }
                }
                HorizontalLayout {
                    height: 32px; spacing: 6px; alignment: start;
                    Swatch { fill: Theme.swatch0; active: root.active-color == 0; clicked => { root.pick-color(0); } }
                    Swatch { fill: Theme.swatch1; active: root.active-color == 1; clicked => { root.pick-color(1); } }
                    Swatch { fill: Theme.swatch2; active: root.active-color == 2; clicked => { root.pick-color(2); } }
                    Swatch { fill: Theme.swatch3; active: root.active-color == 3; clicked => { root.pick-color(3); } }
                    Swatch { fill: Theme.swatch4; active: root.active-color == 4; clicked => { root.pick-color(4); } }
                    Swatch { fill: Theme.swatch5; active: root.active-color == 5; clicked => { root.pick-color(5); } }
                }

                SectionLabel { icon: Icons.RotateCw; text: "Theme"; }
                HorizontalLayout {
                    height: 32px; spacing: 8px; alignment: start;
                    IconButton { icon: Icons.RotateCw; tooltip: "Reload theme"; button-width: 32px; tooltip-below: true; clicked => { root.action("reload-theme"); } }
                    Text { text: "re-reads config.json"; color: Theme.text-faint; font-size: Theme.shortcut-font; vertical-alignment: center; }
                }
            }
        }
    }
```

`Swatch` is defined in `controls.slint` (Task 4) — export it and import it here.

- [ ] **Step 1b: Add the float confidence property and publish it.** Add `in property <float> ocr-confidence: 0.5;` to `EditorWindow`, and in `refresh_chrome` add `self.ui.set_ocr_confidence(self.ocr_confidence);` next to the other readout setters.

- [ ] **Step 2: Parse the value-carrying actions** in `editor.rs::on_action` (before the existing exact matches):

```rust
        if let Some(v) = action.strip_prefix("width-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.width = v.clamp(1.0, 24.0);
                self.settings.annotation_stroke_width = self.width;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
        if let Some(v) = action.strip_prefix("text-size-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.text_size = v.clamp(8.0, 72.0);
                self.settings.theme.fonts.annotation_text = self.text_size;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
        if let Some(v) = action.strip_prefix("ocr-confidence-set:") {
            if let Ok(v) = v.parse::<f32>() {
                self.ocr_confidence = v.clamp(0.1, 0.95);
                self.settings.ocr_confidence = self.ocr_confidence;
                self.persist_settings();
                self.refresh_chrome();
            }
            return;
        }
```

- [ ] **Step 3: Extend `apply_theme`** in `editor.rs` with the new mappings:

```rust
        theme.set_panel_bg(to_slint_color(colors.panel_bg));
        theme.set_panel_border(to_slint_color(colors.panel_border));
        theme.set_control_track(to_slint_color(colors.control_track));
        theme.set_icon_fg_idle(to_slint_color(colors.icon_fg_idle));
        theme.set_icon_fg_active(to_slint_color(colors.icon_fg_active));
        theme.set_icon_fg_disabled(to_slint_color(colors.icon_fg_disabled));
        theme.set_text_faint(to_slint_color(colors.text_faint));
        theme.set_tooltip_border(to_slint_color(colors.tooltip_border));
        let g = &self.settings.theme.geometry;
        theme.set_icon_size(g.icon_size * 1px);
        theme.set_icon_stroke(g.icon_stroke);
        let f = &self.settings.theme.fonts;
        theme.set_row_font(f.row);
        theme.set_section_font(f.section);
        theme.set_shortcut_font(f.shortcut);
```

- [ ] **Step 4: Build, test, lint**

Run: `cargo build -p pocshot-ui-slint && cargo test -p pocshot-config -p pocshot-ui-slint && cargo clippy -p pocshot-ui-slint`
Expected: clean.

Manual: every switch/slider changes state and survives a restart (`config.json`); the sections and icons render; the panel scrolls if it overflows.

- [ ] **Step 5: Commit**

```bash
git add crates/pocshot-ui-slint/ui/main.slint crates/pocshot-ui-slint/src/editor.rs
git commit -m "feat(slint): Lucide settings panel and theme mapping"
```

---

### Task 7: Integration, cleanup, verification

**Files:**
- Modify: `crates/pocshot-ui-slint/ui/main.slint` (remove dead properties/components)
- Modify: `crates/pocshot-ui-slint/src/editor.rs` (remove dead constants/helpers)
- Modify: `docs/gui-architecture.md` (note the Slint UI)

**Interfaces:**
- Consumes: everything above.
- Produces: a clean build and a documented manual-verification pass.

- [ ] **Step 1: Remove dead code.** Confirm these no longer exist and delete any remaining references: `TOOLBAR_W`, `TOOLBAR_H`, `TOOLBAR_MARGIN` (if unused), `HELP_TEXT`, `hint`, `help-text`, `stroke-width` as an `int` fed only to the old text readout (keep the slider), and the old `Chip`/`SettingsRow`/`SelHandle` components if superseded (keep `SelHandle` — the selection handles still need it).

Run: `cargo build -p pocshot-ui-slint 2>&1 | rg "warning|error" || echo clean`

- [ ] **Step 2: Update `docs/gui-architecture.md`** — add a short "Slint UI (`pocshot-ui-slint`)" section: vendored Lucide icons via `scripts/extract_lucide.py`, `ui/icons.slint` + `ui/controls.slint` + `ui/main.slint`, pure `src/layout.rs`, shared theme in `pocshot-config`. Note the vendored-drift tradeoff.

- [ ] **Step 3: Full verification**

Run:
```
cargo test -p pocshot-config -p pocshot-ui-slint -p pocshot-gui
cargo clippy -p pocshot-ui-slint -p pocshot-config -- -D warnings
cargo build -p pocshot --release
```
Expected: all clean.

- [ ] **Step 4: Manual checklist (software renderer is the important one)**

Run both:
```
POCSHOT_RENDERER=software cargo run -p pocshot -- --ui slint
cargo run -p pocshot -- --ui slint
```

- [ ] Icons render crisply and match their slots.
- [ ] Toolbar: below → flips above near the bottom edge → clamps at every edge → wraps at narrow width; canvas drags over the toolbar are ignored.
- [ ] Tooltips appear above normally and below when the toolbar is at the top.
- [ ] Undo/Redo grey out when unavailable and do not act.
- [ ] Settings: every switch/slider mutates state and persists; panel scrolls.
- [ ] Help opens/closes via click-outside, `X`, `?`, `Esc`.
- [ ] Pin: border, hover `X`, drag, corner resize (min 16×16), right-click/Esc close.
- [ ] Clipboard image shown at native size: toolbar still uses the whole viewport.
- [ ] Software-renderer frame cost: dragging a selection feels no worse than before the change (spec's perf gate). If it regresses, record it and open a follow-up to pre-rasterize icons.

- [ ] **Step 5: Commit**

```bash
git add docs/gui-architecture.md crates/pocshot-ui-slint
git commit -m "docs(slint): describe the Lucide-native UI and finish cleanup"
```

---

## Notes for the executor

- Execute in order **1 → 2 → 3 → 4 → 5 → 6 → 7**. Task 2 creates `ui/theme.slint` before `ui/controls.slint`, so the `Theme` global exists when `controls.slint` imports it. Task 1 must precede Task 2 because `theme.slint`'s fields mirror the new `pocshot-config` theme.
- Slint quirks already learned in this repo: no `\r` in Slint strings (use `\u{0d}`); string concat rejects `bool`/`length` (cast with `/ 1px` and ternaries); `PointerEvent` has no position — use the enclosing `TouchArea`'s `mouse-x`/`mouse-y`; int is implicitly converted to float, so `value: root.stroke-width;` is valid.
- Do not add a dependency on `lucide-slint`; the vendored file is the source of truth.
- If Slint rejects `preferred-width` on `Text` (used for tooltip and hint-chip sizing), set a fixed width instead — the values are cosmetic, not functional.
