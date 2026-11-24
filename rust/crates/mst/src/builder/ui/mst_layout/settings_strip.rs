//! The strip of shape and tool settings under the 3D viewport in the builder's side-panel
//! layout.
//!
//! Intent: whatever the person modelling is about to draw or apply, its settings sit in
//! one flowing strip below the viewport: the shape choice, the sizes and counts the
//! current tool reads, the paint slot and colour, and a short read-out of the selection.
//! The strip scrolls rather than crowd out the viewport.
//!
//! Responsibility: it owns the strip's frame and the parts that always exist: the
//! hover read-out, the shape choice, the colour picker with its swatch row and the
//! separators around it, the round-shape and arched-door toggles and the selection
//! read-out. The numeric and choice settings come from the builder's field table, so
//! the builder places them between these parts, fills the swatch row from the palette,
//! decides what shows and answers every edit.
//!
//! Why a node: the strip is hosted in the editor's panel below the 3D viewport and must
//! sit in the tree there to lay out and receive input.
//!
//! Why a new type: a built-in scroll container declared in a scene can hold the parts,
//! but the builder needs to reach each of them by role. Typed references assigned in the
//! scene let the strip be rearranged freely while a missing part is reported to whoever
//! edits the scene, instead of the code hunting for parts by name.
use super::super::super::modeler_ui::assigned::unassigned_warnings;
use godot::classes::{
    CheckBox, ColorPickerButton, HBoxContainer, HFlowContainer, IScrollContainer, Label,
    OptionButton, ScrollContainer, VSeparator,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = ScrollContainer)]
pub struct PixelBuilderSettingsStrip {
    #[export]
    pub strip: Option<Gd<HFlowContainer>>,
    #[export]
    pub hover: Option<Gd<Label>>,
    #[export]
    pub shape_row: Option<Gd<HBoxContainer>>,
    #[export]
    pub shape: Option<Gd<OptionButton>>,
    #[export]
    pub slot_separator: Option<Gd<VSeparator>>,
    #[export]
    pub colour_caption: Option<Gd<Label>>,
    #[export]
    pub colour: Option<Gd<ColorPickerButton>>,
    #[export]
    pub swatches: Option<Gd<HBoxContainer>>,
    #[export]
    pub colour_end_separator: Option<Gd<VSeparator>>,
    #[export]
    pub smooth: Option<Gd<CheckBox>>,
    #[export]
    pub arched: Option<Gd<CheckBox>>,
    #[export]
    pub facts: Option<Gd<Label>>,
    base: Base<ScrollContainer>,
}
#[godot_api]
impl IScrollContainer for PixelBuilderSettingsStrip {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (
                self.strip.is_none(),
                "Assign the flowing row the settings fill.",
            ),
            (self.hover.is_none(), "Assign the hover read-out."),
            (
                self.shape_row.is_none(),
                "Assign the row that holds the shape choice.",
            ),
            (self.shape.is_none(), "Assign the shape choice."),
            (
                self.slot_separator.is_none(),
                "Assign the separator before the paint slot.",
            ),
            (self.colour_caption.is_none(), "Assign the colour caption."),
            (self.colour.is_none(), "Assign the colour picker."),
            (self.swatches.is_none(), "Assign the row the swatches fill."),
            (
                self.colour_end_separator.is_none(),
                "Assign the separator after the swatches.",
            ),
            (self.smooth.is_none(), "Assign the smooth shading toggle."),
            (self.arched.is_none(), "Assign the arched door toggle."),
            (self.facts.is_none(), "Assign the selection read-out."),
        ])
    }
}
