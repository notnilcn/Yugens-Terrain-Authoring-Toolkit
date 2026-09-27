extends SceneTree
## YugenDualGridResolver tests: peering table, TileMapDual layout generation, generic
## mixing, bespoke mixes and layer-order overrides. Pure logic, no scene tree.

var _failures := 0
var _checks := 0


func _initialize() -> void:
	_test_tables()
	_test_empty()
	_test_single_source()
	_test_partial_masks()
	_test_generic_mixing()
	_test_diagonal_variants()
	_test_reverse_order()
	_test_layer_order_rules()
	_test_bespoke_mixes()
	_test_variant_for_terrain()
	_test_block_origin()

	if _failures == 0:
		print("DUAL GRID RESOLVER TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("DUAL GRID RESOLVER TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _check_vec2i(actual: Vector2i, expected: Vector2i, message: String) -> void:
	_check(actual == expected, "%s (got %s, expected %s)" % [message, actual, expected])


## Occupancy callable from a sparse map of world cell -> terrain id.
func _occupancy(cells: Dictionary) -> Callable:
	return func(cell: Vector2i) -> int:
		return int(cells.get(cell, YugenDualGridResolver.EMPTY))


## Builds occupancy for a display cell with the four contributing world cells described by
## `sources` in TL, TR, BL, BR order.
func _neighbourhood(display_coord: Vector2i, sources: Array) -> Callable:
	var cells: Dictionary = {}
	for i in range(4):
		cells[display_coord - YugenDualGridResolver.NEIGHBORS[i]] = sources[i]
	return _occupancy(cells)


func _test_tables() -> void:
	_check(YugenDualGridResolver.TERRAIN_TABLE.size() == 16, "peering table has 16 entries")
	_check_vec2i(YugenDualGridResolver.TERRAIN_TABLE[0], Vector2i(-1, -1), "mask 0 has no tile")
	_check_vec2i(YugenDualGridResolver.atlas_coords_for_mask(15), Vector2i(2, 1), "full mask is at (2, 1)")
	_check_vec2i(YugenDualGridResolver.atlas_coords_for_mask(15, true), Vector2i(6, 1),
		"mixed full mask is offset by +4 in X")

	# TileMapDual v5 "Standard" square preset, index = occupancy mask.
	var expected: Array[Vector2i] = [
		Vector2i(0, 3), Vector2i(3, 3), Vector2i(0, 2), Vector2i(1, 2),
		Vector2i(0, 0), Vector2i(3, 2), Vector2i(2, 3), Vector2i(3, 1),
		Vector2i(1, 3), Vector2i(0, 1), Vector2i(1, 0), Vector2i(2, 2),
		Vector2i(3, 0), Vector2i(2, 0), Vector2i(1, 1), Vector2i(2, 1),
	]
	_check(YugenDualGridResolver.standard_layout() == expected,
		"standard_layout reproduces the TileMapDual standard square preset")


func _test_empty() -> void:
	var occupancy := _occupancy({})
	var resolved: Dictionary = YugenDualGridResolver.resolve(Vector2i(3, 4), occupancy)
	_check(resolved["layers"].is_empty(), "empty neighbourhood produces no layers")
	_check(resolved["source_id"] == YugenDualGridResolver.EMPTY, "empty neighbourhood has no source")


func _test_single_source() -> void:
	var occupancy := _neighbourhood(Vector2i(2, 2), [7, 7, 7, 7])
	var resolved: Dictionary = YugenDualGridResolver.resolve(Vector2i(2, 2), occupancy)
	_check(resolved["layers"].size() == 1, "single terrain uses one layer")
	_check(resolved["source_id"] == 7, "single terrain source is preserved")
	_check_vec2i(resolved["atlas_coords"], Vector2i(2, 1), "single terrain uses the full variant")

	# A single terrain touching only one corner uses the matching edge variant.
	occupancy = _neighbourhood(Vector2i(0, 0), [9, -1, -1, -1])
	resolved = YugenDualGridResolver.resolve(Vector2i(0, 0), occupancy)
	_check(resolved["layers"].size() == 1, "single terrain with one occupied corner uses one layer")
	_check_vec2i(resolved["atlas_coords"], Vector2i(3, 3), "TL-only occupancy maps to (3, 3)")


func _test_partial_masks() -> void:
	for mask in range(1, 16):
		var sources: Array = []
		for i in range(4):
			sources.append(3 if (mask & (1 << i)) != 0 else -1)
		var coord := Vector2i(0, 0)
		var resolved: Dictionary = YugenDualGridResolver.resolve(coord, _neighbourhood(coord, sources))
		var expected: Vector2i = YugenDualGridResolver.TERRAIN_TABLE[mask]
		_check_vec2i(resolved["atlas_coords"], expected, "mask %d resolves to its table entry" % mask)


func _test_generic_mixing() -> void:
	# Four distinct terrains: one layer per world cell, default paint order 3,2,1,0.
	var sources: Array = [1, 2, 3, 4]
	var resolved: Dictionary = YugenDualGridResolver.resolve(Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources))
	_check(resolved["layers"].size() == 4, "four terrains produce four layers")
	var expected_layers: Array = [3, 2, 1, 0]
	var expected_sources: Array = [1, 2, 3, 4]
	for i in range(4):
		_check(resolved["layers"][i]["layer"] == expected_layers[i],
			"layer %d paints on display layer %d" % [i, expected_layers[i]])
		_check(resolved["layers"][i]["source_id"] == expected_sources[i],
			"layer %d keeps source %d" % [i, expected_sources[i]])
	_check(resolved["source_id"] == 1, "primary source is the tile's own cell")


func _test_diagonal_variants() -> void:
	# All four cells occupied; TL/BR are terrain 1, TR/BL are terrain 2.
	var sources: Array = [1, 2, 2, 1]
	var resolved: Dictionary = YugenDualGridResolver.resolve(Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources))
	_check(resolved["layers"].size() == 4, "diagonal mix paints all four slots")
	_check_vec2i(resolved["layers"][0]["atlas_coords"], Vector2i(5, 3),
		"TL terrain uses its mixed diagonal variant")
	_check_vec2i(resolved["layers"][1]["atlas_coords"], Vector2i(4, 0),
		"TR terrain uses its mixed half variant")
	_check_vec2i(resolved["layers"][2]["atlas_coords"], Vector2i(4, 2),
		"BL terrain uses its mixed half variant")
	_check_vec2i(resolved["layers"][3]["atlas_coords"], Vector2i(7, 3),
		"BR terrain uses its mixed diagonal variant")
	_check_vec2i(resolved["atlas_coords"], Vector2i(5, 3), "primary follows the tile's own cell")

	# Same diagonal without full occupancy uses non-mixed variants.
	sources = [1, 2, -1, 1]
	resolved = YugenDualGridResolver.resolve(Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources))
	_check_vec2i(resolved["atlas_coords"], Vector2i(1, 3), "non-full diagonal keeps the unmixed variant")


func _test_reverse_order() -> void:
	var sources: Array = [1, 2, 3, 4]
	var resolved: Dictionary = YugenDualGridResolver.resolve(
		Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources), {"reverse_order": true}
	)
	var expected_layers: Array = [0, 1, 2, 3]
	for i in range(4):
		_check(resolved["layers"][i]["layer"] == expected_layers[i],
			"reverse order paints slot %d on layer %d" % [i, expected_layers[i]])


func _test_layer_order_rules() -> void:
	var sources: Array = [1, 2, 1, 3]
	var rule := YugenLayerOrderOverrideRule.new()
	rule.mask = 0b0101  # TL + BL must match (both terrain 1)
	rule.order = Vector4i(1, 0, 3, 2)
	var resolved: Dictionary = YugenDualGridResolver.resolve(
		Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources), {"order_override_rules": [rule]}
	)
	var expected_layers: Array = [1, 0, 3, 2]
	for i in range(4):
		_check(resolved["layers"][i]["layer"] == expected_layers[i],
			"override rule paints slot %d on layer %d" % [i, expected_layers[i]])

	# A non-matching rule (TR + BL must match, but they differ) is ignored.
	var non_matching := YugenLayerOrderOverrideRule.new()
	non_matching.mask = 0b0110
	non_matching.order = Vector4i(3, 3, 3, 3)
	resolved = YugenDualGridResolver.resolve(
		Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources), {"order_override_rules": [non_matching]}
	)
	_check(resolved["layers"][0]["layer"] == 3, "non-matching override rule is ignored")


func _test_bespoke_mixes() -> void:
	var sources: Array = [1, 1, 2, 2]
	var rule := YugenBespokeMixRule.new()
	rule.primary_source_id = 2
	rule.secondary_source_id = 1
	rule.atlas_offset = 8

	_check(YugenDualGridResolver.find_bespoke_mix([rule], 1, 2) == [2, 8],
		"bespoke lookups are symmetric and prefer the rule's primary")
	_check(YugenDualGridResolver.find_bespoke_mix([rule], 3, 4).is_empty(),
		"unrelated terrains have no bespoke mix")

	var resolved: Dictionary = YugenDualGridResolver.resolve(
		Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources), {"bespoke_rules": [rule]}
	)
	_check(resolved["layers"].size() == 1, "bespoke mix collapses to one layer")
	_check(resolved["source_id"] == 2, "bespoke mix uses the rule's primary terrain")
	# Terrain 2 occupies BL + BR => mask 12 => table (3, 0) + offset 8.
	_check_vec2i(resolved["atlas_coords"], Vector2i(11, 0), "bespoke mix applies the atlas offset")

	# Without the rule, the same neighbourhood falls back to generic mixing.
	resolved = YugenDualGridResolver.resolve(Vector2i(0, 0), _neighbourhood(Vector2i(0, 0), sources))
	_check(resolved["layers"].size() == 4, "no rule => one generic mix layer per occupied slot")


func _test_variant_for_terrain() -> void:
	var sources: Array = [1, 2, 2, 2]
	var coord := Vector2i(0, 0)
	_check_vec2i(YugenDualGridResolver.resolve_variant_for_terrain(coord, _neighbourhood(coord, sources), 1),
		Vector2i(7, 3), "terrain variant adds the mix offset when fully occupied")
	_check_vec2i(YugenDualGridResolver.resolve_variant_for_terrain(coord, _neighbourhood(coord, sources), 2),
		Vector2i(5, 1), "dominant terrain keeps its mixed full-edge variant")

	sources = [1, -1, -1, -1]
	_check_vec2i(YugenDualGridResolver.resolve_variant_for_terrain(coord, _neighbourhood(coord, sources), 1),
		Vector2i(3, 3), "partial occupancy keeps the unmixed edge variant")
	_check_vec2i(YugenDualGridResolver.resolve_variant_for_terrain(coord, _neighbourhood(coord, sources), 9),
		Vector2i(-1, -1), "absent terrain has no variant")


func _test_block_origin() -> void:
	_check_vec2i(YugenDualGridResolver.block_origin_from_full(Vector2i(2, 1)), Vector2i(0, 0),
		"full tile at (2, 1) means a block origin at (0, 0)")
	_check_vec2i(YugenDualGridResolver.block_origin_from_full(Vector2i(10, 5)), Vector2i(8, 4),
		"block origin follows an offset full tile")
