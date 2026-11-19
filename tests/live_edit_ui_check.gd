@tool
extends Node
## Editor UI -> undo -> running game, headless (tests/live_edit_ui_check.sh registers this as a
## temporary autoload and runs the game with --headless).
## Editor side (user arg `live_edit_ui=1`): opens the main scene, selects its terrain and makes
## real strokes with the MST tools by sending mouse / key events to the 3D view's input surface
## (the same path as a user's mouse: Node3DEditorViewport -> the plugin's
## `_forward_3d_gui_input`), then undoes them one by one through the editor's undo history, like
## Ctrl+Z. After every stroke and undo it checks that the data resource holds what the terrain
## shows, that undo restores every chunk exactly, and that the terrain draws what a rebuild from
## scratch draws (no leftovers). With a game running (`play_main_scene`) Godot's live edit
## forwards the undo actions' method calls; the game side (enabled by the `active` file) answers
## each request with its chunk hash and the same rebuild comparison, so every step must reach
## the game too. Files live in tests/captures/live_edit_ui/.

const FP := preload("res://tests/terrain_fp.gd")
const DIR := "res://tests/captures/live_edit_ui/"

var fails := 0
var surface: Control
var plugin: Object
var t
var mouse := Vector2.ZERO
var game_seen := {}
var game_rebuilt := 0


func _ready() -> void:
	if Engine.is_editor_hint():
		if "live_edit_ui=1" in OS.get_cmdline_user_args():
			_editor_run.call_deferred()
	elif FileAccess.file_exists(DIR + "active"):
		_game_run.call_deferred()


func _log(msg: String) -> void:
	print("live_edit_ui: ", msg)


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("live_edit_ui FAIL: ", what)


func _wait(sec: float) -> void:
	await get_tree().create_timer(sec).timeout


func _write(name: String, text: String) -> void:
	var f := FileAccess.open(DIR + name, FileAccess.WRITE)
	f.store_string(text)
	f.close()


func _read(name: String) -> String:
	return FileAccess.get_file_as_string(DIR + name)


func _wait_file(name: String, timeout: float) -> bool:
	var waited := 0.0
	while not FileAccess.file_exists(DIR + name):
		await _wait(0.1)
		waited += 0.1
		if waited > timeout:
			_log("timed out waiting for " + name)
			return false
	return true


static func _hash_blobs(d: Dictionary) -> int:
	var keys := d.keys()
	keys.sort()
	var acc := []
	for k in keys:
		acc.append([k, d[k]])
	return hash(var_to_bytes(acc))


# Rebuild comparison: fingerprint now, rebuild from the data resource, fingerprint again.
# (Waits first: in the editor, prop colliders follow a stroke once the terrain was quiet for
# 350 ms.)
func _rebuilt_mismatches(n) -> Array:
	await _wait(0.6)
	var a := FP.fingerprint(n)
	var blobs := FP.blobs(n)
	n.rebuild()
	await _wait(0.5)
	var bad := FP.mismatches(a, FP.fingerprint(n))
	if FP.blobs(n) != blobs:
		bad.push_front("data resource != terrain")
	return bad


# ------------------------------------------------------------------ input

func _send(ev: InputEvent) -> void:
	if ev is InputEventMouse:
		# Through the editor window like a real mouse: GUI picks the 3D view's input surface.
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


func _button(p: Vector2, pressed: bool) -> void:
	var e := InputEventMouseButton.new()
	e.position = p
	e.global_position = p
	e.button_index = MOUSE_BUTTON_LEFT
	e.pressed = pressed
	e.button_mask = MOUSE_BUTTON_MASK_LEFT if pressed else 0
	await _send(e)


# LMB press at `a`, drag to `b` in steps (holding `hold` seconds at the end), release.
func _drag(a: Vector2, b: Vector2, hold := 0.0) -> void:
	await _move(a, false)
	await _button(a, true)
	for i in 8:
		await _move(a.lerp(b, (i + 1) / 8.0), true)
	if hold > 0.0:
		await _wait(hold)
	await _button(b, false)
	await _wait(0.2)


# A toolbar tool button (index in the toolbar: the nine original tools, then the extras).
func _tool_button(i: int) -> void:
	plugin.call("on_tool", i)
	await _wait(0.2)


# ------------------------------------------------------------------ editor side

func _request(name: String) -> Dictionary:
	if not FileAccess.file_exists(DIR + "active"):
		return {}
	_write("req_" + name, name)
	if not await _wait_file("ack_" + name, 60.0):
		return {"timeout": true}
	return JSON.parse_string(_read("ack_" + name))


func _step_checks(name: String, expect: Dictionary) -> void:
	var now := FP.blobs(t)
	if not expect.is_empty():
		_check(now == expect, "%s: the chunks are not back to their state before the stroke (%d differ)" % [name, FP.diff(expect, now).before.size()])
	var bad = await _rebuilt_mismatches(t)
	_check(bad.is_empty(), "%s: the editor terrain differs from a rebuild in %d entries: %s" % [name, bad.size(), bad.slice(0, 6)])
	var g := await _request(name)
	if g.is_empty():
		return
	_check(not g.has("timeout"), "%s: no answer from the game" % name)
	if g.has("timeout"):
		return
	_check(int(g.blobs) == _hash_blobs(FP.blobs(t)), "%s: the running game's chunks differ from the editor's" % name)
	_check(g.bad.is_empty(), "%s: the running game draws something a rebuild does not: %s" % [name, g.bad])
	_log("%s: game in sync (%d chunks)" % [name, g.chunks])


func _editor_run() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DIR))
	for f in DirAccess.get_files_at(DIR):
		DirAccess.remove_absolute(DIR + f)
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
	var play := "game=1" in OS.get_cmdline_user_args()
	if play:
		_write("active", "1")
		EditorInterface.play_main_scene()
		if not await _wait_file("game_ready", 120.0):
			get_tree().quit(1)
			return
		# Running opens the debugger panel (shrinking the 3D view) and may switch screens.
		plugin.hide_bottom_panel()
		EditorInterface.set_main_screen_editor("3D")
		await _wait(0.5)
	surface.grab_focus()
	var size := surface.size
	var c := size * 0.5
	var cam := vp.get_camera_3d()
	var hit = t.raycast(cam.project_ray_origin(c), cam.project_ray_normal(c), 2000.0)
	_log("toolbar visible %s, surface %s at %s in %s" % [tb.is_visible_in_tree(), surface, surface.get_global_rect(), surface.get_window().size])
	_log("view %s, camera %s, centre ray hits the terrain: %s" % [size, cam.global_position, hit])
	var ur := EditorInterface.get_editor_undo_redo()
	var history: UndoRedo = ur.get_history_undo_redo(ur.get_object_history_id(t))
	var strokes := [
		["level", func(): await _tool_button(1); await _drag(c, c + Vector2(size.x * 0.2, 0))],
		["brush drag", func(): await _tool_button(0); await _drag(c, c + Vector2(10, 0)); await _drag(c + Vector2(5, 0), c + Vector2(5, -40))],
		["smooth", func(): await _tool_button(2); await _drag(c - Vector2(30, 0), c + Vector2(30, 10), 0.3)],
		["bridge", func(): await _tool_button(3); await _drag(c - Vector2(size.x * 0.25, 10), c + Vector2(size.x * 0.1, 20))],
		["vertex paint", func(): await _tool_button(5); await _drag(c, c + Vector2(40, 15))],
		["stamp", func(): await _tool_button(9); await _move(c, false); await _button(c, true); await _button(c, false); await _wait(0.2)],
	]
	await _step_checks("start", {})
	var before_each := []
	var names := []
	for s in strokes:
		var before := FP.blobs(t)
		var version := history.get_version()
		await s[1].call()
		if history.get_version() == version:
			_log("%s: no undo action (terrain changed: %s, history %d, global %d)" % [s[0], FP.blobs(t) != before, ur.get_object_history_id(t), ur.get_history_undo_redo(EditorUndoRedoManager.GLOBAL_HISTORY).get_version()])
			continue
		_check(FP.blobs(t) != before, "%s changed nothing but made an undo action" % s[0])
		before_each.append(before)
		names.append(s[0])
		await _step_checks(s[0], {})
	_check(names.size() >= 4, "only %d of %d strokes edited the terrain: %s" % [names.size(), strokes.size(), names])
	for i in range(names.size() - 1, -1, -1):
		history.undo()  # Ctrl+Z
		await _wait(0.3)
		await _step_checks("undo " + names[i], before_each[i])
	for i in names.size():
		history.redo()
		await _wait(0.2)
	await _step_checks("redo all", {})
	# Texture slot edit (a sub-resource of the terrain's preset, like the texture panel does).
	var slot = t.texture_set.slots[0]
	var tint: Color = slot.tint
	ur.create_action("terrain texture 1 tint (live edit ui check)")
	ur.add_do_property(slot, "tint", Color(0.9, 0.1, 0.2))
	ur.add_undo_property(slot, "tint", tint)
	ur.commit_action()
	var g := await _request("slot tint")
	if not g.is_empty():
		_check(g.get("tint0", "") == Color(0.9, 0.1, 0.2).to_html(), "slot tint: the running game's slot did not change (%s)" % g.get("tint0"))
		_check(int(g.get("rebuilt", 0)) >= int(g.chunks), "slot tint: the running game did not re-apply the terrain look (%s of %s chunks remeshed)" % [g.get("rebuilt"), g.chunks])
	history.undo()
	# Leave the scene unmodified (nothing is saved).
	for i in names.size():
		history.undo()
	if play:
		EditorInterface.stop_playing_scene()
		DirAccess.remove_absolute(DIR + "active")
		await _wait(1.0)
	print("LIVE EDIT UI OK (%d strokes%s)" % [names.size(), ", game in sync" if play else ""] if fails == 0 else "LIVE EDIT UI: %d failure(s)" % fails)
	get_tree().quit(1 if fails else 0)


# ------------------------------------------------------------------ game side

func _game_run() -> void:
	await _wait(3.0)
	var g = get_tree().current_scene.get_node_or_null("MarchingSquaresTerrain")
	g.chunks_rebuilt.connect(func(c): game_rebuilt += c.size())
	_write("game_ready", "1")
	while FileAccess.file_exists(DIR + "active"):
		for f in DirAccess.get_files_at(DIR):
			if f.begins_with("req_") and not game_seen.has(f):
				game_seen[f] = true
				await _wait(0.4)  # the live-edit call and the rebuild it triggers
				await _wait(0.6)
				var rebuilt := game_rebuilt
				var bad = await _rebuilt_mismatches(g)
				game_rebuilt = 0
				var ack := {"blobs": _hash_blobs(FP.blobs(g)), "chunks": g.get_chunks().size(), "bad": bad.slice(0, 6)}
				ack["tint0"] = g.texture_set.slots[0].tint.to_html()
				ack["rebuilt"] = rebuilt  # chunks remeshed since the last request
				_write("ack_" + f.trim_prefix("req_"), JSON.stringify(ack))
		await _wait(0.1)
	get_tree().quit()
