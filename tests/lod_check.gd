# Headless check of the distant low-detail ring (lod_distance / lod_focus / update_lod_at;
# low chunks: simplified mesh, no grass, no scatter props):
#   godot --headless --path . --script res://tests/lod_check.gd
# Prints "LOD OK" or the failed checks (exit code 1).
extends SceneTree

var fails := 0
var frames := 0
var t
var focus: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _faces(c: Vector2i) -> int:
	var m: Mesh = t.get_chunk_mesh(c)
	return m.get_faces().size() / 3 if m else 0


func _grass(c: Vector2i) -> int:
	return t.get_chunk_grass_count(c)


# Scatter props drawn: instances in the internal Scatter node's MultiMeshes (headless keeps
# the counts, not the transforms).
func _shown() -> int:
	var n := 0
	for s in t.get_children(true):
		if s.name == "Scatter":
			for mmi in s.get_children():
				if mmi is MultiMeshInstance3D and not mmi.is_queued_for_deletion():
					n += mmi.multimesh.instance_count
	return n


# Collision shapes of the scatter props (shape owners of the bodies under the internal Scatter node).
func _shapes() -> int:
	var n := 0
	for s in t.get_children(true):
		if s.name == "Scatter":
			for b in s.get_children():
				if b is StaticBody3D and not b.is_queued_for_deletion():
					n += b.get_shape_owners().size()
	return n


# Stored props outside the low-detail chunks (or in chunk `only`).
func _props_outside_low(only := Vector2i(-99, -99)) -> int:
	var low: Array = t.get_low_detail_chunks()
	var n := 0
	for p in t.get_scatter_positions(-1):
		var c: Vector2i = t.get_cell_chunk(t.world_to_cell(t.to_global(Vector3(p.x, 0.0, p.y))))
		if only != Vector2i(-99, -99):
			n += 1 if c == only else 0
		elif not low.has(c):
			n += 1
	return n


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		_setup()
	elif frames == 4:
		_run()
	elif frames == 6:
		_run_focus()
	elif frames == 8:
		_camera_setup()
	elif frames == 12:
		_run_camera()
	elif frames == 16:
		_run_camera_tilted()
		print("LOD OK" if fails == 0 else "LOD: %d failure(s)" % fails)
		quit(1 if fails else 0)
	return false


func _setup() -> void:
	t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = 2
	t.cell_size = Vector2(2, 2)
	t.level_height = 1.0
	t.hex_edge_style = 3
	t.grass_subdivisions = 2
	root.add_child(t)
	for cr in range(-3, 4):
		for cq in range(-3, 4):
			t.add_chunk(Vector2i(cq, cr))
	# Bumpy columns with grass everywhere, so full chunks have bevels, grooves and blades.
	var i := 0
	for c in t.get_chunks():
		for cell in t.get_chunk_cells(c):
			t.set_cell_level(cell, (i * 7) % 4)
			i += 1
	# One built-in prop per cell, so every chunk has props.
	var item = ClassDB.class_call_static("PixelScatterItem", "create_builtin", "rock")
	item.collision = 1
	var items = t.scatter_items
	items.append(item)
	t.scatter_items = items
	t.scatter_fill_cells(0)
	t.rebuild()


func _run() -> void:
	var all: Array = t.get_chunks()
	_check(t.get_low_detail_chunks().is_empty(), "ring off by default")
	var far := Vector2i(3, 3)
	var full_faces := _faces(far)
	var full_grass := _grass(far)
	var all_props := _shown()
	var all_shapes := _shapes()
	var stored: int = t.get_scatter_positions(-1).size()
	_check(stored > 0 and all_props >= stored and _props_outside_low(far) > 0, "props everywhere (%d drawn, %d stored)" % [all_props, stored])
	var parts := all_props / maxi(stored, 1)
	_check(full_faces > 0 and full_grass > 0, "full chunk has faces and grass (%d, %d)" % [full_faces, full_grass])
	t.lod_distance = 15.0
	t.update_lod_at(Vector3.ZERO)
	var low: Array = t.get_low_detail_chunks()
	_check(not low.is_empty() and low.size() < all.size(), "some chunks low (%d of %d)" % [low.size(), all.size()])
	_check(not low.has(Vector2i(0, 0)), "focus chunk stays full")
	_check(low.has(far), "far chunk is low")
	for c in low:
		var p: Vector3 = t.sample_to_world(t.get_chunk_center_cell(c))
		_check(Vector2(p.x, p.z).length() > 15.0 - 3.0, "low chunk %s beyond the distance" % c)
	_check(_faces(far) * 3 < full_faces, "low chunk is simplified (%d vs %d faces)" % [_faces(far), full_faces])
	_check(_grass(far) == 0, "no grass on low chunks")
	_check(_grass(Vector2i(0, 0)) > 0, "grass on full chunks")
	var expect := _props_outside_low() * parts
	_check(_shown() < all_props and _shown() == expect, "no props on low chunks (%d drawn, %d expected)" % [_shown(), expect])
	_check(all_shapes == stored and _shapes() == all_shapes, "prop colliders ignore the ring (%d / %d, %d stored)" % [_shapes(), all_shapes, stored])
	# Heights are unchanged on low chunks: flat tops at the cell levels.
	var cell: Vector2i = t.get_chunk_center_cell(far)
	var w: Vector3 = t.sample_to_world(cell)
	_check(absf(t.get_height_at(w, -99.0) - t.get_cell_height(cell)) < 1e-3, "low chunk top height")
	# Off again: everything back to full detail.
	t.lod_distance = 0.0
	t.update_lod_at(Vector3.ZERO)
	_check(t.get_low_detail_chunks().is_empty(), "ring off again")
	_check(_faces(far) == full_faces and _grass(far) == full_grass, "far chunk full again")
	_check(_shown() == all_props, "props back (%d / %d)" % [_shown(), all_props])


func _run_focus() -> void:
	# lod_focus drives the ring from process(): focus at the far corner, origin goes low.
	focus = Node3D.new()
	root.add_child(focus)
	var p: Vector3 = t.sample_to_world(t.get_chunk_center_cell(Vector2i(3, 3)))
	focus.global_position = p
	t.lod_focus = focus
	t.lod_distance = 15.0
	t.update_lod_at(focus.global_position)
	var low: Array = t.get_low_detail_chunks()
	_check(low.has(Vector2i(0, 0)) and not low.has(Vector2i(3, 3)), "ring follows the focus")


# No lod_focus: the camera drives the ring. A level camera (pitch 0, its view ray never meets
# the base plane) must keep its own surroundings at full detail, props drawn and collidable.
var cam: Camera3D


func _camera_setup() -> void:
	t.lod_focus = null
	focus.queue_free()
	cam = Camera3D.new()
	root.add_child(cam)
	cam.current = true
	cam.global_position = Vector3(0, 4, 0)
	cam.rotation = Vector3.ZERO  # looking -Z, level
	t.lod_distance = 15.0  # resets the LOD timer: re-evaluated on the next frames


func _run_camera() -> void:
	var low: Array = t.get_low_detail_chunks()
	_check(not low.is_empty(), "level camera: ring active (%d low)" % low.size())
	_check(not low.has(Vector2i(0, 0)), "level camera: chunk under the camera stays full %s" % [low])
	_check(_grass(Vector2i(0, 0)) > 0, "level camera: grass under the camera")
	_check(_props_outside_low(Vector2i(0, 0)) > 0 and _shown() > 0, "level camera: props drawn near the camera")
	_check(_shapes() == t.get_scatter_positions(-1).size(), "level camera: colliders everywhere")
	# Slightly downward: the ray meets the plane ~115 m ahead; the focus must stay close.
	cam.rotation = Vector3(deg_to_rad(-2.0), 0, 0)
	t.lod_distance = 15.0


func _run_camera_tilted() -> void:
	var low: Array = t.get_low_detail_chunks()
	_check(not low.is_empty() and not low.has(Vector2i(0, 0)), "tilted camera: chunk under the camera stays full %s" % [low])
	_check(_shapes() == t.get_scatter_positions(-1).size(), "tilted camera: colliders everywhere")
