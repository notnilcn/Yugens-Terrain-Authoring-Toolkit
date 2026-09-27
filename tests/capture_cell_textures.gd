extends SceneTree
## Visual verification for cell-mode textures, vertex paint and grass mask.

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
	cam.position = Vector3(10, 38, 12)
	root.add_child(cam)
	cam.look_at_from_position(cam.position, Vector3(10, 0, 7), Vector3.UP)
	
	_make_hex(root)
	_make_tri(root)
	
	for i in range(8):
		await process_frame
	var img := root.get_texture().get_image()
	img.save_png("user://cell_textures.png")
	print("saved")
	quit(0)


func _make_hex(root: Node) -> void:
	var hex := MarchingSquaresTerrain.new()
	hex.dimensions = Vector3i(10, 10, 6)
	hex.cell_size = Vector2(2.0, 2.0)
	hex.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	root.add_child(hex)
	hex.grid_type = MarchingSquaresTerrain.GridType.HEX
	var chunk := hex.make_chunk() as MarchingSquaresHexChunk
	chunk.name = "Chunk 0_0"
	hex.add_chunk(Vector2i(0, 0), chunk, null, true)
	for r in range(6):
		for c in range(10):
			chunk.draw_height(c, r, 0.0)
			# Stripe by texture slot (texture 6 = "gg" is a solid green).
			var slot := c / 2
			var colors := _slot_colors(slot)
			chunk.draw_color_0(c, r, colors[0])
			chunk.draw_color_1(c, r, colors[1])
			# Grass mask off in the lower half.
			chunk.draw_grass_mask(c, r, Color(1, 1, 1, 1) if r < 3 else Color(0, 0, 0, 0))
	chunk.regenerate_mesh()
	hex.position = Vector3(0, 0, 0)


func _make_tri(root: Node) -> void:
	var tri := MarchingSquaresTerrain.new()
	tri.dimensions = Vector3i(10, 10, 6)
	tri.cell_size = Vector2(2.0, 2.0)
	tri.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	root.add_child(tri)
	tri.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := tri.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	tri.add_chunk(Vector2i(0, 0), chunk, null, true)
	for r in range(6):
		for c in range(20):
			# Terraced columns with visible walls.
			chunk.draw_height(c, r, float((c / 4) % 3) * 2.0)
			var colors := _slot_colors(c / 4)
			chunk.draw_color_0(c, r, colors[0])
			chunk.draw_color_1(c, r, colors[1])
			chunk.draw_grass_mask(c, r, Color(1, 1, 1, 1) if (c / 4) % 2 == 0 else Color(0, 0, 0, 0))
	chunk.regenerate_mesh()
	tri.position = Vector3(24, 0, 0)


func _slot_colors(slot: int) -> Array:
	# Vertex color pair for texture indices: 5 (gg), 6 (gb), 7 (ga), 8 (br), 10 (bb)
	match slot % 5:
		0:
			return [Color(0, 1, 0, 0), Color(0, 1, 0, 0)] # gg
		1:
			return [Color(0, 1, 0, 0), Color(0, 0, 1, 0)] # gb
		2:
			return [Color(0, 0, 1, 0), Color(1, 0, 0, 0)] # br
		3:
			return [Color(1, 0, 0, 0), Color(0, 0, 0, 1)] # ra
		_:
			return [Color(0, 0, 1, 0), Color(0, 0, 1, 0)] # bb
