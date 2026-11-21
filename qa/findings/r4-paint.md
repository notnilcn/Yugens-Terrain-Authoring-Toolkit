# Area 2 — Painting + Foliage + textures (+ undo), round 4

Session `r4-paint`: display `:32`, port `8812`, project copy
`/tmp/opencode/qa/sessions/r4-paint/project`, scene `res://demo/hex_island.tscn`,
window 1600x900. 59 screenshots in `qa/screenshots/r4-paint/`; UI dumps in
`qa/screenshots/r4-paint/ui/`. All coordinates are 1600x900 window-content coordinates.

Log scan (`/tmp/opencode/qa/sessions/r4-paint/editor.log`, 15 lines total): no `ERROR:`,
`SCRIPT ERROR:` or new warning lines from the addon. Only the known startup noise
(`Could not set V-Sync mode`, two `Missing .uid file ... re-created from cache`) and two info
lines:
`Marching Squares Terrain: saved texture preset res://addons/MarchingSquaresTerrain/texture_presets/qa4paint.tres`
(line 14) and `...qa4verify.tres` (line 15). The session copy's `texture_presets/` contains
only `mst_classic.tres` plus those two exported files.

Digest caveat: the `terrain` digest covers cell data (heights, vertex material, wall material,
grass erase) but **not** the texture-assignment resource and **not** scatter counts. Texture
preset/rename checks therefore use `prop tex_assign` (resource path) and UI screenshots;
scatter checks use `call get_scatter_count N`.

## Slope Min slider accepts values far outside 0–90 and then silently blocks painting

- Severity: warning
- Steps: 1) Select terrain, Paint tool. 2) Drag the `Slope Min` EditorSpinSlider 25 px right
  (645,605 -> 670,605). 3) Read the value; paint a stroke on flat island ground.
- Expected: the documented 0–90 degree range (the property is declared `Spin(0.0, 90.0, 0.5)`);
  if a value is out of range it should be clamped/rejected.
- Actual: the field becomes **6702.5** after one short drag, with no unit and no clamp. A paint
  stroke then changes nothing: digest stays `354063828`, no new undo entry. Typing `0` restores
  the value and painting works again (digest `3880537874`, `terrain vertex paint`).
- Evidence: qa/screenshots/r4-paint/09-slope-min-drag-overflow.png (value 6702.5),
  qa/screenshots/r4-paint/10-slope-min-restored.png (typed 0.0)
- Undo: not applicable (tool setting; the blocked stroke created no step)

## Texture-preset undo restores the resource but leaves every texture label stale

- Severity: warning
- Steps: 1) Paint tool, `Texture Preset = qa4verify` (scene set, names Grass/Meadow...).
  2) Apply `MST Classic` (names become Texture 1...Texture 16, dropdown shows MST Classic).
  3) Press `Ctrl+Z` and inspect the strip and right panel.
- Expected: the dropdown, Material list and slot names all revert with the resource.
- Actual: `prop tex_assign --path HexIsland/MarchingSquaresTerrain` correctly returns the
  embedded scene preset (`res://demo/hex_island.tscn::MarchingSquaresTexturePreset_82bka`), but
  the UI still shows `Texture Preset: MST Classic`, `Material: 2. Texture 2` and panel slot 1
  `Texture 1`/`Texture 1` with the black `mst_noise` preview. A later preset save/refresh fixes
  the labels, proving they are stale, not the data.
- Evidence: qa/screenshots/r4-paint/53-preset-applied-before-undo.png,
  qa/screenshots/r4-paint/54-preset-undo-stale-names.png
- Undo: broken (data restored, labels/dropdown stale)

## Slot-rename undo leaves the Material dropdown and slot label stale

- Severity: minor
- Steps: 1) Paint tool, Material `2. Meadow`. 2) Type `qameadow` into `Texture Name` and press
  Enter (`terrain texture 2 display_name`). 3) Click the 3D view, `Ctrl+Z`.
- Expected: the name returns to Meadow everywhere.
- Actual: the undo runs (redo becomes available), and exporting the set immediately afterwards
  writes `display_name = "Meadow"`, yet the strip still reads `Material: 2. qameadow`; the slot
  panel also keeps the old label until the next preset apply/save rebuilds it.
- Evidence: qa/screenshots/r4-paint/20-slot-renamed.png,
  qa/screenshots/r4-paint/21-slot-rename-undone.png,
  qa/screenshots/r4-paint/22-stale-panel-after-rename-undo.png; exported
  `qa4verify.tres` line 28 `display_name = "Meadow"`.
- Undo: broken (data restored, UI label stale)

## Paint panel is ~12 screenfuls tall; only ~1.5 texture slots are visible at once

- Severity: ui
- Steps: 1) Paint tool at 1600x900. 2) Inspect the right `Textures` panel and scroll from slot 1
  to `Export Texture Preset`.
- Expected: the 16 slots are usable without excessive scrolling (docs/04 treats this as a
  slot palette).
- Actual: the scroll viewport is 250x469 px while its content margin is **5767 px** tall
  (~12.3 viewports). One slot block is ~338 px, so slot 1 fills y=193..531 and slot 2 starts
  at 543; only slot 1 and the header of slot 2 are visible. `Export Texture Preset` sits at
  content y=5881 and the Auto Texture Rules block before it. A vertical scrollbar exists
  (`VScrollBar` at 1299,154 14x469), but there is no compact view, fold-all, or search; slot 16
  (Void) is ~6,000 px down.
- Evidence: qa/screenshots/r4-paint/02-paint-tool-panel.png,
  qa/screenshots/r4-paint/57-paint-panel-bottom-slot16-export.png,
  qa/screenshots/r4-paint/58-panel-slot16-void.png,
  qa/screenshots/r4-paint/ui/02-paint-panel.json (content 1067,154 236x5767)
- Undo: not applicable

## Paint/Foliage layout squeezes the 3D view to 534x256; the panel can only grow

- Severity: ui
- Steps: 1) 1600x900 editor, paint mode. 2) Read the `SubViewportContainer` rect. 3) Drag the
  divider at x≈1058 left/right.
- Expected: a workable viewport and a panel the user can shrink/hide.
- Actual: the fixed 250 px texture panel plus the 210 px bottom strip leave the viewport at
  **526,154 534x256** (~28% of the height). A splitter exists (dragger 1057.5,154 8x469) but
  the panel cannot shrink below 250 px; dragging left widens the panel to 411 px and narrows
  the view to 372 px, dragging right does nothing. There is no collapse/hide button. Foliage
  mode has the same panel width but a shallower palette.
- Evidence: qa/screenshots/r4-paint/02-paint-tool-panel.png,
  qa/screenshots/r4-paint/51-panel-splitter-drag-right.png,
  qa/screenshots/r4-paint/52-panel-splitter-drag-left.png,
  qa/screenshots/r4-paint/ui/05-paint-mode-layout.json,
  qa/screenshots/r4-paint/ui/03-viewport.json
- Undo: not applicable

## Save Preset dialog has no Cancel button

- Severity: ui
- Steps: 1) Paint panel, scroll to the bottom, click `Export Texture Preset`.
- Expected: docs/04 describe a name field with OK/Cancel; there should be an explicit way to
  abandon the save.
- Actual: the embedded `Save Preset` dialog is 558x150 with the explanatory label, a name field
  and a single centred `OK`. `Esc` and the window X do cancel and write no file (verified:
  folder stayed at only `mst_classic.tres`), but there is no Cancel control.
- Evidence: qa/screenshots/r4-paint/14-export-preset-dialog.png,
  qa/screenshots/r4-paint/15-preset-dialog-cancelled.png
- Undo: not applicable

## Empty preset name closes the dialog silently

- Severity: minor
- Steps: 1) `Export Texture Preset`. 2) Select-all + Delete in the name field. 3) Click `OK`.
- Expected: inline validation, or the dialog staying open with an explanation.
- Actual: the dialog closes, no `.tres` is created (folder listing unchanged), and there is no
  message in the UI or the editor log.
- Evidence: qa/screenshots/r4-paint/18-preset-empty-name.png,
  qa/screenshots/r4-paint/19-preset-empty-submitted.png
- Undo: not applicable

## Ground-texture preview is black for `mst_noise.png` slots

- Severity: ui
- Steps: 1) Paint tool. 2) Apply `MST Classic`. 3) Look at the `Texture:` resource preview for
  slot 1 (and slot 2) in the right panel.
- Expected: the bundled 128x128 grey noise texture, as shown by the slot's small tinted
  thumbnail.
- Actual: the large `Texture:` preview is solid black for slots whose ground texture is
  `textures/ground/mst_noise.png`; other textures (grass, sand, snow) preview correctly. The
  picker tooltip still names the correct file. The scene's own preset previews correctly, so it
  is specific to those preset textures.
- Evidence: qa/screenshots/r4-paint/12-preset-mst-classic.png,
  qa/screenshots/r4-paint/54-preset-undo-stale-names.png
- Undo: not applicable

## Paint Walls hint still says "Vertex Painting" and reports a floor height

- Severity: minor
- Steps: 1) Paint tool, tick `Paint Walls`. 2) Stroke a cliff face.
- Expected: the status line acknowledges wall painting.
- Actual: the action is correctly named `terrain wall paint` and the digest changes, but the
  live hint above the viewport still reads `Vertex Painting | height 2.50 cell ...`.
- Evidence: qa/screenshots/r4-paint/07-paint-walls-on.png,
  qa/screenshots/r4-paint/08-paint-walls-stroke.png
- Undo: works (`terrain wall paint`, one step; Ctrl+Z/Ctrl+Shift+Z verified)

## Covered

- [x] Paint tool opens the 16-slot Textures panel; all 16 slot names present (Grass, Meadow,
      Forest Floor, Sand, Dirt, Cliff, Dark Rock, Snow, Gravel, Clay, Moss, Path, Stone Tiles,
      Ash, Ice, Void); slot 16 `Void` reachable by scrolling (`ui/02-paint-panel.json`,
      `58-panel-slot16-void.png`).
- [x] Bottom strip labels: Vertex Painting, Brush Type, Size, Aspect, Rotation, Material,
      Texture Name, Texture Preset, Paint Walls, Slope Min/Max; wrap into 4 rows at 534 px and
      widen at 787 px (`02`, `52`). No clipped labels seen in paint mode.
- [x] Painted with slots 8 Snow, 5 Dirt and 2 Meadow; each stroke visibly changed the ground
      (`05`, `08`, `11`, `29`).
- [x] Clicking the `Texture 2` slot button selected `2. Meadow` and updated the strip.
- [x] Ctrl+LMB material pick: from `4. Sand` to `8. Snow` by Ctrl+clicking painted ground
      (`06`).
- [x] Paint Walls: checkbox on, cliff stroke created `terrain wall paint`, digest
      `3822771854 -> 354063828`, Ctrl+Z restored, Ctrl+Shift+Z redid.
- [x] Slope Min: one drag produced 6702.5 (finding); painting then no-ops (digest unchanged);
      typing 0.0 restores painting (`09`, `10`).
- [x] Texture Preset dropdown lists None, MST Classic, qa4paint, qa4verify; `None` restores the
      scene names/preview (`12`, `13`).
- [x] Export Texture Preset: dialog with pre-filled `new_texture_preset`; Esc cancels with no
      file; typing and OK saves `qa4paint.tres` in the **session** project and the dropdown
      immediately lists it (`14`-`17`); log lines 14–15.
- [x] Empty name: dialog closes, no file, no message (`18`, `19`).
- [x] The prior "OK click falls through to the Material dropdown" report was **not reproduced**:
      after clicking OK, `popups` was empty and Material stayed `2. Meadow`.
- [x] Foliage palette: instructions, Grass/Tree/Pine/Bush/Rock/Flowers (+Pebbles on hex_island),
      Edit/Remove, Add Mesh/Scene, Size/Density/Slope Min/Max, Erase All, Fill Terrain, Clear
      (`26`, `27`, `56`). Palette content is 850 px in a 469 px viewport but all items fit
      roughly two screens.
- [x] Foliage paint each type (counts via `get_scatter_count`): Tree 173->219, Pine 28->59,
      Bush 117->168, Rock 429->526, Flowers 89->115; each stroke is one
      `terrain foliage paint props` step and Ctrl+Z restored the count (`29`, `31`).
- [x] Grasses: `Grass` palette entry paints (`terrain foliage paint props`, no digest/count
      change), RMB erases grass (`terrain foliage erase grass`, digest
      `248441933 -> 207381766`, one step; undo/redo verified) (`32`-`34`).
- [x] Shift+LMB erases props: Rock 428->421 (`terrain foliage erase props`), undo restores 428
      (`55`).
- [x] Fill Terrain: Rock 526->428 with one `terrain foliage fill` step; digest unchanged
      (scatter is not in the digest); Ctrl+Z restores 526, Ctrl+Shift+Z returns 428 (`35`, `36`).
- [x] Clear: Rock 428->0 with one `terrain foliage clear` step; Ctrl+Z restores 428 (`37`).
- [x] Two scatter strokes -> two Ctrl+Z -> counts 546 -> 510 -> 428 (one step per stroke).
- [x] Grass toggle via `Terrain Settings -> Grass` checkbox: creates `terrain setting grass`,
      `grass_enabled` false/true; Ctrl+Z/Ctrl+Shift+Z verified; grass blades visibly disappear
      (`38`, `39`, `42`, `43`).
- [x] Grass height limit: `Min Height = 100` removes all grass; action
      `terrain setting min height`; Ctrl+Z restores 0.6 and the blades
      (`47`, `48`). (Also accidentally set Subdivisions=100 and undid it:
      `45`/`46`.)
- [x] Undo/redo after switching tools: paint stroke, switch Paint->Foliage, Ctrl+Z restores the
      digest to `248441933`; Ctrl+Shift+Z reapplies (`49`).
- [x] Undo after Save: paint action `terrain vertex paint` (digest 273529757), Ctrl+S saved the
      scene (tab loses `*`, `50`), Ctrl+Z still restored digest `248441933`.
- [x] Undo after clicking the 3D view: the slot-rename undo worked from viewport focus (`21`).
- [x] Ctrl+Z with `Texture Name` LineEdit focused edits the text (typing `xy`, Ctrl+Z leaves the
      field empty) and does not change the terrain digest or undo stack (`24`, `25`).
- [x] Texture panel splitter: drag right does nothing (already at 250 min), drag left widens the
      panel to 411 px and narrows the viewport (`51`, `52`).

## Action x undo matrix

| Action | Named undo step | Ctrl+Z | Ctrl+Shift+Z | Notes |
|---|---|---|---|---|
| Paint stroke (floor) | `terrain vertex paint` | works, digest restored | works | one step per stroke |
| Paint stroke (wall) | `terrain wall paint` | works | works | distinct action |
| Scatter stroke | `terrain foliage paint props` | works (count restored) | works | digest does not cover scatter |
| Shift+LMB scatter erase | `terrain foliage erase props` | works | works | 428->421->428 |
| Grass RMB erase | `terrain foliage erase grass` | works (digest) | works | 248441933<->207381766 |
| Grass LMB paint | `terrain foliage paint props` | step exists | not exercised | no digest/count change on grass |
| Fill Terrain | `terrain foliage fill` | works (428->526) | works (526->428) | one step |
| Clear | `terrain foliage clear` | works (0->428) | not exercised | one step |
| Texture preset change | `terrain texture preset` | data restored, **UI stale** | data reapplied | digest unchanged; check `tex_assign` |
| Slot rename | `terrain texture 2 display_name` | data restored, **UI stale** | not exercised | digest unchanged |
| Grass enabled toggle | `terrain setting grass` | works (property) | works | via Settings page |
| Grass Min Height | `terrain setting min height` | works (property) | not exercised | grass disappears/reappears |
| Save scene then undo | `terrain vertex paint` | works | n/a | undo history survives save |

## Not tested

- `Erase All` checkbox behavior (right-drag removing every prop).
- Multi-item Ctrl/Shift selection in the foliage palette.
- Foliage `Slope Min/Max` limiting (only the Paint tool's slope limit was proven; the same
  unbounded spin slider is used).
- Grass slope limit specifically (the Settings page exposes only Ledge Cutoff/Min Height; no
  slope control was found).
- Slot-level Pattern/Scale/Color/Has-grass/Sprite pickers (inspected but not changed/painted).
- Add Mesh / Scene, Edit and Remove in the foliage palette.
- Preset overwrite confirmation, and the reported OK click-through (could not reproduce).
- Painting on a visibly vertical wall with a second wall material (the wall stroke was on a
  steep slope, proven by the `terrain wall paint` action and digest).
- 1280x800/1024x600 layout of the paint/foliage panels (Area 7).
- Fill Terrain for Tree/Pine/Bush/Flowers (Rock only) and redo of Clear.
- Tests of the `Texture 16 (void)` material actually cutting a hole (slot reached, not painted).
