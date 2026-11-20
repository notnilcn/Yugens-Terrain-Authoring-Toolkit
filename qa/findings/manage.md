# Area 3 — Inspect, Chunks, Settings, Stamp, Gradient, Select, Tint

Session: `manage`, display `:33`, port `8813`, 1600×900 editor window, scene `res://tutorial/02_island.tscn`.
All coordinates are editor-window coordinates. Undo was always driven with `key z --ctrl`
(and `--ctrl --shift` for redo), never the direct `undo` command.

## Settings enum / slot dropdowns never apply (Grid Type, Blend Mode, Floor Slot, Debug View, Storage Mode, …)

- Severity: error
- Steps:
  1. Select the terrain, click **Settings**, stay on **Grid**.
  2. Change **Grid Type** from `Square` to `Hex` (click the popup item, or keyboard).
  3. The dropdown label changes to `Hex`, but the viewport, the Inspector (still `Square`) and
     `prop grid_type` (still `0`) do not change; `undo-state` gains no action.
  4. Same on **New Chunks → Floor Slot** (`2. Meadow` is shown, `default_floor_slot` stays `0`),
     **Display → Debug View** (`Slope` shown, `debug_view` stays `0`) and **Storage → Storage Mode**
     (`Baked` shown, `storage_mode` stays `0`, the page never exposes Save Data / Data Directory).
  5. Any later strip rebuild (e.g. change Cell Size) snaps the label back to the real value.
- Expected: the property is written through the undo manager and the terrain rebuilds (the code
  has a `terrain setting …` action for these rows).
- Actual: only the OptionButton text changes. No property write, no undo step, no rebuild.
  `ui_kit::option_button` uses integer item ids; the value is converted with
  `value.try_to::<f64>()` in `terrain_setting_changed`, which fails for the `int` emitted by
  `item_selected` and falls back to `0` (`new == old` → early return). Numeric SpinBoxes
  (float) and CheckBoxes work, which matches this theory.
- Evidence: `qa/screenshots/manage/20-settings-hex-select.png` (label `Hex`, Inspector `Square`);
  `qa/screenshots/manage/38-storage-mode-baked.png` (label `Baked`, Resource layout);
  `qa/screenshots/manage/ui/20-settings-hex-select.json`, `.../ui/38-storage-mode.json`;
  `prop grid_type` = `0`, `prop storage_mode` = `0`, `undo-state` unchanged.
- Undo: not applicable — no action is created.

## Save Data / Data Directory are unreachable through Settings

- Severity: warning
- Steps: 1) Settings → Storage with `Storage Mode = Resource`. 2) Choose `Baked` or `Runtime`.
  3) The Baked-only rows (Bake Grass, Bake Collision, Data Directory, Save Data) never appear;
     only Storage Mode, Import, Bake and Export are shown. 4) Setting `storage_mode = 1` with
    `set-prop` (Inspector) and reopening Storage correctly shows all of them.
- Expected: choosing Baked/Runtime exposes Data Directory + Save Data, as documented in
  `docs/02 Your First Terrain.md` ("use Settings → Storage → Save Data").
- Actual: because of the enum bug above, the mode never changes, so the documented Save Data
  button cannot be reached from the UI. Workaround: Inspector `Storage Mode` property.
- Evidence: `qa/screenshots/manage/38-storage-mode-baked.png` (label Baked, Resource page) vs
  `qa/screenshots/manage/43-storage-baked-page.png` (property set to 1: Bake Grass/Bake Collision/
  Data Directory/Save Data appear).
- Undo: not applicable.

## Grass Animation FPS change is not undoable

- Severity: warning
- Steps: 1) Settings → Grass. 2) Set **Animation FPS** from `0` to `12` (typed) then to `13`
  (up arrow). 3) The spin value updates and the shader setting is applied. 4) `undo-state` remains
  on the previous action; there is no `terrain grass animation fps` entry; Ctrl+Z cannot return
  the value to 0.
- Expected: the special-case branch in `terrain_setting_changed` creates an undo action
  (`add_do_method`/`add_undo_method` on the turf material).
- Actual: no undo step; the value is silently lost from history (it stays at 13).
- Evidence: `qa/screenshots/manage/28-grass-fps.png` (field shows 12, Output/undo unchanged).
- Undo: broken (setting not undoable).

## Chunk Size is editable but can never change; the undo step is a no-op

- Severity: minor (ui + misleading history)
- Steps: 1) Settings → Grid. 2) Type `24` (or click the arrows) in **Chunk Size**.
  3) The field snaps back to `32`; Output prints
  `WARNING: MarchingSquaresTerrain: chunk_size is fixed once chunks exist for this grid type
  (clear the data or switch grid type first).` 4) `undo-state` has gained
  `terrain setting chunk size`, but Ctrl+Z / Ctrl+Shift+Z change nothing.
- Expected: the field is disabled (or a visible inline hint / confirmation explains that chunk
  size is fixed while chunks exist). The no-op undo step should not be recorded.
- Actual: editable field + warning only in Output + useless undo step.
- Evidence: `qa/screenshots/manage/23-settings-chunksize.png`,
  `qa/screenshots/manage/24-settings-chunksize-redo.png`; editor.log line
  `WARNING: MarchingSquaresTerrain: chunk_size is fixed once chunks exist ... node.rs:1854`.
- Undo: creates a no-op step.

## Settings tab row jumps vertically between pages; taller pages push the strip up

- Severity: ui
- Steps: 1) Settings → Grid (tabs at y≈378). 2) Switch to New Chunks / Grass / LOD / Storage.
  3) The tab row moves to y≈448 / 483 / 518 because the strip grows upward from the bottom.
  4) Clicking at the old tab position after a taller page opens lands in the 3D view (and in one
     reproduction it cleared the terrain selection, making the whole toolbar/strip disappear).
- Expected: a stable tab row (top-anchored or fixed height), or the strip scrolls instead of
  changing height. The page content itself fits at 1600×900, so this is only the jump.
- Actual: every page has a different strip height; muscle-memory tab clicks can miss.
- Evidence: `qa/screenshots/manage/17-settings-grid.png` (tabs y=378),
  `.../25-settings-new-chunks.png` (y=448), `.../27-settings-grass.png` (y=483),
  `.../35-settings-lod.png` (y=518), `.../43-storage-baked-page.png` (y=448).
- Undo: not applicable.

## Stamp Size field stays stale after a drag-stamp or modifier+wheel

- Severity: ui
- Steps: 1) Stamp tool, Mountain, size 24. 2) Drag from the viewport to pull size/rotation.
  3) The hint updates to `stamp 15° size 63.6 height 8.0`, and Rotation/Height fields update,
     but the **Size** field still reads `24.0`. 4) Shift+wheel and Alt+wheel: hint shows
     `stamp 60° size 77.0 height 9.7`, Rotation shows 60, Height 9.6, Size still `24.0`.
- Expected: the strip's Size value reflects the value that was used (and will be used next).
- Actual: Size display is stale; only the transient hint shows the truth. The next click stamps
  with the displayed 24 (wheel values may be reset?). The drag/wheel itself did apply to the
  stamp that was placed (digest changed).
- Evidence: `qa/screenshots/manage/49-stamp-drag.png` (hint 63.6 vs Size 24.0),
  `qa/screenshots/manage/52-stamp-alt-wheel.png` (hint 77.0 vs Size 24.0).
- Undo: stamp drag/click are individually undoable (`terrain stamp`).

## Gradient Strength slider ignores click position and responds oddly to drags

- Severity: minor
- Steps: 1) Gradient tool (Strength 0.301). 2) Click near the right end of the Strength slider
  (x=680 in a 608..688 track): value stays 0.300. 3) Drag right from x=620 to x=675: value
  goes 0.301 → 0.279. 4) Click x=615 then x=665: value ends 0.2345. 5) A ramp painted at 0.279
  still changes the terrain (digest differs) and undoes as one `terrain gradient` step.
- Expected: a normal HSlider jumps to the clicked position (right end ≈ 1.0) or at least drags
  monotonically with the cursor.
- Actual: clicks do not map to the cursor position; the value drifts slightly. Use Ctrl+wheel
  (docs) or the spin value if precise strength matters.
- Evidence: `qa/screenshots/manage/70-gradient-strength.png`;
  `ui` dumps of the slider (`value=0.3005` after a right-end click, `0.2345` after left→right clicks).
- Undo: gradient strokes undo correctly; the slider itself is tool state, not undoable.

## Grid Size stays enabled while Grid Aligned is off

- Severity: ui (minor)
- Steps: 1) Brush tool (also Gradient/Tint). 2) **Grid Aligned** is unchecked.
  3) The **Grid Size** HSlider at `[626,588,80,35]` still reports `disabled=false` and shows `1`.
- Expected (per FEATURE-MAP): Grid Size is a dependent control that should disable when Grid
  Aligned is off (or the UI should explain that it pre-sets the value for when alignment is on).
- Actual: it looks active and draggable with alignment off.
- Evidence: `qa/screenshots/manage/ui/70-brush-gridaligned.json` (checkbox unchecked, HSlider
  `disabled=false`); screenshot `01-initial.png` shows the unchecked box with an active slider.
- Undo: not applicable.

## Invalid chunk add only reports a raw warning in Output

- Severity: minor
- Steps: 1) Chunks tool. 2) Click a green `+` handle that is not adjacent to an existing chunk
  (water edge). 3) Output prints
  `WARNING: Marching Squares Terrain: new chunks must touch an existing chunk.` and nothing else
  happens. 4) Valid `+` clicks add a chunk and undo correctly.
- Expected: invalid positions are either not offered (no green `+`) or explained with a transient
  in-viewport hint; the same warning should not be the only feedback.
- Actual: raw warning in Output; the handle was drawn, so the user is invited to click it.
- Evidence: `qa/screenshots/manage/08-chunks-removed.png` (warning visible in Output);
  editor.log line `WARNING: Marching Squares Terrain: new chunks must touch an existing chunk.
  ... plugin.rs:1244`.
- Undo: valid add/remove are undoable; invalid click makes no change.

## Log scan

Only the known startup noise plus the two warnings above:

```
WARNING: Could not set V-Sync mode ... (known llvmpipe noise)
WARNING: Marching Squares Terrain: new chunks must touch an existing chunk.  (plugin.rs:1244)
WARNING: MarchingSquaresTerrain: chunk_size is fixed once chunks exist ... (node.rs:1854) ×2
```

No `ERROR:` / `SCRIPT ERROR:` from `mst`/`MarchingSquares`, no crash, no frozen dialog.

---

## Covered

- **Inspect**: cell LMB prints `global Vector3 … sample (x, y) chunk IVec2 … local … height … floor
  … wall … grass … smooth …` to Output; clicking sky/water without a cell prints nothing and
  raises no errors (`03-inspect-cell-output.png`, `04-inspect-sky-click.png`). Note: two of my
  early clicks landed on the Grid toolbar toggle, which is itself undoable (`terrain grid overlay`).
- **Chunks**: strip layout (dropdown + count + Remove Chunk + Merge Mode + Apply Mode to All
  Chunks + Vertex Handles + hint) fits; green `+` adds a chunk (digest + `add chunk` undo);
  Red X / Remove Chunk removes the selected chunk (`remove chunk` undo); Ctrl+LMB selects a chunk
  and updates the dropdown; Merge Mode dropdown (Cubic…Spherical) creates `chunk merge mode`
  undo; Apply Mode to All Chunks creates a second step and changes the mesh digest; undo/redo of
  add, remove, merge and apply-all verified with `terrain` digests (`05`–`16`).
- **Settings — all six pages** screenshotted: Grid (`17`), New Chunks (`25`), Grass (`27`),
  Display (`31`), LOD & Nature (`35`), Storage (`37` and Baked `43`). All fit at 1600×900 with no
  clipped labels.
- **Settings values that do work**: Cell Size (2.0 → 3.015, `terrain setting cell size`, undo to
  2.0); Noise Seed (→7, `terrain setting noise seed`); Noise Amp (→1.005, `terrain setting noise
  amp`); Grass Size (0.808/0.65, `terrain setting grass size`, undo/redo); Grass checkbox
  (`terrain setting grass`, undo); Pixel Size (1→2, `terrain setting pixel size`, undo); Editor
  Grid (checkbox, `terrain setting editor grid`, undo); LOD Distance (0→100, `terrain setting lod
  distance`, undo); Data Directory (text, `terrain setting data directory`); all undoable with
  Ctrl+Z / Ctrl+Shift+Z. Erosion Add from LOD & Nature adds `MarchingSquaresErosion`, selects it
  and undoes (`Add MarchingSquaresErosion`).
- **Storage**: Import button opens a resizable `Open a File or Directory` dialog at the project
  root (qa_tmp visible) and Esc cancels; Bake creates `MarchingSquaresTerrain (Baked)` with 9
  MeshInstance3D + StaticBody3D children and undoes/redoes as `Bake terrain static meshes`;
  Export GLB to `res://qa_tmp/manage_test.glb` writes a 5,025,556-byte file and logs
  `Export GLB to res://qa_tmp/manage_test.glb -> 0`; Save Data with Data Directory
  `res://qa_tmp/data` writes 9 chunk files under `qa_tmp/data/square/` and logs
  `saved 9 chunk file(s) ... 0 -> 491276 bytes`. No demo assets touched.
- **Stamp**: Mountain click stamp (digest changed, `terrain stamp`, undo/redo); Crater drag-stamp
  (size/rotation preview, digest, undo); stamp dropdown items Mountain/Crater/Mesa/Valley/Hill
  listed; Shift+wheel size, Ctrl+wheel height, Alt+wheel rotation all update the hint
  (and height/rotation fields). Size field bug above.
- **Gradient**: Ctrl+LMB start + Ctrl+RMB end markers, LMB drag paints a ramp (digest changed,
  `terrain gradient`), undo returns to the exact pre-stroke digest, redo re-applies; strength
  change and second stroke undo verified. Slider issue above.
- **Select**: drag selects 227 cells (white outline, hint count), `C` copies
  (`copied 227 cells` in Output), `V` enters paste mode with green preview, `Z`/`X` rotate ±90°
  on the square grid, `M` mirrors, LMB pastes an offset copy (`terrain paste`, digest changed),
  undo restores the base digest, redo re-applies, Esc leaves paste mode (`57`–`64`). Rotation
  and Mirror fields reflect the shortcut state.
- **Tint**: tint dropdown (10 colours) and opacity work; LMB paints (red patch visible on the
  island, `terrain tint paint`), RMB erases (patch shrinks, same undo action name), undo removes
  the overlay. Blue paint on underwater cells is not visible above the water plane (expected,
  not a bug). Note: `terrain` digest does not include tint, so undo proof is `undo-state` +
  screenshots (`65`–`69`).
- **Undo/redo**: every action above that reports success was driven with Ctrl+Z and
  Ctrl+Shift+Z from the 3D viewport and produced the expected digest/property; actions that are
  not undoable are called out above (enum settings, grass FPS, chunk-size no-op).

## Not tested / blocked

- **Hex-only fields** (Ring Radius, Wrap Chunks, Edge Style, Instanced) could not be reached
  because Grid Type never changes (first finding). Consequently hex-specific behavior and the
  hex-only dependent controls are untested.
- **Import MST round-trip**: opened and cancelled the picker only. Re-importing a saved Baked
  set into the same terrain was skipped to avoid corrupting the session scene; the saved files
  themselves were verified on disk.
- **Merge Mode / Extra Collision Layer in Settings** were not individually exercised; they use
  the same enum path that failed for Grid Type/Blend/Floor Slot/Debug View/Storage Mode.
- **Chunk vertex handles** (the strip's Vertex Handles checkbox) were not dragged; only the
  Chunks add/remove/select/merge actions were exercised.
- **Gradient Ctrl+wheel strength / Shift+wheel size** not tested (slider issue noted instead).
- **Stamp built-in Mesa/Valley/Hill** were only listed in the dropdown, not stamped; Mountain
  (click) and Crater (drag) were stamped and undone.
