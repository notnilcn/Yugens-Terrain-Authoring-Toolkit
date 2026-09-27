extends SceneTree
## Verifies bridge smoothing on cell terrain: bridge cells blend into a
## continuous slanted surface (matching square-grid bridge behavior) instead
## of rendering as flat steps.
##   godot --headless --path . --script res://tests/run_bridge_smooth_tests.gd

const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")
const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var hex_terrain := MarchingSquaresTerrain.new()
	hex_terrain.dimensions = Vector3i(16, 10, 10)
	hex_terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(hex_terrain)
	_test_hex(hex_terrain)
	hex_terrain.free()

	var tri_terrain := MarchingSquaresTerrain.new()
	tri_terrain.dimensions = Vector3i(16, 10, 10)
	tri_terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(tri_terrain)
	_test_tri(tri_terrain)
	tri_terrain.free()

	_test_cross_chunk()

	if _failures == 0:
		print("BRIDGE SMOOTH TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("BRIDGE SMOOTH TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


## Builds a flat hex terrain, applies a planar bridge band along +x with the
## given slope, marks the band cells smooth and regenerates the mesh.
func _test_hex(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var cells := chunk.cells_per_chunk()
	for r in range(cells.y):
		for c in range(cells.x):
			chunk.draw_height(c, r, 0.0)
	
	var row := 5
	var col_start := 3
	var col_end := 12
	var start_x := chunk.cell_center_world(Vector2i(col_start, row)).x
	var end_x := chunk.cell_center_world(Vector2i(col_end, row)).x
	var start_y := 6.0
	var end_y := 0.0
	for r in range(row - 1, row + 2):
		for c in range(col_start, col_end + 1):
			var h := _plane_height(chunk.cell_center_world(Vector2i(c, r)).x, start_x, end_x, start_y, end_y)
			chunk.draw_height(c, r, h)
			chunk.draw_smooth(c, r, true)
	chunk.regenerate_mesh()
	
	# Interior corners average the planar heights exactly.
	var global := chunk.chunk_coords * cells + Vector2i(7, row)
	_check(terrain.get_cell_smooth(global), "hex cell marked smooth")
	var heights := chunk.cell_corner_heights(global)
	var offsets := chunk.cell_corner_offsets(global)
	var center := chunk.cell_center_world(global)
	for i in range(offsets.size()):
		var corner_x := center.x + offsets[i].x
		var expected := _plane_height(corner_x, start_x, end_x, start_y, end_y)
		_check(absf(heights[i] - expected) < 0.01, "hex corner %d on bridge plane (%.3f vs %.3f)" % [i, heights[i], expected])
	
	# Adjacent smooth cells share identical corner heights (no steps/cracks).
	var global_nb := HexGrid.edge_neighbors(global)[0]
	_check(terrain.get_cell_smooth(global_nb), "hex neighbour marked smooth")
	var keys_a := chunk._corner_keys(global)
	var keys_b := chunk._corner_keys(global_nb)
	var heights_b := chunk.cell_corner_heights(global_nb)
	var shared := 0
	for i in range(keys_a.size()):
		var j : int = keys_b.find(keys_a[i])
		if j != -1:
			shared += 1
			_check(absf(heights[i] - heights_b[j]) < 0.0001, "hex shared corner %d continuous" % i)
	_check(shared == 2, "hex cells share an edge (2 corners), got %d" % shared)
	
	# The bridge band has vertical gap walls against the flat ground.
	_check(_mesh_has_vertical_gap(chunk), "hex bridge emits gap walls to the ground")
	
	# Old trimesh collision and mesh are present.
	_check(chunk.mesh != null, "hex chunk mesh generated")


func _test_tri(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := terrain.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var cells := chunk.cells_per_chunk()
	for r in range(cells.y):
		for c in range(cells.x):
			chunk.draw_height(c, r, 0.0)
	
	var row := 5
	var col_start := 4
	var col_end := 20
	var start_x := chunk.cell_center_world(Vector2i(col_start, row)).x
	var end_x := chunk.cell_center_world(Vector2i(col_end, row)).x
	var start_y := 8.0
	var end_y := 0.0
	for r in range(row - 1, row + 2):
		for c in range(col_start, col_end + 1):
			var h := _plane_height(chunk.cell_center_world(Vector2i(c, r)).x, start_x, end_x, start_y, end_y)
			chunk.draw_height(c, r, h)
			chunk.draw_smooth(c, r, true)
	chunk.regenerate_mesh()
	
	var global := chunk.chunk_coords * cells + Vector2i(10, row)
	_check(terrain.get_cell_smooth(global), "tri cell marked smooth")
	var heights := chunk.cell_corner_heights(global)
	var offsets := chunk.cell_corner_offsets(global)
	var center := chunk.cell_center_world(global)
	for i in range(offsets.size()):
		var corner_x := center.x + offsets[i].x
		var expected := _plane_height(corner_x, start_x, end_x, start_y, end_y)
		_check(absf(heights[i] - expected) < 0.01, "tri corner %d on bridge plane (%.3f vs %.3f)" % [i, heights[i], expected])
	
	var global_nb := TriGrid.edge_neighbors(global)[0]
	_check(terrain.get_cell_smooth(global_nb), "tri neighbour marked smooth")
	var keys_a := chunk._corner_keys(global)
	var keys_b := chunk._corner_keys(global_nb)
	var heights_b := chunk.cell_corner_heights(global_nb)
	var shared := 0
	for i in range(keys_a.size()):
		var j : int = keys_b.find(keys_a[i])
		if j != -1:
			shared += 1
			_check(absf(heights[i] - heights_b[j]) < 0.0001, "tri shared corner %d continuous" % i)
	_check(shared == 2, "tri cells share an edge (2 corners), got %d" % shared)
	
	_check(_mesh_has_vertical_gap(chunk), "tri bridge emits gap walls to the ground")
	
	# Save/load keeps the smooth flags.
	var data := MSTDataHandler.export_cell_chunk_data(chunk)
	_check(data.smooth_map.size() == cells.x * cells.y, "smooth map exported")
	var fresh := MarchingSquaresTriChunk.new()
	fresh.name = "Chunk fresh"
	fresh.terrain_system = terrain
	fresh.chunk_coords = chunk.chunk_coords
	MSTDataHandler.import_cell_chunk_data(fresh, data)
	_check(fresh.get_smooth(Vector2i(10, row)), "smooth flag round-tripped")
	_check(not fresh.get_smooth(Vector2i(0, 0)), "unsmoothed cell stays flat after load")
	fresh.free()


func _plane_height(x: float, start_x: float, end_x: float, start_y: float, end_y: float) -> float:
	var progress := clampf((x - start_x) / maxf(end_x - start_x, 0.0001), 0.0, 1.0)
	return lerpf(start_y, end_y, progress)


## A slanted plane spanning two neighbouring chunks must stay continuous across
## the chunk border (corner averaging resolves cells in other chunks).
func _test_cross_chunk() -> void:
	for grid_type in [MarchingSquaresTerrain.GridType.HEX, MarchingSquaresTerrain.GridType.TRIANGLE]:
		var terrain := MarchingSquaresTerrain.new()
		terrain.dimensions = Vector3i(4, 10, 6)
		terrain.cell_size = Vector2(2.0, 2.0)
		get_root().add_child(terrain)
		terrain.grid_type = grid_type
		var chunk_a : MarchingSquaresCellChunk = terrain.make_chunk() as MarchingSquaresCellChunk
		chunk_a.name = "Chunk 0_0"
		terrain.add_chunk(Vector2i(0, 0), chunk_a, null, true)
		var chunk_b : MarchingSquaresCellChunk = terrain.make_chunk() as MarchingSquaresCellChunk
		chunk_b.name = "Chunk 1_0"
		terrain.add_chunk(Vector2i(1, 0), chunk_b, null, true)
		var cells : Vector2i = terrain.cells_per_chunk()
		for chunk in [chunk_a, chunk_b]:
			for r in range(cells.y):
				for c in range(cells.x):
					var global : Vector2i = chunk.chunk_coords * cells + Vector2i(c, r)
					var center : Vector2 = chunk.cell_center_world(global)
					chunk.draw_height(c, r, center.x * 0.5)
					chunk.draw_smooth(c, r, true)
		chunk_a.regenerate_mesh()
		chunk_b.regenerate_mesh()
		
		# A cell one column inside the second chunk has all corners shared with
		# smoothed cells from both chunks, so its top must follow the plane.
		var global_cell : Vector2i = chunk_b.chunk_coords * cells + Vector2i(1, 1)
		var heights := chunk_b.cell_corner_heights(global_cell)
		var offsets := chunk_b.cell_corner_offsets(global_cell)
		var center := chunk_b.cell_center_world(global_cell)
		var on_plane := true
		for i in range(offsets.size()):
			var expected := (center.x + offsets[i].x) * 0.5
			if absf(heights[i] - expected) > 0.01:
				on_plane = false
		var label := "hex" if grid_type == MarchingSquaresTerrain.GridType.HEX else "tri"
		_check(on_plane, label + " cross-chunk corners follow the plane")
		_check(not _mesh_has_vertical_gap(chunk_b), label + " cross-chunk border has no steps")
		
		# The two cells straddling the chunk border must agree at the border.
		var global_a : Vector2i = chunk_a.chunk_coords * cells + Vector2i(cells.x - 1, 1)
		var global_b : Vector2i = chunk_b.chunk_coords * cells + Vector2i(0, 1)
		var heights_a := chunk_a.cell_corner_heights(global_a)
		var heights_b := chunk_b.cell_corner_heights(global_b)
		var keys_a := chunk_a._corner_keys(global_a)
		var keys_b := chunk_b._corner_keys(global_b)
		var border_shared := 0
		for i in range(keys_a.size()):
			var j : int = keys_b.find(keys_a[i])
			if j != -1:
				border_shared += 1
				_check(absf(heights_a[i] - heights_b[j]) < 0.0001, label + " border corner %d continuous" % i)
		_check(border_shared == 2, label + " border cells share an edge, got %d" % border_shared)
		terrain.free()



## True when the mesh contains vertices stacked on the same XZ (a vertical
## wall), which only happens where a sloped bridge meets flat ground.
func _mesh_has_vertical_gap(chunk: MarchingSquaresCellChunk) -> bool:
	if chunk.mesh == null:
		return false
	var arrays := chunk.mesh.surface_get_arrays(0)
	var verts : PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var by_xz : Dictionary = {}
	for v in verts:
		var key := Vector2i(roundi(v.x * 512.0), roundi(v.z * 512.0))
		if by_xz.has(key):
			if absf(float(by_xz[key]) - v.y) > 0.25:
				return true
		else:
			by_xz[key] = v.y
	return false

