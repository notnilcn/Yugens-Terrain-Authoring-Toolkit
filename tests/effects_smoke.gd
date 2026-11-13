# Windowed effects smoke test: terrain + PixelRenderer + every effect node, captured to
# tests/captures/<name>.png. Usage:
#   godot --path . --script res://tests/effects_smoke.gd -- [name=effects] [time=0.3] [persp]
#     [deck] [noclouds] [norays] [nolamp] [nodiag] [debug=0..5] [coverage=] [cloud_scale=] [shadow=]
extends SceneTree

var frames := 0
var args := {}
var renderer
var sky
var daynight
var diag
var problems: Array[String] = []

func _initialize() -> void:
	pass

func _check(obj, what: String) -> void:
	if obj == null:
		problems.append("could not create " + what)

func _setup() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var root3d := Node3D.new()
	root3d.name = "EffectsScene"
	root.add_child(root3d)

	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.72, 0.86)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.55, 0.6, 0.7)
	e.ambient_light_energy = 0.6
	env.environment = e
	root3d.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.shadow_enabled = true
	root3d.add_child(sun)

	var terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	_check(terrain, "MarchingSquaresTerrain")
	terrain.chunk_size = 16
	terrain.new_chunk_noise_amplitude = 6.0
	terrain.new_chunk_noise_frequency = 0.06
	root3d.add_child(terrain)
	for z in range(-1, 1):
		for x in range(-1, 1):
			terrain.add_chunk(Vector2i(x, z))
	terrain.apply_brush("raise", Vector3(0, 0, 0), 6.0, 5.0, 0)
	terrain.apply_brush("paint", Vector3(-8, 0, 6), 5.0, 5.0, 2)

	# Flood the lowest third of the terrain.
	var lo := 1e9
	var hi := -1e9
	for z in range(-15, 16, 2):
		for x in range(-15, 16, 2):
			var h: float = terrain.get_height_at(Vector3(x, 0, z), 0.0)
			lo = min(lo, h)
			hi = max(hi, h)
	var water_y := lo + (hi - lo) * 0.3
	print("EFFECTS terrain height range %.2f..%.2f, water at %.2f" % [lo, hi, water_y])

	sky = ClassDB.instantiate("PixelSky")
	_check(sky, "PixelSky")
	sky.clouds_enabled = not args.has("noclouds")
	sky.coverage = float(args.get("coverage", "0.55"))
	sky.shadow_strength = float(args.get("shadow", "0.6"))
	sky.cloud_height = 30.0
	sky.deck_enabled = args.has("deck")
	sky.deck_opacity = 0.6
	sky.cloud_scale = float(args.get("cloud_scale", "0.1"))
	root3d.add_child(sky)

	var dn = ClassDB.instantiate("PixelDayNight")
	daynight = dn
	_check(dn, "PixelDayNight")
	dn.time_of_day = float(args.get("time", "0.3"))
	dn.running = false
	root3d.add_child(dn)

	var water = ClassDB.instantiate("PixelWater")
	_check(water, "PixelWater")
	water.position = Vector3(0, water_y, 0)
	root3d.add_child(water)

	var flies = ClassDB.instantiate("PixelFireflies")
	_check(flies, "PixelFireflies")
	flies.night_only = 0.0
	flies.size = 0.25
	flies.position = Vector3(-6, terrain.get_height_at(Vector3(-6, 0, 6), 0.0) + 1.5, 6)
	root3d.add_child(flies)

	var lamp = ClassDB.instantiate("PixelLightVolume")
	_check(lamp, "PixelLightVolume")
	lamp.radius = 6.0
	lamp.position = Vector3(5, terrain.get_height_at(Vector3(5, 0, -5), 0.0) + 1.0, -5)
	if not args.has("nolamp"):
		root3d.add_child(lamp)

	var obj = ClassDB.instantiate("PixelObject")
	root3d.add_child(obj)
	var box := MeshInstance3D.new()
	box.mesh = BoxMesh.new()
	var m := StandardMaterial3D.new()
	m.albedo_color = Color(0.85, 0.3, 0.25)
	box.mesh.material = m
	box.position = Vector3(4, terrain.get_height_at(Vector3(4, 0, -4), 0.0) + 0.5, -4)
	obj.add_child(box)

	var cam = ClassDB.instantiate("PixelOrbitCamera")
	_check(cam, "PixelOrbitCamera")
	cam.orthographic = not args.has("persp")
	cam.ortho_size = 34.0
	cam.distance = 60.0
	root3d.add_child(cam)
	cam.current = true

	renderer = ClassDB.instantiate("PixelRenderer")
	_check(renderer, "PixelRenderer")
	renderer.pixel_size = 3
	renderer.god_rays = not args.has("norays")
	renderer.debug_view = int(args.get("debug", "0"))
	root3d.add_child(renderer)

	diag = ClassDB.instantiate("PixelDiagnostics")
	_check(diag, "PixelDiagnostics")
	diag.start_visible = not args.has("nodiag")
	root3d.add_child(diag)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup()
	if frames == 30:
		var img := root.get_texture().get_image()
		var name: String = args.get("name", "effects")
		img.save_png("res://tests/captures/%s.png" % name)
		print("EFFECTS ", name, " ", renderer.get_status_line())
		print("EFFECTS night=%.2f cloud_offset=%s" % [daynight.get_night_factor(), sky.get_cloud_offset()])
		var s: DirectionalLight3D = root.find_child("*DirectionalLight3D*", true, false)
		if s:
			print("EFFECTS sun color=%s energy=%.2f shadows=%s dir_to_light=%s" % [s.light_color, s.light_energy, s.shadow_enabled, s.global_basis.z])
		var titles := []
		for f in diag.find_children("*", "FoldableContainer", true, false):
			titles.append(f.title)
		print("EFFECTS tuner sections: ", ", ".join(titles))
		if titles.is_empty() and diag.visible:
			problems.append("diagnostics tuner has no sections")
		if problems.is_empty():
			print("EFFECTS OK")
		else:
			for p in problems:
				print("EFFECTS PROBLEM ", p)
		quit()
	return false
