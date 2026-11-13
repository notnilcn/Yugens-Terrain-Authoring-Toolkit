# Fixed overview camera shared by tests/mst_import_check.gd (this project) and the original
# GDScript MarchingSquaresTerrain project, so imports can be compared shot for shot.
#   view=top     orthographic, straight down
#   view=iso     orthographic, 3/4 view
# Running it inside the original project (reference shot of its demo terrain only):
#   godot --path <original> --script <this project>/tests/mst_view.gd -- \
#         scene=res://demo/demo.tscn only=MarchingSquaresTerrain out=/abs/r5_original.png [view=iso]
extends SceneTree

const CENTER := Vector3(0, 0, 0)

var frames := 0
var args := {}
var cam: Camera3D

static func add_neutral_lighting(parent: Node) -> void:
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.55, 0.72, 0.86)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.6, 0.62, 0.7)
	e.ambient_light_energy = 0.6
	env.environment = e
	parent.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, 35, 0)
	parent.add_child(sun)

static func make_camera(view: String, size: float) -> Camera3D:
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = size
	cam.far = 2000.0
	if view == "top":
		cam.position = CENTER + Vector3(0, 400, 0.01)
	else:
		cam.position = CENTER + Vector3(1.0, 1.1, 1.0) * 250.0
	return cam

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		for a in OS.get_cmdline_user_args():
			var kv := a.split("=")
			args[kv[0]] = kv[1] if kv.size() > 1 else "1"
		if args.has("scene"):
			var scene: Node = load(args["scene"]).instantiate()
			if args.has("only"):
				# Just that node (e.g. the terrain) under a neutral light, without the scene's
				# own cameras / post-processing.
				var n := scene.find_child(args["only"], true, false)
				n.get_parent().remove_child(n)
				scene.free()
				add_neutral_lighting(root)
				root.add_child(n)
			else:
				root.add_child(scene)
		cam = make_camera(args.get("view", "iso"), float(args.get("size", "150")))
		root.add_child(cam)
		cam.look_at(CENTER)
		cam.current = true
	elif frames > 3:
		cam.current = true  # scene scripts may grab the view back
	if frames == int(args.get("frames", "90")):
		root.get_texture().get_image().save_png(args.get("out", "user://mst_view.png"))
		print("MST_VIEW saved ", args.get("out", ""))
		quit()
	return false
