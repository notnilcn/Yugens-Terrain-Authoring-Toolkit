//! The flame, smoke and steam batches a fire draws, each as one internal instanced mesh.
//!
//! Each batch builds its own mesh node and rebuilds it whenever the fire's particle budget
//! changes, so the fire's look is never stored in the scene that holds the fire.

use crate::node_children::{add_internal, free_node};
use crate::shader_materials::shader_material;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;
#[derive(Clone, Copy)]
pub struct Part {
    pub pos: Vector3,
    pub vel: Vector3,
    pub age: f32,
    pub life: f32,
    pub size0: f32,
    pub size1: f32,
    pub seed: f32,
    pub sort: u8,
    pub heat: f32,
}
#[derive(Default)]
pub struct PartSet {
    pub parts: Vec<Part>,
    pub mmi: Option<Gd<MultiMeshInstance3D>>,
    pub stuff: Option<Gd<ShaderMaterial>>,
    pub max: usize,
}
impl PartSet {
    pub fn assemble(&mut self, parent: &mut Gd<Node3D>, name: &str, shader: &str, max: usize) {
        free_node(self.mmi.take());
        self.max = max.max(1);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(2.0, 2.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        mm.set_instance_count(self.max as i32);
        crate::writes::set_visible_instances(&mut mm, 0);
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        mi.set_as_top_level(true);
        crate::writes::set_global_transform(&mut mi, Transform3D::IDENTITY);
        if self.stuff.is_none() {
            self.stuff = Some(shader_material(shader));
        }
        if let Some(m) = &self.stuff {
            mi.set_material_override(m);
        }
        add_internal(parent, &mi);
        self.mmi = Some(mi);
        self.parts.truncate(self.max);
    }
    pub fn stack(&mut self, p: Part) {
        if self.parts.len() >= self.max {
            if let Some((i, _)) = self
                .parts
                .iter()
                .enumerate()
                .max_by(|a, b| (a.1.age / a.1.life).total_cmp(&(b.1.age / b.1.life)))
            {
                self.parts.swap_remove(i);
            }
        }
        self.parts.push(p);
    }
    pub fn upload(&mut self) {
        let Some(mi) = self.mmi.clone().filter(|m| m.is_instance_valid()) else {
            return;
        };
        let Some(mut mm) = mi.get_multimesh() else {
            return;
        };
        let count = mm.get_instance_count() as usize;
        let n = self.parts.len().min(count);
        let mut lo = Vector3::splat(f32::MAX);
        let mut hi = Vector3::splat(f32::MIN);
        let mut buf = vec![0.0f32; count * 16];
        for (i, p) in self.parts.iter().take(n).enumerate() {
            let k = (p.age / p.life).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - k) * (1.0 - k);
            let s = (p.size0 + (p.size1 - p.size0) * ease).max(0.0055);
            buf[i * 16..i * 16 + 16].copy_from_slice(&[
                s,
                0.0,
                0.0,
                p.pos.x,
                0.0,
                s,
                0.0,
                p.pos.y,
                0.0,
                0.0,
                s,
                p.pos.z,
                k,
                p.seed,
                p.sort as f32,
                p.heat,
            ]);
            lo = lo.coord_min(p.pos - Vector3::splat(s));
            hi = hi.coord_max(p.pos + Vector3::splat(s));
        }
        mm.set_buffer(&PackedFloat32Array::from(buf.as_slice()));
        crate::writes::set_visible_instances(&mut mm, n as i32);
        if n > 0 {
            mm.set_custom_aabb(Aabb::new(lo, hi - lo));
        }
    }
    pub fn set_layers(&mut self, mask: u32) {
        if let Some(m) = &mut self.mmi {
            let layers = m.get_layer_mask() | mask;
            m.set_layer_mask(layers);
        }
    }
}
