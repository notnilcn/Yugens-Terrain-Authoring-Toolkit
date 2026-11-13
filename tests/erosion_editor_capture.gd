# Interactive-editor check of the background MarchingSquaresErosion run (installed as a
# temporary @tool autoload by tests/erosion_editor_capture.sh; inert unless the user arg
# `erosion_editor_capture` is present). Nothing is saved.
#   tests/erosion_editor_capture.sh <display> [name=erosion_editor] [select=Erosion] [iterations=1500000] [pixels_per_cell=8]
# 1. opens demo/hex_island.tscn, adds an `Erosion` child to the terrain when there is none and
#    selects it by node name (the pixel UI shows the Erosion strip);
# 2. presses the strip's Erode button, watches `get_progress()` / `erosion_progress`, the
#    strip's progress row and the disabled Erode button (<name>_running.png);
# 3. on `erosion_finished(true)` checks the heights changed and exactly one undo action was
#    added (<name>_done.png), undoes it and checks the heights are restored (<name>_undone.png);
# 4. presses Erode again, then the row's Cancel: `erosion_finished(false)`, no height change,
#    no undo action.
# Prints EROSION_EDITOR PASS / FAIL and quits with 0 / 1.
@tool
extends Node

var args := {}
var frame := 0
var scene_path := "res://demo/hex_island.tscn"
var scene_ready := false
var view: Camera3D
var terrain: Node
var erosion: Node
var history: UndoRedo
var snapshot := {}
var version0 := 0
var phase := "wait"
var phase_frame := 0
var problems: Array[String] = []
var progress_signals := 0
var max_progress := -1.0
var finished: Array = []
var shot_running := false

func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if not Engine.is_editor_hint() or not args.has("erosion_editor_capture"):
		set_process(false)
		return
	OS.low_processor_usage_mode = false
	print("EROSION_EDITOR start")

func _check(ok: bool, what: String) -> void:
	print("EROSION_EDITOR ", "ok   " if ok else "FAIL ", what)
	if not ok:
		problems.append(what)

func _find(root: Node, n: String) -> Node:
	return root.find_child(n, true, false) if root else null

func _strip() -> Node:
	return EditorInterface.get_base_control().find_child("PixelizerAttributes", true, false)

func _shot(suffix: String) -> void:
	var name: String = args.get("name", "erosion_editor")
	var path := ProjectSettings.globalize_path("res://tests/captures/%s_%s.png" % [name, suffix])
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	get_viewport().get_texture().get_image().save_png(path)
	print("EROSION_EDITOR capture ", path)

func _heights() -> Dictionary:
	var d := {}
	for c in terrain.get_chunks():
		d[c] = terrain.get_chunk_heights(c)
	return d

func _same(a: Dictionary, b: Dictionary) -> bool:
	if a.size() != b.size():
		return false
	for k in a:
		if not b.has(k) or a[k] != b[k]:
			return false
	return true

## Progress row of the strip: (row control, its strip cell visible?, bar value).
func _row() -> Array:
	var row: Control = _find(_strip(), "Eroding")
	if row == null:
		return [null, false, -1.0]
	var cell: Control = row.get_parent()
	var bar: ProgressBar = row.get_node_or_null("Bar")
	return [row, cell.visible, bar.value if bar else -1.0]

func _erode_button() -> Button:
	return _find(_strip(), "Erode") as Button

func _on_progress(p: float) -> void:
	progress_signals += 1
	# Calling back into the node from its own signal must work (no re-entrant bind panic).
	max_progress = max(max_progress, erosion.get_progress())

func _on_finished(applied: bool) -> void:
	finished.append(applied)
	print("EROSION_EDITOR finished applied=", applied, " running=", erosion.is_running(), " at frame ", frame)

func _go(p: String) -> void:
	phase = p
	phase_frame = frame
	print("EROSION_EDITOR phase ", p, " at frame ", frame)

func _done() -> void:
	if problems.is_empty():
		print("EROSION_EDITOR PASS")
	else:
		printerr("EROSION_EDITOR FAIL: ", ", ".join(problems))
	get_tree().quit(0 if problems.is_empty() else 1)

func _process(_d: float) -> void:
	frame += 1
	var edited := EditorInterface.get_edited_scene_root()
	if not scene_ready:
		if edited != null and edited.scene_file_path == scene_path and not EditorInterface.get_resource_filesystem().is_scanning():
			scene_ready = true
			print("EROSION_EDITOR scene ready at frame ", frame)
			frame = 59
		elif frame >= 20 and frame % 40 == 20:
			EditorInterface.open_scene_from_path(scene_path)
		if frame > 6000:
			printerr("EROSION_EDITOR: scene never opened")
			get_tree().quit(1)
		return
	if frame > 60 and view:
		var ec := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		ec.global_transform = view.global_transform
		ec.fov = view.fov
		if view.projection == Camera3D.PROJECTION_ORTHOGONAL:
			ec.projection = Camera3D.PROJECTION_ORTHOGONAL
			ec.size = view.size
	if frame == 60:
		EditorInterface.set_main_screen_editor("3D")
		if edited.has_node("Camera") and edited.get_node("Camera") is Camera3D:
			view = edited.get_node("Camera")
		terrain = edited.find_children("*", "MarchingSquaresTerrain", true, false)[0]
		var want: String = args.get("select", "Erosion")
		erosion = terrain.get_node_or_null(want)
		if erosion == null:
			erosion = ClassDB.instantiate("MarchingSquaresErosion")
			erosion.name = want
			terrain.add_child(erosion)
			erosion.owner = edited
			print("EROSION_EDITOR added ", edited.get_path_to(erosion))
		# Many droplets on a fine raster: a run of several seconds (progress can be watched)
		# with about the visible effect of the default 30000 droplets at 2 px / cell.
		erosion.iterations = int(args.get("iterations", "1500000"))
		erosion.pixels_per_cell = int(args.get("pixels_per_cell", "8"))
		erosion.erosion_progress.connect(_on_progress)
		erosion.erosion_finished.connect(_on_finished)
	elif frame == 70:
		# select= by node name, as tests/editor_capture.gd.
		var n := edited.find_child(args.get("select", "Erosion"), true, false)
		_check(n == erosion, "found the Erosion node by name")
		EditorInterface.edit_node(erosion)
	elif frame == 100:
		_check(_erode_button() != null, "the pixel UI strip shows the Erode button")
		var r := _row()
		_check(r[0] != null and not r[1], "the progress row exists and is hidden while idle")
		for f in ["Mode", "Level Step", "Min Plateau"]:
			_check(_find(_strip(), f) != null, "strip shows " + f)
		snapshot = _heights()
		var ur := EditorInterface.get_editor_undo_redo()
		history = ur.get_history_undo_redo(ur.get_object_history_id(terrain))
		version0 = history.get_version()
		_erode_button().pressed.emit()
		_check(erosion.is_running(), "Erode starts a background run")
		_go("running")
	elif phase == "running":
		var p: float = erosion.get_progress()
		if not shot_running and p >= 0.25 and frame - phase_frame > 12:
			var r := _row()
			_check(r[1] and r[2] > 0.0, "progress row visible with a value while running (%.2f)" % r[2])
			_check(_erode_button().disabled, "Erode is disabled while running")
			_shot("running")
			shot_running = true
		if not finished.is_empty():
			_check(finished[0] == true, "erosion_finished(true)")
			_check(progress_signals >= 5 and max_progress > 0.5, "progress reported (%d signals, max %.2f)" % [progress_signals, max_progress])
			_check(shot_running, "progress observed in the strip before completion")
			_go("finished")
		elif frame - phase_frame > 3000:
			_check(false, "erosion finished in time")
			_done()
	elif phase == "finished" and frame - phase_frame == 20:
		var h := _heights()
		var changed := 0
		for k in h:
			if h[k] != snapshot[k]:
				changed += 1
		_check(changed > 0, "erosion changed %d chunk(s)" % changed)
		_check(history.get_version() == version0 + 1, "exactly one undo action (version %d -> %d)" % [version0, history.get_version()])
		_check(history.get_current_action_name() == "terrain erosion", "undo action is 'terrain erosion' (%s)" % history.get_current_action_name())
		_check(not _row()[1] and not _erode_button().disabled, "progress row hidden and Erode enabled again")
		_check(erosion.get_progress() < 0.0 and not erosion.is_running(), "node idle again")
		_shot("done")
		history.undo()
		_go("undone")
	elif phase == "undone" and frame - phase_frame == 20:
		_check(_same(_heights(), snapshot), "one undo restores every height")
		_shot("undone")
		finished.clear()
		version0 = history.get_version()
		_erode_button().pressed.emit()
		_go("cancel")
	elif phase == "cancel":
		if frame - phase_frame == 15:
			var r := _row()
			var cancel: Button = r[0].get_node_or_null("Cancel") if r[0] else null
			_check(cancel != null and r[1], "Cancel button visible while running")
			if cancel:
				cancel.pressed.emit()
		if not finished.is_empty():
			_check(finished[0] == false, "cancel gives erosion_finished(false)")
			_go("cancelled")
		elif frame - phase_frame > 3000:
			_check(false, "cancel finished in time")
			_done()
	elif phase == "cancelled" and frame - phase_frame == 20:
		_check(_same(_heights(), snapshot), "cancel leaves the heights unchanged")
		_check(history.get_version() == version0, "cancel adds no undo action")
		_done()
