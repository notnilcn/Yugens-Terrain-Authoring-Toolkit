# r7 pixel/effects verification — build `3474990` (r4base, merge of work/r7)

Session `r7-v3`, display `:93`, harness port `8893`; build
`/home/clintonnguyen/g/code_examples/rrr-wt/r4base` (HEAD `3474990`), Godot 4.7.2,
llvmpipe, editor 1600×900. Scenes: `demo/hex_island.tscn` (editor, #36/fireflies/weather)
and `tutorial/04_world_mood.tscn` (#37, N25); `#38` was measured in the **running tutorials**
(`qa/runtime.sh` on display `:95`, 1600×900) because the r7 fix is in
`tutorial/orbit_camera.gd`, which the editor viewport never runs. Throwaway scene copies live
in the session project only (`qa_tmp/02_noon|dawn.tscn`, `04_noon|dawn.tscn`,
`04_shafts*.tscn`, `04_nowater.tscn`, `04_noterrain.tscn`, `04_nosky.tscn`); no repo file under
`addons/`, `demo/`, `rust/`, `tutorial/` or `project.godot` was touched. Screenshots are in
`qa/screenshots/r7-pixel-effects/`; helpers in `qa/scripts/r7-pixel-effects/`
(`scan.py`, `firstpaint.py`, `clamps.py`, `clamp1.py`, dumps in `data/`).

"Static" pixel diffs use Animate OFF; two consecutive captures are byte-identical
(`changed=0/397664`).

---

## r7 verification (per claimed item)

| # | Claim | Verdict | Numbers |
|---|---|---|---|
| 1 | **#38/R6-12** tutorial sea: orbit camera backs off then raises pitch; 0 void rows at min pitch + 30 zoom notches; residual lighter rotated square | **Pass (void) / Fail (residual square)** | `02` and `04`, noon and dawn, min pitch + 30 wheel notches: **0 flat/void rows** and 0–18 flat-pale px at noon (0.0 %); r6 was 232 px / 32 %. The sea reaches all four frame edges; no straight sea edge and no pale void. **The lighter rotated square is still there and is bigger than "residual"**: four hard straight edges, ≈900×400 px at 1600×900, corners ≈(355,655) (700,395) (1290,500) (960,790) on `04` noon; interior +20 R / +38 G / +33 B over the surrounding sea at the bottom edge. |
| 2 | **#37/R6-18** Dawn fog 0.0045→0.0015; view 150/156/173→113/124/155; water 74/103/157; island 91/91/62 | **Partial (number mismatch)** | Dawn preset on `04_world_mood`: view **130.7/140.5/163.0** (r6 150.5/156.1/173.2; claim 113/124/155), island **89.6/94.8/59.0** (claim 91/91/62 ✓, r6 105.2/107.2/74.3), water **126.3/145.0/168.0** (claim 74/103/157 ✗, r6 148.0/161.1/176.7), sky 158.5/146.0/169.4. Noon **identical to r6** (77.8/129.1/175.3; water 49.0/118.3/169.3; island 80.1/104.2/58.4), dusk within 1–7 levels (143.3/121.9/128.0 vs 142.0/114.9/125.1). Dawn is darker and bluer than r6 but still a lavender-grey wash with an olive island (see `102`-equivalent `320-wm-dawn-preset.png`). |
| 3 | **N25/R6-15** warning computed on request; shafts with pillars 142,649/654,963 px at 400 m / alpha 0.6 / count 64 | **Pass (warning) / Pass (shafts, runtime only)** | Warning **refreshes**: at 120 m it reads “…past the sun's shadows (directional_shadow_max_distance 120 m; shafts fade out from 72 m)…”; after `set-prop directional_shadow_max_distance 150` it reads **150 m … 90 m**; at 400 m the warning disappears (camera 125 m < 240 m). Shafts: in the **editor** visible↔hidden is **0/397664 px**, even at alpha 4.0 (the Pixel Preview never draws them — this is why r6 measured 0). In the **running game** with three pillars, 400 m, alpha 0.6/count 64: 47.1 % of the island patch differs by >6 (control run-to-run 17.0 %), and at alpha 4.0 the whole island is covered by an unmistakable warm yellow veil. |
| 4 | **R6-1** first paint "not reproduced"; **R6-2** first build two rows 405×66 | **R6-2 Pass / R6-1 Fail (unchanged)** | First ever `Pixel*` selection in the session: immediately after `select` the rail is `[287,121 39×0] visible:false`, panel `0×35`; by +2.5 s it settles to rail `39×732`, strip `731×175`, `SectionPages [432,678 405×66]`, `PinnedHeader [329,678 731×68]` — **two rows, `Debug Views` fully inside the 731 px strip** (r6: one 670×33 row, clipped by 42 px). R6-1 is still observable: a rebuild of the Pixelizer takes **2.91 s** (`select` round-trip) vs 0.40 s when warm, and the frame captured immediately after the select shows the **previous tool's rail/strip** (Builder brush rows); the Pixelizer rail + strip only appear ~2 s later. No placeholder/spinner. |
| 5 | Clamps honest + one drag = one undo step | **Pass (6/6)** | Pixel Size 1–16 → 16, Darken 0–1 → 1.0, Ray Alpha 0–4 → 4.0, Sky Coverage 0–1 → 1.0, Water Swell H. 0–2 → 2.0, Fireflies Count 0–1024 → 1024; each `undo-state` names the matching action (`Set Renderer Pixel Size`, `Set Outlines Darken`, `Set God Rays Ray Alpha`, `Set Sky & Clouds Coverage`, `Set Water Swell H.`, `Set Fireflies Count`) and one `Ctrl+Z` restores the exact previous value (3.0 / 0.546 / 0.9975 / 0.504 / 0.121 / 64). The harness's `undo_count` only incremented for the first drag (0→1, then 1→1 for the rest) although each action and each undo were correct — same harness quirk r6 saw on feature toggles. |
| 6 | **#36** smoke, fireflies, weather, pressed states, `+ Add` | **Pass (unchanged)** | Smoke visible↔hidden at the hearth overview (`hex_island`, Animate off): **345 px** in a 22×24 patch (r6 317 px in 24×22) — still a haze puff, no plume change claimed. Fireflies at `time_of_day 0`: **1189 px** in a 118×87 patch (r6 932 px in 130×102). Rain: `set_weather("rain")` → `get_weather()==3`; the transition takes ~8 s (empty haze first, as r6 noted), then grey sky + white specks on the island and wet/dark ground; `clear` restores. Pixel Preview / Animate: `toggled` false→true for both, accent-blue icon + lifted background. `+ Add` Sun Shafts keeps selection on `PixelRenderer`, strip open (`731×173`), History `Add PixelSunShafts`; Ctrl+Z removes it. |
| 7 | **N6/N31** boot noise | **Pass (unchanged)** | `editor.log` 85 lines: **16** `Plane cannot be normalized` + **16** `Vector3 cannot be normalized` + **2** `create_frustum_points` ERRORs, all boot-only (lines 13–79). The pass added one ERROR, and it is mine: `PixelSunShafts::set_position: Cannot convert argument 1 from Array to Vector3` from a harness `call` probe, not the addon. No addon `SCRIPT ERROR` anywhere. |

---

## Finding 1 — #38 is fixed for the void, but the residual lighter rotated square is still large and hard-edged (claim understates it)

- Severity: warning (visible artifact at the tutorial's own extreme framing)
- Steps: 1) run `qa_tmp/02_noon.tscn` / `02_dawn.tscn` / `04_noon.tscn` / `04_dawn.tscn`
  (copies of `tutorial/02_island` / `04_world_mood` with only `time_of_day` changed) as a game
  on `:95`, 1600×900. 2) right-drag from (800,250) to (800,650) (pitch to `min_pitch_deg` 14°).
  3) wheel-down 30 notches (ortho size 120→260, `max_size`). 4) `shot` cropped to 1600×900.
- Expected: sea to the horizon and no plate (round-2 note), or at most a subtle residual
  (r7 claim).
- Actual: **no void** — `scan.py` finds 0 flat rows in the bottom quarter and 0–18 flat-pale px
  at noon on all four frames (r6: 232 px / 32 %). The camera visibly backs off and raises pitch
  (the island is seen from ~29° at 300 m, not 14°), exactly as the r7 note describes.
  The **lighter rotated square remains**: on `305-04-noon-minpitch-zoom30.png` it spans four
  hard straight edges with corners ≈(355,655), (700,395), (1290,500), (960,790); a horizontal
  transect at y=600 crosses it at x≈360 and x≈1180 with a step of ~+22 grey levels (inside
  ≈98–112, outside ≈80–85); at y=700 the step is at x≈640→1000. At dawn the same edges are
  sharper and the square reads as a pale plate over a darker sea.
- `hex_island`: it has no orbit camera (fixed `Camera3D`, size 26), so the tutorial's
  min-pitch/max-zoom combination does not exist for it in a shipped camera. In the editor
  viewport the sea reaches the frame with 0 flat/void rows at ordinary zoom (`309`, 116 m;
  `314`, 39 m). At the editor's own extreme zoom-out (20 notches, 1000 m — well past the
  shipped `max_size 260`) the finite water disc shows a pale sky surround (`308`), i.e. the
  r4 "floating rectangle" topology outside the shipped envelope; no tutorial camera can reach
  it.
- Source probe (scene copies, runtime): hiding `PixelWater` removes the sea entirely (no plate
  left to see); hiding `MarchingSquaresTerrain` removes the island and foam ring but **keeps
  the square**; hiding `PixelSky` keeps the square. With the water hidden, the terrain renders
  as a finite slab whose straight edges sit where the square's edges are — so the square tracks
  the **terrain footprint seen through the water's depth shading** (shallow inside the slab,
  deep outside), not the water node's own rim and not clouds/god rays. This contradicts the
  fixer's “not … water depth bands” note; worth a second look before calling it irreducible.
- Evidence: `301`, `303` (02 noon/dawn), `305`, `307` (04 noon/dawn),
  `304`/`306` (default framing),
  `373-04-noon-terrain-hidden.png` (terrain hidden; the island and its foam ring vanish, the
  square stays), `374-04-noon-water-hidden.png` (water hidden; the terrain slab's edge sits on
  the square's edge), `375-04-noon-sky-hidden.png` (sky hidden; square unchanged),
  `308`/`309`/`314` (hex, editor), `scan.py` output.
- Undo: not applicable (camera + scene copies).

## Finding 2 — #37 dawn is darker, but the claimed numbers do not reproduce and it is still a flat lavender wash

- Severity: minor (palette/lighting)
- Steps: `04_world_mood.tscn`, select `PixelRenderer`, Animate OFF, Pixel World ▸ **Dawn** /
  **Noon** / **Dusk** presets (`undo-state` → `Apply Dawn preset`), capture the 731×544
  viewport, mean over the viewport / island (620,330 80×40 screen) / sea (450,500 120×80) /
  sky patches.
- Actual: dawn view **130.7 / 140.5 / 163.0**, island 89.6/94.8/59.0, water 126.3/145.0/168.0;
  r6 was 150.5/156.1/173.2 (island 105.2/107.2/74.3, water 148.0/161.1/176.7) and the claim is
  113/124/155 with water 74/103/157. The island/water separation is ~37 grey levels and the
  island still olive; the frame is a lavender-grey wash with a pink horizon band. Noon is
  **bit-for-bit the r6 numbers** (77.8/129.1/175.3 and the same island/water patches), so the
  method matches r6 exactly; the dawn number simply is not 113/124/155.
- Evidence: `320-wm-dawn-preset.png`, `321-wm-noon-preset.png`, `322-wm-dusk-preset.png`,
  `310`/`312`/`313` (raw `time_of_day` 0.5 / 0.27 / 0.75 without the preset, for reference).
- Undo: preset is one step (`Apply Dawn preset`).

## Finding 3 — N25 warning now refreshes; shafts are real in the game but invisible in the editor preview

- Severity: minor (QA coverage trap, not a product bug)
- Steps: 1) Pixel World ▸ Sun Shafts (adds `PixelSunShafts`, `sun` wired to `WorldMood/Sun`);
  hover the scene-tree warning icon (240,423) → tooltip at 120 m. 2) `set-prop
  directional_shadow_max_distance 150` → hover again. 3) `400` → hover again. 4) shafts:
  scene copies with three `MeshInstance3D` box pillars, shadows 400 m, `alpha 0.6`, `count 64`,
  noon; run as a game and diff the same copy with `SunShafts.visible=false`; then alpha 4.0.
- Actual: the tooltip **recomputes** — “150 m … 90 m” after the change, and it disappears at
  400 m (125 m camera < 240 m fade end). In the editor 3D view, visible↔hidden is **0 px** even
  at alpha 4.0 (Pixel Preview does not run the additive light pass), which is why r6 saw 0.
  In the running game the island patch changes 47.1 % of pixels by >6 at alpha 0.6 (identical
  run-to-run control: 17.0 %) and alpha 4.0 covers the whole island in a warm yellow veil, i.e.
  shafts do render with an occluder.
- Evidence: `332-shafts-warning-120m.png`, `333-shafts-warning-after-400m.png`,
  `334-shafts-warning-150m.png`, `370-shafts-runtime-on.png`, `371-shafts-runtime-off.png`,
  `372-shafts-runtime-alpha4.png`, `331-editor-shafts.png`.
- Undo: `Add PixelSunShafts` is one step; the property writes are harness `set-prop` calls.

## Finding 4 — R6-2 first-build tab row is fixed; R6-1 first paint is not (claim "not reproduced" is not supported)

- Severity: warning (R6-1) / pass (R6-2)
- Steps: 1) fresh session, nothing selected. 2) `select --path PixelRenderer`; dump the
  Pixelizer controls immediately and after 2.5 s. 3) switch to `MarchingSquaresTerrain`, then
  reselect `PixelRenderer` and capture a `shot` immediately and after 2 s.
- Actual: the first build is correct — `SectionPages [432,678 405×66]` (5 × 2),
  `PinnedHeader [329,678 731×68]` inside the strip, `Debug Views [691,711 104×33]`, nothing
  clipped (r6: one 670×33 row, `Debug V`). But the first paint still shows the old tool: the
  immediate dump is `PixelizerToolbarRail [287,121 39×0] visible:false` and the immediate
  screenshot is the Builder rail + brush strip; the Pixelizer rail/strip appear by ~2 s
  (`341-firstpaint-immediate.png` vs `342-firstpaint-2s.png`). The Pixelizer rebuild takes
  2.91 s (select round-trip) versus 0.40 s once warm, with no spinner/placeholder.
- Evidence: `340-firstpaint-before.png`, `341-firstpaint-immediate.png`,
  `342-firstpaint-2s.png`, `qa/scripts/r7-pixel-effects/data/01-first-select-ui.json`,
  `02-first-build-strip.json`.
- Undo: not applicable.

---

## Covered

- [x] #38 runtime walk, `hex_island` (editor; fixed camera) and `02_island` / `04_world_mood`
      copies at min pitch + 30 zoom notches, noon and dawn; void scan + plate geometry.
- [x] #38 source probe: hide PixelWater / MarchingSquaresTerrain / PixelSky in throwaway copies.
- [x] #37 Dawn / Noon / Dusk presets on `04_world_mood` (means, island/water/sky patches),
      plus raw `time_of_day` 0.5 / 0.27 / 0.75.
- [x] N25 `+ Add` Sun Shafts, `sun` wiring, warning text at 120 / 150 / 400 m, editor vs
      runtime shaft isolation at alpha 0.6 / 4.0 and count 64, 400 m shadows, 3 pillars.
- [x] R6-1/R6-2 first selection timing + first-build tab grid measurements.
- [x] Clamps and undo for all six strip fields (values, action names, Ctrl+Z).
- [x] #36 smoke patch (Animate off, 2 identical baselines), fireflies at night, rain + clear,
      Pixel Preview / Animate pressed states, `+ Add` keeps selection/rail/strip.
- [x] N6/N31 boot noise count + new-error scan.
- [x] First-build `Debug Views` clipping (fit inside the strip).

## New issues

1. **#38 residual plate is still a hard-edged square (Finding 1).** The void half is fixed, but
   the square is ~900×400 px at the tutorial's extreme framing, with 4 hard corners and a
   +20/+38/+33 step at its edge. The r7 note calls it "residual"; it is the dominant artifact
   once the void is gone. Its edges follow the terrain footprint (the depth-band hypothesis the
   fixer ruled out is the one my scene probes point at).
2. **R6-1 first paint is still a ~3 s stale panel (Finding 4).** The claim "not reproduced" is
   not supported by the harness: the rebuild blocks 2.91 s and the first frame shows the
   previous tool's rail/strip.
3. **Dawn numbers do not reproduce (Finding 2).** 130.7/140.5/163.0 vs the claimed
   113/124/155; dusk/noon are unchanged and match r6.
4. **QA trap: N25 cannot be judged in the editor** (0 px in the Pixel Preview at any alpha).
   Any future verification must run the game; the editor hover-warning test is still valid.

## UI feedback

Measured at 1600×900. Window-content coordinates.

1. **Water depth band.** Fade the depth band (or extend the terrain's water coverage) for the
   last few metres of the terrain footprint: the square's edge is a straight +22-grey step and
   is the only thing left of #38.
2. **Pixelizer first build.** 2.9 s of stale UI on every `Pixel*` selection from another tool.
   A one-line placeholder ("Building Pixelizer…") or building the rail before the 631 controls
   would remove the flash r6 filed as R6-1.
3. **Dawn.** The Dawn preset is a lavender-grey wash (view 131/141/163) with an olive island
   (90/95/59). The palette quantisation is real, but a warmer sun tint / lower sky contribution
   would read as dawn instead of dusk-blue.
4. **Sun Shafts warning.** Verified good: the text refreshes on request and vanishes once the
   camera is inside the shadow reach. No change needed.
5. **Pixel World menu.** 235×648 at (1215,115); the Add rows are 23 px apart (Sun Shafts y≈387,
   Pixel Object y≈410, Diagnostics y≈433). The menu is taller than the 900 px window minus the
   menu bar, so at ≤800 px tall it will need its own scroll (not measured here).
6. **Rain.** ~8 s of empty haze before the first drop is visible at Animate ON; a faster
   transition (or starting the rain particle field immediately) would make the weather presets
   testable at a glance.

## Harness notes

- `capture` hangs while the editor is idle; every screenshot here is `shot` (+ crop). The 3D
  viewport at 1600×900 with the Pixelizer open is `[329,121 731×544]`.
- Scene copies: `qa_tmp/*.tscn` in the session project only; the `PackedStringArray` node
  headers must be edited after the closing `]` — inserting a newline inside the header silently
  drops the property (cost one wrong “terrain hidden” run).
- `drive.py call set_position` cannot convert a JSON array to `Vector3`; add/position nodes in
  the `.tscn` instead.
- Pixel World menu rows (this layout): Add Renderer 157, Sky 180, Day/Night 203, Weather 226,
  Water 249, Fireflies 272, Particle 295, Birds 318, Smoke 341, Light Volume 364,
  **Sun Shafts 387**, Pixel Object 410, Diagnostics 433, Erosion 456, Wild Flowers 479,
  Grass Fire 502, Lighting Preset 525; Dawn 548, Noon 571, Dusk 594, Night 617, Storm 640,
  Snow 663.
