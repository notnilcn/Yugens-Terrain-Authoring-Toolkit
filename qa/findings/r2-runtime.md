# Runtime / demo pass r2 — MarchingSquaresTerrain

Plain game sessions (no editor harness) on display `:44`, llvmpipe software rendering,
2026-11-21. Run stopped early by the orchestrator for machine load; scenes 02–04 and the
courtyard were not reached.

Evidence from this pass is `qa/screenshots/r2-runtime/06-*.png` … `36-t01-1280.png` and
`37-t01-orbit-zoom.png`. Numbers 37–43 were written by a concurrent agent, and 01–05 are
from the interrupted first attempt.

Observed frame rate: 2–4 fps; window 1280×720 unless stated. At 1024×600 the first launch
stayed on the Godot splash for >70 s while three editor sessions were running (no log error);
it rendered normally afterwards.

## Camera zoom-out at a low orbit angle clips into the grass layer
- Severity: warning
- Steps: 1) Open `demo/hex_island.tscn` at 1280×720 and let the scene settle. 2) Middle-drag
  upward to lower the camera (orbit). 3) Wheel down 4 steps at the centre of the view.
- Expected: zooming out keeps the camera above the terrain and the player readable.
- Actual: the view fills with giant green grass blades/polygons; the camera is inside the
  grass at ground level. Four wheel-up steps restore the normal view. This is the opposite
  direction from the 2026-11-20 courtyard finding (zoom-in), so the camera lacks collision at
  both ends of the range.
- Evidence: `qa/screenshots/r2-runtime/17-hex-orbit.png` (low orbit before zoom),
  `qa/screenshots/r2-runtime/18-hex-zoom-in.png` (2 wheel-up steps, still coherent),
  `qa/screenshots/r2-runtime/19-hex-zoom-out.png` (broken view); `game.log` has no error.
- Undo: not applicable (runtime)

## F3 Pixel tuner is clipped at 1024×600
- Severity: ui
- Steps: 1) Launch `demo/hex_island.tscn` at 1024×600. 2) Press F3.
- Expected: the panel fits the window, or scrolls with the full weather/day-night controls
  reachable and the game still visible.
- Actual: the panel starts at x≈604 (59% of the 1024-wide window) and is cut at the bottom
  edge in the middle of `PixelWeather: Weather` – the `Weather` row is half visible and the
  transition/strength/precipitation fields are below the window. The panel leaves only 604 px
  of game and the hint line runs under it. At 1280×720 the same panel is usable and scrolls.
- Evidence: `qa/screenshots/r2-runtime/35-hex-1024-wait.png` (F3 open, clipped);
  `qa/screenshots/r2-runtime/10-hex-f3-1280.png` (1280 comparison).
- Undo: not applicable (runtime)

## F1 help is one tiny, low-contrast line with no backing panel
- Severity: ui
- Steps: 1) Launch `demo/hex_island.tscn` at 1280×720. 2) Read the hint at startup; press F1
  after it has faded.
- Expected: a readable controls hint (docs/11: “F1 — Show the controls again”).
- Actual: a single ~15 px line of off-white text with a dark outline, pinned to the bottom
  edge directly over grass/rocks: `WASD move · Shift walk · Space jump · drag / arrows orbit ·
  wheel zoom · F3 tuner · F1 help`. It is legible at 1:1 but small, and its contrast depends on
  the terrain behind it. There is no panel, no multi-line help, and no way to dismiss it
  except waiting 10 s for the fade. The previous report’s claim that “F1 toggles it cleanly”
  is wrong: F1 only re-shows the hint and restarts the 10 s fade (see the correction below).
- Evidence: `qa/screenshots/r2-runtime/11-hex-1280.png`,
  `qa/screenshots/r2-runtime/13-hex-f1-reshown.png`,
  `qa/screenshots/r2-runtime/23-hex-hint-faded-2.png` (faded);
  `qa/screenshots/r2-runtime/35-hex-1024-wait.png` (line partly under the F3 panel).
- Undo: not applicable (runtime)

## F3 scroll position jumps away from Weather after picking a preset
- Severity: minor
- Steps: 1) Open F3 at 1280×720. 2) Scroll to `PixelWeather: Weather` and click `Rain`.
- Expected: weather changes and the panel stays on the weather section.
- Actual: Rain activates (header `weather Rain (wet 0.39, snow 0.00)`, rain streaks and a
  darker sky), but the panel jumps to the `Snow`/`Lightning` fields, so the Weather row and
  the Clear preset are no longer in view. The panel also remembers this scrolled position
  when closed and reopened with F3.
- Evidence: `qa/screenshots/r2-runtime/29-hex-f3-weather-top.png` (before),
  `qa/screenshots/r2-runtime/31-hex-rain2.png` (after, scrolled to Snow/Lightning).
- Undo: not applicable (runtime)

## Correction to the previous runtime report: F1 does not toggle
- Severity: minor (reporting correction, not an addon defect)
- Steps: 1) Launch `demo/hex_island.tscn`; wait for the hint to fade (10 s + 1.5 s).
  2) Press F1. 3) Wait through the fade again; press F3.
- Expected (from `rust/crates/mst_demo/src/hud.rs`): F1 calls `show_hint()`; the hint fades
  after `show_seconds`; F3 does not affect it.
- Actual: matches the source. F1 re-shows the hint (frame 13); it does not hide it when
  already visible (frames 08/09, taken inside the 10 s window); F3 does not re-show it
  (frame 24). The 2026-11-20 finding “F1 toggles it cleanly” should be removed from the
  issue list.
- Evidence: `qa/screenshots/r2-runtime/16-hex-hint-check-late.png` (faded),
  `qa/screenshots/r2-runtime/23-hex-hint-faded-2.png`,
  `qa/screenshots/r2-runtime/24-hex-f3-reopen.png` (F3 open, hint absent).
- Undo: not applicable (runtime)

## 1024×600 launch stays on the Godot splash for over a minute under load
- Severity: minor (environmental; recorded, not confirmed as an addon defect)
- Steps: 1) Launch `demo/hex_island.tscn` at 1024×600 while three editor sessions are running.
- Expected: the ~20–30 s startup seen at 1280×720.
- Actual: the window showed the Godot splash at 28 s, 40 s and ~55 s; it rendered the scene
  after ~70–80 s. The process used ~105% CPU and `game.log` had no error. At 1280×720 the same
  scene was ready in ~25 s, so this is most likely llvmpipe/editor contention rather than a
  1024-specific hang.
- Evidence: `qa/screenshots/r2-runtime/32-hex-1024.png`,
  `qa/screenshots/r2-runtime/34-hex-1024-f3.png` (splash), then
  `qa/screenshots/r2-runtime/35-hex-1024-wait.png` (scene loaded).
- Undo: not applicable (runtime)

## Covered

- [x] `demo/hex_island.tscn` at 1280×720: full-window render, no letterboxing, crisp
      nearest-neighbour pixel blocks, sky/cloud deck, shoreline, outlined props/character.
- [x] Intro camera settles: at 30 s the player was partly below the bottom edge; ~35 s later
      the same view is centred and stable (06 vs 07). Not reported as a defect.
- [x] F1 hint: visible at startup, fades after ~10 s, F1 re-shows, F3 does not (source
      cross-checked; see correction above).
- [x] F3 at 1280: diagnostics header (fps, draws, prims, pixel size, low-res, texel, camera,
      outlines, palette, rays, weather, time, features), Object Inspector, Effects/Render/Scene
      tabs, feature toggles, weather section, scrollable panel.
- [x] Weather: Rain activates via the F3 preset list; rain streaks, darker sky, wet 0.39;
      weather state and time of day advance in the header (09:46 → 11:37).
- [x] Camera: middle-drag orbit changes yaw/pitch; wheel zoom in/out; zoom-out clipping
      captured (finding above); view restored with wheel up.
- [x] Motion evidence pair 14/15 (clouds/birds move between frames).
- [x] `tutorial/01_flat_playground.tscn` at 1280×720: loads without errors; plateau, water,
      foam and hill match `docs/media/example-01.png`. Camera controls were attempted but the
      session was killed by the orchestrator before a valid post-orbit capture (37 is splash).
- [x] Log scan: only the known llvmpipe `Could not set V-Sync mode` warning; no `ERROR` or
      `SCRIPT ERROR`, no shader errors, no crash message. The t01 session was killed
      externally, not a crash.
- [x] Runtimes stopped/relinquished: `r2-runtime`, `r2-hex1024`, `r2-t01` stopped by this pass;
      `r2-t02` was on `:44` when the orchestrator killed that display. No runtime of this pass
      was left running.

## Not tested

- Tutorials `02_island`, `03_pixel_look`, `04_world_mood` and `demo/courtyard` — not reached
  before the orchestrator stopped the pass for machine load.
- 1920×1080 window size; live window resizing; other aspect ratios; HiDPI.
- WASD/Shift/Space jump, Escape behaviour, and walk animation (only orbit/zoom and F3 clicks
  were exercised).
- Weather types other than Rain (Cloudy, Foggy, Snow, Storm, lightning) and the F3 Day/Night
  buttons; the Day/Night section was not found in the scroll range visited.
- Water-plane edge when zoomed out (tutorial 02), courtyard dusk sun shafts, courtyard
  zoom-in through the fox (previous finding) — not re-confirmed this pass.
- Fireflies, dust, lantern cones, bird landing, long-run stability, memory/CPU drift.

## Log scan

- `demo/hex_island.tscn` 1280, 1024: `WARNING: Could not set V-Sync mode ...` (llvmpipe) only.
- `tutorial/01_flat_playground.tscn`: same single warning; no other lines.
- No `ERROR`, `SCRIPT ERROR`, shader compile failure or crash line in any `game.log`.
