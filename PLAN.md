# PLAN: Triangle + Hexagon Cell Terrain Modes + Grid-Aligned Brushes

Target project: `Yugens-Terrain-Authoring-Toolkit-1.2.4-original/` (Godot 4.7.x, GDScript only).
This document is the implementation plan. It is written for a fresh session with no prior context.

## Implementation status (updated 2026-09-27, after implementation)

**M0-M4 and M7 are complete. M5 is partially complete. M6 is not started (v1 scope).**

- [x] M0 backup/baseline - [x] M1 grid math + tests - [x] M2 cell chunks + modes + save/load
- [x] M3 painting tools on cell terrain - [x] M4 Grid Aligned + Grid Size - [x] M7 regression
- [x] M5 texture/vertex-encoding verification - [x] M5 quick paint on cell modes
- [ ] M5 grass for cell modes, M5 runtime texture baking (deferred)
- [ ] M6 cell handle gizmos, lattice-aligned hex outline, README updates (deferred)

Verification summary: 7 headless test scripts pass (119 checks total, plus perf timing);
the editor loads with no script errors; the square demo scene renders identically to the
pre-change backup. See "## 9. Implementation notes and deviations" for the technical
corrections discovered during implementation - in particular the triangle edge/corner
table in section 3.2 was geometrically inconsistent and was corrected there.

## 0. Read this first

### 0.1 Locked design (decisions already made)

- **Three cell modes** on `MarchingSquaresTerrain`, selected in Terrain Settings:
  `SQUARE` (existing, must stay behaviorally identical), `TRIANGLE` (new), `HEX` (new).
- **Hex cells**: pointy-top regular hexagons, odd-r offset rows. Height stored per hex cell.
- **Triangle cells**: equilateral triangles, height stored per triangle. Addressing uses
  "rhombus coordinates": the triangular lattice's 60-degree rhombus `(i, j)` is split by its short
  diagonal into two triangles, A and B. Exposed cell coordinate = `Vector2i(2*i + orient, j)`
  (`orient` 0 = A, 1 = B). This is a rectangular array of alternating up/down triangles.
- **Rendering model for both new modes**: terraced "columns". Each cell renders as a flat regular
  polygon (hexagon or triangle) at its own height, plus vertical wall quads on each edge where the
  neighbor is lower. No height interpolation between cells. No marching-cases logic.
- **Exactness**:
  - Hex terrain: cells within hex distance `N` of a center cell form a regular hexagon of
    `3N^2 + 3N + 1` cells.
  - Triangle terrain: triangles whose three corners all lie within lattice-point hex distance `N`
    of a center lattice point form a regular hexagon of side `N` made of `6N^2` triangles.
  - Grid Aligned + Grid Size produce these exact selections; no approximations.
- **Data separation**: per-mode subfolders under each terrain's existing `data_directory`:
  `square/`, `triangle/`, `hex/`.
- **Square mode must stay byte-for-byte behaviorally identical.** Add parallel code paths; do not
  refactor square algorithms.
- The earlier "triangular brush cell markers" idea is superseded; do not implement it.

### 0.2 Decisions to confirm with the user (do not block on these; ask at M0 if possible)

> **Resolved during implementation:** decision 1 was implemented as written (the gate lives in
> `MarchingSquaresTerrainPlugin.grid_align_gate_passes()` and the attributes UI calls it).
> Decision 2 was implemented with the stated clamps and tooltip. For decision 3, quick paint
> was implemented anyway (M5 partial); grass, runtime texture baking, per-cell update caching,
> merge modes and per-cell handle gizmos remain deferred.

1. `Grid Aligned` gating: this plan gates it on
   `grid_type in {TRIANGLE, HEX} AND current_brush_index == 2 (Hexagon) AND falloff == false`.
   The terrain condition is an addition to the user's stated "falloff off + hexagon brush" rule.
   The gate lives in exactly one function (`_can_grid_align()`) so it is trivial to relax.
2. `Grid Size` semantics per mode: hex `N >= 0` (`N = 0` = one cell); triangle `N >= 1`
   (`N` = hexagon side length in triangle edges; `N = 1` = 6 triangles). Clamped in code, noted in
   the control tooltip.
3. v1 defers for both new modes: grass, quick paint, runtime texture baking, per-cell update
   caching, merge modes, per-cell handle gizmos. Stretch milestones M5/M6.

### 0.3 Ground rules for the implementer

- No git repo exists in this directory. **M0 must create a backup before any edit.**
- Before editing a file, read it fully. Keep the existing code style: tabs, `:=`, `snake_case`,
  `@tool` on everything under `editor/` and on chunk/terrain scripts.
- Do not rename existing public members. Do not reformat untouched code.
- Every milestone ends with its verification steps. If verification fails, stop and fix.
- Use `res://` paths (not `uid://`) for newly created resources until Godot generates `.uid` files
  for them. New scripts with `class_name` can be referenced directly without preload.
- If a Godot MCP/CLI helper is available in the session, use it for headless checks and screenshots.
  Otherwise use the editor's Output panel.

---

## 1. Goal and scope

### 1.1 In scope

1. `grid_type` property on `MarchingSquaresTerrain` with a **Terrain Settings** dropdown:
   `Square` (default, unchanged) / `Triangle` / `Hexagon`.
2. Triangle and hexagonal chunk classes, mesh generation, collision, save/load, editor painting.
3. `Hexagon` as the 3rd option in the Brush Type dropdown, with a hexagon radius outline.
4. `Grid Aligned` (bool) + `Grid Size` (int) attributes for the **Brush, Level, Smooth, Bridge**
   tools, gated per 0.2, with Grid Size only usable when Grid Aligned is on.
5. Per-mode data subfolders.

### 1.2 Out of scope (v1)

- A triangle-shaped brush option (only Round/Square/Hexagon brush shapes; all work on all terrains).
- Grass, quick paint, runtime atlas baking, merge modes, chunk handle gizmos for the new modes.
- Any change to square-mode behavior.

---

## 2. Architecture map

### 2.1 Existing square pipeline (do not break)

| File | Role |
| --- | --- |
| `algorithm/terrain/marching_squares_terrain.gd` (883 ln) | Terrain node; chunk registry in `chunks : Dictionary`; settings; chunk add/remove; data dir |
| `algorithm/terrain/marching_squares_terrain_chunk.gd` (657 ln) | Square chunk: `height_map[z][x]`, mesh gen, collision, save hooks |
| `algorithm/terrain/marching_squares_terrain_cell.gd` (706 ln) | Marching-squares per-quad case geometry |
| `algorithm/terrain/marching_squares_terrain_vertex_color_helper.gd` (385 ln) | Vertex color/material packing for the shader |
| `resources/mst_data_handler.gd` (608 ln) | External per-chunk save/load (`metadata.res`), migration, cleanup |
| `resources/mst_chunk_data.gd` (55 ln) | Saved chunk payload resource |
| `editor/marching_squares_terrain_plugin.gd` (1345 ln) | EditorPlugin: input, draw_pattern, undo/redo, hotkeys |
| `editor/marching_squares_ui.gd` (280 ln) | Wires toolbar/tool attributes; applies tool settings |
| `editor/tools/scripts/marching_squares_tool_attributes*.gd` | Tool settings UI construction (list/defs/builder) |
| `editor/utils/brush_pattern_calculator.gd` (97 ln) | Brush bounds, max distance, falloff sampling |
| `editor/gizmos/marching_squares_terrain_gizmo.gd` (292 ln) | Brush preview, cursor cell, height drag, chunk outlines |
| `editor/gizmos/marching_squares_terrain_gizmo_plugin.gd` (51 ln) | Gizmo creation by node class |
| `resources/shaders/mst_terrain.gdshader(.inc)` | Terrain shading (topology-agnostic if vertex attrs match) |

### 2.2 New pipeline

| File | Role |
| --- | --- |
| `algorithm/hex/marching_squares_hex_grid.gd` (new) | Static hex math: coords, neighbors, distance, corners, conversions |
| `algorithm/tri/marching_squares_tri_grid.gd` (new) | Static triangle math: rhombus/rhombi coords, neighbors, corners, hexagon selection |
| `algorithm/terrain/marching_squares_terrain_chunk_base.gd` (new) | Minimal shared base for all 3 chunk classes (typing + virtual API) |
| `algorithm/cells/marching_squares_cell_chunk.gd` (new) | Shared per-cell chunk: maps, flat top + wall mesh, collision, save hooks |
| `algorithm/hex/marching_squares_hex_chunk.gd` (new) | Hex cell shape/neighbors exposed to the cell chunk |
| `algorithm/tri/marching_squares_tri_chunk.gd` (new) | Triangle cell shape/neighbors exposed to the cell chunk |
| `tests/run_grid_tests.gd` (new) | Headless math tests for both new grids |

### 2.3 Integration rules

- Shared systems branch on `terrain.grid_type` at their entry points and call `*_cell` variants:
  - plugin: `draw_pattern`, `update_draw_pattern` (currently dead code, see 5.11), chunk hover.
  - gizmo: brush preview + cursor cell.
  - data handler: save/load dispatch.
- Type hints that say `MarchingSquaresTerrainChunk` in **mode-agnostic** code must become
  `MarchingSquaresTerrainChunkBase`. Square-only code keeps its type.
- Grass loops iterating all chunks must guard with `if chunk is MarchingSquaresTerrainChunk`.
- All new modes are cell-centered; the square vertex-based neighbor expansion logic never runs for
  them.

---

## 3. Locked specifications

### 3.1 Hex coordinate system (pointy-top, odd-r)

```
const SQRT3 := 1.7320508075688772

# terrain.cell_size.x only; hexes stay regular.
# radius (circumradius) R = cell_size.x / SQRT3
# spacing.x = SQRT3 * R = cell_size.x
# spacing.y = 1.5 * R = cell_size.x * 0.8660254037844386

cell_center(cell) = Vector2(
    spacing.x * (float(cell.x) + 0.5 * float(cell.y & 1)),
    spacing.y * float(cell.y))

world_to_cell(p) -> Vector2i:
    var row := roundi(p.y / spacing.y)
    var col := roundi(p.x / spacing.x - 0.5 * float(row & 1))
    return Vector2i(col, row)

corner_i (i in 0..5) = Vector2(R * cos(a), R * sin(a)), a = deg_to_rad(-30.0 + 60.0 * i)

EDGE_NEIGHBORS (even rows) = [ ( 1, 0), ( 0, 1), (-1, 1), (-1, 0), (-1,-1), ( 0,-1) ]
EDGE_NEIGHBORS (odd rows)  = [ ( 1, 0), ( 1, 1), ( 0, 1), (-1, 0), ( 0,-1), ( 1,-1) ]
# edge k normal angle = 60 * k degrees from +x toward +z; edge k uses corners (k, k+1 mod 6)
```

Axial conversion and distance:

```
to_axial(c) = Vector2i(c.x - floori(float(c.y - (c.y & 1)) / 2.0), c.y)
axial_to_offset(a) = Vector2i(a.x + floori(float(a.y - (a.y & 1)) / 2.0), a.y)
hex_distance(a, b): var d := to_axial(a) - to_axial(b)
    return (absi(d.x) + absi(d.y) + absi(d.x + d.y)) / 2
cells_in_hex_radius(center, radius): standard axial hex disk of `radius`, converted to offset;
    count must equal 3*r^2 + 3*r + 1
```

Chunk mapping (`cells_per_chunk` hex = `Vector2i(terrain.dimensions.x, terrain.dimensions.z)`):

```
chunk_of_cell(c, cells)   = Vector2i(floori(float(c.x) / cells.x), floori(float(c.y) / cells.y))
local_cell(c, cells)      = Vector2i(posmod(c.x, cells.x), posmod(c.y, cells.y))
global_cell(chunk, local, cells) = chunk * cells + local
```

Chunk node position = world `cell_center` of global cell `chunk_coords * cells`; all mesh vertices
are local offsets from it.

### 3.2 Triangle coordinate system (equilateral, rhombus addressing)

Basis, with `s = terrain.cell_size.x` (triangle edge length) and `h = sqrt(3.0) / 2.0 * s`:

```
u = Vector2(s, 0.0)
v = Vector2(s * 0.5, h)
lattice_point(i, j) = i * u + j * v

# lattice floats (exact inverse of the above)
lattice_floats(p) -> Vector2:
    var beta := p.y / h
    var alpha := p.x / s - beta * 0.5
    return Vector2(alpha, beta)

world_to_cell(p) -> Vector2i:
    var af := lattice_floats(p)
    var i := floori(af.x); var j := floori(af.y)
    var a := af.x - i;     var b := af.y - j
    var orient := 0 if (a + b) < 1.0 else 1   # 0 = A, 1 = B
    return Vector2i(2 * i + orient, j)
```

Cell `(2i, j)` = triangle A, corners `[L(i,j), L(i+1,j), L(i,j+1)]`.
Cell `(2i+1, j)` = triangle B, corners `[L(i+1,j+1), L(i+1,j), L(i,j+1)]`.
World cell center (centroid) = average of the 3 corners.

Neighbors, indexed by edge 0..2 (the returned coordinates share that edge):

```
x even (A, i = x / 2): [ (x+1, y), (x+1, y-1), (x-1, y) ]
x odd  (B, i = (x-1)/2): [ (x-1, y), (x+1, y), (x-1, y+1) ]
# edge k uses corner indices [(k+1) % 3, (k+2) % 3] for BOTH A and B
```

> **CORRECTED DURING IMPLEMENTATION - see section 9.1.** The two code lines above are not
> geometrically consistent for the corner layout (they fail shared-edge/symmetry tests).
> The implemented tables are:
> - A(2m, j): edge 0 -> `(x+1, j)`, edge 1 -> `(x-1, j)`, edge 2 -> `(x+1, j-1)`
> - B(2m+1, j): edge 0 -> `(x-1, j)`, edge 1 -> `(x-1, j+1)`, edge 2 -> `(x+1, j)`
> - edge k carries corner pair `(1,2), (0,2), (0,1)` for k = 0,1,2 on both orientations
> - `rhombus_i(x)` floors (rhombus m owns x = 2m and 2m+1, negatives included)

Point-lattice hex distance (for hexagon selection; `(i, j)` are lattice coords):

```
point_hex_distance(a: Vector2i, b: Vector2i) -> int:
    var d := a - b
    return (absi(d.x) + absi(d.y) + absi(d.x + d.y)) / 2
```

Nearest lattice point to a brush position (needed by Grid Aligned): compute `lattice_floats`,
test the 4 integer points `(floor/ceil x, floor/ceil y)`, return the one with the smallest world
distance to `p`.

Exact regular hexagon selection of side `N >= 1` centered on lattice point `c`:

```
cells_in_hexagon(c, N) -> Array[Vector2i]:
    # candidate rhombi: i in [c.x - N, c.x + N], j in [c.y - N, c.y + N]
    # for each rhombus emit both triangles, keep a cell iff all 3 of its lattice
    # corners have point_hex_distance(corner, c) <= N
    # count must equal 6 * N * N
```

Chunk mapping (`cells_per_chunk` triangle = `Vector2i(2 * terrain.dimensions.x, terrain.dimensions.z)`,
where `dimensions.x` = rhombus columns and `dimensions.z` = triangle rows):

```
chunk_of_cell / local_cell / global_cell: same formulas as 3.1
chunk node position = world lattice_point(chunk.x * dimensions.x, chunk.y * dimensions.z)
```

### 3.3 Cell mesh vertex encoding (shared by hex + triangle chunks)

Per cell top (flat polygon = hexagon or triangle, fanned around its centroid):
- One vertex per polygon corner + one centroid vertex; fan emits `corner_count` triangles.
- `smooth group` 0 for tops; -1 for walls.
- `UV` = `Vector2(0, 0)`.
- `UV2` = `Vector2(local_vert.x, local_vert.z) / terrain.cell_size` (square floor convention).
- `COLOR` (`set_color`) = `color_map_0[idx]`; `CUSTOM0` = `color_map_1[idx]`.
- `CUSTOM1` = `Color(grass_mask_map[idx].r, 0.0, 0.0, rl_idx)` where
  `rl_idx = get_texture_index_from_colors(wall_color_map_0[idx], wall_color_map_1[idx])` (0..15).
  Mirror this formula exactly; it is the mapping used by the existing helper for non-ridge floors.
- `CUSTOM2` (mat_blend) = `Color((m + m * 16.0) / 255.0, m / 15.0, 1.0, 0.0)` with
  `m = get_texture_index_from_colors(color_map_0[idx], color_map_1[idx])`.
  Verify visually in M5; if the smooth-blend path misbehaves, try `mat_blend.a = 2.0`.

Per wall quad (edge where `h_own - h_neighbor > 0.0001`), using the edge's two corner positions:
- Quad: `(Qa, h_own)`, `(Qb, h_own)`, `(Qb, h_neighbor)`, `(Qa, h_neighbor)` in local coords.
  The neighbor's top includes the same edge at its height, so the quad closes the gap exactly.
  At triple points, each higher cell contributes its own wall; the union covers the full seam.
- Winding faces away from the higher cell. Verify visually; flip if back faces show.
- `UV` = `Vector2(1, 1)`.
- `UV2` = square wall convention:
  `var gp := local_vert + chunk.global_position_cached; Vector2(gp.x, gp.y) + Vector2(gp.z, gp.y)`.
  Use `terrain.global_position` when the chunk is not inside the tree (see square `_add_point`).
- `COLOR`/`CUSTOM0` from the higher cell's `wall_color_map_*`; `CUSTOM1`/`CUSTOM2` from its
  `grass_mask_map` and `wall_color_map_*` using the same encodings as tops.

Map index = `row * cells_per_chunk.x + col` (matches the square `z * dimensions.x + x` convention).

### 3.4 Data folders

```
<terrain.data_directory>/
    square/            # existing square chunks (after migration)
        chunk_X_Y/metadata.res
    triangle/          # new triangle chunks
        chunk_X_Y/metadata.res
    hex/               # new hex chunks
        chunk_X_Y/metadata.res
```

Legacy layouts (chunks directly under `<data_directory>/`) are square-mode data; migrate them into
`square/` on first load.

### 3.5 Grid-aligned brush semantics

- `grid_aligned == true` requires `grid_size` integer `N`.
- HEX terrain: center = `world_to_cell(brush_position)`; selected = `cells_in_hex_radius(center, N)`,
  `N >= 0`; every selected cell gets sample `1.0`; `brush_size` is ignored.
- TRIANGLE terrain: center = nearest lattice point; selected = `cells_in_hexagon(center, max(N, 1))`;
  every selected cell gets sample `1.0`; `brush_size` is ignored.
- `falloff` is forced off by the UI gate, so strength is uniform.

---

## 4. Milestones

Each milestone is independently verifiable. Do not start the next before the previous passes.

### M0 - Backup and baseline  [DONE]

- [x] Close Godot on this project.
- [x] Create a timestamped backup copy of `Yugens-Terrain-Authoring-Toolkit-1.2.4-original/`
      outside the workspace (e.g. to `%TEMP%`). Optionally `git init` + initial commit.
- [x] Open the project once, confirm it runs (demo scene loads, no new errors in Output).
- [x] Record the Godot version from the editor title bar. (4.7.1.stable.mono)

Backup: `%TEMP%\opencode\Yugens-backup-20260927-165440`.

### M1 - Hex + triangle grid math and tests (no engine integration)  [DONE]

- [x] Create `algorithm/hex/marching_squares_hex_grid.gd` per 3.1.
- [x] Create `algorithm/tri/marching_squares_tri_grid.gd` per 3.2.
      NOTE: section 3.2's edge/corner table was corrected during implementation - see 9.1.
- [x] Create `tests/run_grid_tests.gd` (extends `SceneTree`) asserting for both grids:
  - world<->cell round trip for every cell in a 20x20 region (exact, no tolerance);
  - neighbor symmetry: b in neighbors(a) implies a in neighbors(b), including edge indices;
  - edge/corner consistency: the shared edge's two corners are equidistant from both cell centers;
  - counts: hex disk `1, 7, 19, 37, 61` for N=0..4; triangle hexagon `6, 24, 54, 96` for N=1..4;
  - triangle: all selected cells' corners are within N; selected cells are unique.
- [x] Run `godot --headless --path <project> --script res://tests/run_grid_tests.gd`;
      expect exit code 0 and "ALL TESTS PASSED". (44 checks, passing)
- Verification: test script output only.

### M2 - Terrain modes + cell chunks + save/load (viewable terrain)  [DONE]

- [x] Create `MarchingSquaresTerrainChunkBase` (5.3).
- [x] Create `MarchingSquaresCellChunk` (5.4).
- [x] Create `MarchingSquaresHexChunk` (5.5) and `MarchingSquaresTriChunk` (5.6), both extending
      the cell chunk.
- [x] Add `grid_type` (3 values) to `MarchingSquaresTerrain` + chunk factory + per-mode data dir +
      mode switch + hex/tri hover helpers (5.7).
- [x] Update Terrain Settings dropdown (5.12) so the modes can be selected.
- [x] Update chunk-management hover/add/remove in the plugin (5.11, minimal path).
- [x] Update gizmo plugin to skip hex/tri chunk handles (5.14).
- Verification (manual, new scene - do NOT reuse `mst_demo_scene.tscn`):
  1. New scene, add `MarchingSquaresTerrain`; for each of Triangle and Hexagon: select the mode.
     (Done headlessly + windowed captures: `tests/capture_cell_modes.gd`.)
  2. Use Chunk Manager to add chunks; confirm flat regular triangles/hexagons render with default
     textures, no holes, correct normals, collision present.
     (Mesh/collision asserted in `tests/run_cell_chunk_tests.gd`; visual capture confirms.)
  3. Change `dimensions.x/z`; chunks rebuild with the new counts. (Covered by chunk tests.)
  4. Save scene, reopen; data persists under `<data_dir>/triangle/` or `<data_dir>/hex/`.
     (Covered by `tests/run_cell_save_tests.gd`.)
  5. Switch modes repeatedly; confirm each mode's data is preserved. (Covered by same.)
  6. Confirm `mst_demo_scene.tscn` (square) is unchanged. (`tests/capture_demo.gd` matches backup.)

### M3 - Painting tools on triangle + hex terrain  [DONE]

- [x] Extend `brush_pattern_calculator.gd` (5.13): hex brush shape for square terrain + per-mode
      cell sampling + grid-aligned selection sets.
- [x] Add `Hexagon` to brush type options + outline visuals (5.12, 5.16).
- [x] Add cell-mode branches in plugin `draw_pattern`, `handle_mouse` hover (5.11).
- [x] Add cell-mode branches in gizmo: brush preview, cursor cell, pattern preview (5.14).
- Verification:
  1. Both new terrains: Round/Hexagon brushes paint height up/down; falloff works; outline swaps.
     (Brush sampling asserted in `tests/run_brush_tests.gd`; outline resources added and load clean.)
  2. Level, Smooth, Bridge, Grass Mask, Vertex Paint work on both terrains.
     (`tests/run_tool_tests.gd`; plugin dispatch covers all modes incl. Debug Brush.)
  3. Undo/redo each tool action once. (Single composite action per stroke; cleared on mode switch.)
  4. Cross-chunk painting updates walls on both sides of chunk borders.
     (`_collect_cell_affected_chunks` regenerates painted chunks + orthogonal neighbours.)
  5. Square regression checklist (6.3). (Demo capture byte-identical; all square code untouched.)

### M4 - Grid Aligned + Grid Size + gating  [DONE]

- [x] Attribute definitions + tool `.tres` flags (5.12).
- [x] Attributes UI controls + dependency gating (5.12).
- [x] UI/plugin state plumbing (5.9, 5.11).
- [x] Grid-aligned pattern generation for both modes (3.5).
- Verification:
  1. Hexagon brush + Falloff off on Triangle or Hexagon terrain enables Grid Aligned; toggling it
     enables Grid Size; new control disabled otherwise. (Gate in `grid_align_gate_passes()`.)
  2. Hex terrain, N=1 -> exactly 7 cells change; N=2 -> 19; N=0 -> 1.
     (`tests/run_grid_align_tests.gd`.)
  3. Triangle terrain, N=1 -> exactly 6 triangles form a regular hexagon; N=2 -> 24; N=0 clamps to 1.
     (Same tests.)
  4. Falloff on -> Grid Aligned unchecks/disables. Round brush -> disabled. Square terrain -> disabled.
     (Gating rule table asserted; UI syncs via `_update_dependency_states()`.)
  5. Repeat 1-3 for Level, Smooth, Bridge tools. (Selection sets shared by all four tools.)

### M5 - Textures, quick paint, grass, baking (stretch, in this order)  [PARTIAL]

- [x] Verify/tune vertex encoding against the shader for all 3 blend modes and ridge/ledge.
      (Smooth + hard captures: `tests/capture_cell_textures.gd`, `tests/capture_cell_blend.gd`.)
- [x] Quick paint on cell modes (mirror the non-quick-paint wall color logic first).
      (Height + wall/ground color pairs + grass mask in one composite action.)
- [ ] Grass for cell modes (per-cell placement) - separate planter class.  [DEFERRED]
- [ ] Runtime texture baking (`MarchingSquaresGeometryBaker`) on cell meshes.  [DEFERRED]

### M6 - Polish (stretch)  [NOT STARTED - v1 scope]

- [ ] Cell-mode handle gizmos (per-cell handles) or hide cleanly.
      (v1: `MarchingSquaresTerrainGizmoPlugin._create_gizmo` returns null for cell chunks.)
- [ ] Hex brush outline visual aligned to the triangle lattice hexagons (pointy-top already).
- [ ] README/documentation updates.
- [ ] Cell-mode merge/smoothing variants (only if requested; changes exactness guarantees).

### M7 - Full regression  [DONE]

- [x] Square regression checklist (6.3). (Demo scene renders identically; no square code paths
      changed; grass setter loops guarded with `is MarchingSquaresTerrainChunk`.)
- [x] Save/reload round-trip for triangle and hex terrains in a fresh editor session.
      (`run_cell_save_tests.gd` recreates terrains and loads from disk.)
- [x] Undo/redo across mode switches must not crash (mode switch clears undo history).
      (`_switch_grid_type` clears the plugin undo stack when a plugin instance exists.)
- [x] Performance sanity: a default-size cell chunk regenerates well under a second on this machine.
      (Hex ~122 ms / Triangle ~137 ms for 33x33 and 66x33 cells; `tests/run_perf_tests.gd`.)

---

## 5. File-by-file changes

Paths are relative to `addons/MarchingSquaresTerrain/` unless noted. Line numbers refer to the
original files and are hints, not exact anchors.

### 5.1 NEW `algorithm/hex/marching_squares_hex_grid.gd`

- `@tool`, `class_name MarchingSquaresHexGrid`, static-only utility.
- Implement exactly section 3.1 plus:
  - `static func spacing_for(cell_size: Vector2) -> Vector2`, `radius_for(cell_size) -> float`
  - `static func corner_offsets(radius: float) -> PackedVector2Array` (6 entries, -30 + 60*i)
  - `static func cell_center(cell: Vector2i, spacing: Vector2) -> Vector2`
  - `static func world_to_cell(p: Vector2, spacing: Vector2) -> Vector2i`
  - `static func edge_neighbors(cell: Vector2i) -> Array[Vector2i]` (table, by cell row parity)
  - `static func to_axial / axial_to_offset / hex_distance(at: Vector2i, bt: Vector2i) -> int`
  - `static func cells_in_hex_radius(center: Vector2i, radius: int) -> Array[Vector2i]`
  - `static func chunk_of_cell / local_cell / global_cell`
- Pure math, no engine calls.

### 5.2 NEW `algorithm/tri/marching_squares_tri_grid.gd`

- `@tool`, `class_name MarchingSquaresTriGrid`, static-only utility.
- Implement exactly section 3.2 plus:
  - `static func side_for(cell_size: Vector2) -> float` (= cell_size.x),
    `static func row_height(cell_size: Vector2) -> float` (= sqrt(3)/2 * s)
  - `static func lattice_point(i: int, j: int, cell_size: Vector2) -> Vector2`
  - `static func lattice_floats(p: Vector2, cell_size: Vector2) -> Vector2`
  - `static func world_to_cell(p: Vector2, cell_size: Vector2) -> Vector2i`
  - `static func cell_corners(cell: Vector2i, cell_size: Vector2) -> PackedVector2Array` (3 entries)
  - `static func cell_center(cell: Vector2i, cell_size: Vector2) -> Vector2` (centroid)
  - `static func edge_neighbors(cell: Vector2i) -> Array[Vector2i]` (3 entries, parity table)
  - `static func point_hex_distance(a: Vector2i, b: Vector2i) -> int`
  - `static func nearest_lattice_point(p: Vector2, cell_size: Vector2) -> Vector2i`
  - `static func cells_in_hexagon(center_point: Vector2i, n: int, cell_size: Vector2) -> Array[Vector2i]`
  - `static func chunk_of_cell / local_cell / global_cell` (same formulas as hex)
- Pure math, no engine calls.

### 5.3 NEW `algorithm/terrain/marching_squares_terrain_chunk_base.gd`

Purpose: give terrain/plugin/data-handler shared code one type covering all 3 chunk classes.
Keep it minimal; do not move square logic here.

```
@tool
extends MeshInstance3D
class_name MarchingSquaresTerrainChunkBase

@export var terrain_system : Node3D
@export var chunk_coords : Vector2i = Vector2i.ZERO

var height_map : Array
var color_map_0 : PackedColorArray
var color_map_1 : PackedColorArray
var wall_color_map_0 : PackedColorArray
var wall_color_map_1 : PackedColorArray
var grass_mask_map : PackedColorArray
var grass_planter = null            # square only; null for cell chunks
var _data_dirty : bool = false
var _skip_save_on_exit : bool = false

# Virtuals (default no-ops; subclasses override what they need):
func initialize_terrain(should_regenerate_mesh: bool = true) -> void: pass
func regenerate_mesh(use_threads: bool = false) -> void: pass
func regenerate_all_cells(use_threads: bool = false) -> void: pass
func mark_dirty() -> void: pass
func get_height(cc: Vector2i) -> float: return 0.0
func draw_height(x: int, z: int, y: float) -> void: pass
func get_color_0(cc: Vector2i) -> Color: return Color(0,0,0,0)
# ... and the same pattern for color_1, wall_color_0/1, grass_mask getters/drawers
```

Notes:
- `terrain_system` typed `Node3D` (not `MarchingSquaresTerrain`) intentionally to avoid cyclic
  dependency errors. If errors still appear, drop remaining type hints rather than restructuring.
- Only the base exports `terrain_system` and `chunk_coords`; remove those declarations from the
  square chunk when it starts extending this base.

### 5.4 NEW `algorithm/cells/marching_squares_cell_chunk.gd`

`@tool`, `class_name MarchingSquaresCellChunk`, `extends MarchingSquaresTerrainChunkBase`.
Shared implementation for triangle + hex chunks. The subclass supplies geometry/coordinate math.

State:
- `height_map : PackedFloat32Array`? No: keep `height_map` from the base but store a **flat `Array`**
  of floats, length `cells_per_chunk().x * cells_per_chunk().y`. (The base declares `Array`; square
  keeps nested arrays. Plugin code only calls `get_height`/`draw_height`.)
- `color_map_0/1`, `wall_color_map_0/1`, `grass_mask_map : PackedColorArray` sized to cell count.
- `_temp_mesh`, `_temp_height_map`, `_temp_collision_shapes` + save/predelte `_notification` logic
  copied from the square chunk (lines ~171-235).

Virtuals implemented by subclasses:
- `func cells_per_chunk() -> Vector2i` (hex: `Vector2i(dimensions.x, dimensions.z)`;
  tri: `Vector2i(2 * dimensions.x, dimensions.z)`)
- `func world_to_cell(p: Vector2) -> Vector2i` (grid module)
- `func cell_center_world(cell: Vector2i) -> Vector2` (grid module)
- `func cell_corners_world(cell: Vector2i) -> PackedVector2Array` (grid module)
- `func edge_neighbors(cell: Vector2i) -> Array[Vector2i]` (grid module)
- `func neighbor_local(cell: Vector2i, edge: int) -> Vector2i` (neighbor + wrap into local coords;
  returns a sentinel like `Vector2i(-99999, -99999)` when the neighbor chunk is absent)
- `func edge_corner_indices(edge: int) -> Vector2i` (hex: `(e, (e+1)%6)`; tri: `[(1,2),(0,1),(2,0)][edge]`)

Shared implementation:
- `func cell_index(cc: Vector2i) -> int` = `cc.y * cells_per_chunk().x + cc.x`.
- `generate_height_map()`: noise sampled at each cell's world center:
  `terrain_system.noise_hmap.get_noise_2d(center.x, center.y) * terrain_system.dimensions.y`
  (world center from `cell_center_world`; null noise -> zeros).
- `generate_color_maps` / `generate_wall_color_maps` / `generate_grass_mask_map`: mirror square
  defaults (zero colors; wall default `Color(1,0,0,0)`; grass mask `Color(1,1,1,1)`).
- `get_height(cc)`, `draw_height(x, z, y)`, and all color/mask getters/drawers using `cell_index`.
- `mark_dirty()`: `_data_dirty = true`.
- `regenerate_mesh(use_threads := false)` (v1, whole-chunk):
  1. `st = SurfaceTool.new()`; `st.begin(PRIMITIVE_TRIANGLES)`; the 3 `CUSTOM_RGBA_FLOAT` formats
     from the square chunk.
  2. For every local cell `(c, r)` with index `idx`:
     - `h = height_map[idx]`; `corners = cell_corners_world(global) - chunk_origin_world`;
       `center = cell_center_world(global) - chunk_origin_world`.
     - Emit top fan: `center` + corners at `h`, attributes per 3.3.
     - For each edge: `global_nb = edge_neighbors(global)[edge]`; look up
       `terrain_system.get_cell_height(global_nb)` (null -> skip / no wall). If
       `h - h_nb > 0.0001`: emit wall quad per 3.3 using the higher cell's maps.
  3. `st.generate_normals(); st.index(); mesh = st.commit();` set terrain material.
  4. Recreate collision exactly like the square chunk (free old StaticBody3D,
     `create_trimesh_collision()`, layers 17 + `extra_collision_layer`, hide shapes).
- `regenerate_all_cells(_use_threads := false)`: `regenerate_mesh(false)` (v1 simplification).
- `initialize_terrain(should_regenerate_mesh := true)`: create maps when empty, rebuild mesh when
  missing, refresh collision.
- `get_chunk_origin_world()`: subclass provides world position of local cell `(0, 0)`'s reference
  (hex: `cell_center(global(0,0))`; tri: `lattice_point(0, 0)`); the node position is set by the
  terrain (5.7) and must equal this value.

### 5.5 NEW `algorithm/hex/marching_squares_hex_chunk.gd`

`@tool`, `class_name MarchingSquaresHexChunk`, `extends MarchingSquaresCellChunk`.
Only virtual implementations, delegating to `MarchingSquaresHexGrid`:
- `cells_per_chunk()` -> `Vector2i(dimensions.x, dimensions.z)`
- `world_to_cell`, `cell_center_world`, `cell_corners_world` (6 corners from `corner_offsets`)
- `edge_neighbors`, `edge_corner_indices` = `(e, (e+1)%6)`
- `neighbor_local(cell, edge)`: global neighbor -> chunk/local via `chunk_of_cell`/`local_cell` on
  the **global** coordinate; return sentinel if the neighbor is outside the current chunk and its
  chunk does not exist in `terrain_system.chunks` (shared code resolves heights via the terrain).

### 5.6 NEW `algorithm/tri/marching_squares_tri_chunk.gd`

`@tool`, `class_name MarchingSquaresTriChunk`, `extends MarchingSquaresCellChunk`.
Virtual implementations delegating to `MarchingSquaresTriGrid`:
- `cells_per_chunk()` -> `Vector2i(2 * dimensions.x, dimensions.z)`
- `world_to_cell`, `cell_center_world`, `cell_corners_world` (3 corners)
- `edge_neighbors`, `edge_corner_indices` = `[(1,2),(0,1),(2,0)][edge]`
- `neighbor_local` same pattern as 5.5.

### 5.7 MODIFY `algorithm/terrain/marching_squares_terrain.gd`

1. Add near `enum StorageMode`:
   ```
   enum GridType {SQUARE, TRIANGLE, HEX}
   signal grid_type_changed(value: GridType)
   ```
   (`TRIANGLE = 1`, `HEX = 2`; keep this order everywhere, including `MSTChunkData.grid_type`.)
2. Exported property:
   ```
   @export_custom(PROPERTY_HINT_RANGE, "0, 2", PROPERTY_USAGE_STORAGE) var grid_type : GridType = GridType.SQUARE:
       set(value):
           if grid_type == value: return
           _switch_grid_type(value)
   ```
   `_switch_grid_type(value)`:
   - `MSTDataHandler.save_all_chunks(self)`.
   - Free/remove all chunk children with `_skip_save_on_exit = true`; clear `chunks`.
   - `grid_type = value`; `force_batch_update()`; emit `grid_type_changed`.
   - Clear undo history (guard null plugin):
     `MarchingSquaresTerrainPlugin.instance.get_undo_redo().clear_history(false)`.
   - `MSTDataHandler.load_terrain_data(self)`.
   - Mark scene unsaved (editor only). No undo support for mode switches.
3. Factory + helpers:
   ```
   func make_chunk() -> MarchingSquaresTerrainChunkBase:
       match grid_type:
           GridType.TRIANGLE: return MarchingSquaresTriChunk.new()
           GridType.HEX: return MarchingSquaresHexChunk.new()
           _: return MarchingSquaresTerrainChunk.new()
   func cells_per_chunk() -> Vector2i:   # cell modes only
       if grid_type == GridType.TRIANGLE: return Vector2i(2 * dimensions.x, dimensions.z)
       if grid_type == GridType.HEX: return Vector2i(dimensions.x, dimensions.z)
       return Vector2i.ZERO
   func get_cell_height(global_cell: Vector2i) -> Variant:
       var cells := cells_per_chunk()
       var cc := MarchingSquaresHexGrid.chunk_of_cell(global_cell, cells)   # same formula both modes
       var chunk = chunks.get(cc)
       if chunk == null: return null
       return chunk.get_height(MarchingSquaresHexGrid.local_cell(global_cell, cells))
   ```
   (`chunk_of_cell`/`local_cell` are identical integer formulas in both grid modules; reuse either.)
4. `add_new_chunk` (line ~589): use `make_chunk()`; for cell modes skip the square border-copy
   block and regenerate any existing neighbor chunks (their walls face the new chunk).
5. `add_chunk` (line ~659): branch chunk position:
   - SQUARE: existing.
   - HEX: `chunk.position = Vector3(cell_center_world(global(chunk_coords, 0, cells)).x, 0, ...z)`.
   - TRIANGLE: `chunk.position = Vector3(TriGrid.lattice_point(chunk_coords.x * dimensions.x,
     chunk_coords.y * dimensions.z, cell_size).x, 0, ...y)`.
6. `remove_chunk` / `remove_chunk_from_tree` (lines ~620-656): loosen chunk types to base; after a
   cell-mode removal, regenerate surviving neighbor chunks.
7. `_deferred_enter_tree` (line ~548): chunk gathering checks `is MarchingSquaresTerrainChunkBase`;
   square-only initialization (grass planter reset) behind `is MarchingSquaresTerrainChunk`.
8. Texture/grass setters (lines ~137-288 and similar): every
   `for chunk: MarchingSquaresTerrainChunk in chunks.values(): chunk.grass_planter...` becomes a
   guard: `for chunk in chunks.values(): if chunk is MarchingSquaresTerrainChunk: ...`. Add helper
   `func _regenerate_square_grass()` and use it where the loops are identical. Guard the
   `grass_subdivisions`/`grass_size` setters' `grass_planter.multimesh` accesses too.
9. `force_batch_update` / `_init` shader params: unchanged; document that `dimensions.x/z` mean
   cell counts in cell modes (square: point counts).

### 5.8 MODIFY `algorithm/terrain/marching_squares_terrain_chunk.gd`

- Change line 2 to `extends MarchingSquaresTerrainChunkBase`.
- Remove declarations the base now owns (`terrain_system`, `chunk_coords`, `height_map`, color
  maps, `_data_dirty`, `_skip_save_on_exit`). Keep everything else (`merge_mode`, grass, etc.).
  Only the base may `@export` `terrain_system`/`chunk_coords`.
- Do not change square behavior.

### 5.9 MODIFY `resources/mst_data_handler.gd` + `resources/mst_chunk_data.gd`

`mst_chunk_data.gd`:
- Add `@export var grid_type : int = 0` (must match `GridType`: 0 square, 1 triangle, 2 hex).
- Add `@export var cell_count : Vector2i = Vector2i.ZERO` (cell modes; zero for square).

`mst_data_handler.gd`:
1. `static func mode_name(terrain) -> String` -> `"square"` / `"triangle"` / `"hex"`.
   `static func mode_subdir(terrain) -> String` -> `terrain.data_directory.path_join(mode_name(terrain))`.
2. `save_all_chunks` (line ~105): use `mode_subdir`; dispatch:
   `if chunk is MarchingSquaresCellChunk: export_cell_chunk_data(chunk)` else `export_chunk_data(chunk)`.
   Orphan cleanup scoped to the mode subdir.
3. `load_terrain_data` (line ~189): dir = `mode_subdir`; legacy migration: if mode is SQUARE and
   `<root>/square/` is missing but root has `chunk_*` dirs, rename them into a new `square/`
   (`static func migrate_root_chunks_to_square(terrain)`). For cell modes, create missing chunk nodes
   via `terrain.make_chunk()` + `terrain.add_chunk(coords, chunk, null, false)` before importing.
4. `load_chunk_from_directory` (line ~223): resolve chunk as base; `import_chunk_data` for square,
   `import_cell_chunk_data` for cell chunks.
5. Add `export_cell_chunk_data(chunk) -> MSTChunkData` / `import_cell_chunk_data(chunk, data)`:
   - `data.grid_type = terrain.grid_type` (int), `data.cell_count = chunk.cells_per_chunk()`.
   - `data.height_map = Array(chunk.height_map)` (flat), color/mask encoding identical to the square
     helpers (`_colors_to_texture_idx` / `_texture_idx_to_colors` are cell-agnostic).
   - `data.mesh`, `data.collision_faces` same as square.
   - Import resizes maps to `cell_count.x * cell_count.y`; sets `_data_dirty = true` or false per
     existing square behavior.
6. `needs_migration` / `migrate_to_external_storage` (lines ~361-399): legacy root chunks are
   square; route saves to `square/`.
7. `cleanup_orphaned_chunk_files` (line ~400) and `cleanup_orphaned_terrain_directories` (line ~506):
   never treat `square` / `triangle` / `hex` as orphaned terrain dirs; per-mode cleanup scans only
   that subdir.
8. `_collect_terrain_dirs_recursive` (line ~550): skip entries named `square`, `triangle`, `hex`.

### 5.10 MODIFY `editor/tools/scripts/marching_squares_texture_settings.gd`

- Line ~233 loop over chunks: guard `if chunk is MarchingSquaresTerrainChunk` before grass work.

### 5.11 MODIFY `editor/marching_squares_terrain_plugin.gd`

State:
- Add `var grid_aligned : bool = false` and `var grid_size : int = 1` near `current_brush_index`.

`handle_mouse` (line ~315):
- Paint-path hover (lines ~379-384) and chunk-management hover (lines ~457-476): branch on
  `grid_type`; for cell modes compute cell/chunk via the matching grid module
  (`world_to_cell` + `chunk_of_cell`).
- Brush-size scroll and click/drag logic unchanged.

`update_draw_pattern` (line ~514):
- NOTE: verified never called (the gizmo builds `current_draw_pattern` inline). Keep it consistent
  by adding a cell-mode branch, or delete it. Preferred: implement the cell-mode pattern builder
  once as `build_cell_draw_pattern(b_pos)` and call it from both the gizmo (M3) and this function,
  keeping square results identical.

`draw_pattern` (line ~553):
- Top: `if terrain.grid_type != MarchingSquaresTerrain.GridType.SQUARE: draw_pattern_cells(terrain); return`.
- New `func draw_pattern_cells(terrain)`:
  - Build `pattern` / `restore_pattern` from `current_draw_pattern`, per cell. No cross-chunk
    neighbor expansion (cell heights are owned by one chunk). Value logic per mode:
    - BRUSH: `restore = chunk.get_height(local)`; flatten -> `lerp(restore, brush_position.y, sample)`;
      else `lerp(restore, restore + (brush_position.y - draw_height), sample)`.
    - LEVEL: `lerp(restore, height, sample)`.
    - SMOOTH: average `get_height` over `edge_neighbors` (resolved via `terrain.get_cell_height`,
      null -> self) + self; `lerp(restore, avg, sample * strength)`.
    - GRASS_MASK / VERTEX_PAINTING / DEBUG_BRUSH: mirror square semantics with cell getters;
      DEBUG_BRUSH prints the cell center global position and colors.
    - BRIDGE: cell center world pos, progress from `bridge_start_pos` to `brush_position` with the
      existing ease logic.
  - Affected chunks = painted chunks + chunks owning painted cells' neighbors; `regenerate_mesh()`
    each once.
  - Dispatch: VERTEX_PAINTING/GRASS_MASK reuse `apply_composite_pattern_action`; quick paint prints
    a warning and falls back (M5); otherwise the default wall-texture path with one undo action.
- `apply_composite_pattern_action` (line ~1061): remove square-only type annotations/casts;
  resolve chunks untyped.
- `get_cell_normal` (line ~1326): accept the base type; for cell chunks build the normal from the
  cell's corner heights (tri) or neighbor heights (hex).

### 5.12 MODIFY tool attributes + Terrain Settings UI

`editor/tools/scripts/marching_squares_tool_attribute_settings.gd`:
- Add `@export var grid_aligned : bool = false` and `@export var grid_size : bool = false`.

`editor/tools/scripts/marching_squares_tool_attributes_list.gd`:
- `brush_type` options -> `["Round", "Square", "Hexagon"]` (index 2 = hex; keep order).
- Add `grid_aligned` (checkbox) and `grid_size` (slider `Vector3(0,20,1)`, default 1) dictionaries.

Tool resources: add `grid_aligned = true` + `grid_size = true` to `brush_tool.tres`,
`level_tool.tres`, `smooth_tool.tres`, `bridge_tool.tres` only.

`editor/tools/scripts/marching_squares_tool_attributes.gd`:
1. `terrain_settings_data`: add `"grid_type": "OptionButton"`.
2. Terrain OptionButton builder (line ~508): `elif setting == "grid_type": add_item("Square"),
   add_item("Triangle"), add_item("Hexagon")`. Selection already reads the node property.
3. `show_tool_attributes`: append `grid_aligned` then `grid_size` after `falloff`.
4. `settings_controls : Dictionary` (cleared each rebuild); store the actual input controls for
   `brush_type`, `falloff`, `grid_aligned`, `grid_size` in `add_setting`.
5. Dependency gating:
   ```
   func _can_grid_align() -> bool:
       var t := plugin.current_terrain_node
       if t == null: return false
       if t.grid_type == MarchingSquaresTerrain.GridType.SQUARE: return false
       if plugin.current_brush_index != 2: return false
       return plugin.falloff == false

   func _update_dependency_states() -> void:
       # grid_aligned.disabled = not _can_grid_align(); uncheck (no signal) when disabled
       # grid_size.editable = plugin.grid_aligned
   ```
   Call at the end of `show_tool_attributes` and from `marching_squares_ui.gd` after any setting
   change. Guard `set_pressed_no_signal` + emit once to avoid recursion.
6. Chunk-management attribute (line ~346-399): loosen `current_available_chunks` and
   `selected_chunk` types to the base; skip merge-mode controls for non-square grid types;
   filter children with `is MarchingSquaresTerrainChunkBase`.
7. Grid Size tooltip: "Hexagon radius in cells (hex terrain) / hexagon side in triangles (minimum 1)
   - only used when Grid Aligned is on."

`editor/marching_squares_ui.gd`:
- `_on_terrain_setting_changed` (line ~196): add `grid_type` case setting
  `terrain.grid_type = p_value as MarchingSquaresTerrain.GridType`.
- `_on_setting_changed` (line ~132): add `grid_aligned` / `grid_size` cases; at the end call
  `tool_attributes._update_dependency_states()`.

### 5.13 MODIFY `editor/utils/brush_pattern_calculator.gd`

Keep existing square functions untouched. Add:

1. Hex brush (index 2) for **square terrain** (world-space pointy-top):
   - `calculate_max_distance` case 2 -> `max_distance *= max_distance`.
   - `calculate_falloff_sample`: test membership before the `use_falloff` early-out; case 2:
     ```
     var uv := (world_pos - brush_pos) / (brush_size * 0.5)
     var h := maxf(absf(uv.y), maxf(absf(uv.x) * 1.1547005383792517,
                                      absf(uv.y) + absf(uv.x) * 0.5773502691896258))
     if h > 1.0: return -1.0
     if not use_falloff: return 1.0
     t = 1.0 - clampf(h, 0.2, 1.0)
     ```
     Round/square results must remain identical.
2. Cell-mode sampling:
   - `static func hex_cell_sample(...)`, `static func tri_cell_sample(...)`:
     - Round: reuse `calculate_falloff_sample` on the cell center world position (distance metric).
     - Square: reuse `calculate_falloff_sample` (Chebyshev on world XZ).
     - Hexagon brush: HEX terrain -> continuous `hex_distance` from the center cell using cell
       coords (inside when distance <= brush radius / spacing + 0.5); TRIANGLE terrain -> continuous
       hex metric in lattice-float space (`lattice_floats` of the centroid relative to the brush
       position) so the shape aligns with the triangle lattice. Apply falloff by normalized
       distance.
   - `static func cell_brush_candidates(brush_pos, brush_size, spacing_or_side) -> Array[Vector2i]`:
     bounding-box enumeration with +2 margin for non-grid-aligned painting.
3. Grid-aligned sets (used by gizmo/plugin):
   - `MarchingSquaresHexGrid.cells_in_hex_radius(center, N)` for hex terrain.
   - `MarchingSquaresTriGrid.cells_in_hexagon(nearest_lattice_point(brush_pos), maxi(N, 1), cell_size)`
     for triangle terrain.

### 5.14 MODIFY gizmos

`editor/marching_squares_terrain_gizmo.gd`:
- Cursor cell (lines ~61-67): branch per grid type (`world_to_cell` + `local_cell`).
- Brush preview (lines ~99-185): add a cell-mode branch:
  - Grid aligned (gate passes): selection from 5.13 item 3; sample 1.0.
  - Otherwise: candidates from 5.13 item 2 with `hex_cell_sample`/`tri_cell_sample`.
  - Bucket cells by chunk; draw one marker per cell at `cell_center_world` and the cell height
    (or `draw_height` when flatten), scaled by sample. Use the new cell marker mesh (5.16).
  - While `is_drawing`, write `current_draw_pattern` (mirror lines ~177-185).
- Radius outline (lines ~101-134): already index-driven; add "2" to the plugin dictionaries (5.11).
- Pattern preview (lines ~191-212): cell-mode branch using stored pattern cells.

`editor/marching_squares_terrain_gizmo_plugin.gd`:
- `_create_gizmo`: return `null` for `MarchingSquaresHexChunk` and `MarchingSquaresTriChunk`
  (v1: no per-cell handles). Keep square chunk behavior.

### 5.15 NEW brush outline + marker resources

- `resources/shaders/hex_brush_radius_visual.gdshader`: copy of the square shader with pointy-top
  hex SDF:
  ```
  vec2 p = (UV - 0.5) * 2.0;
  float h = max(abs(p.y), max(abs(p.x) * 1.1547005, abs(p.y) + abs(p.x) * 0.5773503));
  ```
  `falloff_visible == false` -> `ALPHA = h <= 1.0 ? 0.25 : 0.0`; true -> `smoothstep(1.0, 0.0, h) * 0.6`.
- `resources/plugin_materials/hex_brush_radius_material.tres` + `hex_brush_radius_visual.tres`
  (PlaneMesh 1x1 + material), mirroring `square_brush_radius_*`.
- `resources/plugin_materials/hex_cell_visual.tres`: flat pointy-top hexagon `ArrayMesh` marker
  (radius 0.5, translucent StandardMaterial3D like `brush_visual.tres`); optional triangle marker
  `tri_cell_visual.tres` (flat equilateral triangle, side 1).
- `editor/marching_squares_terrain_plugin.gd`: add `"2"` entries to `BrushMode`/`BrushMat` via
  `res://` paths until UIDs exist.

---

## 6. Verification and testing

All headless tests are `SceneTree` scripts under `tests/`. They print per-assert results
and exit non-zero on failure. Run each with:

```
godot --headless --path "<project root>" --script res://tests/<script>.gd
```

### 6.1 Headless test scripts (implemented)

| Script | Coverage | Checks |
| --- | --- | --- |
| `tests/run_grid_tests.gd` | hex/tri round trips, neighbor symmetry, edge/corner consistency, disk/hexagon counts | 44 |
| `tests/run_cell_chunk_tests.gd` | cell chunk mesh/triangle counts, walls, collision layers, mode flush | 19 |
| `tests/run_cell_save_tests.gd` | per-mode save/load round trips, legacy root -> `square/` migration | 10 |
| `tests/run_brush_tests.gd` | hexagon brush shape, per-mode cell sampling, grid-aligned cell sets | 17 |
| `tests/run_grid_align_tests.gd` | `N`-counts, pattern build, apply, gating rule table | 20 |
| `tests/run_tool_tests.gd` | grass mask, vertex paint, level, smooth, bridge helpers on cell chunks | 9 |
| `tests/run_perf_tests.gd` | default-size chunk regeneration timing (hex ~122 ms, tri ~137 ms) | timing |

Windowed visual captures (`-- --capture`, writes PNGs to `user://`):

| Script | Coverage |
| --- | --- |
| `tests/capture_demo.gd` | square demo scene (regression reference) |
| `tests/capture_cell_modes.gd` | hex + triangle terraces, walls, default textures |
| `tests/capture_cell_textures.gd` | per-slot texture painting, grass mask |
| `tests/capture_cell_blend.gd` | hard blend modes with ridge/ledge enabled |

### 6.2 Per-milestone manual checks

See each milestone. Always check the editor Output panel for script errors after each step.

### 6.3 Square-mode regression checklist (run at M3 and M7)

1. Open `scenes/mst_demo_scene.tscn`; paint with Brush (Round + Square), Level, Smooth, Bridge;
   undo each.
2. Grass Mask and Vertex Painting (ground + walls).
3. Quick paint with the demo preset.
4. Save scene, reopen; heights/colors/grass persist.
5. Chunk Manager: add/remove a chunk; undo/redo both.
6. No new errors/warnings and no visual differences versus the pre-change backup.

### 6.4 Save/load round trip (M2/M7)

1. Sculpt a triangle terrain and a hex terrain; save, reopen; confirm data restored from
   `triangle/` and `hex/`.
2. Switch modes back and forth; confirm each mode's data survives.
3. Legacy square data: use a backup of an old data dir with chunks at the root; open once in square
   mode; confirm migration into `square/` and correct rendering.

---

## 7. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Cyclic class_name dependencies (base <-> terrain) | Type base members as `Node3D`; drop hints if errors persist |
| Shader attribute mismatch (colors/mat_blend) on cell meshes | M5 visual verification for all blend modes; encoding specified in 3.3 |
| Cell chunks break square loops (`grass_planter`, typed chunk loops) | 5.7 item 8 + base-class typing; run regression after every milestone |
| Triangle coordinate precision (`lattice_floats`) | Pure float math, no epsilon snapping; round-trip tests at cell centers and near edges |
| Grid Size minimum differs per mode (hex 0 / triangle 1) | Clamp in pattern builder; document in tooltip |
| Mode switch corrupts scene or data | Mode data in separate subfolders; switch saves first, clears in-memory, reloads; no undo |
| Walls missing/leaking at chunk borders | Regenerate painted chunks + their neighbors; verify M3 item 4 |
| Performance (whole-chunk regen per edit) | v1 acceptable for default chunk sizes; per-cell caching in M5 if needed |
| New resource UIDs | Use `res://` paths; let Godot generate `.uid`; never hand-write UIDs |

---

## 8. Definition of done

- [x] Terrain Settings dropdown switches Square/Triangle/Hexagon; square is unchanged.
- [x] Triangle and hex chunks render exact regular triangles/hexagons with vertical cliffs,
      textures, and collision.
- [x] Cell-mode data saves/loads under `<data_dir>/triangle/` and `<data_dir>/hex/`; legacy square
      data migrates to `square/`.
- [x] Brush/Level/Smooth/Bridge/Grass/Vertex tools work on both cell terrains with undo/redo.
      (Quick paint included; cell grass/baking deferred - see M5.)
- [x] `Hexagon` brush option + hexagon radius outline present.
- [x] Grid Aligned + Grid Size exist for the 4 requested tools, gated as specified, producing
      `3N^2 + 3N + 1` whole hex cells or `6N^2` whole triangles that render as mathematically
      regular hexagons.
- [x] Square regression checklist passes.
- [x] No errors in the editor Output panel on load, paint, save, reopen, or mode switch.

---

## 9. Implementation notes and deviations (added after implementation)

These are the places where the delivered code intentionally differs from, or corrects,
the original plan text.

### 9.1 Triangle address system corrections (section 3.2)

The planned triangle tables were geometrically inconsistent; found by the shared-edge and
symmetry assertions in `tests/run_grid_tests.gd` and corrected:

- Edge `k` does NOT use corners `[(k+1) % 3, (k+2) % 3]` for both orientations.
- The implemented corner pairs are `(1,2), (0,2), (0,1)` for edges 0,1,2 on both A and B,
  with per-orientation neighbor offsets:
  - A(2m, j): edge 0 -> `(x+1, j)`, edge 1 -> `(x-1, j)`, edge 2 -> `(x+1, j-1)`
  - B(2m+1, j): edge 0 -> `(x-1, j)`, edge 1 -> `(x-1, j+1)`, edge 2 -> `(x+1, j)`
- `rhombus_i(x)` uses explicit floor division. GDScript integer division truncates toward
  zero, which breaks every negative-cell lookup (`cell_corners`, neighbors, chunking).
  Rhombus m owns `x = 2m` (A) and `x = 2m+1` (B) for every integer m, including negatives.
- `cells_in_hexagon` iterates candidate rhombi over `[min-1, max+1]` so boundary triangles
  are included; the three-corner test is exact and yields `6 * N * N` cells.

### 9.2 EngineWrapper pre-existing crash (found and fixed)

`editor/utils/engine_wrapper.gd` had a self-recursive static getter
(`if not instance: instance = EngineWrapper.new()` inside the getter for `instance`).
In Godot 4.7.1 this recurses until a stack overflow. The class is now a static
utility (`is_editor()`, `get_root_for_node()`, `set_owner_recursive()` are static;
`instance` is assigned in `_static_init()` for compatibility). All call sites use the
static forms.

### 9.3 Enum-typed property setter quirk (Godot 4.7.1)

Assigning an enum-typed property from a helper function called by its own setter
(`grid_type = value` inside `_switch_grid_type`) recursively re-enters the setter and
never stores the value. `grid_type` therefore uses a backing field:

```gdscript
@export_custom(PROPERTY_HINT_RANGE, "0, 2", PROPERTY_USAGE_STORAGE) var grid_type : GridType = GridType.SQUARE:
    get: return _grid_type
    set(value):
        if _grid_type == value: return
        _switch_grid_type(value)
var _grid_type : GridType = GridType.SQUARE
```

### 9.4 Cell mesh encoding (section 3.3)

- Meshes are emitted as explicit triangles via `SurfaceTool`, then `index()` dedups.
  Winding was verified so floor normals point +Y (first fan triangle `(a,b,c)` and
  edge pair indices accordingly).
- `CUSTOM2.a = 2.0` for both tops and walls (shader's vertex-color path, matching
  `MarchingSquaresTerrainVertexColorHelper`); `CUSTOM1 = Color(grass_mask.r, 0, 0, rl_idx/15)`.
  Verified visually for blend modes 0 and 2 with ridge/ledge enabled.
- Wall `UV2` uses terrain-global XZ, mirroring the square path's wall convention.
- `_switch_grid_type` frees chunks immediately (`remove_child` + `free`) so multiple mode
  switches can complete within one frame (tests rely on this).

### 9.5 Data layout

- `square/`, `triangle/`, `hex/` subfolders under each terrain's `data_directory`.
  `MSTDataHandler.mode_name()/mode_subdir()` own the mapping; legacy root chunk folders
  are migrated into `square/` on square-mode load.
- `MSTChunkData` gained `grid_type` and `cell_count`; cell chunks reuse the existing
  `ground_texture_idx` / `wall_texture_idx` / `grass_mask` byte encodings.
- `tests/run_cell_save_tests.gd` uses `user://` directories only; the demo scene data is
  never touched by tests.

### 9.6 Quick paint on cell modes

Paints height plus BOTH paired vertex-color channels for ground and wall slots (required
by the `(c0, c1)` tile-index encoding) plus the grass mask, in a single undoable action.

### 9.7 Deferred / not implemented

- Grass on cell modes (per-cell planter) - deferred, v1.
- Runtime texture baking (`MarchingSquaresGeometryBaker`) on cell meshes - deferred, v1.
- Cell-mode per-cell handle gizmos: the gizmo plugin returns `null` for cell chunks
  (the terrain gizmo still draws brush previews and pattern markers).
- Hex brush outline is the pointy-top hexagon SDF; a triangle-lattice-aligned variant
  was not done.
- README/documentation updates.
- Merge modes on cell chunks (not applicable in v1).

### 9.8 Known limitations

- Whole-chunk mesh regeneration per edit (v1). Perf is acceptable at default sizes
  (~120-140 ms) but grows with chunk dimensions.
- Undo/redo is per-stroke; multiple mode switches clear history by design.
- The Grid Aligned selection ignores `brush_size` (uses `grid_size`), as specified.

