# Optional snow on the hex island: a timed sequence of shots while snow starts, builds up and melts.
#   godot --path . --script res://tests/snow_capture.gd -- name=snow [persp] [dist=N] [ortho=N]
#     [time=0.42] [on=F] [off=F] [shots=F,F,...] [weather=Snow] [node.<Name>.<prop>=<value>]
# Shots are written to tests/captures/<name>_<frame>.png. [on=F] sets Weather.snow_enabled at
# frame F (default 5), [off=F] clears it (no default: snow keeps falling). [weather=Name] switches
# the weather instantly at frame 2 instead (e.g. Snow). [persp] keeps the demo player rig (low-FOV
# perspective camera, [dist=N] camera distance), otherwise the scene's ortho orbit camera
# ([ortho=N] size, [yaw=N], [pitch=N]). [none] never turns snow on (default look check).
# Prints SNOW lines with the cover and visible flake count per shot.
extends SceneTree

var frame := 0
var args := {}
var inst: Node
var shots: Array = []

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	for s in (args.get("shots", "60,240") as String).split(","):
		shots.append(int(s))
	inst = (load(args.get("scene", "res://demo/hex_island.tscn")) as PackedScene).instantiate()
	if args.has("time"):
		inst.get_node("PixelDayNight").time_override = float(args["time"])
	var cam = inst.get_node_or_null("Camera")
	if cam:
		if args.has("ortho"):
			cam.ortho_size = float(args["ortho"])
		if args.has("yaw"):
			cam.yaw_deg = float(args["yaw"])
		if args.has("pitch"):
			cam.pitch_deg = float(args["pitch"])
	if not args.has("persp"):
		var rig := inst.get_node_or_null("PlayerRig")
		if rig:
			inst.remove_child(rig)
			rig.free()
	for k in args:
		if (k as String).begins_with("node."):
			var parts := (k as String).split(".")
			inst.get_node(parts[1]).set(parts[2], str_to_var(args[k]))
	root.add_child.call_deferred(inst)

func _process(_d: float) -> bool:
	frame += 1
	var w := inst.get_node_or_null("Weather")
	if frame == 2:
		var player := inst.find_child("Player", true, false)
		if player and args.has("dist"):
			player.set("distance", float(args["dist"]))
		if w and args.has("weather"):
			w.set_weather_instant(args["weather"])
	if w and not args.has("none") and not args.has("weather") and frame == int(args.get("on", "5")):
		w.snow_enabled = true
	if w and args.has("off") and frame == int(args["off"]):
		w.snow_enabled = false
		if args.has("weather"):
			w.set_weather("Clear")
	if frame in shots:
		var name := "%s_%d" % [args.get("name", "snow"), frame]
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		var snow_mm: MultiMeshInstance3D = null
		if w:
			for c in w.get_children(true):
				if c.name == "Snow":
					snow_mm = c
		print("SNOW %s cover=%.3f flakes=%d wet=%.2f fps=%d" % [name, w.get_snow_cover() if w else 0.0,
			snow_mm.multimesh.visible_instance_count if snow_mm and snow_mm.visible else 0,
			w.get_wetness() if w else 0.0, Engine.get_frames_per_second()])
		if frame == shots.max():
			quit()
	return false
