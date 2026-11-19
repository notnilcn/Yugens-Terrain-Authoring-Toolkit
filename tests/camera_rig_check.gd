# Headless check of the demo's camera rig (runs the real game scene, no rendering needed):
#   godot --headless --path . --script res://tests/camera_rig_check.gd -- [scene=res://demo/hex_island.tscn]
# * the rig is the plain Phantom Camera third-person setup: one PhantomCamera3D in follow mode
#   THIRD_PERSON (spring arm, collision mask 0) following the player's CameraTarget, one
#   Camera3D with a PhantomCameraHost child, and no PlayerSpawn / spawn marker anywhere;
# * nothing moves the camera when the player is occluded: a wall dropped between the knight and
#   the camera (on every physics layer) leaves the real camera-to-target distance, the spring
#   arm's hit length and the view pitch / yaw unchanged for 60 frames;
# * the player only rotates / zooms the pcam: orbit_pixels / zoom_steps change the pcam's third
#   person rotation / spring length and the camera follows them.
# Prints "CAMERA_RIG OK" or "CAMERA_RIG FAIL: ...".
extends SceneTree

var frame := 0
var args := {}
var inst: Node
var player: Node3D
var pcam: Node3D
var cam: Camera3D
var target: Node3D
var fails: PackedStringArray = []
var wall: StaticBody3D
var base := {}
var dev := Vector4.ZERO
var phase := "spawn"
var phase_frame := 0


func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	inst = (load(args.get("scene", "res://demo/hex_island.tscn")) as PackedScene).instantiate()
	root.add_child(inst)
	current_scene = inst


func _check(ok: bool, what: String) -> void:
	print("camera_rig: ", "ok   " if ok else "FAIL ", what)
	if not ok:
		fails.append(what)


func _arm() -> SpringArm3D:
	return pcam.get_parent() as SpringArm3D if pcam else null


func _state() -> Dictionary:
	var arm := _arm()
	return {
		"dist": cam.global_position.distance_to(target.global_position),
		"hit": arm.get_hit_length() if arm else -1.0,
		"pitch": float(player.call("view_pitch")),
		"yaw": float(player.call("view_yaw")),
	}


func _structure() -> void:
	var pcams := inst.find_children("*", "PhantomCamera3D", true, false)
	_check(pcams.size() == 1, "exactly one PhantomCamera3D (%d)" % pcams.size())
	_check(int(pcam.get("follow_mode")) == 6, "pcam follow_mode THIRD_PERSON (%s)" % pcam.get("follow_mode"))
	_check(pcam.get("follow_target") == target, "pcam follows the player's CameraTarget")
	_check(int(pcam.get("collision_mask")) == 0, "spring arm collision off (mask %s)" % pcam.get("collision_mask"))
	_check(pcam.get("look_at_mode") == null or int(pcam.get("look_at_mode")) == 0, "no look-at mode on the pcam")
	var hosts := inst.find_children("*", "PhantomCameraHost", true, false)
	_check(hosts.size() == 1 and hosts[0].get_parent() == cam, "one PhantomCameraHost, child of the player's Camera3D")
	_check(_arm() != null, "pcam sits on its runtime SpringArm3D")
	var spawn_nodes := []
	for n in inst.find_children("*", "", true, false):
		if "spawn" in String(n.name).to_lower() or n.is_in_group("spawn") or n.is_in_group("player_spawn"):
			spawn_nodes.append(String(n.name))
	_check(spawn_nodes.is_empty(), "no PlayerSpawn / spawn marker nodes (%s)" % ", ".join(spawn_nodes))
	_check(not ClassDB.class_exists("PixelOrbitCamera"), "no PixelOrbitCamera class")
	_check(inst.find_children("*", "PixelOrbitCamera", true, false).is_empty(), "no PixelOrbitCamera node")


func _process(_d: float) -> bool:
	frame += 1
	if player == null:
		player = inst.find_child("Player", true, false) as Node3D
		if player:
			pcam = player.get("phantom_camera")
			cam = player.get("camera")
			target = player.get_node_or_null("CameraTarget")
	if player == null or pcam == null or cam == null or target == null:
		if frame > 60:
			_check(false, "player rig found")
			return _finish()
		return false
	phase_frame += 1
	match phase:
		"spawn":
			# Settled: the damped follow has caught up (distance steady for 30 frames).
			var settled := false
			if bool(player.call("is_spawned")) and _arm() != null:
				var d := cam.global_position.distance_to(target.global_position)
				if phase_frame % 30 == 0:
					settled = phase_frame > 90 and absf(d - float(base.get("settle", -1.0))) < 0.002
					base["settle"] = d
			if settled:
				_structure()
				base = _state()
				var spring := float(pcam.call("get_spring_length"))
				_check(absf(base.dist - spring) < 0.6, "camera at the spring length from the target (%.2f vs %.2f)" % [base.dist, spring])
				# A thick wall halfway between the knight's head and the camera, on every layer.
				wall = StaticBody3D.new()
				wall.collision_layer = 0xFFFFFFFF
				wall.collision_mask = 0xFFFFFFFF
				var cs := CollisionShape3D.new()
				var box := BoxShape3D.new()
				box.size = Vector3(30, 30, 2)
				cs.shape = box
				wall.add_child(cs)
				inst.add_child(wall)
				var mid: Vector3 = target.global_position.lerp(cam.global_position, 0.5)
				wall.global_transform = Transform3D(Basis.looking_at(cam.global_position - target.global_position), mid)
				phase = "occluded"
				phase_frame = 0
			elif phase_frame > 1200:
				_check(false, "player spawned on the terrain")
				return _finish()
		"occluded":
			var s := _state()
			dev = Vector4(maxf(dev.x, absf(s.dist - base.dist)), maxf(dev.y, absf(s.hit - base.hit)),
				maxf(dev.z, absf(s.pitch - base.pitch)), maxf(dev.w, absf(angle_difference(deg_to_rad(s.yaw), deg_to_rad(base.yaw)))))
			if phase_frame == 60:
				var ray := root.get_world_3d().direct_space_state.intersect_ray(PhysicsRayQueryParameters3D.create(target.global_position, cam.global_position))
				_check(not ray.is_empty(), "the wall really blocks the knight from the camera")
				_check(dev.x < 0.05, "camera distance unchanged while occluded (max change %.3f m)" % dev.x)
				_check(dev.y < 0.05, "spring arm not pulled in (max hit-length change %.3f m)" % dev.y)
				_check(dev.z < 0.1 and dev.w < 0.01, "view pitch / yaw unchanged while occluded (%.3f deg, %.4f rad)" % [dev.z, dev.w])
				wall.queue_free()
				base = _state()
				# Orbit and zoom in the same frame (a wheel step while dragging).
				player.call("orbit_pixels", Vector2(40, 10))
				player.call("zoom_steps", 2.0)
				phase = "input"
				phase_frame = 0
		"input":
			if phase_frame == 90:
				var s := _state()
				var spring := float(pcam.call("get_spring_length"))
				_check(absf(s.yaw - base.yaw) > 5.0, "mouse orbit turns the pcam (yaw %.1f -> %.1f)" % [base.yaw, s.yaw])
				_check(s.pitch > base.pitch + 1.0, "mouse orbit pitches the pcam (pitch %.1f -> %.1f)" % [base.pitch, s.pitch])
				_check(absf(s.dist - spring) < 0.6 and s.dist > base.dist + 1.0, "wheel zoom lengthens the spring arm (%.2f -> %.2f, spring %.2f)" % [base.dist, s.dist, spring])
				return _finish()
	return false


func _finish() -> bool:
	if fails.is_empty():
		print("CAMERA_RIG OK")
	else:
		print("CAMERA_RIG FAIL: ", "; ".join(fails))
	quit(0 if fails.is_empty() else 1)
	return true
