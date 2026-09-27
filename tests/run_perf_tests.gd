extends SceneTree
## Performance sanity: time mesh regeneration for default-size cell chunks.

func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(33, 32, 33)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var hex_chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	hex_chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), hex_chunk, null, true)
	var t0 := Time.get_ticks_msec()
	hex_chunk.regenerate_mesh()
	var hex_ms := Time.get_ticks_msec() - t0
	print("hex chunk regen: ", hex_ms, " ms (", hex_chunk.cells_per_chunk(), " cells)")
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var tri_chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	tri_chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), tri_chunk, null, true)
	t0 = Time.get_ticks_msec()
	tri_chunk.regenerate_mesh()
	var tri_ms := Time.get_ticks_msec() - t0
	print("tri chunk regen: ", tri_ms, " ms (", tri_chunk.cells_per_chunk(), " cells)")
	
	terrain.free()
	print("PERF DONE")
	quit(0)
