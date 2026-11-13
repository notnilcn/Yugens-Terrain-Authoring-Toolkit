//! Editor UI for the pixelizer nodes (`PixelRenderer`, `PixelObject` and the effects): an
//! MST-style icon toolbar left of the 3D viewport, an attribute strip below it and the
//! "Pixel World" panel on the right.
pub(super) mod actions;
mod fire_section;
mod plugin;
pub(super) mod presets;
pub(super) mod sections;
mod widgets;

pub(super) fn handles(object: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    sections::HANDLED.iter().any(|c| object.is_class(*c))
}
