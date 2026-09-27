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
# Yugen port addition: Grid Align snapping to a MarchingSquaresTerrain lattice.

@tool
extends CyclopsSnappingSystem
class_name YugenSnappingSystemMST

## Snaps edits to the vertex / cell-center lattice of an MST square terrain.
## The terrain is referenced by a scene-relative NodePath and resolved lazily so
## the settings survive scene reloads. MST itself is loaded dynamically: if the
## MarchingSquaresTerrain addon is not installed this system stays inert and the
## rest of Cyclops still loads.

const SNAPPING_TOOL_ID:String = "mst"

const MST_GRID_SNAP_PATH:String = "res://addons/MarchingSquaresTerrain/editor/utils/mst_grid_snap.gd"

enum AlignKind { VERTEX_AND_CENTER, VERTEX_ONLY, CENTER_ONLY }

# Scene-relative path of the assigned terrain. Serialized through the plugin
# snapping cache so it is restored when the editor session restarts.
var mst_terrain_path:NodePath = NodePath()

# Optional direct reference, used by tests and by the properties editor when it
# assigns a terrain. Not serialized.
var mst_terrain:Node3D

var align_kind:AlignKind = AlignKind.VERTEX_AND_CENTER
var cell_multiplier:int = 1
var y_snap:bool = false
var y_snap_distance:float = 1.0

var _mst_grid_snap_script:GDScript


func get_mst_grid_snap_script()->GDScript:
	if !_mst_grid_snap_script && ResourceLoader.exists(MST_GRID_SNAP_PATH):
		_mst_grid_snap_script = load(MST_GRID_SNAP_PATH)
	return _mst_grid_snap_script


func resolve_terrain()->Node3D:
	if is_instance_valid(mst_terrain):
		return mst_terrain
	if mst_terrain_path.is_empty():
		return null
	var scene_root:Node = EditorInterface.get_edited_scene_root()
	if !scene_root:
		return null
	return scene_root.get_node_or_null(mst_terrain_path) as Node3D


func set_terrain(terrain:Node3D):
	mst_terrain = terrain
	if terrain:
		var scene_root:Node = EditorInterface.get_edited_scene_root()
		if scene_root:
			mst_terrain_path = scene_root.get_path_to(terrain)
	flush_cache()


func clear_terrain():
	mst_terrain = null
	mst_terrain_path = NodePath()
	flush_cache()


func is_terrain_supported(terrain:Node3D)->bool:
	var snap_script:GDScript = get_mst_grid_snap_script()
	if !terrain || !snap_script:
		return false
	return snap_script.is_supported(terrain)


func has_usable_terrain()->bool:
	if !get_mst_grid_snap_script():
		return false
	return is_terrain_supported(resolve_terrain())


func build_snap_options()->Dictionary:
	return {
		"cell_multiplier": cell_multiplier,
		"align_kind": align_kind,
		"y_snap": y_snap,
		"y_snap_distance": y_snap_distance,
	}


func _activate(plugin:YugenCyclopsLevelBuilder):
	super._activate(plugin)

	load_from_cache(plugin.get_snapping_cache(SNAPPING_TOOL_ID))


func _deactivate():
	super._deactivate()

	flush_cache()


func load_from_cache(cache:Dictionary):
	mst_terrain = null
	mst_terrain_path = NodePath(cache.get("terrain_path", ""))
	align_kind = _coerce_align_kind(cache.get("align_kind", AlignKind.VERTEX_AND_CENTER))
	cell_multiplier = maxi(int(cache.get("cell_multiplier", 1)), 1)
	y_snap = cache.get("y_snap", false)
	y_snap_distance = maxf(float(cache.get("y_snap_distance", 1.0)), 0.0)


func flush_cache():
	if !plugin:
		return
	plugin.set_snapping_cache(SNAPPING_TOOL_ID, {
		"terrain_path": str(mst_terrain_path),
		"align_kind": int(align_kind),
		"cell_multiplier": cell_multiplier,
		"y_snap": y_snap,
		"y_snap_distance": y_snap_distance,
	})


func _coerce_align_kind(value)->AlignKind:
	match int(value):
		AlignKind.VERTEX_ONLY:
			return AlignKind.VERTEX_ONLY
		AlignKind.CENTER_ONLY:
			return AlignKind.CENTER_ONLY
		_:
			return AlignKind.VERTEX_AND_CENTER


#Point is in world space
func _snap_point(point:Vector3, query:YugenSnappingQuery)->Vector3:
	var snap_script:GDScript = get_mst_grid_snap_script()
	var terrain:Node3D = resolve_terrain()
	if !snap_script || !is_terrain_supported(terrain):
		return point

	var result:Dictionary = snap_script.snap(point, terrain, build_snap_options())
	return result["point"]


func _snap_angle(angle:float, query:YugenSnappingQuery)->float:
	var snap_degrees:float = 15.0
	if plugin:
		var global_scene:YugenCyclopsGlobalScene = plugin.get_global_scene()
		if global_scene:
			snap_degrees = global_scene.settings.get_property(YugenCyclopsGlobalScene.SNAPPING_MST_ANGLE)
	if snap_degrees <= 0.0:
		return angle
	var snap_angle:float = deg_to_rad(snap_degrees)
	return floor(angle / snap_angle) * snap_angle


func _get_properties_editor()->Control:
	var ed:YugenSnappingSystemMSTPropertiesEditor = preload("res://addons/YugenCyclopsLevelBuilder/snapping/snapping_system_mst_properties_editor.tscn").instantiate()
	ed.tool = self

	return ed
