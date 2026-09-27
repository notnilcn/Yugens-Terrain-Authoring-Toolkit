extends SceneTree
## TileMapLayer3D MST Grid Align tests: placement-manager snapping hook and node status
## validation (uniform square terrains, tile-key precision, unsupported types).

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(10, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	# Wait one frame so the terrain is inside the tree and transforms are real.
	await process_frame

	_test_disabled(terrain)
	_test_origin_snap(terrain)
	await _test_offset_terrain(terrain)
	_test_plane_constraints(terrain)
	_test_y_snap(terrain)
	_test_unsupported(terrain)
	await _test_node_status(terrain)

	terrain.free()
	if _failures == 0:
		print("TILEMAP MST ALIGN TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("TILEMAP MST ALIGN TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _check_vec3(actual: Vector3, expected: Vector3, message: String) -> void:
	_check(actual.is_equal_approx(expected), "%s (got %s, expected %s)" % [message, actual, expected])


func _make_manager() -> YugenTilePlacementManager:
	var manager := YugenTilePlacementManager.new()
	manager.grid_size = 1.0
	manager.grid_snap_size = 1.0
	return manager


func _test_disabled(terrain: MarchingSquaresTerrain) -> void:
	var manager: YugenTilePlacementManager = _make_manager()
	_check(not manager.is_mst_snap_active(), "context is inactive without terrain or enable flag")
	manager.set_mst_context(terrain, false, 0, 1, false, 1.0)
	_check(not manager.is_mst_snap_active(), "context stays inactive while disabled")
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.0, 1.3)), Vector3(1.0, 0.0, 1.0),
		"disabled context keeps upstream snapping")
	manager.clear_mst_context()


func _test_origin_snap(terrain: MarchingSquaresTerrain) -> void:
	var manager: YugenTilePlacementManager = _make_manager()
	manager.set_mst_context(terrain, true, 0, 1, false, 1.0)
	_check(manager.is_mst_snap_active(), "context is active for a square terrain")
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.0, 1.3)), Vector3(1.5, 0.0, 1.5),
		"placement projects onto the MST vertex lattice")
	manager.set_mst_context(terrain, true, 2, 1, false, 1.0)
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.0, 1.3)), Vector3(0.5, 0.0, 0.5),
		"center-only alignment snaps to the cell center lattice")
	manager.set_mst_context(terrain, true, 1, 1, false, 1.0)
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.0, 1.3)), Vector3(1.5, 0.0, 1.5),
		"vertex-only alignment snaps to the vertex lattice")
	manager.set_mst_context(terrain, true, 0, 2, false, 1.0)
	_check_vec3(manager.snap_to_grid(Vector3(3.4, 0.0, 3.4)), Vector3(3.5, 0.0, 3.5),
		"cell multiplier enlarges the lattice")


func _test_offset_terrain(terrain: MarchingSquaresTerrain) -> void:
	terrain.position = Vector3(0.3, 0.0, 0.3)
	await process_frame
	var manager: YugenTilePlacementManager = _make_manager()
	manager.set_mst_context(terrain, true, 0, 1, false, 1.0)
	# Upstream would produce (1,0,1); the MST lattice is offset by 0.3 world units,
	# so the nearest center/vertex in grid units shifts.
	_check_vec3(manager.snap_to_grid(Vector3(1.0, 0.0, 1.0)), Vector3(0.8, 0.0, 0.8),
		"offset terrain shifts the projected lattice")
	terrain.position = Vector3.ZERO
	await process_frame


func _test_plane_constraints(terrain: MarchingSquaresTerrain) -> void:
	var manager: YugenTilePlacementManager = _make_manager()
	manager.set_mst_context(terrain, true, 0, 1, false, 1.0)
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.7, 1.3), Vector3.UP), Vector3(1.5, 0.7, 1.5),
		"floor plane keeps Y and aligns XZ")
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 1.3, 1.3), Vector3.RIGHT), Vector3(1.3, 1.0, 1.5),
		"wall plane keeps X and aligns YZ")
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 1.3, 1.3), Vector3.FORWARD), Vector3(1.5, 1.0, 1.3),
		"forward plane keeps Z and aligns XY")


func _test_y_snap(terrain: MarchingSquaresTerrain) -> void:
	var manager: YugenTilePlacementManager = _make_manager()
	manager.set_mst_context(terrain, true, 0, 1, true, 0.5)
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.7, 1.3), Vector3.UP), Vector3(1.5, 0.5, 1.5),
		"Y snapping applies on the floor plane when enabled")


func _test_unsupported(terrain: MarchingSquaresTerrain) -> void:
	var manager: YugenTilePlacementManager = _make_manager()
	manager.set_mst_context(terrain, true, 0, 1, false, 1.0)
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	_check(not manager.is_mst_snap_active(), "triangle terrain disables the MST context")
	_check_vec3(manager.snap_to_grid(Vector3(1.3, 0.0, 1.3)), Vector3(1.0, 0.0, 1.0),
		"unsupported terrain keeps upstream snapping")
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE
	manager.set_mst_context(null, true, 0, 1, false, 1.0)
	_check(not manager.is_mst_snap_active(), "null terrain disables the MST context")


func _test_node_status(terrain: MarchingSquaresTerrain) -> void:
	var node := YugenTileMapLayer3D.new()
	get_root().add_child(node)
	await process_frame

	node.mst_terrain = terrain
	node.mst_grid_align = true
	var status: Dictionary = node.get_mst_grid_align_status()
	_check(status["active"], "node status is active for a square terrain")
	_check(is_equal_approx(status["cell_size"].x, 2.0), "node status reports the MST cell size")

	# A lattice half-spacing below the key quantum must be rejected with a reason.
	terrain.cell_size = Vector2(0.002, 0.002)
	status = node.get_mst_grid_align_status()
	_check(not status["active"], "sub-quantum lattice is rejected")
	_check(String(status["reason"]).contains("precision"), "rejection reason mentions the precision")
	terrain.cell_size = Vector2(2.0, 2.0)

	# Non-uniform cell sizes cannot be represented by a scalar grid size.
	terrain.cell_size = Vector2(2.0, 1.0)
	status = node.get_mst_grid_align_status()
	_check(not status["active"], "non-uniform cell size is rejected")
	_check(String(status["reason"]).contains("uniform"), "rejection reason mentions uniformity")
	terrain.cell_size = Vector2(2.0, 2.0)

	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	status = node.get_mst_grid_align_status()
	_check(not status["active"], "hex terrain is rejected")
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE

	# The node pushes its context to the placement manager.
	var manager: YugenTilePlacementManager = _make_manager()
	node._active_placement_manager = manager
	_check(manager.is_mst_snap_active(), "node pushes the MST context to the placement manager")
	node.mst_grid_align = false
	_check(not manager.is_mst_snap_active(), "disabling Grid Align clears the manager context")

	node.free()
