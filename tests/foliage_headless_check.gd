@tool
extends Node
## Foliage tool check in a *headless* editor (no window, no screenshots): the real user flow
## through the real input routing. Opens the demo, selects the terrain, clicks the Foliage
## toolbar button, checks the right-hand palette (shown, items listed, Tree selected), then
## pushes synthetic mouse events into the 3D viewport (so they go through
## `_forward_3d_gui_input`) to paint, erase, undo and redo trees, and checks the props are built
## as MultiMeshes under the terrain's internal Scatter node right away.
## Run: tests/foliage_headless_check.sh   (prints "FOLIAGE HEADLESS OK" or "... FAIL: ...")

var fails: PackedStringArray = []
var terrain: Node
var container: Control
var vp: SubViewport

func _ready() -> void:
	if not Engine.is_editor_hint() or not "foliage_headless=1" in OS.get_cmdline_user_args():
		return
	_run.call_deferred()

func _log(msg: String) -> void:
	print("foliage_headless: ", msg)

func _check(ok: bool, what: String) -> void:
	_log(("ok   " if ok else "FAIL ") + what)
	if not ok:
		fails.append(what)

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

func _send(ev: InputEvent) -> void:
	get_tree().root.push_input(ev, false)

func _motion(p: Vector2, mask := 0) -> void:
	var mm := InputEventMouseMotion.new()
	mm.position = p
	mm.global_position = p
	mm.relative = Vector2(1, 0)
	mm.button_mask = mask
	_send(mm)

func _button(p: Vector2, idx: MouseButton, pressed: bool, mask := 0, shift := false) -> void:
	var b := InputEventMouseButton.new()
	b.position = p
	b.global_position = p
	b.button_index = idx
	b.pressed = pressed
	b.button_mask = mask
	b.shift_pressed = shift
	_send(b)

## Presses a terrain toolbar button by its node name (= tool name).
func _tool(name: String) -> void:
	var b := _visible(EditorInterface.get_base_control(), name, "Button") as Button
	_check(b != null, "toolbar button " + name)
	if b:
		b.button_pressed = true
		b.pressed.emit()

## Hover, press, drag `drag` px to the right, release.
func _stroke(at: Vector2, idx := MOUSE_BUTTON_LEFT, drag := 24.0, shift := false) -> void:
	var mask := MOUSE_BUTTON_MASK_LEFT if idx == MOUSE_BUTTON_LEFT else MOUSE_BUTTON_MASK_RIGHT
	for i in 3:
		_motion(at + Vector2(i, 0))
		await _frames(1)
	_button(at, idx, true, mask, shift)
	await _frames(1)
	var steps := int(drag / 4.0)
	for i in steps:
		_motion(at + Vector2(i * 4, 0), mask)
		await _frames(1)
	_button(at + Vector2(steps * 4, 0), idx, false, 0, shift)
	await _frames(3)

func _visible(root: Node, name: String, type: String) -> Node:
	for n in root.find_children(name, type, true, false):
		if n is Control and n.is_visible_in_tree():
			return n
	return null

func _ur() -> UndoRedo:
	var m := EditorInterface.get_editor_undo_redo()
	return m.get_history_undo_redo(m.get_object_history_id(terrain))

func _count(item := -1) -> int:
	return terrain.get_scatter_count(item)

## Instances drawn for `item` (MultiMeshes under the terrain's internal "Scatter" node).
func _drawn(item: int) -> int:
	var n := 0
	for c in terrain.get_children(true):
		if c.name == "Scatter":
			for m in c.get_children():
				if m is MultiMeshInstance3D and m.visible and m.multimesh and (" item %d part 0" % item) in String(m.name):
					n += m.multimesh.instance_count
	return n

## Window position of a terrain-local point (through the editor camera).
func _screen(local: Vector3) -> Vector2:
	var cam := vp.get_camera_3d()
	var world: Vector3 = terrain.global_transform * local
	if cam.is_position_behind(world):
		return Vector2(-1, -1)
	var p := cam.unproject_position(world)
	var r := container.get_global_rect()
	return r.position + p * r.size / Vector2(vp.size)

## A point on the island the editor camera sees, away from the view's edges.
func _aim(min_h := 1.0) -> Vector2:
	var r := container.get_global_rect().grow(-40)
	var best := Vector2(-1, -1)
	var best_d := INF
	for chunk in terrain.get_chunks():
		for cell in terrain.get_chunk_cells(chunk):
			var w: Vector3 = terrain.cell_to_world(cell)
			var h: float = terrain.get_cell_height(cell)
			if h < min_h:
				continue
			var s := _screen(Vector3(w.x, h, w.z))
			var d := s.distance_to(r.get_center())
			if r.has_point(s) and d < best_d:
				best_d = d
				best = s
	return best

func _run() -> void:
	await _frames(10)
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.open_scene_from_path("res://demo/hex_island.tscn")
	await _frames(30)
	await load("res://tests/editor_ready.gd").wait(get_tree())
	EditorInterface.set_main_screen_editor("3D")
	await _frames(5)
	var root := EditorInterface.get_edited_scene_root()
	terrain = root.find_child("MarchingSquaresTerrain", true, false)
	vp = EditorInterface.get_editor_viewport_3d(0)
	container = vp.get_parent() as Control
	_log("viewport %s at %s" % [vp.size, container.get_global_rect()])
	EditorInterface.get_selection().clear()
	EditorInterface.get_selection().add_node(terrain)
	EditorInterface.edit_node(terrain)
	await _frames(10)
	var base := EditorInterface.get_base_control()

	# 1. Toolbar button -> palette on the right.
	var btn := _visible(base, "Foliage", "Button") as Button
	_check(btn != null, "Foliage toolbar button is shown")
	if btn == null:
		return _finish()
	btn.button_pressed = true
	btn.pressed.emit()
	await _frames(5)
	var panel := _visible(base, "MarchingSquaresTextureSettings", "ScrollContainer") as Control
	_check(panel != null, "right side panel is shown")
	if panel:
		_log("side panel rect %s" % panel.get_global_rect())
	var list := _visible(base, "FoliagePalette", "ItemList") as ItemList
	_check(list != null, "palette ItemList is shown")
	if list == null:
		return _finish()
	_check(list.get_global_rect().size.x > 40 and list.get_global_rect().size.y > 40, "palette has a usable size %s" % list.get_global_rect().size)
	var names: PackedStringArray = []
	for i in list.item_count:
		names.append(list.get_item_text(i))
	_log("palette: %s, selected %s" % [names, list.get_selected_items()])
	_check(names.size() >= 7 and names[0] == "Grass", "palette lists Grass + the demo's props")
	var tree_row := names.find("Tree")
	_check(tree_row > 0 and list.get_selected_items() == PackedInt32Array([tree_row]), "Tree is selected by default")
	var tree: int = tree_row - 1

	# 2. Paint through the real 3D viewport input.
	var at := _aim()
	_log("aim at %s" % at)
	_check(at.x >= 0, "found an island point in view")
	if at.x < 0:
		return _finish()
	var n0 := _count(tree)
	var d0 := _drawn(tree)
	await _stroke(at)
	var n1 := _count(tree)
	_check(n1 > n0, "LMB stroke in the viewport paints trees (%d -> %d)" % [n0, n1])
	_check(_drawn(tree) > d0, "the new trees are drawn at once (%d -> %d instances)" % [d0, _drawn(tree)])
	_check(_ur().get_current_action_name().begins_with("terrain foliage paint"), "one undo action: " + _ur().get_current_action_name())
	_ur().undo()
	await _frames(3)
	_check(_count(tree) == n0 and _drawn(tree) == d0, "undo removes them (%d, drawn %d)" % [_count(tree), _drawn(tree)])
	_ur().redo()
	await _frames(3)
	_check(_count(tree) == n1, "redo repaints them (%d)" % _count(tree))

	# 3. RMB erases.
	await _stroke(at, MOUSE_BUTTON_RIGHT)
	_check(_count(tree) < n1, "RMB erases (%d -> %d)" % [n1, _count(tree)])

	# 4. Key 5 from another tool comes back to the palette with the selection kept.
	_tool("Brush")
	await _frames(3)
	_check(_visible(base, "FoliagePalette", "ItemList") == null, "palette hides on another tool")
	_tool("Foliage")
	await _frames(3)
	list = _visible(base, "FoliagePalette", "ItemList") as ItemList
	_check(list != null and list.get_selected_items() == PackedInt32Array([tree_row]), "Foliage reopens the palette, Tree still selected")

	# 5. Deselect + reselect the terrain (Godot's edit(null) / make_visible(false) path).
	EditorInterface.get_selection().clear()
	await _frames(5)
	_check(_visible(base, "FoliagePalette", "ItemList") == null, "palette hides when the terrain is deselected")
	EditorInterface.get_selection().add_node(terrain)
	EditorInterface.edit_node(terrain)
	await _frames(5)
	list = _visible(base, "FoliagePalette", "ItemList") as ItemList
	_check(list != null, "palette comes back when the terrain is selected again")
	var n2 := _count(tree)
	await _stroke(_aim())
	_check(_count(tree) > n2, "still paints after reselecting (%d -> %d)" % [n2, _count(tree)])

	# 6. Grass entry paints the grass mask (one undo action).
	if list:
		list.deselect_all()
		list.select(0, true)
		list.multi_selected.emit(0, true)
		await _stroke(_aim(), MOUSE_BUTTON_RIGHT)
		_check(_ur().get_current_action_name().begins_with("terrain foliage erase grass"), "grass erase is an undo action: " + _ur().get_current_action_name())

	# 7. A brand-new terrain (no Scatter Items): the tool still paints trees out of the box.
	var fresh = ClassDB.instantiate("MarchingSquaresTerrain")
	fresh.name = "FreshTerrain"
	root.add_child(fresh)
	fresh.owner = root
	await _frames(5)
	fresh.add_chunk(Vector2i(0, 0))
	await _frames(5)
	terrain.visible = false
	terrain = fresh
	EditorInterface.get_selection().clear()
	EditorInterface.get_selection().add_node(fresh)
	EditorInterface.edit_node(fresh)
	await _frames(10)
	_tool("Foliage")
	await _frames(3)
	list = _visible(base, "FoliagePalette", "ItemList") as ItemList
	_check(list != null, "palette shown for a new terrain")
	if list:
		var fn: PackedStringArray = []
		for i in list.item_count:
			fn.append(list.get_item_text(i))
		_log("new terrain palette: %s, selected %s" % [fn, list.get_selected_items()])
		_check(list.get_selected_items() == PackedInt32Array([fn.find("+ Tree")]), "built-in Tree is selected by default")
		_check(fresh.scatter_items.size() == 0, "opening the tool does not edit the terrain")
		_log("new terrain chunks %s at %s" % [fresh.get_chunks(), fresh.global_position])
		var spot := _aim(-1000.0)
		_check(spot.x >= 0, "new terrain in view")
		if spot.x >= 0:
			await _stroke(spot, MOUSE_BUTTON_LEFT, 40.0)
			_check(fresh.scatter_items.size() == 1 and fresh.get_scatter_count(0) > 0, "LMB adds the Tree item and paints trees (%d items, %d props)" % [fresh.scatter_items.size(), fresh.get_scatter_count(-1)])
			_check(_drawn(0) > 0, "they are drawn (%d)" % _drawn(0))
			await _frames(3)
			list = _visible(base, "FoliagePalette", "ItemList") as ItemList
			var tn: PackedStringArray = []
			for i in list.item_count:
				tn.append(list.get_item_text(i))
			_check(list.get_selected_items() == PackedInt32Array([tn.find("Tree")]), "palette now lists the Tree item, selected: %s %s" % [tn, list.get_selected_items()])
			_ur().undo()
			await _frames(2)
			_check(fresh.get_scatter_count(-1) == 0, "undo removes the trees")
			_ur().undo()
			await _frames(3)
			_check(fresh.scatter_items.size() == 0, "second undo removes the item")
			list = _visible(base, "FoliagePalette", "ItemList") as ItemList
			_check(list != null and list.get_selected_items().size() == 1 and list.get_item_text(list.get_selected_items()[0]) == "+ Tree", "palette goes back to + Tree selected")
	_finish()

func _finish() -> void:
	_log("FOLIAGE HEADLESS OK" if fails.is_empty() else "FOLIAGE HEADLESS FAIL: %s" % [fails])
	get_tree().quit(0 if fails.is_empty() else 1)
