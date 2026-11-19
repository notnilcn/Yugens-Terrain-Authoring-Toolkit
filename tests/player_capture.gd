# Drives the demo player rig with simulated input and captures frames:
#   godot --path . --script res://tests/player_capture.gd -- [scene=res://demo/hex_island.tscn] [name=player] [dist=N] [walk=<action>|cliff|tree] [start=x,y,z]
# Writes tests/captures/<name>_<step>.png for: start, walk_a/walk_b (consecutive frames while
# walking, to judge pixel swimming), jump, turn (walking sideways), orbit (arrow keys), drag
# (mouse drag orbit), zoom_in, zoom_out. Prints player / camera state per step (view yaw /
# pitch, the pcam's spring length vs. the real camera-to-target distance, the renderer's snap
# anchor, the x-ray state). [noxray] removes the x-ray silhouette.
# [dist=N] / [yaw=N] / [pitch=N] start the camera there (`MstDemoPlayer.set_view`).
# [walk=<action>] is the walk direction (default player_forward). Walks stop after [walk_m=N]
# metres (default 9; the side step 3), so the route does not depend on the frame rate. Without
# [yaw] / [pitch] / [dist] the scene's authored start view is kept (on hex_island "forward"
# then walks up the ramp toward the waterfall).
# [start=x,y,z] moves the PlayerRig before the scene enters the tree.
# [walk=cliff] (hex_island, yaw 0; starts on the central plateau at -5,5,-32, north of the 5 m highland cliff,
# unless [start] is given) walks player_back into the cliff south of the start for
# [toward_frames=N] frames and back out for [away_frames=N]: shots cliff_start / cliff_behind /
# cliff_away and a final "CLIFF PASS|FAIL" line: PASS when the camera's view pitch and spring
# length never changed (no occlusion avoidance) and the x-ray showed while the knight was hidden.
# [walk=tree] (hex_island) checks scatter-prop collision: the knight is placed [tree_gap=N] m
# behind the nearest tree of item [tree_item=N] (seen from the camera), shot tree_hidden
# (x-ray), then walks toward the camera into the tree (tree_bump); "TREE min_axis_dist" is the
# closest the body got to the trunk axis.
# [trace] prints a VIEW line every 5 frames; [nosmooth] turns off the renderer's sub-texel
# scroll (walk_a / walk_b then differ by whole texels).
# [orbitdiff] measures orbit swimming: from frame 60 the camera yaw steps by [orbit_step=N]
# degrees (default 0.5) per frame for [orbit_frames=N] (default 30) frames, and consecutive
# frames are compared with Image.compute_image_metrics (whole frame and the central quarter
# around the knight). Prints ORBITDIFF lines (mean / RMS of 0-255 channel differences) for the
# still frames before the orbit, the orbit and the 30 frames after it. [nostab] turns off the
# renderer's rotation re-anchoring; [calm] removes weather, birds, smoke, dust and fireflies.
# [knight] (any sequence) also checks every frame that the knight's skinned meshes are visible
# and inside the camera frustum and prints "KNIGHT VISIBLE n/n occluded_without_xray=k" at the end
# (k = frames where 3+ body rays toward the camera were blocked while the x-ray was below 0.3).
extends SceneTree

var frame := 0
var args := {}
var inst: Node
var player: Node3D
var pcam: Node3D
var steps: Array = []
# Distance-limited key presses: action -> [start position, metres].
var walks := {}
var prev_img: Image
var diffs := {"still": [], "orbit": [], "settle": []}
var knight_frames := 0
var knight_ok := 0
var knight_bare := 0
# Cliff walk: the view at its start, the largest pitch / spring-length change seen, peak x-ray.
var cliff_view := Vector2.ZERO
var cliff_dev := Vector2.ZERO
var cliff_xray := 0.0
var cliff_on := false
## Non-zero once a verdict (CLIFF) failed; the run quits with it.
var exit_code := 0


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	inst = (load(args.get("scene", "res://demo/hex_island.tscn")) as PackedScene).instantiate()
	if args.get("walk", "") == "cliff":
		args["start"] = args.get("start", "-5,5,-32")
	var rig := inst.get_node_or_null("PlayerRig") as Node3D
	if rig and args.has("start"):
		var c: PackedStringArray = (args["start"] as String).split(",")
		rig.position = Vector3(float(c[0]), float(c[1]), float(c[2]))
	var dn := inst.get_node_or_null("PixelDayNight")
	if dn:
		dn.time_of_day = float(args.get("time", "0.4"))
		dn.set("day_length_seconds", 0.0)
	root.add_child.call_deferred(inst)
	(func(): current_scene = inst).call_deferred()
	if args.get("walk", "") == "cliff":
		args["yaw"] = args.get("yaw", "0")
		args["trace"] = "1"
		var a := 61 + int(args.get("toward_frames", "50"))
		var b := a + int(args.get("away_frames", "40"))
		steps = [
			[60, func(): _shot("cliff_start"); cliff_view = _view(); cliff_on = true; _act("player_back", true)],
			[a, func(): _act("player_back", false); _shot("cliff_behind"); _act("player_forward", true)],
			[b, func(): _act("player_forward", false)],
			[b + 30, func(): _shot("cliff_away"); _cliff_verdict()],
			[b + 31, func(): _knight_report(); quit(exit_code)],
		]
		return
	if args.get("walk", "") == "tree":
		# Scatter-prop collision: put the knight just behind the nearest tree (seen from the
		# camera), check the x-ray, then walk into the tree toward the camera.
		args["trace"] = "1"
		steps = [
			[45, func(): _place_behind_tree()],
			[90, func(): _shot("tree_hidden"); _act("player_back", true)],
			[150, func(): _act("player_back", false); _shot("tree_bump"); _tree_verdict()],
			[151, func(): _knight_report(); quit(exit_code)],
		]
		return
	if args.has("orbitdiff"):
		var n := int(args.get("orbit_frames", "30"))
		var stepd := float(args.get("orbit_step", "0.5"))
		steps = [[50, func(): _shot("orbitdiff_start")]]
		for i in 10:
			steps.append([50 + i, func(): _diff_frame("still")])
		for i in n:
			steps.append([60 + i, func():
				player.call("orbit_pixels", Vector2(-stepd / float(player.get("mouse_orbit_sensitivity")), 0.0))
				_diff_frame("orbit" if i > 0 else "")])
		steps.append([60 + n, func(): _diff_frame("orbit"); _shot("orbitdiff_end")])
		for i in 30:
			steps.append([61 + n + i, func(): _diff_frame("settle")])
		steps.append([91 + n, func(): _diff_report(); _knight_report(); quit(exit_code)])
		return
	# [frame, callable]
	var walk: String = args.get("walk", "player_forward")
	var walk_m := float(args.get("walk_m", "9"))
	steps = [
		[60, func(): _shot("start"); if args.has("startonly"): quit(exit_code)],
		[61, func(): _press_for(walk, walk_m)],
		[64, func(): _shot("walk_a")],
		[65, func(): _shot("walk_b")],
		[132, func(): _act(walk, false); _act("player_jump", true)],
		[134, func(): _act("player_jump", false)],
		[136, func(): _shot("jump")],
		[170, func(): _press_for("player_left", 3.0)],
		[215, func(): _shot("turn")],
		[216, func(): _act("player_left", false)],
		[220, func(): _act("camera_orbit_left", true)],
		[260, func(): _act("camera_orbit_left", false)],
		[280, func(): _shot("orbit")],
		[281, func(): _mouse_button(MOUSE_BUTTON_LEFT, true)],
		[282, func(): for i in 10: _mouse_motion(Vector2(-12, 4))],
		[284, func(): _mouse_button(MOUSE_BUTTON_LEFT, false)],
		[298, func(): _shot("drag")],
		[302, func(): for i in 4: _mouse_button(MOUSE_BUTTON_WHEEL_UP, true)],
		[315, func(): _shot("zoom_in")],
		[316, func(): for i in 14: _mouse_button(MOUSE_BUTTON_WHEEL_DOWN, true)],
		[330, func(): _shot("zoom_out")],
		[331, func(): _knight_report(); quit(exit_code)],
	]


# Presses / releases an action through a real input event (MstDemoPlayer reads its action
# state in `_input`; Input.action_press alone sends no event).
func _act(action: String, pressed: bool) -> void:
	var e := InputEventAction.new()
	e.action = action
	e.pressed = pressed
	Input.parse_input_event(e)


func _press_for(action: String, metres: float) -> void:
	_act(action, true)
	walks[action] = [player.global_position if player else Vector3.ZERO, metres]


# (view pitch, spring length).
func _view() -> Vector2:
	return Vector2(float(player.call("view_pitch")), float(pcam.call("get_spring_length")) if pcam else 0.0)


func _process(_d: float) -> bool:
	frame += 1
	for action in walks.keys():
		var w: Array = walks[action]
		var p0: Vector3 = w[0]
		if player and Vector2(player.global_position.x - p0.x, player.global_position.z - p0.z).length() >= float(w[1]):
			_act(action, false)
			walks.erase(action)
	if frame == 2:
		player = inst.find_child("Player", true, false) as Node3D
		pcam = inst.find_child("PlayerPhantomCamera3D", true, false) as Node3D
		if player != null and args.has("noxray"):
			for mi in player.find_children("*", "MeshInstance3D", true, false):
				(mi as MeshInstance3D).material_overlay = null
		var pr := inst.find_child("PixelRenderer", true, false)
		if pr != null and args.has("nosmooth"):
			# Whole-texel scrolling: consecutive walk frames then differ by an integer texel
			# shift only when the camera snap holds (used to check perspective shimmer).
			pr.set("subtexel_scroll", false)
		if args.has("calm"):
			# Fewer moving effects, so frame diffs measure the camera (orbitdiff).
			for n in ["Weather", "Birds", "Smoke", "Dust", "Fireflies"]:
				var e := inst.get_node_or_null(n)
				if e:
					e.queue_free()
		if pr != null and args.has("nostab"):
			# Lattice anchored at the world origin (no re-anchoring on rotation).
			pr.set("stabilize_rotation", false)
		if player != null and (args.has("yaw") or args.has("pitch") or args.has("dist")):
			player.call("set_view", float(args.get("yaw", "0")), float(args.get("pitch", "45")),
				float(args.get("dist", str(pcam.call("get_spring_length") if pcam else 38.0))))
	if player != null and frame > 2 and args.has("trace") and frame % 5 == 0:
		print("VIEW f=%d yaw=%.1f pitch=%.1f spring=%.2f xray_pts=%d xray=%.2f pos=%s" % [
			frame, player.call("view_yaw"), player.call("view_pitch"), _view().y,
			player.call("get_xray_points"), player.call("get_xray_amount"), player.global_position])
	if player != null and cliff_on:
		var d := (_view() - cliff_view).abs()
		cliff_dev = Vector2(maxf(cliff_dev.x, d.x), maxf(cliff_dev.y, d.y))
		cliff_xray = maxf(cliff_xray, float(player.call("get_xray_amount")))
	if player != null and tree_pos != Vector3.ZERO and frame > 45:
		tree_min_dist = minf(tree_min_dist, Vector2(player.global_position.x - tree_pos.x, player.global_position.z - tree_pos.z).length())
		tree_xray = maxf(tree_xray, float(player.call("get_xray_amount")))
	if player != null and frame > 40 and args.has("knight"):
		_check_knight()
	for s in steps:
		if s[0] == frame:
			(s[1] as Callable).call()
	return false


func _cliff_verdict() -> void:
	cliff_on = false
	var ok := cliff_dev.x < 0.5 and cliff_dev.y < 0.01 and cliff_xray > 0.5
	if not ok:
		exit_code = 1
	print("CLIFF %s pitch_change=%.2f spring_change=%.3f peak_xray=%.2f" % ["PASS" if ok else "FAIL", cliff_dev.x, cliff_dev.y, cliff_xray])


var tree_pos := Vector3.ZERO
var tree_min_dist := INF
var tree_xray := 0.0


func _place_behind_tree() -> void:
	var terr := inst.find_child("MarchingSquaresTerrain", true, false) as Node3D
	var pts: PackedVector2Array = terr.call("get_scatter_positions", int(args.get("tree_item", "0")))
	var best := INF
	for p in pts:
		var w := terr.to_global(Vector3(p.x, 0.0, p.y))
		var d := Vector2(w.x - player.global_position.x, w.z - player.global_position.z).length()
		if d < best:
			best = d
			tree_pos = w
	tree_pos.y = float(terr.call("get_height_at", tree_pos, 0.0))
	var back := Vector3(0, 0, 1).rotated(Vector3.UP, deg_to_rad(float(player.call("view_yaw"))))
	player.global_position = tree_pos - back * float(args.get("tree_gap", "1.0")) + Vector3.UP * 1.0
	player.velocity = Vector3.ZERO
	print("TREE at %s (%.1f m from start), knight at %s" % [tree_pos, best, player.global_position])


func _tree_verdict() -> void:
	print("TREE min_axis_dist=%.2f peak_xray=%.2f final=%s" % [tree_min_dist, tree_xray, player.global_position])


# Knight visible: every skinned MeshInstance3D under the player is visible in the tree and its
# AABB centre lies inside the current camera's frustum, in front of the camera.
func _check_knight() -> void:
	var cam := root.get_camera_3d()
	if cam == null:
		return
	knight_frames += 1
	var ok := false
	for mi in player.find_children("*", "MeshInstance3D", true, false):
		var m := mi as MeshInstance3D
		if m.mesh == null or not m.is_visible_in_tree():
			continue
		var c := m.global_transform * m.get_aabb().get_center()
		if cam.is_position_in_frustum(c) and not cam.is_position_behind(c):
			ok = true
			break
	# Hidden behind an occluder (3+ of 5 rays toward the camera) with no x-ray silhouette yet.
	if player.has_method("get_xray_amount") and int(player.call("get_xray_points")) >= 3 and float(player.call("get_xray_amount")) < 0.3:
		knight_bare += 1
		if args.has("trace"):
			print("KNIGHT OCCLUDED_NO_XRAY f=%d" % frame)
	if ok:
		knight_ok += 1
	elif args.has("trace"):
		print("KNIGHT HIDDEN f=%d" % frame)


func _knight_report() -> void:
	if args.has("knight"):
		print("KNIGHT VISIBLE %d/%d occluded_without_xray=%d" % [knight_ok, knight_frames, knight_bare])


func _diff_frame(kind: String) -> void:
	var img := root.get_texture().get_image()
	img.convert(Image.FORMAT_RGB8)
	if prev_img != null and kind != "":
		var w := img.get_width()
		var h := img.get_height()
		var r := Rect2i(w * 3 / 8, h * 3 / 8, w / 4, h / 4)
		var full: Dictionary = img.compute_image_metrics(prev_img, false)
		var mid: Dictionary = img.get_region(r).compute_image_metrics(prev_img.get_region(r), false)
		diffs[kind].append([full.mean, full.root_mean_squared, mid.mean, mid.root_mean_squared])
	prev_img = img


func _diff_report() -> void:
	for kind in ["still", "orbit", "settle"]:
		var a: Array = diffs[kind]
		var s := [0.0, 0.0, 0.0, 0.0]
		for d in a:
			for k in 4:
				s[k] += d[k]
		var n := maxf(a.size(), 1)
		print("ORBITDIFF %s %s frames=%d step=%s full_mean=%.3f full_rms=%.3f centre_mean=%.3f centre_rms=%.3f" % [
			args.get("name", "player"), kind, a.size(), args.get("orbit_step", "0.5"), s[0] / n, s[1] / n, s[2] / n, s[3] / n])


func _shot(step: String) -> void:
	var name := "%s_%s" % [args.get("name", "player"), step]
	root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
	var cam := root.get_camera_3d()
	print("CAPTURE %s player=%s on_floor=%s cam=%s cam_name=%s yaw=%.1f pitch=%.1f" % [
		name, player.global_position if player else "-", player.is_on_floor() if player else "-",
		cam.global_position if cam else "-", cam.name if cam else "-",
		player.call("view_yaw") if player else 0.0, player.call("view_pitch") if player else 0.0])
	var pr := inst.find_child("PixelRenderer", true, false)
	if pr != null and pr.has_method("get_lattice_anchor"):
		# Moves only when the camera rotates: equal across walk shots = translation stays snapped.
		print("SNAP %s anchor=%s" % [name, pr.call("get_lattice_anchor")])
	if cam and player:
		# The pcam follows Player/CameraTarget.
		var ct := player.get_node_or_null("CameraTarget") as Node3D
		var tgt := ct.global_position if ct else player.global_position
		print("CAMDIST %s cam_to_target=%.2f spring=%.2f snap_meta=%s fov=%.1f xray_pts=%d xray=%.2f" % [
			name, cam.global_position.distance_to(tgt), _view().y, cam.get_meta("pixel_snap_distance", "-"),
			cam.fov, player.call("get_xray_points"), player.call("get_xray_amount")])


func _mouse_button(button: MouseButton, pressed: bool) -> void:
	var e := InputEventMouseButton.new()
	e.button_index = button
	e.pressed = pressed
	e.position = Vector2(640, 360)
	Input.parse_input_event(e)
	if button == MOUSE_BUTTON_WHEEL_UP or button == MOUSE_BUTTON_WHEEL_DOWN:
		var r := e.duplicate() as InputEventMouseButton
		r.pressed = false
		Input.parse_input_event(r)


func _mouse_motion(rel: Vector2) -> void:
	var e := InputEventMouseMotion.new()
	e.relative = rel
	e.position = Vector2(640, 360)
	e.button_mask = MOUSE_BUTTON_MASK_LEFT
	Input.parse_input_event(e)
