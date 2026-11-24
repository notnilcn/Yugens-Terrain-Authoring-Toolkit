//! The one place that names the terrain editor's scenes by path, so moving a scene stays a
//! change to one line.
use godot::classes::PackedScene;
use godot::prelude::*;
pub(super) const FOLIAGE_PANEL: &str =
    "res://addons/MarchingSquaresTerrain/editor/foliage_panel.tscn";
pub(super) const PIXEL_PREVIEW_CONTROLS: &str =
    "res://addons/MarchingSquaresTerrain/editor/pixel_preview_controls.tscn";
pub(super) const PIXEL_WORLD_PANEL: &str =
    "res://addons/MarchingSquaresTerrain/editor/pixel_world_panel.tscn";
pub(super) const SAVE_PRESET_DIALOG: &str =
    "res://addons/MarchingSquaresTerrain/editor/save_preset_dialog.tscn";
pub(super) fn instantiate<T: Inherits<Node>>(path: &str) -> Option<Gd<T>> {
    let Ok(scene) = try_load::<PackedScene>(path) else {
        godot_error!("Marching Squares Terrain: could not load the editor scene {path}");
        return None;
    };
    let node = scene.instantiate()?;
    match node.try_cast::<T>() {
        Ok(n) => Some(n),
        Err(node) => {
            godot_error!("Marching Squares Terrain: {path} has an unexpected root node type");
            node.free();
            None
        }
    }
}
