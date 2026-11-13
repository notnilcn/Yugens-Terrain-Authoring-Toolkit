# Headless check of the hex game additions: GPU-instanced prism columns, the height / water /
# biome map API and map window, water levels + waterfalls, and small-hex top decorations
# (incl. the small-hex scatter mode):
#   godot --headless --path . --script res://tests/hex_columns_check.gd
# Prints "HEX COLUMNS OK" or the failed checks (exit code 1).
extends SceneTree

var fails := 0
var frames := 0
var t
var window_signals := 0
var water_node


func _check(ok: bool, what: String) -> void:
	if not ok:
		fails += 1
		printerr("FAIL: ", what)


func _internal(node: Node, cls: String, name_part: String) -> Array:
	var out := []
	for c in node.get_children(true):
		if c.is_class(cls) and (name_part == "" or str(c.name).contains(name_part)):
			out.append(c)
		out.append_array(_internal(c, cls, name_part))
	return out


func _column_nodes() -> Array:
	return _internal(t, "MultiMeshInstance3D", "Columns")


var stages: Array[Callable] = []
var stage := 0
var wait := 0


func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		# Game scale: LargeHex = 6 units, chunk == patch (19 cells), HexPrism columns.
		t = ClassDB.instantiate("MarchingSquaresTerrain")
		t.grid_type = 3
		t.ring_radius = 2
		t.cell_size = Vector2(6, 6)
		t.level_height = 0.5
		t.hex_edge_style = 3
		t.grass_enabled = false
		root.add_child(t)
		t.add_chunk(Vector2i(0, 0))
		t.add_chunk(Vector2i(1, 0))
		t.set_cell_height(Vector2i(0, 0), 3.0)
		t.set_cell_height(Vector2i(1, 0), 0.5)
		t.map_window_changed.connect(func(_r): window_signals += 1)
		stages = [_meshed_then_instanced, _instanced_checks, _height_edit_checked, _lod_on, _lod_checked, _meshed_again, _instanced_again, _map_api, _decorations, _water_node_setup, _water_node_check]
		wait = 2
		return false
	if wait > 0:
		wait -= 1
		return false
	if stage < stages.size():
		stages[stage].call()
		stage += 1
		wait = 2
		return false
	print("HEX COLUMNS OK" if fails == 0 else "HEX COLUMNS: %d failure(s)" % fails)
	quit(1 if fails else 0)
	return false


func _v2i(a: Array) -> Array[Vector2i]:
	var out: Array[Vector2i] = []
	for v in a:
		out.append(v)
	return out


func _meshed_then_instanced() -> void:
	_check(t.get_column_instance_count() == 0 and _column_nodes().is_empty(), "meshed by default")
	var meshes := _internal(t, "MeshInstance3D", "Mesh")
	_check(meshes.size() == 2 and meshes[0].mesh != null, "two meshed chunks")
	t.hex_edge_instanced = true


func _instanced_checks() -> void:
	_check(t.get_column_instance_count() == 38, "one instance per cell (38), got %d" % t.get_column_instance_count())
	var cols := _column_nodes()
	_check(cols.size() == 2, "a Columns MultiMeshInstance3D per chunk")
	for m in _internal(t, "MeshInstance3D", "Mesh"):
		_check(m.mesh == null, "chunk mesh hidden while instanced")
	if cols.size() > 0:
		var mm: MultiMesh = cols[0].multimesh
		_check(mm.use_custom_data and mm.mesh != null, "template mesh + custom data")
		_check(cols[0].custom_aabb.size.y > 3.0, "custom AABB covers the columns %s" % cols[0].custom_aabb)
		_check(cols[0].material_override == t.terrain_material, "columns use the terrain material")
	# The column map: texel = cell - rect.position, R = height.
	var rect: Rect2i = t.get_column_map_rect()
	var tex: Texture2D = t.get_column_map_texture()
	_check(tex != null and Vector2i(rect.size.x * 6, rect.size.y) == Vector2i(tex.get_width(), tex.get_height()), "column map texture %s" % rect)
	if tex:
		var img: Image = t.get_column_map()
		var p := Vector2i(0, 0) - rect.position
		_check(is_equal_approx(img.get_pixel(p.x * 6, p.y).r, 3.0), "map height of (0, 0)")
		# Height edits re-upload the map (no re-instancing).
		t.set_cell_height(Vector2i(0, 0), 4.5)


func _height_edit_checked() -> void:
	var rect: Rect2i = t.get_column_map_rect()
	var p := Vector2i(0, 0) - rect.position
	var img: Image = t.get_column_map()
	_check(is_equal_approx(img.get_pixel(p.x * 6, p.y).r, 4.5), "map follows edits")
	# Collision / height queries still follow the prism surface.
	var c0: Vector3 = t.cell_to_world(Vector2i(0, 0))
	_check(is_equal_approx(t.get_height_at(c0, -1.0), 4.5), "get_height_at on an instanced column")
	var hit: Dictionary = t.raycast(c0 + Vector3(0, 20, 0), Vector3.DOWN, 100.0)
	_check(hit.get("hit", false) and hit["cell"] == Vector2i(0, 0), "raycast on an instanced column")


func _lod_on() -> void:
	# Distant LOD: low chunks draw the low template (INSTANCE_CUSTOM.b = 1).
	t.lod_distance = 10.0
	t.update_lod_at(t.chunk_to_world(Vector2i(0, 0)))


func _lod_checked() -> void:
	# The low chunk draws the groove-less template (fewer faces than the full one).
	var faces := []
	for m in _column_nodes():
		faces.append(m.multimesh.mesh.get_faces().size())
	faces.sort()
	_check(t.get_low_detail_chunks().size() == 1 and faces.size() == 2 and faces[0] < faces[1], "LOD chunk uses low-detail columns %s" % [faces])
	t.lod_distance = 0.0
	t.update_lod_at(t.chunk_to_world(Vector2i(0, 0)))
	# Switching back to meshed removes the instances.
	t.hex_edge_instanced = false


func _meshed_again() -> void:
	_check(t.get_column_instance_count() == 0 and _column_nodes().is_empty(), "meshed again")
	var meshes := _internal(t, "MeshInstance3D", "Mesh")
	_check(meshes.size() == 2 and meshes[0].mesh != null, "meshes back")
	t.hex_edge_instanced = true


func _instanced_again() -> void:
	_check(t.get_column_instance_count() == 38, "instanced again")


func _map_api() -> void:
	var rect: Rect2i = t.get_map_rect_around(Vector2i(0, 0), 2)
	_check(rect == Rect2i(-2, -2, 5, 5), "rect around %s" % rect)
	var cm: Image = t.get_cell_map(rect)
	_check(cm.get_format() == Image.FORMAT_RGBAF and cm.get_width() == 5, "cell map format")
	_check(is_equal_approx(cm.get_pixel(2, 2).r, 4.5), "cell map height")
	_check(cm.get_pixel(0, 0).a < -0.5, "(-2, -2) is outside the patch: missing")
	# Height map upload (RF) round-trips through get_height_map.
	var up := Image.create(3, 1, false, Image.FORMAT_RF)
	up.set_pixel(0, 0, Color(1.0, 0, 0))
	up.set_pixel(1, 0, Color(1.5, 0, 0))
	up.set_pixel(2, 0, Color(2.0, 0, 0))
	t.set_height_map(Vector2i(-1, 1), up, 1.0)
	_check(is_equal_approx(t.get_cell_height(Vector2i(0, 1)), 1.5), "set_height_map")
	var hm: Image = t.get_height_map(Rect2i(-1, 1, 3, 1), -99.0)
	_check(hm.get_format() == Image.FORMAT_RF and is_equal_approx(hm.get_pixel(2, 0).r, 2.0), "get_height_map")
	# Level maps as 8-bit images with a scale.
	var lv := Image.create(1, 1, false, Image.FORMAT_L8)
	lv.set_pixel(0, 0, Color8(6, 6, 6))
	t.set_height_map(Vector2i(0, -1), lv, 255.0 * t.level_height)
	_check(is_equal_approx(t.get_cell_height(Vector2i(0, -1)), 3.0), "L8 level map x scale")
	# Biomes = floor slots.
	var bm := Image.create(1, 1, false, Image.FORMAT_R8)
	bm.set_pixel(0, 0, Color8(7, 0, 0))
	t.set_biome_map(Vector2i(1, -1), bm)
	_check(t.get_sample_slot(Vector2i(1, -1), false) == 7, "set_biome_map")
	var b: Image = t.get_biome_map(rect)
	_check(b.get_format() == Image.FORMAT_R8 and b.get_pixel(3, 1).r8 == 7, "get_biome_map")
	# Water levels: map + waterfalls (5 levels of 0.5 = 2.5).
	t.set_cell_height(Vector2i(0, 0), 4.5)
	t.set_cell_height(Vector2i(1, 0), 0.5) # drop of 4.5 from the pool surface
	t.set_cell_height(Vector2i(-1, 0), 4.0) # drop of 1.0: a cascade
	t.set_cell_water_level(Vector2i(0, 0), 5.0)
	_check(is_equal_approx(t.get_cell_water_level(Vector2i(0, 0)), 5.0) and t.get_cell_water_level(Vector2i(0, 1)) < -9000.0, "water levels")
	var wm: Image = t.get_water_map(rect)
	_check(is_equal_approx(wm.get_pixel(2, 2).r, 5.0) and wm.get_pixel(2, 3).r < -9000.0, "water map")
	var falls: Array = t.get_waterfalls(5)
	var big := 0
	var small := 0
	for f in falls:
		if f["neighbor"] == Vector2i(1, 0):
			big += 1 if f["waterfall"] else 0
		if f["neighbor"] == Vector2i(-1, 0):
			small += 1 if not f["waterfall"] else 0
	_check(big == 1 and small == 1, "waterfall where a neighbour drops 5+ levels, cascade otherwise: %s" % [falls])
	# Water levels live in the chunk data (PTC2 blobs, saved with the terrain data) and feed
	# PixelWater's pools / waterfalls.
	t.set_cell_water_levels(_v2i([Vector2i(0, 0), Vector2i(0, 1)]), PackedFloat32Array([5.0, 0.0]))
	var blob: PackedByteArray = t.get_chunk_data(Vector2i(0, 0))
	_check(blob.slice(0, 4).get_string_from_ascii() == "PTC2", "water stored in the chunk blob")
	var raised: PackedVector3Array = t.get_raised_water_cells()
	_check(raised.size() == 1 and raised[0] == Vector3(0, 0, 5.0), "raised cells %s" % raised)
	_check(t.get_water_cells().size() == 2, "stored water levels (dry one included)")
	var t2 = ClassDB.instantiate("MarchingSquaresTerrain")
	t2.grid_type = 3
	t2.ring_radius = 2
	t2.cell_size = Vector2(6, 6)
	root.add_child(t2)
	t2.set_chunk_data(Vector2i(0, 0), blob)
	_check(is_equal_approx(t2.get_cell_water_level(Vector2i(0, 0)), 5.0), "water level survives a blob round trip")
	t2.queue_free()
	t.clear_water_levels()
	_check(t.get_raised_water_cells().is_empty() and t.get_chunk_data(Vector2i(0, 0)).slice(0, 4).get_string_from_ascii() == "PTC1", "cleared: back to PTC1")
	# Bake / GLB export work in instanced mode (meshed on the fly).
	_check(t.export_glb("user://hex_columns_check.glb") == OK, "export_glb while instanced")
	# Map window around a point: (2R+1)² texture, recentred on demand, signal.
	t.map_window_radius = 4
	_check(t.update_map_window_at(t.cell_to_world(Vector2i(1, -1))), "window built")
	var win: Rect2i = t.get_map_window()
	var mt: Texture2D = t.get_map_texture()
	_check(win == Rect2i(-3, -5, 9, 9) and mt != null and mt.get_width() == 9, "window %s" % win)
	_check(window_signals >= 1, "map_window_changed emitted")
	if mt:
		var p := Vector2i(1, -1) - win.position
		_check(mt.get_image().get_pixel(p.x, p.y).b == 7.0, "window texel = cell map texel")
	t.map_window_radius = 0


func _decorations() -> void:
	# Flat-topped prism column: all 7 small hexes; centre small hex = 3 x cell.
	var c := Vector2i(1, -1)
	var hexes: Array = t.get_cell_decoration_hexes(c, -100.0)
	var spots: PackedVector3Array = t.get_cell_decoration_spots(c, -100.0)
	_check(hexes.size() == 7 and spots.size() == 7 and hexes[0] == Vector2i(3, -3), "7 small hexes %s" % [hexes])
	for h in hexes:
		_check(t.small_hex_to_cell(h) == c, "small hex %s belongs to the cell" % h)
	# Paved / built-on small hexes are skipped, flooded cells too.
	t.set_small_hexes_blocked(_v2i([Vector2i(3, -3)]), true)
	_check(t.get_cell_decoration_hexes(c, -100.0).size() == 6 and t.is_small_hex_blocked(Vector2i(3, -3)), "blocked small hex")
	t.set_cell_water_level(c, t.get_cell_height(c) + 1.0)
	_check(t.get_cell_decoration_spots(c, -100.0).is_empty(), "submerged cell")
	t.clear_water_levels()
	t.clear_blocked_small_hexes()
	# Small-hex scatter mode: props exactly on small-hex centres, removed when paved.
	var item = ClassDB.instantiate("PixelScatterItem")
	item.shape = 4
	item.small_hexes = true
	item.density = 10.0
	item.per_cell_max = 7
	item.slope_max = 90.0
	var items: Array[PixelScatterItem] = [item]
	t.scatter_items = items
	t.scatter_fill_cells(0)
	var n: int = t.get_scatter_count(0)
	_check(n == 38 * 7, "7 props per column, got %d" % n)
	var on_centres := true
	for p in t.get_scatter_positions(0):
		var w := Vector3(p.x, 0, p.y)
		var s: Vector2i = t.world_to_small_hex(w)
		var sw: Vector3 = t.small_hex_to_world(s, 0.0)
		if Vector2(sw.x, sw.z).distance_to(p) > 1e-3:
			on_centres = false
	_check(on_centres, "props stand on small-hex centres")
	t.set_small_hexes_blocked(_v2i([Vector2i(3, -3), Vector2i(4, -3)]), true)
	_check(t.get_scatter_count(0) == n - 2, "paving removes the props there")
	t.scatter_fill_cells(0)
	_check(t.get_scatter_count(0) == n - 2, "refill keeps blocked small hexes free")
	t.scatter_items = [] as Array[PixelScatterItem]


func _water_node_setup() -> void:
	# PixelWater draws the terrain's stored raised water (no water_cells of its own).
	t.set_cell_height(Vector2i(0, 0), 4.5)
	t.set_cell_height(Vector2i(1, 0), 0.5)
	t.set_cell_water_level(Vector2i(0, 0), 5.0)
	water_node = ClassDB.instantiate("PixelWater")
	water_node.size = Vector2(60, 60)
	water_node.position = Vector3(0, -5, 0)
	water_node.terrain = t
	root.add_child(water_node)


func _water_node_check() -> void:
	water_node.update_waterfalls()
	var falls: Array = water_node.get_waterfalls()
	var hit := false
	for f in falls:
		if f["cell"] == Vector2i(0, 0) and f["neighbor"] == Vector2i(1, 0) and f["waterfall"]:
			hit = true
	_check(water_node.water_cells.is_empty() and hit, "PixelWater falls from stored water levels: %s" % [falls])
