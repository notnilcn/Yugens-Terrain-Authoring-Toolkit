# Area C02 — page `03-terrain-sculpting`
Session: `docs-sculpt`, display `:72`, port `8872`, scene `res://tutorial/01_flat_playground.tscn` (also use `res://demo/hex_island.tscn` for hex-grid behaviour).
Extra reading: `qa/FEATURE-MAP.md` Areas 1 and 3 (Stamp/Gradient/Select/Inspect), source `addons/MarchingSquaresTerrain/rust/crates/mst/src/editor/` (tools.rs, terrain_ui.rs, extra_tools.rs, cursor.rs) and `.../terrain/`.

Cover everything about shaping ground:
- The left terrain toolbar (appears when a MarchingSquaresTerrain is selected): every tool button, number-key shortcuts, Collapse, Grid toggle, tooltips; the bottom attribute strip and the hint bar.
- Brush: Drag mode (selection painting: Shift add, Ctrl remove, Alt one cell, RMB/Esc clear; drag selected cells up/down) and Raise/Lower mode (LMB raise toward Ceiling, RMB lower toward Floor, both = flatten to Height, Ctrl+LMB/MMB/RMB pick), Brush Type (every type), Size, Aspect, Rotation, Flatten, Falloff, Peak, Grid Aligned, Grid Size, Quick Paint; wheel shortcuts. Knob series for Size, Falloff, Peak, Aspect.
- Level, Smooth (Terraced vs Sloped, Strength), Bridge (ramp A→B, width).
- Stamp (each built-in stamp: crater, hill, mesa, mountain, valley; size/aspect/rotation/mode; drag-to-size; modifier wheels), Gradient, Select (copy C, paste V, rotate Z/X, mirror M, paste, leave), Inspect (Output panel).
- Erosion (`MarchingSquaresErosion` node/tool if present: how to add, settings, run), any other shaping tool you find in the toolbar.
- Undo/redo for strokes. Square vs hex grid differences when sculpting.
- Recipes with clips: a hill, a cliff/plateau, a valley with a river bed, terraced farmland, a ramp up a cliff, a crater.
- Create `res://tutorial/guide/sculpt_playground.tscn` (a flat terrain ready to sculpt) and copy it into the repo.
- Roblox bridge: Roblox Terrain Editor (Add/Subtract/Grow/Erode/Smooth/Flatten/Paint, brush size/strength) vs these tools.
