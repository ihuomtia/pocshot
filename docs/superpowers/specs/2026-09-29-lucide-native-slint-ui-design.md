# Lucide-native Slint UI — Design

**Date:** 2026-09-29
**Branch:** `slint-ui`
**Status:** Approved design, awaiting spec review

## Problem

The Slint UI fixed the VM/RDP lag that the egui UI suffered from, so it is the
intended path forward. But it is less mature than the egui UI it is replacing:

- Toolbar buttons are Unicode text glyphs (`"V"`, `"▭"`, `"─"`, `"✎"`, `"⌫"`,
  `"↻"`, `"✕ ann"`), which render inconsistently across fonts and DPI and look
  unfinished.
- The toolbar is two fixed 1060 px rows positioned by Rust; it does not adapt to
  the window or the selection.
- Undo/Redo never appear disabled even when unavailable.
- The settings panel is a small list of on/off chips.
- The hint bar appends every shortcut into one unreadable line.
- The pin window has no close affordance or resize.

Goal: reach and exceed egui's maturity in appearance and UX, using
[Lucide](https://lucide.dev) icons via the
[lucide-slint](https://github.com/cnlancehu/lucide-slint) icon data, without
regressing the software-renderer performance that motivated the migration.

## Decisions

1. **Aesthetic: Lucide-native.** Icon-first, 1.75 px strokes on a 24 px grid,
   softer radii, subtle borders. This deliberately diverges from the egui look;
   egui is a reference for capability, not for pixel parity.
2. **Toolbar: one floating toolbar** (actions | tools | swatches | width
   stepper) that moves as a unit — below the selection, flipping above near the
   bottom edge, wrapping to two rows only when the window is too narrow. Not
   egui's three independently reflowing groups.
3. **Icons: vendored subset.** Extract only the icons we use from lucide-slint
   1.48.0's generated data into a local Slint file. No build dependency, no
   1.8 MB parse on every build, no binary bloat.
4. **Theme: extend the shared theme.** Add serde-defaulted fields so one
   `config.json` still drives both UIs.
5. **Scope:** all five areas — icons/visual polish, adaptive toolbar,
   settings/theme panel parity, interaction/UX polish, pin-window UX.

## Architecture

The layering is unchanged: `ui/main.slint` remains a dumb view that forwards
events; `src/editor.rs` owns all state and layout math; `src/lib.rs` is the
facade. The new icon file is a third view-only Slint file.

```
crates/pocshot-ui-slint/
├── ui/
│   ├── main.slint      # EditorWindow, PinWindow (rewritten)
│   └── icons.slint     # NEW: vendored Lucide icons + IconDisplay + IconButton
├── src/
│   ├── lib.rs          # facade (pin resize wiring)
│   ├── editor.rs       # toolbar layout, state, theme mapping
│   ├── platform.rs     # unchanged
│   └── snap.rs         # unchanged
└── build.rs            # unchanged (relative import, no library paths)
```

## Component design

### 1. `ui/icons.slint` (new)

Vendored from lucide-slint 1.48.0, preserving its exact public shape:

- `export struct PathElem { viewbox-x/y, viewbox-width/height: float; command: string; has_fill: bool }`
- `export struct Icon { paths: [PathElem] }`
- `export global Icons { out property <Icon> …; }` — only the icons listed below
- `export component IconDisplay inherits Rectangle` — copied verbatim from
  lucide-slint (properties `icon`, `size`, `stroke`, `stroke-fill`,
  `stroke-width`, `absolute-stroke-width`; draws one `Path` per `PathElem`)

Icon inventory (~31 unique icons, ~15–20 KB):

| Slot | Icon | Slot | Icon |
|---|---|---|---|
| Select | `MousePointer2` | Refresh | `RotateCw` |
| Rectangle | `Square` | Save | `Download` |
| Filled rectangle | `SquareFill` *(custom)* | Copy | `Copy` |
| Line | `Slash` | Undo / Redo | `Undo2` / `Redo2` |
| Arrow | `ArrowUpRight` | Pin | `Pin` |
| Circle | `Circle` | OCR | `ScanText` |
| Filled circle | `CircleFill` *(custom)* | Clear annotations | `Trash2` |
| Pen | `Pen` | Clear selection | `Maximize` |
| Highlighter | `Highlighter` | Settings | `Settings` |
| Redact | `EyeOff` | Help | `CircleHelp` |
| Counter | `Hash` | Quit | `X` |
| Text | `Type` | Width / value steppers | `Minus` / `Plus` |
| Blur | `Waves` | Toggle on | `Check` |
| Pixelate | `Grid3x3` | | |
| Eraser | `Eraser` | | |
| Text border | `BoxSelect` | | |

**Custom filled variants.** lucide-slint's `IconDisplay` sets
`fill: has_fill ? stroke : stroke-fill`, and `Square`/`Circle` have
`has_fill: false`, so `stroke-fill` cannot fill them. `SquareFill` and
`CircleFill` are therefore hand-written `has_fill: true` icons on the same 24 px
grid (a solid square path; a solid circle path). This keeps the filled-tool
icons visually consistent with the rest.

**Counter icon.** Lucide has no numbered-circle icons (`Circle1` is absent), so
Counter uses `Hash`.

**IconButton component.** New, in `icons.slint`:

```
export component IconButton inherits Rectangle {
    in property <Icon> icon;
    in property <string> tooltip: "";
    in property <string> shortcut: "";
    in property <bool> active: false;
    in property <bool> enabled: true;
    in property <length> button-width: 32px;
    in property <bool> tooltip-below: false;
    callback clicked;
}
```

- Background: `active` → accent; hover → hover colour; else idle; `!enabled` →
  disabled colour.
- `IconDisplay` centred; stroke = active/idle/disabled foreground.
- Pointer cursor on hover when `enabled`; no tooltip or click when disabled.
- Tooltip is one rounded rect, width derived from content (`overflow: elide`),
  placed above (default) or below (`tooltip-below`) so it stays on-screen when
  the toolbar is near the top.
- Tooltip text is `tooltip`, with `shortcut` appended on a second line when
  non-empty.

The file carries an ISC (Lucide) + MIT/Apache-2.0 (lucide-slint) attribution
header.

### 2. Single floating toolbar (`main.slint` + `editor.rs`)

One `Rectangle` panel: `panel_bg`, `panel_border` 1 px, `panel_radius`, and a
slightly larger translucent rounded rect behind it as a cheap drop shadow
(Slint has no box-shadow).

Content, left to right:

1. **Actions:** Refresh · Undo · Redo · Save · Copy · Pin · OCR · Clear
   annotations · Clear selection · Settings · Help · Quit
2. Separator
3. **Tools:** the 16 tools from `TOOL_ORDER`
4. Separator
5. **Swatches:** the 6 quick colours
6. Separator
7. **Width stepper:** `Minus` · value · `Plus`

Rust remains the source of truth for the toolbar rect because
`Editor::pointer_in_toolbar` hit-tests it:

- Content width is computed from button counts (action/tool/swatch/stepper) and
  the themed button size — deterministic, as egui does today.
- Placement: below the selection (`sel.max.y + margin`); if that overflows,
  above; else clamped into the window.
- If `content_width + 2 * margin > window_width`, wrap into **two rows**
  (actions+options on row 1, tools on row 2), and set a `wrap` property so
  Slint's `VerticalLayout` matches. Height follows (`row_h` once or twice).
- `TOOLBAR_W`/`TOOLBAR_H` constants are replaced by computed values; the
  `toolbar_rect` (bounding box of both rows when wrapped) stays for hit-testing.

Wrapped layout:

```
[ actions            swatches  width ]
[ tools                              ]
```

### 3. Button states, tooltips, hint bar

- New `can-undo` / `can-redo` properties on `EditorWindow`, published from
  `history` / `redo_stack`. `refresh_hint` is renamed `refresh_chrome` since it
  now updates the hint bar, the button states and the toolbar readouts. The
  flags feed the Undo/Redo `IconButton`s as `enabled`.
- Tooltips everywhere: name (+ shortcut for actions/tools).
- **Hint bar** redesigned: a slim bottom bar with a left status/active-tool
  segment and right-aligned shortcut chips (`Enter Copy`, `Ctrl+S Save`,
  `Esc Cancel`). The full shortcut list moves to Help. Rust publishes a short
  status string and the chip labels; it no longer builds the 500-character
  appendix.

### 4. Settings panel

Scrollable card (`Flickable`), width ~300 px, bottom-right, clamped to the
window. Structure:

- Header: `Settings` icon + title + `X` close
- **Export:** `Copy on save` switch
- **Snapping:** `Snapping`, `Show guides` switches
- **OCR:** `Auto OCR` switch; `Min confidence` slider + readout
- **Annotations:** `Stroke width` slider + readout; `Text size` slider +
  readout; colour swatch grid + current-colour preview
- **Theme:** `Reload` button + `re-reads config.json` helper

New `Switch` component (track + knob, accent when on) and `Slider` component
(track, accent fill, value label). Sections use accent section labels with a
leading icon. Switches/sliders call existing `action(...)` strings; the
existing `settings_visible`, `copy-save`, `snap`, `guides`, `auto-ocr`,
`ocr-confidence±`, `text-size±`, `width±`, `reload-theme` plumbing is reused.

### 5. Help overlay

Centered card, dismissable by clicking outside, the `X`, `?`, or `Esc`. Content
is a real two-column layout passed from Rust:

```
export struct HelpRow { keys: string, text: string }
in property <[HelpRow]> help-rows;
```

Grouped sections (Tools, Actions, Navigation), each row = key chips + text.
Replaces `help-text: string` and the giant wrapped blob. A click-catcher
`TouchArea` behind the card closes it.

### 6. Pin window

- 1 px border (`panel_border`) and rounded corners to read as a card.
- Hover toolbar with a close `X` (`IconButton`).
- Bottom-right resize grip: pointer `move` while dragging emits a callback with
  the new size; Rust calls `ui.window().set_size(...)`. Slint has no built-in
  layout resize, so this is the mechanism.
- Keep `WindowMoveArea` drag and right-click / `Esc` dismiss.

## Rust changes

`src/editor.rs`

- `refresh_toolbar`: content-driven width, wrap decision, below/above/clamp;
  set `toolbar_x/y/width`, new `toolbar_wrap`/`toolbar_height`.
- `refresh_hint` (→ `refresh_chrome`): publish `can-undo`, `can-redo`, status
  text, shortcut chips, active tool/colour, width/text-size readouts.
- New `help_rows()` building `Vec<HelpRow>` from `HELP_TEXT` data; `lib.rs`
  sets them at construction.
- `apply_theme`: map the new colour/geometry/font fields into the Slint `Theme`
  global (and keep the existing mappings).
- `run_pin` (`lib.rs`): handle the resize callback.
- `TOOLBAR_W`/`TOOLBAR_H` constants removed.

`src/lib.rs`

- `run_pin`: wire `on_resize`; set the new theme/pin properties.
- `show_editor`: set help rows instead of `set_help_text`.

`ui/main.slint`

- Rewrite toolbar, settings, help, hint bar, pin using `icons.slint`.
- `Theme` global loses its hardcoded literals and mirrors the shared theme
  (all values pushed from Rust, with built-in defaults in the Slint file).

`build.rs` — unchanged. The vendored file is imported by relative path, so no
`CompilerConfiguration::with_library_paths` is needed.

## Shared theme extension

Add serde-defaulted fields to **both** `crates/pocshot-config/src/theme.rs` and
`crates/pocshot-gui/src/theme.rs` (they are duplicate `Theme` structs — one over
`ecolor::Color32`, one over `eframe::egui::Color32 — and both write the same
`config.json`, so an egui save would otherwise drop new fields):

| Field | Default | Purpose |
|---|---|---|
| `colors.panel_bg` | `#1b1b1ff2` | settings/help/pin card background |
| `colors.panel_border` | `#3a3a42` | card border |
| `colors.control_track` | `#3a3a42` | switch/slider track |
| `geometry.icon_size` | `18.0` | Lucide icon edge length (px) |
| `geometry.icon_stroke` | `1.75` | Lucide stroke width |
| `geometry.panel_radius` | `12.0` | card corner radius |
| `geometry.control_radius` | `8.0` | switch/slider corner radius |
| `geometry.toolbar_padding` | `8.0` | toolbar inner padding |
| `fonts.panel_title` | `15.0` | card title |
| `fonts.section` | `12.0` | section label |
| `fonts.row` | `13.0` | settings row label |
| `fonts.shortcut` | `11.0` | shortcut chip / tooltip hint |

Both structs must stay in sync; existing `Theme` tests (roundtrip, partial-fill,
legacy migration) continue to pass because every field has a serde default.

The two `Theme` copies should eventually be unified (egui depending on
`pocshot-config`). This design keeps them in sync but does not perform that
refactor.

## Testing & verification

- Unit: `cargo test -p pocshot-config -p pocshot-ui-slint`; the config tests
  cover theme roundtrip/legacy migration. Add a test that a theme written with
  the new fields round-trips through both `Theme` types.
- Build/lint: `cargo build -p pocshot-ui-slint`, `cargo clippy -p pocshot-ui-slint`.
- Manual (Linux): run `POCSHOT_RENDERER=software cargo run -p pocshot -- --ui slint`
  and the default GPU path. Verify:
  - icons crisp at the current DPI, correct glyph per slot;
  - toolbar flips above near the bottom edge, clamps at edges, wraps on a
    narrow window, and does not swallow canvas clicks;
  - Undo/Redo disabled with empty/consumed stacks;
  - tooltips (above and below) with shortcuts;
  - settings switches/sliders mutate state and persist to `config.json`;
  - help overlay opens/closes by all four routes;
  - pin drag, resize, close, right-click.
- Windows build remains unverified (existing caveat); this change must not add
  new platform assumptions.

## Risks

- **Software-renderer icon cost.** `IconDisplay` emits `Path` elements, which
  Slint's software renderer rasterizes per frame. ~27 small static icons should
  be cheap, but this is the one real unknown. Mitigation: measure frame cost on
  the software backend; if it regresses, pre-rasterize the icons to `Image`s
  (the `Icon` data is unchanged, only the `IconDisplay` implementation swaps).
- **No native shadow / colour picker.** Emulated with a translucent rect and a
  swatch grid + preview; no free-form picker.
- **Vendored drift.** Icons are frozen at lucide-slint 1.48.0; updating means
  re-extracting. Acceptable for a fixed icon set.
- **Theme duplication.** Adding fields to only one struct would silently lose
  them on the other UI's save; the design requires touching both.

## Out of scope

- Unifying the two `Theme`/`config.rs` copies.
- Any egui rendering changes.
- Tray, capture, OCR, snap-engine changes.
- Windows/multi-monitor verification (tracked separately).
