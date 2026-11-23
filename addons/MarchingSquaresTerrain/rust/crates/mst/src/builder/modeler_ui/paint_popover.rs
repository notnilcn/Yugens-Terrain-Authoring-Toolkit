//! The paint popover of the builder's menu-bar layout.
//!
//! Intent: painting a mesh should feel like picking from a palette. The person modelling
//! opens one popover from the toolbar and finds the brush tools, their material slots,
//! colours and smoothing groups there, switching between them without leaving the
//! viewport.
//!
//! Responsibility: it owns the popover's frame: its heading, the summary of what is
//! selected, the tool toggles, the sampling and options buttons, the page switch, the
//! area each page fills and the menu its tiles open. It keeps itself inside the editor
//! window, scrolling the page area when the page is taller than the room left. The tiles on each page depend on the
//! mesh being edited, so the builder fills that area itself and answers every press.
//!
//! Why a node: the popover is a window of the editor that must exist in the tree to open
//! below the toolbar and receive input.
//!
//! Why a new type: a built-in popup declared in a scene can hold the frame, but the
//! builder needs to reach each part by role. Typed references assigned in the scene let
//! the frame be rearranged freely while a missing part is reported to whoever edits the
//! scene, instead of the code hunting for parts by name.
use super::assigned::unassigned_warnings;
use godot::classes::{
    Button, IPopupPanel, Label, MenuButton, PopupMenu, PopupPanel, ScrollContainer, VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = PopupPanel)]
pub struct PixelModelerPaintPopover {
    #[export]
    pub column: Option<Gd<VBoxContainer>>,
    #[export]
    pub heading: Option<Gd<Label>>,
    #[export]
    pub summary: Option<Gd<Label>>,
    #[export]
    pub brush: Option<Gd<Button>>,
    #[export]
    pub vertex_brush: Option<Gd<Button>>,
    #[export]
    pub sample: Option<Gd<Button>>,
    #[export]
    pub paint_options: Option<Gd<MenuButton>>,
    #[export]
    pub material_page: Option<Gd<Button>>,
    #[export]
    pub colour_page: Option<Gd<Button>>,
    #[export]
    pub smoothing_page: Option<Gd<Button>>,
    #[export]
    pub content: Option<Gd<VBoxContainer>>,
    #[export]
    pub content_scroll: Option<Gd<ScrollContainer>>,
    #[export]
    pub tile_menu: Option<Gd<PopupMenu>>,
    base: Base<PopupPanel>,
}
#[godot_api]
impl IPopupPanel for PixelModelerPaintPopover {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (
                self.column.is_none(),
                "Assign the column that holds the popover's rows.",
            ),
            (self.heading.is_none(), "Assign the heading label."),
            (
                self.summary.is_none(),
                "Assign the selection summary label.",
            ),
            (self.brush.is_none(), "Assign the paint tool toggle."),
            (
                self.vertex_brush.is_none(),
                "Assign the vertex paint tool toggle.",
            ),
            (self.sample.is_none(), "Assign the sample button."),
            (
                self.paint_options.is_none(),
                "Assign the paint options menu.",
            ),
            (
                self.material_page.is_none(),
                "Assign the material page button.",
            ),
            (self.colour_page.is_none(), "Assign the colour page button."),
            (
                self.smoothing_page.is_none(),
                "Assign the smoothing page button.",
            ),
            (
                self.content.is_none(),
                "Assign the container each page fills.",
            ),
            (self.tile_menu.is_none(), "Assign the menu the tiles open."),
            (
                self.content_scroll.is_none(),
                "Assign the scroller around the page area, so a tall page stays inside the window.",
            ),
        ])
    }
}
#[godot_api]
impl PixelModelerPaintPopover {
    #[func]
    fn fit_to_window(&mut self) {
        if !self.base().is_visible() {
            return;
        }
        let Some(mut scroll) = self.content_scroll.clone() else {
            return;
        };
        let mut window = self.to_gd().upcast::<godot::classes::Window>();
        crate::editor::ui_kit::fit_popover(&mut window, &mut scroll);
    }
}
