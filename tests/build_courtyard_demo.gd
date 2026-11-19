# Builds the courtyard demo (headless): a recreation of robert-k's godot-3d-pixel-art main scene
# on a MarchingSquaresTerrain, with its fountain, pillars, stone ring, round trees and Kenney
# stones (addons/MarchingSquaresTerrain/assets/), the fox as the player and PixelSunShafts.
#   godot --headless --path . --script res://tests/build_courtyard_demo.gd
# Writes demo/courtyard_data.res and demo/courtyard.tscn. Prop positions are robert-k's
# basic_scene.blend / main.tscn layout converted to Godot axes (blender (x, y) -> (x, -y)) and
# re-centred on the fountain.
extends SceneTree

const DATA_PATH := "res://demo/courtyard_data.res"
const SCENE_PATH := "res://demo/courtyard.tscn"
const ASSETS := "res://addons/MarchingSquaresTerrain/assets/"
# Reference fountain position (Godot axes); every prop is placed relative to it.
const FOUNTAIN_REF := Vector2(1.596, -1.127)
# Flat courtyard radius, then the hills rise to HILL_HEIGHT over HILL_RAMP metres.
const FLAT_RADIUS := 15.0
const HILL_RAMP := 12.0
const HILL_HEIGHT := 6.0

var frames := 0


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		var terrain = _build_terrain()
		_build_scene(terrain)
		quit()
	return false


func _noise(seed: int, freq: float) -> FastNoiseLite:
	var n := FastNoiseLite.new()
	n.seed = seed
	n.frequency = freq
	n.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	return n


func _ref(x: float, z: float) -> Vector2:
	return Vector2(x, z) - FOUNTAIN_REF


func _height(p: Vector2, hills: FastNoiseLite) -> float:
	var r := p.length() + hills.get_noise_2dv(p * 0.7) * 4.0
	var t := clampf((r - FLAT_RADIUS) / HILL_RAMP, 0.0, 1.0)
	t = t * t * (3.0 - 2.0 * t)
	return t * (HILL_HEIGHT + hills.get_noise_2dv(p * 1.9) * 2.0)


# Floor slots of the default preset: 0 grass, 1 meadow, 2 forest floor, 8 gravel, 10 moss, 11 path.
func _slot(p: Vector2, h: float, patches: FastNoiseLite) -> int:
	var r := p.length()
	if r < 3.6:
		return 11
	if r < 5.0:
		return 8
	if h > 2.5:
		return 2 if patches.get_noise_2dv(p) > 0.1 else 1
	if patches.get_noise_2dv(p * 1.7) > 0.35:
		return 10
	return 0


func _build_terrain():
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 0
	t.chunk_size = 16
	t.cell_size = Vector2(2, 2)
	root.add_child(t)
	for cz in range(-1, 1):
		for cx in range(-1, 1):
			t.add_chunk(Vector2i(cx, cz))
	var hills := _noise(5, 0.03)
	var patches := _noise(17, 0.08)
	for c in t.get_chunks():
		var heights := PackedFloat32Array()
		var floors := PackedByteArray()
		var walls := PackedByteArray()
		for id in t.get_chunk_cells(c):
			var w: Vector3 = t.sample_to_world(id)
			var p := Vector2(w.x, w.z)
			var h := _height(p, hills)
			heights.append(h)
			floors.append(_slot(p, h, patches))
			walls.append(5)
		t.set_chunk_heights(c, heights)
		t.set_chunk_slots(c, floors, false)
		t.set_chunk_slots(c, walls, true)
	t.commit_to_data()
	var uid := ResourceLoader.get_resource_uid(DATA_PATH) if ResourceLoader.exists(DATA_PATH) else ResourceUID.INVALID_ID
	if uid == ResourceUID.INVALID_ID:
		uid = ResourceUID.create_id()
	var data = t.data
	data.take_over_path(DATA_PATH)
	var err := ResourceSaver.save(data, DATA_PATH)
	if err == OK:
		err = ResourceSaver.set_uid(DATA_PATH, uid)
	print("courtyard data: ", err)
	return t


func _add(parent: Node, n: Node, owner: Node, name: String) -> Node:
	n.name = name
	parent.add_child(n)
	n.owner = owner
	return n


func _prop(parent: Node, owner: Node, terrain, file: String, name: String, p: Vector2, yaw_deg := 0.0, scale := 1.0) -> Node3D:
	var inst := (load(ASSETS + file) as PackedScene).instantiate() as Node3D
	inst.position = Vector3(p.x, terrain.get_height_at(Vector3(p.x, 0, p.y), 0.0), p.y)
	inst.rotation_degrees.y = yaw_deg
	inst.scale = Vector3.ONE * scale
	_add(parent, inst, owner, name)
	# Static collision so the player walks around (and onto) the props.
	# (Direct children of the instance root, so the scene saves them.)
	var body := StaticBody3D.new()
	_add(inst, body, owner, "Collision")
	var k := 0
	for m in inst.find_children("*", "MeshInstance3D", true, false):
		var mi := m as MeshInstance3D
		# Trees: trunks only (the canopy is above the player).
		if file.contains("tree") and not mi.name.begins_with("Stem"):
			continue
		var shape := CollisionShape3D.new()
		shape.shape = mi.mesh.create_trimesh_shape()
		shape.transform = inst.global_transform.affine_inverse() * mi.global_transform
		_add(body, shape, owner, "Shape%d" % k)
		k += 1
	return inst


func _build_scene(terrain) -> void:
	var scene := Node3D.new()
	scene.name = "Courtyard"
	root.add_child(scene)

	# robert-k's dusk look: dark violet sky, cool blue-grey ambient, warm low sun.
	var we := WorldEnvironment.new()
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.102, 0.098, 0.196)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.58, 0.596, 0.682)
	env.ambient_light_energy = 0.45
	env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	we.environment = env
	_add(scene, we, scene, "WorldEnvironment")

	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-38, -62, 0)
	sun.light_color = Color(1.0, 0.93, 0.8)
	sun.light_energy = 0.85
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 80.0
	_add(scene, sun, scene, "Sun")

	terrain.get_parent().remove_child(terrain)
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 0
	t.data = load(DATA_PATH)
	t.grass_size = Vector2(0.5, 0.6)
	_add(scene, t, scene, "MarchingSquaresTerrain")
	terrain.free()

	var props = ClassDB.instantiate("PixelObject")
	_add(scene, props, scene, "Courtyard")
	_prop(props, scene, t, "courtyard/fountain.glb", "Fountain", Vector2.ZERO)
	_prop(props, scene, t, "courtyard/stone_ring.glb", "StoneRing", _ref(1.656, -4.906))
	var pillars := [[3.28, -6.824, 0.0], [-7.227, -8.038, 0.0], [-3.207, -0.257, 0.0], [7.79, 6.443, 0.0], [1.268, 4.924, 45.0], [8.977, 1.165, 45.0]]
	for i in pillars.size():
		var q: Array = pillars[i]
		_prop(props, scene, t, "courtyard/pillar.glb", "Pillar%d" % (i + 1), _ref(q[0], q[1]), q[2])
	var trees := [[-8.19, -4.315], [8.59, -1.899], [-1.127, 5.83], [3.453, 7.479], [-7.868, 4.815]]
	for i in trees.size():
		var q: Array = trees[i]
		_prop(props, scene, t, "courtyard/round_tree.glb", "Tree%d" % (i + 1), _ref(q[0], q[1]), i * 47.0)
	# main.tscn's Kenney stones (stone_largeC/D/F at 2x / 4x), in the courtyard frame.
	_prop(props, scene, t, "rocks/stone_largeC.glb", "StoneC", Vector2(4.0, -3.1) * 1.6, 0.0, 2.0)
	_prop(props, scene, t, "rocks/stone_largeD.glb", "StoneD", Vector2(1.8, 6.1) * 1.6, 30.0, 2.0)
	_prop(props, scene, t, "rocks/stone_largeF.glb", "StoneF", Vector2(-7.0, 3.8) * 1.6, 0.0, 4.0)
	_prop(props, scene, t, "rocks/stone_tallB.glb", "StoneTall", Vector2(-3.5, -4.8) * 1.6, 15.0, 2.0)

	# robert-k's warm omni light, as a dithered pixel lamp.
	var lamp = ClassDB.instantiate("PixelLightVolume")
	var lp := Vector2(-7.27, 7.38)
	lamp.position = Vector3(lp.x, t.get_height_at(Vector3(lp.x, 0, lp.y), 0.0) + 1.3, lp.y)
	lamp.radius = 3.4
	_add(scene, lamp, scene, "Lamp")

	# robert-k's god rays, centred on the fountain (denser, fainter sheets than its 3 at
	# alpha 0.05, which wash out the Compatibility frame).
	var shafts = ClassDB.instantiate("PixelSunShafts")
	shafts.position = Vector3(0, 0.65, 0)
	shafts.alpha = 0.008
	shafts.count = 14
	shafts.fade_height = 9.0
	_add(scene, shafts, scene, "SunShafts")
	shafts.sun = sun

	var rig := (load("res://demo/fox_player.tscn") as PackedScene).instantiate() as Node3D
	var sp := Vector2(-1.0, 10.5)
	rig.position = Vector3(sp.x, t.get_height_at(Vector3(sp.x, 0, sp.y), 0.0), sp.y)
	_add(scene, rig, scene, "PlayerRig")

	# Overview camera (when the scene runs without the player rig): robert-k's
	# orthographic rig (yaw 45, pitch 30, 40 m back).
	var cam := Camera3D.new()
	var cam_basis := Basis.from_euler(Vector3(deg_to_rad(-30.0), deg_to_rad(45.0), 0.0), EULER_ORDER_YXZ)
	cam.transform = Transform3D(cam_basis, Vector3(0, 1, 0) + cam_basis.z * 40.0)
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 18.0
	cam.far = 100.0
	_add(scene, cam, scene, "Camera")

	var amb := AudioStreamPlayer.new()
	amb.stream = load("res://demo/assets/audio/wind_and_birds_loop.wav")
	amb.autoplay = true
	amb.volume_db = -8.0
	_add(scene, amb, scene, "Ambience")

	var renderer = ClassDB.instantiate("PixelRenderer")
	renderer.pixel_size = 3
	_add(scene, renderer, scene, "PixelRenderer")
	var diag = ClassDB.instantiate("PixelDiagnostics")
	_add(scene, diag, scene, "PixelDiagnostics")

	var packed := PackedScene.new()
	var err := packed.pack(scene)
	if err == OK:
		err = ResourceSaver.save(packed, SCENE_PATH)
	print("scene ", SCENE_PATH, ": ", err)
	scene.queue_free()
