//! The material palette beside the 3D viewport in the builder's side-panel layout.
//!
//! Intent: the person modelling manages how the selected mesh looks from one column:
//! its material slots, preset materials and palette files, its smoothing groups and the
//! automatic texture mapping of its faces.
//!
//! Responsibility: it owns the column's arrangement and the parts that always exist:
//! the headings, the slot buttons, the smoothing commands, the chip area with its hint
//! and menu, the texture mapping controls. The slot rows and smoothing chips depend on
//! the mesh being edited and the settings rows come from the builder's field tables, so
//! the builder fills those areas itself and answers every press.
//!
//! Why a node: the column is hosted in the editor's side panel and must sit in the tree
//! there to lay out and receive input.
//!
//! Why a new type: a built-in scroll container declared in a scene can hold the parts,
//! but the builder needs to reach each of them by role. Typed references assigned in the
//! scene let the column be rearranged freely while a missing part is reported to whoever
//! edits the scene, instead of the code hunting for parts by name.
use super::super::super::modeler_ui::assigned::unassigned_warnings;
use godot::classes::{
    Button, CheckBox, GridContainer, HFlowContainer, IScrollContainer, Label, MenuButton,
    PopupMenu, ScrollContainer, VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = ScrollContainer)]
pub struct PixelBuilderMaterialPalette {
    #[export]
    pub slots: Option<Gd<VBoxContainer>>,
    #[export]
    pub add_slot: Option<Gd<Button>>,
    #[export]
    pub presets: Option<Gd<MenuButton>>,
    #[export]
    pub palette_menu: Option<Gd<MenuButton>>,
    #[export]
    pub sample: Option<Gd<Button>>,
    #[export]
    pub smooth: Option<Gd<Button>>,
    #[export]
    pub faceted: Option<Gd<Button>>,
    #[export]
    pub new_group: Option<Gd<Button>>,
    #[export]
    pub chips: Option<Gd<HFlowContainer>>,
    #[export]
    pub chip_hint: Option<Gd<Label>>,
    #[export]
    pub chip_menu: Option<Gd<PopupMenu>>,
    #[export]
    pub smoothing_display: Option<Gd<HFlowContainer>>,
    #[export]
    pub uv_grid: Option<Gd<GridContainer>>,
    #[export]
    pub uv_world: Option<Gd<CheckBox>>,
    #[export]
    pub uv_apply: Option<Gd<Button>>,
    #[export]
    pub uv_reset: Option<Gd<Button>>,
    #[export]
    pub uv_rotate: Option<Gd<Button>>,
    base: Base<ScrollContainer>,
}
#[godot_api]
impl IScrollContainer for PixelBuilderMaterialPalette {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (
                self.slots.is_none(),
                "Assign the column the slot rows fill.",
            ),
            (self.add_slot.is_none(), "Assign the add slot button."),
            (self.presets.is_none(), "Assign the preset material menu."),
            (self.palette_menu.is_none(), "Assign the palette file menu."),
            (self.sample.is_none(), "Assign the sample button."),
            (self.smooth.is_none(), "Assign the smooth button."),
            (self.faceted.is_none(), "Assign the faceted button."),
            (self.new_group.is_none(), "Assign the new group button."),
            (
                self.chips.is_none(),
                "Assign the row the smoothing chips fill.",
            ),
            (
                self.chip_hint.is_none(),
                "Assign the hint shown when there are no groups.",
            ),
            (self.chip_menu.is_none(), "Assign the menu the chips open."),
            (
                self.smoothing_display.is_none(),
                "Assign the row the smoothing display settings fill.",
            ),
            (
                self.uv_grid.is_none(),
                "Assign the grid the texture mapping settings fill.",
            ),
            (self.uv_world.is_none(), "Assign the world aligned toggle."),
            (self.uv_apply.is_none(), "Assign the apply button."),
            (self.uv_reset.is_none(), "Assign the reset button."),
            (self.uv_rotate.is_none(), "Assign the rotate button."),
        ])
    }
}
