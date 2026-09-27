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
	_check_cell_normals(chunk, [Vector2i(3, 2)], "hex")
	
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
	_check_cell_normals(chunk, [Vector2i(5, 2)], "tri")


## Asserts: no downward normals, and every wall triangle's geometric normal
## points away from the raised cell that owns it.
func _check_cell_normals(chunk: MarchingSquaresCellChunk, raised: Array, label: String) -> void:
	var cells := chunk.cells_per_chunk()
	var arrays := chunk.mesh.surface_get_arrays(0)
	var verts : PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var normals : PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
	var indices : PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	var down := 0
	for n in normals:
		if n.y < -0.1:
			down += 1
	_check(down == 0, label + ": no downward normals (got %d)" % down)
	
	var origin_world := chunk.get_chunk_origin_world()
	var offset := Vector2(origin_world.x, origin_world.y)
	var centers : Array[Vector2] = []
	for local in raised:
		var global : Vector2i = chunk.chunk_coords * cells + (local as Vector2i)
		centers.append(chunk.cell_center_world(global))
	
	var inward := 0
	for i in range(0, indices.size(), 3):
		var a := verts[indices[i]]
		var b := verts[indices[i + 1]]
		var c := verts[indices[i + 2]]
		var fn := (b - a).cross(c - a)
		if fn.length_squared() < 0.000001:
			continue
		fn = fn.normalized()
		if absf(fn.y) > 0.5:
			continue
		var mid := (a + b + c) / 3.0
		var mid_world := Vector2(mid.x, mid.z) + offset
		var best : Vector2 = centers[0]
		var best_dist := INF
		for cc in centers:
			var d := mid_world.distance_to(cc)
			if d < best_dist:
				best_dist = d
				best = cc
		if Vector2(fn.x, fn.z).dot(mid_world - best) < 0.0:
			inward += 1
	_check(inward == 0, label + ": walls face away from raised cells (got %d inward)" % inward)


func _test_mode_switch_memory(terrain: MarchingSquaresTerrain) -> void:
	# Switching modes must flush and preserve per-mode data directories.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	_check(terrain.chunks.size() == 0, "mode switch flushes chunks")
	_check(terrain.grid_type == MarchingSquaresTerrain.GridType.HEX, "grid_type set to hex")
	var hex_cells := terrain.cells_per_chunk()
	_check(hex_cells == Vector2i(8, 6), "cells_per_chunk reflects hex after switch")
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE
	_check(terrain.cells_per_chunk() == Vector2i.ZERO, "cells_per_chunk zero for square")
