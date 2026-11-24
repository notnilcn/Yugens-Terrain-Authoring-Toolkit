//! The root of the pixel preview controls that sit in the 3D editor's menu bar.
//!
//! Intent: the look and order of the preview switch, the animation switch, the pixel world
//! menu, the layout switch and the project features menu belong to their scene, so an
//! author can restyle or rearrange them there. This node is how the preview plugin is told
//! which declared control plays which part.
//!
//! Responsibility: it holds those controls as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about what a control does or what
//! it shows; the preview plugin owns that.
//!
//! Why a node: the controls are nodes in the editor's menu bar, and their root is the node
//! the plugin adds there and removes again.
//!
//! Why a new type: the preview plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in
//! container has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{Button, HBoxContainer, IHBoxContainer, MenuButton, PopupMenu, Timer};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = HBoxContainer)]
pub struct PixelPreviewControls {
    #[export]
    pub(super) preview_button: Option<Gd<Button>>,
    #[export]
    pub(super) animate_button: Option<Gd<Button>>,
    #[export]
    pub(super) world_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) roblox_button: Option<Gd<Button>>,
    #[export]
    pub(super) features_menu: Option<Gd<PopupMenu>>,
    #[export]
    pub(super) poll_timer: Option<Gd<Timer>>,
    base: Base<HBoxContainer>,
}
#[godot_api]
impl IHBoxContainer for PixelPreviewControls {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../editor/pixel_preview_controls.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelPreviewControls\""));
        assert_eq!(super::super::scene_refs::unresolved_root_refs(SCENE), Vec::<String>::new());
    }
}
