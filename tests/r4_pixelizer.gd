# Pixelizer parity check + captures (windowed; under Xvfb: ~/.local/bin/pt-xvfb 49 godot ...):
#   godot --path . --script res://tests/r4_pixelizer.gd -- [name=r4]
# Builds a scene with PixelObjects using different outline palette ids/colours, a nested
# PixelObject (inherit chain), a PixelObjectProfile, a hover highlight, billboard foliage
# (tree_leaves / grass_billboard), a screen-projection mesh, a LUT PNG bake + reload and the
# diagnostics object inspector. Writes tests/captures/<name>_{outlines,ids,diagnostics}.png.
# Exit code 0 = all checks passed.
extends SceneTree

var frames := 0
var args := {}
var renderer
var diag
var fail := false
var red_obj
var child_obj
var blue_obj
var sphere: MeshInstance3D
var blue_mesh: MeshInstance3D
var hover_mesh: MeshInstance3D

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _mat(c: Color) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = c
	return m

func _mesh(mesh: Mesh, c: Color, pos: Vector3, parent: Node) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = mesh
	mi.material_override = _mat(c)
	mi.position = pos
	parent.add_child(mi)
	return mi

func _shader_mat(name: String) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = load("res://addons/MarchingSquaresTerrain/shaders/" + name)
	return m

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
	terrain.chunk_size = 16
	root3d.add_child(terrain)
	for z in range(-1, 1):
		for x in range(-1, 1):
			terrain.add_chunk(Vector2i(x, z))
	terrain.apply_brush("paint", Vector3(0, 0, 0), 12.0, 5.0, 0)

	# Two objects with different outline ids + colours; a nested child inherits pixel size.
	red_obj = ClassDB.instantiate("PixelObject")
	red_obj.name = "RedOutlined"
	red_obj.outline_id = 5
	red_obj.pixel_size = 2
	red_obj.override_outline_color = true
	red_obj.outline_color = Color(0.95, 0.12, 0.1, 1.0)
	root3d.add_child(red_obj)
	_mesh(BoxMesh.new(), Color(0.9, 0.85, 0.7), Vector3(-4, 1.0, 1), red_obj).scale = Vector3(2, 2, 2)
	var sm := SphereMesh.new()
	sphere = _mesh(sm, Color(0.95, 0.9, 0.85), Vector3(-4, 1.2, -3), red_obj)
	sphere.name = "Sphere"
	sphere.scale = Vector3(2.2, 2.2, 2.2)
	child_obj = ClassDB.instantiate("PixelObject")
	child_obj.name = "NestedChild"
	child_obj.outline_id = 70
	child_obj.override_outline_color = true
	child_obj.outline_color = Color(0.1, 0.85, 0.25, 1.0)
	red_obj.add_child(child_obj)
	var cyl := CylinderMesh.new()
	_mesh(cyl, Color(0.85, 0.8, 0.9), Vector3(-1, 1.0, -5), child_obj).scale = Vector3(1.4, 2, 1.4)

	blue_obj = ClassDB.instantiate("PixelObject")
	blue_obj.name = "BlueOutlined"
	blue_obj.outline_id = 120
	blue_obj.override_outline_color = true
	blue_obj.outline_color = Color(0.15, 0.35, 1.0, 1.0)
	blue_obj.palette = 0
	root3d.add_child(blue_obj)
	blue_mesh = _mesh(PrismMesh.new(), Color(0.95, 0.95, 0.95), Vector3(3, 1.2, 2), blue_obj)
	blue_mesh.name = "Prism"
	blue_mesh.scale = Vector3(2.4, 2.4, 2.4)
	var proj := _mesh(BoxMesh.new(), Color.WHITE, Vector3(4, 1.0, -3), blue_obj)
	proj.name = "ScreenProjected"
	proj.material_override = _shader_mat("screen_projection.gdshader")
	proj.scale = Vector3(2, 2, 2)

	# Hover highlight target (default ids).
	var plain = ClassDB.instantiate("PixelObject")
	plain.name = "Plain"
	root3d.add_child(plain)
	hover_mesh = _mesh(TorusMesh.new(), Color(0.8, 0.55, 0.35), Vector3(0, 0.6, 5), plain)
	hover_mesh.name = "HoveredTorus"
	hover_mesh.scale = Vector3(2, 2, 2)

	# Billboard foliage: a tree (trunk + leaf cards) and a grass patch.
	var foliage = ClassDB.instantiate("PixelObject")
	foliage.name = "Foliage"
	foliage.outline_id = 9
	root3d.add_child(foliage)
	var trunk := _mesh(CylinderMesh.new(), Color(0.45, 0.3, 0.2), Vector3(7, 1.0, 6), foliage)
	trunk.scale = Vector3(0.5, 2, 0.5)
	var leaves := _shader_mat("tree_leaves.gdshader")
	for p in [Vector3(7, 3.2, 6), Vector3(6.2, 2.6, 6.4), Vector3(7.8, 2.7, 5.6)]:
		var card := MeshInstance3D.new()
		card.mesh = QuadMesh.new()
		card.material_override = leaves
		card.position = p
		card.scale = Vector3(2.4, 2.4, 2.4)
		foliage.add_child(card)
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.mesh = QuadMesh.new()
	mm.instance_count = 60
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for i in mm.instance_count:
		var t := Transform3D(Basis.IDENTITY.scaled(Vector3(0.9, 0.9, 0.9)), Vector3(rng.randf_range(-9, -5), 0.0, rng.randf_range(4, 9)))
		mm.set_instance_transform(i, t)
	var grass := MultiMeshInstance3D.new()
	grass.multimesh = mm
	grass.material_override = _shader_mat("grass_billboard.gdshader")
	foliage.add_child(grass)

	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = float(args.get("size", "22"))
	cam.position = Vector3(0.6, 0.9, 0.75) * 30.0
	root3d.add_child(cam)
	cam.look_at(Vector3(0, 1, 0))
	cam.current = true

	renderer = ClassDB.instantiate("PixelRenderer")
	renderer.pixel_size = int(args.get("px", "3"))
	root3d.add_child(renderer)

	diag = ClassDB.instantiate("PixelDiagnostics")
	root3d.add_child(diag)

func _process(_d: float) -> bool:
	frames += 1
	var name: String = args.get("name", "r4")
	if frames == 1:
		_setup()
	elif frames == 4:
		# Inherit chain + profile.
		var eff: Dictionary = child_obj.get_effective()
		_check(eff["pixel_size"] == 2, "nested PixelObject inherits pixel_size 2 (got %s)" % eff["pixel_size"])
		_check(eff["outline_id"] == 70, "nested PixelObject keeps its own outline id")
		_check(blue_obj.get_effective()["palette"] == false, "palette opt-out resolved")
		var prof = red_obj.ensure_profile_for(sphere)
		prof.outline_id = 33
		prof.pixel_size = 1
		renderer.highlight_node(hover_mesh, 1)
		# LUT bake to PNG and reload.
		var pal = ClassDB.instantiate("PixelPalette")
		pal.match_metric = 5
		_check(pal.save_lut_png("user://r4_lut.png"), "LUT baked to PNG")
		var img := Image.load_from_file("user://r4_lut.png")
		_check(img != null and img.get_width() == 1024 and img.get_height() == 32, "LUT PNG is 1024x32")
		var pal2 = ClassDB.instantiate("PixelPalette")
		pal2.lut_image = ImageTexture.create_from_image(img)
		var t = pal2.bake()
		_check(t != null and t.get_width() == 1024, "LUT PNG loads back as lut_image")
	elif frames == 8:
		var f := int(sphere.get_instance_shader_parameter("pt_flags"))
		_check((f >> 8) & 255 == 33 and (f & 3) == 0, "profile overrides sphere id/pixel size (flags %d)" % f)
		var hf := int(hover_mesh.get_instance_shader_parameter("pt_flags"))
		_check((hf >> 8) & 255 == 254, "hover highlight id 254 applied")
	elif frames == 30:
		root.get_texture().get_image().save_png("res://tests/captures/%s_outlines.png" % name)
		renderer.debug_view = 4
	elif frames == 36:
		root.get_texture().get_image().save_png("res://tests/captures/%s_ids.png" % name)
		renderer.debug_view = 0
		diag.set_overlay_visible(true)
		diag.select_node(blue_mesh)
	elif frames == 50:
		_check(diag.get_selected_node() == blue_mesh, "diagnostics selection")
		var cam := root.get_viewport().get_camera_3d()
		var picked = diag.pick_at(cam.unproject_position(hover_mesh.global_position + Vector3(1.5, 0.2, 0.0)))
		_check(picked == hover_mesh, "diagnostics picker hits the torus (got %s)" % [picked])
		root.get_texture().get_image().save_png("res://tests/captures/%s_diagnostics.png" % name)
		print("R4 ", renderer.get_status_line())
		quit(1 if fail else 0)
	return false
