# Headless editor stroke-cost probe. Installed as a temporary @tool autoload by
# tests/stroke_cost.sh; inert unless the user arg `stroke_cost` is present.
#   MST_PROF=1 tests/stroke_cost.sh [scene=res://demo/hex_island.tscn] [ops=level,raise,smooth]
#       [frames=60] [events=2] [radius=6] [max_ms=15]
#       [tools=Level,Brush,Smooth]
# Opens the scene in the (headless) editor, so the terrain runs its editor code paths
# (throttled height publishing, deferred prop / terrain colliders), and sculpts:
# * `ops`: strokes across the island through `apply_brush`, `events` brush applications per
#   frame, then the frame runs the terrain's `_process` (which rebuilds what the edits dirtied,
#   once per frame);
# * `tools`: real strokes with the MST tools (mouse events sent to the 3D view's input surface,
#   like tests/live_edit_ui_check.gd), one motion event per frame for `frames` frames (Brush:
#   drags a selection up and down).
# Prints one line per op / tool:
#   STROKE_COST {"op" | "tool": ..., "event_ms": <edit / input event>, "frame_ms": <the rest of
#                the frame, idle editor frame subtracted>,
#                "per_event_ms": (event_ms * events + frame_ms) / events}
# and "STROKE_COST OK" / "STROKE_COST FAIL" (per_event_ms above `max_ms`). The edits are never
# saved: tests/stroke_cost.sh restores demo/ afterwards.
@tool
extends Node

var args := {}


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if not Engine.is_editor_hint() or not args.has("stroke_cost"):
		return
	_run.call_deferred()


func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame


func _run() -> void:
	OS.low_processor_usage_mode = false
	# Headless frames sleep low_processor_usage_mode_sleep_usec (6.9 ms) when nothing draws.
	Engine.max_fps = 0
	OS.low_processor_usage_mode_sleep_usec = 0
	var p := "res://tests/editor_ready.gd"
	if not FileAccess.file_exists(p):
		p = "res://__editor_ready.gd"
	await _frames(30)
	await load(p).wait(get_tree())
	EditorInterface.open_scene_from_path(args.get("scene", "res://demo/hex_island.tscn"))
	await _frames(60)
	await load(p).wait(get_tree())
	var root := EditorInterface.get_edited_scene_root()
	var terrains := root.find_children("*", "MarchingSquaresTerrain", true, false) if root else []
	if terrains.is_empty():
		printerr("STROKE_COST: no terrain")
		get_tree().quit(1)
		return
	var t: Node3D = terrains[0]
	EditorInterface.get_selection().add_node(t)
	await _frames(30)
	# A line through the middle of the island (between the extreme chunks along x).
	var chunks: Array = t.get_chunks()
	var lo: Vector3 = t.chunk_to_world(chunks[0])
	var hi: Vector3 = lo
	for c in chunks:
		var w: Vector3 = t.chunk_to_world(c)
		if w.x < lo.x:
			lo = w
		if w.x > hi.x:
			hi = w
	var mid := (lo + hi) * 0.5
	var a := mid.lerp(lo, 0.4)
	var b := mid.lerp(hi, 0.4)
	var n := int(args.get("frames", "60"))
	var events := maxi(1, int(args.get("events", "2")))
	var radius := float(args.get("radius", "6"))
	var max_ms := float(args.get("max_ms", "15"))
	var worst := 0.0
	# Idle editor frame (nothing edited): subtracted from the stroke frames.
	await _frames(10)
	var idle_t := Time.get_ticks_usec()
	await _frames(n)
	var idle_ms := (Time.get_ticks_usec() - idle_t) / 1000.0 / n
	for op in (args.get("ops", "level,raise,smooth") as String).split(",", false):
		var amount := 0.0
		match op:
			"level":
				amount = t.get_height_at(a, 0.0) + 1.0
			"raise", "lower":
				amount = 0.05
			"smooth":
				amount = 0.3
		await _frames(10)
		var edit_us := 0
		var frame_us := 0
		var last := Time.get_ticks_usec()
		for i in n:
			for e in events:
				var k := float(i * events + e) / float(n * events)
				var at := a.lerp(b, 0.5 - 0.5 * cos(k * TAU))
				var t0 := Time.get_ticks_usec()
				t.apply_brush(op, at, radius, amount, 0)
				edit_us += Time.get_ticks_usec() - t0
			var t1 := Time.get_ticks_usec()
			await get_tree().process_frame
			# The frame minus the edits: the terrain's update plus the (idle) rest of the editor.
			frame_us += Time.get_ticks_usec() - t1
		var event_ms := edit_us / 1000.0 / (n * events)
		var frame_ms := maxf(0.0, frame_us / 1000.0 / n - idle_ms)
		var per_event := (event_ms * events + frame_ms) / events
		worst = maxf(worst, per_event)
		print("STROKE_COST ", JSON.stringify({"op": op, "events": events, "event_ms": snappedf(event_ms, 0.01), "frame_ms": snappedf(frame_ms, 0.01), "idle_frame_ms": snappedf(idle_ms, 0.01), "per_event_ms": snappedf(per_event, 0.01)}))
	worst = maxf(worst, await _tool_strokes(t, n, idle_ms))
	print("STROKE_COST OK" if worst <= max_ms else "STROKE_COST FAIL (%.1f ms per event > %.1f)" % [worst, max_ms])
	get_tree().quit(0 if worst <= max_ms else 1)


# ------------------------------------------------------------------ real tool strokes

var surface: Control
var mouse := Vector2.ZERO


func _send(ev: InputEventMouse) -> int:
	ev.global_position = surface.get_global_rect().position + ev.position
	ev.position = ev.global_position
	var t0 := Time.get_ticks_usec()
	surface.get_window().push_input(ev)
	return Time.get_ticks_usec() - t0


func _move(p: Vector2, held: bool) -> int:
	var e := InputEventMouseMotion.new()
	e.position = p
	e.relative = p - mouse
	e.button_mask = MOUSE_BUTTON_MASK_LEFT if held else 0
	mouse = p
	return _send(e)


func _button(p: Vector2, pressed: bool) -> void:
	var e := InputEventMouseButton.new()
	e.position = p
	e.button_index = MOUSE_BUTTON_LEFT
	e.pressed = pressed
	e.button_mask = MOUSE_BUTTON_MASK_LEFT if pressed else 0
	_send(e)
	await get_tree().process_frame


# Holds LMB from `a` and sweeps `a` -> `b` -> `a` over `n` frames (one motion event a frame).
# Returns [ms per motion event, ms per frame].
func _sweep(a: Vector2, b: Vector2, n: int) -> Array:
	_move(a, false)
	await get_tree().process_frame
	await _button(a, true)
	var ev_us := 0
	var fr_us := 0
	for i in n:
		var k := float(i + 1) / n
		ev_us += _move(a.lerp(b, 0.5 - 0.5 * cos(k * TAU)), true)
		var t1 := Time.get_ticks_usec()
		await get_tree().process_frame
		fr_us += Time.get_ticks_usec() - t1
	await _button(a, false)
	await _frames(30)
	return [ev_us / 1000.0 / n, fr_us / 1000.0 / n]


func _tool_strokes(t: Node, n: int, idle_ms: float) -> float:
	var names: PackedStringArray = (args.get("tools", "Level,Brush,Smooth") as String).split(",", false)
	if names.is_empty():
		return 0.0
	var plugin: Object = null
	var tb = EditorInterface.get_base_control().find_child("MarchingSquaresToolbar", true, false)
	if tb:
		for b in tb.find_children("*", "BaseButton", true, false):
			for con in b.get_signal_connection_list("pressed"):
				if con.callable.get_object() and con.callable.get_object().get_class() == "MarchingSquaresTerrainPlugin":
					plugin = con.callable.get_object()
	var vp := EditorInterface.get_editor_viewport_3d(0)
	for c in vp.get_parent().get_parent().get_children():
		if c is Control and not c.get_signal_connection_list("gui_input").is_empty():
			surface = c
	EditorInterface.set_main_screen_editor("3D")
	EditorInterface.edit_node(t)
	await _frames(30)
	if plugin == null or surface == null:
		printerr("STROKE_COST: MST plugin / 3D view input surface not found")
		return INF
	surface.grab_focus()
	var size := surface.size
	var c := size * 0.5
	var worst := 0.0
	for name in names:
		var before := _data_hash(t)
		var r: Array
		match name:
			"Level":
				plugin.call("on_tool", 1)
				await _frames(10)
				r = await _sweep(c - Vector2(size.x * 0.2, 0), c + Vector2(size.x * 0.2, 0), n)
			"Brush":
				plugin.call("on_tool", 0)
				await _frames(10)
				# A selection, then drag it up and down.
				await _sweep(c - Vector2(20, 0), c + Vector2(20, 0), 10)
				r = await _sweep(c, c + Vector2(0, -60), n)
			"Smooth":
				plugin.call("on_tool", 2)
				await _frames(10)
				r = await _sweep(c - Vector2(size.x * 0.2, 0), c + Vector2(size.x * 0.2, 10), n)
			_:
				printerr("STROKE_COST: unknown tool ", name)
				continue
		var frame_ms := maxf(0.0, r[1] - idle_ms)
		var per_event: float = r[0] + frame_ms
		if _data_hash(t) == before:
			printerr("STROKE_COST: the %s stroke changed nothing" % name)
			per_event = INF
		worst = maxf(worst, per_event)
		print("STROKE_COST ", JSON.stringify({"tool": name, "events": 1, "event_ms": snappedf(r[0], 0.01), "frame_ms": snappedf(frame_ms, 0.01), "per_event_ms": snappedf(per_event, 0.01)}))
	return worst


func _data_hash(t: Node) -> int:
	var h := 0
	for c in t.get_chunks():
		h = hash([h, t.get_chunk_data(c)])
	return h
