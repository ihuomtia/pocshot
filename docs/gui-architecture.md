# Pocshot GUI Architecture

## Overview

Pocshot uses a single unified binary (`pocshot`) that dispatches to either an interactive GUI or CLI subcommands based on arguments.

- No arguments or `gui` subcommand → fullscreen egui/eframe selection GUI
- `capture` subcommand → CLI capture
- `list` subcommand → list monitors/windows as JSON

## Crate layout

| Crate | Type | Responsibility |
|-------|------|----------------|
| `pocshot-core` | lib | Capture (xcap), save (PNG/JPEG), clipboard (arboard), types, errors |
| `pocshot-snap` | lib | Edge detection and snap line algorithm (imageproc: Sobel → threshold → directional dilate → projection) |
| `pocshot-gui` | lib | egui/eframe GUI: app, canvas, selection, toolbar, snap helpers |
| `pocshot` | bin | Unified binary with clap dispatch |

## GUI modules (`pocshot-gui`)

- `app.rs` — PocshotApp struct, eframe::App impl, input handling, action dispatch
- `canvas.rs` — coordinate transforms (screen↔image), fitting, clamping, UV
- `selection.rs` — ImageSelection, HandleType, handle rect positions
- `toolbar.rs` — Action enum, ToolButton, toolbar rendering, clipboard toggle
- `snap.rs` — snap_pos() helper, SNAP_DISTANCE constant

## Snap line algorithm (`pocshot-snap`)

1. RGBA → grayscale (Luma8)
2. Sobel gradients: `vertical_sobel` for horizontal edges, `horizontal_sobel` for vertical edges
3. i16 → u8 absolute, clamp to 255
4. Threshold at `gradient_threshold` (default 25)
5. **Directional dilation** — horizontal-only for horizontal edges, vertical-only for vertical edges (bridges gaps without thickening)
6. Projection — count non-zero pixels per row/column, threshold at `min_line_ratio` fraction (default 10%)
7. Merge nearby lines within `merge_distance` (default 4px)

### SnapConfig defaults

- `gradient_threshold`: 25
- `min_line_ratio`: 0.10
- `merge_distance`: 4.0
- `dilate_radius`: 7

## Capture surface

- Borderless fullscreen window on launch
- Captures primary monitor immediately
- Background = captured screenshot texture
- Drag to select region
- 8 resize handles (corners + edges) with appropriate cursor icons
- Selection edges snap to detected lines within 8px threshold
- Orange indicator lines shown when snap is active
- Toolbar auto-positions near selection

## Future features

- Auto-region detection from captured pixel buffer
- Magnifier around the cursor
- Annotation layers over the selected crop
- Grid snapping

## Constraints

- Screenshot and clipboard errors must be shown in the GUI instead of panicking
- The GUI must not duplicate capture or save logic from the CLI
- Region geometry is tracked in source-image pixels, not screen points
- Platform limitations (Wayland portal, macOS screen-recording permission) remain core-level errors surfaced by the app
