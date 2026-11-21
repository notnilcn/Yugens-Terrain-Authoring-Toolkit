# Area 7/9 — runtime verification (r6, fresh runtime project)

Session `r6-e4` (display `:74`, runtime scene `res://demo/hex_island.tscn` at 1280×720) against
the merged build `r4base` **HEAD `e50088d`**, Godot 4.7.2, llvmpipe, no window manager on the
session display. Every number below is read back from the client-area screenshots with
`qa/scripts/r6-runtime/probe.py` (dock edges, dark-box fits) and `grid.py` (pixel-grid run
lengths); all `qa/*` commands ran with `/home/clintonnguyen/g/code_examples/rrr-wt/r4base` as the
working directory. Screenshots are the client area only (`cap.py` crops the 1920×1080 screen to
the game window); the window is at 0,0, so screenshot coordinates are client coordinates.

## Finding

**N2 (F3 tuner) and N3 (F1 help) are fixed and verified at 1024×600 and 800×480** — width cap,
two-axis scrolling, every field through Area/Rain Color reachable, click-to-collapse, backed
multi-line help that the tuner cannot reach at any tested size.

**r4 #23 (orbit) and #38 (sea, `hex_island`) pass. #38 still fails in the tutorials**: at the
minimum orbit pitch of `tutorial/orbit_camera.gd` (5°) and the farthest orthographic zoom
(`size` 400), the near edge of the finite sea plane is a perfectly horizontal line across the
full window with a uniform pale band below it — 232 px (32 %) at 1280×720 in both `02_island`
and `04_world_mood`. `hex_island` does not show it: its perspective third-person camera is
capped at a 100 m spring length, so its ground footprint stays well inside the sea.

**Pixel scaling is a crisp integer grid at all four sizes** (run-length mode exactly 3 px at
1280/1024/800/1600 and back to 1280; 1-px runs ≤ 2.5 % from dither), no letterboxing, HUD text
native-sharp. Tutorials 01–04 load and drive clean. All five scene logs contain **no `ERROR`,
no `SCRIPT ERROR`, and none of the N6/N31 editor boot noise** — only the two known llvmpipe/xim
warnings.

One unverified round-2 claim: **N3's "help stays clear when an object is selected" cannot be
exercised in a plain game** — the F3 object picker never selects anything in the runtime
(four armed click attempts on the player, a tree and the terrain, plus three hover probes: no
hover outline, no selection, the inspector falls back to its idle hint). Filed as R6-2.

## Round-2 verification

| Item | Claim | Result | Numbers |
|---|---|---|---|
| N2 width cap | 307 px @1024, 260 px @800 | **pass** | dock 708→1016 = **308 px** @1024 (30.1 % of 1024; cap 0.3·1024 = 307.2); 531→792 = **261 px** @800 (260 floor); 911→1272 = **361 px** @1280 (full `panel_width` 360, non-compact); 1231→1592 = **361 px** @1600 |
| N2 vertical scroll | panel scrolls | **pass** | wheel over the tab body moves the content 0→content end (`02`→`03`→`05`); at the end the last folded section `PixelLightVolume: Lantern` sits at y=558 inside the 592 px bottom; extra wheel then zooms the camera (scroll was at its end) |
| N2 horizontal scroll | panel scrolls both ways | **pass** | h-scrollbar track 708…1008 with a 715…926 thumb at y 578–585 @1024; dragging the thumb 820→970 shifts the content left (labels clip, value column moves, `49`); shift+wheel scrolls horizontally at 800 (`15`) |
| N2 Area/Rain Color | reachable, nothing below the window | **pass** | @1024 at scroll 0: `Area` y=537, `Rain Color` y=567 with its swatch — both inside 600; @800 after 3 notches: `Area` y=225, `Rain Color` y=255 inside 480. The strip is clipped by the ScrollContainer at the panel bottom (474 @800), nothing spills over the window edge |
| N2 title collapse | click "Pixel tuner" collapses/expands | **pass** | @1024 click on the title collapses to a `306×36` header at (710,84) — chevron ▸ and the header button becomes **Show**; second click restores 708→1016 × 83→591. Same at 800 (531→792, world fully visible, `16`) and at 1280/1600 |
| N2 player clear | player not covered | **pass** | player silhouette (helmet+torso) x 470…545 @1024, x 355…425 @800; tuner starts at x=708 / x=531 → clear at both sizes |
| N3 toggle | F1 toggles | **pass** | on (`09`), off (`11`; `probe.py hint` finds no dark panel, `hint: null`), on again for the 800 shot |
| N3 backing | dark backed multi-line panel | **pass** | @1024: panel x 13…414, y 496…586 (**401×91**), 3 lines, dark `rgba(0.04,0.05,0.08,0.84)` + 1 px light border (inside bg sampled (17,25,25) over sand); @800 x 13…400, y 376…466 (387×91); @1280 13…414, 616…706; @1600 13…414, 796…886. White 16 px text (compact font below 1100 px wide) — readable over bright sand/grass |
| N3 no overdraw | F3 does not cut the help | **pass** | tuner left edge 708 vs hint right edge 414 @1024 (**294 px** gap); 531 vs 400 @800 (**131 px**); the help's last line `F3 pixel tuner · F1 hide / show this help` is complete in both. No column overlap exists at 1280/1600 either (gap 498/817 px) |
| N3 object-selected case | help stays clear of an expanded inspector | **unverified — unreachable** | `Select Object` arms (hint changes to "Click a mesh to select it (hover highlights, Esc cancels)", `50`), then hovering/clicking the player (`52`), a tree and the terrain (`51`) never yields a hover outline or a selection; the panel returns to its idle hint and stays header-height, so `pixel_diagnostics_keep_clear` for the left dock is never exercised. See R6-2 |
| r4 #23 orbit (hex) | stops above island/water, no underside/wireframe | **pass** | middle-drag down → top-down (`25`/`40`); two 600 px middle-drags up → level view above the island (`26`/`41`): horizon in frame at y≈440, terrain seen from above only, no underside slabs, no blue wireframe through the rock |
| r4 #38 sea (hex) | sea to horizon, no edge/corner/void | **pass** | 1280×720, min pitch + 40 wheel notches (true max distance, `48`): water meets the sky at y≈400, uniform-band scan finds **0 px** of ≥90 %-uniform "void" colour below the horizon; no straight plane edge, no corner |
| r4 #38 sea (tutorials) | same | **fail** | `02_island` and `04_world_mood` at `min_pitch_deg = 5` + max `size` (`45`, `46`): a perfectly horizontal plane edge at y≈478–488 across the full 1280 px width, uniform band below — t02 (123,168,224) **232 px = 32 %** of 720, t04 (157,102,116) **232 px = 32 %**. `01_flat_playground` shows the same at ~1.5× zoom (`32`/`33`: edge y=566, 154 px). `04` also shows a hard straight **diagonal** boundary in the sea (sun-shadow frustum edge) from the top edge down to the plane edge. See R6-1 |
| Pixel scaling | crisp integer grid, sharp text | **pass** | 1280→1024→800→1600→1280 live resizes (`18`–`22`). Horizontal run-length histogram of exactly-equal RGB over the centre 40 % of each frame: **mode = 3 px** at every size (73.6 / 74.1 / 73.6 / 73.0 / 72.6 % of runs), next modes 6/9/12/15 (flat patches), 1-px runs 0.3–2.5 % (dither). World fills the client area at all four sizes (no letterbox). Status/F1/F3 text is native-resolution and 1:1 sharp at each size (`02`, `12`, `17`, `23`) |
| Tutorials 01–04 | load, camera/WASD/Esc | **pass** | all four load and render (`31`, `34`, `36`, `37`); right-drag orbits (`32`), `W` pans (`33`), `Esc` never quits or errors; after-input captures for all four (`44-*`) |
| Log scan | no addon errors; editor-only noise absent | **pass** | `game-hex.log`, `game-0{1..4}*.log`: **0** `ERROR`, **0** `SCRIPT ERROR`, **0** `create_frustum_points`, **0** plane/vector normalize warnings; only `Failed to create wd.xic` and `Could not set V-Sync mode` once per scene |

### Orbits/zoom actually driven

| Step | Shot | Result |
|---|---|---|
| hex middle-drag down 280 px | `25`, `40` | top-down over the player's helmet, still above the terrain |
| hex middle-drag up 2×580 px | `26`, `41` | camps at a level view: sky above, island below, nothing under the map |
| hex wheel-down 40 (max distance) | `48` | whole island small, sea to the horizon |
| t02/t04 right-drag up 440 px + wheel-down 30 | `45`, `46` | pitch 5°, max ortho size → plane edge + void |

## Harness note (not an addon defect)

`qa/runtime.sh` copies `$MST_QA_STATE/project_base` when `runtime/project` is missing, and
`project_base` is a source-tree copy **without `.godot/`**. The first boot of the fresh copy
failed to register *every* GDExtension class (`Cannot get class 'PhantomCameraManager'`,
`Can't create sub resource of type 'MarchingSquaresTextureSlot'`, all `.godot/imported/*.ctex`
missing) because a plain game run does not import assets. Fix used for this pass:
`godot --headless --path /tmp/opencode/qa/runtime/project --import` once after the copy (leaves
`extension_list.cfg` + `imported/`). The r5 runtime evidence came from a `runtime/project` that
already had a cache, which is why this never showed up before. Any future pass that deletes
`runtime/project` must run that import first (or `runtime.sh` should do it).

## New issues

- **R6-1 — Warning — tutorials/sea (#38 residual).** The finite sea plane's edge is visible as a
  straight horizontal line with a pale void below it when the tutorial orbit camera is at
  `min_pitch_deg = 5` and the wheel is at its maximum orthographic size: 232 px of void (32 % of
  the 720 px window) at 1280×720 in both `02_island` (sky colour 123,168,224) and `04_world_mood`
  (dusk sky 157,102,116); 154 px in `01_flat_playground` before max zoom. `hex_island` does not
  show it (perspective camera, 100 m spring cap → a small ground footprint). Evidence
  `45-t02-true-max-zoom.png`, `46-t04-true-max-zoom.png`, `32-t01-orbit-zoom.png`; the uniform
  band is measurable (probe: `void_top_y = 488`, 232 px). Root cause is the tutorial camera limit
  vs a finite water mesh: an orthographic `size` of 400 at a 5° pitch projects ~4.6 km of ground
  into one 720 px screen, far past the sea. The water shading rim fix does hold (the edge is the
  *plane*, no dark band, no corner). `04` additionally shows a hard straight diagonal
  shadow-frustum boundary across the sea.
- **R6-2 — Minor/UI — the F3 object picker never selects in a plain game.** With
  `Select Object` armed (hint confirmed), moving over / clicking the player, a tree and the
  terrain in `hex_island` produces no hover outline, no selection and no inspector content; the
  panel returns to the "Press Select Object…" hint at header height. So the round-2 claim
  "help stays clear of an expanded object inspector" is not exercisable from the runtime. The
  picker is a working feature of the overlay contract (it has hover/selection outlines, a
  profile create/remove UI), so this reads as a runtime-only projection/input bug — e.g.
  `pick_geometry` calls `get_viewport().get_camera_3d()` while the world is rendered through the
  `PixelRenderer` SubViewports, or the click is consumed before `_unhandled_input`. Evidence
  `50-pick-armed-1024.png`, `51-pick-missed-terrain-1024.png`,
  `52-pick-missed-player-1024.png`.
- **R6-3 — Minor/UI — the F1 help covers the player at 800×480.** The hint panel is
  13…400 × 376…466 and the player sits at x 355…425 in this window; the F3 tuner does not cover
  him (starts at 531). The help is togglable, so this is cosmetic, but the bottom-left corner is
  exactly where the third-person camera puts the character on a small window. Evidence
  `12-hex-800x480-f3-f1.png`, `14-hex-800x480-f3-area.png`.

## Covered

- [x] F3 tuner at 1024×600: open, width cap, vertical scroll to the last section, horizontal
      scrollbar drag, Area/Rain Color visible, title collapse + expand, player clear.
- [x] F3 tuner at 800×480: open, width cap, vertical scroll, Area/Rain Color after 3 notches,
      horizontal scroll (shift+wheel), title collapse, player clear.
- [x] F3 tuner at 1280×720 and 1600×900 (non-compact 360 px), F1 + F3 together.
- [x] F1 help: toggle on/off, measured backed panel at four sizes, readable over bright
      sand/grass, no F3 overdraw at 1024/800, no interaction with the object inspector
      (unreachable — R6-2).
- [x] hex_island orbit: top-down extreme, two long drags to the low clamp, underside/void/
      wireframe check.
- [x] Sea horizon: `hex_island` at min pitch + true max zoom (pass), `tutorial/02_island` and
      `tutorial/04_world_mood` at min pitch + max zoom (fail), `tutorial/01_flat_playground`
      (fail at 1.5× zoom).
- [x] Pixel scaling: live X11 resize 1280→1024→800→1600→1280, 3 px run-length mode at every
      size, no letterbox, HUD text native-sharp.
- [x] Tutorials 01–04: load/render, right-drag orbit, WASD pan, Esc, after-input captures.
- [x] hex_island: WASD movement, Esc (closed the open weather-option popup), F1/F3 hide,
      day-night clock advanced to dusk during the session (no artefact).
- [x] Log scan of all five scenes (0 `ERROR`/`SCRIPT ERROR`, no N6/N31 boot noise).
- Not tested (out of scope for this brief): 0.5×/2× OS scaling, gamepad, audio, the tutorial
  scenes' F3/F1 overlays (they ship without `PixelDiagnostics`/`MstDemoHint`), a second window
  size for the tutorials' sea test besides 1280×720.

## UI feedback (for FEEDBACK.md)

1. **Finish #38 in the tutorials.** The water *shading* fix is right, but the plane still ends:
   extend/fade the sea (or raise `min_pitch_deg` with the ortho `size`, e.g. 5° → ~14° at
   `max_size`) so a grazing view can never see past it. A 232 px flat band under a straight line
   is the most visible defect left in the runtime.
2. **Soften the sun-shadow frustum edge on the water** (`04_world_mood` diagonal, a fainter one
   in `hex_island` at max zoom). It reads as a straight seam in an otherwise organic sea.
3. **Make the runtime object inspector honest**: either project picks through the pixel
   renderer's camera (it is a shipped runtime feature) or hide/disable `Select Object` when
   picking cannot work. A toggle that silently does nothing is worse than no toggle.
4. **Keep the help hint off the player on small windows** — swap it to the bottom-right above the
   tuner, or fade it while the character is behind it.
5. **The tuner's only horizontal overflow is the `Area` row** (two/three spin boxes). Wrapping
   that row would remove the h-scrollbar at 1024/800 and make the panel feel like the r6 promise.

## Files written

- `qa/findings/r6-runtime.md` (this file)
- `qa/screenshots/r6-runtime/` — 56 client-area PNGs (`00`–`52` numbered plus the four
  `44-tutorial-*` captures and the 1024/800/1280/1600 F3/F1 set) and five `game-*.log` copies
- `qa/scripts/r6-runtime/{xwin,cap,probe,grid,hold}.py` — live X11 resize/geometry, client-area
  capture, tuner/help measurement, pixel-grid analysis, sustained key hold

Runtime `r6-e4` on `:74` was stopped at the end of the pass.
