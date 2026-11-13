# Editor performance probe. Installed as a temporary @tool autoload by tests/editor_perf.sh;
# inert unless the user arg `editor_perf` is present.
#   tests/editor_perf.sh scene=res://demo/hex_island.tscn [preview=1] [select=<Class>] [off=A,B]
#       [node.<Name>.<prop>=<value>] [frames=300] [animate=1] [toggle=<feature key>] [label=x]
# Measures, with the scene open in the 3D screen and nothing touched:
#   idle_fps   frames the editor redraws per second in its default low-processor mode
#              (0-1 = the editor idles; ~60+ = something forces a redraw every frame)
#   frame_avg  ms per frame with continuous redraw (low-processor mode off)
#   process    ms per frame spent in _process callbacks (bracketed)
# Prints one `EDITOR_PERF {json}` line and quits.
@tool
extends Node

var args := {}
var frame := 0
var phase := 0
var t0 := 0
var f0 := 0
var idle_fps := 0.0
var samples: Array[float] = []
var last_us := 0
var bracket_us := 0
var t_start := 0
var first: Node
var last: Node


class Stamp extends Node:
	var cb: Callable
	func _process(_d: float) -> void:
		cb.call()


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if not Engine.is_editor_hint() or not args.has("editor_perf"):
		set_process(false)
		return
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)


func _process(_d: float) -> void:
	frame += 1
	var scene_path: String = args.get("scene", "res://demo/hex_island.tscn")
	var edited := EditorInterface.get_edited_scene_root()
	match phase:
		0:
			# Low-processor mode off while loading so the frame counter advances.
			OS.low_processor_usage_mode = false
			if edited != null and edited.scene_file_path == scene_path and not EditorInterface.get_resource_filesystem().is_scanning():
				phase = 1
				frame = 0
				EditorInterface.set_main_screen_editor("3D")
			elif frame % 60 == 20:
				EditorInterface.open_scene_from_path(scene_path)
			if frame > 8000:
				printerr("EDITOR_PERF: scene never opened")
				get_tree().quit(1)
		1:
			if frame == 20 and args.get("preview", "0") == "1":
				var b: Button = EditorInterface.get_base_control().find_child("PixelPreviewButton", true, false)
				if b:
					b.button_pressed = true
			if frame == 22 and args.get("animate", "0") == "1":
				var ab: Button = EditorInterface.get_base_control().find_child("PixelAnimateButton", true, false)
				if ab == null:
					printerr("EDITOR_PERF: no PixelAnimateButton")
				else:
					ab.button_pressed = true
			if frame == 24 and args.has("toggle"):
				# Pixel World > Features: click the item of feature `toggle` twice (off, on again).
				var fm: PopupMenu = EditorInterface.get_base_control().find_child("PixelFeatures", true, false)
				var key: String = args["toggle"]
				var idx := Array(PixelFeatures.get_feature_keys()).find(key)
				if fm == null or idx < 0:
					printerr("EDITOR_PERF: no Features menu or feature ", key)
				else:
					var before := PixelFeatures.is_enabled(key)
					fm.id_pressed.emit(idx)
					var flipped := PixelFeatures.is_enabled(key) != before and fm.is_item_checked(fm.get_item_index(idx)) == PixelFeatures.is_enabled(key)
					fm.id_pressed.emit(idx)
					print("EDITOR_PERF toggle ", key, " ", "OK" if flipped and PixelFeatures.is_enabled(key) == before else "FAIL")
			if frame == 30 and args.has("select"):
				var want: String = args["select"]
				var nodes := edited.find_children("*", want, true, false)
				var n: Node = nodes[0] if not nodes.is_empty() else edited.find_child(want, true, false)
				if n:
					EditorInterface.edit_node(n)
			if frame == 30 and args.has("off"):
				# Remove nodes from the edited scene (never saved: the probe quits without saving).
				for n in (args["off"] as String).split(",", false):
					var node := edited.get_node_or_null(n)
					if node:
						node.get_parent().remove_child(node)
						node.queue_free()
			if frame == 30:
				for k in args:
					if (k as String).begins_with("node."):
						var parts := (k as String).split(".")
						var tn := edited.get_node_or_null(parts[1])
						if tn:
							tn.set(parts[2], str_to_var(args[k]))
			if frame == 240:
				# Idle redraw rate in the editor's default mode.
				OS.low_processor_usage_mode = true
				phase = 2
				t0 = Time.get_ticks_msec()
				f0 = Engine.get_frames_drawn()
		2:
			if Time.get_ticks_msec() - t0 >= 5000:
				idle_fps = (Engine.get_frames_drawn() - f0) * 1000.0 / (Time.get_ticks_msec() - t0)
				OS.low_processor_usage_mode = false
				first = Stamp.new()
				first.process_priority = -2147483647
				first.cb = func(): t_start = Time.get_ticks_usec()
				last = Stamp.new()
				last.process_priority = 2147483647
				last.cb = func(): bracket_us += Time.get_ticks_usec() - t_start
				get_tree().root.add_child(first)
				get_tree().root.add_child(last)
				phase = 3
				frame = 0
		3:
			var now := Time.get_ticks_usec()
			if frame == 30:
				bracket_us = 0
			if frame > 30:
				samples.append((now - last_us) / 1000.0)
			last_us = now
			if frame == 30 + int(args.get("frames", "300")):
				var total := 0.0
				for s in samples:
					total += s
				var out := {
					"label": args.get("label", "editor"),
					"idle_fps": snappedf(idle_fps, 0.1),
					"frame_avg": snappedf(total / samples.size(), 0.01),
					"process": snappedf(bracket_us / 1000.0 / samples.size(), 0.01),
				}
				print("EDITOR_PERF ", JSON.stringify(out))
				first.queue_free()
				last.queue_free()
				get_tree().quit()
