# Capture notes — page `05-foliage-grass` (Area C04 — Foliage & Grass)

Session: `docs-foliage`, display `:74`, port `8874`.
Scenes used:
- `res://tutorial/02_island.tscn` — clean UI stills (the palette there is the simple default:
  Grass / Tree / Rock / + Bush / + Flowers / + Pine, all shown with correct colours).
- `res://demo/hex_island.tscn` — ALL painting / action clips (the rich sandbox: grassy hex hill +
  stone ruin + pond + existing custom props). Its palette additionally lists the terrain's own
  custom scatter items (Pine, Bush, Rock, Flowers, Pebbles) whose thumbnails/meshes are grey
  low-poly placeholders; the painting demos therefore use the built-in `+` presets (+Pine / +Bush /
  +Flowers), which render as proper green trees / bushes / colourful flowers.
- A saved follow-along scene (see "Scenes created").

Status: COMPLETE — 16 MP4 clips, 3 knob series, 2 before/after pairs, 3 recipe stills, ~17 UI stills.

This page teaches the **Foliage tool** (left 3D toolbar, slot 5) — scattering grass, trees, rocks,
bushes, flowers, pines and your own props by painting — plus **grass** (the terrain's grass blades
and their look settings) and the **Wild Flowers** node. It is the sibling of page `04` (Painting the
ground): painting *textures* vs scattering *props*.

ROBLOX BRIDGE (top-level): In Roblox Studio you drag individual trees/rocks from the Toolbox and
place them one by one. Here you *paint* them in bulk with a brush, like Roblox's Terrain "Grass"
material but for 3D props, and each prop type carries its own placement rules (density, slope,
which ground textures it likes).

---

## 1. Feature inventory

### 1a. Left toolbar — Foliage button
- **Foliage** — left 3D toolbar, slot 5. Shortcut **`5`**. Only visible while a
  `MarchingSquaresTerrain` node is selected; the button stays pressed while active. Selecting it
  opens the right **Foliage** palette panel and swaps the bottom strip to the foliage settings.
- Tooltip (verbatim):
  > Foliage Tool [5]
  >
  > Paints grass, trees, rocks, bushes, flowers and your own props onto the terrain.
  > Pick one or more entries in the palette on the right (Ctrl/Shift+click for several).
  >
  > [SHORTCUTS]
  > • LMB-drag: paint the selected entries. A click places at least one prop.
  > • RMB-drag or Shift+LMB-drag: erase the selected entries (every prop with "Erase All").
  > • Shift+MWU/MWD: brush size.
  >
  > [INFO]
  > • Density: 1 = props almost touching (spacing follows each prop's size); a new stroke
  >   over the same spot fills gaps.
  > • Grass grows only on textures with Grass ticked (Vertex Painting panel).
  > • "Fill Terrain" scatters the selected props over the whole terrain with each item's
  >   own rules (ground textures, slope, height, clusters); painting ignores those rules.
  > • Add props with the palette's + entries or "Add Mesh / Scene..."; Edit opens an item in the inspector.
  > Props follow the terrain when it is sculpted.

### 1b. Right palette panel ("Foliage")
Panel title tooltip: "What the Foliage tool paints: grass and the terrain's Scatter Items
(PixelScatterItem resources)." Top of panel is a 3-line instruction label:
"1. Click what to paint below (Ctrl/Shift+click: several).  2. LMB-drag on the terrain to paint it.
RMB / Shift+LMB: erase · Shift+wheel: brush size".

The palette is an **ItemList** (icon grid, 2 columns). Its entries are, in order:
1. **Grass** — always first. Icon = grass-blade glyph. Tooltip: "Grass (the terrain's grass mask).
   Grows only on textures with Grass ticked in the Vertex Painting panel." Selecting it paints the
   grass blade mask instead of a prop.
2. **One entry per Scatter Item the terrain already carries** (its `scatter_items` array), labelled
   by the item's Display Name (Tree, Rock, Pine, Bush, Flowers, Pebbles…). Icon = a rendered preview
   thumbnail. Tooltip: "<name> (Scatter Item N). Double-click to edit it."
3. **Built-in "+" presets** for any of the five kinds the terrain does NOT yet have as a procedural
   item: **+ Rock, + Bush, + Tree, + Pine, + Flowers** (only the missing ones appear). Tooltip:
   "Built-in <kind>: click it and paint. This terrain has no <kind> item yet: the first stroke adds
   it to its Scatter Items (undoable)." Painting a "+" entry MATERIALISES it (adds a real
   PixelScatterItem to the terrain, as one undoable step) and the "+" disappears from its label.
   NOTE: a custom-mesh item of a given kind does NOT suppress that kind's "+" preset, so a terrain
   can list both e.g. "Bush" (custom mesh) and "+ Bush" (built-in).

Selection: single click selects one; **Ctrl/Shift+click** selects several (paint them together).
**Double-click** a prop entry = open it in the Inspector (same as Edit).

Buttons under the list:
- **Edit** — "Shows the selected prop's settings (mesh, scale, placement rules, collision...) in the
  inspector." Disabled unless at least one *prop* (not Grass) is selected.
- **Remove** — "Removes the selected props from this terrain's Scatter Items, with every instance of
  them (undoable)." Disabled unless a prop is selected.
- **Add Mesh / Scene…** — "Adds a prop from a mesh (.mesh / .tres / .res / .obj) or a scene / model
  (.tscn / .glb / .gltf / .blend / .fbx) and selects it." Opens a file dialog titled "Add a prop
  from a mesh or a scene". The chosen file becomes a new PixelScatterItem (named from the file
  stem, `_`→space): a `.tscn/.glb/...` fills its **Scene** slot, a mesh fills its **Cage** slot,
  Form = Custom. THIS IS THE HOOK FOR PAGE 12 ("use your own models as foliage"): page 12 can link
  here; the exact fields a custom model lands in are **Scene** (packed scenes/models) or **Cage**
  (bare meshes) on the PixelScatterItem, Form set to Custom.

### 1c. Bottom strip (settings) — left→right
| Control | Type | Range / default | Tooltip | Notes |
|---|---|---|---|---|
| **Size** | HSlider | 1–50 (shown; accepts a bit higher), default 15 | "Brush diameter in world units (Shift+wheel)." | Brush is always a round footprint for foliage. Also **Shift+MWU/MWD**. |
| **Density** | HSlider | 0.05–1.0, default 0.25 | "How full a stroke paints: 1 = props almost touching, lower = sparser (spacing follows each prop's size). A new stroke over the same spot fills gaps." | Only affects **painting**, not Fill Terrain. |
| **Slope Min** | SpinBox | 0–90°, step 0.5, default 0 | "Only affect terrain at least this steep (degrees)." | Hand-paint slope filter. |
| **Slope Max** | SpinBox | 0–90°, step 0.5, default 90 | "Only affect terrain at most this steep (degrees)." | e.g. Max 20 = only paint on near-flat ground. |
| **Erase All** | CheckBox | off | "Erasing (RMB / Shift+LMB) removes every prop, not only the selected ones." | When on, the hint bar changes to "RMB / Shift+LMB: erase every prop". |
| **Fill Terrain** | Button | — | "Scatters the selected props over the whole terrain with each item's own rules (density, ground textures, slope, height, clusters); replaces their current instances. Undoable." | Big operation; respects each item's own Density/slope/height/texture rules (NOT the strip Density). Acts on selected props only (not Grass). |
| **Clear** | Button | — | "Removes every instance of the selected props from the terrain. Undoable." | Acts on selected props only. |

Hint bar (bottom-left of viewport) with props selected:
`Painting <names>  ·  LMB-drag: paint  ·  RMB / Shift+LMB: erase  ·  Shift+wheel: size  ·  palette on the right: pick what to paint`.
With nothing selected: `Pick Grass, a tree or a rock in the Foliage palette on the right  ·  then LMB-drag on the terrain to paint it`.
"Nothing painted" message (spot full / under water / built over / outside slope): shown when a
stroke changes nothing. "Nothing erased here" message when RMB finds nothing of the selected kind.

Cursor: a round brush disc; **green** while painting, **red/orange** while erasing (Shift held or
RMB). Diameter = Size.

### 1d. Inspector — grass "look" settings (terrain node, Look ▸ Grass subgroup)
Select the terrain, Inspector ▸ Look ▸ Grass. Default values in parentheses.
- **Grass** (grass_enabled, bool, default on) — the grass BLADES on/off globally.
- **Subdivisions** (grass_subdivisions, int 1–8, default 2) — blades per cell = subdivisions².
- **Grass Size** (grass_size, Vector2, default ~ (0.56, 0.66) in hex_island) — blade (width, height).
- **Grass Texture** (grass_texture) — the blade sprite texture (optional override).
- **Turf Stuff** (turf_stuff, ShaderMaterial) — the grass shader material (default mst_grass.gdshader).
  Holds the **Animation FPS** parameter (see Settings ▸ Grass).
- **Grass Pixel Size** (grass_pixel_size, int 1–16+, default 1) — pixel-size multiplier for the grass
  blades in the pixel renderer (bigger = chunkier blocks).
- **Grass Outline** (grass_outline, bool, default off, labelled with an "On" checkbox) — draw a pixel
  outline around grass blades.
- **Ledge Cutoff** (grass_ledge_threshold, 0–1 slider, default 0.5) — "Grass is not planted on
  cliff-edge bands stronger than this." (keeps grass off cliff tops.)
- **Min Height** (grass_min_height, float, default 1.2 in 02_island / 0.6 in hex_island) — "No grass
  below this height (e.g. your water level)."
Media: `20-inspector-grass.png`.

### 1e. Settings ▸ Grass page (Settings tool, slot 9 → "Grass" tab)
Same grass fields surfaced in the bottom strip (Terrain Settings): page tabs = Grid · New Chunks ·
**Grass** · Display · LOD & Nature · Storage. Grass page fields:
- **Grass** (checkbox) — enable grass blades. Tooltip "Grow grass on slots with 'Has grass'."
- **Animation FPS** (spin, default 0) — "> 0 animates the grass with limited fps; 0 = smooth wind."
  (This writes the turf shader's `animation_fps` param; it is undoable as its own action.)
- **Subdivisions** (spin 1–8, default 2) — "Grass blades per cell = subdivisions²."
- **Grass Size** (Vector2 x,y) — "Grass blade size (width, height)."
- **Ledge Cutoff** (0–1) — "Grass is not planted on cliff-edge bands stronger than this."
- **Min Height** (float) — "No grass below this height (e.g. your water level)."
Media: `21-settings-grass.png`.
(Related: whether a ground texture grows grass at all is the **Has grass** tick per texture slot in
the **Paint** tool's panel — that lives on page 04. Grass blades only appear on grass-ticked slots.)

### 1f. PixelScatterItem fields (Edit / double-click opens this resource)
A **PixelScatterItem** (`.tres` resource; the five presets live in
`addons/MarchingSquaresTerrain/scatter_presets/`: rock, bush, tree, pine, flowers). Grouped fields
(verified in scatter.rs and the Inspector):
- **Display Name** (string) — palette label.
- **Form** (enum: Custom / Rock / Bush / Tree / Pine / Flowers) — the built-in procedural shape; set
  to **Custom** when you supply your own Cage/Scene.
- **Shape Seed** (int) — varies the procedural shape.
- **Cage** (Mesh) — your own mesh for the prop (bare meshes land here).
- **Scene** (PackedScene) — your own scene/model for the prop (models land here). ← page-12 hook.
- **Tint** (Color).
- **Placement** group: Density (0.0015–4, the item's own density — used by Fill Terrain), Scale Min,
  Scale Max (0.05–8), Random Yaw (0–360°), Align to Normal (0–1), Y Offset, Per Cell Max (1–16),
  Small Hexes (bool), **Ground Slots** (Texture1…Texture16 flags — which painted ground textures the
  prop is allowed to grow on), Seed.
- **Filters** group: Slope Min / Slope Max (0–90°), Height Min / Height Max, Cluster Scale
  (0–256; clumps props using noise), Cluster Threshold (−1…1).
- **Color** group: Hue Jitter (0–0.5), Value Jitter (0–0.5).
- **Pixel** group: Pixel Size (1–16), Outline (bool), Outline Id (0–7), Cast Shadows (bool),
  Wind Sway (0–1).
- **Collision** group: Collision (None/Cylinder/Box), Collision Size (Vector3), Impact Stratum
  (physics layers), Collision Min Scale.
The five preset values (from the `.tres`): Tree form=3 density 0.02 slope_max 25 cluster 22;
Pine form=4 density 0.026 slope_max 32 cluster 18; Bush form=2 density 0.04 slope_max 30 cluster 14;
Flowers form=5 density 0.08 slope_max 25 cluster 9 (cluster_threshold 0.25); Rock form=1 density
0.03 slope_max 55 align_to_normal 0.8 y_offset −0.12.
Media: `40-scatter-item.png` (top fields + groups), `41-scatter-placement.png` (Placement expanded).

### 1g. Wild Flowers node (MarchingSquaresWildFlowers)
A separate **node** you add under the terrain that plants whole meadows of wild flowers in one go,
weighted by a simple climate model (slope, how wet the ground is, shade from trees, distance from
water). CAPTURED and verified.

**How to add it** (two ways):
- **Settings tool ▸ LOD & Nature page** → the **Add** button on the "Wild Flowers" row ("Select this
  terrain's MarchingSquaresWildFlowers child") — creates the child AND wires its **Terrain** to this
  terrain automatically, and selects it. (Media `50-lod-nature.png`.) The same page has an **Add**
  for a droplet-**Erosion** child.
- Or add a `MarchingSquaresWildFlowers` node in the Scene tree by hand, then drag the terrain into
  its **Terrain** slot. (Config warning "Assign the Terrain to plant: without one, Scatter Wild
  Flowers does nothing." until you do.)

**Inspector** (media `51-wildflowers-inspector.png`): **Terrain** (the terrain to plant on),
**Scatter Wild Flowers** button, **Clear Wild Flowers** button, **Species** (Array[PixelScatterItem]
— auto-filled with ~9 built-in wild species on first scatter: Wild Yarrow, Foxglove, Cowslip,
Harebell, Mallow …), **Seed**, and collapsed **Placement** / **Ground water** groups.

**Dedicated bottom strip** (appears when the node is selected; media `52-wildflowers-strip.png` /
`52b-wildflowers-groundwater.png`): title "Wild Flowers". Two tabs:
- **Placement** ("Scatter / clear and the flower placement."): **Scatter** button
  (tooltip "MarchingSquaresWildFlowers.scatter_flowers() (undoable)"), **Clear** button
  ("clear_flowers() (undoable)"), **Seed**, **Density** (default 1.0), **Patch Scale** (1.0),
  **Threshold** (bloom threshold, 0.1), **Max Slope** (0.374), **Points/Cell** (8), **Jitter** (0.4).
- **Ground Water** ("Ground water that decides where flowers grow."): **Ground Water**, **Variation**,
  **Shore Wet** (flowers gather near water), **Water Level**, **Shore Dist.**, **Trees Shade**
  (fewer flowers under trees).

Pressing **Scatter** plants flowers across the whole terrain in one undoable action
("scatter wild flowers"); they visibly cluster toward the pond shore (shore wetness) — see
`53-wildflowers-scatter.mp4`. **Clear** removes them ("clear wild flowers", `54-wildflowers-clear.mp4`).
Source fields (wild_flowers.rs) with ranges: Bloom Threshold 0–1, Max Slope 0–1, Points Per Cell
2–32, Jitter 0–1, Density 0–2+, Patch Scale 0.05–8; Ground Water / Variation / Shore Wetness 0–1,
Shore Distance 0.01–64, Trees Shade bool.
CAVEAT worth flagging: Scatter permanently ADDS its species to the terrain's Scatter Items, so they
then also appear in the Foliage palette (and stay after Clear) — the palette gets longer.

---

## 2. Walkthroughs (performed)
1. **Switch to Foliage**: select the terrain → click **Foliage** (or press `5`). The right palette
   and the bottom strip appear.
2. **Paint a forest**: click **+ Pine** in the palette → LMB-drag across the grass. (clip 10)
3. **Different props**: click **+ Bush** / **+ Flowers** / **Rock** → drag. (clips 11/12/13)
4. **Erase**: with a prop selected, RMB-drag (or Shift+LMB-drag) over it. Tick **Erase All** to wipe
   every prop regardless of selection. (clips 15/16)
5. **Grass blades**: click **Grass** → LMB-drag adds blades (only on grass-textured ground),
   RMB-drag removes them. (clip 14)
6. **Fill Terrain / Clear**: select a prop → **Fill Terrain** scatters it everywhere by its own
   rules; **Clear** removes every instance. (clips 17/18)
7. **Knobs**: Density 0.05→1.0 (series 30), brush Size 2.5→52 (series 31), Shift+wheel grows the
   brush live (clip 32), grass Pixel Size 1→8 (series 33).
8. **Undo**: every action is one undoable step (Ctrl+Z) — paint, erase, fill, clear, materialise a
   "+", add/remove an item (clip 62 paints rocks then Ctrl+Z removes them).
9. **Adding a palette entry**: click a "+" entry (e.g. + Bush) → first LMB-drag both PAINTS it and
   ADDS the real item to the terrain; the palette label loses its "+" (clip 61). Or
   **Add Mesh / Scene…** to import your own model (→ page 12).
10. **Wild Flowers**: add the node (Settings ▸ LOD & Nature ▸ Add), press **Scatter** → a meadow
    blooms (clip 53); **Clear** removes it (clip 54).

### Recipes (performed; stills)
- **Rocks on a shore** (`70-recipe-rocks-shore.png`): select **Rock**, moderate Density, LMB-drag
  ALONG the pond's edge. Rocks line the waterline like a pebble beach. (Tip: tick a low **Slope
  Max** to keep them off the water itself.)
- **A flower meadow** (`71-recipe-flower-meadow.png`): select **+ Flowers** (or Ctrl+click several
  Wild species), crank **Density** high, LMB-drag back and forth over flat grass for a dense bloom.
- **A forest edge** (`72-recipe-forest-edge.png`): select **+ Pine**, paint a dense band with
  several overlapping strokes, then a single light pass at the margin so the trees thin out into
  open grass — a natural tree line.

### Foliage reacts to sculpting (verified)
Per the tool tooltip "Props follow the terrain when it is sculpted" — VERIFIED: paint a prop
cluster, switch to the **Brush** tool (Raise/Lower mode) and raise the ground under it, and the
props ride up with the surface (they stay planted on the terrain, not floating). No separate clip
shipped (on the hex-prism demo terrain a raise makes blocky columns that read poorly, and the flat
02_island hides height) — the hero still `04-hero-scene.png` shows props correctly sitting on the
terraced hillside. Writer: present this as a short note + the hero still, or a `.callout.tip`.
Grass, likewise, re-drapes the new surface. (This is the big advantage over Roblox: you don't
re-place props after reshaping the ground.)

## 3. Media manifest (final — 16 clips, 3 knob series, 2 before/after pairs, 3 recipe stills)
| File | Kind | Scene | Caption / shows |
|---|---|---|---|
| 00-foliage-overview.png | shot (3 red boxes) | 02_island | The three surfaces: left toolbar, right palette, bottom strip |
| 01-toolbar-foliage.png | shot | 02_island | Left toolbar, Foliage (slot 5) highlighted |
| 02-palette.png | shot | 02_island | Default palette Grass/Tree/Rock/+Bush/+Flowers/+Pine + Edit/Remove/Add |
| 02b-palette-loaded.png | shot | hex_island | A terrain carrying many scatter items |
| 03-strip.png | shot | 02_island | Bottom strip: Size/Density/Slope Min/Slope Max/Erase All/Fill Terrain/Clear |
| 04-hero-scene.png | shot (hero) | hex_island | The sandbox hill; props sit on the terraced slope (= props follow terrain) |
| 05-switch-tool.mp4/.png | clip | hex_island | Click Foliage (slot 5) → palette panel + strip appear |
| 10-paint-pine.mp4/.png | clip | hex_island | Paint a pine forest with +Pine (hero paint) |
| 11-paint-bush.mp4/.png | clip | hex_island | Paint bushes (+Bush) |
| 12-paint-flowers.mp4/.png | clip | hex_island | Paint flowers (+Flowers) |
| 13-paint-rock.mp4/.png | clip | hex_island | Paint rocks (Rock) |
| 14-grass-mask.mp4/.png | clip | hex_island | Grass blades: RMB-erase, then LMB-paint a patch |
| 14b-grass-mask-on/off.png | before/after | hex_island | Grass tufts present vs removed (ground stays green) |
| 15-erase.mp4/.png | clip | hex_island | RMB-erase carves a gap in pines (selected kind only) |
| 16-erase-all.mp4/.png | clip | hex_island | Erase All ticked → RMB removes every prop incl. rocks (hint: "erase every prop") |
| 17-fill-terrain.mp4/.png | clip | hex_island | Fill Terrain scatters pines over the whole terrain by their own rules (dense on the flat fort top) |
| 18-clear.mp4/.png | clip | hex_island | Clear removes every pine instance |
| 20-inspector-grass.png | shot | 02_island | Inspector Look▸Grass: Turf Stuff, Grass Pixel Size, Grass Outline, Ledge Cutoff, Min Height |
| 21-settings-grass.png | shot | hex_island | Settings▸Grass page (page tabs + Grass/Animation FPS/Subdivisions/Grass Size/Ledge Cutoff/Min Height) |
| 22-grass-enabled.mp4/.png | clip | hex_island | Toggle the Grass checkbox → all blade tufts vanish and return |
| 22b-grass-enabled-on/off.png | before/after | hex_island | Grass blades on vs off (ground colour unchanged) |
| 30-density-1..5.png | SERIES (Density) | hex_island | 0.05 / 0.20 / 0.50 / 0.81 / 1.0 — same patch, same camera (+Pine dab→forest) |
| 31-size-1..4.png | SERIES (Brush Size) | hex_island | 2.5 / 15 / 30.5 / 52 — single dab footprint |
| 32-brush-size.mp4/.png | clip | hex_island | Shift+wheel grows the brush (Size value in strip updates), then paint |
| 33-grass-pixel-1..4.png | SERIES (Grass Pixel Size) | hex_island | 1 / 2 / 4 / 8 — grass blades get chunkier/blockier |
| 40-scatter-item.png | shot | hex_island | PixelScatterItem Inspector (Rock): Display Name/Form/Shape Seed/Cage/Scene/Tint + groups |
| 41-scatter-placement.png | shot | hex_island | PixelScatterItem Placement group + Texture1–16 ground-slot flags |
| 50-lod-nature.png | shot | hex_island | Settings▸LOD & Nature: the Add buttons for Erosion / Wild Flowers children |
| 51-wildflowers-inspector.png | shot | hex_island | MarchingSquaresWildFlowers Inspector (Terrain, Scatter/Clear buttons, Species, Seed) |
| 52-wildflowers-strip.png | shot | hex_island | Wild Flowers strip, Placement tab (Scatter/Clear + Density/Patch Scale/Threshold/Max Slope/Points per Cell/Jitter) |
| 52b-wildflowers-groundwater.png | shot | hex_island | Wild Flowers strip, Ground Water tab (Ground Water/Variation/Shore Wet/Water Level/Shore Dist./Trees Shade) |
| 53-wildflowers-scatter.mp4/.png | clip | hex_island | Press Scatter → a wild-flower meadow blooms, clustered near the shore |
| 54-wildflowers-clear.mp4/.png | clip | hex_island | Press Clear → the wild flowers are removed |
| 61-add-entry.mp4/.png | clip | hex_island | Paint a "+" entry → palette "+ Bush" becomes "Bush" (the item is added) |
| 62-undo.mp4/.png | clip | hex_island | Paint a rock cluster, then Ctrl+Z removes it in one step |
| 70-recipe-rocks-shore.png | recipe | hex_island | Rocks painted along the pond shoreline |
| 71-recipe-flower-meadow.png | recipe | hex_island | A dense flower meadow on flat grass |
| 72-recipe-forest-edge.png | recipe | hex_island | A pine forest edge thinning into open grass |

Series values for the scrubber component:
- Density: `30-density-1.png`=0.05, `-2`=0.20, `-3`=0.50, `-4`=0.81, `-5`=1.0.
- Brush Size: `31-size-1.png`=2.5, `-2`=15, `-3`=30.5, `-4`=52.
- Grass Pixel Size: `33-grass-pixel-1.png`=1, `-2`=2, `-3`=4, `-4`=8.

## 4. Roblox bridges
- Palette = like Roblox's Toolbox, but you *paint* props with a brush instead of dragging one model.
- Density/Size sliders = like the Roblox Terrain brush's Size/Strength, but for scattering objects.
- Grass blades here are automatic decoration on grass-textured ground (like Roblox Terrain's Grass
  material look), toggled by one checkbox — not hand-placed.
- "Fill Terrain" has no Roblox equivalent: it's a one-click procedural scatter over the whole map.

## 5. Caveats
- **Grass redraw flash (llvmpipe only)**: right after an erase/scatter edit, the just-edited patch
  can flash to the bare ground texture ("dirt") for ~1–3 s until the editor redraws, then settles
  back to green grass. This is a software-renderer refresh lag, NOT the real result — the grass data
  is intact (undo names only "props"/"grass"). All clips were settled (extra mouse moves) before the
  final frame so they end on the true state. Worth a one-line `.callout.warn` so readers don't think
  erasing props removes grass.
- hex_island's own custom scatter items (Tree/Pine/Bush/Rock/Flowers/Pebbles) preview and render as
  grey low-poly placeholders; the built-in **+** presets render in colour. Painting demos use the
  **+** presets. (A terrain author supplies real meshes via Add Mesh / Scene.)
- Editor is slow (llvmpipe ~5–10 fps); Fill Terrain over a whole terrain takes a second or two.
- "Grass" only appears on ground textures that have **Has grass** ticked (Paint panel, page 04);
  painting Grass on a non-grass texture does nothing.
- **Slope Min / Max** on the strip filter painting by ground steepness (0–90°). On the hex-prism demo
  terrain the "slopes" are the near-vertical hex walls (props don't sit on them anyway), so a
  slope before/after reads poorly there — documented via the tooltips in §1c instead of a figure.
  A square-grid terrain with real hillsides is the place to show it. (Scatter Items ALSO have their
  own Slope Min/Max in their Filters group, used by Fill Terrain.)
- **grass_size** (blade width/height) was NOT captured as a before/after: my attempt to set it from
  the QA harness (`set-prop grass_size [x,y]`) failed because the harness passed a JSON **array** and
  the property expects a **Vector2** ("cannot convert from ARRAY to VECTOR2" in editor.log — a
  harness-call type mismatch, NOT an addon bug; the Inspector/Settings x/y spin fields are the
  correct way and work). So grass_size never actually changed in my shots. The clearly-visible
  grass-look knob that IS shipped is **Grass Pixel Size** (series 33). (The two `set_grass_size()`
  lines in editor.log are from these harness calls only.)
- Palette cell positions shift as items are added/removed (and the Wild Flowers scatter adds ~9
  species), so the writer should describe entries by NAME, not grid position.

## 6. Scenes created
None. The C04 brief does not call for a follow-along scene, and the existing `demo/hex_island.tscn`
is already an ideal sandbox for readers to practise foliage in. The session's hex_island copy was
used for all painting but NOT saved (edits were undone / are disposable); the repo scenes are
untouched.

## 7. Page-12 hook (custom meshes as foliage)
Readers who want their OWN model as a prop use **Add Mesh / Scene…** in the palette (or set a
PixelScatterItem's **Scene** slot for a scene/model, or **Cage** slot for a bare mesh, with Form =
Custom). Record for page 12: supported files are `.tscn / .scn / .glb / .gltf / .blend / .fbx / .obj`
(scenes & models → Scene) and `.mesh / .tres / .res` (meshes → Cage). Page 12 can deep-link to §1b
and §1f here.

STATUS: COMPLETE.

## Recipes

`docs/guide/recipes/05-foliage-grass.json` — 37 items covering every media file on this page
(grouped series where a numbered set shares one setup). Session: 1600x900, `setup` waits for the
editor to settle (`waitready`), collapses the bottom panel by clicking the active bottom-panel tab
(a small helper at `/tmp/opencode/mst_ui_ds.py`, written by `setup`, does the UI lookups), and fails
loudly if the viewport is not full height. Palette entries are clicked by grid cell (row/column) and
checked with `expect` where they matter; the strip sliders are set by thumb fraction and the spin
boxes by `spin`.

Per-item status (all tested with `--out-dir`; every item ran green):

| Item | Outputs | Status |
|---|---|---|
| 00-overview | 00-foliage-overview.png | green — full editor + the three red boxes |
| 01-02-toolbar-palette | 01-toolbar-foliage.png, 02-palette.png | green |
| 02b-palette-loaded | 02b-palette-loaded.png | green — 10 palette entries on hex_island |
| 03-strip | 03-strip.png | green |
| 04-hero | 04-hero-scene.png | green — default hex camera crop |
| 05-switch-tool | 05-switch-tool.mp4/.png | green |
| 10-paint-pine | 10-paint-pine.mp4/.png | green |
| 11-paint-bush | 11-paint-bush.mp4/.png | green |
| 12-paint-flowers | 12-paint-flowers.mp4/.png | green — Density ~0.85 so the blooms read |
| 13-paint-rock | 13-paint-rock.mp4/.png | green |
| 14-grass-mask | 14-grass-mask.mp4/.png | green |
| 14b-grass-mask-pair | 14b-grass-mask-on/off.png | green |
| 15-erase | 15-erase.mp4/.png | green — Size 6 so the erase carves a gap, not the whole band |
| 16-erase-all | 16-erase-all.mp4/.png | green — Erase All ticked, hint bar says "erase every prop" |
| 17-fill-terrain | 17-fill-terrain.mp4/.png | green — the built-in +Pine entry (the scene's custom Pine item fills nothing: empty Ground Slots) |
| 18-clear | 18-clear.mp4/.png | green |
| 20-inspector-grass | 20-inspector-grass.png | green — Turf Stuff → Min Height at 2x |
| 21-settings-grass | 21-settings-grass.png | green |
| 22-grass-enabled | 22-grass-enabled.mp4/.png | green |
| 22b-grass-enabled-pair | 22b-grass-enabled-on/off.png | green |
| 30-density-series | 30-density-1..5.png | green — Density 0.05/0.20/0.50/0.81/1.0 |
| 31-size-series | 31-size-1..4.png | green — Size 2.5/15/30.5/52 |
| 32-brush-size | 32-brush-size.mp4/.png | green — Shift+wheel to Size 28, then paint |
| 33-grass-pixel-series | 33-grass-pixel-1..4.png | green — `set-prop grass_pixel_size 1/2/4/8` (the Inspector spin rows have no stable names) |
| 40-scatter-item | 40-scatter-item.png | green — double-click the Rock palette entry |
| 41-scatter-placement | 41-scatter-placement.png | green — Placement group expanded |
| 50-lod-nature | 50-lod-nature.png | green |
| 51-wildflowers-inspector | 51-wildflowers-inspector.png | green — Add from Settings ▸ LOD & Nature |
| 52-wildflowers-strip | 52-wildflowers-strip.png | green |
| 52b-wildflowers-groundwater | 52b-wildflowers-groundwater.png | green |
| 53-wildflowers-scatter | 53-wildflowers-scatter.mp4/.png | green — viewport-only crop (the Pixel World panel is not in the media) |
| 54-wildflowers-clear | 54-wildflowers-clear.mp4/.png | green |
| 61-add-entry | 61-add-entry.mp4/.png | green — crop covers the viewport right edge + palette so the "+ Bush" → "Bush" relabel is visible |
| 62-undo | 62-undo.mp4/.png | green |
| 70-recipe-rocks-shore | 70-recipe-rocks-shore.png | green — Slope Max 20 keeps the rocks at the waterline |
| 71-recipe-flower-meadow | 71-recipe-flower-meadow.png | green — Density 1.0, close camera |
| 72-recipe-forest-edge | 72-recipe-forest-edge.png | green |

Notes / caveats for the recipes:
- The camera is the scene's default 3D camera; `zoom(n)` = n wheel-up steps over the viewport
  centre (6 for the grass-mask / grass-pixel close-ups, 4 for the paint clips, 5 for the rocks).
- Crop rectangles are window coordinates (1600x900). The panel crops are derived from the live
  control rects where possible (`cropgrass`); the strip crops use fixed y because the bottom panel
  is collapsed and the strip sits at the bottom. Two crops are a little shorter than the media
  (20/40/41/51/53/54/70) because the live layout has taller inspector rows / includes the panel
  edge; the subject and framing match.
- `17/18` use the built-in **+Pine** entry. Filling the scene's custom *Pine* Scatter Item places
  nothing (its Ground Slots are empty), which looks like a no-op.
- Verification: every item was run with `--out-dir` during this pass and produced its outputs
  (all 37 green), and the stills/posters were viewed against the media. Two items got a
  crop-only trim after their green run: `32-brush-size` records 700 px tall instead of 715 (drops
  the bottom-panel tab/status row) and `70-recipe-rocks-shore` crops 620x440 from the same top-left
  instead of 620x540 (drops the strip); both trims were checked against the green frame.
- Near the end of the pass another session with the same recapture session name
  (`recap-05-foliage-grass`) and another editor on the same test display were active; that caused
  intermittent "session already exists"/`cp` races and synthetic clicks landing in the wrong
  window, so the last full-suite rerun could not complete. The recipe itself is unchanged from the
  verified runs (only the two crop trims above).
