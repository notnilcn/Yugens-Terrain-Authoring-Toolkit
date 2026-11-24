# Capture notes — page `02-first-world` (Area C01 — Basics)

Source of truth for the writer. Verified in the **real Godot editor** (session `docs-basics`,
display `:71`, port `8871`, Godot **4.7.2.stable**, Mesa/llvmpipe). This page is the end-to-end
"build your first world from scratch and run it" quick start. Every step below was actually
performed; the finished scene is `res://tutorial/guide/first_world.tscn` (copied into the repo at
`tutorial/guide/first_world.tscn`) and it **runs** (F6) to a pixel-art island — see
`11-run-game.png`.

> **Order note for the writer.** The brief suggests sculpt → paint → foliage → *then* Pixel World.
> In practice a brand-new terrain has **no lighting** (dark, hard to see what you're sculpting), so
> this capture sets up **Pixel World first** (it adds a Sun + sky + the pixel renderer, lighting
> everything), then sculpts/paints/foliages against a lit, pretty scene. The page can present
> either order, but if it keeps the brief's order it must warn that the scene is dark until the
> Sun is added. Also: a fresh terrain starts **empty (0 chunks)** — you must add a chunk before you
> can sculpt. Both facts are easy to miss.

---

## 1. The build, step by step (feature inventory folded in)

### Step 0 — New 3D scene
- **Scene → New Scene** → the Scene dock shows **Create Root Node**: four buttons — **2D Scene**,
  **3D Scene**, **User Interface**, **Other Node**. Click **3D Scene** → a **Node3D** root is
  created. Renamed it **World** (F2). Clip `01-new-scene.mp4`.
- Roblox bridge: the scene root ≈ the Workspace; a Node3D ≈ an empty Model/Folder in 3D space.

### Step 1 — Add the terrain
- Scene dock **+** (Add Child Node, Ctrl+A) → **Create New Node** → search
  `MarchingSquaresTerrain` → **Create**. A `MarchingSquaresTerrain` node appears under World and is
  auto-selected, so the **terrain toolbar** (left, vertical) and the **bottom attribute strip**
  appear. Clip `02-add-terrain.mp4`. Final tree `40-scene-tree.png`.
- A fresh terrain's **Inspector** already has a **Contents** = auto-created
  `MarchingSquaresTerrainData` and a **Tex Assign** = default texture set, Grid Type **Square**,
  Chunk Size **32**, Cell Size **2.0 × 2.0** — but **0 chunks** (a ⚠ config-warning shows on the
  node). Nothing is visible/sculptable yet.

### Step 2 — Add chunks (the missing first step)
- Left toolbar **Chunks** (key **8**). Strip = **Chunk Manager**: a **Chunk** dropdown, chunk
  count, **Remove Chunk**, **Merge Mode** (Polyhedron), **Apply Mode to All Chunks**,
  **Vertex Handles**. Hint: *"LMB on a green +: add a chunk · LMB on a chunk: remove it ·
  Ctrl+LMB: select a chunk."*
- A top-down view shows **green "+" markers** at chunk slots. **LMB a green marker** adds that
  chunk (it fills in with grass immediately). Added **4 chunks** (a 2×2, 128 m × 128 m). Clip
  `03-add-chunks.mp4`, strip `42-chunks-strip.png`.
- Gotcha: LMB on an **existing** chunk **removes** it; only click green markers. Ctrl+LMB selects.

### Step 3 — Pixel World (renderer, sky, day/night, water)
- Toolbar **Pixel World** is a drop-down **Add menu**, not a dialog. Full list
  (`20-pixelworld-menu.png`): **Renderer, Sky & Clouds, Day / Night, Weather, Water, Fireflies,
  Particle Field, Birds, Smoke & Fire, Light Volume, Sun Shafts, Pixel Object, Animation Stepper,
  Diagnostics, Erosion, Wild Flowers, Grass Fire**; then **Lighting Preset**: Dawn, Noon, Dusk,
  Night, Storm, Snow; then **Wire Sun / Env Refs**, and **Features →**.
- Added, in order (clip `21-pixelworld-add.mp4`):
  - **Renderer** → adds a **PixelRenderer** node **and a Camera**, pixelates the view, and opens
    two surfaces: a right-side **Pixel World dock** (rows Renderer/Sky & Clouds/Day-Night/Weather/
    Water/Fireflies/… each with **Select** or **+ Add**) and a bottom **Renderer settings strip**
    (tabs Renderer/Outlines/Color·Palette/Rays/Sun/Sheets/Clouds/Depth Fade/Debug Views; Enabled,
    Mode = Anchor, **Pixel Size = 4**, Fixed Height, Texel Snap, Sub-texel Scroll, Crisp Edges,
    Depth Range = 200, Obj. Pixel).
  - **Sky & Clouds** → adds a **Sun** (directional light) **and a WorldEnvironment** (sky) — this
    is what finally lights the scene.
  - **Day / Night** → adds a **DayNight** node (time-of-day driver).
  - **Water** → adds a **Water** node (a water plane; the flat terrain at height 0 now sits at
    water level, which is what makes the land read as an **island**).
- You can add a component either from this menu **or** from the dock's **+ Add** buttons (same
  result). `Wire Sun / Env Refs` re-connects a component's Sun/Environment references if it shows a
  ⚠ warning. The deeper pixel/atmosphere controls are covered on pages 07 and 08.

### Step 4 — Sculpt a hill (Brush)
- Left toolbar **Brush** (key **1**). Strip: **Brush Type** (Round…), **Brush Mode**, **Size**
  (15 default), Aspect, Rotation, Flatten ✓, Falloff ✓, Peak. `41-brush-strip.png`.
- Set **Brush Mode → Raise / Lower** (the Roblox-like mode). Strip then shows **Ceiling** (16),
  **Height** (0), **Floor** (−8), **Strength** (≈4). Hint: *"Hold LMB: raise · RMB: lower ·
  LMB+RMB: flatten · Ctrl+LMB/MMB/RMB: pick Ceiling/Height/Floor · Shift+wheel: size ·
  Ctrl+wheel: strength."*
- **Held LMB** over the centre raised a hill (verified: a cell's height went 0 → ~10.6 with a
  smooth falloff). Clip `04-sculpt-hill.mp4`.
- (The other Brush mode, **Drag**, paints a cell selection then raises the whole selection; page 03
  covers all sculpt tools in depth.)

### Step 5 — Flatten an area (Level)
- **Level** (key **2**). Strip: Brush Type, Size, Aspect, Rotation, **Height** (target), Falloff,
  Peak. Hint: *"LMB-drag: level to Height · Ctrl+LMB: pick the hovered height."* LMB-dragging over
  the hill flattened it toward Height 0 (a plateau). Verified: digest changed. Clip
  `05-level-flatten.mp4`.

### Step 6 — Paint ground (Paint / Vertex Painting)
- **Paint** (key **6**). Strip: **Material** dropdown with 16 ground textures — **1 Grass,
  2 Meadow, 3 Forest Floor, 4 Sand, 5 Dirt, 6 Cliff, 7 Dark Rock, 8 Snow, 9 Gravel, 10 Clay,
  11 Moss, 12 Path, 13 Stone Tiles, 14 Ash, 15 Ice, 16 Void** — plus Texture Name, Texture Preset,
  Paint Walls, Slope Min/Max. A right-side **Textures** panel appears (per-slot Pattern, Scale,
  Color, Grass toggle + sprite). `43-paint-strip.png`.
- Picked **4. Sand** and LMB-dragged to paint a sandy patch. Hint: *"LMB-drag: paint the Material ·
  Ctrl+LMB: pick the material under the cursor · Shift+wheel: size."* Verified: digest changed.
  Clip `06-paint.mp4`. (Painting is covered fully on page 04.)

### Step 7 — Scatter foliage (Foliage)
- **Foliage** (key **5**). Strip: Size, **Density** (0.253), Slope Min/Max, **Erase All**,
  **Fill Terrain**, **Clear**; a right-side **palette** of props: **Grass, Tree** (selected),
  **+ Bush, + Flowers, + Pine, + Rock**. Hint: *"1. Click what to paint below. 2. LMB-drag on the
  terrain to paint it. RMB/Shift+LMB: erase. Shift+wheel: brush size."* `44-foliage-strip.png`.
- LMB-dragged to scatter **Tree** props (138 props placed, verified via scatter digest). Big 3-D
  trees appeared. Clip `07-foliage.mp4`. (Foliage is covered fully on page 05.)

### Step 8 — Toggle Pixel Preview
- Toolbar **Pixel Preview** toggles the final pixel-art crunch in the editor viewport. Off vs on =
  smoother vs chunkier pixels + dithered edges. Clip `08-pixel-preview.mp4`; before/after
  `30-pixelpreview-off.png` / `31-pixelpreview-on.png`. (The pixel look always previews once a
  PixelRenderer exists; this toggle controls the final-resolution post.)

### Step 9 — The camera
- You do **not** have to add a `Camera3D` by hand: adding the **Renderer** in Step 3 already added
  a **Camera**. Selecting it shows **Preview** / **Pilot** toggles at the top-left of the 3D view
  (look through / fly the camera). The running game uses this camera, framing the island nicely
  (see Step 11). If you ever want a hand-placed camera, Create New Node → `Camera3D` and move it
  with the gizmo (page 01).

### Step 10 — Save
- **Ctrl+S** → **Save Scene As…** dialog (`20-save-dialog.png`). Set the **Path** to
  `res://tutorial/guide/`, the **File** to `first_world.tscn`, **Save**. Clip `10-save.mp4`. The
  scene is **self-contained**: all 18 external resources are addon textures/shaders; the terrain's
  chunk/height/paint/foliage data is embedded in the `.tscn` as sub-resources (no side `.res`).

### Step 11 — Run it (F6)
- Mouse over the 3D view, **F6** (Run Current Scene). A separate game window opens (1280×720),
  shows the Godot splash, then the world: a **green pixel-art island** (the 2×2 terrain) in a blue
  sea under a dawn-gradient sky, with the sand patch and trees at its centre — `11-run-game.png`,
  clip `11-run-game.mp4`. **F8** stops. No errors on run or stop.

---

## 2. Media manifest

Clips (`.mp4`, each has a `.png` poster):

| File | Shows |
|---|---|
| `01-new-scene.mp4` | Scene → New Scene → 3D Scene root (Node3D). |
| `02-add-terrain.mp4` | + → Create New Node → MarchingSquaresTerrain → Create. |
| `03-add-chunks.mp4` | Chunks tool: clicking green +markers to add chunks. |
| `21-pixelworld-add.mp4` | Pixel World Add menu: adding Sky & Clouds, Day/Night, Water. |
| `04-sculpt-hill.mp4` | Brush (Raise/Lower): hold LMB to raise a hill. |
| `05-level-flatten.mp4` | Level: LMB-drag to flatten toward Height. |
| `06-paint.mp4` | Paint: dragging the Sand material onto the ground. |
| `07-foliage.mp4` | Foliage: dragging to scatter trees (138 props). |
| `08-pixel-preview.mp4` | Pixel Preview toggle (pixel-art crunch on/off). |
| `10-save.mp4` | Save Scene As dialog + folder navigation. |
| `11-run-game.mp4` | The running game: the pixel island in the sea. |

Stills:

| File | Shows |
|---|---|
| `20-pixelworld-menu.png` | The full Pixel World Add menu (all components + lighting presets). |
| `20-save-dialog.png` | Save Scene As dialog. |
| `30-pixelpreview-off.png` / `31-pixelpreview-on.png` | Before/after compare of Pixel Preview. |
| `40-scene-tree.png` | Final scene tree (World + terrain, Camera, PixelRenderer, Sun, WorldEnvironment, DayNight, Water, Birds). |
| `41-brush-strip.png` | Brush attribute strip. |
| `42-chunks-strip.png` | Chunk Manager strip. |
| `43-paint-strip.png` | Paint strip (Material dropdown, slope, Textures panel edge). |
| `44-foliage-strip.png` | Foliage strip (Density, Fill Terrain, prop palette). |
| `11-run-game.png` | **Hero shot** — the finished first world running. |

Before/after pair: `30-pixelpreview-off.png` ↔ `31-pixelpreview-on.png`.

---

## 3. Roblox bridges (callout seeds)

- "New Scene → **3D Scene** is like opening a fresh baseplate; the root Node3D is your Workspace."
- "Adding a terrain is like dropping in Roblox's Terrain — but you first **add chunks** (the ground
  tiles) by clicking the green + marks."
- "**Brush → Raise/Lower** is Roblox's terrain Grow/Erode: hold the mouse to raise, right-click to
  lower, both to flatten."
- "**Paint** is like Roblox's terrain Paint — pick a material (Sand, Rock, Snow…) and drag."
- "**Foliage** is like scattering Models/decorations — pick Tree/Rock/Flower and drag to plant them."
- "**Pixel World** is a one-click way to add the sky, sun, day/night and water that make it look
  like a finished game — no equivalent in Roblox, it's the addon's signature look."
- "**F6** = Play, **F8** = Stop."

## 4. Caveats (verified)

- **Empty-on-create:** a new `MarchingSquaresTerrain` has 0 chunks — add at least one chunk
  (Chunks tool) before sculpting. The node shows a ⚠ until it has chunks.
- **Dark until lit:** with no Sun/WorldEnvironment the scene is dark; add **Pixel World → Sky &
  Clouds** (or a Lighting Preset) early so you can see your sculpting.
- **Number keys switch terrain tools** (1–9) while the terrain is selected; they do **not** reach
  editor shortcuts. To use an editor shortcut like **Ctrl+1** (1 Viewport), first select a
  non-terrain node (e.g. the World root). (During capture an accidental multi-viewport layout could
  only be undone after deselecting the terrain.)
- **Island look = flat terrain + Water:** the playable chunks sit at height 0, i.e. at the water
  surface, so once Water is added the flat land reads as an island surrounded by sea. Raising
  terrain makes hills above the water; the running game shows this clearly.
- **Infinite Surroundings:** in the editor the terrain blends into endless procedural grassland
  (the MST "Surroundings" feature), which makes it hard to see where your chunks end. The bounded
  island is obvious once you run the game.
- **Water/Birds config warnings:** after adding via the menu, the Water and Birds nodes showed ⚠
  warnings (they want their Sun/Env references wired — `Pixel World → Wire Sun / Env Refs` clears
  them). The scene still ran correctly. The writer can mention Wire Sun / Env Refs as the fix.
- **Capture-tool note (not reader-facing):** the harness in-engine `capture` hangs on a scene with
  no WorldEnvironment/Camera; `shot` (full-screen) works. Also the editor's **View** MenuButton
  popup did not accept synthetic clicks/keys in this harness — a tooling quirk, not an addon bug.
- No addon `ERROR:`/`SCRIPT ERROR:`/panic occurred at any point (log clean).

## 5. Scenes created

- `res://tutorial/guide/first_world.tscn` — the finished first world: World (Node3D) →
  `MarchingSquaresTerrain` (2×2 chunks, a sculpted hill, a leveled patch, a Sand-painted area,
  scattered Tree foliage), `Camera`, `PixelRenderer`, `Sun`, `WorldEnvironment`, `DayNight`,
  `Water`, `Birds`. Self-contained (only addon textures/shaders as external deps). Saved in the
  session copy and **copied into the repo** at `tutorial/guide/first_world.tscn`.

---

## Re-capture (review pass) — 2026-11-24

Re-shot in session `recap-a` (display `:81`, port `8881`). The empty `qa_tmp/` folder was removed
from the session project so it no longer appears in the file dialogs.

- **01-new-scene.png** — FIXED. Previously showed the old `PixelShowcase` scene. Now shows a genuine
  **new empty scene**: a fresh **World** (Node3D) root in the Scene dock with the empty 3D grid
  viewport (created via Scene → New Scene → 3D Scene → F2 rename to `World`).
- **21-pixelworld-add.png** — FIXED. Previously showed a 4-viewport grass view with no menu. Now
  shows the **Pixel World** toolbar button highlighted and its full **Add** dropdown open: Renderer,
  Sky & Clouds, Day / Night, Weather, Water, Fireflies, Particle Field, Birds, Smoke & Fire, Light
  Volume, Sun Shafts, Pixel Object, Animation Stepper, Diagnostics, Erosion, Wild Flowers, Grass
  Fire; then Lighting Preset (Dawn/Noon/Dusk/Night/Storm/Snow); Wire Sun / Env Refs; Project →
  Features. (Output panel cleared first so nothing shows behind the menu.)
- **03-add-chunks.png** — FIXED. Previously a red debug overlay with no clear markers. Now a
  top-down **Top Orthogonal** view of the Chunks tool showing the grid of **green “+” add-markers**
  (teal circles) at the addable chunk slots around the filled chunks, with the tool hint
  "LMB on a green +: add a chunk · LMB on a chunk: remove it · Ctrl+LMB: select a chunk" and the
  Chunk Manager strip (9 chunks, Remove Chunk, Merge Mode). Markers are large and obvious.
- **10-save.png** / **20-save-dialog.png** — FIXED. Both previously showed `world.tscn` at `res://`.
  Both now show the **Save Scene As…** dialog with **Path = `res://tutorial/guide`** and
  **File = `first_world.tscn`**, matching Step 10. The dialog was set up and captured, then
  **Cancel**led — the repo's real `tutorial/guide/first_world.tscn` was **not** overwritten (nothing
  was saved; work was done only in the session copy).

Could not fix / notes: 03-add-chunks was shot on the existing `01_flat_playground` terrain (which
already has 9 chunks) rather than a brand-new 0-chunk terrain, because the add-markers are identical
either way and this was the reliable path; the surrounding slots still show the green “+” markers the
caption needs. The folder listing in the save dialog shows the sibling guide scenes
(greybox_tower, model_uses, …) that already exist in `tutorial/guide/` — that is the real folder
state, not residue.


---

## Recipes

Replayable recipes live in `docs/guide/recipes/02-first-world.json` (22 items), all run green with
`--out-dir` on `:116`/`8916` and compared with `docs/guide/media/02-first-world/`.

| Item | Kind | Outputs | Status |
|---|---|---|---|
| `01-new-scene` | still | `01-new-scene.png` | green |
| `03-add-chunks` | still | `03-add-chunks.png` | green |
| `20-save-dialog` | still | `20-save-dialog.png` | green |
| `10-save` | still | `10-save.png` | green |
| `21-pixelworld-add` | still | `21-pixelworld-add.png` | green |
| `01-new-scene-clip` | clip | `01-new-scene.mp4` | green |
| `02-add-terrain` | clip | `02-add-terrain.mp4`, `02-add-terrain.png` | green |
| `03-add-chunks-clip` | clip | `03-add-chunks.mp4` | green |
| `04-sculpt-hill` | clip | `04-sculpt-hill.mp4`, `04-sculpt-hill.png` | green |
| `05-level-flatten` | clip | `05-level-flatten.mp4`, `05-level-flatten.png` | green |
| `06-paint` | clip | `06-paint.mp4`, `06-paint.png` | green |
| `07-foliage` | clip | `07-foliage.mp4`, `07-foliage.png` | green |
| `08-pixel-preview` | clip | `08-pixel-preview.mp4`, `08-pixel-preview.png` | green |
| `20-pixelworld-menu` | still | `20-pixelworld-menu.png` | green |
| `30-31-pixelpreview-pair` | series | `30-pixelpreview-off.png`, `31-pixelpreview-on.png` | green |
| `10-save-clip` | clip | `10-save.mp4` | green |
| `11-run-game` | clip | `11-run-game.mp4`, `11-run-game.png` | green |
| `40-scene-tree` | still | `40-scene-tree.png` | green |
| `41-brush-strip` | still | `41-brush-strip.png` | green |
| `42-chunks-strip` | still | `42-chunks-strip.png` | green |
| `43-paint-strip` | still | `43-paint-strip.png` | green |
| `44-foliage-strip` | still | `44-foliage-strip.png` | green |

**Verification notes.** `01-new-scene` and `03-add-chunks` stills were rebuilt from the reviewed passes; the clips replay
the same actions. `04`–`07` (sculpt/level/paint/foliage) and `30`/`31` use `tutorial/guide/first_world.tscn` — the finished
island from this page — so the strips and terrain state match the media, while the camera is the scene's default
Perspective view rather than the original hand-framed close-up (the brush/paint/foliage strokes are visible but the
framing is wider than the media). `11-run-game` records the 1280×720 game window at (320,180) after the boot splash.
`10-save` and `20-save-dialog` set Path `res://tutorial/guide`, File `first_world.tscn` and then **Cancel**; no scene is
saved. `40-scene-tree` and `41`–`44` are the Scene dock and tool strips on the finished world.


**Final verification.** The five original stills (`01-new-scene`, `03-add-chunks`, `20-save-dialog`, `10-save`,
`21-pixelworld-add`) were re-run one item at a time (`--only <id>`) because the harness can race a session stop with the
next start; each returned green individually (`done: 1 ok, 0 failed`). The other 17 items were green in the 22-item suite.
Known framing deviation: `30`/`31` (Pixel Preview pair) show the whole framed island from the scene's default camera rather
than the media's close-up; the toggle state and the pixel-crunch difference are visible.

**Final verification note.** The full suite was run on `:116` (17/22 green); the five original items (`01-new-scene`,
`03-add-chunks`, `20-save-dialog`, `10-save`, `21-pixelworld-add`) were then re-run one at a time after a
`qa/session.sh` stop/start race (the runner's immediate session restart occasionally fails with
`rm: cannot remove ... project` while the previous editor is still shutting down — a harness race, not a recipe step).
Each of the five reported `done: 1 ok, 0 failed` on its own clean run, so all 22 items are green.
