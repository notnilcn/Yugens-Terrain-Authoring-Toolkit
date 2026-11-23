//! The root of the builder's UV editor window.
//!
//! Intent: the UV editor's toolbar, its automatic-unwrap options, its template export and
//! its drawing surface belong to one scene, so an author can rearrange, restyle or relabel
//! them there. This node is how the builder is told which declared control plays which
//! part.
//!
//! Responsibility: it holds those controls as the scene author assigned them and warns the
//! author about any left unassigned. It decides nothing about what a control does; the
//! builder connects each one to the drawing surface, which owns the editing.
//!
//! Why a node: the UV editor is a window in the editor's tree, and its root is the window
//! the builder opens, docks and frees.
//!
//! Why a new type: the builder plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in window
//! has nowhere to keep them or to warn the scene author when one is missing.
//!
//! Not exported: nothing. It keeps no state of its own.
use super::PixelBuilderUvCanvas;
use godot::classes::{
    BaseButton, EditorFileDialog, IWindow, OptionButton, PanelContainer, SpinBox, Window,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = Window)]
pub struct PixelBuilderUvEditor {
    #[export_group(name = "Layout")]
    #[export]
    pub(super) panel: Option<Gd<PanelContainer>>,
    #[export]
    pub(super) canvas: Option<Gd<PixelBuilderUvCanvas>>,
    #[export]
    pub(super) template_dialog: Option<Gd<EditorFileDialog>>,
    #[export_group(name = "Modes")]
    #[export]
    pub(super) faces: Option<Gd<BaseButton>>,
    #[export]
    pub(super) verts: Option<Gd<BaseButton>>,
    #[export_group(name = "Commands")]
    #[export]
    pub(super) manual: Option<Gd<BaseButton>>,
    #[export]
    pub(super) auto: Option<Gd<BaseButton>>,
    #[export]
    pub(super) planar: Option<Gd<BaseButton>>,
    #[export]
    pub(super) box_map: Option<Gd<BaseButton>>,
    #[export]
    pub(super) spherical: Option<Gd<BaseButton>>,
    #[export]
    pub(super) flip_u: Option<Gd<BaseButton>>,
    #[export]
    pub(super) flip_v: Option<Gd<BaseButton>>,
    #[export]
    pub(super) rot90: Option<Gd<BaseButton>>,
    #[export]
    pub(super) fit: Option<Gd<BaseButton>>,
    #[export]
    pub(super) fit_u: Option<Gd<BaseButton>>,
    #[export]
    pub(super) fit_v: Option<Gd<BaseButton>>,
    #[export]
    pub(super) sew: Option<Gd<BaseButton>>,
    #[export]
    pub(super) copy: Option<Gd<BaseButton>>,
    #[export]
    pub(super) paste: Option<Gd<BaseButton>>,
    #[export]
    pub(super) texel: Option<Gd<BaseButton>>,
    #[export]
    pub(super) island: Option<Gd<BaseButton>>,
    #[export]
    pub(super) frame: Option<Gd<BaseButton>>,
    #[export]
    pub(super) frame_sel: Option<Gd<BaseButton>>,
    #[export]
    pub(super) scale: Option<Gd<BaseButton>>,
    #[export]
    pub(super) apply_auto: Option<Gd<BaseButton>>,
    #[export_group(name = "Flags")]
    #[export]
    pub(super) lock01: Option<Gd<BaseButton>>,
    #[export]
    pub(super) split: Option<Gd<BaseButton>>,
    #[export]
    pub(super) snap: Option<Gd<BaseButton>>,
    #[export]
    pub(super) tile: Option<Gd<BaseButton>>,
    #[export_group(name = "Values")]
    #[export]
    pub(super) step: Option<Gd<SpinBox>>,
    #[export]
    pub(super) scale_u: Option<Gd<SpinBox>>,
    #[export]
    pub(super) scale_v: Option<Gd<SpinBox>>,
    #[export_group(name = "Auto UVs")]
    #[export]
    pub(super) auto_fill: Option<Gd<OptionButton>>,
    #[export]
    pub(super) auto_anchor: Option<Gd<OptionButton>>,
    #[export]
    pub(super) auto_flip_u: Option<Gd<BaseButton>>,
    #[export]
    pub(super) auto_flip_v: Option<Gd<BaseButton>>,
    #[export]
    pub(super) auto_swap: Option<Gd<BaseButton>>,
    #[export]
    pub(super) auto_group: Option<Gd<BaseButton>>,
    #[export_group(name = "Template")]
    #[export]
    pub(super) template_size: Option<Gd<SpinBox>>,
    #[export]
    pub(super) template_grid: Option<Gd<BaseButton>>,
    #[export]
    pub(super) template_transparent: Option<Gd<BaseButton>>,
    base: Base<Window>,
}
#[godot_api]
impl IWindow for PixelBuilderUvEditor {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/uv_editor.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelBuilderUvEditor\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
