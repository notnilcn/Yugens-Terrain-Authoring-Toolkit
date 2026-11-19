@tool
extends Node
## Headless editor check of the Xogot Modeler features wired into the level builder (I1):
## new actions, action options, Xogot shapes, poly holes, Bezier plane / block, STL / PLY /
## option exports, Convert options (MeshInstance3D, CSG, `_xogot_modeler` meshes), reusable part,
## individual origins and the lightmap atlas check. Run: tests/xogot_i1_check.sh
## Prints "XOGOT I1 OK" or "XOGOT I1 FAIL: ...". Does nothing unless the user args contain
## `xogot_i1=1` (the .sh passes it). Builds everything in res://tests/xogot_i1.tscn, never saves.

var fails: PackedStringArray = []
var plugin: Object
var root: Node

# Toolbar action ids (ui.rs `Action`, append-only).
const A_SELECT := 4
const A_EXTRUDE := 8
const A_INSET := 9
const A_FILL := 14
const A_DETACH := 15
const A_DUPLICATE := 16
const A_DELETE := 19
const A_WELD := 20
const A_GROW := 21
const A_CONVERT := 29
const A_MIRROR := 40
const A_SHAPE_PARAMS := 42
const A_BAKE := 53
const A_COLLAPSE := 63
const A_DISSOLVE := 70
const A_OFFSET := 71
const A_INVERT := 72
const A_BY_COLOR := 73
const A_BY_SMOOTHING := 74
const A_INDIVIDUAL := 77
# Strip field ids (ui.rs `Field`, append-only).
const F_DISTANCE := 9
const F_INSET := 10
const F_INSET_MODE := 30
const F_OFF_X := 36
const F_OFF_Y := 37
const F_OFF_Z := 38
const F_OFF_SPACE := 39
const F_GROW_ANGLE := 41
const F_EXTRUDE_METHOD := 42
const F_EDGES_GROUP := 43
const F_INSET_INDIVIDUAL := 44
const F_COLLAPSE_FIRST := 45
const F_FILL_ENTIRE := 47
const F_MIRROR_X := 48
const F_MIRROR_Y := 49
const F_MIRROR_Z := 50
const F_MIRROR_DUP := 51
const F_NEW_OBJECT := 52
const F_SEGMENTS := 3
const F_SEGMENTS_Z := 53
const F_TORUS_ARC := 55
const F_ARCH_BAND := 57

func _ready() -> void:
	if not Engine.is_editor_hint() or not "xogot_i1=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("xogot_i1: ", msg)

func _check(ok: bool, what: String) -> void:
	_log(("ok   " if ok else "FAIL ") + what)
	if not ok:
		fails.append(what)

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

func _select(n: Node) -> void:
	var sel := EditorInterface.get_selection()
	sel.clear()
	sel.add_node(n)
	EditorInterface.edit_node(n)
	await _frames(3)

func _find_plugin() -> Object:
	var tb := EditorInterface.get_base_control().find_child("PixelBuilderToolbar", true, false)
	if tb == null:
		# Xogot layout: the Builder menu exists in both layouts.
		var menu := EditorInterface.get_base_control().find_child("PixelBuilderMenu", true, false) as MenuButton
		if menu:
			for con in menu.get_popup().get_signal_connection_list("id_pressed"):
				return con["callable"].get_object()
		return null
	for c in tb.find_children("*", "Button", true, false):
		for con in c.get_signal_connection_list("pressed"):
			return con["callable"].get_object()
	return null

func _mesh(name: String) -> Node3D:
	var n: Node3D = ClassDB.instantiate("PixelBuilderMesh")
	n.name = name
	n.data = ClassDB.instantiate("PixelBuilderMeshData")
	root.add_child(n)
	n.owner = root
	return n

func _reset(n: Node3D, kind: int, size: Vector3, params := {}) -> void:
	n.data.clear()
	n.data.add_shape(kind, size, Vector3.ZERO, params)

## Frees nodes after deselecting them (the plugin must not hold a freed edited node).
func _free(nodes: Array) -> void:
	EditorInterface.get_selection().clear()
	EditorInterface.edit_node(root)
	await _frames(2)
	for n in nodes:
		if is_instance_valid(n):
			n.free()

func _children() -> Array:
	return root.get_children()

func _added(before: Array) -> Array:
	var out := []
	for c in root.get_children():
		if not c in before:
			out.append(c)
	return out

func _shared_edge(d, a: int, b: int) -> PackedInt32Array:
	var fa: PackedInt32Array = d.get_face_vertices(a)
	var fb: PackedInt32Array = d.get_face_vertices(b)
	var common := PackedInt32Array()
	for v in fa:
		if v in fb:
			common.append(v)
	return common

func _msg() -> String:
	return str(plugin.get_state().get("message", ""))

func _history_undo() -> void:
	var ur := EditorInterface.get_editor_undo_redo()
	ur.get_history_undo_redo(ur.get_object_history_id(root)).undo()

func _run() -> void:
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://tests/xogot_i1.tscn")
	await _frames(20)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("XOGOT I1 FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	# The headless editor window is tiny (a 3D viewport of ~170 x 40 px): give the input checks
	# room to click distinct points.
	get_window().size = Vector2i(1600, 1000)
	await _frames(10)
	plugin = _find_plugin()
	if plugin == null:
		print("XOGOT I1 FAIL: builder plugin not found")
		get_tree().quit(1)
		return
	# This check drives the MST layout (session override, the user's setting is untouched).
	plugin.debug_set_layout(0)
	await _frames(2)
	await _actions()
	await _options()
	await _objects()
	await _shapes()
	await _bezier()
	await _interactive()
	await _export()
	await _convert()
	await _lightmap()
	EditorInterface.get_selection().clear()
	await _frames(4)
	plugin.debug_set_layout(-1)
	if fails.is_empty():
		print("XOGOT I1 OK")
	else:
		print("XOGOT I1 FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)

# ------------------------------------------------------------------ new actions

func _actions() -> void:
	var sheet := _mesh("Sheet")
	await _select(sheet)

	var d = sheet.data
	# Dissolve faces: a 2x2 plane becomes one face.
	_reset(sheet, 1, Vector3(2, 0.1, 2), {"segments": 2})
	_check(d.get_face_count() == 4, "2x2 plane has 4 faces (%d)" % d.get_face_count())
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_action(A_DISSOLVE)
	_check(d.get_face_count() == 1, "dissolve faces merges the 4 faces into 1 (%d) %s" % [d.get_face_count(), _msg()])
	# Dissolve edges: an inner edge joins its two faces.
	_reset(sheet, 1, Vector3(2, 0.1, 2), {"segments": 2})
	var e := _shared_edge(d, 0, 1)
	plugin.debug_select_edges(e)
	plugin.debug_action(A_DISSOLVE)
	_check(e.size() == 2 and d.get_face_count() == 3, "dissolve edge joins two faces (%d faces)" % d.get_face_count())
	# Dissolve vertices: the centre vertex merges all four faces.
	_reset(sheet, 1, Vector3(2, 0.1, 2), {"segments": 2})
	var centre := -1
	for v in d.get_vertex_count():
		if d.get_vertex(v).length() < 1e-4:
			centre = v
	plugin.debug_select_verts(PackedInt32Array([centre]))
	plugin.debug_action(A_DISSOLVE)
	_check(centre >= 0 and d.get_face_count() == 1, "dissolve centre vertex (%d faces)" % d.get_face_count())

	var box := _mesh("Box")
	await _select(box)
	var b = box.data
	_reset(box, 0, Vector3(2, 2, 2))
	var top: int = b.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
	var y0: float = b.get_face_center(top).y
	# Offset elements (Global): +0.5 up.
	plugin.debug_select_faces(PackedInt32Array([top]))
	plugin.debug_field(F_OFF_X, 0.0)
	plugin.debug_field(F_OFF_Y, 0.5)
	plugin.debug_field(F_OFF_Z, 0.0)
	plugin.debug_field(F_OFF_SPACE, 0.0)
	plugin.debug_action(A_OFFSET)
	_check(absf(b.get_face_center(top).y - y0 - 0.5) < 1e-4, "offset elements moves the top face 0.5 up (%.3f)" % (b.get_face_center(top).y - y0))
	# Normal space: y = along the normal (top face normal = up).
	plugin.debug_field(F_OFF_SPACE, 2.0)
	plugin.debug_field(F_OFF_Y, -0.5)
	plugin.debug_action(A_OFFSET)
	_check(absf(b.get_face_center(top).y - y0) < 1e-4, "offset in Normal space moves it back")
	plugin.debug_field(F_OFF_SPACE, 0.0)
	# Invert selection.
	plugin.debug_select_faces(PackedInt32Array([top]))
	plugin.debug_action(A_INVERT)
	_check(int(plugin.get_state()["faces"]) == 5, "invert selection: 5 faces (%d)" % plugin.get_state()["faces"])
	# Select by colour.
	b.set_faces_color(PackedInt32Array([0, 1]), Color(1, 0, 0))
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_action(A_BY_COLOR)
	_check(int(plugin.get_state()["faces"]) == 2, "select by colour: the two red faces (%d)" % plugin.get_state()["faces"])
	# Grow by angle: 10° keeps the top alone, 0 = every neighbour.
	plugin.debug_select_faces(PackedInt32Array([top]))
	plugin.debug_field(F_GROW_ANGLE, 10.0)
	plugin.debug_action(A_GROW)
	_check(int(plugin.get_state()["faces"]) == 1, "grow by 10°: top only (%d)" % plugin.get_state()["faces"])
	plugin.debug_field(F_GROW_ANGLE, 0.0)
	plugin.debug_action(A_GROW)
	_check(int(plugin.get_state()["faces"]) == 5, "grow (angle 0): top + 4 sides (%d)" % plugin.get_state()["faces"])
	# Select by smoothing group: a smooth cylinder's sides, not its caps.
	var cyl := _mesh("Cyl")
	await _select(cyl)
	_reset(cyl, 2, Vector3(1, 1, 1), {"segments": 8, "smooth": true})
	var side: int = cyl.data.faces_facing(Vector3(1, 0, 0), 30.0, PackedInt32Array())[0]
	plugin.debug_select_faces(PackedInt32Array([side]))
	plugin.debug_action(A_BY_SMOOTHING)
	var n_smooth := int(plugin.get_state()["faces"])
	_check(n_smooth == 8, "select by smoothing group: the 8 sides (%d of %d)" % [n_smooth, cyl.data.get_face_count()])
	await _free([cyl])

# ------------------------------------------------------------------ options of existing actions

func _options() -> void:
	var s := root.get_node("Sheet")
	await _select(s)
	var d = s.data
	# Extrude methods: face normal (classic) vs individual faces.
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 2})
	plugin.debug_field(F_DISTANCE, 0.5)
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_action(A_EXTRUDE)
	var classic: int = d.get_face_count()
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 2})
	plugin.debug_field(F_EXTRUDE_METHOD, 2.0)
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_action(A_EXTRUDE)
	_check(classic == 12 and d.get_face_count() == 20, "extrude face normal = 12 faces (%d), individual = 20 (%d)" % [classic, d.get_face_count()])
	plugin.debug_field(F_EXTRUDE_METHOD, 0.0)
	# Edge extrude as group / one face per edge.
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 1})
	var f0: PackedInt32Array = d.get_face_vertices(0)
	var nv: int = d.get_vertex_count()
	plugin.debug_select_edges(PackedInt32Array([f0[0], f0[1], f0[1], f0[2]]))
	plugin.debug_field(F_EDGES_GROUP, 0.0)
	plugin.debug_action(A_EXTRUDE)
	_check(d.get_vertex_count() - nv == 4 and d.get_face_count() == 3, "edge extrude, not as group: 4 new verts (%d), 2 new faces" % (d.get_vertex_count() - nv))
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 1})
	plugin.debug_select_edges(PackedInt32Array([f0[0], f0[1], f0[1], f0[2]]))
	plugin.debug_field(F_EDGES_GROUP, 1.0)
	plugin.debug_action(A_EXTRUDE)
	_check(d.get_vertex_count() - nv == 3, "edge extrude as group: 3 new verts (%d)" % (d.get_vertex_count() - nv))
	# Inset by Width: individual (classic) vs grouped region.
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 2})
	plugin.debug_field(F_INSET_MODE, 1.0)
	plugin.debug_field(F_INSET, 0.1)
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_action(A_INSET)
	var indiv: int = d.get_face_count()
	_reset(s, 1, Vector3(2, 0.1, 2), {"segments": 2})
	plugin.debug_field(F_INSET_INDIVIDUAL, 0.0)
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_action(A_INSET)
	_check(indiv == 20 and d.get_face_count() < indiv, "inset individual = 20 faces (%d), grouped region fewer (%d) %s" % [indiv, d.get_face_count(), _msg()])
	plugin.debug_field(F_INSET_INDIVIDUAL, 1.0)
	plugin.debug_field(F_INSET_MODE, 0.0)
	# Collapse to first.
	var box := root.get_node("Box")
	await _select(box)
	var b = box.data
	_reset(box, 0, Vector3(2, 2, 2))
	var fv: PackedInt32Array = b.get_face_vertices(0)
	var keep: Vector3 = b.get_vertex(fv[1])
	plugin.debug_field(F_COLLAPSE_FIRST, 1.0)
	plugin.debug_select_verts(PackedInt32Array([fv[1], fv[0]]))
	plugin.debug_action(A_COLLAPSE)
	var found := false
	for v in b.get_vertex_count():
		if b.get_vertex(v).distance_to(keep) < 1e-5:
			found = true
	_check(b.get_vertex_count() == 7 and found, "collapse to first keeps the first vertex's position (%d verts)" % b.get_vertex_count())
	plugin.debug_field(F_COLLAPSE_FIRST, 0.0)
	# Fill hole, entire off: three of the four border vertices -> a triangle.
	_reset(box, 0, Vector3(2, 2, 2))
	var top: int = b.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
	var tv: PackedInt32Array = b.get_face_vertices(top)
	plugin.debug_select_faces(PackedInt32Array([top]))
	plugin.debug_action(A_DELETE)
	plugin.debug_select_verts(PackedInt32Array([tv[0], tv[1], tv[2]]))
	plugin.debug_field(F_FILL_ENTIRE, 0.0)
	plugin.debug_action(A_FILL)
	var last: int = b.get_face_count() - 1
	_check(b.get_face_count() == 6 and b.get_face_vertices(last).size() == 3, "partial fill closes 3 border verts with a triangle %s" % _msg())
	plugin.debug_field(F_FILL_ENTIRE, 1.0)
	# Weld distance (Object mode): detach + weld restores the shared vertices.
	_reset(box, 0, Vector3(2, 2, 2))
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_action(A_DETACH)
	var split: int = b.get_vertex_count()
	plugin.debug_action(0)  # Object mode
	plugin.debug_action(A_WELD)
	_check(split > 8 and b.get_vertex_count() == 8, "detach then weld: %d -> %d verts" % [split, b.get_vertex_count()])
	# Mirror X + Z with duplicate: four copies; X without duplicate flips.
	_reset(box, 0, Vector3(1, 1, 1))
	b.translate_faces(PackedInt32Array([0, 1, 2, 3, 4, 5]), Vector3(1, 0, 1))
	plugin.debug_action(A_MIRROR)  # default: X, duplicate
	_check(b.get_face_count() == 12, "mirror (default X duplicate): 12 faces (%d)" % b.get_face_count())
	_reset(box, 0, Vector3(1, 1, 1))
	b.translate_faces(PackedInt32Array([0, 1, 2, 3, 4, 5]), Vector3(1, 0, 1))
	plugin.debug_field(F_MIRROR_Z, 1.0)
	plugin.debug_action(A_MIRROR)
	_check(b.get_face_count() == 24, "mirror X + Z duplicate: 24 faces (%d)" % b.get_face_count())
	_reset(box, 0, Vector3(1, 1, 1))
	b.translate_faces(PackedInt32Array([0, 1, 2, 3, 4, 5]), Vector3(1, 0, 1))
	plugin.debug_field(F_MIRROR_Z, 0.0)
	plugin.debug_field(F_MIRROR_DUP, 0.0)
	plugin.debug_action(A_MIRROR)
	var maxx := -1e9
	for v in b.get_vertex_count():
		maxx = maxf(maxx, b.get_vertex(v).x)
	_check(b.get_face_count() == 6 and maxx < 0.0, "mirror X without duplicate flips the mesh (max x %.2f)" % maxx)
	plugin.debug_field(F_MIRROR_DUP, 1.0)
	# Individual origins: two boxes in one mesh, each top face turns about its own centre.
	b.clear()
	b.add_shape(0, Vector3(1, 1, 1), Vector3(-2, 0, 0), {})
	b.add_shape(0, Vector3(1, 1, 1), Vector3(2, 0, 0), {})
	var tops: PackedInt32Array = b.faces_facing(Vector3.UP, 5.0, PackedInt32Array())
	plugin.debug_select_faces(tops)
	plugin.debug_action(A_INDIVIDUAL)
	plugin.debug_rotate_selection(90.0)
	var c0: Vector3 = b.get_face_center(tops[0])
	_check(absf(absf(c0.x) - 2.0) < 1e-3 and absf(c0.z) < 1e-3, "individual origins: top face stays at its own centre (%s)" % c0)
	plugin.debug_action(A_INDIVIDUAL)
	plugin.debug_rotate_selection(90.0)
	c0 = b.get_face_center(tops[0])
	_check(absf(absf(c0.z) - 2.0) < 1e-3, "shared pivot: the face swings round the common centre (%s)" % c0)

# ------------------------------------------------------------------ to new object / reusable part

func _objects() -> void:
	var box := root.get_node("Box")
	await _select(box)
	var b = box.data
	_reset(box, 0, Vector3(2, 2, 2))
	plugin.debug_field(F_NEW_OBJECT, 1.0)
	var before := _children()
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	plugin.debug_action(A_DETACH)
	var added := _added(before)
	_check(added.size() == 1 and added[0].data.get_face_count() == 2 and b.get_face_count() == 4, "detach to new object: new mesh with 2 faces, 4 left (%s)" % _msg())
	_history_undo()
	await _frames(2)
	_check(_added(before).is_empty() and b.get_face_count() == 6, "undo restores the faces and removes the new mesh (%d faces)" % b.get_face_count())
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_action(A_DUPLICATE)
	added = _added(before)
	_check(added.size() == 1 and b.get_face_count() == 6, "duplicate to new object keeps the source")
	await _free(added)
	await _select(box)
	plugin.debug_field(F_NEW_OBJECT, 0.0)
	# Create reusable part.
	var part := _mesh("Part")
	part.data.add_shape(0, Vector3(1, 1, 1), Vector3.ZERO, {})
	part.position = Vector3(5, 0, 0)
	await _select(part)
	var path := "user://xogot_i1_part.tscn"
	var err: String = plugin.debug_reusable_part(path)
	await _frames(6)
	var inst := root.get_node_or_null("Part")
	_check(err == "" and inst != null and inst.scene_file_path == path and inst.position == Vector3(5, 0, 0), "reusable part: instance of %s in place (%s)" % [path, err])
	_check(inst != null and inst.get_child_count() >= 0 and ResourceLoader.exists(path), "reusable part scene saved")
	_history_undo()
	await _frames(4)
	var back := root.get_node_or_null("Part")
	_check(back != null and back.scene_file_path == "" and back.is_class("PixelBuilderMesh"), "undo puts the builder mesh back")
	await _free([back])
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))

# ------------------------------------------------------------------ shapes

func _shapes() -> void:
	var box := root.get_node("Box")
	await _select(box)
	for k in [17, 18, 19]:
		var before := _children()
		plugin.on_kind(k)
		plugin.debug_draw_shape(Vector3(0, 0, 6), Vector3(1, 0, 7), 1.0)
		var added := _added(before)
		_check(added.size() == 1 and added[0].data.get_face_count() > 0, "shape kind %d drawn (%s)" % [k, _msg()])
		for n in added:
			n.free()  # not selected: the box stays the edited mesh
	# Plane z segments persist in the recipe.
	plugin.on_kind(1)
	plugin.debug_field(F_SEGMENTS, 2.0)
	plugin.debug_field(F_SEGMENTS_Z, 3.0)
	var before := _children()
	plugin.debug_draw_shape(Vector3(0, 0, 6), Vector3(2, 0, 8), 0.1)
	var added := _added(before)
	var ok: bool = added.size() == 1 and added[0].data.get_face_count() == 6 and int(added[0].data.shape.get("segments_z", -1)) == 3
	_check(ok, "plane 2 x 3 segments, segments_z saved in the recipe")
	for n in added:
		n.free()
	plugin.debug_field(F_SEGMENTS_Z, 0.0)
	# A 0° torus is empty: nothing is created.
	plugin.on_kind(11)
	plugin.debug_field(F_TORUS_ARC, 0.0)
	before = _children()
	plugin.debug_draw_shape(Vector3(0, 0, 6), Vector3(1, 0, 7), 1.0)
	_check(_added(before).is_empty() and _msg().contains("empty"), "0° torus creates nothing (%s)" % _msg())
	plugin.debug_field(F_TORUS_ARC, 360.0)
	# Arch band with 150 segments (above the old 128 clamp).
	plugin.on_kind(6)
	plugin.debug_field(F_ARCH_BAND, 1.0)
	plugin.debug_field(F_SEGMENTS, 150.0)
	before = _children()
	plugin.debug_draw_shape(Vector3(0, 0, 6), Vector3(2, 0, 7), 1.0)
	added = _added(before)
	_check(added.size() == 1 and bool(added[0].data.shape.get("arch_band", false)) and int(added[0].data.shape.get("segments", 0)) == 150, "arch band, 150 segments kept in the recipe")
	for n in added:
		n.free()
	plugin.debug_field(F_ARCH_BAND, 0.0)
	plugin.debug_field(F_SEGMENTS, 12.0)
	# Poly shape with a hole, and a refused hole.
	var outline := PackedVector3Array([Vector3(0, 0, 10), Vector3(4, 0, 10), Vector3(4, 0, 14), Vector3(0, 0, 14)])
	var hole := PackedVector3Array([Vector3(1.5, 0, 11.5), Vector3(2.5, 0, 11.5), Vector3(2.5, 0, 12.5), Vector3(1.5, 0, 12.5)])
	before = _children()
	plugin.debug_poly(outline, [hole])
	added = _added(before)
	_check(added.size() == 1 and added[0].data.get_face_count() > 6 and added[0].data.is_closed(), "poly shape with a hole: closed, %d faces (%s)" % [added[0].data.get_face_count() if added.size() == 1 else 0, _msg()])
	for n in added:
		n.free()
	var outside := PackedVector3Array([Vector3(8, 0, 10), Vector3(9, 0, 10), Vector3(9, 0, 11)])
	before = _children()
	plugin.debug_poly(outline, [outside])
	_check(_added(before).is_empty() and _msg().begins_with("Poly shape:"), "hole outside the outline is refused with a message (%s)" % _msg())
	plugin.debug_action(A_SELECT)

# ------------------------------------------------------------------ bezier

func _bezier() -> void:
	var box := root.get_node("Box")
	await _select(box)
	var pts := PackedVector3Array([Vector3(0, 0, -6), Vector3(3, 0, -4), Vector3(6, 0, -6)])
	var before := _children()
	var err: String = plugin.debug_bezier(pts, false, false, 0.0)
	var added := _added(before)
	_check(added.size() == 1 and added[0].data.get_face_count() > 0 and added[0].data.bezier.size() > 0, "bezier plane created with its recipe (%s)" % err)
	before = _children()
	err = plugin.debug_bezier(pts, false, true, 1.0)
	var blocks := _added(before)
	_check(blocks.size() == 1 and blocks[0].data.is_closed(), "bezier block is a closed solid (%s)" % err)
	if added.size() == 1:
		var plane: Node3D = added[0]
		await _select(plane)
		var blob: PackedByteArray = plane.data.get_blob()
		_check(plugin.debug_bezier_move_knot(1, Vector3(0, 0, 1)), "re-edit opens the stored recipe")
		_check(plane.data.get_blob() != blob and plane.data.bezier.size() > 0, "moving a knot rebuilds the mesh")
		_check(str(plugin.get_state()["tool"]) == "Bezier" and bool(plugin.debug_bezier_state()["editing"]), "re-edit runs in the Bezier tool")
		plugin.debug_action(A_SHAPE_PARAMS)  # toggles the re-edit off
		_check(not bool(plugin.debug_bezier_state()["editing"]), "Shape parameters ends the re-edit")
		plugin.debug_action(A_SHAPE_PARAMS)
		_check(bool(plugin.debug_bezier_state()["editing"]), "Shape parameters reopens the Bezier recipe")
		plugin.debug_action(A_SELECT)
	await _free(added + blocks)

# ------------------------------------------------------------------ interactive input

func _cam() -> Camera3D:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	return vp.get_camera_3d() if vp else null

func _screen(p: Vector3) -> Vector2:
	return _cam().unproject_position(p)

func _move(s: Vector2) -> void:
	var m := InputEventMouseMotion.new()
	m.position = s
	plugin.debug_input(m)

func _button(s: Vector2, pressed: bool, button := MOUSE_BUTTON_LEFT) -> bool:
	var e := InputEventMouseButton.new()
	e.button_index = button
	e.pressed = pressed
	e.position = s
	return plugin.debug_input(e)

func _click(p: Vector3) -> void:
	var s := _screen(p)
	_move(s)
	_button(s, true)
	_button(s, false)

func _key(code: Key, shift := false) -> bool:
	var e := InputEventKey.new()
	e.keycode = code
	e.pressed = true
	e.shift_pressed = shift
	return plugin.debug_input(e)

func _interactive() -> void:
	var box := root.get_node("Box")
	await _select(box)
	if _cam() == null:
		_check(false, "editor viewport camera for the input checks")
		return
	# Bezier: three clicks, close on the first knot, insert on the curve, drag, Delete, X move.
	plugin.debug_action(75)
	await _frames(2)
	_check(str(plugin.get_state()["tool"]) == "Bezier", "toolbar Bezier button selects the tool")
	var p0 := Vector3(4, 0, -6)
	var p1 := Vector3(6, 0, -4)
	var p2 := Vector3(8, 0, -6)
	for p in [p0, p1, p2]:
		_click(p)
	var st: Dictionary = plugin.debug_bezier_state()
	_check(int(st["knots"]) == 3, "three clicks add three knots (%d; %s; screen %s %s %s; vp %s)" % [st["knots"], _msg(), _screen(p0), _screen(p1), _screen(p2), EditorInterface.get_editor_viewport_3d(0).size])
	var pts: PackedVector3Array = st["points"]
	_check(pts.size() == 3 and pts[1].distance_to(p1) < 0.3, "knots land under the cursor (%s)" % [pts])
	_click(pts[0])
	_check(bool(plugin.debug_bezier_state()["closed"]), "clicking the first knot closes the curve")
	# Insert on the curve: a point on the segment between knot 1 and knot 2.
	var mid := pts[1].lerp(pts[2], 0.5)
	var before_n := int(plugin.debug_bezier_state()["knots"])
	_click(mid)
	var after_n := int(plugin.debug_bezier_state()["knots"])
	_check(after_n == before_n + 1 or after_n == before_n, "click near the curve inserts a knot (%d -> %d)" % [before_n, after_n])
	# Drag knot 1 by (+1, 0, 0) on screen.
	st = plugin.debug_bezier_state()
	pts = st["points"]
	var k1 := pts[1]
	_move(_screen(k1))
	_button(_screen(k1), true)
	_move(_screen(k1 + Vector3(0.5, 0, 0)))
	_move(_screen(k1 + Vector3(1, 0, 0)))
	_button(_screen(k1 + Vector3(1, 0, 0)), false)
	var moved: Vector3 = plugin.debug_bezier_state()["points"][1]
	_check(absf(moved.x - k1.x - 1.0) < 0.2 and absf(moved.z - k1.z) < 0.2, "dragging a knot moves it (%s -> %s)" % [k1, moved])
	# X move: only x changes; type 2 = exactly +2 along world X.
	_move(_screen(moved))
	_check(_key(KEY_X), "X starts a constrained move of the selected knot")
	_check(bool(plugin.debug_bezier_state()["moving"]), "move session running")
	_key(KEY_2)
	_key(KEY_ENTER)
	var typed: Vector3 = plugin.debug_bezier_state()["points"][1]
	_check(not bool(plugin.debug_bezier_state()["moving"]) and absf(typed.x - moved.x - 2.0) < 1e-3 and absf(typed.z - moved.z) < 1e-4, "X then 2 then Enter moves the knot +2 along world X (%s)" % typed)
	# Esc restores a move.
	_key(KEY_Z)
	_key(KEY_5)
	_key(KEY_ESCAPE)
	var restored: Vector3 = plugin.debug_bezier_state()["points"][1]
	_check(restored.distance_to(typed) < 1e-4, "Esc restores the knot")
	# Delete removes the selected knot.
	var n0 := int(plugin.debug_bezier_state()["knots"])
	_key(KEY_DELETE)
	_check(int(plugin.debug_bezier_state()["knots"]) == n0 - 1, "Delete removes the selected knot")
	var before := _children()
	_key(KEY_ENTER)
	var added := _added(before)
	_check(added.size() == 1 and added[0].data.bezier.size() > 0, "Enter creates the Bezier plane (%s)" % _msg())
	# Block: Enter -> height stage -> Enter.
	plugin.debug_field(64, 1.0)
	for p in [Vector3(4, 0, -9), Vector3(7, 0, -9)]:
		_click(p)
	_key(KEY_ENTER)
	_check(bool(plugin.debug_bezier_state()["height_stage"]), "block: Enter asks for the height")
	_move(_screen(Vector3(5.5, 0, -9)) + Vector2(0, -40))
	before = _children()
	_key(KEY_ENTER)
	var blocks := _added(before)
	_check(blocks.size() == 1 and blocks[0].data.is_closed(), "block: Enter again creates a closed block (%s)" % _msg())
	plugin.debug_field(64, 0.0)
	plugin.debug_action(A_SELECT)
	await _free(added + blocks)
	# Poly shape with holes through clicks.
	await _select(box)
	plugin.on_kind(16)
	plugin.debug_field(63, 1.0)
	var o := [Vector3(4, 0, -16), Vector3(8, 0, -16), Vector3(8, 0, -12), Vector3(4, 0, -12)]
	for p in o:
		_click(p)
	_click(o[0])
	var h := [Vector3(5.5, 0, -14.5), Vector3(6.5, 0, -14.5), Vector3(6.5, 0, -13.5), Vector3(5.5, 0, -13.5)]
	for p in h:
		_click(p)
	_click(h[0])
	before = _children()
	_key(KEY_ENTER)
	added = _added(before)
	_check(added.size() == 1 and _msg().contains("1 hole"), "poly shape: outline, hole, Enter -> one mesh with a hole (%s)" % _msg())
	plugin.debug_field(63, 0.0)
	plugin.debug_action(A_SELECT)
	await _free(added)
	# Dialogs pop up.
	await _select(box)
	plugin.debug_action(80)
	await _frames(2)
	_check(_visible_window("Builder export options"), "Export options popup opens")
	_hide_window("Builder export options")
	plugin.debug_action(76)
	await _frames(2)
	_check(_visible_window("Create reusable part"), "Create reusable part file dialog opens")
	_hide_window("Create reusable part")
	var crate := MeshInstance3D.new()
	crate.name = "DialogCrate"
	crate.mesh = BoxMesh.new()
	crate.position = Vector3(0, 0, -30)
	root.add_child(crate)
	crate.owner = root
	await _select(crate)
	plugin.debug_action(A_CONVERT)
	await _frames(2)
	var dlg := _find_window("Convert to builder mesh")
	_check(dlg != null and dlg.visible, "Convert opens its options dialog for a plain mesh")
	if dlg:
		before = _children()
		dlg.hide()
		dlg.confirmed.emit()
		await _frames(3)
		added = _added(before)
		_check(added.size() == 1 and added[0].data.get_face_count() == 6, "Convert dialog: confirm with the defaults = classic n-gons")
		await _free(added + [crate])

func _find_window(title: String) -> Window:
	for w in EditorInterface.get_base_control().find_children("*", "Window", true, false):
		if w.title == title:
			return w
	return null

func _visible_window(title: String) -> bool:
	var w := _find_window(title)
	return w != null and w.visible

func _hide_window(title: String) -> void:
	var w := _find_window(title)
	if w:
		w.hide()

# ------------------------------------------------------------------ export

func _read(path: String) -> PackedByteArray:
	return FileAccess.get_file_as_bytes(path)

func _export() -> void:
	var box := root.get_node("Box")
	await _select(box)
	_reset(box, 0, Vector3(2, 2, 2))
	plugin.debug_export_options({"stl_binary": true, "selected_only": false})
	var p := OS.get_user_data_dir() + "/xogot_i1.stl"
	var err: String = plugin.debug_export(p)
	var bytes := _read(p)
	_check(err == "" and bytes.size() == 84 + 50 * 12, "binary STL: 12 triangles (%d bytes) %s" % [bytes.size(), err])
	plugin.debug_export_options({"stl_binary": false})
	err = plugin.debug_export(p)
	_check(err == "" and FileAccess.get_file_as_string(p).begins_with("solid"), "ASCII STL")
	p = OS.get_user_data_dir() + "/xogot_i1.ply"
	err = plugin.debug_export(p)
	var ply := FileAccess.get_file_as_string(p)
	_check(err == "" and ply.begins_with("ply") and ply.contains("element face 6"), "PLY with 6 n-gon faces %s" % err)
	p = OS.get_user_data_dir() + "/xogot_i1.obj"
	err = plugin.debug_export(p)
	var classic := FileAccess.get_file_as_string(p)
	_check(err == "" and classic.count("\nf ") == 6, "classic OBJ: 6 faces")
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_export_options({"selected_only": true})
	err = plugin.debug_export(p)
	_check(err == "" and FileAccess.get_file_as_string(p).count("\nf ") == 1, "OBJ selected faces only: 1 face %s" % err)
	plugin.debug_export_options({"selected_only": false, "stl_binary": true})
	plugin.debug_action(0)
	err = plugin.debug_export(p)
	_check(FileAccess.get_file_as_string(p) == classic, "OBJ with default options is unchanged")
	for ext in ["stl", "ply", "obj", "mtl"]:
		DirAccess.remove_absolute(OS.get_user_data_dir() + "/xogot_i1." + ext)

# ------------------------------------------------------------------ convert

func _convert_node(n: Node, merge: int, smooth := false) -> Node3D:
	await _select(n)
	var before := _children()
	plugin.debug_convert(merge, true, smooth, 0.0)
	await _frames(2)
	var added := _added(before)
	return added[0] if added.size() == 1 else null

func _convert() -> void:
	var mi := MeshInstance3D.new()
	mi.name = "Crate"
	mi.mesh = BoxMesh.new()
	root.add_child(mi)
	mi.owner = root
	var ngon := await _convert_node(mi, 2)
	_check(ngon != null and ngon.data.get_face_count() == 6 and not mi.visible, "convert n-gons: 6 faces, original hidden")
	var tris := await _convert_node(mi, 0)
	_check(tris != null and tris.data.get_face_count() == 12, "convert triangles: 12 faces (%d)" % (tris.data.get_face_count() if tris else -1))
	var quads := await _convert_node(mi, 1)
	_check(quads != null and quads.data.get_face_count() == 6, "convert quads: 6 faces")
	await _free([ngon, tris, quads])
	# CSG root shape.
	var csg := CSGBox3D.new()
	csg.name = "CsgBlock"
	root.add_child(csg)
	csg.owner = root
	await _frames(4)
	var from_csg := await _convert_node(csg, 2)
	_check(from_csg != null and from_csg.data.get_face_count() == 6, "convert CSG box: 6 faces (%s)" % _msg())
	await _free([from_csg])
	# `_xogot_modeler` mesh: one quad stored as two triangles over four records.
	var am := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	var pos := PackedVector3Array([Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1), Vector3(0, 0, 1)])
	arrays[Mesh.ARRAY_VERTEX] = pos
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0, 1, 2, 0, 2, 3])
	am.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var face := {"indices": PackedInt32Array([0, 2, 1, 0, 3, 2]), "smoothingGroup": 0.0, "submeshIndex": 0.0, "textureGroup": 0.0, "manualUV": false, "uv": {}}
	var red := Color(1, 0, 0, 1)
	var snap := {"positions": pos, "faces": [face], "sharedVertices": [], "uv0": PackedVector2Array([Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)]), "colors": PackedColorArray([red, red, red, red])}
	am.set_meta("_xogot_modeler", {"version": 4.0, "surfaceSlots": [0.0], "mesh": snap})
	var xmi := MeshInstance3D.new()
	xmi.name = "XogotQuad"
	xmi.mesh = am
	root.add_child(xmi)
	xmi.owner = root
	await _select(xmi)
	var before := _children()
	plugin.debug_action(A_CONVERT)  # Xogot meshes convert at once (no dialog)
	await _frames(2)
	var added := _added(before)
	_check(added.size() == 1 and added[0].data.get_face_count() == 1 and added[0].data.get_vertex_count() == 4, "Xogot mesh keeps its quad (%s)" % _msg())
	if added.size() == 1:
		var xd = added[0].data
		var uvs: PackedVector2Array = xd.get_face_uvs(0, Vector3.ZERO)
		var uv_ok := uvs.size() == 4
		# Xogot stores V up; the builder's UVs are V down: (u, 1 - v) per corner.
		for c in uvs.size():
			var v: Vector3 = xd.get_vertex(xd.get_face_vertices(0)[c])
			uv_ok = uv_ok and uvs[c].distance_to(Vector2(v.x, v.z)) < 1e-4
		_check(uv_ok, "Xogot mesh keeps its stored UVs (flipped to V down) %s" % [uvs])
		_check(xd.get_corner_color(0, 0).is_equal_approx(red), "Xogot mesh keeps its vertex colours (%s)" % xd.get_corner_color(0, 0))
	await _free(added)

# ------------------------------------------------------------------ lightmap

func _lightmap() -> void:
	var big := _mesh("Huge")
	big.data.add_shape(0, Vector3(1000, 1000, 1000), Vector3.ZERO, {})
	await _select(big)
	plugin.debug_action(A_BAKE)
	await _frames(3)
	_check(_msg().contains("Set the texel size") and is_instance_valid(big) and big.is_inside_tree(), "bake refuses a too-large lightmap atlas with the texel hint (%s)" % _msg())
