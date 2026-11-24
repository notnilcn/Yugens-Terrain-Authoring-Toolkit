# Capture notes — page `03-terrain-sculpting` (Area C02 — Sculpting)

Source of truth for the writer. Verified in the real editor (session `docs-sculpt`, display
`:72`, port `8872`, window resized to **1600×1040** so the 3D viewport is taller — the stock
1600×900 clips the toolbar's last two buttons behind a scrollbar; see Caveats). Scene:
`res://tutorial/01_flat_playground.tscn` (3×3 chunks, Square grid, chunk size 32, cell size 2.0).
Cross-checked against `addons/MarchingSquaresTerrain/rust/crates/mst/src/editor/tools.rs`,
`extra_tools.rs`, `terrain_ui.rs`, `overlay_ui.rs`, and `pt_core/src/stamp.rs`.

The sculpting UI is **three surfaces**:
- **Left toolbar** — the vertical icon bar that appears in the 3D view *only while a
  MarchingSquaresTerrain node is selected*. Pick a tool here.
- **Bottom attribute strip** — a wrapping row of controls under the 3D view. Its contents change
  with the selected tool. (Internally `MarchingSquaresToolAttributes`, a `MstFlow` that wraps.)
- **Hint bar** — one translucent line of text at the bottom-left of the 3D view that summarises
  the current tool's mouse/modifier actions.

Selecting a `Pixel*` node instead swaps these same containers to the pixelizer UI (another page).

---

## 1. Feature inventory

### 1.1 Left terrain toolbar (top → bottom)

Button order and number shortcuts come from `tools.rs` (`TOOLS` has the number keys 1–9; the
four EXTRA_TOOLS — Stamp, Gradient, Select, Tint — have **no** number shortcut). Each button is a
32-px row; hovering shows a long tooltip (full text below). Selecting a tool keeps its button
pressed (highlighted blue), changes the strip title + hint, and clears any active selection/paste.

| Button (label) | Key | Internal tool | Icon | What it does |
|---|---|---|---|---|
| **Collapse** | — | toggle compact | chevron | Show/hide the tool *names* (icon-only compact bar). Tooltip: "Show / hide the tool names (icons only)." Persists in editor project metadata (`marching_squares_terrain/toolbar_compact`). Icon flips ‹ / ›. |
| **Brush** | 1 | Brush / RaiseLower | brush | Elevate or lower terrain. Two modes (Drag, Raise/Lower) via the Brush Mode dropdown. |
| **Level** | 2 | Level | bell | Flatten terrain toward the **Height** value. |
| **Smooth** | 3 | Smooth | cube | Relax terrain toward neighbours' average; Terraced vs Sloped. |
| **Bridge** | 4 | Bridge | wedge | Build a straight ramp between two dragged points. |
| *(separator)* | | | | |
| **Foliage** | 5 | Scatter | grass | Paint grass/trees/rocks/props (covered on the Foliage page, not here). |
| **Paint** | 6 | VertexPaint | checker | Paint ground textures (covered on the Painting page, not here). |
| *(separator)* | | | | |
| **Inspect** | 7 | Debug | wrench | LMB a cell → prints its data to the Output panel. |
| **Chunks** | 8 | ChunkManager | hex-cube | Add/remove/merge chunks (covered on the Chunks page). |
| **Settings** | 9 | TerrainSettings | gear-blob | Global terrain settings pages (covered on the Settings page). |
| *(separator)* | | | | |
| **Stamp** | — | Stamp | stamp | Stamp a heightmap shape (mountain, crater, mesa, valley, hill, or an image). |
| **Gradient** | — | Gradient | wedge | Pull terrain toward a straight ramp between two picked points. |
| **Select** | — | Select | gear+ | Copy/paste a patch of cells (height + textures + grass). |
| **Tint** | — | Tint | drop | Paint a saved colour tint over cells. |
| *(separator)* | | | | |
| **Grid** | — | toggle overlay | hex grid | Toggle the editor grid overlay (cell lines) on the terrain. Tooltip: "Grid Overlay — Show cell lines on the terrain while editing (show_grid_in_editor)." Pressed state mirrors the terrain's `show_grid_in_editor`. |

This page (C02) documents **Brush, Level, Smooth, Bridge, Stamp, Gradient, Select, Inspect, Tint**
plus Collapse and Grid. Foliage, Paint, Chunks, Settings are on their own pages.

Universal brush shortcuts (from the Brush tooltip, apply to all brush-based tools):
- **Shift+LMB+Drag** — add cells to the current draw selection.
- **Shift+MouseWheel** — increase/decrease brush **size**. (Since the 2026-11 fix this **no longer
  also zooms the 3D camera** — see §7; clip 88 shows the value moving while the camera holds still.)
- **Ctrl+MouseWheel** — increase/decrease **strength** / height. This works on **Brush (Raise/Lower),
  Smooth, Gradient and Stamp** (whose hints all end with "Ctrl+wheel: strength" / "…height"). It does
  **not** apply to **Level** — Level has no Strength, and its hint ends at "Shift+wheel: size" (the
  2026-11 fix removed Level's false Ctrl+wheel claim). Clip 90 shows Strength changing with the
  camera held still; clip 92 shows the corrected Level hint.
- **Alt / RMB / Esc** — reset the current draw selection. **RMB is the reliable mouse clear** (clip
  83). Esc is the documented keyboard equivalent; it could not be exercised through the capture
  harness (synthetic Escape is swallowed before it reaches the 3D-viewport tool — a harness
  limitation, not a tool bug), so the follow-up clip films RMB.
- **1–9** — switch tools (clip 101; works with the toolbar expanded or collapsed).
- **Alt+MouseWheel** — rotate the brush 15°/15° (any brush tool). Since the fix this **no longer
  zooms the camera** either; clip 89 shows a Square footprint spinning in place.

### 1.2 Full tool tooltips (verbatim from source)

- **Brush [1]** — "Used to elevate or lower terrain." Two modes: *Drag* (LMB paints a white-cell
  selection; LMB-drag that starts on a white cell moves every selected cell up/down together, one
  undo step; snaps to the terrace step on Terraces/HexPrism, Ctrl = free; LMB off the selection
  starts a new one; Shift+LMB adds, Ctrl+LMB removes, Alt+LMB drags only the hovered cell, RMB/Esc
  clears). *Raise/Lower* (hold LMB to raise up to Ceiling, RMB to lower down to Floor, LMB+RMB to
  flatten toward Height; Ctrl+LMB / MMB / RMB pick Ceiling / Height / Floor).
- **Level [2]** — "Used to level terrain to a certain height." Ctrl+LMB sets the Level height to
  the hovered cell's Y value.
- **Smooth [3]** — "Used to smooth neighbouring terrain to their average height." Smooth Mode
  Terraced keeps flat cells; Sloped turns smoothed cells into slanted ramps (like Bridge).
- **Bridge [4]** — "Used to create a bridge between two points." LMB-drag start→end. The curve
  falloff is set by the Ease Value attribute (Godot `ease()`: 1 = linear, <0 = in-out, -1 = none).
- **Inspect / Debug Brush [7]** — "Used to print data about selected cells": global position,
  sample/chunk/local coords, height, floor & wall textures, grass, smooth flag.
- **Stamp** — "Stamps a heightmap (mountain, crater, mesa, valley, hill or an image in
  addons/MarchingSquaresTerrain/stamps/) onto the terrain. The preview follows the cursor." Modes
  Add/Max/Min/Replace + sculpting modes. LMB stamps; LMB-drag pins the centre and drags out size &
  rotation; Shift/Ctrl/Alt+wheel = size/height/rotate; Z/X rotate; negative Height carves.
- **Gradient** — "Pulls the terrain towards the straight ramp between two points." Ctrl+LMB = start
  point (position + height), Ctrl+RMB = end point, LMB-drag paints the ramp with the brush.
- **Select** — "Selects cells to copy and paste them (height, textures, grass)." LMB-drag adds
  cells; Alt/Esc clears; C copies (anchor = hovered cell); V or Paste enters paste mode; in paste
  mode Z/X rotate, M mirrors, LMB pastes (one undo step), RMB/Esc leaves.
- **Tint** — "Paints a color over whole cells (territory, biome hints, paths, decals). Tints are
  saved with the terrain data and drawn by the terrain shader in the editor and in game." LMB
  paints the Tint colour at Opacity; RMB erases; Shift+wheel = size.

### 1.3 Attribute-strip controls per tool (name · type · range · default · tooltip)

Ranges, defaults and tooltips are from `tools.rs` (`portray`, `tooltip`, `State::default`).
"Size" is shown as **diameter** (world units); internally it is radius × 2. The Size slider's max
scales with the terrain's cell size / chunk size (`size_scale`), so the numbers below are the base
range before scaling.

**Brush (Drag mode)** — strip order: Brush Type, Brush Mode, Size, Aspect, Rotation, Flatten,
Falloff, Peak, Grid Aligned, Grid Size, Quick Paint.
- **Brush Type** · dropdown · Round / Square / Hexagon / Hexagon30 / Triangle / Triangle180 ·
  default Round · "Brush shape."
- **Brush Mode** · dropdown · Drag / Raise / Lower · default Drag · "Drag: paint a selection, then
  drag it up/down. Raise / Lower: hold LMB/RMB to sculpt continuously." (Switching to Raise/Lower
  swaps the tool and the strip gains Ceiling/Height/Floor/Strength.)
- **Size** · slider · 1–50 (×size_scale) step 0.5 · default 15.0 (radius 7.5) · "Brush diameter in
  world units (Shift+wheel)."
- **Aspect** · slider · 0.2–5.0 step 0.05 · default 1.0 · "Brush Z radius / X radius (ellipse /
  rectangle brushes)."
- **Rotation** · spin · −180…180° step 1 · default 0 · "Brush rotation in degrees (Alt+wheel)."
- **Flatten** · checkbox · default **on** · "Flatten the dragged selection to the clicked height."
- **Falloff** · checkbox · default **on** · "Weaken the brush towards its edge."
- **Peak** · slider · −1…1 step 0.05 · default 0.010 · "Falloff contrast: > 0 pointy peaks,
  < 0 flat-topped mesas."
- **Grid Aligned** · checkbox · default off · "Snap the brush to whole cells (cell grids, Hexagon
  brushes, no falloff)." (Disabled unless a Triangle/Hexagon/Hex-Rings grid **and** Falloff off —
  on a Square grid it is greyed; tooltip when disabled: "Grid Aligned needs a Triangle/Hexagon/Hex
  Rings grid and Falloff off.")
- **Grid Size** · slider · 0–20 step 1 · default 1 · "Size of the grid-aligned stamp in cells."
- **Quick Paint** · dropdown · None / Full Void / Base Grass Masked / Selected Material ·
  default None · "Paint textures while sculpting with the height tools." (Applies a texture as you
  sculpt: Full Void = slot 15 ground+wall no grass; Base Grass Masked = slot 0; Selected Material =
  the current paint slot.)

**Brush (Raise / Lower mode)** adds: Ceiling, Height, Floor, Strength (replaces Flatten).
- **Ceiling** · spin · −50…50 step 0.1 · default 16.0 · "Raise never goes above this height
  (Ctrl+LMB picks it)."
- **Height** · spin · −50…50 step 0.1 · default 0.0 · "Target height (Ctrl+LMB picks it from the
  hovered cell)." (Shown as "Height"; Stamp shows the same control as "Level".)
- **Floor** · spin · −50…50 step 0.1 · default −8.0 · "Lower never goes below this height
  (Ctrl+RMB picks it)."
- **Strength** · slider · (Raise/Lower) 0.25–30 step 0.25, default 4.0 · "Strength of the tool
  (Ctrl+wheel)."

**Level** — Brush Type, Size, Aspect, Rotation, **Height**, Falloff, Peak, Grid Aligned, Grid Size,
Quick Paint. (Height = target flatten height; Ctrl+LMB picks it.)

**Smooth** — **Smooth Mode** (dropdown Terraced / Sloped, default Terraced, "Terraced: smoothed
cells stay flat steps. Sloped: smoothed cells become slanted ramps between their neighbours, like
the Bridge tool"), Brush Type, Size, Aspect, Rotation, **Strength** (Smooth strength slider
0.0255–0.3005 step 0.0055, default ≈0.15, clamped 0.0055–1.0), Grid Aligned, Grid Size, Quick Paint.

**Bridge** — Brush Type, Size, **Ease Value** (spin −5…5 step 0.1, default −1.0, "Bridge height
curve, Godot ease(): 1 = linear, 2 = ease-in, 0.5 = ease-out, <0 = in-out, -1 = no easing"),
Grid Aligned, Grid Size, Quick Paint.

**Inspect (Debug)** — Brush Type, Size only. LMB prints the hovered cell to Output.

**Stamp** — Stamp (dropdown of shapes), Mode, Size, Aspect, Height (="Level" label? no — the Stamp
strip shows **Height** = white-value height), Rotation, Snap Rotation, Contrast, Limits, Ceiling,
Height(Level), Floor. Detailed:
- **Stamp** · dropdown · Mountain / Crater / Mesa / Valley / Hill (+ any PNG/JPG/WEBP/EXR in
  `addons/MarchingSquaresTerrain/stamps/`) · default Mountain · "Stamp heightmap (built-ins +
  images in addons/MarchingSquaresTerrain/stamps/)."
- **Mode** · dropdown · Add / Max / Min / Replace / Lift / Carve / Level / Broad · default Add ·
  "Add: raise. Max / Min: only raise / lower towards the stamp. Replace: set the area. Sculpting
  modes (after Terrain3D): Lift/Carve add/remove height×value; Level pulls toward the Level height;
  Broad builds landmass from Level up by Height, only ever raising."
- **Size** · slider · 2–200 step 0.5 · default 24.0 · "Stamp width in world units (Shift+wheel)."
- **Aspect** · slider · 0.2–5.0 step 0.05 · default 1.0 · "Stamp depth / width."
- **Height** · spin · −100…100 step 0.1 · default 8.0 · "Height of white in the stamp (Ctrl+wheel).
  Negative carves." (NOTE: in the strip the Stamp's `Height` setting is labelled **Height**, while
  the `Ceiling/Height/Floor` block's Height is labelled **Level** for the Stamp tool.)
- **Rotation** · spin · −360…360 step 1 · default 0 · "Stamp rotation in degrees (Alt+wheel, Z/X)."
- **Snap Rotation** · checkbox · default on · "Rotate in 60° steps on hex/triangle grids (15° on
  square grids)."
- **Contrast** · slider · 0.2–4.0 step 0.05 · default 1.0 · "Gamma of the stamp values: > 1 sharper,
  < 1 fuller."
- **Limits** · checkbox · default off · "Stamp limits (sculpting modes): the stamp never raises
  above Ceiling or lowers below Floor (every mode), and Level/Broad use Level as their height. Off:
  no limits, Level/Broad level at the clicked height."
- **Ceiling / Level / Floor** · spins as above (Height shown as **Level** here).

**Gradient** — Brush Type, Size, Aspect, Rotation, **Strength** (Gradient strength slider
0.0055–1.0, default 0.30), Falloff, Peak.

**Select** — Brush Type, Size, Grid Aligned, Grid Size, **Copy** (button), **Paste** (button),
**Paste Rotation** (spin −360…360 step 1, default 0, "Rotation of the pasted patch (60° steps on
hex, 90° on square)"), **Mirror** (checkbox, "Mirror the pasted patch (M)"), **Relative Height**
(checkbox default **on**, "Paste heights relative to the target cell (off: copied absolute
heights)").

**Tint** — Brush Type, Size, Aspect, Rotation, **Tint** (dropdown of 10 colours: Red, Orange,
Yellow, Green, Teal, Blue, Purple, Pink, White, Black; "Color painted by LMB (RMB erases)"),
**Opacity** (slider 0.05–1.0 step 0.05, default 0.5, "Strength of the painted tint (its alpha)"),
Grid Aligned, Grid Size.

---

## 2. Walkthroughs

### W1 — Find the toolbar and read the strip
1. In the Scene dock click **MarchingSquaresTerrain**. The vertical tool toolbar appears at the
   left of the 3D view and the attribute strip under it. (media 00-orientation, 01-toolbar)
2. The highlighted tool is pressed (blue). Hover any button for its tooltip; press **1–9** to
   switch the first nine tools. Click **Collapse** to shrink to icons only. (02-toolbar-compact)
3. Read the strip for the current tool's controls and the hint bar for its mouse actions.
   (03-brush-strip, 04-hint-bar)

### W2 — Raise your first hill (Brush, Raise/Lower)
1. With Brush selected, set **Brush Mode → Raise / Lower** (strip dropdown). The strip now shows
   Ceiling / Height / Floor / Strength. (05-brush-raiselower-strip)
2. Hover the flat ground; a round footprint follows the cursor. **Hold LMB and drag a little** —
   the ground rises toward **Ceiling** (16). (30/31/32-raise)
3. **RMB-drag** lowers toward **Floor** (−8). **LMB+RMB** flattens toward **Height**.
   **Ctrl+LMB / MMB / RMB** pick Ceiling / Height / Floor from the hovered cell.
4. **Falloff on** = soft rounded hill; **off** = flat-topped, abrupt. (36/37) **Peak** tips the
   profile pointy (+1) or flat-topped (−1); the change is subtle on a single stroke. (33/34/35)
5. **Ctrl+Z** undoes the whole stroke; **Ctrl+Shift+Z** redoes it.

### W3 — Paint a selection and lift it as one (Brush, Drag mode)
1. Brush Mode **Drag**. LMB-drag over the ground — the cells you touch turn **white** (a
   selection). Shift+LMB adds more, Ctrl+LMB removes, RMB/Esc clears. (40 clip)
2. LMB-drag **starting on a white cell**, moving the mouse **up/down**, and the whole selection
   moves together — with Flatten on it becomes a flat-topped plateau. One Ctrl+Z. (41-after)

### W4 — Change the brush shape and size
- **Brush Type** (dropdown): Round, Square, Hexagon, Hexagon30, Triangle, Triangle180 — the
  footprint changes to match. (10–15) **Size** 1–50 (Shift+wheel; or drag the slider). (16–19)
  **Aspect** squashes the footprint into an ellipse. (20–23) **Rotation** / Alt+wheel turns it.

*(more walkthroughs — Level, Smooth, Bridge, Stamp, Gradient, Select, Inspect, Tint, recipes —
appended as captured below.)*

## 3. Media manifest
| File | Kind | Shows |
|---|---|---|
| 00-orientation.png | shot (full+box) | Where the terrain toolbar is (red box) |
| 01-toolbar.png | shot | Expanded toolbar, all tools + number keys |
| 02-toolbar-compact.png | shot | Collapsed (icon-only) toolbar |
| 03-brush-strip.png | shot | Brush (Drag) attribute strip |
| 04-hint-bar.png | shot | Brush hint line |
| 05-brush-raiselower-strip.png | shot | Brush (Raise/Lower) strip with Ceiling/Height/Floor/Strength |
| 10–15 brushtype-*.png | series | Round/Square/Hexagon/Hexagon30/Triangle/Triangle180 footprints |
| 16–19 size-*.png | series | Size 5 / 15 / 30 / 50 footprints |
| 20–23 aspect-*.png | series | Aspect 0.3 / 1.0 / 2.5 / 5.0 footprints |
| 30-raise-before / 32-raise-after.png | before/after | Flat plateau → rounded hill |
| 31-raise-hill.mp4 (+poster) | clip | Holding LMB to raise a hill |
| 33–35 peak-*.png | series | Peak −1 / 0 / +1 (subtle) |
| 36-falloff-on / 37-falloff-off.png | before/after | Soft rounded vs flat-topped hill |
| 40-drag-select-raise.mp4 (+poster) | clip | Paint a white selection, drag it up |
| 41-drag-mode-after.png | shot | Selection lifted into a flat plateau |
| 06-stamp-strip.png | shot | Stamp attribute strip (Stamp/Mode/Size/Height/…/Limits) |
| 07-gradient-strip.png | shot | Gradient strip |
| 08-select-strip.png | shot | Select strip (Copy/Paste/Paste Rotation/Mirror/Relative Height) |
| 09-tint-strip.png | shot | Tint strip (Tint colour / Opacity) |
| 42-level-before / 43-level-after.png | before/after | Level raises a flat shelf at Height (set Height=5) |
| 44-smooth-before / 45-smooth-after.png | before/after | Smooth relaxes bumps (subtle on square grid) |
| 48-bridge-before / 50-bridge-after.png | before/after | Bridge ramp from plateau down to water |
| 49-bridge-ramp.mp4 (+poster) | clip | Dragging a bridge ramp between two heights |
| 51–55 stamp-*.png | series | Mountain / Crater / Mesa / Valley / Hill committed shapes |
| 56-stamp-dragsize.mp4 (+poster) | clip | LMB-drag pins centre, drags out size+rotation |
| 46-gradient-before / 47-gradient-after.png | before/after | Gradient pulls a ramp between picked A/B |
| 57-select-copy-paste.mp4 (+poster) | clip | Select a hill, C copy, V paste, LMB drop a copy |
| 58-select-two-hills.png | shot | Original + pasted hill |
| 60-inspect-output.png | shot | Inspect/Debug cell data printed to the Output panel |
| 61-tint-before / 62-tint-after.png | before/after | Painting a red tint over cells |
| 63-grid-off / 64-grid-on.png | before/after | Grid overlay (cell lines) off vs on |

### W5 — Level, Smooth, Bridge
- **Level [2]** flattens the dragged ground toward **Height**. Set Height (or Ctrl+LMB to pick a
  height from a cell), then drag: ground below rises, ground above is cut down — a flat shelf.
  (42/43) Falloff softens the edge.
- **Smooth [3]** relaxes cells toward their neighbours' average (rounds bumps). (44/45) **Smooth
  Mode**: *Terraced* keeps flat cell steps; *Sloped* turns them into ramps — the difference only
  shows on a **cell grid** (hex/triangle); on a square grid it always ramps (see hex section).
- **Bridge [4]** builds a straight ramp: LMB-drag from the start point to the end point and the
  ground between is pulled onto the line connecting their heights. **Ease Value** bends the ramp's
  profile. (48/49/50)

### W6 — Stamp, Gradient, Select, Inspect, Tint
- **Stamp** drops a prebuilt heightmap (Mountain/Crater/Mesa/Valley/Hill, or an image). A live
  wireframe preview follows the cursor; LMB stamps it (51–55). LMB-**drag** pins the centre and
  drags out size + rotation (56); Shift/Ctrl/Alt+wheel = size/height/rotate; Z/X rotate. **Mode**
  Add/Max/Min/Replace + sculpting modes; **negative Height carves** (craters, valleys); **Limits**
  clamps to Ceiling/Floor.
- **Gradient** pulls terrain toward a straight ramp between two picked points: **Ctrl+LMB** start,
  **Ctrl+RMB** end, then LMB-drag to paint the ramp. (46/47)
- **Select** copies a patch (height + textures + grass): LMB-drag to select cells, **C** copy, **V**
  paste mode (ghost follows cursor, Z/X rotate, M mirror), LMB drops the copy, RMB/Esc leaves. One
  undo per paste. (57/58)
- **Inspect [7]** prints the clicked cell's data to the **Output** panel: global position, sample /
  chunk / local coords, height, floor & wall texture slot + name, grass, smooth flag. (60)
- **Tint** paints a saved colour over whole cells (territory/biome/path hints): pick a **Tint**
  colour + **Opacity**, LMB paints, RMB erases. Tints are stored with the terrain and drawn in the
  editor and in game. (61/62)
- **Grid** (toolbar) toggles the white cell-line overlay on the terrain. (63/64)

| 70-hex-grid.png | shot | Hex (honeycomb) cell overlay on hex_island |
| 72-sculpt-playground.png | shot | The flat sculpt_playground scene |

### W7 — Square vs hex grid
- The **flat_playground** uses a **Square** grid (Grid Type = Square): the grid overlay is square
  cells (63/64), **Grid Aligned** is greyed out, brush Rotation snaps 15°/90°, and **Smooth Mode
  Terraced vs Sloped look the same** (a square grid always ramps between samples).
- The **hex_island** uses a **Hex Rings** grid: the overlay is hexagons (70), **Grid Aligned is
  available** (snap the brush to whole hex cells), Stamp/paste Rotation snaps **60°**, and **Smooth
  Mode Terraced (flat hex steps) vs Sloped (ramped hex cells)** visibly differ. To see hexes,
  select the terrain, press **Grid**, and zoom in on a flat patch.

### Toolbar hotspot tour (for `media/03-terrain-sculpting/01-toolbar.png`)
Positions are % of that image (buttons stacked at x ≈ **45%**):
- Collapse 4% · Brush 11% · Level 17% · Smooth 23% · Bridge 29% · Foliage 37% · Paint 43% ·
  Inspect 50% · Chunks 57% · Settings 63% · Stamp 70% · Gradient 76% · Select 82% · Tint 88% ·
  Grid 96%.
Labels/text: see §1.1. (Foliage/Paint/Chunks/Settings are documented on other pages.)

### Recipes (step-by-step; reuse the tool clips)
All on the **sculpt_playground** (flat). Undo a mistake with Ctrl+Z.
- **A rounded hill** — Brush → Raise/Lower; hold **LMB** on the spot; let go when tall enough.
  Falloff on. (31-raise-hill)
- **A cliff / plateau** — Level; type a **Height** (e.g. 6); **Falloff off**; drag a patch → a
  flat-topped shelf with sharp sides. (43-level-after) Or Stamp → **Mesa**. (53)
- **A crater** — Stamp → **Crater**, or Stamp with a **negative Height** (carves a pit); LMB to
  stamp. (52) Floor clamps the depth.
- **A valley with a river bed** — Stamp → **Valley** for the trough (54); then Brush Raise/Lower
  **RMB** (lower) with a small brush to carve the river channel along the bottom; paint it later.
- **Terraced farmland** — on a **hex** terrain, Smooth → **Terraced** over a slope keeps flat
  cell steps; or Level repeatedly at rising Heights to stack shelves.
- **A ramp up a cliff** — Bridge; LMB-drag from the low ground to the top of the cliff → a straight
  ramp. (49-bridge-ramp) Ease Value bends its profile.

### Undo / redo (verified)
- Every stroke is **one** undo step with a readable name in the Output/History ("terrain
  raise/lower", "terrain level", "terrain smooth", "terrain bridge", "terrain stamp", "terrain
  gradient", "terrain paste", "terrain tint paint", "terrain height draw" for Drag mode).
- **Ctrl+Z** undoes, **Ctrl+Shift+Z** redoes. Two strokes = two undos (reverse order). Undo works
  after switching tools and after clicking in the 3D view. A Fill/Clear/Stamp/Paste is a single
  undo step. (Verified repeatedly this session — no broken steps found.)

## 4. Roblox bridges
- Roblox's **Terrain Editor** has Add / Subtract / Grow / Erode / Smooth / Flatten / Paint with a
  brush size & strength. MST splits those jobs across named tools: **Brush (Raise/Lower)** = Add +
  Subtract (LMB raises, RMB lowers); **Level** = Flatten (to a chosen Height); **Smooth** = Smooth;
  **Paint** = Paint (its own page). There is no live "Grow/Erode" sphere — MST is a *cell grid*, so
  terrain is heights on a grid of cells, not a voxel blob.
- Roblox brush size/strength → MST **Size** (Shift+wheel) and **Strength** (Ctrl+wheel).
- The **Drag** brush mode has no Roblox parallel: you first *paint a selection* of cells (like
  selecting Parts), then drag the whole selection up/down as one — closer to selecting and moving
  Parts with the Move tool than to sculpting.
- **Stamp** is like dragging in a prebuilt terrain blob/region; **Select** copy/paste is like
  Ctrl+C / Ctrl+V on a group of Parts, but for a patch of ground.

## 5. Caveats
- **Window height / toolbar scroll:** at the stock 1600×900 the toolbar is taller than the 3D
  view, so the last buttons (Tint, Grid) sit behind a vertical scrollbar and the bar scrolls. I
  resized the window to 1600×1040 so every button is visible for the captures. Worth a `.callout.warn`
  that on a short editor the toolbar scrolls — scroll it or use Collapse (compact) to fit.
- The 3D viewport is short because the Output panel is open; sculpting still reads fine when the
  camera is framed close.
- Grid Aligned is greyed out on a **Square** grid (needs Triangle/Hexagon/Hex-Rings + Falloff off).

## 6. Scenes created
- **`res://tutorial/guide/sculpt_playground.tscn`** (+ its data
  `res://tutorial/guide/sculpt_playground_data.res`) — a **flat** 3×3-chunk Square-grid terrain
  (heights ≈ 0, range −0.9…0.8) with the pixel pipeline, water, sky, day/night and an orbit
  camera, ready for readers to sculpt on. Built by flattening `01_flat_playground` with the Level
  tool (Height 0, Falloff off) in a top-down view, then saving. Copied into the repo under
  `tutorial/guide/`. Loads with no errors; the central green chunk is the main play surface (the
  surrounding chunks sit at/under the water line). Reference shot: 72-sculpt-playground.png.

## 7. Additional caveats found this session
- **FIXED (2026-11): wheel modifiers no longer move the camera.** Previously Shift+wheel (size) and
  Alt+wheel (rotation) also zoomed/moved the 3D camera while resizing/rotating the brush. After the
  rebuild + template refresh, Shift/Alt/Ctrl+wheel act **only** on the brush attribute (size /
  rotation / strength) and the camera holds still. Verified in clips 88 (size), 89 (rotation) and
  90 (strength): the strip value changes while the background/framing stays fixed. The older note
  that said "Shift+wheel also zooms" no longer applies.
- **FIXED (2026-11): Ctrl+wheel strength now works on Smooth, Raise/Lower and Gradient** (and Stamp
  height), and the hints were corrected — **Level no longer claims Ctrl+wheel** (it has no Strength).
  See the Ctrl+MouseWheel bullet in §1.1 and clips 90/92/93/97.
- **The attribute strip reflows when a value's text width changes** (e.g. Peak "0.010" vs
  "-1.000"), shifting the controls to its right horizontally. Not a bug, but worth knowing: re-read
  control positions after changing a neighbouring value.
- Peak's pointy-vs-flat effect is **subtle on a single stroke**; stack strokes to exaggerate.
- On a **Square** grid, Smooth Terraced vs Sloped look identical (by design — square grids always
  ramp between samples); the difference is a hex/triangle-grid feature.
- No `ERROR`/`SCRIPT ERROR`/panic from `mst`/`MarchingSquares` was seen during any sculpting
  operation this session.
- **Erosion is not a sculpt-toolbar tool** — it is a separate **child node** (`MarchingSquaresErosion`)
  you add under the terrain, with its own Inspector settings + an **Erode** button (and a matching
  bottom strip when the node is selected). It is now **documented on this page** in §8 (inventory +
  walkthrough) because it is a shaping/weathering operation a sculptor reaches for; clips 108–114.

---

## 8. Erosion — `MarchingSquaresErosion` (child node, not a toolbar tool)

**What it is.** A worn/weathered look without hand-sculpting every gully. You add a
`MarchingSquaresErosion` **child node under the terrain**, point it at the terrain, tune how the
simulated rain behaves, and press **Erode**. It runs a hydraulic-erosion simulation on a background
thread and applies the result to the terrain as **one undo step** ("terrain erosion"). Source:
`addons/MarchingSquaresTerrain/rust/crates/mst/src/terrain/erosion.rs` (+ `pt_core/src/erosion.rs`).
It is `#[class(tool, base = Node)]` — a plain Node, so it shows an **Inspector** (below) and, when
selected, a small bottom strip echoing **Basics / Droplet** tabs and the **Erode** button.

### 8.1 How to add it (Scene dock)
1. In the **Scene** dock, select **MarchingSquaresTerrain**.
2. Click the **Add Child Node** button (the **+** at the dock's top-left), type **`Erosion`** in the
   search, pick **MarchingSquaresErosion**, **Create**. It appears **indented under the terrain**
   (media 108 add-node dialog, 109 scene dock). A yellow **⚠ warning** sits on it until you assign a
   terrain ("Assign the Terrain to erode: without one, Erode does nothing.").
3. In the **Inspector**, next to **Terrain** click **Assign…** → choose the parent
   **MarchingSquaresTerrain**. The warning clears. (Because the node is a child, you simply pick its
   parent.)

### 8.2 Inspector settings (name as shown · internal · default · range · what it does)
**Basics**
- **Terrain** · `terrain` · (unset) · the terrain this node weathers. **Required** — Erode warns and
  does nothing if empty.
- **Droplets** · `iterations` · **30000** · 100–500000 (or greater), step 100 · how many rain
  droplets to simulate. More droplets = more weathering, slower run.
- **Seed** · `seed` · **0** · RNG seed. **Auto-increments by 1 after every Erode**, so pressing Erode
  again gives a *different* weathering pass (not a repeat).
- **Erode** · button (▶) · runs the simulation (see §8.3).

**Droplet** group — how a single water droplet behaves (media 113):
- **Radius** · `splash_radius` · **4** · 2–8 · splash/deposit disk radius (pixels).
- **Momentum** · `momentum` · **0.051** · 0–1 · how strongly a droplet keeps its direction.
- **Load** · `load_limit` · **4.0** · 0–32 (or greater) · max sediment a droplet can carry.
- **Min Load** · `min_load` · **0.011** · 0–1 · minimum carry before it drops sediment.
- **Carve** · `carve_rate` · **0.3** · 0–1 · how fast it erodes / picks up material.
- **Drop** · `drop_rate` · **0.3** · 0–1 · how fast it deposits material.
- **Dry** · `dry_rate` · **0.011** · 0–1 · evaporation per step (shortens droplet life).
- **Gravity** · `gravity` · **4.0** · 0–32 (or greater) · downhill acceleration.
- **Steps** · `droplet_steps` · **32** · 1–256 · max steps a droplet travels before it dies.
- **Volume** · `start_volume` · **1.0** · 0–8 · starting water volume.
- **Speed** · `start_momentum` · **1.0** · 0–8 · starting speed.

**Terrain** group — how the sim maps onto the cell grid (media 114):
- **Px / Cell** · `pixels_per_cell` · **2** · 1–8 · sim resolution per terrain cell (higher = finer
  detail, slower).
- **Height Scale** · `height_scale` · **0.0** · 0–256 (or greater) · 0 = auto-pick from the terrain.
- **Mode** · `mode` · **Auto** · {Auto, Raw, Terraced} · Auto follows the grid (terraced on cell
  grids, raw/smooth otherwise); Raw = smooth heights; Terraced = snap the result to terrace steps.
- **Level Step** · `level_step` · **0.0** · 0–16 (or greater) · terrace step size; 0 = use the
  terrain's own terrace step.
- **Min Plateau** · `min_plateau_cells` · **4** · 0–64 (or greater) · smallest flat run kept as a
  plateau (stops tiny steps).

*Not exported:* the running job and its progress (they exist only during a run). *Signals:*
`erosion_progress(progress: float)` and `erosion_finished(applied: bool)` — handy for a progress bar.

### 8.3 Walkthrough — weather a blocked-out landscape
1. Block out terrain with sharp features first (stamped mesas/mountains read best). The follow-up
   capture used a flat playground filled with Mountain/Mesa/Hill stamps (sharp terraced sides).
2. Add the node and assign the terrain (§8.1). Leave the defaults, or raise **Droplets** for a
   heavier pass.
3. Press **Erode** (Inspector button, or the **Erode** button in the bottom strip when the node is
   selected). In the editor it runs on a **background thread** — the editor stays responsive; you
   cannot start a second run while one is going (`is_running()` → true). It finishes in a couple of
   seconds for 30000 droplets on this machine.
4. Result applies as **one undo step "terrain erosion"**: sharp stamped ridges become **rounded
   ridgelines with gullies and sediment aprons at their base**; terraced walls soften. (media 110
   before / 112 after; clip 111 shows the whole terrain snap from sharp → weathered when the job
   lands.)
5. **Undo:** Ctrl+Z reverts the entire erosion in a single step (verified: history action
   "terrain erosion", `has_undo: true`). Press **Erode** again (Seed has advanced) for a fresh,
   different weathering layered on top.

**Gotchas.** (a) No terrain assigned → Erode does nothing but prints a warning. (b) Seed advances
each run, so "Erode" is never an exact repeat — reset Seed if you need determinism. (c) It weathers
the **whole** assigned terrain, not a brushed region (the editor exposes no region box). (d) Big
Droplets counts + high Px/Cell are slower.

---

## 9. Clips (follow-up) — media manifest for this pass

All on **sculpt_playground** (square grid) at a low 3/4 framing unless noted; hex clips on
`demo/hex_island.tscn`. MP4 + poster PNG for every clip. The three fixes are proven by clips 88/89
(wheel no longer moves the camera), 90 (Ctrl+wheel strength works) and 92 (Level hint corrected).

| File | Kind | Shows |
|---|---|---|
| 80-drag-shift-add.mp4 | clip | Drag mode: paint a white selection, **Shift+drag adds** cells |
| 81-drag-ctrl-remove.mp4 | clip | Drag mode: **Ctrl+drag removes** cells from the selection |
| 82-drag-alt-onecell.mp4 | clip | Drag mode: **Alt+drag** collapses to **one cell** and lifts it ("1 selected") |
| 83-drag-rmb-clear.mp4 | clip | Drag mode: **RMB clears** the selection (RMB/Esc; Esc = keyboard equivalent) |
| 84-raise-lmb.mp4 | clip | Raise/Lower: **hold LMB** raises a hill toward Ceiling |
| 85-raise-rmb-lower.mp4 | clip | Raise/Lower: **hold RMB** digs a pit toward Floor |
| 86-raise-both-flatten.mp4 | clip | Raise/Lower: **LMB+RMB** flattens a bump toward Height |
| 87-ctrl-pick.mp4 | clip | **Ctrl+LMB/RMB/MMB** pick Ceiling / Floor / Height (strip spins change: 4.6 / −5.3 / −0.2) |
| 88-shift-wheel-size.mp4 | clip | **Shift+wheel = size**; Size value changes, **camera holds still** (fix) |
| 89-alt-wheel-rotation.mp4 | clip | **Alt+wheel = rotation**; a Square footprint spins in place, **camera still** (fix) |
| 90-ctrl-wheel-strength.mp4 | clip | **Ctrl+wheel = strength** (4.0→10.5); hint reads "Ctrl+wheel: strength", **camera still** (fix) |
| 91-brush-type-footprints.mp4 | clip | Brush Type cycles **Round → Square → Hexagon → Triangle**; footprint outline + strip label change |
| 92-level.mp4 | clip | **Level** drags a flat shelf to Height=6, Ctrl+LMB picks a height; hint ends at "Shift+wheel: size" (fix) |
| 93-smooth.mp4 | clip | **Smooth** relaxes a bump+pit; hint ends "…Ctrl+wheel: strength" |
| 94-hex-smooth-terraced-sloped.mp4 | clip | **Hex grid:** Smooth **Terraced** then **Sloped** on the mesa's terraced wall |
| 95-bridge.mp4 | clip | **Bridge** LMB-drag builds a ramp/causeway between two points; hint "Shift+wheel: width" |
| 96-stamp-builtins.mp4 | clip | **Stamp** places Mountain → Crater → Mesa → Valley → Hill across the chunk |
| 97-gradient.mp4 | clip | **Gradient** Ctrl+LMB start, Ctrl+RMB end, LMB-drag paints the ramp; hint "…Ctrl+wheel: strength" |
| 98-select-copy-paste.mp4 | clip | **Select** drag → **C** copy → **V** paste ghost → **Z** rotate / **M** mirror → LMB drop → RMB leave |
| 99-inspect.mp4 | clip | **Inspect** clicks 3 cells; Output prints global pos / sample / height / floor+wall tex / grass / smooth |
| 100-undo-redo.mp4 | clip | **Ctrl+Z / Ctrl+Shift+Z** of a stamp stroke (mountain vanishes, reappears; cursor parked in sky) |
| 101-toolbar-collapse-keys.mp4 | clip | **Number keys 1–4** switch tools (button highlights) + **Collapse** toggles icon-only bar |
| 104-recipe-river-valley.mp4 | clip | Recipe: Stamp **Valley** ×2 → a valley trough with raised walls |
| 107-recipe-crater.mp4 | clip | Recipe: Stamp **Crater** → crater rim/pit (use negative Height for a deeper pit) |
| 108-erosion-add-node.png | shot | Create-Node dialog with **MarchingSquaresErosion** matched |
| 109-erosion-scene-dock.png | shot | Scene dock: **MarchingSquaresErosion** as a child of the terrain (⚠ until terrain assigned) |
| 110-erosion-before.png | before | Sharp stamped mesas/ridges before erosion |
| 111-erosion-run.mp4 | clip | Pressing **Erode** → the whole terrain weathers in one step |
| 112-erosion-after.png | after | Same framing after erosion: rounded, gullied, sediment at the base |
| 113-erosion-inspector-droplet.png | shot | Inspector **Droplet** group (Radius/Momentum/Load/…/Volume/Speed) |
| 114-erosion-inspector-terrain.png | shot | Inspector **Terrain** group (Px/Cell/Height Scale/Mode/Level Step/Min Plateau) |

**Recipe coverage** (brief's six): a **hill** = clip 84 (raise LMB); a **cliff/plateau** = Level
shelf (clip 92) or Stamp **Mesa** (clip 96 / still 53); a **river valley** = clip 104 (Stamp Valley,
then carve the bed with Brush RMB); **terraced farmland** = hex Smooth **Terraced** (clip 94) or
stacked Level shelves; a **ramp up a cliff** = Bridge (clip 95); a **crater** = clip 107 (Stamp
Crater; negative Height carves deeper). Stills 51–55 already show each committed stamp shape.

### Capture notes for this pass (for the writer)
- **Framing:** the SculptPlayground sits inside a stylized pixel world; relief reads clearly only at
  a **low 3/4 angle zoomed onto the worked chunk** (far/top-down looks flat). The Sun/Camera node
  gizmos were hidden (View ▸ Gizmos) for clean shots. The Raise/Lower brush is **gentle** (strength
  is a rate, not a set-height) — for dramatic relief use the **Stamp** tool.
- **Esc-clear** could not be filmed (synthetic Escape is swallowed by the editor before it reaches
  the viewport tool — harness limitation, not a bug); RMB-clear is filmed instead (clip 83) and Esc
  is the documented keyboard equivalent.
- No `ERROR`/`SCRIPT ERROR`/panic from `mst`/`MarchingSquares` occurred during this pass, including
  the erosion run.

## Re-capture (review pass)

- **10–15 brush-type series** — re-shot as one consistent set at a 1600×1080 window (675×450 crops,
  no hint bar), default editor camera, cursor parked on the chunk. Round / Square / Hexagon /
  Hexagon30 / Triangle / Triangle180 outlines are legible and the background is identical between
  frames. (The old 10 was 675×480 and included the hint bar, so it did not match.)
- **44 / 45 smooth before/after** — re-shot with the *same* close camera: a hard, flat-topped mound
  (Brush Raise/Lower, Falloff off, Ceiling 10) before; after two Smooth passes at the maximum
  Strength (0.3005) the top and sides are a soft rounded swell. 600×300 crops from the same view.
- **46 / 47 gradient before/after** — re-shot on the same mound: Ctrl+LMB on its top, Ctrl+RMB at
  its foot, six passes along the line, then the Inspect tool hides the dotted guide line, leaving
  the ramp visible in 47. 600×300 crops, same camera.
- **20–23 aspect** — the shape series (wide bar / square / tall rectangle / strip) was re-verified;
  the stills use the Square brush in Drag mode, where one click selects the footprint that Aspect
  stretches.
- **HTML** — restored the Aspect scrubber, the Smooth compare and the Bridge compare + clip 49,
  re-inserted where they belong (Aspect, Smooth, Bridge); captions, scrubber notes and alt text
  updated for the re-shot stills, incl. Peak (now visibly different) and the square-brush Aspect.
- **Recipes** — `docs/guide/recipes/03-terrain-sculpting.json` (55 items, one per file or numbered
  series). Tested on :112/8912: all pass except `110-112-erosion`, which needs a
  `MarchingSquaresErosion` child node assigned to the terrain first (the item documents the gap).
  Camera framings for the backfilled items are reconstructed with the default editor camera and
  may be slightly wider than the originals; the subject and state are the same.
