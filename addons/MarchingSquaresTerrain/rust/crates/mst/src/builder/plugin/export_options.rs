//! The root of the dialog where someone chooses how the builder exports a mesh to a file.
//!
//! Intent: the wording, order and look of the export choices belong to their scene, so an
//! author can reword or restyle them there. This node is how the builder is told which
//! declared check box stands for which choice.
//!
//! Responsibility: it holds those check boxes as the scene author assigned them and warns
//! the author about any left unassigned. It decides nothing about what a choice means or
//! remembers any choice; the builder keeps the choices and applies them when it exports.
//!
//! Why a node: the dialog is a window in the editor's tree, and its root is the window the
//! builder opens.
//!
//! Why a new type: the builder plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in dialog
//! has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{AcceptDialog, CheckBox, IAcceptDialog};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = AcceptDialog)]
pub struct PixelBuilderExportOptions {
    #[export]
    pub(super) selected_only: Option<Gd<CheckBox>>,
    #[export]
    pub(super) apply_transforms: Option<Gd<CheckBox>>,
    #[export]
    pub(super) vertex_colours: Option<Gd<CheckBox>>,
    #[export]
    pub(super) right_handed: Option<Gd<CheckBox>>,
    #[export]
    pub(super) combine: Option<Gd<CheckBox>>,
    #[export]
    pub(super) stl_binary: Option<Gd<CheckBox>>,
    base: Base<AcceptDialog>,
}
#[godot_api]
impl IAcceptDialog for PixelBuilderExportOptions {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/export_options_dialog.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelBuilderExportOptions\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
