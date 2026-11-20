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
