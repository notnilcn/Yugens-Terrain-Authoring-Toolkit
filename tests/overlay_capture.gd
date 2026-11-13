# Renders terrain overlay features (grid, fog of war, cell tints, auto texturing, debug views):
#   godot --path . --script res://tests/overlay_capture.gd -- name=x [scene=res://demo/hex_island.tscn]
#       [grid=1] [chunks=1] [fog=1] [tint=1] [auto=1] [debug=1..6] [cursor=1]
#       [ortho=40] [yaw=45] [pitch=50] [target=x,y,z] [frames=40] [time=0.4] [nochars=1]
# auto=1 repaints every floor with slot 0 ("unpainted") and enables the preset's auto rules;
# gen=1 replaces the scene's terrain by a generated, unpainted square-grid mountain island.
extends SceneTree

var frames := 0
var args := {}
var target := 40
var inst: Node
var terrain


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	target = int(args.get("frames", "40"))
	var ps: PackedScene = load(args.get("scene", "res://demo/hex_island.tscn"))
	inst = ps.instantiate()
	var cam = inst.get_node("Camera")
	if args.has("ortho"):
		cam.ortho_size = float(args["ortho"])
	if args.has("yaw"):
		cam.yaw_deg = float(args["yaw"])
	if args.has("pitch"):
		cam.pitch_deg = float(args["pitch"])
	if args.has("target"):
		var xz := (args["target"] as String).split(",")
		cam.target = Vector3(float(xz[0]), float(xz[1]), float(xz[2]))
	if args.has("time"):
		var dn := inst.get_node_or_null("PixelDayNight")
		if dn:
			dn.time_of_day = float(args["time"])
	if args.has("nochars"):
		for n in ["Characters", "PlayerRig", "Lantern", "Fireflies"]:
			var c := inst.get_node_or_null(n)
			if c:
				c.queue_free()
	terrain = inst.get_node("MarchingSquaresTerrain")
	if args.has("gen"):
		terrain.data = ClassDB.instantiate("MarchingSquaresTerrainData")
		terrain.grid_type = 0
		terrain.chunk_size = 16
		terrain.cell_size = Vector2(2, 2)
		terrain.default_merge_mode = 3
		terrain.default_floor_slot = 0
		terrain.default_wall_slot = 5
	terrain.show_grid = args.has("grid")
	terrain.grid_chunk_lines = args.has("chunks")
	if args.has("grid"):
		terrain.grid_color = Color(1, 1, 0.85, 0.45)
		terrain.grid_width = float(args.get("width", "0.12"))
	terrain.debug_view = int(args.get("debug", "0"))
	if args.has("auto"):
		_auto_texture()
	root.add_child.call_deferred(inst)


func _auto_texture() -> void:
	# Slots / snow line are set once the terrain is in the tree (_setup_after_load).
	var ts = terrain.texture_set.duplicate(true)
	var rules = ClassDB.instantiate("MarchingSquaresAutoRules")
	rules.enabled = true
	rules.water_level = 0.45
	rules.shore_height = float(args.get("shore", "0.9"))
	ts.auto_rules = rules
	terrain.texture_set = ts


func _generate() -> void:
	var n := FastNoiseLite.new()
	n.seed = 5
	n.frequency = 0.02
	var ridge := FastNoiseLite.new()
	ridge.seed = 9
	ridge.frequency = 0.03
	ridge.fractal_type = FastNoiseLite.FRACTAL_RIDGED
	for cz in range(-2, 2):
		for cx in range(-2, 2):
			terrain.add_chunk(Vector2i(cx, cz))
	for z in range(-32, 33):
		for x in range(-32, 33):
			var p := Vector2(x, z) * 2.0
			var d := p.length() / 60.0
			var shape := 1.0 - smoothstep(0.25, 1.0, d + n.get_noise_2dv(p) * 0.3)
			var h := shape * 9.0 - 2.0 + maxf(ridge.get_noise_2dv(p), 0.0) * 16.0 * shape * shape
			# A plateau with a real cliff on one side.
			if p.x > 8.0 and p.x < 40.0 and p.y > -30.0 and p.y < 2.0 and h > 3.0:
				h += 6.0
			terrain.set_sample_height(Vector2i(x, z), h)


func _setup_after_load() -> void:
	if args.has("gen"):
		_generate()
	if args.has("auto"):
		var hs: Array[float] = []
		for c in terrain.get_chunks():
			var slots: PackedByteArray = terrain.get_chunk_slots(c, false)
			for i in slots.size():
				slots[i] = 0
			terrain.set_chunk_slots(c, slots, false)
			for h in terrain.get_chunk_heights(c):
				hs.append(h)
		hs.sort()
		var snow: float = hs[int(hs.size() * float(args.get("snowq", "0.93")))]
		terrain.texture_set.auto_rules.snow_height = snow
		terrain.texture_set.auto_rules.snow_noise = maxf(0.6, (hs[hs.size() - 1] - hs[0]) * 0.04)
		print("auto rules: snow line ", snow, " max ", hs[hs.size() - 1])
	if args.has("fog") or args.has("tint"):
		var cam = inst.get_node("Camera")
		var centre: Vector3 = cam.target
		var c0: Vector2i = terrain.world_to_fog_cell(centre)
		var rect: Rect2i = terrain.get_cell_rect()
		var cs: float = terrain.cell_size.x
		for y in range(rect.position.y, rect.end.y):
			for x in range(rect.position.x, rect.end.x):
				var cell := Vector2i(x, y)
				var w: Vector3 = terrain.cell_to_world(cell) if terrain.grid_type != 0 else Vector3((x + 0.5) * cs, 0, (y + 0.5) * cs)
				var d := Vector2(w.x - centre.x, w.z - centre.z)
				# Irregular vision: a disk plus a lobe towards -X (explored trail).
				var vis := d.length() < 11.0
				var explored := vis or d.length() < 19.0 or (d.x < 0.0 and absf(d.y) < 6.0 and d.x > -30.0)
				if args.has("fog"):
					terrain.set_cell_explored(cell, explored)
					terrain.set_cell_visibility(cell, vis)
				if args.has("tint") and vis:
					if d.x > 3.0 and d.y > -2.0:
						terrain.set_cell_tint(cell, Color(0.95, 0.3, 0.25, 0.35))
					elif d.x < -3.0 and d.y < 2.0:
						terrain.set_cell_tint(cell, Color(0.3, 0.55, 1.0, 0.35))
		if args.has("tint"):
			terrain.set_cell_tint(c0, Color(1.0, 0.9, 0.3, 0.0))
			terrain.set_cell_highlight(c0, 1.0)
		terrain.finish_fog_transition()
		terrain.fog_enabled = args.has("fog")
	if args.has("cursor"):
		var cam = inst.get_node("Camera")
		terrain.set_cursor(cam.target + Vector3(-6, 0, 3), 5.0, 2, Color(0.35, 1.0, 0.45))


func _process(_d: float) -> bool:
	frames += 1
	if frames == 3:
		_setup_after_load()
	if frames == target:
		var name: String = args.get("name", "overlay")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		print("CAPTURE ", name)
		quit()
	return false
