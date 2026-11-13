//! Editor plugin: terrain authoring tools.
mod chunk_gizmo;
mod cursor;
mod overlay_ui;
mod extra_tools;
mod fire_tool;
mod gizmo_lock;
mod inspector_labels;
mod plugin;
mod pixel_ui;
mod preview_plugin;
mod terrain_ui;
mod tools;
pub mod ui_kit;

/// True for nodes the Pixel World editor plugin (`pixel_ui`) edits.
pub fn pixel_handles(object: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    pixel_ui::handles(object)
}
