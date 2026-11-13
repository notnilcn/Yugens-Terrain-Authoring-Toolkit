# Flipbook particle fields / flipbook rain on a demo scene while the camera moves:
#   godot --path . --script res://tests/particles_capture.gd -- [scene=res://demo/hex_island.tscn]
#     [name=particles] [time=0.32] [weather=Clear] [preset=dust|rain|snow|embers|leaves|none]
#     [persp=<fov deg>] [dist=<m>] [ortho=<size>] [pitch=<deg>] [yaw=<deg>]
#     [shots=4] [gap=2] [move=x,y,z] [turn=<deg per shot>] [zoom=<factor per shot>]
#     [sheet=1] [player=1] [node.<Name>.<prop>=<value>] [field.<prop>=<value>]
# Removes the PlayerRig (kept with [player=1]: the knight's third-person camera renders) so the
# scene's PixelOrbitCamera frames the shot (smoothing off), then
# writes tests/captures/<name>_<i>.png every `gap` frames while the camera target moves by
# `move` (and the yaw by `turn`, the distance / ortho size by `zoom`) per shot, so consecutive
# shots show whether the particles stay anchored in the world. [sheet=1] also saves the
# field's flipbook sheet as tests/captures/<name>_sheet.png.
extends SceneTree

var args := {}
var inst: Node
var cam: Node
var field: Node
var frame := 0
var shot := 0
var warm := 50


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	inst = (load(args.get("scene", "res://demo/hex_island.tscn")) as PackedScene).instantiate()
	var rig := inst.get_node_or_null("PlayerRig")
	if rig and not args.has("player"):
		inst.remove_child(rig)
		rig.free()
	var dn := inst.get_node_or_null("PixelDayNight")
	if dn:
		dn.time_of_day = float(args.get("time", "0.32"))
		dn.set("day_length_seconds", 0.0)
	cam = inst.get_node("Camera")
	cam.smoothing = 0.0
	if args.has("persp"):
		cam.orthographic = false
		cam.fov = float(args["persp"])
		# Same framing as the ortho view: distance so that the view height matches ortho_size.
		var h: float = float(args.get("ortho", str(cam.ortho_size)))
		cam.distance = float(args.get("dist", str(h * 0.5 / tan(deg_to_rad(cam.fov) * 0.5))))
	elif args.has("ortho"):
		cam.ortho_size = float(args["ortho"])
	if args.has("pitch"):
		cam.pitch_deg = float(args["pitch"])
	if args.has("yaw"):
		cam.yaw_deg = float(args["yaw"])
	var preset: String = args.get("preset", "dust")
	var presets := {"dust": 0, "rain": 1, "snow": 2, "embers": 3, "leaves": 4}
	if presets.has(preset):
		field = ClassDB.instantiate("PixelParticleField")
		field.name = "Particles"
		inst.add_child(field)
		field.position = Vector3(0, 0, 0)
		field.set("preset", presets[preset])
	for k in args:
		if (k as String).begins_with("node."):
			var parts := (k as String).split(".")
			inst.get_node(parts[1]).set(parts[2], str_to_var(args[k]))
	root.add_child.call_deferred(inst)
	(func(): current_scene = inst).call_deferred()


func _process(_d: float) -> bool:
	frame += 1
	if frame == 2:
		var w := inst.get_node_or_null("Weather")
		if w:
			w.set_weather_instant(args.get("weather", "Clear"))
		if field:
			# Presets apply once the node is in the tree.
			var p: int = field.get("preset")
			field.set("preset", (p + 1) % 5)
			field.set("preset", p)
			for k in args:
				if (k as String).begins_with("field."):
					field.set((k as String).trim_prefix("field."), str_to_var(args[k]))
			if args.has("sheet"):
				field.call("save_flipbook_png", "res://tests/captures/%s_sheet.png" % args.get("name", "particles"))
	if frame < warm:
		return false
	var gap := int(args.get("gap", "2"))
	if (frame - warm) % gap != 0:
		return false
	var name: String = args.get("name", "particles")
	root.get_texture().get_image().save_png("res://tests/captures/%s_%d.png" % [name, shot])
	print("CAPTURE ", name, "_", shot, " target ", cam.target, " yaw ", cam.yaw_deg)
	shot += 1
	if shot >= int(args.get("shots", "4")):
		quit()
		return false
	if args.has("move"):
		var m := (args["move"] as String).split(",")
		cam.target += Vector3(float(m[0]), float(m[1]), float(m[2]))
	if args.has("turn"):
		cam.yaw_deg += float(args["turn"])
	if args.has("zoom"):
		var z := float(args["zoom"])
		if cam.orthographic:
			cam.ortho_size *= z
		else:
			cam.distance *= z
	return false
