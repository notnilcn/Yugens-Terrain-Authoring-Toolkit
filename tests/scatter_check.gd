# Windowed scatter / stamp check: a small square terrain with a stamped hill, painted props of
# every built-in shape, then a sculpt that the props must follow. Writes tests/captures/<name>.png.
# Also checks prop collision: the trees get cylinder colliders on layer 3 (PixelScatterItem
# `collision`), which a physics ray must hit after the sculpt (re-snapped, above the new surface)
# and miss once the trees are cleared; prints "COLLISION PASS|FAIL" (exit 1 on fail).
#   godot --path . --script res://tests/scatter_check.gd -- [name=scatter_check] [grid=0..3] [pixel]
extends SceneTree

var frames := 0
var args := {}
var terrain
var tree_y0 := 0.0
var collision_ok := true
var items_ok := false
var counts := []
var cleared_cell := Vector2i(-99999, -99999)


func _setup() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var root3d := Node3D.new()
	root.add_child(root3d)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.72, 0.86)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.55, 0.6, 0.7)
	e.ambient_light_energy = 0.6
	env.environment = e
	root3d.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 35, 0)
	sun.shadow_enabled = true
	root3d.add_child(sun)
	terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	terrain.grid_type = int(args.get("grid", "0"))
	terrain.chunk_size = 16
	root3d.add_child(terrain)
	for z in range(-1, 1):
		for x in range(-1, 1):
			terrain.add_chunk(Vector2i(x, z))
	var hill: Image = MarchingSquaresTerrain.create_stamp_image("Mountain", 128)
	terrain.apply_stamp(hill, Vector3(-6, 0, -6), Vector2(26, 26), 30.0, 9.0, 1.0, 0)
	var items: Array[PixelScatterItem] = []
	for k in ["tree", "pine", "bush", "rock", "flowers"]:
		var it: PixelScatterItem = PixelScatterItem.create_builtin(k)
		it.density *= 4.0
		if k == "tree":
			it.collision = 1
			it.collision_size = Vector3(0.4, 1, 0.4)
			it.collision_layer = 4
		items.append(it)
	terrain.scatter_items = items
	terrain.scatter_paint(0, Vector3(8, 0, 8), 7.0)
	terrain.scatter_paint(1, Vector3(-8, 0, -8), 9.0)
	terrain.scatter_paint(2, Vector3(-10, 0, 10), 6.0)
	terrain.scatter_paint(3, Vector3(10, 0, -10), 7.0)
	terrain.scatter_paint(4, Vector3(0, 0, 10), 6.0)
	print("scatter: ", terrain.get_scatter_count(-1), " per item ", [terrain.get_scatter_count(0), terrain.get_scatter_count(1), terrain.get_scatter_count(2), terrain.get_scatter_count(3), terrain.get_scatter_count(4)])
	if args.has("pixel"):
		var r = ClassDB.instantiate("PixelRenderer")
		root3d.add_child(r)
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 40.0
	cam.position = Vector3(30, 34, 30)
	root3d.add_child(cam)
	cam.look_at(Vector3(0, 2, 0))
	cam.current = true


# A vertical physics ray (layer 3 only) through the first tree must hit its collider above the
# terrain surface when `expect`, and nothing once the trees are gone.
func _check_tree_collision(expect: bool) -> void:
	var space := (terrain as Node3D).get_world_3d().direct_space_state
	var p := Vector2(8, 8)
	var pts: PackedVector2Array = terrain.get_scatter_positions(0)
	if expect:
		if pts.is_empty():
			print("COLLISION no trees")
			collision_ok = false
			return
		p = pts[0]
	var top := Vector3(p.x, 60, p.y)
	var hit := space.intersect_ray(PhysicsRayQueryParameters3D.create(top, top - Vector3(0, 120, 0), 4))
	var ground: float = terrain.get_height_at(Vector3(p.x, 0, p.y), 0.0)
	var ok: bool = (not hit.is_empty() and hit.position.y > ground + 0.5) if expect else hit.is_empty()
	print("COLLISION tree at %s ground=%.2f hit=%s expect=%s ok=%s" % [p, ground, hit.get("position", "-"), expect, ok])
	collision_ok = collision_ok and ok


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		# Needs a real renderer (it saves a screenshot): skip cleanly when run headless.
		if DisplayServer.get_name() == "headless":
			print("SKIP scatter_check: windowed check (renders a screenshot); run it without --headless, e.g. via ~/.local/bin/pt-xvfb")
			quit(0)
			return true
		_setup()
	if frames == 5:
		# Sculpt under the trees: they must re-snap to the new surface.
		terrain.apply_brush("raise", Vector3(8, 0, 8), 6.0, 4.0, 0)
	if frames == 30:
		_check_tree_collision(true)
		# A cleared cell (game: built on / paved) hides the prop on it, restoring shows it again.
		var pts: PackedVector2Array = terrain.get_scatter_positions(0)
		if not pts.is_empty():
			cleared_cell = terrain.world_to_sample(Vector3(pts[0].x, 0, pts[0].y))
			terrain.set_cells_cleared([cleared_cell], true)
	if frames == 33 and cleared_cell != Vector2i(-99999, -99999):
		var p: Vector2 = terrain.get_scatter_positions(0)[0]
		var top := Vector3(p.x, 60, p.y)
		var hit := (terrain as Node3D).get_world_3d().direct_space_state.intersect_ray(PhysicsRayQueryParameters3D.create(top, top - Vector3(0, 120, 0), 4))
		var ok: bool = hit.is_empty() and terrain.get_cleared_cells().size() == 1
		print("CLEARED CELL %s hides its tree: %s" % [cleared_cell, ok])
		collision_ok = collision_ok and ok
		terrain.set_cells_cleared([cleared_cell], false)
	if frames == 36 and cleared_cell != Vector2i(-99999, -99999):
		_check_tree_collision(true)
	if frames == 40:
		var name: String = args.get("name", "scatter_check")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		print("CAPTURE ", name)
		terrain.scatter_clear(0)
	if frames == 44:
		_check_tree_collision(false)
		print("COLLISION %s" % ("PASS" if collision_ok else "FAIL"))
		# Item removal keeps the other items' instances: remove_scatter_item (the Scatter
		# tool's Remove Item), then an inspector-style edit of the array.
		counts = [terrain.get_scatter_count(0), terrain.get_scatter_count(1), terrain.get_scatter_count(2), terrain.get_scatter_count(3), terrain.get_scatter_count(4)]
		terrain.remove_scatter_item(1)
		var after := [terrain.get_scatter_count(0), terrain.get_scatter_count(1), terrain.get_scatter_count(2), terrain.get_scatter_count(3)]
		var ok1: bool = after == [counts[0], counts[2], counts[3], counts[4]] and terrain.scatter_items.size() == 4
		print("REMOVE ITEM tool before=%s after=%s ok=%s" % [counts, after, ok1])
		items_ok = ok1
		var arr: Array = terrain.scatter_items.duplicate()
		arr.remove_at(1)
		terrain.scatter_items = arr
	if frames == 48:
		var after := [terrain.get_scatter_count(0), terrain.get_scatter_count(1), terrain.get_scatter_count(2)]
		var ok2: bool = after == [counts[0], counts[3], counts[4]]
		print("REMOVE ITEM inspector after=%s ok=%s" % [after, ok2])
		items_ok = items_ok and ok2
		print("ITEMS %s" % ("PASS" if items_ok else "FAIL"))
		quit(0 if collision_ok and items_ok else 1)
	return false
