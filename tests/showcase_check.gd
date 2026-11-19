# Presentation facts of the demo's opening view (demo/hex_island.tscn as a game):
#   godot --headless --path . --script res://tests/showcase_check.gd
# The third-person camera starts close (knight large in frame) with the campfire, the waterfall
# and the bird flock in the start frustum; the palette is on with real colours; the cloud deck
# is drawn; the campfire burns from the start (a hearth) without the grass fire spreading;
# the day starts in daylight and stays light for the first minutes; the night is readable.
# Prints SHOWCASE OK and exits 0 on success (exit 1 + SHOWCASE FAIL lines otherwise).
extends SceneTree

const CAMPFIRE := Vector3(14.0, 5.6, -2.0)
const WATERFALL := Vector3(3.5, 7.0, -0.87)
## Frames sampled for the moving parts (birds, smoke puffs).
const SAMPLE_FRAMES := 240

var scene: Node
var frame := 0
var failures: Array[String] = []
var birds_seen := 0
var bird_frames := 0
var smoke_seen := false
## Set at the end of each check block, so a script error inside one fails the run.
var static_done := false
var night_done := false


func _initialize() -> void:
	for key in PixelFeatures.get_feature_keys():
		PixelFeatures.set_enabled(key, true)
	scene = (load("res://demo/hex_island.tscn") as PackedScene).instantiate()
	root.add_child(scene)


func check(ok: bool, what: String) -> void:
	print("  ", "ok   " if ok else "FAIL ", what)
	if not ok:
		failures.append(what)


func internals_visible(n: Node) -> bool:
	for i in n.get_child_count(true) - n.get_child_count():
		var c := n.get_child(i, true)
		if c is Node3D and (c as Node3D).visible:
			return true
	return false


func _process(_d: float) -> bool:
	frame += 1
	var cam := root.get_viewport().get_camera_3d()
	if frame > 30 and frame <= 30 + SAMPLE_FRAMES and cam:
		var n := 0
		for p in scene.get_node("Birds").get_bird_positions():
			if cam.is_position_in_frustum(p):
				n += 1
		birds_seen = maxi(birds_seen, n)
		if n >= 3:
			bird_frames += 1
		var smoke := scene.get_node("Smoke") as Node3D
		smoke_seen = smoke_seen or cam.is_position_in_frustum(smoke.global_position + Vector3.UP)
	match frame:
		30 + SAMPLE_FRAMES:
			# After the spawn and the damped follow have settled.
			_static_checks(cam)
			check(birds_seen >= 3, "bird flock in the start view (max %d in frustum)" % birds_seen)
			check(bird_frames > SAMPLE_FRAMES / 2, "flock stays in view most of the time (%d / %d frames)" % [bird_frames, SAMPLE_FRAMES])
			check(smoke_seen, "campfire smoke plume in the start view")
			var dn := scene.get_node("PixelDayNight")
			dn.time_override = 0.0
		30 + SAMPLE_FRAMES + 3:
			_night_checks()
			check(static_done and night_done, "every check block ran to the end")
			print("SHOWCASE ", "OK" if failures.is_empty() else "FAIL (%d)" % failures.size())
			quit(0 if failures.is_empty() else 1)
	return false


func _static_checks(cam: Camera3D) -> void:
	var player := scene.get_node("PlayerRig/Pixel/Player") as Node3D
	var target := player.get_node("CameraTarget") as Node3D
	var pcam: Node = player.phantom_camera
	var dist := cam.global_position.distance_to(target.global_position)
	var spring := float(pcam.call("get_spring_length"))
	print("camera ", cam.global_position, " fov ", cam.fov, " spring ", spring, " dist ", dist)
	check(spring >= 11.0 and spring <= 15.0 and absf(dist - spring) < 1.0, "close start camera (spring %.1f m, %.1f m to the target)" % [spring, dist])
	check(cam.fov >= 36.0 and cam.fov <= 50.0, "start fov %.0f" % cam.fov)
	check(is_equal_approx(float(cam.get_meta("pixel_snap_distance", -1.0)), spring), "pixel_snap_distance follows the spring length")
	check(cam.is_position_in_frustum(player.global_position), "knight in view")
	# Knight size on screen: its height over the frame height at that distance.
	var frac := 1.4 / (2.0 * player.global_position.distance_to(cam.global_position) * tan(deg_to_rad(cam.fov * 0.5)))
	check(frac > 0.12, "knight fills %.0f%% of the frame height" % (frac * 100.0))
	check(cam.is_position_in_frustum(CAMPFIRE), "campfire in the start view")
	check(cam.is_position_in_frustum(WATERFALL), "waterfall in the start view")
	check(scene.get_node("PixelWater").get_waterfalls().size() > 0, "waterfalls built")

	var r := scene.get_node("PixelRenderer")
	var pal: Resource = r.palette
	check(r.palette_enabled and pal != null, "palette enabled")
	check(pal != null and pal.colors.size() >= 32, "palette has its own colours (%d)" % (pal.colors.size() if pal else 0))

	var sky := scene.get_node("PixelSky")
	check(sky.deck_enabled and internals_visible(sky), "cloud deck drawn")
	check(sky.deck_size >= 2.0 * cam.far, "deck reaches the far plane (%.0f m vs far %.0f m)" % [sky.deck_size, cam.far])

	var fire := scene.get_node("MarchingSquaresTerrain/GrassFire")
	check(fire.get_hearth_count() > 0, "campfire hearth burning (%d texels)" % fire.get_hearth_count())
	check(fire.get_burning_count() == 0, "no grass fire spreading (%d)" % fire.get_burning_count())
	check(fire.is_burning_at(CAMPFIRE), "fire state reports the campfire burning")

	var dn := scene.get_node("PixelDayNight")
	var t0: float = dn.time_of_day
	check(dn.get_night_factor() == 0.0, "starts in daylight (time %.3f)" % t0)
	check(dn.day_length_seconds >= 900.0, "long day (%.0f s)" % dn.day_length_seconds)
	# Still daylight after five minutes of play (sampled through the override).
	var light := true
	for s in [60.0, 120.0, 180.0, 240.0, 300.0]:
		dn.time_override = fmod(t0 + s / dn.day_length_seconds, 1.0)
		light = light and dn.get_night_factor() == 0.0
	dn.time_override = -1.0
	check(light, "daylight for the first five minutes")

	var dust := scene.get_node("Dust") as Node3D
	var ground: float = scene.get_node("MarchingSquaresTerrain").get_height_at(player.global_position, 0.0)
	var top: float = dust.global_position.y + dust.layers * dust.spacing
	check(dust.global_position.y <= ground and top >= ground + dust.band, "dust stack covers the start meadow (%.1f..%.1f m, ground %.1f)" % [dust.global_position.y, top, ground])
	static_done = true


func _night_checks() -> void:
	var env: Environment = scene.get_node("WorldEnvironment").environment
	var sun := scene.get_node("Sun") as DirectionalLight3D
	print("midnight ambient ", env.ambient_light_energy, " ", env.ambient_light_color, " moon ", sun.light_energy)
	check(env.ambient_light_energy >= 0.5, "readable night ambient (%.2f)" % env.ambient_light_energy)
	check(sun.light_energy >= 0.3, "moonlight (%.2f)" % sun.light_energy)
	check(scene.get_node("Lantern").energy >= 0.4, "lantern bright enough for the night")
	check(scene.get_node("Fireflies").energy >= 2.5, "fireflies bright enough for the night")
	night_done = true
