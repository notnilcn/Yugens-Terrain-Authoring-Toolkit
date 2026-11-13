# Headless check of hex_edge_style (FlatWalls / Terraces / Slopes / HexPrism) through the node API:
#   godot --headless --path . --script res://tests/hex_edge_check.gd
# Prints "HEX EDGE OK" or the failed checks (exit code 1).
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
		print("HEX EDGE OK" if fails == 0 else "HEX EDGE: %d failure(s)" % fails)
		quit(1 if fails else 0)
	return false


func _run() -> void:
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = 2
	t.cell_size = Vector2(6, 6)
	t.level_height = 1.0
	root.add_child(t)
	t.add_chunk(Vector2i(0, 0))
	t.add_chunk(Vector2i(1, 0))
	# Levels 0..3 across both chunks: one-level steps (terraces) and bigger ones (cliffs).
	var i := 0
	for ch in [Vector2i(0, 0), Vector2i(1, 0)]:
		for c in t.get_chunk_cells(ch):
			t.set_cell_level(c, (i * 7) % 4)
			i += 1
	for style in 4:
		t.hex_edge_style = style
		_check(t.hex_edge_style == style, "style %d set" % style)
		for ch in [Vector2i(0, 0), Vector2i(1, 0)]:
			for c in t.get_chunk_cells(ch):
				var w: Vector3 = t.cell_to_world(c)
				var h: float = t.get_cell_height(c)
				# Cell centres stay on the stored height in every style.
				_check(is_equal_approx(t.get_height_at(w, -99.0), h), "style %d centre %s" % [style, c])
				# A vertical ray lands on the queried surface at random points of the cell.
				for k in 4:
					var p := w + Vector3(cos(k * 1.7) * 2.2, 0, sin(k * 1.7) * 2.2)
					var hit: Dictionary = t.raycast(p + Vector3(0, 30, 0), Vector3.DOWN, 100.0)
					var q: float = t.get_height_at(p, -99.0)
					_check(hit.get("hit", false) and absf(hit["position"].y - q) < 0.05, "style %d ray at %s: %s vs %f" % [style, p, hit, q])
				# HexPrism tops are flat: every cell above the floor offers its 7 small-hex spots.
				if style == 3:
					var spots: PackedVector3Array = t.get_cell_decoration_spots(c, -1.0)
					_check(spots.size() == 7, "prism %s: 7 decoration spots, got %d" % [c, spots.size()])
