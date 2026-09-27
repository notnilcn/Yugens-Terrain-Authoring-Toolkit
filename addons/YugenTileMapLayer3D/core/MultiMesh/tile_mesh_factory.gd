@tool
class_name TileMeshFactory
extends RefCounted


static var _cache: Dictionary = {}


static func get_mesh(
		mesh_mode: YugenGlobalConstants.MeshMode,
		grid_size: float,
		texture_repeat: int = YugenGlobalConstants.TextureRepeatMode.DEFAULT,
		arc_radius: float = YugenGlobalConstants.ARCH_DEFAULT_RADIUS_RATIO
) -> ArrayMesh:
	var key: String = _cache_key(mesh_mode, grid_size, texture_repeat, arc_radius)
	var cached: ArrayMesh = _cache.get(key, null)
	if cached != null:
		return cached

	var mesh: ArrayMesh = _build_mesh(mesh_mode, grid_size, texture_repeat, arc_radius)
	_cache[key] = mesh
	return mesh


static func uses_colors(mesh_mode: YugenGlobalConstants.MeshMode) -> bool:
	match mesh_mode:
		YugenGlobalConstants.MeshMode.FLAT_TRIANGULE, \
		YugenGlobalConstants.MeshMode.BOX_MESH, \
		YugenGlobalConstants.MeshMode.PRISM_MESH:
			return false
		_:
			return true


static func is_arch_mode(mesh_mode: YugenGlobalConstants.MeshMode) -> bool:
	match mesh_mode:
		YugenGlobalConstants.MeshMode.FLAT_ARCH, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_I, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_I, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP_I, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP_DUO, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_C, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_C_I, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_S, \
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_S_I:
			return true
		_:
			return false


## Drop the entire cache (e.g. grid_size change). Strong refs released; chunks must be
## re-pointed at fresh meshes by the caller (rebuild) before the old ones go away.
static func invalidate() -> void:
	_cache.clear()


static func invalidate_arch() -> void:
	for key: String in _cache.keys():
		if key.begins_with("arch:"):
			_cache.erase(key)



static func _cache_key(
		mesh_mode: YugenGlobalConstants.MeshMode,
		grid_size: float,
		texture_repeat: int,
		arc_radius: float
) -> String:
	if is_arch_mode(mesh_mode):
		return "arch:%d:%f:%f" % [int(mesh_mode), grid_size, arc_radius]
	return "%d:%f:%d" % [int(mesh_mode), grid_size, texture_repeat]


static func _build_mesh(
		mesh_mode: YugenGlobalConstants.MeshMode,
		grid_size: float,
		texture_repeat: int,
		arc_radius: float
) -> ArrayMesh:
	var norm_rect: Rect2 = Rect2(0, 0, 1, 1)
	var norm_size: Vector2 = Vector2(1, 1)
	var world_size: Vector2 = Vector2(grid_size, grid_size)

	match mesh_mode:
		YugenGlobalConstants.MeshMode.FLAT_SQUARE:
			return TileMeshGenerator.create_tile_quad(norm_rect, norm_size, world_size)
		YugenGlobalConstants.MeshMode.FLAT_TRIANGULE:
			return TileMeshGenerator.create_tile_triangle(norm_rect, norm_size, world_size)
		YugenGlobalConstants.MeshMode.BOX_MESH:
			if texture_repeat == YugenGlobalConstants.TextureRepeatMode.REPEAT:
				return TileMeshGenerator.create_box_mesh_repeat(grid_size)
			return TileMeshGenerator.create_box_mesh(grid_size)
		YugenGlobalConstants.MeshMode.PRISM_MESH:
			if texture_repeat == YugenGlobalConstants.TextureRepeatMode.REPEAT:
				return TileMeshGenerator.create_prism_mesh_repeat(grid_size)
			return TileMeshGenerator.create_prism_mesh(grid_size)
		YugenGlobalConstants.MeshMode.FLAT_ARCH:
			return TileMeshGenerator.create_arch_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_I:
			return TileMeshGenerator.create_arch_i_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER:
			return TileMeshGenerator.create_arch_corner_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_I:
			return TileMeshGenerator.create_arch_corner_i_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP:
			return TileMeshGenerator.create_arch_corner_cap_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP_I:
			return TileMeshGenerator.create_arch_corner_cap_i_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_CAP_DUO:
			return TileMeshGenerator.create_arch_corner_cap_duo_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_C:
			return TileMeshGenerator.create_arch_corner_c_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_C_I:
			return TileMeshGenerator.create_arch_corner_c_i_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_S:
			return TileMeshGenerator.create_arch_corner_s_mesh(norm_rect, norm_size, world_size, arc_radius)
		YugenGlobalConstants.MeshMode.FLAT_ARCH_CORNER_S_I:
			return TileMeshGenerator.create_arch_corner_s_i_mesh(norm_rect, norm_size, world_size, arc_radius)

	push_error("TileMeshFactory: unknown mesh_mode %d, falling back to quad" % int(mesh_mode))
	return TileMeshGenerator.create_tile_quad(norm_rect, norm_size, world_size)
