# Headless regression check for undo / redo and live edit: every editor stroke is undone and
# redone through `set_chunks_data` (the method the editor's undo actions call, and the call
# Godot's live edit forwards to a running game). After each undo / redo, everything derived from
# the chunk data (chunk meshes, collision faces, grass, instanced HexPrism columns + their column
# map, scatter props, surface heights) must match a fresh terrain built from the same data.
# Strokes cross chunk borders (square chunks share border samples; hex-ring meshes depend on
# neighbour cells two steps away).
#   godot --headless --path . --script res://tests/undo_restore_check.gd
# Prints "UNDO RESTORE OK" or the mismatches (exit code 1).
extends SceneTree

const CONFIGS := [
	{"name": "hex rings prism instanced", "grid_type": 3, "ring_radius": 2, "cell_size": Vector2(2, 2), "hex_edge_style": 3, "hex_edge_instanced": true},
	{"name": "hex rings slopes", "grid_type": 3, "ring_radius": 2, "cell_size": Vector2(2, 2), "hex_edge_style": 2, "hex_edge_instanced": false},
	{"name": "square", "grid_type": 0, "chunk_size": 8, "cell_size": Vector2(1, 1)},
	{"name": "hex prism meshed", "grid_type": 2, "chunk_size": 6, "cell_size": Vector2(2, 2), "hex_edge_style": 3, "hex_edge_instanced": false},
]

var fails := 0
var frames := 0
var cfg_i := 0
var t
var steps: Array[Callable] = []
var step := 0
var wait := 0
var history := []  # [{before: Dictionary, after: Dictionary, name}]
var checks := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL [%s]: %s" % [CONFIGS[cfg_i].name, what])


func _make(cfg: Dictionary):
	var n = ClassDB.instantiate("MarchingSquaresTerrain")
	for k in cfg:
		if k != "name":
			n.set(k, cfg[k])
	n.grass_enabled = true
	n.noise_seed = 7
	return n


func _internal(node: Node, out: Array) -> Array:
	for c in node.get_children(true):
		out.append(c)
		_internal(c, out)
	return out


func _mm_key(mm: MultiMesh) -> Array:
	if mm == null:
		return ["none"]
	return [mm.instance_count, mm.buffer if mm.instance_count > 0 else PackedFloat32Array()]


# Everything derived from the chunk data, keyed by a readable name.
func _fingerprint(n) -> Dictionary:
	var fp := {}
	for c in _internal(n, []):
		var path := str(n.get_path_to(c))
		if c is MeshInstance3D:
			var m: Mesh = c.mesh
			fp[path] = ["none"] if m == null or m.get_surface_count() == 0 else m.surface_get_arrays(0)
		elif c is CollisionShape3D:
			var s = c.shape
			fp[path] = s.get_faces() if s is ConcavePolygonShape3D else [str(s)]
		elif c is MultiMeshInstance3D:
			fp[path] = _mm_key(c.multimesh) + [c.custom_aabb]
	for c in n.get_chunks():
		fp["collision %s" % c] = n.get_chunk_collision_faces(c)
	var img: Image = n.get_column_map()
	fp["column map"] = ["none"] if img == null else [n.get_column_map_rect(), img.get_data()]
	fp["columns"] = n.get_column_instance_count()
	fp["grass"] = n.get_grass_count()
	var heights := PackedFloat32Array()
	for z in range(-12, 26):
		for x in range(-12, 26):
			heights.append(n.get_height_at(Vector3(x * 0.71, 0, z * 0.71), -999.0))
	fp["heights"] = heights
	return fp


# Where two fingerprint values first differ (for the failure message).
func _first_diff(a, b) -> String:
	if typeof(a) != typeof(b):
		return "%s vs %s" % [type_string(typeof(a)), type_string(typeof(b))]
	if a is Array or a is PackedByteArray or a is PackedFloat32Array or a is PackedVector3Array or a is PackedVector2Array or a is PackedInt32Array or a is PackedColorArray:
		if a.size() != b.size():
			return "size %d vs %d" % [a.size(), b.size()]
		for i in a.size():
			if a[i] != b[i]:
				return "[%d] %s" % [i, _first_diff(a[i], b[i])]
		return "equal"
	return "%s vs %s" % [a, b]


func _blobs(n) -> Dictionary:
	var d := {}
	for c in n.get_chunks():
		d[c] = n.get_chunk_data(c)
	return d


# Changed chunks between two snapshots, as an undo / redo pair (empty blob = no chunk).
func _diff(a: Dictionary, b: Dictionary) -> Dictionary:
	var before := {}
	var after := {}
	for c in a.keys() + b.keys():
		var x: PackedByteArray = a.get(c, PackedByteArray())
		var y: PackedByteArray = b.get(c, PackedByteArray())
		if x != y:
			before[c] = x
			after[c] = y
	return {"before": before, "after": after}


# Compares `t` with a fresh terrain loaded from t's chunk data and scatter.
func _compare_fresh(what: String) -> void:
	checks += 1
	var f = _make(CONFIGS[cfg_i])
	f.scatter_items = t.scatter_items
	root.add_child(f)
	f.set_chunks_data(_blobs(t))
	f.set_scatter_data(t.get_scatter_data())
	var a := _fingerprint(t)
	var b := _fingerprint(f)
	var bad := []
	for k in a.keys() + b.keys():
		if a.get(k) != b.get(k) and not bad.has(k):
			bad.append(k)
	_check(bad.is_empty(), "%s: differs from a fresh build in %s" % [what, bad])
	for k in bad.slice(0, 3):
		printerr("   ", k, ": ", _first_diff(a.get(k), b.get(k)), " (fresh second)")
	f.free()


func _stroke(name: String, f: Callable) -> void:
	var before := _blobs(t)
	f.call()
	var d := _diff(before, _blobs(t))
	d["name"] = name
	_check(not d.before.is_empty(), "%s changed nothing" % name)
	history.append(d)


func _border(a: Vector2i, b: Vector2i) -> Vector3:
	return (t.chunk_to_world(a) + t.chunk_to_world(b)) * 0.5


func _setup_config() -> void:
	var cfg: Dictionary = CONFIGS[cfg_i]
	t = _make(cfg)
	root.add_child(t)
	for c in [Vector2i(0, 0), Vector2i(1, 0), Vector2i(0, 1), Vector2i(1, 1)]:
		t.add_chunk(c)
	var items: Array[PixelScatterItem] = [PixelScatterItem.create_builtin("rock")]
	items[0].density *= 4.0
	t.scatter_items = items
	history.clear()
	var mid: Vector3 = (t.chunk_to_world(Vector2i(0, 0)) + t.chunk_to_world(Vector2i(1, 1))) * 0.5
	var e01: Vector3 = _border(Vector2i(0, 0), Vector2i(1, 0))
	var e02: Vector3 = _border(Vector2i(0, 0), Vector2i(0, 1))
	steps = [
		# Painted props keep unquantized hues until they round-trip through the scatter blob.
		func(): t.scatter_paint(0, mid, 30.0); t.set_scatter_data(t.get_scatter_data()),
		func(): _compare_fresh("initial"),
		func(): _stroke("raise", func(): t.apply_brush("raise", e01, 3.0, 2.0, 0)),
		func(): _stroke("raise 2", func(): t.apply_brush("raise", mid, 4.0, 1.5, 0)),
		func(): _stroke("level", func(): t.apply_brush("level", e02, 2.5, 0.5, 0)),
		func(): _stroke("smooth", func(): t.apply_brush("smooth", e01, 3.0, 1.0, 0)),
		func(): _stroke("lower", func(): t.apply_brush("lower", mid, 2.0, 1.0, 0)),
		func(): _stroke("bridge", func(): t.apply_bridge(t.chunk_to_world(Vector2i(0, 0)), t.chunk_to_world(Vector2i(1, 1)) + Vector3(0, 3, 0), 2.0, 0.0)),
		func(): _stroke("grass mask", func(): t.apply_brush("grass_off", e02, 3.0, 0.0, 0)),
		func(): _stroke("vertex paint", func(): t.apply_brush("paint", mid, 3.0, 3.0, 0)),
		func(): _stroke("stamp", func(): t.apply_stamp(MarchingSquaresTerrain.create_stamp_image("Mountain", 64), mid, Vector2(10, 10), 0.0, 4.0, 1.0, 0)),
		func(): _compare_fresh("after strokes"),
		_undo_all,
		func(): _redo_all(),
		_undo_redo_mixed,
		func(): _compare_fresh("after mixed undo / redo"),
		_remove_chunk,
		func(): _compare_fresh("chunk removed"),
		_undo_remove,
		func(): _compare_fresh("after chunk remove undo"),
	]
	step = 0


func _undo(i: int) -> void:
	t.set_chunks_data(history[i].before)


func _redo(i: int) -> void:
	t.set_chunks_data(history[i].after)


func _undo_all() -> void:
	for i in range(history.size() - 1, -1, -1):
		_undo(i)
		_compare_fresh("undo %s" % history[i].name)


func _redo_all() -> void:
	for i in history.size():
		_redo(i)
		_compare_fresh("redo %s" % history[i].name)


func _undo_redo_mixed() -> void:
	var n := history.size()
	_undo(n - 1)
	_undo(n - 2)
	_redo(n - 2)
	_undo(n - 2)
	_undo(n - 3)
	_compare_fresh("undo x3")
	_redo(n - 3)
	_redo(n - 2)
	_redo(n - 1)


var removed := {}


func _remove_chunk() -> void:
	var before := _blobs(t)
	t.remove_chunk(Vector2i(1, 0))  # rebuilds on the next frame
	removed = _diff(before, _blobs(t))


func _undo_remove() -> void:
	t.set_chunks_data(removed.before)


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup_config()
		wait = 2
		return false
	if wait > 0:
		wait -= 1
		return false
	if step < steps.size():
		steps[step].call()
		step += 1
		wait = 2
		return false
	t.free()
	cfg_i += 1
	if cfg_i < CONFIGS.size():
		_setup_config()
		wait = 2
		return false
	print("UNDO RESTORE OK (%d comparisons)" % checks if fails == 0 else "UNDO RESTORE: %d failure(s)" % fails)
	quit(1 if fails else 0)
	return false
