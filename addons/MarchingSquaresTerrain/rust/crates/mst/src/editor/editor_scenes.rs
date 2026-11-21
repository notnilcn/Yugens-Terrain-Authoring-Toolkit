//! The one place that names the terrain editor's scenes by path.
//!
//! Editor plugins are created by the engine rather than placed in a scene, so nothing can
//! hand them a scene by reference; the paths they need live here so that moving a scene
//! means changing a single line.
use godot::classes::PackedScene;
use godot::prelude::*;
pub(super) const FOLIAGE_PANEL: &str =
    "res://addons/MarchingSquaresTerrain/editor/foliage_panel.tscn";
pub(super) const PIXEL_PREVIEW_CONTROLS: &str =
    "res://addons/MarchingSquaresTerrain/editor/pixel_preview_controls.tscn";
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
