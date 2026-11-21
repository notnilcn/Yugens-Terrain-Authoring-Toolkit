# Area 7 + 9 — UI / resizability sweep + runtime (r5)

Session `r5-t5` (display `:55`, port `8855`, window 1600×900 at boot) and runtime
`r5-t5play` (display `:56`), Godot 4.7.2, llvmpipe, editor scale 1.0, scene
`res://demo/hex_island.tscn`. Build: `r4base` (HEAD `ec39910`, includes the r5 layout work
`cd5f077`). Every number below is read back from `qa/drive.py ui --root base --save` dumps; the
compact per-surface dumps are in `qa/screenshots/r5-ui-runtime/ui/*.json` and the flattened table
is `qa/screenshots/r5-ui-runtime/matrix.csv` (111 rows; regenerate with
`qa/scripts/r5-ui-runtime/matrix.py`).

Both sessions were stopped at the end. The runtime log for all five scenes contains only the two
known llvmpipe/XTEST warnings; no `ERROR`/`SCRIPT ERROR` from the addon. The editor log (copied to
`qa/screenshots/r5-ui-runtime/editor.log.txt`) has three `ERROR:` lines: two Godot-internal
`create_frustum_points` errors while a Pixel* node is selected (N6) and one from the Save Preset
dialog's metadata probe (N5).

## Headline

1. **The Pixelizer editor UI is completely dead in this build** — selecting any `Pixel*` node
   leaves the rail, the bottom strip and the Pixel World panel at their *minimum* size (rail
   `39×0`, strip `0×175`, panel `250×0`). It is silent, deterministic, from a fresh session, at
   all five sizes and after opening a second scene. The 3D view simply fills the screen. (N1)
2. **The strip cap works.** No surface squeezes the 3D view below ~300 px any more (worst case
   297 px at 1024×600, i.e. a 3 px miss), and every oversized strip now scrolls internally with a
   real scrollbar (proved by `scroll_v 0→49`). r4 #7 and #8 are fixed.
3. **The new side panel is a genuine improvement** — drag-grip 220–420 px, 30 % window clamp,
   collapse to a 24 px tab, expand restores the last width, auto-collapsed at 1024 for a
   never-toggled panel — but the width/collapse state is **not remembered across an editor
   restart**, and at 1024 an expanded panel leaves a 258×297 px 3D view. (N4, N7)
4. **The runtime is in good shape**: F1 toggles, orbit cannot go under the island, the sea now
   reaches the horizon, tutorials 01–04 all load clean. F3 still does not fit 1024×600 / 800×480
   and cannot be collapsed (r4 #39 still reproduces); F1 help still has no backing panel (N2, N3).

## Findings

### N1. Selecting any Pixel* node leaves the Pixelizer UI at zero size — critical
- **Severity:** critical (all pixel-authoring UI unreachable; no crash, no log error)
- **Steps:** 1) Fresh session, 1600×900, `hex_island`. 2) `select --path PixelRenderer`
  (also `PixelSky`; also after `open-scene res://tutorial/03_pixel_look.tscn`).
  3) `ui --root base --save`.
- **Expected:** the pixelizer toolbar rail on the left, the attribute strip across the bottom,
  the Pixel World side panel on the right — as in r4 (`rail 35×682`, `strip 1055×105`,
  `panel 250×682`).
- **Actual:** the three containers are never laid out by their `SplitContainer` parents; each
  keeps exactly its *minimum* size at the parent's origin:
  `PixelizerToolbarRail 287,121 39×0`, `PixelizerAttributes 287,121 0×175`,
  `PixelWorldPanel 287,121 250×0` (at 1920/1600/1366/1280; at 1024 the strip is `0×99` and the
  panel `24×33`). `Node3DEditorViewport` gets the whole area (`1026×732` at 1600). The children
  *inside* those containers are laid out correctly (e.g. `PixelizerToolbar 35×584`,
  `MstFlow 481×239`, `EditorSpinSlider "Pixel Size"` at `391,258`) — they are simply clipped by a
  0-size parent. A window resize does **not** re-sort them. Nothing is logged: the r4 layout-loop
  crash is gone, and so is the UI.
- **Evidence:** `fresh-1600-pixelizer.png` (Inspector shows PixelRenderer; no pixelizer UI),
  `1600-pixelizer-broken-rail0.png`, `1600x900-70-pixelizer-sweep.png` …
  `1024x600-70-pixelizer-sweep.png`, `pixelizer-sweep.jsonl`;
  dumps `ui/1600x900-70-pixelizer-sweep.json`, `ui/1920x1080-70-pixelizer-sweep.json`.
- **Undo:** n/a (layout). The node is still selectable and editable through the raw Inspector, so
  no data loss — but none of the tool's controls exist on screen.

### N2. F3 tuner still overflows 1024×600 and 800×480, with no way to collapse it — warning
- **Severity:** warning (fields unreachable at small sizes; r4 #39 still reproduces here)
- **Steps:** runtime `hex_island`; resize the game window to 1024×600, then 800×480; press F3;
  click the "Pixel tuner" header.
- **Expected (r5 claim):** "fits small game windows and can be collapsed".
- **Actual:** 1280×720 panel `417×628` at (860,80) — fits, and the player is clear of it.
  1024×600 panel `406×…` at (610,80), bottom clipped mid-row (`Weather Clear` is the last fully
  visible row; Transition/Ground/Strength are below the window). 800×480 panel `410×…` at
  (380,80) covers ~50 % of the window width and clips `PixelWeather: Weather`. Clicking the
  "Pixel tuner" title, the tab row or its header does **not** collapse it; only the individual
  section chevrons (# Features, # PixelWeather) collapse. No scrollbar on the panel.
- **Evidence:** `rt-04-hex-1280-f3.png`, `rt-05-hex-1024-f3.png`,
  `rt-06-hex-1024-f3-collapse-try.png`, `rt-07-hex-800x480-f3.png`.
- **Undo:** n/a.

### N3. F1 help is still an unbacked one-liner and is overdrawn by the F3 panel — minor
- **Severity:** minor/UI
- **Steps:** runtime `hex_island`; press F1; press F1 again; press F3.
- **Expected (r5 claim):** readable multi-line help on a backing panel, clear of the F3 dock.
- **Actual:** F1 toggles correctly (second press hides it — the r4 "one-shot" bug is fixed) and
  the line is fully inside the window, but it is white text directly over the grass with **no
  backing panel**, and the F3 tuner is drawn on top of it, cutting the line at
  "Wheel zoom · F3…" so the "F1 help" part disappears.
- **Evidence:** `rt-02-hex-f1-on.png` (hint on, no panel), `rt-03-hex-f1-off.png` (hidden),
  `rt-04-hex-1280-f3.png` (hint cut by the tuner).
- **Undo:** n/a.

### N4. Side-panel width and collapse state do not survive an editor restart — minor
- **Severity:** minor/UI (contradicts "remembered per panel and project")
- **Steps:** 1) Paint, 1600×900, drag the grip to ~405 px. 2) Confirm 405 px. 3) Clean quit
  (`Ctrl+Q`; the editor exits and saves). 4) Restart the session, select the terrain, open Paint.
- **Expected:** the panel reopens at 405 px (or at least remembers *something*).
- **Actual:** it reopens at the 250 px default. Nothing is written to the project:
  `grep panel_ project/project.godot` finds nothing after the clean quit. In-session persistence
  *does* work (switching Paint → Builder → Paint keeps 405), and the 24 px collapsed state also
  survives in-session.
- **Evidence:** `sp-1600-panel-min220.png`, `sp-1600-panel-collapsed.png`,
  `sp-1280x720-panel.png`, `sp-1024x600-panel-max-clamp.png`; measurements in the matrix below.
- **Undo:** n/a.

### N5. Save Preset logs a metadata `ERROR:` on first open — minor
- **Severity:** minor (log noise)
- **Steps:** Paint → Export Texture Preset in a fresh session; read `editor.log`.
- **Actual:** `ERROR: Couldn't find the given section "marching_squares_terrain" and key
  "save_preset_dialog_size", and no default was given.` The dialog itself then works and remembers
  its size *within* the session (400×179 reopened at 400×179).
- **Evidence:** `qa/screenshots/r5-ui-runtime/editor.log.txt` line 80.
- **Undo:** n/a.

### N6. `create_frustum_points` ERROR while a Pixel* node is selected — minor
- **Severity:** minor (log noise, Godot-internal)
- **Steps:** select `PixelRenderer` / `PixelSky` in `hex_island`; read `editor.log`.
- **Actual:** two
  `ERROR: Condition "!res" is true. Returning: false at create_frustum_points
  (servers/rendering/rendering_light_culler.cpp:682)` plus 34
  `Plane/Vector3 cannot be normalized` warnings from `PixelSky`'s light/camera setup. No addon
  `ERROR`/`SCRIPT ERROR`, and the editor survives.
- **Evidence:** `editor.log.txt` lines 12–28, 55–62.
- **Undo:** n/a.

### N7. No horizontal viewport floor: side panels squeeze the 3D view to 333–416 px — warning
- **Severity:** warning (layout; strip cap fixed the vertical axis only)
- **Steps:** compare Paint and Builder at 1280/1366/1024.
- **Actual:** Paint at 1280 → view **333×341**; Paint at 1366 → **416×389**; Builder at
  1280 → **358×341**; Builder at 1024 → **358×297**. With the panel expanded to 220 px at
  1024×600 the view is **258×297**; at the 30 % clamp (307 px) it is **276×297**. The vertical
  floor is honoured (≥300 except N8), but nothing stops the rail + panel from eating the width.
- **Evidence:** `1280x720-05-paint.png`, `1366x768-05-paint.png`, `1280x720-60-builder.png`,
  `sp-1024x600-panel-max-clamp.png`; matrix rows.
- **Undo:** n/a.

### N8. The 300 px view floor is missed by 3 px at 1024×600 — minor
- **Severity:** minor
- **Steps:** 1024×600, any terrain tool.
- **Actual:** the strip is set to 99 px (the r5 note says "400 → 100 (view 300)") but the view is
  **297 px** tall because the editor chrome above the 3D screen (menu + 3D toolbar + viewport
  menu bar) is 3 px taller than the arithmetic assumes. All nine terrain surfaces read 297.
- **Evidence:** matrix rows `1024x600-*`; `1024x600-01-brush.png`.
- **Undo:** n/a.

### N9. Save Preset request 400×150 is honoured in width but not height — minor
- **Severity:** minor/UI
- **Steps:** Paint → Export Texture Preset; `window-resize --title "Save Preset" 400 150`.
- **Expected:** 400×150 (reported min is `[340,140]`).
- **Actual:** **400×179** — the height refuses to go below the unsized-content height (179) even
  though the reported minimum is 140. Reopening returns 400×179, so the size *is* remembered.
- **Evidence:** `dlg-1600-save-preset.png`, `dlg-1600-save-preset-empty.png`,
  `dlg-1600-save-preset-empty-ok.png`.
- **Undo:** n/a.

### N10. Settings pages keep a 175 px strip even when the page needs 105 — minor
- **Severity:** minor/UI (dead space steals 70 px from the view at every size ≥1280)
- **Steps:** Settings → Display (flow 105) at 1600; compare with Bridge (strip 70).
- **Actual:** every Settings page reserves strip `175`; when the page content is only `105` tall
  (Display at 1600/1920, LOD at 1600/1920/1366) the extra 70 px stays empty instead of returning to
  the 3D view. Terrain tool strips *do* shrink (Bridge 70, Inspect 35), so this is Settings-only.
- **Evidence:** `1600x900-13-settings-display.png`; matrix rows `1600x900-09…15-settings-*`.
- **Undo:** n/a.

### N11. Builder palette and builder rail are not resizable/collapsible — minor
- **Severity:** minor/UI (inconsistent with the terrain Texture/Foliage panel)
- **Steps:** select `BuilderRuins/Ruin`; look for a grip or tab on `PixelBuilderPalette`
  (245–259 px) and on `PixelBuilderToolbarRail` (86 px).
- **Expected (r5 claim "resizable side panels"):** the same grip/collapse affordance.
- **Actual:** `PixelBuilderPalette` is a plain `ScrollContainer` (no `MstSidePanel`, no
  `ExpandTab`, no grip) and `PixelBuilderToolbarRail` is a `ScrollContainer` with its own
  vertical scrollbar. Only the terrain Texture/Foliage panel and (nominally) `PixelWorldPanel` are
  `MstSidePanel`s. The builder rail is at least a stable 86 px at every size (no column jump).
- **Evidence:** `1920x1080-60-builder.png`, `1024x600-60-builder.png`,
  `dlg-1024-builder-prefs.png`; dumps `ui/1024x600-60-builder.json`.
- **Undo:** n/a.

## Measured matrix

`view` = `Node3DEditorViewport`; `strip` = the bottom `ScrollContainer`
(`MarchingSquaresToolAttributes` / `PixelBuilderStripRoot`); `flow` = its `MstFlow` content (the
number that overflows — compare with `strip`); `rail` = `MarchingSquaresToolbarRail`; `panel` =
`MstSidePanel`. Sizes are px. The Output dock was open at its default height; the left/right docks
auto-shrink at ≤1366, which is why the view width drops faster than the window.

| Surface | 1920×1080 | 1600×900 | 1366×768 | 1280×720 | 1024×600 |
|---|---|---|---|---|---|
| Toolbar rail, expanded | 120×912 | 120×732 | 120×567 | 120×519 | 120×399 |
| Toolbar rail, compact | 40×912 | 40×732 | 40×567 | 40×519 | 40×399 |
| Toolbar flow (labelled) | 116×539 | 116×539 | 116×539 | 116×539 | 116×539 |
| Brush | view 1223×804; strip 105 | 903×624; 105 | 669×424; 140 | 586×341; 175 | 586×297; **99** (scroll) |
| Level | 1223×839; 70 | 903×624; 105 | 669×424; 140 | 586×376; 140 | 586×297; 99 (scroll) |
| Smooth | 1223×839; 70 | 903×624; 105 | 669×424; 140 | 586×341; 175 | 586×297; 99 (scroll) |
| Bridge | 1223×839; 70 | 903×659; 70 | 669×459; 105 | 586×411; 105 | 586×297; 99 (scroll) |
| Paint (panel open) | 970×804; 105; panel 250 | 650×554; **175**; 250 | **416×389**; 175 (scroll); 250 | **333×341**; 175 (scroll); 250 | 559×297; 99 (scroll); **24 collapsed** |
| Foliage | 970×839; 70; 250 | 650×624; 105; 250 | 416×424; 140; 250 | 333×341; 175; 250 | 559×297; 99; 24 |
| Inspect | 1223×874; 35 | 903×694; 35 | 669×529; 35 | 586×446; 70 | 586×326; 70 |
| Chunks | 1223×839; 70 | 903×624; 105 | 669×459; 105 | 586×376; 140 | 586×297; 99 (scroll) |
| Settings — Grid | 1223×734; 175; flow 140 | 903×554; 175; flow **210** (scroll) | 669×389; 175 (scroll) | 586×341; 175 (scroll) | 586×297; 99 (scroll) |
| Settings — New Chunks | 1223×734; 175; flow 105 | 903×554; 175 (scroll) | 669×389; 175; flow 280 (scroll) | 586×341; 175; flow 315 | 586×297; 99; flow 315 |
| Settings — Display | 1223×734; 175; flow 105 | 903×554; 175; flow 105 | 669×389; 175; flow 210 | 586×341; 175; flow 245 | 586×297; 99; flow 245 |
| Settings tab row y (strip y) | 858 | 678 | 546 | 533 = strip+35 (wrapped to row 2) | 489 = strip+35 (row 2) |
| Builder — Object | 1009×851; 58; rail 86; palette 245 | 689×539; 157; 86; 245 | 441×450; 114; 86; 259 | **358×341**; 175 (scroll); 86; 259 | **358×297**; 99 (scroll); 86; 259 |
| Builder — Face | 1009×781; 128 | 689×521; 175 (scroll) | 441×450; 114 | 358×341; 175 (flow 428) | 358×297; 99 (flow 428) |
| Builder — Edge | 1009×781; 128 | 689×521; 175 (flow 186) | 441×450; 114 | 358×341; 175 (flow 358) | 358×297; 99 |
| Builder — Vertex | 1009×781; 128 | 689×539; 157 | 441×450; 114 | 358×341; 175 | 358×297; 99 |
| Pixelizer — any Pixel* | view 1346×912; **rail 39×0, strip 0×175, panel 250×0** | 1026×732; 39×0 / 0×175 / 250×0 | 792×567; 39×0 / 0×175 / 250×0 | 709×519; 39×0 / 0×175 / 250×0 | 709×399; 39×0 / **0×99** / **24×33** |
| Texture panel `MstSidePanel` | 250×912 | 250×732 | 250×567 | 250×519 | 24×399 (auto-collapsed) |
| Panel grip drag | 250 → 350 → 420 (clamped) | 405 accepted | — | — | 220 → 307 (30 % clamp) |
| Panel collapse / expand | click grip → 24×…, tab → restores width | ✓ | — | — | ✓ |

Panel behaviours measured at 1600 unless stated: grip = the panel's left 10 px;
`250 → 350` (+100 drag), max clamp **420**, min clamp **220**, click-without-drag → 24 px tab with
`ExpandTab 22×33`, tab click → back to the remembered width, width ≤ 30 % of the window
(307 px at 1024). A panel never toggled starts **collapsed (24 px)** at 1024×600 (seen in the
Paint/Foliage rows) and expanded (250 px) at 1600.

Content sizes (for the "is it scrollable" question): Paint panel content **236×5764** in a
732-tall panel with a visible 14 px scrollbar (4 rows per wheel notch, per the r5 note); Foliage
content 236×666; Chunks strip flow up to 572×315 in a 99 px viewport.

## Scrollbars / overflow — the exact answer

Every attribute strip is a `ScrollContainer` with a **visible vertical scrollbar** whenever its
flow is taller than the cap; the flow is clipped, not spilling under the window. Proof: at
1024×600 with Paint, `scroll_v` goes `0 → 49` after 4 wheel notches over the strip and the rows
move up (454 → 405), revealing `Material 1. Grass` and `Texture Preset None` that were previously
clipped (`1024-paint-strip-scrolled.png`). No horizontal scrollbar appeared for any strip at any
size (`hscroll=no` in every terrain/builder row); the pixelizer rows report `hscroll=yes` only
because its strip is 0 px wide.

**Correction to the earlier T5 observation** ("Paint 319×385 at 1280 with its bottom 163 px below
the window; at 1024 only the first row reachable with no scrollbar"): `319×385` is the *unclipped
`MstFlow`* content, not the strip. At 1280 the strip is `333×175` (3 rows) with a working
scrollbar, and at 1024 it is `559×99` (≈2.5 rows) that scrolls. The overflow is real, but it is
inside a scrolling strip rather than below the window, and a scrollbar exists at both sizes.

## Dialogs

| Dialog | Request | Actual | Notes |
|---|---|---|---|
| Save Preset | open | 420×179, min `[340,140]`, resizable | OK **and Cancel** (105×34 each), live "Saved as res://…" preview, inline red "Enter a name to save the preset." with OK disabled |
| Save Preset | 400×150 | 400×179 | width honoured; height stays at content height (N9); remembers 400×179 on reopen (in-session) |
| Import (file) | open | 1050×700 at (275,100), min `[500,300]` | — |
| Import (file) | 400×200 | **500×300** | clamps to the *reported* minimum — honest now |
| Builder Preferences | open @1024×600 | 484×528 at (270,36) | fits the 600 px window, scrollable list, no action row needed |
| Pixel World add menu | open @1024×600 | popup at (487,0), 28 items | taller than 600 px, clamped to the window with a scrollbar; effects already in scene are disabled (correct) |

## Keyboard

Tab from the 3D area walks into the strip: Tab 1–3 land on the Settings/Storage page buttons
(`Import...` → `Bake` → `Export`), Tab 4 leaves the strip for the bottom dock `TabBar` and its
buttons — i.e. the strip *is* in the tab order and the focused control is scrolled into view.
Focus ring visibility was not separately verified (screenshots of the focused frame are in the
session notes, not conclusive at 1× scale). `Esc` closes an open popup menu and cancels an open
dialog (Save Preset, Import, Builder Preferences); it was not observed to close the side panel.

## Covered

- [x] Terrain surfaces at all five sizes: Brush, Level, Smooth, Bridge, Paint, Foliage, Inspect,
      Chunks, Settings×6 pages — viewport/strip/flow/scrollbar/clipped-count recorded + capture.
- [x] Toolbar rail expanded and compact at all five sizes (fixed 120 / 40 px, no column jump).
- [x] Side panel: default width, grip drag 220/420 clamps, 30 % clamp, collapse to 24 px, expand
      restores width, auto-collapse at 1024 for a never-toggled panel, in-session persistence,
      restart persistence (fails), Paint/Foliage share one panel, builder palette/rail not panels.
- [x] Pixelizer sweep at all five sizes + fresh session + second scene (N1).
- [x] Builder Object/Face/Edge/Vertex at all five sizes; Builder Preferences at 1024×600.
- [x] Strip scrolling proved by wheel (`scroll_v`), horizontal scrollbar absence checked.
- [x] Settings page-tab row y measured on all six pages at every size (stable).
- [x] Dialogs: Save Preset (open/400×150/empty name/reopen), Import (open/400×200), Builder
      Preferences, Pixel World add menu.
- [x] Keyboard: Tab into strip, Esc, popup dismissal.
- [x] Runtime `hex_island`: default look, F1 toggle ×2, F3, middle-drag orbit down and up
      (cannot go under the island), WASD movement, window resize 1280→1024→800→1600, Esc.
- [x] Runtime tutorials 01–04 load and render; game log checked (no addon errors).
- [x] r4 re-checks: #5 Floor Slot (prop 0→9 + `terrain setting floor slot` undo), #5 Storage Mode
      (`storage_mode` 0→1, Save Data row appears), #6 Debug View (`debug_view` 0→2 +
      `terrain setting debug view` undo).

Not tested: HiDPI/editor_scale ≠ 1, the Pixelizer strips (impossible — N1), the Renderer page-tab
row stability (same reason), splitter drag on the godot-level docks, Paint panel scrolled to the
bottom at 1024 (only the strip scroll was driven), tutorial orbit/zoom, and the runtime at
0.5×/2× OS scaling.

## UI feedback (for FEEDBACK.md)

1. **The pixelizer regression is the only blocker** — ship nothing until `PixelRenderer`/
   `PixelSky`/… selection produces a usable rail/strip/panel again. Everything else in this report
   is polish next to it.
2. **Make the viewport floor two-dimensional.** The r5 work enforces height only. A 250 px panel
   plus a 120 px rail leaves 333 px of 3D view at 1280 — unusable for face picking. Enforce
   `view ≥ 480×300` by shrinking the panel first, then auto-collapsing it below ~1100 px (the
   auto-collapse rule already exists; it just isn't applied to *user-toggled* panels).
3. **Remember panel state where it is promised.** Width/collapse is stored via project metadata
   that is never flushed; persist it in `project.godot` (or an editor settings file) on release of
   the grip, the way dock layouts are. Also remember per *tool* (Paint and Foliage currently share
   one 250 px slot but the builder palette gets none).
4. **Give the builder palette and the builder rail the same affordances as the terrain panel.**
   Right now "resizable side panels" means "the terrain texture panel only". The builder palette is
   the widest fixed block in the UI (245–259 px) and cannot be collapsed at 1024.
5. **Squash the strip empty space.** A Settings page that needs 105 px reserves 175. Let the cap be
   a maximum, not a target, and give those 70 px back to the 3D view.
6. **F3 should be a bottom-docked, collapsible panel**, not a fixed overlay at y=80. It covers the
   right half at 1024 and half the window at 800, and it overdraws the F1 hint. Dock it, scroll it,
   and give it one collapse chevron that survives a resize.
7. **F1 help needs the backing panel the r5 note claims.** One line of white text over grass/sand
   is not readable; the runtime already renders dark translucent panels for F3, so reuse that.
8. **Dialogs are now honest — finish the two loose ends:** the Save Preset height ignores the
   reported min (400×150 → 400×179) and opening it logs a metadata `ERROR` on a fresh project.

## New issues (numbered for the merge)

- **N1** (critical) Pixelizer UI at zero size for every `Pixel*` node, every size — see above.
- **N2** (warning) F3 tuner overflows 1024×600 and 800×480 and has no collapse control.
- **N3** (minor) F1 help unbacked and overdrawn by the F3 panel.
- **N4** (minor) Side-panel width/collapse not persisted across an editor restart.
- **N5** (minor) Save Preset logs a project-metadata `ERROR:` on first open.
- **N6** (minor) Two `create_frustum_points` `ERROR:` lines while a Pixel* node is selected.
- **N7** (warning) No horizontal viewport floor: 333–416 px views with a panel open.
- **N8** (minor) 1024×600 view is 297 px, 3 px under the documented 300 px floor.
- **N9** (minor) Save Preset height refuses the reported minimum.
- **N10** (minor) Settings strips reserve 175 px for 105 px of content.
- **N11** (minor) Builder palette/rail are not resizable or collapsible.

## r4 issues — status in this build

**Fixed (re-verified here):**

| r4 # | Issue | Evidence |
|---|---|---|
| 1 | PixelRenderer 1280×720 crash | no crash at any size; but see N1 (the UI is gone instead) |
| 2 | Builder select panic at 1366×768 | selected `BuilderRuins/Ruin` at 1920/1600/1366/1280/1024, no panic |
| 5 | Floor Slot / Storage Mode write nothing | Floor Slot `1. Grass` → `10. Clay` writes `default_floor_slot = 9` and creates a `terrain setting floor slot` undo entry; Storage Mode `Resource` → `Baked` writes `storage_mode = 1` and the Data Directory / Save Data rows appear |
| 6 | Debug View writes nothing | `debug_view` 0→2 (+ undo `terrain setting debug view`) |
| 7 | Strips uncapped, view 39 px | cap 175/140/105/99, view ≥ 297 everywhere (N8 for the 3 px miss) |
| 8 | Builder Face strip 447 px | 128 px at 1920, 175 px (scrolls) at 1600 — r4's 345×39 view is gone |
| 16 | Panels fixed 250 px, unscrollable, Foliage not a Control | grip 220–420, collapse, 30 % clamp; Foliage palette is a real `MstSidePanel` control tree (236×666, scrolls) |
| 17 | Toolbar columns jump 116→236 | 120 px labelled / 40 px compact at every height |
| 18 | Settings tab row jumps | tab row y is constant across all six pages at every size (858/678/546/533/489). At ≤1280 it sits on **row 2** (strip y + 35) because the title row and the six tabs no longer fit on one line — the r5 "tabs stay at row 1" note only holds at ≥1366 |
| 24 | Pixel World panel off-screen at 1024 | panel is 24×33 (collapsed) — but unusable, see N1 |
| 28 | Save Preset: no Cancel, cannot shrink, forgets size | OK+Cancel, min `[340,140]` honoured in width, remembers 400×179 |
| 30 | Empty preset name closes silently | inline red error + OK disabled |
| 38 | Finite water edge visible | hex_island + tutorials 01/02/04: sea to the horizon, no rectangle |
| 40 | File dialog min inaccurate | reports and clamps to 500×300 |
| — | Builder Preferences 484×664 off-screen at 600 px | 484×528, fits, scrolls |

**Still reproducing:**

- **#39** (runtime F3/F1 HUD) — F3 clips at 1024×600 and 800×480 with no collapse; F1 is still a
  single unbacked line. The toggle half is fixed. See N2/N3.

**Not re-tested in this pass** (no opinion): #3, #4, #9–#15, #19–#23, #25–#27, #29, #31–#37,
#41–#43. Two of them are worth a targeted re-run: **#23** (orbit under the island — did *not*
reproduce in 3 middle-drags, looks fixed with #38) and **#35** (`draw_rect` warning spam — the
builder log was clean, but the builder UI was only lightly driven).

## Files written

- `qa/findings/r5-ui-runtime.md` (this file)
- `qa/screenshots/r5-ui-runtime/*.png` (148 captures)
- `qa/screenshots/r5-ui-runtime/ui/*.json` (compact per-surface dumps, 111)
- `qa/screenshots/r5-ui-runtime/matrix.csv`, `pixelizer-sweep.jsonl`, `editor.log.txt`
- `qa/scripts/r5-ui-runtime/{wave,matrix,compact_ui,summarize,buttons,pixel_sweep,xresize}.py`
