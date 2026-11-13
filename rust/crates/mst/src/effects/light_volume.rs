//! `PixelLightVolume` — a point light with a visible, banded pixel-art pool of light.
//!
//! The pool/halo shader is a port of Stable3D PixelArt Orthographic's stencil point light
//! (Lorenzo Santa Cruz, MIT); see `light_volume.gdshader` and CREDITS.md.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::light_3d::Param;
use godot::classes::{INode3D, MeshInstance3D, Node3D, OmniLight3D, ShaderMaterial, QuadMesh};
use godot::prelude::*;
use crate::features::Feature;
use crate::writes::ShaderParams;

use crate::globals;
use crate::util::*;

/// A lantern / campfire / crystal light: an OmniLight3D plus an additive glow: a pool of light
/// on whatever surfaces lie inside the glow radius (rebuilt from the depth buffer) and a small
/// halo around the source, both quantized into a few bands with ordered-dither edges so the
/// light reads as pixel-art lamplight. Optional flicker modulates both. The glow brightens at night when a `PixelDayNight` is present. Color pass only
/// (not part of the metadata layer).
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelLightVolume {
    /// Light range in world units.
    #[export(range = (0.1, 100.0, 0.1, or_greater))]
    #[var(set = set_radius)]
    radius: f32,
    #[export]
    #[var(set = set_light_color)]
    light_color: Color,
    /// Light energy. Keep it low: terrain and props shade point lights in hard toon bands, so a
    /// strong light shows as a flat disc; the glow pool carries the visible lamplight.
    #[export(range = (0.0, 16.0, 0.01))]
    #[var(set = set_energy)]
    energy: f32,
    /// Flicker amount for both the light (game only) and the glow (0 = steady).
    #[export(range = (0.0, 1.0, 0.01))]
    #[var(set = set_flicker)]
    flicker: f32,
    /// Let the light cast shadows (costly in the Compatibility renderer).
    #[export]
    #[var(set = set_shadows)]
    shadows: bool,

    #[export_subgroup(name = "Volume", prefix = "volume_")]
    #[export_group(name = "Volume")]
    /// Draw the glow (pool + halo).
    #[export]
    #[var(set = set_volume_enabled)]
    volume_enabled: bool,
    /// Glow radius as a fraction of `radius`. Keep it above 1: the toon-shaded ground band of
    /// the light then sits inside the dithered pool instead of showing as a hard disc edge.
    #[export(range = (0.05, 2.0, 0.01))]
    #[var(set = set_volume_scale)]
    volume_scale: f32,
    /// Pool brightness at the centre (additive; keep modest so the ground color shows).
    #[export(range = (0.0, 4.0, 0.01))]
    #[var(set = set_volume_intensity)]
    volume_intensity: f32,
    /// Source halo radius in world units (0 = no halo).
    #[export_subgroup(name = "Halo", prefix = "halo_")]
    #[export(range = (0.0, 4.0, 0.01))]
    #[var(set = set_halo_size)]
    halo_size: f32,
    /// Source halo brightness.
    #[export(range = (0.0, 4.0, 0.01))]
    #[var(set = set_halo_intensity)]
    halo_intensity: f32,
    /// Number of brightness steps in the glow.
    #[export(range = (1.0, 8.0, 1.0))]
    #[var(set = set_volume_bands)]
    volume_bands: i32,
    /// How much dimmer the glow is in daylight (0 = same day and night).
    #[export(range = (0.0, 1.0, 0.01))]
    #[var(set = set_night_boost)]
    night_boost: f32,

    light: Option<Gd<OmniLight3D>>,
    volume: Option<Gd<MeshInstance3D>>,
    volume_mat: Option<Gd<ShaderMaterial>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelLightVolume {
    fn init(base: Base<Node3D>) -> Self {
        PixelLightVolume {
            radius: 3.0,
            light_color: Color::from_rgb(1.0, 0.72, 0.4),
            energy: 0.15,
            flicker: 0.15,
            shadows: false,
            volume_enabled: true,
            volume_scale: 1.2,
            volume_intensity: 0.45,
            halo_size: 0.45,
            halo_intensity: 0.5,
            volume_bands: 4,
            night_boost: 1.0,
            light: None,
            volume: None,
            volume_mat: None,
            base,
        }
    }

    fn ready(&mut self) {
        // Internal children are owned (and freed) with this node; build them once.
        let mut light = OmniLight3D::new_alloc();
        light.set_name("Light");
        let mut glow = MeshInstance3D::new_alloc();
        glow.set_name("Volume");
        glow.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let layers = glow.get_layer_mask() & !globals::meta_mask();
        glow.set_layer_mask(layers);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(1.0, 1.0));
        glow.set_mesh(&quad);
        let mat = shader_material("light_volume.gdshader");
        glow.set_material_override(&mat);
        let mut this = self.to_gd();
        add_internal(&mut this, &light);
        add_internal(&mut this, &glow);
        self.light = Some(light);
        self.volume = Some(glow);
        self.volume_mat = Some(mat);
        crate::features::watch(&self.to_gd());
        self._on_pixel_features_changed();
    }

    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        // The light flickers with the shared clock (still in a paused editor).
        if self.flicker <= 0.0 || !crate::clock::running() {
            return;
        }
        // Same waveform as light_volume.gdshader, so the light and its glow pulse together.
        let t = crate::clock::now();
        let f = 1.0 - self.flicker * (0.5 + 0.5 * (t * 13.0).sin() * (t * 7.3 + 1.7).sin());
        let e = self.energy * f;
        if let Some(l) = &mut self.light {
            l.set_param(Param::ENERGY, e);
        }
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::gate_warnings(Feature::LightVolumes)
    }
}

impl PixelLightVolume {
    fn volume_radius(&self) -> f32 {
        (self.radius * self.volume_scale).max(0.01)
    }

    /// Pushes every property to the internal light, glow mesh and material.
    fn apply(&mut self) {
        let glow_r = self.volume_radius();
        let cull_r = glow_r.max(self.halo_size);
        if let Some(l) = &mut self.light {
            l.set_param(Param::RANGE, self.radius);
            l.set_param(Param::ENERGY, self.energy);
            l.set_color(self.light_color);
            l.set_shadow(self.shadows);
        }
        if let Some(v) = &mut self.volume {
            // The shader expands the quad into a camera-facing disc, so cull with the glow.
            v.set_custom_aabb(Aabb::new(Vector3::splat(-cull_r), Vector3::splat(2.0 * cull_r)));
            v.set_visible(self.volume_enabled && crate::features::enabled(Feature::LightVolumes));
        }
        if let Some(m) = &mut self.volume_mat {
            m.param("color", &self.light_color.to_variant());
            m.param("radius", &glow_r.to_variant());
            m.param("intensity", &self.volume_intensity.to_variant());
            m.param("halo_size", &self.halo_size.to_variant());
            m.param("halo_intensity", &self.halo_intensity.to_variant());
            m.param("bands", &self.volume_bands.to_variant());
            m.param("flicker", &self.flicker.to_variant());
            m.param("night_boost", &self.night_boost.to_variant());
        }
    }
}

#[godot_api]
impl PixelLightVolume {
    /// Light Volumes feature gate (`features::CALLBACK`).
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if super::gate(&self.to_gd(), Feature::LightVolumes) {
            // The gate showed every internal; the glow follows `volume_enabled` again.
            self.apply();
        }
    }

    #[func]
    fn set_radius(&mut self, v: f32) {
        self.radius = v.max(0.01);
        self.apply();
    }
    #[func]
    fn set_light_color(&mut self, v: Color) {
        self.light_color = v;
        self.apply();
    }
    #[func]
    fn set_energy(&mut self, v: f32) {
        self.energy = v.max(0.0);
        self.apply();
    }
    #[func]
    fn set_flicker(&mut self, v: f32) {
        self.flicker = v.clamp(0.0, 1.0);
        self.apply();
    }
    #[func]
    fn set_shadows(&mut self, v: bool) {
        self.shadows = v;
        self.apply();
    }
    #[func]
    fn set_volume_enabled(&mut self, v: bool) {
        self.volume_enabled = v;
        self.apply();
    }
    #[func]
    fn set_volume_scale(&mut self, v: f32) {
        self.volume_scale = v.max(0.01);
        self.apply();
    }
    #[func]
    fn set_volume_intensity(&mut self, v: f32) {
        self.volume_intensity = v.max(0.0);
        self.apply();
    }
    #[func]
    fn set_halo_size(&mut self, v: f32) {
        self.halo_size = v.max(0.0);
        self.apply();
    }
    #[func]
    fn set_halo_intensity(&mut self, v: f32) {
        self.halo_intensity = v.max(0.0);
        self.apply();
    }
    #[func]
    fn set_volume_bands(&mut self, v: i32) {
        self.volume_bands = v.clamp(1, 8);
        self.apply();
    }
    #[func]
    fn set_night_boost(&mut self, v: f32) {
        self.night_boost = v.clamp(0.0, 1.0);
        self.apply();
    }
}
