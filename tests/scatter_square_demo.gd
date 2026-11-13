# Scatters the props of demo/demo.tscn (the square island; its terrain's Scatter Items) into
# demo/island_data.res with the deterministic per-cell mode, keeping the characters, the
# player rig, the lantern and the campfire clear. Heights are never touched.
#   godot --headless --path . --script res://tests/scatter_square_demo.gd [-- save=0] [probe=x,z;x,z]
# probe= prints the ground height at world XZ points (to place effect nodes by hand).
extends SceneTree

var frames := 0
var terrain


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		var scene: PackedScene = load("res://demo/demo.tscn")
		var inst := scene.instantiate()
		root.add_child(inst)
		terrain = inst.find_child("MarchingSquaresTerrain", true, false)
	elif frames == 3:
		_run()
		quit()
	return false


func _run() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if args.has("probe"):
		for xz in (args["probe"] as String).split(";"):
			var p := xz.split(",")
			var w := Vector3(float(p[0]), 0.0, float(p[1]))
			print("height ", xz, ": ", terrain.get_height_at(w, -1.0))
	var items: Array = terrain.scatter_items
	print("scatter items: ", items.size())
	terrain.scatter_clear(-1)
	for i in items.size():
		terrain.scatter_fill_cells(i)
		print("  ", items[i].get_label(), ": ", terrain.get_scatter_count(i))
	var root3d: Node = terrain.get_parent()
	for path in ["Characters/Barbarian", "Characters/Mage", "Characters/Rogue", "Characters/Ranger", "Characters/RogueHooded", "PlayerRig", "Lantern", "Smoke"]:
		var n: Node3D = root3d.get_node_or_null(path)
		if n:
			terrain.scatter_erase(-1, n.global_position, 3.5)
	var total: int = terrain.get_scatter_count(-1)
	print("scatter total: ", total)
	assert(total > 100)
	var blob: PackedByteArray = terrain.get_scatter_data()
	terrain.set_scatter_data(blob)
	assert(terrain.get_scatter_data() == blob)
	if args.get("save", "1") == "1":
		# Keep the file's UID (tests/build_demo_data.gd), or the editor rewrites the .res.
		var path: String = terrain.data.resource_path
		var uid := ResourceLoader.get_resource_uid(path)
		if uid == ResourceUID.INVALID_ID:
			uid = ResourceUID.create_id()
		var err := ResourceSaver.save(terrain.data, path)
		if err == OK:
			err = ResourceSaver.set_uid(path, uid)
		print("saved ", terrain.data.resource_path, ": ", err)
