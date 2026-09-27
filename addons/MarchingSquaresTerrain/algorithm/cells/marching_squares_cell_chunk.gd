@tool
class_name MarchingSquaresCellChunk
extends MarchingSquaresTerrainChunkBase
## Shared chunk implementation for cell-centered modes (triangle, hexagon).
## Each cell renders as a flat regular polygon at its own height plus vertical
## wall quads where a neighbor is lower. Subclasses provide the coordinate and
## shape math by delegating to MarchingSquaresHexGrid / MarchingSquaresTriGrid.


# Terrain-side state is fetched through terrain_system (see the base class).
var global_position_cached : Vector3 = Vector3.ZERO

#region temporary storage vars (mirrors the square chunk save hooks)
var _temp_mesh : ArrayMesh
var _temp_collision_shapes : Array[ConcavePolygonShape3D] = []
var _temp_height_map : Array = []
#endregion


#region subclas responsibilities (virtuals)

## Number of cells along x and y in this chunk.
func cells_per_chunk() -> Vector2i:
	return Vector2i.ZERO


## World-space position of a global cell's center.
func cell_center_world(_cell: Vector2i) -> Vector2:
	return Vector2.ZERO


## World-space corner offsets of a global cell relative to its center.
func cell_corner_offsets(_cell: Vector2i) -> PackedVector2Array:
	return PackedVector2Array()


## Edge neighbors of a global cell, indexed by edge.
func edge_neighbors(_cell: Vector2i) -> Array[Vector2i]:
	return []


## Corner index pair carrying an edge.
func edge_corner_indices(_cell: Vector2i, _edge: int) -> Vector2i:
	return Vector2i.ZERO


## World-space position of the chunk's local cell (0, 0) reference point
## (the cell center for hex, the lattice origin for triangle).
func get_chunk_origin_world() -> Vector2:
	return cell_center_world(_chunk_global_cell(Vector2i.ZERO))

#endregion


#region helpers

## Global cell coordinate for a local cell coordinate.
func _chunk_global_cell(local: Vector2i) -> Vector2i:
	return global_cell(chunk_coords, local, cells_per_chunk())


func cell_index(cc: Vector2i) -> int:
	return cc.y * cells_per_chunk().x + cc.x


func global_cell(chunk: Vector2i, local: Vector2i, cells: Vector2i) -> Vector2i:
	return chunk * cells + local


## Rounded world position of a global cell's center.
func _cell_center_rounded(cell: Vector2i) -> Vector2:
	return cell_center_world(cell)


#endregion


func initialize_terrain(should_regenerate_mesh: bool = true):
	var count := cells_per_chunk().x * cells_per_chunk().y
	var generated := false
	if not height_map or height_map.size() != count:
		generate_height_map()
		generated = true
	if not color_map_0 or color_map_0.size() != count:
		generate_color_maps()
	if not wall_color_map_0 or wall_color_map_0.size() != count:
		generate_wall_color_maps()
	if not grass_mask_map or grass_mask_map.size() != count:
		generate_grass_mask_map()
	
	if not mesh and should_regenerate_mesh:
		regenerate_mesh(false)
	elif mesh:
		if terrain_system:
			mesh.surface_set_material(0, terrain_system.terrain_material)
		if not _temp_collision_shapes.is_empty():
			_recreate_collision_body()
		else:
			for child in get_children():
				if child is StaticBody3D:
					child.free()
			create_trimesh_collision()
			_setup_collision_layers()
	
	if generated:
		mark_dirty()


func _notification(what: int) -> void:
	if not EngineWrapper.instance.is_editor():
		return
	
	match what:
		NOTIFICATION_EDITOR_PRE_SAVE:
			_skip_save_on_exit = _skip_save_on_exit # Surpress warning
			_temp_height_map = height_map
			height_map = []
			
			_temp_mesh = mesh
			mesh = null
			
			_temp_collision_shapes.clear()
			var bodies_to_free : Array[StaticBody3D] = []
			for child in get_children():
				if child is StaticBody3D:
					for shape_child in child.get_children():
						if shape_child is CollisionShape3D and shape_child.shape is ConcavePolygonShape3D:
							_temp_collision_shapes.append(shape_child.shape)
							shape_child.shape = null
						shape_child.owner = null
					child.owner = null
					bodies_to_free.append(child)
			for body in bodies_to_free:
				body.name += "_"
				body.queue_free()
		
		NOTIFICATION_EDITOR_POST_SAVE:
			if _temp_height_map:
				height_map = _temp_height_map
				_temp_height_map = []
			
			if _temp_mesh:
				mesh = _temp_mesh
				_temp_mesh = null
			
			if not _temp_collision_shapes.is_empty():
				_recreate_collision_body.call_deferred()
		
		NOTIFICATION_PREDELETE:
			for child in get_children():
				if child is StaticBody3D:
					child.owner = null
					for shape_child in child.get_children():
						if shape_child is CollisionShape3D:
							shape_child.owner = null


func _enter_tree() -> void:
	if get_parent() != terrain_system:
		push_error("Chunk must remain within its parent!")
	if terrain_system:
		terrain_system.chunks[chunk_coords] = self


func _exit_tree() -> void:
	_temp_height_map = []
	_temp_mesh = null
	_temp_collision_shapes.clear()
	
	if EngineWrapper.instance.is_editor():
		for child in get_children():
			if child is StaticBody3D:
				child.owner = null
				for shape_child in child.get_children():
					if shape_child is CollisionShape3D:
						shape_child.owner = null
	
	if terrain_system and terrain_system.chunks.get(chunk_coords) == self:
		terrain_system.chunks.erase(chunk_coords)


## Recreate collision body after scene save (deferred call for proper physics refresh).
func _recreate_collision_body() -> void:
	if not is_inside_tree() or _temp_collision_shapes.is_empty():
		_temp_collision_shapes.clear()
		return
	
	for child in get_children():
		if child is StaticBody3D:
			child.free()
	
	var shape : ConcavePolygonShape3D = _temp_collision_shapes[0]
	_temp_collision_shapes.clear()
	
	var body := StaticBody3D.new()
	body.name = name + "_col"
	body.collision_layer = 17
	if terrain_system:
		body.set_collision_layer_value(terrain_system.extra_collision_layer, true)
	
	var col_shape := CollisionShape3D.new()
	col_shape.name = "CollisionShape3D"
	col_shape.shape = shape
	col_shape.visible = false
	body.add_child(col_shape)
	add_child(body)
	
	if EngineWrapper.instance.is_editor():
		var scene_root = EngineWrapper.instance.get_root_for_node(self)
		if scene_root:
			body.owner = scene_root
			col_shape.owner = scene_root
		for group in get_groups():
			if group.begins_with("navmesh_"):
				body.add_to_group(group)


func _setup_collision_layers() -> void:
	for child in get_children():
		if child is StaticBody3D:
			child.collision_layer = 17
			if terrain_system:
				child.set_collision_layer_value(terrain_system.extra_collision_layer, true)
			for _child in child.get_children():
				if _child is CollisionShape3D:
					_child.set_visible(false)


#region cell_geometry generators (when empty)

func generate_height_map():
	var cells := cells_per_chunk()
	var count := cells.x * cells.y
	height_map = []
	height_map.resize(count)
	var noise = terrain_system.noise_hmap if terrain_system else null
	for r in range(cells.y):
		for c in range(cells.x):
			var global := _chunk_global_cell(Vector2i(c, r))
			var center := cell_center_world(global)
			if noise:
				height_map[cell_index(Vector2i(c, r))] = noise.get_noise_2d(center.x, center.y) * terrain_system.dimensions.y
			else:
				height_map[cell_index(Vector2i(c, r))] = 0.0


func generate_color_maps():
	var count := cells_per_chunk().x * cells_per_chunk().y
	color_map_0 = PackedColorArray()
	color_map_1 = PackedColorArray()
	color_map_0.resize(count)
	color_map_1.resize(count)
	for i in count:
		color_map_0[i] = Color(0, 0, 0, 0)
		color_map_1[i] = Color(0, 0, 0, 0)


func generate_wall_color_maps():
	var count := cells_per_chunk().x * cells_per_chunk().y
	wall_color_map_0 = PackedColorArray()
	wall_color_map_1 = PackedColorArray()
	wall_color_map_0.resize(count)
	wall_color_map_1.resize(count)
	for i in count:
		wall_color_map_0[i] = Color(1, 0, 0, 0) # Default to texture slot 0
		wall_color_map_1[i] = Color(1, 0, 0, 0)


func generate_grass_mask_map():
	var count := cells_per_chunk().x * cells_per_chunk().y
	grass_mask_map = PackedColorArray()
	grass_mask_map.resize(count)
	for i in count:
		grass_mask_map[i] = Color(1.0, 1.0, 1.0, 1.0)

#endregion


#region cell_geometry getters

func get_height(cc: Vector2i) -> float:
	return height_map[cell_index(cc)]


func get_color_0(cc: Vector2i) -> Color:
	return color_map_0[cell_index(cc)]


func get_color_1(cc: Vector2i) -> Color:
	return color_map_1[cell_index(cc)]


func get_wall_color_0(cc: Vector2i) -> Color:
	return wall_color_map_0[cell_index(cc)]


func get_wall_color_1(cc: Vector2i) -> Color:
	return wall_color_map_1[cell_index(cc)]


func get_grass_mask(cc: Vector2i) -> Color:
	return grass_mask_map[cell_index(cc)]

#endregion


#region cell_geometry setters

func draw_height(x: int, z: int, y: float):
	height_map[cell_index(Vector2i(x, z))] = y
	mark_dirty()


func draw_color_0(x: int, z: int, color: Color):
	color_map_0[cell_index(Vector2i(x, z))] = color
	mark_dirty()


func draw_color_1(x: int, z: int, color: Color):
	color_map_1[cell_index(Vector2i(x, z))] = color
	mark_dirty()


func draw_wall_color_0(x: int, z: int, color: Color):
	wall_color_map_0[cell_index(Vector2i(x, z))] = color
	mark_dirty()


func draw_wall_color_1(x: int, z: int, color: Color):
	wall_color_map_1[cell_index(Vector2i(x, z))] = color
	mark_dirty()


func draw_grass_mask(x: int, z: int, masked: Color):
	grass_mask_map[cell_index(Vector2i(x, z))] = masked
	mark_dirty()

#endregion


func mark_dirty() -> void:
	_data_dirty = true


#region mesh generation

func regenerate_mesh(_use_threads: bool = false):
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	st.set_custom_format(0, SurfaceTool.CUSTOM_RGBA_FLOAT)
	st.set_custom_format(1, SurfaceTool.CUSTOM_RGBA_FLOAT)
	st.set_custom_format(2, SurfaceTool.CUSTOM_RGBA_FLOAT)
	
	global_position_cached = global_position if is_inside_tree() else position
	
	var cells := cells_per_chunk()
	var origin := get_chunk_origin_world()
	for r in range(cells.y):
		for c in range(cells.x):
			_emit_cell(st, Vector2i(c, r), origin)
	
	st.generate_normals()
	st.index()
	mesh = st.commit()
	
	if mesh and terrain_system:
		mesh.surface_set_material(0, terrain_system.terrain_material)
	
	for child in get_children():
		if child is StaticBody3D:
			child.free()
	create_trimesh_collision()
	_setup_collision_layers()


func _emit_cell(st: SurfaceTool, local: Vector2i, origin: Vector2) -> void:
	var global := _chunk_global_cell(local)
	var idx := cell_index(local)
	var h : float = height_map[idx]
	var center := cell_center_world(global) - origin
	var offsets := cell_corner_offsets(global)
	var corner_count := offsets.size()
	
	# Top fan around the centroid as explicit triangles (center, i, i+1)
	var center_3d := Vector3(center.x, h, center.y)
	for i in range(corner_count):
		var p0 := center + offsets[i]
		var p1 := center + offsets[(i + 1) % corner_count]
		_emit_top_vertex(st, center_3d, idx)
		_emit_top_vertex(st, Vector3(p0.x, h, p0.y), idx)
		_emit_top_vertex(st, Vector3(p1.x, h, p1.y), idx)
	
	# Walls on edges whose neighbor is lower.
	var neighbors := edge_neighbors(global)
	for edge in range(corner_count):
		var nb : Vector2i = neighbors[edge]
		var h_nb = terrain_system.get_cell_height(nb) if terrain_system else null
		if h_nb == null:
			continue
		if h - float(h_nb) <= 0.0001:
			continue
		var pair := edge_corner_indices(global, edge)
		var qa := center + offsets[pair.x]
		var qb := center + offsets[pair.y]
		_emit_wall(st, idx, qa, qb, h, float(h_nb), origin)


func _emit_top_vertex(st: SurfaceTool, vert: Vector3, idx: int) -> void:
	st.set_smooth_group(0)
	st.set_uv(Vector2(0, 0))
	var uv2 : Vector2 = Vector2(vert.x, vert.z) / terrain_system.cell_size
	st.set_uv2(uv2)
	st.set_color(color_map_0[idx])
	st.set_custom(0, color_map_1[idx])
	st.set_custom(1, _custom1_for(idx, false))
	st.set_custom(2, _custom2_for(idx, false))
	st.add_vertex(vert)


func _emit_wall(st: SurfaceTool, idx: int, qa: Vector2, qb: Vector2, h_own: float, h_nb: float, origin: Vector2) -> void:
	var a_low := Vector3(qa.x, h_nb, qa.y)
	var b_low := Vector3(qb.x, h_nb, qb.y)
	var a_high := Vector3(qa.x, h_own, qa.y)
	var b_high := Vector3(qb.x, h_own, qb.y)
	
	# Quad (a_low, a_high, b_high, b_low) as two triangles. Winding faces away
	# from the higher cell.
	_emit_wall_vertex(st, idx, a_low, origin)
	_emit_wall_vertex(st, idx, a_high, origin)
	_emit_wall_vertex(st, idx, b_high, origin)
	_emit_wall_vertex(st, idx, a_low, origin)
	_emit_wall_vertex(st, idx, b_high, origin)
	_emit_wall_vertex(st, idx, b_low, origin)


func _emit_wall_vertex(st: SurfaceTool, idx: int, vert: Vector3, origin: Vector2) -> void:
	st.set_smooth_group(-1)
	st.set_uv(Vector2(1, 1))
	var gp := Vector2(vert.x + origin.x, vert.z + origin.y)
	st.set_uv2(Vector2(gp.x + gp.y, gp.x + gp.y))
	st.set_color(wall_color_map_0[idx])
	st.set_custom(0, wall_color_map_1[idx])
	st.set_custom(1, _custom1_for(idx, true))
	st.set_custom(2, _custom2_for(idx, true))
	st.add_vertex(vert)


## CUSTOM1 = Color(grass_mask.r, 0, 0, rl_idx / 15) where rl_idx is the wall
## texture index of the cell (mirrors the helper's non-ridge floor output).
func _custom1_for(idx: int, wall: bool) -> Color:
	var map := wall_color_map_0 if wall else color_map_0
	var map_1 := wall_color_map_1 if wall else color_map_1
	var rl_idx := _texture_index_from_colors(map[idx], map_1[idx])
	var c := grass_mask_map[idx]
	return Color(c.r, 0.0, 0.0, float(rl_idx) / 15.0)


## CUSTOM2 = Color((m + m*16)/255, m/15, 1, 2) with m = floor texture index.
## A = 2.0 signals the shader's vertex-color path, matching the square helper.
func _custom2_for(idx: int, wall: bool) -> Color:
	var map := wall_color_map_0 if wall else color_map_0
	var map_1 := wall_color_map_1 if wall else color_map_1
	var m := _texture_index_from_colors(map[idx], map_1[idx])
	return Color((float(m) + float(m) * 16.0) / 255.0, float(m) / 15.0, 1.0, 2.0)


static func _texture_index_from_colors(c0: Color, c1: Color) -> int:
	var c0_idx := 0
	var c0_max := c0.r
	if c0.g > c0_max: c0_max = c0.g; c0_idx = 1
	if c0.b > c0_max: c0_max = c0.b; c0_idx = 2
	if c0.a > c0_max: c0_idx = 3
	
	var c1_idx := 0
	var c1_max := c1.r
	if c1.g > c1_max: c1_max = c1.g; c1_idx = 1
	if c1.b > c1_max: c1_max = c1.b; c1_idx = 2
	if c1.a > c1_max: c1_idx = 3
	
	return c0_idx * 4 + c1_idx

#endregion


func regenerate_all_cells(_use_threads: bool = false) -> void:
	regenerate_mesh(false)
