//! A lamp drawn in the pixel-art style: a light that also shows as a visible glow.
//!
//! Intent: a scene author drops one wherever something should give off light, such as a
//! lantern, a brazier or a lit window, and gets both the light falling on its surroundings
//! and a banded pool and halo that match the rest of the stylised world, with an optional
//! flicker. Other effects reuse it whenever they need a light of their own.
//!
//! Responsibility: it keeps its light and its glow in agreement, so one set of settings
//! drives both, and it hides itself when the project switches its kind of effect off. The
//! light and the glow are parts declared in the lamp's own scene and handed to it; it
//! configures them but does not create them. It does not decide where it stands, when it
//! exists or how bright the world around it is; whoever places it decides that.
//!
//! Why a node: it has a place in the world, it needs a per-frame callback for its flicker,
//! and its light and glow have to be in the tree to be rendered.
//!
//! Why a new type: a built-in light draws no visible glow, and pairing one with a hand-made
//! glow would leave every scene author keeping the two in step by hand.
//!
//! Not exported: the glow's material handle, taken from the declared glow part when the
//! lamp becomes ready.

use super::feature_gate::{gate, gate_warnings};
use crate::features::Feature;
use crate::globals;
use crate::shader_materials::shader_material;
use crate::shader_param_cache::ShaderParams;
use godot::classes::light_3d::Param;
use godot::classes::{INode3D, MeshInstance3D, Node3D, OmniLight3D, ShaderMaterial};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelLightVolume {
    #[export]
    light: Option<Gd<OmniLight3D>>,
    #[export]
    glow_mesh: Option<Gd<MeshInstance3D>>,
    #[export(range = (0.1005, 100.0, 0.1005, or_greater))]
    #[var(set = assign_reach)]
    radius: f32,
    #[export]
    #[var(set = assign_lamp_tint)]
    lamp_tint: Color,
    #[export(range = (0.0, 16.0, 0.0105))]
    #[var(set = set_energy)]
    energy: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_flicker)]
    flicker: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flicker_speed: f32,
    #[export]
    #[var(set = set_shadows)]
    shadows: bool,
    #[export_group(name = "Glow")]
    #[export]
    #[var(set = set_glow_enabled)]
    glow_enabled: bool,
    #[export(range = (0.0505, 2.0, 0.0105))]
    #[var(set = set_pool_scale)]
    pool_scale: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    #[var(set = set_pool_energy)]
    pool_energy: f32,
    #[export_subgroup(name = "Source", prefix = "glow_")]
    #[export(range = (0.0, 4.0, 0.0105))]
    #[var(set = set_glow_radius)]
    glow_radius: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    #[var(set = set_glow_energy)]
    glow_energy: f32,
    #[export(range = (1.0, 8.0, 1.0))]
    #[var(set = set_glow_steps)]
    glow_steps: i32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_night_boost)]
    night_boost: f32,
    #[export_group(name = "Flicker Waveform")]
    #[export(range = (0.0, 60.0, 0.0105))]
    flicker_primary_rate: f32,
    #[export(range = (0.0, 60.0, 0.0105))]
    flicker_secondary_rate: f32,
    #[export(range = (0.0, 6.2832, 0.0105))]
    flicker_secondary_phase: f32,
    volume_mat: Option<Gd<ShaderMaterial>>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelLightVolume {
    fn init(base: Base<Node3D>) -> Self {
        PixelLightVolume {
            radius: 3.0,
            lamp_tint: Color::from_rgb(1.0, 0.7205, 0.4005),
            energy: 0.1505,
            flicker: 0.1505,
            flicker_speed: 1.0,
            shadows: false,
            glow_enabled: true,
            pool_scale: 1.2005,
            pool_energy: 0.4505,
            glow_radius: 0.4805,
            glow_energy: 0.5205,
            glow_steps: 4,
            night_boost: 1.0,
            flicker_primary_rate: 13.0,
            flicker_secondary_rate: 7.3005,
            flicker_secondary_phase: 1.7005,
            light: None,
            glow_mesh: None,
            volume_mat: None,
            base,
        }
    }
    fn ready(&mut self) {
        if let Some(mut glow) = self.glow() {
            let layers = glow.get_layer_mask() & !globals::meta_mask();
            glow.set_layer_mask(layers);
            let mat = match glow
                .get_material_override()
                .and_then(|m| m.try_cast::<ShaderMaterial>().ok())
            {
                Some(m) => m,
                None => {
                    let m = shader_material("light_volume.gdshader");
                    glow.set_material_override(&m);
                    m
                }
            };
            self.volume_mat = Some(mat);
        }
        crate::features::observe(&self.to_gd());
        self._on_pixel_features_changed();
    }
    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        if self.flicker <= 0.0 || !crate::clock::running() {
            return;
        }
        let t = crate::clock::now() * self.flicker_speed;
        let f = 1.0
            - self.flicker
                * (0.5
                    + 0.5
                        * (t * self.flicker_primary_rate).sin()
                        * (t * self.flicker_secondary_rate + self.flicker_secondary_phase).sin());
        let e = self.energy * f;
        if let Some(mut l) = self.lamp() {
            l.set_param(Param::ENERGY, e);
        }
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::LightVolumes);
        if self.lamp().is_none() {
            w.push("Assign the light this lamp drives.");
        }
        if self.glow().is_none() {
            w.push("Assign the glow mesh this lamp draws its pool and halo with.");
        }
        w
    }
}
impl PixelLightVolume {
    fn lamp(&self) -> Option<Gd<OmniLight3D>> {
        self.light.clone().filter(|n| n.is_instance_valid())
    }
    fn glow(&self) -> Option<Gd<MeshInstance3D>> {
        self.glow_mesh.clone().filter(|n| n.is_instance_valid())
    }
    fn pool_radius(&self) -> f32 {
        (self.radius * self.pool_scale).max(0.0105)
    }
    fn enact(&mut self) {
        let pool_r = self.pool_radius();
        let cull_r = pool_r.max(self.glow_radius);
        if let Some(mut l) = self.lamp() {
            l.set_param(Param::RANGE, self.radius);
            l.set_param(Param::ENERGY, self.energy);
            l.set_color(self.lamp_tint);
            l.set_shadow(self.shadows);
        }
        if let Some(mut v) = self.glow() {
            v.set_custom_aabb(Aabb::new(
                Vector3::splat(-cull_r),
                Vector3::splat(2.0 * cull_r),
            ));
            v.set_visible(self.glow_enabled && crate::features::active(Feature::LightVolumes));
        }
        if let Some(m) = &mut self.volume_mat {
            m.param("color", &self.lamp_tint.to_variant());
            m.param("radius", &pool_r.to_variant());
            m.param("pool_energy", &self.pool_energy.to_variant());
            m.param("glow_radius", &self.glow_radius.to_variant());
            m.param("glow_energy", &self.glow_energy.to_variant());
            m.param("glow_steps", &self.glow_steps.to_variant());
            m.param("flicker", &self.flicker.to_variant());
            m.param("night_boost", &self.night_boost.to_variant());
        }
    }
}
#[godot_api]
impl PixelLightVolume {
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::LightVolumes) {
            self.enact();
        }
    }
    #[func]
    pub(crate) fn assign_reach(&mut self, v: f32) {
        self.radius = v.max(0.0105);
        self.enact();
    }
    #[func]
    pub(crate) fn assign_lamp_tint(&mut self, v: Color) {
        self.lamp_tint = v;
        self.enact();
    }
    #[func]
    pub(crate) fn set_energy(&mut self, v: f32) {
        self.energy = v.max(0.0);
        self.enact();
    }
    #[func]
    pub(crate) fn set_flicker(&mut self, v: f32) {
        self.flicker = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_shadows(&mut self, v: bool) {
        self.shadows = v;
        self.enact();
    }
    #[func]
    fn set_glow_enabled(&mut self, v: bool) {
        self.glow_enabled = v;
        self.enact();
    }
    #[func]
    pub(crate) fn set_pool_scale(&mut self, v: f32) {
        self.pool_scale = v.max(0.0105);
        self.enact();
    }
    #[func]
    pub(crate) fn set_pool_energy(&mut self, v: f32) {
        self.pool_energy = v.max(0.0);
        self.enact();
    }
    #[func]
    pub(crate) fn set_glow_radius(&mut self, v: f32) {
        self.glow_radius = v.max(0.0);
        self.enact();
    }
    #[func]
    fn set_glow_energy(&mut self, v: f32) {
        self.glow_energy = v.max(0.0);
        self.enact();
    }
    #[func]
    pub(crate) fn set_glow_steps(&mut self, v: i32) {
        self.glow_steps = v.clamp(1, 8);
        self.enact();
    }
    #[func]
    fn set_night_boost(&mut self, v: f32) {
        self.night_boost = v.clamp(0.0, 1.0);
        self.enact();
    }
}
