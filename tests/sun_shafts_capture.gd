# Windowed check of the sun-shaft sheets in the Compatibility renderer: a roof on four pillars
# under a shadow-casting sun. The sheets must glow where the sun reaches and stay clear in the
# roof's shadow (tests/captures/<name>.png).
#   godot --path . --script res://tests/sun_shafts_capture.gd -- [node] [renderer] [alpha=0.05] [bands=0] [count=3] [name=sun_shafts]
# Without `node` the shader is drawn on three plain sheets (robert-k's demo setup); with `node`
# a PixelSunShafts builds them (needs the sun_shafts feature on).
extends SceneTree

var frames := 0
var args := {}

func _setup() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var root3d := Node3D.new()
	root.add_child(root3d)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.1, 0.1, 0.2)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.58, 0.6, 0.68)
	env.environment = e
	root3d.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.shadow_enabled = true
	root3d.add_child(sun)
	var ground := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(40, 40)
	ground.mesh = pm
	var gm := StandardMaterial3D.new()
	gm.albedo_color = Color(0.53, 0.67, 0.36)
	ground.material_override = gm
	root3d.add_child(ground)
	for i in 4:
		var pillar := MeshInstance3D.new()
		var bm := BoxMesh.new()
		bm.size = Vector3(1, 6, 1)
		pillar.mesh = bm
		pillar.position = Vector3(-6 + i * 4, 3, -2 + (i % 2) * 3)
		root3d.add_child(pillar)
	var roof := MeshInstance3D.new()
	var rm := BoxMesh.new()
	rm.size = Vector3(8, 0.5, 8)
	roof.mesh = rm
	roof.position = Vector3(0, 9, 0)
	root3d.add_child(roof)
	if args.has("node"):
		var shafts = ClassDB.instantiate("PixelSunShafts")
		shafts.alpha = float(args.get("alpha", "0.05"))
		shafts.count = int(args.get("count", "3"))
		root3d.add_child(shafts)
	else:
		var mat := ShaderMaterial.new()
		mat.shader = load("res://addons/MarchingSquaresTerrain/shaders/sun_shafts.gdshader")
		mat.set_shader_parameter("alpha", float(args.get("alpha", "0.05")))
		mat.set_shader_parameter("shaft_steps", int(args.get("bands", "0")))
		var q := QuadMesh.new()
		q.size = Vector2(200, 200)
		q.material = mat
		var basis := Basis.looking_at(sun.global_basis.y)
		for i in 3:
			var mi := MeshInstance3D.new()
			mi.mesh = q
			mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
			var offset := (float(i) / 3.0 - 0.5) * 40.0 * -sun.global_basis.y
			mi.transform = Transform3D(basis, offset)
			root3d.add_child(mi)
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 24
	cam.position = Vector3(0.6, 0.8, 0.6) * 40
	root3d.add_child(cam)
	cam.look_at(Vector3(0, 2, 0))
	cam.current = true
	if args.has("renderer"):
		var r = ClassDB.instantiate("PixelRenderer")
		r.pixel_size = 3
		root3d.add_child(r)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup()
	if frames == 20:
		var img := root.get_texture().get_image()
		var name: String = args.get("name", "sun_shafts")
		img.save_png("res://tests/captures/%s.png" % name)
		print("SUN_SHAFTS ", name)
		quit()
	return false
