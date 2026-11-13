# Captures the weather / water / birds / smoke effects on a demo scene (default: the hex island):
#   godot --path . --script res://tests/weather_capture.gd -- name=r7_rain weather=Rain [frames=90]
#     [strike=N] (lightning N frames before the capture) [ortho=26] [yaw=45] [pitch=40]
#     [target=x,y,z] [time=0.4] [scene=res://demo/hex_island.tscn] [land] (birds perch)
#     [burst] (dust puff ring at the target) [fire] (smoke node gets flames) [probe=x,z;x,z]
extends SceneTree

var frames := 0
var args := {}
var target := 90
var inst: Node

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	target = int(args.get("frames", "90"))
	var ps: PackedScene = load(args.get("scene", "res://demo/hex_island.tscn"))
	inst = ps.instantiate()
	var cam = inst.get_node_or_null("Camera")
	if args.has("time"):
		var dn := inst.get_node_or_null("PixelDayNight")
		if dn:
			dn.time_override = float(args["time"])
	if cam:
		if args.has("ortho"):
			cam.ortho_size = float(args["ortho"])
		if args.has("yaw"):
			cam.yaw_deg = float(args["yaw"])
		if args.has("pitch"):
			cam.pitch_deg = float(args["pitch"])
		if args.has("target"):
			var xz := (args["target"] as String).split(",")
			cam.target = Vector3(float(xz[0]), float(xz[1]), float(xz[2]))
	if args.has("debug"):
		inst.get_node("PixelRenderer").debug_view = int(args["debug"])
	# The player rig would take over the camera; captures want the orbit camera.
	var rig := inst.get_node_or_null("PlayerRig")
	if rig:
		rig.queue_free()
	root.add_child.call_deferred(inst)

func _process(_d: float) -> bool:
	frames += 1
	var w := inst.get_node_or_null("Weather")
	if frames == 2:
		if args.has("probe"):
			var t := inst.get_node("MarchingSquaresTerrain")
			for pair in (args["probe"] as String).split(";"):
				var xz := pair.split(",")
				var p := Vector3(float(xz[0]), 0, float(xz[1]))
				print("PROBE ", p, " h=", t.get_height_at(p, -99.0))
		if w and args.has("weather"):
			w.set_weather_instant(args["weather"])
		var birds := inst.get_node_or_null("Birds")
		if birds and args.has("land"):
			birds.perch_seconds = 999.0
			birds.scare_distance = 0.0
			birds.land_now()
		var smoke := inst.get_node_or_null("Smoke")
		if smoke and args.has("fire"):
			smoke.fire = true
	if args.has("burst") and frames == target - 8:
		var smoke2 := inst.get_node_or_null("Smoke")
		var cam = inst.get_node("Camera")
		if smoke2:
			smoke2.burst(cam.target, 14)
	if args.has("follow_birds") and frames > 2:
		var ps: PackedVector3Array = inst.get_node("Birds").get_bird_positions()
		var c := Vector3.ZERO
		for p in ps:
			c += p
		inst.get_node("Camera").target = c / max(ps.size(), 1)
	if w and args.has("strike") and frames == target - int(args["strike"]):
		w.strike_lightning()
	if frames == target:
		var name: String = args.get("name", "weather")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		var sm := inst.get_node_or_null("Smoke")
		print("CAPTURE ", name, " weather=", w.get_weather_name() if w else "-", " wet=", w.get_wetness() if w else 0.0, " puffs=", sm.get_puff_count() if sm else 0, " fps=", Engine.get_frames_per_second())
		quit()
	return false
