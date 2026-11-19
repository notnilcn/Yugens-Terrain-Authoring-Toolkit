# Headless check of the hex game API (kg_hex conventions):
#   godot --headless --path . --script res://tests/hex_api_check.gd
# Prints "HEX API OK" or the failed checks.
extends SceneTree

var fails := 0
var frames := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		_run()
		print("HEX API OK" if fails == 0 else "HEX API: %d failure(s)" % fails)
		quit(1 if fails else 0)
	return false


func _run() -> void:
	# Game scale: LargeHex = 6 units flat-to-flat, chunk == PatchCoord (19 cells).
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = 2
	t.cell_size = Vector2(6, 6)
	t.level_height = 0.5
	root.add_child(t)
	t.add_chunk(Vector2i(0, 0))
	t.add_chunk(Vector2i(1, 0))

	var spiral: Array = t.get_chunk_cells_spiral(Vector2i(0, 0))
	_check(spiral.size() == 19 and spiral[0] == Vector2i(0, 0) and spiral[1] == Vector2i(0, -1) and spiral[18] == Vector2i(-1, -1), "spiral order %s" % [spiral])
	_check(t.get_chunk_center_cell(Vector2i(1, 0)) == Vector2i(5, -2), "patch centre")
	_check(t.get_cell_chunk(Vector2i(5, -2)) == Vector2i(1, 0), "cell chunk")
	var w: Vector3 = t.chunk_to_world(Vector2i(1, 0))
	_check(w.is_equal_approx(Vector3(6 * (5 - 1), 0, 6 * sqrt(3.0) / 2.0 * -2)), "chunk_to_world %s" % w)

	# Levels in spiral order (a kg_world patch), read back through every accessor.
	var levels := PackedInt32Array()
	for i in 19:
		levels.append(i % 4)
	t.set_cell_levels(spiral, levels)
	_check(t.get_cell_level(spiral[5]) == 1 and is_equal_approx(t.get_cell_height(spiral[6]), 1.0), "set_cell_levels")
	var hs: PackedFloat32Array = t.get_cell_heights(spiral)
	_check(hs.size() == 19 and is_equal_approx(hs[3], 1.5), "get_cell_heights")
	t.set_cell_height(Vector2i(0, 0), 4.0)
	_check(is_equal_approx(t.get_cell_height(Vector2i(0, 0)), 4.0), "set_cell_height")

	# Cell <-> world, height on the flat top, raycast picks the cell.
	var c0: Vector3 = t.cell_to_world(Vector2i(0, 0))
	_check(t.world_to_cell(c0 + Vector3(1.0, 0, 1.0)) == Vector2i(0, 0), "world_to_cell")
	_check(is_equal_approx(t.get_height_at(c0, -1.0), 4.0), "get_height_at centre")
	var hit: Dictionary = t.raycast(c0 + Vector3(0, 20, 0), Vector3.DOWN, 100.0)
	_check(hit.get("hit", false) and hit["cell"] == Vector2i(0, 0) and hit["chunk"] == Vector2i(0, 0), "raycast %s" % hit)
	# A ray hitting the cliff face of the raised centre cell picks that cell, not the one in front.
	var east: Vector3 = t.cell_to_world(Vector2i(1, 0))
	var from := Vector3(east.x + 3.0, 3.0, c0.z)
	var side: Dictionary = t.raycast(from, Vector3(-1, 0, 0), 20.0)
	_check(side.get("hit", false) and side["cell"] == Vector2i(0, 0), "raycast cliff face %s" % side)

	# Small hexes (2 units flat-to-flat, 3x finer axial lattice).
	_check(t.small_hex_to_cell(Vector2i(2, 0)) == Vector2i(1, 0) and t.small_hex_to_cell(Vector2i(1, 1)) == Vector2i(0, 0), "small_hex_to_cell")
	var sw: Vector3 = t.small_hex_to_world(Vector2i(1, 0), 0.0)
	_check(is_equal_approx(sw.x, 2.0) and t.world_to_small_hex(sw) == Vector2i(1, 0), "small hex world %s" % sw)

	# Torus wrap with the game's period (k = 1: P = 19).
	t.wrap_period = 19
	_check(t.wrap_cell(Vector2i(20, -19)) == t.wrap_cell(Vector2i(1, 0)), "wrap_cell")
	# Patch periods for (19, 0) / (0, 19) large hexes are (3, 2) / (-2, 5) patches; the canonical
	# chunk is the one whose centre lies in [0, P)² (kg_hex torus slots): (1, 0) -> (-1, 5).
	_check(t.wrap_chunk(Vector2i(3, 2)) == Vector2i(0, 0) and t.wrap_chunk(Vector2i(1, 0)) == Vector2i(-1, 5) and t.wrap_chunk(Vector2i(-1, 5)) == Vector2i(-1, 5), "wrap_chunk")
	t.queue_free()
