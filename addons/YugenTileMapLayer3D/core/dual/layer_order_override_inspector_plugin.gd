# MIT License
#
# Copyright (c) 2025-2026 Exonfang (godot-dualgrid-unlimited-adjacent-terrains)
#
# Ported into YugenTileMapLayer3D by the Yugen port.

@tool
extends EditorInspectorPlugin
class_name YugenLayerOrderOverrideInspectorPlugin

## Shows the visual quadrant editor for [YugenLayerOrderOverrideRule].


func _can_handle(object: Object) -> bool:
	return object is YugenLayerOrderOverrideRule


func _parse_property(object: Object, type: Variant.Type, name: String, hint_type: PropertyHint,
		hint_string: String, usage_flags: int, wide: bool) -> bool:
	if name == "mask" and type == TYPE_INT:
		add_property_editor(name, YugenEditorPropertyLayerOrderOverride.new(), false, "Layer Order")
		return true

	if name == "order" and type == TYPE_VECTOR4I:
		return true

	return false
