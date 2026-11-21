pub(super) mod actions;
mod fire_section;
mod plugin;
pub(super) mod presets;
pub(super) mod sections;
pub(super) mod widgets;
pub(super) fn handles(object: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    sections::HANDLED.iter().any(|c| object.is_class(*c))
}
