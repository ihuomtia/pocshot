# M1 tool/colour/width summary (Slint UI)

Everything below is also on the bottom toolbar (tools, colour swatches,
width −/+, undo, OCR, copy, save, quit).

Current editor (branch `slint-ui`, crate `crates/pocshot-ui-slint`) supports:

Tools (keyboard):
- `V` select (drag the crop region)
- `R` rectangle outline, `Shift+R` filled rectangle
- `L` line, `A` arrow
- `C` circle, `Shift+C` filled circle
- `P` pen (freehand)
- `H` highlighter
- `D` redact (filled rectangle)
- `N` counter — click drops a numbered bubble, drag extends a callout
- `T` text — click, type, Enter commits (`Esc` cancels, Backspace edits)
- `B` blur, `M` pixelate — drag a region; applied on release (destructive)
- `E` eraser — click an annotation to remove it
- `V` select — drag outside the selection to redraw it, drag inside to move it
- `Ctrl+Z` undoes the last annotation or effect
- Snapping to image/window edges is on by default; hold `Shift` to disable

Colour: keys `1`–`6` pick from the palette.
Stroke width: `[` decrease, `]` increase (1–24).
`Ctrl+Z` undo the last annotation.
`Enter` / `Ctrl+C` copy, `Ctrl+S` save PNG, `Esc` / `q` quit.

Ctrl while dragging constrains to square / circle / 45° (pen and
highlighter lock the angle for the rest of the drag).

Not ported yet: text, counter/step, blur, pixelate, eraser, snapping,
OCR overlay, settings, toolbar buttons (currently keyboard only).
