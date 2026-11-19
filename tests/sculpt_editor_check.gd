@tool
extends Node
## Sculpting check in the real editor, driving real mouse input through the 3D view:
##   1. Brush (Drag mode, Flatten on, Falloff off): LMB paints a selection without sculpting,
##      LMB-drag on it raises every selected cell to one plateau height, as one undo action;
##      undo / redo restore exactly.
##   2. Smooth on the raised plateau: Terraced changes heights but flags no cell smooth; Sloped
##      flags the brushed cells smooth (slanted ramps); both are one undoable action.
## Run: HELPER=sculpt_editor_check.gd tests/terrain_editor_capture.sh <display> -- [scene=res://demo/hex_island.tscn]
##      [at=0.45,0.62] [zoom=-8] [dir=/abs/out]
## Saves <dir or tests/captures>/sculpt_*.png and prints "SCULPT OK" or "SCULPT FAIL: ...".
## Does nothing unless the user args contain `capture=1` (the .sh passes it). The .sh restores
## demo/ afterwards, so the strokes never reach the working tree.

const RADIUS := 7 ## samples around the hovered one that are compared

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
	print("sculpt_check: ", msg)

func _check(ok: bool, what: String) -> void:
	_log(("ok   " if ok else "FAIL ") + what)
	if not ok:
		fails.append(what)

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

## Waits for the editor's filesystem scan / import progress / modal dialogs (tests/editor_ready.gd).
func _editor_ready() -> void:
	var p := "res://tests/editor_ready.gd"
	if not FileAccess.file_exists(p):
		p = "res://__editor_ready.gd"
	await load(p).wait(get_tree())

func _send(ev: InputEvent) -> void:
	get_tree().root.push_input(ev, false)

func _motion(p: Vector2, rel: Vector2, mask := 0) -> void:
	var mm := InputEventMouseMotion.new()
	mm.position = p
	mm.global_position = p
	mm.relative = rel
	mm.button_mask = mask
	_send(mm)

func _button(p: Vector2, idx: MouseButton, pressed: bool, mask := 0) -> void:
	var b := InputEventMouseButton.new()
	b.position = p
	b.global_position = p
	b.button_index = idx
	b.pressed = pressed
	b.button_mask = mask
	_send(b)

## Hover `a`, press LMB, move to `b` in `steps` motion events, release.
func _drag(a: Vector2, b: Vector2, steps := 12) -> void:
	for i in 3:
		_motion(a + Vector2(i, 0), Vector2(1, 0))
		await _frames(2)
	_button(a, MOUSE_BUTTON_LEFT, true, MOUSE_BUTTON_MASK_LEFT)
	await _frames(3)
	for i in steps:
		_motion(a.lerp(b, (i + 1) / float(steps)), (b - a) / steps, MOUSE_BUTTON_MASK_LEFT)
		await _frames(2)
	_button(b, MOUSE_BUTTON_LEFT, false)
	await _frames(6)

func _shot(name: String) -> void:
	await _frames(20)
	var dir: String = args.get("dir", ProjectSettings.globalize_path("res://tests/captures"))
	DirAccess.make_dir_recursive_absolute(dir)
	var path := "%s/sculpt_%s.png" % [dir, name]
	get_viewport().get_texture().get_image().save_png(path)
	_log("saved " + path)

func _find(root: Node, name: String, type: String) -> Node:
	for n in root.find_children(name, type, true, false):
		if n is Control and n.is_visible_in_tree():
			return n
	return null

func _press_tool(name: String) -> bool:
	var b := _find(EditorInterface.get_base_control(), name, "Button") as Button
	if b == null:
		return false
	b.button_pressed = true
	b.pressed.emit()
	await _frames(20)
	return true

func _select_option(name: String, index: int) -> bool:
	var ob := _find(EditorInterface.get_base_control(), name, "OptionButton") as OptionButton
	if ob == null:
		return false
	ob.select(index)
	ob.item_selected.emit(index)
	await _frames(5)
	return true

func _set_check(name: String, on: bool) -> bool:
	var cb := _find(EditorInterface.get_base_control(), name, "CheckBox") as CheckBox
	if cb == null:
		return false
	cb.button_pressed = on
	await _frames(3)
	return cb.button_pressed == on

func _ur() -> UndoRedo:
	var m := EditorInterface.get_editor_undo_redo()
	return m.get_history_undo_redo(m.get_object_history_id(terrain))

## Terrain sample under a window point (camera ray through the 3D view), or null.
func _sample_at(p: Vector2) -> Variant:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	var cam := vp.get_camera_3d()
	var local := (p - view.position) * Vector2(vp.size) / view.size
	var hit: Dictionary = terrain.raycast(cam.project_ray_origin(local), cam.project_ray_normal(local), 10000.0)
	return hit["sample"] if hit.get("hit", false) else null

## {sample: [height, smooth]} around `c`.
func _snapshot(c: Vector2i) -> Dictionary:
	var d := {}
	for y in range(-RADIUS, RADIUS + 1):
		for x in range(-RADIUS, RADIUS + 1):
			var s := c + Vector2i(x, y)
			d[s] = [terrain.get_sample_height(s), terrain.get_sample_smooth(s)]
	return d

## Samples whose height differs between two snapshots -> height delta.
func _height_changes(a: Dictionary, b: Dictionary) -> Dictionary:
	var out := {}
	for s in a:
		var dh: float = b[s][0] - a[s][0]
		if absf(dh) > 1e-4:
			out[s] = dh
	return out

## Samples that became smooth.
func _new_smooth(a: Dictionary, b: Dictionary) -> int:
	var n := 0
	for s in a:
		if b[s][1] and not a[s][1]:
			n += 1
	return n

func _same(a: Dictionary, b: Dictionary) -> bool:
	return _height_changes(a, b).is_empty() and _new_smooth(a, b) == 0 and _new_smooth(b, a) == 0

func _run() -> void:
	await _frames(30)
	await _editor_ready()
	EditorInterface.open_scene_from_path(args.get("scene", "res://demo/hex_island.tscn"))
	await _frames(60)
	await _editor_ready()
	EditorInterface.set_main_screen_editor("3D")
	await _frames(20)
	var root := EditorInterface.get_edited_scene_root()
	terrain = root.find_child("MarchingSquaresTerrain", true, false) if root else null
	if terrain == null:
		_check(false, "scene has a MarchingSquaresTerrain")
		return _finish()
	var vp := EditorInterface.get_editor_viewport_3d(0)
	view = (vp.get_parent() as Control).get_global_rect()
	var center := view.get_center()
	for i in absi(int(args.get("zoom", "-8"))):
		_button(center, MOUSE_BUTTON_WHEEL_UP, true)
		_button(center, MOUSE_BUTTON_WHEEL_UP, false)
		await _frames(2)
	EditorInterface.get_selection().clear()
	EditorInterface.get_selection().add_node(terrain)
	EditorInterface.edit_node(terrain)
	await _frames(40)
	view = (vp.get_parent() as Control).get_global_rect()
	var ap: PackedStringArray = args.get("at", "0.45,0.62").split(",")
	var at := view.position + view.size * Vector2(float(ap[0]), float(ap[1]))
	var hit = _sample_at(at)
	_check(hit != null, "the camera ray at %s hits the terrain" % [args.get("at", "0.45,0.62")])
	if hit == null:
		return _finish()
	var c: Vector2i = hit
	_log("hovered sample %s, height %.2f" % [c, terrain.get_sample_height(c)])

	# 1. Brush in Drag mode: select, then drag the selection up.
	_check(await _press_tool("Brush"), "Brush toolbar button")
	_check(await _select_option("Brush Mode", 0), "Brush Mode = Drag")
	# Flatten on, Falloff off: every selected cell ends at the dragged height (a flat plateau).
	_check(await _set_check("Flatten", true), "Flatten on")
	_check(await _set_check("Falloff", false), "Falloff off")
	var h0 := _snapshot(c)
	var actions0 := _ur().get_history_count()
	await _drag(at, at + Vector2(6, 0), 2)
	_check(_height_changes(h0, _snapshot(c)).is_empty(), "LMB on an empty selection selects without sculpting")
	_check(_ur().get_history_count() == actions0, "selecting adds no undo action")
	await _shot("selected")
	await _drag(at, at + Vector2(0, -90), 16)
	var h1 := _snapshot(c)
	var raised := _height_changes(h0, h1)
	_check(raised.has(c), "the hovered sample was raised (%d samples moved)" % raised.size())
	var top: float = h1[c][0]
	var flat: bool = raised.size() > 0 and top > float(h0[c][0]) + 0.05
	for smp in raised:
		flat = flat and raised[smp] > 0.0 and absf(h1[smp][0] - top) < 1e-3
	_check(flat, "every moved sample went up to one plateau height %.2f (was %.2f)" % [top, h0[c][0]])
	_check(_ur().get_history_count() == actions0 + 1, "the drag is one undo action: " + _ur().get_current_action_name())
	await _shot("raised")
	_ur().undo()
	await _frames(10)
	_check(_same(h0, _snapshot(c)), "undo restores every height")
	_ur().redo()
	await _frames(10)
	_check(_same(h1, _snapshot(c)), "redo raises them again")

	# 2. Smooth the plateau's edge: Terraced vs Sloped.
	_check(await _press_tool("Smooth"), "Smooth toolbar button")
	_check(await _select_option("Smooth Mode", 0), "Smooth Mode = Terraced")
	await _drag(at, at + Vector2(24, 0), 12)
	var ht := _snapshot(c)
	_check(not _height_changes(h1, ht).is_empty(), "Terraced smoothing changes heights (%d samples)" % _height_changes(h1, ht).size())
	_check(_new_smooth(h1, ht) == 0, "Terraced keeps the cells flat (no new smooth flags)")
	_check(_ur().get_current_action_name().begins_with("terrain smooth"), "one undo action: " + _ur().get_current_action_name())
	await _shot("terraced")
	_ur().undo()
	await _frames(10)
	_check(_same(h1, _snapshot(c)), "undo restores the plateau")
	_check(await _select_option("Smooth Mode", 1), "Smooth Mode = Sloped")
	await _drag(at, at + Vector2(24, 0), 12)
	var hs := _snapshot(c)
	_check(not _height_changes(h1, hs).is_empty(), "Sloped smoothing changes heights (%d samples)" % _height_changes(h1, hs).size())
	_check(_new_smooth(h1, hs) > 0, "Sloped flags the brushed cells smooth (%d cells)" % _new_smooth(h1, hs))
	await _shot("sloped")
	_ur().undo()
	await _frames(10)
	_check(_same(h1, _snapshot(c)), "undo clears the smooth flags and heights")
	_ur().undo()
	await _frames(10)
	_check(_same(h0, _snapshot(c)), "undoing the drag restores the original terrain")
	_finish()

func _finish() -> void:
	if fails.is_empty():
		_log("SCULPT OK")
	else:
		_log("SCULPT FAIL: " + ", ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)
