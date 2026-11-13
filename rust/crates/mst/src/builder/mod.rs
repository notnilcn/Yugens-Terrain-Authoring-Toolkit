//! Level builder: editable polygon meshes (`PixelBuilderMesh` + `PixelBuilderMeshData`) and the
//! `PixelBuilderPlugin` editor plugin (object / face / edge / vertex editing, shape creation,
//! terrain-cell snapping). Geometry logic lives in `pt_core::build`.
mod blend;
mod data;
mod node;
mod plugin;
mod prefs;
mod spec;
mod ui;
mod uv_editor;

