# Captures the hex island with a given hex_edge_style (the demo player rig is removed so the
# orbit camera frames the shot):
#   godot --path . --script res://tests/hex_edge_capture.gd -- name=r9_x [style=0|1|2] [ortho=26]
#       [target=x,y,z] [yaw=45] [pitch=50] [time=0.4] [ramp=0..5] [rim=0.35] [rimw=0.3] [wrap=0] [frames=40]
#       [debug=0..5] [level=2.5] [dthr=0.35]
extends SceneTree

var frames := 0
var args := {}


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var inst: Node = (load(args.get("scene", "res://demo/hex_island.tscn")) as PackedScene).instantiate()
	var rig := inst.get_node_or_null("PlayerRig")
	if rig:
		inst.remove_child(rig)
		rig.free()
	var t := inst.get_node("MarchingSquaresTerrain")
	if args.has("style"):
		t.set("hex_edge_style", int(args["style"]))
	var cam := inst.get_node("Camera")
	cam.current = true
	cam.ortho_size = float(args.get("ortho", "26"))
	if args.has("yaw"):
		cam.yaw_deg = float(args["yaw"])
	if args.has("pitch"):
		cam.pitch_deg = float(args["pitch"])
	if args.has("target"):
		var xyz := (args["target"] as String).split(",")
		cam.target = Vector3(float(xyz[0]), float(xyz[1]), float(xyz[2]))
	var dn := inst.get_node_or_null("PixelDayNight")
	if dn:
		dn.time_override = float(args.get("time", "0.4"))
		if args.has("ramp"):
			dn.set("light_ramp_preset", int(args["ramp"]))
		if args.has("rim"):
			dn.set("rim_strength", float(args["rim"]))
		if args.has("rimw"):
			dn.set("rim_width", float(args["rimw"]))
		if args.has("wrap"):
			dn.set("light_wrap", float(args["wrap"]))
	if args.has("debug"):
		inst.get_node("PixelRenderer").debug_view = int(args["debug"])
	if args.has("dthr"):
		inst.get_node("PixelRenderer").depth_threshold = float(args["dthr"])
	if args.has("level"):
		t.set("level_height", float(args["level"]))
	root.add_child.call_deferred(inst)


func _process(_d: float) -> bool:
	frames += 1
	if frames == int(args.get("frames", "40")):
		var n: String = args.get("name", "hex_edge")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % n)
		print("CAPTURE ", n)
		quit()
	return false
