extends SceneTree
## Square-mode regression capture: loads the demo scene and screenshots it.
##   godot --path . --script res://tests/capture_demo.gd -- --capture

var _frames := 0


func _initialize() -> void:
	if not ("--capture" in OS.get_cmdline_user_args()):
		quit(0)
		return
	change_scene_to_file("res://scenes/mst_demo_scene.tscn")


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 180:
		var img := get_root().get_texture().get_image()
		var path := "user://demo_square.png"
		img.save_png(path)
		print("saved ", ProjectSettings.globalize_path(path), " size=", img.get_size())
		quit(0)
		return true
	if _frames > 600:
		quit(0)
		return true
	return false
