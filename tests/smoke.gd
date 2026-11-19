# Windowed smoke test: builds a small scene in code, renders through the pixel pipeline and
# writes tests/captures/<name>.png. Usage:
#   godot --path . --script res://tests/smoke.gd -- [grid=0..3] [name=smoke] [debug=0..5] [persp]
extends SceneTree

var frames := 0
var args := {}
var renderer

func _initialize() -> void:
	pass

func _setup() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var root3d := Node3D.new()
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
	sun.rotation_degrees = Vector3(-55, 35, 0)
	sun.shadow_enabled = true
	root3d.add_child(sun)
	var terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	terrain.grid_type = int(args.get("grid", "0"))
	if args.has("data"):
		terrain.data = load(args["data"])
		root3d.add_child(terrain)
	else:
		terrain.chunk_size = 16
		terrain.new_chunk_noise_amplitude = 6.0
		terrain.new_chunk_noise_frequency = 0.06
		root3d.add_child(terrain)
		for z in range(-1, 1):
			for x in range(-1, 1):
				terrain.add_chunk(Vector2i(x, z))
		terrain.apply_brush("raise", Vector3(0, 0, 0), 6.0, 5.0, 0)
		terrain.apply_brush("paint", Vector3(8, 0, 8), 5.0, 3.0, 0)
		terrain.apply_brush("paint", Vector3(-8, 0, 6), 5.0, 5.0, 2)
	var obj = ClassDB.instantiate("PixelObject")
	root3d.add_child(obj)
	var box := MeshInstance3D.new()
	box.mesh = BoxMesh.new()
	var m := StandardMaterial3D.new()
	m.albedo_color = Color(0.85, 0.3, 0.25)
	box.mesh.material = m
	box.position = Vector3(4, terrain.get_height_at(Vector3(4, 0, -4), 0.0) + 0.5, -4)
	obj.add_child(box)
	var cam := Camera3D.new()
	if args.has("persp"):
		cam.fov = 50
	else:
		cam.projection = Camera3D.PROJECTION_ORTHOGONAL
		cam.size = float(args.get("size", "30"))
	var dist := float(args.get("dist", "30"))
	cam.position = Vector3(0.6, 1.0, 0.6) * dist
	root3d.add_child(cam)
	cam.look_at(Vector3.ZERO)
	cam.current = true
	renderer = ClassDB.instantiate("PixelRenderer")
	renderer.pixel_size = int(args.get("px", "3"))
	renderer.debug_view = int(args.get("debug", "0"))
	root3d.add_child(renderer)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup()
	if frames == 20:
		var img := root.get_texture().get_image()
		var name: String = args.get("name", "smoke")
		img.save_png("res://tests/captures/%s.png" % name)
		print("SMOKE ", name, " ", renderer.get_status_line())
		quit()
	return false
