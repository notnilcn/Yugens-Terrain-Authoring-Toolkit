# Sculpting QA — Brush, Level, Smooth, Bridge, toolbar, undo

Session `sculpt` on display `:31`, port `8811` (Godot 4.7.2, llvmpipe, editor window
1600x900). Scenes: `res://tutorial/02_island.tscn` (Square grid) and
`res://demo/hex_island.tscn` (Hex Rings). Terrain was re-selected after opening a scene.
Nothing under `addons/` or the demo scenes was saved or modified.

Editor log: `/tmp/opencode/qa/sessions/sculpt/editor.log` contains only the known startup
noise (V-Sync, debug adapter / GDScript language server ports) — no `ERROR`, `SCRIPT ERROR`
or addon warning lines were produced by any of the steps below.

## Number keys 1–9 do not switch tools; they change the 3D view orientation

- Severity: warning
- Steps: 1) Select the terrain, click in the 3D view (viewport focused). 2) Press `2`, `3`,
  `4`, then `1`; press `7`. 3) Repeat with the Scene tree focused.
- Expected: The active tool switches to Brush/Level/Smooth/Bridge as stated in
  `docs/02 Your First Terrain.md` ("Click **Brush** (`1`)"), `docs/12 Cheat Sheet.md`
  (Key 1–9 table) and in the toolbar tooltips (`Brush Tool [1]`, `Bridge Tool [4]`, ...).
- Actual: The tool never changes (`ToolTitle` stays `Level`/`Brush`/`Debug Brush`).
  With the 3D view focused, the digits act as Godot's view shortcuts: `7` → `Top Orthogonal`,
  `1` → `Front Orthogonal`. With the Scene tree focused the digits do nothing.
  Tools only change by clicking the toolbar buttons.
- Evidence: qa/screenshots/sculpt/52-key7-view.png (header `Top Orthogonal`),
  qa/screenshots/sculpt/53-key1-view.png (header `Front Orthogonal`),
  qa/screenshots/sculpt/ui/11-key1-front-orthogonal.json (`ToolTitle` = `Level`,
  orientation menu button = `Front Orthogonal`).
- Undo: not applicable (no edit performed).

## Raise / Lower strip clips the tool title and Brush Type row at 1600x900

- Severity: warning
- Steps: 1) Select Brush. 2) Set **Brush Mode** to `Raise / Lower` (default editor window
  1600x900). 3) Look at the bottom strip; try to change **Brush Type**. 4) Mouse-wheel over
  the strip to scroll it.
- Expected: All strip controls stay visible (or the strip scrolls with a visible scrollbar);
  `docs/03 Sculpting.md` describes **Brush Type** as part of the Brush settings and says it is
  shown in the strip.
- Actual: Switching to Raise / Lower makes the flow content 5 rows (175 px) while the strip
  ScrollContainer is 140 px. The top row (`Brush (Raise / Lower)` title + `Brush Type`) is
  laid out at y=448, above the strip viewport (y=483..623), and is clipped. The visible strip
  starts at `Brush Mode`. There is no scrollbar, and wheel events over the strip do not scroll
  it, so Brush Type is unreachable until the user switches back to Drag, changes it, and
  returns. Reproduced on both Square and Hex Rings.
- Evidence: qa/screenshots/sculpt/18-brush-raiselower-strip.png,
  qa/screenshots/sculpt/23c-row1.png, qa/screenshots/sculpt/68-hex-rl-strip-full.png;
  qa/screenshots/sculpt/ui/08-hex-raiselower-clip.json (`ToolTitle`/`Brush Type` rect y=448,
  `Brush Mode` rect y=483).
- Undo: not applicable.

## Shift+wheel / Ctrl+wheel change size and strength but the strip labels stay stale

- Severity: warning
- Steps: 1) Brush → Raise / Lower; note Size `15.0` and Strength `4.000`. 2) Over the 3D view:
  Shift+wheel up ×3, Ctrl+wheel down ×2, Alt+wheel up ×3. 3) Compare the strip with the actual
  values.
- Expected: The numeric labels/slider handles update with the values (the docs advertise
  Shift+wheel as the fast way to set Size and Ctrl+wheel for Strength).
- Actual: The internal values change (UI dump: Size 19.0, Strength 3.256, Rotation 45) and the
  cursor footprint grows/changes, but the strip still displays `15.0` and `4.000` and the slider
  handles do not move. The labels only refresh when the strip is rebuilt (switching tools/modes
  then showed Size `18.9`, i.e. a rounded 19.0). Rotation (a spin slider) does refresh.
- Evidence: qa/screenshots/sculpt/23-lr-wheel-changes.png,
  qa/screenshots/sculpt/23b-lr-wheel-labels-late.png,
  qa/screenshots/sculpt/24-brush-drag-strip-back.png (label refresh after rebuild),
  qa/screenshots/sculpt/ui/07-lr-wheel-stale-values.json (values 19.0 / 3.256).
- Undo: not applicable (settings, not terrain).

## Esc does not clear the Brush / Drag selection (RMB does)

- Severity: warning
- Steps: 1) Brush → Drag. 2) Drag a small selection (status shows `4 selected`). 3) Press
  `Esc` twice. 4) Right-click and compare.
- Expected: Selection clears, per `docs/03 Sculpting.md` ("Right mouse / `Esc` | Clear the
  selection"), `docs/12 Cheat Sheet.md` and the on-screen hint `RMB / Esc: clear`.
- Actual: After both Esc presses the status still reads `4 selected` and the white selection
  remains. Right-click does clear it. The stale selection is not inert: a later LMB-drag that
  starts inside it moves the selection (height draw) instead of painting a new selection —
  this happened during the test.
- Evidence: qa/screenshots/sculpt/69-esc-clears-selection.png,
  qa/screenshots/sculpt/69b-esc-second-press.png,
  qa/screenshots/sculpt/69c-esc-clean-test.png (fresh selection + Esc, still `4 selected`),
  qa/screenshots/sculpt/14-brush-rmb-clear.png (RMB clears).
- Undo: not applicable (selection is editor state, not an undo step); the accidental height
  draw it caused was undone with Ctrl+Z.

## Grid Aligned looks enabled on Square grid but silently refuses to toggle

- Severity: minor
- Steps: 1) Square grid, Brush, Falloff **off**. 2) Click **Grid Aligned** (checkbox and Space
  with focus). 3) Repeat on Hex Rings.
- Expected: The control should be disabled / grayed out when unsupported, as
  `docs/03 Sculpting.md` describes ("If **Grid Aligned** is grayed out, switch back to Brush,
  turn **Falloff** off"), or it should toggle.
- Actual: On Square the checkbox reports `disabled=false`, looks normal, and clicking it does
  nothing (no check, no feedback). Its tooltip says it needs a Triangle/Hexagon/Hex Rings grid.
  On Hex Rings it toggles on, and enabling Falloff auto-unchecks it (correct).
- Evidence: qa/screenshots/sculpt/30b-gridaligned-click2.png,
  qa/screenshots/sculpt/30c-gridaligned-space.png,
  qa/screenshots/sculpt/59-hex-gridaligned.png,
  qa/screenshots/sculpt/70-hex-falloff-grid.png,
  qa/screenshots/sculpt/ui/09-gridaligned-square.json (`toggled:false`, `disabled:false`).
- Undo: not applicable.

## Hint/status text is clipped, and wheel hints disagree with the visible controls

- Severity: minor
- Steps: 1) Brush → Drag, make a selection and read the status line. 2) Switch to Raise / Lower
  and to Level and compare the hint lines with the strip controls.
- Expected: The status/hint fits the viewport (or wraps), and hints only advertise controls that
  exist.
- Actual: The status line is cut at the right edge at 1600x900 (`... RMB/Esc: c...`). The
  Raise / Lower hint lists `Shift+wheel: size` but omits `Ctrl+wheel: strength` (documented and
  working). The Level hint says `Ctrl+wheel: strength`, but the Level strip has no Strength
  field or value to observe.
- Evidence: qa/screenshots/sculpt/69c-esc-clean-test.png (clipped `RMB/Esc: c...`),
  qa/screenshots/sculpt/18-brush-raiselower-strip.png (hint),
  qa/screenshots/sculpt/31-level-strip.png and
  qa/screenshots/sculpt/ui/04-level-strip.json (no Strength control).
- Undo: not applicable.

## Covered

- [x] Selected terrain; toolbar and **Collapse** compact mode (icon-only, tooltips still work);
      expanded again.
- [x] **Grid** overlay on/off (visible cell lines change; overlay toggle is itself an undoable
      `terrain grid overlay` action).
- [x] Tool strips for **Brush** (Drag and Raise / Lower), **Level**, **Smooth**, **Bridge**
      (`docs/03` layout matches except the Raise/Lower clipping above).
- [x] Brush / Drag: paint white selection; selection count and white band; Shift add (160→267);
      Ctrl remove (267→192); Alt drag moves exactly one cell (verified by height diff);
      RMB clears; Esc **fails** (finding).
- [x] Brush / Raise-Lower: LMB hold raises, RMB hold lowers, LMB+RMB flattens toward Height;
      Ctrl+LMB / Ctrl+MMB / Ctrl+RMB pick Ceiling / Height / Floor; values read back from the
      strip.
- [x] Wheel shortcuts: Shift size, Ctrl strength, Alt rotation (values change; labels stale —
      finding).
- [x] Brush Type Round / Square / Hexagon / Triangle footprints (green outlined footprint
      follows the cursor, centered on the picked cell); Size 50, Aspect 5, Rotation 45 footprint
      stretches/rotates as expected; Falloff on/off toggles.
- [x] Level: drag flattens 287 cells to the target Height; Ctrl+LMB picks height; Level works on
      Square and Hex Rings.
- [x] Smooth: Terraced and Sloped both change terrain, Strength `0.15`; both undoable; works on
      Hex Rings.
- [x] Bridge: two-point drag builds a ramp (new heights 3.1..6.1 between 3.09 and 6.10
      endpoints); equal-height bridge flattens the connecting band; Size changes width;
      Ease Value control present; works and undoes.
- [x] Toolbar Collapse, Grid, tooltips (Bridge tooltip shown), 1–9 keys **fail** (finding).
- [x] Undo/redo matrix (see `qa/screenshots/sculpt/undo-matrix.txt`):
      one stroke → Ctrl+Z restores digest → Ctrl+Shift+Z restores edit for Brush move,
      Level, Smooth (Terraced and Sloped), Bridge; two strokes → exactly two Ctrl+Z steps;
      undo after switching tools (Level → Smooth) works; undo after clicking in the 3D view
      works; Ctrl+Z with a LineEdit focused edits the text (terrain untouched); cross-chunk
      Level stroke touching 5 chunks restores all per-chunk hashes in one step; redo stack is
      cleared by a new edit.
- [x] Hex grid: Brush selection, Level, Smooth, Grid Aligned; no hex-specific failures found.
- [x] Editor log scanned before/after: no new errors/warnings.

### Not tested / not conclusive

- **Quick Paint** options were enumerated (`None`, `Full Void`, `Base Grass Masked`,
  `Selected Material`) but no Quick Paint stroke was verified visually (belongs to the Paint
  area).
- **Flatten off vs on** in Brush Drag was toggled but the moved-shape difference was not
  measured; **Peak** and **Grid Size** sliders were present but not exercised beyond defaults.
- Brush Type `Hexagon30` and `Triangle180` were listed but not screenshotted (Round, Square,
  Hexagon, Triangle were).
- The exact Ease Value curve shapes (1, 2, 0.5, -1, -5) were not compared visually; the default
  `-1.0` ramp was proven.
- Ctrl+wheel over the Level tool may change an invisible strength; only the hint/controls
  mismatch is recorded.
