extends SceneTree
## Windowed visual check for cell merge modes: identical stepped terrain with
## CUBIC (vertical cliffs) on the left and SPHERICAL (merged ramps) on the right.
##   godot --path . --script res://tests/capture_cell_merge.gd -- --capture


func _initialize() -> void:
	if not ("--capture" in OS.get_cmdline_user_args()):
		print("pass --capture as a user arg to run")
		quit(0)
		return
	call_deferred("_run")


func _build(terrain: MarchingSquaresTerrain, mode: MarchingSquaresCellChunk.Mode) -> void:
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	terrain.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	terrain.enable_runtime_texture_baking = false
	terrain.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := terrain.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	chunk.merge_mode = mode
	for r in range(6):
		for c in range(8):
			chunk.draw_height(c, r, float(c / 2) * 1.0)
	chunk.regenerate_mesh()


func _run() -> void:
	var root := get_root()
	root.set_size(Vector2i(960, 720))
	
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.15, 0.18, 0.22)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.8, 0.8, 0.8)
	var world_env := WorldEnvironment.new()
	world_env.environment = env
	root.add_child(world_env)
	
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-45, -35, 0)
	sun.light_energy = 1.4
	root.add_child(sun)
	
	var cubic := MarchingSquaresTerrain.new()
	root.add_child(cubic)
	_build(cubic, MarchingSquaresCellChunk.Mode.CUBIC)
	
	var spherical := MarchingSquaresTerrain.new()
	root.add_child(spherical)
	_build(spherical, MarchingSquaresCellChunk.Mode.SPHERICAL)
	spherical.position = Vector3(24, 0, 0)
	
	var cam := Camera3D.new()
	root.add_child(cam)
	cam.look_at_from_position(Vector3(20, 16, 22), Vector3(20, 2, 6))
	
	for i in range(8):
		await process_frame
	var img := root.get_texture().get_image()
	img.save_png("user://cell_merge.png")
	print("saved user://cell_merge.png")
	quit(0)
