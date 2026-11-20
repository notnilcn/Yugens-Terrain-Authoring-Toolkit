# Pixel look QA — Renderer, Palette, Object, Pixel World (+ undo)

Session `pixel` on display `:31`, port `8811` (Godot 4.7.2, llvmpipe, editor window
1600x900). Scene `res://demo/hex_island.tscn`. Session project:
`/tmp/opencode/qa/sessions/pixel/project`; editor log:
`/tmp/opencode/qa/sessions/pixel/editor.log`. Nothing under `addons/` or the repository
scenes was modified; the test LUT was written to the **session** project only
(`res://qa_tmp/island_lut.png`). The LUT bake and all pixel edits from this pass were
discarded by closing the scene without saving.

No new `ERROR` / `SCRIPT ERROR` lines from the addon. The only log lines beyond the known
startup noise are informational prints:
`Pixel World: wired 0 reference(s).` and
`PixelPalette: baked 1024x32 LUT -> res://qa_tmp/island_lut.png`.

## Spin-slider drags leave their field range by orders of magnitude

- Severity: warning
- Steps: 1) Select `PixelRenderer`. 2) Renderer page → drag the **Pixel Size** spin slider
  (value `3`) 55 px to the right over 1.2 s. 3) Outlines page → drag **Darken** (value
  `0.55`, range 0–1) 40 px over 1.2 s.
- Expected: drag clamps to the documented ranges (`Pixel Size` 1–16, `Darken` 0–1). The
  `sections.rs` comments describe `max` as the soft end of the drag range; typing larger
  values is what `ig` is for.
- Actual: `pixel_size` became **44243** and `outline_darken` became **290.19** (range 0–1).
  The viewport degrades to 1-px blocks / near-black terrain. `ui_kit.rs::spin_slider` sets
  `set_allow_greater(true)` and `set_allow_lesser(true)` on every field, including `Float`
  fields that did not ask for it, so the values are accepted by the control.
- Evidence: qa/screenshots/pixel/20b-pixelsize-44243.png (44243),
  qa/screenshots/pixel/21-outlines-darken.png (290.19),
  qa/screenshots/pixel/20e-pixelsize-restored.png (restored).
- Undo: `Darken` drag = one step (correct); `Pixel Size` drag = two steps (see next finding).

## One integer spin-slider drag creates two undo steps

- Severity: warning
- Steps: 1) Renderer page, record `undo-state` (0 actions). 2) Drag **Pixel Size** 55 px.
  3) `undo-state` → `undo_count: 2` (two "Set Renderer Pixel Size" entries in Output).
  4) Ctrl+Z once → 17448 (an intermediate drag value, not 3). 5) Ctrl+Z again → 3.
  6) Ctrl+Shift+Z once → 17448 again.
- Expected: one continuous drag is one undo step.
- Actual: two steps; the first Ctrl+Z only walks back to a mid-drag value. The same
  `EditorSpinSlider` drag on a Float field (Darken) was one step, so this is specific to
  the integer path (or its `allow_greater` state).
- Evidence: qa/screenshots/pixel/20b-pixelsize-44243.png,
  qa/screenshots/pixel/20c-pixelsize-undo1.png (17448),
  qa/screenshots/pixel/20d-pixelsize-undo2.png (3),
  qa/screenshots/pixel/20e-pixelsize-restored.png; Output log lines "Set Renderer Pixel
  Size" twice.
- Undo: broken (granularity, not absence).

## Undo of "Apply Dawn preset" leaves the viewport tinted salmon until the scene is reloaded

- Severity: warning
- Steps: 1) Pixel World panel → **Dawn**. 2) Screenshot 72: the whole sky/cloud deck turns
  salmon-pink. 3) Ctrl+Z. `undo-state` returns to `Apply Dawn preset` in the redo stack;
  `time_of_day` is back to `0.4`, `deck_color`/`deck_shade_color`, `god_rays`,
  `beam_tint`, `ray_alpha`, `day_cycle`, `apply_fog` are all back to their pre-preset
  values. 4) The viewport **stays salmon**. Toggling the cloud deck off, God Rays off, and
  scrubbing Time to 0.42 / 0.5 does not clear it. 5) Close the scene without saving and
  reopen `hex_island` → blue sky again (screenshot 85).
- Expected: undo restores the rendered look, not only the properties.
- Actual: the editor preview keeps the dawn-tinted global sky state (likely a cached
  sky/global-uniform state in the preview renderer) until the scene/viewport is rebuilt.
  Nothing is written to disk, so this is an editor-only stale render, not a saved-scene
  leak.
- Evidence: qa/screenshots/pixel/72-lighting-dawn.png (applied),
  qa/screenshots/pixel/83-godrays-off.png and 84-daynight-time-05.png (still salmon after
  undo and further changes), qa/screenshots/pixel/85-fresh-scene.png (reloaded, blue).
- Undo: broken (properties revert, preview does not).

## Pixel World "+ Add" selects the new node and closes the pixelizer UI and panel

- Severity: ui
- Steps: 1) Select `PixelRenderer` (toolbar + strip + Pixel World panel visible). 2) Click
  the **+ Add** next to `Sun Shafts` (or `Wild Flowers`). 3) The new node is added and
  selected; the left section toolbar, the bottom strip and the Pixel World panel all
  disappear.
- Expected: the panel stays open long enough to add another effect (or focus returns to the
  renderer/terrain).
- Actual: the pixel UI is tied to the selection, so the user must reselect the terrain or
  renderer before adding the next effect. Adding two effects in a row takes two
  reselections.
- Evidence: qa/screenshots/pixel/68-sunshafts-added.png,
  qa/screenshots/pixel/71-wildflowers-added.png,
  qa/screenshots/pixel/70-pixel-ui-restored.png (after reselecting).
- Undo: works — `Ctrl+Z` removes the added node as one action (action names
  `Add PixelSunShafts`, `Add MarchingSquaresWildFlowers`); redo re-adds it.

## Sun Shafts "+ Add" leaves the Sun reference empty; "Wire Sun / Env Refs" reports 0

- Severity: minor
- Steps: 1) Add `Sun Shafts` from the Pixel World panel. 2) Inspect the new
  `HexIsland/SunShafts` node: its **Sun** slot is `Assign...` (empty). 3) Click
  **Wire Sun / Env Refs** in the panel.
- Expected: the `+ Add` tooltip says "references wired"; docs/08 say the Pixel World menu
  wires sun/environment refs. The new shafts node should get the scene's sun.
- Actual: the node's Sun stays empty and the wire action prints
  `Pixel World: wired 0 reference(s).` (`wire_references` only covers PixelSky,
  PixelDayNight and PixelWeather). Any visible shafts therefore need a manual assignment.
- Evidence: qa/screenshots/pixel/68-sunshafts-added.png (Inspector, Sun: Assign...),
  qa/screenshots/pixel/74-wire-refs.png (Output), editor.log line 14.
- Undo: Add is undoable; Wire produced no undo action because it changed nothing.

## Renderer page tab row jumps vertically between pages

- Severity: ui
- Steps: 1) Renderer section, note the page tabs (`Renderer … Debug Views`) sit at y=485
  on the Renderer/Outlines pages. 2) Click `Rays` (tabs move to y=520). 3) Click `Sheets`
  (tabs move to y=555). 4) Try clicking a tab at a fixed y from step 1.
- Expected: a stable tab bar (the strip is bottom-anchored, but the tab row should not jump
  by a whole row between pages).
- Actual: the tab bar shifts by 35–70 px as the page's field count changes, so a tab click
  aimed at the previous position lands in the content area and does nothing. This is also
  why the earlier interrupted attempt produced byte-identical captures 02–06: the
  section icons do switch, but fixed-position page-tab clicks missed and stayed on
  Outlines.
- Evidence: qa/screenshots/pixel/11-renderer-renderer.png (tabs y≈485),
  qa/screenshots/pixel/13-renderer-rays.png (tabs y≈520),
  qa/screenshots/pixel/15-renderer-sheets.png (tabs y≈555); UI dumps in
  qa/screenshots/pixel/ui/.
- Undo: not applicable.

## Disabled Add entries in the Pixel World menu look enabled

- Severity: ui
- Steps: 1) Select the terrain/renderer. 2) Open **Pixel World**. 3) Compare the entries
  that are disabled because the node already exists (`Renderer`, `Sky & Clouds`,
  `Day / Night`, `Weather`, `Water`, `Diagnostics`) with enabled entries (`Fireflies`,
  `Birds`, …).
- Expected: disabled entries are visibly greyed (Godot's normal disabled menu style).
- Actual: the text is the same brightness on both; only the small leading icon is dimmer.
  It is easy to click an already-present effect and wonder why nothing happened.
- Evidence: qa/screenshots/pixel/04-pixel-world-menu.png.
- Undo: not applicable.

## Pixel World panel keeps a scrolled position when its rows rebuild

- Severity: ui
- Steps: 1) With `PixelRenderer` selected, the panel starts at the top (Renderer … Sun
  Shafts). 2) Click a section icon (e.g. `Sky & Clouds`). 3) Observe the panel: it now
  starts at `Particle Field`; `Renderer`/`Sky & Clouds`/`Water` are off the top, and the
  vertical scrollbar is only visible on hover.
- Expected: the panel stays at the top after a section switch (or scrolls the selected row
  into view).
- Actual: the scroll offset is preserved/re-applied (content y ≈ −104 in the UI dump), so
  the top of the list disappears without an obvious cue. Scrolling back needs several
  wheel notches (~28 px each).
- Evidence: qa/screenshots/pixel/02-pixel-toolbar.png (top) vs
  qa/screenshots/pixel/03-sky-section.png (scrolled), qa/screenshots/pixel/73-panel-scrolled-bottom.png.
- Undo: not applicable.

## Covered

- [x] `Pixel Preview` off/on: grid/axes visible vs hidden, pixel look applied; the
      Inspector `Preview` checkbox mirrors the button.
      `06-preview-off.png`, `07-preview-on.png`.
- [x] `Animate` on: two captures 2.5 s apart differ (22,859 px inside the viewport);
      off: two captures 2.5 s apart are byte-identical. `08-animate-a.png`,
      `09-animate-b.png`, `10-animate-off-a.png`, `10b-animate-off-b.png`.
- [x] Pixel World menu: Add list (all 16 sections), lighting presets, Wire refs, and the
      **Features** submenu with 14 toggles. `04-pixel-world-menu.png`,
      `04b-pixel-world-features.png`, `04c-features-submenu.png`.
- [x] Feature switches via the menu: `Outlines` off/on (viewport changes; no undo action
      for a project setting), `God Rays` off/on (restored).
      `05-feature-outlines-off.png`, `05b-feature-outlines-on.png`,
      `87-feature-godrays-off.png`, `88-feature-godrays-on.png`.
- [x] PixelRenderer, all 9 pages: Renderer, Outlines, Color/Palette, Rays, Sun, Sheets,
      Clouds, Depth Fade, Debug Views. `11-renderer-renderer.png`,
      `11-renderer-outlines.png`, `12-renderer-color-palette.png`,
      `13-renderer-rays.png`, `14-renderer-sun.png`, `15-renderer-sheets.png`,
      `16-renderer-clouds.png`, `17-renderer-depth-fade.png`,
      `18-renderer-debug-views.png`.
- [x] Renderer fields: Pixel Size drag (viewport reacts; two-step undo bug),
      Darken drag (viewport darkens; one-step undo), Debug Views → Normals (viewport
      switches to the normal buffer; action `Set Debug Views View`, undo returns Final).
      `20b…20e`, `21`, `19-debug-normals.png`.
- [x] Sky & Clouds section (single page; 14 fields). `22-sec-sky.png`; Deck off is one
      undo action (`Set Sky & Clouds Deck`). `82-sky-deck-off.png`.
- [x] Day / Night, all 3 pages: Time, Sun & Fog, Rim Light. Time edited by text entry
      (`0.4 → 0.42 → 0.5`); each change is one undo action (`Set Day / Night Time`).
      `23-sec-daynight.png`, `40-daynight-sun-fog.png`, `41-daynight-rim-light.png`,
      `81-daynight-time-042.png`, `84-daynight-time-05.png`.
- [x] Weather, all 6 pages: Weather, Snow, Particles, Rain, Lightning, Colors.
      `24-sec-weather.png`, `42-weather-snow.png`, `43-weather-particles.png`,
      `44-weather-rain.png`, `45-weather-lightning.png`, `46-weather-colors.png`.
- [x] Water, all 7 pages: Surface, Waves, Light, Caustics, Foam, Colors, Waterfalls.
      `25-sec-water.png`, `47-water-waves.png`, `48-water-light.png`,
      `49-water-caustics.png`, `50-water-foam.png`, `51-water-colors.png`,
      `52-water-waterfalls.png`.
- [x] Fireflies (single page, 11 fields). `26-sec-fireflies.png`.
- [x] Particle Field, all 4 pages: Field, Layers, Flipbook, Look. `27-sec-particles.png`,
      `53-particles-layers.png`, `54-particles-flipbook.png`, `55-particles-look.png`.
- [x] Birds, all 4 pages: Flock, Steering, Landing, Look. `28-sec-birds.png`,
      `56-birds-steering.png`, `57-birds-landing.png`, `58-birds-look.png`.
- [x] Smoke & Fire, all 4 pages: Emission, Fire Light, Options, Colors.
      `29-sec-smoke.png`, `59-smoke-fire-light.png`, `60-smoke-options.png`,
      `61-smoke-colors.png`.
- [x] Light Volumes (single page, 12 fields). `30-sec-lights.png`.
- [x] Grid / Fog, both pages: Grid, Fog of War. `32-sec-gridfog.png`,
      `62-gridfog-fog-of-war.png`.
- [x] Grass Fire, all 6 pages: Fire, Spread, Wind & Rain, Regrowth, Look (plus Ignite
      Here / Put Out / Reset actions, not exercised). `39-sec-grassfire.png`,
      `63-grassfire-spread.png`, `64-grassfire-wind-rain.png`,
      `65-grassfire-regrowth.png`, `66-grassfire-look.png`.
- [x] Missing-effect empty states: Erosion and Wild Flowers show
      "No MarchingSquaresErosion/WildFlowers in this scene: Add puts one under the
      selected (or first) terrain." `33-sec-erosion-missing.png`,
      `34-sec-wildflowers-missing.png`.
- [x] Pixel World panel: row **Select** selects the node (Water → PixelWater);
      `+ Add` adds Sun Shafts and Wild Flowers (undo/redo one action each);
      lighting preset **Dawn** applies `time_of_day 0.4 → 0.2705` and is one undo action
      (`Apply Dawn preset`); **Wire Sun / Env Refs** ran (0 refs, no change);
      scrolls to the preset grid and Wire button. `89-row-select-water.png`,
      `68-sunshafts-added.png`, `71-wildflowers-added.png`, `72-lighting-dawn.png`,
      `73-panel-scrolled-bottom.png`, `74-wire-refs.png`.
- [x] PixelPalette: opened from the renderer's Palette resource field (inline inspector:
      Tints, Quantize Mode, Weights, Brightness, Output Path, Bake LUT to PNG). Quantize
      Mode `Perceptual → Value` is one undo action (`Set quantize_mode`). Baked a
      1024×32 LUT to `res://qa_tmp/island_lut.png` (file exists in the session project);
      `76-palette-value-mode.png`, `75-palette-bake.png`, `76b-palette-restored.png`,
      `ui/palette-menu.png`.
- [x] PixelObject: selected `Characters`; Object section (single page) with Pixel Size,
      Outline, Outline Id, Palette, Dither, Own Color, Outline Col., Texel Snap, Convert,
      Bands, Shade Floor, Rim; `Profiles` visible in the Inspector. Outline
      `Inherit → Off` changes the prop (−1 → 0), one undo action (`Set Pixel Object
      Outline`); viewport diff at the characters ≈ 322 px (they are tiny in the default
      overview). `77-pixelobject-characters.png`, `78-object-outline-off.png`,
      `79-object-outline-restored.png`.
- [x] PixelOutlineColors: assigned a new resource from the renderer's
      `Outline Colors` slot (one undo action `Set outline_colors`); the inline inspector
      then exposes `Tints`, 256-entry exterior/interior color arrays, and their flags.
      `86-outline-colors-created.png`, `86b-outline-colors-assigned.png`,
      `ui/outline-colors-inline.png`.
- [x] Undo/redo summary: Add node (Sun Shafts, Wild Flowers), Dawn preset, outline
      toggle, deck toggle, palette quantize mode, debug view, day/night time, object
      outline — all verified with `key z --ctrl` / `key z --ctrl --shift` (not the direct
      `undo`/`redo` commands). Float slider drag = 1 step; integer slider drag = 2 steps
      (finding).
- [x] Log scan before/after: no addon `ERROR`/`SCRIPT ERROR`; only the known startup
      warnings and the two informational Pixel World/PixelPalette prints.

## Untested / out of scope

- PixelObjectProfile per-part overrides (the `Profiles` array was visible but no profile
  element was added/edited).
- Editing the 256-color arrays of PixelOutlineColors (the resource was created/assigned
  and its arrays exposed, but no color value was changed).
- Running `MarchingSquaresErosion` (only its empty state and Add path were visited) and
  scattering `MarchingSquaresWildFlowers` (only added/undone).
- Weather/Water/Birds/Smoke effects observed at runtime (sections and fields only; Animate
  proved cloud motion, not rain/snow/lightning).
- Sun Shafts visibility with a manually assigned Sun (the added node was left unwired).
- Grass Fire `Ignite Here` / `Put Out` / `Reset` actions.
- Resizing the editor (the pass ran at 1600×900 only); panel clipping at smaller sizes was
  not audited here.
- Saving the scene with pixel edits and reopening to prove no preview state leaks into the
  `.tscn` (the scene was closed **without saving**; the reopened scene was clean, which
  already shows no on-disk leak).
