# MIT License
#
# Copyright (c) 2025-2026 Exonfang (godot-dualgrid-unlimited-adjacent-terrains)
#
# Ported into YugenTileMapLayer3D by the Yugen port. See
# addons/MarchingSquaresTerrain/documentation/credits.md for attribution.

@tool
extends Resource
class_name YugenLayerOrderOverrideRule

## A description of a layering rule that handles art-layering edge cases in the dual-grid
## display. Derived from LayerOrderOverrideRule in
## godot-dualgrid-unlimited-adjacent-terrains.
##
## The mask selects which display quadrants participate in the rule; the rule only matches
## when every masked quadrant holds the same terrain. [member order] then assigns the paint
## order of the four display layers.

## A mask of display quadrants (TL, TR, BL, BR) whose terrains must match for this rule.
@export_flags("TL", "TR", "BL", "BR") var mask: int = 0:
	set(value):
		mask = value
		emit_changed()

## The layer order to use for the four quadrants when this rule matches.
@export var order: Vector4i = Vector4i(0, 1, 2, 3):
	set(value):
		order = value
		emit_changed()
