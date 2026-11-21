# r4-manage — Area 3 re-test (Inspect, Chunks, Settings, Storage, Stamp, Gradient, Select, Tint, undo)

Session `r4-manage`, display `:33`, port `8813`, scene `res://demo/hex_island.tscn`
(295 chunks, baseline digest `3064656926`), editor 1600×900. Undo driven with
`key z --ctrl` / `key z --ctrl --shift` (never the direct `undo` command) unless a finding
says otherwise. 62 screenshots: `qa/screenshots/r4-manage/01..60`, `26b`, `49b`.

Headline: Chunks, Stamp, Gradient, Select paste, Tint and Storage Bake all undo/redo
correctly. The **Floor Slot / Storage Mode / Debug View dropdowns still only change their
label** (the 2026-11-20 enum bug persists for these rows), **Grid Type works but silently
deletes every chunk**, and **Grass Animation FPS is not undoable**. Clicking the viewport
while Settings is open deselects the terrain and closes the whole tool.

## Floor Slot and Storage Mode dropdowns change the label only (enum bug persists)

- Severity: error
- Steps:
  1. Select the terrain, **Settings → New Chunks**. Click **Floor Slot** (`1. Grass`) and
     choose `2. Meadow` in the popup (popup confirmed first, 16 items).
  2. The field reads `2. Meadow`, but `prop default_floor_slot` stays `0`, `undo-state` gains
     no action (still `terrain setting noise amp` / `undo_count 1`), and Ctrl+Z changes nothing.
  3. **Settings → Storage**. Click **Storage Mode** (`Resource`) and choose `Baked`
     (popup confirmed: Resource/Baked/Runtime).
  4. The field reads `Baked`, but `prop storage_mode` stays `0`, no undo action is created,
     and the page never exposes **Data Directory / Save Data** (only Import / Bake / Export).
- Expected: the property is written through the undo manager (the code has
  `terrain setting …` actions for these rows) and the dependent Storage rows appear.
- Actual: label-only change. This is the same integer-item-id conversion failure reported on
  2026-11-20; it is **not fixed** for Floor Slot and Storage Mode (Grid Type is fixed, below).
- Evidence: `qa/screenshots/r4-manage/17-newchunks-floorslot-meadow.png` (label Meadow),
  `.../37-storage-mode-baked-select.png` (label Baked, Resource layout);
  `ui/17-floorslot.json`, `ui/37-storage-baked.json`; `prop default_floor_slot` = 0,
  `prop storage_mode` = 0, `undo-state` unchanged.
- Undo: not applicable — no action is created.

## Debug View dropdown does not apply the debug view

- Severity: error
- Steps: 1) Select the terrain, **Settings → Display**. 2) Click **Debug View** (`Off`) and
   choose `Slope` in the confirmed popup (7 items). 3) Observe the viewport and the Inspector.
- Expected: `debug_view` becomes `1`, the viewport switches to slope shading, and
  `undo-state` names `terrain setting debug view`.
- Actual: the field reads `Slope`, but `prop debug_view` stays `0`, the viewport keeps normal
  shading, and no undo action is recorded (unlike the r2c report, the page/label now stays
  on Display and the grid overlay is not touched). The setting is lost on the next rebuild.
- Evidence: `qa/screenshots/r4-manage/30-debugview-popup.png` (popup with Off checked),
  `.../31-debugview-slope-result.png` (label Slope, normal viewport, Output unchanged);
  `ui/31-debugview-result.json`; `prop debug_view` = 0.
- Undo: not applicable — no action is created.

## Grid Type change silently deletes all 295 chunks (no confirmation)

- Severity: warning
- Steps: 1) `demo/hex_island.tscn`, 295 chunks. 2) Settings → Grid. 3) Change **Grid Type**
   from `Hex Rings` to `Square`.
- Expected: a confirmation that chunk data is incompatible (or refuse while chunks exist,
  as Chunk Size does).
- Actual: `grid_type` 3→0, `terrain` immediately returns `0 chunks / digest 0`, the island
  vanishes, and the Output has no explanatory line. Undo restores `grid_type = 3`,
  295 chunks and digest `3064656926` exactly.
- Evidence: `qa/screenshots/r4-manage/58-settings-grid-page.png` (before, 295 chunks),
  `59-gridtype-square.png` (Square, empty viewport), `60-gridtype-undo.png` (restored).
- Undo: works (property, chunk count and digest all restored; one Ctrl+Z).

## Grass Animation FPS is not undoable

- Severity: warning
- Steps: 1) Settings → Grass, Animation FPS `0`. 2) Click the spin up arrow: the field goes
   `0 → 1` (ui dump `value 1.0`). 3) `undo-state` still shows the previous action
  (`terrain setting noise amp`, same `undo_count`/`version`); no `terrain grass animation fps`
  entry is added. 4) Ctrl+Z does not return the value to 0 (later reads still `1`).
- Expected: an undo step that restores the previous FPS (the code has a special branch for
  this setting).
- Actual: the value is applied but not recorded; the change is silently lost from history.
- Evidence: `qa/screenshots/r4-manage/22-grass-fps-changed.png` (FPS 2 while history still
  noise amp), `23-grass-fps-after-undo.png`; `undo-state` before/after unchanged; SpinBox
  `value` 0.0 → 1.0 in the `ui` dump.
- Undo: broken (setting not undoable).

## Clicking the 3D view while Settings is open clears the terrain selection and closes the tool

- Severity: ui (workflow trap)
- Steps: 1) Terrain selected, Settings open (any page). 2) Click anywhere in the 3D view
  (sky **or terrain**) with LMB, e.g. (700,180) or (900,450).
- Expected: a viewport click while a terrain tool is active should keep the terrain selected
  (the tool has no viewport action, so Godot's default click-to-deselect fires) or at least
  not tear down the tool.
- Actual: the selection becomes empty (`info.selection: []`), the Inspector empties, the
  terrain toolbar and the Settings strip disappear. Re-selecting the terrain and reopening
  Settings lands on the last page. Happened twice; it makes "adjust a spinbox then click the
  viewport to undo" lose the tool.
- Evidence: `qa/screenshots/r4-manage/28-settings-current.png` (no toolbar/strip, empty
  Inspector); `info` selection count 0 after the click.
- Undo: not applicable (no edit; state loss is UI-only).

## Settings tab row still jumps between pages

- Severity: ui
- Steps: 1) Settings → Grid: tab row at `y≈287` (first open) and `y≈518` after a rebuild.
  2) New Chunks `y≈448`; Grass `y≈483`; Display `y≈483`; LOD & Nature `y≈518`;
   Storage `y≈518`.
- Expected: a stable tab row (or a strip that scrolls instead of growing upward from the
  bottom).
- Actual: the strip is bottom-anchored and its height changes per page, so the tabs move by
  up to ~230 px; fixed-position tab clicks land in the 3D view. Re-running `ui` before each
  click was required. At one point `click-text "Grid"` hit the toolbar **Grid overlay**
  button instead of the Settings tab (9 `Grid` matches; the overlay toggle is itself an
  undoable `terrain grid overlay` action).
- Evidence: `qa/screenshots/r4-manage/15-settings-open.png` (Grid, tabs y=287),
  `16-settings-new-chunks.png` (y=448), `21-settings-grass.png` (y=483),
  `32-settings-lod-nature.png` (y=518), `35-settings-storage.png` (y=518).
- Undo: not applicable.

## Apply Mode to All Chunks reuses the "chunk merge mode" undo name

- Severity: minor
- Steps: 1) Chunks tool, select a chunk (Ctrl+LMB). 2) Change Merge Mode to `Cubic`
  (`chunk merge mode`). 3) Click **Apply Mode to All Chunks**.
- Expected: a distinct history entry (e.g. `apply chunk merge mode to all`).
- Actual: a second, separate undo step is created (digest `2368849203 → 585444633`) but its
  name is also `chunk merge mode`, so the History dock cannot distinguish a single-chunk
  change from the terrain-wide apply. Undo/redo both work.
- Evidence: `qa/screenshots/r4-manage/13-chunk-apply-all.png`,
  `.../14-chunk-undo-all-restored.png`; `undo-state` action `chunk merge mode`,
  `undo_count 2`.
- Undo: works (two Ctrl+Z return to digest `3064656926`).

## Log scan

`grep -E "ERROR|SCRIPT ERROR|WARNING"` minus the known startup noise leaves exactly one line,
produced by the deliberately incomplete first Gradient attempt:

```
WARNING: Marching Squares Terrain: Gradient needs a start (Ctrl+LMB) and an end point (Ctrl+RMB).
```

No `ERROR:` / `SCRIPT ERROR:`, no crash, no frozen dialog. The Import file dialog closed with
Escape after `click-text "Cancel"` missed it (dialog-local coordinates differ from editor
coordinates — harness quirk, not an addon defect; note `ui` reported Cancel at [271,657]
while it was drawn at ~(598,776) in editor space).

---

## Covered

- **Inspect**: LMB on a cell prints
  `global Vector3 { … } sample (-3, 12) chunk IVec2 { -2, 3 } local IVec2 { 3, 1 }: height 5.000, floor 1 'Grass', wall 6 'Cliff', grass 255, smooth false`
  to Output; a sky click (no cell) prints nothing and raises no error. `02`–`04`.
- **Chunks**: strip fits at 1600×900 (Chunk dropdown, `295 chunks`, Remove Chunk, Merge Mode,
  Apply Mode to All Chunks, Vertex Handles, hint). Green `+` adds (`295→296`, `add chunk`),
  Ctrl+LMB selects, **Remove Chunk** removes (`294`, `remove chunk`), Merge Mode `Cubic`
  changes the digest (`chunk merge mode`), Apply Mode to All Chunks adds a second step.
  Add/remove/merge/apply-all all undo and (for add) redo to exact digests (`05`–`14`).
- **Settings — all six pages** screenshotted at 1600×900 with no clipped labels:
  Grid (`15`, `58`), New Chunks (`16`, `17`), Grass (`21`, `29`), Display (`24`, `31`),
  LOD & Nature (`32`), Storage (`35`, `37`). Page heights/tab rows measured (finding above).
- **Settings values**: Noise Amp (`terrain setting noise amp`), Pixel Size
  (`terrain setting pixel size`, 1→2→1), LOD Distance (`terrain setting lod distance`,
  48→49→48), Grid Type (`terrain setting grid type`, 3→0→3) all undo after focus leaves the
  spinbox. Grass FPS does not (finding).
- **Dependent fields**: hex-only rows (Ring Radius, Wrap Chunks, Edge Style, Instanced) are
  present on the Hex Rings grid; after switching to Square the viewport rebuilds empty
  (0 chunks) so the square-page dependent state was not inspected before undo.
- **Storage**: Storage Mode label-only bug (finding); **Bake** creates
  `MarchingSquaresTerrain (Baked)` with chunk `Body/Shape` children and the action
  `Bake terrain static meshes`; undo removes the baked node (`38`, `39`, `41`). **Import...**
  opens a resizable `Open a File or Directory` dialog at the project root (qa_tmp visible);
  Escape cancels (`40`, `43`). No demo assets touched; no file written.
- **Stamp**: strip (`44`); Mountain click stamp changes the digest
  (`3064656926 → 2592342250`, `terrain stamp`), Ctrl+Z restores exactly, Ctrl+Shift+Z
  re-applies; a drag-stamp from (850,430)→(960,410) changes it again
  (`→ 2506500442`, size/rotation pull) and undoes (`45`–`47`). Preview follows the cursor.
- **Gradient**: Ctrl+LMB start + **Ctrl+RMB** end (the first attempt without the right button
  logs the friendly "needs a start … and an end point" warning and does nothing — good
  feedback), then LMB-drag changes the digest (`→ 3209110031`, `terrain gradient`); Ctrl+Z
  restores `3064656926` (`48`, `49b`, `50`).
- **Select**: drag selects cells; `C` copies, `V` enters paste mode with preview, `Z` rotates,
  LMB paste changes the digest (`→ 2128339482`, `terrain paste`), Ctrl+Z restores the base
  digest, RMB/Esc leave (`51`–`53`).
- **Tint**: Tint toolbar button, LMB-drag paints (`terrain tint paint`), Ctrl+Z produces
  `has_undo:false / has_redo:true`; overlay visible before, gone after (`54`–`56`).
  (The `terrain` digest does not cover tint, so undo proof is `undo-state` + screenshots.)
- **Undo/redo**: every successful edit above was driven with Ctrl+Z and, for add/stamp, with
  Ctrl+Shift+Z. Exact digests: chunks add/remove, merge, stamp click, gradient, select paste.
- **Window/panels**: Chunks strip and all Settings pages fit 1600×900; the terrain toolbar
  wraps to a second column (Tint/Grid at x≈407–523, y≈154–228) and remains clickable.
- **Log**: no new ERROR/SCRIPT ERROR; one intentional Gradient warning.

## Not tested

- **Export GLB / Export Path / Save Data**: Export was not clicked (timebox); Save Data and
  Data Directory are unreachable from the UI because Storage Mode never changes (enum bug),
  so the documented Save Data path remains unverified. Import was opened and cancelled only.
- **All five built-in stamps**: only Mountain (click and drag) was stamped; Crater/Mesa/
  Valley/Hill were not individually placed. Stamp **Mode** dropdown, Size/Aspect/Rotation
  fields and Shift/Ctrl/Alt+wheel modifiers were not exercised.
- **Merge Mode variants** beyond Cubic; **Vertex Handles** not dragged.
- **Blend Mode / Edge Style / Extra Collision Layer / Wall Slot** dropdowns not re-tested;
  they share the enum path that failed for Floor Slot and Storage Mode but were not driven.
- **Grid Size dependent disable** (Brush strip when Grid Aligned off) was not re-tested.
- **LOD In Editor**, Erosion/Wild Flowers **Add** buttons were not exercised.
- **Gradient Strength slider** and Ctrl+wheel strength not tested.
- **Select mirror `M`** and rotation offset correctness were not verified beyond `Z`
  (digest proved the paste, not the angle); paste Relative Height not toggled.
- **Tint colour choices / eraser (RMB) / opacity** not tested (one default paint + undo only).
- **Undo after Ctrl+S / after closing and reloading the scene** not tested.
- **Two strokes → two Ctrl+Z** was shown only for chunk merge + apply-all; not repeated for
  stamp/gradient/select.
- **1280×800 / 1024×600** layouts not tested (1600×900 only).

## Action × undo matrix

| Action | Undo step name | Digest/state restored | Redo |
|---|---|---|---|
| Chunk add (green +) | `add chunk` | 296→295, exact digest | yes |
| Chunk remove (button) | `remove chunk` | 294→295, exact digest | not driven |
| Chunk merge mode | `chunk merge mode` | exact digest | yes |
| Apply Mode to All | `chunk merge mode` | exact digest | (undone with second Ctrl+Z) |
| Grid Type → Square | `terrain setting grid type` | 0→295 chunks, exact digest | since undone |
| Noise Amp | `terrain setting noise amp` | spin value restored | not driven |
| Pixel Size 1→2 | `terrain setting pixel size` | 2→1 (only after focus leaves field) | not driven |
| LOD Distance 48→49 | `terrain setting lod distance` | 49→48 | not driven |
| Floor Slot → Meadow | **none** | no change (`default_floor_slot` 0) | — |
| Storage Mode → Baked | **none** | no change (`storage_mode` 0) | — |
| Debug View → Slope | **none** | no change (`debug_view` 0) | — |
| Grass Animation FPS | **none** | value stays | — |
| Bake meshes | `Bake terrain static meshes` | baked node removed | — |
| Stamp click | `terrain stamp` | exact digest | yes |
| Stamp drag | `terrain stamp` | exact digest | — |
| Gradient stroke | `terrain gradient` | exact digest | — |
| Select paste | `terrain paste` | exact digest | — |
| Tint paint | `terrain tint paint` | overlay gone (digest excludes tint) | — |
| Grid overlay (toolbar) | `terrain grid overlay` | toggled back | — |
