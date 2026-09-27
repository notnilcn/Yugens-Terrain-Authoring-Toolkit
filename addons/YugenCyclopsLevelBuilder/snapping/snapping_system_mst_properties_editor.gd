# MIT License
#
# Copyright (c) 2023 Mark McKay
# https://github.com/blackears/cyclopsLevelBuilder
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.
#
# Yugen port addition: properties editor for the MST Grid Align snapping system.

@tool
extends PanelContainer
class_name YugenSnappingSystemMSTPropertiesEditor

var tool:YugenSnappingSystemMST:
	get:
		return tool
	set(value):
		if value == tool:
			return
		tool = value
		update_ui_from_props()


func get_global_scene()->YugenCyclopsGlobalScene:
	var root:Window = Engine.get_main_loop().root
	if !root:
		return null
	return root.get_node_or_null("/root/YugenCyclopsAutoload")


func update_ui_from_props():
	if !tool:
		return

	%option_align_kind.select(int(tool.align_kind))
	%spin_cell_multiplier.value = tool.cell_multiplier
	%check_y_snap.button_pressed = tool.y_snap
	%spin_y_distance.value = tool.y_snap_distance
	%spin_y_distance.editable = tool.y_snap

	update_terrain_status()


func update_terrain_status():
	if !tool:
		return

	if tool.mst_terrain_path.is_empty() && !is_instance_valid(tool.mst_terrain):
		%lbl_terrain.text = "Terrain: none"
		%lbl_status.text = "Assign a MarchingSquaresTerrain to enable Grid Align."
		return

	var terrain:Node3D = tool.resolve_terrain()
	if !tool.mst_terrain_path.is_empty():
		%lbl_terrain.text = "Terrain: %s" % tool.mst_terrain_path
	else:
		%lbl_terrain.text = "Terrain: assigned"

	if !tool.get_mst_grid_snap_script():
		%lbl_status.text = "The MarchingSquaresTerrain addon is not installed; Grid Align is unavailable."
	elif !terrain:
		%lbl_status.text = "Terrain not found in the edited scene."
	elif !tool.is_terrain_supported(terrain):
		%lbl_status.text = "Unsupported terrain type. Grid Align currently supports square terrains only."
	else:
		%lbl_status.text = "Grid Align ready."


func persist_settings():
	if !tool:
		return

	tool.flush_cache()

	var global_scene:YugenCyclopsGlobalScene = get_global_scene()
	if !global_scene:
		return

	global_scene.settings.set_property(YugenCyclopsGlobalScene.SNAPPING_MST_TERRAIN_PATH, tool.mst_terrain_path)
	global_scene.settings.set_property(YugenCyclopsGlobalScene.SNAPPING_MST_ALIGN_KIND, int(tool.align_kind))
	global_scene.settings.set_property(YugenCyclopsGlobalScene.SNAPPING_MST_CELL_MULTIPLIER, tool.cell_multiplier)
	global_scene.settings.set_property(YugenCyclopsGlobalScene.SNAPPING_MST_SNAP_Y, tool.y_snap)
	global_scene.settings.set_property(YugenCyclopsGlobalScene.SNAPPING_MST_Y_DISTANCE, tool.y_snap_distance)
	global_scene.save_settings()


func _on_bn_use_selection_pressed():
	if !tool:
		return

	var terrain:Node3D = null
	for node in EditorInterface.get_selection().get_selected_nodes():
		if node is Node3D && tool.is_terrain_supported(node):
			terrain = node
			break

	if terrain:
		tool.set_terrain(terrain)
		persist_settings()
		update_terrain_status()


func _on_bn_clear_pressed():
	if !tool:
		return

	tool.clear_terrain()
	persist_settings()
	update_terrain_status()


func _on_option_align_kind_item_selected(index:int):
	if !tool:
		return

	tool.align_kind = tool._coerce_align_kind(index)
	persist_settings()


func _on_spin_cell_multiplier_value_changed(value:float):
	if !tool:
		return

	tool.cell_multiplier = maxi(int(value), 1)
	persist_settings()


func _on_check_y_snap_toggled(toggled_on:bool):
	if !tool:
		return

	tool.y_snap = toggled_on
	%spin_y_distance.editable = toggled_on
	persist_settings()


func _on_spin_y_distance_value_changed(value:float):
	if !tool:
		return

	tool.y_snap_distance = maxf(value, 0.0)
	persist_settings()
