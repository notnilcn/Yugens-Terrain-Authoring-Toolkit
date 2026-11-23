mod camera_framing;
mod chunk_gizmo;
mod cursor;
mod drag_select;
mod editor_scenes;
mod extra_tools;
mod fire_tool;
mod foliage;
mod foliage_panel;
mod gizmo_lock;
mod glb_export_dialogs;
mod inspector_labels;
mod line_gizmo;
mod mst_flow;
mod mst_value_label;
mod overlay_ui;
mod pixel_ui;
mod plugin;
mod preview_controls;
mod preview_plugin;
mod save_preset_dialog;
pub(crate) mod scene_refs;
pub(crate) mod terrain_ui;
mod tools;
pub(crate) mod view_commands;
pub mod ui_kit;
pub fn pixel_handles(entity: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    pixel_ui::handles(entity)
}
