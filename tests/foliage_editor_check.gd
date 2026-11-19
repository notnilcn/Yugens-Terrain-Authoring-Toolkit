@tool
extends Node
## Foliage tool check in the real editor (palette, paint, erase, grass, undo / redo).
## Run: HELPER=foliage_editor_check.gd tests/terrain_editor_capture.sh <display> -- [scene=res://demo/hex_island.tscn]
##      [hover=0.3,0.72] [zoom=-12]
## Saves tests/captures/foliage_*.png and prints "FOLIAGE OK" or "FOLIAGE FAIL: ...".
## Does nothing unless the user args contain `capture=1` (the .sh passes it).

var args := {}
var fails: PackedStringArray = []
var terrain: Node
var view := Rect2()

func _ready() -> void:
	if not Engine.is_editor_hint():
		return
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.get("capture", "") == "1":
		_run.call_deferred()

func _log(msg: String) -> void:
	print("foliage_check: ", msg)

func _check(ok: bool, what: String) -> void:
	_log(("ok   " if ok else "FAIL ") + what)
	if not ok:
		fails.append(what)

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

func _send(ev: InputEvent) -> void:
	get_tree().root.push_input(ev, false)

func _motion(p: Vector2, mask := 0, shift := false) -> void:
	var mm := InputEventMouseMotion.new()
	mm.position = p
	mm.global_position = p
	mm.relative = Vector2(1, 0)
	mm.button_mask = mask
	mm.shift_pressed = shift
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

func _key(code: Key, pressed: bool) -> void:
	var k := InputEventKey.new()
	k.keycode = code
	k.physical_keycode = code
	k.pressed = pressed
	_send(k)

## Hover, press, short drag, release (a "click" with the brush).
func _stroke(at: Vector2, idx := MOUSE_BUTTON_LEFT, shift := false, drag := 24.0) -> void:
	var mask := MOUSE_BUTTON_MASK_LEFT if idx == MOUSE_BUTTON_LEFT else MOUSE_BUTTON_MASK_RIGHT
	if shift:
		_key(KEY_SHIFT, true)
	for i in 3:
		_motion(at + Vector2(i, 0), 0, shift)
		await _frames(2)
	_button(at, idx, true, mask, shift)
	await _frames(2)
	var steps := int(drag / 4.0)
	for i in steps:
		_motion(at + Vector2(i * 4, 0), mask, shift)
		await _frames(1)
	_button(at + Vector2(steps * 4, 0), idx, false, 0, shift)
	await _frames(4)
	if shift:
		_key(KEY_SHIFT, false)
	await _frames(2)

func _shot(name: String) -> void:
	await _frames(20)
	var dir: String = args.get("dir", ProjectSettings.globalize_path("res://tests/captures"))
	DirAccess.make_dir_recursive_absolute(dir)
	var path := "%s/foliage_%s.png" % [dir, name]
	get_viewport().get_texture().get_image().save_png(path)
	_log("saved " + path)

func _find(root: Node, name: String, type: String) -> Node:
	for n in root.find_children(name, type, true, false):
		if n is Control and n.is_visible_in_tree():
			return n
	return null

func _ur() -> UndoRedo:
	var m := EditorInterface.get_editor_undo_redo()
	return m.get_history_undo_redo(m.get_object_history_id(terrain))

func _count() -> int:
	return terrain.get_scatter_count(-1)

## Waits for the editor's filesystem scan / import progress / modal dialogs (tests/editor_ready.gd).
func _editor_ready() -> void:
	var p := "res://tests/editor_ready.gd"
	if not FileAccess.file_exists(p):
		p = "res://__editor_ready.gd"
	await load(p).wait(get_tree())

func _run() -> void:
	await _frames(30)
	await _editor_ready()
	EditorInterface.open_scene_from_path(args.get("scene", "res://demo/hex_island.tscn"))
	await _frames(60)
	await _editor_ready()
	EditorInterface.set_main_screen_editor("3D")
	await _frames(20)
	var root := EditorInterface.get_edited_scene_root()
	terrain = root.find_child("MarchingSquaresTerrain", true, false)
	var vp := EditorInterface.get_editor_viewport_3d(0)
	view = (vp.get_parent() as Control).get_global_rect()
	var center := view.get_center()
	for i in absi(int(args.get("zoom", "-12"))):
		_button(center, MOUSE_BUTTON_WHEEL_UP, true)
		_button(center, MOUSE_BUTTON_WHEEL_UP, false)
		await _frames(2)
	EditorInterface.get_selection().clear()
	EditorInterface.get_selection().add_node(terrain)
	EditorInterface.edit_node(terrain)
	await _frames(40)
	var base := EditorInterface.get_base_control()

	# 1. The tool and its palette.
	var tool_btn := _find(base, "Foliage", "Button") as Button
	_check(tool_btn != null, "Foliage toolbar button exists")
	_check(_find(base, "Grass Mask", "Button") == null, "Grass Mask merged (no separate button)")
	if tool_btn == null:
		return _finish()
	tool_btn.button_pressed = true
	tool_btn.pressed.emit()
	await _frames(30)
	var list := _find(base, "FoliagePalette", "ItemList") as ItemList
	_check(list != null, "palette visible in the side panel")
	if list == null:
		return _finish()
	var names: PackedStringArray = []
	for i in list.item_count:
		names.append(list.get_item_text(i))
	_log("palette: %s" % [names])
	_check(names.size() >= 7 and names[0] == "Grass", "palette lists Grass + the demo's props")
	_check(list.get_selected_items() == PackedInt32Array([names.find("Tree")]), "Tree is selected by default")
	_check(_find(base, "Density", "HSlider") != null, "Density slider in the strip")
	await _shot("palette")

	# 2. Paint, undo, redo.
	var hp: PackedStringArray = args.get("hover", "0.3,0.72").split(",")
	var at := view.position + view.size * Vector2(float(hp[0]), float(hp[1]))
	var n0 := _count()
	await _stroke(at)
	var n1 := _count()
	_check(n1 > n0, "LMB stroke paints trees (%d -> %d)" % [n0, n1])
	_check(_ur().get_current_action_name().begins_with("terrain foliage paint"), "one undo action: " + _ur().get_current_action_name())
	await _shot("painted")
	_ur().undo()
	await _frames(5)
	_check(_count() == n0, "undo restores the props (%d)" % _count())
	_ur().redo()
	await _frames(5)
	_check(_count() == n1, "redo repaints them (%d)" % _count())

	# 3. A single click at low density on a fresh spot still places a prop. A spot can already
	# be full (demo props / min-distance), or off the island, so a few spots are tried; the
	# first click that lands on free ground must place one.
	var slider := _find(base, "Density", "HSlider") as HSlider
	slider.value = 0.05
	var n2 := _count()
	var placed := false
	for off in [Vector2(0.25, -0.05), Vector2(0.15, 0.08), Vector2(0.35, 0.02), Vector2(-0.12, -0.1), Vector2(0.05, 0.15), Vector2(0.3, -0.15)]:
		var at2: Vector2 = at + view.size * off
		if not view.has_point(at2):
			continue
		var before := _count()
		await _stroke(at2, MOUSE_BUTTON_LEFT, false, 0.0)
		_log("low-density click at %s: %d -> %d" % [off, before, _count()])
		if _count() > before:
			placed = true
			break
	_check(placed, "a click at 5%% density places at least one prop (%d -> %d)" % [n2, _count()])
	slider.value = 0.25

	# 4. Erase with Shift+LMB and with RMB.
	var n3 := _count()
	await _stroke(at, MOUSE_BUTTON_LEFT, true)
	var n4 := _count()
	_check(n4 < n3, "Shift+LMB erases (%d -> %d)" % [n3, n4])
	await _stroke(at)
	var n5 := _count()
	await _stroke(at, MOUSE_BUTTON_RIGHT)
	_check(_count() < n5, "RMB erases (%d -> %d)" % [n5, _count()])
	_ur().undo()
	await _frames(5)
	_check(_count() == n5, "erase is undoable")

	# 5. Several items at once, then grass.
	list.deselect_all()
	for nm in ["Bush", "Rock"]:
		list.select(names.find(nm), false)
	list.multi_selected.emit(names.find("Rock"), true)
	var bush := names.find("Bush") - 1
	var rock := names.find("Rock") - 1
	var b0: int = terrain.get_scatter_count(bush)
	var r0: int = terrain.get_scatter_count(rock)
	await _stroke(at + Vector2(-view.size.x * 0.12, view.size.y * 0.08), MOUSE_BUTTON_LEFT, false, 60.0)
	_check(terrain.get_scatter_count(bush) > b0 and terrain.get_scatter_count(rock) > r0, "multi-select paints bushes and rocks")
	await _shot("multi")
	list.deselect_all()
	list.select(0, true)
	list.multi_selected.emit(0, true)
	var nprops := _count()
	await _stroke(at, MOUSE_BUTTON_RIGHT)
	var name := _ur().get_current_action_name()
	_check(name.begins_with("terrain foliage erase grass") or name.begins_with("terrain foliage"), "grass erase is one undo action: " + name)
	_check(_count() == nprops, "grass erase keeps the props")
	await _shot("grass_erased")

	# 6. Add a prop from a model file, paint it, remove it, undo the removal.
	var items_before: int = terrain.scatter_items.size()
	var add := _find(base, "Add Mesh / Scene...", "Button") as Button
	if add == null:
		for b in base.find_children("*", "Button", true, false):
			if b.is_visible_in_tree() and b.text == "Add Mesh / Scene...":
				add = b
	_check(add != null, "Add Mesh / Scene... button")
	if add:
		add.pressed.emit()
		await _frames(10)
		var dlg: EditorFileDialog = null
		for d in base.find_children("*", "EditorFileDialog", true, false):
			if d.visible:
				dlg = d
		_check(dlg != null, "file dialog opens")
		if dlg:
			dlg.file_selected.emit(args.get("model", "res://demo/assets/models/enemy.glb"))
			await _frames(20)
			_check(terrain.scatter_items.size() == items_before + 1, "model added to Scatter Items")
			var it = terrain.scatter_items[items_before]
			_check(it != null and it.scene != null and it.get_label() == "enemy", "item uses the scene, named after the file")
			list = _find(base, "FoliagePalette", "ItemList") as ItemList
			_check(list != null and list.item_count == names.size() + 1 and list.get_selected_items() == PackedInt32Array([names.size()]), "palette shows and selects the new prop")
			var c0: int = terrain.get_scatter_count(items_before)
			await _stroke(at)
			_check(terrain.get_scatter_count(items_before) > c0, "the new prop paints (%d)" % terrain.get_scatter_count(items_before))
			await _shot("model")
			var edit: Button = null
			for b in base.find_children("*", "Button", true, false):
				if b.is_visible_in_tree() and b.text == "Edit":
					edit = b
			edit.pressed.emit()
			await _frames(15)
			_check(EditorInterface.get_inspector().get_edited_object() == it, "Edit shows the item in the inspector")
			_check(_find(base, "FoliagePalette", "ItemList") != null, "the tool stays open while editing an item")
			await _shot("edit")
			var rm: Button = null
			for b in base.find_children("*", "Button", true, false):
				if b.is_visible_in_tree() and b.text == "Remove":
					rm = b
			rm.pressed.emit()
			await _frames(10)
			_check(terrain.scatter_items.size() == items_before, "Remove drops the item")
			_ur().undo()
			await _frames(10)
			_check(terrain.scatter_items.size() == items_before + 1 and terrain.get_scatter_count(items_before) > 0, "undo brings item and instances back")
			list = _find(base, "FoliagePalette", "ItemList") as ItemList
			_check(list != null and list.item_count == names.size() + 1, "palette follows undo")
			await _frames(120)
			await _shot("final")
	_finish()

func _finish() -> void:
	if fails.is_empty():
		_log("FOLIAGE OK")
	else:
		_log("FOLIAGE FAIL: %s" % [fails])
	get_tree().quit(0 if fails.is_empty() else 1)
