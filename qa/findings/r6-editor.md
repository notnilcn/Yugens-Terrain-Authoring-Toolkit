# r6 — E1: editor shell verification (round-2 changes)

Session `r6-e1` (display `:71`, port `8871`, window 1600×900 at boot, resized to 1280×720,
1024×600 and back). Build: `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`, HEAD `e50088d`
(the merge of the round-2 fix stack). Godot 4.7.2, llvmpipe, editor scale 1.0, scene
`res://demo/hex_island.tscn`. Every number below is read back from `qa/drive.py ui --root all`
dumps (copied to `qa/screenshots/r6-editor/ui/`) or from `drive.py windows`/`prop`.

The session was **stopped** at the end (`qa/session.sh stop r6-e1`; display `:71` gone). The
editor log has **79 lines from start to finish** — 2 `ERROR:` lines and 16+16 normalize
warnings, all from boot; **no new log line of any kind** during the whole pass (dialogs,
panel builds, resizes, undo).

## Headline

1. **N1 is fixed.** The first `Pixel*` selection in a fresh session now ends with the
   pixelizer UI laid out: rail `39×732`, strip `731×175`, panel `250×732` — no second
   selection and no 2D→3D round-trip needed. **Caveat:** the panels are still zero-sized for a
   couple of seconds while the first build runs (at +2 s the dump still reads rail `39×0`,
   strip `0×35`, panel `250×0` with 631 panel controls not yet created); it self-heals. See
   **R6-1**.
2. **N7, N10, N16, N23, N4, N5/N9, N24 (as scoped), N29/N30/N22 and r4 #19 all verify** on
   the merged build — with two nuances: Settings → Storage shows `107 px` only when Storage is
   the tool's first page (`142` after visiting Layout in the same tool session, `175` on the
   very first Settings open after Paint), and at 1024 the Settings/pixelizer page-tab header
   scrolls away with the body (the documented degradation).
3. **N6/N31 boot noise is unchanged**: 2× `create_frustum_points` `ERROR:` + 16 plane + 16
   vector warnings, all **before any selection** (lines 12–78 of the log, right after
   `MST QA harness listening`). Nothing is added later in the session.
4. Two regression-sweep defects: a 26 px horizontal overflow on Settings → Layout at 1600
   (visible H scrollbar, third column clipped) and the pixelizer strip at 1024 spending 66 of
   its 99 px on two tab rows. Plus two UI notes (EditorSpinSlider ignores the wheel; the
   initial `MstValueLabel` shows a rounded value until the first edit).

## Round-2 verification

| # | Item | Verdict | Measured |
|---|---|---|---|
| 1 | **N1** first `PixelRenderer` selection, fresh session, 1600×900 | **PASS** (with R6-1) | first selection, no reselect: at +2.0 s `PixelizerToolbarRail [287,121 39×0]`, `PixelizerAttributes [287,121 0×35]`, `PixelWorldPanel [287,121 250×0]`, 19 387 controls; by the next dump (≈+5 s) `rail [287,121 39×732]`, `strip [329,678 731×175]`, `panel [1063,121 250×732]`, view `[329,121 731×554]`, 20 018 controls. Screenshot of the same state `n1-01-first-select-1600.png`. |
| 1b | N1 reselect terrain → PixelRenderer | **PASS** | terrain: rail `120×732`, strip `903×105` @410,748, view `903×624` @410,121. After reselect: rail `39×732` @287,121, strip `731×175` @329,678, panel `250×732` @1063,121 (first post-build dump; stale terrain rects in the dump taken at +0.1 s). |
| 1c | N1 2D → 3D | **PASS** | 2D (CanvasItemEditor visible): pixelizer containers all hidden, as expected. Back on the 3D tab: rail `39×732`, strip `731×175` @329,678, panel `250×732` @1063,121 — identical, stable over two dumps 2 s apart. |
| 2 | **N7** width floor, Paint | **PASS** | 1280×720: `MarchingSquaresTextureSettings [964,154 32×519]` (collapsed, `ExpandTab [964,154 32×33]`), view `551×341` @410,154 (≥480 ✓), strip `551×175`. 1600×900: panel re-expanded `250×732` @1063,121, view `650×554`, strip `650×175`. Tab click at 1280: panel **223×519** @773,154 (claim “220”), view `363×341`. |
| 3 | **N10** strip cap = maximum | **PASS**, see note | Settings re-entered on the Storage page (1600): strip `903×107` (min 107), page body `476×70`. Layout first → strip `142`; a later Storage click keeps `142` (high-water per tool ✓). The very first Settings open (from Paint) was `175` for every page, incl. Storage (see R6-4). Pixelizer: strip stays `175` on the shorter Sheets page (high-water ✓). |
| 4a | **N16** pinned title/tabs, body scrolls | **PASS** | 1280×720 Settings → Layout: strip `586×175` @410,498; tabs pinned at y=498/531 through `scroll_v` 0 → 87 → **208**; last body row `Extra Collision Layer:` at y=641 (strip ends 673) reachable; `Blend Mode:` at 501. |
| 4b | N23 at 1024 | **PASS (documented degradation)** | 1024×600 Settings: tabs at strip top y=454; wheel → `scroll_v 49`, tabs move to y=405 (header scrolls with the body, does not cover the 99 px strip). |
| 4c | **N23** pixelizer page tabs | **PASS** | 1600: two rows, `Renderer 432/Outlines 509/Color / Palette 580/Rays 691/Sun 795` then `Sheets 432/Clouds 509/Depth Fade 580/Debug Views 691` (y 678/711) — nothing clipped, strip 731 wide (right edge 1060) vs last tab end 795. 1280: identical rects, strip 632 wide (right edge 961) — still inside. Clicking `Sheets` left every tab rect unchanged (no slide). 1024: two rows in the 99 px strip (66 px of tabs), one body row clipped. |
| 5 | **N4** remembered panel state | **PASS** (file evidence) | grip drag 250 → **320** px (`MarchingSquaresTextureSettings [993,121 320×732]`, view `580×554`); `res://.godot/editor/mst_editor_state.cfg` → `panel_MarchingSquaresTextureSettings_width=320.0`; click-grip collapse → `32×732` + `collapsed=true`; tab click → back to `320` + `collapsed=false`. Restart **not** tested (a session restart re-copies the project, so it would destroy the file under test). |
| 6 | **N5/N9** Save Preset | **PASS** | open 420×179, min `[340,178]`, resizable; log **79 → 79 lines** (no metadata `ERROR:`); `window-resize 400 150` → **400×179**; reopen → 400×179 (remembered); Cancel `105×34` + OK `105×34`; empty name → inline `Enter a name to save the preset.` and **OK disabled**. |
| 7 | **N24** collapsed tab / Inspector at 1024 | **PASS as claimed (partly fixed)** | Pixel World collapsed tab `32×519` @964,154 (1280) and `32×399` @856,154 (1024); `ExpandTab 32×33`. Inspector at 1024: `EditorInspector [891,180 220×416]` → right edge **1111** (87 px past the 1024 window), header and rows visibly cut in `n23-n24-1024x600-pixelizer.png`. |
| 8 | **N29/N30/N22** | **PASS** | Size HSlider step `0.5` (min 1, max 100): wheel 15.0 → 15.5 → 16.0 → 16.5 → 16.0, label `15.5/16.0/…` identical every step. Aspect step `0.0505` (min 0.2005): 1.0085 → 1.059 → 1.1095 → 1.16 (exactly one step per notch), label `1.059/1.110/1.160`. Settings tab is **“Layout”** (not “Grid”). Tint `Grid Size` HSlider: `disabled=false` but ignores the wheel (`1.0` before and after up×2 + down×1) with tooltip `Grid Size applies only with Grid Aligned on.`; `Grid Aligned` is disabled with its own explaining tooltip. |
| 9 | **r4 #19** feature-switch undo | **PASS** | Pixel World ▸ Features ▸ Birds → checked `True`→`False`; History dock lists **“Turn Birds feature off”** above “terrain vertex paint”/“The Beginning”; Escape (popups closed) then one `Ctrl+Z` → Features submenu reads Birds `True` again. `undo-state` (root history) only ever shows `terrain vertex paint` — as the brief warns, the global step is only visible in the History dock. |
| 10 | **N6/N31** boot noise | **still open** (unchanged) | 2× `ERROR: Condition "!res" … create_frustum_points (rendering_light_culler.cpp:682)` (log lines 28, 62) + **16** `Plane cannot be normalized` + **16** `Vector3 cannot be normalized`, all between line 12 and 78 — i.e. at startup, **before any selection** (harness listening at line 11, selection count 0). Zero further `ERROR:`/`SCRIPT ERROR:` in the entire 79-line log. |

### Notes on the N1 measurement

- The first `ui --root all` dump at ≈+2.0 s after `select` still showed the r5 signature
  (`39×0 / 0×35 / 250×0`) **and** a control count of 19 387. The next dump, 20 018 — the 631
  missing controls are the pixelizer panel/strip content, so what is late is the *build*, not
  a relayout of an existing tree. As soon as the panels exist they are correct, so the r5
  behaviour (“only a reselect or a 2D→3D round-trip fixes it”) is gone.
- A dump costs ≈2.3–2.8 s of editor time (19k–20k controls), so my timing resolution on the
  fresh session is: zero-state at +2.0 s, correct at +5 s, no reselect in between (verified:
  exactly one `select` command had been issued and the selection path never changed).

## Findings

### R6-1. First `Pixel*` selection leaves the pixelizer UI empty for >2 s before it builds — warning
- **Severity:** warning (was the critical N1; the sticky failure is fixed, the first-build
  window is not)
- **Steps:** 1) Fresh session, 1600×900, `hex_island`. 2) `select --path PixelRenderer`.
  3) `ui --root all` 2 s later, then again after ~5 s.
- **Expected:** the first selection lays the rail/strip/panel out on the first frame.
- **Actual:** at +2 s the three containers are still at their minimum size at the parent's
  origin — `PixelizerToolbarRail [287,121 39×0]`, `PixelizerAttributes [287,121 0×35]`,
  `PixelWorldPanel [287,121 250×0]` — and the panel contents (631 controls) do not exist yet.
  They appear and lay out correctly (`39×732`, `731×175 @329,678`, `250×732 @1063,121`)
  without any further interaction. Nothing is logged.
- **Evidence:** `qa/screenshots/r6-editor/ui/n1-first-1600-t2s.json` vs `n1-first-1600-t5s.json`;
  `n1-01-first-select-1600.png` (taken ~1 s after the second dump: UI fully built).
- **Undo:** not applicable (layout).

### R6-2. Settings → Layout overflows the strip by 26 px at 1600 — minor
- **Severity:** minor/UI (regression-sweep; pre-existing, not introduced here)
- **Steps:** 1600×900, terrain selected, Settings → Layout.
- **Expected:** all three columns inside the strip (the strip is 903 px wide).
- **Actual:** page content `929×142` (`SettingsPage`, `SettingsColumns`, third column
  `VBoxContainer [1064,715 275×105]`); `_h_scroll` is **visible** at `[410,839 903×14]`
  (max 929) and 11 visible controls run past the strip's right edge (1313) — the third
  column's `Extra Collision Layer` dropdown sits at `[1230,785 109×35]`, i.e. 26 px clipped.
- **Evidence:** `n16-01-settings-layout-1600.png` (scrollbar under the rows),
  `qa/screenshots/r6-editor/ui/` (the dump with `@MstFlow@27156 [410,678 929×142]`).
- **Undo:** not applicable.

### R6-3. EditorSpinSlider ignores the mouse wheel — minor/UI
- **Severity:** minor/UI
- **Steps:** Paint tool in the strip; hover `Rotation` `[490,751 80×29]` (`EditorSpinSlider`,
  value 0.0); wheel up ×2; click it, wheel up again.
- **Expected (N29 wording “wheel-edited settings snap to the slider step”):** the value moves
  in step increments.
- **Actual:** value stays `0.0` and `focus=false` in all three attempts, while the sibling
  HSliders (`Size`, `Aspect`, `Opacity`, `Grid Size`) all react to the same wheel call. So the
  snap fix is verified for HSliders only; EditorSpinSliders cannot be wheel-edited at all.
- **Evidence:** `qa/screenshots/r6-editor/ui/` (`r6-rot-*.json` sequence); the YAML values
  `Rotation val=0.0` before/after every wheel.
- **Undo:** not applicable.

### R6-4. Settings strip inherits the previous tool's 175 px height on its first open — minor
- **Severity:** minor/UI (contradicts “pinned strips keep the tallest height seen *for that
  tool*” in the first open of a session)
- **Steps:** 1600×900, Paint (strip 175) → Settings: every page, Storage included, keeps
  `175`. Then Chunks → Settings: Layout `142`, Storage `142`. Then Chunks → Settings again
  (Storage is the remembered page): `107`.
- **Expected:** the first Settings open should already show the Settings high-water (Layout =
  142 at 1600), not the Paint tool's 175.
- **Actual:** `175` on the first open, `142`/`107` afterwards; min height follows
  (`min=[0,175]` vs `min=[0,142]` / `min=[0,107]`).
- **Evidence:** `n10-01…n10-06` (screenshots), `qa/screenshots/r6-editor/ui/n10-*`.
- **Undo:** not applicable.

### R6-5. Pixelizer strip at 1024 spends 66 of its 99 px on two tab rows — minor/UI
- **Severity:** minor/UI
- **Steps:** 1024×600, select `PixelRenderer`.
- **Actual:** strip `[221,454 632×99]`; page-tab rows at y=454 and y=487 (33 px each) leave
  ~32 px for the body, so the first body row (`Bands/Gap/Energy`) is cut by the strip's bottom
  edge (553). Usable but cramped; the 1024 Settings strip degrades to one scrolling header row
  instead, the pixelizer keeps two fixed rows.
- **Evidence:** `n23-n24-1024x600-pixelizer.png`.
- **Undo:** not applicable.

## Covered

- [x] N1: first selection (fresh), reselect terrain → PixelRenderer, 2D → 3D, at 1600×900;
      screenshots + dumps for all three legs.
- [x] N7: Paint at 1280 (auto-collapse, view 551) and 1600 (re-expand, view 650); tab click
      forces 223 px.
- [x] N10: Settings six pages at 1600 (strip 175 first open, 142 Layout, 107 Storage), high-water
      retention (Layout → Storage = 142), pixelizer high-water (Sheets keeps 175).
- [x] N16: Settings Layout at 1280, `scroll_v` 0/87/208, tabs pinned, last row reachable;
      Settings Layout at 1024 (header scrolls), 1600 (fits, but R6-2).
- [x] N23: pixelizer page tabs at 1600, 1280, 1024; two-row columns; no tab slide on click;
      `SectionTitle` stays left.
- [x] N4: grip drag 250→320, collapse 32, expand 320, `mst_editor_state.cfg` read after every
      step.
- [x] N5/N9: Save Preset open, min, 400×150, remember-on-reopen, Cancel/OK, empty-name inline
      error + disabled OK, log unchanged.
- [x] N24: collapsed tab 32 px at 1280 and 1024; Inspector 220×416 ending at x=1111 at 1024.
- [x] N29/N30/N22: wheel snap on Size/Aspect (values + steps read with `prop step/min/max`),
      “Layout” tab name, Tint Grid Size non-editable + tooltip, Grid Aligned disabled + tooltip.
- [x] r4 #19: Features ▸ Birds off, History entry, Escape, one Ctrl+Z → back on.
- [x] N6/N31: boot log counted; no new `ERROR:`/`SCRIPT ERROR:` all session.
- [x] Regression sweep: toolbar rail collapse 120→40 (strip grows to 983) and back; Chunks tool
      (strip 903×105, hint+status backed above the strip); Storage page (Storage Mode / Import /
      Bake / Export); Import file dialog 1050×700, min 500×300, Esc does **not** close it
      (Cancel does); Paint hint wrap at 1600 and 1280; side-panel grip/collapse/expand.

Not tested: editor restart for N4 (destroys the file under test in this harness); other window
sizes beyond 1600/1280/1024; HiDPI; `N8` (1024 view height); builder/runtime surfaces (other
briefs).

## New issues (numbers for the merge)

- **R6-1** (warning) First `Pixel*` selection builds the pixelizer panels ~2–3 s late; until
  then rail `39×0`, strip `0×35`, panel `250×0` with no content (was N1-critical, now
  self-healing).
- **R6-2** (minor) Settings → Layout is 929 px wide in a 903 px strip at 1600 → horizontal
  scrollbar + 26 px clip of the third column.
- **R6-3** (minor) `EditorSpinSlider` ignores the wheel (Paint `Rotation` stays 0.0), while
  HSliders wheel-edit and snap.
- **R6-4** (minor) Settings' first open inherits the previous tool's 175 px strip; high-water is
  only correct from the second entry onward.
- **R6-5** (minor) Pixelizer strip at 1024: two 33 px tab rows inside a 99 px strip leave one
  clipped body row.

## UI feedback (measured rects)

1. **Ship-blocking order:** nothing here is critical any more. R6-1 is the only item a user
   would notice immediately (select the first pixel node → ~2 s of empty screen), and a
   “Building pixelizer…” placeholder or building the panels on `edit()` instead of a deferred
   call would close it.
2. **The strip/pin work reads well** — `39×732` rail, `731×175` strip, `250×732` panel and a
   `731×554` view at 1600; 1280 auto-collapses the panel to `32×519` keeping a `551` px view;
   1024 keeps the pixelizer usable (`39×399`, `632×99`) instead of collapsing to nothing.
3. **Settings Layout is the only page that does not fit** (929 vs 903 at 1600): the third
   column (`1064…1339`) needs ~30 px more or a narrower control; drop the horizontal scrollbar.
4. **The 175/142/107 strip heights still feel arbitrary on the first open** (R6-4). Reset the
   high-water when a tool is entered rather than when a page changes and the numbers stop
   jumping between visits.
5. **The Inspector still overhangs at 1024** (`220` wide starting at `891` → `1111`): either
   raise the window minimum or let the dock shrink to ~150; the pixelizer strip `99 px` and the
   Inspector's 87 px overhang are the two things that make 1024 feel unfinished.
6. **The pixelizer page tabs** choose good columns (`77/71/111/104/42` at 1280, two rows) and
   never slide under the pointer any more — the `SectionTitle` + tabs block is stable across
   clicks. At 1024 let one tab row scroll like the Settings header (R6-5).
7. **Harness notes for the next driver:** the `2D`/`3D` main-screen buttons are at
   `2D [587,4 58×36]` and `3D [649,4 58×36]` (clicking x=663 hits **3D**, not 2D); a
   `ui --root all` dump costs 2.3–2.8 s of editor time — do not use it to time a build; the
   Import file dialog ignores Escape (use Cancel at `[599,775]` at 1600×900).

## Files written

- `qa/findings/r6-editor.md` (this file)
- `qa/screenshots/r6-editor/*.png` (43 captures, incl. crops `reg-04b/04c-hint-crop.png`)
- `qa/screenshots/r6-editor/ui/*.json` (20 UI dumps)
- `qa/screenshots/r6-editor/editor.log.txt` (full 79-line session log)
- `qa/scripts/r6-editor/{uiq.py,n1_timing.sh,settings_pages.sh}`
