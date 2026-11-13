# Grass parity capture with the original demo's "wide" framing (godot-pixelizer-3d
# demo/player.gd apply_camera_preset: yaw 0, pitch 62, distance 34, perspective fov 24, pivot
# 0.6 above the player):
#   godot --path . --script res://tests/grass_compare.gd -- [scene=res://demo/demo.tscn] [name=w4_grass_port] [dist=34]
# Compare with the original:  godot --path <godot-pixelizer-3d> -- --capture=<abs>/w4_grass_orig.png --fixed-view=wide
extends SceneTree

var frame := 0
var args := {}
var inst: Node


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	inst = (load(args.get("scene", "res://demo/demo.tscn")) as PackedScene).instantiate()
	var dn := inst.get_node_or_null("PixelDayNight")
	if dn:
		dn.time_of_day = float(args.get("time", "0.4"))
		dn.set("day_length_seconds", 0.0)
	root.add_child.call_deferred(inst)
	(func(): current_scene = inst).call_deferred()


func _process(_d: float) -> bool:
	frame += 1
	if frame == 3:
		var player := inst.find_child("Player", true, false)
		var cam := inst.find_child("MainCamera3D", true, false) as Camera3D
		cam.fov = 24.0
		player.set("max_distance", 60.0)
		player.set("pitch_max_deg", 89.9)
		player.set("orthographic", args.has("ortho"))
		player.set("yaw_deg", 0.0)
		player.set("pitch_deg", 62.0)
		player.set("distance", float(args.get("dist", "34")))
	if frame == 150:
		var name: String = args.get("name", "w4_grass_port")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		print("CAPTURE ", name)
		quit()
	return false
