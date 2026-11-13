# Fire / rain / regrowth sequence on the hex island (PixelFireSystem "GrassFire" under the terrain):
#   godot --path . --script res://tests/fire_capture.gd -- name=fire [persp] [fov=15] [ortho=16]
#     [yaw=45] [pitch=40] [target=x,y,z] [at=x,z] [radius=1.0] [regrow=40] [check]
# Ignites the grass next to the campfire, lets it spread, starts rain (PixelWeather "Rain"),
# waits for the fire to go out, then clears the weather and fast-forwards the regrowth.
# Saves tests/captures/<name>_<stage>.png for: 0_before 1_ignite 2_spread 3_spread 4_rain
# 5_rain 6_out 7_regrow 8_regrown. `persp` uses a low-FOV perspective camera (the game's view).
# `check` only runs the API checks (no captures needed) and prints FIRE_CHECK OK / FAIL.
# Keys when run windowed without `name`: F = ignite at the camera target, R = toggle rain.
extends SceneTree

var frames := 0
var args := {}
var inst: Node
var fire: Node
var cam: Node
var weather: Node
var stage := 0
var stage_frame := 0
var events := {"started": 0, "out": 0, "ignited": 0, "extinguished": 0}
var at := Vector3(-2.5, 0.0, 6.0)

# (stage name, frames to wait before the capture, action at the start of the stage)
var plan := [
	["0_before", 20, ""],
	["1_ignite", 18, "ignite"],
	["2_spread", 90, ""],
	["3_spread", 90, ""],
	["4_rain", 45, "rain"],
	["5_rain", 60, ""],
	["6_out", 150, ""],
	["7_regrow", 40, "clear"],
	["8_regrown", 40, "grow"],
]

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if args.has("at"):
		var xz := (args["at"] as String).split(",")
		at = Vector3(float(xz[0]), 0.0, float(xz[1]))
	var ps: PackedScene = load(args.get("scene", "res://demo/hex_island.tscn"))
	inst = ps.instantiate()
	var rig := inst.get_node_or_null("PlayerRig")
	if rig:
		inst.remove_child(rig)
		rig.free()
	var dn := inst.get_node_or_null("PixelDayNight")
	if dn:
		dn.time_override = float(args.get("time", "0.4"))
	cam = inst.get_node("Camera")
	var t := inst.get_node("MarchingSquaresTerrain")
	fire = t.get_node("GrassFire")
	if args.has("name") or args.has("check"):
		fire.simulate = false
	weather = inst.get_node_or_null("Weather")
	fire.fire_started.connect(func(_p): events["started"] += 1)
	fire.fire_out.connect(func(): events["out"] += 1)
	fire.cell_ignited.connect(func(_c): events["ignited"] += 1)
	fire.cell_extinguished.connect(func(_c): events["extinguished"] += 1)
	cam.yaw_deg = float(args.get("yaw", "45"))
	cam.pitch_deg = float(args.get("pitch", "40"))
	if args.has("persp"):
		cam.orthographic = false
		cam.fov = float(args.get("fov", "15"))
		cam.distance = float(args.get("distance", "70"))
	else:
		cam.ortho_size = float(args.get("ortho", "16"))
	root.add_child.call_deferred(inst)

func _ready_target() -> void:
	var h: float = inst.get_node("MarchingSquaresTerrain").get_height_at(at, 9.0)
	at.y = h
	if args.has("target"):
		var xyz := (args["target"] as String).split(",")
		cam.target = Vector3(float(xyz[0]), float(xyz[1]), float(xyz[2]))
	else:
		cam.target = at + Vector3(1.5, 0.0, 1.0)

func _do(action: String) -> void:
	match action:
		"ignite":
			var n: int = fire.ignite(at, float(args.get("radius", "1.0")))
			print("FIRE ignite at ", at, " lit=", n)
		"rain":
			if weather:
				# The fire runs on a fixed step, the weather blend on real time: with `check`
				# switch instantly so the result does not depend on the frame rate.
				if args.has("check"):
					weather.set_weather_instant("Rain")
				else:
					weather.set_weather("Rain")
			fire.rain_override = -1.0
		"clear":
			if weather:
				weather.set_weather_instant("Clear")
			# Fast-forward into the regrowth (seconds of simulated time): half grown...
			fire.step(float(args.get("regrow", "40")) * 0.3)
		"grow":
			# ... then fully grown back.
			fire.step(float(args.get("regrow", "40")))

func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		_ready_target()
	if frames < 3:
		return false
	if not args.has("name") and not args.has("check"):
		return _interactive()
	if stage >= plan.size():
		_report()
		quit()
		return false
	var s: Array = plan[stage]
	if stage_frame == 0 and s[2] != "":
		_do(s[2])
	# Fixed simulated time per frame (one 10 Hz tick), so the sequence does not depend on fps.
	fire.step(0.1)
	stage_frame += 1
	if stage_frame >= int(s[1]):
		var name: String = args.get("name", "fire")
		print("STAGE ", s[0], " burning=", fire.get_burning_count(), " burnt=", snappedf(fire.get_burnt_fraction(), 0.001), " particles=", fire.get_particle_count(), " cells=", fire.get_burning_cells().size(), " wet=", snappedf(weather.get_wetness() if weather else 0.0, 0.01), " fps=", Engine.get_frames_per_second())
		if args.has("name"):
			root.get_texture().get_image().save_png("res://tests/captures/%s_%s.png" % [name, s[0]])
		stage += 1
		stage_frame = 0
	return false

func _report() -> void:
	var cell: Vector2i = inst.get_node("MarchingSquaresTerrain").world_to_cell(at)
	var st: Dictionary = fire.get_cell_state(cell)
	print("FIRE events ", events, " cell ", cell, " ", st)
	if args.has("check"):
		var ok: bool = events["started"] >= 1 and events["out"] >= 1 and events["ignited"] >= 3 and fire.get_burning_count() == 0 and st.size() > 0
		print("FIRE_CHECK ", "OK" if ok else "FAIL")

func _interactive() -> bool:
	if Input.is_key_pressed(KEY_F):
		fire.ignite(cam.target, 1.0)
	if Input.is_key_pressed(KEY_R) and weather and frames % 30 == 0:
		weather.set_weather("Clear" if weather.get_weather_name() == "Rain" else "Rain")
	return false
