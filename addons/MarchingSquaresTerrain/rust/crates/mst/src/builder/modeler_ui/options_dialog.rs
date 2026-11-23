//! The options dialog of the builder's menu-bar layout.
//!
//! Intent: every setting the modelling tools read should be findable in one place. The
//! person modelling opens this dialog, searches or picks a category, and edits that
//! category's settings beside the list.
//!
//! Responsibility: it owns the dialog's frame: the search field, the category list and
//! the scrolling area the settings fill. Which categories exist and which settings each
//! shows come from the builder's tables, so the builder fills the list and the area and
//! answers every edit.
//!
//! Why a node: the dialog is a window of the editor that must exist in the tree to open
//! and receive input.
//!
//! Why a new type: a built-in dialog declared in a scene can hold the frame, but the
//! builder needs to reach each part by role. Typed references assigned in the scene let
//! the frame be rearranged freely while a missing part is reported to whoever edits the
//! scene, instead of the code hunting for parts by name.
use super::assigned::unassigned_warnings;
use godot::classes::{AcceptDialog, IAcceptDialog, ItemList, LineEdit, ScrollContainer};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = AcceptDialog)]
pub struct PixelModelerOptionsDialog {
    #[export]
    pub search: Option<Gd<LineEdit>>,
    #[export]
    pub categories: Option<Gd<ItemList>>,
    #[export]
    pub form_holder: Option<Gd<ScrollContainer>>,
    base: Base<AcceptDialog>,
}
#[godot_api]
impl IAcceptDialog for PixelModelerOptionsDialog {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (self.search.is_none(), "Assign the search field."),
            (self.categories.is_none(), "Assign the category list."),
            (
                self.form_holder.is_none(),
                "Assign the scrolling area the settings fill.",
            ),
        ])
    }
}
