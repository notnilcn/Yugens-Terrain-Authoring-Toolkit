extends SceneTree
## Windowed runtime-baking check for cell modes.
##   godot --path . --script res://tests/capture_cell_bake.gd -- --capture
## Builds hex and triangle terrains with enable_runtime_texture_baking, waits
## for the atlas to be applied, then screenshots the result.

var _hex_chunk : MarchingSquaresCellChunk
var _tri_chunk : MarchingSquaresCellChunk
var _frames := 0


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
	root.add_child(cam)
	cam.look_at_from_position(Vector3(20, 26, 30), Vector3(20, 0, 8))
	
	var bake_enabled : bool = not ("--nobake" in OS.get_cmdline_user_args())
	
	# HEX terrain, runtime baking on, painted slots.
	var hex := MarchingSquaresTerrain.new()
	hex.dimensions = Vector3i(8, 10, 6)
	hex.cell_size = Vector2(2.0, 2.0)
	hex.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	hex.enable_runtime_texture_baking = bake_enabled
	root.add_child(hex)
	hex.grid_type = MarchingSquaresTerrain.GridType.HEX
	_hex_chunk = hex.make_chunk()
	_hex_chunk.name = "Chunk 0_0"
	hex.add_chunk(Vector2i(0, 0), _hex_chunk, null, true)
	for r in range(6):
		for c in range(8):
			_hex_chunk.draw_height(c, r, sin(c * 0.9) * cos(r * 0.8) * 3.0 + 1.5)
			if c < 4:
				_hex_chunk.draw_color_0(c, r, Color(1, 0, 0, 0))
				_hex_chunk.draw_color_1(c, r, Color(0, 0, 1, 0))
			else:
				_hex_chunk.draw_color_0(c, r, Color(0, 1, 0, 0))
				_hex_chunk.draw_color_1(c, r, Color(1, 0, 0, 0))
	_hex_chunk.regenerate_mesh()
	
	# TRI terrain, runtime baking on.
	var tri := MarchingSquaresTerrain.new()
	tri.dimensions = Vector3i(8, 10, 6)
	tri.cell_size = Vector2(2.0, 2.0)
	tri.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	tri.enable_runtime_texture_baking = bake_enabled
	root.add_child(tri)
	tri.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	_tri_chunk = tri.make_chunk()
	_tri_chunk.name = "Chunk 0_0"
	tri.add_chunk(Vector2i(0, 0), _tri_chunk, null, true)
	for r in range(6):
		for c in range(16):
			_tri_chunk.draw_height(c, r, sin(c * 0.45) * cos(r * 0.8) * 3.0 + 1.5)
			_tri_chunk.draw_color_0(c, r, Color(0, 1, 0, 0))
			_tri_chunk.draw_color_1(c, r, Color(0, 0, 0, 1))
	_tri_chunk.regenerate_mesh()
	tri.position = Vector3(22, 0, 0)
	
	# Wait until both bakes applied their atlas material.
	for i in range(180):
		await process_frame
		if _is_baked(_hex_chunk) and _is_baked(_tri_chunk):
			break
	print("hex baked: ", _is_baked(_hex_chunk), " tri baked: ", _is_baked(_tri_chunk))
	
	for i in range(6):
		await process_frame
	var img := root.get_texture().get_image()
	var path := "user://cell_bake.png" if bake_enabled else "user://cell_bake_off.png"
	var err := img.save_png(path)
	print("saved ", path, " err=", err, " size=", img.get_size())
	quit(0)


func _is_baked(chunk: MarchingSquaresCellChunk) -> bool:
	if chunk.mesh == null:
		return false
	var mat := chunk.mesh.surface_get_material(0)
	if mat is StandardMaterial3D:
		return mat.albedo_texture != null
	if mat is ShaderMaterial:
		return mat.get_shader_parameter("texture_albedo") != null
	return false
