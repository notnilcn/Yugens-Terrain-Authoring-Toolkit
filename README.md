# Yūgen's Terrain Authoring Toolkit
The public version of the Marching Squares Terrain plugin for godot.

This project is an effort to create a simple to use and powerfull terrain authoring tool inside godot aimed at 3d pixel art games. However, the plugin featured in this project can be used for a wide variety of games and experimentation is encouraged! As of right now the plugin has the following features:

* Elevate and lower terrain based on cells in a chunk grid
* Level terrain to a user-set height
* Smooth terrain depending on the average height of neighbouring cells
* Create a bridge between two points by drawing a line between them
* Paint up to 15(+1) custom textures onto the terrain
* Paint a mask map that determines whether selected cells should draw `MultiMeshInstance3d` grass instances
* Three grid types: Square (original marching squares), Triangle and Hexagon (flat-topped cell columns), selectable per terrain
* Round, Square and Hexagon brush shapes; exact grid-aligned painting of whole cells/hexagons
* Grass and runtime texture baking on all grid types
* Get debug information for selected cells
* Change the internal marching squares algorithm vertex merge threshold value resulting in smoother or blockier terrain
* Change global terrain settings like the default wall texture, texture blend mode, grass animation fps and more...
* Bundled ports of **Cyclops Level Builder** and **TileMapLayer3D**, both with a shared **MST Grid Align** snapping mode (see below)

## Grid Modes

The Terrain Settings tool has a **Grid Type** dropdown:

* **Square** (default) - the original marching squares pipeline, unchanged.
* **Triangle** - equilateral triangles on a triangular lattice.
* **Hexagon** - regular pointy-top hexagons in odd-r offset rows.

Triangle and hexagon terrains render every cell as a flat polygon at its own height
with vertical cliffs where a neighbour is lower. All painting tools (height, level,
smooth, bridge, vertex paint, grass mask, quick paint) work on all three grid types.

* **Grid Aligned** (Brush, Level, Smooth, Bridge): requires the Hexagon brush and
  Falloff off on a cell terrain. The brush then selects whole cells: `3N² + 3N + 1`
  hexagon cells or `6N²` triangles, where `N` is the **Grid Size** attribute.
* **Merge modes** (per chunk, Chunk Management tool): steps at or below a mode's
  threshold are drawn as 45-degree ramps instead of cliffs. CUBIC keeps exact columns.
* **Data folders**: each grid type stores its chunks in its own subfolder of the
  terrain's data directory (`square/`, `triangle/`, `hex/`). Legacy data at the root
  of the data directory is migrated into `square/` automatically.

For more in-depth documentation, please refer to the _documentation_ folder in the addon.

## Bundled Ports (Grid Align + Dual Grid)

This repository also ships two vendored, renamed ports under `addons/`:

* `addons/YugenCyclopsLevelBuilder/` — full port of [Cyclops Level Builder](https://github.com/blackears/cyclopsLevelBuilder) (MIT, Mark McKay). Adds a **Grid Align** snapping system that snaps block edits to an assigned `MarchingSquaresTerrain` lattice (vertices/cell centers, cell-size multiplier, optional Y snap).
* `addons/YugenTileMapLayer3D/` — full port of [TileMapLayer3D](https://github.com/DanTrz/TileMapLayer3D) (MIT, DanTrz). Adds the same **MST Grid Align** placement snapping plus a **Dual Grid Auto Tile** mode that picks a tile's display variant from its XZ neighbours using the DualGrid / TileMapDual v5 peering rules (bespoke mixes and layer-order overrides included).

Both ports depend on the `MSTGridSnap` helper that ships inside the MST addon, but load and behave like upstream when no terrain is assigned.
See [`documentation/working_with_ports.md`](documentation/working_with_ports.md) for setup and controls.

For community showcases, feature requests and bug reporting, please refer to the [discord](https://discord.gg/ZSeYkTCgft).
A bug can also be reported by opening a new issue thread in the issues tab of this github project.

## Install Guide

To install the plugin, simply download or clone the latest stable version of this project and copy the plugin from this project's addon folder into your own. Make sure to turn on the plugin in godot by going into the project settings and under "plugins" checking the checkbox next to the plugin's name.

Watch the [YouTube](https://www.youtube.com/playlist?list=PLXcmz5ZRdiyTpf_Jk9gGNb9QQ6Hus8xiP) videos to get started with the plugin!!!

## Known Issues

1. Smooth texture blending breaks at certain elevated/lowered cell edge cases
2. d3d12 doesn't load terrain material properly when in game on some devices

## Credits

Developed by [Yūgen](https://www.youtube.com/@yugen_seishin) and originally forked from [Jackachulian](https://github.com/jackachulian/jackachulian) on github.

Collaborators (v1.1.0 ONWARDS):
* [DanTrz](https://github.com/DanTrz)
* [powertomato](https://github.com/powertomato)
* [santarl](https://github.com/santarl)

A special thanks to DanTrz (creator of the TileMapLayer3D plugin), powertomato and santarl for co-authoring big parts of the plugin since the 1.0 release. They have been amazing contributors to the project and awesome people to work with!

Contributors:
* [Dylearn](https://www.youtube.com/@Dylearn)
* [AtSaturn](https://www.youtube.com/@AtPlayerSaturn)
* My lifelong best friends!

###
A big thanks to the above people for giving helpful insights, discussing certain features and thinking together about math related problems. Without them I couldn't have finished the plugin as fast as I have.
