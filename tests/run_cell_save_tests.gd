extends SceneTree
## Save/load round-trip for cell modes using explicit data directories.

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var test_dir := "user://mst_mode_test"
	var abs_dir := ProjectSettings.globalize_path(test_dir)
	# Clean previous runs.
	_rmrf(abs_dir)
	DirAccess.make_dir_recursive_absolute(abs_dir)
	
	_run_round_trip(abs_dir)
	
	var total := _failures
	if total == 0:
		print("SAVE/LOAD TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("SAVE/LOAD TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _run_round_trip(abs_dir: String) -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(6, 10, 4)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	# Set the backing field directly: data_directory's getter auto-fills when in
	# editor mode, which would override an explicit value.
	terrain.set("data_directory", abs_dir)
	
	# --- Hex mode: paint, then save via the data handler.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var hex_chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	hex_chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), hex_chunk, null, true)
	hex_chunk.draw_height(2, 1, 7.5)
	hex_chunk.draw_color_0(1, 1, Color(0, 1, 0, 0))
	hex_chunk.draw_color_1(1, 1, Color(0, 0, 1, 0))
	MSTDataHandler.save_all_chunks(terrain)
	_check(DirAccess.dir_exists_absolute(abs_dir.path_join("hex")), "hex mode dir created on save")
	_check(FileAccess.file_exists(abs_dir.path_join("hex/chunk_0_0/metadata.res")), "hex metadata saved")
	_check(not DirAccess.dir_exists_absolute(abs_dir.path_join("chunk_0_0")), "no root-level square dir for hex mode")
	
	# --- Triangle mode: paint, then save.
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var tri_chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	tri_chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), tri_chunk, null, true)
	tri_chunk.draw_height(3, 2, 4.25)
	MSTDataHandler.save_all_chunks(terrain)
	_check(FileAccess.file_exists(abs_dir.path_join("triangle/chunk_0_0/metadata.res")), "triangle metadata saved")
	_check(FileAccess.file_exists(abs_dir.path_join("hex/chunk_0_0/metadata.res")), "hex metadata still present")
	
	# --- Recreate the terrain from scratch (simulates reopening the scene):
	# create the node, set the mode, add a fresh chunk, then load from disk.
	var terrain2 := MarchingSquaresTerrain.new()
	terrain2.dimensions = Vector3i(6, 10, 4)
	terrain2.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain2)
	terrain2.set("data_directory", abs_dir)
	terrain2.grid_type = MarchingSquaresTerrain.GridType.HEX
	var reload_chunk := terrain2.make_chunk() as MarchingSquaresHexChunk
	reload_chunk.name = "Chunk 0_0"
	terrain2.add_chunk(Vector2i(0, 0), reload_chunk, null, true)
	MSTDataHandler.load_terrain_data(terrain2)
	_check(absf(reload_chunk.get_height(Vector2i(2, 1)) - 7.5) < 0.0001, "hex height persisted")
	var c0 : Color = reload_chunk.get_color_0(Vector2i(1, 1))
	_check(c0.g > 0.5 and c0.r < 0.5, "hex ground color persisted (green)")
	var c1 : Color = reload_chunk.get_color_1(Vector2i(1, 1))
	_check(c1.b > 0.5, "hex ground color 1 persisted (blue)")
	
	# --- Load the triangle data into a fresh terrain.
	var terrain3 := MarchingSquaresTerrain.new()
	terrain3.dimensions = Vector3i(6, 10, 4)
	terrain3.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain3)
	terrain3.set("data_directory", abs_dir)
	terrain3.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var tri_reload := terrain3.make_chunk() as MarchingSquaresTriChunk
	tri_reload.name = "Chunk 0_0"
	terrain3.add_chunk(Vector2i(0, 0), tri_reload, null, true)
	MSTDataHandler.load_terrain_data(terrain3)
	_check(absf(tri_reload.get_height(Vector2i(3, 2)) - 4.25) < 0.0001, "triangle height persisted")
	
	# --- Square mode creates its own subdir on save.
	terrain3.grid_type = MarchingSquaresTerrain.GridType.SQUARE
	var square_chunk := MarchingSquaresTerrainChunk.new()
	square_chunk.name = "Chunk 0_0"
	terrain3.add_chunk(Vector2i(0, 0), square_chunk, null, true)
	square_chunk.mark_dirty()
	MSTDataHandler.save_all_chunks(terrain3)
	_check(DirAccess.dir_exists_absolute(abs_dir.path_join("square")), "square mode dir created")
	
	terrain.free()
	terrain2.free()
	terrain3.free()


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
