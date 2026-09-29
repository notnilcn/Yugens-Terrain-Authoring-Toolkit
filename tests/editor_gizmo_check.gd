extends SceneTree
## Editor-only sanity check: the cell chunk gizmo can be instantiated and its
## height helpers work. Run with:
##   godot --headless --editor --quit-after 1 --script res://tests/editor_gizmo_check.gd

var _failures := 0


func _initialize() -> void:
	if not Engine.is_editor_hint():
		print("EDITOR GIZMO CHECK SKIPPED (not running in editor mode)")
		quit(0)
		return
	
	var gizmo := MarchingSquaresTerrainCellChunkGizmo.new()
	if gizmo == null:
		printerr("FAIL: could not create cell chunk gizmo")
		_failures += 1
		quit(1)
		return
	
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	gizmo.move_cell_height(chunk, 3 * 8 + 2, 3.5)
	if absf(chunk.get_height(Vector2i(2, 3)) - 3.5) > 0.0001:
		printerr("FAIL: gizmo move_cell_height did not update the height")
		_failures += 1
	
	# Triangle cell markers alternate orientation: A cells (even x) must have
	# their apex flipped to +Z, B cells (odd x) keep the mesh apex on -Z.
	var brush_gizmo := MarchingSquaresTerrainGizmo.new()
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var apex_a := brush_gizmo._cell_marker_basis(terrain, Vector2i(0, 0), 1.0) * Vector3(0, 0, -1)
	var apex_b := brush_gizmo._cell_marker_basis(terrain, Vector2i(1, 0), 1.0) * Vector3(0, 0, -1)
	if apex_a.z < 0.9 or apex_b.z > -0.9:
		printerr("FAIL: triangle cell marker orientation does not follow the cell column")
		_failures += 1
	terrain.free()
	
	if _failures == 0:
		print("EDITOR GIZMO CHECK PASSED")
		quit(0)
	else:
		quit(1)
