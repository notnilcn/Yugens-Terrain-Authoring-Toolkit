@tool
extends Node
## Headless editor check of the Xogot Modeler UI ports in the level builder (I2): the live adjust
## panel (re-run, debounce, Done = one undo step, Cancel, commit on mode change, external change),
## the Xogot bevel (edge rims inset their corners, vertex bevel, options, coarse preview), the
## right-click context menu (entries / reasons per mode, opening on a selected element only), H
## mode cycle, Shift (add / subtract / toggle) and box (intersect / complete) selection modes, the
## rebindable editor shortcuts, edge / vertex hover and the viewport feedback (scene info,
## dimensions). Run: tests/xogot_i2_check.sh. Prints "XOGOT I2 OK" or "XOGOT I2 FAIL: ...".
## Does nothing unless the user args contain `xogot_i2=1`. Builds everything in
## res://tests/xogot_i2.tscn and never saves it.

var fails: PackedStringArray = []
var plugin: Object
var root: Node

# Toolbar action ids (ui.rs `Action`, append-only).
const A_OBJECT := 0
const A_FACE := 1
const A_EDGE := 2
const A_VERTEX := 3
const A_SELECT := 4
const A_EXTRUDE := 8
const A_INSET := 9
const A_BEVEL := 10
const A_MERGE := 12
const A_WELD := 20
const A_BRIDGE := 36
const A_MIRROR := 40
const A_SUBDIV_EDGES := 54
const A_MERGE_OBJECTS := 59
const A_COLLAPSE := 63
const A_XRAY := 69
const A_OFFSET := 71
# Strip field ids (ui.rs `Field`, append-only).
const F_DISTANCE := 9
const F_INSET := 10
const F_BEVEL := 11
const F_CUTS := 12
const F_BEVEL_SEGS := 13
const F_EXTRUDE_METHOD := 42
const F_BEVEL_TYPE := 73
const F_BEVEL_PROFILE := 74
const F_BEVEL_CLAMP := 75
const F_BEVEL_HARDEN := 76
# Preference keys (prefs.rs `Key`).
const P_SHIFT_MODE := 12
const P_RECT_MODE := 13
const P_HOVER := 14
const P_SCENE_INFO := 17
const P_DIMENSIONS := 18
const P_ADJUST := 19
# Shape kinds.
const BOX := 0
const PLANE := 1

func _ready() -> void:
	if not Engine.is_editor_hint() or not "xogot_i2=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("xogot_i2: ", msg)

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

func _history_undo() -> void:
	_history().undo()

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

func _key(code: Key, shift := false, ctrl := false, alt := false) -> bool:
	var e := InputEventKey.new()
	e.keycode = code
	e.pressed = true
	e.shift_pressed = shift
	e.ctrl_pressed = ctrl
	e.alt_pressed = alt
	return plugin.debug_input(e)

func _run() -> void:
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://tests/xogot_i2.tscn")
	await _frames(20)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("XOGOT I2 FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	# The headless editor window is tiny: give the viewport room for distinct clicks.
	get_window().size = Vector2i(1600, 1000)
	await _frames(10)
	plugin = _find_plugin()
	if plugin == null:
		print("XOGOT I2 FAIL: builder plugin not found")
		get_tree().quit(1)
		return
	# This check drives the MST layout (session override, the user's setting is untouched).
	plugin.debug_set_layout(0)
	await _frames(2)
	# Known preferences for the run (restored at the end).
	plugin.debug_pref(P_ADJUST, true)
	# Explicit Add / Intersect for the run (the MST layout default is Toggle / Complete).
	plugin.debug_pref(P_SHIFT_MODE, 0)
	plugin.debug_pref(P_RECT_MODE, 0)
	plugin.debug_pref(P_HOVER, true)
	await _shortcuts()
	await _mode_cycle()
	await _adjust()
	await _bevel()
	await _context_menu()
	await _selection_modes()
	await _hover_feedback()
	plugin.debug_pref(P_SCENE_INFO, true)
	plugin.debug_pref(P_DIMENSIONS, false)
	# Back to "Layout default" (the last choice) so the run leaves no explicit choice behind.
	plugin.debug_pref(P_SHIFT_MODE, 3)
	plugin.debug_pref(P_RECT_MODE, 2)
	EditorInterface.get_selection().clear()
	await _frames(4)
	plugin.debug_set_layout(-1)
	if fails.is_empty():
		print("XOGOT I2 OK")
	else:
		print("XOGOT I2 FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)

# ------------------------------------------------------------------ shortcuts

func _shortcuts() -> void:
	var box := _mesh("ShortcutBox")
	await _select(box)
	var d = box.data
	_reset(box, BOX, Vector3(2, 2, 2))
	var sc: Dictionary = plugin.debug_shortcuts()
	var expect := {"extrude": "E", "bevel": "Ctrl+B", "cycle_mode": "H", "tool_bezier": "Shift+C", "mode_face": "2", "select_all": "Ctrl+A", "grid_halve": "BracketLeft"}
	var all_reg := true
	for id in sc:
		if not bool(sc[id][2]) or not bool(sc[id][3]):
			all_reg = false
	_check(sc.size() >= 37 and all_reg, "every builder shortcut is registered under mst_builder/ (%d)" % sc.size())
	for id in expect:
		var txt := str(sc.get(id, ["", ""])[1])
		_check(txt == expect[id] or txt.replace("Command", "Ctrl") == expect[id], "default key of %s is %s (%s)" % [id, expect[id], txt])
	var tip: Array = plugin.debug_tooltip(A_EXTRUDE)
	_check(tip.size() == 2 and str(tip[0]).begins_with("Extrude [E]") and str(tip[1]) == "Extrude [E]", "Extrude tooltip / hover label show the bound key (%s)" % [tip])
	# Rebind Extrude to Y: Y extrudes, E no longer does.
	var es := EditorInterface.get_editor_settings()
	var shortcut: Shortcut = es.get_shortcut("mst_builder/extrude")
	var old_events := shortcut.events.duplicate()
	var y := InputEventKey.new()
	y.keycode = KEY_Y
	shortcut.events = [y]
	es.emit_signal("settings_changed")
	await _frames(2)
	tip = plugin.debug_tooltip(A_EXTRUDE)
	_check(str(tip[1]) == "Extrude [Y]", "tooltip follows the rebinding (%s)" % [tip])
	var top := _top_face(d)
	plugin.debug_select_faces(PackedInt32Array([top]))
	var n0: int = d.get_face_count()
	_check(not _key(KEY_E), "E is no longer the Extrude key after the rebinding")
	_check(d.get_face_count() == n0, "E did not extrude (%d)" % d.get_face_count())
	var hist0 := _history().get_current_action()
	_check(_key(KEY_Y), "Y (the new binding) is consumed")
	_check(d.get_face_count() == n0 + 4, "Y extrudes the face (%d -> %d)" % [n0, d.get_face_count()])
	_check(bool(_adj()["active"]), "the key-run extrude opens the adjust panel")
	_check(_key(KEY_ENTER), "Enter is consumed by the adjust panel")
	_check(not bool(_adj()["active"]) and _history().get_current_action() == hist0 + 1, "Enter commits one undo step (%d -> %d)" % [hist0, _history().get_current_action()])
	_history_undo()
	_check(d.get_face_count() == n0, "one undo takes the whole extrude back (%d)" % d.get_face_count())
	shortcut.events = old_events
	es.emit_signal("settings_changed")
	await _frames(2)
	_check(str(plugin.debug_tooltip(A_EXTRUDE)[1]) == "Extrude [E]", "restored binding shows E again")
	box.free.call_deferred()
	await _select(root)
	await _frames(2)

# ------------------------------------------------------------------ H cycle

func _mode_cycle() -> void:
	var box := _mesh("CycleBox")
	await _select(box)
	_reset(box, BOX, Vector3(1, 1, 1))
	var order := []
	for i in 4:
		_check(_key(KEY_H), "H is consumed (%d)" % i)
		order.append(str(plugin.get_state()["mode"]))
	_check(order == ["Vertex", "Edge", "Face", "Object"], "H cycles Object > Vertex > Edge > Face > Object (%s)" % [order])
	_key(KEY_3)
	_check(str(plugin.get_state()["mode"]) == "Edge", "3 still selects Edge mode")
	_key(KEY_1)
	_check(str(plugin.get_state()["mode"]) == "Object", "1 still selects Object mode")
	await _select(root)
	box.free()
	await _frames(2)

# ------------------------------------------------------------------ adjust panel

func _adjust() -> void:
	var box := _mesh("AdjustBox")
	await _select(box)
	var d = box.data
	_reset(box, BOX, Vector3(2, 2, 2))
	var before_blob: PackedByteArray = d.blob
	var top := _top_face(d)
	var y0 := _top_y(d)
	plugin.debug_field(F_DISTANCE, 0.5)
	plugin.debug_field(F_EXTRUDE_METHOD, 0)
	plugin.debug_select_faces(PackedInt32Array([top]))
	var hist0 := _history().get_current_action()
	plugin.debug_action_adjust(A_EXTRUDE)
	var st := _adj()
	_check(bool(st["active"]) and bool(st["visible"]), "Extrude opens the adjust panel (%s)" % [st])
	_check(PackedInt32Array(st.get("fields", [])) == PackedInt32Array([F_DISTANCE, F_EXTRUDE_METHOD]), "adjust panel shows the extrude options (%s)" % [st.get("fields")])
	_check(int(st.get("controls", 0)) == 2, "the panel holds one control per option")
	_check(_history().get_current_action() == hist0, "no undo step while adjusting")
	_check(absf(_top_y(d) - y0 - 0.5) < 1e-3, "extruded by 0.5 (%.3f)" % _top_y(d))
	# Debounced re-run through the one-shot timer.
	plugin.debug_adjust_field(F_DISTANCE, 1.0, false)
	_check(bool(_adj()["pending"]) and absf(_top_y(d) - y0 - 0.5) < 1e-3, "a changed option waits for the debounce")
	await get_tree().create_timer(0.35).timeout
	_check(not bool(_adj()["pending"]) and absf(_top_y(d) - y0 - 1.0) < 1e-3, "the timer re-runs from the pre-action mesh (top %.3f)" % _top_y(d))
	_check(d.get_face_count() == 10, "re-run replaces the preview, it does not stack (%d faces)" % d.get_face_count())
	# Idle: the one-shot debounce timer is stopped again and nothing of the plugin runs per frame
	# while the panel stays open.
	var timer: Timer = plugin.get_node_or_null("PixelBuilderAdjustTimer")
	_check(timer != null and timer.is_stopped() and timer.one_shot, "debounce timer is one-shot and stopped after the re-run")
	_check(not plugin.is_processing() and not plugin.is_physics_processing(), "no per-frame processing while the panel is open")
	# Commit inside the debounce window renders the latest value first.
	plugin.debug_adjust_field(F_DISTANCE, 0.75, false)
	plugin.debug_adjust_end(false)
	_check(not bool(_adj()["active"]) and not bool(_adj()["visible"]), "Done closes the panel")
	_check(absf(_top_y(d) - y0 - 0.75) < 1e-3, "Done inside the debounce applies the latest value (%.3f)" % _top_y(d))
	_check(_history().get_current_action() == hist0 + 1, "Done adds exactly one undo step (%d -> %d)" % [hist0, _history().get_current_action()])
	_history_undo()
	_check(d.blob == before_blob, "one undo restores the mesh before the action")
	# Cancel restores and records nothing.
	plugin.debug_field(F_INSET, 0.3)
	plugin.debug_select_faces(PackedInt32Array([top]))
	hist0 = _history().get_current_action()
	plugin.debug_action_adjust(A_INSET)
	_check(bool(_adj()["active"]) and d.get_face_count() == 10, "Inset opens the panel (%d faces)" % d.get_face_count())
	plugin.debug_adjust_end(true)
	_check(not bool(_adj()["active"]) and d.blob == before_blob and _history().get_current_action() == hist0, "Cancel restores the mesh without an undo step")
	_check(int(plugin.get_state()["faces"]) == 1, "Cancel restores the selection")
	# Esc cancels too.
	plugin.debug_action_adjust(A_INSET)
	_check(_key(KEY_ESCAPE) and d.blob == before_blob and not bool(_adj()["active"]), "Esc cancels the adjustment")
	# Leaving the mode commits.
	plugin.debug_action_adjust(A_EXTRUDE)
	_check(bool(_adj()["active"]), "extrude again")
	plugin.debug_action(A_EDGE)
	_check(not bool(_adj()["active"]) and d.get_face_count() == 10 and _history().get_current_action() == hist0 + 1, "a mode change commits the adjustment as one step")
	_history_undo()
	_check(d.blob == before_blob, "and that step undoes")
	# Subdivide edges (Cuts) re-runs.
	plugin.debug_select_edges(_first_edge(d))
	plugin.debug_field(F_CUTS, 1)
	plugin.debug_action_adjust(A_SUBDIV_EDGES)
	var v1: int = d.get_vertex_count()
	plugin.debug_adjust_field(F_CUTS, 3, true)
	_check(bool(_adj()["active"]) and d.get_vertex_count() == v1 + 2, "Subdivide edges: Cuts 1 -> 3 adds two vertices (%d -> %d)" % [v1, d.get_vertex_count()])
	plugin.debug_adjust_end(false)
	_history_undo()
	# A script changing the mesh ends the session without touching it.
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action_adjust(A_EXTRUDE)
	_reset(box, BOX, Vector3(1, 1, 1))
	var ext_blob: PackedByteArray = d.blob
	plugin.debug_adjust_field(F_DISTANCE, 2.0, true)
	_check(not bool(_adj()["active"]) and d.blob == ext_blob, "an outside change ends the adjustment and is kept")
	# Preference off: actions commit at once.
	plugin.debug_pref(P_ADJUST, false)
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	hist0 = _history().get_current_action()
	plugin.debug_action_adjust(A_EXTRUDE)
	_check(not bool(_adj()["active"]) and _history().get_current_action() == hist0 + 1, "Adjust panel off: Extrude commits at once")
	plugin.debug_pref(P_ADJUST, true)
	# Weld (Object mode) and Mirror are adjustable too.
	plugin.debug_action(A_OBJECT)
	plugin.debug_action_adjust(A_MIRROR)
	_check(bool(_adj()["active"]) and PackedInt32Array(_adj()["fields"]).size() == 4, "Mirror opens the panel with its axes")
	plugin.debug_adjust_end(true)
	await _select(root)
	box.free()
	await _frames(2)

func _first_edge(d) -> PackedInt32Array:
	var fv: PackedInt32Array = d.get_face_vertices(0)
	return PackedInt32Array([fv[0], fv[1]])

# ------------------------------------------------------------------ bevel

func _rim(d, face: int) -> PackedInt32Array:
	return d.get_face_edges(PackedInt32Array([face]))

func _bevel() -> void:
	var box := _mesh("BevelBox")
	await _select(box)
	var d = box.data
	plugin.debug_field(F_BEVEL, 0.2)
	plugin.debug_field(F_BEVEL_SEGS, 1)
	plugin.debug_field(F_BEVEL_TYPE, 0)
	plugin.debug_field(F_BEVEL_PROFILE, 0.5)
	plugin.debug_field(F_BEVEL_CLAMP, 1)
	plugin.debug_field(F_BEVEL_HARDEN, 0)
	# Edge rim of the top face: a connected loop. The old bevel never inset its corners.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_edges(_rim(d, _top_face(d)))
	plugin.debug_action(A_BEVEL)
	var top := _top_face(d)
	var inset := true
	for v in d.get_face_vertices(top):
		var p: Vector3 = d.get_vertex(v)
		inset = inset and absf(p.x) < 0.95 and absf(p.z) < 0.95
	_check(d.is_closed() and d.get_face_count() > 6, "bevel of the top rim keeps the box closed (%d faces) %s" % [d.get_face_count(), _msg()])
	_check(top >= 0 and inset, "the rim bevel insets the top face's corners")
	_check(_msg().begins_with("Bevel · 1 segments"), "bevel reports its summary (%s)" % _msg())
	var chamfer: int = d.get_face_count()
	# Segments + profile.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_edges(_rim(d, _top_face(d)))
	plugin.debug_field(F_BEVEL_SEGS, 4)
	plugin.debug_field(F_BEVEL_PROFILE, 0.8)
	plugin.debug_action(A_BEVEL)
	_check(d.is_closed() and d.get_face_count() > chamfer, "4 segments make a rounder bevel (%d > %d faces)" % [d.get_face_count(), chamfer])
	_check(_msg().contains("4 segments") and _msg().contains("Profile 0.80"), "summary shows segments and profile (%s)" % _msg())
	# Percent amount type.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_edges(_first_edge(d))
	plugin.debug_field(F_BEVEL_SEGS, 1)
	plugin.debug_field(F_BEVEL_TYPE, 3)
	plugin.debug_field(F_BEVEL, 25)
	plugin.debug_action(A_BEVEL)
	_check(d.is_closed() and _msg().contains("Percent 25"), "Percent amount bevels (%s)" % _msg())
	plugin.debug_field(F_BEVEL_TYPE, 0)
	plugin.debug_field(F_BEVEL, 0.2)
	# Vertex bevel: one corner -> a triangle cap.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_verts(PackedInt32Array([0]))
	plugin.debug_action(A_BEVEL)
	_check(d.is_closed() and d.get_face_count() == 7 and d.get_vertex_count() == 10, "vertex bevel chamfers the corner (%d faces, %d verts) %s" % [d.get_face_count(), d.get_vertex_count(), _msg()])
	_check(str(plugin.get_state()["mode"]) == "Vertex", "vertex bevel stays in Vertex mode")
	# Bevel through the adjust panel: options + status.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_edges(_rim(d, _top_face(d)))
	plugin.debug_action_adjust(A_BEVEL)
	var st := _adj()
	_check(bool(st["active"]) and PackedInt32Array(st["fields"]) == PackedInt32Array([F_BEVEL, F_BEVEL_TYPE, F_BEVEL_SEGS, F_BEVEL_PROFILE, F_BEVEL_CLAMP, F_BEVEL_HARDEN]), "Bevel opens the panel with Amount / type / Segs / Profile / Clamp / Harden (%s)" % [st.get("fields")])
	_check(str(st.get("status_text", "")).begins_with("Bevel ·"), "panel shows the bevel summary (%s)" % st.get("status_text", ""))
	var f1: int = d.get_face_count()
	plugin.debug_adjust_field(F_BEVEL_SEGS, 3, true)
	_check(d.get_face_count() > f1 and str(_adj()["status_text"]).contains("3 segments"), "changing Segs re-bevels (%d -> %d)" % [f1, d.get_face_count()])
	plugin.debug_adjust_field(F_BEVEL, 5.0, true)
	_check(bool(_adj()["active"]) and d.is_closed(), "a huge amount is clamped or refused but the session stays (%s)" % _adj()["status_text"])
	plugin.debug_adjust_end(true)
	plugin.debug_field(F_BEVEL, 0.2)
	plugin.debug_field(F_BEVEL_SEGS, 1)
	plugin.debug_field(F_BEVEL_PROFILE, 0.5)
	# Ctrl+B works in Vertex mode.
	_reset(box, BOX, Vector3(2, 2, 2))
	plugin.debug_select_verts(PackedInt32Array([0, 1]))
	_check(_key(KEY_B, false, true), "Ctrl+B is consumed in Vertex mode")
	_check(d.get_face_count() == 8, "Ctrl+B bevels the two corners (%d faces)" % d.get_face_count())
	_key(KEY_ENTER)
	await _select(root)
	box.free()
	await _frames(2)

# ------------------------------------------------------------------ context menu

func _menu() -> Dictionary:
	var by_id := {}
	for e in plugin.debug_context_menu():
		by_id[int(e["id"])] = e
	return by_id

func _context_menu() -> void:
	var sheet := _mesh("MenuSheet")
	await _select(sheet)
	var d = sheet.data
	_reset(sheet, PLANE, Vector3(2, 0.1, 2), {"segments": 2})
	plugin.debug_select_faces(PackedInt32Array([0]))
	var m := _menu()
	var sections := {}
	for e in plugin.debug_context_menu():
		sections[str(e["section"])] = true
	_check(sections.has("Select") and sections.has("Build") and sections.has("Separate") and sections.has("Join") and sections.has("Normals") and sections.has("Move"), "Face mode menu has the Xogot sections (%s)" % [sections.keys()])
	_check(m.has(A_EXTRUDE) and bool(m[A_EXTRUDE]["enabled"]), "Face mode: Extrude enabled")
	_check(not m.has(A_BEVEL), "Face mode: no Bevel entry")
	_check(m.has(A_MERGE) and not bool(m[A_MERGE]["enabled"]) and str(m[A_MERGE]["reason"]) == "Select at least two faces.", "Face mode: Merge disabled with its reason (%s)" % [m.get(A_MERGE)])
	_check(m.has(46) and str(m[46]["title"]) == "UV Editor…", "UV Editor… entry")
	_check(m.has(A_EXTRUDE) and str(m[A_EXTRUDE]["shortcut"]) == "E", "entries show their shortcut")
	plugin.debug_select_edges(_first_edge(d))
	m = _menu()
	_check(m.has(A_BEVEL) and bool(m[A_BEVEL]["enabled"]) and str(m[A_BEVEL]["title"]) == "Bevel edges", "Edge mode: Bevel edges enabled")
	_check(m.has(A_BRIDGE) and str(m[A_BRIDGE]["reason"]) == "Select two edges.", "Edge mode: Bridge needs two edges")
	plugin.debug_select_verts(PackedInt32Array([0]))
	m = _menu()
	_check(m.has(A_COLLAPSE) and str(m[A_COLLAPSE]["reason"]) == "Select at least two vertices.", "Vertex mode: Collapse needs two vertices (%s)" % [m.get(A_COLLAPSE)])
	_check(m.has(A_BEVEL) and str(m[A_BEVEL]["title"]) == "Bevel vertices", "Vertex mode: Bevel vertices")
	plugin.debug_action(A_OBJECT)
	m = _menu()
	_check(m.has(A_MIRROR) and bool(m[A_MIRROR]["enabled"]), "Object mode: Mirror enabled")
	_check(m.has(A_MERGE_OBJECTS) and str(m[A_MERGE_OBJECTS]["reason"]) == "Select at least two objects.", "Object mode: Merge objects needs two objects")
	_check(not m.has(A_EXTRUDE), "Object mode: no Extrude")
	# Opening: only on a selected element.
	if _cam() == null:
		_check(false, "editor viewport camera")
		return
	plugin.debug_select_faces(PackedInt32Array([0]))
	var c0: Vector3 = sheet.global_transform * d.get_face_center(0)
	var other := 3 if d.get_face_center(3).distance_to(d.get_face_center(0)) > 0.5 else 1
	var c1: Vector3 = sheet.global_transform * d.get_face_center(other)
	_check(plugin.debug_right_click(_screen(c0)), "right-click on the selected face opens the menu")
	var popup: PopupMenu = plugin.debug_context_popup()
	_check(popup != null and popup.get_item_count() >= 6, "the popup has the sections as submenus (%d items)" % (popup.get_item_count() if popup else -1))
	if popup:
		popup.hide()
	_check(not plugin.debug_right_click(_screen(c1)), "right-click on an unselected face is left to Godot")
	_check(not plugin.debug_right_click(Vector2(3, 3)), "right-click on empty space is left to Godot")
	# Activating an entry runs it (and opens the adjust panel for Extrude).
	plugin.debug_right_click(_screen(c0))
	var n0: int = d.get_face_count()
	plugin.on_context_menu_id(A_EXTRUDE)
	_check(d.get_face_count() > n0 and bool(_adj()["active"]), "menu Extrude runs and opens the adjust panel")
	plugin.debug_adjust_end(false)
	if popup:
		popup.hide()
	await _select(root)
	sheet.free()
	await _frames(2)

# ------------------------------------------------------------------ shift / box modes

func _selection_modes() -> void:
	var sheet := _mesh("SelSheet")
	await _select(sheet)
	var d = sheet.data
	_reset(sheet, PLANE, Vector3(2, 0.1, 2), {"segments": 2})
	if _cam() == null:
		_check(false, "editor viewport camera")
		return
	plugin.debug_select_faces(PackedInt32Array())
	var top: PackedInt32Array = d.faces_facing(Vector3.UP, 5.0, PackedInt32Array())
	var f0: int = top[0]
	var f1: int = top[1]
	var s0 := _screen(sheet.global_transform * d.get_face_center(f0))
	var s1 := _screen(sheet.global_transform * d.get_face_center(f1))
	# Add (set explicitly at the start of the run).
	plugin.debug_pick(s0, false)
	plugin.debug_pick(s1, true)
	plugin.debug_pick(s0, true)
	_check(int(plugin.get_state()["faces"]) == 2, "Shift Add: Shift+click on a selected face keeps it (%d)" % plugin.get_state()["faces"])
	# Subtract.
	plugin.debug_pref(P_SHIFT_MODE, 1)
	plugin.debug_pick(s0, true)
	_check(int(plugin.get_state()["faces"]) == 1, "Shift Subtract removes the face (%d)" % plugin.get_state()["faces"])
	# Toggle.
	plugin.debug_pref(P_SHIFT_MODE, 2)
	plugin.debug_pick(s0, true)
	plugin.debug_pick(s1, true)
	_check(int(plugin.get_state()["faces"]) == 1, "Shift Toggle adds one and removes the other (%d)" % plugin.get_state()["faces"])
	plugin.debug_pref(P_SHIFT_MODE, 0)
	# Box: Intersect vs Complete over one quarter of the sheet plus a sliver of the others.
	var xf: Transform3D = sheet.global_transform
	var pts := [Vector3(-1.05, 0.05, -1.05), Vector3(0.15, 0.05, -1.05), Vector3(0.15, 0.05, 0.15), Vector3(-1.05, 0.05, 0.15)]
	var lo := Vector2(INF, INF)
	var hi := Vector2(-INF, -INF)
	for p in pts:
		var s := _screen(xf * p)
		lo = Vector2(minf(lo.x, s.x), minf(lo.y, s.y))
		hi = Vector2(maxf(hi.x, s.x), maxf(hi.y, s.y))
	plugin.debug_marquee(lo, hi, false)
	var inter := int(plugin.get_state()["faces"])
	plugin.debug_pref(P_RECT_MODE, 1)
	plugin.debug_marquee(lo, hi, false)
	var comp := int(plugin.get_state()["faces"])
	_check(inter == 4, "box Intersect takes every face it touches (%d)" % inter)
	_check(comp == 1, "box Complete takes only the face entirely inside (%d)" % comp)
	# Shift+box Subtract.
	plugin.debug_pref(P_RECT_MODE, 0)
	plugin.debug_select_faces(PackedInt32Array([0, 1, 2, 3]))
	plugin.debug_pref(P_SHIFT_MODE, 1)
	plugin.debug_pref(P_RECT_MODE, 1)
	plugin.debug_marquee(lo, hi, true)
	_check(int(plugin.get_state()["faces"]) == 3, "Shift+box Subtract (Complete) removes the inside face (%d)" % plugin.get_state()["faces"])
	plugin.debug_pref(P_SHIFT_MODE, 0)
	plugin.debug_pref(P_RECT_MODE, 0)
	await _select(root)
	sheet.free()
	await _frames(2)

# ------------------------------------------------------------------ hover / feedback

func _hover_feedback() -> void:
	var sheet := _mesh("HoverSheet")
	await _select(sheet)
	var d = sheet.data
	_reset(sheet, PLANE, Vector3(2, 0.1, 2), {"segments": 2})
	if _cam() == null:
		_check(false, "editor viewport camera")
		return
	var xf: Transform3D = sheet.global_transform
	# Edge hover: the midpoint of a top edge.
	var top: int = d.faces_facing(Vector3.UP, 5.0, PackedInt32Array())[0]
	var fv: PackedInt32Array = d.get_face_vertices(top)
	var a: int = fv[0]
	var b: int = fv[1]
	plugin.debug_select_edges(PackedInt32Array())
	plugin.debug_hover(_screen(xf * ((d.get_vertex(a) + d.get_vertex(b)) * 0.5)))
	var e: PackedInt32Array = plugin.debug_hover_state()["edge"]
	_check(e.size() == 2 and ((e[0] == a and e[1] == b) or (e[0] == b and e[1] == a)), "Edge mode: the edge under the cursor is hovered (%s vs %d-%d)" % [e, a, b])
	plugin.debug_hover(Vector2(2, 2))
	_check(PackedInt32Array(plugin.debug_hover_state()["edge"]).is_empty(), "moving off the mesh clears the edge hover")
	# Vertex hover.
	plugin.debug_select_verts(PackedInt32Array())
	plugin.debug_hover(_screen(xf * d.get_vertex(a)))
	_check(int(plugin.debug_hover_state()["vert"]) == a, "Vertex mode: the vertex under the cursor is hovered (%d vs %d)" % [plugin.debug_hover_state()["vert"], a])
	# Hover off.
	plugin.debug_pref(P_HOVER, false)
	plugin.debug_hover(_screen(xf * d.get_vertex(b)))
	_check(int(plugin.debug_hover_state()["vert"]) == -1, "Hover highlight off: nothing hovered")
	plugin.debug_pref(P_HOVER, true)
	# Viewport feedback.
	plugin.debug_action(A_OBJECT)
	plugin.debug_pref(P_SCENE_INFO, true)
	plugin.debug_pref(P_DIMENSIONS, true)
	var hs: Dictionary = plugin.debug_hover_state()
	_check(str(hs["info"]).contains("9 verts") and str(hs["info"]).contains("12 edges") and str(hs["info"]).contains("4 faces"), "scene info counts verts / edges / faces (%s)" % hs["info"])
	_check(str(hs["dims"]).begins_with("2.000 x") and str(hs["dims"]).ends_with("x 2.000"), "dimensions of the whole mesh in Object mode (%s)" % hs["dims"])
	plugin.debug_select_faces(PackedInt32Array([top]))
	hs = plugin.debug_hover_state()
	_check(str(hs["dims"]).begins_with("1.000 x") and str(hs["dims"]).ends_with("x 1.000"), "dimensions of the selected face (%s)" % hs["dims"])
	plugin.debug_pref(P_SCENE_INFO, false)
	plugin.debug_pref(P_DIMENSIONS, false)
	hs = plugin.debug_hover_state()
	_check(str(hs["info"]) == "" and str(hs["dims"]) == "", "both overlays off")
	await _select(root)
	sheet.free()
	await _frames(2)
