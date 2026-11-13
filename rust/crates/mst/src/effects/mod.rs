//! Optional scene effects: sky/clouds, day-night, weather, water, fireflies, light volumes,
//! birds, smoke and flipbook particle fields.
pub mod birds;
pub mod cloud_field;
pub mod day_night;
pub mod fire;
pub mod fireflies;
pub mod light_volume;
pub mod particle_field;
pub mod sky;
pub mod smoke;
pub mod water;
pub mod waterfall;
pub mod weather;

use godot::classes::{Camera3D, Node};
use godot::prelude::*;

use crate::util::is_editor;

/// Small deterministic PRNG (SplitMix64), so a given seed always gives the same result.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [-1, 1).
    pub fn signed(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

/// The camera effects follow: the 3D editor camera in the editor, else the viewport's camera.
pub fn active_camera(node: &Gd<Node>) -> Option<Gd<Camera3D>> {
    if is_editor() {
        crate::render::editor_preview::EditorHost::find().and_then(|h| h.camera())
    } else {
        node.get_viewport().and_then(|vp| vp.get_camera_3d())
    }
}

/// Where the camera's view ray meets the horizontal plane `y = plane_y` (or a point 20 m ahead).
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

/// First `MarchingSquaresTerrain` in the scene (for ground heights), if any.
pub fn find_terrain(node: &Gd<Node>) -> Option<Gd<Node>> {
    if !node.is_inside_tree() {
        return None;
    }
    let tree = node.get_tree();
    let root: Gd<Node> = if is_editor() {
        godot::classes::EditorInterface::singleton().get_edited_scene_root()?
    } else {
        tree.get_current_scene().unwrap_or_else(|| tree.get_root().upcast())
    };
    if root.is_class("MarchingSquaresTerrain") {
        return Some(root);
    }
    root.find_children_ex("*").type_("MarchingSquaresTerrain").recursive(true).owned(false).done().iter_shared().next()
}

/// Terrain height at a world XZ through `MarchingSquaresTerrain.get_height_at`, or `fallback`.
pub fn ground_height(terrain: &mut Option<Gd<Node>>, p: Vector3, fallback: f32) -> f32 {
    match terrain {
        Some(t) if t.is_instance_valid() => {
            t.call("get_height_at", &[p.to_variant(), fallback.to_variant()]).try_to::<f32>().unwrap_or(fallback)
        }
        _ => fallback,
    }
}
