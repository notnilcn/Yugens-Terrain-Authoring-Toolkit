extends SceneTree
## Headless tests for cell-mode brush sampling and pattern application.
## Mirrors what the plugin/gizmo compute during a paint stroke.

const BPC = preload("res://addons/MarchingSquaresTerrain/editor/utils/brush_pattern_calculator.gd")
const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")
const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	_test_hex_brush_square_terrain(terrain)
	_test_hex_sampling(terrain)
	_test_tri_sampling(terrain)
	_test_grid_aligned_sets(terrain)
	_test_pattern_application(terrain)
	
	terrain.free()
	if _failures == 0:
		print("BRUSH TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("BRUSH TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _test_hex_brush_square_terrain(_terrain: MarchingSquaresTerrain) -> void:
	var curve := Curve.new()
	curve.add_point(Vector2(0, 0))
	curve.add_point(Vector2(1, 1))
	var max_d := BPC.calculate_max_distance(10.0, 2)
	# Center inside, corner of bounding box outside for a hexagon.
	var center_sample := BPC.calculate_falloff_sample(Vector2(0, 0), Vector2(0, 0), 10.0, 2, max_d, false, curve)
	_check(absf(center_sample - 1.0) < 0.0001, "hex brush center sample = 1 without falloff")
	var corner_sample := BPC.calculate_falloff_sample(Vector2(4.9, 4.9), Vector2(0, 0), 10.0, 2, max_d, false, curve)
	_check(corner_sample < 0, "hex brush bounding-box corner is outside")
	var edge_sample := BPC.calculate_falloff_sample(Vector2(0, 4.9), Vector2(0, 0), 10.0, 2, max_d, false, curve)
	_check(edge_sample > 0, "hex brush top edge is inside")
	# Round/square results unchanged.
	var round_sample := BPC.calculate_falloff_sample(Vector2(0, 0), Vector2(0, 0), 10.0, 0, BPC.calculate_max_distance(10.0, 0), false, curve)
	_check(round_sample == 1.0, "round brush center sample unchanged")


func _test_hex_sampling(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var spacing := HexGrid.spacing_for(terrain.cell_size)
	var max_d := BPC.calculate_max_distance(12.0, 0)
	var curve := Curve.new(); curve.add_point(Vector2(0, 0)); curve.add_point(Vector2(1, 1))
	var center_cell := Vector2i(3, 3)
	var center := HexGrid.cell_center(center_cell, spacing)
	var sample := BPC.hex_cell_sample(center, center, 12.0, 0, max_d, false, curve, center_cell, spacing)
	_check(sample == 1.0, "hex cell at brush center samples 1.0")
	# A cell far away is excluded.
	var far_cell := Vector2i(20, 20)
	var far_center := HexGrid.cell_center(far_cell, spacing)
	var far_sample := BPC.hex_cell_sample(center, far_center, 12.0, 0, max_d, false, curve, center_cell, spacing)
	_check(far_sample < 0, "hex far cell excluded")


func _test_tri_sampling(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var max_d := BPC.calculate_max_distance(12.0, 0)
	var curve := Curve.new(); curve.add_point(Vector2(0, 0)); curve.add_point(Vector2(1, 1))
	var cell := Vector2i(4, 2)
	var center := TriGrid.cell_center(cell, terrain.cell_size)
	var sample := BPC.tri_cell_sample(center, center, 12.0, 0, max_d, false, curve, terrain.cell_size)
	_check(sample == 1.0, "tri cell at brush center samples 1.0")
	var far := TriGrid.cell_center(Vector2i(30, 30), terrain.cell_size)
	var far_sample := BPC.tri_cell_sample(center, far, 12.0, 0, max_d, false, curve, terrain.cell_size)
	_check(far_sample < 0, "tri far cell excluded")


func _test_grid_aligned_sets(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var cells_n1 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1)
	_check(cells_n1.size() == 7, "grid aligned hex N=1 -> 7 cells")
	var cells_n2 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 2)
	_check(cells_n2.size() == 19, "grid aligned hex N=2 -> 19 cells")
	var cells_n0 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 0)
	_check(cells_n0.size() == 1, "grid aligned hex N=0 -> 1 cell")
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var tris_n1 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1)
	_check(tris_n1.size() == 6, "grid aligned tri N=1 -> 6 triangles")
	var tris_n2 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 2)
	_check(tris_n2.size() == 24, "grid aligned tri N=2 -> 24 triangles")
	var tris_n0 := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 0)
	_check(tris_n0.size() == 6, "grid aligned tri N=0 clamps to 6")


func _test_pattern_application(terrain: MarchingSquaresTerrain) -> void:
	# Simulate what the gizmo/plugin pattern accumulation + apply does.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var spacing := HexGrid.spacing_for(terrain.cell_size)
	var center_cell := Vector2i(3, 3)
	var center := HexGrid.cell_center(center_cell, spacing)
	
	# Accumulate a small pattern around the center.
	var pattern := {}
	for nb in HexGrid.cells_in_hex_radius(center_cell, 1):
		var local := HexGrid.local_cell(nb, terrain.cells_per_chunk())
		pattern[local] = 1.0
	_check(pattern.size() == 7, "pattern has 7 cells")
	
	# Apply heights as the plugin would.
	for local: Vector2i in pattern:
		chunk.draw_height(local.x, local.y, 5.0)
	_check(absf(chunk.get_height(center_cell) - 5.0) < 0.0001, "hex cell height applied")
	chunk.regenerate_mesh()
	var arrays : Array = chunk.mesh.surface_get_arrays(0)
	var idx : PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	_check(idx.size() > 0, "hex mesh regenerated after paint")


