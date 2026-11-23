//! The builder's toolbar in the editor's menu bar, with the popovers it opens.
//!
//! Intent: the person modelling switches between object and element editing, starts a
//! new mesh, converts an existing one, runs commands and reaches the paint, handle and
//! option settings from one strip above the viewport, the way a dedicated modelling
//! program presents its tools.
//!
//! Responsibility: it owns the strip's arrangement: the editing-mode switch, the action
//! buttons, the command menu, and the shape palette and handle settings popovers it
//! opens. It decides nothing about modelling; the builder decides what shows, fills the
//! command menu, the palette tiles and the settings from its tables, and answers every
//! press.
//!
//! Why a node: the strip is hosted in the editor's 3D menu bar and must sit in the tree
//! there to lay out with the editor's own tools and receive input.
//!
//! Why a new type: a built-in container declared in a scene can hold the strip, but the
//! builder needs to reach each part by role. Typed references assigned in the scene let
//! the strip be rearranged freely while a missing part is reported to whoever edits the
//! scene, instead of the code hunting for parts by name.
use super::assigned::unassigned_warnings;
use godot::classes::{
    Button, GridContainer, HBoxContainer, IHBoxContainer, Label, LinkButton, MenuButton,
    PopupPanel, VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = HBoxContainer)]
pub struct PixelModelerToolbar {
    #[export]
    pub mode_object: Option<Gd<Button>>,
    #[export]
    pub mode_vertex: Option<Gd<Button>>,
    #[export]
    pub mode_edge: Option<Gd<Button>>,
    #[export]
    pub mode_face: Option<Gd<Button>>,
    #[export]
    pub add_mesh: Option<Gd<Button>>,
    #[export]
    pub make_editable: Option<Gd<Button>>,
    #[export]
    pub actions: Option<Gd<MenuButton>>,
    #[export]
    pub snap: Option<Gd<Button>>,
    #[export]
    pub paint: Option<Gd<Button>>,
    #[export]
    pub handles: Option<Gd<Button>>,
    #[export]
    pub handles_popup: Option<Gd<PopupPanel>>,
    #[export]
    pub handles_column: Option<Gd<VBoxContainer>>,
    #[export]
    pub handles_title: Option<Gd<Label>>,
    #[export]
    pub all_options: Option<Gd<LinkButton>>,
    #[export]
    pub options: Option<Gd<Button>>,
    #[export]
    pub palette: Option<Gd<PopupPanel>>,
    #[export]
    pub palette_grid: Option<Gd<GridContainer>>,
    base: Base<HBoxContainer>,
}
#[godot_api]
impl IHBoxContainer for PixelModelerToolbar {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (self.mode_object.is_none(), "Assign the object mode button."),
            (self.mode_vertex.is_none(), "Assign the vertex mode button."),
            (self.mode_edge.is_none(), "Assign the edge mode button."),
            (self.mode_face.is_none(), "Assign the face mode button."),
            (self.add_mesh.is_none(), "Assign the add mesh button."),
            (
                self.make_editable.is_none(),
                "Assign the make editable button.",
            ),
            (self.actions.is_none(), "Assign the command menu."),
            (self.snap.is_none(), "Assign the snap toggle."),
            (self.paint.is_none(), "Assign the paint button."),
            (self.handles.is_none(), "Assign the handle settings button."),
            (
                self.handles_popup.is_none(),
                "Assign the handle settings popover.",
            ),
            (
                self.handles_column.is_none(),
                "Assign the handle settings column.",
            ),
            (
                self.handles_title.is_none(),
                "Assign the handle settings title.",
            ),
            (
                self.all_options.is_none(),
                "Assign the link to all options.",
            ),
            (self.options.is_none(), "Assign the options button."),
            (self.palette.is_none(), "Assign the shape palette popover."),
            (
                self.palette_grid.is_none(),
                "Assign the grid the shape tiles fill.",
            ),
        ])
    }
}
