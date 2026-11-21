# QA round 2 — Area 2: Painting, textures, foliage (MarchingSquaresTerrain)

**Status:** session stopped (`qa/session.sh stop r2-paint` confirmed). Partial pass: paint, slot
inventory, preset export and paint undo covered; foliage and resize were not reached because the
session/Xvfb was stopped externally at 04:48 by a **stale duplicate driver** from the earlier killed
attempt (see "Session incidents"). Findings below distinguish what this pass confirmed live from
what only the earlier attempt's leftovers show.

Session: `r2-paint`, display `:32`, port `8812`, scene `res://demo/hex_island.tscn`, project copy
`/tmp/opencode/qa/sessions/r2-paint/project`. Coordinates are 1600×900 window-content coords.
Editor log: `/tmp/opencode/qa/sessions/r2-paint/editor.log`.

## Save Preset dialog has no Cancel button

- Severity: ui
- Steps: 1) Paint tool. 2) `Export Texture Preset` (button at 1179,606 after scrolling to the
  panel bottom). 3) Inspect the dialog.
- Expected: docs/04 describe a name field with OK/Cancel; an explicit way to cancel without
  saving.
- Actual: the embedded `Save Preset` window has a name field and a single `OK` button. The only
  cancel paths are `Esc` or the window close X (both work; no file written).
- Evidence: qa/screenshots/r2-paint/18-preset-export-dialog.png,
  qa/screenshots/r2-paint/22-preset-typed-name.png
- Undo: not applicable

## Empty preset name closes the dialog silently

- Severity: minor
- Steps: 1) `Export Texture Preset`. 2) Select-all + Delete in the name field. 3) `Enter`
  (embedded window has no default-button handling; the dialog stays open) then click `OK`.
- Expected: inline validation, or the dialog staying open with an explanation.
- Actual: the dialog closes, no `.tres` is created, and nothing is reported in the UI or the log.
  The folder stayed at only `mst_classic.tres` (`ls` after the click). The same click-through bug
  below fires on the way out.
- Evidence: qa/screenshots/r2-paint/20-preset-empty-submit.png (empty field, dialog still open);
  `texture_presets/` listing had no new file.
- Undo: not applicable

## Clicking OK in the embedded Save Preset dialog also activates the control underneath

- Severity: ui
- Steps: 1) `Export Texture Preset`. 2) Click `OK` (empty or filled name).
- Expected: the dialog consumes the click; nothing behind it changes.
- Actual: the dialog closes and the release/click reaches the editor underneath. With the empty
  name, the `Material` OptionButton below the dialog opened its 16-item popup (visible in the live
  frame; the popup listed `1. Texture 1` … `16. Void`). This makes it easy to change the active
  material accidentally while saving a preset. The same behaviour likely explains a stray
  `MST Classic` preset application seen earlier.
- Evidence: observed live after clicking OK at (799,501) (popup `@PopupMenu` with 16 items);
  the frame was later overwritten by the concurrent duplicate driver, so no clean PNG remains.
- Undo: not applicable (material selection is a tool setting)

## Texture panel is ~8 screenfuls tall; only two slots are visible at once

- Severity: ui
- Steps: 1) Paint tool. 2) Inspect the right panel / scroll through all slots.
- Expected: 16 slots usable without excessive scrolling.
- Actual: every slot repeats a full block (name, 128 px Texture picker, Pattern, Scale, Color,
  Has-grass, Sprite) so one slot is ~350 px tall. Content height is ~5,600 px in a ~700 px
  viewport: slot 1 starts at content y=-197 when scrolled to the middle, slot 16 is at y=4707 and
  `Export Texture Preset` at y=5530 in the same dump — roughly seven screenfuls of scrolling, and
  only ~2 slots visible at a time. All 16 are reachable (names inventoried in
  `ui/18-panel-bottom.json`).
- Evidence: qa/screenshots/r2-paint/05-paint-panel-continue.png,
  qa/screenshots/r2-paint/ui/18-panel-bottom.json
- Undo: not applicable

## Paint/Foliage tool chrome squeezes the 3D view to 534×256

- Severity: ui
- Steps: 1) 1600×900 editor. 2) Select the terrain and activate Paint.
- Expected: a workable viewport for painting.
- Actual: the fixed 250 px right texture panel plus the ~227 px bottom strip (y=413…640) leave the
  `SubViewportContainer` at **526,154 534×256**. The right panel also sits inside the 3D editor
  while the Inspector stays open to its right, so the terrain view is less than a third of the
  window width. No splitter/resize for the panel was found.
- Evidence: qa/screenshots/r2-paint/17-paint-reopen.png,
  qa/screenshots/r2-paint/ui/05-paint-panel.json (`SubViewportContainer@10230` rect)
- Undo: not applicable

## Texture-preset undo leaves stale slot/material names (carried over, not re-confirmed live)

- Severity: warning
- Steps: 1) Paint tool, apply `MST Classic`. 2) `Ctrl+Z`.
- Expected: slot names and the Material dropdown revert with the resource.
- Actual: the earlier killed attempt's dumps (`/tmp/opencode/qa/sessions/r2-paint/ui/
  preset-direct-undo.json`, `preset-mat.json`) still list `Texture 1 … Texture 16 (void)` after
  the undo, i.e. the MST Classic display names, while the scene's embedded set uses names such as
  `test grass`. This matches the 2026-11-20 report. This pass saw the inverse transition (the
  preset label/name changing) but the session was stopped before I could re-run the undo myself,
  so treat this as supporting evidence, not an independent re-confirmation.
- Evidence: `ui/preset-direct-undo.json`, `ui/preset-mat.json`; previous pass
  qa/screenshots/paint/19-undo-preset-stale-panel.png
- Undo: broken (data restored, labels stale) — per earlier evidence

## Session incidents (not addon findings, but they bound this pass)

- The editor reloaded its layout / closed the scene once mid-pass ("Loading editor layout… 0%",
  `[empty]` tab) with no error in `editor.log`; the reopened scene came back at terrain digest
  `3395664241` (initial `3064656926`).
- A second, still-running copy of this same QA brief from the earlier killed attempt was writing to
  `qa/screenshots/r2-paint/` concurrently (evidence: files `12-slot16-reachable.png`,
  `17-before-clean-paint.png`, `20-ctrl-pick-material.png`, etc. timestamped 04:44–04:47, and its
  `qa/session.sh stop r2-paint` at 04:48 killed the Xvfb/editor while my last commands were
  running). Several of my frames (e.g. `21-preset-empty-ok.png`) were overwritten by that driver.
  Results above are from commands I ran and observed in this conversation; the preset-undo item is
  flagged as carried over.

## Covered

- [x] Paint tool opens the right Texture panel; all 16 slots enumerated with names (Grass, Meadow,
      Forest Floor, Sand, Dirt, Cliff, Dark Rock, Snow, Gravel, Clay, Moss, Path, Stone Tiles, Ash,
      Ice, Void) and slot 16 `Void` reachable by scrolling (`ui/18-panel-bottom.json`, `05`, `12`).
- [x] Material dropdown lists all 16 slots and reports the active one (`1. Grass`…`16. Void`);
      selecting `8. Snow` updated the strip and the right panel scrolled to the selected block.
- [x] Paint stroke changes the terrain digest and is one undo step: `3064656926` →
      `3785910461`, action `terrain vertex paint`, `undo_count 1`; `Ctrl+Z` → `3064656926`,
      `has_redo true`; `Ctrl+Shift+Z` → `3785910461` (`terrain vertex paint`). Performed on the
      3D viewport (the drag leaves keyboard focus on the viewport).
- [x] Paint is visible in the viewport after the stroke (`13-paint-snow-after.png`; small at the
      default zoom of the squeezed viewport).
- [x] Export Texture Preset: dialog opens with a pre-filled name; `Esc`/X cancels; `OK` with a name
      writes `texture_presets/new_texture_preset.tres` **in the session project** and the
      `Texture Preset` dropdown immediately shows `new_texture_preset`
      (`23-preset-saved.png`; log: `Marching Squares Terrain: saved texture preset
      res://addons/MarchingSquaresTerrain/texture_presets/new_texture_preset.tres`).
- [x] `MST Classic` can be applied from the Texture Preset dropdown; the Material list then reads
      `Texture 1…Texture 16` (observed in the popup after the dialog click-through).
- [x] Bottom strip labels match the mission: Vertex Painting, Brush Type, Size, Aspect, Rotation,
      Material, Texture Name, Texture Preset, Paint Walls, Slope Min, Slope Max; Size slider drag
      works (15 → 99 during an accidental drag; restored to 15 via the numeric field).
- [x] No addon (`mst`/`MarchingSquares`) ERROR or SCRIPT ERROR in `editor.log` during this pass.

## Not tested

- Foliage palette: paint Grass/Tree/Rock/Pine/Bush/Flowers, Shift+RMB erase, Erase All, Fill
  Terrain, Clear, and their undo — session was stopped before the tool switch.
- Ctrl+LMB material picking, Paint Walls on a cliff, Slope Min/Max limiting, and Shift+wheel size.
- Texture-preset `None` restore and the preset-change undo (only the carried-over dump above).
- Save Preset overwrite confirmation and a custom (typed) preset name: on the second dialog the
  name field did not take injected keyboard input (embedded window focus), so the default
  `new_texture_preset` was saved.
- Slot rename undo, Ctrl+Z with the Texture Name LineEdit focused, undo after selecting another
  node, two strokes + two undos.
- 1280×800 resize, 250 px panel/scrollbar measurements at the smaller size, bottom action
  reachability, strip clipping.
- Grass height/slope limits and the `grass_enabled` toggle.

## Log scan

- Only the known startup noise (`Could not set V-Sync mode`; two `Missing .uid file` warnings; the
  debug adapter/language-server port lines) plus:
  - `SCRIPT ERROR: Cannot call method 'get_global_rect' on a previously freed instance.` at
    `qa/harness/harness.gd:486` — QA harness, triggered by `ui` dumps while controls were being
    rebuilt, not the addon.
  - `ERROR: scene/3d/node_3d.cpp:649 - Condition "!is_inside_tree()"` bursts — caused by the QA
    driver accidentally creating/deleting a scene node (`Ctrl+A` in the scene tree) and undoing the
    delete; Godot core, not `mst`.
  - Info: `Marching Squares Terrain: saved texture preset res://addons/…/new_texture_preset.tres`.
- No `mst`/`MarchingSquares` errors or warnings.
