extends SceneTree
## TileMapLayer3D tile-key precision tests: COORD_SCALE=100 quantum, key round trips,
## range limits and MST lattice positions expressed in map grid units.

var _failures := 0
var _checks := 0


func _initialize() -> void:
	_test_scale_constants()
	_test_range_limits()
	_test_lattice_round_trip(0.25, 1.0, 1)
	_test_lattice_round_trip(0.25, 1.0, 2)
	_test_lattice_round_trip(0.5, 1.0, 1)
	_test_lattice_round_trip(1.0, 1.0, 1)
	_test_lattice_round_trip(2.0, 1.0, 1)
	_test_lattice_round_trip(0.125, 1.0, 1)
	# Aligned case: map grid size equals the MST cell size.
	_test_lattice_round_trip(0.25, 0.25, 1)
	_test_lattice_round_trip(0.5, 0.5, 2)

	if _failures == 0:
		print("TILEMAP KEY PRECISION TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("TILEMAP KEY PRECISION TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _test_scale_constants() -> void:
	_check(YugenTileKeySystem.COORD_SCALE == 100.0, "COORD_SCALE is raised to 100")
	_check(is_equal_approx(YugenTileKeySystem.get_precision(), 0.01), "key precision is 0.01")
	_check(is_equal_approx(YugenGlobalConstants.GRID_PRECISION, 0.01), "GRID_PRECISION matches")
	_check(is_equal_approx(YugenTileKeySystem.get_max_coordinate(), 327.67), "max coordinate is 327.67")


func _test_range_limits() -> void:
	_check(YugenGlobalConstants.MAX_GRID_RANGE < YugenTileKeySystem.get_max_coordinate(),
		"MAX_GRID_RANGE stays below the encoder limit")
	_check(YugenTileKeySystem.is_position_valid(Vector3(300, 0, -300)), "300 is a valid coordinate")
	_check(not YugenTileKeySystem.is_position_valid(Vector3(301, 0, 0)), "301 is outside the valid range")


## Emulates Grid Align placement: world lattice positions are converted to the map's grid
## units, stored with the tile key and read back. Keys must stay unique and the round trip
## must stay within the key quantum.
func _test_lattice_round_trip(cell_size: float, grid_size: float, multiplier: int) -> void:
	var lattice: float = cell_size * float(multiplier)
	var keys: Dictionary = {}
	for k in range(-4, 5):
		var positions: Array[Vector3] = [
			Vector3(k * lattice, 0.0, k * lattice),
			Vector3((k + 0.5) * lattice, 0.0, (k + 0.5) * lattice),
		]
		for world_pos: Vector3 in positions:
			var grid_pos: Vector3 = YugenGlobalUtil.world_to_grid(world_pos, grid_size)
			var key: int = YugenTileKeySystem.make_tile_key_int(grid_pos, 0)
			_check(not keys.has(key), "unique key for cell %.3f grid %.3f at %s" % [cell_size, grid_size, grid_pos])
			keys[key] = true

			var unpacked: Vector3 = YugenTileKeySystem.unpack_tile_key(key).position
			var world_back: Vector3 = YugenGlobalUtil.grid_to_world(unpacked, grid_size)
			# The lattice point itself may sit on a half-quantum; allow half a quantum per axis.
			var tolerance: float = YugenTileKeySystem.get_precision() * grid_size * 0.5 + 0.0001
			var error: Vector3 = (world_back - world_pos).abs()
			_check(error.x <= tolerance and error.y <= tolerance and error.z <= tolerance,
				"round trip for cell %.3f grid %.3f at %s (got %s)" % [cell_size, grid_size, world_pos, world_back])
