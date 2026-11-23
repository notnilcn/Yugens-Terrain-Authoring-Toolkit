//! The root of the Roblox Studio-style ribbon scene.
//!
//! Intent: the ribbon's look and arrangement belong to its scene, so an author can move,
//! restyle or regroup its tabs, tools, menus and popups there; this node is how the builder
//! is told which declared control plays which part.
//!
//! Responsibility: it holds the ribbon's fixed controls as the scene author assigned them
//! and warns the author about any left unassigned. It decides nothing about what a control
//! does, how it looks in the compact layout or when it is enabled; the ribbon view and the
//! builder plugin own those.
//!
//! Why a node: the ribbon is a piece of the 3D editor's toolbar area and its controls are
//! nodes declared in the same scene.
//!
//! Why a new type: the builder plugin is created by the engine and cannot be handed
//! references into the scene, so the scene's root has to carry them, and a built-in
//! container has nowhere to keep them or to warn the scene author when one is missing.
use godot::classes::{
    Button, CheckBox, ColorPickerButton, HFlowContainer, IVBoxContainer, ItemList,
    Label, LineEdit, MenuButton, OptionButton, PopupMenu, PopupPanel, ScrollContainer, SpinBox,
    TabBar, TextureRect, VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer)]
pub struct PixelRobloxRibbon {
    #[export_group(name = "Tabs")]
    #[export]
    pub(super) model_tab: Option<Gd<Button>>,
    #[export]
    pub(super) edit_tab: Option<Gd<Button>>,
    #[export]
    pub(super) pages: Option<Gd<ScrollContainer>>,
    #[export]
    pub(super) model_page: Option<Gd<HFlowContainer>>,
    #[export]
    pub(super) edit_page: Option<Gd<HFlowContainer>>,
    #[export_group(name = "Steps")]
    #[export]
    pub(super) grid_step: Option<Gd<SpinBox>>,
    #[export]
    pub(super) rot_step: Option<Gd<SpinBox>>,
    #[export_group(name = "Menus")]
    #[export]
    pub(super) drag_mode: Option<Gd<MenuButton>>,
    #[export]
    pub(super) pivot_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) part_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) group_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) lock_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) anchor_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) select_menu: Option<Gd<MenuButton>>,
    #[export]
    pub(super) more_shapes: Option<Gd<PopupMenu>>,
    #[export_group(name = "Commands")]
    #[export]
    pub(super) roblox_select: Option<Gd<Button>>,
    #[export]
    pub(super) gizmo_move: Option<Gd<Button>>,
    #[export]
    pub(super) gizmo_scale: Option<Gd<Button>>,
    #[export]
    pub(super) gizmo_rotate: Option<Gd<Button>>,
    #[export]
    pub(super) gizmo_transform: Option<Gd<Button>>,
    #[export]
    pub(super) snap_grid: Option<Gd<Button>>,
    #[export]
    pub(super) rot_snap_toggle: Option<Gd<Button>>,
    #[export]
    pub(super) tool_edit_pivot: Option<Gd<Button>>,
    #[export]
    pub(super) center_pivot: Option<Gd<Button>>,
    #[export]
    pub(super) insert_part_at_view: Option<Gd<Button>>,
    #[export]
    pub(super) material_picker: Option<Gd<Button>>,
    #[export]
    pub(super) color_picker: Option<Gd<Button>>,
    #[export]
    pub(super) paint_colour: Option<Gd<Button>>,
    #[export]
    pub(super) click_to_paint: Option<Gd<Button>>,
    #[export]
    pub(super) group_selection: Option<Gd<Button>>,
    #[export]
    pub(super) toggle_lock: Option<Gd<Button>>,
    #[export]
    pub(super) toggle_anchor: Option<Gd<Button>>,
    #[export]
    pub(super) align_panel: Option<Gd<Button>>,
    #[export]
    pub(super) roblox_union: Option<Gd<Button>>,
    #[export]
    pub(super) roblox_intersect: Option<Gd<Button>>,
    #[export]
    pub(super) separate: Option<Gd<Button>>,
    #[export]
    pub(super) negate: Option<Gd<Button>>,
    #[export]
    pub(super) mode_object: Option<Gd<Button>>,
    #[export]
    pub(super) mode_face: Option<Gd<Button>>,
    #[export]
    pub(super) mode_edge: Option<Gd<Button>>,
    #[export]
    pub(super) mode_vertex: Option<Gd<Button>>,
    #[export]
    pub(super) extrude: Option<Gd<Button>>,
    #[export]
    pub(super) inset: Option<Gd<Button>>,
    #[export]
    pub(super) bevel: Option<Gd<Button>>,
    #[export]
    pub(super) insert_loop: Option<Gd<Button>>,
    #[export]
    pub(super) bridge: Option<Gd<Button>>,
    #[export]
    pub(super) merge: Option<Gd<Button>>,
    #[export]
    pub(super) delete: Option<Gd<Button>>,
    #[export]
    pub(super) uv_editor: Option<Gd<Button>>,
    #[export]
    pub(super) pivot_to_selection: Option<Gd<Button>>,
    #[export_group(name = "Material Popup")]
    #[export]
    pub(super) material_popup: Option<Gd<PopupPanel>>,
    #[export]
    pub(super) material_search: Option<Gd<LineEdit>>,
    #[export]
    pub(super) material_grid: Option<Gd<Button>>,
    #[export]
    pub(super) material_list: Option<Gd<Button>>,
    #[export]
    pub(super) material_shelves: Option<Gd<TabBar>>,
    #[export]
    pub(super) material_items: Option<Gd<ItemList>>,
    #[export]
    pub(super) material_empty: Option<Gd<Label>>,
    #[export_group(name = "Color Popup")]
    #[export]
    pub(super) color_popup: Option<Gd<PopupPanel>>,
    #[export]
    pub(super) honeycomb: Option<Gd<TextureRect>>,
    #[export]
    pub(super) custom_colour: Option<Gd<ColorPickerButton>>,
    #[export_group(name = "Align Popup")]
    #[export]
    pub(super) align_popup: Option<Gd<PopupPanel>>,
    #[export]
    pub(super) align_min: Option<Gd<Button>>,
    #[export]
    pub(super) align_center: Option<Gd<Button>>,
    #[export]
    pub(super) align_max: Option<Gd<Button>>,
    #[export]
    pub(super) align_x: Option<Gd<CheckBox>>,
    #[export]
    pub(super) align_y: Option<Gd<CheckBox>>,
    #[export]
    pub(super) align_z: Option<Gd<CheckBox>>,
    #[export]
    pub(super) align_relative: Option<Gd<OptionButton>>,
    #[export]
    pub(super) align_local: Option<Gd<Button>>,
    #[export]
    pub(super) align_apply: Option<Gd<Button>>,
    base: Base<VBoxContainer>,
}
#[godot_api]
impl IVBoxContainer for PixelRobloxRibbon {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/roblox_ribbon.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("[node name=\"PixelRobloxRibbon\" type=\"PixelRobloxRibbon\""));
        assert_eq!(crate::editor::scene_refs::unresolved_root_refs(SCENE), Vec::<String>::new());
    }
}
