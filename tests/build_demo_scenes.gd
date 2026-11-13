# Builds the demo scenes from the demo data (run after build_demo_data.gd):
#   godot --headless --path . --script res://tests/build_demo_scenes.gd [-- only=hex_island|island|hex_world]
# Without `only=` it rebuilds demo/hex_island.tscn only: demo/demo.tscn and demo/hex_world.tscn
# carry hand-edited parts (player rig) and are regenerated only when named explicitly.
extends SceneTree

const WATER_Y := 0.45

var frames := 0


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		var only := ""
		for a in OS.get_cmdline_user_args():
			if a.begins_with("only="):
				only = a.substr(5)
		if only == "island":
			_build("res://demo/demo.tscn", "res://demo/island_data.res", 0, true, "Demo")
		if only == "hex_world":
			_build("res://demo/hex_world.tscn", "res://demo/hex_world_data.res", 3, false, "HexWorld")
		if only == "" or only == "hex_island":
			# The square island resampled on a hex-rings grid (build_demo_data.gd), same camera.
			_build("res://demo/hex_island.tscn", "res://demo/hex_island_data.res", 3, true, "HexIsland")
		quit()
	return false


func _own(n: Node, owner: Node) -> void:
	n.owner = owner
	for c in n.get_children():
		if c.owner == null and not c.scene_file_path:
			_own(c, owner)


func _add(parent: Node, n: Node, owner: Node, name: String) -> Node:
	n.name = name
	parent.add_child(n)
	n.owner = owner
	return n


func _height_of(n: Node) -> float:
	var lo := INF
	var hi := -INF
	for m in n.find_children("*", "MeshInstance3D", true, false):
		var mi := m as MeshInstance3D
		var aabb := mi.global_transform * mi.get_aabb()
		lo = min(lo, aabb.position.y)
		hi = max(hi, aabb.end.y)
	return hi - lo if hi > lo else 0.0


func _build(path: String, data_path: String, grid: int, island: bool, scene_name: String) -> void:
	var scene := Node3D.new()
	scene.name = scene_name
	root.add_child(scene)

	var we := WorldEnvironment.new()
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.53, 0.72, 0.88)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.62, 0.68, 0.8)
	env.ambient_light_energy = 0.55
	env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	we.environment = env
	_add(scene, we, scene, "WorldEnvironment")

	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 40, 0)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 120.0
	_add(scene, sun, scene, "Sun")

	var terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	terrain.grid_type = grid
	terrain.data = load(data_path)
	# One level = one demo terrace (build_demo_data.gd TERRACE): hex_edge_style Terraces steps it.
	terrain.level_height = 2.5
	# Hex cells place grass at full density (no lattice rejection), so the same subdivisions match.
	terrain.grass_subdivisions = 2
	terrain.grass_size = Vector2(0.55, 0.65)
	terrain.grass_min_height = WATER_Y + 0.15
	var mat := ShaderMaterial.new()
	mat.shader = load("res://addons/MarchingSquaresTerrain/shaders/mst_terrain.gdshader")
	mat.set_shader_parameter("slope_rock", 0.72)
	mat.set_shader_parameter("shore_enabled", true)
	mat.set_shader_parameter("shore_level", WATER_Y)
	mat.set_shader_parameter("shore_band", 0.7)
	mat.set_shader_parameter("shore_slot", 3)
	terrain.terrain_material = mat
	_add(scene, terrain, scene, "MarchingSquaresTerrain")

	var water = ClassDB.instantiate("PixelWater")
	water.size = Vector2(240, 240)
	water.position = Vector3(0, WATER_Y, 0)
	# Raised water (pools / waterfalls) carved by build_demo_data.gd; drops of 4 game levels
	# (0.5 m each) become waterfalls.
	water.water_cells = terrain.data.get_meta("water_cells", PackedVector3Array())
	water.waterfall_level_height = 0.5
	water.waterfall_min_levels = 4
	_add(scene, water, scene, "PixelWater")

	var sky = ClassDB.instantiate("PixelSky")
	sky.coverage = 0.5
	sky.cloud_scale = 0.03
	_add(scene, sky, scene, "PixelSky")
	sky.sun = sun

	var dn = ClassDB.instantiate("PixelDayNight")
	dn.time_of_day = 0.4
	dn.day_length_seconds = 300.0
	_add(scene, dn, scene, "PixelDayNight")
	dn.sun = sun
	dn.environment = we

	var props = ClassDB.instantiate("PixelObject")
	props.snap_to_texel = true
	_add(scene, props, scene, "Characters")
	var spots := [Vector2(-6, 4), Vector2(-2, 6), Vector2(3, 3), Vector2(6, 7)] if island else [Vector2(-4, 2), Vector2(0, 4), Vector2(4, 1), Vector2(1, -3)]
	var models := ["Knight", "Barbarian", "Mage", "Rogue"]
	for i in models.size():
		var ps: PackedScene = load("res://demo/assets/characters/%s.fbx" % models[i])
		var inst := ps.instantiate() as Node3D
		var p: Vector2 = spots[i]
		var y: float = terrain.get_height_at(Vector3(p.x, 0, p.y), 0.0)
		inst.position = Vector3(p.x, y, p.y)
		inst.rotation_degrees.y = 30.0 * i + 20.0
		props.add_child(inst)
		var h := _height_of(inst)
		if h > 0.001:
			inst.scale = Vector3.ONE * (1.9 / h)
		inst.owner = scene
		inst.name = models[i]

	# The Knight is the playable character (demo/player.tscn: MstDemoPlayer + PhantomCamera3D);
	# MstDemoPlayer drops itself onto the terrain on the first physics frames.
	var rig := (load("res://demo/player.tscn") as PackedScene).instantiate() as Node3D
	rig.position = props.get_node("Knight").position + Vector3(0, 1, 0)
	props.get_node("Knight").free()
	_add(scene, rig, scene, "PlayerRig")

	var lamp = ClassDB.instantiate("PixelLightVolume")
	var lp := Vector2(1, 5) if island else Vector2(0, 1)
	lamp.position = Vector3(lp.x, terrain.get_height_at(Vector3(lp.x, 0, lp.y), 0.0) + 1.6, lp.y)
	_add(scene, lamp, scene, "Lantern")

	var ff = ClassDB.instantiate("PixelFireflies")
	ff.position = Vector3(0, terrain.get_height_at(Vector3.ZERO, 0.0) + 1.0, 4)
	_add(scene, ff, scene, "Fireflies")

	var cam = ClassDB.instantiate("PixelOrbitCamera")
	cam.target = Vector3(0, terrain.get_height_at(Vector3(0, 0, 4), 0.0), 4)
	cam.ortho_size = 26.0 if island else 40.0
	cam.pitch_deg = 50.0
	_add(scene, cam, scene, "Camera")
	cam.current = true

	var renderer = ClassDB.instantiate("PixelRenderer")
	renderer.pixel_size = 3
	renderer.god_rays = true
	renderer.palette = ClassDB.instantiate("PixelPalette")
	_add(scene, renderer, scene, "PixelRenderer")

	# Where a game/player rig spawns: on dry flat land near the camera target.
	var spawn := Marker3D.new()
	var sp := Vector2(0, 8)
	spawn.position = Vector3(sp.x, terrain.get_height_at(Vector3(sp.x, 0, sp.y), 0.0), sp.y)
	_add(scene, spawn, scene, "PlayerSpawn")

	var diag = ClassDB.instantiate("PixelDiagnostics")
	_add(scene, diag, scene, "PixelDiagnostics")

	# Weather (Clear by default; set_weather("rain") etc.), a bird flock and a campfire smoke.
	var weather = ClassDB.instantiate("PixelWeather")
	weather.position = Vector3(0, terrain.get_height_at(Vector3(0, 0, 4), 0.0) - 2.0, 0)
	_add(scene, weather, scene, "Weather")
	weather.sun = scene.get_node("Sun")
	weather.environment = scene.get_node("WorldEnvironment")
	var birds = ClassDB.instantiate("PixelBirds")
	birds.position = Vector3(0, terrain.get_height_at(Vector3(0, 0, 4), 0.0) - 2.0, 2)
	_add(scene, birds, scene, "Birds")
	var smoke = ClassDB.instantiate("PixelSmoke")
	var sm := Vector2(-4, 4) if island else Vector2(-2, 2)
	smoke.position = Vector3(sm.x, terrain.get_height_at(Vector3(sm.x, 0, sm.y), 0.0), sm.y)
	smoke.fire = true
	_add(scene, smoke, scene, "Smoke")

	var packed := PackedScene.new()
	var err := packed.pack(scene)
	if err == OK:
		err = ResourceSaver.save(packed, path)
	print("scene ", path, ": ", err)
	scene.queue_free()
