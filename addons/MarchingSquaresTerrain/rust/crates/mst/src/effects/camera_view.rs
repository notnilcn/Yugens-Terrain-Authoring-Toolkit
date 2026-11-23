use crate::editor_detection::is_editor;
use godot::classes::{Camera3D, Node};
use godot::prelude::*;
pub fn active_camera(node: &Gd<Node>) -> Option<Gd<Camera3D>> {
    if is_editor() {
        crate::render::editor_preview::EditorHost::locate().and_then(|h| h.camera())
    } else {
        node.get_viewport().and_then(|vp| vp.get_camera_3d())
    }
}
pub fn camera_focus(cam: &Gd<Camera3D>, plane_y: f32) -> Vector3 {
    let t = cam.get_global_transform();
    let fwd = -t.basis.col_c();
    if fwd.y < -1e-3 {
        let d = (plane_y - t.origin.y) / fwd.y;
        if d > 0.0 {
            return t.origin + fwd * d.min(1000.0);
        }
    }
    t.origin + fwd * 20.0
}
