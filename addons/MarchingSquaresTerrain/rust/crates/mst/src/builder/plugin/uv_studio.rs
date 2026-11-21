//! Shows the builder's UV editor either as a floating window or docked in the editor's
//! bottom area, depending on which editing layout is active, and moves it between the two.
//!
//! The window is instantiated from its scene the first time the person opens the UV
//! editor, and the dock that holds it is created the first time the docked layout needs
//! it, because whether and when either is wanted is only known while the editor runs. Both
//! belong to the editor, never to a scene the author edits.
use super::editor_scenes;
use super::*;
impl PixelBuilderPlugin {
    pub(super) fn unveil_tex_coord_studio(&mut self) {
        if self
            .uv_window
            .as_ref()
            .is_none_or(|w| !w.is_instance_valid())
            || self
                .uv_panel
                .as_ref()
                .is_none_or(|p| !p.is_instance_valid())
        {
            let Some(mut win) =
                editor_scenes::instantiate::<godot::classes::Window>(editor_scenes::UV_EDITOR)
            else {
                return;
            };
            let Some((panel, canvas)) = uv_editor::wire_window(&mut win) else {
                win.free();
                return;
            };
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&win);
            }
            self.uv_window = Some(win);
            self.uv_panel = Some(panel);
            self.uv_canvas = Some(canvas);
        }
        let docked = self.is_modeler();
        self.place_uv(docked);
        if docked {
            if let Some(d) = &mut self.uv_dock {
                d.make_visible();
            }
        } else if let Some(w) = &mut self.uv_window {
            if w.is_visible() {
                w.grab_focus();
            } else {
                w.popup_centered();
            }
        }
        self.push_uv_target();
    }
    pub(super) fn place_uv(&mut self, dock: bool) {
        let (Some(mut panel), Some(mut win)) = (
            self.uv_panel.clone().filter(|p| p.is_instance_valid()),
            self.uv_window.clone().filter(|w| w.is_instance_valid()),
        ) else {
            return;
        };
        let s = crate::editor::ui_kit::editor_scale();
        if dock {
            if self.uv_dock.as_ref().is_none_or(|d| !d.is_instance_valid()) {
                use godot::classes::editor_dock::{DockLayout, DockSlot};
                let mut d = godot::classes::EditorDock::new_alloc();
                d.set_name("PixelBuilderMeshUv");
                d.set_title("Mesh UV");
                d.set_layout_key("PixelBuilderMeshUv");
                d.set_default_slot(DockSlot::BOTTOM);
                d.set_available_layouts(DockLayout::HORIZONTAL | DockLayout::FLOATING);
                d.set_transient(true);
                d.set_global(false);
                d.set_closable(true);
                d.set_clip_contents(true);
                self.base_mut().add_dock(&d);
                self.uv_dock = Some(d);
            }
            let Some(mut d) = self.uv_dock.clone() else {
                return;
            };
            if panel
                .get_parent()
                .is_none_or(|p| p.instance_id() != d.instance_id())
            {
                win.hide();
                if let Some(mut p) = panel.get_parent() {
                    p.remove_child(&panel);
                }
                panel.set_custom_minimum_size(Vector2::new(0.0, 320.0 * s));
                d.add_child(&panel);
            }
        } else {
            if panel
                .get_parent()
                .is_none_or(|p| p.instance_id() != win.instance_id())
            {
                if let Some(mut p) = panel.get_parent() {
                    p.remove_child(&panel);
                }
                panel.set_custom_minimum_size(Vector2::ZERO);
                win.add_child(&panel);
                panel.set_anchors_and_offsets_preset(
                    godot::classes::control::LayoutPreset::FULL_RECT,
                );
            }
            if let Some(d) = &mut self.uv_dock
                && d.is_instance_valid()
                && d.is_visible_in_tree()
            {
                d.close();
            }
        }
    }
    pub(super) fn tex_coord_unveil(&self) -> bool {
        if self.tex_coord_carrier() == "bottom_panel" {
            return self
                .uv_panel
                .as_ref()
                .is_some_and(|p| p.is_instance_valid() && p.is_visible_in_tree());
        }
        self.uv_window
            .as_ref()
            .is_some_and(|w| w.is_instance_valid() && w.is_visible())
    }
    pub(super) fn tex_coord_carrier(&self) -> &'static str {
        match &self.uv_panel {
            Some(p) if p.is_instance_valid() => {
                if self.uv_dock.as_ref().is_some_and(|d| {
                    d.is_instance_valid()
                        && p.get_parent()
                            .is_some_and(|x| x.instance_id() == d.instance_id())
                }) {
                    "bottom_panel"
                } else {
                    "window"
                }
            }
            _ => "",
        }
    }
    pub(super) fn tune_tex_coord_studio(&mut self) {
        if self.tex_coord_unveil() {
            self.push_uv_target();
        }
    }
    pub(super) fn push_uv_target(&mut self) {
        let faces = if self.mode == Mode::Face {
            super::super::data::face_array(&self.sel_faces)
        } else {
            PackedInt32Array::new()
        };
        let target = self.target.clone();
        if let Some(c) = &mut self.uv_canvas
            && c.is_instance_valid()
        {
            c.bind_mut().set_target(target, faces);
        }
    }
}
