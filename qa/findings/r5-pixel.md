# R5 Pixel look + Effects + Pixel World — findings (T3)

Session `r5-t3`, display `:53`, port `8853`; build `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`
(merged r4base); Godot 4.7.2, llvmpipe, editor 1600×900 (resized to 1280×720 and 1024×600).
Scenes: `demo/hex_island.tscn` (boot, rich), `tutorial/04_world_mood.tscn` (sky/daynight/weather/
water/fireflies/birds). Log `/tmp/opencode/qa/sessions/r5-t3/editor.log`.
Screenshots `qa/screenshots/r5-pixel/`; helpers `qa/scripts/r5-pixel/`.

Animate is ON by default in a fresh session? **No** — Pixel Preview is ON, Animate OFF at boot.
Every "static" diff below was taken with Animate OFF (verified byte-identical over 1.5–3 s), so the
pixel counts are real effect contributions, not cloud/water drift (animation noise floor is
21–22 % of viewport pixels between two frames when Animate is ON).

---

## Finding 1 — Selecting a Pixel node leaves the whole Pixelizer UI at zero size until a 2D→3D main-screen switch
- Severity: error (the Pixel look toolchain is unreachable)
- Steps: 1) Start a fresh session (boots `demo/hex_island.tscn`, Pixel Preview on). 2) Click
  `PixelRenderer` in the Scene dock (or `drive.py select --path PixelRenderer`). 3) Read the
  control tree / screenshot.
- Expected: the pixelizer left rail, the bottom attribute strip and the right Pixel World panel
  appear (as in `67-after-2d3d-fixed.png`).
- Actual: nothing appears. The controls exist but are collapsed: `PixelizerToolbarRail`
  `[287,121 39x0]`, `PixelizerAttributes` `[287,121 0x175]`, `PixelWorldPanel` `[287,121 250x0]`.
  The Inspector shows the PixelRenderer properties, the viewport stays full-size, and the whole
  bottom strip area is empty. A one-time **main screen 2D → 3D** round trip fixes it for the rest
  of the session (strip `[329,678 731x175]`, panel `[1063,121 250x732]`); after that, switching
  terrain↔renderer rebuilds the strip correctly.
- Reproducible: 3/3 fresh-session attempts (initial boot + harness select + Scene-dock click in a
  second, restarted session). Not a one-off race with scene loading: it persisted 9 s after the
  scene finished loading.
- Evidence: `qa/screenshots/r5-pixel/09-pixelrenderer-settled.png` (selected, no UI),
  `63-fresh-first-select-pixelrenderer.png`, `64-fresh-after-wait.png`,
  `66-tree-select-pixelrenderer.png` (Inspector populated, no pixelizer UI),
  `67-after-2d3d-fixed.png` (after 2D→3D, UI present). UI dumps: `ui.py Pixelizer` output in the
  session transcript (39x0 / 0x175 / 250x0).
- Undo: not applicable.

## Finding 2 — Pixel World panel and Inspector are squeezed to a sliver at 1024×600
- Severity: ui (presets, Select/Add and Wire unreachable)
- Steps: 1600×900 → select `PixelRenderer` → `resize 1024 600` → select `PixelRenderer` again →
  capture + `rects.py`.
- Expected: the panel stays usable (or collapses to a tab/drawer), viewport ≥480×300.
- Actual: `PixelWorldPanel` = `[864,154 24x399]` — **24 px wide**; the Inspector dock shrinks to
  ~136 px and its rows are clipped (`Pixel Si…`, `Ass…`). The 3D viewport is `640x221` (below the
  480×300 floor). The strip fits horizontally (`[221,378 640x175]`) but its page tabs and fields
  are only partly visible. No crash. This is r4 issue 24 reproducing in a different shape (r4 saw
  the panel pushed to x≈968; here it is squeezed to 24 px).
- Evidence: `qa/screenshots/r5-pixel/33-1024-pixelrenderer.png`; rects `PixelWorldPanel
  x=864 y=154 w=24 h=399`, `Node3DEditorViewport x=221 y=154 w=640 h=221`.
- Undo: not applicable.

## Finding 3 — Page-tab grid is clipped once the strip is narrower than 731 px
- Severity: ui (the 4th tab column is unreachable)
- Steps: 1280×720, select `PixelRenderer`, Renderer section; and at 1600×900 look at the strip's
  own overflow.
- Expected: all page tabs reachable at the smallest supported window (1024×600); no horizontal
  scrolling for a 10-tab group.
- Actual: at 1280×720 `PixelizerAttributes` is `414x175` while `SectionPages` is `378x99` at
  x=432 → right edge 810 > 743, so the 4th column (`Rays`, `Depth Fade`) is cut at the strip edge;
  at 1600×900 the content (`MstFlow` 717x204) is taller than the 175 px viewport, so the last
  field row (`Crisp Edges`, `Wide Ids`, `Id Edges`, `Fold Limit`) is `clipped: true` and a
  horizontal scrollbar exists (`HScrollBar` `[303,839 996x14]`, `value=-166`) but is not visually
  obvious. Page-tab row y is stable at 678 across sections (Renderer 3 rows, Day/Night 1 row,
  Weather 2 rows) — the r4 vertical jump does **not** reproduce.
- Evidence: `31-1280-pixelrenderer.png`, `32-1280-strip.png`,
  `12-page-renderer.png`, `13-page-outlines.png`; dump `SectionPages [432,498 378x99] clipped`.
- Undo: not applicable.

## Finding 4 — Startup log noise: 16 plane-normalization warnings + 2 engine ERRORs
- Severity: warning (Output/error badge noise; not a functional failure)
- Steps: open `demo/hex_island.tscn` with the pixel preview on; read `editor.log` without touching
  anything.
- Expected: only the known V-Sync / missing-UID / LSP-port noise (that is what every r4 session
  log has: 0 such lines).
- Actual: at boot, **16×** `WARNING: Plane cannot be normalized …` + **16×** `WARNING: Vector3
  cannot be normalized …` and **2×** `ERROR: Condition "!res" is true. Returning: false` from
  `create_frustum_points (servers/rendering/rendering_light_culler.cpp:682)`. Every r5 session log
  checked (`r5-t1`, `r5-t2`, `r5-smoke`, `r5-smoke2`, `f1`, `r5-t3`) has exactly 16/16/2; every
  r4 session log has 0/0/0, so this is new in the r5 line. The flood repeats at scene load; no
  panic, no functional break observed.
- Evidence: `qa/screenshots/r5-pixel/editor.log-before-restart.txt` lines 13–45;
  `grep -c "Plane cannot be normalized" /tmp/opencode/qa/sessions/r5-t3/editor.log` = 16.
- Undo: not applicable.

## Finding 5 — Default PixelSmoke plume is still effectively invisible
- Severity: minor (effect appears broken at authored defaults)
- Steps: `demo/hex_island`, Smoke node (Emitting on, Rate 3.0, Start/End 0.1/0.45), Animate OFF,
  capture; `set-prop visible false --path Smoke`, capture; diff.
- Expected: a visible plume near the hearth.
- Actual: smoke contributes **78 pixels** in a 10×16 px patch at (425,315)–(435,331) of the
  731×554 viewport; the 4× crop shows 2–3 grey haze pixels behind the fire sparkles. r4 issue 36's
  smoke half therefore still reproduces.
- Evidence: `72-smoke-static-on-a.png`, `73-smoke-static-on-b.png` (byte-identical),
  `74-smoke-static-off.png`, `75-smoke-default-crop4x-on.png`,
  `76-smoke-default-crop4x-off.png`; diff `changed=78/404974`.
- Undo: not applicable (visual only; the smoke strip fields are unchanged).

## Finding 6 — Dawn preset (and Time 0.75) still flattens the whole frame
- Severity: minor (palette/lighting; possibly intentional but it erases the water/land split)
- Steps: `04_world_mood`, Pixel World → **Dawn** (`time_of_day` 0.72 → 0.2705), capture; then
  Time 0.75 (dusk), capture.
- Expected: a warm dawn/dusk sky with the water still reading as water and the island keeping its
  green.
- Actual: Dawn gives mean RGB (177.0, 177.5, 187.5) — a uniform pale lavender wash; sky, sea and
  island are within a few units of each other, island green is gone. Dusk (0.75) gives
  (170.5, 133.2, 132.3), a pink-mauve wash. The r4 "salmon/pink" tint is gone (undo now clears it,
  see below), but the washed-out/flattened look remains. Noon (0.5) is clean blue with a green
  island — no white-out in this scene.
- Evidence: `49-wm-preset-Dawn.png`, `49-wm-preset-Noon.png`, `58-shafts-dusk-on.png`
  (time 0.75), `48-wm-time-0.25.png` / `48-wm-time-0.5.png` (time-only captures: 0.25 mauve,
  0.5 blue).
- Undo: preset undo restores the properties **and** the rendered preview (Finding "fixed" below).

## Finding 7 — The finite water plane still reads as a floating rectangle at low orbit / zoom-out
- Severity: minor (view-dependent diorama look; r4 #38)
- Steps: `04_world_mood`, Time 0.5, middle-drag the viewport down (pitch under), then wheel-down ×5
  to zoom out; `shot` the viewport.
- Expected: sea continues to the horizon or fades into the sky.
- Actual: a hard-edged rectangular water quad is visible (lighter blue) over a darker blue
  background, with the island sitting on its top edge; the magenta frustum lines are visible too.
  The island reads as a diorama on a slab. Same at night.
- Evidence: `87-wm-zoom-out-day.png` (clear day), `86-wm-zoom-out.png` (night),
  `85-wm-orbit-down.png` (intermediate).
- Undo: not applicable (camera).

## Finding 8 — Added SunShafts is now wired to the Sun, but no shafts were visible
- Severity: minor (wiring fixed; the visual claim remains unproven)
- Steps: `04_world_mood`, Pixel World → Sun Shafts → **+ Add**; read `prop sun --path SunShafts`;
  `set-prop alpha 0.6`, `set-prop count 64`; toggle `visible` at Time 0.5 and 0.75 with Animate
  off.
- Expected: shafts streaming from the sun.
- Actual: `sun` is **wired** to `WorldMood/Sun` (`DirectionalLight3D`) — r4 issue 20 is fixed, no
  warning triangle, no `wired 0 reference(s)` message. But toggling the node on/off changed no
  isolatable shaft pixels at either time of day (the only diffs were the animated water dither,
  32–50 % of the viewport with identical means). The node is a small island with no canopy /
  occluder at the default camera, so this is reported as unverified rather than absent.
- Evidence: `53-wm-sunshafts-added.png`; `54-shafts-on.png` / `55-shafts-off.png`,
  `56-shafts-strong-on.png` / `57-shafts-strong-off.png`,
  `58-shafts-dusk-on.png` / `59-shafts-dusk-off.png`; `prop sun → WorldMood/Sun`.
- Undo: Add is undoable (r4 verified the same path); the property writes above were harness
  `set-prop` calls, not undo tests.

---

## Finding 9 — Feature switches still record no undo entry
- Severity: minor (project setting; outside undo by design, but undocumented)
- Steps: `04_world_mood`, Pixel World menu → **Features** → uncheck **Clouds**; then `undo-state`.
- Expected: either an undo entry or an explicit “this is a project setting” note in the UI.
- Actual: the viewport changes immediately (45.7 % of pixels, mean +2.3/channel) but `undo-state`
  stays `action: "", undo_count: 0, version: 1`; Ctrl+Z creates nothing. The r4 “Ctrl+Z stays dead
  until the viewport is clicked” half could not be isolated (there was no prior history entry to
  undo at that moment), so only the “not undoable” half is confirmed.
- Evidence: `89-features-before.png`, `90-features-clouds-off.png` (diff 45.7 %),
  `91-features-after-ctrlz.png`.
- Undo: no entry (by design for a project setting).

## r4 issues: still reproduces / fixed

| # | r4 issue | Verdict in this build |
|---|---|---|
| 1 | **Critical**: selecting PixelRenderer at 1280×720 crashes the editor | **Not reproduced.** 1280×720 select → ping ok, full UI, `31-1280-pixelrenderer.png`. Same at 1024×600. |
| 4 | `PixelizerEditorPlugin::on_float` re-entrant borrow panic | **Not reproduced** — 0 `panic`/`bind_mut` lines in the whole session log. |
| 10 | Sliders accept absurd out-of-range values | **Fixed (6/6 fields).** All read back `max_value` = tooltip max and stop there: Pixel Size 3→**16** (range 1–16), Darken 0.546→**1.0** (0–1), Ray Alpha 0.998→**4.0** (0–4), Sky Coverage 0.504→**1.0** (0–1), Water Swell H. 0.121→**2.0** (0–2), Fireflies Count 64→**1024** (0–1024). No spikes/white-out. |
| 11 | One Pixel Size drag = two undo steps | **Fixed.** undo_count 4→5; one Ctrl+Z 16→3 (one step), redo 3→16. |
| 14 | Lighting-preset undo leaves the preview tinted | **Fixed.** With Animate off: Dawn changes 94.1 % of viewport pixels; after Ctrl+Z the frame is **byte-identical** to before (`changed=0/404974`), `time_of_day` 0.2705→0.4. |
| 18 | Renderer page-tab row jumps vertically | **Fixed** at 1600×900/1280: tab row y=678 for every section (1–3 rows of tabs). (Finding 3 covers the clipped 4th column.) |
| 20 | Added SunShafts has a null Sun | **Fixed** — `sun` wired to the scene's `DirectionalLight3D`. |
| 19 | Feature switches not undoable; Ctrl+Z dead after the popup | **Half reproduces** — no undo entry for the Clouds toggle (Finding 9); the dead-shortcut half was not isolatable this pass. |
| 21 | Fireflies render nothing at night | **Fixed — confirmed in both scenes.** hex_island, Animate on, 6 frames: warm/hot pixels **624** with fireflies at size 1.5 vs **34** with the node hidden (the lantern pool). 04_world_mood, night, Animate off, authored 100/0.12: **1426** pixels differ with the node visible vs hidden (bbox 123×62 over the island; 4× crop shows dozens of yellow dots). |
| 24 | Pixel World panel off-screen at 1024×600 | **Still reproduces** — panel `24x399` (Finding 2). |
| 32 | Pixel Preview / Animate have no pressed state | **Fixed.** Pressed background 56→66 (preview) / 60→71 (animate), icon turns accent blue `(87,160,255)`, and `toggled:true` in the dump. |
| 33 | “+ Add” closes the pixelizer UI | **Fixed.** After adding Sun Shafts, the selection stayed on PixelRenderer and the rail/strip/panel stayed open. |
| 36 | Rain invisible at overview; smoke plume invisible; Sky Coverage can change 0 px | **Rain fixed** — six weather types captured; rain streaks + splashes clearly visible at the ~60 m overview (`51-wm-day-Rain.png`), storm too, plus a captured **lightning flash** (frame mean 94→106/137/171). **Coverage fixed** — 0.504→1.0 changes 65 % of viewport pixels. **Smoke still reproduces** (Finding 5). |
| 37 | Dawn/noon wash out the image | **Partially reproduces** — no longer pink, now a pale lavender-grey flat wash (Finding 6); noon is clean in `04_world_mood`. |
| 38 | Finite water plane edge visible | **Still reproduces** — hard rectangular water quad at low orbit / zoom-out (Finding 7): `87-wm-zoom-out-day.png`. |
| 7 | Attribute strips uncapped; viewport 39 px | **Improved** — the pixelizer strip is fixed at 175 px and the viewport is 731×554 (1600), 414×341 (1280), 640×221 (1024). At 1024 the height is still below the 480×300 floor, but it is a dock/size issue, not the strip. |

## Covered

- [x] Pixel Preview toggle: viewport grid/axes appear/disappear; pressed state visible
      (`01-preview-on.png`, `02-preview-off.png`).
- [x] Animate toggle: OFF → two 3 s frames byte-identical (0 px); ON → 17.7 % of viewport pixels
      change in 3 s (`03/04-animate-off`, `06/07-animate-on`).
- [x] Pixel World menu present (Pixel Preview / Animate / Pixel World) at 1600/1280/1024.
- [x] Pixel World panel: Lighting Preset grid (Dawn/Noon/Dusk/Night/Storm/Snow) all apply and
      change the sky; Wire button, Select rows, + Add rows, 27-row scroll content.
- [x] Lighting presets Dawn/Noon/Dusk/Night: each changes the frame; one `Apply * preset` undo
      step; Ctrl+Z restores properties **and** rendering.
- [x] Renderer section: 10 page tabs (Renderer, Outlines, Color/Palette, Rays, Sun, Sheets,
      Clouds, Depth Fade, Debug Views) visited; fields present (`12-page-renderer.png`,
      `13-page-outlines.png`).
- [x] Page-tab row stable at y=678 for Renderer (3 rows), Day/Night (1), Weather (2), Water (2).
- [x] Clamp audit (6 fields, drag +80 px each): Pixel Size, Darken, Ray Alpha, Coverage, Swell H.,
      Firefly Count all stop at their declared max; values read back through the harness.
- [x] One drag = one undo step for Pixel Size (int), Darken (float), Ray Alpha, Coverage,
      Fireflies Count; Ctrl+Z + Ctrl+Shift+Z verified for Fireflies Count, Ray Alpha, Pixel Size.
- [x] Sky & Clouds: Coverage drag = one step (`Set Sky & Clouds Coverage`), viewport 65 % changed.
- [x] Water: Swell H. drag = one step (`Set Water Swell H.`), max 2.0, water surface changes (2.3 %
      pixels from the overview), no spikes.
- [x] Fireflies: strip (Count/Area XYZ/Seed/Size/Wander/Blink/Glow/Night Only), count 1024→
      night swarm visible on hex_island (624 vs 34 warm pixels, on/off, 6 frames) and on
      04_world_mood at the authored 100/0.12 (1426 px on/off, `81/82`, 4× crop `83-wm-ff-on-crop4x.png`).
- [x] Weather: all six types captured at Time 0.5 (`51-wm-day-*.png`); each is one
      `Set Weather Weather` undo step; lightning flash captured during Storm
      (`52-storm-watch-5.png`, mean 94.2/113.9/135.7 → 106.3/137.3/171.1).
- [x] Snow: falling flakes clearly visible.
- [x] Day/Night: `time_of_day` 0.0 / 0.25 / 0.5 / 0.75 captured, each a distinct frame
      (99.8 % pixels changed between adjacent steps).
- [x] SunShafts: added from the Pixel World panel; sun wired; UI stays open; undo path unchanged.
- [x] 1280×720 and 1024×600: no crash, no panics, editor responsive after both.
- [x] Log scan: no addon `ERROR`/`SCRIPT ERROR` attributed to a UI action; the only new lines are
      the boot-time plane/frustum noise (Finding 4).

## Not tested

- [ ] PixelPalette presets / LUT bake, PixelObjectProfile arrays, PixelOutlineColors 256-entry
      arrays (same gap as r4).
- [ ] Erosion and WildFlowers add/run/undo in this build (r4 covered them; not re-driven here).
- [ ] Birds count/speed, ParticleField look, LightVolume energy, Smoke strip fields (visual only,
      no field edits).
- [ ] SunShafts in a scene with an occluder (canopy/pillars) — the visibility question is open.
- [ ] 1920×1080 / 1366×768; only 1600×900, 1280×720, 1024×600 were driven.
- [ ] Feature-switch undo (r4 #19) and per-switch gating — not exercised this pass.

## Harness notes

- At ~11 GB/14 GB used with three editors running, in-engine `capture` occasionally exceeded the
  120 s RPC timeout (frame await). `shot` (X11 grab) worked as a fallback for the last captures;
  the crops are the same window-content coordinates (`--crop` uses the window origin).
- Popup items have no `rect` in `popups`; the README formula (pos + 8 + (i+0.5)·(h−16)/n) worked
  for the 6-item Weather dropdown (`qa/scripts/r5-pixel/pickmenu.py`).
- EditorSpinSlider: drag may land on the value text and do nothing; clicking the number and typing
  (`qa/scripts/r5-pixel/setnum.sh`, which uses `key period`) is reliable. Do **not** send `Ctrl+A`
  outside a focused LineEdit — it opened the Scene dock's "Create New Node" dialog.

## UI feedback (for FEEDBACK.md)

Measured rects at 1600×900 unless stated; window-content coordinates.

1. **Pixelizer strip** `PixelizerAttributes` `[329,678 731x175]` at 1600, `[329,498 414x175]` at
   1280, `[221,378 640x175]` at 1024. Content (`MstFlow`) is 717×**204** → one field row is always
   clipped at 1600 (the strip needs 204 px or an internal vertical scroll). The horizontal
   scrollbar exists (`[303,839 996x14]`, value `-166`) and auto-scrolls the content, which is
   disorienting: the page-tab grid slides left when the strip rebuilds.
2. **Page-tab grid** `SectionPages` is a 4-column GridContainer sized to its content (378 px for
   Renderer). At 1280 the strip is 414 px wide but the grid starts at x=432 → columns 3–4
   (`Rays`, `Depth Fade`) are cut. Suggested: 3 columns + wrap, or a capped-width tab bar with a
   `»` overflow.
3. **Pixel World panel** fixed 250 px, `[1063,121 250x732]` at 1600 (content ~940 px, 9 rows
   below the fold, including Sun Shafts/Lights/Add entries); `[864,154 24x399]` at 1024. No
   splitter, no collapse tab. Suggested: collapse to a 32 px icon rail or a tab, and remember a
   220–420 px drag width.
4. **Zoom/Perspective menu button** at `[339,131 115x33]` overlaps the pixelizer rail visually —
   the rail is 39 px and the viewport menu starts at x=339, so the rail's tooltips open over the
   viewport menu.
5. **Pixel Preview / Animate pressed state** is only a 10-unit background lift (56→66, 60→71) plus
   an accent-blue icon. It is measurable but subtle at 33 px height; a 2 px accent border on the
   active mode would be clearer.
6. At 1024×600 the Inspector shrinks to ~136 px and truncates every property row (`Pixel Si…`,
   `Ass…`); it should keep a 260 px minimum or auto-collapse to tabs.
