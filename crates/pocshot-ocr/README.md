# pocshot-ocr

Optional, pluggable text detection for Pocshot.

Pure-Rust ONNX inference via [`tract-onnx`] — no native/C++ runtime, no network
calls at processing time. Detection only; recognition (`TextRegion.text`) is
kept separate so a recognition model can be added later without touching the
snapping or rendering layers.

## Pipeline

```
screenshot (RgbaImage)
  → letterbox into a fixed square canvas (aspect preserved, pad 114)
  → BGR normalize → NCHW floats
  → onnx DBNet inference (tract-onnx)
  → decode probability map → raw boxes (model coords)
  → undo letterbox offset/scale → original screenshot coordinates
  → confidence filter → line-group merge → TextRegion[]
  → text_regions_to_guides() → SnapGuides feeding the app's snapping engine
```

The ONNX input is pinned to a **fixed concrete square** shape
(`max_side`, default 960, rounded up to a multiple of `stride` 32). DBNet
backbones have deep stride/concat chains; leaving the input fully dynamic makes
tract's shape inference fail to unify the symbolic dims, so we commit to one
resolution and letterbox every screenshot into it. All coordinate mapping is
handled by the detector so snapping never sees an offset.

Defaults (`DbDetectorConfig::default`) follow the common PaddleOCR PP-OCR DBNet
export conventions (`max_side=960`, stride 32, mean/std
`(0.485,0.456,0.406)/(0.229,0.224,0.225)`, BGR, pad 114). For the
**docTR / OnnxTR** model family use [`DbDetectorConfig::doctr`]: fixed 1024×1024
RGB input, docTR per-channel mean/std `(0.798,0.785,0.772)/(0.264,0.2749,0.287)`,
black padding, `box_score_threshold=0.1`. Everything is tunable via
`DbDetectorConfig`.

## Model placement

Detection is off until three things are true: the app setting "Enable text
detection" is on, the model file exists, and hardware supports it. The model
is resolved, in order:

1. the **Model path** field in Settings → `...`, or
2. the `POCSHOT_OCR_MODEL` environment variable, or
3. the default location: `$XDG_CONFIG_HOME/pocshot/models/text_det.onnx`
   (`~/.config/pocshot/models/text_det.onnx` on most Linux setups).

Place any DBNet-family ONNX export there, e.g. the widely available
PaddleOCR PP-OCRv4/v5 DBNet detection export (det.onnx). It is loaded **once**
per app run and reused for every capture; the app never downloads models.

### Recommended: docTR / OnnxTR detection model

For tighter, document-aware boxes (good for screenshots) use the docTR DBNet
models shipped as raw ONNX by [OnnxTR](https://github.com/felixdittrich92/OnnxTR):

```sh
mkdir -p ~/.config/pocshot/models
curl -L -o ~/.config/pocshot/models/db_mobilenet_v3_large_8bit.onnx \
  https://github.com/felixdittrich92/OnnxTR/releases/download/v0.2.0/db_mobilenet_v3_large_static_8_bit-535a6f25.onnx
```

Then point the app's **Model path** at that file and run e.g.:

```sh
cargo run -p pocshot-ocr --example ocr_smoke -- \
  ~/.config/pocshot/models/db_mobilenet_v3_large_8bit.onnx /path/to/screenshot.png --preset doctr
```

Other docTR weights: `db_mobilenet_v3_large` fp32
(`v0.2.0/db_mobilenet_v3_large-4987e7bd.onnx`), `db_resnet34`
(`v0.0.1/db_resnet34-b4873198.onnx`), `db_resnet50`
(`v0.0.1/db_resnet50-69ba0015.onnx`). All need `--preset doctr` normalization
(RGB, docTR stats, 1024×1024).

docTR exports declare a dynamic `batch_size` dim but bake literal `1` for it
in places (common with quantized models). tract's loader handles this by
keeping the batch dimension symbolic during shape analysis and binding the
symbol to 1 with `set_symbols` *after* typing — see the two-strategy loader in
`detect.rs` (`load_typed_runnable`); it first tries a fully concrete input
fact, then falls back to symbol binding.

## Testing

```sh
cargo test -p pocshot-ocr
```

All geometry logic is tested with synthetic data (probability masks, rects,
settings) — no model file required for `cargo test`.

## Debugging

In the app's Settings panel: enable "Enable text detection", "Show text
bounding boxes" and "Show OCR debug zones" (raw boxes with confidence values),
then use Capture/Recapture. A full screen capture is loaded automatically at
startup if OCR is enabled, so you can tune detection without the whole flow.