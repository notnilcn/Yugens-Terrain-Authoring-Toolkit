extends SceneTree
## Cyclops MST Grid Align snapping tests: config-scene registration, MST snap
## passthrough, unsupported terrain handling, option plumbing and cache round trip.

const CONFIG_SCENE_PATH := "res://addons/YugenCyclopsLevelBuilder/gui/configuration.tscn"

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(10, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	# Wait one frame so the terrain is inside the tree and transforms are real.
	await process_frame

	_test_config_registration()
	_test_mst_snap(terrain)
	_test_unsupported(terrain)
	_test_options(terrain)
	_test_cache_round_trip()
	_test_grid_system_still_works()

	terrain.free()
	if _failures == 0:
		print("CYCLOPS SNAP TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("CYCLOPS SNAP TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _check_vec3(actual: Vector3, expected: Vector3, message: String) -> void:
	_check(actual.is_equal_approx(expected), "%s (got %s, expected %s)" % [message, actual, expected])


func _test_config_registration() -> void:
	var config:Node = load(CONFIG_SCENE_PATH).instantiate()
	_check(config != null, "configuration scene loads")
	if config:
		var mst_node := config.get_node_or_null("Snapping/View3D/Snap MST")
		_check(mst_node is YugenSnappingSystemMST, "config scene registers the MST snapping system")
		var ref_node := config.get_node_or_null("Views/View3D/Snapping/snap mst")
		_check(ref_node is SnapButtonRef, "config scene registers an MST snap button ref")
		if ref_node is SnapButtonRef:
			_check(ref_node.snapping_node == mst_node, "MST snap button points at the MST snapping system")
		config.free()


func _test_mst_snap(terrain: MarchingSquaresTerrain) -> void:
	var system := YugenSnappingSystemMST.new()
	system.mst_terrain = terrain
	_check(system.has_usable_terrain(), "square terrain is usable")

	var query := YugenSnappingQuery.new()
	_check_vec3(system._snap_point(Vector3(0.1, 1.2, 0.1), query), Vector3(0.0, 1.2, 0.0),
		"vertex snap through the Cyclops system")
	_check_vec3(system._snap_point(Vector3(0.9, 1.2, 0.9), query), Vector3(1.0, 1.2, 1.0),
		"center snap through the Cyclops system")

	system.align_kind = YugenSnappingSystemMST.AlignKind.CENTER_ONLY
	_check_vec3(system._snap_point(Vector3(0.1, 1.2, 0.1), query), Vector3(1.0, 1.2, 1.0),
		"center-only alignment")
	system.align_kind = YugenSnappingSystemMST.AlignKind.VERTEX_ONLY
	_check_vec3(system._snap_point(Vector3(0.9, 1.2, 0.9), query), Vector3(0.0, 1.2, 0.0),
		"vertex-only alignment")

	system.align_kind = YugenSnappingSystemMST.AlignKind.VERTEX_AND_CENTER
	system.y_snap = true
	system.y_snap_distance = 0.5
	_check_vec3(system._snap_point(Vector3(0.1, 1.2, 0.1), query), Vector3(0.0, 1.0, 0.0),
		"Y snapping through the Cyclops system")
	system.free()


func _test_unsupported(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var system := YugenSnappingSystemMST.new()
	system.mst_terrain = terrain
	_check(not system.has_usable_terrain(), "triangle terrain is not usable")
	var query := YugenSnappingQuery.new()
	_check_vec3(system._snap_point(Vector3(0.1, 1.2, 0.1), query), Vector3(0.1, 1.2, 0.1),
		"unsupported terrain passes the point through unchanged")

	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	_check(not system.has_usable_terrain(), "hex terrain is not usable")
	system.free()
	terrain.grid_type = MarchingSquaresTerrain.GridType.SQUARE


func _test_options(terrain: MarchingSquaresTerrain) -> void:
	var system := YugenSnappingSystemMST.new()
	system.mst_terrain = terrain
	system.cell_multiplier = 2
	system.align_kind = YugenSnappingSystemMST.AlignKind.CENTER_ONLY
	var query := YugenSnappingQuery.new()
	_check_vec3(system._snap_point(Vector3(1.5, 0.0, 1.5), query), Vector3(2.0, 0.0, 2.0),
		"cell multiplier scales the lattice")

	var options := system.build_snap_options()
	_check(int(options["cell_multiplier"]) == 2, "options carry the multiplier")
	_check(int(options["align_kind"]) == int(YugenSnappingSystemMST.AlignKind.CENTER_ONLY),
		"options carry the align kind")

	# Angle snapping falls back to 15 degrees without a global scene.
	var angle := system._snap_angle(deg_to_rad(20.0), query)
	_check(is_equal_approx(angle, deg_to_rad(15.0)), "angle snapping uses the default step")
	system.free()


func _test_cache_round_trip() -> void:
	var system := YugenSnappingSystemMST.new()
	system.load_from_cache({
		"terrain_path": "Terrain/Child",
		"align_kind": int(YugenSnappingSystemMST.AlignKind.CENTER_ONLY),
		"cell_multiplier": 4,
		"y_snap": true,
		"y_snap_distance": 0.25,
	})
	_check(system.mst_terrain_path == NodePath("Terrain/Child"), "cache restores the terrain path")
	_check(system.align_kind == YugenSnappingSystemMST.AlignKind.CENTER_ONLY, "cache restores the align kind")
	_check(system.cell_multiplier == 4, "cache restores the multiplier")
	_check(system.y_snap, "cache restores the Y snap toggle")
	_check(is_equal_approx(system.y_snap_distance, 0.25), "cache restores the Y distance")

	# flush_cache without a plugin must be a safe no-op.
	system.flush_cache()
	_check(true, "flush_cache without a plugin does not crash")

	# Invalid cache values are clamped.
	system.load_from_cache({"align_kind": 99, "cell_multiplier": -3, "y_snap_distance": -1.0})
	_check(system.align_kind == YugenSnappingSystemMST.AlignKind.VERTEX_AND_CENTER, "invalid align kind falls back")
	_check(system.cell_multiplier == 1, "multiplier is clamped to at least 1")
	_check(system.y_snap_distance >= 0.0, "Y distance is clamped to 0")
	system.free()


func _test_grid_system_still_works() -> void:
	var grid := YugenSnappingSystemGrid.new()
	grid.snap_to_grid_util.unit_size = 2.0
	var query := YugenSnappingQuery.new()
	_check_vec3(grid._snap_point(Vector3(1.1, 0.2, 3.2), query), Vector3(2.0, 0.0, 4.0),
		"upstream grid snapping is unchanged")
	grid.free()
