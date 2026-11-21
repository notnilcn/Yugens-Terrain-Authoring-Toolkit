//! Instantiates the builder's fixed editor panels, dialogs and menus from the scenes that
//! declare them.
//!
//! The code only decides when each one first exists: the first time the person editing
//! asks for it. What each one contains lives in its scene. An editor plugin has no scene
//! of its own that could hold references to them, so they are reached by path, and a
//! missing or broken scene is reported instead of being fatal.
use godot::classes::ResourceLoader;
use godot::prelude::*;
const DIR: &str = "res://addons/MarchingSquaresTerrain/editor/builder/";
pub(super) const ADJUST_PANEL: &str = "adjust_panel.tscn";
pub(super) const BUILDER_MENU: &str = "builder_menu.tscn";
pub(super) const CONVERT_DIALOG: &str = "convert_dialog.tscn";
pub(super) const EXPORT_OPTIONS_DIALOG: &str = "export_options_dialog.tscn";
pub(super) const UV_EDITOR: &str = "uv_editor.tscn";
pub(super) fn instantiate<T: Inherits<Node>>(file: &str) -> Option<Gd<T>> {
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
pub(in crate::builder) fn unique<T: Inherits<Node>>(
    root: &Gd<impl Inherits<Node>>,
    name: &str,
) -> Option<Gd<T>> {
    let found = root
        .upcast_ref::<Node>()
        .try_get_node_as::<T>(&format!("%{name}"));
    if found.is_none() {
        godot_error!("Pixel Builder: editor scene is missing its {name} node");
    }
    found
}
