//! Creates the file pickers the builder opens when the person editing asks to export,
//! save or load something, and gives every builder dialog its place in the editor's own
//! window.
//!
//! The pickers are made in code because they exist only once someone asks for one, and
//! the same picker is then reused with different filters and titles for each request.
//! They belong to the editor window rather than to any scene, so they never end up in the
//! scene being edited.
use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::{EditorFileDialog, EditorInterface};
use godot::prelude::*;
pub(super) fn add_to_editor(n: &Gd<impl Inherits<Node>>) {
    if let Some(mut base) = EditorInterface::singleton().get_base_control() {
        base.add_child(n);
    }
}
pub(super) fn file_dialog(
    name: Option<&str>,
    mode: Option<FileMode>,
    access: Access,
    on_selected: &Callable,
) -> Gd<EditorFileDialog> {
    let mut d = EditorFileDialog::new_alloc();
    if let Some(name) = name {
        d.set_name(name);
    }
    if let Some(mode) = mode {
        d.set_file_mode(mode);
    }
    d.set_access(access);
    d.connect("file_selected", on_selected);
    add_to_editor(&d);
    d
}
