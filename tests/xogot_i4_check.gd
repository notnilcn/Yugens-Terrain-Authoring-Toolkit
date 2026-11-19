@tool
extends Node
## Headless editor check of the I4 builder ports (docs/port/06-xogot.md):
## A. the R1 review fixes: Ctrl+Z while the adjust panel is open (real key through the editor's
##    input), a value typed into the panel applies on Done, right-drag on the mesh free-looks
##    (press / release pass through), Convert falls back to triangles when `_xogot_modeler`
##    metadata does not decode, damaged slot numbers are renumbered;
## B. paint in the MST layout: preset slots, palette save / load (JSON and .tres, Xogot label
##    keys), reset, sample, smoothing section (Smooth / Faceted / new group / chips / merge /
##    clear), smoothing tint + normals overlay (rebuilt on change only);
## C. UV extras in the UV editor (spherical, auto settings, island, Ctrl+click stitch,
##    Ctrl+Shift+click copy layout, template PNG);
## D. the inspector "Editable Mesh" counts refresh after edits.
## Run: tests/xogot_i4_check.sh. Prints "XOGOT I4 OK" or "XOGOT I4 FAIL: ...".
## Does nothing unless the user args contain `xogot_i4=1`. Builds everything in
## res://tests/xogot_i4.tscn and never saves it; files go to user://.

var fails: PackedStringArray = []
var plugin: Object
var root: Node

# Toolbar action ids (ui.rs `Action`, append-only).
const A_OBJECT := 0
const A_FACE := 1
const A_EXTRUDE := 8
const A_INSET := 9
const A_CONVERT := 29
const A_UV_EDITOR := 46
const A_SET_COLLIDER := 67
# Strip field ids (ui.rs `Field`, append-only).
const F_DISTANCE := 9
const F_INSET := 10
const F_SMOOTH_TINT := 77
const F_SHOW_NORMALS := 78
const F_NORMAL_LENGTH := 79
const P_ADJUST := 19
const BOX := 0
const PLANE := 1
const SPHERE := 4

func _ready() -> void:
	if not Engine.is_editor_hint() or not "xogot_i4=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("xogot_i4: ", msg)

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
	var menu := EditorInterface.get_base_control().find_child("PixelBuilderMenu", true, false) as MenuButton
	if menu:
		for con in menu.get_popup().get_signal_connection_list("id_pressed"):
			return con["callable"].get_object()
	return null

func _mesh(name: String, pos := Vector3.ZERO) -> Node3D:
	var n: Node3D = ClassDB.instantiate("PixelBuilderMesh")
	n.name = name
	n.data = ClassDB.instantiate("PixelBuilderMeshData")
	n.position = pos
	root.add_child(n)
	n.owner = root
	return n

func _reset(n: Node3D, kind: int, size: Vector3, params := {}) -> PackedInt32Array:
	n.data.clear()
	return n.data.add_shape(kind, size, Vector3.ZERO, params)

func _msg() -> String:
	return str(plugin.get_state().get("message", ""))

func _history() -> UndoRedo:
	var ur := EditorInterface.get_editor_undo_redo()
	return ur.get_history_undo_redo(ur.get_object_history_id(root))

func _top_y(d) -> float:
	var y := -INF
	for v in d.get_vertex_count():
		y = maxf(y, d.get_vertex(v).y)
	return y

func _top_face(d) -> int:
	var best := -1
	var by := -INF
	for f in d.get_face_count():
		if d.get_face_normal(f).y > 0.9 and d.get_face_center(f).y > by:
			by = d.get_face_center(f).y
			best = f
	return best

func _adj() -> Dictionary:
	return plugin.debug_adjust_state()

func _cam() -> Camera3D:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	return vp.get_camera_3d() if vp else null

func _screen(p: Vector3) -> Vector2:
	return _cam().unproject_position(p)

func _children() -> Array:
	return root.get_children()

func _added(before: Array) -> Array:
	var out := []
	for c in root.get_children():
		if not c in before:
			out.append(c)
	return out

func _free(nodes: Array) -> void:
	await _select(root)
	for n in nodes:
		if is_instance_valid(n):
			n.free()
	await _frames(2)

func _run() -> void:
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://tests/xogot_i4.tscn")
	await _frames(20)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("XOGOT I4 FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	get_window().size = Vector2i(1600, 1000)
	await _frames(10)
	plugin = _find_plugin()
	if plugin == null:
		print("XOGOT I4 FAIL: builder plugin not found")
		get_tree().quit(1)
		return
	# This check drives the MST layout (session override, the user's setting is untouched).
	plugin.debug_set_layout(0)
	await _frames(2)
	plugin.debug_pref(P_ADJUST, true)
	await _undo_during_adjust()
	await _typed_value()
	await _right_drag()
	await _convert_fallback()
	await _paint()
	await _uv_extras()
	await _inspector()
	EditorInterface.get_selection().clear()
	await _frames(4)
	plugin.debug_set_layout(-1)
	if fails.is_empty():
		print("XOGOT I4 OK")
	else:
		print("XOGOT I4 FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)

# ------------------------------------------------------------------ A. R1 fixes

func _ctrl_key(code: Key, shift := false) -> InputEventKey:
	var e := InputEventKey.new()
	e.keycode = code
	e.physical_keycode = code
	e.pressed = true
	e.ctrl_pressed = true
	e.shift_pressed = shift
	return e

## R1 #1: Ctrl+Z with the panel open (focus in a panel field) commits the adjusted action first,
## so the undo takes back exactly it, not the step before.
func _undo_during_adjust() -> void:
	var box := _mesh("UndoBox")
	await _select(box)
	var d = box.data
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_action(A_FACE)
	plugin.debug_field(F_INSET, 0.2)
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action(A_INSET)  # the previous step (committed at once)
	var after_inset: PackedByteArray = d.blob
	var hist0 := _history().get_current_action()
	plugin.debug_field(F_DISTANCE, 0.5)
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action_adjust(A_EXTRUDE)
	_check(bool(_adj()["active"]) and d.blob != after_inset, "Extrude opens the adjust panel")
	_check(plugin.debug_adjust_type(F_DISTANCE, "0.5"), "a panel field can take the keyboard focus")
	# The real path: the key goes through the editor window (`_input`, then Godot's shortcuts).
	EditorInterface.get_base_control().get_viewport().push_input(_ctrl_key(KEY_Z))
	await _frames(3)
	_check(not bool(_adj()["active"]), "Ctrl+Z ends the adjust session")
	if d.blob == after_inset:
		_check(true, "Ctrl+Z undid the adjusted extrude (the inset stays)")
	else:
		# Headless Godot may not run its undo shortcut: then the commit must be one undo step.
		_check(_history().get_current_action() == hist0 + 1, "Ctrl+Z committed the adjusted extrude as one step (%d -> %d)" % [hist0, _history().get_current_action()])
		_history().undo()
		_check(d.blob == after_inset, "one undo then restores the inset mesh")
	await _free([box])

## R1 #2: a value typed into the panel (still focused) is applied on Done (no re-entrant signal).
func _typed_value() -> void:
	var box := _mesh("TypedBox")
	await _select(box)
	var d = box.data
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_action(A_FACE)
	var y0 := _top_y(d)
	plugin.debug_field(F_DISTANCE, 0.5)
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action_adjust(A_EXTRUDE)
	_check(plugin.debug_adjust_type(F_DISTANCE, "0.8"), "typed 0.8 into Distance (focused)")
	plugin.debug_adjust_end(false)
	await _frames(2)
	_check(not bool(_adj()["active"]) and not bool(_adj()["visible"]), "Done closes the panel")
	_check(absf(_top_y(d) - y0 - 0.8) < 1e-3, "the typed value is applied on Done (top %.3f)" % (_top_y(d) - y0))
	await _free([box])

## R1 #3: a right press on the mesh passes through (free-look); only a click opens the menu.
func _right_drag() -> void:
	var sheet := _mesh("RightSheet")
	await _select(sheet)
	_reset(sheet, PLANE, Vector3(2, 0, 2))
	plugin.debug_action(A_OBJECT)
	await _frames(2)
	if _cam() == null:
		_check(false, "editor viewport camera")
		return
	var p := _screen(sheet.global_position)
	var drag: Dictionary = plugin.debug_right_drag(p, 40.0)
	_check(not bool(drag["press_consumed"]) and not bool(drag["release_consumed"]), "right press / release on the mesh pass through to Godot (%s)" % [drag])
	_check(not bool(drag["open"]), "a right-drag (free-look) opens no menu")
	var click: Dictionary = plugin.debug_right_drag(p, 0.0)
	_check(bool(click["open"]) and not bool(click["press_consumed"]), "a right click still opens the menu (%s)" % [click])
	var popup: PopupMenu = plugin.debug_context_popup()
	if popup:
		popup.hide()
	await _free([sheet])

## R1 #4 / #6b: undecodable `_xogot_modeler` metadata converts from the triangles; a damaged slot
## number is renumbered instead of making a 60000-slot material array.
func _convert_fallback() -> void:
	var am := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	var pos := PackedVector3Array([Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1), Vector3(0, 0, 1)])
	arrays[Mesh.ARRAY_VERTEX] = pos
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0, 1, 2, 0, 2, 3])
	am.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	am.set_meta("_xogot_modeler", {"version": 4.0, "surfaceSlots": [0.0]})  # no "mesh"
	var mi := MeshInstance3D.new()
	mi.name = "BrokenXogot"
	mi.mesh = am
	root.add_child(mi)
	mi.owner = root
	await _select(mi)
	var before := _children()
	plugin.debug_action(A_CONVERT)
	await _frames(2)
	var added := _added(before)
	if added.is_empty():
		# The fallback goes through the convert dialog's options: confirm with the defaults.
		plugin.debug_convert(2, true, false, 0.0)
		await _frames(2)
		added = _added(before)
	_check(added.size() == 1 and added[0].data.get_face_count() >= 1, "damaged Xogot metadata falls back to the triangle convert (%s)" % _msg())
	_check(_msg().contains("could not be read"), "the fallback is noted (%s)" % _msg())
	await _free(added + [mi])
	# Slot 60000 on the second face.
	var am2 := ArrayMesh.new()
	am2.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var f0 := {"indices": PackedInt32Array([0, 2, 1]), "smoothingGroup": 0.0, "submeshIndex": 0.0, "uv": {}}
	var f1 := {"indices": PackedInt32Array([0, 3, 2]), "smoothingGroup": 0.0, "submeshIndex": 60000.0, "uv": {}}
	var snap := {"positions": pos, "faces": [f0, f1], "sharedVertices": []}
	am2.set_meta("_xogot_modeler", {"version": 4.0, "surfaceSlots": [0.0], "mesh": snap})
	var mi2 := MeshInstance3D.new()
	mi2.name = "HighSlot"
	mi2.mesh = am2
	root.add_child(mi2)
	mi2.owner = root
	await _select(mi2)
	before = _children()
	plugin.debug_action(A_CONVERT)
	await _frames(2)
	added = _added(before)
	var ok := added.size() == 1
	if ok:
		var mats: Array = added[0].data.materials
		ok = mats.size() == 2 and added[0].data.get_face_slot(1) == 1
	_check(ok, "a damaged slot number is renumbered to the used slots (%s)" % _msg())
	await _free(added + [mi2])

# ------------------------------------------------------------------ B. paint / palettes / smoothing

const M_SLOT_CLEAR := 4
const M_GROUP_MERGE := 10
const M_GROUP_CLEAR := 11

func _ps() -> Dictionary:
	return plugin.debug_paint_state()

func _ov() -> Dictionary:
	return plugin.debug_attr_overlay()

func _paint() -> void:
	var box := _mesh("PaintBox")
	await _select(box)
	var d = box.data
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_action(A_OBJECT)
	var n0: int = d.materials.size()
	for i in 5:
		plugin.on_preset_menu(i)
	var mats: Array = d.materials
	var presets_ok: bool = mats.size() == n0 + 5
	for i in range(n0, mats.size()):
		presets_ok = presets_ok and mats[i] != null and mats[i].has_meta("xogot_preset")
	_check(presets_ok, "+ Preset adds the five built-in slots (%d -> %d)" % [n0, mats.size()])
	var checker: StandardMaterial3D = mats[n0 + 1] if mats.size() > n0 + 1 else null
	_check(checker != null and checker.albedo_texture != null and checker.vertex_color_use_as_albedo, "the Checker preset has its pattern texture and vertex colour")
	# Sample (match) slot + smoothing from the selection.
	plugin.debug_action(A_FACE)
	d.set_faces_material(PackedInt32Array([0, 1]), n0 + 1)
	d.set_faces_smooth(PackedInt32Array([0, 1]), 4)
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	plugin.on_paint_sample()
	var ps := _ps()
	_check(int(ps["slot"]) == n0 + 1 and int(ps["smooth_group"]) == 4, "Sample takes the selection's slot and smoothing group (%s)" % [ps])
	_check(int(ps.get("chips", -1)) == 1 and not bool(ps.get("chip_hint", true)), "the palette shows one smoothing chip")
	# Smooth joins the current group; Faceted; New group; Merge; Select; Clear.
	plugin.debug_select_faces(PackedInt32Array([2]))
	plugin.on_xg_smoothing(0, 0)
	_check(d.get_face_smooth(2) == 4, "Smooth puts the face in the current group 4 (%d)" % d.get_face_smooth(2))
	plugin.on_xg_smoothing(1, 0)
	_check(d.get_face_smooth(2) == 0, "Faceted: group 0")
	plugin.debug_select_faces(PackedInt32Array([3]))
	plugin.on_xg_smoothing(2, 0)
	_check(d.get_face_smooth(3) == 1 and int(_ps()["smooth_group"]) == 1, "New group: the next free group 1 becomes current (%s)" % [_ps()])
	_check(int(_ps().get("chips", -1)) == 2, "two chips")
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.on_xg_paint_menu(M_GROUP_MERGE * 1000 + 1)
	_check(d.get_face_smooth(3) == 4 and int(_ps().get("chips", -1)) == 1, "Merge group 1 into the selection's group 4")
	plugin.on_xg_smoothing(3, 4)
	_check(int(plugin.get_state()["faces"]) == 3, "a chip click selects its faces (%d)" % int(plugin.get_state()["faces"]))
	# Overlay: tint + normals, rebuilt on change only.
	plugin.debug_field(F_SMOOTH_TINT, 1)
	var ov := _ov()
	_check(bool(ov["shown"]) and int(ov["tint_triangles"]) == 6, "Tint by group: 3 grouped quads = 6 triangles (%s)" % [ov])
	plugin.debug_field(F_SHOW_NORMALS, 1)
	ov = _ov()
	_check(int(ov["normal_lines"]) == 24, "Show normals: one line per face corner (%s)" % [ov])
	var r0: int = int(ov["rebuilds"])
	for k in 3:
		plugin.debug_redraw()
	await _frames(5)
	_check(int(_ov()["rebuilds"]) == r0, "redraws / idle frames without a change do not rebuild the overlay")
	plugin.debug_field(F_NORMAL_LENGTH, 0.3)
	_check(int(_ov()["rebuilds"]) == r0 + 1, "a new normal length rebuilds once")
	var hist := _history().get_current_action()
	plugin.on_xg_paint_menu(M_GROUP_CLEAR * 1000 + 4)
	_check(d.get_face_smooth(0) == 0 and int(_ov()["tint_triangles"]) == 0, "Clear group: faceted, no tint (%s)" % [_ov()])
	_check(bool(_ps().get("chip_hint", false)) and int(_ps()["smooth_group"]) == 0, "no chips left; the current group is forgotten")
	_check(_history().get_current_action() == hist + 1, "Clear group is one undo step")
	_history().undo()
	await _frames(4)
	_check(d.get_face_smooth(0) == 4 and int(_ov()["tint_triangles"]) == 6, "undo restores the group and the tint follows (%s)" % [_ov()])
	plugin.debug_field(F_SMOOTH_TINT, 0)
	plugin.debug_field(F_SHOW_NORMALS, 0)
	_check(not bool(_ov()["shown"]), "both off: no overlay")
	# Palettes: .tres + JSON in Xogot's shape.
	var tres := "user://xogot_i4_palette.tres"
	var json := "user://xogot_i4_palette.json"
	var sw0: Color = _ps()["swatches"][0]
	var msg: String = plugin.debug_palette_file(tres, true)
	_check(msg.contains("saved"), "save .tres (%s)" % msg)
	var res := ResourceLoader.load(tres, "", ResourceLoader.CACHE_MODE_IGNORE)
	var meta: Dictionary = res.get_meta("_xogot_modeler_palette", {}) if res else {}
	_check(int(meta.get("version", 0)) == 1 and (meta.get("colors", PackedColorArray()) as PackedColorArray).size() == 10 and (meta.get("materials", []) as Array).size() >= 10, "the .tres holds Xogot's palette dictionary (10 colours, >= 10 slots)")
	_check((meta.get("labels", PackedStringArray()) as PackedStringArray).size() == (meta.get("materials", []) as Array).size() and meta.has("colorPresetLabels") and meta.has("emptyColorPresetIndices"), "material labels under `labels`, colour labels under `colorPresetLabels`")
	msg = plugin.debug_palette_file(json, true)
	var text := FileAccess.get_file_as_string(json)
	var parsed = JSON.parse_string(text)
	_check(parsed is Dictionary and (parsed["colors"] as Array).size() == 10 and text.contains("modeler-builtin:"), "save .json: Xogot's JSON with built-in references (%s)" % msg)
	plugin.on_xg_color(Color(0.1, 0.2, 0.3))
	plugin.on_swatch_store(0)
	plugin.on_xg_paint_menu(M_SLOT_CLEAR * 1000 + n0)
	_check(d.materials[n0] == null and not (_ps()["swatches"][0] as Color).is_equal_approx(sw0), "changed swatch 0, cleared the Grid slot")
	msg = plugin.debug_palette_file(tres, false)
	_check((_ps()["swatches"][0] as Color).is_equal_approx(sw0) and d.materials[n0] != null, "load .tres restores the swatch and the slot (%s)" % msg)
	plugin.on_xg_paint_menu(M_SLOT_CLEAR * 1000 + n0)
	msg = plugin.debug_palette_file(json, false)
	_check(d.materials[n0] != null and d.materials[n0].has_meta("xogot_preset"), "load .json regenerates the built-in slot (%s)" % msg)
	plugin.on_xg_color(Color(0.1, 0.2, 0.3))
	plugin.on_swatch_store(1)
	plugin.on_palette_menu(2)
	_check((_ps()["swatches"][1] as Color).is_equal_approx(Color(0.9015, 0.1985, 0.2015)), "Reset Colours: default swatches")
	var bad := FileAccess.open("user://xogot_i4_bad.json", FileAccess.WRITE)
	bad.store_string("{}")
	bad.close()
	msg = plugin.debug_palette_file("user://xogot_i4_bad.json", false)
	_check(msg.contains("not a valid"), "a bad palette is refused (%s)" % msg)
	for f in ["xogot_i4_palette.tres", "xogot_i4_palette.json", "xogot_i4_bad.json"]:
		DirAccess.remove_absolute(OS.get_user_data_dir() + "/" + f)
	# The Xogot Paint popover's Smoothing page has the same display rows.
	plugin.debug_set_layout(1)
	await _frames(3)
	plugin.debug_xogot_open("paint")
	plugin.on_xg_paint_page(2)
	await _frames(2)
	var pop := EditorInterface.get_base_control().find_child("PaintPopover", true, false)
	_check(pop != null and pop.find_child("SmoothingDisplay", true, false) != null, "Xogot Paint › Smoothing shows Tint / Normals rows")
	plugin.debug_xogot_close()
	plugin.debug_set_layout(0)
	await _frames(3)
	await _free([box])

# ------------------------------------------------------------------ C. UV extras

## Press at `at`, optional motion by `jitter` px (a real hand moves a little), release there.
func _click(canvas, at: Vector2, ctrl: bool, shift: bool, jitter := Vector2.ZERO) -> void:
	var e := InputEventMouseButton.new()
	e.button_index = MOUSE_BUTTON_LEFT
	e.position = at
	e.pressed = true
	e.ctrl_pressed = ctrl
	e.shift_pressed = shift
	canvas.debug_input(e)
	if jitter != Vector2.ZERO:
		var m := InputEventMouseMotion.new()
		m.position = at + jitter
		m.relative = jitter
		m.ctrl_pressed = ctrl
		m.shift_pressed = shift
		m.button_mask = MOUSE_BUTTON_MASK_LEFT
		canvas.debug_input(m)
	var u := InputEventMouseButton.new()
	u.button_index = MOUSE_BUTTON_LEFT
	u.position = at + jitter
	u.pressed = false
	u.ctrl_pressed = ctrl
	u.shift_pressed = shift
	canvas.debug_input(u)

func _shift_uvs(d, f: int, node: Node3D, by: Vector2) -> void:
	var moved := PackedVector2Array()
	for uv in d.get_face_uvs(f, node.global_position):
		moved.append(uv + by)
	d.set_face_uvs(f, moved)

func _stitched(d, a: int, b: int, node: Node3D) -> bool:
	var ua := _uv_of(d, a, node)
	var ub := _uv_of(d, b, node)
	var shared := 0
	var ok := true
	for v in ua:
		if ub.has(v):
			shared += 1
			ok = ok and (ua[v] as Vector2).distance_to(ub[v]) < 1e-4
	return shared == 2 and ok

func _uv_of(d, f: int, node: Node3D) -> Dictionary:
	var out := {}
	var uvs: PackedVector2Array = d.get_face_uvs(f, node.global_position)
	var vs: PackedInt32Array = d.get_face_vertices(f)
	for i in vs.size():
		out[vs[i]] = uvs[i]
	return out

func _uv_extras() -> void:
	var sheet := _mesh("UvSheet")
	await _select(sheet)
	var d = sheet.data
	_reset(sheet, PLANE, Vector3(2, 0, 2))
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array())
	var canvas = plugin.debug_uv_editor()
	var nf: int = d.get_face_count()
	_check(canvas != null and nf >= 4, "UV editor open on a grid sheet (%d faces)" % nf)
	if canvas == null:
		return
	# Island: the automatic sheet is one chart.
	canvas.debug_select(PackedInt32Array([0]))
	canvas.debug_act("island")
	_check((canvas.debug_state()["faces"] as PackedInt32Array).size() == nf, "Select UV island follows automatic faces (%d of %d)" % [(canvas.debug_state()["faces"] as PackedInt32Array).size(), nf])
	# Stitch: move face 1's UVs away, Ctrl+click it with face 0 selected.
	_shift_uvs(d, 1, sheet, Vector2(3, 3))
	canvas.refresh()
	canvas.debug_select(PackedInt32Array([0]))
	_click(canvas, canvas.debug_face_px(1), true, false)
	_check(_stitched(d, 0, 1, sheet), "Ctrl+click stitches face 1 onto face 0's shared edge (%s)" % canvas.debug_state()["status"])
	# The same with a 2 px hand jitter and Snap off (the scale preview moves): still a click.
	canvas.set_flag(false, 3)  # Snap off
	_shift_uvs(d, 1, sheet, Vector2(3, 3))
	canvas.refresh()
	var face0: PackedVector2Array = d.get_face_uvs(0, sheet.global_position)
	canvas.debug_select(PackedInt32Array([0]))
	_click(canvas, canvas.debug_face_px(1), true, false, Vector2(2, 1))
	_check(_stitched(d, 0, 1, sheet) and d.get_face_uvs(0, sheet.global_position) == face0, "a jittered Ctrl+click (2 px, Snap off) still stitches and leaves face 0 unscaled (%s)" % canvas.debug_state()["status"])
	canvas.set_flag(true, 3)
	# Copy layout: Ctrl+Shift+click face 3 (1 px along the axis of the selection centre).
	canvas.debug_select(PackedInt32Array([0]))
	_click(canvas, canvas.debug_face_px(3), true, true, Vector2(1, 0))
	_check(d.get_face_uvs(3, sheet.global_position) == d.get_face_uvs(0, sheet.global_position), "Ctrl+Shift+click copies face 0's layout onto face 3 (%s)" % canvas.debug_state()["status"])
	# Spherical projection.
	canvas.debug_select(PackedInt32Array([0, 1, 2, 3]))
	canvas.debug_act("spherical")
	var all_manual := true
	for f in 4:
		all_manual = all_manual and d.get_face_manual_uvs(f).size() == 4
	_check(all_manual, "Spherical: the faces get manual UVs (%s)" % canvas.debug_state()["status"])
	# Auto settings: Fit + upper-left anchor bakes UVs starting at (0, 0) inside 0-1.
	canvas.debug_auto_settings({"fill": 1, "anchor": 1})
	canvas.debug_select(PackedInt32Array([nf - 1]))
	canvas.debug_act("apply_auto")
	var uvs: PackedVector2Array = d.get_face_uvs(nf - 1, sheet.global_position)
	var lo := Vector2(INF, INF)
	var hi := Vector2(-INF, -INF)
	for uv in uvs:
		lo = lo.min(uv)
		hi = hi.max(uv)
	_check(lo.distance_to(Vector2.ZERO) < 1e-4 and hi.x <= 1.0001 and hi.y <= 1.0001 and maxf(hi.x, hi.y) > 0.999, "Auto settings Fit + Upper left: UVs fill 0-1 from the corner (%s..%s)" % [lo, hi])
	# Tile, no anchor, flip U: plain automatic UVs again.
	canvas.debug_auto_settings({"fill": 0, "anchor": 0, "flip_u": true})
	canvas.debug_act("apply_auto")
	_check(not d.is_face_uv_manual(nf - 1), "Auto settings Tile + Flip U stay automatic UVs")
	# Template PNG.
	var png := "user://xogot_i4_uv.png"
	var st: String = canvas.debug_export_template(png, 256, true, true)
	var img := Image.load_from_file(png)
	var clear := 0
	var opaque := 0
	if img:
		for y in range(0, 256, 4):
			for x in range(0, 256, 4):
				var c := img.get_pixel(x, y)
				if c.a < 0.01:
					clear += 1
				elif c.a > 0.5:
					opaque += 1
	_check(img != null and img.get_width() == 256 and clear > 0 and opaque > 0, "UV template PNG: 256 px, transparent background with lines (%s)" % st)
	DirAccess.remove_absolute(OS.get_user_data_dir() + "/xogot_i4_uv.png")
	canvas.get_window().hide()
	await _free([sheet])

# ------------------------------------------------------------------ D. inspector refresh

func _stats() -> Node:
	return EditorInterface.get_inspector().find_child("Stats", true, false)

## Every mesh rebuild re-parses the inspector (`MeshInstance3D.set_mesh` notifies a property-list
## change), so the section is rebuilt with fresh counts; the stats' own deferred `changed` hook
## is the fallback for a data change without a rebuild. Checked: the section is current after
## an adjust preview, its cancel, a script edit, an undoable edit and Set Collider, the hook is
## wired, and nothing refreshes while idle.
func _current(d, what: String) -> void:
	await _frames(3)
	var st := _stats()
	var counts := str(st.debug_state()["counts"]) if st else "no stats"
	_check(counts.contains("%d faces" % d.get_face_count()), "inspector counts current after %s: %s" % [what, counts])

func _inspector() -> void:
	var box := _mesh("InspectBox")
	_reset(box, BOX, Vector3(2, 2, 2))
	await _select(box)
	plugin.debug_action(A_FACE)
	await _frames(6)
	var stats := _stats()
	_check(stats != null and box.data.is_connected("changed", Callable(stats, "refresh")), "the Editable Mesh stats follow the data's changed signal")
	if stats == null:
		await _free([box])
		return
	var d = box.data
	await _current(d, "selection")
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action_adjust(A_EXTRUDE)
	await _current(d, "an adjust preview")
	plugin.debug_adjust_end(true)
	await _current(d, "its cancel")
	d.subdivide(PackedInt32Array([0]))
	await _current(d, "a script edit")
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action(A_EXTRUDE)
	await _current(d, "an undoable edit")
	_history().undo()
	await _current(d, "undo")
	plugin.debug_action(A_OBJECT)
	plugin.debug_action(A_SET_COLLIDER)
	await _frames(3)
	_check(_stats() != null and str(_stats().debug_state()["collision"]).contains("Collider"), "after Set Collider: %s" % (_stats().debug_state()["collision"] if _stats() else "no stats"))
	plugin.debug_action(A_SET_COLLIDER)
	await _frames(3)
	stats = _stats()
	var r: int = int(stats.debug_state()["refreshes"]) if stats else -1
	await _frames(10)
	_check(stats != null and is_instance_valid(stats) and _stats() == stats and int(stats.debug_state()["refreshes"]) == r, "no refresh / re-parse while nothing changes")
	await _free([box])
