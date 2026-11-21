# Area 2 — Painting + Foliage + textures, pass r2b (2026-11-21)

Status: partial pass, session stopped. Paint/presets/export/undo core verified; foliage only
partly done (Tree paint only). The editor died mid-pass; a second OS process (pid 525417, parent
240476) was concurrently running `drive.py` against this session's env and did `resize 1024 600` +
`click-text Level` — a documented layout-crash trigger. The last 2–3 commands are therefore
unreliable and the remaining foliage/grass/resize tests were not completed. No addon ERROR/
SCRIPT ERROR appears in the log.

Session: `r2b-paint` (display `:47`, port `8847`, project copy `/tmp/opencode/qa/sessions/r2b-paint/project`).
Scene: `demo/hex_island.tscn` (+ `tutorial/02_island.tscn` for Fill/Clear if time). All coordinates 1600x900 unless noted.

## Verified — paint strokes, material pick, two-step undo
- Paint tool opens right Texture panel + strip; slot 5 (Dirt) selected via panel click, Material
  dropdown follows (`5. Dirt`), stroke changes digest 3064656926 -> 1621466960 and records
  `terrain vertex paint`. Ctrl+Z -> 3064656926 (has_redo), Ctrl+Shift+Z -> 1621466960.
- Ctrl+LMB pick: at (860,290) picked `1. Grass`; at (700,330) picked `4. Sand`.
- Two strokes (Sand then Snow slot 8) = two undo steps: D0 1621466960 -> D1 1296675103 ->
  D2 1535857993; Ctrl+Z twice returns D1/D0, Ctrl+Shift+Z twice returns D2.
- Paint Walls: checkbox toggles on; stroke at (790,300)-(830,320) on hex_island created
  `terrain wall paint`, digest 4147534113 -> 2142026054; Ctrl+Z/Ctrl+Shift+Z restore/redo. Earlier
  attempts at (880,350)/(895,320)/(700,265)/(950,300) produced nothing (mouse over sky/water after
  zoom) — not a defect.

## Shift+wheel changes the brush Size but the Size label does not refresh

- Severity: minor (label/UI refresh; the previous pass reported the wheel as doing nothing — refuted)
- Steps: 1) Paint tool, initial `Size` 15.0. 2) Shift+wheel up x3 then down x5 over the viewport;
  read `Size` -> still 15.0. 3) More Shift+wheel down x4 -> label still 14.5. 4) Switch to the
  Brush tool and back to Paint; read `Size` -> 10.3.
- Expected: the `Size` value updates immediately on Shift+wheel (the strip hint says
  `Shift+wheel: size`, docs/04).
- Actual: the underlying size does change (15.0 -> ... -> 10.3 measured only after a tool switch
  rebuilds the strip), but the number label is not refreshed by the wheel; it also did not refresh
  on a slot click, only on a tool switch / texture-preset apply. Users see a frozen size.
- Evidence: qa/screenshots/r2b-paint/29-size-stale-label.png (10.3 shown only after the switch),
  qa/screenshots/r2b-paint/02-slot5-selected.png (same hint, 15.0)
- Undo: not applicable (tool setting)

## Slope Min spin slider is unbounded and jumps thousands of degrees per drag

- Severity: warning
- Steps: 1) Paint tool, Slope Min starts 0.0. 2) Drag the spin slider handle ~25 px right.
  3) Read the value; type `0` + Enter to recover.
- Expected: clamp to the documented/declared 0–90 range (step 0.5).
- Actual: 25 px -> `5012.5`; a second 40 px drag left -> `32855`; a later value reached `1705.0`.
  With such a value any paint stroke is silently blocked (no digest change, no undo step); type
  `0` into the number and the same stroke paints (`terrain vertex paint`, digest
  1535857993 -> 4147534113). Confirms the 2026-11-20 finding (8287.5) with different values.
- Evidence: qa/screenshots/r2b-paint/12-slope-min-drag.png (5012.5),
  qa/screenshots/r2b-paint/13-slope-block-vs-allow.png (1705.0, blocked),
  qa/screenshots/r2b-paint/14-slope-reset.png (32855), 15-slope-typed0.png (0.0),
  16-slope0-paint.png (paint works)
- Undo: not applicable (tool setting)

## Save Preset dialog has no Cancel button (re-confirmed)

- Severity: ui
- Steps: 1) Paint panel scrolled to the bottom. 2) Click `Export Texture Preset`.
- Expected: OK + Cancel (docs/04 describe a name field with OK/Cancel).
- Actual: only `OK`. Esc cancels (no file written); Cancel exists only on the separate
  overwrite confirmation.
- Evidence: qa/screenshots/r2b-paint/21-save-preset-dialog.png,
  qa/screenshots/r2b-paint/24-save-preset-named.png
- Undo: not applicable

## Empty preset name closes the dialog with no feedback (re-confirmed)

- Severity: minor
- Steps: 1) `Export Texture Preset`. 2) Clear the name field. 3) Enter.
- Expected: inline validation or the dialog stays open.
- Actual: dialog closes, no `.tres` created, no message. Only `mst_classic.tres` + the previously
  saved `r2bcheck.tres` remain.
- Evidence: qa/screenshots/r2b-paint/22-save-preset-empty-field.png,
  qa/screenshots/r2b-paint/23-save-preset-empty-after.png
- Undo: not applicable

## Texture-panel labels are clipped at the 1600x900 panel width

- Severity: ui
- Steps: 1) Paint tool. 2) Scroll the right panel to Auto Texture Rules.
- Expected: full labels.
- Actual: the fixed ~224 px content column truncates `Auto Text...`, `Filler Amo...`,
  `Water Lev...`, `Shore Hei...`, `Snow Hei...`, `Snow Noi...`, `Snow Slo...` (label column is
  96 px with ellipsis). Panel has a vertical scrollbar and is not resizable.
- Evidence: qa/screenshots/r2b-paint/19-panel-export-bottom.png,
  qa/screenshots/r2b-paint/25-overwrite-confirm.png (right panel behind dialog)
- Undo: not applicable

## Verified — wall paint, preset export/save/overwrite
- Paint Walls now works on hex_island: ticked checkbox + stroke at (790,300)-(830,320) created
  `terrain wall paint`, digest 4147534113 -> 2142026054; Ctrl+Z/Shift+Z restore/redo. Earlier
  attempts at (880,350)/(895,320)/(700,265)/(950,300) did nothing (likely sky/water after zoom,
  or cells whose wall slot was already the active one) — not recorded as a defect.
- Export Texture Preset: Esc cancels (no file); empty name silently no-ops; `r2bcheck` saved to
  `/tmp/opencode/qa/sessions/r2b-paint/project/addons/MarchingSquaresTerrain/texture_presets/r2bcheck.tres`
  (6566 B); re-saving same name shows "Overwrite File?" with Cancel/OK; Cancel leaves the file
  unchanged (mtime/size preserved). After a save the Texture Preset dropdown auto-selects the new
  preset (`r2bcheck`).

## Undo of a Texture Preset reverts the resource but leaves the labels stale/mixed

- Severity: warning
- Steps: 1) Paint tool, Texture Preset = `MST Classic` (applies an embedded copy; slot names become
  `1. Texture 1`...). 2) Ctrl+Z. 3) Inspect `tex_assign`, the Material dropdown and the slot panel.
- Expected: the resource and every slot label revert to the scene's embedded set.
- Actual: `prop tex_assign` is back to
  `res://demo/hex_island.tscn::MarchingSquaresTexturePreset_82xx5`, but the Material dropdown lists
  `1. Texture 1` ... `6. Texture 6` (MST Classic names) for slots 1–6 while slots 7–16 show the
  original names (`Dark Rock`, `Snow`, `Gravel`, ...), and the Texture Preset dropdown still shows
  `MST Classic` instead of going back to the scene set. The slot panel still shows `Texture 1`.
  Re-confirms the 2026-11-20 stale-label finding with the additional mixed-name detail.
- Evidence: qa/screenshots/r2b-paint/26-preset-mst-classic.png,
  qa/screenshots/r2b-paint/27-preset-undo-stale.png,
  qa/screenshots/r2b-paint/28-preset-undo-panel.png ; ui dumps
  qa/screenshots/r2b-paint/ui/01-paint-full.json
- Undo: broken (data restored, UI labels/dropdown stale)

## Texture presets: save/apply facts (no separate defect)
- `r2bcheck` was saved while the terrain still had the scene set; after saving, the Texture Preset
  dropdown selects `r2bcheck` immediately. Applying `MST Classic` replaced `tex_assign` with an
  embedded copy (`resource_path` empty) and changed the viewport look; undo restored the scene
  resource. `None` restore was not tested.
- Export: Esc cancels with no file; empty name silently no-ops; overwrite prompt has Cancel/OK and
  Cancel preserves the file (6566 B, mtime unchanged).

## Covered
- [x] Paint tool panel: 16 slot buttons present (`Texture 1`..`Texture 16 (void)`), slot 5 and
      slot 16 selected from the panel; Material dropdown follows (`5. Dirt`, `16. Void`).
- [x] All 16 slots reachable by scrolling the right panel (click-text auto-scroll); panel bottom
      (Auto Texture Rules + Export Texture Preset) reachable by wheel; slot 16 details are below
      the fold at 1600x900.
- [x] Paint strokes with slots 5 (Dirt), 4 (Sand), 8 (Snow), 7 (Dark Rock), 16 (Void selected);
      visible material change and digest change for the strokes; two strokes = two undo steps.
- [x] Ctrl+LMB picks material (`1. Grass`, `4. Sand`).
- [x] Paint Walls on a cliff/terrace: `terrain wall paint`, undo/redo works.
- [x] Slope Min limiting: 1705 (slider-produced) blocks all painting; typed 0 allows painting.
- [x] Texture Preset popup lists None / MST Classic / r2bcheck; applying MST Classic updates slots
      and painting appearance; undo restores the resource (labels stale — finding above).
- [x] Export Texture Preset dialog, Esc cancel, empty-name no-op, save to the session project,
      overwrite confirmation + Cancel.
- [x] Foliage panel: palette (Grass, Tree, Pine, Bush, Rock, Flowers, Pebbles), Size, Density,
      Slope Min/Max, Erase All, Fill Terrain, Clear, Edit, Remove, Add Mesh / Scene...
- [x] Tree foliage paint: 173 -> 203, one `terrain foliage paint props` step; Ctrl+Z -> 173,
      Ctrl+Shift+Z -> 203.
- [x] Two strokes after switching slots are separate undo steps; Ctrl+Z/Ctrl+Shift+Z pairs
      restore and redo both.
- [x] No addon ERROR/SCRIPT ERROR in the editor log up to the session death.

## Not tested (and why)
- Bush/Rock/Flowers/Pebbles/Grass painting, Right-drag or Shift+LMB erase, Erase All, Fill
  Terrain, Clear: the editor died before these; the last state was corrupted by a concurrent
  `drive.py` process resizing to 1024x600 and clicking Level (pid 525417). A Pine stroke at
  (800,280) placed nothing, but that observation is contaminated by the concurrent driver and is
  not reported as a finding.
- Texture Name LineEdit rename + Ctrl+Z with the field focused; Ctrl+Z after clicking the 3D view;
  Ctrl+Z with another node selected: not reached.
- `None` preset restore, slot 16 paint (selected only), texture Pattern/Scale/Color pickers,
  Auto Texture Rules toggling: not reached.
- Fill Terrain / Clear on 02_island, grass height/slope limits and `grass_enabled` toggle: not
  reached.
- 1280x800 resize / panel clipping matrix: not reached (the only resize observed was the
  concurrent process's 1024x600, which likely caused the documented layout crash).
- Log scan after the crash: the log ends at the preset save; no traceback, so the death is not
  attributable to the addon from this evidence.

## Log scan
- `/tmp/opencode/qa/sessions/r2b-paint/editor.log`: only known startup noise (V-Sync, missing
  `.uid` re-created, debug-adapter/LSP ports) plus the info line
  `Marching Squares Terrain: saved texture preset res://addons/MarchingSquaresTerrain/texture_presets/r2bcheck.tres`.
  No `ERROR:` / `SCRIPT ERROR:` from `mst`/`MarchingSquares` before the editor process exited.
  The final log line is that preset save; there is no crash traceback.

## Evidence index
- 34 PNGs in `qa/screenshots/r2b-paint/`: 01–05 paint/undo, 06/17 wall paint, 07–11 zoom/wall
  attempts, 12–16 Slope Min, 18–25 panel bottom/export/overwrite, 26–28 preset apply/undo,
  29 Size label, 30–32 foliage panel/tree, 33–34 Pine/state.
- 6 control dumps in `qa/screenshots/r2b-paint/ui/`.
