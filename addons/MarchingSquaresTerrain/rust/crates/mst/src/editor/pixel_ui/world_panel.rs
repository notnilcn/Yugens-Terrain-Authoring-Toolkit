//! The root of the pixel workspace's world panel beside the 3D viewport.
//!
//! Intent: the panel's title, the area that lists the scene's pixel nodes, the lighting
//! preset buttons, the reference-wiring button and the preview hint belong to one scene, so
//! an author can rearrange or reword them there. This node is how the pixel workspace is
//! told which declared part plays which role, including the clock that keeps the workspace
//! in step with edits made elsewhere.
//!
//! Responsibility: it holds those parts as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about which rows are listed, what a
//! button does or how often the workspace looks again; the pixel workspace fills and
//! answers them.
//!
//! Why a node: the panel is a piece of the editor's interface, and its root is the node the
//! pixel workspace docks beside the viewport and frees when it stops.
//!
//! Why a new type: the pixel workspace plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in
//! container has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use godot::classes::{
    Button, GridContainer, IScrollContainer, Label, MarginContainer, ScrollContainer, Timer,
    VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = ScrollContainer)]
pub struct PixelWorldPanel {
    #[export]
    pub(super) margin: Option<Gd<MarginContainer>>,
    #[export]
    pub(super) column: Option<Gd<VBoxContainer>>,
    #[export]
    pub(super) world_title: Option<Gd<Button>>,
    #[export]
    pub(super) effect_rows: Option<Gd<VBoxContainer>>,
    #[export]
    pub(super) presets_title: Option<Gd<Button>>,
    #[export]
    pub(super) preset_buttons: Option<Gd<GridContainer>>,
    #[export]
    pub(super) wire_button: Option<Gd<Button>>,
    #[export]
    pub(super) preview_hint: Option<Gd<Label>>,
    #[export]
    pub(super) refresh_timer: Option<Gd<Timer>>,
    base: Base<ScrollContainer>,
}
#[godot_api]
impl IScrollContainer for PixelWorldPanel {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/pixel_world_panel.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelWorldPanel\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
