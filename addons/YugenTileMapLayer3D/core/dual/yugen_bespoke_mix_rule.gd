# MIT License
#
# Copyright (c) 2025-2026 Exonfang (godot-dualgrid-unlimited-adjacent-terrains)
# Copyright (c) 2024 Pablo Gila-Herranz (TileMapDual)
#
# Ported into YugenTileMapLayer3D by the Yugen port. See
# addons/MarchingSquaresTerrain/documentation/credits.md for attribution.

@tool
extends Resource
class_name YugenBespokeMixRule

## Defines a relationship between two terrain ids with an atlas offset used for a bespoke
## tile mix. Derived from BespokeMixRule in godot-dualgrid-unlimited-adjacent-terrains.

## The primary terrain id for this rule.
@export_range(0, 256, 1, "or_greater") var primary_source_id: int = 0
## The secondary terrain id for this rule.
@export_range(0, 256, 1, "or_greater") var secondary_source_id: int = 0
## The atlas offset for this mix, in sequential order within the X axis of the primary
## terrain's atlas block.
@export_range(8, 256, 4, "or_greater") var atlas_offset: int = 8
