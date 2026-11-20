# Your First Terrain

This is the moment where it starts feeling like a game. You will open a practice scene, select the terrain, and raise a hill.

![[first-terrain.mp4]]

## Step 1 — Open the project

1. Start **Godot 4.7**.
2. In the project manager, click **Import**, find the game folder, and choose `project.godot`. (If the project is already in the list, just double-click it.)
3. The editor opens the hex island demo. You can close that tab; we will use a practice scene.

> [!tip]
> The first time the project opens, Godot may spend a minute importing textures. Let it finish. The progress bar is at the bottom-right.

## Step 2 — Open the practice scene

In the **FileSystem** panel (bottom-left), open:

```
tutorial → 01_flat_playground.tscn
```

Double-click it. You should see a green field with a hill, a blue sky, and clouds.

This scene already has everything a terrain needs:

| Node | Job |
|---|---|
| `MarchingSquaresTerrain` | The ground itself |
| `Camera` | The view you see |
| `PixelRenderer` | Turns the 3D scene into pixel art |
| `PixelSky` | Clouds and their shadows |
| `PixelDayNight` | Sun colour and time of day |
| `WorldEnvironment` | Background colour and ambient light |

## Step 3 — Select the terrain

Click `MarchingSquaresTerrain` in the **Scene tree** (top-left).

A vertical toolbar appears on the left edge of the 3D view. This is the terrain toolbox. If you do not see it, you have something else selected.

![[terrain-toolbar.png]]

The toolbar is grouped:

| Group | Tools |
|---|---|
| Sculpt | Brush, Level, Smooth, Bridge |
| Paint | Foliage, Paint |
| Inspect / manage | Inspect, Chunks, Settings |
| Extra | Stamp, Gradient, Select, Tint |
| Bottom | Grid toggle |

At the bottom of the screen, a strip shows the settings for the active tool.

## Step 4 — Make a hill

There are two ways to use the Brush. The default way is **Drag**:

1. Click **Brush** (`1`).
2. Click and drag on the terrain. You are painting a **white selection** of cells, not raising them yet.
3. Release, then click on one of the white cells and drag **up**. The whole selection rises together.
4. Release. One `Ctrl+Z` undoes the whole stroke.

The second way is easier for free-form sculpting. In the Brush strip at the bottom, change **Brush Mode** from `Drag` to `Raise / Lower`.

1. Hold **left mouse button** on the terrain to raise.
2. Hold **right mouse button** to lower.
3. Hold **both** to flatten.
4. `Shift` + mouse wheel changes the brush size.

> [!tip]
> Start with a **Size** around 15 and a **Strength** around 4. You can always make a hill bigger; it is harder to make a giant crater look natural again.

![[brush-strip.png]]

## Step 5 — Smooth it

A raw brush hill looks lumpy. That is normal.

1. Click **Smooth** (`3`).
2. Drag over the hill. It relaxes the ground toward its neighbours.
3. If you want flat steps instead of soft slopes, leave **Smooth Mode** on `Terraced`. If you want ramps, choose `Sloped`.

![[smooth-tool.mp4]]

## Step 6 — Undo and save

- `Ctrl+Z` undoes the last stroke.
- `Ctrl+S` saves the scene.

Because this is a practice scene, use **File → Save Scene As…** and save your own copy, for example `tutorial/my_first_hill.tscn`. That way the original stays clean for the next person.

> [!warning]
> If you save over a scene while it is playing, stop the game first (`F8`).

### Where the terrain is stored

A terrain's heights and paints are stored in a **MarchingSquaresTerrainData** resource. In the example scenes it is a separate `.res` file next to the scene, so the scene stays small. `Ctrl+S` saves both the scene and its terrain data.

If the terrain Inspector says **Storage Mode: Baked** or **Runtime**, the chunks are written as separate files in the **Data Directory**. In those modes, use **Settings → Storage → Save Data** as well as `Ctrl+S`. For normal artist work, leave **Storage Mode** on `Resource`.

> [!tip]
> If you move a scene to another folder, move its `.res` data file too. If you forget, the terrain will be empty when the scene opens.

## What is a chunk?

The terrain is divided into square **chunks** so the editor only rebuilds the part you touched. The practice scene already has 9 chunks (a 3×3 square).

If you ever make a terrain from scratch and it looks empty, it has no chunks. Select the terrain, choose **Chunks** (`8`), and click one of the green **+** discs in the 3D view. New chunks must touch an existing chunk.

![[chunks-panel.png]]

## Making a terrain from scratch (optional)

The practice scenes are the easy path. If you want to build one from nothing:

1. **Scene → New Scene**, choose **3D**.
2. Add a `MarchingSquaresTerrain` node.
3. Select it, choose **Chunks** (`8`), and click the green **+** at the world origin.
4. Add a `Camera3D`, a `DirectionalLight3D`, a `WorldEnvironment`, a `PixelSky`, a `PixelDayNight`, and a `PixelRenderer`.
5. In the terrain's **Inspector**, set **Tex Assign** to `res://addons/MarchingSquaresTerrain/texture_presets/mst_classic.tres` (or any preset you saved).
6. Turn on **Pixel Preview** at the top of the 3D view.

That is a lot of nodes for a first day. Copying an example scene and changing it is faster and is how most artists work.

> [!tip]
> The **Pixel World** menu at the top of the 3D view can add most of these nodes for you, already wired to the sun and camera. Use **Pixel World → Add**.

Next: [[03 Sculpting]]
