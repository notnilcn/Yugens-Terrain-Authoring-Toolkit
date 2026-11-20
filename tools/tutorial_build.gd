extends SceneTree
## Builds the beginner example scenes in res://tutorial/ and their terrain data.
## Run with: godot --headless --path . --script tools/tutorial_build.gd
##
## This is a documentation utility, not part of the addon. It deliberately uses only
## public addon APIs so the generated scenes match what the editor produces.

const OUT := "res://tutorial/"

func _init() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(OUT + "data"))
	_build_flat_playground()
	_build_island()
	_build_pixel_showcase()
	_build_world_effects()
	quit()

func _preset() -> MarchingSquaresTexturePreset:
	var p := MarchingSquaresTexturePreset.new()
	p.assign_default_assets()
	p.preset_name = "Tutorial"
	return p

func _environment() -> Environment:
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.52, 0.67, 0.87)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.55, 0.6, 0.72)
	env.ambient_light_energy = 0.6
	return env

func _sun() -> DirectionalLight3D:
	var sun := DirectionalLight3D.new()
	sun.name = "Sun"
	sun.rotation_degrees = Vector3(-48, -38, 0)
	sun.light_color = Color(1.0, 0.96, 0.88)
	sun.light_energy = 1.1
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 120.0
	return sun

func _camera(target: Vector3, distance: float, yaw_deg: float, pitch_deg: float, size: float) -> Camera3D:
	var cam := Camera3D.new()
	cam.name = "Camera"
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = size
	cam.current = true
	var yaw := deg_to_rad(yaw_deg)
	var pitch := deg_to_rad(pitch_deg)
	cam.position = target + Vector3(sin(yaw) * cos(pitch), sin(pitch), cos(yaw) * cos(pitch)) * distance
	cam.look_at_from_position(cam.position, target)
	cam.set_script(load(OUT + "orbit_camera.gd"))
	cam.set("target", target)
	cam.set("distance", distance)
	cam.set("yaw_deg", yaw_deg)
	cam.set("pitch_deg", pitch_deg)
	return cam

func _renderer() -> PixelRenderer:
	var r := PixelRenderer.new()
	r.name = "PixelRenderer"
	r.pixel_size = 2
	r.studio_prelook = true
	var palette := PixelPalette.new()
	palette.resource_name = "Tutorial"
	r.palette = palette
	r.palette_enabled = false
	return r

func _sky(sun: DirectionalLight3D) -> PixelSky:
	var s := PixelSky.new()
	s.name = "PixelSky"
	s.sun = sun
	s.coverage = 0.45
	s.softness = 0.12
	s.cloud_scale = 0.02
	# The overview cameras sit ~100-250 m up. Keep the cloud plane above them: a deck *below*
	# the camera is drawn between camera and ground, and weather (Rain/Snow/Storm) thickens
	# it into a solid sheet that hides the whole island.
	s.cloud_height = 300.0
	s.deck_enabled = true
	s.deck_size = 500.0
	s.deck_opacity = 0.85
	return s

## The built-in weather fog is tuned for cameras 20-40 m from the action. The tutorial's
## orthographic overview camera stands 165 m back, so that fog would wash the island out.
## Presets named like a built-in replace it: same weather, a tenth of the fog.
func _overview_weather_presets() -> Array[PixelWeatherPreset]:
	var out: Array[PixelWeatherPreset] = []
	for n in ["Cloudy", "Foggy", "Rain", "Snow", "Storm"]:
		var p := PixelWeatherPreset.from_builtin(n)
		p.fog_density *= 0.1
		out.append(p)
	return out

func _daynight(sun: DirectionalLight3D, env: WorldEnvironment) -> PixelDayNight:
	var d := PixelDayNight.new()
	d.name = "PixelDayNight"
	d.sun = sun
	d.environment = env
	d.time_of_day = 0.42
	d.day_length_seconds = 600.0
	return d

func _add_terrain(root: Node3D, data: MarchingSquaresTerrainData, chunks: Array,
		height_fn: Callable, slot_fn: Callable, grid: int = 0) -> MarchingSquaresTerrain:
	var t := MarchingSquaresTerrain.new()
	t.name = "MarchingSquaresTerrain"
	t.grid_type = grid
	t.chunk_size = 16
	t.cell_size = Vector2(2, 2)
	t.contents = data
	t.tex_assign = _preset()
	root.add_child(t)
	for c in chunks:
		t.add_chunk(c)
	# heights
	for c in chunks:
		var cells := t.get_chunk_cells(c)
		var heights := PackedFloat32Array()
		for cell in cells:
			heights.append(height_fn.call(cell))
		t.set_cell_heights(cells, heights)
	# floor slots
	for c in chunks:
		var cells := t.get_chunk_cells(c)
		var slots := PackedByteArray()
		for cell in cells:
			slots.append(slot_fn.call(cell, height_fn.call(cell)))
		t.set_chunk_slots(c, slots, false)
	t.commit_to_data()
	return t

func _set_owners(root: Node, node: Node) -> void:
	for child in node.get_children():
		child.owner = root
		_set_owners(root, child)

func _build_flat_playground() -> void:
	var root := Node3D.new()
	root.name = "Playground"
	var we := WorldEnvironment.new()
	we.name = "WorldEnvironment"
	we.environment = _environment()
	root.add_child(we)
	var sun := _sun()
	root.add_child(sun)
	var noise := FastNoiseLite.new()
	noise.seed = 7
	noise.frequency = 0.035
	noise.fractal_octaves = 3
	var h := func(cell: Vector2i) -> float:
		var x := float(cell.x) * 2.0
		var z := float(cell.y) * 2.0
		var d := Vector2(x - 48.0, z - 48.0).length()
		# A flat-topped hill with steep sides, plus gentle noise around it.
		var hill := smoothstep(26.0, 15.0, d) * 9.0
		return noise.get_noise_2d(x, z) * 1.4 + hill
	var slot := func(_cell: Vector2i, height: float) -> int:
		if height > 6.0:
			return 1
		return 0
	var data := MarchingSquaresTerrainData.new()
	var chunks: Array[Vector2i] = []
	for y in range(3):
		for x in range(3):
			chunks.append(Vector2i(x, y))
	var t := _add_terrain(root, data, chunks, h, slot)
	t.name = "MarchingSquaresTerrain"
	t.grass_size = Vector2(0.55, 0.65)
	t.grass_min_height = -1.0
	var water := PixelWater.new()
	water.name = "PixelWater"
	water.size = Vector2(400, 400)
	water.subdivisions = 128
	water.position = Vector3(48, -1.2, 48)
	root.add_child(water)
	var cam := _camera(Vector3(48, 3, 48), 180.0, 36.0, 30.0, 135.0)
	root.add_child(cam)
	root.add_child(_renderer())
	root.add_child(_sky(sun))
	root.add_child(_daynight(sun, we))
	var data_path := OUT + "data/01_flat_playground.res"
	var saved: MarchingSquaresTerrainData = t.contents
	print("data chunks before save: ", saved.get("chunks").size())
	ResourceSaver.save(saved, data_path)
	saved.take_over_path(data_path)
	_set_owners(root, root)
	var packed := PackedScene.new()
	packed.pack(root)
	ResourceSaver.save(packed, OUT + "01_flat_playground.tscn")
	print("built 01")


func _build_island() -> void:
	var root := Node3D.new()
	root.name = "Island"
	var we := WorldEnvironment.new()
	we.name = "WorldEnvironment"
	we.environment = _environment()
	root.add_child(we)
	var sun := _sun()
	root.add_child(sun)
	var noise := FastNoiseLite.new()
	noise.seed = 21
	noise.frequency = 0.05
	noise.fractal_octaves = 2
	var h := func(cell: Vector2i) -> float:
		var x := float(cell.x) * 2.0
		var z := float(cell.y) * 2.0
		var d := Vector2(x - 48.0, z - 48.0).length()
		if d > 52.0:
			return -2.0
		if d > 46.0:
			return 0.6
		var base := 3.0 + (1.0 - d / 46.0) * 5.0
		var n := noise.get_noise_2d(x, z) * 1.1
		# Quantize into steps so the square grid builds real cliffs.
		return snappedf(base + n, 1.4)
	var slot := func(_cell: Vector2i, height: float) -> int:
		if height < 1.0:
			return 3       # Sand
		if height > 6.5:
			return 5       # Cliff
		return 0           # Grass
	var data := MarchingSquaresTerrainData.new()
	var chunks: Array[Vector2i] = []
	for y in range(3):
		for x in range(3):
			chunks.append(Vector2i(x, y))
	var t := _add_terrain(root, data, chunks, h, slot)
	t.name = "MarchingSquaresTerrain"
	t.grass_size = Vector2(0.55, 0.65)
	t.grass_min_height = 1.2
	var water := PixelWater.new()
	water.name = "PixelWater"
	water.size = Vector2(300, 300)
	water.subdivisions = 128
	water.position = Vector3(48, 1.2, 48)
	root.add_child(water)
	var tree: PixelScatterItem = PixelScatterItem.create_builtin("Tree")
	tree.display_name = "Tree"
	tree.density = 0.06
	tree.ground_slots = 1 << 0
	var rock: PixelScatterItem = PixelScatterItem.create_builtin("Rock")
	rock.display_name = "Rock"
	rock.density = 0.025
	rock.ground_slots = 1 << 3
	var items := t.scatter_items
	items.append(tree)
	items.append(rock)
	t.scatter_items = items
	for i in range(items.size()):
		t.scatter_fill_cells(i)
	var cam := _camera(Vector3(48, 3, 48), 165.0, 36.0, 38.0, 120.0)
	root.add_child(cam)
	root.add_child(_renderer())
	root.add_child(_sky(sun))
	root.add_child(_daynight(sun, we))
	var data_path := OUT + "data/02_island.res"
	var saved: MarchingSquaresTerrainData = t.contents
	ResourceSaver.save(saved, data_path)
	saved.take_over_path(data_path)
	_set_owners(root, root)
	var packed := PackedScene.new()
	packed.pack(root)
	ResourceSaver.save(packed, OUT + "02_island.tscn")
	print("built 02")

func _build_pixel_showcase() -> void:
	var root := Node3D.new()
	root.name = "PixelShowcase"
	var we := WorldEnvironment.new()
	we.name = "WorldEnvironment"
	we.environment = _environment()
	root.add_child(we)
	var sun := _sun()
	root.add_child(sun)
	var h := func(_cell: Vector2i) -> float:
		return 0.0
	var slot := func(_cell: Vector2i, _height: float) -> int:
		return 0
	var data := MarchingSquaresTerrainData.new()
	var chunks: Array[Vector2i] = []
	for y in range(2):
		for x in range(2):
			chunks.append(Vector2i(x, y))
	var t := _add_terrain(root, data, chunks, h, slot)
	t.grass_size = Vector2(0.55, 0.65)
	# A reclaimed ruin made with the Pixel Builder, plus imported rocks.
	var ruin := PixelObject.new()
	ruin.name = "BuilderRuin"
	root.add_child(ruin)
	var mesh := PixelBuilderMesh.new()
	mesh.name = "Ruin"
	mesh.contents = load("res://demo/builder_demo_ruin.tres")
	ruin.add_child(mesh)
	mesh.position = Vector3(32, 0, 32)
	mesh.scale = Vector3(4, 4, 4)
	var cam := _camera(Vector3(32, 2, 32), 38.0, 35.0, 35.0, 24.0)
	root.add_child(cam)
	root.add_child(_renderer())
	root.add_child(_sky(sun))
	root.add_child(_daynight(sun, we))
	var data_path := OUT + "data/03_pixel_look.res"
	var saved: MarchingSquaresTerrainData = t.contents
	ResourceSaver.save(saved, data_path)
	saved.take_over_path(data_path)
	_set_owners(root, root)
	var packed := PackedScene.new()
	packed.pack(root)
	ResourceSaver.save(packed, OUT + "03_pixel_look.tscn")
	print("built 03")

func _build_world_effects() -> void:
	# Start from the island, then add the atmosphere layer.
	var ps: PackedScene = load(OUT + "02_island.tscn")
	var root: Node3D = ps.instantiate()
	root.name = "WorldMood"
	var weather := PixelWeather.new()
	weather.name = "PixelWeather"
	weather.position = Vector3(48, 6, 48)
	weather.sun = root.get_node("Sun")
	weather.environment = root.get_node("WorldEnvironment")
	weather.presets = _overview_weather_presets()
	root.add_child(weather)
	var fireflies := PixelFireflies.new()
	fireflies.name = "Fireflies"
	fireflies.position = Vector3(48, 5, 48)
	fireflies.extents = Vector3(22, 3, 22)
	fireflies.count = 100
	root.add_child(fireflies)
	var birds := PixelBirds.new()
	birds.name = "Birds"
	birds.position = Vector3(48, 10, 48)
	birds.radius = 24
	birds.altitude = 8
	root.add_child(birds)
	var dust := PixelParticleField.new()
	dust.name = "Dust"
	dust.position = Vector3(48, 1.5, 48)
	dust.density = 0.2
	root.add_child(dust)
	var daynight := root.get_node("PixelDayNight")
	daynight.time_of_day = 0.72
	weather.weather = 0
	_set_owners(root, root)
	var packed := PackedScene.new()
	packed.pack(root)
	ResourceSaver.save(packed, OUT + "04_world_mood.tscn")
	print("built 04")
