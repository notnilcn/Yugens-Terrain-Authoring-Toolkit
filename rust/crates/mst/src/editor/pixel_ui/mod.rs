pub(super) mod actions;
mod census;
mod fire_section;
mod plugin;
pub(super) mod presets;
pub(super) mod sections;
pub(super) mod widgets;
mod world_panel;
pub(super) fn handles(object: &godot::prelude::Gd<godot::prelude::Object>) -> bool {
    sections::workspace_class(|c| object.is_class(c)).is_some()
}
