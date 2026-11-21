# r2c-manage — Area 3 re-test (Inspect, Chunks, Settings, Storage, Stamp, Gradient, Select, Tint, undo)

Status: DONE (timebox reached; session stopped). Session `r2c-manage`, display `:36`, port `8836`,
scene `res://demo/hex_island.tscn` (295 chunks, baseline digest `3064656926`), editor 1600×900.
Undo was driven with `key z --ctrl` / `key z --ctrl --shift`, never the direct `undo` command.

Headline: the 2026-11-20 report's top finding ("every Settings enum / slot dropdown only changes
its label") is **refuted for Grid Type** — it now writes the property and undoes. But **Debug View
still fails**, and in a new way: choosing `Slope` created an unrelated `terrain grid overlay`
action and left `debug_view = 0`. Grid Type also wipes all chunks with no confirmation.

## Settings enum dropdowns: Grid Type is fixed (previous error finding REFUTED)

- Severity: none (regression check passed)
- Steps: 1) Select the terrain, **Settings → Grid** (page open on `Hex Rings`). 2) Click **Grid Type**,
   choose `Square`.
- Expected: the property is written through the undo manager and the terrain rebuilds.
- Actual: `prop grid_type` `3` → `0`; Inspector shows `Square` / `Chunk Size 32`; the viewport
  rebuilds to the square grid; `undo-state` = `terrain setting grid type`. One `Ctrl+Z` restores
  `grid_type = 3`, 295 chunks and digest `3064656926`; `Ctrl+Shift+Z` re-applies it.
- Evidence: `qa/screenshots/r2c-manage/07-settings-grid.png` (Hex Rings),
  `09-gridtype-square.png` (Square, 0 chunks), `10-gridtype-undo.png` (restored);
  `ui/07-grid-type.json`; `prop grid_type` before/after.
- Undo: works (property + terrain digest restore exactly).
- Caveat: this pass did **not** re-test Blend Mode / Merge Mode / Floor Slot / Storage Mode, so the
  previous "all enums" claim is only partly refuted. Debug View is still broken (next finding).

## Debug View dropdown does not set `debug_view`; it toggles the editor grid instead

- Severity: error
- Steps:
  1. Select the terrain, **Settings → Display** (page shows Pixel Size / Editor Grid / Chunk Lines /
     Editor Fog / Debug View = `Off`).
  2. Click **Debug View**, choose `Slope` (popup confirmed open first:
     `qa/screenshots/r2c-manage/22-debugview-popup-open.png`).
  3. Click the `Slope` item.
- Expected: `debug_view` becomes `1`, the viewport switches to the slope debug shading, and
  `undo-state` names a `terrain setting debug view` action.
- Actual: `prop debug_view` stays `0`; the dropdown label snaps back to `Off`; no debug shading.
  Instead the undo history gains `terrain grid overlay`, the yellow editor grid turns on in the 3D
  view, and the Settings strip reverts to the **Grid** page. `undo-state` action =
  `terrain grid overlay`.
- Evidence: `qa/screenshots/r2c-manage/22-debugview-popup-open.png` (popup with Off checked),
  `23-debugview-slope-result.png` (Grid page, grid overlay on, Output `terrain grid overlay`),
  `19-debug-view-slope.png`; `prop debug_view` = 0 after the click.
- Undo: broken for the intended setting — the produced `terrain grid overlay` action is unrelated
  (Ctrl+Z at that point toggles the editor grid, not a debug view).

## Grid Type change silently deletes all chunks (no confirmation)

- Severity: warning (recoverable with Ctrl+Z, but destructive and silent)
- Steps: 1) Open `demo/hex_island.tscn` (295 chunks). 2) Settings → Grid → change `Hex Rings` to
  `Square`.
- Expected: a confirmation that existing chunk data is incompatible and will be cleared, or the
  change refused while chunks exist (Chunk Size already refuses with a warning).
- Actual: the strip immediately shows `0 chunks`, the island disappears from the viewport, and the
  Output log has no explanatory line. Undo restores all 295 chunks.
- Evidence: `qa/screenshots/r2c-manage/09-gridtype-square.png` (empty viewport + `0 chunks`),
  `10-gridtype-undo.png` (restored).
- Undo: works (Ctrl+Z restores digest `3064656926` / 295 chunks).

## Chunk count / tool strip stale after undoing a Grid Type change

- Severity: ui
- Steps: 1) Change Grid Type (strip shows `0 chunks`). 2) `Ctrl+Z` — the viewport terrain returns
  (digest `3064656926`, 295 chunks) but the strip still shows `0 chunks` and an empty Chunk field.
  3) Reselect the node / rebuild the tool and it shows 295 again.
- Expected: the strip reflects the restored 295 chunks right after the undo.
- Actual: the strip is stale until the tool is rebuilt. In the same frame the Settings toolbar
  button stayed highlighted while the strip had already switched to the Chunk Manager, i.e. the
  toolbar and strip disagree.
- Evidence: `qa/screenshots/r2c-manage/10-gridtype-undo.png` (terrain visible, strip "0 chunks").
- Undo: n/a (stale UI after a successful undo).

## Settings tab row still jumps vertically between pages

- Severity: ui
- Steps: 1) Settings → Grid (page tabs at `y≈273`). 2) Switch to Grass/Display/LOD (tabs at
  `y≈448–483`). 3) Click where the tab row was on the previous page.
- Expected: a stable tab row (or a strip that scrolls instead of growing upward).
- Actual: the strip is bottom-anchored and grows with each page, so the tab row moves by 175–210 px
  between Grid and Display/Grass (273 → 483 measured this pass; Grass also moved 448 → 483 between
  the first visit and the second). Fixed-position tab clicks miss and land in the 3D view; one such
  miss cleared the terrain selection and made the whole toolbar/strip disappear.
- Evidence: `qa/screenshots/r2c-manage/07-settings-grid.png` (Grid, tabs y=273),
  `18-settings-grass-reopen.png` / `21-settings-display-before.png` (tabs y=483),
  `13-settings-grass.png` (y=448).
- Undo: not applicable.

## Storage page is hard to reach (settings page state/position)

- Severity: ui
- Steps: 1) Settings (opens on the last page). 2) Try to click the **Storage** tab at its last
  position (y≈519 / y≈499 when the page is shorter).
- Expected: the Storage tab opens the Storage page.
- Actual: because the tab row moves with the page content (above finding), the click landed in the
  3D view and cleared the terrain selection; the Settings strip and toolbar vanished. Reopening
  Settings landed on Display/Grid, never Storage. The Storage page and its Save Data / Data
  Directory / Import / Bake / Export controls were not reached in this pass.
- Evidence: `qa/screenshots/r2c-manage/16-settings-storage.png` is **NOT** the Storage page (the
  click had switched back to Brush); do not use it as storage evidence.
- Undo: not applicable.

## Stamp: click-stamp and Ctrl+Z / Ctrl+Shift+Z verified (no new defect)

- Severity: none (pass)
- Steps: 1) Stamp tool, built-in `Mountain`, Size 24, Height 8 (Limits off). 2) Click the island.
- Expected: digest changes, one `terrain stamp` undo step, Ctrl+Z restores the exact digest.
- Actual: digest `3064656926` → `3609518331`; `undo-state` = `terrain stamp`; `Ctrl+Z` from the 3D
  viewport restores `3064656926` (has_redo true); `Ctrl+Shift+Z` restores `3609518331`.
- Evidence: `qa/screenshots/r2c-manage/24-stamp-strip.png` (strip),
  `27-stamp-applied.png`, `28-stamp-undo.png`, `29-stamp-redo.png`.
- Undo: works, including after the viewport click that created the stamp and after switching tools
  (see next item).

## Undo after switching tools / after viewport focus (verified)

- Severity: none (pass)
- Steps: 1) Apply a stamp (viewport click). 2) Click the Brush toolbar button. 3) `Ctrl+Z` without
  touching the viewport. 4) `Ctrl+Shift+Z`.
- Expected: the stamp action is undone/redone even though focus/tool changed.
- Actual: digest `3609518331` → `3064656926` → `3609518331`, exact both ways.
- Evidence: `qa/screenshots/r2c-manage/30-undo-after-toolswitch.png`.
- Undo: works.

## Inspect: cell output and empty-space click (pass)

- Severity: none
- Steps: 1) Inspect tool (`7`). 2) LMB a terrain cell. 3) Click sky/water with no cell.
- Expected: cell data in Output; no error for a miss.
- Actual: Output prints e.g.
  `Marching Squares Terrain: global Vector3 { x: 19.437674, y: 2.4989886, z: 21.487686 } sample (4, 12) chunk IVec2 { x: -1, y: 4 } local IVec2 { x: 3, y: 0 }: height 2.500, floor 3 'Forest Floor', wall 7 'Dark Rock', grass 255, smooth false`.
  A sky click prints `Marching Squares Terrain: no sample at (182, -267)` and raises nothing.
- Evidence: `qa/screenshots/r2c-manage/03-inspect-cell.png`,
  `04-inspect-cell-output.png`, `05-inspect-sky-click.png`, `06-inspect-strip.png`.
- Undo: n/a.

## Log scan

`grep -E "ERROR|SCRIPT ERROR|WARNING"` over the session log, minus the known startup noise, leaves
exactly one line:

```
WARNING: Marching Squares Terrain: new chunks must touch an existing chunk.
```

No `ERROR:` / `SCRIPT ERROR:` from `mst` / `MarchingSquares`, no crash, no frozen dialog. The
warning was not accompanied by a green `+` click in this pass (it came from a stray early click
while the Chunks tool was active); the invalid-handle feedback finding from 2026-11-20 was not
re-exercised.

## Covered

- **Inspect** — cell print to Output, sky/no-cell click, no error spam.
- **Settings pages** — Grid, New Chunks, Grass, Display, LOD & Nature screenshotted at 1600×900;
  **Storage not reached**.
- **Settings values** — Grid Type (works, undo/redo); Debug View (broken, see above).
- **Stamp** — strip, Mountain click stamp, digest, `terrain stamp` undo + redo from the viewport
  and after a tool switch.
- **Undo/redo** — Grid Type change, stamp; Ctrl+Z and Ctrl+Shift+Z exact digests; undo after
  clicking the 3D viewport and after switching tools.
- **UI layout** — Settings tab-row movement measured on four pages; no clipped labels at 1600×900.

## Not tested (and why)

- **Chunks add/remove/select/merge/apply-all and Vertex Handles** — not exercised. The session's
  tool strip kept rebuilding after the Grid Type / Debug View sequences and viewport clicks were
  clearing the selection; I ran out of the timebox before a clean Chunks run. (`06-chunks-strip.png`
  shows the strip layout only.)
- **Blend Mode, Merge Mode, Floor Slot, Edge Style, Extra Collision Layer, Storage Mode** — not
  re-tested; only Grid Type and Debug View were driven. The Storage Mode → Save Data path therefore
  was not reached.
- **Storage** — Save Data / Data Directory / Import / Bake / Export GLB were not tested at all.
- **Gradient** — not tested (points, strength, undo).
- **Select** — not tested (copy/paste/rotate/mirror/undo).
- **Tint** — not tested.
- **Stamp drag-to-size/rotation and Shift/Ctrl/Alt+wheel** — not tested this pass; the previous
  report's stale Size-field finding was not re-confirmed.
- **Two changes + two undos** — attempted with two extra stamps, but both produced no digest change
  (clicks landed on already-stamped/empty cells), so the "two strokes, two Ctrl+Z" case is not
  demonstrated.
- **1280×800 window** — not tested (only 1600×900 captures).
- **Grass FPS undo, Chunk Size no-op, Gradient slider, Grid Size dependent-enable** — previous
  findings not re-confirmed.

## Harness notes

- `drive.py popups` returned `[]` while the Debug View popup was visibly open (screenshot 22), and
  `prop` needs `--class`/`--path` (without it: `node not found`). These are harness quirks, not
  addon defects.
- `terrain` transiently returned `digest 0 / chunks 0` while a Grid Type rebuild was in flight;
  a re-query after the rebuild returned the correct `3064656926 / 295`. Treat a 0/0 digest taken
  immediately after a grid-type change as a race, not data loss.
