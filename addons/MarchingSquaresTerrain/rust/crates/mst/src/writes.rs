use std::cell::RefCell;
use std::collections::HashMap;
use godot::classes::{Node3D, ShaderMaterial};
use godot::prelude::*;
thread_local! {
    static NAMES: RefCell<HashMap<&'static str, StringName>> = RefCell::new(HashMap::new());
}
pub fn clear_names() {
    NAMES.with_borrow_mut(|m| m.clear());
}
pub fn moniker(n: &'static str) -> StringName {
    NAMES.with_borrow_mut(|m| m.entry(n).or_insert_with(|| StringName::from(n)).clone())
}
pub trait ShaderParams {
    fn param(&mut self, name: &'static str, value: &Variant);
}
impl ShaderParams for Gd<ShaderMaterial> {
    fn param(&mut self, name: &'static str, value: &Variant) {
        let sn = self::moniker(name);
        if self.get_shader_parameter(&sn) == *value {
            return;
        }
        self.set_shader_parameter(&sn, value);
    }
}
pub fn set_global_transform(node: &mut Gd<impl Inherits<Node3D>>, t: Transform3D) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_transform() != t {
        n.set_global_transform(t);
    }
}
pub fn set_internals_visible(node: &Gd<impl Inherits<Node>>, shown: bool) {
    let node = node.upcast_ref::<Node>();
    let internal = node.get_child_count_ex().include_internal(true).done() - node.get_child_count();
    for i in 0..internal {
        if let Some(c) = node.get_child_ex(i).include_internal(true).done()
            && let Ok(mut n) = c.try_cast::<Node3D>()
            && n.is_visible() != shown
        {
            n.set_visible(shown);
        }
    }
}
pub fn push_instance(
    buf: &mut Vec<f32>,
    t: Transform3D,
    tint: Option<Color>,
    custom: Option<Color>,
) {
    let (a, b, c, o) = (t.basis.col_a(), t.basis.col_b(), t.basis.col_c(), t.origin);
    buf.extend_from_slice(&[a.x, b.x, c.x, o.x, a.y, b.y, c.y, o.y, a.z, b.z, c.z, o.z]);
    for v in [tint, custom].into_iter().flatten() {
        buf.extend_from_slice(&[v.r, v.g, v.b, v.a]);
    }
}
#[derive(Default)]
pub struct InstanceBuffer {
    contents: Vec<f32>,
    last: Vec<f32>,
    aabb: Option<Aabb>,
}
impl InstanceBuffer {
    pub fn purge(&mut self) {
        self.contents.clear();
    }
    pub fn stack(&mut self, t: Transform3D, custom: Color) {
        let (a, b, c, o) = (t.basis.col_a(), t.basis.col_b(), t.basis.col_c(), t.origin);
        self.contents
            .extend_from_slice(&[a.x, b.x, c.x, o.x, a.y, b.y, c.y, o.y, a.z, b.z, c.z, o.z]);
        self.contents
            .extend_from_slice(&[custom.r, custom.g, custom.b, custom.a]);
    }
    pub fn upload(&mut self, mm: &mut Gd<godot::classes::MultiMesh>, aabb: Aabb) {
        if self.contents.len() == mm.get_instance_count().max(0) as usize * 16
            && self.contents != self.last
        {
            mm.set_buffer(&PackedFloat32Array::from(self.contents.as_slice()));
            std::mem::swap(&mut self.contents, &mut self.last);
        }
        if self.aabb != Some(aabb) {
            mm.set_custom_aabb(aabb);
            self.aabb = Some(aabb);
        }
    }
    pub fn revert(&mut self) {
        self.last.clear();
        self.aabb = None;
    }
}
pub fn assign_world_location(node: &mut Gd<impl Inherits<Node3D>>, p: Vector3) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_position() != p {
        n.set_global_position(p);
    }
}
pub fn set_visible_instances(mm: &mut Gd<godot::classes::MultiMesh>, count: i32) {
    if mm.get_visible_instance_count() != count {
        mm.set_visible_instance_count(count);
    }
}
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
        let basis = Basis::from_cols(
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(4.0, 5.0, 6.0),
            Vector3::new(7.0, 8.0, 9.0),
        );
        let mut b = InstanceBuffer::default();
        b.stack(
            Transform3D::new(basis, Vector3::new(10.0, 11.0, 12.0)),
            Color::from_rgba(0.1, 0.2, 0.3, 0.4),
        );
        assert_eq!(
            b.contents,
            vec![
                1.0, 4.0, 7.0, 10.0, 2.0, 5.0, 8.0, 11.0, 3.0, 6.0, 9.0, 12.0, 0.1, 0.2, 0.3, 0.4
            ]
        );
        b.purge();
        assert!(b.contents.is_empty());
    }
}
