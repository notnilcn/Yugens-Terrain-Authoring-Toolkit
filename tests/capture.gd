# Runs a scene for a few frames and saves a screenshot:
#   godot --path . --script res://tests/capture.gd -- scene=res://demo/demo.tscn name=demo [frames=40] [time=0.4] [ortho=26] [yaw=45] [target=x,y,z] [nograss] [noplayer] [norenderer] [diag] [node.<Name>.<prop>=<value>] [tmat.<param>=<value>]
# noplayer removes the demo PlayerRig so the scene's PixelOrbitCamera frames the shot.
extends SceneTree

var frames := 0
var args := {}
var target := 40

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	target = int(args.get("frames", "40"))
	var ps: PackedScene = load(args.get("scene", "res://demo/demo.tscn"))
	var inst := ps.instantiate()
	if args.has("time"):
		var dn := inst.get_node_or_null("PixelDayNight")
		if dn:
			dn.time_of_day = float(args["time"])
	if args.has("noplayer") and inst.has_node("PlayerRig"):
		var rig := inst.get_node("PlayerRig")
		inst.remove_child(rig)
		rig.free()
	if args.has("norenderer") and inst.has_node("PixelRenderer"):
		# Plain (non-pixel) rendering of the same scene, for comparisons.
		var r := inst.get_node("PixelRenderer")
		inst.remove_child(r)
		r.free()
	if args.has("ortho"):
		inst.get_node("Camera").ortho_size = float(args["ortho"])
	if args.has("yaw"):
		inst.get_node("Camera").yaw_deg = float(args["yaw"])
	if args.has("nograss"):
		inst.get_node("MarchingSquaresTerrain").grass_enabled = false
	if args.has("target"):
		var xz := (args["target"] as String).split(",")
		inst.get_node("Camera").target = Vector3(float(xz[0]), float(xz[1]), float(xz[2]))
	for k in args:
		# node.<NodeName>.<property>=<value>, e.g. node.PixelRenderer.depth_threshold=1.0
		if (k as String).begins_with("node."):
			var parts := (k as String).split(".")
			inst.get_node(parts[1]).set(parts[2], str_to_var(args[k]))
		# tmat.<param>=<value> sets a terrain material shader parameter.
		if (k as String).begins_with("tmat."):
			var m: ShaderMaterial = inst.get_node("MarchingSquaresTerrain").terrain_material
			m.set_shader_parameter((k as String).trim_prefix("tmat."), str_to_var(args[k]))
	if args.has("debug"):
		inst.get_node("PixelRenderer").debug_view = int(args["debug"])
	if args.has("noplayer"):
		# Scenes with a player rig: drop it so the scene's own orbit camera renders.
		var rig := inst.get_node_or_null("PlayerRig")
		if rig:
			inst.remove_child(rig)
			rig.free()
	root.add_child.call_deferred(inst)
	if args.has("diag"):
		(func(): inst.get_node("PixelDiagnostics").visible = true).call_deferred()

func _process(_d: float) -> bool:
	frames += 1
	if frames == target:
		var name: String = args.get("name", "capture")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		print("CAPTURE ", name)
		quit()
	return false
