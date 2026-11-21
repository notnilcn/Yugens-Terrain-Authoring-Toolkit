# Runtime / demo pass r4 — MarchingSquaresTerrain

Plain game sessions (no editor harness) on display `:44`, llvmpipe software rendering,
2026-11-21. Six launches: `demo/hex_island.tscn` at 1280×720 and 1024×600, then
`tutorial/01_flat_playground`, `02_island`, `03_pixel_look`, `04_world_mood` at 1280×720.
Every scene loaded and rendered; no crash or hang. All runtimes stopped at the end of the
pass (`qa/runtime.sh stop`), no game or Xvfb process on `:44` remains.

Observed frame rate (F3 header): 2–3 fps at 1280×720 on the default overview (2 fps at
1024×600), 7–14 fps with the camera looking down at the ground (less overdraw, not a
correctness issue). Startup is dominated by llvmpipe: the Godot splash was still visible at
28 s on the 1024×600 launch while two editor sessions were running; the scene first rendered
at ~40 s. At 1280×720 the scene rendered between 18 s and 30 s.

Evidence: 71 PNGs in `qa/screenshots/r4-runtime/`.

## Camera orbit lets the view go under the island; terrain underside fills the screen

- Severity: warning
- Steps: 1) Launch `demo/hex_island.tscn` at 1280×720. 2) Middle-drag `500,300 → 500,470` to
  pitch to a top-down view (screenshot 28). 3) Middle-drag `500,450 → 500,230` to pitch the
  other way. 4) Inspect the frame, then middle-drag `500,230 → 500,420` to recover.
- Expected: pitch/zoom clamps (or camera collision) keep the camera above the terrain and
  outside props at every orbit angle.
- Actual: the camera passes below the island. The top of the frame is filled by two huge flat
  gray slabs (the underside of the terrain/ruin), the player is seen through a rock as a blue
  wireframe-like silhouette, and the water is a flat blue field. The view is unusable until
  the camera is orbited back. This is the orbit-range counterpart to the earlier zoom
  findings: zoom-in, zoom-out and pitch can all put the camera inside geometry.
- Evidence: `qa/screenshots/r4-runtime/37-hex-noon-normal.png` (broken underside view),
  `qa/screenshots/r4-runtime/28-hex-orbit-down.png` (top-down state before the pitch),
  `qa/screenshots/r4-runtime/38-hex-noon-recovered.png` (recovered); `game.log` has no error.
- Undo: not applicable (runtime demo has no undo manager)

## Finite water plane is exposed when orbiting low or zooming out

- Severity: minor
- Steps: 1) `tutorial/02_island.tscn` at 1280×720. 2) Right-drag `500,350 → 700,420` to lower
  the orbit. 3) Wheel down ×4 at the centre. 4) Repeat in `tutorial/04_world_mood.tscn`
  (right-drag, then wheel down ×3).
- Expected: the sea continues to the horizon or fades into the sky; the island does not read
  as a diorama floating in a void.
- Actual: the water is a finite rectangular sheet. At a low orbit its hard diagonal edges and
  flat pale-blue background are plainly visible; the island appears to float. At dusk in
  tutorial 04 the background beyond the sheet is bright pink (the sky near the sun), so the
  hard sheet edge is even more obvious. Shore foam is still correct at the default view.
- Evidence: `qa/screenshots/r4-runtime/53-t02-orbit.png`,
  `qa/screenshots/r4-runtime/54-t02-zoom-out.png`,
  `qa/screenshots/r4-runtime/60-t04-orbit.png`,
  `qa/screenshots/r4-runtime/61-t04-zoom-out.png`,
  `qa/screenshots/r4-runtime/51-t01-pan-a.png` (pale corner beyond the water in tutorial 01).
- Undo: not applicable (runtime demo has no undo manager)

## F3 diagnostics panel covers 41% of a 1024×600 window and clips its lower content

- Severity: ui
- Steps: 1) Launch `demo/hex_island.tscn` at 1024×600 (fresh launch, or live-resize from
  1280×720). 2) Press F3. 3) Compare with 1280×720 and with a still smaller 800×480 window.
- Expected: the tuner fits the window, or scrolls with an obvious scrollbar and leaves enough
  game visible to use the controls against the scene.
- Actual: the panel is a fixed ~420 px wide sheet starting at x≈604 (59% of 1024), leaving
  604 px of game. Its content is cut at the window bottom: at 1024×600 the top of the panel
  shows the feature list and the `PixelWeather: Weather` preset row, but the transition /
  ground / strength fields are below the window; only wheel-scrolling reaches them. The
  scrollbar is a thin 8 px bar at the far right, easy to miss, and the panel remembers its
  scroll position across close/reopen. At 800×480 the panel covers the player completely and
  the panel itself is cut mid-`Weather` header. No collapse/“fit” control exists. At
  1280×720 and 1600×900 the same panel is comfortable and scrolls normally.
- Evidence: `qa/screenshots/r4-runtime/70-hex1024-f3-default.png` (clipped, hint under panel),
  `qa/screenshots/r4-runtime/40-hex-1024-f3.png`,
  `qa/screenshots/r4-runtime/41-hex-1024-f3-top.png`,
  `qa/screenshots/r4-runtime/42-hex-800x480-f3.png` (panel covers the player),
  `qa/screenshots/r4-runtime/07-hex-f3-open.png` (1280 comparison),
  `qa/screenshots/r4-runtime/43-hex-1600x900-f3.png` (1600 comparison).
- Undo: not applicable (runtime demo has no undo manager)

## F1 help is one small, unbacked line and is overlapped by F3 at 1024×600

- Severity: ui
- Steps: 1) Launch `demo/hex_island.tscn` at 1280×720, wait for the startup hint to fade.
  2) Press F1. 3) Repeat at 1024×600 with F3 open. 4) Press F1 again while it is visible and
  watch it for ~20 s.
- Expected: a readable controls overlay that stays legible over bright terrain and is not
  covered by other HUD panels (docs/11: “F1 — Show the controls again”).
- Actual: one ~15 px single line pinned to the bottom edge,
  `WASD move · Shift walk · Space jump · drag / arrows orbit · wheel zoom · F3 tuner · F1 help`,
  drawn directly over grass/sand with a thin dark outline. It is legible at 1:1 over mid-tone
  ground and poor over bright sand/sky; there is no backing panel or multi-line help. At
  1024×600 the F3 panel starts at x≈604 and cuts the line after “drag / arrows orbit”, so
  most of the hint is hidden. F1 is one-shot (re-shows and restarts the fade), not a toggle,
  and there is no other way to dismiss it. At 2–4 fps the fade took visibly longer than the
  authored 10 s + 1.5 s (still present 12 s after a re-show, gone by 18 s).
- Evidence: `qa/screenshots/r4-runtime/02-hex-1280-f1.png` (over grass),
  `qa/screenshots/r4-runtime/03-hex-f1-second-press.png` (second F1, still shown),
  `qa/screenshots/r4-runtime/06-hex-hint-gone.png` (faded),
  `qa/screenshots/r4-runtime/69-hex1024-f1.png`,
  `qa/screenshots/r4-runtime/70-hex1024-f3-default.png` (hint clipped by the tuner),
  `qa/screenshots/r4-runtime/68-hex1024-glide-c.png` (hint gone after the startup fade).
- Undo: not applicable (runtime demo has no undo manager)

## Noon on the sandy plateau is washed out at a top-down camera angle

- Severity: minor (view-dependent; possibly intended palette, but it loses legibility)
- Steps: 1) In `demo/hex_island.tscn`, use F3 → PixelDayNight → Noon (time 12:05).
  2) Walk W for ~5 s onto the pale sand plateau. 3) Orbit to a near-top-down view.
- Expected: the sand keeps some texture/contrast at noon; paths and water channels remain
  distinguishable.
- Actual: the ground becomes almost uniform pale yellow with white channels and scattered
  speckles; only the rocks, the player and the dotted path edge keep contrast. It reads as
  overexposed and the terrain shape is hard to see. Morning (10:42, screenshots 01/65) keeps
  green grass and readable detail, and night is readable, so this is specific to noon on the
  bright sand at this angle.
- Evidence: `qa/screenshots/r4-runtime/36-hex-noon-topdown.png`,
  `qa/screenshots/r4-runtime/38-hex-noon-recovered.png`,
  `qa/screenshots/r4-runtime/44-hex-1600x900.png`,
  `qa/screenshots/r4-runtime/45-hex-restored-1280.png` (same washed-out ground after resize).
- Undo: not applicable (runtime demo has no undo manager)

## Harness note: `qa/drive.py info/windows/resize` cannot drive a runtime session

- Severity: minor (QA tooling, not the addon)
- Steps: 1) Start a plain game with `qa/runtime.sh start ...`. 2) Run
  `python3 qa/drive.py info` (or `windows`, `resize`, `window-resize`) without `--port`.
- Expected: a runtime-aware path, or an explicit “no harness” error.
- Actual: `info`/`windows`/`resize`/`window-resize` are RPC commands and default to port 8791
  (another session's editor harness in this environment); they block for ~30 s and then fail,
  and they must not be used in a game session. `shot`/`click`/`key`/`drag`/`wheel`/`move` work
  over X11 as intended. Window resizing was therefore done with a direct X11
  `ConfigureWindow` on the “Marching Squares Terrain (DEBUG)” window; the game handled it
  correctly at every size tested (see Covered).
- Evidence: this pass; `qa/runtime.sh` env sets `MST_WINDOW_MATCH` but no harness port, and
  `qa/drive.py` dispatches those four commands through `Rpc` before the X11 path.
- Undo: not applicable

## Covered

- [x] `demo/hex_island.tscn` at 1280×720: waterfall, ruin, knight, rocks, grass, path, sky and
      cloud deck, campfire flame and smoke, birds; thick dark outlines and a coherent limited
      palette; no letterboxing, crisp nearest-neighbour pixel blocks.
- [x] Startup camera glide observed at 1024×600: frames at 40 s, 48 s and 58 s show the camera
      continuing to pull back/up and the knight shrinking (66 → 67 → 68), so the glide lasts
      well past first render; no jump or snap.
- [x] F1: startup hint, fade (~10 s authored; slower at 2–4 fps), re-show with F1; one-shot,
      not a toggle; F3 does not re-show it (68 → 69).
- [x] F3: open/close; header (fps, draws, prims, pixel size, low-res, texel, camera, outlines,
      palette, rays, weather, time, features); Effects/Render/Scene tabs; features list with
      All on / Defaults; weather section; Day/Night section via wheel scroll; Rescan.
- [x] Weather at runtime via F3 presets: Clear → Rain (wet 0.19, darkening sky, rain streaks) →
      Snow (wet 0.51, snow 0.02, white flakes) → Storm (wet 1.00, heavy rain, darker ground) →
      Clear. Transitions blend over the configured 6 s.
- [x] Time of day: Night via F3 (time 00:02, night 1.00, dark blue scene, campfire glow,
      fireflies visible as warm dots) with storm and with clear weather; Noon (12:05, time
      0.504). Dawn/Dusk not selected (see Not tested).
- [x] Camera: middle-drag orbit, left-drag orbit, wheel zoom in/out, right-drag orbit and WASD
      pan in the tutorials, WASD / Shift+W / D movement in hex (held 2–2.5 s; terrain and
      camera follow), Space jump submitted (no unambiguous airborne still at 3 fps), Esc leaves
      the game running with no pause/menu overlay.
- [x] Live window resizing on the game window: 1280×720 → 1024×600 → 800×480 → 1600×900 →
      1280×720. Each time the pixel renderer reflowed to the new aspect (low-res 430×242 →
      344×202 → 270×162 → 536×302 at pixel size 3), kept the 3×3 pixel grid and no blur, and
      the HUD/diagnostics stayed sharp. No min/max size was hit; 800×480 was accepted.
- [x] F3 at 1024×600 and 800×480: panel is scrollable and all controls are reachable; clipping
      and coverage findings recorded above. F3 at 1600×900 fits the full top section.
- [x] `tutorial/01_flat_playground.tscn`: green plateau, terraced hill, surrounding sea, foam
      and caustics; matches `docs/media/example-01.png`; right-drag orbit, wheel zoom-out, WASD
      pan work; finite water-plane edge visible when panned/zoomed.
- [x] `tutorial/02_island.tscn`: sandy shore, cliffs, trees, rocks, foam and caustics; matches
      `docs/media/example-02.png`; right-drag orbit and wheel zoom work.
- [x] `tutorial/03_pixel_look.tscn`: ruin close-up with orange beams, column, rubble and grass;
      matches `docs/media/example-03.png`; right-drag orbit and wheel zoom-in work; outlines
      crisp.
- [x] `tutorial/04_world_mood.tscn`: dusk island with darker water and sky; matches the mood of
      `docs/media/example-04.png` (runtime is a little darker/olive); right-drag orbit and
      wheel zoom work; 3 s apart frames show cloud/water animation.
- [x] Log scan for all six launches: only
      `WARNING: Could not set V-Sync mode, as changing V-Sync mode is not supported by the
      graphics driver.` at `platform/linuxbsd/x11/gl_manager_x11.cpp:372` (known llvmpipe
      noise). No `ERROR`, `SCRIPT ERROR`, shader compile failure, missing resource or null-node
      line. Logs: `/tmp/opencode/qa/runtime/<r4-hex|r4-hex1024|r4-t01..r4-t04>/game.log`.
- [x] Undo: Ctrl+Z is **not applicable** to the runtime demo/tutorial scenes — they are plain
      games with no editor undo manager and no undo UI. The only “reset” surface found is F3's
      `All on` / `Defaults` feature buttons, which are presets, not undo. No undo-like
      behaviour is exposed.
- [x] All runtimes stopped; no game process or Xvfb `:44` left running.

## Not tested

- Weather `Cloudy` and `Foggy`, and a visible lightning strike on `Storm` (the `Lightning
  Enabled` checkbox was on, but no bolt was caught in the stills at 2–4 fps).
- Day/Night presets `Dawn` and `Dusk`; custom Time Of Day scrub and Time Scale/Override
  fields; weather Transition/Ground/Strength/wind edits.
- Individual feature toggles (Clouds, Outlines, …); only `All on` was clicked. Sun shafts,
  dust motes and the lantern light volume were not isolated from the F3 list.
- Precise jump height/airborne frame; walk animation; the fox and `demo/courtyard.tscn`
  (outside this brief).
- Live resize through a window manager (no WM on the Xvfb display) or maximize; only direct
  X11 `ConfigureWindow` was used. `qa/drive.py resize/window-resize` is harness-only (see
  harness note).
- F1 at 800×480 and at 1600×900; F3 clipping at other aspect ratios (e.g. portrait).
- F3 `Object inspector` Select Object / Clear and mesh picking; pointer-to-world accuracy was
  only checked indirectly via F3 preset buttons and camera input.
- Long-run stability (> a few minutes per scene), memory/CPU drift, audio.
- Undo/redo: not applicable to this runtime surface.
