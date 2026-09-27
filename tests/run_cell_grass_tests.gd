extends SceneTree
## Cell-mode grass tests: planter lifecycle, masking rules, placement bounds,
## height following, texture-based on/off, and save/load of the grass multimesh.
## NOTE: must run windowed - the dummy headless renderer does not store
## MultiMesh instance data, so transform readback is always the identity there.
##   godot --path . --script res://tests/run_cell_grass_tests.gd

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var test_dir := "user://mst_cell_grass_test"
	var abs_dir := ProjectSettings.globalize_path(test_dir)
	_rmrf(abs_dir)
	DirAccess.make_dir_recursive_absolute(abs_dir)
	
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(6, 10, 4)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	terrain.set("data_directory", abs_dir)
	
	_test_hex_planter(terrain)
	_test_tri_planter(terrain)
	terrain.free()
	
	_test_save_load(abs_dir)
	
	if _failures == 0:
		print("CELL GRASS TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("CELL GRASS TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _visible_count(mm: MultiMesh, from: int, to: int) -> int:
	var count := 0
	for i in range(from, to):
		var t := mm.get_instance_transform(i)
		if t.basis.get_scale().length_squared() > 0.001:
			count += 1
	return count


func _test_hex_planter(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	
	_check(chunk.grass_planter != null, "hex grass planter created")
	_check(chunk.grass_planter is MarchingSquaresCellGrassPlanter, "hex planter class")
	
	var subdiv : int = terrain.grass_subdivisions
	var per_cell := subdiv * subdiv
	var expected := 6 * 4 * per_cell
	_check(chunk.grass_planter.multimesh.instance_count == expected,
		"hex planter instance count = cells * subdivisions^2")
	
	# Default colors are (0,0,0,0) -> texture id 1 (base grass), mask defaults on.
	var base := (1 * 6 + 2) * per_cell
	_check(_visible_count(chunk.grass_planter.multimesh, base, base + per_cell) == per_cell,
		"hex cell starts fully grassed")
	
	# Masking hides every instance of that cell.
	chunk.draw_grass_mask(1, 1, Color(0, 0, 0, 0))
	chunk.regenerate_mesh()
	base = (1 * 6 + 1) * per_cell
	_check(_visible_count(chunk.grass_planter.multimesh, base, base + per_cell) == 0,
		"masked hex cell has no grass")
	
	# Non-grass texture slot (id 9 = blue/red) disables grass.
	chunk.draw_grass_mask(1, 1, Color(1, 0, 0, 0))
	chunk.draw_color_0(1, 1, Color(0, 0, 1, 0))
	chunk.draw_color_1(1, 1, Color(1, 0, 0, 0))
	chunk.regenerate_mesh()
	_check(_visible_count(chunk.grass_planter.multimesh, base, base + per_cell) == 0,
		"non-grass texture id places no grass")
	
	# Green mask channel forces grass on even for non-grass texture slots.
	chunk.draw_grass_mask(1, 1, Color(1, 1, 0, 0))
	chunk.regenerate_mesh()
	_check(_visible_count(chunk.grass_planter.multimesh, base, base + per_cell) == per_cell,
		"green mask channel forces grass on")
	
	# Height and placement: grass follows the cell height, stays inside the cell.
	chunk.draw_height(1, 1, 6.0)
	chunk.regenerate_mesh()
	var global := Vector2i(1, 1)
	var center := chunk.cell_center_world(global)
	var radius := MarchingSquaresHexGrid.radius_for(terrain.cell_size)
	var y_ok := true
	var bounds_ok := true
	for i in range(base, base + per_cell):
		var t : Transform3D = chunk.grass_planter.multimesh.get_instance_transform(i)
		if t.basis.get_scale().length_squared() <= 0.001:
			y_ok = false
			continue
		if absf(t.origin.y - 6.0) > 0.001:
			y_ok = false
		var origin : Vector3 = chunk.position + t.origin
		if Vector2(origin.x, origin.z).distance_to(center) > radius + 0.001:
			bounds_ok = false
	_check(y_ok, "hex grass instances follow cell height")
	_check(bounds_ok, "hex grass instances stay inside the cell")
	
	# Subdivisions change resizes the buffer.
	terrain.grass_subdivisions = 2
	_check(chunk.grass_planter.multimesh.instance_count == 6 * 4 * 2 * 2,
		"hex planter resizes on subdivision change")
	terrain.grass_subdivisions = 3


func _test_tri_planter(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	
	_check(chunk.grass_planter != null, "tri grass planter created")
	var subdiv : int = terrain.grass_subdivisions
	var per_cell := subdiv * subdiv
	_check(chunk.grass_planter.multimesh.instance_count == 12 * 4 * per_cell,
		"tri planter instance count = cells * subdivisions^2")
	
	# Place grass on a specific triangle and check centroid distance bounds.
	chunk.draw_height(5, 2, 3.0)
	chunk.regenerate_mesh()
	var base := (2 * 12 + 5) * per_cell
	var center := chunk.cell_center_world(Vector2i(5, 2))
	var corners := MarchingSquaresTriGrid.cell_corners(Vector2i(5, 2), terrain.cell_size)
	var max_dist := 0.0
	for c in corners:
		max_dist = maxf(max_dist, center.distance_to(c))
	var ok := true
	for i in range(base, base + per_cell):
		var t : Transform3D = chunk.grass_planter.multimesh.get_instance_transform(i)
		if t.basis.get_scale().length_squared() <= 0.001:
			continue
		var origin : Vector3 = chunk.position + t.origin
		if absf(t.origin.y - 3.0) > 0.001:
			ok = false
		if Vector2(origin.x, origin.z).distance_to(center) > max_dist + 0.001:
			ok = false
	_check(ok, "tri grass follows height and stays inside the cell")


func _test_save_load(abs_dir: String) -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(6, 10, 4)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	terrain.set("data_directory", abs_dir)
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	terrain.bake_grass = true
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	# Mask one cell so the persisted multimesh is distinguishable from a fresh one.
	chunk.draw_grass_mask(2, 2, Color(0, 0, 0, 0))
	chunk.regenerate_mesh()
	MSTDataHandler.save_all_chunks(terrain)
	
	var meta_path := abs_dir.path_join("hex/chunk_0_0/metadata.res")
	_check(FileAccess.file_exists(meta_path), "cell metadata saved with grass")
	var data : MSTChunkData = load(meta_path)
	_check(data != null and data.grass_multimesh != null, "grass multimesh stored in metadata")
	
	# Simulate reload: fresh terrain, then load and initialize.
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
	chunk2.initialize_terrain(false)
	
	var subdiv : int = terrain2.grass_subdivisions
	var per_cell := subdiv * subdiv
	var base := (2 * 6 + 2) * per_cell
	_check(_visible_count(chunk2.grass_planter.multimesh, base, base + per_cell) == 0,
		"masked cell grass state survives reload")
	
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
