# Shared helpers of the terrain live-edit / undo checks (tests/live_edit_*_check.gd):
# a fingerprint of everything a MarchingSquaresTerrain draws or collides with, and chunk-blob
# snapshots. Under the headless (dummy) renderer MultiMesh buffers do not read back, so the
# column instances / grass blades compare by count, mesh and AABB; the column map compares
# its CPU copy (texel for texel).
extends RefCounted


static func internal(node: Node, out: Array) -> Array:
	for c in node.get_children(true):
		out.append(c)
		internal(c, out)
	return out


# Node path with auto-generated names (@Class@N) replaced by class + child index.
static func stable_path(base: Node, n: Node) -> String:
	var parts := []
	while n != base:
		var nm := str(n.name)
		parts.push_front("%s#%d" % [n.get_class(), n.get_index(true)] if nm.begins_with("@") else nm)
		n = n.get_parent()
	return "/".join(parts)


# The editor plugin's cursor / chunk-line meshes (internal MeshInstance3D children of the
# terrain itself, drawn from the mouse position) are not terrain output.
static func _editor_overlay(base: Node, n: Node) -> bool:
	while n.get_parent() != base:
		n = n.get_parent()
	return n is MeshInstance3D or str(n.name).begins_with("MarchingSquaresTerrain")


# PixelFireSystem flames / soot / steam: live particles (random per frame, e.g. the demo's
# campfire hearth), not terrain output.
static func _fire_particles(n: Node) -> bool:
	return n is MultiMeshInstance3D and n.get_parent() != null and n.get_parent().is_class("PixelFireSystem")


static func mm_key(mm: MultiMesh) -> Array:
	if mm == null:
		return ["none"]
	return [mm.instance_count, mm.visible_instance_count, mesh_key(mm.mesh), mm.buffer if mm.instance_count > 0 else PackedFloat32Array()]


# Meshes by content (templates are rebuilt per terrain).
static func mesh_key(m: Mesh):
	if m == null:
		return "none"
	if m is PrimitiveMesh:
		return [m.get_class(), m.size if "size" in m else null]
	return [m.get_surface_count(), hash(var_to_bytes(m.surface_get_arrays(0))) if m.get_surface_count() > 0 else 0]


static func fingerprint(n) -> Dictionary:
	var fp := {}
	for c in internal(n, []):
		if _editor_overlay(n, c) or _fire_particles(c):
			continue
		var path := stable_path(n, c)
		if c is Node3D:
			fp[path + " :xf"] = [c.transform, c.visible]
		if c is GeometryInstance3D:
			fp[path + " :gi"] = [c.layers, c.custom_aabb, c.material_override, c.cast_shadow]
		if c is MeshInstance3D:
			var m: Mesh = c.mesh
			fp[path] = ["none"] if m == null or m.get_surface_count() == 0 else [m.get_surface_count(), m.surface_get_arrays(0)]
		elif c is CollisionShape3D:
			var s = c.shape
			fp[path] = [c.disabled, s.get_faces() if s is ConcavePolygonShape3D else str(s)]
		elif c is MultiMeshInstance3D:
			fp[path] = mm_key(c.multimesh)
		elif c is CollisionObject3D:
			fp[path] = [c.collision_layer, c.collision_mask]
	var img: Image = n.get_column_map()
	fp["column map"] = ["none"] if img == null else [n.get_column_map_rect(), img.get_data()]
	fp["columns"] = n.get_column_instance_count()
	fp["grass"] = n.get_grass_count()
	fp["lod"] = n.get_low_detail_chunks()
	var heights := PackedFloat32Array()
	var b := AABB(n.chunk_to_world(n.get_chunks()[0]), Vector3.ZERO)
	for c in n.get_chunks():
		b = b.expand(n.chunk_to_world(c))
	b = b.grow(12.0)
	for z in 48:
		for x in 48:
			heights.append(n.get_height_at(b.position + Vector3(b.size.x * x / 47.0, 0, b.size.z * z / 47.0), -999.0))
	fp["heights"] = heights
	return fp


static func first_diff(a, b) -> String:
	if typeof(a) != typeof(b):
		return "%s vs %s" % [type_string(typeof(a)), type_string(typeof(b))]
	if a is Array or a is PackedByteArray or a is PackedFloat32Array or a is PackedVector3Array or a is PackedVector2Array or a is PackedInt32Array or a is PackedColorArray:
		if a.size() != b.size():
			return "size %d vs %d" % [a.size(), b.size()]
		for i in a.size():
			if a[i] != b[i]:
				return "[%d] %s" % [i, first_diff(a[i], b[i])]
		return "equal"
	return "%s vs %s" % [a, b]


static func blobs(n) -> Dictionary:
	var d := {}
	for c in n.get_chunks():
		d[c] = n.get_chunk_data(c)
	return d


static func diff(a: Dictionary, b: Dictionary) -> Dictionary:
	var before := {}
	var after := {}
	for c in a.keys() + b.keys():
		var x: PackedByteArray = a.get(c, PackedByteArray())
		var y: PackedByteArray = b.get(c, PackedByteArray())
		if x != y:
			before[c] = x
			after[c] = y
	return {"before": before, "after": after}




# Keys whose values differ between two fingerprints.
static func mismatches(a: Dictionary, b: Dictionary) -> Array:
	var bad := []
	for k in a.keys() + b.keys():
		if a.get(k) != b.get(k) and not bad.has(k):
			bad.append(k)
	return bad
