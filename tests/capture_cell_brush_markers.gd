extends SceneTree
## Windowed visual check that cell-mode brush markers are visible over terrain.
##   godot --path . --script res://tests/capture_cell_brush_markers.gd -- --capture
## Draws the cell marker meshes exactly like the gizmo: one marker per cell,
## coplanar with the cell top, using the mesh's own material. All markers must
## be visible (correct windings + on-top material), so the PNG shows two
## triangle-terrain and hex-terrain marker clusters.

const BPC = preload("res://addons/MarchingSquaresTerrain/editor/utils/brush_pattern_calculator.gd")
const CELL_TRI_VISUAL : Mesh = preload("res://addons/MarchingSquaresTerrain/resources/plugin_materials/tri_cell_visual.tres")
const CELL_HEX_VISUAL : Mesh = preload("res://addons/MarchingSquaresTerrain/resources/plugin_materials/hex_cell_visual.tres")


func _initialize() -> void:
	if not ("--capture" in OS.get_cmdline_user_args()):
		print("pass --capture as a user arg to run")
		quit(0)
		return
	call_deferred("_run")


func _run() -> void:
	var root := get_root()
	root.set_size(Vector2i(1280, 720))
	
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
	cam.position = Vector3(16, 34, 40)
	cam.look_at(Vector3(16, 0, 11))
	
	var tri := _make_terrain(MarchingSquaresTerrain.GridType.TRIANGLE, Vector3(0, 0, 0))
	var hex := _make_terrain(MarchingSquaresTerrain.GridType.HEX, Vector3(22, 0, 0))
	print("triangle markers=", _draw_markers(tri, CELL_TRI_VISUAL))
	print("hex markers=", _draw_markers(hex, CELL_HEX_VISUAL))
	
	for i in range(8):
		await process_frame
	var img := root.get_texture().get_image()
	var path := "user://cell_brush_markers.png"
	var err := img.save_png(path)
	print("saved ", path, " err=", err, " size=", img.get_size())
	print("abs path: ", ProjectSettings.globalize_path(path))
	quit(0)


func _make_terrain(grid: int, origin: Vector3) -> MarchingSquaresTerrain:
	var terrain := MarchingSquaresTerrain.new()
	terrain.dimensions = Vector3i(8, 10, 6)
	terrain.cell_size = Vector2(2.0, 2.0)
	terrain.storage_mode = MarchingSquaresTerrain.StorageMode.RUNTIME
	terrain.enable_runtime_texture_baking = false
	get_root().add_child(terrain)
	terrain.grid_type = grid
	var chunk := terrain.make_chunk()
	chunk.name = "Chunk 0_0"
	terrain.add_chunk(Vector2i(0, 0), chunk, null, true)
	var cells := terrain.cells_per_chunk()
	for r in range(cells.y):
		for c in range(cells.x):
			chunk.draw_height(c, r, 0.0)
	chunk.regenerate_mesh()
	terrain.position = origin
	return terrain


## Places markers like the gizmo cell brush does: one mesh per cell at the
## cell's top height, scaled by the brush falloff sample.
func _draw_markers(terrain: MarchingSquaresTerrain, mesh: Mesh) -> int:
	var count := 0
	for r in range(2, 4):
		for c in range(4, 12):
			var global := Vector2i(c, r)
			var center := BPC.cell_center_for(terrain, global)
			var h = terrain.get_cell_height(global)
			if h == null:
				continue
			var mi := MeshInstance3D.new()
			mi.mesh = mesh
			mi.position = Vector3(center.x, float(h), center.y)
			terrain.add_child(mi)
			count += 1
	return count
