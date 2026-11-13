# Scatters the props of demo/hex_island.tscn (its terrain's Scatter Items) into
# demo/hex_island_data.res with the deterministic per-cell mode, keeping the characters and
# the player spawn clear. Also writes the built-in stamp images into
# addons/MarchingSquaresTerrain/stamps/ and checks the scatter / stamp API.
#   godot --headless --path . --script res://tests/scatter_demo.gd [-- save=0]
extends SceneTree

var frames := 0
var terrain


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		var scene: PackedScene = load("res://demo/hex_island.tscn")
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
	var items: Array = terrain.scatter_items
	print("scatter items: ", items.size())
	terrain.scatter_clear(-1)
	for i in items.size():
		terrain.scatter_fill_cells(i)
		print("  ", items[i].get_label(), ": ", terrain.get_scatter_count(i))
	# Keep the characters and the spawn point free.
	var root3d: Node = terrain.get_parent()
	for path in ["Characters/Barbarian", "Characters/Mage", "Characters/Rogue", "PlayerSpawn"]:
		var n: Node3D = root3d.get_node_or_null(path)
		if n:
			terrain.scatter_erase(-1, n.global_position, 3.5)
	# ...and the raised pool cells (build_demo_data.gd), so no tree stands in the water.
	for w in terrain.data.get_meta("water_cells", PackedVector3Array()):
		terrain.scatter_erase(-1, terrain.cell_to_world(Vector2i(int(w.x), int(w.y))), 1.8)
	var total: int = terrain.get_scatter_count(-1)
	print("scatter total: ", total)
	assert(total > 100)
	# Deterministic: a second fill gives the same blob.
	var blob: PackedByteArray = terrain.get_scatter_data()
	terrain.set_scatter_data(blob)
	assert(terrain.get_scatter_data() == blob)
	if args.get("save", "1") == "1":
		# Keep (or create) the file's UID, or the editor's first scan rewrites the .res.
		var data_path: String = terrain.data.resource_path
		var uid := ResourceLoader.get_resource_uid(data_path)
		if uid == ResourceUID.INVALID_ID:
			uid = ResourceUID.create_id()
		var err := ResourceSaver.save(terrain.data, data_path)
		if err == OK:
			err = ResourceSaver.set_uid(data_path, uid)
		print("saved ", data_path, ": ", err)
		DirAccess.make_dir_recursive_absolute("res://addons/MarchingSquaresTerrain/stamps")
		for k in ["Mountain", "Crater", "Mesa", "Valley", "Hill"]:
			var img: Image = MarchingSquaresTerrain.create_stamp_image(k, 256)
			var path := "res://addons/MarchingSquaresTerrain/stamps/%s.png" % k.to_lower()
			print("stamp ", path, ": ", img.save_png(path))
