use super::PixelBuilderPlugin;
use super::editor_scenes;
use godot::classes::Control;
use godot::prelude::*;
fn find_mesh_menu(ours: Option<&Gd<godot::classes::MenuButton>>) -> Option<Gd<Control>> {
    let parent = ours?.get_parent()?;
    parent
        .get_children()
        .iter_shared()
        .filter_map(|c| c.try_cast::<godot::classes::MenuButton>().ok())
        .find(|m| {
            m.get_text().to_string() == "Mesh" && m.get_name().to_string() != "PixelBuilderMenu"
        })
        .map(|m| m.upcast())
}
impl PixelBuilderPlugin {
    pub(super) fn sync_menus(&mut self, shown: bool) {
        use godot::classes::MenuButton;
        use godot::classes::editor_plugin::CustomControlContainer;
        if self
            .builder_menu
            .as_ref()
            .is_none_or(|m| !m.is_instance_valid())
            && !self.builder_menu_failed
        {
            match editor_scenes::instantiate::<MenuButton>(editor_scenes::BUILDER_MENU) {
                Some(mb) => {
                    if let Some(mut p) = mb.get_popup() {
                        p.connect("id_pressed", &self.to_gd().callable("on_builder_menu"));
                    }
                    self.base_mut()
                        .add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &mb);
                    self.builder_menu = Some(mb);
                    self.refresh_menu_keys();
                    self.refresh_menu_layout();
                }
                None => self.builder_menu_failed = true,
            }
        }
        let show = shown && self.target.is_some();
        if let Some(m) = &mut self.builder_menu {
            m.set_visible(show);
        }
        if show {
            if self
                .hidden_mesh_menu
                .as_ref()
                .is_none_or(|m| !m.is_instance_valid())
            {
                self.hidden_mesh_menu = find_mesh_menu(self.builder_menu.as_ref());
            }
            if let Some(m) = &mut self.hidden_mesh_menu {
                m.set_visible(false);
            }
        } else if let Some(mut m) = self.hidden_mesh_menu.take()
            && m.is_instance_valid()
        {
            m.set_visible(true);
        }
    }
    pub(super) fn refresh_menu_keys(&mut self) {
        let k = crate::builder::shortcuts::key_text("uv_editor");
        if let Some(m) = &self.builder_menu
            && m.is_instance_valid()
            && let Some(mut p) = m.get_popup()
        {
            p.set_item_text(
                0,
                &if k.is_empty() {
                    "UV editor…".to_string()
                } else {
                    format!("UV editor…  [{k}]")
                },
            );
        }
    }
    pub(super) fn remove_menus(&mut self) {
        use godot::classes::editor_plugin::CustomControlContainer;
        if let Some(mut m) = self.hidden_mesh_menu.take()
            && m.is_instance_valid()
        {
            m.set_visible(true);
        }
        if let Some(mb) = self.builder_menu.take()
            && mb.is_instance_valid()
        {
            self.base_mut()
                .remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &mb);
            mb.free();
        }
    }
}
