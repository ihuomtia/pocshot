# Pocshot

Pocshot is a Rust screenshot tool with a unified binary providing both an interactive GUI and a CLI.

## Features

- Interactive fullscreen GUI: capture, drag-select, resize handles, edge-snapping
- Capture the primary monitor or a selected monitor
- Capture a rectangular region on a selected monitor
- Capture a selected non-minimized window where the platform supports it
- Save PNG or JPEG screenshots
- Add a capture delay
- Copy the captured image to the clipboard when requested
- List monitors and windows as JSON for scripting
- Edge detection snap lines: selection edges snap to detected UI boundaries
- Text detection (optional, ONNX): detected text boxes feed the same snapping
  engine and can be visualized as an overlay. See
  [crates/pocshot-ocr/README.md](crates/pocshot-ocr/README.md) for model
  placement.

## Usage

Launch the interactive GUI (default — no subcommand needed):

```sh
cargo run -p pocshot
```

CLI capture and list:

```sh
cargo run -p pocshot -- capture --mode screen --output shot.png
cargo run -p pocshot -- capture --mode region --x 0 --y 0 --width 800 --height 600
cargo run -p pocshot -- list monitors
cargo run -p pocshot -- list windows
```

Build a standalone binary:

```sh
cargo build -p pocshot --release
```

The binary is written to `target/release/pocshot`.

## Project Structure

```
crates/
├── pocshot-core/   # Capture, save, clipboard, types, errors
├── pocshot-ocr/    # Optional ONNX text detection + post-processing
├── pocshot-snap/   # Edge detection and snap line algorithm (imageproc)
├── pocshot-gui/    # egui/eframe GUI library (app, canvas, selection, toolbar, snap)
└── pocshot/        # Unified binary (clap: gui / capture / list)
```

## Linux Dependencies

`xcap` needs native screen capture libraries. On Arch Linux install:

```sh
sudo pacman -S base-devel clang libxcb libxrandr dbus libpipewire
```

On Debian or Ubuntu install:

```sh
sudo apt-get install pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libx11-dev libpng-dev
```

Wayland support depends on compositor and portal availability.
