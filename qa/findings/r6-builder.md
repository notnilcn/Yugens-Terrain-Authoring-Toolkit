# Builder QA (r6 E2) — round-2 verification on the merged build

Session `r6-e2` on display `:72`, port `8872`, Godot **4.7.2**, Linux, Mesa **llvmpipe**,
editor scale 1.0, window content 1600×900 (Xvfb 1920×1080, resized during the pass). Build:
`/home/clintonnguyen/g/code_examples/rrr-wt/r4base` (**HEAD `e50088d`** — the round-2 merge).
Scene `res://demo/hex_island.tscn`, target `BuilderRuins/Ruin` (209 verts / 401 edges / 210
faces, 2 material slots) plus a `Box` created during the pass. Editor log:
`/tmp/opencode/qa/sessions/r6-e2/editor.log` (copies:
[`editor-log-boot.log`](../screenshots/r6-builder/ui/editor-log-boot.log),
[`editor-log-mid.log`](../screenshots/r6-builder/ui/editor-log-mid.log),
[`editor-log-final.log`](../screenshots/r6-builder/ui/editor-log-final.log)). The session was
started once and stopped at the end; it never crashed or restarted (one `Godot Engine v` line,
79 log lines total).

Helpers written for this pass (read-only drivers): `qa/scripts/r6-builder/d.sh` (session
`drive.py` wrapper), `u.py` (visible-Control table), `tree.py` (Control subtree),
`m.py` (builder layout rects), `tabs.py` (strip page tabs), `focus.py` (focus owner),
`w.py` (visible dialogs).

Measurements below are window-content px from `ui --root all`; the 3D viewport is
`@Node3DEditorViewportContainer@10229`, the strip is `PixelBuilderStripRoot`/`PixelBuilderStrip`
(its `MstFlow` content in the second number), the palette is `PixelBuilderPalette`, the rail is
`PixelBuilderToolbarRail` (content `PixelBuilderToolbar`).

---

## Finding — the Materials palette collapse hides the content but keeps its 277 px allocation (N11 regression)

- Severity: **warning** (the documented "collapse to a 24 px tab" does not happen; the 3D view
  gains nothing, and the leftover strip is a large dead area)
- Steps: 1) 1600×900, select `BuilderRuins/Ruin`, palette at its expanded default (277 px wide,
  view 657). 2) Click the palette grip (the chevron at its top-left, x = palette left + 5).
- Expected (N11 claim): the panel collapses to a 24/32 px tab and the 3D view widens to ≈910 px.
- Actual: the content hides and `ExpandTab` (`‹`) appears, `panel_PixelBuilderPalette_collapsed=true`
  is written, but the panel keeps **277 px**: `PixelBuilderPalette [1036,154,277,699]`,
  `ExpandTab [1036,154,277,33]` (the tab is as wide as the panel), the 3D viewport stays
  **657×611**. At 1280×800 the same click leaves the palette at 277 and the view at **340×478**.
  A 1280→1281→1280 resize does not re-flow it.
- Because the panel is a child of the editor's `HSplitContainer` and `MstSidePanel::apply()`
  only changes its `custom_minimum_size`, the split offset is never moved, so the freed minimum
  is never reclaimed. (The `Pixelizer`/Paint panels, which are not in a split container, do
  collapse — this path is builder-only.)
- Evidence: [`42-palette-collapsed.png`](../screenshots/r6-builder/42-palette-collapsed.png)
  (1600: empty 277 px strip + `‹`), [`45-palette-collapse-1280.png`](../screenshots/r6-builder/45-palette-collapse-1280.png)
  (1280: 340 px view beside a 277 px empty strip), `mst_editor_state.cfg`:
  `panel_PixelBuilderPalette_collapsed=true`.
- Undo: n/a.

## Finding — the palette grip cannot narrow the panel below the split allocation (min width 220 is stored but not applied)

- Severity: **minor** (the documented 220–420 px range is one-sided)
- Steps: 1) 1600×900, palette 420 px (grip dragged left; it clamps correctly). 2) Drag the grip
  right by 250 px.
- Expected: the palette narrows to its 220 px minimum and the view grows.
- Actual: `panel_PixelBuilderPalette_width=220.0` is written, but the rendered panel stays at
  **277 px** and the view at 657 (the split offset bounds it; nothing forces the child smaller).
- Evidence: state file after the drag (`..._width=220.0`) vs
  [`41-palette-grip-narrow.png`](../screenshots/r6-builder/41-palette-grip-narrow.png) (palette
  277, view 657).
- Undo: n/a.

## Finding — the editor's split dragger still squeezes the 3D view under 480 px and can widen the palette past 420 (N32 residual)

- Severity: **warning** (layout floor bypassable; the panel's own clamp is not authoritative)
- Steps: 1) 1600×900, palette expanded. 2) Drag the `SplitContainerDragger` at the palette's left
  edge (`[1030,154,8,699]`) far left, twice.
- Expected: the palette clamps at 420 and the 3D view keeps the 480 px floor (the round-2 notes
  claim the floor is enforced).
- Actual: the palette reached **437 px** (view 497×582), then **537 px** (view **397×553**) — both
  outside the 220–420 range and below the 480 px view floor. The panel's own grip and the
  splitter are two different resize paths with different limits.
- Evidence: `m.py` output at those two drags (palette 437/view 497, palette 537/view 397);
  [`40-palette-grip-wide.png`](../screenshots/r6-builder/40-palette-grip-wide.png) is the
  420 px/514 px grip result for comparison.
- Undo: n/a.

## Finding — Ctrl+D still duplicates the mesh *node* when the focus owner is outside the 3D main screen

- Severity: **warning** (N12 residual; the documented builder key silently becomes a scene edit)
- Steps: 1) 1600×900, select `BuilderRuins/Ruin`, Face mode. 2) Click the **History** dock tab
  (focus owner becomes `@TabBar@35`, outside the 3D main screen). 3) Press `Ctrl+D`.
- Expected: per the docs "Ctrl+D — Duplicate faces"; at worst the builder's HUD hint
  `Duplicate: select faces (Face mode)`.
- Actual: Godot's *Duplicate Node(s)* runs: a sibling **`Ruin2`** appears under `BuilderRuin`,
  the scene is marked modified, the Inspector switches to `Ruin2`, History shows
  `Duplicate Node(s)`, the builder closes (HUD back to Object mode). With the 3D view focused
  instead, the key is claimed in every element mode and only the hint appears (see the pass
  table) — the outcome depends purely on who owns the keyboard focus.
- The claim lives in `PixelBuilderPlugin::claim_early_key`, which refuses when
  `main_screen_has_focus` is false (its comment: "with the focus elsewhere (Scene dock), it
  reaches Godot"). That is a deliberate design choice, but the docs do not say it, and a History
  dock click is one keystroke away from a destructive duplicate.
- Evidence: [`20-ctrl-d-focus-history.png`](../screenshots/r6-builder/20-ctrl-d-focus-history.png)
  (Ruin2 + `Duplicate Node(s)`), [`11-edge-ctrl-d.png`](../screenshots/r6-builder/11-edge-ctrl-d.png),
  [`14-edge-ctrl-d-no-sel.png`](../screenshots/r6-builder/14-edge-ctrl-d-no-sel.png).
- Undo: the node duplicate itself undoes correctly (one `Ctrl+Z`); the builder does not reopen.

## Finding — the builder HUD status line is sticky across mode and mesh changes

- Severity: **minor** (misleading status text; the r5 "Editing the … recipe" case is fixed but the
  generic message is not)
- Steps: 1) Face mode, press `Ctrl+D` with no face picked (HUD line 3 becomes
  `Duplicate: select faces (Face mode)`). 2) Switch to Vertex, then Object, then Face, select
  `Box`, run `+ Slot`/undo, click in the viewport. 3) Or: bake the mesh (`Builder ▸ Bake`), undo
  it, select another mesh.
- Expected: line 3 clears (or follows the current mesh/mode) like the "Editing the … recipe"
  hint now does.
- Actual: `Duplicate: select faces (Face mode)` stayed on screen through a Vertex→Object→Face
  round trip, a mesh reselection and several palette edits
  ([`30-f-face-before.png`](../screenshots/r6-builder/30-f-face-before.png)); after a bake test the
  line `Baked Ruin to MeshInstance3D (lightmap UV2 at texel 0.2505)` survived the undo and a
  re-selection of the restored `Ruin` ([`62-1280-clean.png`](../screenshots/r6-builder/62-1280-clean.png),
  [`94-reloaded-1280.png`](../screenshots/r6-builder/94-reloaded-1280.png)).
- Evidence: as above; the message is only replaced by the *next* builder message.
- Undo: n/a.

## Finding — HUD second line is clipped at the viewport edge at ≤1366 px (carried)

- Severity: **ui**
- Steps: select the mesh in Face mode at 1280×800 (view 340 px) or 1366×768 (441 px).
- Expected: `selected: N faces …` readable.
- Actual: `209 verts  401 edges  210 faces | selecte…` is cut at the viewport's right edge
  (`[63-face-pick-1280.png]`, HUD crop `…/30-f-face-before.png`). Line 1 and the badge are fine.
- Evidence: [`63-face-pick-1280.png`](../screenshots/r6-builder/63-face-pick-1280.png),
  [`44-face-strip-1280x800.png`](../screenshots/r6-builder/44-face-strip-1280x800.png).
- Undo: n/a.

## Finding — the thin strip stays at the 175 px cap after a tall section was visited

- Severity: **ui** (new-section tabs work, but the "one section at a time" strip often stays tall)
- Steps: 1) 1280×800, Face mode, click the **Extrude** tab (strip 175, content 307). 2) Click
  **Ops** (62 px of fields) or switch to Edge/Vertex.
- Expected: the strip shrinks to the selected section (62–124 px, as at 1600×900).
- Actual: the strip stays at its 175 px cap (content 209–307 px scrolling) at 1280×800 in
  Edge and Vertex after the Extrude tab was visited; at 1600×900 it correctly shows 62 px
  (Extrude/Ops) and 132 px (Face/Extrude). This is the "pinned strip height" behaviour
  (fc89ca7) rather than a per-section height.
- Evidence: [`46-edge-strip-1280x800.png`](../screenshots/r6-builder/46-edge-strip-1280x800.png),
  [`47-vertex-strip-1280x800.png`](../screenshots/r6-builder/47-vertex-strip-1280x800.png) vs
  [`43-face-strip-1600x900.png`](../screenshots/r6-builder/43-face-strip-1600x900.png).
- Undo: n/a.

---

## Round-2 verification (item → verdict + measurements)

### N12 — Ctrl+D (`Error`) → **PASS for the intended case; one residual** (see finding)

| Case (1600×900) | Result | Evidence |
|---|---|---|
| Face mode, 1 face picked, 3D view focused, `Ctrl+D` | **pass** — HUD 209/401/210 → **213/405/211**, History `Pixel Builder: Duplicate`, no `Ruin2` | `06`, `07`, `08` |
| `Ctrl+Z` after it | **pass** — HUD back to 209/401/210, `selected: 0 faces` (selection cleared as before), History entry stays as redo | `09` |
| Edge mode, 4 edges picked, 3D view focused | **pass** — claimed; HUD hint `Duplicate: select faces (Face mode)`, no node duplicate | `16`, `17` |
| Vertex mode, 3D view focused | **pass** — same hint, no node duplicate | `23`, `24` |
| Face mode, History dock focused (after clicking the History tab) | **fail (residual)** — Godot `Duplicate Node(s)` → `Ruin2`, Inspector → `Ruin2`, builder closes | `20` |
| Object mode (3D view unfocused after a dock click) | reaches Godot → `Ruin2` — **as designed** ("In Object mode … reaches Godot") | `22` |

No `Ruin2` node, no `Duplicate Node(s)` history entry in any of the pass cases.

### N13 — Materials palette undo (`Warning`) → **PASS**

- Select `Ruin` (2 slots) → `+ Slot` → palette rows **3**, History `Pixel Builder: Add slot`,
  Inspector `3 material slots` → `Ctrl+Z` → palette rows **2** (rebuilt: all row node ids
  change), Inspector `2 material slots`.
- `Ctrl+Shift+Z` → 3 rows; `Ctrl+Z` → 2 rows again (follows the data both ways).
- There is no phantom row left to press `+ Preset` on; the palette is fully re-synced.
- Evidence: [`25-palette-before.png`](../screenshots/r6-builder/25-palette-before.png),
  [`26-slot-added.png`](../screenshots/r6-builder/26-slot-added.png),
  [`27-slot-undo.png`](../screenshots/r6-builder/27-slot-undo.png).

### N14 — HUD (`Minor`) → **partly fixed**

- **"Editing the … recipe" hint clears on mesh change: PASS.** Select the created `Box` →
  Inspector `Edit Shape` → HUD line 3 `Editing the Box recipe; change the shape fields to
  rebuild it` → select `Ruin` → line 3 is gone (2 lines only, badge now `Following Ruin`).
  Evidence: [`37-edit-shape-hint.png`](../screenshots/r6-builder/37-edit-shape-hint.png),
  [`38-hint-after-select-ruin.png`](../screenshots/r6-builder/38-hint-after-select-ruin.png).
- **`Following <node>` badge vs HUD line 1: PASS (no overlap).** Badge
  `[386,203,107,33]` (bottom y = 236); HUD line 1 is drawn from y ≈ 231–245, so the badge sits
  just above it (the text is fully legible). Evidence:
  [`34-after-f.png`](../screenshots/r6-builder/34-after-f.png) (badge visible, 11.7 m readout).
- **`F`: it is Godot's frame/follow, not the builder.** In Face mode with no face picked (the
  builder's fill guard refuses), `F` reached Godot: one attempt moved the camera from the close
  framing to an overview (no badge); a later attempt at an 11.7 m framing left the distance at
  **11.7 m** and only drew `Following Ruin`. So `F` does not zoom/fit the mesh; it turns on the
  preview-camera follow. The docs' "frame" wording is still not what happens.
- The generic HUD message line is **sticky** — see the finding above.

### N11 + N32 — Materials palette grip / collapse / view width → **partly fixed** (collapse broken)

| State (1600×900) | Palette | 3D view | Evidence |
|---|---|---|---|
| Expanded default | 277 | **657×611** (Object) | `01`, `02` |
| Grip dragged left 200 px | **420** (clamped) | 514×582 | `40` |
| Grip dragged right (min) | 277 rendered (220 stored) | 657×611 | `41` |
| Split dragger dragged left (bypass) | 537 | **397×553** | `40` family / m.py |
| Grip clicked (collapse) | **277** (content hidden, `ExpandTab` 277 wide) | **657×611** | `42` |

| State (1280×800) | Palette | 3D view |
|---|---|---|
| Expanded | 277 | **340×478** |
| Collapsed | 277 | **340×478** |

- Grip drag range: `PANEL_MAX_WIDTH` (420) is enforced when widening; the 220 px minimum is
  recorded in `mst_editor_state.cfg` but not applied to the split, so narrowing stops at the
  split allocation (277).
- **Collapse to a tab: FAIL** — the `‹` tab is the whole 277 px panel; the view keeps its width.
  The r6 verification note "657→910 px" does not reproduce here at 1600×900 or 1280×800.
- **Can the view still fall below 480 px? YES** — 340 px at 1280×800 and 1024×600, and 397 px
  when the split dragger is used. The palette never auto-collapses at 1280 (the starved branch
  in `MstSidePanel::apply()` did not fire there).
- The builder's 86 px left rail still has no collapse (unchanged).

### N15 — Builder file dialogs (`UI`) → **PASS**

- `Builder ▸ Export OBJ…` → `Export builder mesh (OBJ)`: opens **1050×700**, `windows` reports
  `min [500,300]`, resizable.
- `window-resize 400 200` → **actual [500,300]** (clamped to the reported minimum) and the body
  fits: Path / Favorites / Recent / Directories & Files / File name / Cancel / Save all visible.
- Evidence: [`56-export-obj-dialog.png`](../screenshots/r6-builder/56-export-obj-dialog.png),
  [`57-export-obj-400x200-request.png`](../screenshots/r6-builder/57-export-obj-400x200-request.png).

### Section tabs (round-2) — **PASS**

- One section at a time: `PixelBuilderStripPages` holds `Extrude / Ops / Move / Paint`; the strip
  body shows only the selected section's fields (plus the strip's own Slot/Color/Grid line where
  the section includes it).
- Empty tabs hidden: **Vertex** shows only `Ops / Move / Paint` (`Extrude` `visible=False`);
  Face and Edge show all four at these widths.
- Chosen tab kept across mode switches: chosen `Extrude` in Face → switch to Edge → `Extrude`
  still `toggled=True`; Vertex (no Extrude) falls back to `Ops`; back to Face the `Ops` choice
  survives. Evidence: `tabs.py` output for the four transitions, `43`, `48`, `49`.

| Size | Mode / section | Viewport | Strip root / content | Tabs row |
|---|---|---|---|---|
| 1600×900 | Face / Ops | 657×**634** | 657×**62** / 657×62 | [600,791,214,33] |
| 1600×900 | Face / Extrude | 657×**564** | 657×**132** / 657×132 | [600,721,214,33] |
| 1600×900 | Edge / Extrude | 657×**634** | 657×**62** / 657×62 | [600,791,214,33] |
| 1600×900 | Vertex / Ops (no Extrude) | 657×**572** | 657×**124** / 657×124 | [600,729,157,33] |
| 1280×800 | Face / Ops | 340×**478** | 340×**118** / 326×118 | [376,664,214,33] |
| 1280×800 | Face / Extrude | 340×**421** | 340×**175** / 326×307 | [376,607,214,33] |
| 1280×800 | Edge / Ops | 340×**421** | 340×**175** / 326×209 | [376,607,214,33] |
| 1280×800 | Vertex / Ops | 340×**421** | 340×**175** / 326×209 | [376,607,157,33] |
| 1024×600 | Face / Ops | 340×**297** | 340×**99** / 326×118 | [268,483,214,33] |
| 1024×600 | Edge / Ops | 340×**297** | 340×**99** / 326×209 | [268,483,214,33] |
| 1024×600 | Vertex / Ops | 340×**297** | 340×**99** / 326×209 | [268,483,157,33] |

At 1600×900 the old "whole strip" 175 px is replaced by 62–132 px. At 1280 the 175 px cap is
hit (pinned, see the finding); at 1024 the root is 99 px. The 3D view is unchanged at 340 wide
in every Element mode — the strip no longer takes the width, the palette does.

### Crash watch — **PASS**

Select `BuilderRuins/Ruin` at each size with the pointer parked over the toolbar rail, waited
2 s, captured:

| Size | Result | Notes |
|---|---|---|
| 1024×600 | **survived** | `60-crashwatch-1024x600.png` |
| 1280×720 | **survived** | `60-crashwatch-1280x720.png` |
| 1366×768 | **survived** | `60-crashwatch-1366x768.png` |
| 1600×900 | **survived** | `60-crashwatch-1600x900.png` |
| 1920×1080 | **survived** | `60-crashwatch-1920x1080.png` |

No `bind_mut`, no layout-loop error, no `SCRIPT ERROR`, no `draw_rect`, no auto-restart: one
`Godot Engine v` line and 79 total log lines at the end of the pass. The only log entries are the
known boot noise (2× `ERROR … create_frustum_points`, 32× plane/vector normalize warnings —
N6/N31, unchanged).

---

## Covered

- [x] Session started once (`:72`/`8872`), `env.sh` sourced, stopped at the end; the editor never
      crashed or restarted.
- [x] **N12 Ctrl+D**: face duplicate + undo (209→213→209), Edge/Vertex claimed with the hint,
      dock-focused fall-through to `Duplicate Node(s)` and Object mode reaching Godot.
- [x] **N13 palette undo**: `+ Slot` → 3 rows → `Ctrl+Z` → 2 rows, redo/undo both follow, no
      phantom row.
- [x] **N14 HUD**: Box recipe hint clears on reselect; `Following` badge clears HUD line 1;
      `F` behaviour recorded (frame/follow, no fit); sticky generic message found.
- [x] **N11/N32 palette**: grip widen to the 420 clamp (view 514), grip narrow (stays 277),
      collapse (stays 277, tab 277 wide), splitter bypass (537, view 397), 1280 collapse
      (no reclaim), 1024 view 340.
- [x] **N15 dialogs**: Export OBJ opens 1050×700, min [500,300], 400×200 → 500×300, body fits.
- [x] **Section tabs**: one section at a time, empty tabs hidden, choice survives Face→Edge→
      Vertex→Face, measured at 1600×900, 1280×800, 1024×600.
- [x] **Crash watch**: 5 sizes with the pointer over the toolbar, no crash, log clean.
- [x] **Regression sweep (1280×800 MST, and Modeler layout)**: select mesh; Face pick; **extrude**
      (`E` card → Done → 213/409/214 → `Ctrl+Z` → 209/401/210); **inset** (`I` → Done →
      213/409/214 → `Ctrl+Z` → 209/401/210); **bevel** (`Ctrl+A` + `Ctrl+B` → live 824/1614/808,
      refused on Done with `Bevel: the bevel would make faces overlap. Use a smaller…`, state
      unchanged at 209/401/210); **draw** (`C` footprint drag + click → `Box` 8/12/6, History
      `Pixel Builder: Create Box`); **paint** (swatch + click → History `Pixel Builder: Paint`);
      **Modeler layout** (Builder menu → Modeler layout: toolbar in the menu bar, full-width view
      292–994, back to MST); **save/reload** (`Ctrl+S`, Scene ▸ Reload Saved Scene → `Ruin`
      survives with 209/401/210 and 2 material slots, builder UI reopens). Note: the `Box`
      created earlier was already gone at save time — an extra `Ctrl+Z` pressed during the
      refused-bevel test had undone `Pixel Builder: Create Box` (one step per press, as
      designed); this is not a save/reload loss.
- [x] Builder menu inspected (16 entries; Export OBJ/glTF/STL/PLY/.res, bake, Modeler layout,
      Modeler options).
- [x] Log scan: `ERROR`/`SCRIPT ERROR`/`bind_mut`/`draw_rect`/panic = none beyond the known
      2 `create_frustum_points` boot errors.

## New issues (summary)

1. **Materials palette collapse keeps its 277 px allocation** (warning) — content hides, the
   `‹` tab is 277 px wide, the 3D view does not widen (657/340). N11's "collapse to a tab /
   657→910" does not reproduce on the merged build.
2. **Palette grip minimum is recorded but not applied** (minor) — `width=220` stored, panel
   stays at the split allocation (277).
3. **Split dragger bypasses the palette clamp** (warning) — 537 px palette / 397 px view, both
   outside the intended 220–420 / 480 floor.
4. **Ctrl+D duplicates the node when the focus is outside the 3D main screen** (warning) —
   e.g. after clicking the History dock tab, Face-mode `Ctrl+D` → `Ruin2` + `Duplicate Node(s)`.
   Documented as "Ctrl+D — Duplicate faces" with no focus caveat.
5. **HUD status line is sticky** (minor) — `Duplicate: select faces (Face mode)` / `Baked Ruin to
   MeshInstance3D (…)` survive mode changes, mesh reselection and undo.
6. **HUD line 2 clipped at ≤1366** (ui, carried from r5) — `selected: N faces …` cut at the
   viewport edge with a 340 px view.
7. **Strip pinned at the 175 px cap after a tall section** (ui) — Edge/Vertex at 1280×800 keep
   175 px with 209–307 px scrolling content after the Extrude tab was visited.
8. **Startup `create_frustum_points` ERROR + normalize warnings** (minor, environmental, N6/N31)
   — still present and still not in the README's known-noise list.

## UI feedback for FEEDBACK.md (measured)

| Size / state | 3D view | Strip root / content | Palette | Rail root / content |
|---|---|---|---|---|
| 1600×900 Object | 657×611 | 657×85 / 657×85 | 277 | 86×699 / 72×1076 |
| 1600×900 Face/Ops | 657×634 | 657×62 | 277 | 86×699 / 72×1364 |
| 1600×900 Face/Extrude | 657×564 | 657×132 | 277 | 86×699 / 72×1364 |
| 1600×900 Edge/Extrude | 657×634 | 657×62 | 277 | 86×699 / 72×1084 |
| 1600×900 Vertex/Ops | 657×572 | 657×124 | 277 | 86×699 / 72×1048 |
| 1600×900 palette 420 | 514×582 | 514×114 | 420 | 86×699 |
| 1600×900 palette collapsed | 657×611 | 657×85 | **277** (tab) | 86×699 |
| 1280×800 Face/Ops | 340×478 | 340×118 | 277 | 86×599 / 72×1364 |
| 1280×800 Face/Extrude | 340×421 | 340×175 / 326×307 | 277 | 86×599 / 72×1364 |
| 1280×800 Edge/Vertex | 340×421 | 340×175 / 326×209 | 277 | 86×599 / 72×1084 / 1048 |
| 1280×800 collapsed | 340×478 | 340×118 | **277** (tab) | 86×599 |
| 1280×800 Modeler | 702×572 (from the capture) | — | — | toolbar 2 rows (116…192) |
| 1024×600 Face | 340×297 | 340×99 / 326×118 | 277 | 86×399 / 72×1364 |
| 1024×600 Edge/Vertex | 340×297 | 340×99 / 326×209 | 277 | 86×399 / 72×1084 / 1048 |

1. **Fix the collapse path first.** `MstSidePanel::apply()` must move the parent split (or the
   panel must set its `size_flags` so the split recomputes), otherwise a collapsed builder panel
   is a 277 px empty strip. Same for the 220 px minimum: it must be applied to the allocation,
   not only to `custom_minimum_size`. Until then the only way to reclaim the space is dragging
   the editor's splitter (which also breaks the 420/480 limits).
2. **Make the splitter respect the panel's limits** (or hide the dragger and keep only the grip):
   a 537 px palette and a 397 px view should not be reachable.
3. **Claim `Ctrl+D` for the builder whenever the mesh is edited**, not only while the 3D main
   screen owns the focus — or document the focus caveat and put the key in the 3D view's context
   menu. The current behaviour turns a documented "duplicate faces" into a scene duplicate after
   one History-dock click.
4. **Clear the HUD message line on mode/mesh change** (and on undo), like the recipe hint.
5. **Give the strip a per-section height** (or a hard 132 px cap in Element modes) so the short
   Ops/Move/Paint sections do not inherit the 175 px pinned height at 1280.
6. **Right-align or wrap HUD line 2** below 480 px view width; `selected: …` is the only feedback
   for a face pick and it is the first thing clipped.
7. **The palette is still the space hog at ≤1366**: with the collapse fixed, collapsing at
   1280×800 would give the view 340 → ~585 px; that is the single highest-value layout change
   left in the builder.
