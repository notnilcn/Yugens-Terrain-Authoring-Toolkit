use godot::classes::{Camera3D, Control, EditorInterface, SubViewport};
use godot::prelude::*;
pub struct EditorHost {
    pub viewport: Gd<SubViewport>,
    pub container: Gd<Control>,
}
impl EditorHost {
    pub fn locate() -> Option<EditorHost> {
        let viewport = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()?;
        let container = viewport.get_parent()?.try_cast::<Control>().ok()?;
        Some(EditorHost {
            viewport,
            container,
        })
    }
    pub fn check_sane(&self) -> bool {
        self.viewport.is_instance_valid() && self.container.is_instance_valid()
    }
    pub fn camera(&self) -> Option<Gd<Camera3D>> {
        self.viewport.get_camera_3d()
    }
    pub fn screen_size(&self) -> Vector2 {
        self.container.get_size()
    }
    pub fn is_shown(&self) -> bool {
        self.container.is_visible_in_tree()
    }
}
