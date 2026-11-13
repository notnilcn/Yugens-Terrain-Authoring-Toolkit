# Frame-time probe: runs a scene, lets it settle, then samples frame / process / render times.
#   godot --path . --script res://tests/perf_probe.gd -- scene=res://demo/hex_island.tscn \
#       [warmup=120] [frames=300] [off=Birds,Weather] [node.<Name>.<prop>=<value>]
#       [feature.<key>=0|1] [features=0|1] [label=x]
# `feature.<key>` switches one project feature (PixelFeatures) for this run; `features=0|1`
# switches every feature first (e.g. `features=0` = a project that opted in to nothing).
# `off` removes top-level nodes by name before the scene enters the tree (feature ablation).
# Prints one `PERF {json}` line. Render times come from RenderingServer's per-viewport
# measurement (root viewport + every SubViewport, summed), in ms.
extends SceneTree

var args := {}
var frame := 0
var warmup := 120
var samples := 300
var last_us := 0
var frame_ms: Array[float] = []
var cpu_ms := 0.0
var gpu_ms := 0.0
var vps: Array[RID] = []
var bracket_us := 0
var t_start := 0


class Stamp extends Node:
	var cb: Callable
	func _process(_d: float) -> void:
		cb.call()

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	warmup = int(args.get("warmup", "120"))
	samples = int(args.get("frames", "300"))
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	Engine.max_fps = 0
	if args.has("features"):
		for key in PixelFeatures.get_feature_keys():
			PixelFeatures.set_enabled(key, args["features"] == "1")
	for k in args:
		if (k as String).begins_with("feature."):
			PixelFeatures.set_enabled((k as String).trim_prefix("feature."), args[k] == "1")
	var ps: PackedScene = load(args.get("scene", "res://demo/hex_island.tscn"))
	var inst := ps.instantiate()
	if args.has("off"):
		for n in (args["off"] as String).split(",", false):
			var node := inst.get_node_or_null(n)
			if node:
				node.get_parent().remove_child(node)
				node.free()
			else:
				push_warning("perf_probe: no node %s" % n)
	for k in args:
		if (k as String).begins_with("node."):
			var parts := (k as String).split(".")
			var target := inst.get_node_or_null(parts[1])
			if target:
				target.set(parts[2], str_to_var(args[k]))
	root.add_child.call_deferred(inst)
	# Bracket every node's _process: the first stamp runs before all nodes, the last after.
	var first := Stamp.new()
	first.process_priority = -2147483647
	first.cb = func(): t_start = Time.get_ticks_usec()
	var last := Stamp.new()
	last.process_priority = 2147483647
	last.cb = func(): bracket_us += Time.get_ticks_usec() - t_start
	root.add_child.call_deferred(first)
	root.add_child.call_deferred(last)

func _collect(n: Node) -> void:
	if n is Viewport:
		vps.append((n as Viewport).get_viewport_rid())
	for c in n.get_children(true):
		_collect(c)

func _process(_d: float) -> bool:
	frame += 1
	var now := Time.get_ticks_usec()
	if frame == warmup + 2:
		bracket_us = 0
	if frame == warmup:
		vps.clear()
		_collect(root)
		for v in vps:
			RenderingServer.viewport_set_measure_render_time(v, true)
	elif frame > warmup + 2 and frame <= warmup + 2 + samples:
		frame_ms.append((now - last_us) / 1000.0)
		for v in vps:
			cpu_ms += RenderingServer.viewport_get_measured_render_time_cpu(v)
			gpu_ms += RenderingServer.viewport_get_measured_render_time_gpu(v)
	elif frame > warmup + 2 + samples:
		var s := frame_ms.duplicate()
		s.sort()
		var total := 0.0
		for f in s:
			total += f
		var n := float(s.size())
		var out := {
			"label": args.get("label", args.get("off", "all")),
			"frame_avg": snappedf(total / n, 0.01),
			"frame_p95": snappedf(s[int(n * 0.95)], 0.01),
			"process": snappedf(bracket_us / 1000.0 / n, 0.01),
			"render_cpu": snappedf(cpu_ms / n, 0.01),
			"render_gpu": snappedf(gpu_ms / n, 0.01),
			"viewports": vps.size(),
			"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
			"objects": Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME),
			"nodes": Performance.get_monitor(Performance.OBJECT_NODE_COUNT),
		}
		print("PERF ", JSON.stringify(out))
		quit()
	last_us = now
	return false
