# Area 7 — UI / resizability audit (r4)

Session `r4-uiresize` (display `:37`, port `8817`), Godot 4.7.2, llvmpipe, editor scale 1.0,
scene `res://demo/hex_island.tscn`. 60 PNGs in `qa/screenshots/r4-uiresize/`; three full UI dumps
in `qa/screenshots/r4-uiresize/ui/` (`00-1600-boot.json`, `06-1920-paint.json`,
`07-1920-foliage.json`). The rest of the numbers below come from live `ui --filter` dumps taken
immediately after each capture (filtered dumps are not saved because each full dump is ~14 MB).
Screenshot names are relative to `qa/screenshots/r4-uiresize/`.

Two editor crashes occurred (one builder panic, one Pixelizer layout loop) and the session was
restarted twice; the crash log segments were read before the restarts overwrote `editor.log`.
The editor minimum is still **1024x600**: `resize 800 600` reports `actual [1024, 600]`
(`58-1024-clamp-800x600-request.png`). Unlike the previous pass, **Level/Smooth/Tint at 1024x600
did not crash**; all three were switched and captured (`59`, `60`, `61`).

## Measured matrix

`view` = 3D viewport size (the `PixelPreviewOverlay` rect, which matches the visible 3D area);
`strip` = `MarchingSquaresToolAttributes` / `PixelizerAttributes` / `PixelBuilderStrip` size
(rows ≈ height/35); `panel` = side panel width. The bottom Output dock was open (224 px) for every
measurement, exactly as the editor layout arrived; closing it would add ~224 px to the view height
but does not change any width result. Docks auto-shrink at 1366/1280/1024 (left dock 280 → 172),
so those widths are Godot-level, not addon-level.

| Surface | 1920x1080 | 1600x900 | 1366x768 | 1280x720 | 1024x600 |
|---|---|---|---|---|---|
| Terrain toolbar, expanded | 116x682, 1 col | 236x469, 2 col | 236x337, 2 col | 236x289, 2 col | 236x287, 2 col |
| Terrain toolbar, compact | 36x682, 1 col | n/t (36 expected) | n/t | n/t | 76x392 (2 col) / 116x252 (3 col) — changes with strip height |
| Brush | view 1227x574; strip 105 | view 787x361; strip 105 | view 553x124; strip **210** | view **470x41**; strip **245** | view **470x39**; strip **245** (compact: view 590x39, strip 175) |
| Level | view 1227x609; strip 70 | n/t | view 553x159; strip 175 | n/t | view 590x39; strip 140 |
| Smooth | view 1227x609; strip 70 | n/t | n/t | n/t | view 590x39; strip 175 |
| Bridge / Stamp / Gradient / Select / Tint | strips 70–71 | n/t | n/t | n/t | Tint strip 140, view 590x39 |
| Chunks | view 1227x609; strip 70 | n/t | n/t | n/t | view 590x39; strip 140 |
| Paint | view 974x574; strip 974x105; panel 250x682 (content **5767**) | view **534x256**; strip **534x210**; panel 250x469 | view **300x39**; strip **300x385** | view **292x39**; strip **292x385** | view **377x39**; strip 377x350; panel 250x392 |
| Foliage | view 974x609; strip 105; panel 250x682 | view 534x361; strip 105; panel 250x469 | n/t | n/t | n/t |
| Settings — Grid | strip 140; 6 tabs at x 575–1013 | view 787x291; strip 175 | view 553x124; strip 210 | view **470x39**; strip **385** | view 630x39; strip 350 (compact toolbar) |
| Settings — other pages | strip 105, no clipping | n/t | n/t | n/t | Storage strip 350, buttons reachable |
| Pixelizer (Renderer) | view 1055x574; rail 35x682; strip 1055x105 (min 670); panel 250x682 (content 941) | view 696x293; rail 74x469; strip 696x173 (min **670**); panel 250x469 | view 670x161; rail 74x337; strip **670x173** (fits by 0 px) | **crash on select** | n/t |
| Pixelizer (PixelSky) | strip 1055x105 (min 186) | n/t | n/t | n/t | n/t |
| Builder — Object | view 871x617; toolbar 224x682; strip 871x62; palette 245x682 (content 659) | view 421x310; toolbar 340x469; strip 421x156; palette 259x469 (content 659) | **crash on select** | n/t | n/t |
| Builder — Face | view 795x502; toolbar 303; strip 795x177 | view **345x39**; toolbar **416x489**; strip **345x447** | n/t | n/t | n/t |
| Save Preset dialog | 558x150; narrow request 400x150 clamps to **558x150**; 800x220 accepted; reopens 558x150 | n/t | n/t | n/t | n/t |
| Import file dialog | 1050x700 at (435,190); 400x200 clamps 498x200 | n/t | n/t | n/t | 819x480 at (102,60), fits |
| Pixel World menu popup | 203x648 at (1223,115) | n/t | n/t | n/t | n/t (would exceed 600) |

Notable deltas from the previous pass: 1600 Brush is now 361 px tall (was 326) because the strip
settles at 105, not 140; 1600 Paint is **worse** (view 534x256 vs 662x320) because the expanded
2-column toolbar takes 236 px, leaving a 534 px strip that wraps to 6 rows. At 1366/1280/1024 the
panel/strip reflow (not the window height alone) is what collapses the view.

## Findings

### F1. Selecting a PixelBuilderMesh while the pointer sits over the builder toolbar panics the Rust plugin and kills the editor — error
- Steps: 1) At 1366x768, with the pointer at (345,478) (the area where the Builder toolbar appears),
  run `select --path BuilderRuins/Ruin`. 2) The harness socket drops; the editor exits.
- Expected: selecting the node opens the Builder UI, as it does at 1920.
- Actual: two `Gd<T>::bind_mut() failed, already bound; T = ...PixelBuilderPlugin` panics from
  `PixelBuilderPlugin::on_button_hover` fired by `mouse_exited`, then
  `Failed method: Container::_sort_children. Message queue out of memory.` and the editor is gone.
  The selection never completed. Moving the pointer to the 3D view before selecting avoided it in
  the next session (1920/1600 selections worked).
- Evidence: quoted from `editor.log` at the time (that log was overwritten by the restart):
  `ERROR: [panic .../godot-core-0.5.5/src/storage/mod.rs:38] Gd<T>::bind_mut() failed, already
  bound; T = marching_squares_terrain::builder::plugin::PixelBuilderPlugin.` followed by
  `ERROR: Error calling from signal 'mouse_exited' to callable: 'PixelBuilderPlugin::on_button_hover'`
  and `Failed method: Container::_sort_children. Message queue out of memory.`; last capture
  before the crash `43-1366-pixelizer.png`.
- Undo: not applicable (process died).

### F2. Selecting a PixelRenderer at 1280x720 crashes the editor with a container layout loop — error
- Steps: 1) 1280x720, terrain selected, pointer neutral at (700,300). 2) `select --path PixelRenderer`.
- Expected: the Pixelizer UI opens (it does at 1366, where its strip fits its 670 px minimum).
- Actual: connection reset; `editor.log` line 14 is
  `Failed method: Container::_sort_children. Message queue out of memory. Message queue out of memory.
  Try increasing 'memory/limits/message_queue/max_size_mb' in project settings.` followed by a
  188k-line flood of `Object was deleted while awaiting a callback.` At 1280 the available strip
  width is below the Pixelizer strip's hard 670 px minimum, so the ScrollContainer/flow never
  converges. At 1366 the strip is exactly 670 px and does not crash.
- Evidence: quoted `editor.log` (overwritten by restart); `49-1280-pixelizer.png` was not written
  because the process died; `43-1366-pixelizer.png` is the same surface at the last working size.
- Undo: not applicable.

### F3. Attribute strips have no height cap and squeeze the 3D view to a 39 px slit — error
- Steps: 1) Select the terrain. 2) Resize 1600→1366→1280→1024 and switch Paint/Settings.
- Expected: the strip caps at ~3 rows (105 px) or scrolls internally, keeping at least 480x300 for
  the 3D view.
- Actual: the strip takes its full flow height and the VSplit gives the remainder to the viewport:
  Brush 553x124 (1366), 470x41 (1280), 470x39 (1024); Paint 300x39 (1366), 292x39 (1280),
  377x39 (1024); Settings Grid 470x39 at 1280 with a **385 px** strip (11 rows). At 1024 the strip
  content itself is fully visible (0 clipped controls; Brush 5 rows at 590 px wide, Settings 10
  rows, no horizontal scrollbar), so this is not label clipping — it is unbounded strip growth.
- Evidence: `39-1366-brush.png`, `41-1366-paint.png`, `42-1366-settings.png`,
  `46-1280-brush.png`, `47-1280-paint.png`, `48-1280-settings.png`, `52-1024-brush.png`,
  `54-1024-paint.png`, `55-1024-settings-grid.png`; matrix above.
- Undo: n/a (layout).

### F4. Builder Face/Edge/Vertex strips are 447 px tall and collapse the view — error
- Steps: 1600x900, select `BuilderRuins/Ruin`, click `Face mode [2]`.
- Expected: a grouped/scrolling strip ≤240 px, viewport ≥480x300.
- Actual: viewport **345x39**, strip **345x447** (min 238x447, not scrollable), and the Builder
  toolbar grows from 340 to **416x489** (extra columns) — together they consume the width and all
  of the height. At 1920 the same mode is usable (view 795x502, strip 177), so this is a
  1600-and-below failure, not a small-window-only one.
- Evidence: `29-1920-builder-face.png` (works), `37-1600-builder-face.png` (fails).
- Undo: n/a.

### F5. Paint/Foliage/Pixel World/Materials panels are fixed-width and (mostly) unscrollable; Paint's panel is 5.8k px tall — warning
- Steps: Paint at 1920; look for a splitter; wheel to the bottom; repeat for Foliage, Pixel World,
  Builder Materials.
- Expected: a visible splitter (220–420 px), a collapse tab, and a scrollbar that reaches the
  action row in a few notches.
- Actual: Paint panel fixed **250x682**, content **5767 px**, only ~1.9 texture slots visible at
  1080p; the vertical scrollbar exists (14 px, visible) but **90 wheel notches** were needed to
  bring `Export Texture Preset` into view (`06`, `19`). Foliage is also fixed 250 px with no
  scroll, but its palette is not in the editor Control dump at all (only the 250 px viewport
  shrink is measurable), so it cannot be tabbed to or scrolled like a normal panel. Pixel World
  is 250x682 with a 941 px content and a scrollbar; Builder Materials is 245–259 px wide with
  659 px content in 469 px, so its bottom slots need scrolling.
- Evidence: `06-1920-paint-strip.png`, `19-1920-paint-panel-bottom.png`,
  `07-1920-foliage-panel-strip.png`, `26-1920-pixelizer-renderer.png`,
  `28-1920-builder-object.png`, `32-1600-paint.png`.
- Undo: n/a.

### F6. The terrain toolbar's column count depends on the shrinking editor height, so it steals 236–416 px and jumps between 1/2/3 columns — warning
- Steps: compare 1920 (1 column, 116 px) with 1600/1366/1280 (2 columns, 236 px) and 1024 compact
  after changing tools (2 columns 76 px → 3 columns 116 px).
- Expected: a fixed 30–36 px icon rail with vertical scrolling, or at most 2 columns, never
  wider than ~80 px in compact mode.
- Actual: expanded 116→236 px (at 1600 that is 15% of the window); compact at 1024 started at
  76x392 (2 columns) and later became 116x252 (3 columns) as the strip grew, so the tool positions
  move while the user works. At 1024 the compact rail is still 116 px, not the 36 px measured at
  1920. This is also what makes the Paint strip 534 px wide at 1600 (6 rows) instead of ~660.
- Evidence: `02-1920-toolbar-collapsed.png`, `31-1600-brush.png`, `52-1024-brush.png`,
  `53-1024-toolbar-collapsed.png`; matrix.
- Undo: n/a.

### F7. Settings pages are fine at 1920, but the Grid page alone is 4 rows and grows to 11 rows at 1280 — warning
- Steps: Settings at 1920/1600/1366/1280; visit all six pages.
- Expected: page content scrolls inside a capped strip; the page tabs and every field remain
  reachable at 1024x600.
- Actual: at 1920 all six tabs fit (x 575–1013) and no page clips (105 px except Grid 140). At
  1024 the tabs also fit (x 427–865 inside a 630 px strip) and no controls are clipped, but the
  Grid page makes the strip 350 px and the view 630x39; at 1280 the same page makes the strip
  385 px and the view 470x39. The Storage page's buttons were reachable at every size. The tool
  title is vertically centered rather than top-left: at 1920 Grid the tabs are at y=663 and the
  title at y=715; at Storage the title is at y=733 — it wastes the top-left cell and looks
  misaligned (`09`, `23`, `56`).
- Evidence: `09-1920-settings-grid.png` … `14-1920-settings-storage.png`, `34-1600-settings-grid.png`,
  `42-1366-settings.png`, `48-1280-settings.png`, `55-1024-settings-grid.png`,
  `56-1024-settings-storage.png`.
- Undo: n/a.

### F8. Save Preset cannot shrink, has no Cancel, and forgets its size — minor
- Steps: Paint → Export Texture Preset; request 400x150; request 800x220; Esc; reopen.
- Expected: a compact 400–450 px dialog with OK and Cancel; the last size remembered.
- Actual: opens **558x150**; a 400x150 request is clamped to 558x150 although the window reports
  `min [0,0]`; 800x220 is accepted and leaves blank space; reopen returns to 558x150. Only an OK
  button exists (Esc/X close it). At 1024 the default 558x150 still fits.
- Evidence: `20-1920-save-preset-dialog.png`, `21-1920-save-preset-narrow-request.png`,
  `22-1920-save-preset-large.png`.
- Undo: n/a.

### F9. File dialog's reported minimum is not its real minimum — ui
- Steps: Storage → Import at 1920/1024; request 400x200.
- Expected: either honour 400x200 or report the actual minimum.
- Actual: reports `min [200,70]` but clamps 400x200 to **498x200**; defaults are 1050x700 at 1920
  and 819x480 at 1024 (both fit their screen). Not a blocker, but the min size is misleading.
- Evidence: `24-1920-import-dialog.png`, `25-1920-import-dialog-narrow.png`,
  `57-1024-import-dialog.png`.
- Undo: n/a.

### F10. The 39 px viewport is not practically editable; a brush drag at 1024 produced no edit — error (consequence of F3)
- Steps: 1024x600, Brush (Drag), drag 600,170 → 660,185 and click 600,175.
- Expected: a selection stroke (or at least a registered stroke) and an undo entry.
- Actual: terrain digest `3064656926` before and after, `undo_count: 0` both times. (The same
  viewport slice may simply show water at that point; either way there is no usable feedback at
  39 px.) At 1366 a Level drag in the 553x159 view did edit: digest `3064656926 → 1921710316`,
  action `terrain level`, and Ctrl+Z restored `3064656926`.
- Evidence: `52-1024-brush.png`, `62-1024-chunks.png`; undo state printed above.
- Undo: at 1366 works. Resizing itself created **no** undo entries and did **not** clear history:
  after the stroke (`undo_count: 1`) a 1366→1600 resize kept `action: terrain level`, and Ctrl+Z
  still restored the original digest.

## Keyboard / focus / hit targets (measured/observed)
- No Tab order or focus-ring test was completed (time). The strips use real Controls and should
  tab; the Foliage palette does not appear as Controls at all, so it cannot be part of a tab order.
- Toolbar buttons: terrain expanded 116x32 (label + key), compact 36x32, Pixelizer rail 30x30,
  Builder mode 34x34, strip rows 35 px, panel labels 74 px. These are mouse-sized, not touch-sized.
  All sizes are defined in `editor/ui_kit.rs` (`TOOL_BUTTON_SIZE = 30`, `ROW_HEIGHT = 35`,
  `PANEL_WIDTH = 250`, `FORM_LABEL_WIDTH = 74`) and passed through `sz()`/`px()`, which multiply by
  `EditorInterface.get_editor_scale()`; so they do scale with editor_scale (scale was 1.0 here).
  The layout failure will scale with them (a 250 px panel becomes 500 px at scale 2).
- Tooltips: the Collapse button's tooltip was read directly ("Show / hide the tool names (icons
  only)."); small-size tooltip behaviour was not re-tested.
- Popups: the Pixel World menu at 1920 is 203x648 at (1223,115), i.e. it fits 1080p with 317 px to
  spare, but it cannot fit a 600 px window without scrolling/flipping; small-size popups were not
  opened.

## Resizability policy (opinion)

| Surface | Should be | Min / max | Default | Remember | Collapse |
|---|---|---|---|---|---|
| 3D viewport | protected | **>=480x300**, ideally 640x360 | remaining space | n/a | n/a |
| Attribute strip | resizable (VSplit) | 35 / 175 px and <=35% of view | 3 rows (105) | per project + tool | chevron to title row |
| Settings page content | **not** strip-growing | 1 / 3 rows, then internal page scroll or sub-tabs | 3 rows | per project | tabs stay 1 row |
| Terrain toolbar | **fixed rail** | 30–36 px wide; vertical scroll | 36 px icon rail at <1400 px, labels at >=1400 | global | Collapse = 36 px, not 76/116 |
| Paint / Foliage / Pixel World / Materials | resizable (HSplit) | 220 / 420 px | 250 | per project | 24 px tab |
| Panel content | must scroll with a visible bar and keyboard/wheel reach | first action within 1–2 screens | top | n/a | n/a |
| Builder Face/Edge/Vertex strip | resizable + grouped | 3 rows + scroll, max 240 | 3 rows | per project | section tabs/popovers |
| Save Preset | resizable | 420x150 / 900x260 | 420x150 | last size | needs Cancel |
| Preferences | resizable + scroll, clamp to screen−80 | 480x400 | 484x600 | last size | n/a |
| File dialogs | leave to Godot | — | Godot default | global | n/a |
| Popups | fixed, viewport-clamped, scroll beyond 12 rows | — | — | n/a | n/a |
| Toolbar icons / row heights / pixel grid | fixed (scaled by editor_scale) | 30 px icon, 35 px row | — | n/a | n/a |

Non-negotiable: the addon must auto-collapse (panel → tab, strip → scroll, toolbar → rail) before
the 3D view falls below 480x300. Today every size below 1920 violates that, and 1600 violates it
for Paint (534x256), Settings (291) and Pixelizer (293); Builder Face violates it even at 1600.

## Priority fixes (opinion)

1. **Stop the two hard crashes.** (a) Builder `on_button_hover` must not re-enter `bind_mut` during
   selection/`mouse_exited` (F1); (b) Pixelizer's 670 px minimum must scroll/overflow instead of
   looping at 1280 (F2). Both are data-loss-grade: the editor exits.
2. **Cap the strip height and add a real scroll** (F3). This one change restores 1280/1024 and
   Paint/Settings at 1366; cap at 105–175 px, never let flow content set the VSplit height.
3. **Group the Builder Face/Edge/Vertex strips into tabs/popovers** and cap at ~240 px (F4);
   the current 447 px strip is unusable at 1600.
4. **Side-panel splitter + collapse + fast scrolling** (F5). Paint's 5767 px content / 90 wheel
   notches is the worst example; Foliage should become a real scrollable Control.
5. **Fix the toolbar adaptation** (F6): one fixed icon rail with vertical scroll; compact = 36 px;
   never 76/116 px or 2→3 column jumps while the strip changes height.
6. **Settings page scroll + title alignment** (F7): keep tabs on row 1, put the title top-left,
   scroll page content inside 3 rows.
7. **Dialog polish** (F8/F9): real Cancel, remember size, honest minimums.

## Covered

- [x] 1920x1080: terrain toolbar expanded + collapsed; Brush, Level, Smooth, Bridge, Paint
      (+panel scrolled to bottom), Foliage, Chunks, all six Settings pages, Stamp, Gradient,
      Select, Tint; Pixelizer Renderer + PixelSky + Pixel World panel; Builder Object + Face;
      Save Preset (open/narrow/large/reopen), Import dialog (open/narrow); clamp test not needed.
- [x] 1600x900: window shot, Brush, Paint (+panel), Foliage, Settings Grid, Pixelizer + panel,
      Builder Object + Face.
- [x] 1366x768: window shot, Brush, Level, Paint, Settings, Pixelizer; Builder selection attempted
      (crash captured in log).
- [x] 1280x720: window shot, Brush, Paint, Settings; Pixelizer attempted (crash).
- [x] 1024x600: window shot, Brush expanded + compact, Paint, Settings Grid + Storage, Import
      dialog, clamp 800x600, Level, Smooth, Tint, Chunks; undo attempt in the 39 px view.
- [x] Undo spot-check: Level stroke at 1366 → Ctrl+Z restored; redo → resize 1600 → history
      intact → Ctrl+Z restored. Resizing created no undo entries.
- [x] No addon `ERROR`/`SCRIPT ERROR` lines during normal (non-crash) operation; only the two
      crashes above.
- [x] `editor_scale` audit: the 30/35/250/74 px constants are multiplied by `editor_scale()` via
      `ui_kit::sz`/`px`; they will scale (not measured at scale != 1).

## Not tested / blocked

- Pixelizer, Pixel World, and Builder at 1024x600 (Pixelizer crashed at 1280; Builder crashed at
  1366; not retried to protect the remaining 1024 evidence). Builder Edge/Vertex at all sizes.
- Preferences dialog (Builder menu), Pixel World → Add/Features/lighting-popup at 1024, Builder
  menu popup at 1024, tooltips at small sizes, popup flip/scroll at 600 height.
- Tab order / focus ring / keyboard-only operation through strips and panels; Escape behaviour
  beyond the Save Preset/Import dialogs (both closed with Esc).
- HiDPI (`editor_scale != 1`), editor settings display scale, dark/light theme differences.
- Layout with the Output dock closed; all numbers above include the 224 px Output dock.
- Foliage panel at 1366/1280/1024; Paint panel scroll-rate at 1600/1024; drag/undo in the 39 px
  viewport (two attempts produced no edit; recorded as F10).
- 800x600 (clamped by Godot to 1024x600; not forced).
