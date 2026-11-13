# Builds the hex island's builder structure (a small ruined watch hut made with PixelBuilderMesh
# ops, a blend material and vertex paint) and writes it to res://demo/builder_demo_ruin.tres,
# which demo/hex_island.tscn references from its "BuilderRuin" node. The terrain is not touched:
# the ruin is seated on the terrain height under its footprint.
#   godot --headless --path . --script res://tests/builder_island.gd -- [probe] [save=1]
# probe prints the terrain levels around the spawn (to pick a flat spot).
extends SceneTree

const BOX := 0
const CYLINDER := 2
const STAIRS := 5
const ARCH := 6
const HEX_PRISM := 9
const DOOR := 12

var frames := 0
var args := {}
var fail := false

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		var inst: Node = load("res://demo/hex_island.tscn").instantiate()
		if inst.has_node("BuilderRuin"):
			var old := inst.get_node("BuilderRuin")
			inst.remove_child(old)
			old.free()
		root.add_child(inst)
	elif frames == 3:
		var island := root.get_child(root.get_child_count() - 1)
		var terrain = island.get_node("MarchingSquaresTerrain")
		if args.has("probe"):
			_probe(terrain)
		else:
			_build(island, terrain)
		quit(1 if fail else 0)
	return false

func _probe(terrain) -> void:
	var spawn: Vector3 = Vector3(0, 7.5, 8)
	for z in range(-14, 16, 2):
		var line := "z=%4d " % z
		for x in range(-14, 16, 2):
			var h: float = terrain.get_height_at(Vector3(x, 0, z), -99.0)
			line += "%5.1f" % h
		print(line)
	print("spawn h=", terrain.get_height_at(spawn, -99.0))
	var sc = terrain.get_scatter_positions(-1)
	print("scatter: ", sc.size())
	for p in sc:
		if absf(p.x) < 16 and absf(p.y) < 16:
			print("  prop ", p)

## Crumbled wall-top height at a local point: tall at the back-left corner, broken down towards
## the front of the right run, with a few deterministic notches.
func _crumble(p: Vector3) -> float:
	var h := 1.95
	if p.x > 0.4:
		h -= (p.x - 0.4) * 0.55
	if p.z > -0.6:
		h -= (p.z + 0.6) * (0.55 if p.x < 0.0 else 0.4)
	h += 0.16 * sin(p.x * 7.3 + p.z * 5.1) - 0.1 * absf(sin(p.x * 13.0 - p.z * 3.0))
	return clampf(h, 0.55, 2.05)

## First edge (vertex pair) of the faces from `from` on whose end points `pred` holds.
func _edge_where(d, from: int, pred: Callable) -> PackedInt32Array:
	var faces := PackedInt32Array()
	for f in range(from, d.get_face_count()):
		faces.append(f)
	var e: PackedInt32Array = d.get_face_edges(faces)
	for i in range(0, e.size(), 2):
		if pred.call(d.get_vertex(e[i]), d.get_vertex(e[i + 1])):
			return PackedInt32Array([e[i], e[i + 1]])
	return PackedInt32Array()

func _build(island, terrain) -> void:
	var up := Vector3.UP
	var none := PackedInt32Array()
	var d = ClassDB.instantiate("PixelBuilderMeshData")
	var stone := Color(0.8, 0.77, 0.72)

	# Floor: flagstone slab, subdivided so the moss mask has corners to blend between.
	var slab = d.add_shape(BOX, Vector3(3.6, 0.3, 3.2), Vector3(0, -0.12, 0.1), {"color": Color(0.7, 0.68, 0.64)})
	for i in 2:
		d.subdivide(PackedInt32Array(Array(d.faces_facing(up, 5.0, none)).filter(func(f): return d.get_face_center(f).y > 0.0)))
	var slab_top: PackedInt32Array = PackedInt32Array(Array(d.faces_facing(up, 5.0, none)))

	# U-shaped wall (one swept polyline, mitred corners), open towards +Z.
	var n0: int = d.get_face_count()
	var ring := PackedVector3Array([Vector3(-1.55, 0, 1.25), Vector3(-1.55, 0, -1.3), Vector3(1.55, 0, -1.3), Vector3(1.55, 0, 0.55)])
	var wall = d.add_wall(ring, false, 1.95, Vector3(0, 0.15, 0), {"thickness": 0.3, "color": stone})
	_check(d.get_face_count() == n0 + 14, "wall polyline (%d faces)" % wall.size())
	# One horizontal edge loop all round at half height (corners for the moss gradient), then
	# vertical loops across each run (Unibuilder Insert loop) to crumble the top down.
	var vert := _edge_where(d, n0, func(a: Vector3, b: Vector3): return absf(a.y - b.y) > 1.0 and a.z < -1.4 and absf(a.x) < 1.0)
	if vert.size() == 0:
		vert = _edge_where(d, n0, func(a: Vector3, b: Vector3): return absf(a.y - b.y) > 1.0)
	var hl: PackedInt32Array = d.insert_loop(vert[0], vert[1], 1)
	_check(hl.size() > 0, "horizontal loop around the wall (%d edges)" % (hl.size() / 2))
	for run in [[func(a: Vector3, b: Vector3): return a.y > 2.0 and b.y > 2.0 and absf(a.z - b.z) < 1e-3 and a.z < -1.0, 4], [func(a: Vector3, b: Vector3): return a.y > 2.0 and b.y > 2.0 and absf(a.x - b.x) < 1e-3 and a.x < -1.0, 3], [func(a: Vector3, b: Vector3): return a.y > 2.0 and b.y > 2.0 and absf(a.x - b.x) < 1e-3 and a.x > 1.0, 2]]:
		var te := _edge_where(d, n0, run[0])
		if te.size() > 0:
			d.insert_loop(te[0], te[1], run[1])
	for v in d.get_vertex_count():
		var p: Vector3 = d.get_vertex(v)
		if p.y > 2.0:
			d.set_vertex(v, Vector3(p.x, 0.15 + _crumble(p), p.z))
	# The crumbled tops are no longer planar: triangulate them (Unibuilder Triangulate).
	var tops := PackedInt32Array()
	for f in range(n0, d.get_face_count()):
		if d.get_face_normal(f).y > 0.3 and d.get_face_center(f).y > 0.6:
			tops.append(f)
	var tris: PackedInt32Array = d.triangulate(tops)
	_check(tris.size() == tops.size() * 2, "triangulated %d crumbled top quads" % tops.size())
	_check(d.is_closed(), "crumbled wall stays watertight")

	# A window in the back wall: connect the two vertical edges of an upper outer panel through
	# their midpoints (Unibuilder Connect edges), then inset and recess the upper half.
	var panel := -1
	for f in d.faces_facing(Vector3(0, 0, -1), 10.0, none):
		var c: Vector3 = d.get_face_center(f)
		if c.y > 1.0 and c.z < -1.4 and absf(c.x + 0.3) < 0.5:
			panel = f
	var verticals := PackedInt32Array()
	if panel >= 0:
		var fv: PackedInt32Array = d.get_face_vertices(panel)
		for i in fv.size():
			var a: Vector3 = d.get_vertex(fv[i])
			var b2: Vector3 = d.get_vertex(fv[(i + 1) % fv.size()])
			if absf(a.x - b2.x) < 1e-3 and absf(a.y - b2.y) > 0.2:
				verticals.append_array([fv[i], fv[(i + 1) % fv.size()]])
	var chords: PackedInt32Array = d.connect_edges(verticals) if verticals.size() == 4 else PackedInt32Array()
	_check(chords.size() == 2, "connect edges across the back wall")
	if chords.size() == 2:
		var y_mid: float = d.get_vertex(chords[0]).y
		for f in d.faces_facing(Vector3(0, 0, -1), 10.0, none):
			var c: Vector3 = d.get_face_center(f)
			if c.z < -1.4 and absf(c.x + 0.3) < 0.5 and c.y < y_mid and c.y > 0.8:
				d.extrude(d.inset(PackedInt32Array([f]), 0.3), -0.2)
				break

	# Corner pillar with an inset, raised cap and chamfered rim.
	var pillar = d.add_shape(HEX_PRISM, Vector3(0.62, 2.35, 0.62), Vector3(1.55, 0.15, 1.15), {"color": Color(0.74, 0.71, 0.66)})
	var ptop: PackedInt32Array = d.faces_facing(up, 5.0, pillar)
	var cap = d.inset(ptop, 0.3)
	d.extrude(cap, 0.18)
	d.bevel(d.get_face_edges(cap), 0.04)

	# Fallen roof beam and rubble: separate pieces merged in with a transform (Unibuilder Merge
	# objects); the beam's faces flip their diagonal (Flip face edge) for the shading seam.
	var beam = ClassDB.instantiate("PixelBuilderMeshData")
	var bf = beam.add_shape(BOX, Vector3(3.0, 0.24, 0.26), Vector3.ZERO, {"material": 1, "color": Color(0.56, 0.38, 0.22)})
	_check(beam.flip_face_edge(bf) == bf.size(), "flip face edge on the beam")
	# Long edges in three (Unibuilder Subdivide edges): corners for the charred-end paint.
	var long := PackedInt32Array()
	var be: PackedInt32Array
	for f in bf:
		be = beam.get_face_edges(PackedInt32Array([f]))
		for i in range(0, be.size(), 2):
			if beam.get_vertex(be[i]).distance_to(beam.get_vertex(be[i + 1])) > 2.0:
				long.append_array([be[i], be[i + 1]])
	_check(beam.subdivide_edges(long, 3).size() == 8, "subdivide the beam's long edges")
	beam.paint_brush(Vector3(1.5, 0, 0), 1.1, 0.8, 0.6, Color(0.18, 0.14, 0.12), 1)
	var lean := Transform3D(Basis.from_euler(Vector3(0, deg_to_rad(-28), deg_to_rad(-27))), Vector3(-0.05, 0.85, -0.55))
	var nf: int = d.get_face_count()
	var merged: PackedInt32Array = d.merge_from(beam, lean)
	_check(merged.size() == bf.size() and d.get_face_count() == nf + bf.size(), "merge the beam in")
	var plank = ClassDB.instantiate("PixelBuilderMeshData")
	plank.add_shape(BOX, Vector3(1.5, 0.12, 0.3), Vector3.ZERO, {"material": 1, "color": Color(0.5, 0.34, 0.2)})
	d.merge_from(plank, Transform3D(Basis.from_euler(Vector3(0, deg_to_rad(64), deg_to_rad(4))), Vector3(-0.7, 0.07, 0.75)))
	var rocks := [[Vector3(2.05, 0.0, 0.3), 0.42, 20.0], [Vector3(2.25, 0.0, -0.35), 0.3, -35.0], [Vector3(1.95, 0.32, -0.05), 0.26, 50.0], [Vector3(-2.0, 0.0, -0.6), 0.34, 12.0]]
	for r in rocks:
		var rock = ClassDB.instantiate("PixelBuilderMeshData")
		var s: float = r[1]
		rock.add_shape(BOX, Vector3(s * 1.3, s * 0.8, s), Vector3.ZERO, {"color": Color(0.7, 0.68, 0.63)})
		rock.triangulate(PackedInt32Array())
		d.merge_from(rock, Transform3D(Basis.from_euler(Vector3(deg_to_rad(r[2] * 0.2), deg_to_rad(r[2]), 0)), r[0]))

	# Blend material: stone (layer A) to moss (layer B) by the painted vertex alpha.
	var blend = ClassDB.instantiate("PixelBuilderBlendMaterial")
	blend.ensure_shader()
	blend.set_shader_parameter("texture_a", load("res://addons/MarchingSquaresTerrain/textures/ground/cliff.png"))
	blend.set_shader_parameter("texture_b", load("res://addons/MarchingSquaresTerrain/textures/ground/moss.png"))
	blend.set_shader_parameter("albedo_a", Color(0.86, 0.83, 0.78))
	blend.set_shader_parameter("albedo_b", Color(0.42, 0.6, 0.3))
	blend.set_shader_parameter("uv_scale_a", Vector2(0.5, 0.5))
	blend.set_shader_parameter("uv_scale_b", Vector2(0.5, 0.5))
	blend.set_shader_parameter("mask_steps", 3)
	var wood := StandardMaterial3D.new()
	wood.vertex_color_use_as_albedo = true
	wood.roughness = 1.0
	var mats: Array[Material] = [blend, wood]
	d.materials = mats
	var all := PackedInt32Array()
	for f in d.get_face_count():
		all.append(f)
	d.paint_faces(all, Color(1, 1, 1, 0), 2)  # all stone
	# Moss creeping up from the ground along the walls, into the floor corners and over the
	# tall back-left corner.
	for p in [Vector3(-1.55, 0.2, 0.9), Vector3(-1.55, 0.2, -0.4), Vector3(-1.1, 0.2, -1.3), Vector3(0.3, 0.2, -1.3), Vector3(1.55, 0.2, -0.9), Vector3(-1.4, 2.0, -1.2), Vector3(1.55, 0.2, 1.0), Vector3(-0.8, 0.2, 0.9)]:
		d.paint_brush(p, 1.0, 1.0, 0.5, Color(0, 0, 0, 1), 2)
	# Soot / weathering in RGB on the inside of the back wall.
	d.paint_brush(Vector3(0.0, 1.2, -1.15), 0.9, 0.55, 0.8, Color(0.45, 0.42, 0.4), 1)
	var mossy := 0
	var corners := 0
	for f in slab_top:
		for c in d.get_face_vertices(f).size():
			corners += 1
			if d.get_corner_color(f, c).a > 0.5:
				mossy += 1
	_check(mossy > 3 and mossy < corners, "moss painted into part of the floor (%d / %d corners)" % [mossy, corners])
	print("ruin: ", d.get_face_count(), " faces ", d.get_vertex_count(), " verts")

	# Seat it on the plateau east of the spawn basin (flat 10.0, clear of props and the ramp).
	var at := Vector3(7.2, 0, 3.0)
	var cell: Vector2i = terrain.world_to_sample(at)
	var probe = ClassDB.instantiate("PixelBuilderMesh")
	island.add_child(probe)
	probe.place_on_cell(terrain, cell)
	var pos: Vector3 = probe.global_position
	print("cell ", cell, " pos ", pos, " heights ", terrain.get_height_at(pos + Vector3(1.8, 0, 1.8), -1.0), " ", terrain.get_height_at(pos - Vector3(1.8, 0, 1.8), -1.0), " ", terrain.get_height_at(pos + Vector3(1.8, 0, -1.8), -1.0), " ", terrain.get_height_at(pos + Vector3(-1.8, 0, 1.8), -1.0))
	var xf := Transform3D(Basis(Vector3.UP, deg_to_rad(40.0)), pos)
	print("NODE transform = ", var_to_str(xf), " anchor_cell = ", var_to_str(cell))
	if args.has("save"):
		# Run windowed (pt-xvfb): headless has no shader compiler, so the blend material's
		# defaults would be saved as `null` and every editor session would rewrite them.
		var path := "res://demo/builder_demo_ruin.tres"
		var err: Error = preload("res://tests/build_demo_data.gd").save_with_uid(d, path)
		_check(err == OK, "saved " + path)
		_add_ext_uids(path)

## Writes the uid="uid://…" of every ext_resource like the editor does, so opening the demo in
## the editor does not rewrite the file.
func _add_ext_uids(path: String) -> void:
	var text := FileAccess.get_file_as_string(path)
	var out := PackedStringArray()
	for line in text.split("\n"):
		if line.begins_with("[ext_resource ") and not line.contains(" uid=\""):
			var p0 := line.find("path=\"") + 6
			var res_path := line.substr(p0, line.find("\"", p0) - p0)
			var id := ResourceLoader.get_resource_uid(res_path)
			if id != ResourceUID.INVALID_ID:
				line = line.replace(" path=\"", " uid=\"%s\" path=\"" % ResourceUID.id_to_text(id))
		out.append(line)
	var f := FileAccess.open(path, FileAccess.WRITE)
	f.store_string("\n".join(out))
