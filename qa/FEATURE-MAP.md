# Feature test map

Coverage matrix for the addon's editor surfaces. Each area is one subagent pass. Steps assume
`qa/README.md` (session + `qa/drive.py`) and a session with `source .../env.sh`.

The tool UI is the **left icon toolbar in the 3D view** (only while the terrain is selected) +
the **bottom attribute strip** + the **right panel** (Paint/Foliage only). Selecting a Pixel*
node swaps the same containers to the pixelizer UI (sections + world panel).

Scenes: `02_island` (boot), `demo/hex_island` (rich), `03_pixel_look`, `04_world_mood`.
Re-select the terrain after opening a scene: `select --class MarchingSquaresTerrain`.

---

## Area 1 — Sculpting: Brush, Level, Smooth, Bridge (+ undo)

Expected: docs/03 Sculpting.md, docs/12 Cheat Sheet.

1. Select terrain. Click each tool (`Brush`, `Level`, `Smooth`, `Bridge`); screenshot the strip.
   Check the tool button stays pressed and the strip title/hint changes.
2. Brush, Drag mode: LMB-drag paints white cells (selection); LMB-drag starting on a white cell
   raises/lowers the selected cells. Shift adds, Ctrl removes, Alt drags one cell, RMB/Esc
   clears. Test each modifier; screenshot before/after.
3. Brush, Raise / Lower mode (Brush Mode dropdown → "Raise / Lower"): hold LMB raises toward
   Ceiling; RMB lowers toward Floor; LMB+RMB flattens toward Height; Ctrl+LMB/MMB/RMB picks
   Ceiling/Height/Floor. Verify the height value changes in the strip/hint.
4. Brush settings: Brush Type (Round/Square/…), Size, Aspect, Rotation, Flatten, Falloff, Peak,
   Grid Aligned, Grid Size, Quick Paint. Change each and observe the cursor footprint; wheel
   shortcuts (Shift = size, Ctrl = strength) must update the strip values.
5. Level: drag flattens to Height; Ctrl+LMB picks the hovered height.
6. Smooth: drag relaxes; Smooth Mode "Terraced" vs "Sloped" visibly differ; Strength works.
7. Bridge: drag from A to B builds a ramp; width/size affects it.
8. Undo: `terrain` digest before each stroke, then Ctrl+Z and Ctrl+Shift+Z. One stroke = one
   step. Try 2 strokes + 2 undos; undo after switching tools; undo with focus in the 3D view.
9. Toolbar extras: "Collapse" (toolbar compact mode; icon-only), "Grid" toggle (overlay on/off),
   hovering each button shows the tooltip; `1`–`9` switch tools.
10. Log: no new errors/warnings. Note any visual issue (cursor footprint offset, cursor stuck).
11. Screenshots: each tool's strip, a stroke before/after, undo result, compact toolbar, grid off.

Things worth flagging: stroke that does not raise when the hint says it should; undo reverting
more/less than one stroke; Ctrl+Z doing nothing after clicking the 3D view; cursor not following
the mouse; brush size not clamped at edges; Level/Smooth doing nothing on hex grid.

---

## Area 2 — Painting + Foliage + textures (+ undo)

Expected: docs/04 Painting the Ground.md, docs/05 Plants, Trees and Props.md.

1. Paint tool (`Paint`): right panel appears with 16 texture slots, previews, name field and
   ground/wall pickers; bottom strip has Brush Type, Size, Aspect, Rotation, Material, Texture
   Name, Texture Preset, Paint Walls, Slope Min/Max.
2. Paint on the terrain; the painted material visibly changes. Ctrl+LMB picks the material
   under the cursor. Test Paint Walls on a cliff face, and slope min/max limiting.
3. Click several slots; the preview/name changes; paint with each. Assign a preset via the
   Texture Preset dropdown; "Export Texture Preset" (button in the panel) opens a dialog —
   cancel it and then save one; check the file appears under
   `addons/MarchingSquaresTerrain/texture_presets/` in the **session** project.
4. Texture presets: select one, verify slots update and painting changes; "None" restores.
5. Foliage tool (`Foliage`): right palette (Grass/Tree/Rock/+Pine/+Bush/+Flowers), size,
   density, slope min/max, Erase All, Fill Terrain, Clear. Paint each type; Shift+LMB or RMB
   erases. Fill Terrain populates the whole terrain (wait; it can be slow). Clear empties.
6. Grass: verify grass appears only within height/slope limits; toggling grass in the scene's
   Terrain Settings → Grass or the terrain `grass_enabled` changes the view.
7. Undo each class: paint stroke, scatter stroke, Fill Terrain, Clear, texture preset change.
   Note if Fill/Clear cannot be undone (should be one action).
8. Log and screenshots: panel, painting before/after, each foliage type, erase, fill/clear,
   preset export dialog, undo results.

---

## Area 3 — Inspect, Chunks, Settings, Stamp, Gradient, Select, Tint (+ undo)

Expected: docs/02, docs/06, docs/12, docs/13.

1. Inspect (`Inspect`): LMB a cell prints its data to the Output panel; screenshot the Output.
   Check no error spam when clicking water/sky (no cell).
2. Chunks (`Chunks`): strip shows Chunk dropdown, Remove Chunk, Merge Mode, Apply to All, and
   the viewport shows green + handles and red X on chunks. Add a chunk (green +), remove one
   (red X / button), select with Ctrl+LMB, change merge mode, apply to all. Undo each.
3. Settings (`Settings`): six pages — Grid, New Chunks, Grass, Display, LOD & Nature, Storage.
   Visit every page; screenshot each. Toggle/change representative fields on each (grid type,
   chunk size, cell size, blend mode, default slots, noise, grass fps, pixel size, grid
   overlay, fog, LOD ring, storage mode, export path). Check dependent fields disable correctly
   (e.g. Grid Size only when Grid Aligned; Hex fields only on hex grid).
4. Storage page: Save Data, Import, Bake Meshes, Export GLB / Export Path (buttons and file
   dialogs). Save to a temp path in the session project; reimport; verify no crash. Do not
   overwrite demo assets — use `res://qa_tmp/`.
5. Stamp: choose each built-in stamp, size/aspect/rotation/mode; click to stamp; drag to pull
   size/rotation; Shift/Ctrl/Alt+wheel. Check preview follows the cursor. Undo.
6. Gradient: pick two points (LMB drag) and verify the ground is pulled toward the ramp; modes
   and strength.
7. Select: drag a region, `C` copy, `V` paste mode, `Z`/`X` rotate, `M` mirror, LMB paste,
   RMB/Esc leave. Undo each paste. Check pasted terrain is offset/rotated as advertised.
8. Tint: choose a tint color, paint; the overlay appears; erase/clear if available. Undo.
9. Window/panels: Chunks strip and Settings page tabs must fit at 1600×900 (no clipped text).
10. Log and screenshots for every page/tool plus undo results.

---

## Area 4 — Pixel look: Renderer, Palette, Object, Preview/Animate/Pixel World (+ undo)

Expected: docs/07 The Pixel Look.md, docs/08 (Animate).

Use `02_island` / `03_pixel_look` / `hex_island`.

1. `Pixel Preview`, `Animate`, `Pixel World` buttons in the 3D view menu bar:
   - Pixel Preview toggles the pixel look in the editor (grid/axes hidden); screenshot before/
     after; toggle off restores.
   - Animate toggles animation in the editor (clouds/water/birds move); compare two screenshots
     ~2 s apart; toggle off freezes.
   - Pixel World opens the right-side panel: list of pixel nodes with select/add buttons,
     lighting presets, "Wire refs", feature switches. Screenshot; select a node from the list;
     add a missing effect via Add; apply a lighting preset; use Wire refs.
2. Select `PixelRenderer`: left toolbar sections (Renderer, Outlines, Color, God Rays, Sky,
   Day/Night, Weather, Water, Fireflies, Particles, Birds, Smoke, Lights, Object, Debug,
   Grid/Fog, ...). Visit each; screenshot the strip; change representative fields (pixel size,
   mode, outlines thresholds/darken, palette, god ray strength/color, camera group).
   Verify the viewport changes and undo works (section changes should be undoable).
3. Pixel Palette: select `PixelPalette` (Color section); test palette presets, LUT bake menu
   item (`Project > Tools > Bake PixelPalette LUT to PNG` or the addon's menu), color changes.
4. PixelObject + PixelObjectProfile: select the object in hex_island/courtyard; sections
   (Object) fields; change outline/profile; check the prop's pixel outline.
5. PixelOutlineColors resource: assign/create; change colors; check outlines.
6. Feature switches (Project Settings → marching_squares_terrain/features): toggle one effect
   off/on and verify the node's output disappears/appears; restore.
7. Undo: one change per section → Ctrl+Z reverts exactly that field; redo restores. Check slider
   drags merge into one undo step (drag, release, one Ctrl+Z).
8. Screenshots of each section and before/after for preview/animate. Log any `ERROR` from
   changing a section (missing node, null shader, etc.).

---

## Area 5 — Effects: Sky, Day/Night, Weather, Water, atmosphere, erosion, wild flowers

Expected: docs/08, docs/09, docs/13. Use `04_world_mood`, `02_island`, `hex_island`.

1. PixelSky: open the Sky section; change cloud coverage/speed/color/shadow; two screenshots
   over time show drift when Animate is on. Check the section's page tabs.
2. PixelDayNight: set time of day 0.0 / 0.25 / 0.5 / 0.75; the sun/sky colors change. Test
   presets (dawn/noon/dusk/night), sun/moon lights, ambient, follow-camera.
3. PixelWeather: all six types (Clear, Cloudy, Foggy, Rain, Snow, Storm); screenshot each;
   check lightning on Storm and that rain/snow particles are visible; test intensity/wind.
   Restore Clear.
4. PixelWater: water level, color, wave speed/height, foam, transparency; toggle the water node
   off/on. Waterfall child if present.
5. Fireflies: visible at night, count/color/range; invisible at noon (expected); toggle.
6. Birds: flocks move when Animate is on; count/height/speed.
7. Smoke + PixelFireSystem hearth: smoke rises, fire light flickers; extinguish/relight.
8. ParticleField: dust motes/leaves visible; size/count/color.
9. LightVolume: lantern cone visible; color/energy/range.
10. SunShafts: shafts through trees/pillars; strength/color; visible only from certain angles.
11. Erosion (MarchingSquaresErosion child): add under terrain, run erosion in the strip; terrain
    changes; undo. Check settings (iterations, strength, droplet count).
12. WildFlowers (MarchingSquaresWildFlowers): add; scatter/paint flowers; density/color/slope;
    undo.
13. Feature switches: turn each effect's project switch off/on and verify disappearance/
    reappearance (clouds, god rays, water, weather, birds, fireflies, smoke, fire, particles,
    light volumes, sun shafts).
14. Undo/redo for every section change. Screenshots for each effect and each weather type.

---

## Area 6 — Builder: PixelBuilderMesh (+ undo)

Expected: docs/10 Making Props in the Builder.md.

1. Open `demo/hex_island` (has `BuilderRuins/Ruin` PixelBuilderMesh) or add a `PixelBuilderMesh`
   via the Scene tree + button (search class). Select it.
2. Builder UI appears: "Pixel Builder - <mode> mode, <tool>" label at top-left, a **Builder**
   menu in the 3D view's menu bar, a bottom strip (Shape, Smooth, Color, W/H/D), a right
   Materials panel. Screenshot each surface.
3. Modes: `1` Object, `2` Face, `3` Edge, `4` Vertex, `H` cycles. Verify the label/mode changes.
4. Shape: choose Box/other, set W/H/D, press `C`, drag the footprint, move up for height, click
   to finish. The mesh appears; undo (Ctrl+Z) removes it as one action. Shift+C draw appends to
   the selected mesh (vs a new sibling).
5. Tools: `W` select/move, `E` extrude, `I` inset, `Ctrl+B` bevel, `Ctrl+R` edge loop, `M`
   merge, `J` connect, `F` fill, `Ctrl+T` triangulate, `Ctrl+D` duplicate, `Delete`, `G`/
   `Shift+G`, `L`/`Alt+L`, `U` UV editor, `[`/`]` grid step, `Esc` cancel. Test a
   representative subset in Face/Edge/Vertex mode; screenshot before/after.
6. Paint: `B`, click faces with the strip Color; `Shift+click` fills a flat area; `Ctrl+click`
   picks. Vertex paint `5`. Materials panel: `+ Slot`, `+ Preset` (Grid, Checker, No Draw,
   Collider, Trigger), drag a material onto a face if feasible.
7. Smooth/faceted switch; collision toggle; rebuild warning when W/H/D changes after vertex
   edits (should warn/confirm, not silently destroy).
8. Builder menu: "Modeler layout" swaps to the larger palette; screenshot both; Add Mesh grid.
9. UV editor (`U`): open; move/scale/rotate UVs; screenshot. Export (STL/GLB) if available;
   save/reload the scene and verify the mesh persists.
10. Undo/redo for: draw, move, extrude, bevel, paint, material assign, delete. Each gesture is
    one undo step; check the History dock entries.
11. Log any builder script errors; screenshot the builder UI at 1600×900 and 1280×800.

---

## Area 7 — UI / resizability audit (FEEDBACK.md core)

Expected: user preference "things should be resizable, but not everything needs to be".

1. At 1920×1080, 1600×900, 1366×768, 1280×720, 1024×600:
   - `resize W H`, wait 1–2 s, `capture` the full window for each tool/section.
   - For each surface (terrain toolbar, each tool strip, Paint panel, Foliage panel, Chunks
     strip, Settings pages, Pixelizer toolbar/strip/world panel, Builder strip/materials,
     dialogs/popups): check for clipped labels, overlapping controls, horizontal scrollbars,
     controls pushed off-screen, tiny hit targets, missing scroll, fixed widths that waste
     space, panels that cannot be resized/collapsed.
   - Record `ui` dumps: rects, `min`, `custom_min`, `clip_contents`, `size_flags`, `outside_root`.
2. Side panels: are the 250 px Texture/Foliage/Pixel World panels resizable? Is there a splitter?
   Does the 3D viewport get too narrow at 1024? Do panels scroll?
3. Bottom strip: does it wrap (MstFlow) instead of clipping? At 1024×600 do the Settings page
   tabs and fields all remain reachable? Is there a scrollbar?
4. Toolbar: Collapse/compact mode; does the compact toolbar still fit vertically at 600 px
   height? Do tooltips appear?
5. Dialogs: Export Texture Preset (LineEdit + OK/Cancel), preset overwrite confirmation, Import
   Data, bake/export dialogs, Builder menus, Pixel World add menu. Check each is resizable
   (or deliberately not), fits, has readable text, and remembers size if expected.
6. Text scale / HiDPI: set `Editor Settings > Interface > Editor > Display Scale` if reachable,
   or just note fixed pixel sizes (`35 px` rows, `30 px` buttons, `250 px` panels) that will
   not scale. Check for hard-coded sizes in the UI that do not use `editor_scale`.
7. Keyboard-only: Tab order through the strip/panels; focus ring visibility; Escape closes.
8. Mouse targets: sliders/spin boxes/checkbox hit areas; are the 30 px toolbar icons too small?
9. Screenshots at every size for every panel; write `qa/findings/ui.md` with concrete
   recommendations (what should be resizable vs what can stay fixed).

This area feeds `FEEDBACK.md`; be opinionated but concrete.

---

## Area 8 — Deep undo/redo

1. For every terrain tool (Brush/LR, Level, Smooth, Bridge, Paint, Foliage, Stamp, Gradient,
   Select paste, Tint, Chunks add/remove/merge, Settings changes, terrain Import/Bake):
   - one gesture → Ctrl+Z → original digest; Ctrl+Shift+Z → edited digest.
   - two gestures → two Ctrl+Z (in reverse order); two Ctrl+Shift+Z.
   - undo after switching tools; undo after selecting another node; undo after clicking in the
     ​3D view; undo with a LineEdit focused (should edit the text field, not the terrain).
2. Check the History dock lists each action with a sensible name ("terrain level", "terrain
   paint", ...). Check undo of a stroke that touched multiple chunks restores all of them.
3. Check undo of Fill Terrain / Clear / Erosion / wild-flower scatter / texture preset change.
4. Check undo of a slider drag merges into one action (not one per value).
5. Redo stack is cleared by a new edit (standard behavior) — verify.
6. Save scene, reload it, and confirm the undo history is empty/not restoring stale state.
7. Find any action that is not undoable and would lose user work (high severity).

---

## Area 9 — Runtime / demo (`godot` game, no editor harness)

1. Launch a plain game session on a spare display:
   `DISPLAY=:4N godot --path <session project> res://demo/hex_island.tscn` (or tutorial 01–04).
   Screenshot with `python3 qa/drive.py --display :4N shot out.png` (choose a display with no
   editor).
2. Demo controls: F1 hint overlay, F3 diagnostics (time of day / weather changes), camera orbit
   (middle-drag), zoom, WASD/fox movement, Esc. Confirm they work and the UI is legible.
3. Check the pixel look at runtime (not editor preview): outlines, palette, water, sky, weather,
   effects. Compare with docs' screenshots.
4. Tutorial scenes 01–04: each loads without errors; camera controls work; the scene looks as
   documented.
5. Check the game window resize (Godot window resize) — does the pixel renderer scale correctly,
   does the UI/hint stay legible, does the aspect ratio break the pixel grid?
6. Collect console warnings/errors (`2>&1 | tee`), screenshots, and any crash.
