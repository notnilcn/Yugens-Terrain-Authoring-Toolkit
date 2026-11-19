# Headless check of what a running game shows after live edit / undo: plays the demo scene
# (game, not editor: distant LOD on, the player as LOD focus, props, grass, water, fire, the
# pixel renderer), edits its terrain like the editor brushes do, then applies the undo / redo
# blobs through `set_chunks_data` (the call an editor undo action makes and Godot's live edit
# forwards to the game). After each step every node the terrain draws (chunk meshes, collision,
# grass, instanced columns, LOD, scatter props, wild flowers) and its CPU-side maps (column map,
# published heights) must match a fresh terrain built from the same data in the same scene.
#   godot --headless --path . --script res://tests/live_edit_game_check.gd
# Prints "LIVE EDIT GAME OK" or the mismatches (exit code 1).
extends SceneTree

const FP := preload("res://tests/terrain_fp.gd")

var fails := 0
var checks := 0
var scene: Node
var t


func _initialize() -> void:
	_run.call_deferred()


func _wait(sec: float) -> void:
	await create_timer(sec).timeout


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


# Compares what t draws now with t rebuilt from scratch (`rebuild`: reloads the data resource,
# frees every chunk node and meshes everything again, in the same scene with the same LOD
# focus, water and fire). Undo / live edit write the data resource like a brush stroke does.
func _compare_fresh(what: String) -> void:
	checks += 1
	await _wait(0.3)
	var a := FP.fingerprint(t)
	var blobs := FP.blobs(t)
	t.rebuild()
	await _wait(0.6)
	_check(FP.blobs(t) == blobs, "%s: the data resource does not hold what the terrain shows" % what)
	var b := FP.fingerprint(t)
	var bad := FP.mismatches(a, b)
	_check(bad.is_empty(), "%s: the game's terrain differs from a fresh build in %d entries: %s" % [what, bad.size(), bad.slice(0, 8)])
	for k in bad.slice(0, 4):
		printerr("   ", k, ": ", FP.first_diff(a.get(k), b.get(k)), " (fresh second)")


func _run() -> void:
	var packed: PackedScene = load(ProjectSettings.get_setting("application/run/main_scene"))
	scene = packed.instantiate()
	root.add_child(scene)
	current_scene = scene
	t = scene.get_node("MarchingSquaresTerrain")
	await _wait(1.5)
	var player: Node3D = t.get_lod_focus()
	var at: Vector3 = player.global_position if player else t.chunk_to_world(t.get_chunks()[0])
	var far: Vector3 = t.chunk_to_world(t.get_chunks()[t.get_chunks().size() - 1])
	await _compare_fresh("initial")
	var history := []
	for s in [["raise", at, 5.0, 4.0], ["paint", at + Vector3(3, 0, 0), 4.0, 3.0], ["grass_off", at, 3.0, 0.0], ["lower", far, 6.0, 2.5], ["level", at - Vector3(4, 0, 2), 4.0, 0.5]]:
		var before := FP.blobs(t)
		t.apply_brush(s[0], s[1], s[2], s[3], 0)
		var d := FP.diff(before, FP.blobs(t))
		_check(not d.before.is_empty(), "%s changed nothing" % s[0])
		d["name"] = s[0]
		history.append(d)
		t.commit_to_data()  # like finish_stroke's write_chunks
		await _wait(0.5)
	await _compare_fresh("after edits")
	for i in range(history.size() - 1, -1, -1):
		t.set_chunks_data(history[i].before)
		await _wait(0.3)
		await _compare_fresh("undo " + history[i].name)
	for i in history.size():
		t.set_chunks_data(history[i].after)
		await _wait(0.3)
	await _compare_fresh("redo all")
	print("LIVE EDIT GAME OK (%d comparisons)" % checks if fails == 0 else "LIVE EDIT GAME: %d failure(s)" % fails)
	quit(1 if fails else 0)
