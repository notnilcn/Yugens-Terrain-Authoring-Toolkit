# Setting Up a New Project

You will normally work in the game project that is already set up. This note is for the day you want to make a separate prototype or a fresh scene collection.

The addon is a **GDExtension**: it is a compiled library plus assets, not a normal Godot plugin. There is no “enable plugin” checkbox.

## What you need

- **Godot 4.7** (the project is built for 4.7).
- The folder `addons/MarchingSquaresTerrain/`, including its `bin/` folder with the compiled library for your operating system.
- The **Compatibility** renderer (`gl_compatibility`), not Forward+ or Mobile. The pixel pipeline is written for Compatibility.

> [!warning]
> `bin/` is platform-specific. A Windows `.dll` does not work on Linux or macOS. Ask the programmer for the right build, or build it with `tools/build.sh` if you have the Rust toolchain.

## Copy the addon

1. Create a new Godot 4.7 project.
2. Copy the whole `addons/MarchingSquaresTerrain/` folder into the new project's `addons/` folder.
3. In **Project Settings → Rendering → Renderer**, choose **Compatibility**.
4. In **Project Settings → Rendering → Textures → Default Texture Filter**, choose **Nearest**. This keeps pixel textures crisp.
5. In **Project Settings → marching_squares_terrain → features**, turn on the features you need. For a first scene: **Outlines**, **Outline Ids**, **Grass**. Add **Clouds**, **Water**, **Weather**, etc. when you use those nodes.

## Build a first scene

1. New 3D scene.
2. Add a `MarchingSquaresTerrain`.
3. Select it → **Chunks** (`8`) → click the green **+** at the origin.
4. In the terrain Inspector, assign **Tex Assign** = `res://addons/MarchingSquaresTerrain/texture_presets/mst_classic.tres`.
5. Add a `Camera3D` (orthographic, `Size` around 20), a `DirectionalLight3D` with shadows, and a `WorldEnvironment` (background Color or ProceduralSky).
6. Add a `PixelRenderer`. Leave **Camera** empty.
7. Add a `PixelSky` and a `PixelDayNight` if you want clouds and a time of day.
8. Turn on **Pixel Preview** in the 3D toolbar.
9. Press `F5`.

If the ground is invisible, you forgot the chunk. If the pixel look is missing, you forgot the PixelRenderer or Compatibility.

## Copying scenes between projects

A `.tscn` references resources by path. If you copy a scene that uses textures or models from the addon, copy those paths too, or make the textures local to your project. The terrain's **Contents** data resource must come with the scene (or be embedded in it).

The easiest safe move is to copy the whole example scene **and** its data resource, keeping the same folder layout. Do not rename the addon folder.

## When to ask the programmer

- You need a build for a new operating system.
- You want to add a new shader or change the pixel pipeline.
- You want a new feature switch.
- A red error mentions Rust, a `.so`, `.dll` or `.dylib`.

Everything else — terrain, painting, props, sky, weather, water and effects — is meant to be yours to use without touching code.

Next: [[11 Example Scenes]]
