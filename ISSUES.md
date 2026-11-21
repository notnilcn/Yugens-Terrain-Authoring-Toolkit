# MarchingSquaresTerrain — Issues

Round 3 (r4) pass, 2026-11-21. Godot **4.7.2**, Linux, Mesa **llvmpipe** on isolated Xvfb virtual
displays, editor scale 1.0, real X11 mouse/keyboard input through the repo's QA harness
(`qa/drive.py`). The editor's `--headless` mode uses the dummy renderer and **cannot render or
screenshot the UI**, so every pass used Xvfb (headless-capable, real rendering) instead. Nine
area passes plus a fresh-session verification pass drove 591 PNGs and 60 UI dumps into
`qa/screenshots/r4-*`; the per-area reports are in `qa/findings/r4-*.md`.

**Undo headline:** `Ctrl+Z` works for **35 of 42** tested action/context rows, including every
core terrain tool and a 19-chunk stroke. Five rows are broken and two were not exercised. The
failures are enumerated in [Undo / redo — verified working](#undo--redo--verified-working) and in
issues 5, 6, 11, 12, 13, 14, 19.

**Coverage:** 9 areas (sculpt, paint, manage, pixel, effects, builder, UI resizability, deep undo,
runtime) plus verification. Prior-round reports (`qa/findings/*.md`, `r2-*`, `r3-*`) were consulted
as hints; every current claim below was re-driven unless marked carried over.

Legend: **Critical** = crash/editor exit; **Error** = a documented control does the wrong thing or
nothing; **Warning** = works but is misleading, destructive, unreachable or not undoable;
**Minor** = edge case/polish; **UI** = layout, wording, discoverability.

## Summary

| # | Sev | Area | Issue |
|---|---|---|---|
| 1 | **Critical** | Pixelizer | Selecting `PixelRenderer` at 1280×720 crashes the editor with a container layout loop (verified fresh) |
| 2 | **Critical** | Builder | Selecting `PixelBuilderMesh` at 1366×768 with the pointer over the builder toolbar panics `bind_mut` and exits the editor |
| 3 | Error | Scene lifecycle | Closing/reopening a scene with a terrain tool active floods 196–287 `Node3D::upcast_ref … after it has been freed` errors |
| 4 | Error | Pixelizer | `PixelizerEditorPlugin::on_float` panics with a re-entrant borrow during a strip rebuild (editor survives) |
| 5 | Error | Settings | **Floor Slot** and **Storage Mode** dropdowns change only the label (property stays 0, no undo) — verified fresh |
| 6 | Error | Settings | **Debug View** dropdown changes only the label (`debug_view` stays 0, no undo) — verified fresh |
| 7 | Error | Layout | Attribute strips have no height cap: the 3D viewport collapses to 39 px at 1024/1280 (39 px is not practically editable) |
| 8 | Error | Builder | Face/Edge/Vertex strip is 447 px tall at 1600×900; viewport 345×39 |
| 9 | Error | Shortcuts | Documented tool/mode keys are dead or hijacked: sculpt `1`–`9`; builder `1`–`4`, `H` hides the node, `E`/`F` go to the 3D editor |
| 10 | Warning | Sliders | Many sliders/spins accept far out-of-range values (Slope 6702°, Pixel Size 39087, Coverage 325, Swell 108.9, firefly count 4096) |
| 11 | Warning | Undo | PixelRenderer pixel-size integer drag creates **two** undo steps |
| 12 | Warning | Undo | Grass Animation FPS is not undoable; the next Ctrl+Z reverts an unrelated terrain edit |
| 13 | Warning | Undo/UI | Undo restores data but leaves visible UI stale (texture preset/name, Grid Type label, builder HUD counts/selection) |
| 14 | Warning | Pixel | Undo of a lighting preset restores properties but leaves the preview tinted until the scene/session is rebuilt |
| 15 | Warning | Settings | Grid Type change silently deletes all 295 chunks (no confirmation; Ctrl+Z recovers) |
| 16 | Warning/UI | Panels | Side panels are hard-fixed at 250 px, with no splitter/collapse; Paint content is 5,767 px (90 wheel notches) and Foliage is not a Control |
| 17 | Warning | Toolbar | Terrain toolbar column count changes with height (1/2/3 columns), stealing 116–416 px; compact mode is 76/116 px, not 36 |
| 18 | Warning | Settings/Pixel | Settings Grid page grows the strip to 350–385 px; Settings and Renderer page-tab rows jump vertically |
| 19 | Warning | Feature switches | Project feature switches are not undoable; Ctrl+Z stays dead until the viewport is clicked |
| 20 | Warning | Sun shafts | Added SunShafts has a null Sun; “Wire refs” reports 0 references |
| 21 | Warning | Fireflies | Fireflies render nothing at night even at Count 400 / Size 0.5 / Night Only 0 |
| 22 | Warning | Sculpt | Shift/Ctrl-wheel change Size/Strength but the strip labels and slider handles stay stale |
| 23 | Warning | Runtime | Camera orbit can pass under the island; the terrain underside fills the screen |
| 24 | UI | Pixel World | Panel is off-screen at 1024×600; the Lighting Preset row sits below the panel viewport |
| 25 | UI | Settings | Clicking the 3D view while Settings is open deselects the terrain and closes the whole tool |
| 26 | Minor | Sculpt | Esc clears the selection but the HUD keeps “N selected” until the next mouse move |
| 27 | Minor | History | Ambiguous undo names: “chunk merge mode” for single and apply-all; tint erase is called “tint paint”; Smooth Terraced/Sloped share a name |
| 28 | UI | Dialogs | Save Preset has no Cancel, cannot shrink below 558 px, and forgets its size |
| 29 | Minor | Paint | Paint Walls hint says “Vertex Painting” and reports a floor height |
| 30 | Minor | Paint | Empty preset name closes the dialog silently and writes nothing |
| 31 | UI | Paint | `mst_noise.png` ground-texture preview renders solid black |
| 32 | Minor | Pixel | Pixel Preview / Animate have no visible pressed state |
| 33 | UI | Pixel World | “+ Add” selects the new node and closes the pixelizer UI |
| 34 | UI | Builder | Modeler “Created Box” toast covers the viewport at 1280×800 |
| 35 | Warning | Builder | New `draw_rect() "width" argument has no effect …` warning spam from the builder UI |
| 36 | Warning | Effects | Rain is invisible at the default overview distance; PixelSmoke’s default plume is invisible; Sky Coverage can change 0 viewport pixels |
| 37 | Minor | Effects/Runtime | Dawn preset washes water/terrain pink; noon on the sandy plateau is nearly white |
| 38 | Minor | Runtime | Finite water plane edge is obvious at low orbit/zoom-out |
| 39 | UI | Runtime | F3 tuner is fixed 420 px and clips at 1024×600; F1 help is one small, unbacked line |
| 40 | Minor | Dialogs | File dialog reports `min [200,70]` but clamps 400×200 to 498×200 |
| 41 | Minor | Sculpt | Grid Aligned looks enabled but silently refuses to toggle while Falloff is on |
| 42 | Minor | Builder | Shift+C opens the Bezier tool instead of the documented “append to selected mesh” |
| 43 | UI | Sculpt | Status/hint line is clipped at the right edge; the Level hint advertises a Strength control that does not exist |

---

## Critical

### 1. Selecting PixelRenderer at 1280×720 crashes the editor

- **Severity:** Critical (process exit; unsaved work at risk)
- **Steps:** 1) Open `res://tutorial/02_island.tscn` or `demo/hex_island.tscn`. 2) Set the editor
  window to **1280×720**. 3) `select --path PixelRenderer`.
- **Expected:** the Pixelizer UI opens (it does at 1366, where its strip fits its 670 px minimum).
- **Actual:** the selection returns once, then the harness socket drops and the editor exits.
  `editor.log` line 14 is
  `Failed method: Container::_sort_children. Message queue out of memory. …` followed by a
  392,253-line flood of `Object was deleted while awaiting a callback.` The Pixelizer strip's hard
  670 px minimum exceeds the available width at 1280, so the container loop never converges.
- **Evidence:** fresh-session reproduction in `qa/screenshots/r4-verify/03-pixelizer-crash-excerpt.txt`
  (saved before the editor died); last working size `qa/screenshots/r4-uiresize/43-1366-pixelizer.png`;
  `qa/findings/r4-uiresize.md` F2. The same crash class is F1 in that report.
- **Undo:** not applicable (process died).

### 2. Selecting a PixelBuilderMesh panics the builder plugin and exits the editor

- **Severity:** Critical (process exit)
- **Steps:** 1) At **1366×768**, leave the pointer at `(345,478)` — the area where the Builder
  toolbar appears. 2) `select --path BuilderRuins/Ruin`.
- **Expected:** the Builder UI opens, as it does at 1920.
- **Actual:** two
  `Gd<T>::bind_mut() failed, already bound; T = … PixelBuilderPlugin` panics from
  `PixelBuilderPlugin::on_button_hover` fired by `mouse_exited`, then
  `Failed method: Container::_sort_children. Message queue out of memory.` and the editor is gone.
  Moving the pointer into the 3D view before selecting avoided it in the next session.
- **Evidence:** quoted from `editor.log` in `qa/findings/r4-uiresize.md` F1; the log was overwritten
  by the automatic restart before it could be copied. Two fresh 1366×768 attempts — including a
  pointer at `(345,478)` and a terrain→builder→terrain→builder round trip — did **not** reproduce
  it, so the trigger is hover/timing dependent. It is reported as seen by the UI pass, not as a
  deterministic crash. The same re-entrant-borrow class is issue 4.
- **Undo:** not applicable (process died).

---

## Errors

### 3. Closing/reopening a scene floods use-after-free errors

- **Severity:** Error (editor survives; Output and error badge flooded)
- **Steps:** 1) Open `demo/hex_island.tscn`, select the terrain, use a terrain tool, `Ctrl+S`.
  2) Scene → **Close Scene**. 3) Reopen the scene. 4) Count
  `upcast_ref: access to instance` in `editor.log`.
- **Expected:** the plugin releases the terrain reference cleanly and reattaches on reopen.
- **Actual:** identical Rust panics:
  `Node3D::upcast_ref: access to instance with ID … after it has been freed` from `godot-core 0.5.5`.
  The sculpt pass measured 196 errors (26 on the first close) and the undo pass 287 (73 → 202 on
  close, 202 → 287 after reopen/close). The first close had 26 errors against a baseline of 0.
  The editor stays alive and the reopened scene works, but the Output is unusable.
- **Evidence:** `qa/screenshots/r4-sculpt/50-close-scene-error-spam.png`,
  `qa/screenshots/r4-undodeep/58-history-dock-pixel.png`, `qa/screenshots/r4-undodeep/61-history-dock.png`;
  `/tmp/opencode/qa/sessions/r4-sculpt/editor.log` lines 14, 92.
- **Undo:** not applicable (scene lifecycle); history is correctly empty after reopen.

### 4. `PixelizerEditorPlugin::on_float` re-entrant borrow panic

- **Severity:** Error (5 ERROR lines; editor survives, UI may rebuild incorrectly)
- **Steps:** 1) Resize the editor and select `PixelRenderer` so the Pixelizer strip rebuilds
  (intermittent). 2) Read Output.
- **Expected:** the strip rebuild completes without a Rust panic.
- **Actual:**
  `ERROR: [panic … godot-core-0.5.5/src/storage/mod.rs:38]` and
  `ERROR: Error calling from signal 'value_changed' to callable: 'PixelizerEditorPlugin::on_float': .`
  (5 occurrences). The UI recovers on the next rebuild, but this is the same re-entrant
  `bind_mut` family as the two crashes.
- **Evidence:** `/tmp/opencode/qa/sessions/r4-pixel/editor.log` lines 14–20;
  `qa/screenshots/r4-pixel/73-resize-1024-renderer.png`.
- **Undo:** not applicable.

### 5. Floor Slot and Storage Mode dropdowns write nothing

- **Severity:** Error (lost setting; verified fresh)
- **Steps:** 1) Settings → **New Chunks** → Floor Slot `1. Grass` → `2. Meadow`.
  2) `prop default_floor_slot`, `undo-state`. 3) Settings → **Storage** → Storage Mode
  `Resource` → `Baked`; `prop storage_mode`.
- **Expected:** the property is written through the undo manager and the dependent Storage rows
  (Data Directory / Save Data) appear.
- **Actual:** the label reads `2. Meadow` / `Baked`, but `default_floor_slot` stays `0`,
  `storage_mode` stays `0`, no undo action is created, and Save Data never appears. This is the
  same integer-item-id conversion failure reported in the 2026-11-20 pass; **not fixed**. Grid Type
  is the exception — it now writes.
- **Evidence:** `qa/screenshots/r4-verify/01-floorslot-label-meadow-prop-0.png`;
  `qa/screenshots/r4-manage/17-newchunks-floorslot-meadow.png`,
  `qa/screenshots/r4-manage/37-storage-mode-baked-select.png`,
  `qa/screenshots/r4-undodeep/49-floorslot.png`; UI dumps `ui/17-floorslot.json`,
  `ui/37-storage-baked.json`.
- **Undo:** broken — nothing is recorded.

### 6. Debug View dropdown writes nothing

- **Severity:** Error (lost setting; verified fresh)
- **Steps:** 1) Settings → **Display** → Debug View `Off` → `Slope`. 2) `prop debug_view`,
  `undo-state`, viewport.
- **Expected:** `debug_view` becomes `1`, the viewport switches to slope shading, and
  `terrain setting debug view` appears in history.
- **Actual:** the field reads `Slope`, but `debug_view` stays `0`, the viewport is unchanged, and
  no undo action is recorded. The setting is lost on the next strip rebuild. (The prior pass's
  side effect of toggling the grid overlay did **not** reproduce.)
- **Evidence:** `qa/screenshots/r4-verify/02-debugview-label-slope-prop-0.png`;
  `qa/screenshots/r4-manage/30-debugview-popup.png`,
  `qa/screenshots/r4-manage/31-debugview-slope-result.png`,
  `qa/screenshots/r4-undodeep/51-debugview-slope.png`.
- **Undo:** broken — nothing is recorded.

### 7. Attribute strips are uncapped; the 3D view collapses to 39 px

- **Severity:** Error (editing becomes impossible at small sizes)
- **Steps:** 1) Select the terrain. 2) Resize 1600 → 1366 → 1280 → 1024. 3) Switch
  Brush/Paint/Settings Grid and measure the viewport/strip.
- **Expected:** the strip caps at ~3 rows (105 px) or scrolls internally, keeping the 3D view at
  least 480×300.
- **Actual:** the strip takes its full flow height and the VSplit gives the remainder to the
  viewport: Brush 553×124 (1366), 470×41 (1280), 470×39 (1024); Paint 300×39, 292×39, 377×39;
  Settings Grid 470×39 with a 385 px strip. At 1024 the content is fully visible (no clipped
  labels), so this is unbounded growth, not wrapping. A Brush drag in the 39 px slice produced no
  terrain change and `undo_count: 0`.
- **Evidence:** `qa/screenshots/r4-uiresize/39-1366-brush.png`,
  `41-1366-paint.png`, `46-1280-brush.png`, `47-1280-paint.png`, `52-1024-brush.png`,
  `54-1024-paint.png`, `55-1024-settings-grid.png`; measured matrix in
  `qa/findings/r4-uiresize.md`.
- **Undo:** n/a (layout); the same drag at 1366 works and undoes.

### 8. Builder Face/Edge/Vertex strip is 447 px tall at 1600×900

- **Severity:** Error (viewport 345×39; face picking impractical)
- **Steps:** 1) 1600×900, select `BuilderRuins/Ruin`. 2) Click **Face mode**. 3) Measure
  `Node3DEditorViewportContainer` and `PixelBuilderStrip`.
- **Expected:** a grouped/scrolling strip ≤240 px, viewport ≥480×300.
- **Actual:** viewport **345×39**, strip **345×447** (min 238×447, not scrollable), Builder
  toolbar 416×489. At 1920 the same mode is usable (view 795×502, strip 177). Object mode is
  421×310; Edge/Vertex are 385×163.
- **Evidence:** `qa/screenshots/r4-builder/04-mode-face-button.png`,
  `qa/screenshots/r4-builder/25-mst-layout-face.png`, `…/32-mode-edge.png`,
  `…/33-mode-vertex.png`, `qa/screenshots/r4-uiresize/37-1600-builder-face.png`;
  `qa/screenshots/r4-builder/ui/01-builder-controls.json`.
- **Undo:** n/a.

### 9. Documented tool/mode shortcuts are dead or hijacked

- **Severity:** Error (documented workflows blocked)
- **Steps:** 1) Terrain selected: press `2`, then `1`; read `ToolTitle`. 2) Builder selected:
  press `2`, then `H`, then `E`/`F`.
- **Expected:** digits switch tools/modes; `H` cycles builder modes; `E` extrudes; `F` frames.
- **Actual:** sculpt digits never change the tool (the 3D view consumes them). Builder `2` leaves
  Object mode; `H` triggers Godot's *Toggle Selected Node(s) Visibility* and hides the Ruin (undo
  restores it); `E` is consumed by the built-in rotate mode; `F` does not frame. `Shift+C` opens
  the Bezier tool instead of appending (issue 42).
- **Evidence:** `qa/screenshots/r4-sculpt/37-key2-after.png`;
  `qa/screenshots/r4-builder/02-mode-key2.png`, `…/03-mode-keyH.png`,
  `…/11-visibility-undone.png`, `…/07-keyF-framing.png`.
- **Undo:** the accidental H action undoes correctly; the intended actions never happen.

---

## Warnings

### 10. Sliders and spins accept absurd out-of-range values

- **Severity:** Warning (silent no-ops or broken visuals)
- **Steps / actual:** drag a little in each field:
  Paint **Slope Min** → `6702.5°` (painting then silently no-ops);
  PixelRenderer **Pixel Size** → `39087`; **Darken** → `283.3`; **Ray Alpha** → `357.5`;
  PixelSky **Coverage** → `325.76`; PixelWater **Swell Height** → `108.93` (water mesh spikes);
  Fireflies **Count** → `4096` (then nothing renders).
- **Expected:** documented ranges clamp (slope 0–90; coverage/alpha 0–1; sensible counts).
- **Evidence:** `qa/screenshots/r4-paint/09-slope-min-drag-overflow.png`,
  `qa/screenshots/r4-pixel/47-pixelsize-drag.png`, `…/51-darken-drag.png`,
  `qa/screenshots/r4-effects/67-sky-coverage-out-of-range.png`,
  `…/32-water-swell-drag.png`.
- **Undo:** the value changes are undoable; the out-of-range value itself is the defect.

### 11. One PixelRenderer pixel-size drag creates two undo steps

- **Severity:** Warning (undo granularity)
- **Steps:** Select `PixelRenderer`, drag the integer Pixel Size slider once, then Ctrl+Z twice.
- **Expected:** one drag = one `Set Renderer Pixel Size` step.
- **Actual:** two `Set Renderer Pixel Size` steps. The first Ctrl+Z lands on an intermediate drag
  value (12972 in one run, 148 in another), not the pre-drag `3`/`2`; the second returns to the
  start. One continuous gesture should be one historical action.
- **Evidence:** `qa/screenshots/r4-pixel/47-pixelsize-drag.png`,
  `qa/screenshots/r4-pixel/48-pixelsize-undo1.png`,
  `qa/screenshots/r4-undodeep/57-pixelsize-drag.png`;
  `qa/findings/r4-undodeep.md` row 42.
- **Undo:** broken granularity (both steps do undo correctly).

### 12. Grass Animation FPS is not undoable

- **Severity:** Warning (silent history loss; Ctrl+Z hits an unrelated edit)
- **Steps:** Settings → Grass → Animation FPS `0` → `12`, Enter. `undo-state`, Ctrl+Z.
- **Expected:** `terrain grass animation fps` restores 0.
- **Actual:** no action/version change; the field keeps 12; the next Ctrl+Z reverts the previous
  terrain action instead. This is the only real non-undoable setting found in the deep pass.
- **Evidence:** `qa/screenshots/r4-manage/22-grass-fps-changed.png`,
  `…/23-grass-fps-after-undo.png`; `qa/findings/r4-undodeep.md` row 34.
- **Undo:** broken.

### 13. Undo restores data but leaves visible UI stale

- **Severity:** Warning (data is correct; the UI lies)
- **Steps / actual:** 1) Apply a texture preset, Ctrl+Z: `tex_assign` is restored, but the
  Material dropdown still says `2. Texture 2` and slot labels keep the preset name.
  2) Rename a slot, Ctrl+Z: data reverts, label stays `undotest`. 3) Undo Grid Type: chunks return
  but the strip still says `Square` while `grid_type = 3`. 4) Builder Extrude/Inset undo: mesh
  counts revert but the HUD keeps 213/409/214 or 217/417/218 and a stale selected face.
- **Expected:** undo refreshes the owning panel/HUD in the same action.
- **Evidence:** `qa/screenshots/r4-paint/54-preset-undo-stale-names.png`,
  `qa/screenshots/r4-manage/60-gridtype-undo.png` (restored chunks, stale label),
  `qa/screenshots/r4-builder/17-extrude-undo.png`, `…/22-inset-undo.png`.
- **Undo:** data works; UI refresh broken.

### 14. Lighting-preset undo leaves the preview tinted

- **Severity:** Warning (visual state survives undo)
- **Steps:** PixelRenderer → Lighting Preset **Dawn** (or Weather **Storm**), then Ctrl+Z.
- **Expected:** the tint and sky revert with the properties.
- **Actual:** properties revert, but the viewport stays salmon/pink (`218,150,153`); toggling
  Preview off/on does not clear it; only a scene/session restart does. Storm has the same class of
  stale tint.
- **Evidence:** `qa/screenshots/r4-pixel/10-lighting-dawn.png`,
  `…/11-dawn-undone.png`, `…/12-dawn-stale-after-preview-toggle.png`;
  `qa/screenshots/r4-effects/19-preset-dawn.png`.
- **Undo:** broken preview restore; property undo itself works.

### 15. Grid Type change silently deletes every chunk

- **Severity:** Warning (destructive, no confirmation; undo recovers)
- **Steps:** `demo/hex_island.tscn` (295 chunks) → Settings → Grid → Grid Type `Hex Rings` → `Square`.
- **Expected:** a confirmation that chunk data is incompatible, or a refusal while chunks exist.
- **Actual:** `grid_type` changes 3→0, `terrain` immediately returns `0 chunks / digest 0`, the
  island disappears, and the Output has no explanation. One Ctrl+Z restores 295 chunks and digest
  `3064656926`.
- **Evidence:** `qa/screenshots/r4-manage/58-settings-grid-page.png`, `…/59-gridtype-square.png`,
  `…/60-gridtype-undo.png`.
- **Undo:** works.

### 16. Side panels are fixed at 250 px and mostly unscrollable

- **Severity:** Warning / UI (unreachable actions, wasted space)
- **Steps:** Paint at 1920×1080; look for a splitter/collapse; wheel to the bottom; repeat for
  Foliage, Pixel World, Builder Materials.
- **Expected:** a splitter (220–420 px), a collapse tab, and a scrollbar that reaches the action
  row in a few notches.
- **Actual:** Paint panel `250×682`, content **5,767 px**; only ~1.5–2 of 16 slots visible; **90
  wheel notches** to reach Export Texture Preset. Foliage is 250 px and its palette is not in the
  editor Control dump at all, so it cannot be tabbed to or scrolled like a normal panel. Pixel
  World is 250×682 (content 941); Builder Materials 245–259 px (content 659 in 469).
- **Evidence:** `qa/screenshots/r4-paint/ui/02-paint-panel.json`,
  `qa/screenshots/r4-paint/02-paint-tool-panel.png`, `…/52-panel-splitter-drag-left.png`,
  `qa/screenshots/r4-uiresize/06-1920-paint-strip.png`, `…/19-1920-paint-panel-bottom.png`.
- **Undo:** n/a.

### 17. Toolbar column count jumps with editor height

- **Severity:** Warning (tool positions move while working)
- **Steps:** compare 1920/1600/1366/1280/1024 expanded and compact.
- **Expected:** one fixed icon rail (30–36 px) with vertical scrolling.
- **Actual:** 1920 expanded 116 px (1 column); 1600/1366/1280 236 px (2 columns); 1024 compact
  started at 76×392 (2 columns) and later became 116×252 (3 columns) as the strip grew. At 1024
  the compact rail is still 116 px, not 36. At 1600 the expanded toolbar takes 15% of the width,
  which is why the Paint strip wraps to 534 px / 6 rows.
- **Evidence:** `qa/screenshots/r4-uiresize/02-1920-toolbar-collapsed.png`,
  `…/31-1600-brush.png`, `…/52-1024-brush.png`, `…/53-1024-toolbar-collapsed.png`.
- **Undo:** n/a.

### 18. Settings Grid grows the strip; Settings and Renderer tab rows jump

- **Severity:** Warning (muscle-memory clicks miss; view collapses)
- **Steps:** Settings → Grid / New Chunks / Grass / Display / LOD & Nature / Storage; record tab y.
  Then select `PixelRenderer` and switch Renderer → Outlines → Rays; record tab y.
- **Expected:** stable tab rows; page content scrolls inside a capped strip.
- **Actual:** Settings tab row at y≈287 (first open) then 448/483/518; settings strip 350 px at
  1024 and 385 px at 1280 for Grid, leaving a 39 px view. The Renderer page tabs jump y=485
  (Renderer/Color/Sky), 450 (Outlines), 520 (Rays); clicking the previous position misses. At one
  point `click-text "Grid"` hit the toolbar Grid overlay button instead of the Settings tab (9
  matches). The tool title is vertically centered instead of top-left, wasting the top-left cell.
- **Evidence:** `qa/screenshots/r4-manage/15-settings-open.png` … `…/35-settings-storage.png`;
  `qa/screenshots/r4-pixel/34-renderer-page.png` (tabs y≈485), plus the Outlines/Rays captures;
  `qa/screenshots/r4-uiresize/09-1920-settings-grid.png` … `…/56-1024-settings-storage.png`.
- **Undo:** n/a.

### 19. Feature switches are not undoable; Ctrl+Z stays dead until a viewport click

- **Severity:** Warning (documented toggles leave no history)
- **Steps:** Pixel World → Features; toggle a project feature off/on; press Ctrl+Z.
- **Expected:** an undo entry or at least a live shortcut.
- **Actual:** no undo entry is created; the first Ctrl+Z after closing the Features popup does
  nothing until the 3D viewport is clicked, then the shortcut works again.
- **Evidence:** `qa/findings/r4-effects.md` “Feature switches…”; `qa/findings/r4-pixel.md`
  feature-switch row.
- **Undo:** not recorded.

### 20. Added SunShafts has a null Sun; Wire refs reports 0

- **Severity:** Warning (node added in a broken state)
- **Steps:** Pixel World → Add → Sun Shafts. Read Inspector; click “Wire Sun / Env Refs”.
- **Expected:** the scene's Sun is wired and the node produces shafts.
- **Actual:** `prop sun → null`, Inspector shows “Assign…” and a warning triangle; Output says
  `Pixel World: wired 0 reference(s).`
- **Evidence:** `qa/screenshots/r4-effects/52-sunshafts-added.png`,
  `qa/screenshots/r4-pixel/14-sunshafts-added.png`.
- **Undo:** Add-node undo works, but the incorrect wiring remains.

### 21. Fireflies render nothing at night

- **Severity:** Warning (effect absent)
- **Steps:** `demo/hex_island.tscn`, night time; Fireflies Count 100→400, Size 0.5, Night Only 0.
- **Expected:** visible points at night.
- **Actual:** three screenshots (including a 3× brightened crop) show an empty scene; no firefly
  pixels at any tested count/size. The node exists and the section responds.
- **Evidence:** `qa/screenshots/r4-effects/39-fireflies-noon.png`,
  `…/40-fireflies-night.png`, `…/41-fireflies-night-big.png`,
  `…/42-fireflies-nightonly0.png`, `…/43-fireflies-crop-bright.png`.
- **Undo:** Count changes undo correctly; the visual output is the defect.

### 22. Wheel size/strength changes leave the strip stale

- **Severity:** Warning (misleading values)
- **Steps:** Brush → Raise/Lower; strip shows Size 15.0 / Strength 4.000. Over the 3D view:
  Shift+wheel ×3, Ctrl+wheel ×2, Alt+wheel ×3.
- **Expected:** labels and slider handles update.
- **Actual:** internal values become Size 19.0 / Strength 3.2565 / Rotation 45; Rotation refreshes,
  but Size and Strength labels stay 15.0 / 4.000 and the slider handles do not move until the strip
  is rebuilt (`18.9 / 3.303` after a tool switch).
- **Evidence:** `qa/screenshots/r4-sculpt/28-rl-wheel-values.png`,
  `…/29-rl-wheel-footprint.png`, `ui/28-after-wheel.json`, `…/42-brush-square-footprint.png`.
- **Undo:** n/a (tool settings).

### 23. Runtime orbit goes under the island

- **Severity:** Warning (view unusable until recovered)
- **Steps:** `demo/hex_island.tscn` 1280×720; middle-drag `500,300 → 500,470`, then
  `500,450 → 500,230`.
- **Expected:** pitch/zoom clamps or camera collision keep the camera above terrain and outside
  props.
- **Actual:** the camera passes below the island; two huge gray underside slabs fill the top of the
  frame, the player shows through a rock as a blue wireframe, and the water is a flat field.
  Orbiting back recovers.
- **Evidence:** `qa/screenshots/r4-runtime/37-hex-noon-normal.png`,
  `…/28-hex-orbit-down.png`, `…/38-hex-noon-recovered.png`.
- **Undo:** not applicable (runtime has no undo).

---

## UI

### 24. Pixel World panel is off-screen or clipped at small sizes

- **Severity:** UI (controls unreachable)
- **Steps:** 1024×600, select `PixelRenderer` (or open Pixel World); also inspect the Lighting
  Preset row at 1600.
- **Expected:** panel fits the window and the action row is reachable without blind scrolling.
- **Actual:** at 1024 the 250 px panel is placed at x≈968 (only 56 px visible), and the panel
  content is taller than its viewport. The Lighting Preset row sits at y=848–917 below the panel
  viewport and is reachable only by wheel-scrolling. (The 1280 attempt crashed — issue 1.)
- **Evidence:** `qa/screenshots/r4-pixel/73-resize-1024-renderer.png`,
  `qa/screenshots/r4-effects/17-pixel-world-scrolled-presets.png`.
- **Undo:** n/a.

### 25. Clicking the 3D view while Settings is open closes the tool

- **Severity:** UI (workflow trap)
- **Steps:** terrain selected, Settings open (any page); LMB-click sky or terrain in the 3D view.
- **Expected:** the tool stays selected (it has no viewport action) or at least the selection is
  retained.
- **Actual:** selection becomes empty, Inspector empties, toolbar and Settings strip disappear.
  Re-selecting lands on the last Settings page. Happened twice.
- **Evidence:** `qa/screenshots/r4-manage/28-settings-current.png`; `info.selection: []`.
- **Undo:** n/a (no edit).

### 26. Esc leaves a stale “N selected” HUD

- **Severity:** Minor
- **Steps:** Brush → Drag, paint a selection (`175 selected`), press Esc once, wait 2.5 s.
- **Expected:** the status returns to `LMB: select cells`.
- **Actual:** the HUD still reads `175 selected | LMB-drag: move selection` until the next mouse
  move; the cells themselves are gone (next drag starts a new selection).
- **Evidence:** `qa/screenshots/r4-sculpt/11-status-after-selection.png`,
  `…/12-status-after-esc.png`, `…/14-status-after-esc-then-drag.png`.
- **Undo:** n/a.

### 27. Ambiguous undo names in History

- **Severity:** Minor (hard to audit history)
- **Steps:** Chunks: change Merge Mode, then Apply Mode to All; Tint: LMB paint then RMB erase;
  Smooth: Terraced then Sloped.
- **Expected:** distinct names (`apply chunk merge mode to all`, `tint erase`, …).
- **Actual:** both chunk operations are `chunk merge mode`; tint erase is `terrain tint paint`;
  both Smooth modes are `terrain smooth`. Undo/redo still works.
- **Evidence:** `qa/screenshots/r4-manage/13-chunk-apply-all.png`,
  `…/14-chunk-undo-all-restored.png`; `qa/findings/r4-undodeep.md` rows 9/24/28.
- **Undo:** works.

### 28. Save Preset dialog has no Cancel / cannot shrink / forgets size

- **Severity:** UI
- **Steps:** Paint → Export Texture Preset; request 400×150, then 800×220; Esc; reopen.
- **Expected:** a compact 400–450 px dialog with OK and Cancel; remember the last size.
- **Actual:** opens 558×150; a 400×150 request clamps to 558×150 although the window reports
  `min [0,0]`; 800×220 is accepted with blank space; reopen returns to 558×150. Only OK exists
  (Esc/X closes). The empty-name case closes silently (issue 30).
- **Evidence:** `qa/screenshots/r4-paint/14-export-preset-dialog.png`;
  `qa/screenshots/r4-uiresize/20-1920-save-preset-dialog.png`,
  `…/21-1920-save-preset-narrow-request.png`, `…/22-1920-save-preset-large.png`.
- **Undo:** n/a.

### 29. Paint Walls hint is wrong

- **Severity:** Minor
- **Steps:** select Paint with Paint Walls on; read the hint.
- **Expected:** a wall-paint hint and the actual wall height.
- **Actual:** it still says `Vertex Painting | height …` and reports a floor height.
- **Evidence:** `qa/screenshots/r4-paint/07-paint-walls-on.png`,
  `…/08-paint-walls-stroke.png`.
- **Undo:** paint itself works.

### 30. Empty preset name closes silently

- **Severity:** Minor
- **Steps:** Export Texture Preset → clear the name → OK.
- **Expected:** validation message or disabled OK.
- **Actual:** the dialog closes, nothing is written, no feedback.
- **Evidence:** `qa/screenshots/r4-paint/18-preset-empty-name.png`,
  `…/19-preset-empty-submitted.png`.
- **Undo:** n/a.

### 31. `mst_noise.png` preview renders black

- **Severity:** UI
- **Steps:** Paint panel, scroll to the slots that use `mst_noise.png`.
- **Expected:** a visible noise-thumbnail preview.
- **Actual:** the preview is solid black; the material still paints.
- **Evidence:** `qa/screenshots/r4-paint/12-preset-mst-classic.png` (slot 1/2 preview),
  `qa/screenshots/r4-paint/53-preset-applied-before-undo.png`.
- **Undo:** n/a.

### 32. Pixel Preview / Animate have no pressed state

- **Severity:** Minor (can't tell whether the mode is on)
- **Steps:** click Pixel Preview, then Animate, in the 3D view menu bar.
- **Expected:** a toggled/pressed visual state.
- **Actual:** only the leading icon changes; no button stays pressed. The effect does toggle
  correctly.
- **Evidence:** `qa/screenshots/r4-pixel/03-preview-on-before.png`,
  `…/04-preview-off.png`, `ui/preview-off.json`.
- **Undo:** n/a.

### 33. Pixel World “+ Add” closes the pixelizer UI

- **Severity:** UI
- **Steps:** Pixel World → `+ Add` a missing effect.
- **Expected:** the panel stays open so the new node can be configured.
- **Actual:** the new node is selected and the pixelizer UI closes; the user must reopen it.
- **Evidence:** `qa/screenshots/r4-pixel/14-sunshafts-added.png`;
  `qa/findings/r4-pixel.md`.
- **Undo:** Add is undoable; the UI close is not.

### 34. Builder “Created Box” toast covers the viewport

- **Severity:** UI
- **Steps:** Modeler layout at 1280×800; draw a box.
- **Expected:** a toast outside the work area or a shorter duration.
- **Actual:** the “Created Box” toast covers the viewport while the user is placing the next box.
- **Evidence:** `qa/screenshots/r4-builder/40-builder-1280x800.png`.
- **Undo:** n/a.

### 35. `draw_rect` warning spam from the builder UI

- **Severity:** Warning
- **Steps:** select the builder mesh and use the builder UI.
- **Expected:** clean log.
- **Actual:** three `The draw_rect() "width" argument has no effect when "filled" is "true"` lines
  (editor.log 14–19). No other builder ERROR/SCRIPT ERROR.
- **Evidence:** `/tmp/opencode/qa/sessions/r4-builder/editor.log` lines 14–19.
- **Undo:** n/a.

### 36. Rain, smoke and sky-coverage changes can be invisible at default settings

- **Severity:** Warning (effects appear broken or absent)
- **Steps:** Weather Rain at the default overview camera; then PixelSmoke default plume; then Add
  smoke; then Sky & Clouds → Coverage 0.4515 → 0.6.
- **Expected:** visible rain streaks, a smoke plume near the hearth, and a visibly different cloud
  deck.
- **Actual:** rain is invisible at ~60 m but clearly visible at 23 m; the default plume is
  invisible, one puff appears only at Start/End 1.0/4.0 Rate 10, and Add spawns smoke ~40 m
  off-island below water. The Sky Coverage before/undo viewport crops are byte-identical (0 pixels
  changed), likely because the deck is at Height 300 above the overview camera.
- **Evidence:** `qa/screenshots/r4-effects/25-weather-rain.png`,
  `…/26-weather-rain-close.png`, `…/46-smoke-hex-a.png`, `…/48-smoke-big.png`;
  `qa/screenshots/r4-pixel/56-sky-coverage.png`, `…/57-sky-coverage-undone.png`.
- **Undo:** weather/smoke/coverage changes undo correctly; the visual output is the defect.

### 37. Dawn and noon wash out the image

- **Severity:** Minor (view-dependent; possibly intentional palette)
- **Steps:** apply Dawn (or set Time 0.27); then noon on the sandy plateau at a top-down angle.
- **Expected:** water/terrain keep their palette; sand keeps texture at noon.
- **Actual:** Dawn washes water and terrain pink; noon turns the plateau almost uniform pale
  yellow/white with white channels, losing terrain shape. Night and morning are readable.
- **Evidence:** `qa/screenshots/r4-pixel/10-lighting-dawn.png`,
  `qa/screenshots/r4-effects/` Dawn captures,
  `qa/screenshots/r4-runtime/36-hex-noon-topdown.png`, `…/38-hex-noon-recovered.png`.
- **Undo:** preset undo leaves the stale tint (issue 14).

### 38. Finite water plane edge is visible

- **Severity:** Minor
- **Steps:** `tutorial/02_island` or `04_world_mood`; lower the orbit and wheel out.
- **Expected:** sea continues to the horizon or fades into the sky.
- **Actual:** a hard rectangular water edge and flat pale-blue/pink void; the island reads as a
  diorama floating in space.
- **Evidence:** `qa/screenshots/r4-runtime/53-t02-orbit.png`, `…/54-t02-zoom-out.png`,
  `…/60-t04-orbit.png`, `…/61-t04-zoom-out.png`.
- **Undo:** n/a.

### 39. Runtime F3/F1 HUD is not resize-safe

- **Severity:** UI
- **Steps:** hex_island at 1024×600 and 800×480; press F3, then F1.
- **Expected:** panels fit or scroll with an obvious bar; help is readable over any terrain.
- **Actual:** F3 is fixed ~420 px wide, starts at x≈604 (59% of a 1024 window), clips the
  transition/ground/strength fields and cuts the F1 line; at 800×480 it covers the player
  completely. F1 is one ~15 px unbacked line, low contrast over bright sand/sky, and is one-shot
  (not a toggle). No collapse/fit control exists.
- **Evidence:** `qa/screenshots/r4-runtime/70-hex1024-f3-default.png`, `…/42-hex-800x480-f3.png`,
  `…/02-hex-1280-f1.png`, `…/69-hex1024-f1.png`.
- **Undo:** not applicable.

### 40. File dialog minimum size is inaccurate

- **Severity:** Minor
- **Steps:** Storage → Import; request 400×200.
- **Expected:** honour the request or report the real minimum.
- **Actual:** reports `min [200,70]` but clamps to **498×200**; defaults 1050×700 (1920) and
  819×480 (1024) both fit. Not a blocker, but misleading.
- **Evidence:** `qa/screenshots/r4-uiresize/24-1920-import-dialog.png`,
  `…/25-1920-import-dialog-narrow.png`, `…/57-1024-import-dialog.png`.
- **Undo:** n/a.

### 41. Grid Aligned silently refuses with Falloff on

- **Severity:** Minor
- **Steps:** Hex Rings, Brush Raise/Lower, Falloff on; click Grid Aligned; then Falloff off and
  click again.
- **Expected:** the control is disabled/grayed while unavailable, or it toggles.
- **Actual:** it looks enabled (`disabled=false`), but the click leaves `toggled=false` with no
  feedback. After Falloff off the same click toggles it on.
- **Evidence:** `qa/screenshots/r4-sculpt/43-grid-aligned-on.png`,
  `…/44-grid-aligned-falloff-off.png`.
- **Undo:** n/a.

### 42. Shift+C opens Bezier instead of appending

- **Severity:** Minor (documented append workflow blocked)
- **Steps:** builder mesh selected; press Shift+C.
- **Expected:** append the new shape to the selected mesh (docs/10).
- **Actual:** the Bezier tool opens; no sibling is created, but the documented append path is not
  reachable by the advertised shortcut.
- **Evidence:** `qa/screenshots/r4-builder/38-shiftC-append.png`.
- **Undo:** n/a.

### 43. Sculpt status/hint line is clipped; Level hint advertises a missing Strength control

- **Severity:** UI (truncated guidance and a phantom control)
- **Steps:** 1) Brush → Drag, paint a selection; read the status line at 1600×900. 2) Select Level;
  read its hint and its strip. 3) Select Brush → Raise/Lower; read its hint.
- **Expected:** the full hint/status fits or wraps; hints only mention controls that exist.
- **Actual:** the status is cut at the viewport edge (`… RMB/Esc: c…`, and after a selection
  `… RMB/Esc` with no `clear` suffix). The Level hint says `Ctrl+wheel: strength`, but the Level
  strip has no Strength value. The Raise/Lower hint lists `Shift+wheel: size` but omits the working
  `Ctrl+wheel: strength`.
- **Evidence:** `qa/screenshots/r4-sculpt/06-brush-selection-painted.png`,
  `…/10-esc-cleared.png` (clipped edge), `…/30-level-strip-hex.png` (Level hint vs strip),
  `…/22-brush-raiselower-strip.png`.
- **Undo:** n/a.

---

## Undo / redo — verified working

The deep pass built a 42-row action × context matrix (`qa/findings/r4-undodeep.md`) using only the
real shortcuts `key z --ctrl` and `key z --ctrl --shift`. **35 rows pass**, 5 fail, 2 were not
exercised.

**Confirmed working (one gesture = one step; Ctrl+Z restores the exact digest; Ctrl+Shift+Z
redoes):**

- **Sculpt:** Brush height draw, Raise, Lower, flatten, Alt one-cell drag, Level, Smooth
  Terraced/Sloped, Bridge.
- **Paint/Foliage:** vertex paint, wall paint, scatter, erase, Fill Terrain, Clear, grass toggle,
  grass Min Height, texture preset (data), slot rename (data).
- **Manage/extras:** Stamp click, Stamp drag, Gradient, Select paste/rotate/mirror, Tint paint and
  erase, chunk add/remove/merge/apply-all, Cell Size, Grid Type (see issue 15), Noise Amp, Display
  Pixel Size, LOD Distance, Bake.
- **Pixel:** God Rays, Use Palette, Darken, Ray Alpha, Sky Coverage, PixelObject outline, Add node,
  Weather Add, palette clear, Fireflies Count, Water Swell/visibility, DayNight time/energy/presets,
  Weather type change, Erosion run, WildFlowers scatter.
- **Builder:** Create Box (one step), Extrude, Inset, H visibility; redo verified for draw/extrude.
  Builder actions go to the **Global** history, so `drive.py undo-state` (root history) reads empty
  even when they undo correctly; proof is mesh counts + `Global Undo/Redo: Pixel Builder: …`.

**Context checks that pass:**

- Two gestures → two Ctrl+Z in reverse order → original; two redos → edited.
- A stroke touching **19 chunks** restores every per-chunk hash and the exact digest.
- Undo after switching tools, selecting `PixelSky`, clicking the 3D view, and after Ctrl+S.
- Ctrl+Z inside a LineEdit (uncommitted text) edits the text and leaves terrain history intact;
  inside an EditorSpinSlider it edits the field. Enter releases LineEdit focus in this build.
- Resizing the editor creates no undo entries and does not clear history.
- Close/reopen clears history (`version 1`, `undo_count 0`); Ctrl+Z does not restore stale state.
- A new edit after undo clears the redo stack.
- The prior “one Ctrl+Z reverts two strokes” and “Ctrl+Z dead after clicking the 3D view” bugs did
  **not** reproduce.

**Broken:** Grass Animation FPS (12), pixel-size one-drag=two-steps (11), lighting-preset preview
tint (14), stale labels after undo (13), Floor Slot/Debug View no-ops (5/6), feature switches not
recorded (19). See also issue 15 for the destructive Grid Type behavior.

---

## Coverage

| Area | Report | PNG | UI dumps | Findings |
|---|---|---|---|---|
| Sculpt (Brush/Level/Smooth/Bridge/toolbar) | `qa/findings/r4-sculpt.md` | 50 | 7 | 6 |
| Paint + Foliage + textures | `qa/findings/r4-paint.md` | 59 | 7 | 9 |
| Manage (Inspect/Chunks/Settings/Storage/Stamp/Gradient/Select/Tint) | `qa/findings/r4-manage.md` | 62 | 14 | 7 |
| Pixel look (15 sections + 9 Renderer pages + Pixel World) | `qa/findings/r4-pixel.md` | 82 | 8 | 10 |
| Effects (12 surfaces + 14 feature switches) | `qa/findings/r4-effects.md` | 89 | 13 | 8 |
| Builder | `qa/findings/r4-builder.md` | 43 | 3 | 6 |
| UI / resizability (5 sizes) | `qa/findings/r4-uiresize.md` | 60 | 3 | 10 |
| Deep undo/redo | `qa/findings/r4-undodeep.md` | 72 | 5 | 5 broken + matrix |
| Runtime / demo / tutorials | `qa/findings/r4-runtime.md` | 71 | — | 6 |
| Fresh-session verification | this file | 3 | — | re-checked 1, 5, 6; re-ran 2 |

All screenshots are local under `qa/screenshots/r4-*` (git-ignored). The index is
`qa/SCREENSHOTS.md`.

## Prior-round items (not re-tested here)

These were reported in the 2026-11-20/2026-11-21 passes and were not re-driven in r4. They are
listed so the text reports keep their provenance; do not treat them as fresh verdicts.

- **Enum family:** the prior pass reported Blend Mode and Wall Slot as label-only too. r4 confirmed
  the same failure for Floor Slot and Storage Mode, found Grid Type fixed, and did not re-test
  Blend Mode/Wall Slot.
- **Stamp and minor settings:** Stamp Size stays stale after a drag/modifier+wheel; chunk-size
  changes record a no-op undo step when refused; Gradient Strength ignores click position;
  Grid Size stays enabled while Grid Aligned is off; an invalid chunk add is only a raw Output
  warning; Foliage Fill Terrain is a no-op for the Tree item on `02_island`.
- **Builder:** at 1280×800 the MST layout clips toolbar headers, cuts the strip behind the
  Materials panel and leaves a ~172×60 viewport; a newly drawn sibling keeps the old mesh
  selected; the Builder Preferences dialog is 484×664, non-resizable, and its action row is
  off-screen at 600 px height.
- **Pixel World:** disabled Add entries were reported as too bright; the panel keeps a scrolled
  offset when rows rebuild; the Inspector value desyncs from the strip after a strip edit.
- **Effects:** a lightning flash was not observed during a 15 s Storm watch (could be a long
  random interval; the r4 pass saw a storm flash but did not capture a bolt).
- **Refuted/fixed since the prior pass:** disabled `+ Add` rows now dim correctly; Debug View no
  longer toggles the grid overlay; Level/Smooth/Tint at 1024×600 did **not** crash; the Paint
  Save Preset OK click-through did **not** reproduce; the LineEdit-keeps-focus-after-Enter bug did
  **not** reproduce; “Ctrl+Z dead after clicking the 3D view” did **not** reproduce; the
  Brush/Raise-Lower strip no longer clips its title at 1600×900 (it expands to 175 px instead).

## Testing limitations

- **Headless:** Godot `--headless` uses the dummy renderer and cannot capture the editor UI. All
  passes used Xvfb virtual displays (1920×1080 screen, 1600×900 editor default), which is
  headless-capable and still renders. No physical display was used.
- **Resource monitor:** `qa/resource-monitor.sh` was used for every wave (it resolves the ai-tools
  checkout at `../Yugens-Terrain-Authoring-Toolkit/tools/ai-tools` because `../rr/tools/ai-tools`
  is absent in this workspace). Recommended widths were 2–4; the orchestrator capped at 3 editor
  sessions.
- **Crash evidence:** the Pixelizer 1280×720 crash was independently re-reproduced and its log
  excerpt saved. The Builder 1366 crash log was overwritten by the automatic restart before it
  could be copied; two fresh attempts (pointer at `(345,478)`, plus a terrain→builder round trip)
  did not reproduce it, so it remains hover/timing dependent.
- **Prior-round claim refuted:** the 2026-11-20 “Level/Smooth/Tint at 1024×600 crashes” did **not**
  reproduce; those three tools were switched and captured at 1024 (`r4-uiresize/59–61`).
- **Not tested:** hexagon/triangle brush footprints and Quick Paint; paint Erase All, multi-select
  palette, slot pattern/scale/color/sprite pickers, overwrite confirmation; Settings Export GLB /
  Export Path / Save Data / Import beyond opening-canceling; Pixel palette inline editing and LUT
  bake, PixelObjectProfile, PixelOutlineColors arrays, DayNight sub-pages; Cloudy/Foggy weather, a
  visible lightning strike, per-switch gating for all 14 features, long snow accumulation;
  Builder bevel/edge-loop/merge/connect/fill/triangulate/duplicate/delete/move, applied paint,
  materials +Slot/+Preset, vertex paint, smooth/collision/rebuild warning, export, save/reload;
  runtime Dawn/Dusk presets and long-run stability; HiDPI (`editor_scale != 1`), Tab order/focus
  rings, and Pixelizer/Builder at 1024 after the crashes.
- **Undo shortcut scope:** every verdict above was driven with the real **Ctrl+Z / Ctrl+Shift+Z**
  keys, as requested. The editor's Edit-menu Undo item and the History dock buttons share the same
  undo manager but were not separately exercised.
- **Harness quirks (not addon defects):** `drive.py undo-state` reads the root history while
  builder edits are Global; dialog-local coordinates reported by `ui` differ from editor
  coordinates; `drive.py info/windows/resize` cannot drive a runtime session (use live X11 resize);
  one harness-induced `PixelSmoke::set_position` Array→Vector3 error appeared in the effects log
  and is excluded.
- The addon was never edited; all saves, scenes and preset files were written only inside
  `/tmp/opencode/qa/sessions/*/project`.

---

## r5 verification pass (2026-11-21, post-fix build)

A second full pass drove the build produced by the fix wave (`fix/r4-all`: the WIP fix stack
plus five issue groups). Five DeepSeek V4.1 Flash drivers covered sculpt/paint, manage/undo,
pixel/effects, builder, and UI+runtime. Per-area evidence (574 PNGs, UI dumps, measured
matrices) is in `qa/findings/r5-sculpt-paint.md`, `r5-manage.md`, `r5-pixel.md`,
`r5-builder.md`, `r5-ui-runtime.md` and `qa/screenshots/r5-*`.

### r4 items confirmed fixed

1, 2, 4, 5, 6, 7, 8 (for the builder above 720 px height), 9, 10, 11, 12, 13, 14, 15, 16,
17, 18, 20, 21, 24 (replaced by N1 below), 26, 27, 28 (except its remembered size), 29, 30,
31, 32, 33, 34, 35, 36 (rain, sky coverage, lightning), 38 (scene/sea half), 40, 41, 42, 43;
plus the orbit clamp (#23) and the F1 toggle (#39).

Notable proofs: PixelRenderer at 1280×720 and 1024×600 no longer crashes; the builder at
1366×768 shows no `bind_mut` panic; Floor Slot writes `9` + undo and Storage Mode writes `1`
with the Save Data row; Debug View writes `2`; Grid Type asks before dropping 295 chunks and
one Ctrl+Z restores the digest; one pixel-size drag is one undo step; fireflies went from 34
to 624 warm pixels at night; the builder strip is capped (689×175 at 1600×900).

### r4 items still reproducing

> The entries below are the raw r5 snapshot. #36 (smoke), #37 (dawn/dusk), #38 (water edge) and
> #39 (F3/F1) were fixed in Round 2 — see the remediation status at the end of this file.

- **#19** feature switches record no visible undo entry (`drive.py undo-state` reads the root
  history only, so this needs a `Ctrl+Z`/History check before it is called broken).
- **#24** is superseded by **N1**: at 1024×600 the Pixel World panel is 24 px wide and the
  Inspector ~136 px.
- **#36** the default `PixelSmoke` plume is still invisible (78 px of 10×16-patch output).
- **#37** dawn still flattens the frame (mean 177/177/187, water≈land) and dusk 0.75 is
  pink-mauve; noon is clean.
- **#38** the water plane is still a hard-edged floating rectangle at low orbit/zoom-out.
- **#39** F3 still clips at 1024×600 / 800×480 and cannot be collapsed; F1 is unbacked and F3
  overdraws it (the F1 toggle itself works).

### New defects (r5)

| # | Sev | Area | Issue |
|---|---|---|---|
| N1 | **Critical** | Pixelizer | Selecting any `Pixel*` node on a fresh session leaves the whole Pixelizer UI at zero size (rail 39×0, strip 0×175, panel 250×0; at 1024: 0×99 / 24×33); only a 2D→3D view round-trip lays it out. Deterministic, all sizes, nothing logged. Pixel effects are then editable only through the raw Inspector. |
| N2 | Warning | Runtime | F3 tuner overflows 1024×600 and 800×480 (fields below the window) and cannot be collapsed. |
| N3 | Minor | Runtime | F1 is unbacked and the F3 panel overdraws/cuts it. |
| N4 | Minor | Editor | Side-panel width/collapse is not remembered across an editor restart (in-memory project metadata only). |
| N5 | Minor | Dialogs | Save Preset logs `ERROR: Couldn't find … "save_preset_dialog_size"` on first open and forgets its size. |
| N6 | Minor | Logging | 2× `ERROR … create_frustum_points (rendering_light_culler.cpp:682)` and 16× plane/vector normalize warnings at boot while a `Pixel*` node is selected. |
| N7 | Warning | Layout | No horizontal viewport floor: Paint view 333×341 at 1280, 416×389 at 1366; Builder 358 wide at 1280/1024; a 220 px panel at 1024 leaves 258×297. |
| N8 | Minor | Layout | 1024×600 view height is 297 px, 3 px under the documented 300 floor. |
| N9 | Minor | Dialogs | Save Preset honours a 400 width but not a 150 height (opens 400×179; reports min 140). |
| N10 | Minor | Layout | Settings strips reserve 175 px for 105 px of content (dead space). |
| N11 | Minor | Builder | Builder Materials palette (245–259 px) and the builder rail have no grip or collapse. |
| N12 | Error | Builder | `Ctrl+D` is still Godot's *Duplicate Node(s)*: it creates a sibling `Ruin2` instead of duplicating faces, loses the Face selection and puts “Duplicate Node(s)” in History. |
| N13 | Warning | Builder | `+ Slot` then Ctrl+Z leaves a phantom third row in the Materials palette; `+ Preset` on it re-creates slot 2. |
| N14 | Minor | Builder | The “Editing the Box recipe…” HUD hint leaks onto another selected mesh; the `Following <node>` badge overlaps the HUD's first line; `F` shows the badge but does not change camera distance. |
| N15 | UI | Builder | Export OBJ still reports `min [200,70]` and clamps to 498×200 with a clipped body. |
| N16 | UI | Manage | Settings → Grid clips its last row (Blend Mode at y=757 in a 175 px viewport that ends at 757) and the page tabs are inside the scroll content, so scrolling to it hides the tabs. |
| N17 | UI | Manage | Chunks status line is clipped at the viewport edge (the refusal text is cut); the green `+` handles are nearly invisible on bright terrain; add/remove are worded three different ways; no “N selected” feedback for Ctrl+LMB. |
| N18 | UI | Manage | Storage → Export GLB has no path field (Inspector-only) and silently wrote a 32 MB GLB; Save Data is always enabled and gives Output-only feedback. |
| N19 | UI | Manage | The bottom-panel splitter cannot be shrunk below 136 px after expanding. |
| N20 | UI | Manage | Import dialog is cramped at its new 500×300 minimum (2-row list, `All Recogniz…`, dead Favorites/Recent). |
| N21 | Minor | Manage | LOD & Nature “Add” (Erosion) steals the selection; undoing the add leaves nothing selected and closes the terrain tool. |
| N22 | Minor | Manage | Tint “Grid Size” stays enabled while “Grid Aligned” is off (prior-round item). |
| N23 | UI | Pixel | Page-tab grid (`SectionPages`, 378 px of fixed 4 columns) is clipped in strips as narrow as 414/640 px; the strip content is 204 px in a 175 px viewport, so one row is always clipped and the horizontal auto-scroll slides the tabs. |
| N24 | UI | Pixel | Pixel World panel content is ~940 px (9 rows below the fold) and collapses to 24 px at 1024; pressed state for Preview/Animate is only a 10-unit background lift. |
| N25 | Warning | Effects | SunShafts is wired but no shaft is isolatable at alpha 0.6 / count 64 (scene has no occluder). |
| N26 | Warning | Paint | Foliage **Fill Terrain** records an undo step but paints nothing for the Tree item (862→862 props). |
| N27 | UI | Sculpt | Hint block does not wrap (first line clipped mid-word at 1280/1024); the status block grows to three lines and sits on the strip's top edge at 1600. |
| N28 | Minor | Sculpt | Grid Aligned stays checked-but-disabled after a hex→square scene switch; the Raise/Lower pick hint says `Ctrl+LMB / MMB / RMB` although plain MMB does nothing. |
| N29 | Minor | Sculpt | `MstValueLabel` drifts from the slider value after wheel edits (label 18.9 vs value 19.0; 4.844 vs 4.7595). |
| N30 | Minor | Settings | The Settings “Grid” tab and the toolbar “Grid” overlay share a label; `click-text "Grid"` hits the overlay and creates a `terrain grid overlay` undo entry. |
| N31 | Minor | Logging | Boot log noise common to all r5 sessions: 16× plane/vector normalize warnings and 2× `create_frustum_points` ERRORs (same as N6). |

**Correction to an intermediate note:** the earlier “Paint strip 385 px, bottom off-window” was
the unclipped `MstFlow` content; the strip itself is 333×175 (1280) and 559×99 (1024) and
scrolls. No surface spills below the window. The real width problem is N7.

### Round 2 remediation status (2026-11-21)

A second fix wave was started against this list but was stopped by the Claude provider's
weekly usage limit (resets 2026-11-25 15:00 Australia/Brisbane). Only one fix landed:

- **N12 — fixed (build-verified, not re-driven in the UI):** `Ctrl+D` is now claimed in the
  builder plugin's `_input` while a mesh is edited in Face/Edge/Vertex mode, so it duplicates
  faces instead of running Godot's *Duplicate Node(s)* (commit `e93a672`, merged into
  `fix/r4-all` at `4d7b8dd`; verifying run: no `Ruin2`, no `Duplicate Node(s)` history).
- **N2 / N3 — fixed (runtime-verified at 1024×600 and 800×480):** the F3 tuner has a hard
  width cap (307 px at 1024, 260 px at 800), scrolls both ways, and collapses by clicking its
  title; the F1 help is a dark backed panel that stays clear of the tuner and of an expanded
  object inspector (`pixel_diagnostics_keep_clear`). Commits `91fdee6`, `21d183e`, `aa7a61e`,
  merged at `4d5b722`. The r5 runtime evidence above predates #50/#51 (see the harness note).
  One case is unconfirmed: the help staying clear when an object is selected.
- **r4 #23 and #38 (runtime) — verified-working** on the current build: the orbit stops at
  terrain level and the sea band reaches the horizon.
- **N13 — fixed (verified live):** the Materials palette re-syncs from the mesh data on every
  undo/redo and clamps the current slot to existing slots (`+ Slot` → rows 0/1/2, Ctrl+Z →
  0/1). Commit `3dc7acf`, merged at `ffb018b`.
- **N14 — partly fixed:** the “Editing the … recipe” hint clears on mesh/mode change, the HUD
  sits below the viewport labels and the `Following` badge no longer overlaps line one
  (`ff1a9b0`). `F` is still Godot's command: it re-centres but does not zoom, and no badge
  appears. The builder cannot set the editor camera, so the tooltip/doc wording (“frames”) is
  the remaining gap.
- **N11 — partly fixed:** the Materials palette now has the terrain side panel's 220–420 px grip
  and 24 px collapse (verified: viewport 657→910 px at 1600×900, `fecb0ef`). Remaining: it
  cannot auto-collapse (needs a `ui_kit` export to collapse without saving the user's choice),
  and the expanded default is 277 px, so at 1280×800 the viewport is **340 px, down from 358**.
  The builder's 86 px left rail still has no collapse. See **N32**.
- **N15 — fixed (verified live):** every builder file dialog has a real 500×300 minimum and
  opens ≥760×520; Export OBJ reports `min [500,300]` and a 400×200 request clamps to 500×300
  with the body fitting (`9340588`).
- **Builder Face/Edge/Vertex strip (finding 6) — fixed (verified live):** element modes show one
  section at a time (Extrude / Ops / Move / Paint tabs; empty tabs hidden). At 1600×900 Face
  mode the strip is 62–132 px instead of 175 px root / 194–428 px content (`7d6cab5`,
  `f23a426`).
- **N32 (new, warning):** after the palette grip change, its expanded default is 277 px, so at
  1280×800 the builder 3D view is 340 px wide (was 358). Auto-collapse below a 480 px view is
  still missing (blocked on a `ui_kit` auto-collapse API).
- **r4 #36 (smoke) — fixed (verified):** puffs never shrink below 3 low-res pixels and the plume
  thins more slowly; changed area went from 78 px to 389 px at a comparable viewport. The scene's
  4 m plume still reads as a small puff at a full-island overview (`632e20f`).
- **r4 #37 (dawn/dusk) — fixed (verified):** the wash was fog distance, not palette: fog density
  is now scaled to a 40 m camera-to-focus distance and the day/night haze tints the sky at 0.35
  instead of 1. Dawn mean 147/154/172 (was 177/177/187), dusk 138/112/124, noon 64/123/172; the
  island stays green and the water distinct in all three (`c45b9e8`).
- **r4 #38 (water edge) — fixed (verified):** the dark band was a linear-vs-sRGB colour
  conversion in the pixel pass, not the horizon midpoint; the rim now tints to the sky colour in
  sRGB and fades on a circle, so there is no straight edge or corner even at the farthest zoom
  (`0d6bb68`, `2abe3c0`, `361fe50`).
- **N25 (SunShafts) — verified-working within the sun's shadow range**, plus new configuration
  warnings when the camera is past 60% of the shadow distance or nothing tall occludes. From the
  overview nothing shows because the QA camera (~165 m) is past the node's 120 m shadows
  (`1c26419`). Scenes that want shafts at the overview need a longer
  `directional_shadow_max_distance` (e.g. 300 in `04_world_mood`).
- **N6/N31 (boot log noise) — still open, attributed.** It reproduces only at editor start
  (16 plane + 16 vector warnings and 2 `create_frustum_points` errors), and only from the r5
  build onwards; reopening scenes, 2D↔3D switches, `PixelRenderer` selection and resizes add
  none. No effects node creates a camera/viewport; the likely source is the r5 editor layout
  work (`cd5f077`: `ui_kit.rs`, `pixel_ui/plugin.rs`). It needs a boot comparison with those
  changes reverted.
- **N5 / N9 — fixed (verified):** Save Preset no longer logs the metadata ERROR, reports a real
  minimum (340×178) so a 400×150 request opens 400×179, and remembers its size (`6599289`).
- **N4 — fixed (file write verified; restart not tested):** panel width/collapse is written
  through to `res://.godot/editor/mst_editor_state.cfg` on every change (`6599289`).
- **N7 — fixed (verified):** side panels keep the 3D view at least 480 px wide. Paint at 1280
  collapses the panel to its tab (view 551 px, was 333); 1600 re-expands to 250; the tab forces
  it open at 220 (`b620f54`, `93945af`).
- **N10 — fixed (verified):** the strip cap is a maximum — Settings → Storage is 107 px, not
  175. Pinned strips keep the tallest height seen for that tool until the tool changes
  (`fc89ca7`, `93945af`).
- **N16 — fixed (verified):** the title and page tabs stay pinned while the page body scrolls to
  its last row.
- **N23 — fixed (verified):** page-tab columns are chosen from the strip width (two rows,
  68 px); at 1024 the header becomes a normal scrolling first row instead of covering the 99 px
  strip.
- **N24 — partly fixed:** the collapsed tab is now 32 px; the Inspector 260 px minimum is not
  fixed, so at 1024 it still runs past the window edge.
- **N29 / N30 — fixed (build only):** wheel-edited settings snap to the slider step; the
  Settings “Grid” tab is renamed **“Layout”** (scripts/`click-text` must use that name).
- **N22 — verified-working:** the Grid Size slider was already non-editable (the QA dump only
  reports `disabled` for buttons); a tooltip now explains why.
- **r4 #19 — verified-working (harness limitation):** History shows “Turn Birds feature off” and
  one Ctrl+Z sets `features/birds` back to true. Ctrl+Z pressed while the popup is still open
  does nothing; it works after Escape.

**Merge note:** the editor and builder round-2 branches both changed
`ui_kit::page_tabs` (new `columns` argument) and its builder call site, so the textual merge
left a semantic mismatch that did not compile. It was reconciled in `e50088d` (builder page
tabs pass `STRIP_PAGES.len()` and `Ui::pages` is a `GridContainer`).

**Harness note (affects the r5 runtime evidence):** `qa/runtime.sh` reuses
`$MST_QA_STATE/runtime/project` on every start, so a runtime pass inherits whatever build first
created that copy. The r5 runtime observations were taken from a copy that predated #50/#51.
Delete `runtime/project` (and `runtime/<name>`) before a runtime pass to force a fresh copy.

**N1 still reproduces** on the merged build, but only on the **first** selection after a fresh
session. A second selection (select the terrain, then the `Pixel*` node again) lays everything
out correctly — rail `39×732`, strip `731×173`, panel `250×732` — and a 2D→3D round-trip also
recovers it. So the defect is a missing first-frame relayout when the panels are created and
shown for the first time; a deferred sort of the main screen (or a relayout on the next idle
frame) is the place to fix it. A partial attempt (blocking signals on the panel's descendants
instead of its root, then `queue_relayout`, commit `3fbacf5`) is merged but does **not** fix the
first-selection path. Deterministic repro: fresh session → `select --path PixelRenderer` →
`ui --root all` shows `PixelizerToolbarRail 39×0`, `PixelizerAttributes 0×35`,
`PixelWorldPanel 250×0`; reselect once and they size correctly.

Everything else in N1–N32 was **not attempted** (N1 needs the first-frame relayout described
above, N8, N11 auto-collapse / the N32 regression, N17–N21, N24's Inspector minimum, N26–N28
and N6/N31). Round-2 briefs are staged in `/tmp/opencode/r4groups/R2A-editor.md`,
`R2C-builder.md` and `R2D-effects.md`.

---

## r6 evaluation pass (2026-11-21, post-Round-2)

Four DeepSeek V4.1 Flash drivers re-verified the Round-2 build (`fix/r4-all` @ `e50088d`):
editor shell, builder, pixel/effects, and runtime on a **fresh** runtime project copy. Evidence:
`qa/findings/r6-editor.md`, `r6-builder.md`, `r6-pixel-effects.md`, `r6-runtime.md` and
`qa/screenshots/r6-*` (271 PNGs + dumps + logs).

### Verified fixed (Round-2 claims hold)

- **N1** — the first `Pixel*` selection now lays out without reselecting: rail **39×732**,
  strip **731×175**, panel **250×732** (reselection and 2D→3D give identical rects). The
  zero-size state is now only the first build frame / ~2–3 s while 631 controls are created,
  then it self-corrects (see R6-1).
- **N7** (Paint at 1280 → 551 px view, panel collapses; 1600 re-expands), **N10** (Storage strip
  107 px; high-water pin per tool), **N16** (tabs pinned while the body scrolls), **N23**
  (two-row tabs, no sliding; first-build overflow in R6-2), **N4** (cfg file records
  width/collapse), **N5/N9** (420×179, min 340×178, no ERROR, 400×150→400×179, remembered),
  **N24** (32 px tab; Inspector still 87 px past 1024), **N29/N30/N22** (wheel snapping,
  “Layout” tab, Grid Size tooltip + disabled), **r4 #19** (History “Turn Birds feature off”;
  one Ctrl+Z after Escape restores).
- **Builder:** **N12** (Face 209/401/210 → 213/405/211, History `Pixel Builder: Duplicate`, no
  `Ruin2`; residual R6-9), **N13** (palette 3 rows → 2 on undo, no phantom), **N15** (500×300
  honoured, body fits), **section tabs** (one at a time; 62/132 px at 1600), **crash watch** (no
  crash/bind_mut at five sizes), regression sweep (draw/extrude/inset/paint/bevel/modeler/
  save-reload) all pass.
- **Runtime:** **N2** F3 (308 px @1024, 261 px @800, scrolls, title-click collapse, player
  clear), **N3** F1 (backed 401×91, no F3 overdraw), **r4 #23** orbit, **pixel scaling**,
  **tutorials 01–04**, and a clean game log (N6/N31 is editor-only).
- **Pixel/effects:** clamps **6/6**, **#36** smoke (78 → 317 changed px), fireflies, weather,
  `+ Add` stays open, Preview/Animate pressed states, preset-undo tint byte-identical.

### Still not fixed / residuals

- **r4 #38** — `hex_island` passes (sea to horizon, 0 px void), but `tutorial/02_island` and
  `04_world_mood` still show a straight sea edge + ~32% pale void at minimum pitch and maximum
  ortho zoom; the pixel pass sees a rotated square with four sharp corners on `04_world_mood`
  (**R6-12**).
- **r4 #37** — dawn is improved (150/156/173 vs 177/177/187) but still a flat pale wash
  (**R6-18**).
- **N25 SunShafts** — warning text is stale after `directional_shadow_max_distance` changes, and
  no shaft isolates at 400 m / alpha 0.6 / count 64 (**R6-15**).
- **N11/N32 builder palette** — collapse keeps a 277 px allocation, the 220 px minimum is
  recorded but not applied, the split dragger bypasses the 420/480 limits (537 px palette →
  397 px view), and the view is still 340 px at 1280/1024 (**R6-7**, **R6-8**).
- **N6/N31** — boot noise unchanged (16+16 normalize warnings + 2 `create_frustum_points`
  errors at editor startup only; nothing added during a session).

### New r6 defects

| # | Sev | Area | Issue |
|---|---|---|---|
| R6-1 | Warning | Pixelizer | First `Pixel*` selection shows an empty Pixelizer UI for ~2–3 s (631 controls built late); no spinner/placeholder. |
| R6-2 | Warning | Pixel tabs | First build makes one 670×33 tab row; `Debug Views` ends 42 px past the 731 px strip and renders “Debug V” until the next rebuild. |
| R6-3 | Minor | Settings | Settings → **Layout** overflows at 1600: content 929 px in 903 px, visible H scrollbar, 26 px of the third column clipped. |
| R6-4 | Minor | Settings | First Settings open inherits the previous tool's 175 px strip; the high-water pin then keeps it. |
| R6-5 | Minor | Pixelizer | At 1024 the strip spends 66 of 99 px on two tab rows, clipping one body row. |
| R6-6 | Minor | Sliders | `EditorSpinSlider` ignores the wheel (N29's snapping verified on HSliders only). |
| R6-7 | Warning | Builder | Palette collapse leaves a 277 px allocation (content hidden, `ExpandTab` 277 wide); the 220 px minimum is stored but not applied. |
| R6-8 | Warning | Builder | The split dragger bypasses the 420 px palette / 480 px view limits: 537 px palette leaves a 397 px view. |
| R6-9 | Warning | Builder | `Ctrl+D` with focus outside the 3D main screen (e.g. History dock) still runs Godot's Duplicate Node(s) → `Ruin2`. |
| R6-10 | Minor | Builder | The generic HUD status line is sticky across mode/mesh/undo; HUD line 2 still clips at ≤1366. |
| R6-11 | UI | Builder | The strip stays pinned at the 175 px cap after a tall section at 1280. |
| R6-12 | Warning | Runtime | Tutorial sea plane edge + pale void at min pitch/max zoom (see #38 above). |
| R6-13 | Minor | Runtime | The F3 object picker never selects in a plain game (4 clicks + 3 hovers: no outline), so N3's object-clearance case is untested. |
| R6-14 | Minor | Runtime | F1 help (13–400 × 376–466) covers the player (x 355–425) at 800×480; F3 does not. |
| R6-15 | Warning | Effects | SunShafts configuration warning does not refresh after `directional_shadow_max_distance` changes; 0 shafts at 400 m / alpha 0.6 / count 64. |
| R6-16 | Minor | Pixel | Feature-switch Ctrl+Z is dead while the popup is open; it works after Escape (known). |
| R6-17 | Minor | Harness | Strip number fields show drag saturation (6 px → max, 4 px → nothing); likely XTEST/llvmpipe, flag for a real-mouse check. |
| R6-18 | Minor | Effects | Dawn still a flat pale wash (mean 150/156/173; island 105/107/74 vs water 148/161/177). |

### Harness notes

- `project_base` has no `.godot/`, so a freshly copied runtime project cannot register the
  GDExtension classes until `godot --headless --path <project> --import` runs once.
  `qa/runtime.sh` now does that automatically when `.godot` is absent.
- A `ui --root all` dump costs ~2.5 s of editor time at llvmpipe; the 2D main-screen button is
  at x 587–645 (not the x≈663 some scripts assumed); the Import dialog ignores Escape.

---

## r7 fixer pass (2026-11-21) — claimed fixes under evaluation

One Claude Opus 5.5 (medium) agent owned the whole tree (`work/r7`, merged into `fix/r4-all` at
`4b6811e`). Build + 30/30 tests pass. The next evaluation wave verifies these claims:

| Item | Claim |
|---|---|
| N11/N32, R6-7/R6-8 builder palette | fixed: splitter drags go through the panel's own 220–420 / 480-view limits; collapse gives the space back (420→32 px, view 514→902 at 1600); view 585 px at 1280×800 and 1024×600 (r6: 340); 220 px minimum applies and is saved |
| #38/R6-12 tutorial sea | partly fixed: the orbit camera backs off then raises pitch until the view's bottom edge clears the sea; 0 void rows in 02/04 at min pitch + 30 zoom notches (r6: 232 px / 32%). **Residual:** a lighter rotated square centred on the island at that extreme, not the cloud deck, god rays or water depth bands |
| R6-1 Pixelizer first paint | not reproduced (first rendered frame already laid out); no change |
| R6-2 first-build tab row | fixed: first build is now two rows 405×66 with `Debug Views` inside the strip |
| #37/R6-18 dawn | partly fixed: Dawn fog 0.0045→0.0015; whole view 150/156/173→113/124/155, water 74/103/157, island 91/91/62 (still olive; palette quantization) |
| N25/R6-15 SunShafts | warning computed on request (text unverified through the harness); shafts verified with pillars: 142,649/654,963 px change at 400 m / alpha 0.6 / count 64 |
| R6-3 Settings Layout | fixed: two columns, no H scrollbar (212 px content in 175 px scrolls vertically) |
| R6-4 Settings strip height | fixed: reopening from Paint gives 138 px (r6: 175) |
| R6-5 Pixelizer 1024 | fixed: page tabs become a single 33 px dropdown, 66 of 99 px for the body |
| R6-6 spin-slider wheel | fixed: Paint Rotation steps 0→2→1 |
| N24 Inspector minimum | blocked by Godot: the main screen's 709 px minimum (3D toolbar 701 px) plus docks exceeds 1024; our panels already collapse to 32 px |
| R6-9 Ctrl+D focus | fixed: History-dock focus shows the builder hint, no `Ruin2`, undo count 0; Scene/FileSystem/text fields keep Godot behaviour |
| R6-10 HUD | fixed: status clears on mode/mesh/undo; line 2 fits at 585 px and wraps at 397 px |
| R6-11 strip stuck at 175 | fixed by the palette work: 62/124 px at 1280 |
| R6-13 F3 picker | not reproduced (hover outlines the knight, click selects `Knight_Body`) |
| R6-14 F1 covers player | fixed: at 800×480 help ends x≈246 (player x≈355), 7 lines |
| N6/N31, R6-16, R6-17 | not attempted |

Harness: `qa/runtime.sh` now imports a freshly copied runtime project (the r7 branch missed this
until the merge); editor `capture` can hang while idle — use `shot` instead.
