//! The root of the foliage panel the terrain editor shows beside the viewport.
//!
//! Intent: the panel's hint, its palette of things to paint and its buttons for editing,
//! removing and adding props belong to their scene, so an author can reshape or restyle
//! them there. This node is how the terrain editor is told which declared control plays
//! which part.
//!
//! Responsibility: it holds those controls as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about what the palette lists or what
//! a button does; the terrain editor fills and answers them for whichever terrain is
//! selected.
//!
//! Why a node: the panel is a piece of the editor's interface, and its root is the node the
//! terrain editor adds to its side panel and throws away when the panel is rebuilt.
//!
//! Why a new type: the terrain editor plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in
//! container has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{Button, IVBoxContainer, ItemList, VBoxContainer};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer)]
pub struct MstFoliagePanel {
    #[export]
    pub(super) palette: Option<Gd<ItemList>>,
    #[export]
    pub(super) edit_button: Option<Gd<Button>>,
    #[export]
    pub(super) remove_button: Option<Gd<Button>>,
    #[export]
    pub(super) add_file_button: Option<Gd<Button>>,
    base: Base<VBoxContainer>,
}
#[godot_api]
impl IVBoxContainer for MstFoliagePanel {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../editor/foliage_panel.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"MstFoliagePanel\""));
        assert_eq!(super::super::scene_refs::unresolved_root_refs(SCENE), Vec::<String>::new());
    }
}
