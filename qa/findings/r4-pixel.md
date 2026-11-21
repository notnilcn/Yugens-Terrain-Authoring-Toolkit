# R4 Pixel Look QA — Renderer, Palette, Object, Pixel World (+ undo)

Session `r4-pixel` on display `:34`, port `8814`, Godot 4.7.2, llvmpipe, 1600×900.
Scene `res://tutorial/03_pixel_look.tscn`, plus `res://demo/hex_island.tscn` for PixelObject /
feature switches. Project copy: `/tmp/opencode/qa/sessions/r4-pixel/project`; log
`/tmp/opencode/qa/sessions/r4-pixel/editor.log`. Addon untouched; every scene edit was undone
or discarded (the session copy was restarted once). Screenshots: `qa/screenshots/r4-pixel/`.
UI dumps: `qa/screenshots/r4-pixel/ui/`.

## PixelizerEditorPlugin::on_float panics with a re-entrant borrow during a UI rebuild

- Severity: error
- Steps: 1) At 1600×900 select `PixelRenderer` and visit several strip pages (Rays, Color,
  Sky, Day/Night, Pixel Object in `hex_island`). 2) Resize to 1024×600. 3) Select
  `HexIsland/PixelRenderer` from the Scene tree.
- Expected: the pixelizer strip rebuilds for the new selection without an error.
- Actual: the Output dock fills with four `ERROR` lines; the editor continues but the strip
  stops responding to the slider that emitted. This is a Rust panic converted by godot-rust:
  ```
  ERROR: [panic /home/clintonnguyen/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/godot-core-0.5.5/src/storage/mod.rs:38]
    Gd<T>::bind_mut() failed, already bound; T = marching_squares_terrain::editor::pixel_ui::plugin::PixelizerEditorPlugin.
      Make sure to use `self.base_mut()` instead of `self.to_gd()` when possible.
      Details: cannot borrow while accessible mutable borrow exists.
  ERROR: Error calling from signal 'value_changed' to callable: 'PixelizerEditorPlugin::on_float': .
     at: emit_signalp (core/object/object.cpp:1311)
  ```
  The same two lines appeared again later in the Output dock (5 errors in total on the badge)
  but a second `resize 1024 600` + `select PixelRenderer` did not reproduce it, so it is
  timing-dependent (a strip field emits `value_changed` while the plugin is rebuilding).
- Evidence: qa/screenshots/r4-pixel/73-resize-1024-renderer.png,
  `76-resize-1024-panel-clipped.png`; editor.log lines 14–20.
- Undo: not applicable (log/UI error, no document edit).

## Pixel World panel is placed off-screen at 1024×600

- Severity: ui
- Steps: 1) Select `PixelRenderer` at 1600×900 (panel is a 250 px right-side ScrollContainer).
  2) `resize 1024 600`, wait. 3) Capture; dump `ui --root all --filter PixelWorldPanel`.
- Expected: the panel stays reachable inside the window (or collapses/overlays).
- Actual: `PixelWorldPanel` rect is `[968, 121, 250, 215]` — its right edge is x=1218 on a
  1024 px window, so only a ~56 px sliver is visible and no Preset / Wire button can be
  reached. The Inspector dock is also pushed off-screen, and the 3D viewport shrinks to
  roughly 125 px tall. The pixelizer strip still fits horizontally (ends near x=966).
- Evidence: qa/screenshots/r4-pixel/73-resize-1024-renderer.png,
  `74-resize-1024-fullscreen.png`, `76-resize-1024-panel-clipped.png`;
  `ui/1024-panel.json`.
- Undo: not applicable. At 1600×900 the panel is a non-resizable, non-collapsible
  250×469 ScrollContainer with 941 px of content; the Lighting Preset grid and
  Wire button are reachable only after scrolling (`16-pixel-world-panel-bottom.png`).

## Spin-slider drags leave their documented range by orders of magnitude

- Severity: warning
- Steps: 1) Renderer page, drag **Pixel Size** (2, range 1–16+) 20 px right over 1.2 s.
  2) Outlines page, drag **Darken** (0.55, range 0–1) 18 px. 3) Rays page, drag
  **Ray Alpha** 30 px.
- Expected: drag clamps at the soft range end; larger values are only possible by typing.
- Actual: `pixel_size` = **39087**, `outline_darken` = **283.311**, `ray_alpha` = **357.47**.
  The viewport degrades to 1-px blocks / near-white or near-black output. This confirms the
  2026-11-20 finding: every field is created with `set_allow_greater(true)` /
  `set_allow_lesser(true)`, including Float fields that have a 0–1 range.
- Evidence: qa/screenshots/r4-pixel/47-pixelsize-drag.png,
  `51-darken-drag.png`, `44-ray-godrays-on.png`.
- Undo: one Ctrl+Z restores each value (works); the range problem is separate from undo.

## One integer spin-slider drag still creates two undo steps

- Severity: warning
- Steps: 1) Record `undo-state` (0 actions). 2) Drag Pixel Size 20 px. 3) `undo-state`
  shows `undo_count: 2`, both named `Set Renderer Pixel Size`. 4) Ctrl+Z once → 12972;
  Ctrl+Z again → 2. 5) Ctrl+Shift+Z once → 12972.
- Expected: one continuous drag is one undo step.
- Actual: two steps; the first Ctrl+Z only walks back to an intermediate drag value
  (12972), not to the pre-drag 2. A Float drag (Darken) was one step in the same build.
- Evidence: qa/screenshots/r4-pixel/47-pixelsize-drag.png,
  `48-pixelsize-undo1.png`, `49-pixelsize-restored.png`.
- Undo: broken (granularity, not absence).

## Undo of a lighting preset leaves the editor preview tinted until the scene/session is rebuilt

- Severity: warning
- Steps: 1) Pixel World menu → **Dawn**. Sky goes flat salmon (R=218, G=150, B=153);
  `time_of_day` 0.42 → 0.2705, `god_rays` false → true, one action `Apply Dawn preset`.
  2) Ctrl+Z: Inspector values revert (0.42, god_rays false) but the sky stays salmon;
  Pre-dawn was blue (126,170,227); 203,103 viewport pixels still differ. 3) Toggle
  Pixel Preview off/on: still salmon (218,149,153). 4) Pixel World → **Storm**: sky becomes
  light blue (159,188,219); Ctrl+Z reverts `time_of_day` but the sky stays light blue.
- Expected: undo restores the rendered preview, not only the Inspector properties.
- Actual: the preview's sky/grade state is cached; only a **session restart** cleared it in
  this pass (closing/reopening the scene tab with Ctrl+W did not; it kept the same tab).
  Nothing is saved to the `.tscn` — the session copy was restarted without saving.
- Evidence: qa/screenshots/r4-pixel/10-lighting-dawn.png,
  `11-dawn-undone.png`, `12-dawn-stale-after-preview-toggle.png`,
  `17-lighting-noon.png` (actually Storm), `18-storm-undone.png`,
  `19-clean-renderer.png` (after restart, blue again).
- Undo: broken (properties revert, preview does not).

## "Wire Sun / Env Refs" reports 0 references and leaves a new SunShafts node unwired

- Severity: minor
- Steps: 1) Pixel World menu → **Sun Shafts** (`Add PixelSunShafts`). 2) Inspect the new
  `SunShafts`: its `sun` property is `null` (Inspector shows `Assign...`). 3) Run
  **Wire Sun / Env Refs**.
- Expected: the add path or the wire action fills the scene's DirectionalLight3D (the Add
  tooltip says "references wired" and docs/08 say the menu wires sun/environment refs).
- Actual: the wire action prints `Pixel World: wired 0 reference(s).` (editor.log) and the
  SunShafts `sun` stays `null`. Wire covers only PixelSky / PixelDayNight / PixelWeather.
- Evidence: qa/screenshots/r4-pixel/14-sunshafts-added.png,
  `13b-wire-refs-output.png`; editor.log line "Pixel World: wired 0 reference(s).".
- Undo: Add is one undo action (verified Ctrl+Z removes it, Ctrl+Shift+Z re-adds);
  Wire creates no action because it changes nothing.

## Pixel Preview / Animate have no visible pressed state

- Severity: minor
- Steps: 1) Capture with Pixel Preview on (Inspector `Preview` checked, no grid/axes).
  2) Click **Pixel Preview** (grid/axes appear, Inspector unchecked). 3) Compare button rect
  `[291,115,124,33]`.
- Expected: a toggle button shows its on/off state (pressed background, tick, etc.).
- Actual: the button background/text look identical in both states. The same is true for
  **Animate**; the only feedback is the viewport and the Inspector checkbox.
- Evidence: qa/screenshots/r4-pixel/03-preview-on-before.png vs `04-preview-off.png`;
  `ui/preview-off.json` (`toggled:false`). Animate on moved 14,052 viewport pixels in 3 s;
  off was byte-identical over 3 s, so the feature works, only the affordance is missing.
- Undo: not applicable (editor toggles, no document action).

## Renderer page tab row jumps vertically between pages

- Severity: ui
- Steps: 1) With the Renderer section active, dump `ui` and note the page-tab row.
  2) Switch Renderer → Outlines → Rays; re-dump after each switch.
- Expected: a stable tab bar so a tab can be clicked repeatedly.
- Actual: the row is at y=485 on the Renderer/Color/Sky pages, y=450 on Outlines, and
  y=520 on Rays (one or two 35 px rows). Clicking a tab at the previous y (for example
  x=476,y=502 after switching from Renderer to Outlines) misses the tab and does nothing;
  re-dumping the UI before every click was required. This re-confirms the 2026-11-20 finding.
- Evidence: qa/screenshots/r4-pixel/34-renderer-page.png (tabs y≈485),
  `43-outlines-retry.png` (tabs y≈450), `37c-rays-int-click.png` (tabs y≈520); UI rects in
  `ui/` (`SectionPages`).
- Undo: not applicable.

## Pixel World "+ Add" closes the pixelizer UI by selecting the new node

- Severity: ui
- Steps: 1) Select `PixelRenderer`. 2) Pixel World panel → Sun Shafts → **+ Add**.
- Expected: the panel stays usable to add a second effect (or focus returns to the renderer).
- Actual: the new `SunShafts` is selected; the left section toolbar, the strip and the
  Pixel World panel all disappear until the renderer/terrain is reselected.
- Evidence: qa/screenshots/r4-pixel/14-sunshafts-added.png.
- Undo: works — one Ctrl+Z removes the node (`Add PixelSunShafts`), Ctrl+Shift+Z re-adds it.

## PixelSky Coverage change had no visible viewport effect

- Severity: minor
- Steps: 1) Sky & Clouds section → drag **Coverage** from 0.4515 to ~0.6. 2) Diff the
  viewport against the before capture.
- Expected: cloud shadows / deck coverage visibly change (the same field is documented as
  0.3–0.7).
- Actual: 0 viewport pixels changed (before/after and before/undo are byte-identical in the
  viewport crop). Likely because the deck sits at Height 300 above this overview camera, but
  the field looks inert from the default 03_pixel_look view. The change is still recorded as
  one undo action (`Set Sky & Clouds Coverage`) and Ctrl+Z reverts it.
- Evidence: qa/screenshots/r4-pixel/55-sky-section.png, `56-sky-coverage.png`,
  `57-sky-coverage-undone.png`.
- Undo: works (one step).

## Covered

- [x] Pixel Preview on/off: grid + origin axes hidden/shown; Inspector `Preview` mirrors the
      button; turning it off and on restores the pixel look. `03-preview-on-before.png`,
      `04-preview-off.png`, `12-dawn-stale-after-preview-toggle.png`.
- [x] Animate on: two 3 s captures differ by 14,052 viewport pixels (clouds/terrain shading).
      Animate off: two 3 s captures are byte-identical (frozen). `05-animate-a.png`,
      `06-animate-b.png`, `07-animate-off-a.png`, `08-animate-off-b.png`.
- [x] Pixel World menu: 27-item add/preset/wire menu, Fitting in 648 px; disabled
      Renderer / Sky & Clouds / Day / Night entries are visibly greyed. `09-pixel-world-menu.png`.
- [x] Pixel World panel: row Select (Renderer), row Add, Lighting Preset grid (Dawn/Noon/
      Dusk/Night/Storm/Snow), Wire button, disabled rows greyed, scrollable to the bottom.
      `16-pixel-world-panel-bottom.png`, `70b-features-submenu.png`.
- [x] PixelRenderer section + every page: Renderer, Outlines, Color / Palette, Rays, Sun,
      Sheets, Clouds, Depth Fade, Debug Views. `34-renderer-page.png`,
      `43-outlines-retry.png`, `52-color-page.png`, `37c-rays-int-click.png`,
      `38-renderer-sun.png`, `39-renderer-sheets.png`, `40-renderer-clouds.png`,
      `41-renderer-depthfade.png`, `42-renderer-debug.png`.
- [x] Renderer fields: Pixel Size drag (2 → 39087, two-step undo), Darken drag (0.55 →
      283.311, one step), God Rays checkbox (false→true→false, one step), Ray Alpha drag
      (1.0 → 357.47, one step), Use Palette (off→on→off, one step). `47`–`54`.
- [x] Sky & Clouds: full single page (Clouds/Coverage/Softness/Scale/Wind/Height/Shadow/
      Bands/Deck/Deck Size/Opacity/Color/Shade); Coverage drag = one undo step.
      `55`–`57`.
- [x] Day / Night: page tabs (Time / Sun & Fog / Rim Light), Time spinner present; a
      Day/Night Time change records `Set Day / Night Time`, but a 12 px drag left the value
      at 0.42 and the earlier keyboard attempt produced two undo entries (inconclusive; not
      reported as a defect). `58-daynight-section.png`, `61-daynight-restored.png`,
      `62-daynight-drag.png`.
- [x] Weather: empty-state "No PixelWeather in this scene — Add Weather"; **Add Weather**
      creates `PixelWeather`, one undo action (`Add PixelWeather`), Ctrl+Z removes it.
      `63-weather-section.png`, `64-weather-added.png`.
- [x] Water, Fireflies, Particle Field, Birds, Smoke & Fire, Light Volumes, Grid / Fog,
      Erosion (empty state), Wild Flowers (empty state), Grass Fire: section strips visited
      and captured. `23`–`33`.
- [x] PixelObject: `HexIsland/Characters` Object section (Pixel Size, Outline, Outline Id,
      Palette, Dither, Own Color, Outline Col., Texel Snap, Convert, Bands, Shade Floor,
      Rim). Outline `Inherit → Off` = one undo action (`Set Pixel Object Outline`); Ctrl+Z
      reverts. `65-pixelobject-section.png`, `66-object-outline-popup.png`,
      `67-object-outline-off.png`, `68-object-outline-undone.png`.
- [x] PixelOutlineColors: encountered as the renderer's Color slot; assigning/editing the
      256-entry color arrays was not exercised (see Not tested).
- [x] Feature switches: Pixel World → **Features** submenu (14 toggles, all checked);
      **Outlines** off changed 61,306 viewport pixels (grid/outline pass off), restored on;
      no undo action (project setting, expected). `70b-features-submenu.png`,
      `71-feature-outlines-off.png`, `72-feature-outlines-on.png`.
- [x] Undo after clicking in the 3D view: changed `texel_snap` (true→false), clicked empty sky
      (selection cleared), Ctrl+Z restored `true` and pushed the change to redo.
      `69-hex-terrain-selected.png`; `ui/` dumps.
- [x] Undo after section/node switches: Add/undo/redo for SunShafts and Weather, object outline,
      Sky Coverage, God Rays, Ray Alpha, Use Palette, and the palette clear all verified with
      `key z --ctrl` / `key z --ctrl --shift` (not the direct `undo`/`redo` commands).
- [x] Resizability: 1600×900 and 1024×600 captured; panel off-screen at 1024×600 (finding).
      `73-resize-1024-renderer.png`, `74-resize-1024-fullscreen.png`, `75-resize-back-1600.png`,
      `76-resize-1024-panel-clipped.png`.
- [x] Log scan: no addon errors on startup (only the known V-Sync / missing-UID warnings);
      the on-float panic is the one new error (finding above).

## Not tested

- [ ] PixelPalette resource editing: opening the embedded `Tutorial` palette inline,
      palette presets, Tints/source image changes, Quantize Mode changes, and
      **Bake LUT to PNG** (the Color page's Palette slot was only inspected; clicking it
      accidentally cleared the resource, which was undone).
- [ ] PixelObjectProfile per-part overrides (the Profiles array was not expanded/edited).
- [ ] PixelOutlineColors: creating/assigning a new resource and changing its 256-entry arrays.
- [ ] Weather fields after adding PixelWeather (Rain/Snow/Storm, lightning, intensity), and
      Water/Fireflies/Particles/Birds/Smoke/Light Volume field changes with undo.
- [ ] Daylight/Dusk/Night presets (only Dawn and Storm were applied; both had the stale
      preview issue).
- [ ] Project Settings → marching_squares_terrain/features dialog itself (the same switches
      were exercised from the Pixel World → Features submenu).
- [ ] LUT bake menu item (`Project > Tools` or addon menu) and palette presets.
- [ ] 1920×1080 / 1366×768 window sizes; 1024×600 was the only small size audited.
- [ ] Multi-select/cycling when several PixelRenderers exist.

## Action × undo matrix

| Action | Ctrl+Z result | Redo | Verdict |
|---|---|---|---|
| Pixel Preview toggle | no action | — | not applicable (editor toggle) |
| Animate toggle | no action | — | not applicable (editor toggle) |
| Apply Dawn / Storm preset | properties revert; preview stays tinted | — | broken (preview stale) |
| Add PixelSunShafts / PixelWeather | node removed, one step | re-adds | works |
| Renderer Pixel Size drag (int) | only walks back to mid-drag value | advances one step | broken granularity (2 steps) |
| Outlines Darken drag (float) | restores 0.55 | restores drag value | works (over-range accepted) |
| God Rays checkbox | false | true | works (one step) |
| Ray Alpha drag (float) | restores 1.0 | restores drag value | works (over-range accepted) |
| Use Palette checkbox | false | true | works (one step) |
| Sky & Clouds Coverage drag | restores 0.4515 | restores value | works (no visible diff) |
| Day / Night Time | action recorded; drag value unchanged in test | — | inconclusive |
| Weather Add Weather | node removed | re-adds | works |
| PixelObject Outline Inherit→Off | restores Inherit | Off | works (one step) |
| Feature switch Outlines | no action (project setting) | — | not undoable by design |
| Palette resource clear (accidental) | palette restored (embedded resource) | — | works |
| Undo after 3D-view click | restores field | — | works |
