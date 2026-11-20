# Example Scenes

There are two kinds of examples in this project:

- **Tutorial scenes** in `tutorial/` — small, clean scenes for learning one thing at a time.
- **Demo scenes** in `demo/` — finished showcase worlds you can play.

Open any scene by double-clicking it in the FileSystem panel, then press `F5`.

## The tutorial scenes

### 01 — Flat Playground

`tutorial/01_flat_playground.tscn`

![[example-01.png]]

A green plateau with a hill, water around it and a blue sky. It is deliberately simple so you can see every brush stroke.

**Try this:** raise a second hill with Brush, flatten its top with Level, and cut a path up it with Bridge.

### 02 — Island

`tutorial/02_island.tscn`

![[example-02.png]]

A small island with a sandy shore, cliffs, trees, rocks, and water with foam and caustics.

**Try this:** paint a dirt path from the beach to the center, add more trees with Foliage, and change the water's Y to flood more of the island.

### 03 — Pixel Look

`tutorial/03_pixel_look.tscn`

![[example-03.png]]

A close-up of a ruin made in the Pixel Builder, wrapped in a PixelObject. This is the scene for studying outlines, palettes and the pixel camera.

**Try this:** turn the PixelRenderer's **Outlines** off and on, change **Pixel Size** from `3` to `4`, and enable a palette. Look at the difference in the small details.

### 04 — World Mood

`tutorial/04_world_mood.tscn`

![[example-04.png]]

The island at dusk, with weather, birds, fireflies and dust nodes ready to play with.

**Try this:** set the weather to Rain, then Storm. Scrub **Time** to night and watch the fireflies. Turn **Animate** on in the editor to see the clouds and water move.

### Camera controls in the tutorial scenes

The tutorial scenes use a small orbit camera, not a player character:

| Input | Action |
|---|---|
| Right-drag | Orbit |
| Mouse wheel | Zoom |
| `W` `A` `S` `D` | Pan |
| `Q` / `E` | Move down / up |

This camera is intentionally simple. The real game will use the player camera.

## The demo scenes

### Hex Island

`demo/hex_island.tscn`

![[demo-hex-island.png]]

The main showcase: a hex-grid island with cliffs, a waterfall, a village ruin, a knight, birds, weather, day/night, fire, smoke and a lantern.

Play it with `F5`.

| Input | Action |
|---|---|
| `W A S D` | Move |
| `Shift` | Walk |
| `Space` | Jump |
| Mouse drag / arrow keys | Orbit the camera |
| Mouse wheel | Zoom |
| `F3` | Diagnostics overlay (FPS, weather, time of day, debug views) |
| `F1` | Show the controls again |

Use `F3` → weather/time buttons to change the mood while the game runs. That is a great way to see how the same scene looks at noon, dusk, in rain and in a storm.

### Courtyard

`demo/courtyard.tscn`

![[demo-courtyard.png]]

A smaller hand-built courtyard at dusk: fountain, pillars, round trees, a fox, and `PixelSunShafts` cutting through the shadows.

It is a good reference for:

- Laying out props on a square-grid terrain.
- Placing imported models under one PixelObject.
- A static, non-cycling time of day.
- Sun shafts with a low sun.

## How to make a scene yours

1. Open an example.
2. **File → Save Scene As…** and save a copy in your own folder (`tutorial/` is fine for practice; make a new folder for real work).
3. Change the terrain, add props, and save again.
4. Never edit the original demo scenes unless you mean to change the showcase.

> [!tip]
> A good way to learn the addon is to open `demo/hex_island.tscn`, select each node in the Scene tree, and read its Inspector. Almost every feature in this manual is used somewhere in that scene.

Next: [[12 Cheat Sheet]]
