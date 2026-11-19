# Editor brush performance probe. Installed as a temporary @tool autoload by
# tests/brush_perf.sh; inert unless the user arg `brush_perf` is present.
#   tests/brush_perf.sh [scene=res://demo/hex_island.tscn] [tools=Brush,Level,...] [frames=120]
# Selects the terrain, then for every tool hovers the 3D view for `frames` frames (the cursor
# sweeps across the view) and drags with LMB held for `frames` frames. Prints one line per tool:
#   BRUSH_PERF {"tool": ..., "hover_ms": ..., "stroke_ms": ...}
# (mean ms per frame, low-processor mode off). Run with MST_PROF=1 for a per-stage breakdown.
# Quits without saving: strokes never reach the scene's data on disk.
@tool
extends Node

var args := {}
var draw_t0 := 0
var draw_us := 0
var proc_ms := 0.0
var input_us := 0


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if not Engine.is_editor_hint() or not args.has("brush_perf"):
		return
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	RenderingServer.frame_pre_draw.connect(func(): draw_t0 = Time.get_ticks_usec())
	RenderingServer.frame_post_draw.connect(func(): draw_us += Time.get_ticks_usec() - draw_t0)
	_run.call_deferred()


func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame


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


func _view_rect() -> Rect2:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	var c := vp.get_parent() as Control if vp else null
	return c.get_global_rect() if c else Rect2(0, 0, 1280, 720)


func _find_button(root: Node, name: String) -> Button:
	for n in root.find_children(name, "Button", true, false):
		if n is Button and n.is_visible_in_tree():
			return n
	return null


## Mean ms per frame over `n` frames while the cursor sweeps a band of the view; also
## `last_split`: mean draw (frame_pre_draw..post_draw) and process (TIME_PROCESS) ms.
var last_split := {}
func _sweep(n: int, mask: int) -> float:
	draw_us = 0
	proc_ms = 0.0
	input_us = 0
	var r := _view_rect()
	var a := r.position + r.size * Vector2(0.35, 0.5)
	var b := r.position + r.size * Vector2(0.65, 0.55)
	var prev := a
	var t0 := Time.get_ticks_usec()
	for i in n:
		var k := float(i) / n
		var p := a.lerp(b, 0.5 - 0.5 * cos(k * TAU * 2.0))
		var ti := Time.get_ticks_usec()
		_motion(p, p - prev, mask)
		input_us += Time.get_ticks_usec() - ti
		prev = p
		await get_tree().process_frame
		proc_ms += Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0
	last_split = {"draw": snappedf(draw_us / 1000.0 / n, 0.01), "process": snappedf(proc_ms / n, 0.01), "input": snappedf(input_us / 1000.0 / n, 0.01)}
	return (Time.get_ticks_usec() - t0) / 1000.0 / n



## Waits for the editor's filesystem scan / import progress / modal dialogs (tests/editor_ready.gd).
func _editor_ready() -> void:
	var p := "res://tests/editor_ready.gd"
	if not FileAccess.file_exists(p):
		p = "res://__editor_ready.gd"
	await load(p).wait(get_tree())


func _run() -> void:
	OS.low_processor_usage_mode = false
	var scene: String = args.get("scene", "res://demo/hex_island.tscn")
	await _frames(30)
	await _editor_ready()
	EditorInterface.open_scene_from_path(scene)
	await _frames(60)
	await _editor_ready()
	EditorInterface.set_main_screen_editor("3D")
	await _frames(20)
	var root := EditorInterface.get_edited_scene_root()
	var terrains := root.find_children("*", "MarchingSquaresTerrain", true, false) if root else []
	if terrains.is_empty():
		printerr("BRUSH_PERF: no terrain")
		get_tree().quit(1)
		return
	var missing := 0
	var n := int(args.get("frames", "120"))
	var r := _view_rect()
	# Baseline: nothing selected.
	EditorInterface.get_selection().clear()
	await _frames(30)
	var base_ms := await _sweep(n, 0)
	print("BRUSH_PERF ", JSON.stringify({"tool": "unselected", "hover_ms": snappedf(base_ms, 0.01), "hover_split": last_split}))
	EditorInterface.get_selection().add_node(terrains[0])
	EditorInterface.edit_node(terrains[0])
	await _frames(60)
	var base := EditorInterface.get_base_control()
	var tools: PackedStringArray = args.get("tools", "Brush,Level,Smooth,Foliage,Vertex Painting").split(",", false)
	for t in tools:
		var btn := _find_button(base, t)
		if btn == null:
			printerr("BRUSH_PERF: no tool button ", t)
			missing += 1
			continue
		btn.button_pressed = true
		btn.pressed.emit()
		await _frames(20)
		var hover := await _sweep(n, 0)
		var hover_split := last_split
		var at := r.position + r.size * Vector2(0.35, 0.5)
		_motion(at, Vector2.ZERO)
		_button(at, MOUSE_BUTTON_LEFT, true, MOUSE_BUTTON_MASK_LEFT)
		await _frames(1)
		var stroke := await _sweep(n, MOUSE_BUTTON_MASK_LEFT)
		_button(at, MOUSE_BUTTON_LEFT, false)
		await _frames(10)
		print("BRUSH_PERF ", JSON.stringify({"tool": t, "hover_ms": snappedf(hover, 0.01), "stroke_ms": snappedf(stroke, 0.01), "hover_split": hover_split, "stroke_split": last_split}))
	get_tree().quit(1 if missing else 0)
