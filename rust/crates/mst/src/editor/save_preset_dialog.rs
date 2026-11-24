//! The root of the dialogs that save the selected terrain's textures as a reusable preset.
//!
//! Intent: saving a preset asks the author for a name and, when a preset of that name
//! already exists, whether to replace it. Both questions belong to one scene, so an author
//! can reword or restyle them there; this node is how the terrain editor is told which
//! declared dialog and field plays which part.
//!
//! Responsibility: it holds the two dialogs and the name field as the scene author assigned
//! them and warns the author about any left unassigned. It decides nothing about what is
//! saved or where; the terrain editor answers both dialogs and frees the whole scene once
//! the author is done with it.
//!
//! Why a node: the dialogs are windows in the editor's tree, and their common root is what
//! the terrain editor adds when the author asks to save and frees when the author is done.
//!
//! Why a new type: the terrain editor plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a plain node has
//! nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{AcceptDialog, ConfirmationDialog, INode, Label, LineEdit, Node};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = Node)]
pub struct MstSavePresetDialog {
    #[export]
    pub(super) save_dialog: Option<Gd<AcceptDialog>>,
    #[export]
    pub(super) prompt: Option<Gd<Label>>,
    #[export]
    pub(super) preset_name: Option<Gd<LineEdit>>,
    #[export]
    pub(super) overwrite_dialog: Option<Gd<ConfirmationDialog>>,
    base: Base<Node>,
}
#[godot_api]
impl INode for MstSavePresetDialog {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../editor/save_preset_dialog.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"MstSavePresetDialog\""));
        assert_eq!(super::super::scene_refs::unresolved_root_refs(SCENE), Vec::<String>::new());
    }
}
