# Headless check: in demo/hex_island.tscn, decorations (grass, scatter props) and their
# colliders must not depend on the camera pitch. The third-person camera can be pulled back up
# to 100 m from the player (the LOD focus); looking level, it then sees its own foreground,
# which is far from the player. That foreground must stay full detail, with props drawn and
# not faded by the shader (CPU mirror of pt_lod_keep), at every pitch and zoom.
#   godot --headless --path . --script res://tests/level_camera_check.gd
# Prints "LEVEL CAMERA OK" or the failed checks (exit code 1).
extends SceneTree

const VIEWS := [
	# [pitch below the horizon (deg), spring length (m)]
	[45.0, 24.0], [15.0, 24.0], [2.0, 24.0], [0.0, 60.0], [2.0, 100.0], [-8.0, 100.0], [60.0, 100.0],
]

var fails := 0
var frames := 0
var scene: Node
var t
var p
var cam: Camera3D
var step := -1
var wait := 0
var base_shapes := -1
var base_stored := -1


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


# Scatter props drawn and their collision shapes (internal Scatter node).
func _scatter() -> Vector2i:
	var shown := 0
	var shapes := 0
	for s in t.get_children(true):
		if s.name == "Scatter":
			for c in s.get_children():
				if c.is_queued_for_deletion():
					continue
				if c is MultiMeshInstance3D:
					shown += c.multimesh.instance_count
				elif c is StaticBody3D:
					shapes += c.get_shape_owners().size()
	return Vector2i(shown, shapes)


func _grass(c: Vector2i) -> int:
	return t.get_chunk_grass_count(c)


# CPU mirror of pt_lod_keep (pixel_noise.gdshaderinc) with the published globals.
func _keep(w: Vector3) -> float:
	var f: Vector4 = t.get_lod_fade()
	if f.w <= 0.0:
		return 1.0
	var a := Vector2(f.x, f.y)
	var ab: Vector2 = t.get_lod_eye() - a
	var ap := Vector2(w.x, w.z) - a
	var k := clampf(ap.dot(ab) / maxf(ab.dot(ab), 1e-6), 0.0, 1.0)
	return clampf((f.z - (ap - ab * k).length()) / f.w, 0.0, 1.0)


func _chunk_of(w: Vector3) -> Vector2i:
	return t.get_cell_chunk(t.world_to_cell(w))


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		scene = load("res://demo/hex_island.tscn").instantiate()
		root.add_child(scene)
		t = scene.get_node("MarchingSquaresTerrain")
		p = scene.get_node("PlayerRig/Pixel/Player")
		cam = scene.get_node("PlayerRig/MainCamera3D")
	elif frames == 60:
		_check(p.is_spawned(), "player spawned")
		_check(t.lod_distance > 0.0 and t.lod_focus == p, "hex_island LOD follows the player")
		var s := _scatter()
		base_shapes = s.y
		base_stored = t.get_scatter_positions(-1).size()
		_check(base_shapes > 0, "prop colliders built (%d)" % base_shapes)
		_next_view()
	elif step >= 0:
		wait -= 1
		if wait <= 0:
			_measure()
			_next_view()
	return false


func _next_view() -> void:
	step += 1
	if step >= VIEWS.size():
		print("LEVEL CAMERA OK" if fails == 0 else "LEVEL CAMERA: %d failure(s)" % fails)
		quit(1 if fails else 0)
		return
	var v: Array = VIEWS[step]
	p.set_view(p.view_yaw() + 40.0, v[0], v[1])
	wait = 120  # the damped camera settles, the LOD timer (0.2 s) re-evaluates, chunks flush


func _measure() -> void:
	var v: Array = VIEWS[step]
	var tag := "pitch %.0f, %.0f m" % [v[0], v[1]]
	var cp := cam.global_position
	var pp: Vector3 = p.global_position
	var flat := Vector2(cp.x - pp.x, cp.z - pp.z).length()
	_check(t.get_lod_eye().distance_to(Vector2(cp.x, cp.z)) < 2.0, "%s: LOD eye follows the camera" % tag)
	var low: Array = t.get_low_detail_chunks()
	var s := _scatter()
	_check(s.y == base_shapes, "%s: colliders unchanged (%d / %d)" % [tag, s.y, base_shapes])
	_check(t.get_scatter_positions(-1).size() == base_stored, "%s: stored props unchanged" % tag)
	# Sample the ground from the player to the camera (and a bit past it): every chunk on the
	# line stays full detail and nothing there is faded.
	var n := 12
	var checked := 0
	for i in range(n + 1):
		var w := pp.lerp(cp, float(i) / n)
		var c := _chunk_of(w)
		if not t.get_chunks().has(c):
			continue
		checked += 1
		_check(not low.has(c), "%s: chunk %s on the camera line (%.0f%%) is full detail" % [tag, c, 100.0 * i / n])
		_check(_keep(w) >= 1.0, "%s: no fade on the camera line at %.0f%% (keep %.2f)" % [tag, 100.0 * i / n, _keep(w)])
	_check(checked > 0, "%s: camera line over the terrain" % tag)
	_check(_grass(_chunk_of(pp)) > 0, "%s: grass under the player" % tag)
	# Every stored prop within 20 m of the camera or the player is drawn and unfaded.
	var near := 0
	var faded := 0
	for q in t.get_scatter_positions(-1):
		var w: Vector3 = t.to_global(Vector3(q.x, 0.0, q.y))
		var dc := Vector2(w.x - cp.x, w.z - cp.z).length()
		var dp := Vector2(w.x - pp.x, w.z - pp.z).length()
		if dc < 20.0 or dp < 20.0:
			near += 1
			if _keep(w) < 1.0 or low.has(_chunk_of(w)):
				faded += 1
	_check(near > 0 and faded == 0, "%s: props near the camera / player shown (%d of %d hidden)" % [tag, faded, near])
	print("%s: camera %s, eye %s, " % [tag, cp, t.get_lod_eye()], "spring ", p.get_spring_length() if p.has_method("get_spring_length") else -1.0)
	print("%s: camera %.0f m from the player, %d low chunks, %d props drawn, %d colliders" % [tag, flat, low.size(), s.x, s.y])
