# Level-builder capture: builds a small hex-aligned hamlet (house, tower, arch bridge, stairs)
# with PixelBuilderMesh on a hex-ring terrain, renders it through the pixel pipeline and writes
# tests/captures/<name>.png. Also checks a few builder invariants (exit code 1 on failure).
#   godot --path . --script res://tests/builder_demo.gd -- [name=builder_demo] [px=3] [debug=0] [size=34]
# With save=1 it also writes the scene to res://demo/builder_demo.tscn (terrain samples in
# res://demo/builder_demo_data.res, builder meshes embedded).
extends SceneTree

const BOX := 0
const CYLINDER := 2
const CONE := 3
const STAIRS := 5
const ARCH := 6
const HEX_CLUSTER := 10
const TORUS := 11
const DOOR := 12
const CURVED_STAIRS := 13
const TRIM := 14

var frames := 0
var args := {}
var renderer
var terrain
var fail := false

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _builder(parent: Node, name: String, cell: Vector2i) -> Node:
	var b = ClassDB.instantiate("PixelBuilderMesh")
	b.name = name
	b.data = ClassDB.instantiate("PixelBuilderMeshData")
	parent.add_child(b)
	b.place_on_cell(terrain, cell)
	return b

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
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.shadow_enabled = true
	root3d.add_child(sun)

	terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	terrain.grid_type = 3
	terrain.ring_radius = 7
	terrain.new_chunk_noise_amplitude = 1.5
	terrain.new_chunk_noise_frequency = 0.08
	# Demo-island grass: the 1 m default blades dwarf the 2.4 m house.
	terrain.grass_size = Vector2(0.55, 0.65)
	root3d.add_child(terrain)
	terrain.add_chunk(Vector2i(0, 0))
	# Plateaus for the buildings, a dip for the bridge.
	terrain.apply_brush("level", Vector3(-5, 0, -1), 5.0, 1.0, 0)
	terrain.apply_brush("level", Vector3(6, 0, 1), 4.0, 2.0, 0)
	terrain.apply_brush("level", Vector3(0.5, 0, 0), 2.0, -1.2, 0)
	terrain.apply_brush("paint", Vector3(0.5, 0, 0), 2.5, 2.0, 0)
	for p in [Vector3(-5, 0, -1), Vector3(6, 0, 1), Vector3(0.5, 0, 0), Vector3(4, 0, 4)]:
		terrain.apply_brush("grass_off", p, 4.5, 1.0, 0)

	var obj = ClassDB.instantiate("PixelObject")
	obj.name = "Buildings"
	root3d.add_child(obj)

	# House: a 7-cell hex cluster with a door, windows and a six-sided roof.
	var house_cell: Vector2i = terrain.world_to_sample(Vector3(-5, 0, -1))
	var house = _builder(obj, "House", house_cell)
	var d = house.data
	var cell_w: float = terrain.cell_size.x
	var walls = d.add_shape(HEX_CLUSTER, Vector3(cell_w, 2.4, cell_w), Vector3.ZERO, {"rings": 1, "color": Color(0.86, 0.78, 0.62)})
	_check(d.is_closed(), "hex cluster is closed")
	var front = d.faces_facing(Vector3(0.5, 0, 0.87), 25.0, walls)
	_check(front.size() > 0, "house has front walls")
	if front.size() > 0:
		var door = d.inset(PackedInt32Array([front[0]]), 0.32)
		d.extrude(door, -0.15)
		d.set_faces_color(door, Color(0.42, 0.26, 0.16))
	var side = d.faces_facing(Vector3(1, 0, 0), 25.0, walls)
	for f in side:
		var win = d.inset(PackedInt32Array([f]), 0.38)
		d.extrude(win, -0.1)
		d.set_faces_color(win, Color(0.3, 0.45, 0.65))
	d.add_shape(CONE, Vector3(7.6, 2.6, 7.6), Vector3(0, 2.4, 0), {"segments": 6, "smooth": false, "rotation_y": 30.0, "color": Color(0.72, 0.28, 0.22)})
	_check(d.is_closed(), "house stays closed after inset/extrude")

	# Tower: one terrain hex cell, recessed crenellated top, cone roof on a ring of posts.
	var tower_cell: Vector2i = terrain.world_to_sample(Vector3(6, 0, 1))
	var tower = _builder(obj, "Tower", tower_cell)
	var td = tower.data
	var body = td.add_terrain_cells(terrain, [tower_cell], tower_cell, 5.0, Vector3.ZERO, {"color": Color(0.62, 0.62, 0.66)})
	var top = td.faces_facing(Vector3.UP, 5.0, body)
	var inner = td.inset(top, 0.22)
	td.extrude(inner, -0.35)
	var band = td.faces_facing(Vector3(0, 0, 1), 40.0, body)
	for f in band:
		var slit = td.inset(PackedInt32Array([f]), 0.4)
		td.extrude(slit, -0.08)
		td.set_faces_color(slit, Color(0.2, 0.2, 0.25))
	td.add_shape(CYLINDER, Vector3(0.25, 1.2, 0.25), Vector3(0, 5.0, 0), {"segments": 6})
	td.add_shape(CONE, Vector3(cell_w * 1.25, 2.0, cell_w * 1.25), Vector3(0, 6.2, 0), {"segments": 6, "smooth": false, "rotation_y": 30.0, "color": Color(0.3, 0.38, 0.62)})
	var cfg_w: float = cell_w
	_check(absf(td.get_face_center(top[0]).y - 4.65) < 0.05, "tower top recessed (%.2f)" % td.get_face_center(top[0]).y)
	_check(cfg_w > 0.0, "terrain cell width")

	# Arch bridge over the dip, stairs up to the tower.
	var bridge_cell: Vector2i = terrain.world_to_sample(Vector3(0.5, 0, 0))
	var bridge = _builder(obj, "Bridge", bridge_cell)
	bridge.position.y = 0.6
	bridge.data.add_shape(ARCH, Vector3(6.5, 2.0, 2.0), Vector3(0, -0.8, 0), {"thickness": 0.45, "segments": 10, "color": Color(0.7, 0.66, 0.6)})
	var stairs_cell: Vector2i = terrain.world_to_sample(Vector3(4, 0, 4))
	var stairs = _builder(obj, "Stairs", stairs_cell)
	stairs.data.add_shape(STAIRS, Vector3(1.6, 1.4, 2.4), Vector3.ZERO, {"steps": 5, "rotation_y": float(args.get("srot", "150")), "color": Color(0.66, 0.6, 0.52)})
	_check(house.anchored and house.anchor_cell == house_cell, "house anchored to its cell")

	# Bevelled plinth with a merged, re-textured top (bevel / merge / UV / OBJ / convert API).
	var plinth_cell: Vector2i = terrain.world_to_sample(Vector3(-1.5, 0, 4.5))
	var plinth = _builder(obj, "Plinth", plinth_cell)
	var pd = plinth.data
	var pf = pd.add_shape(BOX, Vector3(2.4, 0.8, 1.6), Vector3.ZERO, {"color": Color(0.55, 0.58, 0.62)})
	var ptop = pd.faces_facing(Vector3.UP, 5.0, pf)
	var strips = pd.bevel(pd.get_face_edges(ptop), 0.2)
	_check(strips.size() == 8 and pd.is_closed(), "bevelled plinth rim (%d strips + corners)" % strips.size())
	var tops = pd.subdivide(pd.faces_facing(Vector3.UP, 5.0, PackedInt32Array()))
	_check(pd.merge_faces(tops) >= 0 and pd.is_closed(), "merge subdivided top back into one face")
	pd.set_faces_uv(pd.faces_facing(Vector3.UP, 5.0, PackedInt32Array()), Vector2(0.25, 0), Vector2(2, 2), 90.0, true)
	var obj_text: String = pd.to_obj("plinth")
	_check(obj_text.count("\nf ") == pd.get_face_count(), "OBJ has one n-gon per face")
	var conv = ClassDB.instantiate("PixelBuilderMeshData")
	conv.set_from_mesh(BoxMesh.new())
	_check(conv.get_face_count() == 6 and conv.is_closed(), "BoxMesh converts to 6 quads (%d)" % conv.get_face_count())

	# Vertex paint + blend material (Unibuilder paint tool / UBBlendMaterial): a paved path
	# painted into a grass courtyard through the blend mask (alpha), and a weathered tint
	# brushed onto the house's front base (RGB only).
	var yard_slab = _builder(obj, "Courtyard", terrain.world_to_sample(Vector3(-0.3, 0, -4.6)))
	var cd = yard_slab.data
	cd.add_shape(BOX, Vector3(2.6, 0.25, 2.6), Vector3.ZERO, {})
	for i in 3:
		cd.subdivide(cd.faces_facing(Vector3.UP, 5.0, PackedInt32Array()))
	var blend = ClassDB.instantiate("PixelBuilderBlendMaterial")
	blend.ensure_shader()
	blend.set_shader_parameter("texture_a", load("res://addons/MarchingSquaresTerrain/textures/ground/path.png"))
	blend.set_shader_parameter("texture_b", load("res://addons/MarchingSquaresTerrain/textures/ground/grass.png"))
	blend.set_shader_parameter("uv_scale_a", Vector2(0.5, 0.5))
	blend.set_shader_parameter("uv_scale_b", Vector2(0.5, 0.5))
	blend.set_shader_parameter("mask_steps", 2)
	# The ground textures are grey detail maps: tint them like terrain slots.
	blend.set_shader_parameter("albedo_a", Color(0.66, 0.58, 0.47))
	blend.set_shader_parameter("albedo_b", Color(0.36, 0.6, 0.27))
	var mats: Array[Material] = [blend]
	cd.materials = mats
	var top_faces: PackedInt32Array = cd.faces_facing(Vector3.UP, 5.0, PackedInt32Array())
	_check(top_faces.size() == 64, "courtyard top subdivided (%d faces)" % top_faces.size())
	for k in 9:
		var t := float(k) / 8.0
		cd.paint_brush(Vector3(lerpf(-1.3, 1.3, t), 0.25, lerpf(1.1, -1.3, t) + sin(t * 6.0) * 0.25), 0.55, 1.0, 0.4, Color(0, 0, 0, 0), 2)
	var path_corners := 0
	for f in top_faces:
		for c in cd.get_face_vertices(f).size():
			if cd.get_corner_color(f, c).a < 0.5:
				path_corners += 1
	_check(path_corners > 20 and path_corners < 200, "blend mask painted along the path (%d corners)" % path_corners)
	_check(cd.get_corner_color(top_faces[0], 0).r > 0.99, "alpha-only paint keeps the colour")
	var blob: PackedByteArray = cd.blob
	var copy = ClassDB.instantiate("PixelBuilderMeshData")
	copy.blob = blob
	_check(copy.get_corner_color(top_faces[3], 1) == cd.get_corner_color(top_faces[3], 1), "corner colours survive the blob (v3)")
	if front.size() > 0:
		var c: Vector3 = d.get_face_center(front[0])
		d.paint_brush(Vector3(c.x, 0.0, c.z), 1.6, 0.7, 1.0, Color(0.5, 0.42, 0.34), 1)
		_check(d.get_corner_color(front[0], 0).g < 0.95, "house front base tinted by the brush")

	# Unibuilder ports: garden wall (polyline), door, curved stairs, torus, trim, rounded
	# bevel, edge loops, knife and mirror.
	var yard_cell: Vector2i = terrain.world_to_sample(Vector3(-5, 0, -1))
	var yard = _builder(obj, "GardenWall", yard_cell)
	var yd = yard.data
	var ring := PackedVector3Array([Vector3(-0.6, 0, 2.6), Vector3(-3.0, 0, 2.6), Vector3(-3.0, 0, -2.6), Vector3(2.2, 0, -2.6), Vector3(2.2, 0, 2.6), Vector3(1.0, 0, 2.6)])
	yd.add_wall(ring, false, 1.1, Vector3.ZERO, {"thickness": 0.25, "color": Color(0.66, 0.62, 0.56)})
	_check(yd.is_closed() and yd.get_face_count() == 22, "open garden wall is one swept solid (%d faces)" % yd.get_face_count())
	var nwall: int = yd.get_face_count()
	var gate_faces = yd.add_shape(DOOR, Vector3(1.6, 1.8, 0.3), Vector3(0.2, 0, 2.6), {"opening": 0.8, "door_height": 1.4, "arched": true, "segments": 8, "color": Color(0.6, 0.5, 0.42)})
	_check(yd.is_closed() and gate_faces.size() > 10 and yd.get_face_count() == nwall + gate_faces.size(), "arched gate in the garden wall is closed")
	var spiral = _builder(obj, "SpiralStairs", terrain.world_to_sample(Vector3(7.5, 0, -1.5)))
	spiral.data.add_shape(CURVED_STAIRS, Vector3(2.2, 2.0, 2.2), Vector3.ZERO, {"steps": 8, "arc": 120.0, "inner": 0.35, "color": Color(0.6, 0.6, 0.64)})
	_check(spiral.data.is_closed(), "curved stairs are closed")
	var well = _builder(obj, "Well", terrain.world_to_sample(Vector3(2.5, 0, -4)))
	var wd = well.data
	wd.add_shape(CYLINDER, Vector3(1.4, 0.5, 1.4), Vector3.ZERO, {"segments": 10, "color": Color(0.5, 0.5, 0.55)})
	wd.add_shape(TORUS, Vector3(1.6, 0.3, 1.6), Vector3(0, 0.45, 0), {"thickness": 0.15, "segments": 16, "steps": 6, "color": Color(0.55, 0.42, 0.3)})
	_check(wd.is_closed(), "well (cylinder + torus) is closed")
	var trim_faces = pd.add_shape(TRIM, Vector3(0.2, 0.15, 1.6), Vector3(1.3, 0, 0), {"profile": 3, "thickness": 0.12, "segments": 4, "color": Color(0.45, 0.47, 0.52)})
	_check(trim_faces.size() > 6, "cove trim along the plinth (%d faces)" % trim_faces.size())

	var lab = ClassDB.instantiate("PixelBuilderMeshData")
	var lf = lab.add_shape(BOX, Vector3(1, 1, 1), Vector3.ZERO, {})
	var ltop = lab.faces_facing(Vector3.UP, 5.0, lf)
	var ledges: PackedInt32Array = lab.get_face_edges(ltop)
	var round = lab.bevel_rounded(PackedInt32Array([ledges[0], ledges[1]]), 0.2, 4)
	_check(round.size() == 4 and lab.is_closed(), "rounded bevel: 4 strips (%d)" % round.size())
	var vtx: PackedInt32Array = lab.get_face_vertices(lab.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0])
	var a := -1
	var b := -1
	for i in vtx.size():
		var p0: Vector3 = lab.get_vertex(vtx[i])
		var p1: Vector3 = lab.get_vertex(vtx[(i + 1) % vtx.size()])
		if absf(p0.y - p1.y) > 0.5 and absf(p0.z - p1.z) < 1e-4:
			a = vtx[i]
			b = vtx[(i + 1) % vtx.size()]
			break
	var nf: int = lab.get_face_count()
	var loop_edges: PackedInt32Array = lab.insert_loop(a, b, 2) if a >= 0 else PackedInt32Array()
	_check(loop_edges.size() > 0 and lab.is_closed() and lab.get_face_count() > nf, "insert edge loops (%d edges)" % (loop_edges.size() / 2))
	var cut_box = ClassDB.instantiate("PixelBuilderMeshData")
	var cf = cut_box.add_shape(BOX, Vector3(2, 1, 2), Vector3.ZERO, {})
	var ctop: int = cut_box.faces_facing(Vector3.UP, 5.0, cf)[0]
	var inner_face: int = cut_box.cut_face(ctop, PackedVector3Array([Vector3(-0.5, 1, -0.5), Vector3(0.5, 1, -0.5), Vector3(0.5, 1, 0.5), Vector3(-0.5, 1, 0.5)]))
	_check(inner_face >= 0 and cut_box.is_closed(), "knife cuts a square into the top")
	var half = ClassDB.instantiate("PixelBuilderMeshData")
	half.add_shape(BOX, Vector3(1, 1, 1), Vector3(0.6, 0, 0), {})
	var nv: int = half.get_vertex_count()
	half.mirror(0)
	_check(half.get_face_count() == 12 and half.get_vertex_count() == nv * 2, "mirror doubles a box off the plane")
	# W9 Unibuilder ports: merge objects, subdivide / connect edges, flip edge, split, triangulate.
	var ops = ClassDB.instantiate("PixelBuilderMeshData")
	var of = ops.add_shape(BOX, Vector3(1, 1, 1), Vector3.ZERO, {})
	var other = ClassDB.instantiate("PixelBuilderMeshData")
	other.add_shape(BOX, Vector3(1, 1, 1), Vector3.ZERO, {})
	_check(ops.merge_from(other, Transform3D(Basis(Vector3.UP, 0.5), Vector3(3, 0, 0))).size() == 6 and ops.is_closed(), "merge objects appends a transformed copy")
	var otop: int = ops.faces_facing(Vector3.UP, 5.0, of)[0]
	var ov: PackedInt32Array = ops.get_face_vertices(otop)
	_check(ops.subdivide_edges(PackedInt32Array([ov[0], ov[1]]), 4).size() == 3 and ops.is_closed(), "subdivide edges into 4")
	# The two sides next to the split edge are opposite each other.
	var opp := PackedInt32Array([ov[1], ov[2], ov[3], ov[0]])
	_check(opp.size() == 4 and ops.connect_edges(opp).size() == 2 and ops.is_closed(), "connect edges splits the top")
	var nv2: int = ops.get_vertex_count()
	_check(ops.split_vertices(PackedInt32Array([0])).size() == 2 and ops.get_vertex_count() == nv2 + 2, "split vertices")
	_check(ops.triangulate(PackedInt32Array()).size() >= 24, "triangulate the whole object")
	# W10 Unibuilder ports: segments subdivide, width / equal inset, insert + collapse, trim walls, poly.
	var w10 = ClassDB.instantiate("PixelBuilderMeshData")
	var wf = w10.add_shape(BOX, Vector3(2, 1, 2), Vector3.ZERO, {})
	var wtop: int = w10.faces_facing(Vector3.UP, 5.0, wf)[0]
	_check(w10.subdivide_segments(PackedInt32Array([wtop]), 4).size() == 16 and w10.is_closed(), "subdivide into 4 x 4")
	var wside: int = w10.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0]
	_check(w10.inset_width(PackedInt32Array([wside]), 0.2, true).size() == 1 and w10.is_closed(), "equal inset")
	var wb: PackedInt32Array = w10.get_face_vertices(w10.faces_facing(Vector3.DOWN, 5.0, PackedInt32Array())[0])
	var nv3: int = w10.get_vertex_count()
	var iv: int = w10.insert_vertex(wb[0], wb[1], w10.get_vertex(wb[0]).lerp(w10.get_vertex(wb[1]), 0.5))
	_check(iv == nv3 and w10.collapse_vertices(PackedInt32Array([iv, wb[0]])) >= 0 and w10.get_vertex_count() == nv3 and w10.is_closed(), "insert vertex + collapse")
	var walls4 := PackedInt32Array()
	for f in w10.get_face_count():
		if absf(w10.get_face_normal(f).y) < 0.1:
			walls4.append(f)
	_check(w10.trim_walls(walls4, false, 0.08, 0.15, 3).size() > 0, "trim walls (cove) around the base")
	var pf2 = w10.add_poly(PackedVector3Array([Vector3(0, 0, 0), Vector3(2, 0, 0), Vector3(2, 0, 1), Vector3(1, 0, 1), Vector3(1, 0, 2), Vector3(0, 0, 2)]), 0.5, Vector3(4, 0, 0), {})
	_check(pf2.size() == 8, "L-shaped poly prism (%d faces)" % pf2.size())

	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = float(args.get("size", "26"))
	var dist := 40.0
	var yaw := deg_to_rad(float(args.get("yaw", "30")))
	cam.position = Vector3(sin(yaw) * 0.85, 0.75, cos(yaw) * 0.85) * dist + Vector3(0.5, 0, 0)
	root3d.add_child(cam)
	cam.look_at(Vector3(0.5, 1.5, 0))
	cam.current = true
	renderer = ClassDB.instantiate("PixelRenderer")
	renderer.pixel_size = int(args.get("px", "3"))
	renderer.debug_view = int(args.get("debug", "0"))
	root3d.add_child(renderer)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup()
	if frames == 24:
		var mi: MeshInstance3D = root.find_child("House", true, false)
		_check(mi != null and mi.mesh != null and mi.mesh.get_surface_count() > 0, "house compiled to a mesh")
		_check(mi != null and (mi.layers & (1 << 19)) != 0, "house joined the pixel metadata layer")
		var court: MeshInstance3D = root.find_child("Courtyard", true, false)
		_check(court != null and (court.layers & (1 << 19)) != 0, "blend-material courtyard joined the pixel metadata layer")
		var sm = court.get_surface_override_material(0) if court else null
		_check(sm == null or sm is ShaderMaterial, "blend material kept as authored by PixelObject")
		var name: String = args.get("name", "builder_demo")
		# Headless has no framebuffer to read back: run the checks (and save=1), skip the PNG.
		if DisplayServer.get_name() != "headless":
			var img := root.get_texture().get_image()
			if img:
				img.save_png("res://tests/captures/%s.png" % name)
		if args.has("save"):
			_save_scene()
		print("BUILDER ", name, " ", renderer.get_status_line())
		quit(1 if fail else 0)
	return false


func _own(n: Node, owner: Node) -> void:
	for c in n.get_children():
		c.owner = owner
		_own(c, owner)

func _save_scene() -> void:
	var scene: Node3D = root.get_child(root.get_child_count() - 1)
	scene.name = "BuilderDemo"
	var td = terrain.data
	if td:
		preload("res://tests/build_demo_data.gd").save_with_uid(td, "res://demo/builder_demo_data.res")
		terrain.data = load("res://demo/builder_demo_data.res")
	# Undo the renderer's runtime takeover of the camera and give nodes readable names.
	for c in scene.get_children():
		if c is Camera3D:
			c.cull_mask = 0xFFFFF
		var n := String(c.name).trim_prefix("@").get_slice("@", 0)
		if n == "MarchingSquaresTerrain":
			n = "Terrain"
		c.name = n
	_own(scene, scene)
	var packed := PackedScene.new()
	var err := packed.pack(scene)
	if err == OK:
		err = ResourceSaver.save(packed, "res://demo/builder_demo.tscn")
	_check(err == OK, "saved res://demo/builder_demo.tscn")
