@tool
extends Node
## Headless editor check of the I5 integration (docs/port/06-xogot.md, 07-xogot-ui.md):
## A. R2 review fixes: Delete / F / G / R with the Scene dock focused leave the mesh alone (the
##    Xogot Actions menu no longer fires its item shortcuts); Alt-click on a Paint material tile
##    samples without painting; Select All from the Actions menu re-syncs the UI; Generate
##    Lightmap UVs records no empty undo step; a preference change stores that key;
## B. Godot Mesh export (.res / .tres, combine + world transforms) and its menu entries;
## C. the "Mesh UV" bottom dock in the Xogot layout (same canvas, moves back to the window for
##    MST, the dock is freed);
## D. the HUD follows the 3D viewport that had the last input;
## E. Xogot confirmations (Mirror / Detach to new object / Merge objects; MST runs directly);
## F. R3 review fixes (I6): a mouse press outside the 3D main screen commits an open adjust session
##    (Edit › Undo by mouse undoes all of it); New group / Smooth with a typed Normal Length and a
##    HUD move with a typed Tool Options field do not re-enter the plugin (no panic logged —
##    the .sh fails on "[panic"); the smoothing overlay follows a swapped data resource.
## Run: tests/xogot_i5_check.sh. Prints "XOGOT I5 OK" or "XOGOT I5 FAIL: ...".
## Does nothing unless the user args contain `xogot_i5=1`. Builds everything in
## res://tests/xogot_i5.tscn and never saves it; files go to user://.

var fails: PackedStringArray = []
var plugin: Object
var root: Node

# Action ids (ui.rs `Action`, append-only).
const A_OBJECT := 0
const A_FACE := 1
const A_SELECT := 4
const A_SHAPE := 5
const A_EXTRUDE := 8
const A_DETACH := 15
const A_MIRROR := 40
const A_UV_EDITOR := 46
const A_MERGE := 59
const A_SELECT_ALL := 81
const A_UV2 := 84
const A_EXPORT_RES := 87
# Field ids (ui.rs `Field`, append-only).
const F_MIRROR_X := 48
const F_MIRROR_DUP := 51
const F_TO_NEW := 52
const F_DISTANCE := 9
const F_SMOOTH_TINT := 77
const F_SHOW_NORMALS := 78
const F_NORMAL_LENGTH := 79
const P_GRID_STEP := 2
const P_ADJUST := 19
const BOX := 0

func _ready() -> void:
	if not Engine.is_editor_hint() or not "xogot_i5=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("xogot_i5: ", msg)

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

func _select_many(nodes: Array) -> void:
	# edit_node first: it makes the edited node the only selection.
	await _select(nodes[0])
	var sel := EditorInterface.get_selection()
	for n in nodes.slice(1):
		sel.add_node(n)
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
	n.data.clear()
	n.data.add_shape(BOX, Vector3(2, 2, 2), Vector3.ZERO, {})
	return n

func _free(nodes: Array) -> void:
	await _select(root)
	for n in nodes:
		if is_instance_valid(n):
			n.free()
	await _frames(2)

func _lay() -> Dictionary:
	return plugin.debug_layout()

func _history() -> UndoRedo:
	var ur := EditorInterface.get_editor_undo_redo()
	return ur.get_history_undo_redo(ur.get_object_history_id(root))

func _key(code: Key, alt := false) -> InputEventKey:
	var e := InputEventKey.new()
	e.keycode = code
	e.physical_keycode = code
	e.pressed = true
	e.alt_pressed = alt
	return e

func _hide_dialogs() -> void:
	for c in EditorInterface.get_base_control().find_children("*", "AcceptDialog", true, false):
		if c.visible:
			_log("closing a dialog: %s" % c.title)
			c.hide()

func _run() -> void:
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://tests/xogot_i5.tscn")
	await _frames(20)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("XOGOT I5 FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	get_window().size = Vector2i(1600, 1000)
	await _frames(10)
	plugin = _find_plugin()
	if plugin == null:
		print("XOGOT I5 FAIL: builder plugin not found")
		get_tree().quit(1)
		return
	plugin.debug_set_layout(1)
	await _frames(3)
	await _scene_dock_keys()
	await _alt_click_tile()
	await _select_all_sync()
	await _uv2_no_op()
	await _pref_key()
	await _export_res()
	await _mesh_uv_dock()
	await _hud_viewport()
	await _confirmations()
	await _r3_adjust_outside_click()
	await _r3_typed_rebuild()
	await _r3_overlay_data_swap()
	await _r3_uv_template()
	EditorInterface.get_selection().clear()
	await _frames(4)
	plugin.debug_set_layout(-1)
	await _frames(3)
	if fails.is_empty():
		print("XOGOT I5 OK")
	else:
		print("XOGOT I5 FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)

# ------------------------------------------------------------------ A. R2 fixes

## R2 #1: the Actions MenuButton's item shortcuts fired from any focused control. Keys typed in
## the Scene dock must not edit the mesh (the 3D viewport handles the builder keys itself).
func _scene_dock_keys() -> void:
	var box := _mesh("KeysBox")
	await _select(box)
	var d = box.data
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	plugin.debug_xogot_actions()  # fills the Actions popup (its items carry the shortcuts)
	var blob: PackedByteArray = d.blob
	var hist := _history().get_version()
	var docks := EditorInterface.get_base_control().find_children("*", "SceneTreeDock", true, false)
	var trees := docks[0].find_children("*", "Tree", true, false) if docks.size() > 0 else []
	var tree: Tree = trees[0] if trees.size() > 0 else null
	_check(tree != null, "found the Scene dock tree")
	if tree == null:
		return
	tree.grab_focus()
	await _frames(1)
	_check(tree.has_focus(), "the Scene dock tree has the focus")
	for k in [KEY_F, KEY_G, KEY_R, KEY_DELETE]:
		EditorInterface.get_base_control().get_viewport().push_input(_key(k))
		await _frames(2)
		_hide_dialogs()
	await _frames(2)
	_check(d.blob == blob, "F / G / R / Delete in the Scene dock leave the mesh unchanged")
	_check(_history().get_version() == hist, "no builder undo step from those keys (%d -> %d)" % [hist, _history().get_version()])
	await _free([box])

## R2 #2: Alt-click on a material tile samples the selection; its release must not then paint
## the selection with that tile's slot.
func _alt_click_tile() -> void:
	var box := _mesh("TileBox")
	var mats: Array[Material] = [StandardMaterial3D.new(), StandardMaterial3D.new()]
	box.data.materials = mats
	await _select(box)
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_xogot_open("paint")
	await _frames(3)
	var popup: Window = EditorInterface.get_base_control().find_child("PaintPopover", true, false)
	var tile: Button = null
	if popup:
		for b in popup.find_children("*", "Button", true, false):
			if b.text.begins_with("1  ") and b.is_visible_in_tree():
				tile = b
	_check(tile != null, "found material tile 1 in the Paint popover")
	if tile:
		var at := tile.get_global_rect().get_center()
		for pressed in [true, false]:
			var e := InputEventMouseButton.new()
			e.button_index = MOUSE_BUTTON_LEFT
			e.pressed = pressed
			e.alt_pressed = true
			e.position = at
			e.global_position = at
			popup.push_input(e, true)
			await _frames(1)
		await _frames(2)
		_check(int(box.data.get_face_slot(0)) == 0, "Alt-click on tile 1 did not paint the face (slot %d)" % box.data.get_face_slot(0))
		_check(str(plugin.get_state().get("message", "")).begins_with("Sampled slot 0"), "Alt-click sampled the selection (%s)" % plugin.get_state().get("message", ""))
	plugin.debug_xogot_close()
	await _free([box])

## R2 #3: menu-only selection commands sync the UI (here the Paint popover's selection summary).
func _select_all_sync() -> void:
	var box := _mesh("AllBox")
	await _select(box)
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([]))
	plugin.debug_xogot_open("paint")
	await _frames(2)
	var popup: Window = EditorInterface.get_base_control().find_child("PaintPopover", true, false)
	var summary: Label = popup.find_child("Summary", true, false) if popup else null
	var before := summary.text if summary else ""
	plugin.on_xg_action_id(A_SELECT_ALL)
	await _frames(1)
	var n := int(plugin.get_state()["faces"])
	_check(n == 6, "Select All from the Actions menu selects the 6 faces (%d)" % n)
	_check(summary != null and summary.text != before and "6" in summary.text, "the Paint summary updated at once ('%s' -> '%s')" % [before, summary.text if summary else ""])
	plugin.debug_xogot_close()
	await _free([box])

## R2 #4: Generate Lightmap UVs when it is already on at that texel adds no undo step.
func _uv2_no_op() -> void:
	var box := _mesh("Uv2Box")
	await _select(box)
	plugin.debug_action(A_OBJECT)
	plugin.on_action(A_UV2)
	await _frames(3)
	var v1 := _history().get_version()
	_check(bool(box.live_uv2), "Generate Lightmap UVs turns Live UV2 on")
	plugin.on_action(A_UV2)
	await _frames(3)
	_check(_history().get_version() == v1, "a second Generate Lightmap UVs adds no undo step (%d -> %d)" % [v1, _history().get_version()])
	await _free([box])

## R2 #8: a preference change stores its key (one metadata write per change).
func _pref_key() -> void:
	var es := EditorInterface.get_editor_settings()
	var old = es.get_project_metadata("pixel_builder", "grid_step", 0.25)
	plugin.debug_pref(P_GRID_STEP, 0.5)
	_check(is_equal_approx(float(es.get_project_metadata("pixel_builder", "grid_step", -1.0)), 0.5), "the changed preference key is stored")
	plugin.debug_pref(P_GRID_STEP, old)

# ------------------------------------------------------------------ B. Godot Mesh export

func _export_res() -> void:
	var a := _mesh("ResA")
	var b := _mesh("ResB", Vector3(5, 0, 0))
	await _select(a)
	plugin.debug_action(A_OBJECT)
	var titles := []
	for e in plugin.debug_xogot_actions():
		if int(e["id"]) == A_EXPORT_RES:
			titles.append(e["title"])
	_check(titles == ["Godot Mesh…"], "Actions › Export Mesh has Godot Mesh… (%s)" % [titles])
	var menu := EditorInterface.get_base_control().find_child("PixelBuilderMenu", true, false) as MenuButton
	_check(menu != null and menu.get_popup().get_item_index(14) >= 0, "the Builder menu has Export Godot mesh (id 14)")
	var path := "user://xogot_i5_mesh.res"
	var err: String = plugin.debug_export(path)
	_check(err == "", "export .res (%s)" % err)
	var m := ResourceLoader.load(path, "", ResourceLoader.CACHE_MODE_IGNORE) as ArrayMesh
	var src: ArrayMesh = a.mesh
	_check(m != null and src != null and m.get_surface_count() == src.get_surface_count(), "the .res is an ArrayMesh with the compiled surfaces")
	if m:
		_check(m.surface_get_material(0) != null, "the surfaces carry their materials")
		_check(m.resource_path != src.resource_path and src.resource_path == "", "the node's own mesh stays embedded (a copy was saved)")
	err = plugin.debug_export("user://xogot_i5_mesh.tres")
	_check(err == "" and ResourceLoader.load("user://xogot_i5_mesh.tres", "", ResourceLoader.CACHE_MODE_IGNORE) is ArrayMesh, "export .tres (%s)" % err)
	# Combine + world transforms: both meshes, B moved by +5 x.
	await _select_many([a, b])
	plugin.debug_export_options({"combine": true, "apply_transforms": true})
	err = plugin.debug_export(path)
	m = ResourceLoader.load(path, "", ResourceLoader.CACHE_MODE_IGNORE) as ArrayMesh
	_check(err == "" and m != null and m.get_surface_count() == 2 * src.get_surface_count(), "combine writes both meshes' surfaces (%s)" % err)
	if m:
		_check(m.get_aabb().end.x > 5.5, "world transforms baked in (aabb %s)" % m.get_aabb())
	plugin.debug_export_options({"combine": false, "apply_transforms": false})
	for f in ["user://xogot_i5_mesh.res", "user://xogot_i5_mesh.tres"]:
		DirAccess.remove_absolute(ProjectSettings.globalize_path(f))
	await _free([a, b])

# ------------------------------------------------------------------ C. Mesh UV dock

func _dock_of(c: Node) -> Node:
	var n := c
	while n:
		if n.get_class() == "EditorDock":
			return n
		n = n.get_parent()
	return null

func _mesh_uv_dock() -> void:
	var box := _mesh("UvBox")
	await _select(box)
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([0]))
	var canvas: Control = plugin.debug_uv_editor()
	await _frames(3)
	var l := _lay()
	_check(str(l.get("uv_host", "")) == "bottom_panel", "Xogot: the UV editor opens in the bottom dock (%s)" % l.get("uv_host", ""))
	var dock := _dock_of(canvas)
	_check(dock != null and str(dock.get("title")) == "Mesh UV", "the dock is titled Mesh UV")
	_check(bool(l.get("uv_open", false)), "the Mesh UV dock is showing")
	var dock_id := dock.get_instance_id() if dock else 0
	var canvas_id := canvas.get_instance_id()
	plugin.debug_set_layout(0)
	await _frames(3)
	l = _lay()
	_check(str(l.get("uv_host", "")) == "window" and bool(l.get("uv_open", false)), "MST: the open UV editor moves into its window (%s)" % [l.get("uv_host", "")])
	_check(is_instance_id_valid(canvas_id) and _dock_of(canvas) == null, "the same canvas instance, out of the dock")
	var dk = instance_from_id(dock_id) if dock_id != 0 else null
	_check(dk != null and not dk.is_visible_in_tree(), "MST takes the panel back and the dock is closed (kept until the plugin exits)")
	plugin.debug_set_layout(1)
	await _frames(3)
	_check(str(_lay().get("uv_host", "")) == "bottom_panel" and _dock_of(canvas) != null and _dock_of(canvas).get_instance_id() == dock_id, "back in Xogot: in the same dock again")
	_check(_dock_of(canvas).is_visible_in_tree(), "the dock is shown again")
	# Leave it closed in the window (MST), so nothing stays docked for the next checks.
	plugin.debug_set_layout(0)
	await _frames(2)
	var win := canvas.get_window()
	if win and win != get_window():
		win.hide()
	plugin.debug_set_layout(1)
	await _frames(2)
	_check(str(_lay().get("uv_host", "")) == "window" and not bool(_lay().get("uv_open", true)), "a closed UV editor stays in its window until opened")
	await _free([box])

# ------------------------------------------------------------------ D. HUD per viewport

func _hud_viewport() -> void:
	var box := _mesh("HudBox")
	await _select(box)
	plugin.debug_action(A_OBJECT)
	plugin.on_action(A_SHAPE)
	await _frames(2)
	var l := _lay()
	_check(bool(l.get("hud_visible", false)), "the Draw tool shows the HUD")
	var vp1 := EditorInterface.get_editor_viewport_3d(1)
	if vp1 == null or vp1.get_camera_3d() == null:
		_check(false, "editor viewport 1 exists")
		return
	var mm := InputEventMouseMotion.new()
	mm.position = Vector2(50, 50)
	plugin.debug_viewport_input(1, mm)
	await _frames(1)
	l = _lay()
	_check(int(l.get("hud_viewport", -1)) == 1 and int(l.get("hud_parent_viewport", -1)) == 1, "input in viewport 1 moves the HUD there (%s / %s)" % [l.get("hud_viewport"), l.get("hud_parent_viewport")])
	_check(bool(l.get("hud_visible", false)), "the HUD stays shown after the move")
	plugin.debug_viewport_input(1, mm)
	plugin.debug_viewport_input(0, mm)
	await _frames(1)
	l = _lay()
	_check(int(l.get("hud_parent_viewport", -1)) == 0, "input in viewport 0 moves it back (%s)" % l.get("hud_parent_viewport"))
	plugin.on_action(A_SELECT)
	await _free([box])

# ------------------------------------------------------------------ E. confirmations

func _confirmations() -> void:
	plugin.debug_pref(P_ADJUST, true)
	var box := _mesh("MirrorBox", Vector3(1.5, 0, 0))
	await _select(box)
	plugin.debug_action(A_OBJECT)
	plugin.debug_field(F_MIRROR_X, 1)
	plugin.debug_field(F_MIRROR_DUP, 1)
	var blob: PackedByteArray = box.data.blob
	plugin.on_action(A_MIRROR)
	await _frames(1)
	var c: Dictionary = plugin.debug_confirmation()
	_check(bool(c.get("open", false)) and str(c.get("title", "")) == "Mirror Objects", "Xogot: Mirror asks first (%s)" % [c])
	_check(c.get("rows", PackedStringArray()).size() == 4, "the confirmation shows Mirror's options (%s)" % [c.get("rows")])
	_check(box.data.blob == blob, "nothing changed before OK")
	plugin.debug_confirm(false)
	_check(box.data.blob == blob and int(plugin.debug_confirmation()["action"]) == -1, "Cancel drops it")
	plugin.on_action(A_MIRROR)
	plugin.debug_confirm(true)
	await _frames(1)
	_check(box.data.blob != blob, "OK runs Mirror")
	_check(not bool(plugin.debug_adjust_state()["active"]), "a confirmed action opens no adjust panel")
	# Detach to a new object asks; in place it does not.
	box.data.clear()
	box.data.add_shape(BOX, Vector3(2, 2, 2), Vector3.ZERO, {})
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_field(F_TO_NEW, 0)
	plugin.on_action(A_DETACH)
	await _frames(1)
	_check(not bool(plugin.debug_confirmation()["open"]), "Detach in place does not ask")
	if bool(plugin.debug_adjust_state()["active"]):
		plugin.debug_adjust_end(false)
	plugin.debug_select_faces(PackedInt32Array([0]))
	plugin.debug_field(F_TO_NEW, 1)
	var before := root.get_child_count()
	plugin.on_action(A_DETACH)
	await _frames(1)
	c = plugin.debug_confirmation()
	_check(bool(c["open"]) and str(c["title"]) == "Detach Faces", "Detach to a new object asks (%s)" % [c])
	plugin.debug_confirm(true)
	await _frames(2)
	_check(root.get_child_count() == before + 1, "OK detached the face into a new node")
	plugin.debug_field(F_TO_NEW, 0)
	# Merge objects: no options -> "Run Merge Objects?"; auto-confirm hook for headless runs.
	var other := _mesh("MergeOther", Vector3(4, 0, 0))
	await _select_many([box, other])
	plugin.debug_action(A_OBJECT)
	plugin.on_action(A_MERGE)
	await _frames(1)
	c = plugin.debug_confirmation()
	_check(bool(c["open"]) and str(c["text"]) == "Run Merge Objects?", "Merge objects asks 'Run Merge Objects?' (%s)" % [c])
	plugin.debug_confirm(false)
	plugin.debug_auto_confirm(true)
	plugin.on_action(A_MERGE)
	await _frames(2)
	plugin.debug_auto_confirm(false)
	var merged := not is_instance_valid(other) or not other.is_inside_tree()
	_check(str(plugin.debug_confirmation()["last"]) == "Merge Objects" and merged, "auto-confirm ran Merge objects (other mesh removed: %s)" % merged)
	# The MST layout keeps its direct actions.
	plugin.debug_set_layout(0)
	await _frames(2)
	await _select(box)
	plugin.debug_action(A_OBJECT)
	blob = box.data.blob
	plugin.on_action(A_MIRROR)
	await _frames(1)
	_check(box.data.blob != blob, "MST: Mirror runs at once")
	_check(not bool(plugin.debug_confirmation()["open"]), "MST: no confirmation")
	if bool(plugin.debug_adjust_state()["active"]):
		plugin.debug_adjust_end(false)
	plugin.debug_set_layout(1)
	await _frames(2)
	var extra := []
	for ch in root.get_children():
		extra.append(ch)
	await _free(extra)

# ------------------------------------------------------------------ F. R3 fixes (I6)

func _top_face(d) -> int:
	var best := -1
	var by := -INF
	for f in d.get_face_count():
		var c: Vector3 = d.get_face_center(f)
		if c.y > by:
			by = c.y
			best = f
	return best

## R3 #4: Edit › Undo clicked with the mouse while the adjust panel is open used to drop the
## adjusted action. The press on the menu bar (outside the 3D main screen) now commits it first.
func _r3_adjust_outside_click() -> void:
	var box := _mesh("OutsideBox")
	await _select(box)
	var d = box.data
	plugin.debug_action(A_FACE)
	var before: PackedByteArray = d.blob
	var hist0 := _history().get_current_action()
	plugin.debug_field(F_DISTANCE, 0.5)
	plugin.debug_select_faces(PackedInt32Array([_top_face(d)]))
	plugin.debug_action_adjust(A_EXTRUDE)
	_check(bool(plugin.debug_adjust_state()["active"]) and d.blob != before, "Extrude opens an adjust session")
	# A right press (no control acts on it) in the editor's top bar, left of the main screen tabs.
	var vp := EditorInterface.get_base_control().get_viewport()
	var at := Vector2(vp.get_visible_rect().size.x * 0.3, 6)
	for pressed in [true, false]:
		var mb := InputEventMouseButton.new()
		mb.button_index = MOUSE_BUTTON_RIGHT
		mb.pressed = pressed
		mb.position = at
		mb.global_position = at
		vp.push_input(mb)
	await _frames(2)
	_check(not bool(plugin.debug_adjust_state()["active"]), "a press outside the 3D main screen ends the session")
	_check(_history().get_current_action() == hist0 + 1, "... committing the extrude as one undo step (%d -> %d)" % [hist0, _history().get_current_action()])
	_history().undo()
	_check(d.blob == before, "one undo takes the whole adjusted extrude back")
	await _free([box])

func _spin_type(parent: Node, text: String) -> bool:
	for sb in parent.find_children("*", "SpinBox", true, false):
		if sb.is_visible_in_tree():
			var le: LineEdit = sb.get_line_edit()
			le.grab_focus()
			le.text = text
			return le.has_focus()
	return false

## R3 #2 / #3: a focused SpinBox with typed text applies it on focus loss; the Paint page rebuild
## (New group) and the HUD move to another viewport must not let that signal re-enter the plugin.
func _r3_typed_rebuild() -> void:
	var box := _mesh("TypedPaintBox")
	await _select(box)
	plugin.debug_action(A_FACE)
	plugin.debug_field(F_SHOW_NORMALS, 1.0)
	plugin.debug_select_faces(PackedInt32Array([0, 1]))
	_check(plugin.debug_xogot_open("paint"), "the Paint popover opens")
	plugin.on_xg_paint_page(2)
	await _frames(2)
	var popups := EditorInterface.get_base_control().find_children("*", "PopupPanel", true, false).filter(func(p): return p.visible and not p.find_children("*", "SpinBox", true, false).is_empty())
	var typed: bool = not popups.is_empty() and _spin_type(popups[0], "0.37")
	_check(typed, "typed into Normal Length (focused)")
	plugin.on_xg_smoothing(2, 0)  # New group: the Smoothing page is rebuilt (its chips changed)
	await _frames(2)
	_check(int(box.data.get_face_smooth(0)) > 0, "New group still applies with a typed field")
	plugin.debug_xogot_close()
	plugin.debug_field(F_SHOW_NORMALS, 0.0)
	# HUD: Draw tool, Tool Options open with typed text, then input in viewport 1.
	plugin.debug_action(A_OBJECT)
	plugin.on_action(A_SHAPE)
	await _frames(2)
	_check(plugin.debug_xogot_open("tool_options"), "Tool Options opens")
	await _frames(1)
	var hud: Node = EditorInterface.get_base_control().find_child("XogotModelerHUD", true, false)
	_check(hud != null and _spin_type(hud, "1.75"), "typed into a Tool Options field (focused)")
	var vp1 := EditorInterface.get_editor_viewport_3d(1)
	if vp1 and vp1.get_camera_3d():
		var mm := InputEventMouseMotion.new()
		mm.position = Vector2(50, 50)
		plugin.debug_viewport_input(1, mm)
		await _frames(1)
		_check(int(_lay().get("hud_parent_viewport", -1)) == 1, "the HUD moved with the typed field open")
		plugin.debug_viewport_input(0, mm)
		await _frames(1)
	plugin.debug_xogot_close()
	plugin.on_action(A_SELECT)
	await _free([box])

## R3 #5: the overlay key holds the data resource, so a swapped `data` rebuilds the tint even
## when its revision matches the old one.
func _r3_overlay_data_swap() -> void:
	var box := _mesh("OverlayBox")
	box.data.set_faces_smooth(PackedInt32Array([0, 1]), 3)
	await _select(box)
	plugin.debug_field(F_SMOOTH_TINT, 1.0)
	await _frames(1)
	var ov: Dictionary = plugin.debug_attr_overlay()
	_check(int(ov.get("tint_triangles", 0)) > 0, "tint on: the smoothed faces are tinted (%s)" % [ov])
	# Same edit count as the box's data (clear, add_shape, one smoothing call): same revision.
	var other = ClassDB.instantiate("PixelBuilderMeshData")
	other.clear()
	other.add_shape(BOX, Vector3(2, 2, 2), Vector3.ZERO, {})
	other.set_faces_smooth(PackedInt32Array([0, 1]), 0)
	box.data = other
	await _frames(2)
	plugin.debug_redraw()
	var ov2: Dictionary = plugin.debug_attr_overlay()
	_check(ov.get("revision") == ov2.get("revision"), "the swapped data has the same revision (%s / %s)" % [ov.get("revision"), ov2.get("revision")])
	_check(int(ov2.get("tint_triangles", -1)) == 0, "the overlay follows the swapped data (no tint left: %s)" % [ov2])
	plugin.debug_field(F_SMOOTH_TINT, 0.0)
	await _free([box])

## R3 #6: the template dialog gives OS paths; a PNG inside the project is reported and imported
## under its res:// path, and the size is capped at 4096.
func _r3_uv_template() -> void:
	var box := _mesh("TemplateBox")
	await _select(box)
	plugin.debug_action(A_FACE)
	plugin.debug_select_faces(PackedInt32Array([0]))
	var canvas: Object = plugin.debug_uv_editor()
	await _frames(2)
	var res := "res://tests/xogot_i6_template.png"
	var os_path := ProjectSettings.globalize_path(res)
	var status: String = canvas.debug_export_template(os_path, 8192, false, false)
	_check(status.contains(res), "an OS path inside the project is reported as %s (%s)" % [res, status])
	_check(status.contains("4096 px"), "the template size is capped at 4096 (%s)" % status)
	var fs := EditorInterface.get_resource_filesystem()
	for i in 600:
		await get_tree().process_frame
		if fs.get_file_type(res) != "" and not fs.is_scanning():
			break
	_check(fs.get_file_type(res) != "", "the editor imported the new PNG (%s)" % fs.get_file_type(res))
	# Saved again over a known file: re-imported, no error.
	status = canvas.debug_export_template(os_path, 64, false, false)
	await _frames(10)
	_check(status.contains(res) and ResourceLoader.exists(res), "a second export over the imported file (%s)" % status)
	DirAccess.remove_absolute(os_path)
	DirAccess.remove_absolute(os_path + ".import")
	fs.scan()
	await _frames(10)
	await _free([box])
