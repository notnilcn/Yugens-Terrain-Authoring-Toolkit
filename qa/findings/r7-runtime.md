# Area 7/9 — runtime verification (r7, fresh runtime project)

Sessions `r7-v4`, `r7-v4b`, `r7-v4c` (hex), `r7-v4-t01`…`-t04` (tutorials), all on display `:94`,
run from `/home/clintonnguyen/g/code_examples/rrr-wt/r4base` at **HEAD `3474990`** (merge of
`work/r7`), Godot 4.7.2, llvmpipe, no window manager. `qa/runtime.sh` imported a freshly copied
project (`/tmp/opencode/qa/runtime/project`, no stale cache) on the first `start`; every scene
booted from that copy. Screenshots are the client area only (`cap.py`); the window is at 0,0, so
screenshot coordinates are client coordinates. Numbers below come from
`qa/scripts/r7-runtime/{cap,xwin,probe,runs,grid,sea,diff,hold}.py`.

## Finding

**#38 (P0) is fixed for the tutorials.** `tutorial/02_island` and `04_world_mood` at the lowest
orbit drag + 40 wheel notches (true max ortho size, 400) show **0 flat/void rows** — the r6
232 px (32 %) horizontal plane edge and pale band are gone in both. `hex_island` is clean at
800×480 and 1280×720 max zoom (no band wider than a 12 px water/shadow tone step). The camera
fix behaves as claimed: a vertical orbit drag that used to reach 5° now clamps the pitch up (both
tutorials end near top-down at the drag limit, with the sea and sky never showing an edge). The
residual the r7 notes admit is real and very visible in `04`: a **lighter rotated square (light
/ shadow frustum) centred on the island**, ~730 px across at max zoom.

**The F3 object picker works in the plain game** (r6's R6-2 "never selects" is fixed): hovering
the player draws the gold hover outline plus the translucent AABB box, and clicking selects
`Knight_Body` (`MeshInstance3D`, path `…/PlayerRig/Pixel/Player/Visual/Knight/Rig_Medium/
Skeleton3D/Knight_Body`) with the full inspector: pixel settings (`pixelized: yes | pixel size 1 |
outline on | id 3 | palette on | either on`), `Owning PixelObject: Pixel`, `Create profile`,
`Geometry: Knight_Body`. **But the picker only resolves that mesh**: tree canopies, rocks and the
terrain surface never hover and never select — a click on them **clears** the current selection.
So of the brief's three targets, player = pass, tree = fail, terrain = fail.

**#38's N3 promise now holds with an expanded inspector**: with the player selected the F1 help
panel still sits fully below the left dock (2–6 px gap at 800/1024/1280/1600), and hiding the
help lets the inspector grow into the freed space (800: 305 → 471). F1 at 800×480 ends at
x = 246 while the player sits at x 375–425; the F3 tuner starts at x = 531.

## r7 verification

| Item | Claim under test | Result | Numbers |
|---|---|---|---|
| #38 t02 sea | 0 void rows at min pitch + 30 zoom notches | **pass** | `02_island` 1280×720, min-pitch drag + 40 notches (`42`): sea reaches every edge, `sea.py` finds **0** flat bands / 0.0 % flat rows (r6: 232 px, 32 %). After the drag alone (`41`) the camera clamps to a near-top-down view, sea fills the frame |
| #38 t04 sea | same | **pass** | `04_world_mood` (`46`, `47`): 0 flat bands / 0.0 % at min pitch and at max zoom; the dusk water fills the frame, no straight plane edge (r6: 232 px / 32 %, plus a straight diagonal) |
| #38 hex | still clean | **pass** | `hex_island` min pitch + 40 notches: 800×480 (`35`) max flat band 12 px (a water tone step), 11.3 % flat rows but all thin sky/water bands; 1280×720 (`71`) 12 flat rows / 1.7 % — sea to the horizon, no edge, no void |
| F3 picker — player | hover outlines, click selects | **pass** | Armed hint "Click a mesh to select it (hover highlights, Esc cancels)." (`17`). Hover (`18`): gold outline + translucent box on the knight (`18b-knight-hover-crop-2x.png` vs `18a-knight-idle-crop-2x.png`). Click (`19`): inspector expands, `Knight_Body (MeshInstance3D)`, `PixelObject: Pixel`, profile create + geometry sections; outline turns orange (`19a-knight-selected-crop-2x.png`) |
| F3 picker — tree | pick a tree | **fail** | Hover over the big canopy (`22`, `25` at (300,560), `64` at 1024 (650,240)): no outline, no box. Click (`23`): inspector collapses to the idle hint — selection cleared. Trees are scatter `MultiMeshInstance3D`s |
| F3 picker — terrain | pick terrain | **fail** | Click on the dirt path (`27` at 1280 (600,420), `65` at 1024 (500,300)): no hover, no selection, panel returns to "Press \"Select Object\"…" |
| F1 clear when selected (N3) | help stays clear of an expanded inspector | **pass** | inspector bottom 614 vs hint top 616 @1280 (`29`, `runs.py col 200`); 380 vs 385 @1024 (`30`); 305 vs 307–310 @800 (`31`, `67`); ≤794 vs 796 @1600 (`37`). F1 off at 800 (`68`): hint gone, inspector grows 305→471 |
| F1 at 800×480 (R6-14) | ends x≈246, player x 355–425 | **pass** | hint 13…246 × 303…466 = **233 × 164**, 7 lines (one binding per line); player silhouette x 375…425; F3 tuner left edge 531 → 285 px gap (`04`, `31`, `67`) |
| N2 width cap | 361/308/260 | **pass** | dock 911→1272 = **361** @1280 (`02`), 708→1016 = **308** @1024 (`03`, `61`; cap 0.3·1024 = 307.2), 531→792 = **261** @800 (`04`, `05`, `07`; floor 260), 1231→1592 = **361** @1600 (`37`; non-compact `panel_width` 360) |
| N2 scroll | two-axis, Area/Rain Color reachable | **pass** | 800: 3 notches move the body (`12`), 10 more reach `Area` y 318 / `Rain Color` y 348 with the h-scrollbar track at y 464 (`13`); 1024 at scroll 0 `Area` y 537 / `Rain Color` y 567 (`63`, `65`); the strip is clipped by the panel, nothing spills over the window edge |
| N2 title collapse | click "Pixel tuner" collapses/expands | **pass** | 800: 531…792 × 83…471 → header only 83…119, chevron ▸, button "Show" (`14`); second click restores full height (`15`). 1024: collapse (`62`) and expand back to 708…1016 × 83…591 (`63`) |
| N2 player clear | player not covered | **pass** | @1024 player x 480…545, tuner from 708 (`61`); @800 player 375…425, tuner from 531 (`67`) |
| N3 backing | dark backed multi-line panel | **pass** | 401 × 91 (3 lines) @1280 (`02`), 281 × 199 (7 lines) @1024 (`61`), 233 × 164 (7 lines) @800 (`04`), ~401 × 91 @1600 (`37`); dark translucent fill + 1 px light border, white text readable over sand/grass |
| N3 toggle | F1 hides/shows | **pass** | on (`02`, `61`), off (`32`, `68`; `probe.py hint` then finds only the inspector, and the inspector takes the space) |
| N3 no F3 overdraw | tuner never cuts the help | **pass** | tuner left 911/708/531/1231 vs help right 413/294/246/413 → **497 / 414 / 285 / 818 px** gaps; last line `F3 pixel tuner · F1 hide / show this help` complete at every size |
| #23 orbit | stops above island/water, no underside/wireframe | **pass** | middle-drag down 280 px → top-down over the helmet (`33`); two 580 px middle-drags up → level view: sky above, island below, horizon in frame, no underside slab, no wireframe (`34`) |
| Pixel scaling | crisp integer grid at 1280→1024→800→1600 | **pass** | live X11 resizes, centre 40 % patch (`36-*`): run-length mode **3 px** at every size (74.0 / 73.8 / 71.8 / 76.5 % of runs), next modes 6/9/12/15, 1-px runs 1.1–2.5 % (dither). World fills the client area at all four sizes (no letterbox), HUD text native-sharp |
| Tutorials 01–04 | load, orbit, WASD, Esc | **pass** | all four load and render (`40`, `45`, `48`, `50`); right-drag orbits (`51`), `W` pans (`43`, `52`), `Esc` never quits or errors (`44`, `53`, process still alive) |
| Logs | clean | **pass** | `game-hex.log`, `game-hex2.log`, `game-hex3.log`, `game-t0{1..4}.log`: **0** `ERROR`, **0** `SCRIPT ERROR`; only `Failed to create wd.xic` + `Could not set V-Sync mode` once per scene |

### Sea scan detail (`sea.py`, modal-colour flat band ≥ 90 % of the row)

| Scene / size | Shot | Flat rows | Largest flat band | Verdict |
|---|---|---|---|---|
| t02 1280×720, min pitch | `41` | 0 (0.0 %) | — | pass |
| t02 1280×720, max zoom | `42` | 0 (0.0 %) | — | pass + frustum square |
| t04 1280×720, max zoom | `47` | 0 (0.0 %) | — | pass + frustum square (strong) |
| t01 1280×720, min pitch + max zoom | `49` | 0 (0.0 %) | — | pass |
| hex 800×480, max zoom | `35` | 54 (11.3 %) | 12 px water tone step | pass (no row-wide band) |
| hex 1280×720, max zoom | `71` | 12 (1.7 %) | 3 px sky band | pass |

## Covered

- [x] `#38` sea: `02_island`, `04_world_mood` at minimum orbit + true max ortho zoom; `hex_island`
      at 800×480 and 1280×720 max zoom; `01_flat_playground` at max zoom.
- [x] F3 object picker: arm/disarm, armed hint, hover on the player (outline + box), click on the
      player (select + full inspector), hover/click on trees (canopy at two points, two sizes) and
      the terrain surface (two points, two sizes), selection cleared by a miss.
- [x] F1 + expanded object inspector at 800/1024/1280/1600 (keep-clear), F1 off → inspector grows.
- [x] F1 help at 1280/1024/800/1600 (width, height, line count, player clearance), F3 tuner width
      cap and player clearance at the same sizes, no overdraw between the two docks.
- [x] F3 tuner: vertical wheel scroll to `Area`/`Rain Color` at 800 and 1024, title-click collapse
      and expand at 800 and 1024, 1600 non-compact width.
- [x] `hex_island` orbit extremes: top-down clamp and the low clamp (level view), underside /
      wireframe / void check.
- [x] Pixel scaling: live resize 1280→1024→800→1600, integer-grid histogram at each size.
- [x] Tutorials 01–04: load/render, right-drag orbit, `W` pan, `Esc`; after-input captures.
- [x] Log scan of all six scene runs (0 `ERROR` / `SCRIPT ERROR`; no editor-only boot noise).
- Not tested (out of scope / no fixture): OS 0.5×/2× scaling, gamepad, audio, the tutorials' F3/F1
  overlays (they ship without `PixelDiagnostics`/`MstDemoHint`), the picker on the fox/courtyard
  scenes.

## New issues

- **R7-1 — Minor/UI — the F3 picker only resolves `MeshInstance3D` objects, not the terrain or the
  scatter meshes.** Player: hover outline + box, click selects `Knight_Body`, inspector fills
  (`18`, `19`). Tree canopy (scatter `MultiMeshInstance3D`) and the terrain surface: no hover
  outline, no selection, and because a pick miss is treated as "clear", clicking them **drops the
  current selection** (`22`, `23`, `25`, `27`, `64`, `65`). The printed help still says "click a
  mesh in the viewport", which reads as working for everything. Either pick `MultiMesh`/terrain
  chunks (the AABB pre-pass already handles non-`MeshInstance3D` by box) or stop promising it.
  Evidence: `/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-runtime/19-hex-picker-click-player.png`,
  `.../23-hex-picker-click-tree.png`, `.../27-hex-picker-click-ground.png`,
  `.../65-hex2-click-terrain-1024.png`.
- **R7-2 — UI — #38 residual: a lighter rotated square sits centred on the island at min pitch +
  max zoom.** In `02_island` it is faint; in `04_world_mood` it is a bright ~730 px diamond over
  the dusk sea (`42`, `47`), with straight edges that read as a seam in the water. It is the
  light/shadow frustum, not the plane edge: the sea itself now reaches every screen edge.
  Evidence: `.../42-t02-maxzoom.png`, `.../47-t04-maxzoom.png`, `.../46-t04-minpitch.png`.
- **R7-3 — Minor — `03_pixel_look` exposes its finite field when panned.** A 1 s `W` hold after an
  orbit drag pushes the 24 m field's edge into frame: a straight diagonal grass/sky boundary in
  the top-left corner (`52`). The pitch/zoom clamp cannot help because the target moves; the
  tutorial scene's field is simply small. Evidence: `.../52-t03-wpan.png`.

## UI feedback (for FEEDBACK.md)

1. **Kill the light frustum square on the water** (R7-2). It is now the largest visual defect in
   the tutorials: at max zoom in `04_world_mood` a bright diamond covers most of the sea. Fading it
   with distance/angle or clipping the volume to the island would finish #38.
2. **Make the object picker honest** (R7-1). Picking the player is a good demo; clicking a tree and
   watching the selection vanish is worse than a picker that only arms for objects it can resolve.
   At minimum keep the current selection when the click misses.
3. **The compact help is 164–199 px tall** (7 lines at ≤1100 px wide). With a selection open at
   800×480 the inspector is left with ~222 px and scrolls immediately. A shorter compact form
   ("WASD · Shift · Space / drag+arrows: orbit · wheel: zoom / F3 tuner · F1 help") or a two-column
   layout would give the dock room back.
4. **The `Area` row still forces the tuner's only horizontal scroll** (three spin boxes at 1024/
   800). Wrapping it would remove the h-scrollbar at both small sizes.
5. **Live resize can leave the dock one relayout behind** — one 1.6 s frame after 1024→800 showed
   the dock at 308 px (the 1024 cap) instead of 261; two retries with 1.5–3 s waits gave 261, so it
   is a transient, but a `size_changed`-driven dock relayout would remove it.

## Files written

- `/home/clintonnguyen/g/code_examples/rrr/qa/findings/r7-runtime.md` (this file)
- `/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-runtime/` — 70 PNGs (`00`–`71` with
  gaps, plus three 2× knight crops) and seven `game-*.log` copies
- `/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r7-runtime/` — `xwin.py`, `cap.py`,
  `probe.py`, `runs.py`, `grid.py`, `sea.py`, `diff.py`, `hold.py`

All runtime sessions were stopped at the end of the pass (no `godot --path
/tmp/opencode/qa/runtime/project` process remained; the `:94` Xvfb was torn down).
