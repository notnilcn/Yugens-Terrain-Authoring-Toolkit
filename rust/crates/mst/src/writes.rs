//! Change-only writes to the RenderingServer.
//!
//! Every RenderingServer write (a shader parameter, a global uniform, a node transform) wakes
//! the editor out of its low-processor mode for another full redraw, and a material parameter
//! write also re-uploads that material's uniform buffer. Nodes that sync state every frame
//! therefore go through these helpers, which skip values that did not change: at runtime that
//! saves the uploads, and in the editor a scene whose effects are paused stops redrawing.
//!
//! Names are interned once (`&'static str` -> `StringName`), so hot paths do not rebuild a
//! `StringName` from a string on every call.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::{Node3D, ShaderMaterial};
use godot::prelude::*;

thread_local! {
    static NAMES: RefCell<HashMap<&'static str, StringName>> = RefCell::new(HashMap::new());
}

/// Drops the interned names (before the engine unloads; see `lib.rs`).
pub fn clear_names() {
    NAMES.with_borrow_mut(|m| m.clear());
}

/// The interned `StringName` of a static name.
pub fn name(n: &'static str) -> StringName {
    NAMES.with_borrow_mut(|m| m.entry(n).or_insert_with(|| StringName::from(n)).clone())
}

/// Change-only shader parameter writes on a `ShaderMaterial`.
pub trait ShaderParams {
    /// Sets `name` to `value` unless the material already holds that value.
    fn param(&mut self, name: &'static str, value: &Variant);
}

impl ShaderParams for Gd<ShaderMaterial> {
    fn param(&mut self, name: &'static str, value: &Variant) {
        let sn = self::name(name);
        if self.get_shader_parameter(&sn) == *value {
            return;
        }
        self.set_shader_parameter(&sn, value);
    }
}

/// Sets a node's global transform unless it is already there (Godot re-sends an unchanged
/// transform to the RenderingServer).
pub fn set_global_transform(node: &mut Gd<impl Inherits<Node3D>>, t: Transform3D) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_transform() != t {
        n.set_global_transform(t);
    }
}

/// Shows or hides the internal children of `node` (the meshes an effect node builds for
/// itself), leaving the node's own `visible` (saved with the scene) untouched.
pub fn set_internals_visible(node: &Gd<impl Inherits<Node>>, visible: bool) {
    let node = node.upcast_ref::<Node>();
    let internal = node.get_child_count_ex().include_internal(true).done() - node.get_child_count();
    // Internal children are added in front (`util::add_internal`), so they come first.
    for i in 0..internal {
        if let Some(c) = node.get_child_ex(i).include_internal(true).done()
            && let Ok(mut n) = c.try_cast::<Node3D>()
            && n.is_visible() != visible
        {
            n.set_visible(visible);
        }
    }
}

/// One frame of MultiMesh instance data (`TRANSFORM_3D` + custom data), uploaded with a single
/// `set_buffer` call instead of two RenderingServer calls per instance, and only when it
/// differs from the last upload.
#[derive(Default)]
pub struct InstanceBuffer {
    data: Vec<f32>,
    last: Vec<f32>,
    aabb: Option<Aabb>,
}

impl InstanceBuffer {
    /// Starts a new frame.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Appends one instance: its transform (relative to the MultiMesh node) and custom data.
    pub fn push(&mut self, t: Transform3D, custom: Color) {
        let (a, b, c, o) = (t.basis.col_a(), t.basis.col_b(), t.basis.col_c(), t.origin);
        self.data.extend_from_slice(&[a.x, b.x, c.x, o.x, a.y, b.y, c.y, o.y, a.z, b.z, c.z, o.z]);
        self.data.extend_from_slice(&[custom.r, custom.g, custom.b, custom.a]);
    }

    /// Uploads the frame (the MultiMesh must use `TRANSFORM_3D` + custom data, no colors, and
    /// have exactly the pushed instance count) and its culling box, each only when changed.
    pub fn upload(&mut self, mm: &mut Gd<godot::classes::MultiMesh>, aabb: Aabb) {
        if self.data.len() == mm.get_instance_count().max(0) as usize * 16 && self.data != self.last {
            mm.set_buffer(&PackedFloat32Array::from(self.data.as_slice()));
            std::mem::swap(&mut self.data, &mut self.last);
        }
        if self.aabb != Some(aabb) {
            mm.set_custom_aabb(aabb);
            self.aabb = Some(aabb);
        }
    }

    /// Forget the last upload (after the MultiMesh was replaced).
    pub fn reset(&mut self) {
        self.last.clear();
        self.aabb = None;
    }
}

/// Sets a node's global position unless it is already there.
pub fn set_global_position(node: &mut Gd<impl Inherits<Node3D>>, p: Vector3) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_position() != p {
        n.set_global_position(p);
    }
}

/// Sets a MultiMesh's visible instance count unless it already has it.
pub fn set_visible_instances(mm: &mut Gd<godot::classes::MultiMesh>, count: i32) {
    if mm.get_visible_instance_count() != count {
        mm.set_visible_instance_count(count);
    }
}

/// `set_changed!(obj, getter, setter, value)` (or `set_changed!(obj, getter(arg), setter(arg),
/// value)` for indexed properties such as `Light3D` params): calls the setter only when the
/// getter returns something else. For engine setters that do not check for an unchanged value.
#[macro_export]
macro_rules! set_changed {
    ($obj:expr, $get:ident($($a:expr),*), $set:ident($($b:expr),*), $v:expr) => {{
        let v = $v;
        if $obj.$get($($a),*) != v {
            $obj.$set($($b,)* v);
        }
    }};
    ($obj:expr, $get:ident, $set:ident, $v:expr) => {{
        let v = $v;
        if $obj.$get() != v {
            $obj.$set(v);
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_buffer_matches_godot_transform_3d_layout() {
        // MultiMesh TRANSFORM_3D rows: (basis row 0, origin.x), (row 1, origin.y), (row 2, origin.z),
        // then the custom data.
        let basis = Basis::from_cols(Vector3::new(1.0, 2.0, 3.0), Vector3::new(4.0, 5.0, 6.0), Vector3::new(7.0, 8.0, 9.0));
        let mut b = InstanceBuffer::default();
        b.push(Transform3D::new(basis, Vector3::new(10.0, 11.0, 12.0)), Color::from_rgba(0.1, 0.2, 0.3, 0.4));
        assert_eq!(b.data, vec![1.0, 4.0, 7.0, 10.0, 2.0, 5.0, 8.0, 11.0, 3.0, 6.0, 9.0, 12.0, 0.1, 0.2, 0.3, 0.4]);
        b.clear();
        assert!(b.data.is_empty());
    }
}
