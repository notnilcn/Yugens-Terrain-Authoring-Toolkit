//! `PixelFireflies` — a swarm of blinking, wandering glow sprites.
//!
//! `PixelFireflies` is a plain `Node3D` that owns its `MultiMeshInstance3D` as an *internal*
//! child (`Swarm`), so the inspector shows only firefly settings and Godot's *MultiMesh* menu
//! does not appear. Scenes saved while it was a `MultiMeshInstance3D` still load: the old
//! `multimesh` / `material_override` / `cast_shadow` / … entries are accepted and dropped
//! (`on_set`; the node rebuilds its swarm and material itself), `layers` is kept.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;

use crate::globals;
use super::Rng;
use crate::util::*;

/// Fireflies: additive billboards that wander and blink inside a box. By default they fade
/// in at night (driven by `PixelDayNight` through `pt_night`). Color pass only: they are
/// not pixelized by the metadata pass and cast no shadows.
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelFireflies {
    /// Number of fireflies.
    #[export(range = (0.0, 4096.0, 1.0, or_greater))]
    #[var(set = set_count)]
    count: i32,
    /// Half-size of the box (centred on the node) the fireflies are scattered in.
    #[export]
    #[var(set = set_extents)]
    extents: Vector3,
    /// Random seed for the placement.
    #[export]
    #[var(set = set_seed)]
    seed: i32,
    #[export]
    #[var(set = set_color)]
    color: Color,
    /// Sprite size in world units.
    #[export(range = (0.01, 2.0, 0.01))]
    #[var(set = set_size)]
    size: f32,
    /// 1 = only visible at night, 0 = always visible.
    #[export(range = (0.0, 1.0, 0.01))]
    #[var(set = set_night_only)]
    night_only: f32,

    /// How far each firefly drifts from its home position.
    #[export(range = (0.0, 4.0, 0.01))]
    #[var(set = set_wander)]
    wander: f32,

    #[export(range = (0.0, 10.0, 0.01))]
    #[var(set = set_blink_speed)]
    blink_speed: f32,

    /// Glow brightness (additive).
    #[export(range = (0.0, 8.0, 0.01))]
    #[var(set = set_energy)]
    energy: f32,
    /// Render layers of the swarm (never the pixel renderer's metadata layer: fireflies are
    /// color-pass only). Same name as the old MultiMeshInstance3D property.
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,

    material: Option<Gd<ShaderMaterial>>,
    /// Internal swarm node (never saved).
    swarm: Option<Gd<MultiMeshInstance3D>>,
    base: Base<Node3D>,
}

/// Properties of the old `MultiMeshInstance3D`-based nodes (`PixelFireflies`, `PixelBirds`)
/// still found in saved scenes; the nodes rebuild their MultiMesh and material themselves, so
/// these are accepted and ignored.
pub(crate) const LEGACY_MULTIMESH_PROPS: &[&str] = &[
    "multimesh",
    "mesh",
    "material_override",
    "material_overlay",
    "cast_shadow",
    "gi_mode",
    "gi_lightmap_scale",
    "gi_lightmap_texel_scale",
    "visibility_range_begin",
    "visibility_range_begin_margin",
    "visibility_range_end",
    "visibility_range_end_margin",
    "visibility_range_fade_mode",
    "lod_bias",
    "extra_cull_margin",
    "custom_aabb",
    "sorting_offset",
    "sorting_use_aabb_center",
    "ignore_occlusion_culling",
    "transparency",
];

/// `on_set` helper: is `name` an old MultiMeshInstance3D property to drop?
pub(crate) fn is_legacy_multimesh_prop(name: &str) -> bool {
    LEGACY_MULTIMESH_PROPS.contains(&name) || name.starts_with("instance_shader_parameters/")
}

/// How far the shader lets a firefly drift from its spawn point.
const WANDER: f32 = 0.6;

#[godot_api]
impl INode3D for PixelFireflies {
    fn init(base: Base<Node3D>) -> Self {
        PixelFireflies {
            count: 48,
            extents: Vector3::new(8.0, 1.5, 8.0),
            seed: 1,
            color: Color::from_rgb(1.0, 0.9, 0.45),
            size: 0.12,
            night_only: 1.0,
            wander: WANDER,
            blink_speed: 1.3,
            energy: 2.0,
            layers: 1,
            material: None,
            swarm: None,
            base,
        }
    }

    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        // Compatibility with scenes saved when PixelFireflies was a MultiMeshInstance3D.
        is_legacy_multimesh_prop(&property.to_string())
    }

    fn ready(&mut self) {
        self.rebuild();
    }
}

impl PixelFireflies {
    /// The internal swarm node (created on first use).
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

    fn rebuild(&mut self) {
        if !self.base().is_inside_tree() {
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
        for i in 0..n {
            let p = Vector3::new(
                (rng.next_f32() * 2.0 - 1.0) * e.x,
                (rng.next_f32() * 2.0 - 1.0) * e.y,
                (rng.next_f32() * 2.0 - 1.0) * e.z,
            );
            mm.set_instance_transform(i, Transform3D::new(Basis::IDENTITY, p));
            mm.set_instance_custom_data(i, Color::from_rgba(rng.next_f32(), 0.0, 0.0, 0.0));
        }
        // The shader moves vertices, so give culling a box that covers the whole swarm.
        let grow = Vector3::splat(self.wander + self.size);
        mm.set_custom_aabb(Aabb::new(-e - grow, (e + grow) * 2.0));
        self.swarm().set_multimesh(&mm);

        if self.material.is_none() {
            self.material = Some(shader_material("firefly.gdshader"));
        }
        self.update_material();
    }

    fn update_material(&mut self) {
        let Some(mut m) = self.material.clone() else { return };
        m.set_shader_parameter("color", &self.color.to_variant());
        m.set_shader_parameter("size", &self.size.to_variant());
        m.set_shader_parameter("wander", &self.wander.to_variant());
        m.set_shader_parameter("blink_speed", &self.blink_speed.to_variant());
        m.set_shader_parameter("energy", &self.energy.to_variant());
        m.set_shader_parameter("night_only", &self.night_only.to_variant());
        if let Some(s) = &mut self.swarm {
            s.set_material_override(&m);
        }
    }
}

#[godot_api]
impl PixelFireflies {
    /// The internal swarm instance (read-only access for tools and tests).
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
    fn set_count(&mut self, v: i32) {
        self.count = v.max(0);
        self.rebuild();
    }
    #[func]
    fn set_extents(&mut self, v: Vector3) {
        self.extents = v;
        self.rebuild();
    }
    #[func]
    fn set_seed(&mut self, v: i32) {
        self.seed = v;
        self.rebuild();
    }
    #[func]
    fn set_color(&mut self, v: Color) {
        self.color = v;
        self.update_material();
    }
    #[func]
    fn set_size(&mut self, v: f32) {
        self.size = v.max(0.001);
        self.update_material();
    }
    #[func]
    fn set_night_only(&mut self, v: f32) {
        self.night_only = v.clamp(0.0, 1.0);
        self.update_material();
    }
    #[func]
    fn set_wander(&mut self, v: f32) {
        self.wander = v.max(0.0);
        self.rebuild();
    }
    #[func]
    fn set_blink_speed(&mut self, v: f32) {
        self.blink_speed = v.max(0.0);
        self.update_material();
    }
    #[func]
    fn set_energy(&mut self, v: f32) {
        self.energy = v.max(0.0);
        self.update_material();
    }
}
