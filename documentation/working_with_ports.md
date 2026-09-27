# Working with the bundled ports (Cyclops + TileMapLayer3D)

The repository root is the development project for `addons/MarchingSquaresTerrain` (MST) and
also vendors two renamed, sibling ports. This document covers how to enable them, how Grid
Align works, and the differences from upstream.

## Installation

Copy the addon folders you want into another project's `addons/` folder:

| Addon | Provides | Depends on |
|---|---|---|
| `addons/MarchingSquaresTerrain/` | The terrain plugin + the shared `MSTGridSnap` helper. | nothing |
| `addons/YugenCyclopsLevelBuilder/` | Block-out level editor, with the MST Grid Align snapping system. | MST for Grid Align only |
| `addons/YugenTileMapLayer3D/` | 2.5D tile editor, with MST Grid Align and dual-grid auto tiling. | MST for Grid Align only |

Enable the plugins under **Project Settings > Plugins**. If a port is installed without MST,
it still loads; its Grid Align UI simply reports that MST is not installed.

> The autoload for Cyclops (`YugenCyclopsAutoload`) is registered by the plugin itself when it
> is enabled (both via the Plugins tab and when enabled at startup), so a restart is not
> required.

## Cyclops: Grid Align

1. Select a `MarchingSquaresTerrain` node that uses the **Square** grid type.
2. In the 3D editor's Cyclops toolbar, open the snapping dropdown and pick **Snap MST** (or it
   becomes the active system automatically when a supported terrain was assigned previously).
3. Open the **Snap** dock and press **Use Selected Terrain**.
   * **Align to** — Vertex and Center (default), Vertex Only, or Center Only.
   * **Cell multiplier** — enlarges the lattice (`cell_size * multiplier`).
   * **Snap Y / Y snap distance** — optional vertical snapping in terrain-local space.
4. If the terrain is Triangle or Hexagonal the status line explains that Grid Align is
   unavailable and snapping stays on the regular numeric grid.

All block create/move/edit/clip tools go through the snapping manager, so Grid Align applies
everywhere at once. Assigning/clearing the terrain and the settings persist through the
plugin's `user://yugen_cyclops_editor_cache.json` and `user://yugen_cyclops_settings.config`.

## TileMapLayer3D: Grid Align

Select the `YugenTileMapLayer3D` node and use the **MST Grid Align** export group in the
Inspector:

* `mst_terrain` — the terrain whose lattice to snap to.
* `mst_grid_align` — enables snapping; the Settings tab shows a read-only mirror of the grid
  size plus the MST source. The map's own grid size is **not** rescaled, so existing tiles are
  untouched; snapped positions are expressed in map grid units.
* `mst_align_kind`, `mst_cell_multiplier`, `mst_snap_y`, `mst_y_snap_distance` — as in Cyclops.

The hook lives in `YugenTilePlacementManager.snap_to_grid()`, so cursor placement, area fill,
sculpt, smart fill and the runtime API all flow through it. The plane constraint is preserved:
on the floor plane Y is only snapped when `mst_snap_y` is on.

Grid Align is rejected (with a reason in the settings tab) when the terrain is not square, its
cell size is not uniform, or the resulting lattice half-spacing is finer than the tile-key
precision (0.01 grid units, see below).

### Tile key precision

The port raises `YugenTileKeySystem.COORD_SCALE` from 10 to 100 (0.01 grid-unit quantum,
±327.67 grid range) so MST lattice positions such as `cell_size 0.25` at `grid_size 1.0`
store exactly. `YugenGlobalConstants.MAX_GRID_RANGE` is reduced from 2500 to 300 to match.
Data saved with the stock TileMapLayer3D decodes with finer precision, which is compatible
for positions on the old 0.1 grid but shifts sub-0.1 quantization slightly.

## TileMapLayer3D: Dual Grid Auto Tile

Enable `dual_grid_auto_tile` on the node. When painting/erasing flat **FLOOR** square tiles,
the affected display variants are recomputed from the four surrounding XZ cells:

* 15-variant peering table and mixed variants, identical to DualGrid and the TileMapDual v5
  "Standard" square layout (`YugenDualGridResolver`).
* With a `TileSet` assigned, the resolved variant is written to the tile's
  `atlas_source_id` / `atlas_coords`; without one, the variant maps into a 4x4 terrain block
  in the texture atlas, derived from the tile's current UV rect (or set
  `dual_grid_terrain_block_origin` explicitly).
* Optional `YugenBespokeMixRule` resources (`dual_grid_bespoke_rules`) override the generic
  mix for a pair of terrain ids; `YugenLayerOrderOverrideRule` resources
  (`dual_grid_order_rules`) handle layer-ordering edge cases. Both have dedicated inspectors.
* Walls, BOX/PRISM, tilted and vertex-edited tiles keep upstream behaviour and are excluded.

## Verification

From the repository root (Godot 4.7.1 mono console build):

```powershell
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --editor --quit --path .
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_mst_snap_tests.gd
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_cyclops_snap_tests.gd
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_tilemap_key_precision_tests.gd
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_tilemap_mst_align_tests.gd
& "C:\Users\Clinton\g\Godot\Godot-stable_mono_win64_console.exe" --headless --path . --script res://tests/run_dual_grid_resolver_tests.gd
```

The rename map (Tier-1 class names, path rewrites, settings paths) is documented in
[`PORT_RENAME_MAP.md`](../PORT_RENAME_MAP.md); licenses and attribution are in
`addons/MarchingSquaresTerrain/documentation/credits.md`.
