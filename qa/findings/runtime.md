# Runtime / demo pass — MarchingSquaresTerrain

Plain game sessions (no editor harness) on display `:44`, llvmpipe software rendering,
2026-11-20. All six scenes booted; all three window sizes rendered; no crash or hang.
52 screenshots in `qa/screenshots/runtime/`. All runtimes stopped at the end of the pass.

Observed frame rate: 2–4 fps at 1280×720 and 1024×600, 3 fps at 1920×1080 (llvmpipe), so
timing-dependent checks below are slower than the configured seconds.

## Camera zoom clips through the ground and the fox
- Severity: warning
- Steps: 1) Open `demo/courtyard.tscn` at 1280×720; wait for the default camera. 2) Middle
  wheel up twice at the centre of the view.
- Expected: zoom-in stops before the camera enters the terrain or the character; the fox
  stays readable.
- Actual: the camera drops below the grass plane and into the fox; the view becomes a flat
  dark-green plane with the trees on the horizon, and the fox is drawn as a cyan/white
  wireframe-like silhouette (backfaces/outline colour). One wheel-out restores the normal
  view.
- Evidence: `qa/screenshots/runtime/39-courtyard-orbit.png` (broken close-up),
  `qa/screenshots/runtime/40-courtyard-restored.png` (normal after zoom-out).
- Undo: not applicable

## F1 help is one tiny, low-contrast line with no backing panel
- Severity: ui
- Steps: 1) Open `demo/hex_island.tscn`. 2) Press F1 at 1280×720, 1920×1080 and 1024×600.
- Expected: a readable controls hint/overlay (docs/11: “F1 — Show the controls again”).
- Actual: a single ~10 px line of grey text pinned to the bottom edge
  (`WASD move · Shift walk · Space jump · drag / arrows orbit · Wheel zoom · F3 tuner ·
  F1 help`) drawn directly over bright grass. It is readable at 1:1 but small, and contrast
  varies with the terrain behind it; there is no panel/background and no multi-line help.
  The line fits at all three sizes and F1 toggles it cleanly.
- Evidence: `qa/screenshots/runtime/02-hex-1280-f1.png`,
  `qa/screenshots/runtime/31-hex-1920-f1.png`,
  `qa/screenshots/runtime/36-hex-1024-f1.png`; crop `/tmp/opencode/qa/hint-crop-2x.png`
  (not committed).
- Undo: not applicable

## F3 Pixel tuner is cramped and clipped at 1024×600
- Severity: ui
- Steps: 1) Open `demo/hex_island.tscn` at 1024×600. 2) Press F3.
- Expected: the tuner panel fits the window, or scrolls with a visible scrollbar / can be
  collapsed.
- Actual: the panel starts at x≈604 (59% of the 1024-wide window) and its content is cut at
  the bottom edge: only the top of the `PixelWeather: Weather` section is visible and the
  Clear/Cloudy/… buttons and fields are below the window. The file works, but the user has
  to wheel-scroll a panel that leaves almost no room for the game. At 1280 and 1920 the
  same panel is fine and scrolls; the Day/Night buttons there change time correctly.
- Evidence: `qa/screenshots/runtime/35-hex-1024-f3.png` (clipped),
  `qa/screenshots/runtime/03-hex-1280-f3.png` (fine at 720p).
- Undo: not applicable

## Water plane edge is exposed when zooming out; far sea can read as flat sky
- Severity: minor
- Steps: 1) Open `tutorial/02_island.tscn` and zoom out (wheel down ×2). 2) Orbit a middle
  drag. 3) In `demo/hex_island.tscn`, orbit toward the coast and zoom out ×5.
- Expected: water continues to the horizon or fades into the sky; waves/foam stay visible at
  the shoreline.
- Actual: tutorial 02 shows hard diagonal edges of the finite water plane with flat light
  blue beyond it. In hex_island the zoomed-out coast shows a large uniform blue field with a
  hard horizon and no readable waves or shoreline foam at that distance, unlike the clear
  foam/caustics in tutorial 01/02 at their default distances.
- Evidence: `qa/screenshots/runtime/44-t02-orbit.png`,
  `qa/screenshots/runtime/33-hex-1920-sea-view.png`.
- Undo: not applicable

## Possible sun-shaft band renders dark in the courtyard dusk view
- Severity: minor
- Steps: 1) Open `demo/courtyard.tscn` and wait for the default dusk view.
- Expected: `PixelSunShafts` adds bright shafts through the trees/pillars (docs/11, docs/09).
- Actual: the upper-left sky has a large, straight-edged dark navy wedge with no bright
  shaft visible. It may be a cloud shadow or a shaft sheet failing to add light, but the
  advertised bright shafts are not visible in the default runtime view. Orbiting/zooming
  changes the wedge, so it is view-dependent.
- Evidence: `qa/screenshots/runtime/38-courtyard-1280.png`,
  `qa/screenshots/runtime/40-courtyard-restored.png`.
- Undo: not applicable

## Covered

- [x] `demo/hex_island.tscn` at 1280×720, 1920×1080, 1024×600; full-window render, no
      letterboxing/cropping inside the game window; crisp nearest-neighbour pixels (no blur).
- [x] F1 show/hide and legibility at all three sizes (finding above on size/contrast).
- [x] F3 diagnostics + Pixel tuner at 1280 and 1920: FPS/draws/prims/pixel size/low-res/camera/
      outlines/palette/rays/time/features; Effects/Render/Scene tabs; features list; weather
      section; Day/Night section; scrollable panel.
- [x] Changed weather to Rain via the F3 list (visible rain streaks, wet ground, sky darkening)
      and back to Clear; changed time of day via Night and Noon buttons (night 1.00, time
      00:08 vs noon 0.501, 12:02).
- [x] Camera orbit with middle-drag and left-drag, wheel zoom in/out, WASD movement,
      Shift+W walk, Space jump (attempted), Esc (no quit/menu; game continues).
- [x] `demo/courtyard.tscn`: dusk courtyard with fountain, pillars, round trees, fox, rocks,
      sun-shaft candidate; orbit/zoom work.
- [x] `tutorial/01_flat_playground.tscn`: green plateau + hill + water + foam/caustics; matches
      docs/11 and `docs/media/example-01.png`; orbit/zoom work.
- [x] `tutorial/02_island.tscn`: island, sandy shore, cliffs, trees/rocks, foam and caustics;
      matches `docs/media/example-02.png`; orbit/zoom work.
- [x] `tutorial/03_pixel_look.tscn`: ruin close-up with orange beams, column, rubble, grass;
      matches `docs/media/example-03.png`; orbit/zoom work.
- [x] `tutorial/04_world_mood.tscn`: dusk island matching `docs/media/example-04.png`; day/night
      advanced to night during the pass (island very dark, water deep blue).
- [x] Runtime visuals observed: thick dark outlines on terrain/props/character, coherent
      palette, sky/cloud deck, water + shoreline foam in 01/02, rain, night lighting, campfire
      flames and grey smoke puffs, birds in the hex sky, warm fire/lantern glow at night.
- [x] `game.log` for every runtime: only `Could not set V-Sync mode` (known llvmpipe noise);
      no `ERROR`/`SCRIPT ERROR`/shader warnings, no crash, no hang.
- [x] All runtimes stopped (`qa/runtime.sh stop`); no game process or Xvfb `:44` left running.

## Not tested

- Editor-only paths: Pixel Preview, Animate, Pixel World, inspector sections, undo/redo,
  texture/preset panels (covered by the editor passes, not this runtime pass).
- Weather types other than Rain/Clear: Cloudy, Foggy, Snow, Storm (and lightning). F3 was only
  available in `hex_island`; tutorial 04 has no runtime tuner.
- Object Inspector “Select Object” / mesh picking and precise click-to-world aiming; only
  relative orbit/zoom and F3 button hits were used to check pointer alignment.
- Jump could not be confirmed from a still: at 2–4 fps no clearly airborne frame was captured
  after Space; movement (W/Shift+W) was confirmed.
- Firefly visibility at the default overview distance (night frames are consistent with
  fireflies but no unambiguous warm glowing dot cluster was captured).
- Dust motes, lantern cone, and whether the courtyard wedge is a sun shaft or a cloud shadow.
- Live window resizing/maximise (each size was a fresh launch at that resolution), HiDPI/text
  scaling, and UI at other aspect ratios.
- Long-run stability (> a few minutes per scene) and memory/CPU drift.
