use godot::prelude::*;
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
pub fn set_visible_instances(mm: &mut Gd<godot::classes::MultiMesh>, count: i32) {
    if mm.get_visible_instance_count() != count {
        mm.set_visible_instance_count(count);
    }
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
