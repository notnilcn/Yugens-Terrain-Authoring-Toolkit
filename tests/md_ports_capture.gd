# Captures the Minor Deity ports on the hex island (nothing is saved):
#   godot --path . --script res://tests/md_ports_capture.gd -- name=md_x [erode=30000] [mode=0|1|2] [step=0] [flowers=1]
#       [ortho=26] [yaw=45] [pitch=50] [target=x,y,z] [frames=40]
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
		dn.time_override = 0.4
	root.add_child.call_deferred(inst)
	_apply.call_deferred(inst)


func _apply(inst: Node) -> void:
	var t := inst.get_node("MarchingSquaresTerrain")
	if int(args.get("erode", "0")) > 0:
		var e := ClassDB.instantiate("MarchingSquaresErosion") as Node
		e.set("iterations", int(args["erode"]))
		e.set("pixels_per_cell", int(args.get("ppc", "2")))
		e.set("height_scale", float(args.get("hscale", "0")))
		e.set("mode", int(args.get("mode", "0"))) # 0 auto, 1 raw (faithful port), 2 terraced
		e.set("level_step", float(args.get("step", "0")))
		t.add_child(e)
		var t0 := Time.get_ticks_msec()
		e.call("erode")
		print("ERODED in %d ms" % (Time.get_ticks_msec() - t0))
	if args.get("flowers", "0") == "1":
		var f := ClassDB.instantiate("MarchingSquaresWildFlowers") as Node
		f.set("water_level", float(args.get("water", "0")))
		t.add_child(f)
		print("FLOWERS ", f.call("scatter_flowers"))


func _process(_d: float) -> bool:
	frames += 1
	if frames == int(args.get("frames", "40")):
		var n: String = args.get("name", "md_ports")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % n)
		print("CAPTURE ", n)
		quit()
	return false
