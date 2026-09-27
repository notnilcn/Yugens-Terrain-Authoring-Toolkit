# Port rename map — Cyclops + TileMapLayer3D into YTAT

Working note for the port described in `../PORT_PLAN_cyclops_tilemaplayer3d_to_yugen.md`.
Applied mechanically by a one-shot local script (not committed); this file records exactly what
was renamed, so the divergence from upstream is auditable.

Source folders (read-only, sibling checkouts on this machine):

| Upstream | Ported to |
|---|---|
| `cyclopsLevelBuilder/godot/addons/cyclops_level_builder/` | `addons/YugenCyclopsLevelBuilder/` |
| `TileMapLayer3D/addons/TileMapLayer3D/` | `addons/YugenTileMapLayer3D/` |

## Tier 1 — renamed identifiers (whole-word, single pass)

Order below is the application order.

### `addons/YugenTileMapLayer3D`

| Upstream | Ported |
|---|---|
| `TileMapLayer3DPlugin` | `YugenTileMapLayer3DPlugin` |
| `TileMapLayer3D` | `YugenTileMapLayer3D` |
| `TileMapLayerData` | `YugenTileMapLayerData` |
| `TileMapLayerSettings` | `YugenTileMapLayerSettings` |
| `TilePlacementManager` | `YugenTilePlacementManager` |
| `TileKeySystem` | `YugenTileKeySystem` |
| `GlobalConstants` | `YugenGlobalConstants` |
| `GlobalUtil` | `YugenGlobalUtil` |
| `GlobalPlaneDetector` | `YugenGlobalPlaneDetector` |
| `GlobalTileMapEvents` | `YugenGlobalTileMapEvents` |
| `RegionSystem` | `YugenRegionSystem` |
| `RegionBaker` | `YugenRegionBaker` |
| `RegionBakeOptions` | `YugenRegionBakeOptions` |
| `SelectionManager` | `YugenSelectionManager` |

### `addons/YugenCyclopsLevelBuilder`

| Upstream | Ported |
|---|---|
| `SnappingSystemGridPropertiesEditor` | `YugenSnappingSystemGridPropertiesEditor` |
| `SnappingSystemVertexPropertiesEditor` | `YugenSnappingSystemVertexPropertiesEditor` |
| `SnappingSystemVertexSettings` | `YugenSnappingSystemVertexSettings` |
| `SnappingSystemGrid` | `YugenSnappingSystemGrid` |
| `SnappingSystemVertex` | `YugenSnappingSystemVertex` |
| `SelectionList` | `YugenCyclopsSelectionList` |
| `Selection` | `YugenCyclopsSelection` |
| `SnappingManager` | `YugenSnappingManager` |
| `SnapToGridUtil` | `YugenSnapToGridUtil` |
| `SnappingQuery` | `YugenSnappingQuery` |
| `CyclopsLevelBuilder` | `YugenCyclopsLevelBuilder` |
| `CyclopsGlobalScene` | `YugenCyclopsGlobalScene` |
| `CyclopsAutoload` (string, autoload name) | `YugenCyclopsAutoload` |
| `MathUtil` | `YugenMathUtil` |

Tier 2 classes keep upstream names (`Cyclops*`, `Tool*`, `Action*`, `Command*`, `Keymap*`,
`Gizmo*`, `Uv*`, `Tile*`, `Autotile*`, `Sculpt*`, ...). No collisions exist between the two ports
and the MST addon: the three class-name sets (299 + 61 + 35 names) were compared pairwise and are
disjoint.

## Path and settings rewrites (both ports)

| Upstream | Ported |
|---|---|
| `res://addons/cyclops_level_builder/` | `res://addons/YugenCyclopsLevelBuilder/` |
| `res://addons/TileMapLayer3D/` | `res://addons/YugenTileMapLayer3D/` |
| `addons/TileMapLayer3D/` (EditorSettings keys) | `addons/YugenTileMapLayer3D/` |
| `user://keymap.tres` | `user://yugen_keymap.tres` |
| `user://cyclops_editor_cache.json` | `user://yugen_cyclops_editor_cache.json` |
| `cyclops_settings.config` (relative → project root) | `user://yugen_cyclops_settings.config` |
| `/root/CyclopsAutoload` | `/root/YugenCyclopsAutoload` |
| `plugin.cfg` names | `Yūgen Cyclops Level Builder`, `Yūgen TileMapLayer3D` |

`.uid` sidecars were copied as well so local `uid://` references keep resolving; they stay
untracked (root `.gitignore` ignores `*.uid`), which matches the plan's re-import story. `.import`
files were not copied; Godot regenerates them on first import.

## Exceptions to the mechanical pass

- `plugin.cfg` `version` keeps the upstream version with a `+yugen.1` suffix.
- License files are kept in place (`LICENSE.md` / `LICENSE.txt`) and credits are extended in
  `addons/MarchingSquaresTerrain/documentation/credits.md`.
- `gui/controls/vec4D1.tmp` (a dead upstream temp file) had its two ext_resource paths
  rewritten to the new folder; it is not imported by Godot.
- Binary demo assets under `YugenTileMapLayer3D/DemoScene/` (`demo_scene_v_2.scn`, its
  `_SavedData/*.res` and `temp_tileset/Demo_TileSet_v03.res`) were loaded and re-saved through
  Godot so their embedded `res://` paths point at the new addon folder. This follows the
  plan's re-save/re-import remedy; the old paths cannot be rewritten at the byte level safely
  because Godot's binary format length-prefixes strings.
- Feature work (MST snapping system, `COORD_SCALE`, dual-grid resolver) is separate from this map.
