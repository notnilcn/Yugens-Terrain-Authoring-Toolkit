//! Level builder: editable polygon meshes (`PixelBuilderMesh` + `PixelBuilderMeshData`) and the
//! `PixelBuilderPlugin` editor plugin (object / face / edge / vertex editing, shape creation,
//! terrain-cell snapping). Geometry logic lives in `pt_core::build`. The mode, tool and snapping
//! vocabulary follows cyclopsLevelBuilder (MIT).
mod blend;
mod commands;
mod data;
mod forms;
mod inspector;
mod layout;
mod modeler_ui;
mod node;
mod plugin;
mod prefs;
mod shortcuts;
mod spec;
mod ui;
mod uv_editor;

mod modeler_import;

/// N15: every builder file dialog gets an honest minimum (500×300, editor-scaled) instead of
/// Godot's reported 200×70 that its content then clamps to ~498×200 with a clipped body, and an
/// initial size at which the Favorites / Recent lists, the file list and the Path / File rows all
/// fit.
pub(crate) fn fit_file_dialog(d: &mut godot::prelude::Gd<godot::classes::EditorFileDialog>) {
    use godot::prelude::Vector2i;
    let k = crate::editor::ui_kit::editor_scale();
    let px = |v: f32| (v * k).round() as i32;
    d.set_min_size(Vector2i::new(px(500.0), px(300.0)));
    let cur = d.get_size();
    d.set_size(Vector2i::new(cur.x.max(px(760.0)), cur.y.max(px(520.0))));
}
