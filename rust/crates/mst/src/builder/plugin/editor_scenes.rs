//! Instantiates the builder's fixed editor panels, dialogs and menus from the scenes that
//! declare them.
//!
//! They are instantiated at run time because each is a piece of the editor's own interface
//! that first exists when the person editing asks for it, never part of a scene they edit.
//! What each one contains lives in its scene, so a panel can be reshaped without touching
//! the code that opens it.
use godot::classes::ResourceLoader;
use godot::prelude::*;
const DIR: &str = "res://addons/MarchingSquaresTerrain/editor/builder/";
pub(super) const ADJUST_PANEL: &str = "adjust_panel.tscn";
pub(super) const BUILDER_MENU: &str = "builder_menu.tscn";
pub(super) const CONVERT_DIALOG: &str = "convert_dialog.tscn";
pub(super) const EXPORT_OPTIONS_DIALOG: &str = "export_options_dialog.tscn";
pub(super) const UV_EDITOR: &str = "uv_editor.tscn";
pub(in crate::builder) const ROBLOX_RIBBON: &str = "roblox_ribbon.tscn";
pub(in crate::builder) const ROBLOX_GRID_PLANE: &str = "roblox_grid_plane_button.tscn";
pub(in crate::builder) fn instantiate<T: Inherits<Node>>(file: &str) -> Option<Gd<T>> {
    let path = format!("{DIR}{file}");
    let Some(scene) = ResourceLoader::singleton()
        .load(&path)
        .and_then(|r| r.try_cast::<PackedScene>().ok())
    else {
        godot_error!("Pixel Builder: could not load {path}");
        return None;
    };
    let Some(root) = scene.instantiate() else {
        godot_error!("Pixel Builder: could not instantiate {path}");
        return None;
    };
    match root.try_cast::<T>() {
        Ok(t) => Some(t),
        Err(root) => {
            godot_error!("Pixel Builder: {path} has an unexpected root type");
            root.free();
            None
        }
    }
}
