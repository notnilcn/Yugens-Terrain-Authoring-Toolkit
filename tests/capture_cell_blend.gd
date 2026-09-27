extends SceneTree
## Capture cell modes with blend_mode 1/2 and ridge/ledge enabled.

func _initialize() -> void:
	if not ("--capture" in OS.get_cmdline_user_args()):
		quit(0)
		return
	call_deferred("_run")


func _run() -> void:
	var root := get_root()
	root.set_size(Vector2i(1100, 760))
	
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.10, 0.12, 0.16)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(1, 1, 1)
	var world_env := WorldEnvironment.new()
	world_env.environment = env
	root.add_child(world_env)
	
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-60, -30, 0)
	sun.light_energy = 1.0
	root.add_child(sun)
	
	var cam := Camera3D.new()
	cam.position = Vector3(10, 34, 18)
	root.add_child(cam)
	cam.look_at_from_position(cam.position, Vector3(10, 0, 7), Vector3.UP)
	
	_make(root, MarchingSquaresTerrain.GridType.HEX, MarchingSquaresTerrain.GridType.HEX, 0)
	_make(root, MarchingSquaresTerrain.GridType.TRIANGLE, MarchingSquaresTerrain.GridType.TRIANGLE, 24)
	
	for i in range(8):
		await process_frame
	var img := root.get_texture().get_image()
	img.save_png("user://cell_textures_hard.png")
	print("saved")
	quit(0)


func _make(root: Node, grid: int, _label: int, x_offset: float) -> void:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(10, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	terrain.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	terrain.blend_mode = 2
	terrain.use_ridge_texture = true
	terrain.use_ledge_texture = true
	terrain.ridge_threshold = 0.4
	terrain.ledge_threshold = 0.4
	root.add_child(terrain)
	terrain.grid_type = grid
	var chunk = terrain.make_chunk()
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var cols : int = chunk.cells_per_chunk().x
	for r in range(6):
		for c in range(cols):
			chunk.draw_height(c, r, float((c / 5) % 3) * 2.0)
			match (c / 5) % 3:
				0:
					chunk.draw_color_0(c, r, Color(0, 1, 0, 0))
					chunk.draw_color_1(c, r, Color(0, 1, 0, 0))
				1:
					chunk.draw_color_0(c, r, Color(0, 0, 1, 0))
					chunk.draw_color_1(c, r, Color(0, 1, 0, 0))
				_:
					chunk.draw_color_0(c, r, Color(1, 0, 0, 0))
					chunk.draw_color_1(c, r, Color(0, 1, 0, 0))
	chunk.regenerate_mesh()
	terrain.position = Vector3(x_offset, 0, 0)
