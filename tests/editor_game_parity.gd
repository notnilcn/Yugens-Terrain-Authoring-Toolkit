@tool
extends Node
## Dumps the lighting state a scene ends up with — every global shader parameter this addon
## publishes, the suns, the effective environment (and its sky), the cameras' environments and
## every water material — as JSON, in the editor or in the running game, so
## tests/editor_game_parity.sh can check that the editor shows the scene lit like the game.
## Headless on both sides (the dummy renderer keeps no global shader values, so the globals
## come from `PixelDiagnostics.get_published_globals()`; on a GPU the RenderingServer's own
## values are dumped as well, under `rs.*`).
##
## Installed as a temporary autoload by the .sh; inert unless the user args contain
## `parity=editor` (headless editor: opens `scene`) or `parity=game` (the scene runs as main
## scene). `out=<file>` is the JSON path, `frames=<n>` how long the scene runs before the dump.
## In the game the day / night clock is stopped and set back to the scene's saved time of day
## just before the dump, so both dumps show the same time; everything else runs as usual.
## A second dump (`at_time.*`) follows on both sides after every PixelDayNight was moved to
## `time=<t>` (default 0.3): the saved scene already holds graded light, so only a time that
## differs from it shows whether the editor really re-applies the lighting.

const EditorReady = preload("res://tests/editor_ready.gd")

var args := {}

func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var mode: String = args.get("parity", "")
	if mode == "editor" and Engine.is_editor_hint():
		_run_editor.call_deferred()
	elif mode == "game" and not Engine.is_editor_hint():
		_run_game.call_deferred()

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

func _run_editor() -> void:
	var scene: String = args.get("scene", "res://demo/hex_island.tscn")
	await EditorReady.wait(get_tree())
	EditorInterface.open_scene_from_path(scene)
	var root: Node = null
	for i in 600:
		await get_tree().process_frame
		root = EditorInterface.get_edited_scene_root()
		if root != null and root.scene_file_path == scene:
			break
	if root == null or root.scene_file_path != scene:
		printerr("PARITY: scene ", scene, " never opened in the editor")
		get_tree().quit(2)
		return
	EditorInterface.set_main_screen_editor("3D")
	await EditorReady.wait(get_tree())
	await _frames(int(args.get("frames", "120")))
	var vp := EditorInterface.get_editor_viewport_3d(0)
	await _dump_both(root, vp.find_world_3d() if vp else null, vp.get_camera_3d() if vp else null)
	get_tree().quit(0)

func _run_game() -> void:
	await _frames(int(args.get("frames", "120")))
	var root := get_tree().current_scene
	if root == null:
		printerr("PARITY: no current scene in the game")
		get_tree().quit(2)
		return
	# Back to the saved time of day (the editor's clock is still), then let it apply.
	var saved := (load(root.scene_file_path) as PackedScene).instantiate()
	for n in root.find_children("*", "PixelDayNight", true, false):
		var s := saved.get_node_or_null(root.get_path_to(n))
		n.set("running", false)
		if s:
			n.set("time_of_day", s.get("time_of_day"))
	saved.free()
	await _frames(3)
	var vp := get_viewport()
	await _dump_both(root, vp.find_world_3d(), vp.get_camera_3d())
	get_tree().quit(0)

## The scene as loaded, then at `time=` (both under one JSON object).
func _dump_both(root: Node, world: World3D, cam: Camera3D) -> void:
	var d := {"mode": args["parity"], "adapter": RenderingServer.get_video_adapter_name()}
	_collect(d, "saved.", root, world, cam)
	for n in root.find_children("*", "PixelDayNight", true, false):
		n.set("time_of_day", float(args.get("time", "0.3")))
	await _frames(10)
	_collect(d, "at_time.", root, world, cam)
	var f := FileAccess.open(args.get("out", "user://parity_%s.json" % args["parity"]), FileAccess.WRITE)
	f.store_string(JSON.stringify(d, "  ", true))
	f.close()
	print("PARITY wrote ", d.size(), " values (", args["parity"], ") to ", args.get("out", ""))

func _collect(out: Dictionary, prefix: String, root: Node, world: World3D, cam: Camera3D) -> void:
	var d := {}
	var globals: Dictionary = ClassDB.class_call_static("PixelDiagnostics", "get_published_globals")
	for k in globals:
		d["global." + str(k)] = _j(globals[k])
		var rs = RenderingServer.global_shader_parameter_get(StringName(k))
		if rs != null:
			d["rs." + str(k)] = _j(rs)
	for n in root.find_children("*", "DirectionalLight3D", true, false):
		var l := n as DirectionalLight3D
		var p := "sun." + str(root.get_path_to(l))
		d[p + ".visible"] = l.is_visible_in_tree()
		d[p + ".color"] = _j(l.light_color)
		d[p + ".energy"] = l.light_energy
		d[p + ".indirect_energy"] = l.light_indirect_energy
		d[p + ".shadow"] = l.shadow_enabled
		d[p + ".shadow_opacity"] = l.shadow_opacity
		d[p + ".dir"] = _j(l.global_basis.z)
		d[p + ".sky_mode"] = l.sky_mode
		d[p + ".cull_mask"] = l.light_cull_mask
	for n in root.find_children("*", "WorldEnvironment", true, false):
		var we := n as WorldEnvironment
		_dump_env(d, "world_env." + str(root.get_path_to(we)), we.environment)
		_dump_props(d, "world_env." + str(root.get_path_to(we)) + ".attributes", we.camera_attributes)
	# What the view actually renders with: the world's environment, unless the camera has its own.
	_dump_env(d, "effective.env", cam.environment if cam and cam.environment else (world.environment if world else null))
	_dump_props(d, "effective.attributes", cam.attributes if cam and cam.attributes else (world.camera_attributes if world else null))
	d["effective.world_has_env"] = world != null and world.environment != null
	# Every light / environment that renders into this view's world, the editor's own preview
	# sun and environment included (they must stay out while the scene has its own).
	var lit: PackedStringArray = []
	for cls in ["DirectionalLight3D", "WorldEnvironment"]:
		for n in get_tree().root.find_children("*", cls, true, false):
			if not n.is_inside_tree() or (n is Node3D and not (n as Node3D).is_visible_in_tree()):
				continue
			if world and n.get_viewport() and n.get_viewport().find_world_3d() != world:
				continue
			lit.append("%s %s" % [cls, ("scene:" + str(root.get_path_to(n))) if root.is_ancestor_of(n) else "outside the scene"])
	lit.sort()
	d["world.lights_and_environments"] = ", ".join(lit)
	# The lighting effect nodes' own settings (a script / the game changing one shows up here).
	for cls in ["PixelDayNight", "PixelSky", "PixelWeather", "PixelWater"]:
		for n in root.find_children("*", cls, true, false):
			_dump_props(d, "node." + str(root.get_path_to(n)), n, true)
		for n in root.find_children("*", "PixelDayNight", true, false):
			d["node.%s.night_factor" % root.get_path_to(n)] = n.call("get_night_factor")
	for n in root.find_children("*", "PixelWater", true, false):
		var p := "water." + str(root.get_path_to(n))
		d[p + ".visible"] = (n as Node3D).is_visible_in_tree()
		_dump_props(d, p + ".material", n.get("material"))
		for g in n.find_children("*", "GeometryInstance3D", true, false):
			var gi := g as GeometryInstance3D
			var gp := p + "/" + str(n.get_path_to(gi))
			d[gp + ".visible"] = gi.is_visible_in_tree()
			_dump_props(d, gp + ".override", gi.material_override)
	# Terrain surface materials: per shader, the sorted set of its distinct materials' values
	# (node order differs between the editor and the game, so there are no per-node keys).
	for t in root.find_children("*", "MarchingSquaresTerrain", true, false):
		var seen := {}
		var by_shader := {}
		for g in t.find_children("*", "GeometryInstance3D", true, false):
			var gi := g as GeometryInstance3D
			var mats: Array = [gi.material_override]
			if gi is MeshInstance3D and (gi as MeshInstance3D).mesh:
				for s in (gi as MeshInstance3D).mesh.get_surface_count():
					mats.append((gi as MeshInstance3D).get_active_material(s))
			for m in mats:
				if not (m is ShaderMaterial and (m as ShaderMaterial).shader) or seen.has(m.get_instance_id()):
					continue
				seen[m.get_instance_id()] = true
				var one := {}
				_dump_props(one, "m", m)
				var file := (m as ShaderMaterial).shader.resource_path.get_file()
				var text := JSON.stringify(one, "", true)
				if not by_shader.has(file):
					by_shader[file] = []
				if not (by_shader[file] as Array).has(text):
					by_shader[file].append(text)
		for file in by_shader:
			var list: Array = by_shader[file]
			list.sort()
			d["terrain.%s.%s" % [root.get_path_to(t), file]] = list
	for k in d:
		out[prefix + k] = d[k]

func _dump_env(d: Dictionary, p: String, env: Environment) -> void:
	_dump_props(d, p, env)
	if env and env.sky:
		_dump_props(d, p + ".sky.material", env.sky.sky_material)

## Every stored property of a resource (shader parameters of a ShaderMaterial included).
## `node`: skip Node / Node3D base properties (transform, process flags, ...).
func _dump_props(d: Dictionary, p: String, r: Object, node := false) -> void:
	if r == null:
		d[p] = null
		return
	d[p + ".class"] = r.get_class()
	var base := {}
	if node:
		for prop in ClassDB.class_get_property_list("Node3D" if r is Node3D else "Node"):
			base[prop["name"]] = true
	for prop in r.get_property_list():
		if not (prop["usage"] & PROPERTY_USAGE_STORAGE):
			continue
		var name: String = prop["name"]
		if base.has(name) or name in ["resource_path", "resource_name", "resource_local_to_scene", "script", "shader", "next_pass"]:
			continue
		var v = r.get(name)
		if v is Object:
			v = v.get_class() if v else null
		d[p + "." + name] = _j(v)

## JSON-friendly form of a variant (vectors / colours as float arrays).
func _j(v):
	match typeof(v):
		TYPE_VECTOR2, TYPE_VECTOR2I:
			return [v.x, v.y]
		TYPE_VECTOR3, TYPE_VECTOR3I:
			return [v.x, v.y, v.z]
		TYPE_VECTOR4, TYPE_VECTOR4I, TYPE_QUATERNION:
			return [v.x, v.y, v.z, v.w]
		TYPE_COLOR:
			return [v.r, v.g, v.b, v.a]
		TYPE_BASIS:
			return [_j(v.x), _j(v.y), _j(v.z)]
		TYPE_TRANSFORM3D:
			return [_j(v.basis), _j(v.origin)]
		TYPE_OBJECT:
			return v.get_class() if v else null
		TYPE_NIL, TYPE_BOOL, TYPE_INT, TYPE_FLOAT, TYPE_STRING:
			return v
		TYPE_STRING_NAME, TYPE_NODE_PATH:
			return str(v)
		_:
			return var_to_str(v)
