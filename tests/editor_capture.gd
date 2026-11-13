# Screenshots the EDITOR window (pixel editor preview + pixel node UI). Installed as a
# temporary @tool autoload by tests/editor_capture.sh (via override.cfg); inert unless the
# user arg `editor_capture` is present.
#   tests/editor_capture.sh <display> scene=res://demo/demo.tscn name=w2_x [preview=1] [no_renderer]
#       [select=PixelSky] [section=Water] [menu=<id>] [time=0.9] [debug=<view>] [frames=240]
#   select=<Class>   select the first node of that class, else of that name (shows the pixel UI)
#   section=<Label>  press that left-toolbar button ("Water", ...), else that strip tab ("Waterfalls")
#   menu=<id>        trigger a "Pixel World" menu entry (0.. = add ADDABLE[id], 100+i = preset i)
#   time=<t>         force PixelDayNight.time_override
#   pixel=<n>        set PixelRenderer.pixel_size
#   node.<Name>.<prop>=<value>  set a property of a node under the scene root
#   nogrid           switch the 3D editor's View Grid / View Origin off
#   filter=<text>    type into the inspector property filter (unfolds matching groups)
#   keep_view        leave the editor camera alone (default: copy the scene's Camera3D)
#   cycle            first select every node saved in the scene in turn (2 frames each; inspector
#                    plugins run for each), then do `select=`; prints EDITOR_CAPTURE cycled <n>
@tool
extends Node

var args := {}
var frame := 0
var target := 240
var view: Camera3D
var selected := false
var cycle_nodes: Array = []
var cycle_at := -1
var scene_ready := false
## Give up when the scene is not open after this many frames (first-run import included).
const SCENE_WAIT_FRAMES := 6000
## Frames to keep retrying `select=` (first editor run: the scene opens while the extension
## and imports are still loading, so the edited root / class lookup can lag behind).
const SELECT_RETRY_FRAMES := 120

func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if not Engine.is_editor_hint() or not args.has("editor_capture"):
		set_process(false)
		return
	target = int(args.get("frames", "240"))
	# The editor only redraws on changes (low-processor mode): render every frame instead.
	OS.low_processor_usage_mode = false
	print("EDITOR_CAPTURE start")

# select=<Class or name>: the first node of that class under the edited scene root (owned or
# not, e.g. inside an instanced rig), else the first node with that name. Retried for
# SELECT_RETRY_FRAMES frames; with `last` set a miss is reported.
func _try_select(last: bool) -> void:
	var want: String = args["select"]
	var root := EditorInterface.get_edited_scene_root()
	var scene_path: String = args.get("scene", "res://demo/demo.tscn")
	var node: Node = null
	if root != null and root.scene_file_path == scene_path:
		if root.is_class(want):
			node = root
		else:
			var nodes := root.find_children("*", want, true, false)
			node = nodes[0] if not nodes.is_empty() else root.find_child(want, true, false)
	if node != null:
		EditorInterface.edit_node(node)
		selected = true
		print("EDITOR_CAPTURE selected ", root.get_path_to(node), " (", node.get_class(), ") at frame ", frame)
	elif last:
		selected = true
		printerr("EDITOR_CAPTURE: no ", want, " in ", root.scene_file_path if root else "<no edited scene>")

func _find(n: String) -> Node:
	return EditorInterface.get_base_control().find_child(n, true, false)

func _process(_d: float) -> void:
	frame += 1
	if frame % 30 == 0:
		print("EDITOR_CAPTURE frame ", frame)
	# The script's timeline (frames 60..target) starts once the requested scene is the edited
	# one, and restarts if the editor switches away from it: on a first run (no .godot cache) the
	# editor is still importing at frame 20 and afterwards restores / opens its own startup scene
	# in a new tab, so fixed frame numbers found no scene, or the wrong one ("no PixelRenderer").
	var scene_path: String = args.get("scene", "res://demo/demo.tscn")
	var edited := EditorInterface.get_edited_scene_root()
	if scene_ready and (edited == null or edited.scene_file_path != scene_path):
		print("EDITOR_CAPTURE editor switched to ", edited.scene_file_path if edited else "<none>", "; restarting")
		scene_ready = false
		selected = false
		view = null
		EditorInterface.open_scene_from_path(scene_path)
	if not scene_ready:
		if edited != null and edited.scene_file_path == scene_path and not EditorInterface.get_resource_filesystem().is_scanning():
			scene_ready = true
			print("EDITOR_CAPTURE scene ready at frame ", frame)
			frame = 59
		elif frame >= 20 and frame % 40 == 20:
			EditorInterface.open_scene_from_path(scene_path)
		if frame > SCENE_WAIT_FRAMES:
			printerr("EDITOR_CAPTURE: scene ", scene_path, " never opened")
			get_tree().quit(1)
		return
	if frame == 60:
		EditorInterface.set_main_screen_editor("3D")
		var root := EditorInterface.get_edited_scene_root()
		if args.has("no_renderer"):
			for r in root.find_children("*", "PixelRenderer", true, false):
				r.get_parent().remove_child(r)
				r.free()
		if args.has("debug"):
			for r in root.find_children("*", "PixelRenderer", true, false):
				r.debug_view = int(args["debug"])
		if args.has("pixel"):
			for r in root.find_children("*", "PixelRenderer", true, false):
				r.pixel_size = int(args["pixel"])
		for k in args:
			# node.<NodeName>.<property>=<value> (as in capture.gd)
			if (k as String).begins_with("node."):
				var parts := (k as String).split(".")
				root.get_node(parts[1]).set(parts[2], str_to_var(args[k]))
		if args.has("time"):
			for d in root.find_children("*", "PixelDayNight", true, false):
				d.time_override = float(args["time"])
		# The scene's own camera (owned by the root), not one inside an instanced rig such as
		# demo/player.tscn (hex_island's PlayerRig camera sits at the origin, under the island).
		if not args.has("keep_view"):
			var cams := root.find_children("*", "Camera3D", true, true)
			if root.has_node("Camera") and root.get_node("Camera") is Camera3D:
				view = root.get_node("Camera")
			elif not cams.is_empty():
				view = cams[0]
	elif frame == 66 and args.has("nogrid"):
		for mb in EditorInterface.get_editor_main_screen().find_children("*", "MenuButton", true, false):
			var pm: PopupMenu = mb.get_popup()
			for i in pm.item_count:
				if pm.get_item_text(i) in ["View Grid", "View Origin"] and pm.is_item_checked(i):
					pm.id_pressed.emit(pm.get_item_id(i))
					print("EDITOR_CAPTURE toggled ", pm.get_item_text(i))
	elif frame == 75 and args.has("menu"):
		var m: MenuButton = _find("PixelWorldMenu")
		if m == null:
			printerr("EDITOR_CAPTURE: no PixelWorldMenu")
		else:
			m.get_popup().id_pressed.emit(int(args["menu"]))
	elif frame == 85 and args.has("section"):
		var tb := _find("PixelizerToolbar")
		var b: Button = tb.find_child(args["section"], true, false) if tb else null
		if b == null:
			# Further sections / pages of a node are tabs of its strip.
			var strip := _find("PixelizerAttributes")
			b = strip.find_child(args["section"], true, false) as Button if strip else null
		if b == null:
			printerr("EDITOR_CAPTURE: no section button ", args["section"])
		else:
			b.pressed.emit()
	elif frame == 80 and args.get("preview", "0") == "1":
		var b: Button = _find("PixelPreviewButton")
		if b == null:
			printerr("EDITOR_CAPTURE: no PixelPreviewButton")
		else:
			b.button_pressed = true
	if frame == target - 30 and args.has("filter"):
		# filter=<text>: type into the inspector's property filter (unfolds matching groups).
		var insp := EditorInterface.get_inspector()
		var dock := insp.get_parent()
		while dock and not dock.find_children("*", "LineEdit", true, false).any(func(l): return (l as LineEdit).placeholder_text.begins_with("Filter")):
			dock = dock.get_parent()
		if dock:
			for l in dock.find_children("*", "LineEdit", true, false):
				if (l as LineEdit).placeholder_text.begins_with("Filter"):
					l.text = args["filter"]
					l.text_changed.emit(args["filter"])
					break
	if args.has("cycle") and frame >= 70 and cycle_at < 0:
		var root := EditorInterface.get_edited_scene_root()
		if root != null:
			cycle_nodes = [root] + root.find_children("*", "", true, true)
			cycle_at = 0
	if cycle_at >= 0 and cycle_at < cycle_nodes.size() * 2:
		if cycle_at % 2 == 0:
			var n: Node = cycle_nodes[cycle_at / 2]
			if is_instance_valid(n):
				EditorInterface.edit_node(n)
		cycle_at += 1
		if cycle_at == cycle_nodes.size() * 2:
			print("EDITOR_CAPTURE cycled ", cycle_nodes.size())
		target = maxi(target, frame + 60)
		return
	if frame >= 70 and args.has("select") and not selected:
		_try_select(frame >= 70 + SELECT_RETRY_FRAMES)
	if frame >= 60 and frame < target - 2 and view:
		# Put the editor camera where the scene's game camera is.
		var ec := EditorInterface.get_editor_viewport_3d(0).get_camera_3d()
		ec.global_transform = view.global_transform
		ec.fov = view.fov
		if view.projection == Camera3D.PROJECTION_ORTHOGONAL:
			ec.projection = Camera3D.PROJECTION_ORTHOGONAL
			ec.size = view.size
	if frame == target:
		var name: String = args.get("name", "editor_capture")
		var path := ProjectSettings.globalize_path("res://tests/captures/%s.png" % name)
		DirAccess.make_dir_recursive_absolute(path.get_base_dir())
		get_viewport().get_texture().get_image().save_png(path)
		print("EDITOR_CAPTURE ", name)
		get_tree().quit()
