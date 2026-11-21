# Area 5 (r4) — Effects: Sky, Day/Night, Weather, Water, atmosphere, erosion, wild flowers

Session: `r4-effects`, display `:35`, port `8815`, editor 1600×900, Compatibility/llvmpipe.
Scenes: `res://tutorial/04_world_mood.tscn` (PixelSky, PixelDayNight, PixelWeather, PixelWater,
Fireflies, Birds, Dust), `res://demo/hex_island.tscn` (adds Smoke, Lantern; Erosion, Wild Flowers
and SunShafts added through the Pixel World menu). Pixel Preview and Animate were on for the pass.

## Strip sliders jump far outside their field range on a small drag
- Severity: warning
- Steps: 1) Select `PixelSky`; drag the **Coverage** bar ~20 px right. 2) Select `PixelWater` →
  **Waves**; drag **Swell H.** ~20 px right. 3) Select `Fireflies`; drag **Count** ~15 px right.
- Expected: the value stays inside the tooltip range (Coverage 0–1, Swell H. 0–2, Count 0–4096)
  and the viewport shows a plausible change.
- Actual: Coverage becomes **325.76** (also 365.96 on a second run), Swell H. becomes **108.93**
  and Count jumps from **100 to 4096** in one gesture. The water mesh explodes into giant
  vertical spikes/holes at 108.93. The strip read-out and the node property keep the out-of-range
  value; the drag is recorded as one undo step.
- Evidence: `qa/screenshots/r4-effects/67-sky-coverage-out-of-range.png` (Coverage 325.76),
  `qa/screenshots/r4-effects/32-water-swell-drag.png` (Swell H. 108.93, spiked water),
  `qa/screenshots/r4-effects/05-sky-coverage-after-undo.png`; log action names
  `Set Sky & Clouds Coverage`, `Set Water Swell H.`, `Set Fireflies Count`.
- Undo: works — one Ctrl+Z restores 0.4205 / the previous Swell H. / Count 100; redo reapplies.

## Added PixelSunShafts has a null Sun even though the scene has one
- Severity: warning
- Steps: 1) Open `demo/hex_island.tscn`. 2) Pixel World menu → **Sun Shafts** (the menu and the
  right-panel "+ Add" both say references are wired). 3) Select the new `SunShafts`; read
  Inspector → **Sun**; watch the viewport with Animate on.
- Expected: **Sun** is assigned to the scene's `Sun` (`DirectionalLight3D`) and shafts render.
- Actual: **Sun** shows `Assign…` (empty); `prop sun` returns `null`; the Scene tree row shows a
  configuration-warning triangle. Alpha 0.011 / count 12 leave the viewport unchanged (the thin
  yellow lines are gizmos/renderer god rays).
- Evidence: `qa/screenshots/r4-effects/52-sunshafts-added.png`; `prop sun → null`, existing
  `Sun` node listed by `nodes DirectionalLight3D`.
- Undo: adding the node is one undo step (`Add PixelSunShafts`).

## Fireflies render nothing at night, even enlarged
- Severity: warning
- Steps: 1) Open `04_world_mood`. 2) Apply the **Night** preset (Time 0.9505). 3) Select
  `Fireflies` (Count 100, Size 0.12, Glow 2, Night Only 1) and capture. 4) Raise Count to 400
  and Size to 0.5; capture two frames 2 s apart; set **Night Only 0** and capture; brighten a
  crop 3×.
- Expected: a visible blinking warm-yellow swarm; Night Only 0 should show it in daylight too.
- Actual: no firefly pixels in any frame or in the brightened crop. The `Swarm`
  `MultiMeshInstance3D` exists (`call get_swarm` returns it) and the Fireflies feature is on;
  only cloud/water motion changes between frames. At the default 0.12/100 nothing is visible
  either.
- Evidence: `qa/screenshots/r4-effects/39-fireflies-noon.png`,
  `qa/screenshots/r4-effects/40-fireflies-night.png`,
  `qa/screenshots/r4-effects/41-fireflies-night-big.png`,
  `qa/screenshots/r4-effects/42-fireflies-nightonly0.png`,
  `qa/screenshots/r4-effects/43-fireflies-crop-bright.png`.
- Undo: Count drag is one step and Ctrl+Z restores 100; Size/Night Only were changed with the
  harness (not an undo test).

## PixelSmoke default plume is invisible; only hugely enlarged puffs show
- Severity: minor
- Steps: 1) Open `demo/hex_island.tscn`; select `Smoke` (Emitting on, Fire on, Rate 2.52,
  Start/End 0.1/0.45, position 14,5,-2). 2) Watch two frames 3 s apart with Animate on.
  3) Set Start Size 1.0, End Size 4.0, Rate 10, Fire off.
- Expected: a grey plume rising from the fire at the authored settings.
- Actual: at the authored values no grey puffs are visible (only tiny yellow fire sparkles);
  with Start/End 1.0/4.0 and Rate 10 a single white cloud-like puff appears above the building.
  Separately, **Pixel World → Smoke & Fire** in `04_world_mood` adds the node at
  (87.9, -0.8, 81.2) — ~40 m off the island and below the water plane — so a user must move it
  before seeing anything.
- Evidence: `qa/screenshots/r4-effects/46-smoke-hex-a.png`,
  `qa/screenshots/r4-effects/46-smoke-hex-b.png`,
  `qa/screenshots/r4-effects/47-smoke-hex-crop.png`,
  `qa/screenshots/r4-effects/48-smoke-big.png`,
  `qa/screenshots/r4-effects/45-smoke-added.png`.
- Undo: Add PixelSmoke is undoable (`Add PixelSmoke` → Ctrl+Z removes it → Ctrl+Shift+Z restores);
  the size/rate changes were made with the harness.

## Rain is invisible at the default overview camera distance
- Severity: minor
- Steps: 1) `04_world_mood`, Time 0.5, Weather **Rain**, Transition 0.2, Animate on.
  2) Capture at the default camera (~60 m). 3) Zoom to 23.4 m and capture.
- Expected: rain streaks are visible when the weather is Rain.
- Actual: at the overview distance the frame only darkens (mean 102.8 vs 114.4 Cloudy) with no
  discernible streaks; at 23.4 m the same settings show clear white rain streaks. No error log.
- Evidence: `qa/screenshots/r4-effects/25-weather-rain.png`,
  `qa/screenshots/r4-effects/26-weather-rain-close.png`,
  `qa/screenshots/r4-effects/24-weather-cloudy.png`.
- Undo: weather selection is one undo step; Ctrl+Z goes Rain→Snow, Ctrl+Shift+Z reapplies.

## Dawn preset / Time 0.27 washes water and terrain pink
- Severity: minor
- Steps: 1) `04_world_mood`, apply Pixel World **Dawn** preset (Time 0.2705, Coverage 0.3505).
- Expected: a warm dawn sky; water stays blue and terrain keeps its material colours.
- Actual: the entire viewport turns pale pink/magenta — sky, fog, water and terrain — and the
  island reads as a mustard blob. Time 0.5 clears it.
- Evidence: `qa/screenshots/r4-effects/19-preset-dawn.png`,
  `qa/screenshots/r4-effects/18-preset-before-dawn.png`.
- Undo: preset application is one step (`Apply Dawn preset`); Ctrl+Z restores Time 0.9 /
  Coverage 0.45, Ctrl+Shift+Z reapplies.

## Lighting Preset row is below the Pixel World panel viewport
- Severity: ui
- Steps: 1) Select a pixel node so the right-hand Pixel World panel is open. 2) Look for the
  **Lighting Preset** row (Dawn/Noon/Dusk/Night/Storm/Snow) without scrolling the panel.
- Expected: the row is visible/clickable at 1600×900.
- Actual: the row is laid out at y = 848–917 while the viewport/panel ends at y ≈ 620 (window
  900 px); it is reachable only after wheel-scrolling the panel. After 10 wheel steps the row
  moves to y = 337–448. The Pixel World **menu** version (top button) exposes the same presets
  directly, so the panel is the awkward path.
- Evidence: `qa/screenshots/r4-effects/08-pixel-daynight-strip.png` (panel shows only up to
  Sun Shafts), `qa/screenshots/r4-effects/17-pixel-world-scrolled-presets.png` (row visible
  after scrolling); ui dumps `ui/02-daynight-ui.json` (`Dawn [1067,848,72,33]`) and
  `ui/…` after scroll.
- Undo: not applicable.

## Feature switches are not undoable; Ctrl+Z is dead right after the Features popup
- Severity: minor
- Steps: 1) `demo/hex_island`, Pixel World menu → **Features** → uncheck **Clouds** (viewport
  changes; 12.8k pixels). 2) Press Ctrl+Z. 3) Click once in the 3D view and press Ctrl+Z again.
- Expected: project-switch changes are either recorded (undoable) or at least documented as
  outside undo; Ctrl+Z works immediately.
- Actual: the Clouds toggle records **no undo action** (`undo-state` still reports
  `scatter wild flowers`, unchanged version). Ctrl+Z does nothing until focus returns to the 3D
  view; after a viewport click Ctrl+Z undoes the previous scene action (scatter wild flowers).
  The switch itself never becomes undoable.
- Evidence: `qa/screenshots/r4-effects/62-features-before-clouds-off.png`,
  `qa/screenshots/r4-effects/63-features-clouds-off.png`,
  `qa/screenshots/r4-effects/64-features-clouds-undo-attempt.png`,
  `qa/screenshots/r4-effects/65-undo-after-3d-click.png`.
- Undo: broken for the feature toggle (not in the undo stack); normal scene undo works again
  after a viewport click.

## Session log
The only new `ERROR` line in `editor.log` is harness-induced, not an addon defect:
`ERROR: Error calling method from 'callv': 'PixelSmoke::set_position': Cannot convert argument 1 from Array to Vector3.`
It was produced while trying to move a node from the test driver. No `SCRIPT ERROR`, no
shader/compile error, and no addon `ERROR`/`WARNING` was emitted by any effect surface.

## Covered
- [x] PixelSky: strip (single "Sky & Clouds" page, no extra tabs); Coverage and Shadow fields;
      Clouds checkbox off/on; two frames 3 s apart with Animate on show drift (68,170 changed
      pixels in the viewport); Coverage drag out-of-range (finding) and one-step undo/redo.
- [x] PixelDayNight: Time page; times 0.0 / 0.2505 / 0.4995 / 0.75 / 0.9 captured; Sun & Fog
      and Rim Light pages; Sun Energy 0.2 change + undo; Dawn/Noon/Night presets (multi-property
      one-step undo/redo); undo after switching nodes/sections and after a 3D-view click.
- [x] PixelWeather: all six types (Clear, Cloudy, Foggy, Rain, Snow, Storm) captured; Weather,
      Snow, Particles, Rain, Lightning, Colors tabs; Weather selection one-step undo/redo;
      transition 0.2; storm flash observed in frame 06 (mean 85.6 vs 77.4 baseline); rain at
      23.4 m shows streaks; Clear restored.
- [x] PixelWater: Surface, Waves, Light, Caustics, Foam, Colors, Waterfalls pages; Swell H.
      drag (out-of-range, water mesh spikes) + one-step undo; visibility off/on + undo
      (`Toggle Visible`); Size/Subdiv/Depth fields present.
- [x] Fireflies: noon vs night, count/size/night-only experiments (invisible — finding);
      Count drag + one-step undo.
- [x] Birds: strip and Flock/Steering/Landing/Look tabs, In Editor on; noon frame shows the
      flock; two frames differ (38k pixels incl. cloud drift) — not isolated from other motion.
- [x] Smoke + fire: hex_island authored smoke, two frames, Fire on/off, Emitting; Add/undo of
      PixelSmoke; plume finding.
- [x] ParticleField (Dust): Field/Layers/Flipbook/Look tabs; Preset Dust Motes, Density 0.2,
      Beam 1, Emitting on; noon/night frames; no motes confidently isolated (God-Rays gated).
- [x] LightVolume (`Lantern`): full strip (Radius/Color/Energy/Flicker/Shadows/Glow/Pool/
      Source/Night Boost); day and night frames; orange pool visible at 13.7 m at night.
- [x] SunShafts: added from Pixel World; strip/Inspector; null Sun + warning icon (finding).
- [x] Erosion: added under `MarchingSquaresTerrain` via Pixel World menu; Basics/Droplet pages;
      30,000 droplets changed the terrain digest 3064656926 → 1246527589; Ctrl+Z → 3064656926;
      Ctrl+Shift+Z → 1246527589; action `terrain erosion`.
- [x] WildFlowers: added under terrain; Placement/Ground Water pages; Scatter produced 15,171
      flowers; undo `scatter wild flowers` → previous state; redo verified.
- [x] Feature switches: Features submenu lists 14 switches (all checked); Clouds off/on changed
      the viewport; non-undoable + focus loss finding.
- [x] Add-node undo: PixelSmoke add/undo/redo; SunShafts, Erosion, WildFlowers added through the
      Pixel World menu (each reported one undo action).
- [x] No addon `ERROR`/`SCRIPT ERROR`; the one ERROR line is the harness `set_position` call.

## Not tested / blocked
- PixelWater colour pickers (Shallow/Deep/Foam) and PixelSky deck colors: the embedded color
  popups do not take injected keyboard focus; not attempted this pass.
- Waterfalls/raised pools: Waterfalls page was screenshotted; no raised water cells were added.
- Birds count/height/speed changes (only viewed; the flock is tiny at this camera).
- LightVolume energy/colour/range changes (viewed only; day/night screenshot only).
- ParticleField layer/flipbook/look changes and a clean god-ray-shaft isolation (dust motes were
  not confidently visible).
- Snow accumulation over minutes and a longer lightning watch (only ~12 frames).
- Erosion with the default 30k vs the low 2,000-droplet no-op (2,000 produced no digest change).
- All 14 feature switches individually; only Clouds was toggled (water/weather were exercised
  through their node properties, not their gates).
- Undo of Add for SunShafts/Erosion/WildFlowers individually (Smoke add/undo is the proof that
  the Add path is undoable).

## Action × undo matrix
| Action | Undo step name | Ctrl+Z | Ctrl+Shift+Z |
|---|---|---|---|
| Sky Coverage drag | Set Sky & Clouds Coverage | works (restores 0.4205) | works (restores 325.76) |
| Sky Clouds checkbox | Set Sky & Clouds Clouds | works | works |
| Day/Night Time field | Set Day / Night Time (×5) | works (one step) | works |
| Day/Night Sun Energy | Set Day / Night Sun Energy | works | works |
| Day/Night preset | Apply Dawn / Noon / Night preset | works (all properties) | works |
| Weather type | Set Weather Weather | works (Storm→Snow) | works |
| Water Swell H. drag | Set Water Swell H. | works | not retested |
| Water visibility | Toggle Visible | works | not retested (eye toggle) |
| Fireflies Count drag | Set Fireflies Count | works (4096→100) | not retested |
| Add PixelSmoke | Add PixelSmoke | works (node removed) | works (node restored) |
| Add PixelSunShafts | Add PixelSunShafts | not individually retested | not retested |
| Erode 30k | terrain erosion | works (digest restored) | works (digest reapplied) |
| Scatter flowers | scatter wild flowers | works | works |
| Feature switch (Clouds) | — | no undo entry; shortcut dead until viewport click | not applicable |
