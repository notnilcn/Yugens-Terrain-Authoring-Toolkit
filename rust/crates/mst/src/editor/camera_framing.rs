//! Lifts the editor camera above a terrain that would otherwise open with the camera
//! buried inside it, by borrowing the editor's own focus-selection framing.
//!
//! It creates a short-lived marker node in code because the point to frame is computed
//! from the terrain's extent at the moment the scene opens; the marker exists only for the
//! duration of one framing request and is released straight after.
//!
//! It runs outside any borrow of the terrain plugin, because the viewport input it pushes
//! is forwarded by the editor back to the plugin's own input handling.
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::{Control, EditorInterface, InputEventMouseButton};
use godot::global::MouseButton;
use godot::prelude::*;
pub(super) fn frame_if_buried(terrain: Gd<Node3D>) {
    if !terrain.is_inside_tree() {
        return;
    }
    let Ok(t) = terrain.clone().try_cast::<MarchingSquaresTerrain>() else {
        return;
    };
    let Some(bounds) = t.bind().world_bounds() else {
        return;
    };
    let ei = EditorInterface::singleton();
    let Some(vp) = ei.get_editor_viewport_3d_ex().idx(0).done() else {
        return;
    };
    let Some(cam) = vp.get_camera_3d() else {
        return;
    };
    let top = bounds.position.y + bounds.size.y;
    if cam.get_global_position().y > top + 1.0 {
        return;
    }
    let mut n: Option<Gd<Node>> = vp.get_parent();
    while let Some(p) = n.clone() {
        if p.is_class("Node3DEditorViewport") {
            break;
        }
        n = p.get_parent();
    }
    let Some(view) = n else { return };
    let Some(mut sel) = ei.get_selection() else {
        return;
    };
    let previous: Vec<Gd<Node>> = sel.get_selected_nodes().iter_shared().collect();
    let mut marker = Node3D::new_alloc();
    marker.set_name("__mst_focus");
    let mut parent = terrain.clone();
    parent.add_child(&marker);
    let c = bounds.position + bounds.size * 0.5;
    marker.set_global_position(Vector3::new(c.x, top, c.z));
    sel.clear();
    sel.add_node(&marker);
    if super::view_commands::focus_selection(&view) {
        let span = bounds.size.x.max(bounds.size.z).max(8.0);
        let steps = ((span * 0.6005 / 4.0).ln() / 1.08f32.ln()).clamp(0.0, 60.0) as i32;
        zoom_out(&view, steps);
    } else {
        godot_warn!(
            "Marching Squares Terrain: could not frame the terrain (no Focus Selection shortcut)"
        );
    }
    sel.clear();
    for p in previous {
        sel.add_node(&p);
    }
    marker.queue_free();
}
fn zoom_out(view: &Gd<Node>, steps: i32) {
    let Ok(view) = view.clone().try_cast::<Control>() else {
        return;
    };
    let Some(mut host) = view.get_viewport() else {
        return;
    };
    let at = view.get_global_rect().center();
    for _ in 0..steps {
        for pressed in [true, false] {
            let mut ev = InputEventMouseButton::new_gd();
            ev.set_button_index(MouseButton::WHEEL_DOWN);
            ev.set_pressed(pressed);
            ev.set_factor(1.0);
            ev.set_position(at);
            ev.set_global_position(at);
            host.push_input(&ev);
        }
    }
}
