# Capture notes — page `07-pixelizer` (The Pixel Look)

Area C06. Session `docs-pixel`, display `:76`, port `8876`, scene `res://tutorial/03_pixel_look.tscn`.
Source of truth for the writer agent. Everything below was verified in the running editor and
cross-checked against the Rust source under
`addons/MarchingSquaresTerrain/rust/crates/mst/src/` (`render/`, `editor/pixel_ui/`,
`editor/preview_plugin.rs`, `editor/preview_controls.rs`).

This page owns the **renderer-level "look" sections**: the 3D-view pixel controls
(Pixel Preview / Animate / Pixel World), and — when you select a `PixelRenderer` — the
Renderer, Outlines, Color / Palette, God Rays, Debug Views, Pixel Object, and Grid / Fog
sections, plus the `PixelPalette`, `PixelObject` / `PixelObjectProfile`, `PixelOutlineColors`
resources and the Project-Settings feature switches.
**Page 08 (another agent) owns** Sky & Clouds, Day / Night, Weather, Water, Fireflies,
Particle Field, Birds, Smoke & Fire, Light Volumes, Sun Shafts — this page captures only ONE
overview shot of each and says "see page 08".

---

## 0. Big picture — how the pixel workspace is laid out

The addon renders your 3D scene as a flat, outlined, limited-palette **pixel-art** image (think
a 2000s handheld RPG) instead of smooth 3D. A `PixelRenderer` node drives it; a family of
`Pixel*` nodes add the sky, weather, water and effects.

There are **three editor surfaces**, all only visible in the **3D view** (the big centre
viewport):

1. **The 3D-view menu bar** (top strip of the viewport, next to `Transform` / `View`): four
   controls — **Pixel Preview**, **Animate**, **Pixel World** (a dropdown menu), **Roblox UI**
   (a toggle; belongs to the Builder page 10). These show **always**, even with nothing selected.
2. **The left icon toolbar** + **the bottom attribute strip**: appear only while a `Pixel*`
   node (or the terrain, for Grid/Fog) is selected. The left toolbar is one icon per node
   *family* (Renderer, Sky & Clouds, Day / Night, …); the bottom strip shows that family's
   settings, split into **tabs** across the top of the strip.
3. **The right "Pixel World" panel**: a list of every pixel node family with Select / + Add
   buttons, a row of **Lighting Preset** buttons, a **Wire Sun / Env Refs** button and a hint
   line. Appears whenever the workspace is active.

**Roblox bridge:** This is like Roblox Studio's *View* → turning on a rendering style, plus a
docked panel of object categories. The left toolbar ≈ the Studio ribbon's tool groups; the
bottom strip ≈ the Properties panel for the selected thing; the right panel ≈ the Explorer's
"insert object" list but scoped to pixel effects.

### The scene used (`03_pixel_look.tscn`)
Nodes: `PixelShowcase` (root) → `WorldEnvironment`, `Sun` (DirectionalLight3D),
`MarchingSquaresTerrain`, `BuilderRuin` (a **PixelObject**) → `Ruin` (a PixelBuilderMesh),
`Camera` (MstOrbitCamera), `PixelRenderer` (pixel_size 2, palette enabled), `PixelSky`,
`PixelDayNight`. No Weather/Water/effects present → those rows show "+ Add" in the panel and
their toolbar icons are dimmed.

---

## 1. 3D-view menu bar (always visible)

| Control | Type | What it does | Tooltip (verbatim) |
|---|---|---|---|
| **Pixel Preview** | toggle button (icon + text) | Routes the **editor** viewport through the pixel renderer (the scene's `PixelRenderer`, or a default one if none). While ON, the editor grid and origin axes are **hidden**; the move/rotate/scale gizmo and node gizmos still draw on top. | "Pixel Preview\nShow the editor viewport through the pixel renderer (the scene's PixelRenderer, or a default one).\nThe editor grid and origin axes are hidden while it is on; the move / rotate / scale gizmo and node gizmos still draw on top." |
| **Animate** | toggle button | Runs the effects **in the editor**: clouds drift, water waves, grass sways, birds fly, weather and fire move. OFF = everything holds still so the editor only redraws when something changes (much lighter). A running game always animates. Stored in Editor Settings `marching_squares_terrain/animate_effects_in_editor`. | "Animate Effects\nRun the effects in the editor: clouds drift, water waves, grass sways, birds fly, weather and fire move.\nOff: everything holds still, so the editor only redraws when something changes (much lighter). A running game always animates." |
| **Pixel World** | dropdown menu (MenuButton) | Opens a menu: **Add** (every pixel node family, icon + label; unique ones disable when present), **Lighting Preset** (Dawn/Noon/Dusk/Night/Storm/Snow), **Wire Sun / Env Refs**, **Project → Features** submenu (the feature check-list). Mirrors the right panel's actions. | "Add pixel effects to the edited scene (sun / environment / camera refs are wired), apply a lighting preset." |
| **Roblox UI** | toggle switch | Switches the Builder's modelling tools to a Roblox-Studio ribbon. **Belongs to Builder page 10** — just note it is here. | "Roblox UI: show the modelling tools as a Roblox Studio-style ribbon with Roblox handles and shortcuts.\nOff: the regular Builder layout." |

**Gotcha — Pixel Preview default:** in `03_pixel_look.tscn` the scene's `PixelRenderer` has
`studio_prelook = true`, so **Pixel Preview is ON the moment the scene opens**. Clicking it the
first time turns it OFF (revealing the raw smooth scene). This surprised me during capture.

**Gotcha — Animate cost:** with Animate OFF the viewport is almost static (good on the slow
llvmpipe test box). Turn it on only to see motion.

---

## 2. Left icon toolbar + bottom strip (select a `PixelRenderer`)

Selecting the **PixelRenderer** node (Scene tree, or the panel's "Renderer → Select") opens the
workspace. The **left toolbar** shows one icon per node family. The **bottom strip** shows the
selected family's settings; the strip's top row is **tabs**. For the Renderer the tabs are:
**Renderer · Outlines · Color / Palette · Rays · Sun · Sheets · Clouds · Depth Fade · Debug
Views** (Rays/Sun/Sheets/Clouds/Depth Fade are the five *pages* of the God Rays section).

The toolbar icon is dimmed (40% alpha) when that family has no node in the scene; its tooltip
then says why ("no PixelWeather in the scene yet. Open it and press Add." / "needs a
MarchingSquaresTerrain…"). Each field's tooltip is `<Label>\n<Class>.<prop>\nRange a to b
(drag, or click to type).`

### Editing model (applies to every field in every section)
- **Spin-slider** (numbers): drag left/right to change, or click to type. Dragging a slider =
  **one** undo step (grab→release). `Float` fields have a step; `Int` fields step by 1; some
  Int fields are "or greater" (you can type past the max).
- **Checkbox** (bool), **dropdown** (enum), **colour button** (opens a colour picker).
- Every change is recorded in the editor's **Undo History** as `Set <Section> <Field>`
  (e.g. "Set Renderer Pixel Size"), fully undoable with Ctrl+Z / redo Ctrl+Shift+Z.
- The strip **live-refreshes** ~ every poll if the value changes elsewhere (Inspector, undo).

### 2a. Renderer section (`PixelRenderer`, tab "Renderer")
The core of the pixel look: how big the macro-pixels are and how the low-res image is built.

| Field | prop | Type | Range / default | What it does |
|---|---|---|---|---|
| Enabled | `active` | bool | — | Master switch for the whole pixel renderer. Off = scene renders normally. |
| Mode | `mode` | enum | Anchor (0) / Low Res (1) | **Anchor** = full pipeline (depth/normal outlines, palette, god rays). **Low Res** = colour only (cheaper, no outline/normal pass). |
| Pixel Size | `pixel_size` | int (1–16, or greater) | default 2 | Macro-pixel size: how many screen pixels each "art pixel" is. 1 ≈ full res; higher = chunkier/blockier. **The headline knob.** |
| Fixed Height | `fixed_height` | int (0–1080) | 0 | Lock the internal render height in pixels (0 = derive from pixel size). |
| Texel Snap | `texel_snap` | bool | on | Snap the image to the pixel grid so it doesn't shimmer when the camera moves. |
| Sub-texel Scroll | `subtexel_scroll` | bool | on | Smoothly scroll within a texel so slow pans aren't steppy. |
| Crisp Edges | `crisp_edges` | bool | on | Keep hard pixel edges (no smoothing/AA on the upscale). |
| Depth Range | `depth_range` | float (1–4000) | 200 | Depth window used for outline/precision. |
| Obj. Pixel | `object_pixel_size` | int (1–16, or greater) | 1 | Default macro-pixel size for `PixelObject` props that inherit (-1). |

### 2b. Outlines section (`PixelRenderer`, tab "Outlines")
Depth/normal edge detection → black silhouettes + crease highlights.

| Field | prop | Type | Range / default | What it does |
|---|---|---|---|---|
| Outlines | `outlines` | bool | on | Master outline switch. |
| Background | `outline_background` | bool | — | Also outline against the sky/background. |
| Depth Thr. | `depth_threshold` | float (0.0105–10) | — | How big a depth jump counts as an edge (silhouettes). Lower = more outlines. |
| Normal Thr. | `normal_threshold` | float (0–2) | — | How sharp a surface-angle change counts as a crease edge. |
| Darken | `outline_darken` | float (0–1) | — | How dark the outline is (0 = invisible, 1 = black). **Thickness/strength knob.** |
| Highlight | `highlight` | float (0–1) | — | Bright highlight on the lit side of a crease. |
| Wide Ids | `extended_ids` | bool | — | Use the extended id range for outline ids. |
| Id Edges | `id_edges` | bool | — | Draw edges where object ids differ (needs the id pass / Outline Ids feature). |
| Obj. Outline | `object_outline` | bool | — | Draw outlines around `PixelObject` props. |
| Fold Limit | `fold_limit` | float (0–20) | — | Suppress false creases on gentle folds below this. |
| Ledge Walls | `ledge_walls` | bool | — | Outline terrain ledge walls. |
| Auto Id From | `auto_id_first` | int (1–253) | — | First auto-assigned outline id. |
| Auto Focus | `auto_focus` | bool | — | Auto-pick the focus object for id edges. |

### 2c. Color / Palette section (`PixelRenderer`, tab "Color / Palette")
Palette quantisation, ordered dithering, colour grading.

| Field | prop | Type | Range / default | What it does |
|---|---|---|---|---|
| Use Palette | `palette_enabled` | bool | on (this scene) | Snap every colour to the assigned `PixelPalette`. Off = full colour. |
| Mix | `palette_mix` | float (0–1) | — | Blend between original colour (0) and palette-snapped (1). **Palette strength knob.** |
| Dither | `dither_enabled` | bool | — | Ordered (Bayer) dithering between palette colours. |
| Spread | `dither_spread` | float (0–0.5) | — | How far dithering reaches between two palette colours. |
| Matrix | `dither_size` | enum | Bayer 4 (4) / Bayer 8 (8) | Dither matrix size (4×4 vs 8×8 pattern). |
| Saturation | `saturation` | float (0–2) | 1 | Colour grade: saturation. |
| Contrast | `contrast` | float (0–2) | 1 | Colour grade: contrast. |
| Brightness | `brightness` | float (−1–1) | 0 | Colour grade: brightness. |

The palette itself is a **PixelPalette** resource (see §4). Selecting it opens the same
Color / Palette strip but drives the palette's colours.

### 2d. God Rays section (`PixelRenderer`) — pages Rays / Sun / Sheets / Clouds / Depth Fade
World-anchored sun shafts (light sheets along the sun beam, lit through the PixelSky cloud gaps).
Tooltip: "Ray Alpha 1 = the default look." The notable per-field hint is on **Adaptation**
(`eye_adaptation`, default 1.6): scales the sheet stack so side-on views keep distinct shafts
instead of a milky veil.

Page **Rays**: God Rays (bool), Ray Alpha (0–4), Color (`beam_tint`), Max Sheets (2–52),
Ray Depth (1–4000), Reach (0–1000). **Sun**: Sun Color (bool), Day Cycle (bool),
Dawn/Dusk (0–1), Sun Follow (0–1). **Sheets**: Bands (1–16), Gap (0.05–50), Energy (0–10),
Max Stack (0–10), Falloff (0–1), Surface Fade (0–1000), Adaptation (0–10). **Clouds**:
Cloud Soft. (0–0.5), Cloud Occl. (0–1), Gap Erode (0–0.99). **Depth Fade**: Start Depth (0–100),
Depth Fade (0–100), Depth Power (0.05–8). **Headline knob:** Ray Alpha (strength).

### 2e. Debug Views section (`PixelRenderer`, tab "Debug Views")
One dropdown **View** (`debug_view`): Final (0) / Normals (1) / Depth (2) / Pixel Size (3) /
Outline Ids (4) / Raw Color (5) / Outline Cause (6) / Shafts (7). Shows the renderer's
intermediate buffers. Outline Cause colour key (from tooltip): red = silhouette, blue = id edge,
yellow = crease, green = suppressed fold/ledge.

### 2f. Grid / Fog section (terrain, `MarchingSquaresTerrain`)
Reached via the Grid/Fog toolbar icon or by selecting the terrain. Overlays that live on the
terrain: cell grid lines (editor/in-game), fog of war, and terrain debug views. Fields:
Game Grid (bool), Editor Grid (bool), Grid Color, Grid Width (0.0055–2), Grid Fade (0–2000),
Chunk Lines (bool), Game Fog (bool), Editor Fog (bool), Fog Fade s (0–5), Fog Color,
Fog Dither (`fog_color_alt`), Hidden Dim (0–1), Hatch (0–1), Softness (0–0.5),
Debug View enum (Off / Slope / Height Contours / Slot Ids / Chunk Borders / Cell Ids / Cell Data).

---

## 3. Pixel Object (select a `PixelObject`, tab "Pixel Object")
Per-object override of the pixel look on a prop (the scene's `BuilderRuin` is one). `-1` /
"Inherit" means "use the parent PixelObject, else the renderer's Object Defaults".

Fields (from `render/object.rs` / sections.rs): Pixel Size (−1/Inherit…16, or greater),
Outline (Inherit/Off/On), Outline Id (−1…253), Palette (Inherit/Off/On),
Dither (Inherit/Off/On), Own Color (`supersede_contour_tint`, bool), Outline Col.
(`contour_tint`), Texel Snap (bool), Convert (`convert_materials`, bool), Bands (1–8),
Shade Floor (0–1), Rim (0–1). Also (Inspector only): `alpha_scissor_threshold`,
`profiles` (Array of **PixelObjectProfile**) + `profile_targets`.

**PixelObjectProfile** resource (`render/profile.rs`): a reusable override you drop into a
PixelObject's `profiles`. Props: active (bool, default true), pixel_size (−1…16),
outline (−1…1), outline_id (−2…253), palette (−1…1), dither (−1…1),
supersede_contour_tint (bool), contour_tint (Color default very-dark navy). `has_overrides()`
decides if it changes anything.

**PixelOutlineColors** resource (`render/outline_colors.rs`): colours per outline id.
Props: `silhouette_colors` (PackedColorArray), `edge_colors` (PackedColorArray),
`modulate_rows` (bool). Defaults: all neutral except id for Hover (yellow 1,0.93,0.35) and
Select (orange 1,0.55,0.12). Assigned on the renderer; Inspector-only.

---

## 4. PixelPalette + Bake LUT
`PixelPalette` is a Resource (`render/palette.rs`) assigned to `PixelRenderer.palette`.
Properties (Inspector): `tints` (PackedColorArray — the palette colours; defaults to a built-in
set), `source_image` (Texture2D — read colours from an image instead), `quantize_mode`
(Perceptual / Channel / Hue / HueWeighted / Lab / Value), `weights` (Vector3),
`brightness_curve` (Curve), `lut_image` (import a 1024×32 addon LUT or a 1024×1024 colour-cube
chart), `output_path` (*.png). It has a built-in **"Bake LUT to PNG"** inspector button
(`export_tool_button`), and the editor adds a **Tools menu item**:
`Project → Tools → Bake PixelPalette LUT to PNG` (handled by the pixelizer plugin's
`on_bake_lut`). Both write the baked lookup texture to `output_path` (default
`res://palette_lut.png`) and rescan the filesystem. If no PixelPalette is selected and the
selected renderer has one, it bakes that; otherwise a warning is printed.

---

## 5. Pixel World panel (right side) + Lighting Presets + Wire refs
The right panel (`PixelWorldPanel` scene, filled by `pixel_ui/plugin.rs`):
- **Title** "Pixel World" (a button; tooltip explains Select-to-cycle).
- **Effect rows** — one per family from `ADDABLE`: Renderer, Sky & Clouds, Day / Night,
  Weather, Water, Fireflies, Particle Field, Birds, Smoke & Fire, Light Volume, Sun Shafts,
  Pixel Object, Animation Stepper, Diagnostics, Erosion, Wild Flowers, Grass Fire. Each row:
  an icon+name button (click to select / cycle if present; dimmed if absent) and a right
  button **Select** (present) or **+ Add** (absent). "+ Add" inserts the node with its sun /
  environment / camera refs wired, **undoable**; for terrain-child families (Erosion, Wild
  Flowers, Grass Fire) it needs a terrain and adds under it. A count "(2)" shows when several.
- **Lighting Preset** header + a grid of buttons: **Dawn, Noon, Dusk, Night, Storm, Snow**
  (from `pixel_ui/presets.rs`). Each applies a bundle of values to the scene's Day/Night, Sky,
  Weather and the renderer's god rays (undoable). (Effects themselves are page 08; the preset
  button is here because the right panel owns it.)
- **Wire Sun / Env Refs** button — fills empty sun/environment references of Sky, Day/Night,
  Weather and Sun Shafts with the selected light/environment or the ones other effects use
  (undoable). Prints "Pixel World: wired N reference(s)."
- **Preview hint** line — tells you whether Pixel Preview is on and what it does.

---

## 6. Project-Settings feature switches
`Project Settings → marching_squares_terrain/features/<name>` — booleans that compile each
feature's nodes in or out. Reached from **Pixel World menu → Project → Features** (a check-list)
or Project Settings directly. Turning one OFF makes that feature's nodes **build and draw
nothing** (their settings are kept). List (`features.rs`, default): Clouds (off), God Rays
(off), **Outlines (on)**, **Outline Ids (on)**, Water (off), Weather (off), Birds (off),
Fireflies (off), Smoke (off), Fire (off), Particle Fields (off), Light Volumes (off),
Sun Shafts (off), **Grass (on)**. Each item tooltip names the cost and that it is saved in
Project Settings. (Note: when you "+ Add" a family whose feature is off, the plugin turns that
feature ON for you and prints a note.)

---

## Walkthroughs (performed in the editor)

### A. Turn the pixel look on and off
1. Open `res://tutorial/03_pixel_look.tscn`. It already looks pixelated (Pixel Preview defaults
   on here). Find the 3D-view menu bar: **Pixel Preview · Animate · Pixel World · Roblox UI**
   (`01-menu-bar.png`).
2. Click **Pixel Preview** once → the viewport switches to the raw smooth scene with the editor
   grid + coloured origin axes (`02-preview-off.png`). Click again → back to the pixel look with
   the grid hidden (`02-preview-on.png`).

### B. Open the renderer workspace and read a setting
1. In the Scene tree click **PixelRenderer**. The left icon toolbar, the bottom strip and the
   right **Pixel World** panel appear (`03-workspace-overview.png`).
2. The bottom strip opens on **Renderer** with tabs across the top (`10-strip-renderer.png`).
   Click the tabs to visit **Outlines** (`11`), **Color / Palette** (`12`), **Rays** (God Rays,
   `13`), **Debug Views** (`14`).

### C. Chunkier pixels
1. On the Renderer tab, drag **Pixel Size** from 1 to 12. The art gets blockier
   (series `20-pixelsize-1..12.png`).

### D. Outlines
1. Outlines tab → uncheck/recheck **Outlines** (`21-outlines-off/on.png`).
2. Drag **Darken** 0 → 1: the black edge thickens/darkens (series `22-darken-*.png`).

### E. Palette
1. Color / Palette tab → uncheck **Use Palette**: the muted palette gives way to the raw blue
   sky + smooth greens (`23-palette-off/on.png`).
2. Drag **Mix** 0 → 1 to fade into the palette (series `24-mix-*.png`).

### F. God rays
1. Rays tab → check **God Rays**, raise **Ray Alpha**. Teal light shafts fall from the sun
   (`26-godrays-off/on.png`, series `27-rayalpha-*.png`).

### G. Debug buffers
1. Debug Views tab → **View** dropdown → Normals / Depth / Pixel Size / Outline Ids /
   Outline Cause (`25-debug-*.png`). Outline Cause: red = silhouette, blue = id edge,
   yellow = crease.

### H. A prop's own outline (Pixel Object)
1. Select **BuilderRuin** (the PixelObject; NOT its child Ruin, which opens the Builder).
   The **Pixel Object** strip appears (`15-strip-pixelobject.png`).
2. Set **Outline** = On (and renderer **Obj. Outline** on) to draw the prop's silhouette
   (`16-object-outline-off/on.png`).

### I. Pixel World panel: add, preset, wire
1. The panel lists every pixel family (`30-pixelworld-panel-top.png`). Present ones show
   **Select**; absent ones show **+ Add** (dimmed).
2. Scroll down → **Lighting Preset** row (Dawn/Noon/Dusk/Night/Storm/Snow,
   `31-pixelworld-panel-presets.png`) and **Wire Sun / Env Refs** + the preview hint
   (`32-pixelworld-wire-hint.png`).
3. Click **Dusk** → the whole scene recolours to a warm dusk (`33-preset-before/after.png`);
   the History shows one action "Apply Dusk preset"; Ctrl+Z reverts it.

### J. Animate
1. Click **Animate** in the menu bar → cloud shadows drift across the ground, grass sways
   (`40-animate.mp4`).

### K. Palette LUT bake
1. Select PixelRenderer → Inspector → expand **Color → Palette** → the PixelPalette resource
   with **Bake LUT to PNG** (`52-pixelpalette-bake-button.png`). Click it (or use
   `Project → Tools → Bake PixelPalette LUT to PNG`). It writes a 1024×32 LUT
   (`53-baked-lut.png`) to the palette's Output Path and logs
   "PixelPalette: baked 1024x32 LUT -> res://palette_lut.png".

### L. Feature switches
1. `Project → Project Settings`, filter "features", open **Marching Squares Terrain → Features**
   (`50-feature-switches.png`). Each checkbox compiles a feature in/out.

### M. Undo a section edit
1. In any section, change a field (e.g. uncheck **Outlines**) → History shows
   "Set Outlines Outlines" → **Ctrl+Z** restores it (`60-undo-edit.mp4`). A slider drag is one
   undo step (grab→release).

## Media manifest

| File | Kind | Caption / shows |
|---|---|---|
| 01-menu-bar.png | shot | The 3D-view pixel menu bar: Pixel Preview, Animate, Pixel World, Roblox UI. |
| 02-preview-off.png / 02-preview-on.png | before/after | Pixel Preview off (raw scene + grid + axes) vs on (pixel look, grid hidden). |
| 03-workspace-overview.png | shot (full, boxed) | The whole pixel workspace: 1 menu bar, 2 left toolbar, 3 bottom strip, 4 Pixel World panel. |
| 10-strip-renderer.png | shot | Renderer strip + section tabs (Enabled, Mode, Pixel Size, …). |
| 11-strip-outlines.png | shot | Outlines strip (Depth/Normal Thr., Darken, Highlight, …). |
| 12-strip-color-palette.png | shot | Color / Palette strip (Use Palette, Mix, Dither, Saturation…). |
| 13-strip-godrays.png | shot | God Rays "Rays" page strip. |
| 14-strip-debug.png | shot | Debug Views strip (View dropdown). |
| 15-strip-pixelobject.png | shot | Pixel Object strip (per-prop overrides). |
| 16-object-outline-off.png / -on.png | before/after | Prop (Ruin) outline off vs on. |
| 17-strip-gridfog.png | shot | Grid / Fog strip (Grid / Fog of War tabs). |
| 20-pixelsize-1/2/4/8/12.png | series "Pixel Size" | Macro-pixel size 1→12, same crop. |
| 21-outlines-off.png / -on.png | before/after | Renderer outlines off vs on (hill silhouette). |
| 22-darken-0.0/0.3/0.6/1.0.png | series "Darken" | Outline darken 0→1. |
| 23-palette-off.png / -on.png | before/after | Use Palette off (blue sky, smooth) vs on (muted palette). |
| 24-mix-0.0/0.33/0.66/1.0.png | series "Mix" | Palette mix (blend) 0→1. |
| 25-debug-normals/depth/pixelsize/outlineids/outlinecause.png | series/shots | Debug buffer views. |
| 26-godrays-off.png / -on.png | before/after | God rays off vs on (teal shafts). |
| 27-rayalpha-0.5/1.0/2.0/4.0.png | series "Ray Alpha" | God-ray alpha 0.5→4. |
| 30-pixelworld-panel-top.png | shot | Panel rows (Select vs + Add). |
| 31-pixelworld-panel-presets.png | shot | Panel scrolled: more rows + Lighting Preset buttons. |
| 32-pixelworld-wire-hint.png | shot | Wire Sun / Env Refs button + preview hint. |
| 33-preset-before.png / -after.png | before/after | Dusk lighting preset applied. |
| 40-animate.mp4 (+ .png poster) | clip | Animate on: cloud shadows drift over the terrain. |
| 50-feature-switches.png | shot | Project Settings → Features checkbox list. |
| 52-pixelpalette-bake-button.png | shot | PixelPalette resource in the Inspector + Bake LUT to PNG button. |
| 53-baked-lut.png | shot | The baked 1024×32 palette LUT PNG. |
| 60-undo-edit.mp4 (+ .png poster) | clip | Toggle Outlines off then Ctrl+Z restores it (one undo step). |
| 70-page08-sky.png | shot (overview) | Sky & Clouds strip — SEE PAGE 08. |
| 71-page08-daynight.png | shot (overview) | Day / Night strip — SEE PAGE 08. |

### Hotspot tour — `03-workspace-overview.png` (positions are % of the full 1600×900 image)
- 1 Menu bar — ~31% / 15% — "Pixel Preview / Animate / Pixel World / Roblox UI".
- 2 Left toolbar — ~19% / 44% — "one icon per pixel-node family; dim = not in the scene".
- 3 Bottom strip — ~44% / 60% — "the selected family's settings, in tabs".
- 4 Pixel World panel — ~74% / 44% — "add effects, apply a lighting preset, wire refs".

### Hotspot tour — `30-pixelworld-panel-top.png` (% of that crop)
- Title (~18%/6%): "Pixel World — lists every pixel family."
- A **Select** button (~85%/14%): "jump to that node (click again to cycle duplicates)."
- A **+ Add** button (~85%/39%): "insert that effect with its sun/env/camera refs wired (undoable)."
- A dim row (~30%/39%): "greyed = not in the scene yet."

## Roblox bridges
- **Pixel Preview** ≈ a Studio *View* style toggle: it changes how the viewport is drawn, not the
  objects. Like turning on a graphics preset.
- **Left toolbar + bottom strip** ≈ selecting a Part and using the **Properties** panel — except
  here one icon = a whole effect node, and the strip shows only its most useful properties.
- **Pixel World panel** ≈ the **Explorer + Insert Object**, but filtered to pixel effects, with
  one-click "+ Add" that also wires up the sun/camera for you (Roblox has no equivalent of the
  auto-wiring).
- **Lighting Preset** buttons ≈ Studio **Lighting** presets / the **Atmosphere** + **Sky**
  combos — one click sets time of day, clouds and sun shafts together.
- **Feature switches** ≈ Studio's per-place toggles (e.g. turning off a heavy effect): here they
  compile whole effect families in or out for the project.
- **Undo** is the same **Ctrl+Z** habit; every strip/panel change is one history step.

## Caveats (verified; none block capture)
- Editor runs ~5–21 fps (llvmpipe); wait 1–2 s after clicks that rebuild the strip/panel.
- **Pixel Preview defaults ON** in `03_pixel_look.tscn` (`PixelRenderer.studio_prelook = true`).
  The first click turns it OFF. Tell the reader.
- **Animate drives the editor refresh.** With Animate OFF the editor only redraws on change, so
  effects hold still AND the Sky / Day-Night look can go stale after you change their settings or
  undo a lighting preset (the node re-evaluates on the clock tick). The underlying data is always
  correct (verified: after Ctrl+Z the properties were restored exactly); only the preview image
  lagged. Turn Animate on, or nudge a value, to refresh. This is a PAGE-08 (Sky/Day-Night) preview
  behaviour, not a renderer bug — my renderer sections (pixel size, outlines, palette, god rays,
  debug) all refreshed instantly.
- **Lighting presets set a persistent global.** Applying a preset (e.g. Dusk) changes global
  shader/sun state that survived a scene reload within the same editor session (the sky stayed
  dusk-orange even though every node property was back to default). It clears on a new editor
  session. Again: data correct, preview sticky. Capture clean "look" shots BEFORE applying any
  preset, or restart the editor.
- **Editor grid overlay** (Grid / Fog → Editor Grid) did not visibly render in the pixel preview
  during capture even with a bright colour and large width (only ~hundreds of pixels changed).
  The toggle and strip work; the visible grid may need the game runtime. No before/after shipped
  for it — strip shot only (`17`).
- **Pixel World menu-bar dropdown** could not be screenshot: the harness's synthetic click opens
  then immediately closes the MenuButton popup. Its contents are documented from source in §1/§5
  (Add families · Lighting Preset · Wire Sun / Env Refs · Project → Features). The right-side
  **panel** exposes the same actions and IS captured. (The top-bar *Project* menu opens fine.)
- **Slider drags overshoot under the test harness.** A synthetic drag of an `EditorSpinSlider`
  jumped Pixel Size to 46637 and Darken to 342 (both out of nominal range) — an XTEST artifact of
  the fast synthetic drag, not an addon bug; a human dragging slowly gets sensible values, and the
  change still undoes as ONE step. Knob-series shots were made with exact `set-prop` values, not
  drags.
- One `editor.log` ERROR — `MarchingSquaresTerrain::set_grid_color() … cannot convert from ARRAY
  to COLOR` — was caused by MY `set-prop grid_color '[...]'` (passing a colour as a JSON array);
  the real UI colour picker is unaffected. Not an addon defect.
- Selecting the **Ruin** (PixelBuilderMesh, child of BuilderRuin) opens the **Builder** UI
  (page 10). Select **BuilderRuin** (the PixelObject parent) for the Pixel Object section.
- Default field values worth quoting (verified in-editor): Pixel Size 2 · Depth Thr. 0.35 ·
  Normal Thr. 0.5 · Darken 0.55 · Highlight 0.22 · Fold Limit 4.0 · Mix 1.0 · Spread 0.03 ·
  Matrix Bayer 4 · Saturation 1.05 · Contrast 1.0 · Brightness 0.0 · Ray Alpha 1.0 ·
  Max Sheets 26 · Ray Depth 305 · Reach 283.4.

## Scenes created
- None. All capture used the stock `res://tutorial/03_pixel_look.tscn`; no scene was saved
  (every edit was reverted or left only in the disposable session copy).

## Re-capture (review pass, 2026-11-24)
- **42-pixel-preview-toggle** re-shot: the clip now starts with Pixel Preview **ON** whatever
  the editor's button state is at session start. The 3D-view button is clicked at a fixed point
  (353,137) rather than by label (a second `click-text "Pixel Preview"` also matched the preview
  hint label and silently did nothing). Poster is the ON state at 6.5 s.
- **47-wire-refs** re-shot: same panel crop, daytime pixel viewport sliver, tooltip then click;
  verified the click lands by the preceding `ui --root all --filter Refs` expect.
- Recipes for both items live in `docs/guide/recipes/07-pixelizer.json` and replay clean on
  `:113`/`8913`.
- **16-object-outline-off/on** keep the old, very subtle pair (the outline difference is small at
  this crop). Optional re-shoot, not done in this pass.

## Recipes (backfill, 2026-11-24)
`docs/guide/recipes/07-pixelizer.json` now has an item for every media file on the page
(22 items; grouped series where a numbered set shares one setup):
menu bar + preview off/on + workspace, the Renderer/Outlines/Color/Rays/Debug/Pixel Object
strips, Grid/Fog, the Pixel Size and Darken/Palette/Mix series, the Debug Views series,
God Rays off/on + Ray Alpha series, the Pixel World panel + Dusk pair, the feature/LUT shot,
the page-08 strips, the six Quantize Modes, and the nine clips (Animate, Pixel Size drag,
Outlines darken, Palette mix, God Ray alpha, Lighting preset, Wire refs, Debug cycle, Undo).
Tested against the real media on `:113`/`8913`; strips use the 1600x1040 layout crops.

**Known gaps in this backfill:**
- `50-feature-switches.png` — the Project Settings dialog is not reproducible from stable
  harness controls (menu item coordinates / filtered list do not settle); its item was removed
  from that recipe item's outputs.
- `81-palette-channel … 85-palette-value.png` — the five non-default Quantize Modes live on the
  `PixelPalette` **subresource** in the Inspector; clicking its `EditorPropertyEnum` opens the
  popup but the harness key sequence does not change the mode (frames stay identical). Only
  `80-palette-perceptual.png` is reproducible.

**Update (palette modes closed):** the `81–85` gap above is fixed. The Quantize Mode lives on the
`PixelPalette` subresource, whose enum popup ignores synthetic keys, but the harness can write it
directly: `set-prop palette:quantize_mode <0-5> --class PixelRenderer`. `80-palette-perceptual`
and `81…85` are now six separate still items in `docs/guide/recipes/07-pixelizer.json` (one mode
each, fresh session, `select --path PixelRenderer` then the `set-prop`), crop `339,220,705,363` —
the same viewport framing as before, shifted up 20 px so the Renderer strip stays out of frame.
All six ran green on `:117`/`8917` and show the distinct palettes (perceptual = mauve sky, channel
= cyan dither, hue/hue-weighted = cyan-green dither, lab = purple/green, value = orange/pink).

## Re-shoot (recipes)

Staged with `qa/recapture.py --only 16-object-outline-pair --out-dir` on `:125`/`8925`, verified,
then copied over `docs/guide/media/07-pixelizer/`.

| File(s) | Result |
|---|---|
| `16-object-outline-off.png`, `16-object-outline-on.png` | **Re-shot.** Same camera for both. Focus the ruin mesh (`BuilderRuin/Ruin` + `F`), select `BuilderRuin`, Esc to clear the yellow builder selection box, then frame the wall from ~20 m (`09-frame.py`, dist 20, az 28.6, elev 10). Crop 600,330,300,150 around the tower/roof edge against the sky, upscaled 2× NEAREST (600×300). Outline Off vs On is now obvious: ON has a hard dark line along the whole silhouette/roofline. Alt text and the recipe item updated. |
