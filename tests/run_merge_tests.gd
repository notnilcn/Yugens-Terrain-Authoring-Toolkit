extends SceneTree
## Cell-mode merge mode tests: thresholds turn walls into ramps, mode changes
## regenerate the mesh, and merge_mode persists through save/load.

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	_test_hex_merge(terrain)
	_test_tri_merge(terrain)
	terrain.free()
	
	_test_merge_save_load()
	
	if _failures == 0:
		print("MERGE TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("MERGE TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


## Closest chunk-local distance from the raised cell's centre to any vertex at
## the base height (the wall/ramp bottom line).
func _bottom_distance(chunk: MarchingSquaresCellChunk, global: Vector2i) -> float:
	var arrays := chunk.mesh.surface_get_arrays(0)
	var verts : PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var uvs : PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
	var origin := chunk.get_chunk_origin_world()
	var center := chunk.cell_center_world(global) - origin
	var min_dist := INF
	for i in range(verts.size()):
		# Wall/ramp vertices are tagged with UV (1, 1); tops use (0, 0).
		if uvs[i] == Vector2(1, 1) and absf(verts[i].y) < 0.001:
			min_dist = minf(min_dist, Vector2(verts[i].x, verts[i].z).distance_to(center))
	return min_dist


func _edge_distance(chunk: MarchingSquaresCellChunk, global: Vector2i) -> float:
	var offsets := chunk.cell_corner_offsets(global)
	return ((offsets[0] + offsets[1]) * 0.5).length()


func _corner_distance(chunk: MarchingSquaresCellChunk, global: Vector2i) -> float:
	return chunk.cell_corner_offsets(global)[0].length()


## Ramp bottoms move each corner by `run` along the direction from the cell
## centre through the edge midpoint.
func _expected_ramp_distance(corner: float, edge: float, run: float) -> float:
	return sqrt(corner * corner + run * run + 2.0 * run * edge)


func _test_hex_merge(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	_check(chunk.merge_mode == MarchingSquaresCellChunk.Mode.CUBIC, "hex cell chunk defaults to CUBIC")
	
	var global := Vector2i(3, 2)
	var edge_dist := _edge_distance(chunk, global)
	var corner_dist := _corner_distance(chunk, global)
	
	# Step below the CUBIC threshold -> ramp pushed into the neighbour.
	chunk.draw_height(global.x, global.y, 0.5)
	chunk.regenerate_mesh()
	var ramp_dist := _bottom_distance(chunk, global)
	var expected_ramp := _expected_ramp_distance(corner_dist, edge_dist, 0.5)
	_check(absf(ramp_dist - expected_ramp) < 0.001,
		"hex CUBIC 0.5 step becomes a ramp (dist %.3f vs %.3f)" % [ramp_dist, expected_ramp])
	
	# Step above the threshold -> vertical wall at the shared edge.
	chunk.draw_height(global.x, global.y, 2.0)
	chunk.regenerate_mesh()
	var wall_dist := _bottom_distance(chunk, global)
	_check(absf(wall_dist - corner_dist) < 0.001,
		"hex CUBIC 2.0 step stays vertical (dist %.3f vs %.3f)" % [wall_dist, corner_dist])
	
	# Switching to SPHERICAL merges the same step.
	chunk.merge_mode = MarchingSquaresCellChunk.Mode.SPHERICAL
	_check(chunk.merge_threshold == 20.0, "hex SPHERICAL threshold stored")
	chunk.regenerate_mesh()
	var merged_dist := _bottom_distance(chunk, global)
	# The ramp run is capped at the edge distance so it stops before the
	# neighbour's centre.
	var merged_run := minf(2.0, edge_dist)
	var expected_merged := _expected_ramp_distance(corner_dist, edge_dist, merged_run)
	_check(absf(merged_dist - expected_merged) < 0.001,
		"hex SPHERICAL 2.0 step becomes a ramp (dist %.3f vs %.3f)" % [merged_dist, expected_merged])
	
	# Back to vertical for the normal checks.
	chunk.merge_mode = MarchingSquaresCellChunk.Mode.CUBIC
	chunk.draw_height(global.x, global.y, 2.0)
	chunk.regenerate_mesh()
	var arrays := chunk.mesh.surface_get_arrays(0)
	var normals : PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
	var down := 0
	for n in normals:
		if n.y < -0.1:
			down += 1
	_check(down == 0, "hex merged geometry keeps no downward normals")


func _test_tri_merge(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	
	var global := Vector2i(5, 2)
	var edge_dist := _edge_distance(chunk, global)
	var corner_dist := _corner_distance(chunk, global)
	chunk.draw_height(global.x, global.y, 0.4)
	chunk.regenerate_mesh()
	var ramp_dist := _bottom_distance(chunk, global)
	var expected_ramp := _expected_ramp_distance(corner_dist, edge_dist, 0.4)
	_check(absf(ramp_dist - expected_ramp) < 0.001,
		"tri CUBIC 0.4 step becomes a ramp (dist %.3f vs %.3f)" % [ramp_dist, expected_ramp])
	
	chunk.merge_mode = MarchingSquaresCellChunk.Mode.ROUNDED_POLYHEDRON
	_check(chunk.merge_threshold == 2.1, "tri ROUNDED_POLYHEDRON threshold stored")
	chunk.draw_height(global.x, global.y, 1.5)
	chunk.regenerate_mesh()
	var merged_dist := _bottom_distance(chunk, global)
	_check(merged_dist > edge_dist + 0.001,
		"tri ROUNDED_POLYHEDRON 1.5 step becomes a ramp")


func _test_merge_save_load() -> void:
	var test_dir := "user://mst_merge_test"
	var abs_dir := ProjectSettings.globalize_path(test_dir)
	_rmrf(abs_dir)
	DirAccess.make_dir_recursive_absolute(abs_dir)
	
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(6, 10, 4)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	terrain.set("data_directory", abs_dir)
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	chunk.merge_mode = MarchingSquaresCellChunk.Mode.SEMI_ROUND
	chunk.mark_dirty()
	MSTDataHandler.save_all_chunks(terrain)
	
	var terrain2 := MarchingSquaresTerrain.new()
	terrain2.dimensions = Vector3i(6, 10, 4)
	terrain2.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain2)
	terrain2.set("data_directory", abs_dir)
	terrain2.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk2 := terrain2.make_chunk() as MarchingSquaresHexChunk
	chunk2.name = "Chunk 0_0"
	terrain2.add_chunk(Vector2i(0, 0), chunk2, null, true)
	MSTDataHandler.load_terrain_data(terrain2)
	_check(chunk2.merge_mode == MarchingSquaresCellChunk.Mode.SEMI_ROUND, "cell merge_mode persists through save/load")
	
	terrain.free()
	terrain2.free()


func _rmrf(path: String) -> void:
	var dir := DirAccess.open(path)
	if not dir:
		return
	dir.list_dir_begin()
	var n := dir.get_next()
	while n != "":
		if dir.current_is_dir():
			_rmrf(path.path_join(n))
		else:
			dir.remove(n)
		n = dir.get_next()
	dir.list_dir_end()
	DirAccess.remove_absolute(path)
