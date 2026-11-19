# Marching Squares Terrain (pixel edition)

**Sculpt, paint and play 3D pixel-art terrain in Godot 4.7 — one dependency-free Rust
GDExtension for the Compatibility (OpenGL 3.3) renderer.**

![The hex island demo](docs/images/island.png)

Marching Squares Terrain merges *Yūgen's Terrain Authoring Toolkit* (marching-squares terrain)
with a 3D pixel-art pipeline into one addon. Terrain, grass, props and water come out as
crisp, outlined, world-locked pixel art with no wiring: add a terrain, add a `PixelRenderer`,
press play. Square, triangle and hex grids; reference-style sculpting; foliage painting;
sky, weather, day/night, water and fire effects.

## Install / build

You need Godot 4.7 and Rust 1.88+.

```bash
git clone --recursive <this repo> && cd <repo>
tools/build.sh            # builds the addon + demo libraries into their bin/ folders
                          # (Windows: powershell -File tools/build.ps1)
godot -e --path .         # open the project
```

The libraries are not committed on `rust-rewrite`; build them once per platform. Godot locks
them while it runs, so close the editor before rebuilding. To use the addon in your own project,
copy `addons/MarchingSquaresTerrain/` (with its built `bin/`) and set the project to the
Compatibility renderer — there is no plugin to enable. The addon's Rust sources ship inside
that folder under `rust/` if you want to inspect or rebuild them.

### Release branch: no build needed

The [`release` branch](https://github.com/notnilcn/Yugens-Terrain-Authoring-Toolkit-rust/tree/release)
commits the built Windows x86_64 libraries at the paths the `.gdextension` files load, so the
project opens and plays without a Rust toolchain:

```bash
git clone -b release --recursive <this repo> && cd <repo>
godot -e --path .        # press F5 for the demo
```

`release/README.md` lists exactly what is committed there and how to refresh it. Linux and macOS
still build their own libraries with `tools/build.sh`.

## Open the demo

`demo/hex_island.tscn` is the main scene: a hex island with cliffs, a waterfall, a village
ruin, birds, weather and a knight to walk around. Press **F5** to play it.

![Playing the demo](docs/images/play.png)

| Play | |
|---|---|
| `W` `A` `S` `D` | move (hold `Shift` to walk) |
| `Space` | jump |
| mouse drag / arrow keys | orbit the camera |
| mouse wheel | zoom |
| `F3` | diagnostics overlay (fps, weather / time-of-day buttons, debug views) |
| `F1` | show the controls hint again |

`demo/courtyard.tscn` is a smaller scene after robert-k's godot-3d-pixel-art demo. It has a
fountain, pillars and round trees on a flat meadow, a fox to play with the same controls, and
`PixelSunShafts` light shafts cut by the sun's shadows, under a dusk sky.

![The courtyard demo](docs/images/courtyard.png)

## Quickstart: sculpt → paint foliage → play

Select **MarchingSquaresTerrain** in the demo (or add one to a new 3D scene). A labelled
toolbar appears on the left of the 3D view, the active tool's settings along the bottom, and a
one-line help for the tool at the bottom-left of the view.

![The terrain editor](docs/images/editor.png)

1. **Sculpt** — pick **Brush** (`1`). Click (or drag) on the terrain to select cells, then
   drag the selection up or down. `Shift` adds cells, `Ctrl` removes, `RMB`/`Esc` clears.
   **Smooth** (`3`) rounds things off (*Smooth Mode*: Terraced keeps steps, Sloped makes
   ramps); **Level** (`2`) flattens; **Stamp** drops mountains and craters. Every stroke is
   one `Ctrl+Z`.
2. **Paint foliage** — pick **Foliage** (`5`), choose Grass, Tree, Rock… in the palette on the
   right (`Ctrl`/`Shift`+click for several) and paint with LMB; RMB erases. `Shift`+wheel
   resizes the brush. **Paint** (`6`) paints ground textures the same way.
3. **Play** — press **F5** (the demo) or, in your own scene, add a **PixelRenderer** and a
   `Camera3D` and press **F6**. The world renders as low-res pixel art; your UI stays sharp.

New terrain from scratch: add a `MarchingSquaresTerrain`, pick **Chunks** (`8`) and click the
green `+` to add chunks, then sculpt as above. **Settings** (`9`) holds grid type, cell size,
texture blending, grass and storage options.

## Learn more

* [docs/reference.md](docs/reference.md) — the full manual: every node and resource, all editor
  tools, storage modes, the level builder, effects, custom shaders, hex grids and the game API.
* [docs/testing.md](docs/testing.md) — unit tests, headless checks and the windowed editor
  checks.
* [CREDITS.md](CREDITS.md) — sources and licences. MIT licensed (the demo-only Phantom Camera
  port is GPL-3.0).
