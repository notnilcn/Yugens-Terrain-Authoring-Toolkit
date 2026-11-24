//! Runs the 3D editor's own view commands. The editor offers them only through the menus of
//! each viewport and the shortcuts bound to them, so this is the one place that knows how
//! to reach them.
use godot::classes::{EditorInterface, InputEvent, MenuButton};
use godot::prelude::*;
const FOCUS_SELECTION: &str = "spatial_editor/focus_selection";
pub(crate) fn focus_selection(viewport_host: &Gd<Node>) -> bool {
    let Some(settings) = EditorInterface::singleton().get_editor_settings() else {
        return false;
    };
    if !settings.has_shortcut(FOCUS_SELECTION) {
        return false;
    }
    let Some(shortcut) = settings.get_shortcut(FOCUS_SELECTION) else {
        return false;
    };
    let events: Vec<Gd<InputEvent>> = shortcut
        .get_events()
        .iter_shared()
        .filter_map(|e| e.try_to::<Gd<InputEvent>>().ok())
        .collect();
    viewport_host
        .find_children_ex("*")
        .type_("MenuButton")
        .owned(false)
        .done()
        .iter_shared()
        .filter_map(|n| n.try_cast::<MenuButton>().ok())
        .filter_map(|mb| mb.get_popup())
        .any(|mut popup| events.iter().any(|e| popup.activate_item_by_event(e)))
}
