# Area 5 — Effects: Sky, Day/Night, Weather, Water, atmosphere, erosion, wild flowers

Scene: `res://demo/hex_island.tscn`, editor 1600×900, Compatibility/llvmpipe, Pixel Preview + Animate on.
Added `PixelSunShafts` (root), `MarchingSquaresErosion` and `MarchingSquaresWildFlowers`
(children of `MarchingSquaresTerrain`) through the Pixel World menu / `+ Add` rows; each add is one
undo step and Ctrl+Z/Ctrl+Shift+Z removes/restores it.

## Strip spin-slider drag jumps far outside the field range
- Severity: warning
- Steps: 1) Select `PixelSky`. 2) Drag the **Coverage** spin-slider bar ~20 px to the right.
  3) Select `PixelWater` → **Waves** page and drag **Swell H.** ~30 px.
- Expected: Coverage stays in 0–1; Swell H. stays in 0–2 (both tooltips state those ranges).
- Actual: Coverage becomes **377.40** and Swell H. becomes **142.85** in one gesture. The strip
  read-out and the node property keep the out-of-range value; the Inspector clamps its own display
  (Coverage shows 1.0/0.0 while the strip holds 377.4). The same behaviour reproduces on every
  strip slider tried (Sky and Water shader params).
- Evidence: `qa/screenshots/effects/10-sky-coverage-377.png`,
  `qa/screenshots/effects/46-water-waves-swell.png`; editor.log `Set Sky & Clouds Coverage`,
  `Set Water Swell H.`.
- Undo: works — one Ctrl+Z restores 0.5 / 0.121 (drag is a single merged step).

## Added PixelSunShafts is inert and its Sun reference is empty
- Severity: warning
- Steps: 1) Pixel World → **+ Add** next to Sun Shafts. 2) Select `SunShafts`; inspect **Sun**
  (empty). 3) Change **Alpha** from 0.011 to 0.0 and **Count** from 12 to 30.
- Expected: "+ Add" wires references; alpha 0 removes the shafts and count changes density.
- Actual: Sun is `null`; alpha 0 and count 30 leave the viewport unchanged. The thin yellow
  streaks visible before/after are the renderer's **God Rays**, not this node.
- Evidence: `qa/screenshots/effects/67-sunshafts-default.png`,
  `69-sunshafts-alpha0.png`, `70-sunshafts-count30.png`.
- Undo: adding the node is undoable; the field changes were made with `set-prop` (no undo step).

## PixelSmoke produces no visible plume in hex_island
- Severity: warning
- Steps: 1) Select `Smoke` (Emitting on, Fire on, position 14,5,-2, Rate 2.5, Start/End 0.1/0.45).
  2) Watch for 4 s with Animate on. 3) Raise Rate to 5 and Start/End to 1.0/2.0.
- Expected: a grey plume rising from the fire, with a flickering light.
- Actual: only a tiny orange flame pixel near the building base; no smoke puffs at either size,
  and no plume above the terrain. (The scene's authored transform was restored after a harness
  set-prop mistake; the result is the same at the authored position.)
- Evidence: `qa/screenshots/effects/58-smoke-a.png`, `61-smoke-fire-center.png`,
  `64-smoke-big.png`, `66-smoke-restored.png`. No addon error in the log.
- Undo: not applicable (property reads only).

## Lighting Preset row is under the bottom status bar at 1600×900
- Severity: ui
- Steps: 1) Select any pixel node so the Pixel World panel is open. 2) Look at **Lighting Preset**
  / Dawn / Noon / Dusk / Night / Storm / Snow / Wire Sun / Env Refs without scrolling the panel.
- Expected: all preset buttons are clickable.
- Actual: the row sits at y = 848–917 while the window is 900 px tall; the editor's bottom status
  bar covers it. Only a 1-px sliver of **Night** is reachable at y = 900; Dawn/Noon/Dusk do not
  respond until the panel is scrolled (wheel over the panel). The panel has a scrollbar, so the
  block is reachable, but the default view clips it.
- Evidence: `qa/screenshots/effects/07-sky-strip.png` (buttons at the bottom edge),
  `qa/screenshots/effects/19-daynight-preset-night.png`; UI dump rects
  `Dawn [1067,848,72,33]`, `Night [1067,884,72,33]` with window height 900.
- Undo: preset application is one undo step (`Apply Dawn preset` → time 0.2705, coverage 0.3505;
  Ctrl+Z restores 0.5/0.5, Ctrl+Shift+Z reapplies).

## Dawn / time 0.25 sky is saturated magenta with dense vertical stripes
- Severity: minor
- Steps: 1) Select `PixelDayNight`; set **Time** to 0.25, then apply the **Dawn** preset.
- Expected: a warm dawn sky (the preset's deck tint is warm white/orange in the docs).
- Actual: the whole sky is neon pink/magenta with a dense vertical dither pattern; terrain reads
  bright green/orange. At 0.4 the same sky is grey-blue and foggy.
- Evidence: `qa/screenshots/effects/16-daynight-time-0.25.png`,
  `qa/screenshots/effects/20-daynight-preset-dawn.png`.
- Undo: preset undo/redo works.

## Fireflies are invisible at their default size at a 30 m editor camera
- Severity: minor
- Steps: 1) `PixelDayNight.Time = 0.9` (night). 2) Select `Fireflies` (count 48, size 0.12,
  energy 3, night-only 1). 3) Raise size to 0.6 and count to 200.
- Expected: a visible blinking swarm at the documented 0.1–0.2 size.
- Actual: nothing visible at 0.12/48 even at 30 m; at 0.6/200 the swarm appears, but the dots read
  white/blue rather than the warm yellow tint at energy 3.
- Evidence: `qa/screenshots/effects/51-fireflies-night-close.png`,
  `54-fireflies-big-night.png`, `53-fireflies-noon-close.png` (invisible at noon, expected).
- Undo: property changes via `set-prop` (no undo test).

## Storm lightning not observed
- Severity: minor
- Steps: 1) `Weather` = Storm, Transition 0.2, Strength 1, Lightning enabled, Flash 2.5.
  2) Capture 15 frames over ~15 s with Animate on.
- Expected: periodic flashes/bolts.
- Actual: frame brightness stays 95.1–96.2; no bolt or flash captured (small max-brightness
  outliers only). Could be a long random interval; needs a longer watch to confirm.
- Evidence: `qa/screenshots/effects/31-storm-shot-1.png` … `31-storm-shot-5.png`.
- Undo: not applicable.

## Inspector value desyncs from the strip
- Severity: minor
- Steps: 1) Select `PixelSky`. 2) Change **Coverage** in the bottom strip to 0.8. 3) Read the
  Inspector's Coverage value.
- Expected: both show 0.8.
- Actual: the strip shows 0.8 and the node property is 0.8, while the Inspector still shows 0.0
  (after an earlier out-of-range drag it showed 1.0 while the strip held 377.4).
- Evidence: `qa/screenshots/effects/11-sky-coverage-08.png` (strip 0.8, Inspector 0.0).
- Undo: strip change undo works.

## Strip page tabs move when a page changes height
- Severity: ui
- Steps: 1) Select `PixelWater` (Surface page, 3 rows). 2) Click **Waves** (6 rows), then try to
  click **Light** at the same y position.
- Expected: the tab row stays put while switching pages.
- Actual: the bottom-anchored strip grows upward; the tab row jumps from y≈519 to y≈449 when a
  taller page is selected, so a second click at the old position lands on a field row/slider. The
  tabs themselves work when their current rect is used.
- Evidence: `qa/screenshots/effects/ui/45-water-page-*.png`; UI dump
  `Waves tab [535,449]` after switching vs `[535,519]` before.
- Undo: not applicable.

## Covered

- [x] PixelSky strip; coverage 0.8 vs 0.2; shadow 0 vs 1; deck on; two frames 4 s apart with
      Animate on prove cloud drift (14.7k changed pixels); undo of coverage drag (one step) and
      redo.
- [x] PixelDayNight times 0.0 / 0.25 / 0.5 / 0.75; Time / Sun & Fog / Rim Light pages; Dawn
      preset (multi-property single undo step; undo + redo verified); restored 0.4.
- [x] PixelWeather six types (Clear, Cloudy, Foggy, Rain, Snow, Storm) with visual differences;
      Weather / Snow / Particles / Rain / Lightning / Colors pages; Weather dropdown selection
      through the popup (one undo step, Ctrl+Z/Ctrl+Shift+Z verified); strength 0.2 vs 1.0;
      15 storm frames for lightning; restored Clear.
- [x] PixelWater: Surface / Waves / Light / Caustics / Foam / Colors / Waterfalls pages; water
      level raised to y=3 (floods the coast) and lowered to y=-2; Swell H. shader slider change
      and one-step undo; visibility off/on (84/85); restored position y=0.45.
- [x] Fireflies day (invisible, expected) vs night; count/size raise makes the swarm visible.
- [x] Birds: flock visible and moves between frames 4 s apart; Flock/Steering/Landing/Look tabs.
- [x] Smoke + Fire: strip + pages; fire on/off and emitting on/off; light/flicker fields.
      (Plume not visible — see finding.)
- [x] ParticleField (Dust): preset Dust Motes, density 0.2, beam 1, count 150; emitting off/on
      captures (no visible change at this distance).
- [x] LightVolume (`Lantern`): radius 3, glow on, night shot, energy 0.5 vs 0 (subtle).
- [x] SunShafts: added, strip, alpha 0 vs 0.0105, count 12 vs 30 (inert — see finding).
- [x] Erosion: added under terrain; terrain digest 3064656926 → 3692886391 after **Erode**
      (30k droplets); one undo step `terrain erosion`, undo/redo restore both digests.
- [x] WildFlowers: added under terrain; **Scatter** produces 15,434 flowers; `scatter wild flowers`
      is one undo step; undo/redo verified.
- [x] Feature switches: Pixel World → Features submenu (14 checkable switches); Weather off with
      Rain removes all rain and the overcast tint, Weather on restores it; all switches restored on.
- [x] Add/undo of SunShafts, Erosion, WildFlowers through the Pixel World menu (Erosion and Wild
      Flowers auto-parent under the terrain).
- [x] No addon `ERROR:`/`SCRIPT ERROR:` lines; only three harness `Cannot navigate to …` errors
      caused by typing into the FileSystem path field during a color-picker test.

## Not covered / blocked

- PixelWater shallow/deep/foam **color** changes: the embedded ColorPicker popup does not receive
  injected keyboard focus (typing the hex went to the FileSystem path field), and no swatch was
  clicked. Untested.
- Waterfalls/raised pools: the `Waterfalls` page was screenshotted but no cells were added.
- Storm lightning longer watch (only 15 s) and Snow accumulation over time.
- `LightVolume` at a player-distance camera (energy 0 vs 0.5 was too subtle to grade).
- Color pickers on PixelSky deck colors and Weather bolt color (same popup-focus limitation).
