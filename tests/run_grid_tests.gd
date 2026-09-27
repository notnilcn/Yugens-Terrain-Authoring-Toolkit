extends SceneTree
## Headless math tests for MarchingSquaresHexGrid and MarchingSquaresTriGrid.
## Run with:
##   godot --headless --path <project root> --script res://tests/run_grid_tests.gd
## Prints per-assert results and exits non-zero on failure.

const HexGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/hex/marching_squares_hex_grid.gd")
const TriGrid = preload("res://addons/MarchingSquaresTerrain/algorithm/tri/marching_squares_tri_grid.gd")

const CELL_SIZE := Vector2(2.0, 2.0)
const TEST_RADIUS := 10 # cells/rhombi around origin

var _failures : int = 0
var _checks : int = 0


func _initialize() -> void:
	print("=== Grid tests: hex ===")
	_test_hex_round_trip()
	_test_hex_neighbor_symmetry()
	_test_hex_edge_corner_consistency()
	_test_hex_disk_counts()
	_test_hex_chunk_mapping()
	
	print("=== Grid tests: triangle ===")
	_test_tri_round_trip()
	_test_tri_neighbor_symmetry()
	_test_tri_edge_corner_consistency()
	_test_tri_corner_layout()
	_test_tri_hexagon_selection()
	_test_tri_nearest_lattice_point()
	_test_tri_chunk_mapping()
	
	print("=== Grid tests: shared chunk math ===")
	_test_shared_chunk_formulas()
	
	if _failures == 0:
		print("ALL TESTS PASSED (", _checks, " checks)")
		quit(0)
	else:
		printerr("TESTS FAILED: ", _failures, " of ", _checks, " checks failed")
		quit(1)


func _check(condition: bool, message: String) -> void:
	_checks += 1
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)


func _check_eq(a: Variant, b: Variant, message: String) -> void:
	_check(a == b, "%s (got %s, expected %s)" % [message, str(a), str(b)])


#region hex tests

func _test_hex_round_trip() -> void:
	var spacing := HexGrid.spacing_for(CELL_SIZE)
	var mismatches := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS, TEST_RADIUS + 1):
			var cell := Vector2i(x, y)
			var center := HexGrid.cell_center(cell, spacing)
			var back := HexGrid.world_to_cell(center, spacing)
			if back != cell:
				mismatches += 1
	_check_eq(mismatches, 0, "hex world<->cell round trip for all centers in 21x21 region")
	
	# Also verify points near edges map back to one of the two bordering cells.
	var edge_mismatches := 0
	for y in range(-3, 4):
		for x in range(-3, 4):
			var cell := Vector2i(x, y)
			var center := HexGrid.cell_center(cell, spacing)
			for i in range(6):
				var corner := center + HexGrid.corner_offsets(HexGrid.radius_for(CELL_SIZE))[i] * 0.999
				if HexGrid.hex_distance(HexGrid.world_to_cell(corner, spacing), cell) > 1:
					edge_mismatches += 1
	_check_eq(edge_mismatches, 0, "hex world_to_cell near corners maps to self or a direct neighbor")


func _test_hex_neighbor_symmetry() -> void:
	var failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS, TEST_RADIUS + 1):
			var cell := Vector2i(x, y)
			var neighbors := HexGrid.edge_neighbors(cell)
			if neighbors.size() != 6:
				failures += 1
				continue
			for edge in range(6):
				var nb : Vector2i = neighbors[edge]
				var nb_neighbors := HexGrid.edge_neighbors(nb)
				if not nb_neighbors.has(cell):
					failures += 1
	_check_eq(failures, 0, "hex neighbor tables are symmetric over the tested region")


func _test_hex_edge_corner_consistency() -> void:
	var spacing := HexGrid.spacing_for(CELL_SIZE)
	var corners := HexGrid.corner_offsets(HexGrid.radius_for(CELL_SIZE))
	var failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS, TEST_RADIUS + 1):
			var cell := Vector2i(x, y)
			var center := HexGrid.cell_center(cell, spacing)
			var neighbors := HexGrid.edge_neighbors(cell)
			for edge in range(6):
				var k0 := edge
				var k1 := (edge + 1) % 6
				var shared_a := (center + corners[k0] + center + corners[k1]) / 2.0
				var nb_center := HexGrid.cell_center(neighbors[edge], spacing)
				var d_self := center.distance_to(shared_a)
				var d_nb := nb_center.distance_to(shared_a)
				if absf(d_self - d_nb) > 0.0001:
					failures += 1
	_check_eq(failures, 0, "hex shared edges are equidistant from both cell centers")


func _test_hex_disk_counts() -> void:
	var expected := [1, 7, 19, 37, 61]
	var center := Vector2i(3, -2)
	for n in range(5):
		var cells := HexGrid.cells_in_hex_radius(center, n)
		_check_eq(cells.size(), expected[n], "hex disk count for N=%d" % n)
		var unique := {}
		var all_sane := true
		for c in cells:
			unique[c] = true
			if HexGrid.hex_distance(c, center) > n:
				all_sane = false
		_check_eq(unique.size(), cells.size(), "hex disk cells unique for N=%d" % n)
		_check(all_sane, "hex disk cells within distance N=%d" % n)
	_check_eq(HexGrid.cells_in_hex_radius(center, -1).size(), 0, "hex disk with negative radius is empty")


func _test_hex_chunk_mapping() -> void:
	var cells := Vector2i(8, 6)
	var failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS, TEST_RADIUS + 1):
			var cell := Vector2i(x, y)
			var chunk := HexGrid.chunk_of_cell(cell, cells)
			var local := HexGrid.local_cell(cell, cells)
			if local.x < 0 or local.x >= cells.x or local.y < 0 or local.y >= cells.y:
				failures += 1
			if HexGrid.global_cell(chunk, local, cells) != cell:
				failures += 1
	_check_eq(failures, 0, "hex chunk/local/global mapping round trip")

#endregion


#region triangle tests

func _test_tri_round_trip() -> void:
	var mismatches := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS * 2, TEST_RADIUS * 2 + 2):
			var cell := Vector2i(x, y)
			var center := TriGrid.cell_center(cell, CELL_SIZE)
			var back := TriGrid.world_to_cell(center, CELL_SIZE)
			if back != cell:
				mismatches += 1
	_check_eq(mismatches, 0, "triangle world<->cell round trip for all centroids in tested region")


func _test_tri_neighbor_symmetry() -> void:
	var failures := 0
	var edge_failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS * 2, TEST_RADIUS * 2 + 2):
			var cell := Vector2i(x, y)
			var neighbors := TriGrid.edge_neighbors(cell)
			if neighbors.size() != 3:
				failures += 1
				continue
			for edge in range(3):
				var nb : Vector2i = neighbors[edge]
				var nb_neighbors := TriGrid.edge_neighbors(nb)
				if not nb_neighbors.has(cell):
					failures += 1
				var shared_edge := _tri_shared_edge(cell, nb)
				if shared_edge == Vector2i(-1, -1):
					edge_failures += 1
				else:
					var expected := TriGrid.edge_corner_indices(cell, edge)
					if shared_edge != expected:
						edge_failures += 1
	_check_eq(failures, 0, "triangle neighbor tables are symmetric over the tested region")
	_check_eq(edge_failures, 0, "triangle edge indices match the shared corner pairs")


func _test_tri_edge_corner_consistency() -> void:
	var failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS * 2, TEST_RADIUS * 2 + 2):
			var cell := Vector2i(x, y)
			var corners := TriGrid.cell_corners(cell, CELL_SIZE)
			var center := TriGrid.cell_center(cell, CELL_SIZE)
			var neighbors := TriGrid.edge_neighbors(cell)
			for edge in range(3):
				var pair := TriGrid.edge_corner_indices(cell, edge)
				var edge_mid := (corners[pair.x] + corners[pair.y]) / 2.0
				var nb_center := TriGrid.cell_center(neighbors[edge], CELL_SIZE)
				if absf(center.distance_to(edge_mid) - nb_center.distance_to(edge_mid)) > 0.0001:
					failures += 1
				# The shared edge corners must lie on the neighbor's corner set.
				var nb_corners := TriGrid.cell_corners(neighbors[edge], CELL_SIZE)
				var matched_a := false
				var matched_b := false
				for nc in nb_corners:
					if nc.is_equal_approx(corners[pair.x]):
						matched_a = true
					if nc.is_equal_approx(corners[pair.y]):
						matched_b = true
				if not (matched_a and matched_b):
					failures += 1
	_check_eq(failures, 0, "triangle shared edges are identical for both cells")


func _test_tri_corner_layout() -> void:
	# A triangle (2i, j) = [L(i,j), L(i+1,j), L(i,j+1)]
	var a := TriGrid.cell_corners(Vector2i(4, 2), CELL_SIZE)
	var la := TriGrid.lattice_point(2, 2, CELL_SIZE)
	var lb := TriGrid.lattice_point(3, 2, CELL_SIZE)
	var lc := TriGrid.lattice_point(2, 3, CELL_SIZE)
	_check(a[0].is_equal_approx(la) and a[1].is_equal_approx(lb) and a[2].is_equal_approx(lc), "A triangle corners")
	
	# B triangle (2i+1, j) = [L(i+1,j+1), L(i+1,j), L(i,j+1)]
	var b := TriGrid.cell_corners(Vector2i(5, 2), CELL_SIZE)
	var lb2 := TriGrid.lattice_point(3, 3, CELL_SIZE)
	_check(b[0].is_equal_approx(lb2) and b[1].is_equal_approx(lb) and b[2].is_equal_approx(lc), "B triangle corners")
	
	# Every triangle must be equilateral with side == cell_size.x.
	var side := TriGrid.side_for(CELL_SIZE)
	var layout_failures := 0
	for y in range(-3, 4):
		for x in range(-6, 7):
			var corners := TriGrid.cell_corners(Vector2i(x, y), CELL_SIZE)
			for i in range(3):
				var d := corners[i].distance_to(corners[(i + 1) % 3])
				if absf(d - side) > 0.0001:
					layout_failures += 1
	_check_eq(layout_failures, 0, "every triangle is equilateral with side = cell_size.x")


func _test_tri_hexagon_selection() -> void:
	var expected := [6, 24, 54, 96]
	var center := TriGrid.nearest_lattice_point(Vector2(0.0, 0.0), CELL_SIZE)
	for n in range(1, 5):
		var cells := TriGrid.cells_in_hexagon(center, n, CELL_SIZE)
		_check_eq(cells.size(), expected[n - 1], "triangle hexagon count for N=%d" % n)
		var unique := {}
		var all_corners_inside := true
		for c in cells:
			unique[c] = true
			for corner in TriGrid.cell_corners(c, CELL_SIZE):
				var lp := TriGrid.nearest_lattice_point(corner, CELL_SIZE)
				if TriGrid.point_hex_distance(lp, center) > n:
					all_corners_inside = false
		_check_eq(unique.size(), cells.size(), "triangle hexagon cells unique for N=%d" % n)
		_check(all_corners_inside, "triangle hexagon corners within N=%d" % n)
	_check_eq(TriGrid.cells_in_hexagon(center, 0, CELL_SIZE).size(), 0, "triangle hexagon with N=0 is empty")


func _test_tri_nearest_lattice_point() -> void:
	var base := Vector2i(3, -1)
	var failures := 0
	for di in range(-2, 3):
		for dj in range(-2, 3):
			var lp := Vector2i(base.x + di, base.y + dj)
			var p := TriGrid.lattice_point(lp.x, lp.y, CELL_SIZE) + Vector2(0.05, 0.05)
			if TriGrid.nearest_lattice_point(p, CELL_SIZE) != lp:
				failures += 1
	_check_eq(failures, 0, "nearest_lattice_point finds the exact lattice point when very close")


func _test_tri_chunk_mapping() -> void:
	var cells := Vector2i(16, 6)
	var failures := 0
	for y in range(-TEST_RADIUS, TEST_RADIUS + 1):
		for x in range(-TEST_RADIUS * 2, TEST_RADIUS * 2 + 2):
			var cell := Vector2i(x, y)
			var chunk := TriGrid.chunk_of_cell(cell, cells)
			var local := TriGrid.local_cell(cell, cells)
			if local.x < 0 or local.x >= cells.x or local.y < 0 or local.y >= cells.y:
				failures += 1
			if TriGrid.global_cell(chunk, local, cells) != cell:
				failures += 1
	_check_eq(failures, 0, "triangle chunk/local/global mapping round trip")

#endregion


#region shared tests

func _test_shared_chunk_formulas() -> void:
	var cells := Vector2i(7, 5)
	var failures := 0
	for y in range(-8, 9):
		for x in range(-8, 9):
			var cell := Vector2i(x, y)
			if HexGrid.chunk_of_cell(cell, cells) != TriGrid.chunk_of_cell(cell, cells):
				failures += 1
			if HexGrid.local_cell(cell, cells) != TriGrid.local_cell(cell, cells):
				failures += 1
			if HexGrid.global_cell(HexGrid.chunk_of_cell(cell, cells), HexGrid.local_cell(cell, cells), cells) != cell:
				failures += 1
	_check_eq(failures, 0, "hex/tri chunk formulas are identical")


func _tri_shared_edge(a: Vector2i, b: Vector2i) -> Vector2i:
	var corners_a := TriGrid.cell_corners(a, CELL_SIZE)
	var corners_b := TriGrid.cell_corners(b, CELL_SIZE)
	var matches : Array[int] = []
	for i in range(3):
		for j in range(3):
			if corners_a[i].is_equal_approx(corners_b[j]):
				matches.append(i)
				break
	if matches.size() == 2:
		if matches[0] > matches[1]:
			return Vector2i(matches[1], matches[0])
		return Vector2i(matches[0], matches[1])
	return Vector2i(-1, -1)

#endregion
