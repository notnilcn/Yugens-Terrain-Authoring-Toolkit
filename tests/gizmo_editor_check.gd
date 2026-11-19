@tool
extends Node
## Headless editor check of Godot's own move / rotate / scale gizmo on the demo's nodes.
## Run: tests/gizmo_editor_check.sh [scene=res://demo/hex_island.tscn]
## Prints "GIZMO OK" or "GIZMO FAIL: ...". Does nothing unless the user args contain
## `gizmo_check=1` (the .sh passes it).
##
## For each node an artist selects in the demo (characters, the builder ruin, the knight rig,
## lights, effects, cameras) it selects the node through EditorSelection like a click does and
## checks everything this addon controls that can keep Godot's transform gizmo away:
##   * no `_edit_lock_` meta on the node (Node3DEditor skips locked nodes),
##   * the editor camera still sees the transform gizmo layers (GIZMO_EDIT 26, GIZMO_BASE 27),
##     with the Pixel Preview on and off,
## The builder may lock its own mesh only while one of its element modes (Face / Edge / Vertex)
## is active; that is checked too, as is that the lock is gone again in Object mode and never
## saved into the scene.

const GIZMO_LAYERS := (1 << 26) | (1 << 27)

var args := {}
var fails: PackedStringArray = []
var root: Node

func _ready() -> void:
	if not Engine.is_editor_hint():
		return
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.get("gizmo_check", "") == "1":
		_run.call_deferred()

func _log(msg: String) -> void:
	print("gizmo_check: ", msg)

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
	await _frames(6)

func _editor_cam() -> Camera3D:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	return vp.get_camera_3d() if vp else null

func _locked(n: Node) -> bool:
	return n.has_meta("_edit_lock_") and bool(n.get_meta("_edit_lock_"))

func _renderer() -> Node:
	for n in root.find_children("*", "PixelRenderer", true, false):
		return n
	return null

func _set_preview(on: bool) -> void:
	var r := _renderer()
	if r:
		r.set("editor_preview", on)
	await _frames(8)

func _check_node(path: String) -> void:
	var n := root.get_node_or_null(path)
	if n == null:
		_check(false, "node %s exists" % path)
		return
	await _select(n)
	var cam := _editor_cam()
	_check(not _locked(n), "%s: no _edit_lock_ while selected" % path)
	if cam:
		_check(cam.cull_mask & GIZMO_LAYERS == GIZMO_LAYERS, "%s: editor camera sees the gizmo layers (mask 0x%x)" % [path, cam.cull_mask])

func _run() -> void:
	var p := "res://tests/editor_ready.gd"
	await load(p).wait(get_tree())
	EditorInterface.open_scene_from_path(args.get("scene", "res://demo/hex_island.tscn"))
	await _frames(30)
	root = EditorInterface.get_edited_scene_root()
	if root == null:
		print("GIZMO FAIL: no scene")
		get_tree().quit(1)
		return
	EditorInterface.set_main_screen_editor("3D")
	await _frames(10)
	var nodes := ["Characters/Barbarian", "Characters/Mage", "Characters/Rogue", "Characters", "BuilderRuin",
		"BuilderRuin/Ruin", "PlayerRig", "Lantern", "Fireflies", "Camera", "Sun", "Weather", "Birds", "Smoke",
		"PixelWater", "MarchingSquaresTerrain/GrassFire"]
	for preview in [true, false]:
		await _set_preview(preview)
		_log("--- preview %s" % preview)
		for path in nodes:
			await _check_node(path)
	await _set_preview(true)

	# Builder: element modes may lock its mesh, Object mode must not; the lock is never saved.
	var ruin := root.get_node_or_null("BuilderRuin/Ruin")
	var plugin: Object = null
	for c in get_tree().root.find_children("*", "PixelBuilderPlugin", true, false):
		plugin = c
	if ruin and plugin and plugin.has_method("on_action"):
		await _select(ruin)
		plugin.call("on_action", 1)
		await _frames(4)
		_check(_locked(ruin), "Ruin: locked in Face mode (builder handles own the transform)")
		# What a scene save does: EDITOR_PRE_SAVE, pack, EDITOR_POST_SAVE.
		root.propagate_notification(Node3D.NOTIFICATION_EDITOR_PRE_SAVE)
		var packed := PackedScene.new()
		packed.pack(root)
		root.propagate_notification(Node3D.NOTIFICATION_EDITOR_POST_SAVE)
		_check(not ("_edit_lock_" in var_to_str(packed._bundled)), "Ruin: lock not packed into the scene")
		_check(_locked(ruin), "Ruin: lock back after the save")
		plugin.call("on_action", 0)
		await _frames(4)
		_check(not _locked(ruin), "Ruin: unlocked again in Object mode")
		plugin.call("on_action", 1)
		await _frames(4)
		var other := root.get_node_or_null("Characters/Barbarian")
		await _select(other)
		_check(not _locked(ruin), "Ruin: lock released when another node is selected")
		_check(not _locked(other), "Barbarian: never locked by the builder")
		# Re-selecting it later starts in Object mode: Godot's gizmo shows again.
		await _select(ruin)
		_check(not _locked(ruin) and str(plugin.call("get_state")["mode"]).to_lower().begins_with("object"),
			"Ruin: a fresh selection starts in Object mode, unlocked (%s)" % plugin.call("get_state"))
	else:
		_log("builder mode hook not found (plugin %s)" % plugin)

	# Nothing in the saved scene carries a lock.
	var any := root.find_children("*", "", true, false)
	any.append(root)
	EditorInterface.get_selection().clear()
	await _frames(6)
	var stale := []
	for n in any:
		if is_instance_valid(n) and n.has_meta("_edit_lock_"):
			stale.append(str(root.get_path_to(n)))
	_check(stale.is_empty(), "no _edit_lock_ left on any node after deselecting (%s)" % ", ".join(stale))

	if fails.is_empty():
		print("GIZMO OK")
	else:
		print("GIZMO FAIL: ", "; ".join(fails))
	get_tree().quit(0 if fails.is_empty() else 1)
