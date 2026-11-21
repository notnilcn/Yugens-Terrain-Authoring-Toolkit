//! A swarm of fireflies that drift and blink, mostly at night.
//!
//! Intent: a scene author places one over a meadow or a pond and gets glowing points that
//! wander within an area and pulse, fading in as night falls.
//!
//! Responsibility: it owns where the fireflies hover and how they glow. It reads the night
//! amount others publish and follows the project switch for its kind of effect. It lights
//! nothing else.
//!
//! Why a node: it has a place and an area in the world, and its drawing has to be in the
//! tree to be rendered.
//!
//! Why a new type: a built-in particle system re-emits and drifts away; this swarm keeps a
//! fixed, seeded population inside its area so that it looks the same every time the scene
//! loads.
//!
//! Created in code: the swarm is drawn by one internal instanced mesh the node builds for
//! itself and rebuilds when the swarm's size or area changes. Authors place this node by its
//! type alone, and the mesh is shaped entirely by the node's own settings, so there is
//! nothing for a scene to declare.
//!
//! Not exported: handles to the parts it builds.

use super::feature_gate::{gate, gate_warnings};
use super::rng::Rng;
use crate::features::Feature;
use crate::globals;
use crate::node_children::add_internal;
use crate::shader_materials::shader_material;
use crate::writes::{InstanceBuffer, ShaderParams};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelFireflies {
    #[export(range = (0.0, 4096.0, 1.0, or_greater))]
    #[var(set = assign_tally)]
    count: i32,
    #[export]
    #[var(set = set_extents)]
    extents: Vector3,
    #[export]
    #[var(set = set_seed)]
    seed: i32,
    #[export]
    #[var(set = assign_tint)]
    tint: Color,
    #[export(range = (0.0105, 2.0, 0.0105))]
    #[var(set = assign_measure)]
    size: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_night_only)]
    night_only: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    #[var(set = set_wander)]
    wander: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    #[var(set = set_blink_speed)]
    blink_speed: f32,
    #[export(range = (0.0, 8.0, 0.0105))]
    #[var(set = set_energy)]
    energy: f32,
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,
    stuff: Option<Gd<ShaderMaterial>>,
    swarm: Option<Gd<MultiMeshInstance3D>>,
    base: Base<Node3D>,
}
const WANDER: f32 = 0.6005;
#[godot_api]
impl INode3D for PixelFireflies {
    fn init(base: Base<Node3D>) -> Self {
        PixelFireflies {
            count: 48,
            extents: Vector3::new(8.0, 1.5005, 8.0),
            seed: 1,
            tint: Color::from_rgb(1.0, 0.9005, 0.4505),
            size: 0.1205,
            night_only: 1.0,
            wander: WANDER,
            blink_speed: 1.3005,
            energy: 2.0,
            layers: 1,
            stuff: None,
            swarm: None,
            base,
        }
    }
    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        super::legacy_multimesh_properties::is_legacy_multimesh_prop(&property.to_string())
    }
    fn ready(&mut self) {
        crate::features::observe(&self.to_gd());
        self.reassemble();
        gate(&self.to_gd(), Feature::Fireflies);
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        gate_warnings(Feature::Fireflies)
    }
}
impl PixelFireflies {
    fn swarm(&mut self) -> Gd<MultiMeshInstance3D> {
        if let Some(s) = &self.swarm
            && s.is_instance_valid()
        {
            return s.clone();
        }
        let mut n = MultiMeshInstance3D::new_alloc();
        n.set_name("Swarm");
        n.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        add_internal(&mut self.to_gd(), &n);
        self.swarm = Some(n.clone());
        self.apply_layers();
        n
    }
    fn apply_layers(&mut self) {
        let layers = self.layers & !globals::meta_mask();
        if let Some(s) = &mut self.swarm {
            s.set_layer_mask(layers);
        }
    }
    fn reassemble(&mut self) {
        if !self.base().is_inside_tree() || !crate::features::active(Feature::Fireflies) {
            return;
        }
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(1.0, 1.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        let n = self.count.max(0);
        mm.set_instance_count(n);
        let mut rng = Rng(self.seed as u64 ^ 0x5DEE_CE66_D1CE_4E5B);
        let e = self.extents.abs();
        let mut buffer = InstanceBuffer::default();
        for _ in 0..n {
            let p = Vector3::new(
                (rng.next_f32() * 2.0 - 1.0) * e.x,
                (rng.next_f32() * 2.0 - 1.0) * e.y,
                (rng.next_f32() * 2.0 - 1.0) * e.z,
            );
            let custom = Color::from_rgba(rng.next_f32(), 0.0, 0.0, 0.0);
            buffer.stack(Transform3D::new(Basis::IDENTITY, p), custom);
        }
        let grow = Vector3::splat(self.wander + self.size);
        buffer.upload(&mut mm, Aabb::new(-e - grow, (e + grow) * 2.0));
        self.swarm().set_multimesh(&mm);
        if self.stuff.is_none() {
            self.stuff = Some(shader_material("firefly.gdshader"));
        }
        self.revise_stuff();
    }
    fn revise_stuff(&mut self) {
        let Some(mut m) = self.stuff.clone() else {
            return;
        };
        m.param("color", &self.tint.to_variant());
        m.param("size", &self.size.to_variant());
        m.param("wander", &self.wander.to_variant());
        m.param("blink_speed", &self.blink_speed.to_variant());
        m.param("energy", &self.energy.to_variant());
        m.param("night_only", &self.night_only.to_variant());
        if let Some(s) = &mut self.swarm {
            s.set_material_override(&m);
        }
    }
}
#[godot_api]
impl PixelFireflies {
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::Fireflies) && self.get_swarm().is_none() {
            self.reassemble();
        }
    }
    #[func]
    pub fn get_swarm(&self) -> Option<Gd<MultiMeshInstance3D>> {
        self.swarm.clone().filter(|s| s.is_instance_valid())
    }
    #[func]
    fn set_layers(&mut self, v: u32) {
        self.layers = v;
        self.apply_layers();
    }
    #[func]
    fn assign_tally(&mut self, v: i32) {
        self.count = v.max(0);
        self.reassemble();
    }
    #[func]
    fn set_extents(&mut self, v: Vector3) {
        self.extents = v;
        self.reassemble();
    }
    #[func]
    fn set_seed(&mut self, v: i32) {
        self.seed = v;
        self.reassemble();
    }
    #[func]
    fn assign_tint(&mut self, v: Color) {
        self.tint = v;
        self.revise_stuff();
    }
    #[func]
    fn assign_measure(&mut self, v: f32) {
        self.size = v.max(0.0015);
        self.revise_stuff();
    }
    #[func]
    fn set_night_only(&mut self, v: f32) {
        self.night_only = v.clamp(0.0, 1.0);
        self.revise_stuff();
    }
    #[func]
    fn set_wander(&mut self, v: f32) {
        self.wander = v.max(0.0);
        self.reassemble();
    }
    #[func]
    fn set_blink_speed(&mut self, v: f32) {
        self.blink_speed = v.max(0.0);
        self.revise_stuff();
    }
    #[func]
    fn set_energy(&mut self, v: f32) {
        self.energy = v.max(0.0);
        self.revise_stuff();
    }
}
