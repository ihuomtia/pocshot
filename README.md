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
- Edit an image already on the clipboard in the same fullscreen annotator
  (`pocshot edit`, or the tray menu when the clipboard holds an image)
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

Edit the image currently on the clipboard (errors if it does not hold an image):

```sh
cargo run -p pocshot -- edit
```

Build a standalone binary:

```sh
cargo build -p pocshot --release
```

The binary is written to `target/release/pocshot`.

## GPU-less machines (RDP, VMs)

The default binary carries both of eframe's renderers and picks one at startup:

- **OpenGL** (eframe's glow backend) when it initializes. On Linux that still
  works without a GPU via Mesa's llvmpipe; on Windows a machine with no real
  OpenGL 3.x driver (RDP sessions, GPU-less VMs) only exposes GDI's OpenGL 1.1.
- **Software** otherwise: wgpu on a CPU rasterizer (WARP on Windows, lavapipe on
  Linux). It needs no extra runtime DLLs — shader compilation falls back from DXC
  to the bundled FXC when DirectX Shader Compiler is absent — and is
  intentionally slower.

`auto` probes adapters once at startup. If only a CPU adapter is present
(WARP on Windows, lavapipe on Linux), it goes straight to the software
renderer without attempting OpenGL, so there is no panic and no process
relaunch on GPU-less machines. If a machine reports a hardware adapter but
OpenGL setup still fails, the process relaunches itself with
`POCSHOT_RENDERER=software` as a backstop.

To force a renderer:

```sh
pocshot --software                  # CPU (wgpu) renderer, any subcommand
pocshot --software gui              # same, explicit
POCSHOT_RENDERER=gpu pocshot        # OpenGL only; surface the error if unusable
POCSHOT_RENDERER=software pocshot   # CPU rasterizer only
```

The `--software` flag is equivalent to `POCSHOT_RENDERER=software` and takes
precedence over it. `POCSHOT_RENDERER=gpu` also bypasses the adapter probe and
forces the glow attempt — use it for the rare machine where OpenGL works but
wgpu enumerates no hardware adapter. Note: wgpu's CPU path needs a software
Vulkan driver — `lavapipe` on Linux (Arch: `vulkan-swrast`), WARP on Windows.
Without it wgpu picks the real GPU.

Slim single-renderer builds (smaller binary, no fallback):

```sh
cargo build --release -p pocshot --no-default-features --features gpu
cargo build --release -p pocshot --no-default-features --features software
```

`just build-soft` and `just build-win-soft` wrap the software-only builds and
copy the result to `pocshot-soft` / `pocshot-soft.exe`.

## Project Structure

```
crates/
├── pocshot-core/   # Capture, save, clipboard, types, errors
├── pocshot-ocr/    # Optional ONNX text detection + post-processing
├── pocshot-snap/   # Edge detection and snap line algorithm (imageproc)
├── pocshot-gui/    # egui/eframe GUI library (app, canvas, selection, toolbar, snap)
└── pocshot/        # Unified binary (clap: gui / capture / edit / tray / list / debug)
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
