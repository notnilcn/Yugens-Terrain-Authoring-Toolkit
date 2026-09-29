extends SceneTree
## Grid Aligned pattern tests: verifies the gating rules and that exactly the
## expected cells/groups change for a grid-aligned paint.

const BPC = preload("res://addons/MarchingSquaresTerrain/editor/utils/brush_pattern_calculator.gd")
const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")
const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")

var _failures := 0
var _checks := 0


func _initialize() -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(10, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	get_root().add_child(terrain)
	
	_test_counts(terrain)
	_test_pattern_build(terrain)
	_test_apply(terrain)
	_test_gate_rules(terrain)
	_test_outline(terrain)
	
	terrain.free()
	if _failures == 0:
		print("GRID ALIGN TESTS PASSED (", _checks, ")")
		quit(0)
	else:
		printerr("GRID ALIGN TESTS FAILED: ", _failures, "/", _checks)
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _test_counts(terrain: MarchingSquaresTerrain) -> void:
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 0).size() == 1, "hex N=0 -> 1")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1).size() == 7, "hex N=1 -> 7")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 2).size() == 19, "hex N=2 -> 19")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 3).size() == 37, "hex N=3 -> 37")
	
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1).size() == 6, "tri N=1 -> 6")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 2).size() == 24, "tri N=2 -> 24")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 3).size() == 54, "tri N=3 -> 54")
	_check(BPC.grid_aligned_cells(terrain, Vector2.ZERO, 0).size() == 6, "tri N=0 clamps to 6")


func _test_pattern_build(terrain: MarchingSquaresTerrain) -> void:
	# Replicates the plugin's grid-aligned branch.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var cells := terrain.cells_per_chunk()
	var pattern := {}
	for global in BPC.grid_aligned_cells(terrain, Vector2.ZERO, 2):
		var chunk_coords := HexGrid.chunk_of_cell(global, cells)
		var local := HexGrid.local_cell(global, cells)
		if not pattern.has(chunk_coords):
			pattern[chunk_coords] = {}
		pattern[chunk_coords][local] = 1.0
	var total := 0
	for key in pattern:
		total += pattern[key].size()
	_check(total == 19, "grid aligned hex N=2 pattern has 19 cells")
	
	# Same for triangle terrain.
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	cells = terrain.cells_per_chunk()
	pattern = {}
	for global in BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1):
		var chunk_coords := TriGrid.chunk_of_cell(global, cells)
		var local := TriGrid.local_cell(global, cells)
		if not pattern.has(chunk_coords):
			pattern[chunk_coords] = {}
		pattern[chunk_coords][local] = 1.0
	total = 0
	for key in pattern:
		total += pattern[key].size()
	_check(total == 6, "grid aligned tri N=1 pattern has 6 cells")


func _test_apply(terrain: MarchingSquaresTerrain) -> void:
	# A grid-aligned LEVEL paint should set exactly the selected cells.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var cells := terrain.cells_per_chunk()
	var selected := BPC.grid_aligned_cells(terrain, Vector2.ZERO, 1)
	var center_cell := HexGrid.world_to_cell(Vector2.ZERO, HexGrid.spacing_for(terrain.cell_size))
	var inside := 0
	for global in selected:
		var chunk_coords := HexGrid.chunk_of_cell(global, cells)
		if chunk_coords == Vector2i.ZERO:
			inside += 1
	_check(inside == 3, "hex disk at world origin clips to 3 cells inside chunk 0_0")
	# Re-run centered away from the border to get the full 7.
	selected = BPC.grid_aligned_cells(terrain, HexGrid.cell_center(Vector2i(4, 3), HexGrid.spacing_for(terrain.cell_size)), 1)
	_check(selected.size() == 7, "hex disk N=1 centered inside chunk has 7 cells")
	var target_height := 4.0
	var changed := 0
	for global in selected:
		var chunk_coords := HexGrid.chunk_of_cell(global, cells)
		if chunk_coords != Vector2i.ZERO:
			continue
		var local := HexGrid.local_cell(global, cells)
		var restore : float = chunk.get_height(local)
		var draw_value = lerp(restore, target_height, 1.0)
		chunk.draw_height(local.x, local.y, draw_value)
		changed += 1
	_check(changed == 7, "7 cells changed for N=1 centered on a full chunk")
	var center_local := HexGrid.local_cell(Vector2i(4, 3), cells)
	_check(absf(chunk.get_height(center_local) - target_height) < 0.0001, "center cell leveled to target")


## Mirror of the outline SDF used by the brush radius shader.
func _outline_h(terrain: MarchingSquaresTerrain, outline: Dictionary, p: Vector2) -> float:
	var center : Vector2 = outline["center"]
	var radius : float = outline["radius"]
	var rotation : float = outline["rotation"]
	var local := (p - center).rotated(-rotation) / radius
	return maxf(absf(local.y), maxf(absf(local.x) * 1.1547005383792517,
		absf(local.y) + absf(local.x) * 0.5773502691896258))


func _test_outline(terrain: MarchingSquaresTerrain) -> void:
	# Every selected cell's center must be inside the drawn outline, and the
	# first unselected ring must be outside it.
	terrain.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var n := 2
	var selected := BPC.grid_aligned_cells(terrain, Vector2.ZERO, n)
	var outline := BPC.grid_aligned_outline(terrain, Vector2.ZERO, n)
	var all_inside := true
	for cell in selected:
		var center := BPC.cell_center_for(terrain, cell)
		if _outline_h(terrain, outline, center) > 1.0001:
			all_inside = false
	_check(all_inside, "triangle outline encloses all selected cells")
	var outside_count := 0
	for i in range(-n - 2, n + 3):
		for j in range(-n - 2, n + 3):
			var cell := Vector2i(2 * i, j)
			if cell in selected:
				continue
			var center := BPC.cell_center_for(terrain, cell)
			if _outline_h(terrain, outline, center) > 1.0:
				outside_count += 1
	_check(outside_count > 0, "triangle outline excludes unselected cells")
	
	# Hexagon outline radius must cover the N-ring and exclude ring N+1.
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	n = 1
	outline = BPC.grid_aligned_outline(terrain, Vector2.ZERO, n)
	var spacing := MarchingSquaresHexGrid.spacing_for(terrain.cell_size)
	var center_cell := MarchingSquaresHexGrid.world_to_cell(Vector2.ZERO, spacing)
	var ring1_ok := true
	for cell in MarchingSquaresHexGrid.cells_in_hex_radius(center_cell, n):
		var center := BPC.cell_center_for(terrain, cell)
		if _outline_h(terrain, outline, center) > 1.0001:
			ring1_ok = false
	_check(ring1_ok, "hex outline encloses the radius-N disk")
	var inner := {}
	for cell in MarchingSquaresHexGrid.cells_in_hex_radius(center_cell, n):
		inner[cell] = true
	var ring2_ok := true
	for cell in MarchingSquaresHexGrid.cells_in_hex_radius(center_cell, n + 1):
		if inner.has(cell):
			continue
		var center := BPC.cell_center_for(terrain, cell)
		if _outline_h(terrain, outline, center) <= 1.0:
			ring2_ok = false
	_check(ring2_ok, "hex outline excludes the radius-N+1 ring")
	
	# N=0 selects a single pointy-top cell, so the outline must use the cell's
	# orientation and enclose its corners exactly.
	n = 0
	outline = BPC.grid_aligned_outline(terrain, Vector2.ZERO, n)
	_check(absf(outline["rotation"]) < 0.0001, "hex outline N=0 is pointy-top")
	var cell_center := MarchingSquaresHexGrid.cell_center(center_cell, spacing)
	var corners_inside := true
	for corner in MarchingSquaresHexGrid.corner_offsets(MarchingSquaresHexGrid.radius_for(terrain.cell_size)):
		if _outline_h(terrain, outline, cell_center + corner) > 1.001:
			corners_inside = false
	_check(corners_inside, "hex outline N=0 encloses the cell corners")


func _test_gate_rules(_terrain: MarchingSquaresTerrain) -> void:
	# Rule table from the plan: enabled only for hexagon brush, falloff off,
	# on triangle/hex terrain.
	var cases := [
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 2, "falloff": false, "expect": true},
		{"grid": MarchingSquaresTerrain.GridType.TRIANGLE, "brush": 2, "falloff": false, "expect": true},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 3, "falloff": false, "expect": true},
		{"grid": MarchingSquaresTerrain.GridType.TRIANGLE, "brush": 3, "falloff": false, "expect": true},
		{"grid": MarchingSquaresTerrain.GridType.SQUARE, "brush": 2, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.SQUARE, "brush": 3, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 0, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 1, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 4, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 5, "falloff": false, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 2, "falloff": true, "expect": false},
		{"grid": MarchingSquaresTerrain.GridType.HEX, "brush": 3, "falloff": true, "expect": false},
	]
	for case in cases:
		var passes : bool = case["grid"] != MarchingSquaresTerrain.GridType.SQUARE \
			and BPC.is_hexagon_brush(case["brush"]) and case["falloff"] == false
		_check(passes == case["expect"], "gate %s" % str(case))
