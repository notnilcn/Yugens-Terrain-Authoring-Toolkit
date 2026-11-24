//! The round grid-plane switch that sits on the 3D viewport while the combined Transform handles
//! are shown in the Roblox layout.
//!
//! It is instantiated from its scene when that layout is chosen and freed with the layout,
//! because the editor's viewport belongs to no scene an author edits and the layout choice
//! decides when the switch exists.
use super::super::plugin::editor_scenes;
use super::super::ui::Action;
use godot::classes::{Button, Control, EditorInterface};
use godot::prelude::*;
pub struct GridPlaneSwitch {
    pub button: Gd<Button>,
}
impl GridPlaneSwitch {
    pub fn assemble(addressee: &Gd<Object>) -> Option<GridPlaneSwitch> {
        let mut button = editor_scenes::instantiate::<Button>(editor_scenes::ROBLOX_GRID_PLANE)?;
        button.connect(
            "pressed",
            &Callable::from_object_method(addressee, "upon_deed")
                .bind(&[(Action::GridPlane as i64).to_variant()]),
        );
        Some(GridPlaneSwitch { button })
    }
    pub fn host_on_viewport(&mut self, idx: i32) -> bool {
        let Some(mut host) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(idx)
            .done()
            .and_then(|vp| vp.get_parent())
            .and_then(|c| c.get_parent())
            .and_then(|h| h.try_cast::<Control>().ok())
        else {
            return false;
        };
        if let Some(mut p) = self.button.get_parent() {
            p.remove_child(&self.button);
        }
        host.add_child(&self.button);
        true
    }
    pub fn sync(&mut self, shown_with_plane: Option<bool>) {
        if !self.button.is_instance_valid() {
            return;
        }
        let visible = shown_with_plane.is_some();
        if self.button.is_visible() != visible {
            self.button.set_visible(visible);
        }
        if let Some(on) = shown_with_plane
            && self.button.is_pressed() != on
        {
            self.button.set_pressed_no_signal(on);
        }
    }
    pub fn free(self, now: bool) {
        let mut b = self.button;
        if !b.is_instance_valid() {
            return;
        }
        if now {
            b.free();
        } else {
            if let Some(mut p) = b.get_parent() {
                p.remove_child(&b);
            }
            b.queue_free();
        }
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str =
        include_str!("../../../../../../editor/builder/roblox_grid_plane_button.tscn");
    #[test]
    fn the_switch_is_a_declared_toggle_that_takes_no_focus() {
        assert!(SCENE.contains("[node name=\"PixelRobloxGridPlane\" type=\"Button\"]"));
        assert!(SCENE.contains("toggle_mode = true"));
        assert!(SCENE.contains("focus_mode = 0"));
    }
}
