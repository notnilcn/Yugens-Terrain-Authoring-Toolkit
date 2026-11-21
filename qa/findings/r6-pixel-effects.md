# R6 pixel/effects verification — merged build `e50088d` (r4base)

Session `r6-e3`, display `:73`, port `8873`; build
`/home/clintonnguyen/g/code_examples/rrr-wt/r4base` (HEAD `e50088d`), Godot 4.7.2,
llvmpipe, editor 1600×900 (resized 1280×720 and 1024×600). Scenes:
`demo/hex_island.tscn` (the brief's N1/#36/#37 scene) and
`tutorial/04_world_mood.tscn` (the #37/#38/N25 scene the round-2 notes measure).
Log `/tmp/opencode/qa/sessions/r6-e3/editor.log`. Screenshots
`qa/screenshots/r6-pixel-effects/`; helpers `qa/scripts/r6-pixel-effects/`
(`rects.py`, `tree.py`, `fields.py`, `rail_sweep.py`, `diff.py`, `stats.py`).

All "static" pixel diffs below were taken with **Animate OFF** (`PixelAnimateButton`
toggled false); two consecutive captures with Animate off are byte-identical
(`changed=0/397664`, `90`/`91-smoke-on-a/b.png`), so the counts are effect pixels, not
water/cloud drift.

---

## Round-2 verification

| Item | Verdict | Numbers |
|---|---|---|
| 1. N1 first-selection rects | **Pass (with the Finding 1 caveat)** | First select settles with no reselect: rail `39×732`, strip `731×175`, panel `250×732`. Reselect and 2D→3D give identical rects. The one-frame zero state (`39×0`, `0×35`, `250×0`) is gone by 2 s. |
| 2. N23 page tabs | **Partial** | Correct 5-column × 2-row grid (`405×66`) at 1600/1280/1024 after any rebuild; the **first** build is one 670 px row and clips `Debug Views` by 42 px. Nothing is lost at 1024 (strip scrolls, `scroll_v` 0→74). Tab row does not slide (`scroll_h` 0 everywhere). |
| 3. r4 #19 feature switches | **Partial** | Toggle → Escape → Ctrl+Z reverts `features/clouds` (and `features/god_rays`) and History shows `Turn Clouds feature off`. Ctrl+Z **with the popup open does nothing** (still requires Escape). |
| 4. Clamps | **Pass** | 6/6 fields stop at the declared max (16 / 1.0 / 4.0 / 1.0 / 2.0 / 1024), one drag = one undo step, one Ctrl+Z restores. Drag *sensitivity* is erratic under synthetic input (Finding 3). |
| 5. r4 #36 smoke | **Pass (improved)** | 317 px changed in a 24×22 patch (r5: 78 px); plume visible as a haze column at the hearth; scene `start/end 0.18/0.90` (r5 0.1/0.45). |
| 6. r4 #37 dawn/dusk | **Partial** | Dawn 150/156/173 (r5 177/177/187) with island 105/107/74 vs water 148/161/177; dusk 0.75 142/115/125; noon 78/129/175. Dawn still reads as a flat pale wash. |
| 7. r4 #38 water edge | **Fail** | The water plane is still a straight-edged rotated square with four sharp corners at low orbit / zoom-out, noon and dawn (`112c-check.png`, `113-wm-far-dawn.png`). |
| 8. N25 SunShafts | **Partial** | Wired (`WorldMood/Sun`); exact warning captured; 0/397664 px changed at alpha 0.6 / count 64 with shadow distance 400 m. Warning text is stale (still says 120 m). |
| 9. N6/N31 boot noise | **Unchanged** | 16 plane + 16 vector warnings + 2 `create_frustum_points` ERRORs, all during boot (log lines 13–79); whole pass adds 0 lines. Not correlated with selecting a `Pixel*` node. |
| Regression: Preview/Animate pressed states | **Pass** | Accent-blue icon + lifted background; `toggled` reads true/false. |
| Regression: Pixel World `+ Add` | **Pass** | Selection stays on `PixelRenderer`, rail/strip/panel stay open, History gets `Add PixelSunShafts`. |
| Regression: preset undo clears tint | **Pass** | Dawn 99.2 % of viewport changed; one Ctrl+Z → byte-identical (`0/397664`). |
| Regression: fireflies at night | **Pass** | 932 px in a 130×102 patch visible-vs-hidden, Animate off. |
| Regression: weather / rain | **Pass** | Rain and Storm streaks + splash clusters, Snow flakes (needs Animate ON). |

---

## Finding 1 — First-selection Pixelizer layout: N1 no longer sticks, but the page-tab grid is built with the wrong column count on the very first build

- Severity: error (page tabs overflow the strip on the first selection; self-corrects only
  after another section/page rebuild)
- Steps: 1) fresh session (boots `demo/hex_island.tscn`), nothing selected. 2)
  `drive.py select --path PixelRenderer`. 3) Dump the control tree immediately and again
  after 2 s.
- Expected: rail / strip / panel laid out, and the page-tab grid wrapped to the strip
  width (as the round-2 note claims: "two rows, 68 px").
- Actual: **N1's zero-size state is gone.** Immediately after the select the panel is still
  one frame old (`PixelizerToolbarRail 39×0`, `PixelizerAttributes 0×35`,
  `PixelWorldPanel 250×0`, all `visible:false`), but it settles **by itself within 2 s**
  to the correct rects — no reselect and no 2D→3D round-trip needed:
  - `PixelizerToolbarRail [287,121 39×732]`
  - `PixelizerAttributes [329,678 731×175]`
  - `PixelWorldPanel [1063,121 250×732]`
  - `SectionPages [432,678 670×33]` ← **one row of 9 tabs, 670 px wide, clipped by the
    731 px strip**: the `Debug Views` tab `[998,678 104×33]` ends at x=1102, 42 px past
    the strip's right edge (1060) and is rendered as `Debug V` (`04-1600-strip.png`).
  - After switching section and back (any rebuild of the strip) the same Renderer page
    lays out as `SectionPages [432,680 405×66]` — 5 columns × 2 rows, `Debug Views`
    fully visible (`12-1600-strip-rebuilt.png`).
- Reselect path: select `MarchingSquaresTerrain`, then `PixelRenderer` again → identical
  rects (`rail 39×732`, `strip 731×175`; `170-reselect.json`). 2D→3D round trip → identical
  rects and `SectionPages [432,678 405×66]` (`171-2d3d.json`). So the r5 remedy path is no
  longer required; the only residual defect is the first build's tab-column count.
- Reproducible: first build only (the first `Pixel*` selection in a session). Every later
  build (section switch, resize, page switch) uses the width-derived column count.
- Evidence: `03-first-settled.png` / `03-first-key.json` (first build, `PinnedHeader 773×37`
  and `SectionPages 670×33` both wider than the 731 px strip; `Debug Views` `clipped`),
  `04-1600-strip.png` (crop: `Debug V` cut), `12-1600-strip-rebuilt.png` (corrected
  5×2 grid), `02-first-key.json` (the one-frame zero rects), `12-renderer-rebuilt.json`.
- Undo: not applicable.

## Finding 2 — Feature-switch undo works, but only after the popup is dismissed (r4 #19, half)

- Severity: minor (the shortcut is dead exactly while the menu is open)
- Steps: 1) `Pixel World` menu → `Features` → uncheck `Clouds`; `features/clouds` leaves
  `project.godot` (set to default/false). 2) `Escape` twice (the submenu eats the first
  Escape), then `Ctrl+Z`. 3) Repeat with `God Rays` but press `Ctrl+Z` **while both popups
  are still open**.
- Expected: the toggle is one undo step; `Ctrl+Z` reverts it; History shows it.
- Actual: after the popups are closed, `Ctrl+Z` restores `features/clouds=true`
  (`project.godot` line 33) and History shows **"Turn Clouds feature off"** above
  "The Beginning" (`47-history-dock.png`, `48-history-two-entries.png`). With the popup
  still open, `Ctrl+Z` does **nothing** (`features/god_rays` stays absent until Escape +
  `Ctrl+Z`, which restores it). This is the pre-existing behaviour the round-2 note
  describes, not a regression.
- Note: `drive.py undo-state` still reads `action:"", undo_count:0` for these toggles —
  the entry lives in the global history, not the edited scene's history (harness
  limitation, same as the r5 note). Use the History dock to read it.
- Evidence: `43-features-before.png`, `44-features-clouds-off.png`, `47-history-dock.png`,
  `48-history-two-entries.png`; `project.godot` `[marching_squares_terrain]` before/after.
- Undo: works after Escape; dead while the popup is open.

## Finding 3 — Clamped ranges are honest, but a drag on a strip field is unusable with synthetic input

- Severity: minor (drag behaviour; may be an llvmpipe/XTEST artefact — see the caveat)
- Steps: for each field, one real `drive.py drag` of +80 px (1.2 s) on the strip field,
  then `undo-state`, then `Ctrl+Z`.
- Expected: the value stops at the widget's declared max, one drag = one undo step, one
  `Ctrl+Z` restores.
- Actual (all six **pass** the clamp/undo requirement):

  | Field (page/section) | widget range | before → after +80 px | undo action | Ctrl+Z |
  |---|---|---|---|---|
  | Pixel Size (Renderer) | 1–16 | 3 → **16** | `Set Renderer Pixel Size` | 3 |
  | Darken (Outlines) | 0–1 | 0.546 → **1.0** | `Set Outlines Darken` | 0.546 |
  | Ray Alpha (Rays) | 0–4 | 0.9975 → **4.0** | `Set God Rays Ray Alpha` | 0.9975 |
  | Sky Coverage (Sky & Clouds) | 0–1 | 0.504 → **1.0** | `Set Sky & Clouds Coverage` | 0.504 |
  | Water Swell H. (Water → Waves) | 0–2 | 0.121 → **2.0** | `Set Water Swell H.` | 0.121 |
  | Fireflies Count (Fireflies) | 0–1024 | 64 → **1024** | `Set Fireflies Count` | 64 |

  `undo_count` goes 0→1 (or n→n+1) for each; one `Ctrl+Z` undoes exactly that step
  (`71-*`, `66-*`, `59-*`, `62-*`, `56-*`, `52-*` dumps).
- **Drag anomaly:** a *short* drag does not move proportionally — it saturates. Pixel Size:
  a 6 px right drag (0.45–1 s) went 3→16; a 20 px **left** drag also went 3→16; a 12 px
  right drag spread over 3 s (24 steps) left it at 3; a 4 px slow drag left Darken at
  0.3045, while a 20 px drag (either direction, fast or slow) took Darken 0.546→1.0.
  A plain click in the middle of a field does nothing (fields are drag-only). Because the
  editor runs at 5–10 fps under llvmpipe, this is most likely the synthetic-motion path
  (`EditorSpinSlider` drag is velocity/frame sensitive) rather than a user-facing
  regression — the r5 pass only ever dragged +80 px, which lands on the max anyway.
  Flagged for a real-mouse spot check.
- Evidence: `51`–`90-*.json`/`.png` dumps listed above (`fields.py` output).
- Undo: works (one step per drag).

## Finding 4 — r4 #36 smoke: the plume is now measurable, but it reads as a haze puff

- Severity: minor (visual only)
- Steps: `demo/hex_island`, Animate OFF, capture; `set-prop visible false --path Smoke`;
  capture; diff the 731×544 viewport.
- Expected: a visible plume at the hearth.
- Actual: **317 px change** in a 24×22 px patch at viewport (416,309)–(440,331)
  (window 745,430–769,452) — up from r5's 78 px in a 10×16 patch, so the round-2 puff
  change is real. The 4× crops show a pale grey-blue haze column over the hearth fire
  (`93-smoke-on-crop4x.png` vs `94-smoke-off-crop4x.png`); the fire sparkles are unchanged.
  The scene's own node values are `rate 3.0`, `start_size 0.18`, `end_size 0.90`
  (r5 recorded 0.1 / 0.45 — the scene was retuned in round 2).
- Evidence: `90-smoke-on-a.png` (float), `91-smoke-on-b.png` (identical),
  `92-smoke-off.png` (hidden), `93/94-...-crop4x.png`; diff `changed=317/397664`.
- Undo: not applicable (visibility toggled through the harness).

## Finding 5 — r4 #37 dawn/dusk: numbers match the round-2 claim; dawn is still flat/low-contrast

- Severity: minor (palette/lighting; improved but not resolved)
- Steps: `04_world_mood`, `Pixel World` → Dawn / Noon / Dusk presets (and `time_of_day`
  0.75 by hand), Animate OFF, capture the viewport, mean RGB over the viewport, the island
  patch (620,330 80×40) and a sea patch (450,500 120×80).
- Expected: a warm dawn/dusk, island green, water clearly water.
- Actual:

  | state | time_of_day | viewport mean | island patch | water patch | sky patch |
  |---|---|---|---|---|---|
  | Noon (`04_world_mood`) | 0.5 | 77.7 / 129.1 / 175.3 | 80.4 / 104.9 / 58.5 (green) | 49.1 / 118.3 / 169.3 (blue) | 122.4 / 165.7 / 220.8 |
  | Dawn (`04_world_mood`) | 0.2705 | 150.5 / 156.1 / 173.2 | 105.2 / 107.2 / 74.3 (olive) | 148.0 / 161.1 / 176.7 | 162.0 / 149.7 / 171.1 |
  | Dusk 0.75 (`04_world_mood`) | 0.75 | 142.0 / 114.9 / 125.1 | 102.2 / 81.6 / 48.8 (brown) | 115.1 / 110.4 / 137.0 | 168.7 / 114.2 / 119.9 |
  | Dusk preset (`hex_island`) | 0.7305 | 153.8 / 126.1 / 119.1 | 129.4 / 104.1 / 80.9 | 163.0 / 122.6 / 141.0 | — |
  | Dusk 0.75 (`hex_island`) | 0.75 | 153.2 / 120.8 / 115.2 | 128.5 / 99.0 / 78.5 | 162.2 / 121.9 / 138.2 | — |

  Dawn on `04_world_mood` is 150/156/173 — matching the round-2 claim (147/154/172) and far
  from r5's flat 177/177/187 lavender; the island (105/107/74) and the sea (148/161/177)
  are clearly separated. Dusk is warm pink-mauve. **But** the dawn frame is still a
  low-contrast pale wash (`102-wm-dawn.png`) — the island's green is olive-grey and the sea
  is pale silver-lavender. On `hex_island` dawn reads 159/155/148 with the island tan-green.
- Evidence: `96-preset-dawn.png` (noon on hex_island), `97-preset-dawn.png` (dawn on
  hex_island), `98-preset-dusk.png`, `99-time-075.png`, `100-wm-default.png`,
  `101-wm-noon.png`, `102-wm-dawn.png`, `104-wm-dusk-075.png`. The `04_world_mood` Dusk
  *preset* capture (103) timed out at the harness (Animate was on, two editors on the host);
  the 0.75 row above is the post-preset `time_of_day 0.75` frame.
- Undo: preset undo covered below (regression sweep); the 0.75 step was a harness `set-prop`.

## Finding 6 — Page tabs at 1280/1600/1024: correct after a rebuild, nothing lost

- Severity: ui (only the first-build case in Finding 1 breaks)
- Steps: resize 1600×900 → 1280×720 → 1024×600; per size, select the Renderer / Water /
  Weather pages and record `SectionPages` plus every tab rect; scroll the 1024 strip.
- Actual: after a rebuild the grid is 5 columns × 2 rows (`405×66`) at all three sizes and
  fits the strip (`432..837` in a 632 px strip at 1280; `324..729` in a 632 px strip at
  1024). Weather (382 px), Water (447 px), Day/Night (210 px), Birds (242 px) all fit on
  one row. The tab row's *y* moves with the strip's content height (Renderer 175 px strip →
  y=680 at 1600; Weather/Water 107 px → y=746; Sky 140 px → y=713; Birds 72 px → y=781) —
  the strip is bottom-anchored, so the header sits at the top of each section's strip; the
  tab row does not slide horizontally and no `_h_scroll` is visible (`scroll_h` 0).
  At 1024 the strip is only 99 px tall: header 66 px + **one** body row, and the remaining
  rows need the strip's own vertical scrollbar (`scroll_v` 0→74 brings `Crisp Edges` into
  view, `31`/`33-1024-*`).
- Evidence: `20-1280.json`, `21-1280-strip.png`, `22/23-1280-water.json`,
  `31-1024-renderer.json`, `31-1024-strip.png`, `32/33-1024-scrolled*.png`,
  `rail-sweep.json` (all 15 rail sections).
- Undo: not applicable.

## Finding 7 — r4 #38 water edge: the finite water plane still shows straight edges and corners at low orbit / zoom-out

- Severity: minor (view-dependent; contradicts the round-2 "fixed" claim)
- Steps: `04_world_mood`, select `PixelRenderer`, noon; middle-drag the viewport down
  (pitch under), wheel-down to a full-island zoom-out; then the same at dawn.
- Expected (round-2 note): "the rim now tints to the sky colour in sRGB and fades on a
  circle, so there is no straight edge or corner even at the farthest zoom".
- Actual: the water plane renders as a **rotated square with four straight edges and four
  sharp corners** (lighter blue interior over the darker sea band), the island sitting on
  its top edge. The corners are at ≈(90,380), (500,230), (720,340), (320,470) in the
  viewport crop — identical geometry to r5's `87-wm-zoom-out-day.png`, only with a slightly
  softer rim. Pushing the wheel further out ends with the camera inside the water volume
  (the surface fills the frame), so the last reachable framing is the diamond.
- Evidence: `110-wm-zoomed-noon.png`, `112b-wm-far-noon-vp.png`, `112c-check.png`
  (noon), `113-wm-far-dawn.png` (dawn), `111b-wm-zoommax-noon-vp.png` (past the end).
- Undo: not applicable (camera only).

## Finding 8 — N25 SunShafts: wired, and the configuration warning is exact, but no shaft pixel is isolatable

- Severity: minor (effect unverified; no shipped scene has an occluder)
- Steps: `04_world_mood` → Pixel World panel → **Sun Shafts + Add**; read `prop sun`;
  hover the node's scene-tree warning icon for the text; set
  `directional_shadow_max_distance` 400 (was 120), `alpha` 0.6, `count` 64, noon,
  zoom in two notches, Animate OFF; capture with the node visible and hidden.
- Expected: shafts appear once the camera is inside the sun's shadow distance.
- Actual: `sun` is wired to `WorldMood/Sun` (`DirectionalLight3D`). The scene-tree warning
  icon's tooltip reads verbatim:

  > **Node configuration warning:**
  > • The camera is 225 m from what it looks at, past the sun's shadows
  > (directional_shadow_max_distance 120 m; shafts fade out from 72 m): there is no shadow
  > to cut shafts, so nothing shows. Raise the sun's directional_shadow_max_distance to at
  > least 375 m or move the camera closer.

  After raising the shadow distance to 400 m and zooming in, the warning tooltip was
  **stale** (still "120 m … 72 m … 375 m" after reselecting the node — the node only
  recomputes warnings on its own property edits, and the harness `set-prop` bypasses
  that). Toggling the node visible/hidden changed **0 of 397664** viewport pixels at
  alpha 0.6 / count 64, so no shaft is isolatable even inside the new shadow reach. This
  scene has no tall occluder, and the "no shadow caster" half of the warning did not
  appear (the terrain counts as an occluder); the visible-shaft claim therefore remains
  unproven.
- Evidence: `121-sunshafts-added.png`, `122-shafts-inspector.png`,
  `123-shafts-warning-tooltip.png`, `124-shafts-noon-on.png` vs `125-shafts-noon-off.png`
  (`changed=0`), `126/127-shafts-warning-*.png` (stale text).
- Undo: Add is one step ("Add PixelSunShafts" in History); the property writes above were
  harness `set-prop` calls.

## Finding 9 — N6/N31 boot log noise unchanged: 16 + 16 + 2, boot only

- Severity: minor (Output noise)
- Steps: count the markers in `editor.log` at boot and after a full pass of selects,
  scene opens, panel builds, resizes and effect toggles.
- Actual: **16** `Plane cannot be normalized` + **16** `Vector3 cannot be normalized` +
  **2** `ERROR: Condition "!res" is true. Returning: false` at
  `create_frustum_points (rendering_light_culler.cpp:682)`, all inside lines 13–79 of the
  log, i.e. before the harness served its first request. The whole pass (fresh select,
  section/page sweeps, 1280/1024 resizes, `04_world_mood` open, Sun Shafts add, presets,
  weather/fireflies edits) added **no further lines** (log still 79 lines, 0
  `SCRIPT ERROR`, 0 `panic`). So the noise is boot-only and is **not** produced by
  selecting a `Pixel*` node — it is already counted before any selection.
- Evidence: `/tmp/opencode/qa/sessions/r6-e3/editor.log` (kept); counts 16/16/2.
- Undo: not applicable.

## New issues

1. **First-build page-tab overflow (Finding 1)** — the tab grid is built with one column
   per page before the strip has its width, so the first Renderer page shows `Debug Views`
   clipped by 42 px until the strip is rebuilt. Same root cause family as N1 (first-frame
   layout), narrower symptom.
2. **Drag saturation on strip number fields (Finding 3)** — a 6 px drag reaches the max and
   a slow 12 px drag does nothing. Needs a real-mouse confirmation before filing.
3. **Feature-switch Ctrl+Z is dead while the popup is open (Finding 2)** — known, still true.
4. **Water plane edges/corners still visible (Finding 7)** — the round-2 "fades on a
   circle" fix is not visible in this build on `04_world_mood`.
5. **SunShafts configuration warning does not refresh** when `directional_shadow_max_distance`
   changes (it still reports 120 m / 375 m after the sun is set to 400 m). Cosmetic, but it
   sends the user to the wrong number.

## Covered

- [x] N1 first-selection rects (fresh session, first select, reselect, 2D→3D) — layout now
      settles without a round-trip; measured rail 39×732, strip 731×175, panel 250×732.
- [x] Pixel/effects page tabs at 1600/1280/1024, all 15 rail sections (`rail_sweep.py`).
- [x] r4 #19 feature switches: toggle → Escape → Ctrl+Z reverts the project setting and
      History shows `Turn Clouds feature off`; Ctrl+Z with the popup open is inert.
- [x] Clamps: Pixel Size 16, Darken 1.0, Ray Alpha 4.0, Sky Coverage 1.0, Swell H. 2.0,
      Fireflies Count 1024; one drag = one undo step + Ctrl+Z for each.
- [x] r4 #36 smoke plume (317 px, 24×22 patch, crops).
- [x] r4 #37 dawn/dusk/noon means on `04_world_mood` and `hex_island`.
- [x] r4 #38 water edge at low orbit / zoom-out, noon and dawn — still a straight-edged
      diamond with corners.
- [x] N25 SunShafts: added via the panel, `sun` wired, exact warning text captured, 0 px
      change on visible/hidden at alpha 0.6 / count 64.
- [x] N6/N31 boot noise: 16 plane + 16 vector + 2 `create_frustum_points`, boot only.
- [x] Animate OFF static baseline is byte-identical (0 px).
- [x] Regression: Pixel Preview / Animate pressed states (blue icon + lifted background,
      `toggled` true/false in the dump, `130`–`132-preview/animate-*.png`).
- [x] Regression: Pixel World **+ Add** keeps the selection on `PixelRenderer` and the
      rail/strip open (`121-after-add.json`, History entry `Add PixelSunShafts`).
- [x] Regression: lighting-preset undo clears the tint — Dawn changes 99.2 % of the
      viewport (`changed=394492/397664`), one `Ctrl+Z` returns **byte-identical**
      (`changed=0/397664`, `160`–`162-preset-*.png`, undo action `Apply Dawn preset`).
- [x] Regression: fireflies at night — `time_of_day 0`, Animate OFF, node visible vs hidden
      = **932 px** in a 130×102 patch over the island; 2× crop shows dozens of yellow dots
      (`145/146-ff-*.png`, `147-ff-on-crop2x.png`).
- [x] Regression: weather — Rain and Storm show white streaks + splash clusters at the
      overview, Snow shows large flakes (`150`–`154-wm-*.png`); weather transitions need
      **Animate ON** (Animate off freezes them, so the first Rain capture was empty haze).
- [x] Log scan: no new `ERROR` / `SCRIPT ERROR` from any UI action; the 79-line log is all
      boot (16/16/2).

## UI feedback (for FEEDBACK.md)

Measured at 1600×900 unless stated; window-content coordinates.

1. **Strip width vs header width (Finding 1).** On the first build the strip header is
   `773×37` inside a 731 px strip (`PinnedHeader [329,678 773×37]`, `clipped: true`) and the
   tab grid is `670×33`. The header should be clamped to the strip width before the tab grid
   picks its column count — i.e. build the page tabs from the *rounded* strip width, not from
   the sum of the tab labels.
2. **Page-tab grid spacing.** The 5×2 grid is fine at 1600 (`405×66`), but the grid starts
   103 px into a 731 px strip (the `SectionTitle` cell), so short strips (1024, 632 px)
   still show only `Renderer…Sun` on row 1 and `Sheets…Debug Views` on row 2 — readable but
   the strip body below it is a single 33 px row at 1024. A 3-column grid at ≤700 px would
   leave more body.
3. **1024×600 strip.** `PixelizerAttributes [221,454 632×99]`: header 66 px, body 33 px,
   everything else behind the strip's own vertical scrollbar (max `scroll_v` 74). The wheel
   over the strip moves 37 px per 3 notches, so revealing the third row takes ~8 notches.
   Consider pinning a minimum 140 px strip when a page has 3 rows.
4. **Number fields.** A drag either does nothing (≤4 px, slow) or jumps to the maximum
   (≥10 px, any direction) — never a proportional change (Finding 3).
5. **SunShafts warning.** The text is precise but does not refresh after the sun's
   `directional_shadow_max_distance` changes (still reports `120 m … 375 m` when the sun is
   at 400 m), so it sends the user to a stale number.
6. **Water plane.** The "fade on a circle" rim is not visible: the plane still ends in a
   four-corner rectangle (Finding 7). A radial alpha fade in the water shader (screen-space
   circle centred on the plane, radius = plane half-size) would hide the corners.

## Harness notes

- `capture` (in-engine, awaits a frame) repeatedly exceeded the 120 s RPC timeout once
  Animate was on and two editors shared the host; `shot` (X11 grab, window at 0,0 so the
  same window-content coordinates apply) was reliable. The last half of this pass used
  `shot` + a PIL crop to the viewport (`329,121 731×544`).
- Full `ui --root all` dumps are ~14 MB each; the kept dumps are trimmed
  (`03-first-key.json`, `02-first-key.json`) and the rest were deleted.
- Popup item rects are not reported by `popups`; the menu is a fixed 235×648 at (1215,115)
  and the preset rows are reachable by absolute coordinates (Dawn y≈544, Noon y≈567,
  Dusk y≈590; "Features" at (1300,753), submenu rows 23 px apart from y≈580).
