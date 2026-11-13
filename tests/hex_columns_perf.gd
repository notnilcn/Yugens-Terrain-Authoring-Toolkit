# Meshed vs GPU-instanced HexPrism columns on the hex island: build time, frame time, draw
# calls, primitives, video memory. Run windowed (a GPU / llvmpipe renderer is needed):
#   godot --path . --script res://tests/hex_columns_perf.gd -- [instanced=1] [frames=240] [edit=1]
# Prints one "PERF ..." line. `edit=1` also times 20 single-cell height edits (remesh vs map
# upload + collision soup).
extends SceneTree

var args := {}
var frames := 0
var inst: Node
var t
var t0 := 0
var built_usec := 0
var times: Array[float] = []
var last := 0
var calls := 0
var prims := 0
var proc_ms := 0.0
var measure := 240


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	measure = int(args.get("frames", "240"))
	if args.has("noplayer"):
		pass
	var ps: PackedScene = load("res://demo/hex_island.tscn")
	inst = ps.instantiate()
	t = inst.find_child("MarchingSquaresTerrain", true, false)
	t.hex_edge_instanced = args.get("instanced", "0") == "1"
	t0 = Time.get_ticks_usec()
	root.add_child(inst)
	built_usec = Time.get_ticks_usec() - t0


func _process(_d: float) -> bool:
	frames += 1
	var now := Time.get_ticks_usec()
	if frames > 60 and frames <= 60 + measure:
		times.append((now - last) / 1000.0)
		calls += RenderingServer.get_rendering_info(RenderingServer.RENDERING_INFO_TOTAL_DRAW_CALLS_IN_FRAME)
		prims += RenderingServer.get_rendering_info(RenderingServer.RENDERING_INFO_TOTAL_PRIMITIVES_IN_FRAME)
		proc_ms += Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0
	last = now
	if frames == 61 + measure:
		times.sort()
		var avg := 0.0
		for x in times:
			avg += x
		avg /= times.size()
		var edit_ms := -1.0
		if args.get("edit", "0") == "1":
			var e0 := Time.get_ticks_usec()
			for i in 20:
				var c := Vector2i(i - 10, 3)
				t.set_cell_height(c, t.get_cell_height(c) + 2.5)
				# Flush the edit now (the terrain's process() runs update()).
				t.notification(Node.NOTIFICATION_PROCESS)
			edit_ms = (Time.get_ticks_usec() - e0) / 1000.0 / 20.0
		print("PERF instanced=%s build_ms=%.0f frame_avg_ms=%.2f frame_p50_ms=%.2f frame_p95_ms=%.2f draw_calls=%d primitives=%d video_mb=%.1f columns=%d edit_ms=%.1f process_ms=%.2f" % [
			args.get("instanced", "0"), built_usec / 1000.0, avg, times[times.size() / 2], times[int(times.size() * 0.95)],
			calls / measure, prims / measure,
			RenderingServer.get_rendering_info(RenderingServer.RENDERING_INFO_VIDEO_MEM_USED) / 1048576.0,
			t.get_column_instance_count(), edit_ms, proc_ms / measure])
		quit()
	return false
