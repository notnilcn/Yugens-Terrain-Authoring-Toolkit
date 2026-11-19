@tool
extends Node
## Live-edit check (editor -> running game), registered as a temporary autoload by
## tests/live_edit_check.sh. Editor side (user arg `live_edit=1`): opens the main scene, plays
## it, then makes a terrain edit through an undo action exactly like the MST brush does
## (`set_chunks_data` do / undo methods, committed without executing), and undoes it. Godot's
## live edit forwards those method calls to the running game. Game side (enabled by the
## `active` file the editor writes): on each request, saves a capture and the terrain state
## (column map hash, surface height and physics-ray height at the edit point) so the shell
## script can check that the game's visuals and collision follow the edit and the undo.
## Files live in tests/captures/live_edit/.

const DIR := "res://tests/captures/live_edit/"
const EDIT_AT := Vector3(4.0, 0.0, 6.0)

var game_seen := {}


func _ready() -> void:
	if Engine.is_editor_hint():
		if "live_edit=1" in OS.get_cmdline_user_args():
			_editor_run.call_deferred()
	elif FileAccess.file_exists(DIR + "active"):
		_game_run.call_deferred()


func _log(msg: String) -> void:
	print("live_edit: ", msg)


func _wait(sec: float) -> void:
	await get_tree().create_timer(sec).timeout


func _write(name: String, text: String) -> void:
	var f := FileAccess.open(DIR + name, FileAccess.WRITE)
	f.store_string(text)
	f.close()


func _wait_file(name: String, timeout: float) -> bool:
	var t := 0.0
	while not FileAccess.file_exists(DIR + name):
		await _wait(0.25)
		t += 0.25
		if t > timeout:
			_log("timed out waiting for " + name)
			return false
	return true


# ------------------------------------------------------------------ editor side

func _blobs(t) -> Dictionary:
	var d := {}
	for c in t.get_chunks():
		d[c] = t.get_chunk_data(c)
	return d


func _request(name: String) -> bool:
	_write("req_" + name, name)
	return await _wait_file("ack_" + name, 60.0)


func _editor_run() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DIR))
	for f in DirAccess.get_files_at(DIR):
		DirAccess.remove_absolute(DIR + f)
	await _wait(3.0)
	EditorInterface.open_scene_from_path(ProjectSettings.get_setting("application/run/main_scene"))
	await _wait(3.0)
	var scene := EditorInterface.get_edited_scene_root()
	var t = scene.get_node_or_null("MarchingSquaresTerrain") if scene else null
	if t == null:
		_log("FAIL no terrain in the edited scene")
		get_tree().quit(1)
		return
	_write("active", "1")
	EditorInterface.play_main_scene()
	if not await _wait_file("game_ready", 180.0):
		get_tree().quit(1)
		return
	await _wait(2.0)
	await _request("before")
	# One brush stroke, recorded like MarchingSquaresTerrainPlugin::finish_stroke / commit_undo.
	var before := _blobs(t)
	t.apply_brush("raise", EDIT_AT, 6.0, 7.5, 0)
	var after := _blobs(t)
	var undo := {}
	var redo := {}
	for c in before:
		if before[c] != after.get(c):
			undo[c] = before[c]
			redo[c] = after[c]
	_log("stroke changed %d chunks" % redo.size())
	var ur := EditorInterface.get_editor_undo_redo()
	ur.create_action("terrain brush (live edit check)")
	ur.add_do_method(t, "set_chunks_data", redo)
	ur.add_undo_method(t, "set_chunks_data", undo)
	ur.commit_action(false)
	await _wait(2.0)
	await _request("after")
	ur.get_history_undo_redo(ur.get_object_history_id(t)).undo()
	await _wait(2.0)
	await _request("undone")
	ur.get_history_undo_redo(ur.get_object_history_id(t)).redo()
	await _wait(2.0)
	await _request("redone")
	# Leave the scene as it was (the data resource is not saved: no files change).
	ur.get_history_undo_redo(ur.get_object_history_id(t)).undo()
	EditorInterface.stop_playing_scene()
	DirAccess.remove_absolute(DIR + "active")
	await _wait(1.0)
	_log("done")
	get_tree().quit(0)


# ------------------------------------------------------------------ game side

func _game_run() -> void:
	await _wait(4.0)
	_write("game_ready", "1")
	while FileAccess.file_exists(DIR + "active"):
		for f in DirAccess.get_files_at(DIR):
			if f.begins_with("req_") and not game_seen.has(f):
				game_seen[f] = true
				await _game_capture(f.trim_prefix("req_"))
		await _wait(0.2)


func _game_capture(name: String) -> void:
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	img.save_png(DIR + "live_" + name + ".png")
	var t = get_tree().current_scene.get_node_or_null("MarchingSquaresTerrain")
	var stats := {}
	if t:
		var map: Image = t.get_column_map()
		stats["column_map"] = hash(map.get_data()) if map else 0
		stats["height"] = t.get_height_at(EDIT_AT, -999.0)
		var space: PhysicsDirectSpaceState3D = t.get_world_3d().direct_space_state
		var q := PhysicsRayQueryParameters3D.create(EDIT_AT + Vector3(0, 200, 0), EDIT_AT - Vector3(0, 200, 0), t.collision_layer)
		var hit := space.intersect_ray(q)
		stats["collision"] = hit.position.y if hit else -999.0
		var rect: Rect2i = t.get_column_map_rect()
		var cell: Vector2i = t.world_to_cell(EDIT_AT)
		var p := cell - rect.position
		stats["map_height"] = map.get_pixel(p.x * 6, p.y).r if map and rect.has_point(cell) else -999.0
	_write("ack_" + name, JSON.stringify(stats))
	print("live_edit game: ", name, " ", stats)
