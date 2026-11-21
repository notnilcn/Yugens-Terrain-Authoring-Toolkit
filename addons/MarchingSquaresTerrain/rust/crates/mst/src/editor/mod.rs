mod camera_framing;
mod chunk_gizmo;
mod cursor;
mod drag_select;
mod editor_scenes;
mod extra_tools;
mod fire_tool;
mod foliage;
mod gizmo_lock;
mod inspector_labels;
mod line_gizmo;
mod mst_flow;
mod mst_value_label;
mod overlay_ui;
mod pixel_ui;
mod plugin;
mod preview_plugin;
mod terrain_ui;
mod tools;
pub mod ui_kit;
pub fn pixel_handles(entity: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    pixel_ui::handles(entity)
}
