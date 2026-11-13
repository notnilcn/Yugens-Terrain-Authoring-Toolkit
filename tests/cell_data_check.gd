# Headless check of the cell data API (fog of war / tint / highlight), auto rules and overlays:
#   godot --headless --path . --script res://tests/cell_data_check.gd
# Prints "CELL DATA OK" or the failed checks (exit code 0 = pass).
extends SceneTree

var fails := 0
var frames := 0
var t


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		_run()
	if frames == 30:
		_after_frames()
		print("CELL DATA OK" if fails == 0 else "CELL DATA: %d failure(s)" % fails)
		quit(1 if fails else 0)
	return false


func _run() -> void:
	# kg_hex scale: LargeHex = 6 units, chunk == patch (19 cells), torus wrap P = 19 * 2.
	t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = 2
	t.cell_size = Vector2(6, 6)
	t.wrap_period = 38
	root.add_child(t)
	t.add_chunk(Vector2i(0, 0))
	t.add_chunk(Vector2i(1, 0))

	# The rectangle covers every chunk cell (axial q, r).
	var rect: Rect2i = t.get_cell_rect()
	_check(rect.size.x > 0 and rect.size.y > 0, "cell rect %s" % rect)
	for c in t.get_chunks():  # canonical (wrapped) chunk coordinates
		for cell in t.get_chunk_cells(c):
			_check(rect.has_point(cell), "rect has %s" % cell)

	# Everything starts unexplored.
	_check(not t.is_cell_explored(Vector2i(0, 0)) and not t.get_cell_visibility(Vector2i(0, 0)), "initially unexplored")
	# Visible implies explored; hiding keeps explored.
	_check(t.set_cell_visibility(Vector2i(0, 0), true), "set visible")
	_check(t.get_cell_visibility(Vector2i(0, 0)) and t.is_cell_explored(Vector2i(0, 0)), "visible + explored")
	t.hide_all_cells()
	_check(not t.get_cell_visibility(Vector2i(0, 0)) and t.is_cell_explored(Vector2i(0, 0)), "hidden but explored")
	t.set_cell_explored(Vector2i(0, 0), false)
	_check(not t.is_cell_explored(Vector2i(0, 0)), "un-explore")

	# World <-> fog cell is world_to_cell on hex grids.
	var w: Vector3 = t.cell_to_world(Vector2i(1, -1))
	_check(t.world_to_fog_cell(w) == Vector2i(1, -1) and t.world_to_cell(w) == Vector2i(1, -1), "world_to_fog_cell")

	# Wrapped coordinates address the canonical cell (kg_hex torus).
	_check(t.set_cell_visibility(Vector2i(1, -1) + Vector2i(38, 0), true), "wrapped set")
	_check(t.get_cell_visibility(Vector2i(1, -1)), "wrapped get")

	# Bulk list + bytes round trip.
	var spiral: Array = t.get_chunk_cells_spiral(Vector2i(1, 0))
	t.set_cells_visibility(spiral, true)
	for c in spiral:
		_check(t.get_cell_visibility(c), "bulk visible %s" % c)
	var bytes: PackedByteArray = t.get_cells_bytes()
	_check(bytes.size() == rect.size.x * rect.size.y * 4, "bytes size %d" % bytes.size())
	t.reset_fog()
	_check(not t.get_cell_visibility(spiral[0]), "reset fog")
	t.set_cells_from_bytes(bytes)
	_check(t.get_cell_visibility(spiral[0]) and t.get_cell_visibility(spiral[18]), "bytes restore")
	t.reveal_all_cells()
	_check(t.get_cell_visibility(Vector2i(0, 0)), "reveal all")

	# Tints and highlights.
	_check(t.set_cell_tint(Vector2i(0, 0), Color(1, 0, 0, 0.5)), "tint")
	var tc: Color = t.get_cell_tint(Vector2i(0, 0))
	_check(tc.r > 0.99 and absf(tc.a - 0.5) < 0.01, "tint read %s" % tc)
	t.set_cells_tint(spiral, Color(0, 0, 1, 1))
	_check(t.get_cell_tint(spiral[3]).b > 0.99, "bulk tint")
	var tints: PackedByteArray = t.get_cell_tints_bytes()
	t.clear_cell_tints()
	_check(t.get_cell_tint(spiral[3]).a == 0.0, "clear tints")
	t.set_cell_tints_from_bytes(tints)
	_check(t.get_cell_tint(spiral[3]).b > 0.99, "tint bytes restore")
	_check(t.set_cell_highlight(Vector2i(0, 0), 1.0), "highlight")

	# Fog on: the transition runs and the globals are published.
	t.fog_enabled = true
	t.hide_all_cells()
	t.set_cell_visibility(Vector2i(0, 0), true)

	# Overlays: properties exist and the shader cursor is accepted.
	t.show_grid = true
	t.grid_chunk_lines = true
	t.debug_view = 5
	t.set_cursor(w, 6.0, 2, Color.GREEN)
	t.clear_cursor()

	# Auto rules: off by default, rock on a steep square-grid slope when enabled.
	var s = ClassDB.instantiate("MarchingSquaresTerrain")
	s.grid_type = 0
	s.chunk_size = 8
	s.cell_size = Vector2(2, 2)
	s.default_merge_mode = 4
	root.add_child(s)
	s.add_chunk(Vector2i(0, 0))
	for z in 9:
		for x in 9:
			s.set_sample_height(Vector2i(x, z), float(x) * 2.5 if x > 4 else 0.0)
	_check(s.get_auto_slot_at(Vector3(3, 0, 8)) == -1, "auto rules off by default")
	var rules = ClassDB.instantiate("MarchingSquaresAutoRules")
	rules.enabled = true
	rules.filler_amount = 0.0
	rules.water_level = -10.0
	rules.snow_height = 100.0
	s.texture_set.auto_rules = rules
	_check(s.get_auto_slot_at(Vector3(13, 0, 8)) == rules.rock_slot, "rock on slope: %d" % s.get_auto_slot_at(Vector3(13, 0, 8)))
	_check(s.get_auto_slot_at(Vector3(3, 0, 8)) == -1, "flat ground keeps base")
	rules.water_level = 0.0
	_check(s.get_auto_slot_at(Vector3(3, 0, 8)) == rules.shore_slot, "shore near water")
	rules.snow_height = -5.0
	rules.snow_noise = 0.0
	rules.snow_blend = 0.01
	_check(s.get_auto_slot_at(Vector3(3, 0, 8)) == rules.snow_slot, "snow above the line")


func _painted_tints() -> void:
	# Painted (saved) tints: stored in the data resource, under runtime tints, survive a reload.
	var red := Color(1, 0, 0, 0.5)
	_check(t.paint_cell_tint(Vector2i(2, -1), red), "paint tint")
	_check(t.get_painted_tint(Vector2i(2, -1)).is_equal_approx(Color8(255, 0, 0, 128)), "painted tint read back")
	_check(t.get_cell_tint(Vector2i(2, -1)).r > 0.9, "painted tint shows in the cell data")
	t.set_cell_tint(Vector2i(2, -1), Color(0, 0, 1, 1))
	t.clear_cell_tints()
	_check(t.get_cell_tint(Vector2i(2, -1)).r > 0.9, "clear_cell_tints keeps painted tints")
	var blob: PackedByteArray = t.get_painted_tints_data()
	_check(blob.size() == 8 + 12, "PTT1 blob size %d" % blob.size())
	_check(t.data.is_dirty(), "painting marks the data dirty")
	var copy: Node3D = ClassDB.instantiate("MarchingSquaresTerrain")
	copy.grid_type = 3
	copy.ring_radius = 2
	copy.cell_size = Vector2(6, 6)
	copy.wrap_period = 38
	copy.data = t.data.duplicate(true)
	root.add_child(copy)
	_check(copy.get_painted_tint(Vector2i(2, -1)).r > 0.9, "painted tint loads from the data")
	_check(copy.get_cell_tint(Vector2i(2, -1)).r > 0.9, "loaded painted tint is in the cell data")
	copy.queue_free()
	t.paint_cell_tint(Vector2i(2, -1), Color(0, 0, 0, 0))
	_check(t.get_painted_tints_data().size() == 8, "erase painted tint")
	t.set_painted_tints_data(blob)
	_check(t.get_painted_tint(Vector2i(2, -1)).r > 0.9, "set_painted_tints_data (undo)")
	t.set_painted_tints_data(PackedByteArray())
	_check(t.get_cell_tint(Vector2i(2, -1)).a == 0.0, "clearing painted tints clears the cells")


func _after_frames() -> void:
	_painted_tints()
	# (Global shader parameters cannot be read back outside the editor; the render captures in
	# tests/overlay_capture.gd cover the GPU side.)
	_check(t.get_cell_visibility(Vector2i(0, 0)) and not t.get_cell_visibility(Vector2i(1, -1)), "state after fog frames")
	t.finish_fog_transition()
	t.fog_enabled = false
	t.clear_cell_tints()
