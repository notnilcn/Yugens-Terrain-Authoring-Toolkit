# Tests-only editor screenshot helper. Does nothing unless the editor is started with the
# `capture` user argument:
#   pt-xvfb <N> godot --path . --editor res://tests/builder_editor.tscn -- capture
# Builds a hex terrain with a builder mesh, selects it, drives the builder toolbar (face mode +
# a face selection, then vertex mode) and saves the whole editor window to tests/captures/.
@tool
extends Node3D

var frame := 0
var house: Node3D
var plugin: Object
var terrain: Node3D
var lab: Node3D
var cutter: Node3D
var fail := false
var shed: Node3D
var catcher: Control
var block: Node3D
var drag_mat: Material
var drag_face := -1
var annex: Node3D

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _ready() -> void:
	if not Engine.is_editor_hint() or not "capture" in OS.get_cmdline_user_args():
		set_process(false)
		return
	# Lit like the demos (the editor's preview sun / sky only show without scene lights, and
	# washed the builder walls out to flat white).
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.72, 0.86)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.55, 0.6, 0.7)
	e.ambient_light_energy = 0.4
	env.environment = e
	add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 40, 0)
	sun.light_energy = 1.0
	sun.shadow_enabled = true
	add_child(sun)
	terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	terrain.grid_type = 3
	terrain.ring_radius = 4
	terrain.new_chunk_noise_amplitude = 0.6
	# Demo-island grass (the 1 m default dwarfs a 2 m house).
	terrain.grass_size = Vector2(0.55, 0.65)
	add_child(terrain)
	terrain.add_chunk(Vector2i(0, 0))
	terrain.apply_brush("level", Vector3(0, 0, 0), 4.0, 0.5, 0)
	house = ClassDB.instantiate("PixelBuilderMesh")
	house.name = "House"
	house.data = ClassDB.instantiate("PixelBuilderMeshData")
	add_child(house)
	var cell: Vector2i = terrain.world_to_sample(Vector3.ZERO)
	house.place_on_cell(terrain, cell)
	var d = house.data
	var walls = d.add_shape(10, Vector3(2, 2.2, 2), Vector3.ZERO, {"rings": 1, "color": Color(0.86, 0.78, 0.62)})
	for f in d.faces_facing(Vector3(1, 0, 0), 25.0, walls):
		d.extrude(d.inset(PackedInt32Array([f]), 0.38), -0.1)
	d.add_shape(3, Vector3(7.6, 2.4, 7.6), Vector3(0, 2.2, 0), {"segments": 6, "smooth": false, "rotation_y": 30.0, "color": Color(0.72, 0.28, 0.22)})
	# Lab box (loops, knife, boolean) and a cylinder cutter overlapping it.
	lab = ClassDB.instantiate("PixelBuilderMesh")
	lab.name = "Lab"
	lab.data = ClassDB.instantiate("PixelBuilderMeshData")
	add_child(lab)
	lab.global_position = house.global_position + Vector3(3.2, 0, 2.6)
	lab.data.add_shape(0, Vector3(1.6, 1.2, 1.6), Vector3.ZERO, {"color": Color(0.46, 0.52, 0.6)})
	cutter = ClassDB.instantiate("PixelBuilderMesh")
	cutter.name = "Cutter"
	cutter.data = ClassDB.instantiate("PixelBuilderMeshData")
	add_child(cutter)
	cutter.global_position = lab.global_position + Vector3(0.8, 0.6, -0.8)
	cutter.data.add_shape(2, Vector3(1.0, 1.4, 1.0), Vector3.ZERO, {"segments": 8, "color": Color(0.9, 0.5, 0.3)})

## Window position of the dragged face's centre (the catcher spans the 3D viewport).
func _drop_point() -> Vector2:
	var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
	var spot: Vector2 = cam.unproject_position(shed.global_transform * shed.data.get_face_center(drag_face))
	return catcher.get_global_rect().position + spot

func _undo(obj: Object) -> void:
	var ur := EditorInterface.get_editor_undo_redo()
	ur.get_history_undo_redo(ur.get_object_history_id(obj)).undo()

func _make_builder(n: String, at: Vector3, size: Vector3, col: Color) -> Node3D:
	var b = ClassDB.instantiate("PixelBuilderMesh")
	b.name = n
	b.data = ClassDB.instantiate("PixelBuilderMeshData")
	add_child(b)
	b.global_position = at
	b.data.add_shape(0, size, Vector3.ZERO, {"color": col})
	return b

func _find_plugin() -> Object:
	var tb := EditorInterface.get_base_control().find_child("PixelBuilderToolbar", true, false)
	if tb == null:
		return null
	# Buttons sit in centring cells (same layout as the terrain toolbar).
	for c in tb.find_children("*", "Button", true, false):
		for con in c.get_signal_connection_list("pressed"):
			return con["callable"].get_object()
	return null

func _press(index: int) -> void:
	var tb := EditorInterface.get_base_control().find_child("PixelBuilderToolbar", true, false)
	var buttons := tb.find_children("*", "Button", true, false)
	buttons[index].emit_signal("pressed")

func _save(name: String) -> void:
	var img := EditorInterface.get_base_control().get_viewport().get_texture().get_image()
	img.save_png("res://tests/captures/%s.png" % name)
	print("EDITOR CAPTURE ", name, " ", img.get_size())

func _process(_d: float) -> void:
	frame += 1
	if frame == 30:
		EditorInterface.set_main_screen_editor("3D")
		EditorInterface.edit_node(house)
	elif frame == 45:
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		cam.global_position = house.global_position + Vector3(7, 7.5, 9)
		cam.look_at(house.global_position + Vector3(0, 1.4, 0))
		plugin = _find_plugin()
		print("plugin: ", plugin)
		plugin.on_add_slot()
		plugin.on_add_slot()
		_press(1)  # face mode
		var faces: PackedInt32Array = house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array())
		plugin.debug_select_faces(faces)
		plugin.on_slot_pick(1)
		print("state: ", plugin.get_state())
	elif frame == 70:
		_save("w5_editor_face_palette")
		# Edge mode: bevel the top rim of the walls.
		var top: PackedInt32Array = house.data.faces_facing(Vector3(0, 1, 0), 5.0, PackedInt32Array())
		var rim := PackedInt32Array()
		var v0: PackedInt32Array = house.data.get_face_edges(PackedInt32Array([top[0]]))
		rim.append_array(v0)
		plugin.debug_select_edges(rim)
		plugin.debug_action(10)  # Bevel
		print("after bevel: ", plugin.get_state(), " closed=", house.data.is_closed())
	elif frame == 90:
		_save("w5_editor_edge_bevel")
		_press(0)  # object mode
		plugin.debug_action(5)  # shape tool
		plugin.on_kind(5)  # stairs
		plugin.on_slot_pick(0)  # default slot, stone colour (slot 1 is the mint test material)
		plugin.on_color(Color(0.62, 0.56, 0.48))
		var p: Vector3 = house.global_position
		plugin.debug_draw_shape(p + Vector3(2.5, 0, -1.0), p + Vector3(4.5, 0, 2.0), 1.5)
		print("stairs drawn: ", plugin.get_state())
	elif frame == 110:
		_save("w5_editor_shape")
		EditorInterface.edit_node(house)
	elif frame == 125:
		plugin.debug_action(6)  # paint tool
		print("paint: ", plugin.get_state())
	elif frame == 140:
		_save("w5_editor_paint")
		var err: String = plugin.debug_export("res://tests/captures/w5_house.obj")
		print("export obj: '", err, "' ", FileAccess.file_exists("res://tests/captures/w5_house.obj"))
		err = plugin.debug_export("res://tests/captures/w5_house.glb")
		print("export glb: '", err, "' ", FileAccess.file_exists("res://tests/captures/w5_house.glb"))
		# Rotate handles on the house's side walls.
		_press(1)  # face mode
		plugin.debug_select_faces(house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array()))
		plugin.debug_set_xform(1)
	elif frame == 160:
		_save("w6_editor_rotate")
		plugin.debug_action(33)  # soft selection on
		plugin.debug_set_xform(2)  # scale handles
	elif frame == 175:
		_save("w6_editor_scale_soft")
		plugin.debug_action(33)  # soft selection off
		plugin.debug_set_xform(0)
		EditorInterface.edit_node(lab)
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		cam.global_position = lab.global_position + Vector3(2.4, 2.6, 3.0)
		cam.look_at(lab.global_position + Vector3(-0.2, 0.4, 0))
	elif frame == 190:
		# Edge loops across the lab box, then select one loop.
		var d = lab.data
		var side: PackedInt32Array = d.get_face_vertices(d.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0])
		var pair := PackedInt32Array()
		for i in side.size():
			var p0: Vector3 = d.get_vertex(side[i])
			var p1: Vector3 = d.get_vertex(side[(i + 1) % side.size()])
			if absf(p0.y - p1.y) < 1e-4:
				pair = PackedInt32Array([side[i], side[(i + 1) % side.size()]])
				break
		plugin.debug_select_edges(pair)
		plugin.on_field(2.0, 12)  # Cuts = 2
		plugin.debug_action(35)  # insert loop
		print("insert loop: ", plugin.get_state(), " faces=", d.get_face_count(), " closed=", d.is_closed())
		plugin.debug_action(37)  # select loop
		print("select loop: ", plugin.get_state())
	elif frame == 205:
		_save("w6_editor_loops")
		var d = lab.data
		var top: int = d.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
		var c: Vector3 = d.get_face_center(top)
		plugin.debug_knife(top, PackedVector3Array([c + Vector3(-0.3, 0, -0.3), c + Vector3(0.3, 0, -0.3), c + Vector3(0, 0, 0.35)]))
		print("knife: ", plugin.get_state(), " faces=", d.get_face_count(), " closed=", d.is_closed())
	elif frame == 220:
		_save("w6_editor_knife")
		_press(0)
		var p: Vector3 = house.global_position
		plugin.on_field(0.25, 5)  # wall thickness
		plugin.on_color(Color(0.66, 0.6, 0.52))  # stone wall, not the default white
		plugin.debug_wall(PackedVector3Array([p + Vector3(-3, 0, 3), p + Vector3(3, 0, 3), p + Vector3(3, 0, -3.5)]), false)
		print("wall: ", plugin.get_state())
	elif frame == 240:
		_save("w6_editor_wall")
		EditorInterface.edit_node(lab)
	elif frame == 250:
		var before: int = lab.data.get_face_count()
		print("boolean start faces=", before)
		plugin.debug_boolean(cutter, 1)  # subtract
	elif frame == 275:
		print("boolean result faces=", lab.data.get_face_count(), " closed=", lab.data.is_closed(), " cutter alive=", is_instance_valid(cutter) and cutter.is_inside_tree())
		_save("w6_editor_boolean")
		# UV editor: planar-project the house's +X walls, rotate them 90° and fit to 0-1.
		EditorInterface.edit_node(house)
	elif frame == 290:
		_press(1)  # face mode
		var walls: PackedInt32Array = house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array())
		plugin.debug_select_faces(walls)
		var canvas = plugin.debug_uv_editor()
		canvas.debug_select(walls)
		canvas.debug_act("planar")
		canvas.debug_act("rot90")
		canvas.debug_act("frame_sel")
		print("uv editor: manual=", house.data.is_face_uv_manual(walls[0]), " uvs=", house.data.get_face_uvs(walls[0], house.global_position))
		print("gizmo lock in face mode: ", house.has_meta("_edit_lock_"))
	elif frame == 310:
		var canvas = plugin.debug_uv_editor()
		var win: Window = canvas.get_window()
		var img := win.get_texture().get_image()
		img.save_png("res://tests/captures/w7_editor_uv_window.png")
		print("EDITOR CAPTURE w7_editor_uv_window ", img.get_size())
		_save("w7_editor_uv")
		win.hide()
		var prefs: Window = plugin.debug_preferences()
	elif frame == 330:
		var prefs: Window = plugin.debug_preferences()
		var img := prefs.get_texture().get_image()
		img.save_png("res://tests/captures/w7_editor_prefs.png")
		print("EDITOR CAPTURE w7_editor_prefs ", img.get_size())
		prefs.hide()
		_press(0)  # object mode: the lock is released
	elif frame == 345:
		print("gizmo lock in object mode: ", house.has_meta("_edit_lock_"))
		var flipped: int = house.data.conform_normals()
		print("conform normals flipped: ", flipped)
		# UV lock: move the +X walls (manual UVs from the UV editor) sideways: rigid, so their
		# coordinates ride along; then stretch one wall corner: refitted.
		_press(1)  # face mode
		var walls: PackedInt32Array = house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array())
		plugin.debug_select_faces(walls)
		var before: PackedVector2Array = house.data.get_face_uvs(walls[0], house.global_position)
		plugin.debug_move_selection(Vector3(0, 0.25, 0))
		var after: PackedVector2Array = house.data.get_face_uvs(walls[0], house.global_position)
		_check(plugin.get_state()["uv_lock"] and before == after, "UV lock: rigid move keeps manual UVs")
	elif frame == 360:
		# Vertex paint: brush a warm tint over the front walls, alpha mask on the roof.
		var walls: PackedInt32Array = house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array())
		plugin.on_color(Color(0.95, 0.45, 0.2))
		plugin.debug_field(26, 1.2)  # Radius
		plugin.debug_field(28, 0.6)  # Softness
		var pts := PackedVector3Array()
		var fs := PackedInt32Array()
		for i in 5:
			pts.append(house.global_transform * (house.data.get_face_center(walls[0]) + Vector3(0, -0.6 + 0.3 * i, -0.4 + 0.2 * i)))
			fs.append(walls[0])
		plugin.debug_vpaint(pts, fs)
		var c: Color = house.data.get_corner_color(walls[0], 0)
		print("vertex paint: ", plugin.get_state(), " corner=", c)
		_check(house.data.get_corner_color(walls[0], 0) != Color(0.86, 0.78, 0.62), "vertex paint changed a wall corner")
		# Hover the brush over the wall for the reticle.
		plugin.vp_hover_debug(house.global_transform * house.data.get_face_center(walls[0]), walls[0])
	elif frame == 375:
		_save("w8_editor_vpaint")
		# Blend material on the walls: layer B = moss texture, paint the mask with Fill / Alpha.
		var walls: PackedInt32Array = house.data.faces_facing(Vector3(1, 0, 0), 25.0, PackedInt32Array())
		plugin.debug_select_faces(walls)
		plugin.debug_action(50)  # new blend material
		var slot: int = house.data.get_face_slot(walls[0])
		var mat = house.data.materials[slot]
		print("blend material: slot=", slot, " ", mat.get_class())
		_check(mat != null and mat.get_class() == "PixelBuilderBlendMaterial", "blend material assigned to the walls")
		mat.set_shader_parameter("texture_b", load("res://addons/MarchingSquaresTerrain/textures/ground/moss.png"))
		mat.set_shader_parameter("albedo_a", Color(0.86, 0.78, 0.62))
		plugin.debug_field(25, 2.0)  # Channel = Alpha
		plugin.debug_field(29, 0.0)  # Alpha = 0 (layer A)
		plugin.debug_field(24, 1.0)  # Fill
		var fs := PackedInt32Array([walls[0]])
		plugin.debug_vpaint(PackedVector3Array([house.global_transform * house.data.get_face_center(walls[0])]), fs)
		_check(house.data.get_corner_color(walls[0], 0).a < 0.01, "alpha fill painted layer A on one wall")
		plugin.debug_action(51)  # invert alpha (selected walls)
		_check(house.data.get_corner_color(walls[0], 0).a > 0.99, "invert alpha swapped the layers")
		plugin.debug_action(51)
		plugin.debug_action(4)  # select tool
	elif frame == 390:
		_save("w8_editor_blend")
		# Gizmo-lock save check: own the house so it is saved, save while in Face mode.
		house.owner = self
		_press(1)
		print("lock before save: ", house.has_meta("_edit_lock_"))
		EditorInterface.save_scene_as("res://tests/captures/builder_lock_check.tscn")
	elif frame == 400:
		var text := FileAccess.get_file_as_string("res://tests/captures/builder_lock_check.tscn")
		_check(text.length() > 0 and text.contains("PixelBuilderMesh"), "scene saved with the builder mesh")
		_check(not text.contains("_edit_lock_") and not text.contains("_pt_builder_lock"), "no gizmo lock meta in the saved scene")
		_check(house.has_meta("_edit_lock_") and house.has_meta("_pt_builder_lock"), "gizmo lock restored after the save (still in Face mode)")
		# Bake the house to a MeshInstance3D with lightmap UV2.
		_press(0)
		plugin.debug_action(53)
	elif frame == 415:
		var baked = get_node_or_null("House")
		var ok: bool = baked != null and baked.get_class() == "MeshInstance3D" and baked.mesh != null
		var uv2 := false
		if ok:
			uv2 = (baked.mesh.surface_get_format(0) & Mesh.ARRAY_FORMAT_TEX_UV2) != 0
		if not ok:
			print("children: ", get_children().map(func(c): return "%s:%s" % [c.name, c.get_class()]))
		_check(ok, "house baked to a plain MeshInstance3D")
		_check(uv2, "baked mesh has lightmap UV2")
		_check(ok and baked.find_child("CollisionShape3D", true, false) != null, "baked mesh has a static collider")
		# W9 ops (Unibuilder ports): a shed and an annex for merge / flip edge / triangulate /
		# connect / subdivide edges / split / pivot / freeze / material drop.
		shed = _make_builder("Shed", lab.global_position + Vector3(-5.7, 0, -8.6), Vector3(1.6, 1.2, 1.4), Color(0.72, 0.6, 0.46))
		annex = _make_builder("Annex", shed.global_position + Vector3(1.3, 0, 0.2), Vector3(0.9, 0.8, 0.9), Color(0.5, 0.56, 0.62))
		annex.rotation_degrees = Vector3(0, 25, 0)
		EditorInterface.edit_node(shed)
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		cam.global_position = shed.global_position + Vector3(4.6, 3.4, -4.2)
		cam.look_at(shed.global_position + Vector3(0.6, 0.4, 0))
	elif frame == 430:
		var n_shed: int = shed.data.get_face_count()
		var n_annex: int = annex.data.get_face_count()
		var annex_world: Vector3 = annex.global_transform * annex.data.get_vertex(0)
		EditorInterface.get_selection().add_node(annex)
		plugin.debug_action(59)  # merge objects
		var merged: int = shed.data.get_face_count()
		_check(merged == n_shed + n_annex and not annex.is_inside_tree(), "merge objects: %d + %d faces, annex removed" % [n_shed, n_annex])
		var nv: int = shed.data.get_vertex_count()
		var found := false
		for i in nv:
			if (shed.global_transform * shed.data.get_vertex(i)).distance_to(annex_world) < 1e-3:
				found = true
		_check(found, "merge objects keeps the annex in place (transform baked)")
		# Face ops on the shed's top: flip the quad diagonal, then triangulate (Ctrl+T action).
		var top: int = shed.data.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
		var before: PackedInt32Array = shed.data.get_face_vertices(top)
		plugin.debug_select_faces(PackedInt32Array([top]))
		plugin.debug_action(56)  # flip face edge
		var after: PackedInt32Array = shed.data.get_face_vertices(top)
		_check(after.size() == 4 and after[0] == before[1] and after[3] == before[0], "flip face edge rotates the quad loop")
		# Edge ops on a side wall: connect two opposite edges, subdivide another.
		var side: int = shed.data.faces_facing(Vector3(0, 0, -1), 5.0, PackedInt32Array())[0]
		var sv: PackedInt32Array = shed.data.get_face_vertices(side)
		var nf: int = shed.data.get_face_count()
		plugin.debug_select_edges(PackedInt32Array([sv[0], sv[1], sv[2], sv[3]]))
		plugin.debug_action(55)  # connect edges
		_check(shed.data.get_face_count() == nf + 1 and plugin.get_state()["edges"] == 1, "connect edges splits the wall (%s)" % plugin.get_state())
		var right: int = shed.data.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0]
		var rv: PackedInt32Array = shed.data.get_face_vertices(right)
		var nvert: int = shed.data.get_vertex_count()
		plugin.on_field(2.0, 12)  # Cuts = 2 -> 3 segments
		plugin.debug_select_edges(PackedInt32Array([rv[0], rv[1]]))
		plugin.debug_action(54)  # subdivide edges
		_check(shed.data.get_vertex_count() == nvert + 2, "subdivide edges adds Cuts vertices")
		_check(shed.data.is_closed(), "shed stays watertight after connect / subdivide")
		plugin.debug_select_faces(shed.data.faces_facing(Vector3.UP, 5.0, PackedInt32Array()))
		var ntop: int = plugin.get_state()["faces"]
		plugin.debug_action(58)  # triangulate
		_check(plugin.get_state()["faces"] > ntop and shed.data.is_closed(), "triangulate the tops (%d -> %d faces)" % [ntop, plugin.get_state()["faces"]])
	elif frame == 445:
		_save("w9_editor_face_ops")
		# Split a corner vertex, then undo it (closed again).
		var nv: int = shed.data.get_vertex_count()
		plugin.debug_select_verts(PackedInt32Array([0]))
		plugin.debug_action(57)  # split vertices
		_check(shed.data.get_vertex_count() > nv and not shed.data.is_closed(), "split vertices un-shares the corner")
		_undo(shed.data)
		_check(shed.data.get_vertex_count() == nv and shed.data.is_closed(), "undo split vertices")
		# Object mode: freeze a rotated / scaled transform, then centre the pivot.
		_press(0)
		var p0: Vector3 = shed.global_transform * shed.data.get_vertex(3)
		shed.rotation_degrees = Vector3(0, 20, 0)
		shed.scale = Vector3(1.1, 1.0, 1.1)
		var p1: Vector3 = shed.global_transform * shed.data.get_vertex(3)
		plugin.debug_action(62)  # freeze transform
		_check(shed.transform.is_equal_approx(Transform3D.IDENTITY) and (shed.global_transform * shed.data.get_vertex(3)).distance_to(p1) < 1e-3, "freeze transform bakes into the mesh")
		plugin.debug_action(60)  # center pivot
		var c: Vector3 = (shed.global_transform * shed.data.get_vertex(3))
		_check(c.distance_to(p1) < 1e-3 and shed.position.distance_to(p0) > 0.01, "center pivot moves the origin, not the mesh")
		_undo(shed.data)
		_undo(shed.data)
		_check(shed.transform.basis.get_scale().x > 1.05, "undo freeze restores the transform")
		shed.rotation_degrees = Vector3.ZERO
		shed.scale = Vector3.ONE
		# Material drag-and-drop onto the front faces (Face mode: the hovered face).
		_press(1)
		var drop_mat := StandardMaterial3D.new()
		drop_mat.albedo_color = Color(0.35, 0.55, 0.3)
		var front: int = shed.data.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0]
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		var spot: Vector2 = cam.unproject_position(shed.global_transform * shed.data.get_face_center(front))
		var ok: bool = plugin.debug_drop_material(drop_mat, spot)
		var slot: int = shed.data.get_face_slot(front)
		_check(ok and slot >= 0 and shed.data.materials.size() > slot and shed.data.materials[slot] == drop_mat, "material dropped onto the hovered face (slot %d)" % slot)
		_check(not plugin.debug_drop_material(drop_mat, Vector2(2, 2)), "a drop on empty space is refused")
		print("w9 state: ", plugin.get_state())
	elif frame == 460:
		_save("w9_editor_object_ops")
		# A real drag-and-drop through the GUI: start a Material drag, move over the shed's top
		# face and release; the builder's drop catcher over the viewport must take it.
		catcher = EditorInterface.get_base_control().find_child("_PixelBuilderMaterialDrop", true, false)
		_check(catcher != null, "material drop catcher sits over the 3D viewport")
		drag_mat = StandardMaterial3D.new()
		drag_mat.albedo_color = Color(0.75, 0.55, 0.3)
		drag_face = shed.data.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
		if catcher:
			catcher.force_drag({"type": "resource", "resource": drag_mat}, null)
	elif frame == 463 or frame == 466:
		if catcher:
			var ev := InputEventMouseMotion.new()
			ev.position = _drop_point()
			ev.global_position = ev.position
			ev.button_mask = MOUSE_BUTTON_MASK_LEFT
			# The GUI resolves the drop target at the pointer, so move the real pointer too.
			catcher.get_viewport().warp_mouse(ev.position)
			catcher.get_viewport().push_input(ev)
	elif frame == 469:
		if catcher:
			_check(catcher.mouse_filter == Control.MOUSE_FILTER_PASS, "drop catcher catches the mouse while a material is dragged")
			var b := InputEventMouseButton.new()
			b.button_index = MOUSE_BUTTON_LEFT
			b.pressed = false
			b.position = _drop_point()
			b.global_position = b.position
			catcher.get_viewport().push_input(b)
	elif frame == 475:
		var slot: int = shed.data.get_face_slot(drag_face)
		_check(slot >= 0 and slot < shed.data.materials.size() and shed.data.materials[slot] == drag_mat, "dragged material dropped onto the shed top through the GUI (slot %d)" % slot)
		_check(catcher == null or catcher.mouse_filter == Control.MOUSE_FILTER_IGNORE, "drop catcher lets the mouse through again")
	elif frame == 480:
		_save("w10_editor_material_drag")
		# Toolbar: one header per section, distinct names; Builder menu replaces Godot's "Mesh".
		var tb := EditorInterface.get_base_control().find_child("PixelBuilderToolbar", true, false)
		var heads := {}
		var dup := false
		for l in tb.find_children("*Header", "Label", true, false):
			if heads.has(l.text):
				dup = true
			heads[l.text] = true
		_check(not dup and not heads.has("Mesh"), "builder toolbar sections are unique (%s)" % ", ".join(heads.keys()))
		var menus: Dictionary = plugin.debug_menus()
		_check(menus["builder_menu"] and menus["builder_menu_text"] == "Builder", "Builder menu in the 3D toolbar")
		_check(menus["mesh_menu_hidden"], "Godot's Mesh menu hidden while a builder mesh is edited")
		# W10 ports on a fresh block.
		block = _make_builder("Block", shed.global_position + Vector3(0, 0, 2.6), Vector3(1.6, 1.0, 1.2), Color(0.62, 0.6, 0.66))
		EditorInterface.edit_node(block)
	elif frame == 490:
		var d = block.data
		var top: int = d.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
		# Subdivide with 4 segments (Unibuilder subdivide_faces).
		plugin.debug_field(31, 4.0)
		plugin.debug_select_faces(PackedInt32Array([top]))
		plugin.debug_action(11)  # subdivide
		_check(plugin.get_state()["faces"] == 16 and d.is_closed(), "subdivide the top into 4 x 4 (%d faces)" % plugin.get_state()["faces"])
		# Equal-width inset on one front face.
		var front: int = d.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0]
		var corners: int = d.get_face_vertices(front).size()
		var nf0: int = d.get_face_count()
		plugin.debug_field(30, 2.0)  # Inset by = Equal
		plugin.debug_field(10, 0.15)  # Inset = 0.15 m
		plugin.debug_select_faces(PackedInt32Array([front]))
		plugin.debug_action(9)  # inset
		_check(d.is_closed() and d.get_face_count() == nf0 + corners, "equal inset adds one rim quad per corner (%d -> %d faces)" % [nf0, d.get_face_count()])
		# Trim walls along the base of the side walls.
		var walls := PackedInt32Array()
		for f in d.get_face_count():
			if absf(d.get_face_normal(f).y) < 0.1 and d.get_face_center(f).y < 0.6:
				walls.append(f)
		var nf: int = d.get_face_count()
		plugin.debug_select_faces(walls)
		plugin.debug_action(65)  # trim walls
		_check(d.get_face_count() > nf, "trim walls added a moulding (%d faces)" % (d.get_face_count() - nf))
		# Insert vertex on a top edge of the far side, then collapse it back.
		var bottom: int = d.faces_facing(Vector3.DOWN, 5.0, PackedInt32Array())[0]
		var bv: PackedInt32Array = d.get_face_vertices(bottom)
		var nv: int = d.get_vertex_count()
		plugin.debug_insert_vertex(bv[0], bv[1], block.global_transform * d.get_vertex(bv[0]).lerp(d.get_vertex(bv[1]), 0.3))
		_check(d.get_vertex_count() == nv + 1, "insert vertex splits the edge")
		plugin.debug_select_verts(PackedInt32Array([nv, bv[0]]))
		plugin.debug_action(63)  # collapse
		_check(d.get_vertex_count() == nv, "collapse merges the inserted vertex back")
		# Handles: Normal orientation (screenshot), collider toggle.
		plugin.debug_select_faces(PackedInt32Array([front]))
		plugin.debug_field(32, 2.0)  # Axes = Normal
		plugin.debug_set_xform(1)
	elif frame == 500:
		_save("w10_editor_ops")
		plugin.debug_set_xform(0)
		plugin.debug_field(32, 0.0)
		_press(0)
		plugin.debug_action(67)  # collider
		_check(block.entity_type == 1 and block.material_override != null, "collider: entity type set, tinted in the editor")
		_undo(block)
		_check(block.entity_type == 0 and block.material_override == null, "undo collider")
		# Draw on surface: the placement on the block's +X face stands along +X.
		plugin.debug_action(66)
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		cam.global_position = block.global_position + Vector3(4.5, 1.2, 0.4)
		cam.look_at(block.global_position + Vector3(0, 0.5, 0))
	elif frame == 505:
		var cam := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		var side: int = block.data.faces_facing(Vector3(1, 0, 0), 5.0, PackedInt32Array())[0]
		var spot := cam.unproject_position(block.global_transform * (block.data.get_face_center(side) + Vector3(0, 0.25, 0.3)))
		var pl: Dictionary = plugin.debug_placement(spot)
		_check(pl.has("up") and (pl["up"] as Vector3).dot(Vector3.RIGHT) > 0.95, "draw on surface stands shapes along the face normal (%s)" % [pl.get("up")])
		plugin.debug_action(66)
		# Poly shape (clicked outline extruded up).
		plugin.on_kind(16)
		plugin.debug_field(1, 0.8)  # H
		var p: Vector3 = block.global_position + Vector3(-3.0, 0, 0)
		plugin.debug_wall(PackedVector3Array([p, p + Vector3(1.5, 0, 0), p + Vector3(1.5, 0, 1.0), p + Vector3(0.6, 0, 1.6), p + Vector3(0, 0, 1.0)]), true)
		var poly = get_node_or_null("Polyshape")
		_check(poly != null and poly.data.is_closed() and poly.data.get_face_count() == 7, "poly shape: a closed 5-sided prism")
		plugin.on_kind(0)
		EditorInterface.edit_node(block)
		# Live lightmap UV2 on the editable mesh (unwrapped half a second after the last edit).
		block.live_uv2 = true
	elif frame == 515:
		_save("w10_editor_poly")
	elif frame == 600:
		var m: Mesh = block.mesh
		_check(m != null and (m.surface_get_format(0) & Mesh.ARRAY_FORMAT_TEX_UV2) != 0, "live UV2: the editable mesh carries lightmap UV2")
		print("BUILDER EDITOR ", "FAIL" if fail else "OK")
		get_tree().quit()
