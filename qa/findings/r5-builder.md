# Builder QA (r5) — PixelBuilderMesh, MST + Modeler layouts

Session `r5-t4` on display `:54`, port `8854`, Godot **4.7.2**, Linux, Mesa **llvmpipe**,
editor scale 1.0, window content 1600×900 (Xvfb 1920×1080). Build: `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`
(merged r4-fix tree). Scene `res://demo/hex_island.tscn`, target `BuilderRuins/Ruin`
(209 verts / 401 edges / 210 faces) and a `Box` drawn during the pass. Editor log:
`/tmp/opencode/qa/sessions/r5-t4/editor.log` (copy:
[`ui/editor-log-r5-t4-final.log`](../screenshots/r5-builder/ui/editor-log-r5-t4-final.log)).

Method notes:
- `drive.py undo-state` reads the **root** history; builder edits land in the **Global**
  history, so every undo claim below was read from the **History dock** plus the HUD/Inspector
  counts. Shortcut tests used real X11 keys (`key z --ctrl`, `key 2`, `key e`, …).
- Builder digit/tool keys are consumed only when the 3D view has focus. Clicking empty viewport
  space **deselects** the mesh (documented editor behavior), so mode changes were also driven by
  the `ModeObject`/`ModeFace`/`ModeEdge`/`ModeVertex` toolbar buttons and the readback is the
  strip/viewport rect pair plus a HUD crop.
- One editor at a time; the session survived the whole pass. No `pkill` was used.

## Crash watch — selecting `BuilderRuins/Ruin` at every size

| Size | Result | Notes |
|---|---|---|
| 1920×1080 | **survived** | viewport 1009×851, strip 58 |
| 1600×900 | **survived** | viewport 689×611 (Object) |
| 1366×768 | **survived** (3 attempts) | incl. pointer parked at the r4 crash point `(345,478)` and a terrain→builder→terrain→builder round trip |
| 1280×720 | **survived** | viewport 358×341 (Face) |
| 1024×600 | **survived** | viewport 358×272 (Face) |

No `bind_mut`, `SCRIPT ERROR`, crash, or editor restart occurred. The whole pass added **no**
`draw_rect` warning, so r4 #35 is not present in this build.

## Layout matrix — measured rects (window-content px, editor scale 1.0)

All coordinates are `[x, y, w, h]` from `ui --root all`; the 3D viewport is
`@Node3DEditorViewportContainer@10229`, the strip is `PixelBuilderStripRoot` (ScrollContainer)
and `PixelBuilderStrip` (its `MstFlow` content).

| Size | Mode | Viewport | Strip root | Strip content | Palette | Toolbar rail (content) |
|---|---|---|---|---|---|---|
| 1600×900 | Object / Select | 376,154,689,611 | 376,768,689,85 | 689×85 | 1068,154,245,699 | 287,154,86,699 (72×1076) |
| 1600×900 | Face | 376,154,689,**521** | 376,678,689,**175** | 675×**194** | 1068,154,245,699 | 287,154,86,699 (72×1364) |
| 1600×900 | Edge | 376,154,689,521 | 376,678,689,175 | 675×**186** | 1068,154,245,699 | 287,154,86,699 (72×1084) |
| 1600×900 | Vertex | 376,154,689,**539** | 376,696,689,**157** | 689×157 | 1068,154,245,699 | 287,154,86,699 (72×1048) |
| 1280×800 | Face | 376,154,**358×421** | 376,578,358,175 | 344×**428** | 737,154,259,599 | 287,154,86,599 (72×1364) |
| 1280×800 | Edge | 376,154,358×421 | 376,578,358,175 | 344×**358** | 737,154,259,599 | 287,154,86,599 (72×1084) |
| 1280×800 | Vertex | 376,154,358×421 | 376,578,358,175 | 344×**329** | 737,154,259,599 | 287,154,86,599 (72×1048) |
| 1280×720 | Face | 376,154,358×341 | 376,498,358,175 | 344×428 | 737,154,259,519 | 287,154,86,519 (72×1364) |
| 1366×768 | Face | 376,154,**441×389** | 376,546,441,175 | 427×**337** | 820,154,259,567 | 287,154,86,567 (72×1364) |
| 1024×600 | Object | 268,154,358×**297** | 268,454,358,**99** | 344×170 | 629,154,259,399 | 179,154,86,399 (72×1076) |
| 1024×600 | Face | 268,154,358×**272** | 268,429,358,**124** | 344×**428** | 629,154,259,399 | 179,154,86,399 (72×1364) |

**Face/Edge/Vertex strip vs viewport (the requested measurement):**

- **1600×900:** viewport **689×521**, strip **689×175** (content 675×194 Face, 675×186 Edge;
  Vertex 157 and no scrollbar). The strip is capped and scrolls; the viewport keeps 58–74 % of the
  height. **The r4 "447 px strip / 39 px viewport" regression is gone.**
- **1280×800:** viewport **358×421**, strip **358×175** (content 344×428 Face / 358 Edge /
  329 Vertex, so Face needs a 253 px internal scroll). Strip bottom is 753, window 800 → **on
  window**; it does not exceed the window at this size.
- For comparison, at **1280×720** the same Face strip is still 175 tall (bottom 673, window 720)
  and at **1024×600** it is 124 tall (bottom 553, window 600). The builder strip never leaves
  the window; it caps and scrolls.

## Finding — `Ctrl+D` duplicates the mesh *node* instead of the selected faces

- Severity: **error** (documented shortcut performs a different, scene-changing action)
- Steps: 1) Select `BuilderRuin/Ruin`, press `2` (Face mode). 2) Click a face (HUD
  `selected: 1 face`). 3) Press `Ctrl+D`.
- Expected: `docs/10` / `docs/12`: "Ctrl+D — Duplicate faces" (one `Pixel Builder: Duplicate`
  undo step).
- Actual: Godot's own *Duplicate Node(s)* wins: a sibling **`Ruin2`** appears under
  `BuilderRuin`, the scene becomes modified (`hex_island(*)`), the Inspector switches to `Ruin2`,
  and the History dock shows **`Duplicate Node(s)`**. The builder's Face selection is lost.
  `Ctrl+Z` removes `Ruin2` but leaves **no node selected** (builder UI closes).
- Evidence: `qa/screenshots/r5-builder/42-duplicate.png` (tree `Ruin2`, History
  `Duplicate Node(s)`), `43-after-dup-undo.png`.
- Undo: the node duplicate itself undoes correctly.

## Finding — Materials palette keeps an undone slot row (stale after `Add slot` undo)

- Severity: **warning** (the panel and the data disagree; the next click acts on a phantom slot)
- Steps: 1) Select `Ruin` (2 material slots). 2) Click **+ Slot** → 3 rows, History
  `Pixel Builder: Add slot`, Inspector `3 material slots`. 3) `Ctrl+Z`.
- Expected: the palette drops back to 2 rows.
- Actual: the Inspector returns to **2 material slots**, but the palette still shows **three
  rows** (row 2 = blue sphere). Clicking **+ Preset** on that phantom row and choosing *Checker*
  re-creates slot 2: Inspector `3 material slots`, History `Pixel Builder: Add preset slot`.
  The panel is not rebuilt on the undo path (the HUD counts are fixed, the palette is not).
- Evidence: `60-slot-added.png`, `62-slot-undo.png` (3 rows + Inspector "2 material slots"),
  `64-preset-checker.png` (clicking the phantom row adds the slot back).
- Undo: data undo works; the palette view does not follow.

## Finding — "Editing the Box recipe…" HUD hint leaks onto another mesh

- Severity: **minor** (misleading status line)
- Steps: 1) Select `Box` → Inspector **Edit Shape** (HUD "Editing the Box recipe; change the
  shape fields to rebuild"). 2) Change W (History `Pixel Builder: Shape parameters`). 3) Select
  `BuilderRuin/Ruin`.
- Expected: the HUD tool detail follows the newly selected mesh (normal Object hint).
- Actual: the Ruin HUD reads `209 verts 401 edges 210 faces` but the third line still says
  **`Editing the Box recipe; change the shape fields to rebuild`**, while the strip shows the
  normal Object fields (no W/H/D), i.e. the shape-edit session is over.
- Evidence: `qa/screenshots/r5-builder/109-F-after-1366x768.png`.
- Undo: not applicable.

## Finding — `F` frame/follow is inconsistent on a small mesh

- Severity: **minor** (documented "frame" key does not visibly frame)
- Steps: 1) Face mode, no faces selected, camera at the default overview. 2) Press `F`.
- Expected: the camera frames the mesh (docs/10 lists `F` as frame; the build now leaves `F`
  free when no fill can run).
- Actual: the HUD shows a **`Following Ruin`** badge, but the camera distance/position does not
  change (the ruin stays a ~40 px object at 34.5 m). In a later Object-mode attempt `F` produced
  no badge and no camera change at all. Wheel zoom works.
- Evidence: `20-frame-F-face-1600x900.png` (before) / `21-frame-F-after-viewport-click-1600x900.png`
  (badge only), `109-F-after-1366x768.png`.
- Undo: not applicable (camera only).

## Finding — `Following Ruin` badge overlaps the builder HUD first line

- Severity: **ui**
- Steps: select a builder mesh in Face mode, press `F` (or click the mesh after follow is on).
- Expected: the follow indicator and the HUD do not collide.
- Actual: the badge is drawn at the HUD's top-left, over `Pixel Builder - Face mode, Select / Move`
  (`21-frame-F-after-viewport-click-1600x900.png`, crop `23b-hud-face-picked.png`). The HUD's
  second line is also clipped at the viewport's right edge at ≤1366 px.
- Evidence: `23b-hud-face-picked.png`, `109-F-after-1366x768.png`.
- Undo: n/a.

## Finding — Builder viewport is only 358 px wide at 1280 and 1024 (below the 480×300 floor)

- Severity: **warning** (practical editing room at the sizes the pass was asked about)
- Steps: 1) 1280×800, Face mode. 2) Measure viewport/strip/palette. 3) Repeat at 1024×600.
- Expected: the 3D view keeps the QA floor (480×300) or the palette/rail collapses.
- Actual: at **1280×800** the viewport is **358×421** (rail 86 + palette 259 + editor docks leave
  358 px); at **1024×600** it is **358×272** (height below 300 too). The strip and the toolbar
  rail scroll internally (`Face` content 428 in a 175/124 root; toolbar content 1364 in 399);
  the Materials palette is 259 px and cannot collapse. At 1024 the Inspector dock is also
  half off-window, so `Export & Data` (Set Collider / Export Native Mesh…) needs scrolling in a
  panel that is itself clipped. The face-strip content is *fully reachable* (scrollbars exist),
  so this is room, not clipping.
- Evidence: `08-mode-face-1280x800.png`, `09-mode-edge-1280x800.png`,
  `10-mode-vertex-1280x800.png`, `104-face-1024x600.png`, `14-select-1024x600.png`.
- Undo: n/a.

## Finding — Export OBJ dialog reports min 200×70 but clamps to 498×200 and clips its body

- Severity: **minor** (carried r4 #40; builder export path)
- Steps: select `Box`, Builder menu → **Export OBJ…**, then `window-resize 400 200 --title "Export builder mesh (OBJ)"`.
- Expected: a 400×200 dialog (its reported minimum is 200×70) or an honest minimum.
- Actual: clamps to **498×200**; at that size the Favorites/Recent/File rows and the file list are
  cut, only Path/File/Cancel/Save remain usable.
- Evidence: `96-export-dialog-400x200.png`; `windows` → `min [200,70]`, actual `[498,200]`.
- Undo: n/a.

## Finding — startup `create_frustum_points` ERROR + normalize warnings (environmental)

- Severity: **minor** (log noise; every session on this machine, not builder-specific)
- Steps: boot a session and read `editor.log` before touching the scene.
- Actual: 2× `ERROR: Condition "!res" is true. Returning: false at: create_frustum_points
  (servers/rendering/rendering_light_culler.cpp:682)` plus 16×
  `WARNING: Plane cannot be normalized…` / `Vector3 cannot be normalized…` in pairs. The same
  counts appear in the other r5 sessions (`r5-t1/t2/t3/t5/smoke`), so it is not caused by the
  builder; it is simply not in the README's known-noise list.
- Evidence: `ui/editor-log-before-crashwatch.log` lines 14–62; `editor-log-r5-t4-final.log`.

## Covered

- [x] Selected `BuilderRuins/Ruin`; builder UI appears (Builder menu, left Mode/Tool rail, bottom
      strip, right Materials palette) at 1600×900 — `01-builder-selected-1600x900.png`.
- [x] **Modes** via toolbar: Object/Face/Edge/Vertex with strip/viewport readback — `05/06/07`
      (1600), `08/09/10` (1280×800), `99/102/104/110` (720/600/1366).
- [x] **Digit keys** `1`–`4` and **`H`** now switch/cycle modes (r4 #9 fixed) — HUD crops
      `18-hud-after-key2.png` ("Face mode"), `19-hud-after-H.png` ("Edge mode").
- [x] **`E` extrude** works from the keyboard with a face selected: HUD 209→217 verts,
      Adjust Extrude card (Dist 0.99 / Face normal / Cancel / Done), Done commits — `24-key-E-extrude.png`.
- [x] **Undo/redo refresh the HUD and clear the stale selection** (r4 #13 fixed): extrude
      Done→217, `Ctrl+Z`→209 + `selected: 0 faces`, `Ctrl+Shift+Z`→217 — `25/27/29/30`.
- [x] **Inset** `I`: Adjust Inset card (fraction 0.2, Individual), Done commits to 217/417/218,
      History `Pixel Builder: Inset`, `Ctrl+Z`→209 — `31/32/33`.
- [x] **Bevel** `Ctrl+B` (Edge mode, 8-edge loop selected): Adjust Bevel card, live HUD
      217/415/216, Done commits, History `Pixel Builder: Bevel`, `Ctrl+Z`→209 — `35/37/38b/39/41`.
- [x] **Triangulate** `Ctrl+T`: 209/401/210 → 209/406/215 (6 faces selected), History
      `Pixel Builder: Triangulate faces`, undo restores — `49/50/51`.
- [x] **Delete** `Delete`: removes the selected face (209 faces, node kept), History
      `Pixel Builder: Delete`; undo restores — `46/47/48`.
- [x] **Duplicate** `Ctrl+D` — **broken** (see finding): duplicates the node (`Ruin2`).
- [x] **Create Box** `C` + footprint drag + height + click: new sibling `Box` under
      `BuilderRuin`, HUD `Created Box` as a status line (no centre card — r4 #34 fixed),
      History `Pixel Builder: Create Box`; `Ctrl+Z` removes, `Ctrl+Shift+Z` restores — `71/72/73/74/75/76`.
- [x] **Append** `Shift+click` a stamped shape onto the selected mesh: `Box` 8→16 verts,
      `nodes PixelBuilderMesh` still `['Box','Ruin']`, History `Pixel Builder: Add shape`;
      undo restores 8 verts — `83/84`, `88`.
- [x] **Shift+C** opens the **Bezier** tool (HUD `Object mode, Bezier`), so the docs/shortcut
      conflict of r4 #42 is resolved; append is the Shift+click gesture — `81/82`.
- [x] **Esc** cancels the pending Bezier plane; no stray action.
- [x] **Paint** `B`: Paint tool HUD, swatch selectable, click records `Pixel Builder: Paint`;
      a red tint appears in the before/after crop — `52/53/54`. Undo/redo of that action left the
      mesh counts unchanged (paint is colour-only) — `58/59`.
- [x] **Materials** `+ Slot` (History `Pixel Builder: Add slot`), `+ Preset` popup
      (Grid/Checker/No Draw/Collider/Trigger) and a Checker slot
      (History `Pixel Builder: Add preset slot`) — `60/63/64/65`.
- [x] **Smooth / Faceted** buttons clickable (`1100,459` / `1183,459`); no undo entry was created
      by the toggle (not verified as undoable) — `66/68`.
- [x] **Edit Shape** + `W/H/D`: Inspector "Rebuilding replaces the element edits. Undo can
      restore them."; changing W is a silent rebuild (no confirm dialog) recorded as
      `Pixel Builder: Shape parameters` — `88/89/90/91`.
- [x] **Builder menu** (16 entries incl. UV editor, Preferences, Bake, Export OBJ/glTF/STL/PLY/res,
      Create reusable part, Generate lightmap UVs, Modeler layout, Modeler options) — popup dump.
- [x] **Modeler layout** both sizes: 1600×900 toolbar in the menu bar, full-width viewport,
      bottom-centre tool card 300×147 at `650,690`; 1280×800 toolbar wraps to 2 rows
      (296×70), card at `491,590` (bottom 737, window 800), Inspector section 251 wide —
      `85-modeler-1600x900.png`, `86-modeler-1280x800.png`; switched back to MST — `87`.
- [x] **UV editor** via Inspector `Open UV Editor`: window 820×620, min 420×320, resizable,
      Faces/Verts, Manual/Auto/Planar/Box/Spherical, Fit/Sew/Island/Frame, Auto UVs + Export UV
      Template, UV canvas, hints — `97/98`.
- [x] **Export OBJ**: dialog `Export builder mesh (OBJ)` opens; Save/Cancel reachable — `96`.
- [x] **Persistence**: `Ctrl+S`, Scene → *Reload Saved Scene* → `Box` survives with 8 verts and
      3 material slots; History resets to `The Beginning` but keeps stale pre-reload action
      labels (Godot Global-history behaviour; `Ctrl+Z` then does nothing) — `93/94/95`.
- [x] Log scan: no `draw_rect`, `bind_mut`, `SCRIPT ERROR`, panic or restart; only the
      environmental frustum/normalize lines above.

## Not tested (time-boxed pass)

- Select/move (`W` + gizmo drag) and its undo; edge loop `Ctrl+R`; merge `M`; connect `J`;
  fill `F`; grow/shrink `G`/`Shift+G`; select loop/ring `L`/`Alt+L`; knife `K`; place `P`;
  vertex paint `5`; soft selection `O`; rotate `R`/`Alt+R`; cut/paste; reusable part.
- Paint Shift+click fill / Ctrl+click pick / vertex-colour channels; material drag-onto-face;
  collider/trigger toggles.
- Export save round-trip (dialog opened only), Bake, Generate lightmap UVs.
- UV canvas editing (move/scale/rotate) and stitching.
- `[`/`]` grid-step shortcut (wheel over the Grid field did not change it either; shortcut focus
  path not isolated).

## UI feedback for FEEDBACK.md (measured)

| Size | Viewport | Strip (root / content) | Palette fixed | Toolbar rail (root / content) |
|---|---|---|---|---|
| 1600×900 Face | 689×521 | 689×175 / 675×194 | 245 | 86×699 / 72×1364 |
| 1280×800 Face | 358×421 | 358×175 / 344×428 | 259 | 86×599 / 72×1364 |
| 1366×768 Face | 441×389 | 441×175 / 427×337 | 259 | 86×567 / 72×1364 |
| 1024×600 Face | 358×272 | 358×124 / 344×428 | 259 | 86×399 / 72×1364 |

1. **The 3D view is the squeezed surface at ≤1366 px.** The builder spends 86 px on the tool
   rail and 245–259 px on the Materials palette before the editor docks (Scene ≈ 287 px,
   Inspector ≈ 300 px) — 358 px of viewport at 1280 and 1024. Collapse the palette to an
   overlay/icon strip below ~1500 px width (or add a splitter and remember 180 px).
2. **The strip cap works (175 px, scrolls) but the content is 3.4× taller at 1024.** Face mode
   needs 428 px of content in 124 px. Group the fields by section (Shape / UV / Inset / Extrude /
   Select) like the Modeler layout already does, or allow a 2-row cap and a "More" popover.
3. **The HUD is clipped and collides.** Line 2 (`selected: …`) is cut at the viewport's right
   edge at 1366×768 and below, and the `Following <node>` badge is painted over line 1. Right-align
   the badge or reserve a HUD top margin.
4. **The Materials palette is fixed and its undo path is stale.** No splitter/collapse; after
   `Ctrl+Z` of `Add slot` the row stays (see finding). Rebuild the palette from the data on every
   undo/redo (the HUD already does).
5. **Documented `Ctrl+D` never reaches the builder.** It is the only documented editing key in the
   set that is shadowed by a Godot editor shortcut. Rebind the default (e.g. `Shift+D`) or claim
   it in the 3D viewport input before the editor handler; keep `Delete`, which works.
6. **1024×600 is below the editor's comfortable width.** The right Inspector is half off-window
   and `Export & Data` / `Set Collider` are only reachable by scrolling a clipped panel. Consider
   a builder “compact” palette (hide the slot spheres, keep the names) under 1200 px.
7. **Modeler layout scales better than MST** — at 1280×800 the toolbar wraps cleanly to 2 rows and
   the bottom-centre tool card (300×147) does not cover the model. Worth making the default for
   windows ≤1366 px wide.

## r4 issues — status in this build

| r4 # | Issue | Status |
|---|---|---|
| 2 | **Critical** builder `bind_mut` panic at 1366×768 with the pointer over the toolbar | **Not reproduced** — 3 attempts at 1366×768 (incl. `(345,478)` and a terrain↔builder round trip) plus one select each at 1024×600, 1280×720, 1600×900, 1920×1080 |
| 7 / 8 | Unbounded attribute strip; builder Face strip 447 px / viewport 345×39 | **Fixed for the builder** at ≥720 px height (strip 175, viewport 521/421/389); at **1024×600** the viewport is 358×272, still under the 480×300 floor |
| 9 | Documented mode/tool keys dead or hijacked (`1`–`4`, `H` hides the node, `E`/`F`) | **Mostly fixed:** `1`–`4`, `H`, `E` now work; `F` is inconsistent (finding); `Ctrl+D` is hijacked (new) |
| 13 | Undo restores data but leaves visible UI stale (builder HUD counts/selection) | **HUD fixed** (counts + selection refresh, verified on extrude/inset/bevel/triangulate/delete); **Materials palette still stale** after `Add slot` undo (new finding) |
| 34 | Modeler "Created Box" toast covers the viewport | **Fixed** — "Created Box" is a HUD status line; only tool/error cards are drawn (`73-box-created.png`, `85/86`) |
| 35 | `draw_rect()` warning spam from the builder UI | **Not present** — zero `draw_rect` lines in the log after the whole pass |
| 40 | File dialog reports an inaccurate minimum | **Still reproduces** — builder Export OBJ: `min [200,70]`, clamps to 498×200, body clipped (`96`) |
| 42 | Shift+C opens Bezier instead of the documented append | **Resolved** — docs now say Shift+C = Bezier and Shift+click = append; both verified (`81/82`, `83/84`) |

## New issues (summary)

1. **Ctrl+D duplicates the mesh node instead of faces** (error) — documented "Duplicate faces"
   is shadowed by Godot's *Duplicate Node(s)*.
2. **Materials palette keeps an undone slot row** (warning) — stale after `Add slot` undo;
   a later `+ Preset` on the phantom row re-creates the slot.
3. **"Editing the Box recipe…" HUD hint leaks onto another mesh** (minor).
4. **`F` frame/follow is inconsistent; no visible reframe** (minor).
5. **`Following <node>` badge overlaps the builder HUD** (ui).
6. **Builder viewport 358 px wide at 1280/1024** and 358×272 at 1024×600 (warning, layout).
7. **Startup `create_frustum_points` ERROR + normalize warnings** (minor, environmental; add to
   the known-noise list).
