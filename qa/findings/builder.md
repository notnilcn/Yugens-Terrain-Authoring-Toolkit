# Builder QA — PixelBuilderMesh, draw, modes, edit, paint, materials, UV, layout, undo

Session `builder` on display `:33`, port `8813` (Godot 4.7.2, llvmpipe, editor window
1600x900, restored to 1600x900 after the 1280x800 pass). Scene `res://demo/hex_island.tscn`
(`HexIsland/BuilderRuin/Ruin`, plus a `Box` created during the pass and saved). Nothing under
`addons/` or the repository demo scenes was modified; the scene save happened in the session
copy only (`/tmp/opencode/qa/sessions/builder/project`).

Editor log: `/tmp/opencode/qa/sessions/builder/editor.log` produced **no addon errors** during
the pass. The six `ERROR` lines in the log (`godot-rust function call failed:
PixelBuilderPlugin::debug_placement()`) were induced by the QA harness while probing the
`debug_placement` scripting hook with JSON values (it needs a real `Vector2`); they are not
addon defects and are excluded below. Startup noise (`V-Sync`, debug adapter / GDScript
language server ports, missing `.uid`) is the known set.

## Number keys 1–4 never switch the builder mode (docs promise Object/Face/Edge/Vertex)

- Severity: error
- Steps: 1) Select `BuilderRuin/Ruin`, click the 3D viewport once (viewport focused), confirm
  `Object` mode. 2) Press `2`, wait, press `3`, press `4`, press `1`/`3`/`4`/`2` again.
  3) Compare with the toolbar buttons and with `H`.
- Expected: `1` Object, `2` Face, `3` Edge, `4` Vertex, as documented in
  `docs/10 Making Props in the Builder.md` ("The four modes" table) and in the toolbar
  tooltips (`Face mode [2]`, `Edge mode [3]`, `Vertex mode [4]`).
- Actual: The mode never changes from the keyboard. `H` works but cycles *backwards*
  (`Face → Object → Vertex → Edge → Face`), and only after the viewport/button focus was in the
  editor. Mode changes only work by clicking the `ModeObject`/`ModeFace`/`ModeEdge`/
  `ModeVertex` buttons. `B`/`C`/`H` do reach the builder, so this is specific to digits —
  Godot's 3D-view shortcuts consume `1`–`4` before the plugin (the same failure was recorded
  for the terrain toolbar in `qa/findings/sculpt.md`).
- Evidence: qa/screenshots/builder/ui/27-state.json (state `"mode": "Object"` after the digit
  sequence); qa/screenshots/builder/06-mode-vertex.png (mode reached only via `H`);
  qa/screenshots/builder/07-mode-face.png and 14 (mode after clicking the toolbar button).
- Undo: not applicable (no edit).

## Face/Edge/Vertex strip squeezes the 3D viewport to 39 px at 1600x900

- Severity: error
- Steps: 1) Select a PixelBuilderMesh. 2) Stay in the default `MST` layout. 3) Click
  `ModeFace` (or press the button, since `2` is dead). 4) Measure the viewport and the strip.
- Expected: The bottom attribute strip is scrollable/collapsible and leaves a usable 3D view
  for selecting faces/edges/vertices (docs describe the strip as a row of Shape, Smooth,
  Color and size fields).
- Actual: Selecting Face mode shows the full face-operation set (Color, Grid, Dist, Inset,
  Profile, Inset by, Subdiv, Axes, Trim at, T depth/height, Off x/y/z, Space, Grow, Extrude,
  To new object). `PixelBuilderStrip.get_combined_minimum_size()` is **238x447** and is not in
  a ScrollContainer, so the VSplit gives the strip 447 px and the `Node3DEditorViewportContainer`
  becomes **345x39** (global rect `[706,154,345,39]`). The HUD status lines are clipped away and
  the island is a 39 px sliver. Clicks aimed at the model land on the strip, so face selection is
  effectively impossible without resizing or switching to the Modeler layout. Reproduced directly
  after creating `Box` and switching to Face mode.
- Evidence: qa/screenshots/builder/14-box-selected-face-mode.png (39 px view + full face strip),
  qa/screenshots/builder/15-face-selected.png, qa/screenshots/builder/ui/27-builder-1600x900.json
  (strip min/rect; viewport container rect).
- Undo: not applicable.

## 1280x800 MST layout clips the toolbar labels, the strip and the viewport

- Severity: warning
- Steps: 1) Select the builder mesh in the MST layout (1600x900). 2) `drive.py resize 1280 800`,
  wait 2 s, screenshot. 3) Restore 1600x900.
- Expected: The toolbar/strip stay readable, fields wrap or scroll, and the 3D viewport keeps a
  usable size.
- Actual: At 1280x800 the viewport shrinks to roughly 172x60 px; the bottom strip is cut by the
  Materials panel (only Slot, Color, Grid, Subdiv, Weld, mirror checkboxes visible, `Editing
  builder mesh` label clipped); toolbar headers collide with the icons (`Combi` for Combine);
  the Materials palette loses its lower content. Restoring 1600x900 restores the layout.
- Evidence: qa/screenshots/builder/28-builder-1280x800.png,
  qa/screenshots/builder/29-builder-restored-1600x900.png.
- Undo: not applicable.

## A newly drawn shape is created but the selection stays on the previous mesh

- Severity: minor
- Steps: 1) Select `BuilderRuin/Ruin` in Shape/Box tool. 2) Drag a footprint and click to finish
  (no Shift). 3) Watch the Scene tree, Inspector and HUD.
- Expected (docs beginner workflow): the new sibling shape is ready to edit/paint; the builder
  HUD says "Created Box" and the workflow continues with `W`/`B`.
- Actual: The new `Box` node is added under the same parent (`BuilderRuin/Box`), but the Scene
  tree/Inspector and the builder target remain on `Ruin` (HUD still reports the 209-vert Ruin;
  `select --path BuilderRuin/Box` was needed to edit or paint it). A user who presses `B` right
  after drawing paints the old mesh.
- Evidence: qa/screenshots/builder/13-shape-created.png (tree `Box` unselected, `Ruin` selected,
  HUD "209 verts ... Created Box").
- Undo: works — one `Ctrl+Z` removed the new `Box` (undo action `Pixel Builder: Create Box`),
  `Ctrl+Shift+Z` restored it.

## Modeler layout shows a modal-looking "Modeler / Created Box" toast over the viewport

- Severity: ui
- Steps: 1) Builder menu → `Modeler layout`. 2) Create/draw a shape or switch layout with edits
  pending. 3) Look at the centre of the 3D view.
- Expected: HUD status stays at the top-left; the viewport remains clear.
- Actual: A centred "Modeler" card (`Created Box` + `Dismiss`) covers the model and blocks
  clicking the faces underneath until dismissed. It also appears immediately after toggling
  the layout, not only on a relevant action.
- Evidence: qa/screenshots/builder/16-modeler-layout-face.png (toast over the island/gizmo).
- Undo: not applicable.

## H cycles modes in reverse order

- Severity: minor
- Steps: Start in Face mode; press `H` four times (with the viewport focused).
- Expected: A predictable cycle (docs only say "H cycles through the modes").
- Actual: `Face → Object → Vertex → Edge → Face` (mode − 1), i.e. the reverse of the documented
  1→2→3→4 order. Works, but is surprising next to the dead number keys.
- Evidence: qa/screenshots/builder/06-mode-vertex.png (after the first H from Face).
- Undo: not applicable.

## Covered

- [x] Selected `BuilderRuin/Ruin`; builder UI appears: HUD label, **Builder** menu, bottom strip,
      right **Materials** panel (qa/screenshots/builder/02-builder-ui-selected.png).
- [x] Builder menu opened; menu items present (UV editor, Preferences, Bake, Export OBJ/glTF/STL/
      PLY/Godot mesh, options, reusable part, lightmap UVs, **Modeler layout**) — 03-builder-menu.png.
- [x] Modeler layout toggled on (large viewport; 04-modeler-layout.png, 16-modeler-layout-face.png)
      and back to MST (21/27).
- [x] Add Mesh grid in the Modeler toolbar; 05-add-mesh-grid.png.
- [x] Mode buttons Object/Face/Edge/Vertex change the HUD and the strip; 06/07/14.
      Number-key shortcuts and H cycle noted above.
- [x] Shape = Box, W/H/D fields, `C` draw tool; top view used to reach the island surface;
      drag footprint + move + click created a new sibling **Box** (12/13); one-step undo/redo.
- [x] Face mode: clicked the top face of the created Box (1 face selected, HUD updates);
      17-face-selected.png / 15.
- [x] Extrude `E` with a face selected: committed `Pixel Builder: Extrude` as one undo step;
      Ctrl+Z reverts (version 10→9), Ctrl+Shift+Z restores (9→10); 18-extrude.png.
- [x] Paint `B`: Paint popover opens; color swatches select a color (state.json `color` changed
      to blue); clicking a face commits exactly one `Pixel Builder: Paint` undo step and
      Ctrl+Z/Ctrl+Shift+Z toggle it (version 11→10→11); 19/21/22/23.
- [x] Vertex paint `5` tool button exists and is reachable (not exercised on geometry).
- [x] Materials panel: `+ Slot` present, `+ Preset` menu lists Grid, Checker, No Draw, Collider,
      Trigger (30-material-presets.png); adding **Grid** created a third slot and one undo step
      `Pixel Builder: Add preset slot`, Ctrl+Z/redo verified (31/32/33).
- [x] Smooth/Faceted/New group controls and the "Rebuilding replaces the element edits. Undo can
      restore them." warning are visible in the Inspector (16/17/33); no confirm dialog was
      produced by **Edit Shape** (it reopened the recipe fields, HUD "Editing the Box recipe…").
- [x] UV editor `U`: opens the "Pixel Builder - UV Editor" window with Faces/Verts, Manual/Auto/
      Planar/Box/Spherical, Rot/Fit/Copy/Paste/Texel/Island/Frame, Auto UVs template and a UV
      canvas; 24-uv-editor.png. A marquee + drag was attempted (25-uv-moved.png), **not visually
      verified**.
- [x] Save + reopen: `Ctrl+S`, `open-scene res://demo/hex_island.tscn`; both `Ruin` and the new
      `Box` persist with the same transform (-6,10,-3.464 and 31.5,2.5,11.258) and the Box's
      edited 12 verts/20 edges/10 faces + 3 material slots; 26-reloaded-scene.png.
- [x] 1600x900 ↔ 1280x800 resize screenshots for the builder strip/Materials/viewport
      (27/28/29).
- [x] Undo names seen: `Pixel Builder: Create Box`, `Pixel Builder: Extrude`, `Pixel Builder:
      Paint`, `Pixel Builder: Add preset slot` (Scene Undo/Redo lines in the Output); each was
      one version step and redid correctly. Ctrl+Z after clicking the 3D view worked (the paint
      undo above was issued from the viewport focus).
- [x] Log scan: no `ERROR`/`SCRIPT ERROR` from the addon (only the harness-induced
      `debug_placement` argument errors noted at the top).

## Not tested (time-boxed pass)

- Shape variants other than Box (Plane, Cylinder, Cone, Sphere, Stairs, Arch, Ramp, Pipe, hex
  shapes, Torus, Door, Wall, Poly shape, Prism, Sprite, Icosphere) and `Shift+C` append-to-mesh.
- Inset `I`, bevel `Ctrl+B`, edge loop `Ctrl+R`, merge `M`, connect `J`, fill `F`,
  triangulate `Ctrl+T`, duplicate `Ctrl+D`, Delete, grow/shrink `G`/`Shift+G`, loop/ring
  `L`/`Alt+L`, `[`/`]`, `Esc` cancels.
- Vertex paint on geometry, Shift+click flood fill, Ctrl+click color pick.
- Smooth vs Faceted visual comparison; Set Collider / Set Trigger; the W/H/D rebuild
  confirmation after actual vertex edits (only the static Inspector warning was seen).
- UV move/scale/rotate verification and UV persistence after save (the scene was saved before
  the UV drag).
- Export STL/GLB/OBJ/PLY to `res://qa_tmp/` (not attempted; the menu entries exist).
- History dock entries (only the Output "Scene Undo/Redo: Pixel Builder: …" lines were read).
- `Ctrl+Z` with a LineEdit focused, redo-stack clearing by a new edit.
