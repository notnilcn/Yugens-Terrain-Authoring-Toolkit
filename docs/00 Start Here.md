# Start Here

Welcome! This folder is a small manual for **MarchingSquaresTerrain**, the terrain-and-pixel-art addon used by our game. It is written for someone who has never used Godot or Blender. You do not need to know how to code.

The short version: you will open a Godot project, click on the terrain, and use the toolbar on the left of the 3D view to **sculpt ground, paint textures, plant grass and trees, place props, and set the sky and weather**. A separate set of nodes turns the whole 3D scene into crisp pixel art.

![[hero-island.png]]

## What is in this vault

Read these in order the first time. Each note is short and has pictures or a video.

| Note | What you will learn |
|---|---|
| [[01 Meet Godot]] | The five parts of the Godot window, moving the camera, selecting things, and pressing Play. |
| [[02 Your First Terrain]] | Open the project, find the terrain, and make a hill. |
| [[03 Sculpting]] | The Brush, Level, Smooth and Bridge tools. |
| [[04 Painting the Ground]] | The 16 texture slots, texture presets, and painting cliffs and paths. |
| [[05 Plants, Trees and Props]] | Grass, trees, rocks, flowers, and imported 3D models. |
| [[06 Stamps, Ramps and Copying]] | Stamp mountains, paint gradients, copy and paste terrain, and tint areas. |
| [[07 The Pixel Look]] | The PixelRenderer, outlines, palettes, and pixel objects. |
| [[08 Sky, Time and Weather]] | Clouds, day/night, rain, snow, lightning, and fog. |
| [[09 Water, Fire and Atmosphere]] | Water, waterfalls, fireflies, birds, smoke, fire, dust, and light shafts. |
| [[10 Making Props in the Builder]] | Build a simple prop inside Godot, without Blender. |
| [[11 Example Scenes]] | The practice scenes that ship with this manual. |
| [[12 Cheat Sheet]] | Every important shortcut and setting on one page. |
| [[13 Troubleshooting]] | What to do when something is invisible, gray, or not clickable. |
| [[14 Setting Up a New Project]] | Copy the addon into a fresh project. Most artists never need this. |

## The three things you will use most

1. **The Scene tree** (top-left). It lists everything in the scene. Click a name to select it. The terrain is usually called `MarchingSquaresTerrain`.
2. **The 3D view** (the big middle area). This is where you sculpt. When the terrain is selected, a toolbar appears on its left edge.
3. **The Inspector** (right side). It shows the settings of whatever is selected. This is where you change the pixel size, sky, water, and so on.

> [!tip]
> If the terrain toolbar is missing, you almost certainly have something else selected. Click `MarchingSquaresTerrain` in the Scene tree and it will appear.

## A note about “nodes”

Godot scenes are built from **nodes**. A node is a small object with a job: a camera, a light, a terrain, a sound. Nodes are arranged in a tree, and a node can contain other nodes. You do not have to create nodes from scratch most of the time — the example scenes already have the right ones. When you do need a new one, you will use the **+** button at the top of the Scene tree.

## What “pixel art” means here

The addon does not make the models look blocky by changing the models. Instead, a node called **PixelRenderer** draws the 3D scene at a low resolution (for example, 426 × 240 pixels), then scales that image up without blurring it. It also adds dark outlines around objects. The result looks like a 3D pixel-art game, while the UI and text stay sharp.

![[pixel-look-example.png]]

## The golden rule

**Save before you experiment.** `Ctrl+S` saves the scene. `Ctrl+Z` undoes one stroke. Terrain strokes, foliage strokes, stamps, and paint strokes are each one undo step, so you can be brave.

> [!warning]
> Do not edit files inside `addons/MarchingSquaresTerrain/`. That folder is the addon itself. If something there breaks, the whole editor can break. Make your scenes and assets outside of it.

## If you get lost

Go to [[13 Troubleshooting]] or press `F1` in the running game to show the controls again. The demo also has an `F3` diagnostics panel that can change the time of day and weather while the game is running.

## How to read this in Obsidian

Open the `docs` folder as a vault in Obsidian. The pictures and videos are in `docs/media/`; the `![[...]]` embeds play inline. If a video does not play, click it once; Obsidian uses the system video player.

Next: [[01 Meet Godot]]
