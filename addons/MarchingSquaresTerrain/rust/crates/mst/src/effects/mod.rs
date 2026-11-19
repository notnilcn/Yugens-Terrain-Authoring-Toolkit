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
pub mod sun_shafts;
pub mod water;
pub mod waterfall;
pub mod weather;

use godot::classes::{Camera3D, Node};
use godot::prelude::*;

use crate::features::{self, Feature};
use crate::util::is_editor;

/// The feature gate every effect node applies (from `ready` and from
/// `features::CALLBACK`): shows or hides the meshes the node built for itself (its internal
/// children), turns its `_process` on or off and refreshes its configuration warnings.
/// Returns whether the feature is on. The node's own `visible` is never touched (it is saved).
pub fn gate(node: &Gd<impl Inherits<Node>>, f: Feature) -> bool {
    let on = features::enabled(f);
    crate::writes::set_internals_visible(node, on);
    let mut n = node.clone().upcast::<Node>();
    n.set_process(on);
    // Deferred: the editor reads the warnings back through the (currently bound) node.
    n.call_deferred("update_configuration_warnings", &[]);
    on
}

/// `get_configuration_warnings` body of a gated effect node.
pub fn gate_warnings(f: Feature) -> PackedStringArray {
    let mut w = PackedStringArray::new();
    if let Some(msg) = features::warning(f) {
        w.push(&msg);
    }
    w
}

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
    find_in_scene(node, "MarchingSquaresTerrain")
}

thread_local! {
    /// `find_in_scene` cache: class -> (scene root, found node, retry time of a miss in ms).
    static SCENE_LOOKUPS: std::cell::RefCell<std::collections::HashMap<&'static str, (InstanceId, Option<Gd<Node>>, u64)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// First node of `class` in the scene `node` belongs to (the edited scene in the editor, the
/// current scene in game), for effects whose exported slot was left empty. Hits are cached
/// while the node stays in the tree; a miss is retried at most once a second, so callers can
/// ask every frame without walking the scene.
pub fn find_in_scene(node: &Gd<Node>, class: &'static str) -> Option<Gd<Node>> {
    if !node.is_inside_tree() {
        return None;
    }
    let tree = node.get_tree();
    let root: Gd<Node> = if is_editor() {
        godot::classes::EditorInterface::singleton().get_edited_scene_root()?
    } else {
        tree.get_current_scene().unwrap_or_else(|| tree.get_root().upcast())
    };
    let now = godot::classes::Time::singleton().get_ticks_msec();
    let root_id = root.instance_id();
    // Inspected in place: cloning a Gd whose node was freed panics.
    enum Cached {
        Hit(Gd<Node>),
        Miss,
        Stale,
    }
    let cached = SCENE_LOOKUPS.with_borrow(|m| match m.get(class) {
        Some((rid, Some(n), _)) if *rid == root_id && n.is_instance_valid() && n.is_inside_tree() => Cached::Hit(n.clone()),
        Some((rid, None, retry_at)) if *rid == root_id && now < *retry_at => Cached::Miss,
        _ => Cached::Stale,
    });
    match cached {
        Cached::Hit(n) => return Some(n),
        Cached::Miss => return None,
        Cached::Stale => {}
    }
    let found = if root.is_class(class) {
        Some(root.clone())
    } else {
        root.find_children_ex("*").type_(class).recursive(true).owned(false).done().iter_shared().next()
    };
    SCENE_LOOKUPS.with_borrow_mut(|m| m.insert(class, (root_id, found.clone(), now + 1000)));
    found
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
