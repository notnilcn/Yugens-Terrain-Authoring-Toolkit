@tool
extends Node
## Headless editor check of the builder's UI layout switch (I3, docs/port/07-xogot-ui.md):
## MST ⇄ Xogot both ways with the edit state preserved (mode, selection, settings), the Xogot
## toolbar in the 3D menu bar and the MST toolbar / strip / palette gone, the Actions menu per mode
## (Xogot titles, builder extras, menu-only actions run through the menu path), the popovers
## (Add Mesh, Paint, Selection & Handles, Options + search, Tool Options), the HUD for a drawing
## tool, colour-only paint / select by slot / colour, Generate Lightmap UVs (+ undo), the
## inspector "Editable Mesh" section, the per-layout Shift / box selection defaults, a double swap
## and the real Editor Setting path (restored afterwards).
## Run: tests/xogot_i3_check.sh. Prints "XOGOT I3 OK" or "XOGOT I3 FAIL: ...".
## Does nothing unless the user args contain `xogot_i3=1`. Builds everything in
## res://tests/xogot_i3.tscn and never saves it.

var fails: PackedStringArray = []
var plugin: Object
var root: Node

const A_OBJECT := 0
const A_FACE := 1
const A_EDGE := 2
const A_SELECT_ALL := 81
const A_CLEAR := 82
const A_SELECT_RING := 83
const A_UV2 := 84
const A_ROTATE_NEG := 85
const A_PAINT_COLOUR := 86
const F_DISTANCE := 9
const P_SHIFT_MODE := 12
const P_RECT_MODE := 13
const BOX := 0
const PLANE := 1
const LAYOUT_KEY := "marching_squares_terrain/builder/ui_layout"

func _ready() -> void:
	if not Engine.is_editor_hint() or not "xogot_i3=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("xogot_i3: ", msg)

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
	# The Builder menu exists in both layouts.
	var menu := EditorInterface.get_base_control().find_child("PixelBuilderMenu", true, false) as MenuButton
	if menu:
		for con in menu.get_popup().get_signal_connection_list("id_pressed"):
			return con["callable"].get_object()
	return null

func _mesh(name: String) -> Node3D:
	var n: Node3D = ClassDB.instantiate("PixelBuilderMesh")
	n.name = name
	n.data = ClassDB.instantiate("PixelBuilderMeshData")
	root.add_child(n)
	n.owner = root
	return n

func _state() -> Dictionary:
	return plugin.get_state()

func _lay() -> Dictionary:
	return plugin.debug_layout()

func _titles() -> Dictionary:
	var out := {}
	for e in plugin.debug_xogot_actions():
		out[str(e["title"])] = e
	return out

func _cam() -> Camera3D:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	return vp.get_camera_3d() if vp else null

func _run() -> void:
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://tests/xogot_i3.tscn")
	await _frames(20)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("XOGOT I3 FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	get_window().size = Vector2i(1600, 1000)
	await _frames(10)
	plugin = _find_plugin()
	if plugin == null:
		print("XOGOT I3 FAIL: builder plugin not found")
		get_tree().quit(1)
		return
	var es := EditorInterface.get_editor_settings()
	var had_setting := es.has_setting(LAYOUT_KEY)
	var setting_before = es.get_setting(LAYOUT_KEY) if had_setting else 0
	_check(had_setting, "the ui_layout Editor Setting is registered")
	_log("editor started in the %s layout (setting %s)" % [_lay()["layout"], setting_before])
	if int(setting_before) == 1:
		_check(str(_lay()["layout"]) == "Xogot" and str(_lay().get("toolbar_parent", "")) != "", "start-up in the Xogot layout puts its toolbar in the 3D menu bar (%s)" % _lay().get("toolbar_parent", ""))
	plugin.debug_set_layout(0)
	await _frames(2)
	await _toggle_and_state()
	await _actions_menu()
	await _popovers()
	await _paint_cmds()
	await _hud()
	await _uv2_and_inspector()
	await _selection_defaults()
	await _double_swap()
	await _setting_path(setting_before)
	plugin.debug_set_layout(-1)
	es.set_setting(LAYOUT_KEY, setting_before)
	EditorInterface.get_selection().clear()
	await _frames(4)
	if fails.is_empty():
		print("XOGOT I3 OK")
	else:
		print("XOGOT I3 FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)

# ------------------------------------------------------------------ toggle

var box: Node3D

func _toggle_and_state() -> void:
	box = _mesh("LayoutBox")
	await _select(box)
	box.data.clear()
	box.data.add_shape(BOX, Vector3(2, 2, 2), Vector3.ZERO, {})
	var l := _lay()
	_check(str(l["layout"]) == "MST" and bool(l.get("mst_toolbar", false)), "starts in the MST layout (%s)" % [l])
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	plugin.debug_field(F_DISTANCE, 1.7)
	var mst_ids := _ids(["PixelBuilderToolbar", "PixelBuilderStrip"])
	_check(mst_ids.size() == 2, "the MST toolbar and strip exist before the swap")
	plugin.debug_set_layout(1)
	await _frames(3)
	l = _lay()
	var st := _state()
	_check(str(l["layout"]) == "Xogot", "switched to the Xogot layout")
	_check(bool(l.get("toolbar", false)), "the Xogot toolbar is in the 3D menu bar (%s)" % l.get("toolbar_parent", ""))
	# Freed, not merely out of the tree (find_child cannot see a node removed from the tree).
	_check(_all_freed(mst_ids), "the MST toolbar and strip are freed")
	_check(str(st["mode"]) == "Face" and int(st["faces"]) == 2, "mode and selection kept (%s)" % [st])
	_check(is_equal_approx(float(st["distance"]), 1.7), "settings kept (distance %s)" % st["distance"])
	var b: Dictionary = l.get("buttons", {})
	_check(bool(b.get("mode_face", false)) and bool(b.get("actions", false)) and bool(b.get("paint", false)) and bool(b.get("handles", false)), "mode / Actions / Paint / Handles buttons shown with a target (%s)" % [b])
	_check(str(l.get("mode_pressed", "")) == "Face", "the Face mode button is pressed")
	_check(not bool(b.get("make_editable", true)), "Make Editable hidden on a builder mesh")
	# A fresh selection while in the Xogot layout shows the buttons again.
	await _select(root)
	await _select(box)
	b = _lay().get("buttons", {})
	_check(bool(b.get("mode_face", false)) and bool(b.get("actions", false)) and bool(b.get("paint", false)), "buttons shown again after reselecting the mesh (%s)" % [b])
	plugin.debug_select_faces(PackedInt32Array([0, 1]))

# ------------------------------------------------------------------ actions menu

func _actions_menu() -> void:
	plugin.debug_select_faces(PackedInt32Array([0]))
	var t := _titles()
	for want in ["Select All Elements", "Clear Element Selection", "Select Ring", "Extrude Faces", "Inset Faces", "Dissolve Faces", "Cut", "UV Editor…", "Rotate -90°", "Paint colour"]:
		_check(t.has(want), "Face mode Actions has '%s'" % want)
	_check(not t.has("Bevel Edges") and not t.has("Connect Vertices"), "Face mode Actions has no edge / vertex commands")
	_check(t.has("Merge Faces") and not bool(t["Merge Faces"]["enabled"]) and str(t["Merge Faces"]["reason"]) == "Select at least two faces.", "disabled entries carry the reason")
	_check(t.has("Extrude Faces") and str(t["Extrude Faces"]["section"]) == "Build", "Extrude Faces under Build")
	var n: int = box.data.get_face_count()
	plugin.on_xg_action_id(A_SELECT_ALL)
	_check(int(_state()["faces"]) == n, "Select All Elements through the menu (%d of %d)" % [_state()["faces"], n])
	plugin.on_xg_action_id(A_CLEAR)
	_check(int(_state()["faces"]) == 0, "Clear Element Selection through the menu")
	plugin.debug_action(A_EDGE)
	t = _titles()
	_check(t.has("Bevel Edges") and t.has("Extrude Edges") and t.has("Select Ring") and t.has("Delete"), "Edge mode: Bevel Edges, Extrude Edges, Select Ring, Delete (builder)")
	plugin.debug_action(A_OBJECT)
	t = _titles()
	_check(t.has("Mirror Objects") and t.has("Generate Lightmap UVs") and t.has("Union") and t.has("OBJ…"), "Object mode: Mirror, Generate Lightmap UVs, Boolean / Export submenus")
	_check(not t.has("Select All Elements"), "Object mode: no element selection commands")
	plugin.debug_action(A_FACE)

# ------------------------------------------------------------------ popovers

func _popovers() -> void:
	_check(plugin.debug_xogot_open("palette"), "Add Mesh palette opens")
	plugin.debug_xogot_close()
	_check(plugin.debug_xogot_open("handles"), "Selection & Handles opens")
	var rows: PackedStringArray = _lay()["handles_rows"]
	_check("Selection X-Ray" in rows and "Orientation" in rows and "Shift selects" in rows, "handles rows in Face mode (%s)" % [rows])
	plugin.debug_xogot_close()
	_check(plugin.debug_xogot_open("paint"), "Paint popover opens")
	plugin.on_xg_paint_page(1)
	_check(int(_lay()["paint_page"]) == 1, "Paint: Colour page")
	plugin.on_xg_paint_page(2)
	_check(int(_lay()["paint_page"]) == 2, "Paint: Smoothing page")
	plugin.on_xg_paint_page(0)
	plugin.debug_xogot_close()
	_check(plugin.debug_xogot_open("options"), "Options dialog opens")
	plugin.debug_xogot_options_search("weld")
	var l := _lay()
	_check(Array(l["options_categories"]) == ["Weld"], "Options search 'weld' leaves Weld (%s)" % [l["options_categories"]])
	_check("Collapse to First Vertex" in l["options_rows"], "Weld rows shown (%s)" % [l["options_rows"]])
	plugin.debug_xogot_options_search("harden")
	l = _lay()
	_check(str(l["options_current"]) == "Bevel" and Array(l["options_rows"]) == ["Harden Normals"], "a row-label search shows only that row (%s %s)" % [l["options_current"], l["options_rows"]])
	plugin.debug_xogot_options_search("")
	_check(PackedStringArray(_lay()["options_categories"]).size() >= 15, "empty search lists every category")
	plugin.debug_xogot_close()

# ------------------------------------------------------------------ paint commands

func _paint_cmds() -> void:
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.on_xg_color(Color(1, 0, 0))
	plugin.on_action(A_PAINT_COLOUR)
	var c: Color = box.data.get_corner_color(0, 0)
	_check(c.r > 0.95 and c.g < 0.05, "Paint colour paints the face (%s)" % c)
	_check(box.data.get_face_slot(0) == box.data.get_face_slot(1), "the slot is unchanged")
	plugin.on_select_slot(box.data.get_face_slot(0))
	_check(int(_state()["faces"]) == box.data.get_face_count(), "select by slot takes every face of the slot")
	plugin.on_swatch_store(1)  # swatch 1 = red now
	plugin.on_select_colour(1)
	_check(int(_state()["faces"]) == 1, "select by colour takes the painted face (%d)" % _state()["faces"])
	# Smoothing: a new group from the selection, select it, clear it.
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	plugin.on_xg_smoothing(2, 0)
	var msg := str(_state()["message"])
	_check(msg.begins_with("New smoothing group"), "New from Selection makes a group (%s)" % msg)
	var g := int(msg.split(" ")[-1])
	plugin.debug_select_faces(PackedInt32Array())
	plugin.on_xg_smoothing(3, g)
	_check(int(_state()["faces"]) == 2, "a group chip selects its faces")
	plugin.on_xg_smoothing(4, g)
	_check(str(_state()["message"]).contains("cleared"), "Clear Group facets them")
	# Built-in preset slot, tile menu, palette save / reset / load.
	var slots0: int = box.data.materials.size()
	plugin.on_xg_paint_option(10)  # Grid preset
	_check(box.data.materials.size() == slots0 + 1 and box.data.materials[slots0] is StandardMaterial3D, "Grid preset adds a slot")
	plugin.on_xg_paint_menu(1000 + 0)  # slot 0: Select Faces Using This
	_check(int(_state()["faces"]) > 0, "tile menu: Select Faces Using This")
	var path := "user://xogot_i3_palette.json"
	plugin.on_xg_color(Color(0.1, 0.9, 0.2))
	plugin.on_swatch_store(2)
	plugin.xg_file_debug(2, path)  # save
	plugin.on_xg_paint_option(2)  # reset palette
	plugin.on_swatch_use(2)
	_check(not (_state()["color"] as Color).is_equal_approx(Color(0.1, 0.9, 0.2)), "Reset Palette restores the swatches")
	plugin.xg_file_debug(1, path)  # load
	plugin.on_swatch_use(2)
	_check((_state()["color"] as Color).is_equal_approx(Color(0.1, 0.9, 0.2)), "palette save / load round-trips the swatches (%s)" % _state()["color"])
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))

# ------------------------------------------------------------------ HUD

func _hud() -> void:
	plugin.on_palette_tile(BOX)
	await _frames(2)
	var l := _lay()
	_check(str(_state()["tool"]) == "Draw shape" and bool(l["hud_visible"]) and str(l.get("hud_title", "")) == "Box", "a Box tile starts the shape tool with its HUD (%s)" % [l.get("hud_title")])
	_check("Create" in l["hud_buttons"] and "Cancel" in l["hud_buttons"], "HUD buttons (%s)" % [l["hud_buttons"]])
	_check(plugin.debug_xogot_open("tool_options"), "Tool Options opens")
	_check("W" in _lay()["tool_options_rows"] or "H" in _lay()["tool_options_rows"], "Tool Options shows the shape fields (%s)" % [_lay()["tool_options_rows"]])
	plugin.debug_xogot_close()
	var n_before := root.get_child_count()
	plugin.on_xg_hud(0)  # Create: places the box at once
	await _frames(3)
	_check(root.get_child_count() == n_before + 1, "HUD Create places the shape (%d -> %d nodes)" % [n_before, root.get_child_count()])
	plugin.on_xg_hud(1)  # Cancel = Esc
	await _frames(2)
	_check(str(_state()["tool"]) == "Select / Move", "HUD Cancel leaves the tool")
	plugin.on_action(6)  # Paint tool: Done leaves it
	plugin.on_xg_hud(0)
	_check(str(_state()["tool"]) == "Select / Move", "HUD Done ends the paint tool")
	await _select(box)

# ------------------------------------------------------------------ UV2 / inspector

func _uv2_and_inspector() -> void:
	plugin.debug_action(A_OBJECT)
	var before: bool = box.live_uv2
	plugin.on_action(A_UV2)
	await _frames(3)
	_check(box.live_uv2 and not before, "Generate Lightmap UVs turns Live UV2 on")
	var ur := EditorInterface.get_editor_undo_redo()
	ur.get_history_undo_redo(ur.get_object_history_id(box)).undo()
	_check(not box.live_uv2, "undo turns it off again")
	EditorInterface.inspect_object(box)
	await _frames(3)
	_check(EditorInterface.get_inspector().find_child("XogotModelerInspectorSection", true, false) != null, "inspector shows the Editable Mesh section")

# ------------------------------------------------------------------ Shift / box defaults

func _sheet_points(sheet: Node3D) -> Array:
	var d = sheet.data
	var top: PackedInt32Array = d.faces_facing(Vector3.UP, 5.0, PackedInt32Array())
	var s0 := _cam().unproject_position(sheet.global_transform * d.get_face_center(top[0]))
	var xf: Transform3D = sheet.global_transform
	var lo := Vector2(INF, INF)
	var hi := Vector2(-INF, -INF)
	for p in [Vector3(-1.05, 0.05, -1.05), Vector3(0.15, 0.05, -1.05), Vector3(0.15, 0.05, 0.15), Vector3(-1.05, 0.05, 0.15)]:
		var s := _cam().unproject_position(xf * p)
		lo = Vector2(minf(lo.x, s.x), minf(lo.y, s.y))
		hi = Vector2(maxf(hi.x, s.x), maxf(hi.y, s.y))
	return [s0, lo, hi]

func _selection_defaults() -> void:
	if _cam() == null:
		_check(false, "editor viewport camera")
		return
	var sheet := _mesh("SelSheet")
	sheet.position = Vector3(6, 0, 0)
	await _select(sheet)
	sheet.data.clear()
	sheet.data.add_shape(PLANE, Vector3(2, 0.1, 2), Vector3.ZERO, {"segments": 2})
	var cam := _cam()
	cam.global_position = Vector3(6, 6, 0.01)
	cam.look_at(Vector3(6, 0, 0))
	await _frames(2)
	plugin.debug_pref(P_SHIFT_MODE, 3)  # layout default
	plugin.debug_pref(P_RECT_MODE, 2)
	for lay in [0, 1]:
		plugin.debug_set_layout(lay)
		await _frames(2)
		plugin.debug_action(A_FACE)
		var p := _sheet_points(sheet)
		plugin.debug_pick(p[0], false)
		plugin.debug_pick(p[0], true)
		var n := int(_state()["faces"])
		_check(n == (0 if lay == 0 else 1), "%s: Shift+click on a selected face %s (%d)" % [["MST", "Xogot"][lay], ["toggles it off", "keeps it (Add)"][lay], n])
		plugin.debug_marquee(p[1], p[2], false)
		n = int(_state()["faces"])
		_check(n == (1 if lay == 0 else 4), "%s: box selects %s (%d faces)" % [["MST", "Xogot"][lay], ["Complete", "Intersect"][lay], n])
	plugin.debug_set_layout(1)
	await _select(box)

# ------------------------------------------------------------------ swaps

func _ids(names: Array) -> Array:
	var out := []
	for n in names:
		var c := EditorInterface.get_base_control().find_child(n, true, false)
		if c:
			out.append(c.get_instance_id())
	return out

func _all_freed(ids: Array) -> bool:
	for id in ids:
		if is_instance_id_valid(id):
			return false
	return true

func _double_swap() -> void:
	plugin.debug_select_faces(PackedInt32Array([2, 3]))
	var xg_ids := _ids(["XogotModelerToolbar"])
	plugin.debug_set_layout(0)
	var mst_ids := _ids(["PixelBuilderToolbar"])
	plugin.debug_set_layout(1)
	var xg2_ids := _ids(["XogotModelerToolbar"])
	plugin.debug_set_layout(0)
	await _frames(3)
	_check(xg_ids.size() == 1 and mst_ids.size() == 1 and xg2_ids.size() == 1, "each quick swap built its toolbar")
	_check(_all_freed(xg_ids + xg2_ids), "the Xogot toolbars of the quick swaps are freed")
	_check(not _all_freed(_ids(["PixelBuilderToolbar"])) and _all_freed(mst_ids), "only the last MST toolbar is alive")
	var st := _state()
	_check(str(_lay()["layout"]) == "MST" and EditorInterface.get_base_control().find_child("PixelBuilderToolbar", true, false) != null, "three quick swaps end in MST with its toolbar")
	_check(str(st["mode"]) == "Face" and int(st["faces"]) == 2 and is_equal_approx(float(st["distance"]), 1.7), "state kept through the swaps (%s)" % [st])
	_check(EditorInterface.get_base_control().find_child("XogotModelerToolbar", true, false) == null, "no Xogot toolbar left in the tree")

func _setting_path(before) -> void:
	# The real switch: the preferences row writes the Editor Setting and swaps deferred.
	plugin.debug_set_layout(-1)
	await _frames(2)
	var mst_ids := _ids(["PixelBuilderToolbar", "PixelBuilderStrip"])
	plugin.on_layout_choice(1)
	await _frames(4)
	_check(str(_lay()["layout"]) == "Xogot" and int(EditorInterface.get_editor_settings().get_setting(LAYOUT_KEY)) == 1, "on_layout_choice(1) writes the setting and swaps to Xogot")
	_check(mst_ids.size() == 2 and _all_freed(mst_ids), "deferred swap: the MST toolbar / strip are freed")
	# The Builder menu's "Xogot layout" check item (id 11) toggles through the same deferred path.
	var xg_ids := _ids(["XogotModelerToolbar"])
	plugin.on_builder_menu(11)
	await _frames(4)
	_check(str(_lay()["layout"]) == "MST" and int(EditorInterface.get_editor_settings().get_setting(LAYOUT_KEY)) == 0, "Builder menu item 11 swaps to MST")
	_check(xg_ids.size() == 1 and _all_freed(xg_ids), "Builder menu swap: the Xogot toolbar is freed")
	plugin.on_builder_menu(11)
	await _frames(4)
	_check(str(_lay()["layout"]) == "Xogot", "Builder menu item 11 swaps back to Xogot")
	plugin.on_layout_choice(0)
	await _frames(4)
	_check(str(_lay()["layout"]) == "MST", "on_layout_choice(0) swaps back")
	EditorInterface.get_editor_settings().set_setting(LAYOUT_KEY, 1)
	await _frames(4)
	_check(str(_lay()["layout"]) == "Xogot", "setting the Editor Setting directly swaps (settings_changed)")
	EditorInterface.get_editor_settings().set_setting(LAYOUT_KEY, before)
	await _frames(4)
