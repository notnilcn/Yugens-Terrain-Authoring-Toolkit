# Area 2 — Painting + Foliage + textures (+ undo)

Session: `paint` (display `:32`, port `8812`, project copy `/tmp/opencode/qa/sessions/paint/project`).
Scenes: `demo/hex_island.tscn` (paint/texture/preset tests) and `tutorial/02_island.tscn`
(foliage Fill/Clear, small terrain). All coordinates 1600x900.

Log scan: no `ERROR:` / `SCRIPT ERROR:` and no new warnings in
`/tmp/opencode/qa/sessions/paint/editor.log`; only the known
`Could not set V-Sync mode ...` startup warning. The save/export path logs an info line:
`Marching Squares Terrain: saved texture preset res://addons/MarchingSquaresTerrain/texture_presets/qapainttest.tres`.

## Shift+wheel does not change the brush Size

- Severity: warning
- Steps: 1) Select terrain, Paint tool. 2) Hover the 3D viewport and Shift+wheel up 4 times,
  then down 5 times. 3) Read `Size:` in the strip.
- Expected: brush Size changes (the strip hint says `Shift+wheel: size`; docs/04 says the same).
- Actual: `Size` stays `15.0` for every Shift+wheel step, over both the island and open water.
  Plain wheel still zooms the camera, so the wheel reaches the viewport.
- Evidence: qa/screenshots/paint/42-brush-square-aspect-rot.png (hint + `Size: 15.0`),
  qa/screenshots/paint/44-grass-on.png (same hint, `Size: 15.0`)
- Undo: not applicable

## Slope Min spin slider accepts values far above 90 (e.g. 8287.5)

- Severity: warning
- Steps: 1) Paint tool. 2) Drag the `Slope Min` EditorSpinSlider from x=640 to x=665 at
  y=605. 3) Read the field, then drag a paint stroke on the island.
- Expected: field stays inside the documented 0–90 degree range and painting is limited to
  slopes in that range (the field is declared `Spin(0.0, 90.0, 0.5)` in tools.rs).
- Actual: field becomes `8287.5` with no unit; the slider has no effective clamp
  (`set_allow_greater/lesser(true)` in ui_kit::spin_slider). Painting then silently no-ops
  because no slope can exceed 8287 degrees. Typing a value works, but the slider itself is
  unbounded and one 25 px drag jumps thousands of degrees.
- Evidence: qa/screenshots/paint/37-slope-min-8287.png,
  qa/screenshots/paint/38-slope-min-blocks-paint.png
- Undo: not applicable (tool setting)

## Undo of a Texture Preset reverts the resource but leaves the Material names stale

- Severity: warning
- Steps: 1) Paint tool, Texture Preset = `MST Classic` (Material becomes `1. Texture 1`).
  2) `Ctrl+Z`. 3) Inspect `tex_assign` and the Material dropdown / right panel.
- Expected: both the terrain's texture set and all labels/names revert to the previous set
  (the scene embedded set, slot 1 `test grass`).
- Actual: `prop tex_assign` confirms the resource is back to
  `res://demo/hex_island.tscn::MarchingSquaresTexturePreset_82xx5`, but the Material dropdown
  still lists `1. Texture 1` and the slot panel still shows `Texture 1` (MST Classic names).
  Exporting that same terrain writes slot 1 as `test grass`, proving the panel is showing the
  pre-undo names. A later preset save/refresh corrects the labels.
- Evidence: qa/screenshots/paint/17b-preset-panel.png (before undo),
  qa/screenshots/paint/19-undo-preset-stale-panel.png,
  qa/screenshots/paint/19b-undo-preset-window.png; the exported
  `qapainttest.tres` contains `display_name = "test grass"`.
- Undo: broken (data restored, UI labels stale)

## Ground-texture preview is black for `mst_noise.png`

- Severity: ui
- Steps: 1) Paint tool. 2) Texture Preset = `MST Classic`. 3) Look at the `Texture:` resource
  picker preview of slot 1 (and slot 2, both use `mst_noise.png`).
- Expected: the picker shows the bundled 128x128 grey noise texture (the slot's small tinted
  preview on the left already shows it).
- Actual: the large `Texture:` preview is solid black for every slot using
  `textures/ground/mst_noise.png`; other textures (dark_rock, snow, grass) preview correctly.
  The one-line tooltip still names the correct file, so only the thumbnail is wrong.
- Evidence: qa/screenshots/paint/17b-preset-panel.png,
  qa/screenshots/paint/19-undo-preset-stale-panel.png,
  qa/screenshots/paint/27-preset-undo-stale.png
- Undo: not applicable

## Save Preset dialog has no Cancel button

- Severity: ui
- Steps: 1) Paint panel, scroll to the bottom, `Export Texture Preset`.
- Expected: docs/04 describe a name field with OK/Cancel; the overwrite confirmation does have
  both buttons.
- Actual: the Save Preset dialog only shows `OK`; cancellation is only possible with `Esc` or
  the window close X. `Esc` does cancel and writes no file.
- Evidence: qa/screenshots/paint/20-save-preset-dialog.png,
  qa/screenshots/paint/21-save-preset-cancelled.png
- Undo: not applicable

## Empty preset name closes the dialog silently

- Severity: minor
- Steps: 1) `Export Texture Preset`. 2) Clear the name field. 3) `Enter`.
- Expected: an inline error/validation, or the dialog staying open.
- Actual: the dialog closes, no `.tres` is created, and there is no message. The only way to
  know it failed is to check the folder.
- Evidence: qa/screenshots/paint/22-save-preset-empty-name.png (dialog gone; only
  `mst_classic.tres` in the folder afterwards)
- Undo: not applicable

## Fill Terrain is a no-op for the Tree item on 02_island (but records a step)

- Severity: minor (possibly intended item-rule filtering; listed because the UI gives no reason)
- Steps: 1) Open `tutorial/02_island.tscn`, select terrain, Foliage tool. 2) Palette has
  `Tree` selected (`Painting Tree` in the hint). 3) `Slope Min` = 0. 4) Click `Fill Terrain`.
- Expected: the whole terrain is repopulated (docs/05 says Fill uses each item's own rules and
  replaces the current instances); 97 trees existed before.
- Actual: `get_scatter_count(0)` stays exactly 97; the first click adds a
  `terrain foliage fill` undo entry with no data change, subsequent clicks add no step.
  Selecting `Rock` and filling the same terrain does work (766 -> 752 instances; undo restores
  766, redo returns 752), so the button itself is wired. Likely the Tree item's ground-slot
  mask matches no current cell; the palette should say why it placed nothing.
- Evidence: qa/screenshots/paint/49-fill-terrain-slope0.png,
  qa/screenshots/paint/50-fill-terrain-later.png,
  qa/screenshots/paint/53-fill-rock.png
- Undo: Fill step is created but cannot restore data (there is nothing to restore);
  Rock fill undo/redo works

## Paint Walls hint still reports "Vertex Painting" and a height

- Severity: minor
- Steps: 1) Paint tool. 2) Tick `Paint Walls`. 3) Paint on a cliff.
- Expected: the status line acknowledges wall mode.
- Actual: the action is a distinct `terrain wall paint` undo step (good), but the live hint
  under the viewport still reads `Vertex Painting | height 5.00 cell ...`.
- Evidence: qa/screenshots/paint/34-paint-walls-snow.png
- Undo: works (`terrain wall paint`)

## Covered

- [x] Paint tool opens the right-side Texture panel; captured top, scrolled slots and bottom
      strip (`01`, `02`, `03`, `04`, `12`, `17b`, `19`).
- [x] Slot buttons `Texture 1`, `Texture 5`, `Texture 8`, `Texture 12`, `Texture 16 (void)`
      change the Material dropdown and paint: slots 5 (Dirt), 8 (Snow), 12 (Path) painted;
      slot 16 (Void) recorded `terrain vertex paint` and is undoable.
- [x] The Material dropdown lists all 16 slots with names and updates with renames.
- [x] Ctrl+LMB picks the material under the cursor: selected Grass, Ctrl+click picked
      `4. Sand`, `2. Meadow`, `8. Snow` at different cells; verified via Material text.
- [x] Paint Walls: checkbox toggles; cliff stroke creates a distinct `terrain wall paint`
      undo action and changes the digest.
- [x] Slope Min/Max: value 89 blocks painting on the island (no digest change, no undo step);
      value 0 allows it. (Slope Min 35 still painted some "flat-looking" top cells, i.e. those
      cells are steeper than 35; not treated as a failure.)
- [x] Brush Type dropdown: Round -> Square (all six types listed: Round/Square/Hexagon/
      Hexagon30/Triangle/Triangle180). Aspect slider changed 1.000 -> 3.584. Rotation typed 45.
      Size slider accepts keyboard/drag but see Shift+wheel finding.
- [x] Slot name LineEdit: text changes the Material name and creates an undo action
      `terrain texture 1 display_name`; with real focus retained, Ctrl+Z reverts the name and
      does not touch the terrain digest. (Harness default `key z --ctrl` releases focus first,
      which made it look like a no-op; raw unfocus=false test confirmed correct behavior.)
- [x] Texture presets: `MST Classic` applied (names -> `Texture 1...`, `terrain texture preset`
      undo action), `None` restores default names, custom `qapainttest` appears in the dropdown
      after export. Preset undo restores the resource; UI names are stale (finding above).
- [x] Export Texture Preset: dialog opens with a pre-filled name and OK; `Esc` cancels with no
      file; empty name silently ignored; saving `qapainttest` writes
      `.../texture_presets/qapainttest.tres` in the session copy; exporting the same name again
      shows the overwrite confirmation with Cancel/OK; Cancel keeps the old file.
- [x] Foliage palette: Grass, Tree, Rock, +Pine, +Bush, +Flowers (plus Pebbles on hex_island);
      Edit/Remove and Add Mesh/Scene visible. Palette fits and scrolls; screenshot `28`, `29`,
      `47`.
- [x] Foliage paint: Tree 173 -> 189, Rock 429 -> 453, Flowers 89 -> 107; each is one undo step
      (`terrain foliage paint props`); Ctrl+Z/Ctrl+Shift+Z restore/redo counts.
- [x] Shift+LMB erases foliage (Flowers 107 -> 76, `terrain foliage erase props`, undoable).
- [x] Fill Terrain: works for Rock on 02_island (766 -> 752), one undo step; undo/redo verified.
      Tree no-op documented above.
- [x] Clear: Tree 97 -> 0 (`terrain foliage clear`); Ctrl+Z restores 97.
- [x] Undo after switching Paint <-> Foliage reverts the last foliage action (Fill 752 -> 766
      after switching to Paint) and redo returns 752. Paint undo/redo after switching tools also
      kept the digest stable.
- [x] Paint stroke undo: digest 3064656926 -> 1905370340 -> Ctrl+Z 3064656926 -> Ctrl+Shift+Z
      1905370340, one step per stroke; repeated for Path and Void strokes.
- [x] Grass: `set-prop grass_enabled false` visibly removes the grass layer and true restores it
      (`44`/`45`/`46`); the terrain's grass inspector page is present.
- [x] No new errors/warnings in the editor log across paint, preset export, foliage, fill/clear.

## Not covered (and why)

- Grass palette painting on/off with the Foliage brush: only the global `grass_enabled` toggle
  was tested (time).
- Erase All checkbox behavior (right-drag removing every prop) and multi-item Ctrl/Shift
  selection in the palette: not tested (time).
- Pattern, Scale, Color and Grass Sprite pickers inside a slot: inspected but not changed/
  painted; the texture resource picker was opened but its popup did not stay open long enough
  to exercise; time.
- Grass height/slope limits (Min Height) and Settings -> Grass page fields: not tested (time;
  different area for the Settings page).
- Slope Max lower-bound behavior (only Slope Min limit was proven) and Paint Walls on a true
  vertical face with a second material: not tested.
- Undo Ctrl+Shift+Z specifically for a slot rename: only Ctrl+Z was verified; paint/foliage/fill
  redo were verified.
- Fill/Clear on `demo/hex_island` not tested; mission asked for the smaller 02_island.
- Resizability of the 250 px Paint/Foliage panels at smaller window sizes: other area.
