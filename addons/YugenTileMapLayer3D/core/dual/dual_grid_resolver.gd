# MIT License
#
# Copyright (c) 2025-2026 Exonfang (godot-dualgrid-unlimited-adjacent-terrains)
# Copyright (c) 2024 Pablo Gila-Herranz (TileMapDual)
#
# Ported into YugenTileMapLayer3D by the Yugen port. See
# addons/MarchingSquaresTerrain/documentation/credits.md for attribution.

@tool
extends RefCounted
class_name YugenDualGridResolver

## Engine-independent dual-grid rule resolver.
##
## Combines the peering rules of DualGrid (godot-dualgrid-unlimited-adjacent-terrains) with
## the standard atlas layout of TileMapDual v5:
## - a 4x4 terrain block per terrain, indexed by the occupancy mask of the four contributing
##   world cells (TL=1, TR=2, BL=4, BR=8);
## - generic mixed variants at +4 in X of the same block;
## - unlimited adjacent terrains by painting up to four display layers;
## - optional bespoke mixes ([YugenBespokeMixRule]) and layer-order overrides
##   ([YugenLayerOrderOverrideRule]).
##
## All methods are static and do not touch the scene tree, so the rules are headless-testable.

## Returned when a world cell holds no tile.
const EMPTY: int = -1

## World-neighbour offsets in TL, TR, BL, BR order (same order as DualGrid).
const NEIGHBORS: Array[Vector2i] = [Vector2i(0, 0), Vector2i(1, 0), Vector2i(0, 1), Vector2i(1, 1)]

## Human-readable names for the four contributing world cells, in [constant NEIGHBORS] order.
const NEIGHBOR_NAMES: Array[String] = ["TL", "TR", "BL", "BR"]

## Mixed variants are offset by +4 in X inside a terrain's atlas block / UV layout.
const MIXED_OFFSET: Vector2i = Vector2i(4, 0)

const FULL_MASK: int = 15
const FULL_ATLAS_COORDS: Vector2i = Vector2i(2, 1)
## TileMapDual's background tile in the standard square preset.
const EMPTY_LAYOUT_COORDS: Vector2i = Vector2i(0, 3)

## Peering table: occupancy bitmask [TL=1, TR=2, BL=4, BR=8] to atlas coords in the 4x4
## terrain block. Mask 0 is empty (no tile). Matches both DualGrid's TERRAIN table and the
## TileMapDual v5 standard square preset for masks 1-15.
const TERRAIN_TABLE: Dictionary = {
	0: Vector2i(-1, -1),
	1: Vector2i(3, 3),
	2: Vector2i(0, 2),
	3: Vector2i(1, 2),
	4: Vector2i(0, 0),
	5: Vector2i(3, 2),
	6: Vector2i(2, 3),
	7: Vector2i(3, 1),
	8: Vector2i(1, 3),
	9: Vector2i(0, 1),
	10: Vector2i(1, 0),
	11: Vector2i(2, 2),
	12: Vector2i(3, 0),
	13: Vector2i(2, 0),
	14: Vector2i(1, 1),
	15: Vector2i(2, 1),
}


## Terrain ids of the four world cells that contribute to [param display_coord], in
## [constant NEIGHBORS] order. [param occupancy] is a Callable(Vector2i) -> int that returns
## [constant EMPTY] for unoccupied cells.
static func gather_sources(display_coord: Vector2i, occupancy: Callable) -> Array[int]:
	var sources: Array[int] = []
	for neighbor: Vector2i in NEIGHBORS:
		sources.append(int(occupancy.call(display_coord - neighbor)))
	return sources


## Unique non-empty terrain ids in an occupancy list, in first-seen order.
static func unique_sources(sources: Array[int]) -> Array[int]:
	var unique: Array[int] = []
	for source: int in sources:
		if source != EMPTY and not unique.has(source):
			unique.append(source)
	return unique


## Occupancy mask of [param source] inside [param sources].
static func mask_for(sources: Array[int], source: int) -> int:
	var mask: int = 0
	for i in range(sources.size()):
		if sources[i] == source:
			mask |= 1 << i
	return mask


## Mask of every occupied cell, regardless of terrain.
static func occupancy_mask(sources: Array[int]) -> int:
	var mask: int = 0
	for i in range(sources.size()):
		if sources[i] != EMPTY:
			mask |= 1 << i
	return mask


## Atlas coords for an occupancy mask; adds the mixed-variant offset when requested.
static func atlas_coords_for_mask(mask: int, mixed: bool = false) -> Vector2i:
	var coords: Vector2i = TERRAIN_TABLE.get(mask, Vector2i(-1, -1))
	if mixed and mask != 0:
		coords += MIXED_OFFSET
	return coords


## Variant for one terrain layer, reproducing DualGrid's `_calculate_display_tile_for_source_id`:
## diagonal-opposite masks receive dedicated atlas variants when [param all_occupied] is set.
static func display_coords_for_source(mask: int, all_occupied: bool, tile_count: int) -> Vector2i:
	if all_occupied:
		if mask == 9:
			if tile_count == 3:
				return Vector2i(3, 3) + MIXED_OFFSET
			elif tile_count == 0:
				return Vector2i(1, 3) + MIXED_OFFSET
		elif mask == 6:
			if tile_count == 2:
				return Vector2i(0, 2) + MIXED_OFFSET
			elif tile_count == 1:
				return Vector2i(0, 0) + MIXED_OFFSET
		return atlas_coords_for_mask(mask, true)

	if mask == 9:
		if tile_count == 3:
			return Vector2i(3, 3)
		elif tile_count == 0:
			return Vector2i(1, 3)
	elif mask == 6:
		if tile_count == 2:
			return Vector2i(0, 2)
		elif tile_count == 1:
			return Vector2i(0, 0)
	return atlas_coords_for_mask(mask, false)


## Resolves all display layers for [param display_coord].
##
## [param occupancy] returns the terrain id at a world coord ([constant EMPTY] when empty).
## [param opts] keys: bespoke_rules (Array[YugenBespokeMixRule]), order_override_rules
## (Array[YugenLayerOrderOverrideRule]), reverse_order (bool).
##
## Returns a Dictionary:
## - "layers": Array of { layer: int, source_id: int, atlas_coords: Vector2i }, in paint order;
## - "source_id" / "atlas_coords": the primary (most relevant) layer. This is the tile's own
##   terrain when its own world cell is occupied, else the topmost painted layer.
static func resolve(display_coord: Vector2i, occupancy: Callable, opts: Dictionary = {}) -> Dictionary:
	var result: Dictionary = {
		"layers": [],
		"source_id": EMPTY,
		"atlas_coords": Vector2i(-1, -1),
	}
	var sources: Array[int] = gather_sources(display_coord, occupancy)
	var unique: Array[int] = unique_sources(sources)
	if unique.is_empty():
		return result

	var all_occupied: bool = not sources.has(EMPTY)

	if unique.size() == 1:
		var coords: Vector2i = atlas_coords_for_mask(occupancy_mask(sources), false)
		result["layers"] = [{"layer": 0, "source_id": unique[0], "atlas_coords": coords}]
		result["source_id"] = unique[0]
		result["atlas_coords"] = coords
		return result

	if unique.size() == 2 and all_occupied:
		var mix: Array = find_bespoke_mix(opts.get("bespoke_rules", []), unique[0], unique[1])
		if not mix.is_empty():
			var primary: int = mix[0]
			var offset: int = mix[1]
			var mix_mask: int = mask_for(sources, primary)
			var mix_coords: Vector2i = atlas_coords_for_mask(mix_mask, false) + Vector2i(offset, 0)
			result["layers"] = [{"layer": 0, "source_id": primary, "atlas_coords": mix_coords}]
			result["source_id"] = primary
			result["atlas_coords"] = mix_coords
			return result

	var paint_order: Array[int] = [3, 2, 1, 0]
	if opts.get("reverse_order", false):
		paint_order.reverse()
	var active_rule: YugenLayerOrderOverrideRule = find_layer_order_rule(
		opts.get("order_override_rules", []), sources
	)
	if active_rule:
		paint_order = [active_rule.order.x, active_rule.order.y, active_rule.order.z, active_rule.order.w]

	var layers: Array = []
	for slot in range(NEIGHBORS.size()):
		var source: int = sources[slot]
		if source == EMPTY:
			continue
		var slot_mask: int = mask_for(sources, source)
		var slot_coords: Vector2i = display_coords_for_source(slot_mask, all_occupied, slot)
		layers.append({"layer": paint_order[slot], "source_id": source, "atlas_coords": slot_coords})
	result["layers"] = layers

	if sources[0] != EMPTY:
		result["source_id"] = sources[0]
		result["atlas_coords"] = display_coords_for_source(mask_for(sources, sources[0]), all_occupied, 0)
	elif not layers.is_empty():
		var top: Dictionary = layers[layers.size() - 1]
		result["source_id"] = top["source_id"]
		result["atlas_coords"] = top["atlas_coords"]

	return result


## Convenience for single-tile storage: the variant of [param terrain] at [param display_coord].
## Returns (-1, -1) when the neighbourhood holds no tiles or the terrain is absent.
static func resolve_variant_for_terrain(display_coord: Vector2i, occupancy: Callable, terrain: int) -> Vector2i:
	var sources: Array[int] = gather_sources(display_coord, occupancy)
	if terrain == EMPTY or not sources.has(terrain):
		return Vector2i(-1, -1)
	var unique: Array[int] = unique_sources(sources)
	if unique.is_empty():
		return Vector2i(-1, -1)
	if unique.size() == 1:
		return atlas_coords_for_mask(occupancy_mask(sources), false)

	var mask: int = mask_for(sources, terrain)
	if mask == 0:
		return Vector2i(-1, -1)
	var all_occupied: bool = not sources.has(EMPTY)
	return display_coords_for_source(mask, all_occupied, 0)


## Finds a bespoke mix between two terrains. Returns [primary_source_id, atlas_offset] or [].
static func find_bespoke_mix(rules: Array, source_a: int, source_b: int) -> Array:
	for rule in rules:
		if rule == null or not rule is YugenBespokeMixRule:
			continue
		if rule.primary_source_id == source_a and rule.secondary_source_id == source_b:
			return [source_a, rule.atlas_offset]
		if rule.primary_source_id == source_b and rule.secondary_source_id == source_a:
			return [source_b, rule.atlas_offset]
	return []


## Finds the first layer-order override rule whose masked quadrants all hold the same terrain.
static func find_layer_order_rule(rules: Array,
		neighbourhood_ids: Array[int]) -> YugenLayerOrderOverrideRule:
	for rule in rules:
		if rule == null or not rule is YugenLayerOrderOverrideRule:
			continue
		var target: int = EMPTY
		var has_target: bool = false
		var matches: bool = true
		for i in range(4):
			if (rule.mask & (1 << i)) != 0:
				if not has_target:
					target = neighbourhood_ids[i]
					has_target = true
				elif neighbourhood_ids[i] != target:
					matches = false
					break
		if matches:
			return rule
	return null


## TileMapDual v5 "Standard" square atlas layout: index = occupancy mask, value = atlas
## coords in a 4x4 block. Index 0 is the background tile.
static func standard_layout() -> Array[Vector2i]:
	var layout: Array[Vector2i] = []
	for mask in range(16):
		layout.append(EMPTY_LAYOUT_COORDS if mask == 0 else TERRAIN_TABLE[mask])
	return layout


## Block origin in atlas coords when [param full_coords] is the block's full (mask 15) tile.
static func block_origin_from_full(full_coords: Vector2i) -> Vector2i:
	return full_coords - FULL_ATLAS_COORDS
