# Capture notes — page `04-terrain-painting` (Area C03 — Paint + Tint)

Source of truth for the writer. Captured in the real Godot 4.7 editor, session `docs-paint`
(display `:73`, port `8873`), scenes `res://tutorial/02_island.tscn` (square grid, primary) and
`res://demo/hex_island.tscn` (hex grid, secondary). Cross-checked against the Rust source:
`addons/MarchingSquaresTerrain/rust/crates/mst/src/editor/tools.rs`,
`.../editor/terrain_ui.rs`, `.../editor/save_preset_dialog.rs`,
`.../terrain/textures.rs`.

Media live in `docs/guide/media/04-terrain-painting/`.

---

## 0. Orientation — where the Paint tool lives

The terrain tools only appear while a **MarchingSquaresTerrain** node is selected in the Scene
dock. Selecting it shows three surfaces:
- **Left icon toolbar** inside the 3D view (Brush, Level, Smooth, Bridge, Foliage, **Paint**,
  Inspect, Chunks, Settings, then Stamp / Gradient / Select / Tint under separators). Each tool
  has a number shortcut; **Paint is tool `6`**. (Tint is in the lower "extra tools" group and has
  NO number shortcut — you click its icon.)
- **Bottom attribute strip** — the settings for the active tool. For Paint its title is
  "Vertex Painting".
- **Right "Textures" panel** — appears only for Paint and Foliage. For Paint it lists the 16
  texture slots with editable fields; this panel sits between the 3D view and the Inspector.

The tool is internally called **Vertex Paint** / "Vertex Painting"; the toolbar button label is
**Paint**. The tool's own tooltip (hover the Paint icon): *"Vertex Paint Tool [6] — Used to paint
textures onto the terrain. There are 16 available textures. Every texture can have grass ('Has
grass' in the right panel). 16 is a void material making terrain invisible. Texture presets can be
used to quickly swap between texture palettes; they can be exported via the right-hand panel at the
bottom. 'Quick Paints' are a way to quickly set textures while modelling the terrain.
[SHORTCUTS] Ctrl+LMB: Pick the material under the cursor."*

In-view hint line (shown at the bottom of the 3D view while Paint is active):
**"LMB-drag: paint the Material · Ctrl+LMB: pick the material under the cursor · Shift+wheel: size"**

### Wheel shortcuts over the 3D view while Paint is active
- **Shift + mouse-wheel** = change brush **Size**.
- **Ctrl + mouse-wheel** = zoom the camera (Paint has no Strength, so Ctrl-wheel is free and just
  zooms; this differs from the height tools where Ctrl-wheel is Strength).
- **Plain mouse-wheel** = zoom the camera (normal Godot navigation).
- (Alt+wheel = Rotation on brush tools, but Paint's brush rotation matters only for non-round
  brushes.)

---

## 1. Feature inventory — Paint (Vertex Painting)

### 1a. Bottom strip "Vertex Painting" (left → right, exact order from source `settings_for`)
All ranges/defaults verified in `tools.rs` (`portray`, `State::default`).

| Control | Type | Range / options | Default | What it does | Tooltip |
|---|---|---|---|---|---|
| Brush Type | dropdown | Round, Square, Hexagon, Hexagon30, Triangle, Triangle180 | Round | Shape of the paint brush footprint | "Brush shape." |
| Size | slider | 1 – 50 (world units, diameter) | 15.0 | Brush diameter in world units | "Brush diameter in world units (Shift+wheel)." |
| Aspect | slider | 0.20 – 5.0 | 1.0 (shows ~1.009) | Brush Z radius / X radius (ellipse/rectangle brushes) | "Brush Z radius / X radius (ellipse / rectangle brushes)." |
| Rotation | spinbox | -180 – 180 (deg) | 0 | Brush rotation (matters for non-round brushes) | "Brush rotation in degrees (Alt+wheel)." |
| Material | dropdown | the 16 slots by name (e.g. "1. Grass") | 1. Grass (slot 0) | Which texture slot you paint | "Texture slot to paint." |
| Texture Name | text field | free text (placeholder "New name here…") | empty | Rename the selected material; Enter applies | "Rename the selected material (Enter to apply)." |
| Texture Preset | dropdown | None + every `.tres` in texture_presets/ | None | Apply a whole texture palette to this terrain | "Apply a texture preset to this terrain." |
| Paint Walls | checkbox | on/off | off | Paint the wall/cliff texture instead of the floor | "Paint wall textures instead of the floor." |
| Slope Min | spinbox | 0 – 90 (deg) | 0.0 | Only paint terrain at least this steep | "Only affect terrain at least this steep (degrees)." |
| Slope Max | spinbox | 0 – 90 (deg) | 90.0 | Only paint terrain at most this steep | "Only affect terrain at most this steep (degrees)." |

NOTE: **Paint has no Strength control** (unlike Smooth/Raise). Confirmed in `settings_for`.

### 1b. Right "Textures" panel — per-slot fields (16 slots)
Panel title "Textures" with tooltip *"Texture slots of this terrain's texture set. Click a slot to
paint with it (same as the Material dropdown)."* Each slot (except slot 16 "void") shows:

| Field | Type | What it does | Tooltip |
|---|---|---|---|
| Preview swatch | 36×36 image | Live preview of the slot (texture or procedural pattern, tinted by Color) | "Preview of this slot (texture or pattern, tinted)." |
| "Texture N" button | toggle | Select this slot to paint (same as Material dropdown). Pressed = active slot | "Paint with this texture (Material)." |
| name field | LineEdit | The slot's display name (shown in the Material dropdown) | "Texture name (shown in the Material dropdown)." |
| Texture | resource picker (Texture2D) | The slot's image. **This is how you swap in your own PNG.** Empty = use the procedural Pattern | "Ground texture of this slot (white-ish pixel art, tinted by Color). Empty = the procedural Pattern." |
| Pattern | dropdown | 13 built-in patterns: Grass, Meadow, Forest, Sand, Dirt, Cliff, Dark Rock, Snow, Gravel, Clay, Moss, Path, Plain (slot 16/void hides Pattern) | "Ground pattern: picks the bundled ground texture of this pattern (textures/ground/). Without a texture the pattern is generated procedurally." |
| Scale | slider 0.1–40 | Texture / pattern tiling scale | "Texture / pattern scale." |
| Color | color button (no alpha) | Multiplies the texture/pattern (the ground tint) | "Ground color (multiplies the texture / pattern)." |
| Grass | checkbox + color | Grow grass on this texture + grass color | "Grow grass on this texture." / "Grass color on this texture." |
| Sprite | resource picker (Texture2D) | Grass sprite for this texture; empty = terrain's default grass | "Grass sprite on this texture: white leaves on alpha, colored by Grass… Empty = the terrain's grass texture." |

Below the 16 slots there is an **Auto Rules** section (procedural auto-texturing rules; cross-link
to Area 3/Settings — it is advanced), then the **"Export Texture Preset"** button (tooltip:
*"Save this terrain's texture set as a preset in res://addons/MarchingSquaresTerrain/texture_presets/."*).

The default 16 slot names (from `textures.rs` DEFAULTS): 1 Grass, 2 Meadow, 3 Forest Floor,
4 Sand, 5 Dirt, 6 Cliff, 7 Dark Rock, 8 Snow, 9 Gravel, 10 Clay, 11 Moss, 12 Path,
13 Stone Tiles, 14 Ash, 15 Ice, 16 Void (the invisible material). Slots 1,2,3,11 have grass on
by default.

### 1c. Ctrl+LMB — eyedropper / pick
Ctrl+LMB over the terrain picks the material currently under the cursor and makes it the active
slot (the Material dropdown + the slot button update). Same gesture used by Roblox's eyedropper-
style sampling.

---

## 2. Feature inventory — Tint tool
Tint is in the lower "extra tools" group (no number shortcut; click the tint icon). Its strip has
NO right panel. Tool tooltip: *"Tint Tool — Paints a color over whole cells (territory, biome
hints, paths, decals). Tints are saved with the terrain data and drawn by the terrain shader in
the editor and in game, under fog of war and under runtime set_cell_tint() tints. [SHORTCUTS]
LMB: paint the Tint color at Opacity. RMB: erase tints. Shift+MWU/MWD: brush size."*
In-view hint: **"LMB-drag: paint the tint · RMB: erase · Shift+wheel: size"**.

Strip controls (order from `settings_for(Tool::Tint)`):

| Control | Type | Range | Default | What it does |
|---|---|---|---|---|
| Brush Type | dropdown | Round…Triangle180 | Round | Brush shape |
| Size | slider | 1 – 50 | 15.0 | Brush diameter |
| Aspect | slider | 0.20 – 5.0 | 1.0 | Ellipse aspect |
| Rotation | spinbox | -180 – 180 | 0 | Brush rotation |
| Tint | dropdown | Red, Orange, Yellow, Green, Teal, Blue, Purple, Pink, White, Black | Red | The color painted by LMB (RMB erases) |
| Opacity | slider | 0.05 – 1.0 | 0.5 | Strength / alpha of the painted tint |
| Grid Aligned | checkbox | on/off | off | Snap brush to whole cells |
| Grid Size | slider | 0 – 20 | 1 | Size of the grid-aligned stamp in cells |

Tint colors (RGB from `TINT_COLORS` in tools.rs): Red, Orange, Yellow, Green, Teal, Blue, Purple,
Pink, White, Black. RMB erases the tint (sets it back to none).

---

## 3. Inspector — texture-related terrain properties
(Selected terrain, Inspector dock, right side.) Relevant to painting:
- **Tex Assign** — the terrain's assigned `MarchingSquaresTexturePreset` resource (the texture set
  the 16 slots live in). Internal: `texture_set_ref()`. Changing the Texture Preset dropdown in the
  strip replaces this.
- **Grid Type** (Square / Hex / Triangle), **Chunk Size** (32), **Cell Size** (x/y 2.0) — these set
  the grid the paint lands on.
- **Look** category → **Terrain** and **Grass** sub-groups: terrain material / blend-related
  properties (the terrain shader that draws the painted textures and tints).
- Grass fields (Grass Pixel Size, Grass Outline, Ledge Cutoff, Min Height) affect how grass (from
  slots with Grass ticked) renders.
- **Blend Mode** lives on the **Settings** tool (Area 3), not the Paint strip, but it changes how
  the painted textures blend at their borders — note for the writer as a cross-link.

(TODO: capture the Inspector Look/Terrain/Grass expanded; record exact field list.)

---

## 4. Texture presets
- Bundled preset in the repo: `texture_presets/mst_classic.tres`.
- The **Texture Preset** dropdown lists **None** + every `.tres` under
  `res://addons/MarchingSquaresTerrain/texture_presets/`. Choosing one rebuilds all 16 slots and
  repaints the terrain's texture array; choosing **None** clears the assignment back to default.
- **Export Texture Preset** (button at the bottom of the right panel) opens a dialog:
  "Preset name (saved to res://addons/MarchingSquaresTerrain/texture_presets/):" + a LineEdit
  (prefilled "new_texture_preset" or the current preset name) + OK/Cancel. On a name clash a
  second confirmation "<path> already exists. Overwrite it?" appears. The file is saved as
  `snake_case(name).tres`. (In this capture session it writes to the *session copy* only.)

---

## 5. Undo
Every paint stroke is one undo step named **"terrain vertex paint"**; tint strokes are
**"terrain tint paint"** (from `undo_name` in tools.rs). Ctrl+Z reverts one stroke; Ctrl+Shift+Z
redoes. (To verify with digests during capture.)

---

## 6. Roblox bridges (for the writer)
- Roblox Studio's **Terrain Editor → Paint** also paints materials onto terrain with a brush; here
  the equivalent is the Paint tool, but you paint one of **16 texture slots** instead of Roblox's
  fixed material list. The slots are fully editable (swap the image, tint, pattern, grass).
- Roblox's material **eyedropper** ≈ **Ctrl+LMB** pick here.
- Roblox has no per-slot "grass" or procedural pattern — call that out.
- Roblox terrain "Grassy", "Rock", etc. map to the default slot names (Grass, Cliff, Dark Rock…).
- Tint has no Roblox equivalent (Roblox colors are per-material); explain it as a paint-over color
  layer for biome/territory hints.

---

## 3b. Inspector detail (verified, media 24-inspector-tex-assign.png)
Selected terrain → Inspector shows, top to bottom: **Contents** (the texture-set resource,
e.g. `hex_…` / `02_i…`), **Grid Type** (Square / Hex Rings / …), **Cell Size** (x/y, 2.0),
**Tex Assign** (the `MarchingSquaresTexturePreset` — on hex_island it reads "Textures"; this is
the live texture set the 16 paint slots edit), then collapsible groups **Grid**, **Look**
(→ **Terrain**, **Grass** sub-groups — the terrain shader that draws the painted textures & tints),
**Turf Stuff**, **Grass Pixel Size**, **Grass Outline**, **Ledge Cutoff**, **Min Height**,
**Foliage**, **Overlay**, **Advanced**, **Tools** (Export Path, Export GLB, Rebuild Meshes).
Changing the strip's **Texture Preset** replaces **Tex Assign**. "(N changes)" tags mean the
group differs from the resource's defaults.

---

## 7. Media manifest

| File | Kind | Shows | Notes |
|---|---|---|---|
| 01-paint-tool-location.png | shot (full+box) | Whole editor, red box on the **Paint** tool (6) | Orientation: toolbar, strip, Textures panel, Inspector all visible |
| 02-paint-strip.png | shot | The "Vertex Painting" bottom strip | All 10 paint controls labelled |
| 03-textures-panel.png | shot | Right Textures panel, slot 1 (Grass) | preview, Texture, Pattern, Scale, Color, Grass, Sprite |
| 04-toolbar-tools.png | shot | Left tool column Brush→Select | tool icons + number keys |
| 05-material-dropdown.png | shot | Material dropdown open, all 16 slot names | the palette list |
| 06-paint-stroke.mp4 (+before/after/poster) | clip + before/after | Painting a Sand stroke on grass | hero paint demo; before=green grass, after=tan sand |
| 07-ctrl-pick.mp4 (+poster) | clip | **Ctrl+LMB** eyedropper; Material field updates (Meadow→Sand) | strip visible so the pick is readable |
| 08-texture-preset-dropdown.png | shot | Texture Preset dropdown: None, MST Classic | (exported presets also appear here) |
| 09-preset-apply.mp4 (+before/after/poster) | clip + before/after | Applying **MST Classic**; whole palette shifts (brighter→darker green) | also a full-strip reference |
| 10-slot-detail-cliff.png | shot | Slot 6 (Cliff) detail — a **non-grass** slot | Grass unticked, Sprite <empty> |
| 11-export-preset-dialog.png | shot | **Save Preset** dialog (name field + path + OK) | opened by "Export Texture Preset" |
| 12-size-change.mp4 (+poster) | clip | **Shift+wheel** changing brush Size (strip value + footprint) | |
| 13-size-series-1..4.png | series (Size) | Brush footprint at 4 growing sizes | values ≈ 1 → ~45 (min → large); same framing |
| 14-brush-type-dropdown.png | shot | Brush Type dropdown: Round, Square, Hexagon, Hexagon30, Triangle, Triangle180 | |
| 16-undo.mp4 (+poster) | clip | Paint a stroke then **Ctrl+Z** — it reverts | digest-verified to revert exactly |
| 17-tint-strip.png | shot | The **Tint** tool strip | Brush Type, Size, Aspect, Rotation, Tint (Red), Opacity, Grid Aligned, Grid Size |
| 18-tint-paint.mp4 (+before/after/poster) | clip + before/after | Painting a vivid **red tint** over terrain | before=green, after=red wash |
| 19-tint-erase.mp4 (+poster) | clip | **RMB** erasing the red tint | red wash removed |
| 20-paint-walls.mp4 (+before/after/poster) | clip + before/after | **Paint Walls** on: a brown cliff face turns white (Snow) | angled view; floor on top stays green |
| 21-slope-limited.mp4 (+before/after/poster) | clip + before/after | **Slope Min = 50** — only steep cliff/beach cells take the paint | flat grass spared |
| 22-swap-texture-dialog.png | shot | "Select Texture2D" quick-load dialog (repo PNG thumbnails) | step in the swap-your-own-image flow |
| 22-swap-texture-slot.png | shot | Slot 10 after its Texture was swapped to a repo PNG | preview + picker show the new image |
| 23-swap-texture-paint.mp4 (+poster) | clip | Painting with the **swapped custom texture** (slot 10) | colourful patch = your own image on the ground |
| 24-inspector-tex-assign.png | shot | Terrain Inspector: Tex Assign, Look/Terrain/Grass | |
| 25-slope-series-1..3.png | series (Slope Min) | Same stroke at Slope Min **0 / 45 / 80** | 0 = solid band, 45 = slopes only, 80 = almost nothing |

**Scrubber series for the writer:**
- *Size*: `13-size-series-1.png`..`13-size-series-4.png` (brush footprint grows; Size ≈ 1, ~12, ~25, ~45).
- *Slope Min*: `25-slope-series-1.png` (0° — paints everywhere) / `-2.png` (45° — only slopes) /
  `-3.png` (80° — only near-vertical cliffs, so almost nothing on this beach).

**Before/after compares:** 06 (paint stroke), 09 (preset swap), 18 (tint), 20 (paint walls),
21 (slope-limited). Only two presets exist (MST Classic + any you export), so 09 is the preset
before/after.

---

## 8. Walkthroughs (performed in the editor)

### A. Paint your first patch (media 05, 06)
1. Select **MarchingSquaresTerrain** in the Scene dock. The 3D-view tool column appears.
2. Click **Paint** (or press `6`). The right **Textures** panel and the "Vertex Painting" strip appear.
3. In the strip, open **Material** and choose a slot (e.g. **4. Sand**). (Or click a slot's
   "Texture N" button in the right panel — same thing.)
4. Mouse over the terrain and **LMB-drag**. Cells under the round brush take the texture.
5. **Shift+wheel** to resize the brush; **Ctrl+LMB** to sample (eyedropper) the material already
   on a cell.
6. **Ctrl+Z** undoes the whole stroke (one step).

### B. Paint a cliff face (media 20)
1. Angle the camera so a cliff wall is visible (View menu → Top, then middle-drag to tilt; or any
   perspective view).
2. Tick **Paint Walls** in the strip.
3. Pick a Material (e.g. **8. Snow**) and LMB-drag across the cliff face — the **wall** texture
   changes; the flat floor on top is left alone. Untick Paint Walls to go back to painting floors.

### C. Rock only on steep ground (media 21, 25)
1. Set **Slope Min** to ~45 (e.g. for Cliff/Dark Rock).
2. Paint a wide stroke over mixed terrain — only cells steeper than 45° take the texture; flat
   grass stays. Raise Slope Min toward 90 to restrict to near-vertical cliffs only. (**Slope Max**
   does the opposite — keep painting off anything steeper than its value.)

### D. Apply / export a texture preset (media 08, 09, 11)
1. Strip → **Texture Preset** → choose **MST Classic**. All 16 slots + the terrain repaint.
   **None** is the first entry.
2. To save your current set: scroll the right panel to the bottom, click **Export Texture Preset**,
   type a name, **OK**. It writes `res://addons/MarchingSquaresTerrain/texture_presets/<name>.tres`
   and the new name then appears in the Texture Preset dropdown. (A name clash asks to overwrite.)

### E. Use your own image as a texture (media 22, 23)
1. In the right **Textures** panel, find a slot and click the little ▾ on its **Texture** field
   (an EditorResourcePicker).
2. Choose **Quick Load…** (searchable) or **Load…** (file browser) and pick a PNG from the project.
3. The slot's preview updates to your image. Select that slot as the Material and paint — the
   ground now shows your texture. (Tip: leave **Texture** empty to use the procedural **Pattern**
   instead; set **Color** to tint it and **Scale** to change tiling.)

### F. Tint a region (media 17, 18, 19)
1. In the tool column (below Select) click **Tint** (no number key). The strip shows Tint + Opacity.
   *(If you can't see the Tint button, the bottom Output panel is covering it — collapse Output,
   or use the "Collapse" button to shrink the toolbar.)*
2. Pick a **Tint** colour (Red…Black) and an **Opacity**, then **LMB-drag** to wash colour over the
   ground. **RMB** erases the tint. Tints sit on top of the textures and are saved with the terrain.

---

## 9. Caveats / things to tell the reader
- **The pink ring + highlighted cells under the cursor is the brush footprint preview**, not paint.
  It only shows while the mouse is over the 3D view; a large brush makes a large ring. (For a clean
  "after" screenshot, move the mouse off the viewport first.)
- **Painting registers in the undo history correctly** — digest-verified: a stroke changes the
  terrain digest and **Ctrl+Z restores it exactly** (action name "terrain vertex paint";
  tint = "terrain tint paint").
- **Top-down (Top Orthogonal) view shows the ground textures without grass** — grass sprites cull
  at that far, straight-down distance, which is actually handy for seeing exactly what you paint.
  In a normal angled/perspective view grass covers grass-enabled slots.
- The editor runs at ~3–10 fps (llvmpipe); use slow (2 s) drags for strokes.
- **Where you click must be an actual terrain cell.** On the sparse island scenes (02_island) a
  click in a gap/water prints "no sample" and paints nothing. hex_island is dense and easier.
- The **Tint** and **Grid** buttons sit at the very bottom of the tool column and can be hidden
  behind the Output panel until it is collapsed (editor layout quirk, not a bug).
- **Minor:** this is a texture/colour tool only — it never changes terrain height. **Slope Max**
  default 90 means "no upper limit" (paint everything up to vertical).
- Possible visual-refresh nit (low confidence, not reproduced cleanly): after an undo the grass
  overlay sometimes looked slow to redraw while the data had already reverted. The data/undo itself
  is correct (digest verified); flag only if the writer notices stale visuals.

## 10. Scenes created
None. All demos used the bundled scenes `res://demo/hex_island.tscn` (primary for painting/tint
action, in a **Top Orthogonal** view; an angled view for the cliff/Paint-Walls demo) and
`res://tutorial/02_island.tscn` (panel/strip/dropdown reference stills). No follow-along scene was
saved. One throwaway preset `guide_demo_preset.tres` was written into the **session copy's**
`addons/MarchingSquaresTerrain/texture_presets/` to verify the Export flow (session-only, not the
repo).

## 11. Capture method (for a resuming agent)
- Session `docs-paint`, display `:73`, port `8873`.
- Deterministic framing trick: **View menu → Top View** gives a stable orthographic camera
  (`projection:1`, fixed at y≈2000) where screen↔terrain mapping is linear and never drifts while
  painting (verified: camera position is identical before/after a stroke). Tilt with a middle-drag
  for the cliff/wall shots. Auto-Orthogonal keeps top-down shots ortho.
- Collapsing the bottom **Output** panel (click the "Output" tab) gives a taller 3D viewport and
  reveals the full tool column (Tint, Grid).

## Re-capture (review pass)

- **23-swap-texture-paint-poster.png** — re-extracted at t = 3.0 s from the existing clip: the frame
  where the bright multi-coloured swapped image is clearly visible on the ground under the brush
  (the old poster mostly showed the blue painted lake). The HTML caption for clip 23 was rewritten
  to describe that frame (no house / red-brown claim).
- **Recipes** — `docs/guide/recipes/04-terrain-painting.json` (20 items covering every file/series).
  Tested on :112/8912; all pass. Item 22 opens the Select Texture2D Quick Load dialog; the actual
  PNG pick and the swapped-slot thumbnail are a manual step because the texture swap is session-only
  (the fresh capture session shows the stock Clay slot).
