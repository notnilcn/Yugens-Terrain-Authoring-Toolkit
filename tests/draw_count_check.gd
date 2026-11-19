# Headless draw-call budget check of a demo scene (default: the hex island start view).
#   godot --headless --path . --script res://tests/draw_count_check.gd -- [scene=res://demo/hex_island.tscn]
#       [max_draws=450] [max_terrain=90] [verbose=1]
# Headless rendering reports 0 draw calls, so this counts what each pixel-pipeline camera would
# draw from the scene tree: every visible GeometryInstance3D on the camera's cull mask, one draw
# per mesh surface (MultiMeshes: per surface of their mesh). Frustum culling and shadow passes
# are ignored (an upper bound for the start view, which sees the whole island).
# Checks: the colour + meta + id passes stay under `max_draws`, the terrain draws with region
# batches (no per-chunk `Chunk (x, y)` mesh nodes, at most `max_terrain` terrain instances) and
# every chunk keeps its collision. Prints "DRAW COUNT OK" or the failures (exit code 1).
extends SceneTree

var args := {}
var frame := 0
var fails := 0


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	root.add_child(load(args.get("scene", "res://demo/hex_island.tscn")).instantiate())


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _walk(n: Node, cls: String, out: Array) -> void:
	if n.is_class(cls):
		out.append(n)
	for c in n.get_children(true):
		_walk(c, cls, out)


func _surfaces(g: GeometryInstance3D) -> int:
	if g is MeshInstance3D:
		return g.mesh.get_surface_count() if g.mesh else 0
	if g is MultiMeshInstance3D:
		var mm: MultiMesh = g.multimesh
		return mm.mesh.get_surface_count() if mm and mm.mesh and mm.visible_instance_count != 0 and mm.instance_count > 0 else 0
	return 1


func _process(_d: float) -> bool:
	frame += 1
	if frame < 10:
		return false
	var geo := []
	_walk(root, "GeometryInstance3D", geo)
	geo = geo.filter(func(g): return g.is_visible_in_tree())
	var cams := []
	_walk(root, "Camera3D", cams)
	var passes := {}
	for c in cams:
		var path := str(c.get_path())
		if not path.contains("PixelPipeline"):
			continue
		var n := 0
		for g in geo:
			if g.layers & c.cull_mask:
				n += _surfaces(g)
		passes[str(c.name)] = n
	var total := 0
	for k in passes:
		total += passes[k]
	var terrains := []
	_walk(root, "MarchingSquaresTerrain", terrains)
	var terrain_draws := 0
	var chunk_nodes := 0
	for t in terrains:
		var inner := []
		_walk(t, "GeometryInstance3D", inner)
		for g in inner:
			if g.is_visible_in_tree() and _surfaces(g) > 0:
				terrain_draws += 1
			if str(g.name).begins_with("Chunk ("):
				chunk_nodes += 1
		var bodies := []
		_walk(t, "StaticBody3D", bodies)
		var owners := 0
		for b in bodies:
			# Region bodies only (scatter props have their own).
			if str(b.name).begins_with("Region (") and str(b.name).ends_with(" collision"):
				owners += b.get_shape_owners().size()
		var with_faces := 0
		for c in t.get_chunks():
			if t.get_chunk_collision_faces(c).size() > 0:
				with_faces += 1
		_check(owners == with_faces and with_faces > 0, "%s: one collision shape per chunk (%d shapes, %d chunks with faces)" % [t.name, owners, with_faces])
	var out := {"passes": passes, "total": total, "instances": geo.size(), "terrain_instances": terrain_draws, "chunk_nodes": chunk_nodes}
	print("DRAW_COUNT ", JSON.stringify(out))
	if args.has("verbose"):
		var by := {}
		for g in geo:
			var key: String = g.get_class() + " " + str(g.name).get_slice(" (", 0)
			by[key] = by.get(key, 0) + _surfaces(g)
		print(by)
	_check(not passes.is_empty(), "no pixel pipeline cameras")
	_check(total <= int(args.get("max_draws", "450")), "%d draws over the colour / meta / id passes (max %s)" % [total, args.get("max_draws", "450")])
	_check(chunk_nodes == 0, "%d per-chunk mesh nodes (regions draw the chunks)" % chunk_nodes)
	_check(terrain_draws <= int(args.get("max_terrain", "90")), "%d terrain instances (max %s)" % [terrain_draws, args.get("max_terrain", "90")])
	print("DRAW COUNT OK" if fails == 0 else "DRAW COUNT: %d failure(s)" % fails)
	quit(1 if fails else 0)
	return false
