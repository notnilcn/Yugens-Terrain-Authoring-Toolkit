# Builder QA (r4) — PixelBuilderMesh, modes, strip, edit tools, undo

Session `r4-builder` on display `:36`, port `8816` (Godot 4.7.2, llvmpipe). Scene
`res://demo/hex_island.tscn`, target `HexIsland/BuilderRuin/Ruin` (209 verts / 401 edges /
210 faces). A `Box` was drawn during the pass and is still in the session copy; nothing under
`addons/` or the repository scenes was modified. Editor log:
`/tmp/opencode/qa/sessions/r4-builder/editor.log`.

Method note: `drive.py undo-state` reads the root object's history, while builder edits are pushed
to the **Global** history. `undo-state` therefore stayed empty for builder edits even when they
were undoable; the proof below is the mesh state in the Inspector/HUD plus the Output lines
`Global Undo/Redo: Pixel Builder: …`. Ctrl+Z / Ctrl+Shift+Z were always sent as real keys.

## Digit keys 1–4 never switch the builder mode; H hides the selected node instead of cycling modes

- Severity: error
- Steps: 1) Select `BuilderRuin/Ruin`, click the viewport, confirm `Object` mode. 2) Press `2`,
   wait 1 s, screenshot. 3) Press `H`, wait, screenshot. 4) Switch modes only via the
   `ModeObject`/`ModeFace`/`ModeEdge`/`ModeVertex` toolbar buttons.
- Expected: `1`/`2`/`3`/`4` = Object/Face/Edge/Vertex and `H` cycles modes (docs/10, and the
   button tooltips say `Face mode [2]` etc.).
- Actual: `2` leaves the HUD at `Pixel Builder - Object mode, Select / Move`; the mode does not
  change. `H` is consumed by Godot's 3D-view **Toggle Selected Node(s) Visibility** shortcut: the
  mode stays Object, the Ruin disappears from the viewport, and the undo history gains
  `Toggle Selected Node(s) Visibility`. Only the toolbar buttons change mode. `E` (extrude) is
  likewise consumed by the built-in rotate-mode shortcut: with 1 face selected, `E` created no
  undo action and did not extrude. `F` did not frame the selection either. This is the same
  shortcut-collision class for digits/H/W/E/F/R/G.
- Evidence: qa/screenshots/r4-builder/02-mode-key2.png (still Object after `2`);
  qa/screenshots/r4-builder/03-mode-keyH.png (still Object after `H`);
  qa/screenshots/r4-builder/11-visibility-undone.png (Ruin restored by undo of the H action,
  Output `Scene Undo: Toggle Selected Node(s) Visibility`);
  qa/screenshots/r4-builder/07-keyF-framing.png (camera unchanged after `F`).
- Undo: works for the accidental H action (one Ctrl+Z restores the hidden mesh).

## Face-mode strip squeezes the viewport to 39 px at 1600×900; strip is not scrollable

- Severity: error
- Steps: 1) At 1600×900 with the MST layout, select the builder mesh. 2) Click `ModeFace`.
   3) Measure `Node3DEditorViewportContainer` and `PixelBuilderStrip` with `ui`.
- Expected: a usable 3D view for face picking; the strip should scroll, collapse or wrap.
- Actual: viewport container `[706,154,345,39]` (345×39) while `PixelBuilderStrip` is
  `[706,196,345,447]` with minimum `238×447`. The face-operation fields (Color, Grid, Dist,
  Inset, Profile, Subdiv, Trim, T, Off, Space, Grow, Extrude) fill the whole panel; the island is a
  39 px sliver and the HUD status lines are clipped. Face selection is only practical after
  switching to the Modeler layout. The other modes are also cramped at 1600×900:
  Object viewport 421×310 (strip 421×156), Edge/Vertex viewport 385×163 (strip 385×303,
  min 197×303).
- Evidence: qa/screenshots/r4-builder/04-mode-face-button.png,
  qa/screenshots/r4-builder/25-mst-layout-face.png,
  qa/screenshots/r4-builder/32-mode-edge.png,
  qa/screenshots/r4-builder/33-mode-vertex.png;
  qa/screenshots/r4-builder/ui/01-builder-controls.json (strip/viewport rects).
- Undo: not applicable.

## Undo/redo changes the mesh but the builder HUD keeps stale vertex/face counts

- Severity: warning
- Steps: 1) Face mode, select one face. 2) Extrude (`Actions → Extrude Faces`, Enter to commit);
   mesh becomes 213 verts / 409 edges / 214 faces. 3) `Ctrl+Z`. 4) Read the HUD and the Inspector.
   5) Repeat with Inset (`I`, Enter, `Ctrl+Z`).
- Expected: after undo the HUD reports the reverted geometry (and the selection should be
   refreshed/cleared if it no longer exists).
- Actual: after the extrude undo the Inspector is back to 209/401/210 but the HUD still reads
  `213 verts 409 edges 214 faces | selected: 1 face`. After the inset undo the Inspector is
  213/409/214 while the HUD still reads `217 verts 417 edges 218 faces | selected: 1 face`.
  Redo refreshes the HUD, so only the undo path is stale. A user can run the next operation against
  a selection that no longer exists.
- Evidence: qa/screenshots/r4-builder/17-extrude-undo.png (Inspector 209, HUD 213),
  qa/screenshots/r4-builder/18-extrude-redo.png (both 213),
  qa/screenshots/r4-builder/22-inset-undo.png (Inspector 213, HUD 217).
- Undo: works (geometry and Output action name correct); HUD refresh is broken.

## Shift+C opens the Bezier tool, not “append to the selected mesh” (docs promise)

- Severity: minor (documentation/shortcut conflict, blocks the documented append workflow)
- Steps: 1) Select the existing `Box` child. 2) Press `Shift+C` and drag on the ground.
- Expected (docs/10 warning): a new shape is appended to the currently selected mesh, with no new
  sibling.
- Actual: the HUD switches to `Pixel Builder - Object mode, Bezier` and the `Bezier Plane` panel
  opens (`2 knots … Create`). No new `PixelBuilderMesh` sibling is created, but the operation is a
  Bezier plane, not a Box append. `shortcuts.rs` binds `Shift+C` to `tool_bezier`, so the
  documented append gesture is unreachable from this key. The pending plane was canceled.
- Evidence: qa/screenshots/r4-builder/38-shiftC-append.png (HUD “Bezier”, Bezier Plane panel),
  `nodes PixelBuilderMesh` still lists only `Box` and `Ruin`.
- Undo: not applicable (pending plane canceled); append itself not verified.

## Modeler-layout “Created Box” toast covers the viewport

- Severity: ui
- Steps: 1) Builder menu → Modeler layout. 2) Draw a Box (or resize to 1280×800 after drawing).
   3) Look at the centre of the 3D view.
- Expected: status messages stay in the top-left HUD; the model stays clickable.
- Actual: a centred `Modeler / Created Box / Dismiss` card overlays the model and intercepts clicks
  until dismissed. It appears after creating the Box and again at 1280×800.
- Evidence: qa/screenshots/r4-builder/40-builder-1280x800.png.
- Undo: not applicable.

## New `draw_rect` warning spam from the builder UI

- Severity: warning
- Steps: 1) Use the builder (select mesh, switch layout, enter Paint); 2) scan the editor log.
- Expected: no engine warnings from the builder UI.
- Actual: three identical warnings were appended:
  `WARNING: The draw_rect() "width" argument has no effect when "filled" is "true".`
  `at: draw_rect (scene/main/canvas_item.cpp:925)` (editor.log lines 14–19, three pairs). The exact
  gesture was not isolated (they first appeared around the layout/paint pass); they are not in the
  known-startup-noise list.
- Evidence: `/tmp/opencode/qa/sessions/r4-builder/editor.log` lines 14–19;
  qa/screenshots/r4-builder/28-paint-applied.png (Output panel showing the warnings).
- Undo: not applicable.

## Covered

- [x] Selected `BuilderRuin/Ruin`; builder UI appears: HUD label, **Builder** menu, bottom strip,
      right **Materials** panel — 01-builder-ui-selected.png, 25-mst-layout-face.png.
- [x] **Builder** menu opened; items (UV editor, Preferences, Bake, Export OBJ/glTF/STL/PLY/Godot
      mesh, reusable part, lightmap UVs, Modeler layout/options) — 05-builder-menu.png.
- [x] Modeler layout toggled both ways (06-modeler-layout-face.png, 27/29) — viewport becomes
      full width; the strip and Materials panel disappear in this layout.
- [x] Modes Object/Face/Edge/Vertex via toolbar buttons; HUD and strip change
      (04/25/32/33); digits and H fail (see finding).
- [x] Shape = Box, default W/H/D; `C`, drag footprint, move up, click created a new sibling **Box**
      under `BuilderRuin` (34/35); one `Ctrl+Z` removed it (36, nodes list only `Ruin`), redo
      restored it (37, `Box`+`Ruin`). Output: `Pixel Builder: Create Box`, Scene Undo/Redo.
- [x] Face pick works (HUD `selected: 1 face`, 13-face-selected.png); selection outline visible.
- [x] Extrude via `Actions → Extrude Faces` and adjust panel (`Done`/Enter): 209→213 verts,
      210→214 faces; Ctrl+Z→209, redo→213 (14/15/16/17/18).
- [x] Inset `I` (keyboard works): adjust panel, 213→217 verts/214→218 faces; Enter commit; Ctrl+Z
      back to 213/214 (20/21/22).
- [x] Paint tool `B` opens the Paint panel (23, 29); color swatch selectable in the MST strip
      (26). A face-paint click while the panel was open committed no paint undo action
      (28/30/31) — not verified as a paint defect; see Not tested.
- [x] Visibility H accidentally tested: hides the selected Ruin and is undoable (03/11).
- [x] UV editor `U` opens the **Mesh UV** panel / “Pixel Builder - UV Editor” window with
      Faces/Verts, Manual/Auto/Planar/Box/Spherical, Fit/Sew/Island/Frame, Auto UVs template,
      Export UV Template and a UV canvas (39). No UV transform verified.
- [x] Log scan: no `ERROR` / `SCRIPT ERROR`; three new `draw_rect` warnings (above); known
      `V-Sync`/`.uid` startup noise only.

## Not tested (time-boxed pass)

- `W`/`E` select-move and extrude by shortcut (E is consumed by the built-in rotate mode);
  `Ctrl+B` bevel, `Ctrl+R` edge loop, `M` merge, `J` connect, `F` fill, `Ctrl+T` triangulate,
  `Ctrl+D` duplicate, `Delete`, `G`/`Shift+G`, `L`/`Alt+L`, `[`/`]`, `Esc` tool cancel.
- Merge/connect/fill/triangulate/duplicate/delete undo; W/H/D rebuild confirmation after vertex
  edits; smooth/faceted visual comparison; collision Set Collider/Set Trigger.
- Paint on a visible face (panel is modal and covered the target; no `Pixel Builder: Paint` action
  was produced), Shift+click fill, Ctrl+click pick, vertex paint `5`, material drag-to-face.
- Materials `+ Slot` / `+ Preset` and their undo; Materials panel collapse/resize.
- Shift+C append (opened Bezier instead), Bezier `Create`.
- UV move/scale/rotate; export STL/GLB/OBJ/PLY/res; save + reload persistence.
- 1280×800 builder strip measurement (a UV editor window and an accidental PixelSky selection
  invalidated the first attempt; 40 is a Modeler-layout capture only).

## Action × undo matrix

| Action | Gesture used | Changed? | Ctrl+Z | Ctrl+Shift+Z | Action name / note |
|---|---|---|---|---|---|
| Create Box | Shape tool `C` + drag/click | yes, new sibling | works | works | `Pixel Builder: Create Box` (Scene history) |
| Extrude 1 face | Actions menu + Enter | 209→213 verts | works (Inspector 209) | works (213) | `Pixel Builder: Extrude`; HUD stale after undo |
| Inset 1 face | `I` + Enter | 213→217 verts | works (213) | not retried | `Pixel Builder: Inset`; HUD stale after undo |
| Toggle visibility (accidental H) | `H` | mesh hidden | works | not retried | `Toggle Selected Node(s) Visibility` |
| Bezier plane | `Shift+C` + drag | pending only | n/a | n/a | canceled; docs’ append gesture conflicts |
| Paint face | `B` + click + Done | not verified | n/a | n/a | no paint action in Output |
| Move / Bevel / Loop / Merge / Connect / Fill / Triangulate / Duplicate / Delete / Material / W-H-D rebuild | not run | — | — | — | see Not tested; keyboard path blocked by 3D-editor shortcut collisions |
