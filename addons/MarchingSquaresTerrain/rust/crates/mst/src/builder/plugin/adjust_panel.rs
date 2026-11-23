//! The root of the floating panel that tunes the last builder operation, such as an extrude
//! or a bevel, while its result is still being previewed.
//!
//! Intent: the panel's heading, its accept and cancel buttons, the area its value fields go
//! in, its status line and the pause before a change is applied belong to their scene, so
//! an author can restyle or rearrange them there. This node is how the builder is told
//! which declared part plays which role.
//!
//! Responsibility: it holds those parts as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about which fields appear or what
//! they change; the builder fills the field area for each operation and answers the
//! buttons.
//!
//! Why a node: the panel floats over the 3D viewport, and its root is the node the builder
//! adds there and removes again.
//!
//! Why a new type: the builder plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in panel
//! has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{Button, HFlowContainer, IPanelContainer, Label, PanelContainer, Timer};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = PanelContainer)]
pub struct PixelBuilderAdjustPanel {
    #[export]
    pub(super) heading: Option<Gd<Label>>,
    #[export]
    pub(super) cancel: Option<Gd<Button>>,
    #[export]
    pub(super) done: Option<Gd<Button>>,
    #[export]
    pub(super) fields: Option<Gd<HFlowContainer>>,
    #[export]
    pub(super) status: Option<Gd<Label>>,
    #[export]
    pub(super) debounce_timer: Option<Gd<Timer>>,
    base: Base<PanelContainer>,
}
#[godot_api]
impl IPanelContainer for PixelBuilderAdjustPanel {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/adjust_panel.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelBuilderAdjustPanel\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
