//! The root of the dialog that asks how an ordinary mesh should be turned into a builder
//! mesh.
//!
//! Intent: the wording, order and look of the conversion choices belong to their scene, so
//! an author can reword or restyle them there. This node is how the builder is told which
//! declared control stands for which choice.
//!
//! Responsibility: it holds those controls as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about what a choice means or
//! remembers any choice; the builder keeps the choices and performs the conversion.
//!
//! Why a node: the dialog is a window in the editor's tree, and its root is the window the
//! builder opens.
//!
//! Why a new type: the builder plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in dialog
//! has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{CheckBox, ConfirmationDialog, IConfirmationDialog, OptionButton, SpinBox};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = ConfirmationDialog)]
pub struct PixelBuilderConvertDialog {
    #[export]
    pub(super) merge: Option<Gd<OptionButton>>,
    #[export]
    pub(super) keep_materials: Option<Gd<CheckBox>>,
    #[export]
    pub(super) keep_smoothing: Option<Gd<CheckBox>>,
    #[export]
    pub(super) smoothing_angle: Option<Gd<SpinBox>>,
    base: Base<ConfirmationDialog>,
}
#[godot_api]
impl IConfirmationDialog for PixelBuilderConvertDialog {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/convert_dialog.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelBuilderConvertDialog\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
