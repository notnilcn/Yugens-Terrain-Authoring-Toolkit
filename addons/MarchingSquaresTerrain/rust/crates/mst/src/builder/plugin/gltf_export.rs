//! Writes a builder mesh to a glTF file.
//!
//! The engine's glTF writer only reads from a node tree, so a throwaway tree holding the
//! mesh is built for the length of one export and freed as soon as the file is written.
//! It never enters the scene being edited.
use godot::classes::{GltfDocument, GltfState, Mesh, MeshInstance3D, Node3D};
use godot::global::Error;
use godot::prelude::*;
pub(super) fn write_gltf(name: &StringName, mesh: &Gd<Mesh>, path: &str) -> Error {
    let mut root = Node3D::new_alloc();
    root.set_name(name);
    let mut mi = MeshInstance3D::new_alloc();
    mi.set_name(name);
    mi.set_mesh(mesh);
    root.add_child(&mi);
    mi.set_owner(&root);
    let mut doc = GltfDocument::new_gd();
    let state = GltfState::new_gd();
    let err = doc.append_from_scene(&root, &state);
    let res = if err == Error::OK {
        doc.write_to_filesystem(&state, path)
    } else {
        err
    };
    root.free();
    res
}
