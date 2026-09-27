extends SceneTree
## Windowed check for the grid-aligned brush outline on triangle terrain.
##   godot --path . --script res://tests/capture_lattice_outline.gd -- --capture
## Draws the outline transform exactly like the gizmo and marks the selected
## cells; the outline must enclose the marked cells (pointy/flat orientation).

const BPC = preload("res://addons/MarchingSquaresTerrain/editor/utils/brush_pattern_calculator.gd")


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
	
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 11.0
	root.add_child(cam)
	cam.look_at_from_position(Vector3(11, 30, 5.196), Vector3(11, 0, 5.196), Vector3(0, 0, -1))
	
	# Triangle terrain, flat at height 1.5.
	var tri := MarchingSquaresTerrain.new()
	tri.dimensions = Vector3i(8, 10, 6)
	tri.cell_size = Vector2(2.0, 2.0)
	tri.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	tri.enable_runtime_texture_baking = false
	root.add_child(tri)
	tri.grid_type = MarchingSquaresTerrain.GridType.TRIANGLE
	var chunk := tri.make_chunk() as MarchingSquaresTriChunk
	chunk.name = "Chunk 0_0"
	tri.add_chunk(Vector2i(0, 0), chunk, null, true)
	for r in range(6):
		for c in range(16):
			chunk.draw_height(c, r, 1.5)
	chunk.regenerate_mesh()
	
	# Selection center: a lattice point with a couple of free rings around it.
	var cell_size := tri.cell_size
	var lattice := Vector2i(4, 3)
	var brush_pos := MarchingSquaresTriGrid.lattice_point(lattice.x, lattice.y, cell_size)
	var grid_size := 2
	var selected := BPC.grid_aligned_cells(tri, brush_pos, grid_size)
	
	# Outline transform: same math as the gizmo helper.
	var outline := BPC.grid_aligned_outline(tri, brush_pos, grid_size)
	var radius : float = outline["radius"]
	var rotation : float = outline["rotation"]
	var center : Vector2 = outline["center"]
	print("outline center=", center, " requested brush_pos=", brush_pos, " radius=", radius, " rotation_deg=", rad_to_deg(rotation))
	
	var marker := MeshInstance3D.new()
	marker.mesh = load("res://addons/MarchingSquaresTerrain/resources/plugin_materials/hex_brush_radius_visual.tres")
	var mat := load("res://addons/MarchingSquaresTerrain/resources/plugin_materials/hex_brush_radius_material.tres")
	# 1x1 plane: SDF boundary at half the basis scale (gizmo convention).
	var basis := Basis(Vector3.UP, rotation) * Basis(
		Vector3.RIGHT * radius * 2.0, Vector3.UP, Vector3.BACK * radius * 2.0)
	marker.transform = Transform3D(basis, Vector3(center.x, 1.55, center.y))
	if mat is ShaderMaterial:
		mat = mat.duplicate()
		mat.set_shader_parameter("falloff_visible", false)
	marker.material_override = mat
	root.add_child(marker)
	
	# Mark every selected cell with a small sphere at its center.
	var sphere := SphereMesh.new()
	sphere.radius = 0.18
	sphere.height = 0.36
	var sphere_mat := StandardMaterial3D.new()
	sphere_mat.albedo_color = Color(1.0, 0.2, 0.2)
	sphere_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	for cell in selected:
		var c := BPC.cell_center_for(tri, cell)
		var inst := MeshInstance3D.new()
		inst.mesh = sphere
		inst.material_override = sphere_mat
		inst.position = Vector3(c.x, 1.7, c.y)
		root.add_child(inst)
	print("selected cells: ", selected.size())
	
	# Direction reference markers: blue = +X (expected vertex for flat-top),
	# green = +Z (expected edge midpoint for flat-top).
	var blue := StandardMaterial3D.new()
	blue.albedo_color = Color(0.2, 0.4, 1.0)
	blue.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	var green := StandardMaterial3D.new()
	green.albedo_color = Color(0.2, 1.0, 0.4)
	green.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	var big := SphereMesh.new()
	big.radius = 0.35
	big.height = 0.7
	var east := MeshInstance3D.new()
	east.mesh = big
	east.material_override = blue
	east.position = Vector3(center.x + radius, 1.7, center.y)
	root.add_child(east)
	var south := MeshInstance3D.new()
	south.mesh = big
	south.material_override = green
	south.position = Vector3(center.x, 1.7, center.y + radius)
	root.add_child(south)
	
	for i in range(8):
		await process_frame
	var img := root.get_texture().get_image()
	img.save_png("user://lattice_outline.png")
	print("saved user://lattice_outline.png")
	quit(0)
