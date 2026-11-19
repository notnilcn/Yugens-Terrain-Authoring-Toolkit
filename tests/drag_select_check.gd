@tool
extends Node
## Brush tool Drag mode from several editor cameras, headless (tests/drag_select_check.sh
## registers this as a temporary autoload). Through the 3D view's real input path it selects a
## cell, drags it up by fixed mouse distances from a close view, then from far and steep views
## (wheel zoom out, keypad orbit up), and checks per view:
##   * the drag raises the selected cell, and one drag is one undo action that restores it;
##   * the selection survives the drag (a second drag moves it again);
##   * the height per pixel stays under one level per `selection::DRAG_PX_PER_STEP` (16) pixels.
## Prints "DRAG SELECT OK" or the failures (exit 1).

const PX_PER_STEP := 16.0

var fails := 0
var surface: Control
var plugin: Object
var t
var mouse := Vector2.ZERO


func _ready() -> void:
	if Engine.is_editor_hint() and "drag_select=1" in OS.get_cmdline_user_args():
		_run.call_deferred()


func _log(msg: String) -> void:
	print("drag_select: ", msg)


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("drag_select FAIL: ", what)


func _wait(sec: float) -> void:
	await get_tree().create_timer(sec).timeout


func _send(ev: InputEvent) -> void:
	if ev is InputEventMouse:
		ev.global_position = surface.get_global_rect().position + ev.position
		ev.position = ev.global_position
	surface.get_window().push_input(ev)
	await get_tree().process_frame


func _move(p: Vector2, held: bool) -> void:
	var e := InputEventMouseMotion.new()
	e.position = p
	e.global_position = p
	e.relative = p - mouse
	e.button_mask = MOUSE_BUTTON_MASK_LEFT if held else 0
	mouse = p
	await _send(e)


func _button(p: Vector2, idx: MouseButton, pressed: bool) -> void:
	var e := InputEventMouseButton.new()
	e.position = p
	e.global_position = p
	e.button_index = idx
	e.pressed = pressed
	e.button_mask = (MOUSE_BUTTON_MASK_LEFT if idx == MOUSE_BUTTON_LEFT else 0) if pressed else 0
	await _send(e)


func _key(k: Key) -> void:
	for pressed in [true, false]:
		var e := InputEventKey.new()
		e.keycode = k
		e.physical_keycode = k
		e.pressed = pressed
		await _send(e)


func _drag(a: Vector2, b: Vector2) -> void:
	await _move(a, false)
	await _wait(0.05)
	await _button(a, MOUSE_BUTTON_LEFT, true)
	for i in 10:
		await _move(a.lerp(b, (i + 1) / 10.0), true)
	await _button(b, MOUSE_BUTTON_LEFT, false)
	await _wait(0.2)


var sample := Vector2i.ZERO


## Heights of the samples around the clicked one (the brush footprint may weight the clicked
## sample below 1, so the drag offset is the largest change among them).
func _heights() -> PackedFloat32Array:
	var out := PackedFloat32Array()
	for y in range(-4, 5):
		for x in range(-4, 5):
			out.append(t.get_sample_height(sample + Vector2i(x, y)))
	return out


func _raised(before: PackedFloat32Array) -> float:
	var now := _heights()
	var m := 0.0
	for i in now.size():
		if absf(now[i] - before[i]) > absf(m):
			m = now[i] - before[i]
	return m


func _measure(label: String, history: UndoRedo, cam: Camera3D, c: Vector2) -> void:
	var hit: Dictionary = t.raycast(cam.project_ray_origin(c), cam.project_ray_normal(c), 5000.0)
	if not hit.get("hit", false):
		_check(false, "%s: the centre ray misses the terrain" % label)
		return
	var p: Vector3 = hit.position
	sample = hit.sample
	var pitch := rad_to_deg(asin(-cam.project_ray_normal(c).y))
	var dist := cam.global_position.distance_to(p)
	# A click selects the cell under the cursor (no sculpting).
	await _move(c, false)
	await _button(c, MOUSE_BUTTON_LEFT, true)
	await _button(c, MOUSE_BUTTON_LEFT, false)
	await _wait(0.1)
	var h0 := _heights()
	var level: float = abs(t.level_height)
	for px in [24.0, 48.0, 96.0]:
		var v := history.get_version()
		await _drag(c, c - Vector2(0, px))
		var dh := _raised(h0)
		# What the uncapped camera-facing plane (the old mapping) gives for this drag.
		var o := cam.project_ray_origin(c - Vector2(0, px))
		var d := cam.project_ray_normal(c - Vector2(0, px))
		var n := Vector3(d.x, 0, d.z)
		var plane := INF if n.length() < 0.2 else o.y + d.y * n.dot(p - o) / n.dot(d) - p.y
		var limit: float = px * level / PX_PER_STEP + level * 0.5 + 1e-3  # + half a step of snapping
		_log("%s: dist %.1f pitch %.0f deg, %d px up -> %+.2f (plane %+.2f, limit %.2f, %.1f px/level)" % [label, dist, pitch, px, dh, plane, limit, px / max(dh / level, 1e-3)])
		_check(dh > 0.0 or px < PX_PER_STEP * 0.5, "%s: a %d px drag on the selection raises it (%+.3f)" % [label, px, dh])
		_check(dh <= limit, "%s: %d px raised %.2f > %.2f" % [label, px, dh, limit])
		_check(history.get_version() == v + 1, "%s: one undo action per drag (%d -> %d)" % [label, v, history.get_version()])
		history.undo()
		await _wait(0.2)
		_check(absf(_raised(h0)) < 1e-4, "%s: undo restores the height" % label)
	# Only the first drag follows the selecting click; the others raising too shows the
	# selection persisted through each drag and its undo.


func _run() -> void:
	await _wait(2.0)
	EditorInterface.open_scene_from_path(ProjectSettings.get_setting("application/run/main_scene"))
	await _wait(2.0)
	EditorInterface.set_main_screen_editor("3D")
	var scene := EditorInterface.get_edited_scene_root()
	t = scene.get_node_or_null("MarchingSquaresTerrain") if scene else null
	var tb = EditorInterface.get_base_control().find_child("MarchingSquaresToolbar", true, false)
	if t == null or tb == null:
		_log("FAIL no terrain / MST toolbar")
		get_tree().quit(1)
		return
	for b in tb.find_children("*", "BaseButton", true, false):
		for con in b.get_signal_connection_list("pressed"):
			if con.callable.get_object() and con.callable.get_object().get_class() == "MarchingSquaresTerrainPlugin":
				plugin = con.callable.get_object()
	var vp := EditorInterface.get_editor_viewport_3d(0)
	for c in vp.get_parent().get_parent().get_children():
		if c is Control and not c.get_signal_connection_list("gui_input").is_empty():
			surface = c
	EditorInterface.edit_node(t)
	await _wait(1.0)
	if plugin == null or surface == null:
		_log("FAIL plugin / 3D view input surface not found")
		get_tree().quit(1)
		return
	surface.grab_focus()
	plugin.call("on_tool", 0)  # Brush (Drag mode)
	await _wait(0.2)
	var ur := EditorInterface.get_editor_undo_redo()
	var history: UndoRedo = ur.get_history_undo_redo(ur.get_object_history_id(t))
	var c := surface.size * 0.5
	var cam := vp.get_camera_3d()
	_log("level_height %.2f, terrain scale %s, view %s" % [t.level_height, t.global_transform.basis.get_scale(), surface.size])
	await _measure("start view", history, cam, c)
	# Zoom in a few wheel steps for a close view.
	for i in 6:
		await _button(c, MOUSE_BUTTON_WHEEL_UP, true)
		await _button(c, MOUSE_BUTTON_WHEEL_UP, false)
	await _wait(0.5)
	await _measure("close", history, cam, c)
	for i in 14:
		await _button(c, MOUSE_BUTTON_WHEEL_UP, true)
		await _button(c, MOUSE_BUTTON_WHEEL_UP, false)
	await _wait(0.5)
	await _measure("very close", history, cam, c)
	for i in 14:
		await _button(c, MOUSE_BUTTON_WHEEL_DOWN, true)
		await _button(c, MOUSE_BUTTON_WHEEL_DOWN, false)
	for i in 16:
		await _button(c, MOUSE_BUTTON_WHEEL_DOWN, true)
		await _button(c, MOUSE_BUTTON_WHEEL_DOWN, false)
	await _wait(0.5)
	await _measure("far", history, cam, c)
	for i in 3:
		await _key(KEY_KP_8)  # orbit the view up 15 degrees
	await _wait(0.5)
	await _measure("far steep", history, cam, c)
	await _key(KEY_KP_7)  # top view: the drag plane degenerates
	await _wait(0.5)
	await _measure("top down", history, cam, c)
	_log("DRAG SELECT OK" if fails == 0 else "DRAG SELECT FAIL (%d)" % fails)
	get_tree().quit(0 if fails == 0 else 1)
