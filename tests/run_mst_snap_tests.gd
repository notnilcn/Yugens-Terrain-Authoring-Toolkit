extends SceneTree
## MSTGridSnap tests: vertex/center selection, cell-size multipliers, Y snapping,
## negative coordinates, terrain transforms and the unsupported-terrain flag.

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(10, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	# Wait one frame so the terrain is really inside the tree and its
	# transform participates in to_local()/to_global().
	await process_frame
	
	_test_support(terrain)
	_test_vertices_and_centers(terrain)
	_test_multipliers(terrain)
	_test_negative_coordinates(terrain)
	_test_y_snap(terrain)
	_test_transform(terrain)
	_test_candidate_helpers(terrain)
	
	terrain.free()
	if _failures == 0:
		print("MST SNAP TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("MST SNAP TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _check_vec2(actual: Vector2, expected: Vector2, message: String) -> void:
	_check(actual.is_equal_approx(expected), "%s (got %s, expected %s)" % [message, actual, expected])


func _check_vec3(actual: Vector3, expected: Vector3, message: String) -> void:
	_check(actual.is_equal_approx(expected), "%s (got %s, expected %s)" % [message, actual, expected])


func _test_support(terrain: MarchingSquaresTerrain) -> void:
	_check(MSTGridSnap.is_supported(terrain), "square terrain is supported")
	_check(not MSTGridSnap.is_supported(null), "null terrain is unsupported")
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	_check(not MSTGridSnap.is_supported(terrain), "triangle terrain is unsupported")
	var result := MSTGridSnap.snap(Vector3(1.234, 2.0, 5.678), terrain)
	_check(result["unsupported"], "triangle snap reports unsupported")
	_check_vec3(result["point"], Vector3(1.234, 2.0, 5.678), "triangle snap returns input unchanged")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(1.234, 5.678), terrain), Vector2(1.234, 5.678),
		"triangle snap_xz returns input unchanged")
	_check(MSTGridSnap.candidate_points(Vector2.ZERO, terrain).is_empty(),
		"triangle candidate_points is empty")
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	_check(not MSTGridSnap.is_supported(terrain), "hex terrain is unsupported")
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE
	_check(MSTGridSnap.is_supported(terrain), "terrain is supported again after switching back")


func _test_vertices_and_centers(terrain: MarchingSquaresTerrain) -> void:
	var vertex_kind := MSTGridSnap.AlignKind.VERTEX_ONLY
	var center_kind := MSTGridSnap.AlignKind.CENTER_ONLY
	_check_vec2(MSTGridSnap.snap_xz(Vector2(0.1, 0.2), terrain, 1, vertex_kind), Vector2.ZERO,
		"near vertex snaps to origin")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(2.2, 4.4), terrain, 1, vertex_kind), Vector2(2.0, 4.0),
		"near lattice vertex snaps to multiple of cell_size")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(0.4, 0.4), terrain, 1), Vector2.ZERO,
		"closer vertex wins over cell center")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(0.9, 0.9), terrain, 1), Vector2(1.0, 1.0),
		"closer cell center wins over vertex")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(3.4, 5.4), terrain, 1), Vector2(3.0, 5.0),
		"cell center is lattice vertex plus half cell")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(3.4, 5.4), terrain, 1, center_kind), Vector2(3.0, 5.0),
		"center-only kind returns the center lattice")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(3.4, 5.4), terrain, 1, vertex_kind), Vector2(4.0, 6.0),
		"vertex-only kind returns the vertex lattice")
	# Non-uniform cell sizes must stay per-axis.
	terrain.cell_size = Vector2(2.0, 1.0)
	_check_vec2(MSTGridSnap.snap_xz(Vector2(2.1, 1.4), terrain, 1, vertex_kind), Vector2(2.0, 1.0),
		"non-uniform cell_size snaps per axis")
	terrain.cell_size = Vector2(2.0, 2.0)


func _test_multipliers(terrain: MarchingSquaresTerrain) -> void:
	# Lattice grows with the multiplier: a 4x lattice puts a center at (2, 2).
	_check_vec2(MSTGridSnap.snap_xz(Vector2(1.9, 1.9), terrain, 2), Vector2(2.0, 2.0),
		"multiplier 2 snaps to half of the 4-unit lattice")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(2.1, 2.1), terrain, 2), Vector2(2.0, 2.0),
		"multiplier 2 keeps the nearest center")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(1.0, 1.0), terrain, 2), Vector2.ZERO,
		"multiplier 2 ties resolve to the vertex")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(1.0, 1.0), terrain, 3), Vector2.ZERO,
		"multiplier 3 vertex lattice")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(3.1, 3.1), terrain, 3), Vector2(3.0, 3.0),
		"multiplier 3 center lattice")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(3.9, 3.9), terrain, 4), Vector2(4.0, 4.0),
		"multiplier 4 center lattice")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(2.1, 2.1), terrain, 0), Vector2(2.0, 2.0),
		"multiplier below 1 clamps to 1")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(2.1, 2.1), terrain, -5), Vector2(2.0, 2.0),
		"negative multiplier clamps to 1")


func _test_negative_coordinates(terrain: MarchingSquaresTerrain) -> void:
	_check_vec2(MSTGridSnap.snap_xz(Vector2(-0.2, -0.2), terrain, 1), Vector2.ZERO,
		"negative near-vertex snaps to origin")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(-0.9, -0.9), terrain, 1), Vector2(-1.0, -1.0),
		"negative near-center snaps to (-1, -1)")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(-2.2, -3.1), terrain, 1, MSTGridSnap.AlignKind.VERTEX_ONLY),
		Vector2(-2.0, -4.0), "negative exact lattice vertex")
	_check_vec2(MSTGridSnap.snap_xz(Vector2(-3.0, -3.0), terrain, 2, MSTGridSnap.AlignKind.CENTER_ONLY),
		Vector2(-2.0, -2.0), "negative multiplier center lattice")


func _test_y_snap(terrain: MarchingSquaresTerrain) -> void:
	var base := Vector3(0.1, 1.2, 0.1)
	var result := MSTGridSnap.snap(base, terrain, {"y_snap": false})
	_check_vec3(result["point"], Vector3(0.0, 1.2, 0.0), "Y untouched when snapping is off")
	
	result = MSTGridSnap.snap(base, terrain, {"y_snap": true, "y_snap_distance": 0.5})
	_check_vec3(result["point"], Vector3(0.0, 1.0, 0.0), "Y snaps to the configured distance")
	
	result = MSTGridSnap.snap(base, terrain, {"y_snap": true, "y_snap_distance": 0.25})
	_check_vec3(result["point"], Vector3(0.0, 1.25, 0.0), "Y snaps to a smaller distance")
	
	result = MSTGridSnap.snap(base, terrain, {"y_snap": true, "y_snap_distance": 0.0})
	_check_vec3(result["point"], Vector3(0.0, 1.2, 0.0), "non-positive Y distance leaves Y untouched")
	
	result = MSTGridSnap.snap(base, terrain, {"cell_multiplier": 2, "align_kind": MSTGridSnap.AlignKind.CENTER_ONLY})
	_check_vec3(result["point"], Vector3(2.0, 1.2, 2.0), "snap options combine multiplier and kind")
	
	_check_vec3(MSTGridSnap.snap_point(base, terrain, {"y_snap": true, "y_snap_distance": 1.0}),
		Vector3(0.0, 1.0, 0.0), "snap_point convenience wrapper")


func _test_transform(terrain: MarchingSquaresTerrain) -> void:
	terrain.position = Vector3(5.0, 0.0, -3.0)
	terrain.rotation_degrees = Vector3(0.0, 90.0, 0.0)
	
	# Local (1.9, 2, 1.9) snaps to the vertex (2, 2, 2) in terrain space.
	var world_point := terrain.to_global(Vector3(1.9, 2.0, 1.9))
	var result := MSTGridSnap.snap(world_point, terrain)
	_check_vec3(result["point"], terrain.to_global(Vector3(2.0, 2.0, 2.0)),
		"snapping respects the terrain transform")
	_check_vec3(terrain.to_local(result["point"]), Vector3(2.0, 2.0, 2.0),
		"snapped point is the expected local lattice vertex")
	
	var scaled_world := terrain.to_global(Vector3(1.9, 3.7, 1.9))
	result = MSTGridSnap.snap(scaled_world, terrain, {"y_snap": true, "y_snap_distance": 1.0})
	_check_vec3(terrain.to_local(result["point"]), Vector3(2.0, 4.0, 2.0),
		"Y snapping happens in terrain-local space")
	
	terrain.position = Vector3.ZERO
	terrain.rotation_degrees = Vector3.ZERO


func _test_candidate_helpers(terrain: MarchingSquaresTerrain) -> void:
	var candidates := MSTGridSnap.candidate_points(Vector2(0.2, 0.2), terrain, 1)
	_check(candidates.size() == 2, "combined kind returns vertex and center candidates")
	_check_vec2(candidates[0], Vector2.ZERO, "first candidate is the vertex")
	_check_vec2(candidates[1], Vector2(1.0, 1.0), "second candidate is the center")
	_check(MSTGridSnap.candidate_points(Vector2(0.2, 0.2), terrain, 1, MSTGridSnap.AlignKind.VERTEX_ONLY).size() == 1,
		"vertex-only kind returns one candidate")
	_check(MSTGridSnap.nearest_kind(Vector2(0.2, 0.2), terrain, 1) == MSTGridSnap.AlignKind.VERTEX_ONLY,
		"nearest_kind detects the vertex lattice")
	_check(MSTGridSnap.nearest_kind(Vector2(0.8, 0.8), terrain, 1) == MSTGridSnap.AlignKind.CENTER_ONLY,
		"nearest_kind detects the center lattice")
