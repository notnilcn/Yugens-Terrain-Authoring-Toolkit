# Drives the demo player rig with simulated input and captures frames:
#   godot --path . --script res://tests/player_capture.gd -- [scene=res://demo/demo.tscn] [name=player] [persp] [dist=N] [walk=<action>|cliff]
# Writes tests/captures/<name>_<step>.png for: start, walk_a/walk_b (consecutive frames while
# walking, to judge pixel swimming), jump, turn (walking sideways), orbit (arrow keys), drag
# (mouse drag orbit), zoom_in, zoom_out. Prints player / camera state per step, and the
# camera-to-target distance vs. the player's arm length, and the occlusion avoidance state
# (pitch added on top of the user's, whether terrain still hides the knight; [noavoid] turns it off,
# [noxray] the x-ray silhouette). [dist=N] starts at camera distance N.
# [walk=<action>] is the walk direction (default player_forward). Walks stop after [walk_m=N]
# metres (default 9; the side step 3), so the route does not depend on the frame rate. On
# hex_island the default camera yaw is 120: the spawn sits in a small basin and "forward" then
# runs ~16 m through open ground, so the default frames show the normal camera.
# [walk=tree] (hex_island) checks scatter-prop collision: the knight is placed [tree_gap=N] m
# behind the nearest tree of item [tree_item=N] (seen from the camera), shot tree_hidden (occlusion
# raise / x-ray), then walks toward the camera into the tree (tree_bump); "TREE min_axis_dist"
# is the closest the body got to the trunk axis.
# [walk=cliff] (hex_island) is the cliff walk instead: [relax] with toward=player_back and yaw 0,
# into the cliff south of the spawn (raise + x-ray) and back out (relax).
# [raydebug] (with trace) also prints the body rays' physics / terrain hits per pitch.
# [ortho] uses the orthographic camera instead of the default low-FOV perspective one,
# [yaw=N] / [pitch=N] start the camera there, [trace] prints a PITCH line every 5 frames,
# [nosmooth] turns off the renderer's sub-texel scroll (walk_a / walk_b then differ by whole texels).
# [relax] runs the occlusion-relax path instead: walk [toward=<action>] (default player_forward)
# for [toward_frames=N] frames, then [away=<action>] (default: the opposite) for [away_frames=N],
# then stand still. Shots relax_start / relax_cliff / relax_away / relax_settle_a / _b, PITCH
# lines every 5 frames and a final "RELAX PASS|FAIL" line: PASS when the camera was raised at
# some point, the added pitch returned to 0 within 1.2 s of the user's pitch becoming clear, and
# the x-ray is off at the end.
# [orbitdiff] measures orbit swimming instead: from frame 60 the camera yaw steps by
# [orbit_step=N] degrees (default 0.5) per frame for [orbit_frames=N] (default 30) frames (set
# directly, no input easing), and consecutive frames are compared with Image.compute_image_metrics
# (whole frame and the central quarter around the knight). Prints ORBITDIFF lines (mean / RMS of
# 0-255 channel differences) for the still frames before the orbit (idle baseline), the orbit and
# the 30 frames after it (settle). [eased] orbits through the player's eased mouse-drag input
# (`orbit_pixels`) instead, so the settle phase includes the ease's tail. [nostab] turns off the
# renderer's rotation re-anchoring, [settle=N] / [yaw_step=N] set the player's orbit settle
# tolerance / yaw steps (nostab settle=0 = the behaviour before both existed). [calm] removes
# weather, birds, smoke, dust and fireflies (less frame-to-frame noise).
# [knight] (any sequence) also checks every frame that the knight's skinned meshes are visible
# and inside the camera frustum and prints "KNIGHT VISIBLE n/n occluded_without_xray=k" at the end
# (k = frames where 3+ body rays toward the camera were blocked while the x-ray was below 0.3).
extends SceneTree

var frame := 0
var args := {}
var inst: Node
var player: Node3D
var steps: Array = []
var clear_since := -1.0
# Game time (sum of process deltas): the relax limit is about what the player sees, so a slow
# or loaded capture machine (Xvfb, llvmpipe) does not fail it by wall-clock stalls.
var game_time := 0.0
var relax_ok := false
var max_added := 0.0
# Distance-limited key presses: action -> [start position, metres].
var walks := {}
var prev_img: Image
var diffs := {"still": [], "orbit": [], "settle": []}
var knight_frames := 0
var knight_ok := 0
var knight_bare := 0


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
	var island := (args.get("scene", "") as String).ends_with("hex_island.tscn")
	if args.get("walk", "") == "cliff":
		args["relax"] = "1"
		args["toward"] = "player_back"
		args["yaw"] = args.get("yaw", "0")
		args["toward_frames"] = args.get("toward_frames", "50")
		args["away_frames"] = args.get("away_frames", "40")
	elif island and not args.has("relax"):
		args["yaw"] = args.get("yaw", "120")
	if args.get("walk", "") == "tree":
		# Scatter-prop collision: put the knight just behind the nearest tree (seen from the
		# camera), check the occlusion raise / x-ray, then walk into the tree toward the camera.
		args["trace"] = "1"
		steps = [
			[45, func(): _place_behind_tree()],
			[90, func(): _shot("tree_hidden"); Input.action_press("player_back")],
			[150, func(): Input.action_release("player_back"); _shot("tree_bump"); _tree_verdict()],
			[151, func(): _knight_report(); quit()],
		]
		return
	if args.has("orbitdiff"):
		var n := int(args.get("orbit_frames", "30"))
		var stepd := float(args.get("orbit_step", "0.5"))
		steps = [[50, func(): _shot("orbitdiff_start")]]
		for i in 10:
			steps.append([50 + i, func(): _diff_frame("still")])
		var eased := args.has("eased")
		for i in n:
			steps.append([60 + i, func():
				if eased:
					player.call("orbit_pixels", Vector2(-stepd / float(player.get("mouse_orbit_sensitivity")), 0.0))
				else:
					player.set("yaw_deg", float(player.get("yaw_deg")) + stepd)
				_diff_frame("orbit" if i > 0 else "")])
		steps.append([60 + n, func(): _diff_frame("orbit"); _shot("orbitdiff_end")])
		for i in 30:
			steps.append([61 + n + i, func(): _diff_frame("settle")])
		steps.append([91 + n, func(): _diff_report(); _knight_report(); quit()])
		return
	if args.has("relax"):
		var toward: String = args.get("toward", "player_forward")
		var opposite := {"player_forward": "player_back", "player_back": "player_forward", "player_left": "player_right", "player_right": "player_left"}
		var away: String = args.get("away", opposite.get(toward, "player_back"))
		var a := 61 + int(args.get("toward_frames", "90"))
		var b := a + int(args.get("away_frames", "90"))
		args["trace"] = "1"
		steps = [
			[60, func(): _shot("relax_start")],
			[61, func(): Input.action_press(toward)],
			[a, func(): Input.action_release(toward); _shot("relax_cliff"); Input.action_press(away)],
			[b, func(): Input.action_release(away); _shot("relax_away")],
			[b + 30, func(): _shot("relax_settle_a")],
			[b + 90, func(): _shot("relax_settle_b"); _relax_verdict()],
			[b + 91, func(): _knight_report(); quit()],
		]
		return
	# [frame, callable]
	var walk: String = args.get("walk", "player_forward")
	var walk_m := float(args.get("walk_m", "9"))
	steps = [
		[60, func(): _shot("start")],
		[61, func(): _press_for(walk, walk_m)],
		[64, func(): _shot("walk_a")],
		[65, func(): _shot("walk_b")],
		[132, func(): Input.action_release(walk); Input.action_press("player_jump")],
		[134, func(): Input.action_release("player_jump")],
		[136, func(): _shot("jump")],
		[170, func(): _press_for("player_left", 3.0)],
		[215, func(): _shot("turn")],
		[216, func(): Input.action_release("player_left")],
		[220, func(): Input.action_press("camera_orbit_left")],
		[260, func(): Input.action_release("camera_orbit_left")],
		[280, func(): _shot("orbit")],
		[281, func(): _mouse_button(MOUSE_BUTTON_LEFT, true)],
		[282, func(): for i in 10: _mouse_motion(Vector2(-12, 4))],
		[284, func(): _mouse_button(MOUSE_BUTTON_LEFT, false)],
		[298, func(): _shot("drag")],
		[302, func(): for i in 4: _mouse_button(MOUSE_BUTTON_WHEEL_UP, true)],
		[315, func(): _shot("zoom_in")],
		[316, func(): for i in 14: _mouse_button(MOUSE_BUTTON_WHEEL_DOWN, true)],
		[330, func(): _shot("zoom_out")],
		[331, func(): _knight_report(); quit()],
	]


func _press_for(action: String, metres: float) -> void:
	Input.action_press(action)
	walks[action] = [player.global_position if player else Vector3.ZERO, metres]


func _process(_d: float) -> bool:
	frame += 1
	game_time += _d
	for action in walks.keys():
		var w: Array = walks[action]
		var p0: Vector3 = w[0]
		if player and Vector2(player.global_position.x - p0.x, player.global_position.z - p0.z).length() >= float(w[1]):
			Input.action_release(action)
			walks.erase(action)
	if frame == 2:
		player = inst.find_child("Player", true, false) as Node3D
		if player != null and args.has("persp"):
			player.set("orthographic", false)
		if player != null and args.has("ortho"):
			player.set("orthographic", true)
		if player != null and args.has("noavoid"):
			player.set("occlusion_avoid", false)
		if player != null and args.has("noxray"):
			for mi in player.find_children("*", "MeshInstance3D", true, false):
				(mi as MeshInstance3D).material_overlay = null
		if player != null and args.has("dist"):
			player.set("distance", float(args["dist"]))
		var pr := inst.find_child("PixelRenderer", true, false)
		if pr != null and args.has("nosmooth"):
			# Whole-texel scrolling: consecutive walk frames then differ by an integer texel
			# shift only when the camera snap holds (used to check perspective shimmer).
			pr.set("smooth_scroll", false)
		if args.has("calm"):
			# Fewer moving effects, so frame diffs measure the camera (orbitdiff).
			for n in ["Weather", "Birds", "Smoke", "Dust", "Fireflies"]:
				var e := inst.get_node_or_null(n)
				if e:
					e.queue_free()
		if pr != null and args.has("nostab"):
			# Lattice anchored at the world origin (no re-anchoring on rotation).
			pr.set("stabilize_rotation", false)
		if player != null and args.has("settle"):
			player.set("orbit_settle_deg", float(args["settle"]))
		if player != null and args.has("yaw_step"):
			player.set("orbit_yaw_step_deg", float(args["yaw_step"]))
		if player != null and args.has("yaw"):
			player.set("yaw_deg", float(args["yaw"]))
		if player != null and args.has("pitch"):
			player.set("pitch_deg", float(args["pitch"]))
	if player != null and frame > 2 and player.has_method("is_occlusion_raised"):
		_track()
	if player != null and tree_pos != Vector3.ZERO and frame > 45:
		tree_min_dist = minf(tree_min_dist, Vector2(player.global_position.x - tree_pos.x, player.global_position.z - tree_pos.z).length())
		tree_raised = tree_raised or bool(player.call("is_occlusion_raised"))
	if player != null and frame > 40 and args.has("knight"):
		_check_knight()
	for s in steps:
		if s[0] == frame:
			(s[1] as Callable).call()
	return false


# Occlusion relax tracking: from the first frame the user's pitch is clear again (fewer blocked
# body rays than the trigger), the added pitch must be back to 0 within 1.2 s (release delay +
# ease), in game time; the capture runs at a few fps, so this includes a frame or two of
# quantisation.
func _track() -> void:
	var added: float = player.call("get_occlusion_pitch")
	var raised: bool = player.call("is_occlusion_raised")
	var user_clear: bool = int(player.call("get_occlusion_points", false)) < int(player.get("occlusion_trigger_points"))
	max_added = maxf(max_added, added)
	var t := game_time
	if not user_clear:
		clear_since = -1.0
	elif added > 0.0 and clear_since < 0.0:
		clear_since = t
	elif added == 0.0 and not raised and clear_since >= 0.0:
		print("RELAXED after %.2f s" % (t - clear_since))
		relax_ok = t - clear_since <= 1.2
		clear_since = -1.0
	if args.has("trace") and frame % 5 == 0:
		print("PITCH f=%d t=%.2f user=%.1f shown=%.1f added=%.1f raised=%s pts_user=%d pts_shown=%d xray=%.2f pos=%s" % [
			frame, t, player.get("pitch_deg"), player.call("get_shown_pitch"), added, raised,
			player.call("get_occlusion_points", false), player.call("get_occlusion_points", true),
			player.call("get_xray_amount"), player.global_position])
		if args.has("raydebug"):
			# Per pitch: physics / terrain-raycast hit heights above the feet for the 5 body rays
			# (along the camera's back axis at the current yaw), and terrain heights behind the knight.
			var space := player.get_world_3d().direct_space_state
			var terr := inst.find_child("MarchingSquaresTerrain", true, false)
			var feet := player.global_position - Vector3(0, 0.9, 0)
			var yaw := deg_to_rad(float(player.get("yaw_deg")))
			var line := "RAYS"
			for p in [45.0, 55.0, 62.0, 67.0, 72.0, 82.0]:
				var back := Vector3(0, 0, 1).rotated(Vector3.RIGHT, -deg_to_rad(p)).rotated(Vector3.UP, yaw)
				var ph := ""
				var th := ""
				for h in [0.2, 0.55, 0.9, 1.25, 1.6]:
					var from := feet + Vector3(0, h, 0)
					var r := space.intersect_ray(PhysicsRayQueryParameters3D.create(from, from + back * 37.5, 1, [player.get_rid()]))
					ph += ("%.1f," % (r.position.y - feet.y)) if r else "-,"
					var tr: Dictionary = terr.call("raycast", from, back, 37.5) if terr else {}
					th += ("%.1f," % (tr.position.y - feet.y)) if tr.has("position") else "-,"
				line += " p%d[phys %s terr %s]" % [p, ph, th]
			if terr:
				var hb := Vector3(0, 0, 1).rotated(Vector3.UP, yaw)
				line += " ground:"
				for d in [0.0, 0.2, 0.4, 0.6, 1.0]:
					line += " %.1f" % (float(terr.call("get_height_at", feet + hb * d, NAN)) - feet.y)
			print(line)


var tree_pos := Vector3.ZERO
var tree_min_dist := INF
var tree_raised := false


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
	var back := Vector3(0, 0, 1).rotated(Vector3.UP, deg_to_rad(float(player.get("yaw_deg"))))
	player.global_position = tree_pos - back * float(args.get("tree_gap", "1.0")) + Vector3.UP * 1.0
	player.velocity = Vector3.ZERO
	print("TREE at %s (%.1f m from spawn), knight at %s" % [tree_pos, best, player.global_position])


func _tree_verdict() -> void:
	print("TREE min_axis_dist=%.2f raised_seen=%s final=%s" % [tree_min_dist, tree_raised, player.global_position])


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
	if player.has_method("get_xray_amount") and int(player.call("get_occlusion_points", true)) >= 3 and float(player.call("get_xray_amount")) < 0.3:
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


func _relax_verdict() -> void:
	var added: float = player.call("get_occlusion_pitch")
	var xray: float = player.call("get_xray_amount")
	var ok := max_added > 0.0 and relax_ok and added == 0.0 and xray == 0.0
	print("RELAX %s max_added=%.1f final_added=%.1f final_xray=%.2f shown=%.1f user=%.1f" % [
		"PASS" if ok else "FAIL", max_added, added, xray, player.call("get_shown_pitch"), player.get("pitch_deg")])


func _shot(step: String) -> void:
	var name := "%s_%s" % [args.get("name", "player"), step]
	root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
	var cam := root.get_camera_3d()
	print("phys=", Engine.get_physics_frames(), " vel=", player.velocity if player else "-", " proc=", player.is_physics_processing() if player else "-", " t=", Time.get_ticks_msec())
	print("CAPTURE %s player=%s on_floor=%s cam=%s cam_name=%s yaw=%.1f pitch=%.1f dist=%.1f" % [
		name, player.global_position if player else "-", player.is_on_floor() if player else "-",
		cam.global_position if cam else "-", cam.name if cam else "-",
		player.get("yaw_deg"), player.get("pitch_deg"), player.get("distance")])
	var pr := inst.find_child("PixelRenderer", true, false)
	if pr != null and pr.has_method("get_snap_anchor"):
		# Moves only when the camera rotates: equal across walk shots = translation stays snapped.
		print("SNAP %s anchor=%s" % [name, pr.call("get_snap_anchor")])
	if cam and player:
		# The pcam follows Player/CameraTarget (0.6 above the body).
		var tgt := player.global_position + Vector3(0, 0.6, 0)
		print("CAMDIST %s cam_to_target=%.2f arm=%.2f proj=%s size=%.2f fov=%.1f" % [
			name, cam.global_position.distance_to(tgt), player.call("get_camera_arm_length"),
			"ortho" if cam.projection == Camera3D.PROJECTION_ORTHOGONAL else "persp", cam.size, cam.fov])
		if player.has_method("is_occluded"):
			# Occlusion avoidance: pitch added on top of the user's, whether occluders still hide
			# the knight, blocked body rays (of 5) at the user's / shown pitch, x-ray strength.
			print("OCCLUSION %s shown_pitch=%.1f added=%.1f occluded=%s raised=%s pts_user=%d pts_shown=%d xray=%.2f" % [
				name, player.call("get_shown_pitch"), player.call("get_occlusion_pitch"), player.call("is_occluded"),
				player.call("is_occlusion_raised"), player.call("get_occlusion_points", false),
				player.call("get_occlusion_points", true), player.call("get_xray_amount")])


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
