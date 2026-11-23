//! The root of the builder's own section in a mesh's inspector.
//!
//! Intent: an author inspecting a mesh expects one block that says whether the builder can
//! work on it, summarises it when it can, and offers the builder's actions for it: turning
//! a plain mesh into an editable one, reopening the recipe it was drawn from, unwrapping
//! lightmap coordinates, choosing its role in the game and exporting or stripping its
//! editable data.
//!
//! Responsibility: it holds the section's parts as the scene author laid them out, so the
//! code that shows the section can reach each part without knowing where it sits. It does
//! not decide which parts apply to the inspected object, perform any action, or keep the
//! summary current; the inspector integration, the builder plugin and the summary node do
//! those.
//!
//! Why a node: the section is a piece of the inspector's tree, built afresh each time a
//! mesh is inspected, and its parts are nodes declared in the same scene.
//!
//! Why a new type: the inspector integration is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in
//! container has nowhere to keep them or to warn the scene author when one is missing.
use super::super::inspector_stats::PixelBuilderInspectorStats;
use godot::classes::{
    Button, ConfirmationDialog, FoldableContainer, IVBoxContainer, Label, VBoxContainer,
};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer)]
pub struct PixelBuilderInspectorSection {
    #[export]
    pub title: Option<Gd<Label>>,
    #[export_group(name = "Not Editable")]
    #[export]
    pub not_editable: Option<Gd<VBoxContainer>>,
    #[export]
    pub make_editable: Option<Gd<Button>>,
    #[export_group(name = "Mesh")]
    #[export]
    pub mesh_fold: Option<Gd<FoldableContainer>>,
    #[export]
    pub stats: Option<Gd<PixelBuilderInspectorStats>>,
    #[export_group(name = "Recipe")]
    #[export]
    pub recipe_fold: Option<Gd<FoldableContainer>>,
    #[export]
    pub edit_recipe: Option<Gd<Button>>,
    #[export_group(name = "UV and Lightmap")]
    #[export]
    pub uv_lightmap_fold: Option<Gd<FoldableContainer>>,
    #[export]
    pub open_uv_editor: Option<Gd<Button>>,
    #[export]
    pub generate_uv2: Option<Gd<Button>>,
    #[export_group(name = "Collision")]
    #[export]
    pub collision_fold: Option<Gd<FoldableContainer>>,
    #[export]
    pub set_collider: Option<Gd<Button>>,
    #[export]
    pub set_trigger: Option<Gd<Button>>,
    #[export_group(name = "Export and Data")]
    #[export]
    pub export_data_fold: Option<Gd<FoldableContainer>>,
    #[export]
    pub export_native: Option<Gd<Button>>,
    #[export]
    pub strip: Option<Gd<Button>>,
    #[export]
    pub strip_confirm: Option<Gd<ConfirmationDialog>>,
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for PixelBuilderInspectorSection {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}

#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/editable_mesh_inspector.tscn");
    #[test]
    fn every_assigned_part_is_declared() {
        assert!(SCENE.contains("type=\"PixelBuilderInspectorSection\""));
        assert_eq!(
            crate::editor::scene_refs::unresolved_root_refs(SCENE),
            Vec::<String>::new()
        );
    }
}
