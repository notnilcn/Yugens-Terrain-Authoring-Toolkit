# Headless check of PixelWater raised water / waterfalls on a hex terrain through the node API:
#   godot --headless --path . --script res://tests/waterfall_check.gd
# Prints "WATERFALL OK" or the failed checks (exit code 1).
extends SceneTree

var fails := 0
var frames := 0
var t
var water


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		t = ClassDB.instantiate("MarchingSquaresTerrain")
		t.grid_type = 3
		t.ring_radius = 2
		t.cell_size = Vector2(2, 2)
		t.level_height = 0.5
		t.hex_edge_style = 3 # HexPrism
		root.add_child(t)
		t.add_chunk(Vector2i(0, 0))
		# A plateau at 3 (6 levels) with a sunken pool cell in the middle, over a sea at 0.2.
		var centre := Vector2i(0, 0)
		for c in t.get_chunk_cells(Vector2i(0, 0)):
			t.set_cell_height(c, 3.0)
		t.set_cell_height(centre, 2.5)
		var nbs: Array = t.get_cell_neighbors(centre)
		t.set_cell_height(nbs[0], 1.5) # 3 levels down: a cascade
		t.set_cell_height(nbs[1], -1.0) # under the sea: a waterfall into the sea
		water = ClassDB.instantiate("PixelWater")
		water.size = Vector2(40, 40)
		water.position = Vector3(0, 0.2, 0)
		water.terrain = t
		root.add_child(water)
		water.set_cell_water(centre, 3.0)
	elif frames == 3:
		_run()
		print("WATERFALL OK" if fails == 0 else "WATERFALL: %d failure(s)" % fails)
		quit(1 if fails else 0)
	return false


func _run() -> void:
	water.update_waterfalls()
	var falls: Array = water.get_waterfalls()
	var nbs: Array = t.get_cell_neighbors(Vector2i(0, 0))
	_check(falls.size() == 2, "two spilling edges, got %d" % falls.size())
	for f in falls:
		if f["neighbor"] == nbs[0]:
			_check(not f["waterfall"] and not f["into_water"], "1.5 m drop is a cascade (min 5 levels of 0.5)")
		elif f["neighbor"] == nbs[1]:
			_check(f["waterfall"] and f["into_water"], "falls into the sea")
			_check(is_equal_approx(f["bottom"], 0.2), "lands on the sea surface: %f" % f["bottom"])
		else:
			_check(false, "unexpected fall %s" % f)
	# Lower the threshold: the cascade becomes a waterfall too.
	water.waterfall_min_levels = 3
	water.update_waterfalls()
	var n := 0
	for f in water.get_waterfalls():
		n += 1 if f["waterfall"] else 0
	_check(n == 2, "min 3 levels: both are waterfalls (%d)" % n)
	var mesh_node: MeshInstance3D = water.get_node_or_null("Waterfalls")
	_check(mesh_node != null and mesh_node.mesh != null and mesh_node.mesh.get_surface_count() == 1, "waterfall mesh built")
	_check(mesh_node != null and mesh_node.owner == null, "internal child, not saved")
	_check_flowers()
	# No raised water: no falls, mesh removed.
	water.clear_cell_water(Vector2i(0, 0))
	water.update_waterfalls()
	_check(water.get_waterfalls().is_empty(), "cleared")


# MarchingSquaresWildFlowers skip cells under any PixelWater: the raised pool cell (ground 2.5,
# water 3.0) and the cell under the sea plane (ground -1, sea 0.2; water_level is set far below
# so only the PixelWater can exclude it).
func _check_flowers() -> void:
	var f = ClassDB.instantiate("MarchingSquaresWildFlowers")
	f.ground_water = 1.0
	f.ground_water_variation = 0.0
	f.density = 2.0
	f.water_level = -100.0
	# A terrain-stored water level (set_cell_water_level, saved in the chunk data) floods a
	# plateau cell too.
	var stored: Vector2i = t.get_cell_neighbors(Vector2i(0, 0))[3]
	t.set_cell_water_level(stored, 50.0)
	t.add_child(f)
	var placed: int = f.scatter_flowers()
	_check(placed > 0, "wild flowers placed (%d)" % placed)
	var nbs: Array = t.get_cell_neighbors(Vector2i(0, 0))
	var per_cell := {}
	for p in t.get_scatter_positions(-1):
		var cell: Vector2i = t.world_to_cell(t.to_global(Vector3(p.x, 0.0, p.y)))
		per_cell[cell] = per_cell.get(cell, 0) + 1
	_check(per_cell.get(Vector2i(0, 0), 0) == 0, "no flowers in the raised pool (%d)" % per_cell.get(Vector2i(0, 0), 0))
	_check(per_cell.get(nbs[1], 0) == 0, "no flowers under the sea plane (%d)" % per_cell.get(nbs[1], 0))
	_check(per_cell.get(nbs[2], 0) > 0, "flowers on dry plateau cells")
	_check(per_cell.get(stored, 0) == 0, "no flowers under a stored water level (%d)" % per_cell.get(stored, 0))
	t.set_cell_water_level(stored, -10000.0)
	f.clear_flowers()
	t.remove_child(f)
	f.free()
