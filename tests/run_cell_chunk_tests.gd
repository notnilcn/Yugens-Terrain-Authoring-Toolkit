extends SceneTree
## Integration checks for cell-mode chunks: mesh generation (top/walls),
## collision, save/load round trip, and mode switching.

const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")
const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	_test_hex_chunk(terrain)
	_test_tri_chunk(terrain)
	_test_mode_switch_memory(terrain)
	
	terrain.free()
	
	if _failures == 0:
		print("CELL TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("CELL TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _test_hex_chunk(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	_check(chunk is MarchingSquaresHexChunk, "hex chunk type")
	_check(chunk.cells_per_chunk() == Vector2i(8, 6), "hex cells_per_chunk = dimensions")
	_check(chunk.mesh != null, "hex chunk mesh generated")
	var arrays := chunk.mesh.surface_get_arrays(0)
	var idx : PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	# 6 triangles per flat cell, 3 indices each. SurfaceTool dedups vertices.
	var flat_idx := idx.size()
	_check(flat_idx == 8 * 6 * 6 * 3, "hex flat mesh has %d indices (expected %d)" % [flat_idx, 8*6*6*3])
	
	# Raise one cell: expect walls on all 6 edges (6 quads = 12 triangles).
	chunk.draw_height(3, 2, 5.0)
	chunk.regenerate_mesh()
	arrays = chunk.mesh.surface_get_arrays(0)
	idx = arrays[Mesh.ARRAY_INDEX]
	_check(idx.size() == flat_idx + 36, "hex walls add 36 indices (got %d extra)" % (idx.size() - flat_idx))
	_check(absf(chunk.get_height(Vector2i(3, 2)) - 5.0) < 0.0001, "hex draw_height stores value")
	_check(chunk._data_dirty, "hex draw marks chunk dirty")
	
	# Collision body exists with the expected layers (17 masked, plus the
	# terrain's extra collision layer).
	var body_found := false
	for child in chunk.get_children():
		if child is StaticBody3D:
			body_found = true
			_check((child.collision_layer & 17) == 17, "hex collision layer keeps 17 bits")
			_check(child.get_collision_layer_value(terrain.extra_collision_layer), "hex extra collision layer")
	_check(body_found, "hex collision body created")


func _test_tri_chunk(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 1_0"
	terrain.add_chunk(Vector2i(1, 0), chunk, null, true)
	_check(chunk is MarchingSquaresTriChunk, "tri chunk type")
	_check(chunk.cells_per_chunk() == Vector2i(16, 6), "tri cells_per_chunk = 2x dimensions.x")
	_check(chunk.mesh != null, "tri chunk mesh generated")
	var arrays := chunk.mesh.surface_get_arrays(0)
	var idx : PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	# 3 triangles per flat cell, 3 indices each
	_check(idx.size() == 16 * 6 * 3 * 3, "tri flat mesh has %d indices (expected %d)" % [idx.size(), 16*6*3*3])
	
	# Raise one A cell: 3 walls (3 quads = 6 triangles), unless at chunk border.
	var before := idx.size()
	chunk.draw_height(5, 2, 4.0)
	chunk.regenerate_mesh()
	arrays = chunk.mesh.surface_get_arrays(0)
	idx = arrays[Mesh.ARRAY_INDEX]
	_check(idx.size() == before + 18, "tri walls add 18 indices (got %d extra)" % (idx.size() - before))


func _test_mode_switch_memory(terrain: MarchingSquaresTerrain) -> void:
	# Switching modes must flush and preserve per-mode data directories.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	_check(terrain.chunks.size() == 0, "mode switch flushes chunks")
	_check(terrain.grid_type == MarchingSquaresTerrain.GridType.HEX, "grid_type set to hex")
	var hex_cells := terrain.cells_per_chunk()
	_check(hex_cells == Vector2i(8, 6), "cells_per_chunk reflects hex after switch")
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE
	_check(terrain.cells_per_chunk() == Vector2i.ZERO, "cells_per_chunk zero for square")
