@tool
extends Node
## Editor screenshot helper. Registered as an autoload only temporarily by
## tests/terrain_editor_capture.sh; does nothing unless the user args contain `capture=1`.
## Args: scene=res://... node=NodeName tools=Brush,Level,... out=prefix [dir=/abs/out/dir]
##       [focus=NodeName] (frame the view on that node with F) [zoom=N] (wheel steps, <0 = in)
##       [orbit=dx,dy] (middle-drag pixels) [hover=fx,fy] (cursor at a fraction of the 3D view)
##       [option=Name:index,...] (select an item of a named OptionButton, e.g. Stamp Mode:7)
##       [press=Button Name,...] (extra buttons pressed after each tool, e.g. option items)
##       [click=N] (N LMB clicks with a short drag at the hover point, after hovering)
##       [keys=c|v|x] (key presses after the clicks) [hover2=fx,fy] (then move the cursor there)
## Saves <dir or tests/captures>/<out>_<tool>.png for every tool button name, then quits.
## All input goes through the root window (real GUI dispatch), in window coordinates.

var args := {}

func _ready() -> void:
	_log("ready %s" % [OS.get_cmdline_user_args()])
	if not Engine.is_editor_hint():
		return
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.get("capture", "") != "1":
		return
	_run.call_deferred()

func _dir() -> String:
	return args.get("dir", ProjectSettings.globalize_path("res://tests/captures"))

func _log(msg: String) -> void:
	print("editor_capture: ", msg)

func _frames(n: int) -> void:
	for i in n:
		await get_tree().process_frame

func _find_button(root: Node, name: String) -> Button:
	for n in root.find_children(name, "Button", true, false):
		if n is Button and n.is_visible_in_tree():
			return n
	for n in root.find_children(name, "Button", true, false):
		return n
	return null

func _send(ev: InputEvent) -> void:
	get_tree().root.push_input(ev, false)

func _motion(p: Vector2, rel := Vector2.ZERO, mask := 0) -> void:
	var mm := InputEventMouseMotion.new()
	mm.position = p
	mm.global_position = p
	mm.relative = rel
	mm.button_mask = mask
	_send(mm)

func _button(p: Vector2, idx: MouseButton, pressed: bool, mask := 0) -> void:
	var b := InputEventMouseButton.new()
	b.position = p
	b.global_position = p
	b.button_index = idx
	b.pressed = pressed
	b.button_mask = mask
	_send(b)

func _view_rect() -> Rect2:
	var vp := EditorInterface.get_editor_viewport_3d(0)
	var c := vp.get_parent() as Control if vp else null
	return c.get_global_rect() if c else Rect2(0, 0, 1280, 720)

func _run() -> void:
	_log("run")
	await _frames(30)
	_log("open scene")
	var scene: String = args.get("scene", "res://demo/hex_world.tscn")
	EditorInterface.open_scene_from_path(scene)
	await _frames(60)
	EditorInterface.set_main_screen_editor("3D")
	await _frames(20)
	var root := EditorInterface.get_edited_scene_root()
	var r := _view_rect()
	var center := r.get_center()
	if args.has("focus") and root:
		var f := root.find_child(args["focus"], true, false)
		if f:
			EditorInterface.get_selection().clear()
			EditorInterface.get_selection().add_node(f)
			await _frames(10)
			_motion(center)
			await _frames(2)
			var k := InputEventKey.new()
			k.keycode = KEY_F
			k.physical_keycode = KEY_F
			k.pressed = true
			_send(k)
			var k2 := k.duplicate()
			k2.pressed = false
			_send(k2)
			await _frames(40)
		else:
			_log("no focus node " + args["focus"])
	var zoom := int(args.get("zoom", "0"))
	for i in absi(zoom):
		var idx := MOUSE_BUTTON_WHEEL_DOWN if zoom > 0 else MOUSE_BUTTON_WHEEL_UP
		_button(center, idx, true)
		_button(center, idx, false)
		await _frames(2)
	if args.has("orbit"):
		var o: PackedStringArray = args["orbit"].split(",")
		var d := Vector2(float(o[0]), float(o[1]))
		_motion(center)
		_button(center, MOUSE_BUTTON_MIDDLE, true, MOUSE_BUTTON_MASK_MIDDLE)
		for i in 10:
			_motion(center + d * (i + 1) / 10.0, d / 10.0, MOUSE_BUTTON_MASK_MIDDLE)
			await _frames(1)
		_button(center + d, MOUSE_BUTTON_MIDDLE, false)
	await _frames(30)
	var node := root.find_child(args.get("node", "MarchingSquaresTerrain"), true, false) if root else null
	if node:
		EditorInterface.get_selection().clear()
		EditorInterface.get_selection().add_node(node)
		EditorInterface.edit_node(node)
		# set=prop:value;prop:value (values parsed with str_to_var, e.g. chunk_handles:true)
		for kv in args.get("set", "").split(";", false):
			var p: PackedStringArray = String(kv).split(":", true, 1)
			if p.size() == 2:
				node.set(p[0], str_to_var(p[1]))
	await _frames(60)
	var base := EditorInterface.get_base_control()
	var out: String = args.get("out", "editor")
	DirAccess.make_dir_recursive_absolute(_dir())
	var tools: PackedStringArray = args.get("tools", "").split(",", false)
	if tools.is_empty():
		tools = PackedStringArray([""])
	for t in tools:
		if t != "":
			var b := _find_button(base, t)
			if b:
				b.button_pressed = true
				b.pressed.emit()
			else:
				_log("no button " + t)
		await _frames(10)
		for o in args.get("option", "").split(",", false):
			# option=Name:index selects an item of a named OptionButton (e.g. Stamp Mode:7).
			var kv: PackedStringArray = (o as String).split(":")
			var ob := _find_button(base, kv[0]) as OptionButton
			if ob and kv.size() == 2:
				ob.select(int(kv[1]))
				ob.item_selected.emit(int(kv[1]))
			else:
				_log("no option " + o)
			await _frames(5)
		for p in args.get("press", "").split(",", false):
			var pb := _find_button(base, p)
			if pb:
				pb.button_pressed = not pb.button_pressed if pb.toggle_mode else true
				if pb.toggle_mode:
					pb.toggled.emit(pb.button_pressed)
				pb.pressed.emit()
			else:
				_log("no button " + p)
			await _frames(5)
		if args.has("hover"):
			r = _view_rect()
			var hp: PackedStringArray = args["hover"].split(",")
			var at := r.position + r.size * Vector2(float(hp[0]), float(hp[1]))
			for i in 3:
				_motion(at + Vector2(i, 0), Vector2(1, 0))
				await _frames(2)
			if args.has("click"):
				# click=N: N LMB clicks (with a small drag) at the hover point, e.g. to stamp / paint.
				for c in int(args["click"]):
					_button(at, MOUSE_BUTTON_LEFT, true, MOUSE_BUTTON_MASK_LEFT)
					await _frames(2)
					for i in 6:
						_motion(at + Vector2(i * 4, 0), Vector2(4, 0), MOUSE_BUTTON_MASK_LEFT)
						await _frames(1)
					_button(at + Vector2(20, 0), MOUSE_BUTTON_LEFT, false)
					await _frames(5)
			for kname in args.get("keys", "").split("|", false):
				# keys=c|v|x (or ctrl+z): key presses sent to the 3D view (after the clicks).
				var parts: PackedStringArray = kname.split("+")
				var k := InputEventKey.new()
				k.keycode = OS.find_keycode_from_string(parts[parts.size() - 1])
				k.physical_keycode = k.keycode
				k.ctrl_pressed = parts.has("ctrl")
				k.shift_pressed = parts.has("shift")
				k.pressed = true
				_send(k)
				var ku := k.duplicate()
				ku.pressed = false
				_send(ku)
				await _frames(4)
			if args.has("hover2"):
				var hp2: PackedStringArray = args["hover2"].split(",")
				var at2 := r.position + r.size * Vector2(float(hp2[0]), float(hp2[1]))
				for i in 3:
					_motion(at2 + Vector2(i, 0), Vector2(1, 0))
					await _frames(2)
		await _frames(20)
		var img := get_viewport().get_texture().get_image()
		var path := "%s/%s_%s.png" % [_dir(), out, t.to_snake_case() if t != "" else "none"]
		img.save_png(path)
		_log("saved " + path)
	get_tree().quit()
