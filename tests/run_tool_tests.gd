extends SceneTree
## Simulates cell-mode tool strokes through the same chunk calls the plugin
## uses, verifying grass mask, vertex paint, level, smooth and bridge logic.

const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")
const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	_test_grass_and_vertex(terrain)
	_test_level_smooth(terrain)
	_test_bridge(terrain)
	
	terrain.free()
	if _failures == 0:
		print("TOOL TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("TOOL TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _test_grass_and_vertex(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	
	# Grass mask: masking removes grass.
	chunk.draw_grass_mask(2, 2, Color(0, 0, 0, 0))
	_check(chunk.get_grass_mask(Vector2i(2, 2)).r < 0.5, "grass mask clears")
	chunk.draw_grass_mask(2, 2, Color(1, 1, 1, 1))
	_check(chunk.get_grass_mask(Vector2i(2, 2)).r > 0.5, "grass mask restores")
	
	# Vertex paint ground and wall colors.
	chunk.draw_color_0(1, 1, Color(0, 1, 0, 0))
	chunk.draw_color_1(1, 1, Color(0, 0, 1, 0))
	_check(chunk.get_color_0(Vector2i(1, 1)).g > 0.5, "ground color 0 painted")
	_check(chunk.get_color_1(Vector2i(1, 1)).b > 0.5, "ground color 1 painted")
	chunk.draw_wall_color_0(1, 1, Color(1, 0, 0, 0))
	_check(chunk.get_wall_color_0(Vector2i(1, 1)).r > 0.5, "wall color 0 painted")


func _test_level_smooth(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	
	# Level: set a patch to 3.0.
	for r in range(2):
		for c in range(4):
			var restore : float = chunk.get_height(Vector2i(c, r))
			var draw_value = lerp(restore, 3.0, 0.75)
			chunk.draw_height(c, r, draw_value)
	_check(chunk.get_height(Vector2i(1, 1)) > 1.5, "level raises terrain")
	
	# Smooth: average neighbours into the cell.
	chunk.draw_height(2, 1, 10.0)
	chunk.draw_height(1, 1, 0.0)
	chunk.draw_height(3, 1, 0.0)
	var global := Vector2i(2, 1)
	var total := chunk.get_height(global)
	var count := 1.0
	for nb in chunk.edge_neighbors(global):
		var h_nb = terrain.get_cell_height(nb)
		if h_nb != null:
			total += h_nb
			count += 1.0
	var avg = total / count
	var smoothed = lerp(chunk.get_height(global), avg, 1.0 * 0.5)
	chunk.draw_height(global.x, global.y, smoothed)
	_check(chunk.get_height(global) < 10.0, "smooth lowered the spike")


func _test_bridge(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 1_0"
	terrain.add_chunk(Vector2i(1, 0), chunk, null, true)
	var start := Vector2(9.0, 1.0)
	var end := Vector2(9.0, 8.0)
	var bridge_length := (end - start).length()
	_check(bridge_length > 0.5, "bridge length valid")
	var cells := terrain.cells_per_chunk()
	var start_cell := HexGrid.world_to_cell(start, HexGrid.spacing_for(terrain.cell_size))
	var end_cell := HexGrid.world_to_cell(end, HexGrid.spacing_for(terrain.cell_size))
	var progress_start := (HexGrid.cell_center(start_cell, HexGrid.spacing_for(terrain.cell_size)) - start).dot((end - start) / bridge_length) / bridge_length
	var progress_end := (HexGrid.cell_center(end_cell, HexGrid.spacing_for(terrain.cell_size)) - start).dot((end - start) / bridge_length) / bridge_length
	_check(progress_end > progress_start, "bridge progress increases along the line")
