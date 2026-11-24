# Notes — page 06-chunks-settings (Area C05)

Source of truth for the writer. Captured in the real Godot 4.7 editor, session `docs-chunks`
(display :75, port 8875), scenes `res://tutorial/02_island.tscn` (square grid) and
`res://demo/hex_island.tscn` (hex grid, richer). Cross-checked against the Rust source under
`addons/MarchingSquaresTerrain/rust/crates/mst/src/editor/` and `.../terrain/node.rs`.

This page covers the **Chunks** tool, the **Settings** tool's six pages, the terrain's own
**Inspector**, and **Storage** (save/import/bake/export). It does NOT cover sculpting (page 03),
painting/foliage (pages 04/05), the pixel look (07), or the builder (09/10).

---

## 0. Orientation — where these tools live

- The terrain tools only appear **while the `MarchingSquaresTerrain` node is selected** in the
  Scene dock (left) or the 3D view. Select it first; if the toolbar is gone, you clicked away.
- The **left icon toolbar** in the 3D view holds the tools, numbered 1–9 plus three more:
  Brush(1), Level(2), Smooth(3), Bridge(4), Foliage(5), Paint(6), Inspect(7), **Chunks(8)**,
  **Settings(9)**, Stamp, Gradient, Select. Press the number key to switch, or click the icon.
- The **bottom attribute strip** changes per tool (Chunks shows the chunk controls; Settings
  shows a six-tab page of fields).
- The **Inspector** (right dock) always shows the terrain node's own properties, grouped
  (Grid, Look, Overlay, Advanced, Tools, Surroundings). The Settings tool is a friendlier,
  curated subset of these same properties.
- "Collapse" (top of the toolbar) shrinks the toolbar to icons only.

## 1. Feature inventory

### 1A. The Chunks tool (Chunk Manager) — toolbar icon 8, shortcut key `8`

**What a chunk is.** The terrain is a grid of square **cells**; cells are grouped into square
**chunks** (default 32×32 cells each, `Chunk Size`). A chunk is the unit the terrain loads,
meshes and saves — think of it as one tile of the map. `02_island` has **9 chunks** laid out
3×3, named `Chunk (0, 0)` … `Chunk (2, 2)` (column, row). The whole world is just the chunks
that exist; empty space around them has no data until you add a chunk there.

**Toolbar button tooltip:** "Chunk Management Tool [8] — Used to create, delete and change
chunks." Selecting the tool draws a **chunk overlay** in the 3D view and shows the chunk strip
at the bottom. The viewport caption reads: *"LMB on a green +: add a chunk · LMB on a chunk:
remove it · Ctrl+LMB: select a chunk."* A second line shows the hovered sample:
`Chunk Manager | height -0.54 cell (43, 41) chunk (1, 1)`.

**The 3D overlay (what you see on the terrain):**
- Each existing chunk is outlined with white borders; its cells are shaded.
- Around the edge of the existing chunks, every **empty neighbour** cell shows a **green disc
  with a white "+"** — click one to **add** a chunk there. (Source: a new chunk must *touch* an
  existing chunk; clicking a + that isn't adjacent warns "new chunks must touch an existing
  chunk." With zero chunks, the hovered cell becomes the first +.)
- Hovering an **existing** chunk tints it **red/orange with an X** (the remove marker) — click
  to **remove** that chunk.
- A **selected** chunk (Ctrl+LMB, or chosen in the dropdown) gets a **blue** outline.

**Bottom strip controls (left→right, top row then second row):**
| Control | Type | What it does | Tooltip / notes |
|---|---|---|---|
| **Chunk Manager** | strip title button | Just the tool label. | |
| **Chunk:** dropdown | OptionButton (130 px) | Picks the "active" chunk by name `Chunk (x, y)`. Its merge mode, vertex handles and Remove act on this chunk. | "Chunk to edit: its merge mode, vertex handles and Remove act on it (or Ctrl+LMB a chunk in the viewport)." Lists every chunk. |
| **N chunks** | label | Live count ("9 chunks"). | Reads `1 chunk` singular. |
| **Remove Chunk** | button | Removes the selected chunk (undoable). | "Removes the selected chunk (undoable). In the viewport: LMB a chunk (red X) removes it, LMB a green + adds one." **Disabled** when no chunk is selected. |
| **Merge Mode:** dropdown | OptionButton (212 px) | Sets the **selected** chunk's merge mode. Values: **Cubic, Polyhedron, Rounded Polyhedron, Semi Round, Spherical**. | "Merge mode of the selected chunk (Ctrl+LMB a chunk to select it)." Visible effect only on triangle/hex grids with stepped heights; on a square grid it is near-invisible. |
| **Apply Mode to All Chunks** | button | Applies the current merge mode to **every** chunk of the terrain (one undo step "chunk merge mode"). | "Applies the merge mode to every chunk of this terrain." |
| **Vertex Handles:** checkbox | CheckBox | Turns on draggable height handles on **every sample** of the selected chunk (the original's chunk gizmo). Drag a handle up/down to move that terrain point; **one undo step per drag** ("move terrain point"). | "Drag handles on every sample of the selected chunk (the original's chunk gizmo; one undo step per drag)." On a 32-cell chunk this is ~33×33 = 1089 handles — a dense cluster (see Caveats). |
| hint text | label | "LMB +: add chunk · LMB chunk: remove · Ctrl+LMB: select" | |

**Gestures (verified):**
- **Add chunk:** LMB a green +. Count 9 → 10; undo action "add chunk".
- **Remove chunk:** LMB an existing chunk (red X) **or** select it and press Remove Chunk.
  Count 10 → 9; undo action "remove chunk".
- **Select chunk:** Ctrl+LMB a chunk → it gets a blue outline, the Chunk dropdown updates to
  it, and Remove Chunk enables. (Plain LMB on a chunk with no Ctrl *removes* it — so to select,
  always hold Ctrl.)
- **Change merge mode:** pick in Merge Mode dropdown → sets selected chunk; "Apply Mode to All
  Chunks" sets all.
- **Vertex handle drag:** check Vertex Handles, drag a handle → terrain point moves (digest
  changes), one undo step.

**Undo (all verified):** add chunk, remove chunk, chunk merge mode, and move-terrain-point are
each a single `Ctrl+Z` step; `Ctrl+Shift+Z` redoes. The History dock names them "add chunk",
"remove chunk", "chunk merge mode", "move terrain point".

### 1B. The Settings tool (Terrain Settings) — toolbar icon 9, shortcut key `9`

**Button tooltip:** "Terrain Settings Tool [9] — Used to tweak global terrain settings." The strip
becomes a **six-tab page** (a friendlier, curated subset of the terrain's Inspector). The tab row
sits just above the field rows: **Grid · New Chunks · Grass · Display · LOD & Nature · Storage**.
Each tab's tooltip:
- **Grid** — "Grid type, chunk and cell size, blending, merge mode, collision."
- **New Chunks** — "Textures and noise given to newly added chunks."
- **Grass** — "Grass growth, animation and blade size."
- **Display** — "Pixel size, grid overlay, fog and debug views."
- **LOD & Nature** — "Distant low-detail ring; droplet erosion and wild flowers (child nodes with their own strip)."
- **Storage** — "Where the terrain data lives; save, import, bake and export."

The strip auto-wraps the fields into as many columns as fit the viewport width (so the exact
column layout shifts with window size, but the fields are the same).

Every numeric/enum change is **one undo step** named `terrain setting <label>` (e.g. "terrain
setting pixel size"). Changing a field re-applies live.

#### Grid page
| Field | Control | Range / values | Default | What it does | Notes |
|---|---|---|---|---|---|
| **Grid Type** | dropdown | Square, Triangle, Hex, Hex Rings | Square | Square = marching-squares over vertex heights (smooth); Triangle / Hex / Hex Rings = flat cells. | Changing it clears the selection and rebuilds. |
| **Chunk Size** | spin | 2–64 | 32 | Cells per chunk edge (the original's "Dimensions"). | **Fixed once chunks exist** for this grid type — the setter warns "chunk_size is fixed once chunks exist … (clear the data or switch grid type first)" and ignores the change. Set it on an empty terrain. (Shown only on non-Hex-Rings grids.) |
| **Ring Radius** | spin | 1–16 | — | Hex Rings only: radius of the hex disk each chunk owns. | Replaces Chunk Size on Hex Rings grids. **Locked** once hex-ring chunks exist ("Fixed once hex-ring chunks exist: remove every chunk to change it") — the field is greyed. |
| **Cell Size** | x/y spin | 0.1005–64 (min clamp 0.0505) | 2.0, 2.0 | Cell size in world units (hex/triangle use X only). Scales the whole terrain. | Changing it rebuilds all chunks. |
| **Wrap Chunks** | x/y spin (Hex Rings) | 0–128 | 0,0 | Hex Rings torus wrap in chunks (x cols, y rows); 0 = no wrap. | Locked once hex-ring chunks exist. |
| **Blend Mode** | dropdown | Smooth, Hard Squares, Hard Vertices | Smooth | Texture blending between slots. | |
| **Merge Mode** | dropdown | Cubic, Polyhedron, Rounded Polyhedron, Semi Round, Spherical | Polyhedron | Merge mode given to **newly added** chunks (the default; per-chunk merge mode is set in the Chunks tool). | |
| **Edge Style** | dropdown | Flat Walls, Terraces, Slopes, Hex Prism | Hex Prism (hex scenes) | Triangle/hex grids only: how height steps render — ramps (Flat Walls/Slopes), stepped terraces (Terraces), or stacked prism columns (Hex Prism). | Shown only when Grid Type ≠ Square. |
| **Instanced** | checkbox | — | — | Hex + Hex Prism only: draw columns as GPU MultiMesh instances instead of a meshed chunk (height edits only re-upload the cell map). | Shown only when Grid Type ≥ Hex. |
| **Extra Collision Layer** | dropdown | None, 9…32 | None | Adds one extra physics layer (9–32) to the terrain collision. | |

#### New Chunks page (textures + noise for newly added chunks)
| Field | Control | Range | Default | What it does |
|---|---|---|---|---|
| **Floor Slot** | dropdown (slot names) | slots 0–15 | 1. Grass | Floor texture of newly added chunks. |
| **Wall Slot** | dropdown (slot names) | slots 0–15 | 6. Cliff | Wall texture of newly added chunks. |
| **Noise Amp** | spin | 0–256 | 0.0 | Height amplitude of fractal noise for new chunks (0 = flat). |
| **Noise Freq** | spin | 0–4 | 0.051 | Frequency of the new-chunk noise. |
| **Noise Seed** | spin | int32 | 1 | Seed of the new-chunk noise. |
| **Noise Hmap** | resource picker | a `Noise` resource | empty | Noise resource for new chunks (heights = noise × Noise Height). **Overrides Noise Amp / Freq.** |
| **Noise Height** | spin | 0–1024 | 32.0 | Height range of Noise Hmap. |

> Adding a chunk (Chunks tool) uses these: Noise Amp = 0 gives a flat chunk; Noise Amp > 0 gives
> rolling bumps shaped by the seed/frequency. This is the closest thing to Roblox's "Generate".

#### Grass page
| Field | Control | Range | Default | What it does |
|---|---|---|---|---|
| **Grass** | checkbox | — | on | Grow grass on slots marked "Has grass". |
| **Animation FPS** | spin | 0–60 | 0 | >0 animates grass at limited fps; 0 = smooth wind. (Writes the turf shader's `animation_fps`; undo step "terrain grass animation fps".) |
| **Subdivisions** | spin | 1–8 | 2 | Grass blades per cell = subdivisions². |
| **Grass Size** | x/y spin | 0.0505–16 | 0.56, 0.66 | Grass blade size (width, height). |
| **Ledge Cutoff** | slider | 0–1 | 0.5 | Grass not planted on cliff-edge bands stronger than this. |
| **Min Height** | spin | −1024…1024 | 1.2 | No grass below this height (e.g. your water level). |

#### Display page
| Field | Control | Range | Default | What it does |
|---|---|---|---|---|
| **Pixel Size** | spin | 1–16 (or greater) | 1 | Pixel-size multiplier in the pixel renderer — bigger value = chunkier blocks. **Clear visible effect** (see series). |
| **Editor Grid** | checkbox | — | off | Cell lines on the terrain while editing (in game: Show Grid). Subtle on a busy texture; clearest up close. |
| **Chunk Lines** | checkbox | — | off | The grid overlay also draws chunk borders. |
| **Editor Fog** | checkbox | — | off | Show the fog of war (cell data) while editing — with no revealed cells the terrain goes dark under fog. **Dramatic.** |
| **Debug View** | dropdown | Off, Slope, Height Contours, Slot Ids, Chunk Borders, Cell Ids, Cell Data | Off | Terrain debug shader view. All seven are visually distinct (see series). |

#### LOD & Nature page
| Field | Control | Range | Default | What it does |
|---|---|---|---|---|
| **LOD Distance** | spin | 0–4000 (or greater) | 0 | Distant low-detail ring: chunks far from the LOD focus and camera get flat cell tops, plain walls, no grass, no drawn props (collision stays). 0 = off. |
| **LOD In Editor** | checkbox | — | off | Apply the distant LOD ring in the editor too (off: editor always shows full detail). |
| **Erosion** | button (Add / Select) | — | — | Selects this terrain's `MarchingSquaresErosion` child, adding one if there is none (undoable). Hydraulic droplet erosion with its own strip. (Detail is page 08/effects.) |
| **Wild Flowers** | button (Add / Select) | — | — | Selects this terrain's `MarchingSquaresWildFlowers` child, adding one if none (undoable). Clustered meadow flowers. |

#### Storage page (changes with Storage Mode)
Always shown: **Storage Mode**, **Import MST**, **Bake Meshes**, **Export GLB**.
When Storage Mode is **Baked** or **Runtime**, four more appear: **Save Data**, **Data Directory**,
and (Baked only) **Bake Grass** + **Bake Collision**.
| Field | Control | What it does |
|---|---|---|
| **Storage Mode** | dropdown: Resource / Baked / Runtime | Resource = all chunks live in the data resource (the `.res` next to the scene). Baked / Runtime = one file per chunk in Data Directory, written on scene save (only changed chunks); Baked also stores meshes for fast loading. |
| **Bake Grass** | checkbox (Baked) | Also store the grass multimeshes in the baked files. |
| **Bake Collision** | checkbox (Baked) | Also store the collision faces. |
| **Data Directory** | text ("(generated on save)") | Folder of the per-chunk files (empty = generated next to the scene on save). Press Enter to apply. |
| **Save Data** | "Save" button (Baked/Runtime) | Write the terrain data now (changed chunks only). |
| **Import MST** | "Import..." button | Import the **original** MarchingSquaresTerrain's data: opens a file dialog filtered to `*.tscn` (original scene — reads grid settings + data_directory) or `*.res` (MSTChunkData / metadata.res); you can also pick its data folder. |
| **Bake Meshes** | "Bake" button | Bakes the chunks into a static copy beside the terrain (undoable). |
| **Export GLB** | "Export" button | Geometry baker: exports every chunk to the terrain's Export Path as a GLB with a baked texture atlas. |

### 1C. The terrain's own Inspector (right dock) — the full property set

The Settings tool is a curated subset; the Inspector shows **everything**. Top-level (ungrouped)
fields first, then collapsible groups. (Field = Inspector label; the Settings-tool name, where
different, is in the reference above.)

- **Top level:** Contents (the `MarchingSquaresTerrainData` resource = where chunk data lives),
  Grid Type, Chunk Size, Cell Size, Tex Assign (a `MarchingSquaresTexturePreset`).
- **Grid** group → subgroups:
  - *Hex Rings* (hex only): Ring Radius, Wrap Chunks, Wrap Period.
  - *Levels*: Level Height.
  - *Hex Edges* (hex only): Hex Edge Style, Hex Edge Instanced, Hex Edge Grooves.
  - *Blending*: Blend Mode.
  - *Defaults*: Default Merge Mode, Default Floor Slot, Default Wall Slot.
  - *New Chunk Noise*: amplitude, frequency, seed, Noise Hmap, Noise Hmap Height.
  - (Hex Rings / Hex Edges subgroups are hidden on a Square grid — proven in editor.)
- **Look** group → subgroups:
  - *Terrain*: Land Stuff (shader material), Pixel Size, Outline, Outline ID, render Layers, Cast Shadows.
  - *Grass*: Grass enabled, Subdivisions, Grass Size, Grass Texture, Turf Stuff (material), Grass Pixel Size, Grass Outline, Ledge Threshold, Min Height.
  - *Foliage*: Scatter Items, Scatter Group Tiles, Scatter Body Quiet ms, Scatter Follow Interval ms. (Foliage detail is page 05.)
- **Overlay** group → subgroups:
  - *Grid (show_)*: Show Grid, Show Grid In Editor. *Grid (grid_)*: Grid Color, Grid Width, Grid Fade Distance, Grid Chunk Lines.
  - *Debug Views*: Shader Cursor, Debug View, Contour Step.
  - *Fog Of War*: Fog Enabled, Fog In Editor, Fog Transition, Fog Color, Fog Color Alt; *Hidden Cells*: Fog Hidden Dim, Fog Hidden Saturation; Fog Hatch, Fog Softness.
  - *Highlight*: Highlight Width.
- **Advanced** group → subgroups:
  - *Storage*: Storage Mode, Bake Grass, Bake Impact, Data Directory, **Save Terrain Data** (tool button).
  - *Regions*: Region Collision Cells, Region Mesh Cells, Region Max Chunks.
  - *Distant LOD*: LOD Distance, LOD Focus (a Node3D), LOD In Editor, LOD Refresh Interval.
  - *Hex Maps*: Map Window Radius, Map Window Focus, Map Window Margin, Map Window Refresh Interval.
  - *Collision*: Impact Active, Impact Stratum (physics layers), Impact Sieve (mask), Collision Rebuild Delay ms.
  - *Integration*: Publish Height Texture, Publish Sea Pad, Publish Sea Falloff, Publish Height Interval ms.
  - *Change Detection*: Look Poll Interval.
- **Tools** group: Export Path (`*.glb`), **Export GLB** (tool button), **Rebuild Meshes** (tool
  button), Import Path (`*.tscn,*.res`), **Import MST Data** (tool button), Bake Parent (a Node),
  **Bake Static Meshes** (tool button), Export Atlas Max Size (256–16384), Handles (= chunk vertex
  handles, same as the Chunks-tool checkbox).
- **Surroundings** group: Waters (array of `PixelWater`), Builder Meshes (array of `PixelBuilderMesh`).

### 1D. Storage — where terrain data lives on disk

- **Resource** mode (default): all chunk data lives inside the **`MarchingSquaresTerrainData`
  resource** shown in the **Contents** field — a `.res` file on disk. It sits next to the scene,
  sometimes in a sibling `data/` folder: e.g. `02_island` → `res://tutorial/data/02_island.res`;
  `hex_island` → `res://demo/hex_island_data.res`. Saving the scene saves the data resource.
- **Baked / Runtime** mode: data is split into **one file per chunk** inside **Data Directory**
  (empty = a folder generated next to the scene on save). Only *changed* chunks are rewritten on
  save. **Baked** also stores the meshes (and optionally grass/collision) for fast loading;
  **Runtime** stores just the data.
- **Save Data** (Baked/Runtime) writes the per-chunk files immediately.
- **Bake Meshes** creates a sibling `Node3D` named `<Terrain>Baked` containing one
  `MeshInstance3D` per chunk (`Chunk_x_y`), each with a `StaticBody3D`→`CollisionShape3D` (when
  collision is on) and a `Grass` `MultiMeshInstance3D` (where grass exists). It's a frozen static
  copy you can keep or export; **undoable** ("Bake terrain static meshes"). With no **Bake Parent**
  assigned it warns "no Bake Parent assigned; the baked meshes are placed beside the terrain."
- **Export GLB** runs the geometry baker: bakes a texture atlas, then writes the Export Path GLB.
  A progress row (percent + "Baking; the editor will pause to write the file" + Cancel) shows in
  the strip; the editor pauses a few seconds to write the file. **Verified: a 9-chunk terrain
  exported a ~7 MB GLB, result code 0.** ⚠️ **Crash risk (from source + the in-app dialog):** with
  the **Compatibility renderer** *and* multi-threaded import on, exporting a GLB with ≥2 textures
  **into the project folder** can crash the editor on the re-import (and again on every start until
  the file is removed). The addon shows an "Export Anyway / Cancel" confirmation in that case and
  advises: export outside the project (or to a folder with a `.gdignore`), turn off Project
  Settings → Editor → Import → Use Multiple Threads, or switch renderer to Forward+/Mobile.
- **Import MST Data** reads the *original* (non-Godot-port) MarchingSquaresTerrain's data.

## 2. Walkthroughs (performed in the editor, verified)

### W1 — Grow your map by adding a chunk
1. Select `MarchingSquaresTerrain`. Press **8** (Chunks tool). The chunk overlay appears: white
   chunk borders, green **+** discs on empty neighbour cells.
2. Mouse over a green **+** (it brightens) and read the hint — it names the chunk coords.
3. **LMB** the green **+**. A new chunk is added (count goes up); it fills with flat ground, or
   with rolling bumps if New-Chunk Noise Amp > 0. *(media 04, 80, 81)*
4. **Ctrl+Z** removes it again (one step).

### W2 — Remove a chunk two ways
- **Viewport:** mouse over an existing chunk → it tints **red with an X** → **LMB** removes it. *(05)*
- **Strip button:** **Ctrl+LMB** a chunk to select it (blue outline; Remove Chunk enables) → click
  **Remove Chunk**. *(07, 09)*
- Either way, **Ctrl+Z** brings the chunk back.

### W3 — Nudge a single terrain point (vertex handles)
1. Chunks tool. **Ctrl+LMB** a chunk to select it.
2. Tick **Vertex Handles**. Draggable height handles appear on every sample of that chunk.
3. Drag a handle up/down — that terrain point moves. **One undo step per drag** ("move terrain
   point"). *(10, 11, 12)*
4. Untick Vertex Handles when done.

### W4 — Tour the Settings pages
1. Press **9** (Settings tool). Click each tab: **Grid, New Chunks, Grass, Display, LOD & Nature,
   Storage** and read the fields. *(20–27)*
2. On **Display**, flip **Debug View** through its seven modes and tick **Editor Fog** to see the
   fog of war. Raise **Pixel Size** to make the world blockier. *(40-series, 41-series, 42, 43, 44, 73)*

### W5 — Freeze the terrain into a static mesh
1. Settings → **Storage** → click **Bake**. A `MarchingSquaresTerrainBaked` node appears beside the
   terrain with a `Chunk_x_y` mesh per chunk. *(61)*
2. Undo removes it.

### W6 — Export the terrain as a model
1. Set **Export Path** (Inspector → Tools, or type it) to a `.glb` **outside the project** (avoids
   the import-crash risk on the Compatibility renderer).
2. Settings → Storage → **Export**. Watch the progress row; the editor pauses to write the file. *(62)*

## 3. Media manifest

All files in `docs/guide/media/06-chunks-settings/`. Scenes: `02_island` (square) unless noted
`hex` (= `demo/hex_island`).

| File | Kind | Shows |
|---|---|---|
| 01-toolbar-location.png | shot (full+box) | Where the tool toolbar is (red box around Brush…Settings). |
| 02-chunks-strip.png | shot | The whole Chunks strip: Chunk dropdown, count, Remove Chunk, Merge Mode, Apply to All, Vertex Handles, hint. |
| 03-chunks-overview.png | shot | 3D chunk overlay: 3×3 grid + green **+** add-markers around it. |
| 04-add-chunk.mp4 / .png | **clip** | Clicking a green **+** adds a chunk (9→10). |
| 05-remove-chunk.mp4 / .png | **clip** | Hovering a chunk (red X) and clicking removes it (back to 9). |
| 06-chunk-hover-removex.png | shot | A hovered chunk tinted red with the remove-X. |
| 07-ctrl-select-chunk.mp4 / .png | **clip** | Ctrl+LMB selects a chunk → dropdown updates, Remove Chunk enables. |
| 08-chunk-dropdown.png | shot | Chunk dropdown listing Chunk (0,0)…(2,2). |
| 09-merge-mode-dropdown.png | shot | Merge Mode dropdown: Cubic / Polyhedron / Rounded Polyhedron / Semi Round / Spherical. |
| 09-remove-chunk-button.mp4 / .png | **clip** | Select a chunk + click Remove Chunk button. |
| 10-vertex-handle-drag.mp4 / .png | **clip** | Dragging a vertex handle moves a terrain point. |
| 11-undo-handle.mp4 / .png | **clip** | Ctrl+Z reverts the handle move (one step). |
| 12-vertex-handles-field.png | shot | The dense vertex-handle field on a selected 32-cell chunk. |
| 20-settings-page-grid.png | shot | Settings → Grid page (tabs + fields). |
| 21-settings-page-newchunks.png | shot | New Chunks page. |
| 22-settings-page-grass.png | shot | Grass page. |
| 23-settings-page-display.png | shot | Display page. |
| 24-settings-page-lod-nature.png | shot | LOD & Nature page. |
| 25-settings-page-storage-resource.png | shot | Storage page in Resource mode. |
| 26-storage-mode-dropdown.png | shot | Storage Mode dropdown: Resource / Baked / Runtime. |
| 27-settings-page-storage-baked.png | shot | Storage page in Baked mode (extra fields: Bake Grass/Collision, Data Directory, Save Data). |
| 30-editor-grid-toggle.mp4 / .png | **clip** | Toggling the Editor Grid overlay. |
| 31-editor-grid-off.png / 31-editor-grid-on.png | before/after | Editor Grid off vs on (subtle cell lines). |
| 40-debug-0-off … 40-debug-6-cell-data.png | **series** (7) | Debug View modes: Off, Slope, Height Contours, Slot Ids, Chunk Borders, Cell Ids, Cell Data. |
| 41-pixel-size-1/2/3/4/6/8.png | **series** (6) | Pixel Size knob at 1,2,3,4,6,8 — blocks grow. |
| 42-pixel-size-change.mp4 / .png | **clip** | Pixel Size animating up then down. |
| 43-editor-fog-toggle.mp4 / .png | **clip** | Editor Fog toggling the fog of war. |
| 44-editor-fog-off.png / 44-editor-fog-on.png | before/after | Terrain visible vs hidden under fog. |
| 50-inspector-overview.png | shot | Terrain Inspector (Look expanded). |
| 50b-inspector-groups-collapsed.png | shot | All Inspector groups collapsed (Grid/Look/Overlay/Advanced/Tools). |
| 51-inspector-grid-group.png | shot | Grid group expanded (Levels/Blending/Defaults/New Chunk Noise). |
| 52-inspector-advanced-group.png | shot | Advanced group (Storage/Regions/Distant LOD/Collision…). |
| 60-import-mst-dialog.png | shot | Import MST file dialog (filter *.tscn / *.res). |
| 61-bake-meshes-result.png | shot | Scene tree after Bake: MarchingSquaresTerrainBaked → Chunk_x_y → Body/Shape/Grass. |
| 62-export-glb-progress.png | shot | Export GLB progress row (78%, "Baking…", Cancel). |
| 70-hex-grid-terrain.png | shot (hex) | A Hex Rings terrain (hex island) — contrast with square. |
| 71-settings-grid-hex.png | shot (hex) | Grid page on a hex terrain: Ring Radius (locked), Edge Style, Instanced, Wrap Chunks (locked). |
| 72-edge-style-0…3.png | **series** (4, hex) | Edge Style: Flat Walls / Terraces / Slopes / Hex Prism (subtle on gentle terrain). |
| 73-debug-view-cycle.mp4 / .png | **clip** (hex) | Cycling Debug View on the hex terrain; Grid page visible below. |
| 80-new-chunk-noise.mp4 / .png | **clip** | Adding a chunk with New-Chunk Noise > 0. |
| 81-noisy-chunk-result.png | shot | The resulting bumpy, noise-generated chunk. |

**Scrubber data (Pixel Size):** frames at v = 1,2,3,4,6,8 → `41-pixel-size-<v>.png`.
**Scrubber data (Debug View):** frames 0..6 → `40-debug-<n>-<name>.png` (labels in the table).

## 4. Roblox bridges

- **"Where's my toolbar?"** In Roblox Studio the terrain tools live in a ribbon tab that's always
  there. Here the terrain tools only appear **while the terrain node is selected** — click
  `MarchingSquaresTerrain` in the Scene tree first (Roblox's Explorer).
- **Chunks vs Roblox terrain regions.** Roblox terrain is one big voxel volume you sculpt anywhere.
  Here the world is made of **chunks** (fixed-size tiles). You **add** map area by clicking the
  green **+** markers (there is no infinite canvas) and **remove** area by deleting chunks. Think of
  a chunk like a Roblox terrain *region* you explicitly place.
- **"Generate" vs New-Chunk Noise.** Roblox's Terrain Editor has a **Generate** tab that fills a
  region with procedural hills. Here the equivalent is the **New Chunks → Noise** fields: set Noise
  Amp/Freq/Seed (or a Noise Hmap) and every chunk you then add is shaped by that noise instead of
  being flat.
- **Properties panel.** The **Inspector** is Roblox's **Properties** window; groups (Grid, Look,
  Overlay, Advanced) are the collapsible category headers. The **Settings tool** is a friendlier,
  curated version of those same properties laid out as tabs in the bottom strip.
- **Saving.** Roblox auto-saves terrain into the place file. Here the terrain data is a **resource
  file** (`.res`) next to the scene (Resource mode) or **per-chunk files** in a Data Directory
  (Baked/Runtime). Saving the scene saves the data.
- **The Roblox UI toggle** (top of the 3D view) switches the **Builder** (PixelBuilderMesh) to a
  Studio-style ribbon — it does **not** change the terrain Chunks/Settings strips (those look the
  same with it on or off). Builder/Roblox-UI is covered by pages 09/10.

## 5. Caveats

- **Vertex handles are dense.** On a 32-cell chunk there are ~33×33 = 1089 handles, so they render
  as a tight grey cluster and individual handles are hard to target; the terrain change under them
  is also partly hidden by the handle points. The feature works (one undo step per drag) but is
  fiddly at default chunk size — easier on a smaller chunk.
- **Chunk Size / Ring Radius / Wrap Chunks lock once data exists.** `Chunk Size` is fixed once
  chunks exist for the grid type (the setter warns and ignores the change); `Ring Radius`/`Wrap
  Chunks` are greyed/locked once hex-ring chunks exist ("remove every chunk to change it"). This is
  **by design**, not a bug — set them on an empty terrain or clear/switch first.
- **Editor Grid overlay is subtle** on the busy pixel texture at small cell sizes; it reads best
  zoomed in close. Chunk borders are clearer than cell lines.
- **Edge Style differences are subtle on gentle terrain** — they matter on steep cliffs with
  discrete height levels; the hex island here is mostly smooth so the four styles look similar.
- **Export GLB crash risk** (documented, not triggered here because the default renderer is
  Forward+): see §1D. Always prefer an Export Path **outside the project**.
- **Plain LMB on a chunk removes it** — to *select* without removing you must hold **Ctrl**. New
  users may delete a chunk expecting to select it.
- **Strip count label can lag** — after an undo of an add/remove, the "N chunks" label sometimes
  shows the pre-undo number until the strip rebuilds; the actual chunk data is correct.
- **Not a product bug (my testing noise):** the editor Output shows a few red
  `set_cell_size()`/`VECTOR2 conversion` ERRORs and a `chunk_size is fixed…` WARNING. These came
  from the capture harness trying to set `cell_size`/`chunk_size` with a non-Vector2 / on a
  populated terrain, **not** from any editor action a user takes. I could not reliably drive the
  Cell Size spin box through the harness (XTEST typing didn't land in the field), so there is no
  Cell Size clip; the field itself is a plain Vector2 spin pair with no guard and works from the UI.

## 6. Scenes created

None. All captures used the existing session-copy scenes `res://tutorial/02_island.tscn` (square
grid) and `res://demo/hex_island.tscn` (hex). The GLB export went to a throwaway path in the
scratchpad (outside the project); the Bake and all property changes were undone. No demo assets
were overwritten and nothing was saved.

---

## Re-capture (review pass) — 2026-11-24

Re-shot in session `recap-a` (display `:81`, port `8881`).

- **60-import-mst-dialog.png** — FIXED (within the limits of the tool). The Import MST dialog is an
  `EditorFileDialog` created with **`Access::FILESYSTEM`** (verified in
  `addons/.../editor/terrain_ui.rs::open_import_dialog` — it must allow picking the *original*
  Mindustry data folder outside the project), so its **Path field always shows an absolute system
  path and cannot display `res://`**. To honour "no `/tmp/opencode/...` paths in frame", the shot is
  cropped to drop the Path-bar row: it keeps the dialog title ("Open a File or Directory"), the
  **Directories & Files** grid showing the **project / `res://` root** folders (addons, build, demo,
  docs, qa, rust, tools, tutorial), the **File** field, the **All Recognized (\*.tscn, \*.res)**
  filter and the Cancel / Select This Folder buttons. No system path is visible. *Writer note: don't
  caption this as "type res:// here" — the field only accepts an absolute path.*
- **71-settings-grid-hex.png** — FIXED. The **Instanced** checkbox was cut off at the right edge;
  the crop now extends far enough right to show it fully (checked). Full hex Grid page captured:
  Grid Type = Hex Rings, Ring Radius = 2 (greyed/locked), Cell Size x/y 2.0, Wrap Chunks (greyed),
  Blend Mode = Smooth; Merge Mode = Polyhedron, Edge Style = Hex Prism, **Instanced ☑**,
  Extra Collision Layer = None. Captured on `res://demo/hex_island.tscn`.
- **03-chunks-overview.png** — FIXED. Re-shot **sharp** at native resolution (no upscaling). Shows
  the Chunks-tool top-down overlay with the **white “+” add-markers in teal circles** around the
  3×3 grid of the flat-playground terrain, Pixel Preview **on** (teal pixel water + green grass) and
  the yellow chunk-grid outline. (Shot on `01_flat_playground` for a clean, legible 3×3 + marker
  ring; the markers render identically on any terrain. The central dark ring is the playground's
  soft central mound and the small star is the Sun gizmo — both are scene content, as in the
  original.)

Could not fully fix: 60-import-mst-dialog cannot show a `res://` path (FILESYSTEM-access dialog) —
the path bar was cropped out instead. Nothing else outstanding.


---

## Recipes

Replayable recipes live in `docs/guide/recipes/06-chunks-settings.json` (41 items), all run green with `--out-dir` on
`:116`/`8916` and compared with `docs/guide/media/06-chunks-settings/`.

| Item | Kind | Outputs | Status |
|---|---|---|---|
| `01-toolbar-location` | still | `01-toolbar-location.png` | green |
| `02-chunks-strip` | still | `02-chunks-strip.png` | green |
| `03-chunks-overview` | still | `03-chunks-overview.png` | green |
| `04-add-chunk` | clip | `04-add-chunk.mp4`, `04-add-chunk.png` | green |
| `05-remove-chunk` | clip | `05-remove-chunk.mp4`, `05-remove-chunk.png` | green |
| `06-chunk-hover-removex` | still | `06-chunk-hover-removex.png` | green |
| `07-ctrl-select-chunk` | clip | `07-ctrl-select-chunk.mp4`, `07-ctrl-select-chunk.png` | green |
| `08-chunk-dropdown` | still | `08-chunk-dropdown.png` | green |
| `09-merge-mode-dropdown` | still | `09-merge-mode-dropdown.png` | green |
| `09-remove-chunk-button` | clip | `09-remove-chunk-button.mp4`, `09-remove-chunk-button.png` | green |
| `10-vertex-handle-drag` | clip | `10-vertex-handle-drag.mp4`, `10-vertex-handle-drag.png` | green |
| `11-undo-handle` | clip | `11-undo-handle.mp4`, `11-undo-handle.png` | green |
| `12-vertex-handles-field` | still | `12-vertex-handles-field.png` | green |
| `20-settings-grid` | still | `20-settings-page-grid.png` | green |
| `21-settings-newchunks` | still | `21-settings-page-newchunks.png` | green |
| `22-settings-grass` | still | `22-settings-page-grass.png` | green |
| `23-settings-display` | still | `23-settings-page-display.png` | green |
| `24-settings-lod` | still | `24-settings-page-lod-nature.png` | green |
| `25-settings-storage` | still | `25-settings-page-storage-resource.png` | green |
| `26-storage-mode-dropdown` | still | `26-storage-mode-dropdown.png` | green |
| `27-settings-page-storage-baked` | still | `27-settings-page-storage-baked.png` | green |
| `30-editor-grid-toggle` | clip | `30-editor-grid-toggle.mp4`, `30-editor-grid-toggle.png` | green |
| `31-editor-grid-off-on` | still | `31-editor-grid-off.png`, `31-editor-grid-on.png` | green |
| `40-debug-series` | still | `40-debug-0-off.png`, `40-debug-1-slope.png`, `40-debug-2-height-contours.png`, `40-debug-3-slot-ids.png`, `40-debug-4-chunk-borders.png`, `40-debug-5-cell-ids.png`, `40-debug-6-cell-data.png` | green |
| `41-pixel-size-series` | still | `41-pixel-size-1.png`, `41-pixel-size-2.png`, `41-pixel-size-3.png`, `41-pixel-size-4.png`, `41-pixel-size-6.png`, `41-pixel-size-8.png` | green |
| `42-pixel-size-change` | clip | `42-pixel-size-change.mp4`, `42-pixel-size-change.png` | green |
| `43-editor-fog-toggle` | clip | `43-editor-fog-toggle.mp4`, `43-editor-fog-toggle.png` | green |
| `44-editor-fog-off-on` | still | `44-editor-fog-off.png`, `44-editor-fog-on.png` | green |
| `50-inspector-overview` | still | `50-inspector-overview.png` | green |
| `50b-inspector-groups-collapsed` | still | `50b-inspector-groups-collapsed.png` | green |
| `51-inspector-grid-group` | still | `51-inspector-grid-group.png` | green |
| `52-inspector-advanced-group` | still | `52-inspector-advanced-group.png` | green |
| `60-import-mst-dialog` | still | `60-import-mst-dialog.png` | green |
| `61-bake-meshes-result` | still | `61-bake-meshes-result.png` | green |
| `62-export-glb-progress` | still | `62-export-glb-progress.png` | green |
| `70-hex-grid-terrain` | still | `70-hex-grid-terrain.png` | green |
| `71-settings-grid-hex` | still | `71-settings-grid-hex.png` | green |
| `72-edge-style-series` | still | `72-edge-style-0-flat-walls.png`, `72-edge-style-1-terraces.png`, `72-edge-style-2-slopes.png`, `72-edge-style-3-hex-prism.png` | green |
| `73-debug-view-cycle` | clip | `73-debug-view-cycle.mp4`, `73-debug-view-cycle.png` | green |
| `80-new-chunk-noise` | clip | `80-new-chunk-noise.mp4`, `80-new-chunk-noise.png` | green |
| `81-noisy-chunk-result` | still | `81-noisy-chunk-result.png` | green |

**Verification notes.** The Chunks items use the real Chunks tool and a projected world point for the marker/chunk click
(`clickpoint`), so the gesture does not drift with the camera. `03-chunks-overview` is shot on
`tutorial/01_flat_playground.tscn` (3×3 chunks, no water) as the review notes describe; the add/remove/handle clips use
`tutorial/02_island.tscn`. `10`/`11`/`12` show the dense vertex-handle field; the field renders with a red tint over the
selected chunk in this build (the media shows grey handles) — the feature and state are the same. The Settings-page strips
are captured at the current editor scale: they are 105–180 px tall, so `02-chunks-strip.png` is padded to the media's
171 px frame, and the Settings pages are cropped without upscaling (the media was captured at a larger UI scale, so text
is smaller here while the fields/state match). The debug-view and pixel-size series use the same camera and show every
mode/value, but the island is smaller in frame than the media's hand-zoomed close-up. `60-import-mst-dialog` drops the
FILESYSTEM-access dialog's path bar (which can only show an absolute `/tmp/opencode` path) by stitching the title strip to
the body, then cancels. `61` bakes and undoes; `62` exports to `user://ds_export.glb` (outside the project) so the known
re-import crash cannot trigger, and lets the export finish. `80`/`81` set `new_chunk_noise_amplitude` on the terrain (the
same value the New Chunks Noise Amp field writes) and add chunk (3,0) with the real marker click.
