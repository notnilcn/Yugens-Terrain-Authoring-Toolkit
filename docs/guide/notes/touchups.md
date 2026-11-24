# Touch-up capture — pages 07-pixelizer and 09-builder-greybox

> Session `docs-touchup`, display `:76`, port `8876`. Media only; the orchestrator updates the HTML.
> This file lists what the touch-up run produced (re-takes keep their old filenames; new files are
> numbered after the existing ones). Captured 2026-11-24.

## A. Page 07-pixelizer (scene `res://tutorial/03_pixel_look.tscn`) — DONE

### Re-takes (same filenames)
- `15-strip-pixelobject.png` — **DONE.** Re-cropped to include the cut-off top row
  (**Pixel Object** tab · **Pixel Size** · **Outline**) down through Bands/Shade Floor/Rim.
- `25-debug-normals.png`, `25-debug-depth.png`, `25-debug-pixelsize.png`,
  `25-debug-outlineids.png`, `25-debug-outlinecause.png` — **DONE.** Re-shot the whole debug
  series at one side-on framing (terrain + trees against the sky) so the terrain is clearly
  visible in every view. Normals is no longer a flat lavender frame: ground reads green, sky
  blue, tree silhouettes stand out. Captured by setting `PixelRenderer.debug_view` 1/2/3/4/6
  (Normals/Depth/Pixel Size/Outline Ids/Outline Cause). Crop (345,210,1050,598).
- `26-godrays-off.png`, `26-godrays-on.png`, `27-rayalpha-0.5/1.0/2.0/4.0.png` — **DONE,
  whole set re-shot** at ONE sun-facing camera (the old camera could not be reproduced, so the
  whole god-ray set was re-taken for a consistent scrubber). God rays now visibly ON at every
  alpha; `0.5` is a genuine faint shaft (843 changed px vs `off`, growing to 2615 at `1.0`),
  no longer a bit-copy of `off`. `god_rays`/`ray_alpha` set via set-prop; Animate OFF for
  stable clouds; `26-on` == `27-rayalpha-1.0` (identical settings). Crop (305,170,1058,460).
  NOTE for the writer: `0.5` is deliberately faint (half strength) — it reads as a soft shaft,
  not a strong beam; the beam strengthens across 1.0→4.0.

### New media
- **Palette-preset series** — PixelPalette `quantize_mode` (the only built-in multi-choice
  "palette preset"): `80-palette-perceptual.png`, `81-palette-channel.png`, `82-palette-hue.png`,
  `83-palette-hueweighted.png`, `84-palette-lab.png`, `85-palette-value.png`. Same framing/crop
  (345,210,1050,598), only the quantize mode changes. Scrubber-ready (6 frames in enum order).
  Looks: Perceptual = lavender sky / muted greens (default); Channel shifts greens; Hue +
  HueWeighted turn the sky teal (these two are visually identical for this scene — a 0-pixel
  diff, expected); Lab shifts again; Value is dramatic (orange sky, multicolor terrain patches).
  Set via the palette sub-inspector's **Quantize Mode** OptionButton (Color group → Palette →
  Quantize Mode). The quantize enum lives on the PixelPalette resource (Inspector only), NOT in
  the bottom Color/Palette strip.
- **8 clips** (all MP4 + poster, verified by poster + mid-frame):
  - `41-pixelsize-drag.mp4` — scrubbing **Pixel Size** 1→12→2 on the Renderer tab; viewport
    chunkifies, field + tooltip shown. (Real slider drags overshoot wildly under XTEST —
    see caveat — so the value was stepped with set-prop while the cursor tracked the slider.)
  - `42-pixel-preview-toggle.mp4` — clicking **Pixel Preview** off (raw smooth scene + editor
    grid + origin axes) then on (pixel look, grid hidden).
  - `43-outlines-darken.mp4` — **Outlines** tab, scrubbing **Darken** 0→1 (edges thicken/darken).
  - `44-palette-mix.mp4` — **Color / Palette** tab, scrubbing **Mix** 0→1 (raw colours fade to
    the muted palette).
  - `45-godray-alpha.mp4` — **Rays** tab, scrubbing **Ray Alpha** 0→4 (teal shafts grow in).
  - `46-lighting-preset.mp4` — **Pixel World** panel → click **Dusk**: the whole scene recolours
    to warm dusk orange (daytime → dusk). Animate ON so the Sky/Day-Night preview refreshes.
  - `47-wire-refs.mp4` — **Pixel World** panel → **Wire Sun / Env Refs** (tooltip + click;
    HUD/log: "Pixel World: wired 0 reference(s)." — this scene was already fully wired).
  - `48-debug-cycle.mp4` — **Debug Views** tab, cycling the **View** dropdown
    Final→Normals→Depth→Pixel Size→Outline Ids→Raw Color→Outline Cause→Shafts→Final.

### Capture caveats (Page A)
- **EditorSpinSlider drags overshoot under XTEST** (a 50 px drag sent Pixel Size to 42582).
  All four slider "drag" clips (41/43/44/45) were recorded by stepping the value with set-prop
  while moving the cursor along the slider, so they read as a controlled scrub without garbage
  values. Knob stills are unaffected.
- The **Quantize Mode OptionButton popup** is flaky through synthetic keyboard input (dropped/mis-
  counted keys). Reliable method: open the popup, then click the target item by **pixel position**
  (the inline popup renders just under the field) and verify the committed label via `ui`.
- Captured at window **1600×900** for everything except clips **46/47**, where the window was
  grown to **1600×1060** to bring the Pixel World panel's Lighting Preset + Wire buttons on-screen
  (they sit below the fold at 900 and the panel's wheel-scroll did not respond; its VScrollBar
  thumb had to be dragged).
- Applying the **Dusk** preset (clip 46) sets a persistent global dusk look for the rest of the
  session (documented caveat); all clean "look" shots (palette/debug/godrays/outlines) were taken
  before it.

## B. Page 09-builder-greybox (scene `res://demo/hex_island.tscn`) — BLOCKED

**Blocker: the "new parts = solid Roblox Medium Stone Grey" fix is NOT in the session's
compiled GDExtension.** Newly inserted PixelBuilderMesh parts still render **see-through**
(you see the terrain through their faces) in BOTH the pixel-preview and the raw 3D view —
exactly the pre-fix behaviour the whole Page-B re-record is meant to replace.

- **Verified:** with Roblox UI on, `BuilderRuin/Ruin` selected, clicked **Insert part** → a Box
  dropped at the view centre and rendered as a transparent blue-outlined ghost. Toggling
  **Pixel Preview** off (raw view) showed the same see-through box. No default material was set.
  Evidence: `docs/guide/notes/touchup-pageB-seethrough-box.png` (raw view, ghost box over terrain).
- **The fix exists only as uncommitted working-tree source**, not in the loaded `.so`:
  `git diff` shows `builder/plugin/roblox_panels.rs` (give_new_part_material now calls
  `part_rules::grey_bare_faces(...)` when no default material is set) and a new
  `builder/roblox_ui/part_rules.rs::grey_bare_faces`. The session boots a pre-built binary and
  CAPTURE.md forbids running cargo builds, so I cannot compile the fix into the capture binary.
- **Why not fake it:** the pre-fix "step 0" workaround (set the **Grid** built-in material as the
  new-parts default, with nothing selected) DOES work in the old binary — I opened the Material
  picker (Built-in shelf) and selected Grid. But Grid is a **grid-textured** grey, not the fix's
  plain **Medium Stone Grey**; clips shot that way would not match the shipped fixed look and
  would need re-recording after a rebuild anyway, so I did not record them.

**What the orchestrator needs to do:** rebuild the MarchingSquaresTerrain GDExtension so the
loaded binary includes the committed `grey_bare_faces` fix, then a fresh capture agent can
re-record the 8 clips (they will be auto-grey on insert, no manual material needed):
`10-insert-shapes`, `18-intersect`, `19-union-separate`, `40-doorway-02-union`,
`21-tower-floor`, `22-tower-walls`, `23-tower-roof`, `24-tower-final` — same content/framing as
the originals (rects in `09-builder-greybox.md` follow-up manifest). _Alternatively_, if the
Grid-material (greybox) grey is acceptable instead of plain Medium Stone Grey, say so and a
re-record can set Grid as the new-parts default first (no rebuild needed).

> Capture note: while probing this, a `Delete` on the test Box triggered a confirm dialog whose
> buttons have no screen rect in the harness; a stray click emptied the hex_island edit session.
> Recovered cleanly with **Scene → Reload Saved Scene → Don't Save** (the `.tscn` on disk was
> never modified). Session left at the restored scene.
