# Pocshot GUI Architecture

## Overview

Pocshot uses a single unified binary (`pocshot`) that dispatches to either an interactive GUI or CLI subcommands based on arguments.

- No arguments or `gui` subcommand → fullscreen egui/eframe selection GUI
- `capture` subcommand → CLI capture
- `pin` subcommand → borderless pinned image window
- `edit` subcommand → fullscreen editor on the image currently on the clipboard
- `tray` subcommand → background tray daemon
- `list` subcommand → list monitors/windows as JSON

## Crate layout

| Crate | Type | Responsibility |
|-------|------|----------------|
| `pocshot-core` | lib | Capture (xcap), save (PNG/JPEG), clipboard (arboard), platform helpers, error dialog, types |
| `pocshot-snap` | lib | Edge detection and snap line algorithm (imageproc: Sobel → threshold → directional dilate → projection) |
| `pocshot-gui` | lib | egui/eframe GUI: app, canvas, selection, toolbar, snap helpers, OS window snap (`window_snap`) |
| `pocshot-tray` | lib | System-tray daemon (`pocshot tray`): tray-rs (`tray` + `tray-menu`) |
| `pocshot` | bin | Unified binary with clap dispatch |

## GUI modules (`pocshot-gui`)

- `app.rs` — PocshotApp struct, eframe::App impl, input handling, action dispatch
- `canvas.rs` — coordinate transforms (screen↔image), fitting, clamping, UV
- `selection.rs` — ImageSelection, HandleType, handle rect positions
- `toolbar.rs` — Action enum, ToolButton, toolbar rendering, clipboard toggle
- `snap.rs` — snap_pos() helper, SNAP_DISTANCE constant

## Snap line algorithm (`pocshot-snap` + `window_snap`)

Two region-detection passes are combined:

1. **OS-level windows** (`pocshot-core::list_windows` → `window_snap::window_snap_lines`):
   top-level windows intersecting the captured monitor contribute their four
   outer frame edges. This catches the outer regions the edge pass can't.
2. **Edge detection** (`pocshot-snap`) finds the panels *inside* those windows:

   1. RGBA → grayscale (Luma8)
   2. Sobel gradients: `vertical_sobel` for horizontal edges, `horizontal_sobel` for vertical edges
   3. i16 → u8 absolute, clamp to 255
   4. Threshold at `gradient_threshold` (default 25)
   5. **Directional dilation** — horizontal-only for horizontal edges, vertical-only for vertical edges (bridges gaps without thickening)
   6. Projection — count non-zero pixels per row/column, threshold at `min_line_ratio` fraction (default 10%)
   7. Merge nearby lines within `merge_distance` (default 4px)

Both passes run on a background thread after the capture arrives; window
coordinates are translated from the virtual-desktop origin to image space and
clamped, and windows owned by this process (the GUI itself) are excluded.

### SnapConfig defaults

- `gradient_threshold`: 25
- `min_line_ratio`: 0.10
- `merge_distance`: 4.0
- `dilate_radius`: 7

## Capture surface

- Borderless fullscreen window on launch, created transparent; `clear_color`
  stays fully transparent until the screenshot texture exists, so startup does
  not flash an opaque black fullscreen
- Screen capture runs on a background thread; the window paints immediately and
  swaps in the screenshot when ready (`Capturing screen…` pill meanwhile)
- Background = captured screenshot texture
- Drag on empty canvas to select a region
- **Drag inside an existing selection moves it** (clamped to the image, no
  snapping); drag outside starts a new region
- 8 resize handles (corners + edges) with appropriate cursor icons
- Selection edges snap to detected lines within 8px threshold
- Orange indicator lines shown when snap is active
- Toolbar auto-positions near selection
- **Text border** tool (`T`): drag over detected text and a padded rectangular
  border is committed around the union of the OCR regions under the drag
- **Redact** tool (`D`): same gesture, but commits a solid rectangle in the
  current annotation color, sized to the exact text bounding box plus 1px top
  and bottom (no side padding)
- Both text-region tools clamp their result to the active selection, so a text
  box extending past the selection never draws outside it
- OCR text blocks contribute snap guides only through their outer block borders
  (paragraph-level), so per-line boxes don't flood the snapping engine

## Error reporting

Fatal startup failures (e.g. no usable GPU/OpenGL adapter) are reported through
`pocshot_core::show_error_dialog`: a native `MessageBoxW` on Windows (where the
release GUI has no console), stderr elsewhere. `install_panic_dialog_hook`
routes panics through the same dialog on Windows.

## Tray daemon (`pocshot tray`)

A background process hosting a status icon via tray-rs (`tray` + `tray-menu`):
native X11 system-tray protocol on Linux and `Shell_NotifyIconW` on Windows.
The crate pumps its platform event loop on an internal worker thread, so the
daemon just polls the event channel (no manual Win32 message pump / D-Bus
service).

- Left click, or the "Take screenshot" menu item, spawns `pocshot gui`
  detached via `pocshot_core::{detach, quiet_io}`; the GUI exits as usual
- The "Edit clipboard image" item appears only when the clipboard holds an
  image (`pocshot_core::clipboard_has_image`) and spawns `pocshot edit`
- Right-click popup menu (GTK on Linux, `TrackPopupMenu` on Windows):
  "Take screenshot" / ["Edit clipboard image"] / separator / "Exit" (stops the
  daemon)
- The procedural icon (rounded accent square + lens) is generated in code, so
  there is no binary asset to ship
- Linux menus need GTK3 (the `gtk` feature); the icon itself needs no toolkit
- Requires an X11 session / compositor that hosts the legacy XEmbed tray
  protocol; on pure Wayland (no XWayland tray host) the icon will not appear
- No single-instance guard: two rapid clicks can spawn two GUIs

## Future features

- Automatic note taking
- Context-aware screenshots (window/app metadata)
- Magnifier around the cursor
- Grid snapping

## Constraints

- Screenshot and clipboard errors must be shown in the GUI instead of panicking
- The GUI must not duplicate capture or save logic from the CLI
- Region geometry is tracked in source-image pixels, not screen points
- Platform limitations (Wayland portal, macOS screen-recording permission) remain core-level errors surfaced by the app
