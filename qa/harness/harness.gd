@tool
extends EditorPlugin
## QA harness for the MarchingSquaresTerrain editor UI.
##
## Listens on 127.0.0.1:$MST_QA_PORT (default 8791) for newline-delimited JSON
## requests and answers with one JSON line each. It exists only to let an
## external driver inspect and drive the *real* editor: selection, scene
## opening, UI dumps with global rects, synthetic input, undo/redo state and
## terrain snapshots. It does not change addon behaviour.

const DEFAULT_PORT := 8791
const MAX_LINE := 1 << 22

var _server: TCPServer
var _peers: Array[StreamPeerTCP] = []
var _buffers: Dictionary = {}
var _port := DEFAULT_PORT

func _enter_tree() -> void:
	var env_port := OS.get_environment("MST_QA_PORT")
	if env_port != "":
		_port = int(env_port)
	_server = TCPServer.new()
	var err := _server.listen(_port, "127.0.0.1")
	if err != OK:
		push_error("MST QA harness: cannot listen on 127.0.0.1:%d (%s)" % [_port, error_string(err)])
		_server = null
	Engine.max_fps = 30
	set_process(true)
	print("MST QA harness listening on 127.0.0.1:%d" % _port)

func _exit_tree() -> void:
	for p in _peers:
		if is_instance_valid(p):
			p.disconnect_from_host()
	_peers.clear()
	if _server:
		_server.stop()
		_server = null

func _process(_delta: float) -> void:
	if _server == null:
		return
	while _server.is_connection_available():
		var peer := _server.take_connection()
		_peers.append(peer)
		_buffers[peer.get_instance_id()] = PackedByteArray()
	var dead: Array[StreamPeerTCP] = []
	for peer in _peers:
		if not is_instance_valid(peer) or peer.get_status() != StreamPeerTCP.STATUS_CONNECTED:
			dead.append(peer)
			continue
		var n := peer.get_available_bytes()
		if n <= 0:
			continue
		var got := peer.get_partial_data(n)
		if got[0] != OK:
			dead.append(peer)
			continue
		var buf: PackedByteArray = _buffers.get(peer.get_instance_id(), PackedByteArray())
		buf.append_array(got[1])
		if buf.size() > MAX_LINE:
			_respond(peer, {"ok": false, "error": "request too large"})
			buf = PackedByteArray()
			continue
		while true:
			var idx := buf.find(10) # '\n'
			if idx < 0:
				break
			var line := buf.slice(0, idx).get_string_from_utf8()
			buf = buf.slice(idx + 1)
			if line.strip_edges() != "":
				_dispatch(peer, line)
		_buffers[peer.get_instance_id()] = buf
	for peer in dead:
		_buffers.erase(peer.get_instance_id())
		_peers.erase(peer)

func _respond(peer: StreamPeerTCP, data: Dictionary) -> void:
	var line := JSON.stringify(data) + "\n"
	peer.put_data(line.to_utf8_buffer())

func _dispatch(peer: StreamPeerTCP, line: String) -> void:
	var parsed: Variant = JSON.parse_string(line)
	if typeof(parsed) != TYPE_DICTIONARY:
		_respond(peer, {"ok": false, "error": "expected a JSON object"})
		return
	var req: Dictionary = parsed
	var cmd := str(req.get("cmd", ""))
	var res: Dictionary
	match cmd:
		"ping":
			res = {"ok": true, "pong": true, "harness": "mst-qa"}
		"info":
			res = _cmd_info()
		"select":
			res = _cmd_select(req)
		"open_scene":
			res = _cmd_open_scene(req)
		"save_scene":
			res = _cmd_save_scene()
		"main_screen":
			res = _cmd_main_screen(req)
		"ui":
			res = _cmd_ui(req)
		"windows":
			res = _cmd_windows()
		"popups":
			res = _cmd_popups()
		"ensure_visible":
			res = await _cmd_ensure_visible(req)
		"window_resize":
			res = await _cmd_window_resize(req)
		"control_at":
			res = _cmd_control_at(req)
		"click":
			res = await _cmd_click(req)
		"move":
			res = _cmd_move(req)
		"key":
			res = await _cmd_key(req)
		"unfocus":
			res = _cmd_unfocus()
		"undo":
			res = _cmd_undo(false)
		"redo":
			res = _cmd_undo(true)
		"undo_state":
			res = _cmd_undo_state()
		"prop":
			res = _cmd_prop(req)
		"set_prop":
			res = _cmd_set_prop(req)
		"call":
			res = _cmd_call(req)
		"nodes":
			res = _cmd_nodes(req)
		"terrain":
			res = _cmd_terrain(req)
		"capture":
			res = await _cmd_capture(req)
		"resize":
			res = await _cmd_resize(req)
		"console":
			res = _cmd_console(req)
		"quit":
			_respond(peer, {"ok": true})
			get_tree().quit()
			return
		_:
			res = {"ok": false, "error": "unknown cmd '%s'" % cmd}
	if not res.has("ok"):
		res["ok"] = true
	_respond(peer, res)

# ----------------------------------------------------------------- helpers

func _editor_window() -> Window:
	return get_tree().root

func _edited_root() -> Node:
	return EditorInterface.get_edited_scene_root()

func _find_scene_node(path: String) -> Node:
	if path.begins_with("/root/"):
		return get_node_or_null(NodePath(path))
	var root := _edited_root()
	if root == null:
		return null
	if path == "" or path == ".":
		return root
	var rel := path
	var root_name := str(root.name)
	if rel == root_name:
		return root
	if rel.begins_with(root_name + "/"):
		rel = rel.substr(root_name.length() + 1)
	var direct := root.get_node_or_null(NodePath(rel))
	if direct:
		return direct
	# Fall back to a unique name search under the scene root.
	var matches := root.find_children(rel.get_file(), "", true, false)
	if matches.size() == 1:
		return matches[0]
	return null

func _find_by_class(type_name: String) -> Node:
	var root := _edited_root()
	if root == null:
		return null
	if root.is_class(type_name):
		return root
	var stack: Array[Node] = [root]
	while not stack.is_empty():
		var n: Node = stack.pop_back()
		if n.is_class(type_name):
			return n
		for c in n.get_children():
			stack.append(c)
	return null

func _jsonable(v: Variant, depth := 0) -> Variant:
	if depth > 6:
		return "<deep>"
	match typeof(v):
		TYPE_NIL, TYPE_BOOL, TYPE_INT, TYPE_FLOAT, TYPE_STRING:
			return v
		TYPE_STRING_NAME:
			return str(v)
		TYPE_VECTOR2:
			return [v.x, v.y]
		TYPE_VECTOR2I:
			return [v.x, v.y]
		TYPE_VECTOR3:
			return [v.x, v.y, v.z]
		TYPE_VECTOR3I:
			return [v.x, v.y, v.z]
		TYPE_RECT2:
			return [v.position.x, v.position.y, v.size.x, v.size.y]
		TYPE_COLOR:
			return [v.r, v.g, v.b, v.a]
		TYPE_ARRAY:
			var out: Array = []
			for e in v:
				out.append(_jsonable(e, depth + 1))
			return out
		TYPE_PACKED_FLOAT32_ARRAY:
			var out: Array = []
			for i in range(mini(v.size(), 4096)):
				out.append(v[i])
			return out
		TYPE_PACKED_BYTE_ARRAY:
			return {"size": v.size(), "hash": hash(v)}
		TYPE_DICTIONARY:
			var out := {}
			for k in v.keys():
				out[str(k)] = _jsonable(v[k], depth + 1)
			return out
		TYPE_OBJECT:
			if v == null:
				return null
			if v is Node:
				return {"class": v.get_class(), "path": str(v.get_path()), "name": str(v.name)}
			if v is Resource:
				return {"class": v.get_class(), "resource_path": str(v.resource_path)}
			return {"class": v.get_class()}
		_:
			return str(v)

func _control_snapshot(c: Control, root_rect: Rect2) -> Dictionary:
	var r := c.get_global_rect()
	var text := ""
	if "text" in c:
		text = str(c.get("text"))
	var items: Array = []
	if c is OptionButton:
		for i in range(c.item_count):
			items.append(c.get_item_text(i))
	var value: Variant = null
	if c is Range:
		value = c.value
	var d := {
		"name": str(c.name),
		"class": c.get_class(),
		"path": str(c.get_path()),
		"rect": [r.position.x, r.position.y, r.size.x, r.size.y],
		"min": [c.get_combined_minimum_size().x, c.get_combined_minimum_size().y],
		"custom_min": [c.custom_minimum_size.x, c.custom_minimum_size.y],
		"visible": c.is_visible_in_tree(),
		"focus": c.has_focus(),
		"disabled": c is BaseButton and c.disabled,
		"text": text,
		"tooltip": c.tooltip_text,
		"clip_contents": c.clip_contents,
		"size_flags_h": c.size_flags_horizontal,
		"size_flags_v": c.size_flags_vertical,
		"outside_root": not root_rect.encloses(r),
		"clipped": _is_clipped(c),
	}
	if c is BaseButton:
		d["toggled"] = c.button_pressed
	if c is Range:
		d["value"] = value
		d["min_value"] = c.min_value
		d["max_value"] = c.max_value
	if items.size() > 0:
		d["items"] = items
		d["selected"] = c.selected
	if c is LineEdit:
		d["placeholder"] = c.placeholder_text
	if c is Label:
		d["autowrap"] = c.autowrap_mode
	if c is ScrollContainer:
		d["scroll_h"] = c.scroll_horizontal
		d["scroll_v"] = c.scroll_vertical
	return d

func _is_clipped(c: Control) -> bool:
	var p := c.get_parent()
	while p is Control:
		var pc: Control = p
		if pc.clip_contents and not pc.get_global_rect().encloses(c.get_global_rect()):
			return true
		p = p.get_parent()
	return false

func _walk_controls(node: Node, out: Array, root_rect: Rect2, depth: int) -> void:
	if node is Control:
		out.append(_control_snapshot(node, root_rect))
	for child in node.get_children(true):
		_walk_controls(child, out, root_rect, depth + 1)

func _now_ms() -> int:
	return Time.get_ticks_msec()

# ----------------------------------------------------------------- commands

func _cmd_info() -> Dictionary:
	var win := _editor_window()
	var vp := win.get_visible_rect()
	var scene := ""
	if _edited_root():
		scene = _edited_root().scene_file_path
	return {
		"ok": true,
		"window_pos": [win.position.x, win.position.y],
		"window_size": [win.size.x, win.size.y],
		"viewport_size": [vp.size.x, vp.size.y],
		"screen_size": [DisplayServer.screen_get_size().x, DisplayServer.screen_get_size().y],
		"screen_pos": [DisplayServer.screen_get_position().x, DisplayServer.screen_get_position().y],
		"screen_scale": DisplayServer.screen_get_scale(),
		"editor_scale": EditorInterface.get_editor_scale(),
		"scene": scene,
		"project": ProjectSettings.get_setting("application/config/name", ""),
		"fps": Engine.get_frames_per_second(),
		"selection": _selection_paths(),
	}

func _selection_paths() -> Array:
	var out: Array = []
	var sel := EditorInterface.get_selection()
	if sel:
		for n in sel.get_selected_nodes():
			out.append(str(n.get_path()))
	return out

func _cmd_select(req: Dictionary) -> Dictionary:
	var node: Node = null
	if req.has("path"):
		node = _find_scene_node(str(req["path"]))
	elif req.has("class"):
		node = _find_by_class(str(req["class"]))
	if node == null:
		return {"ok": false, "error": "node not found"}
	var sel := EditorInterface.get_selection()
	sel.clear()
	sel.add_node(node)
	var ok := false
	if node is Node3D:
		EditorInterface.edit_node(node)
		ok = true
	return {"ok": true, "path": str(node.get_path()), "class": node.get_class(), "edited": ok}

func _cmd_open_scene(req: Dictionary) -> Dictionary:
	var path := str(req.get("path", ""))
	if not ResourceLoader.exists(path):
		return {"ok": false, "error": "scene does not exist: %s" % path}
	EditorInterface.open_scene_from_path(path)
	return {"ok": true, "path": path}

func _cmd_save_scene() -> Dictionary:
	var root := _edited_root()
	if root == null:
		return {"ok": false, "error": "no scene"}
	var err := EditorInterface.save_scene()
	return {"ok": err == OK, "error": error_string(err) if err != OK else ""}

func _cmd_main_screen(req: Dictionary) -> Dictionary:
	var name := str(req.get("name", "3D"))
	EditorInterface.set_main_screen_editor(name)
	return {"ok": true, "main_screen": name}

func _cmd_ui(req: Dictionary) -> Dictionary:
	var root: Node = null
	match str(req.get("root", "base")):
		"base":
			root = EditorInterface.get_base_control()
		"all":
			root = get_tree().root
		"scene":
			root = _edited_root()
		"path":
			root = get_node_or_null(NodePath(str(req.get("path", ""))))
		_:
			root = EditorInterface.get_base_control()
	if root == null:
		return {"ok": false, "error": "ui root not found"}
	var win := _editor_window()
	var root_rect := Rect2(Vector2.ZERO, Vector2(win.size))
	var out: Array = []
	_walk_controls(root, out, root_rect, 0)
	# Filter to a subtree path if provided.
	var filter := str(req.get("filter", ""))
	if filter != "":
		var f: Array = []
		for d in out:
			if str(d["path"]).find(filter) >= 0 or str(d["text"]).find(filter) >= 0:
				f.append(d)
		out = f
	return {"ok": true, "window_pos": [win.position.x, win.position.y], "controls": out}

func _cmd_windows() -> Dictionary:
	var out: Array = []
	var stack: Array[Node] = [get_tree().root]
	while not stack.is_empty():
		var n: Node = stack.pop_back()
		if n is Window and n != get_tree().root:
			var w: Window = n
			out.append({
				"title": w.title,
				"name": str(w.name),
				"path": str(w.get_path()),
				"position": [w.position.x, w.position.y],
				"size": [w.size.x, w.size.y],
				"visible": w.visible,
				"transient": w.transient,
				"exclusive": w.exclusive,
				"resizable": w.unresizable == false,
				"min": [w.min_size.x, w.min_size.y],
				"max": [w.max_size.x, w.max_size.y],
			})
		for c in n.get_children(true):
			stack.append(c)
	return {"ok": true, "windows": out}

func _cmd_popups() -> Dictionary:
	var out: Array = []
	var stack: Array[Node] = [get_tree().root]
	while not stack.is_empty():
		var n: Node = stack.pop_back()
		if n is PopupMenu and n.visible:
			var p: PopupMenu = n
			var items: Array = []
			for i in range(p.get_item_count()):
				if p.is_item_separator(i):
					items.append({"index": i, "separator": true})
					continue
				items.append({
					"index": i, "text": p.get_item_text(i), "id": p.get_item_id(i),
					"disabled": p.is_item_disabled(i),
					"checked": p.is_item_checked(i) if p.is_item_checkable(i) else null,
					"submenu": p.get_item_submenu(i),
					"accel": p.get_item_accelerator(i),
				})
			out.append({
				"name": str(p.name), "path": str(p.get_path()),
				"pos": [p.position.x, p.position.y], "size": [p.size.x, p.size.y],
				"items": items,
			})
		for c in n.get_children(true):
			stack.append(c)
	return {"ok": true, "popups": out}

func _cmd_ensure_visible(req: Dictionary) -> Dictionary:
	var target: Control = null
	if req.has("path"):
		var n := get_node_or_null(NodePath(str(req["path"])))
		if n is Control:
			target = n as Control
	if target == null:
		var text := str(req.get("text", ""))
		var found := _find_control_by_text(EditorInterface.get_base_control(), text)
		target = found
	if target == null:
		return {"ok": false, "error": "control not found"}
	var scroll: Node = target.get_parent()
	while scroll != null and not (scroll is ScrollContainer):
		scroll = scroll.get_parent()
	if scroll == null:
		return {"ok": false, "error": "no ScrollContainer ancestor"}
	var sc := scroll as ScrollContainer
	sc.ensure_control_visible(target)
	await get_tree().process_frame
	await get_tree().process_frame
	await get_tree().process_frame
	var r := target.get_global_rect()
	return {"ok": true, "rect": [r.position.x, r.position.y, r.size.x, r.size.y], "path": str(target.get_path())}

func _find_control_by_text(node: Node, text: String) -> Control:
	var best: Control = null
	var stack: Array[Node] = [node]
	while not stack.is_empty():
		var n: Node = stack.pop_back()
		if n is Control and n.is_visible_in_tree():
			var t := ""
			if "text" in n:
				t = str(n.get("text"))
			if t == text:
				return n
			if t.to_lower().contains(text.to_lower()) and best == null:
				best = n
		for c in n.get_children(true):
			stack.append(c)
	return best

func _cmd_window_resize(req: Dictionary) -> Dictionary:
	var target: Window = null
	if req.has("path"):
		var n := get_node_or_null(NodePath(str(req["path"])))
		if n is Window:
			target = n as Window
	if target == null and req.has("title"):
		var title := str(req["title"])
		var stack: Array[Node] = [get_tree().root]
		while not stack.is_empty():
			var n: Node = stack.pop_back()
			if n is Window and n != get_tree().root:
				var w: Window = n
				if w.visible and (title == "" or w.title == title):
					target = w
					break
			for c in n.get_children(true):
				stack.append(c)
	if target == null:
		return {"ok": false, "error": "window not found"}
	var desired_w := int(req.get("width", 800))
	var desired_h := int(req.get("height", 600))
	target.size = Vector2i(desired_w, desired_h)
	await get_tree().process_frame
	await get_tree().process_frame
	await get_tree().process_frame
	return {
		"ok": true,
		"title": target.title,
		"actual": [target.size.x, target.size.y],
		"resizable": not target.unresizable,
		"min": [target.min_size.x, target.min_size.y],
		"max": [target.max_size.x, target.max_size.y],
	}

func _cmd_control_at(req: Dictionary) -> Dictionary:
	var pos := Vector2(float(req.get("x", 0)), float(req.get("y", 0)))
	var base := EditorInterface.get_base_control()
	var found := _hit_test(base, pos)
	if found == null:
		return {"ok": false, "error": "no control at %s" % pos}
	return {"ok": true, "control": _control_snapshot(found, Rect2(Vector2.ZERO, Vector2(_editor_window().size)))}

func _hit_test(node: Node, pos: Vector2) -> Control:
	if node is Control:
		var c: Control = node
		if not c.is_visible_in_tree():
			return null
		if not c.get_global_rect().has_point(pos):
			return null
		for child in c.get_children(true):
			var hit := _hit_test(child, pos)
			if hit != null:
				return hit
		if c.mouse_filter != Control.MOUSE_FILTER_IGNORE:
			return c
	return null

func _cmd_move(req: Dictionary) -> Dictionary:
	var pos := Vector2(float(req.get("x", 0)), float(req.get("y", 0)))
	var ev := InputEventMouseMotion.new()
	ev.position = pos
	ev.global_position = pos
	Input.parse_input_event(ev)
	return {"ok": true}

func _cmd_click(req: Dictionary) -> Dictionary:
	var pos := Vector2(float(req.get("x", 0)), float(req.get("y", 0)))
	var button := int(req.get("button", MOUSE_BUTTON_LEFT))
	var hold_frames := int(req.get("hold_frames", 1))
	_send_mouse_button(pos, button, true)
	for i in range(max(hold_frames, 1)):
		await get_tree().process_frame
	_send_mouse_button(pos, button, false)
	await get_tree().process_frame
	return {"ok": true}

func _send_mouse_button(pos: Vector2, button: int, pressed: bool) -> void:
	var ev := InputEventMouseButton.new()
	ev.position = pos
	ev.global_position = pos
	ev.button_index = button
	ev.pressed = pressed
	Input.parse_input_event(ev)

func _keycode(name: String) -> int:
	var k := name.to_upper()
	match k:
		"CTRL": return KEY_CTRL
		"SHIFT": return KEY_SHIFT
		"ALT": return KEY_ALT
		"ESC", "ESCAPE": return KEY_ESCAPE
		"ENTER", "RETURN": return KEY_ENTER
		"TAB": return KEY_TAB
		"SPACE": return KEY_SPACE
		"BACKSPACE": return KEY_BACKSPACE
		"DELETE": return KEY_DELETE
		"F": return KEY_F
		"G": return KEY_G
		"Z": return KEY_Z
		"X": return KEY_X
		"Y": return KEY_Y
		"V": return KEY_V
		"C": return KEY_C
		"M": return KEY_M
		"H": return KEY_H
		"W": return KEY_W
		"E": return KEY_E
		"I": return KEY_I
		"B": return KEY_B
		"U": return KEY_U
		"L": return KEY_L
		"J": return KEY_J
		"K": return KEY_K
		"T": return KEY_T
		"D": return KEY_D
		"R": return KEY_R
		"S": return KEY_S
		"Q": return KEY_Q
		"A": return KEY_A
		"P": return KEY_P
		"N": return KEY_N
		"O": return KEY_O
		"0": return KEY_0
		"1": return KEY_1
		"2": return KEY_2
		"3": return KEY_3
		"4": return KEY_4
		"5": return KEY_5
		"6": return KEY_6
		"7": return KEY_7
		"8": return KEY_8
		"9": return KEY_9
	return KEY_NONE

func _cmd_key(req: Dictionary) -> Dictionary:
	var keyname := str(req.get("key", ""))
	var code := _keycode(keyname)
	if code == KEY_NONE:
		return {"ok": false, "error": "unknown key '%s'" % keyname}
	if bool(req.get("unfocus", true)):
		_unfocus()
	var ev := InputEventKey.new()
	ev.keycode = code
	ev.physical_keycode = code
	ev.ctrl_pressed = bool(req.get("ctrl", false))
	ev.shift_pressed = bool(req.get("shift", false))
	ev.alt_pressed = bool(req.get("alt", false))
	ev.pressed = true
	Input.parse_input_event(ev)
	await get_tree().process_frame
	var up := ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	await get_tree().process_frame
	return {"ok": true}

func _cmd_unfocus() -> Dictionary:
	return {"ok": true, "released": _unfocus()}

func _unfocus() -> bool:
	var focus := _editor_window().gui_get_focus_owner()
	if focus:
		focus.release_focus()
		return true
	return false

func _undo_manager() -> EditorUndoRedoManager:
	return EditorInterface.get_editor_undo_redo()

func _history() -> UndoRedo:
	var mgr := _undo_manager()
	if mgr == null:
		return null
	var target := _edited_root()
	var id := EditorUndoRedoManager.GLOBAL_HISTORY
	if target:
		var oid: int = mgr.get_object_history_id(target)
		if oid >= 0:
			id = oid
	return mgr.get_history_undo_redo(id)

func _cmd_undo(is_redo: bool) -> Dictionary:
	var h := _history()
	if h == null:
		return {"ok": false, "error": "no undo history"}
	var ok := h.redo() if is_redo else h.undo()
	return {"ok": ok, "action": h.get_current_action_name(), "version": h.get_version()}

func _cmd_undo_state() -> Dictionary:
	var h := _history()
	if h == null:
		return {"ok": false, "error": "no undo history"}
	return {
		"ok": true,
		"action": h.get_current_action_name(),
		"version": h.get_version(),
		"undo_count": h.get_history_count(),
		"has_undo": h.has_undo(),
		"has_redo": h.has_redo(),
	}

func _cmd_resize(req: Dictionary) -> Dictionary:
	# Godot's editor layout is not stable below ~1024x600; forcing it can crash
	# the editor while the harness is awaiting frames.
	var w := clampi(int(req.get("width", 1600)), 1024, 3840)
	var h := clampi(int(req.get("height", 600)), 600, 2160)
	DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_WINDOWED)
	DisplayServer.window_set_size(Vector2i(w, h))
	_editor_window().size = Vector2i(w, h)
	await get_tree().process_frame
	await get_tree().process_frame
	await get_tree().process_frame
	return {"ok": true, "window_size": [w, h], "actual": [DisplayServer.window_get_size().x, DisplayServer.window_get_size().y]}

func _get_node_from_req(req: Dictionary) -> Node:
	if req.has("path"):
		var p := str(req["path"])
		if p.begins_with("/root/"):
			return get_node_or_null(NodePath(p))
		return _find_scene_node(p)
	if req.has("class"):
		return _find_by_class(str(req["class"]))
	return null

func _cmd_prop(req: Dictionary) -> Dictionary:
	var node := _get_node_from_req(req)
	if node == null:
		return {"ok": false, "error": "node not found"}
	var prop := str(req.get("prop", ""))
	if prop == "" or not (prop in node):
		return {"ok": false, "error": "no property '%s' on %s" % [prop, node.get_class()]}
	return {"ok": true, "value": _jsonable(node.get(prop))}

func _cmd_set_prop(req: Dictionary) -> Dictionary:
	var node := _get_node_from_req(req)
	if node == null:
		return {"ok": false, "error": "node not found"}
	var prop := str(req.get("prop", ""))
	if prop == "" or not (prop in node):
		return {"ok": false, "error": "no property '%s'" % prop}
	node.set(prop, req.get("value"))
	return {"ok": true, "value": _jsonable(node.get(prop))}

func _cmd_call(req: Dictionary) -> Dictionary:
	var node := _get_node_from_req(req)
	if node == null:
		return {"ok": false, "error": "node not found"}
	var method := str(req.get("method", ""))
	if method == "" or not node.has_method(method):
		return {"ok": false, "error": "no method '%s'" % method}
	var args: Array = []
	if req.get("args") is Array:
		args = req["args"]
	var result: Variant = node.callv(method, args)
	if result is Object and not (result is Resource) and not (result is Node):
		return {"ok": true, "value": null}
	return {"ok": true, "value": _jsonable(result)}

func _cmd_nodes(req: Dictionary) -> Dictionary:
	var type_name := str(req.get("class", "Node"))
	var base: Node = _edited_root()
	if base == null:
		return {"ok": false, "error": "no scene"}
	var out: Array = []
	var stack: Array[Node] = [base]
	while not stack.is_empty():
		var n: Node = stack.pop_back()
		if n.is_class(type_name):
			out.append({"path": str(n.get_path()), "name": str(n.name), "class": n.get_class()})
		for c in n.get_children():
			stack.append(c)
	return {"ok": true, "nodes": out}

## Terrain digest used by undo tests. Reports chunk count + a rolling hash of
## every chunk blob and the sample heights of the requested chunk (or chunk 0,0).
func _cmd_terrain(req: Dictionary) -> Dictionary:
	var node := _get_node_from_req(req) if (req.has("path") or req.has("class")) else _find_by_class("MarchingSquaresTerrain")
	if node == null:
		return {"ok": false, "error": "no MarchingSquaresTerrain in scene"}
	var chunks: Array = node.call("get_chunks")
	var h := 0
	var per_chunk: Array = []
	for c in chunks:
		var blob: PackedByteArray = node.call("get_chunk_data", c)
		var ch := hash([c.x, c.y, blob])
		h = hash([h, ch])
		per_chunk.append({"chunk": [c.x, c.y], "hash": ch, "bytes": blob.size()})
	var heights: Array = []
	var target := Vector2i(int(req.get("chunk_x", 0)), int(req.get("chunk_y", 0)))
	if chunks.has(target) or true:
		var packed: PackedFloat32Array = node.call("get_chunk_heights", target)
		for i in range(mini(packed.size(), 4096)):
			heights.append(packed[i])
	return {
		"ok": true,
		"path": str(node.get_path()),
		"chunks": per_chunk.size(),
		"digest": h,
		"per_chunk": per_chunk if bool(req.get("verbose", false)) else [],
		"heights": heights,
	}

func _cmd_capture(req: Dictionary) -> Dictionary:
	var path := str(req.get("path", ""))
	if path == "":
		return {"ok": false, "error": "path required"}
	await RenderingServer.frame_post_draw
	var img := _editor_window().get_texture().get_image()
	if img == null or img.is_empty():
		return {"ok": false, "error": "empty viewport image"}
	if req.has("crop"):
		var cr: Array = req["crop"]
		if cr.size() == 4:
			var r := Rect2i(int(cr[0]), int(cr[1]), int(cr[2]), int(cr[3]))
			img = img.get_region(r)
	var err := img.save_png(path)
	return {"ok": err == OK, "path": path, "size": [img.get_width(), img.get_height()], "error": error_string(err) if err != OK else ""}

func _cmd_console(req: Dictionary) -> Dictionary:
	var text := str(req.get("text", ""))
	var kind := str(req.get("kind", "log"))
	match kind:
		"warn":
			push_warning("QA: " + text)
		"error":
			push_error("QA: " + text)
		_:
			print("QA: " + text)
	return {"ok": true}
