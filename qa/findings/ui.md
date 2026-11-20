# UI / resizability audit — terrain toolbar, strips, panels, pixelizer, builder, dialogs

Session `uiresize` (display `:34`, port `8814`), Godot 4.7.2, llvmpipe, scene
`res://demo/hex_island.tscn`. All coordinates are window-content pixels; the Xvfb screen was
1920x1080 and the editor was resized with `drive.py resize`. 51 screenshots and 11 UI JSON
dumps live in `qa/screenshots/uiresize/` and `qa/screenshots/uiresize/ui/`; screenshot names in
this file are relative to `qa/screenshots/uiresize/`. The editor log is
`/tmp/opencode/qa/sessions/uiresize/editor.log`.

Method note: immediately after a `resize`, the first `ui` dump can still report the previous
split (for example a strip at y=483 with the 787 px width of 1600x900 while the capture already
shows the 1024 layout). Rect numbers below are taken from dumps that match the capture, or from
the capture's measured geometry; treat the PNG as authoritative when the two disagree.

The editor accepts **1024x600 as its hard minimum**: `resize 800 600` is clamped to 1024x600
(`07-brush-request-800x600-clamped-1024x600.png`, `info` reports `window_size [1024,600]`).
800x600 is therefore out of scope; do not force it (an 800x600 attempt destabilised the
layout). Restored editor layout is also a hazard: after a crash the saved layout at
1024x600 can make the next launch start with the same queue-OOM loop; each clean run below
started with the session's `home/` removed.

## a) Surface × size matrix

Values are the **3D viewport size** unless noted; “strip” is the bottom attribute strip size
(`rect` from `ui`, `min` in parentheses). `n/t` = not tested at that size.

| Surface | 1920x1080 | 1600x900 | 1366x768 | 1280x800 | 1024x600 | Verdict |
|---|---|---|---|---|---|---|
| Terrain toolbar, expanded | 116x682, 1 col; all 16 buttons reachable | 236x469, 2 cols; all reachable | 236x337, 2 cols | 236x369, 2 cols | 236x287, 2 cols (3 cols with Paint, ~320 wide) | Works but eats height/width; no scroll, no compact default |
| Brush strip | 1227x574 view / strip 1227x105 | 787x326 / 787x140 | 553x159 / 553x175 | 470x121 / 470x245 | **470x39** / 470x245 | Fails below 1600; unusable at 1024 |
| Level strip | n/t | 787x~326 view; strip visible, 4 rows | n/t | n/t | **editor crash** on resize | Error at 1024 |
| Smooth strip | n/t | 1600 capture only | n/t | n/t | **editor crash** on resize | Error at 1024 |
| Bridge / Foliage / Inspect / Chunks / Stamp / Gradient / Select / Tint strips | n/t | captured at 1600 (`45-*`); Tint reachable at (465,170) | n/t | n/t | Tint **editor crash**; others not reached | Small-screen coverage blocked by crash |
| Paint strip + Textures panel | n/t | viewport ~662x320; panel 250x469 shows 2.5/16 slots; no scrollbar | n/t | n/t | viewport **x39**; strip 10+ rows clipped; panel 250x~400 | Fails; scroll is wheel-only and slow |
| Foliage panel | n/t | 250 px panel, same shell as Paint (`45-foliage-1600x900.png`) | n/t | n/t | not reached (tool switch risk) | Likely same as Paint |
| Settings (6 pages) | n/t | captured (Grid) | n/t | n/t | viewport **x39**; 6 page tabs fit x=426..854; fields clipped; Storage leaves ~200 px empty strip below | Tabs OK, content fails |
| Pixelizer rail + strip + Pixel World panel | n/t | viewport 662x320, panel 250 visible at right | n/t | n/t | rail 152x180 (4 cols, 15 icons); viewport **670x39**; strip 670x138; tabs x=334..1004; panel 250 starts x=**1007** → 17 px visible | Viewport and panel fail at 1024 |
| Builder MST — Object | n/t | viewport 421x310; strip 421x156; Materials 259x469 | n/t | n/t | face 238x39; object strip clipped | Object usable at 1600 only |
| Builder MST — Face/Edge/Vertex | n/t | **viewport 345x39**; strip 345x447 (min 238x447) | n/t | **238x39**; strip 238x699 | **238x39**; strip 238x699; Materials 245x741 clipped by window | Broken at every size tested |
| Builder Modeler layout | n/t | captured `25-builder-modeler-1600x900.png` | n/t | n/t | top toolbar + viewport only; bottom strip and Materials panel absent | Dense but usable; palette/strip lost |
| Dialogs — Save Preset | n/t | n/t | n/t | n/t | 558x150 fits; 400-wide resize clamped to 558; 800x220 accepted | Fits, but cannot shrink; OK only |
| Dialogs — Preferences | n/t | n/t | n/t | n/t | 484x664 at (270,-3), **non-resizable**, bottom 61 px off-screen | Fails at 600 height |
| Dialogs — Import file | n/t | n/t | n/t | n/t | 819x480 fits; resize 400x200 clamps to 498x200 | Fine; min size misleading |
| Popups — Brush Type | n/t | n/t | n/t | n/t | 154x156 at (621,231), fits below button | Fine |
| Popups — Builder menu | n/t | n/t | n/t | n/t | 252x372 at (517,148), 16 rows, ends y=520 | Fits at 1024; tight |
| HUD / hints | n/t | normal | n/t | n/t | covers/clips the 39 px viewport sliver | Fails with the viewport |

Measured strip/viewport pairs (Brush): at 787 px strip width the flow is 4 rows / 140 px; at
553 px it is 5 rows / 175 px; at 470 px it is 7 rows / **245 px**. The strip has a
`ScrollContainer` but its minimum height is the whole flow height, so the VSplit honours 245 px
and clamps the viewport to its 39 px minimum. This is the root cause of the 1280/1024 failure,
not the window height per se: 1280x800 is worse than 1366x768.

## b) Defects

### D1. Switching to Level/Smooth/Tint at 1024x600 crashes the editor (queue OOM) — error
- Steps: 1) Open hex_island, select the terrain, `resize 1024 600`, wait. 2) Click `Level` in the
  toolbar (also reproduced by selecting Level at 1600 then resizing down, and with `Smooth` and
  `Tint`). 3) Observe the harness socket drop and the editor process exit.
- Expected: the strip reflows to 7 rows and stays interactive; the viewport may shrink but the
  editor must not die.
- Actual: the first runtime message after the resize is
  `Failed method: Container::_sort_children. Message queue out of memory...`, followed by
  hundreds of thousands of `Object was deleted while awaiting a callback.` lines; the editor is
  gone. This is a **container layout loop**, not a GPU/llvmpipe allocation: a FlowContainer /
  ScrollContainer reflow at 470 px never converges. Reproduced on 5 separate clean launches
  (Level twice, Smooth once, Tint once, Level after a pre-select at 1600); the first crash
  followed an 800x600 request that the editor already clamped, but the later four ran from a
  clean `home/` at 1024x600. Brush, Paint, Settings, Pixelizer and Builder did **not** crash at
  the same size, so it is strip-content-triggered.
- Evidence: `/tmp/opencode/qa/sessions/uiresize/editor.log` (last crash: 105,978 lines / 4.9 MB,
  OOM at line 14 after a clean 13-line startup; an earlier crash log grew to 192 MB / 4.1 M
  lines); screenshots before the crash `09-brush-1600x900-clean.png`,
  `10b-level-1600x900.png`; no screenshot after (process gone). This is the highest-priority
  defect: the addon must survive a tool switch at its own supported minimum window.
- Undo: not applicable.

### D2. Builder Face mode squeezes the viewport to 39 px at 1600 — error
- Steps: select `BuilderRuin/Ruin`; stay in MST layout; click the `Face mode [2]` button.
- Expected: the face strip scrolls/collapses and leaves a usable 3D view (at least ~300 px tall).
- Actual: `PixelBuilderStrip` is **345x447** (min **238x447**) and not scrollable; the viewport is
  **345x39**. At 1280x800 the strip wraps to **238x699** (viewport 238x39) and at 1024x600 to
  **238x699** with the bottom rows clipped by the window. Clicking faces is effectively
  impossible; the HUD is clipped over the sliver.
- Evidence: `21-builder-face-1600x900.png`, `22-builder-face-1280x800.png`,
  `23-builder-face-1024x600.png`, `ui/21-…json`, `ui/22-…json`, `ui/23-…json`.
- Undo: not applicable.

### D3. Terrain attribute strips have no height cap and no usable scroll — error
- Steps: select the terrain; Brush; resize 1920 → 1600 → 1366 → 1280 → 1024.
- Expected: the strip keeps a bounded height (or scrolls internally) and the viewport keeps a
  usable size.
- Actual: viewport heights 574 → 326 → 159 → **121** → **39** px; strips 105 → 140 → 175 → 245 →
  245. At 1024 the strip is given 245 px and the viewport is clamped to 39 px. The strip's
  ScrollContainer does not show a vertical scrollbar because its min height equals its content;
  at still narrower effective widths the same reflow loop produces the
  `Container::_sort_children` queue OOM of D1. The scene cannot be sculpted at 1024.
- Evidence: `01-brush-1920x1080.png`, `09-brush-1600x900-clean.png`, `02-brush-1366x768.png`,
  `03-brush-1280x800.png`, `04-brush-1024x600.png`, UI dumps for the same names.
- Undo: not applicable.

### D4. Paint/Foliage side panel is fixed at 250 px, has no visible scrollbar, and the bottom action is far away — error
- Steps: Paint tool at 1600x900. Try to resize the right “Textures” panel; wheel over it; look for
  “Export Texture Preset”.
- Expected: a draggable splitter (220–420 px), a visible scrollbar, and the export action reachable
  without dozens of wheel turns.
- Actual: fixed 250 px, no splitter, no visible scrollbar. The panel shows only **2.5 of 16**
  slots at 1600; 40 wheel ticks only advanced about 7 slots (≈6 ticks per slot), so the bottom
  Export button is ~100 ticks away. At 1024 the panel is 250 px over a 39 px viewport, slots are
  cut at the window bottom, and the Paint strip itself is clipped after “Slope Min/Max”.
- Evidence: `40-paint-1600x900.png`, `41-paint-scrolled-1600x900.png`,
  `42-paint-bottom-1600x900.png`, `43-paint-1024x600.png`.
- Undo: not applicable.

### D5. Pixel World panel is off-screen at 1024x600 — error
- Steps: select `PixelRenderer`; resize 1024x600.
- Expected: the Pixel World panel remains fully visible (or collapses to a tab).
- Actual: `PixelizerAttributes`/`PixelizerToolbar` consume the width; the `PixelWorldPanel` is
  placed at x=**1007** with width 250, so only 17 px is inside the 1024 window. All “Select” /
  “Add” buttons are unreachable. The viewport is 670x39. The nine page tabs happen to fit
  (last “Debug Views” ends at x=1004), but with no margin.
- Evidence: `31-pixelizer-1024x600.png`, `ui/31-pixelizer-1024x600.json`, section pages
  `32`–`35`.
- Undo: not applicable.

### D6. Settings page tabs fit, but the page content and hint do not — warning
- Steps: Settings; visit Grid, New Chunks, Grass, Display, LOD & Nature, Storage at 1024x600.
- Expected: all six tabs and every field reachable; the strip scrolls; the viewport hint readable.
- Actual: six tabs fit (x=426..854). Fields below the first ~9 rows are clipped with no visible
  scrollbar; the strip hint is cut at the left (“…ings: edit them in the strip below”) and
  overlapped by the “Perspective” label; the 39 px viewport persists even on Storage, which only
  needs 4 rows, because the split ratio is not returned.
- Evidence: `60-settings-1024x600.png`, `61-settings-*.png` (six pages), `45-settings-1600x900.png`.
- Undo: not applicable.

### D7. Terrain toolbar silently grows into 2–3 columns and steals viewport width — warning
- Steps: at 1024x600 compare the Brush layout (2 columns, 236 px) with Paint/Settings (3 columns,
  ~320 px); observe the 3D view start x.
- Expected: a fixed 30 px icon rail, optionally scrollable; compact mode should be explicit.
- Actual: the toolbar is a VFlowContainer that wraps based on available height; it expands
  horizontally to keep all 16 tools visible. At 1024 Paint the viewport starts at x≈507 rather
  than 418. Tint/Grid remain reachable in the Brush layout, so the toolbar itself is complete, but
  the cost is 80–200 px of viewport width and no “more” affordance/scrollbar.
- Evidence: `04-brush-1024x600.png` (2 cols), `43-paint-1024x600.png` (3 cols),
  `60-settings-1024x600.png` (3 cols), `00-terrain-toolbar-1600x900.png` (2 cols at 1600).
- Undo: not applicable.

### D8. Pixel Builder Preferences is taller than a 600 px window and non-resizable — warning
- Steps: select BuilderRuin/Ruin; Builder menu → Preferences…; resize to 1024x600.
- Expected: dialog fits, scrolls, or is resizable; OK/Close reachable.
- Actual: window `Pixel Builder Preferences` is 484x664 at (270,−3), `resizable: false`,
  `min [0,0]`; the bottom 61 px (including the action row) is off-screen and there is no scroll.
- Evidence: `24-preferences-dialog-1024x600.png`.
- Undo: not applicable.

### D9. Save Preset dialog cannot be narrowed and has no Cancel — minor
- Steps: Paint → Export Texture Preset at 1024; `window-resize 400 150 --title "Save Preset"`.
- Expected: a compact dialog (e.g. 420x150) with OK **and** Cancel.
- Actual: window reports `min [0,0]` but the requested 400x150 is clamped to **558x150** because
  the one-line label (“Preset name (saved to res://addons/…/)”) has min width 540. Enlarging to
  800x220 works but leaves a large blank area. Only an **OK** button exists (the X title-bar
  button closes it); Esc closes it.
- Evidence: `50-save-preset-1024x600.png`, `51-save-preset-narrow-400x150.png` (actually 558 wide),
  `52-save-preset-large-800x220.png`, `ui/50-…json`.
- Undo: not applicable.

### D10. File-dialog minimum size is not what the window reports — ui
- Steps: Storage → Import… at 1024; `window-resize 400 200 --title "Open a File or Directory"`.
- Expected: either allow 400x200 or report the real minimum.
- Actual: `min [200,70]` but the request clamps to **498x200** (content minimum); the file list and
  path bar are cramped but the action row stays visible. At the default 819x480 it fits 1024x600.
- Evidence: `70-import-dialog-1024x600.png`, `71-import-dialog-narrow-400x200.png`.
- Undo: not applicable.

### D11. Modeler layout at 1024 drops the bottom strip and Materials palette — ui
- Steps: Builder menu → Modeler layout at 1024x600.
- Expected (docs/Area 6): the larger palette/Add Mesh grid remains usable alongside the viewport.
- Actual: only a dense top toolbar (Add Mesh, Actions, icons) and the viewport remain; the bottom
  strip and Materials panel are gone, and the viewport is still halved by the Output panel.
  At 1600 the same layout looks correct (`25-builder-modeler-1600x900.png`).
- Evidence: `24c-builder-modeler-1024x600.png`, `25-builder-modeler-1600x900.png`.
- Undo: not applicable.

### D12. HUD/hint text is drawn over the viewport sliver and clipped — warning
- Steps: 1024x600: Brush (3-line hint), Settings (hint), Builder Face (“Pixel Builder - Face
  mode …”), Pixelizer (Perspective label).
- Expected: hints wrap in a readable area or move to a status bar when the viewport is short.
- Actual: the Brush hint occupies more vertical space than the entire 39 px viewport; the Settings
  hint is cut on the left and overlapped by `Perspective`; the Builder HUD is clipped to one/two
  lines. Text remains legible only because it spills over the 3D area.
- Evidence: `04-brush-1024x600.png`, `60-settings-1024x600.png`, `23-builder-face-1024x600.png`,
  `31-pixelizer-1024x600.png`.

### D13. Right inspector collapses to ~126 px at 1024 (editor-level, context)
- At 1024x600 the terrain/Paint layouts leave the Inspector only ~126 px; labels are cut
  (“MarchingSqua…”, “Conten…”). This is Godot's dock minimum, not the addon, but it means the
  addon cannot rely on Inspector fallbacks at 1024. Evidence: `43-paint-1024x600.png`,
  `60-settings-1024x600.png`.

## c) Resizability policy (opinionated)

| Surface | Resizable? | Suggested min / max | Default | Persistence | Collapse behaviour |
|---|---|---|---|---|---|
| Editor window | yes | hard 1024x600; support target 1280x720; max screen | 1600x900 | global `EditorSettings` | n/a |
| Terrain toolbar | **no width drag**; fixed 30 px icon rail | 30x30 icons; max 2 columns; vertical scrollbar | 1 column, icon-only at <1400 px | global (compact on/off) | `Collapse` = icon-only 30 px; remember |
| Attribute strips | **yes**, vertical splitter | min 1 row / 35 px; max 240 px **and 35% of the 3D area** | 3 rows; Face mode 3 rows + scroll | per-project + per-tool | collapse to title row; “More…” popover for advanced fields |
| Paint/Foliage/Pixel World/Materials side panels | **yes**, horizontal splitter | 220 / 420 px | 250 px | per project | collapse to a 24 px tab; no horizontal scrollbar |
| Strip flow | no free wrap beyond 3 rows | wrap 1–3 rows; then scroll vertically with a **visible** scrollbar | 2–3 rows | per project | “More” groups fields into popovers |
| Pixelizer section rail | no width drag | 30 px; 2 columns max; vertical scroll | 2 columns | global | icon-only always |
| Pixelizer page tabs row | no resize | horizontal scroll or `»` overflow menu | all 9 | n/a | overflow menu below 1024 |
| Builder Face strip | yes, scroll | max 240 px; groups in tabs (Transform / Inset / Trim / Offsets) | 3 rows | per project | “Advanced” popover |
| Builder Materials palette | yes, splitter | 220 / 360 px | 259 px | per project | collapse |
| Save Preset | yes | 420x150 / 900x260; content scroll if narrower | 558x150 | last size | needs a real Cancel button |
| Preferences | yes + scroll | 480x400 / screen−80 | 484x600 | last size | scroll |
| Import/Export file dialogs | yes (Godot default) | 480x320 / 1600x1000 | 819x480 | global | n/a |
| Popups | no | viewport-clamped; flip above the anchor near the bottom; scroll for >12 items | n/a | n/a | submenu scrolls |

Non-negotiable rule: the 3D viewport must keep **at least 480x300** (and ideally 640x360). When a
layout would violate it, auto-collapse in this order: (1) side panel → tab, (2) strip → 2 rows +
“More”, (3) toolbar → icon rail. The current layouts violate this at 1280x800 Brush (470x121),
at 1024 Brush (470x39), and at every Face mode.

Side panels should use `HSplitContainer` with a visible dragger and store the split in project
metadata (`ProjectSettings` or the addon's editor config) so each project keeps its panel width;
the window size and dialog sizes belong in `EditorSettings` (global). Side panels must never be
fixed 250 px; 220 px is enough for the slot thumbnails at this scale.

The strip must have a **fixed max height**, not just a ScrollContainer: a ScrollContainer that
reports its content min height is what causes the 39 px viewport. Either set
`custom_minimum_size` and enable `vertical_scroll_mode`, or put the strip in a `VSplitContainer`
with a max-size clamp; add a visible scrollbar and a “collapse strip” chevron.

## d) Small-screen priority fixes (in order)

1. **Fix the 1024 crash (D1)** — reproduce with Level/Smooth/Tint; likely a layout/redraw loop in
   the reflow at 470 px width. Until fixed, 1024x600 is not a supported editor size.
2. **Cap the strip height + scroll (D3)** — the single change that restores 1280/1366/1024
   (121→≥300 px and 39→≥300 px viewport) for every terrain tool.
3. **Rebuild Face/Edge/Vertex strips (D2)** — group fields into tabs/popovers and cap at 240 px;
   Face mode must not take 447–699 px at any window size.
4. **Side-panel splitter + collapse + visible scrollbar (D4)** — especially Paint/Foliage at 1600.
5. **Keep Pixel World on screen at 1024 (D5)** — collapse the panel to a tab when the center width
   would drop below 640 px; page tabs need an overflow menu.
6. **Settings page scroll + hint fit (D6)**; return space to the viewport when switching to a
   short page (Storage).
7. **Toolbar rail (D7)**: never wrap past 2 columns; use a scrollbar and compact icons.
8. **Preferences dialog (D8)**: make it resizable and scrollable, clamp to screen−80.
9. **HUD/hint policy (D12)**: move hints to a one-line status bar when the viewport is under
   ~200 px tall; never draw a 3-line hint over a 39 px view.
10. **Save Preset width / Cancel (D9)** and the file-dialog minimum (D10) — polish.

## Covered

- [x] Editor min size: 800x600 request clamps to 1024x600; recorded (`07-…clamped…`, `info`).
- [x] Terrain toolbar at 1920/1600/1366/1280/1024; all 16 buttons reachable at 1024 in Brush
      layout (2 columns); Tint/Grid on the second column at 1600; `Collapse` present.
- [x] Brush strip measured at all five sizes; viewport numbers above.
- [x] Level captured at 1600; Smooth/Bridge/Foliage/Paint/Inspect/Chunks/Settings/Stamp/Gradient/
      Select/Tint each captured at 1600 (`45-*`).
- [x] Paint panel scrolling (1 → 10 → bottom), Export Texture Preset at the bottom; panel fixed
      250, no visible scrollbar.
- [x] Settings: all six pages captured at 1024; tabs fit; Storage/Import dialog opened.
- [x] Pixelizer at 1600 and 1024; Renderer/Outlines/Color/Sheets/Debug pages; 4-column rail at
      1024 with all 15 section icons; page tabs all fit (barely).
- [x] Builder MST Object 1600; Face 1600/1280/1024; Modeler 1024 and 1600; Builder menu popup at
      1024 (16 rows, fits).
- [x] Save Preset dialog: open, narrow request, enlarge, Esc close; Import file dialog: open,
      narrow request, Esc close.
- [x] Brush Type popup at 1024 (154x156 under the button, 6 items) and Esc close.
- [x] Esc closes the Builder menu, Save Preset, Preferences and Import dialogs.
- [x] No addon `ERROR`/`SCRIPT ERROR` lines during normal (non-crash) operation; the only fatal
      log is the queue-OOM crash in D1.

## Untested / blocked

- 1920, 1366, 1280 for every tool except Brush (window time lost to crash restarts).
- Foliage/Inspect/Chunks/Stamp/Gradient/Select at 1024; Edge/Vertex mode at all sizes; Level/
  Smooth at 1280/1366 (crash blocked the 1024 pass).
- Overwrite confirmation for Save Preset (would need a real save; not exercised).
- Settings → Storage → Bake/Export dialogs and the main Settings → Import Data dialog.
- Material/Stamp-image OptionButtons with long item lists at the bottom edge; submenu flipping.
- Tab order/focus ring through strips and panels; tooltips at small sizes; HiDPI/Editor Display
  Scale (editor_scale was 1.0; all sizes above are physical pixels). Hard-coded 30/35 px rows and
  the 250 px panel will not scale with `editor_scale`.
- The 800x600 window itself (clamped by Godot); do not force it.
