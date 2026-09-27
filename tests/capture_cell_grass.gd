extends SceneTree
## Windowed visual check for cell-mode grass.
##   godot --path . --script res://tests/capture_cell_grass.gd -- --capture
## Builds hex and triangle terrains with grass, masks one cell each, saves PNGs.

const OUT_DIR := "user://"


func _initialize() -> void:
	if not ("--capture" in OS.get_cmdline_user_args()):
		print("pass --capture as a user arg to run")
		quit(0)
		return
	call_deferred("_run")


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
	sun.rotation_degrees = Vector3(-55, -35, 0)
	sun.light_energy = 1.4
	root.add_child(sun)
	
	var cam := Camera3D.new()
	cam.position = Vector3(0, 0, 0)
	root.add_child(cam)
	cam.look_at_from_position(Vector3(12, 22, 14), Vector3(8, 0, 6))
	
	# HEX terrain with grass; mask the centre cell.
	var hex := MarchingSquaresTerrain.new()
	hex.dimensions = Vector3i(8, 10, 6)
	hex.cell_size = Vector2(2.0, 2.0)
	hex.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	root.add_child(hex)
	hex.grid_type = MarchingSquaresTerrain.GridType.HEX
	var hex_chunk := hex.make_chunk() as MarchingSquaresHexChunk
	hex_chunk.name = "Chunk 0_0"
	hex.add_chunk(Vector2i(0, 0), hex_chunk, null, true)
	for r in range(6):
		for c in range(8):
			var h := 1.5
			hex_chunk.draw_height(c, r, h)
	hex_chunk.draw_grass_mask(4, 3, Color(0, 0, 0, 0))
	hex_chunk.regenerate_mesh()
	hex.position = Vector3(0, 0, 0)
	var hex_mm : MultiMesh = hex_chunk.grass_planter.multimesh
	var visible := 0
	for i in range(hex_mm.instance_count):
		if hex_mm.get_instance_transform(i).basis.get_scale().length_squared() > 0.001:
			visible += 1
	print("hex grass instances: ", visible, " / ", hex_mm.instance_count)
	
	# TRIANGLE terrain with grass; mask two adjacent cells.
	var tri := MarchingSquaresTerrain.new()
	tri.dimensions = Vector3i(8, 10, 6)
	tri.cell_size = Vector2(2.0, 2.0)
	tri.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	root.add_child(tri)
	tri.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var tri_chunk := tri.make_chunk() as MarchingSquaresTriChunk
	tri_chunk.name = "Chunk 0_0"
	tri.add_chunk(Vector2i(0, 0), tri_chunk, null, true)
	for r in range(6):
		for c in range(16):
			var h := 1.5
			tri_chunk.draw_height(c, r, h)
	tri_chunk.draw_grass_mask(8, 3, Color(0, 0, 0, 0))
	tri_chunk.draw_grass_mask(9, 3, Color(0, 0, 0, 0))
	tri_chunk.regenerate_mesh()
	tri.position = Vector3(22, 0, 0)
	
	cam.look_at_from_position(Vector3(26, 24, 16), Vector3(20, 0, 5))
	
	await process_frame
	await process_frame
	await process_frame
	await process_frame
	await process_frame
	await process_frame
	var img := root.get_texture().get_image()
	var path := "user://cell_grass.png"
	var err := img.save_png(path)
	print("saved ", path, " err=", err, " size=", img.get_size())
	print("abs path: ", ProjectSettings.globalize_path(path))
	
	# Close-up of the hex plate to inspect grass clearly.
	cam.look_at_from_position(Vector3(6, 5, 12), Vector3(8, 1.5, 5))
	for i in range(6):
		await process_frame
	img = root.get_texture().get_image()
	path = "user://cell_grass_closeup.png"
	err = img.save_png(path)
	print("saved ", path, " err=", err)
	quit(0)
