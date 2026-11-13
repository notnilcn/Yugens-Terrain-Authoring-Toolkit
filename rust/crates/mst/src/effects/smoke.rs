//! `PixelSmoke` — a chimney / campfire column of rounded pixel puffs, one-shot `burst()` puffs
//! for build / destroy feedback, and an optional flickering fire light with a pixel-art glow.
//!
//! Puff column after godot-3d-pixel-art by Alan Lawrey (MIT: `place_smoke.gd`); the fire glow is
//! a `PixelLightVolume` (Stable3D stencil point light port); see CREDITS.md.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, MultiMesh, MultiMeshInstance3D, Node, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;

use super::light_volume::PixelLightVolume;
use super::{Rng, weather};
use crate::globals;
use crate::util::*;

#[derive(Clone, Copy)]
struct Puff {
    pos: Vector3,
    vel: Vector3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    seed: f32,
    /// 0 smoke, 1 flame, 2 dust.
    kind: u8,
}

/// Smoke column for chimneys and campfires: rounded, camera-facing pixel puffs rise, grow, drift
/// with the wind (PixelSky's cloud drift, scaled by PixelWeather), lighten from soot to pale
/// smoke and erode from the rim with an ordered dither. Puffs are toon lit by the scene lights,
/// write the metadata pass and snap to the texel lattice. `fire` adds flame puffs at the base
/// and a flickering light with a banded, dithered glow pool (an internal `PixelLightVolume`).
///
/// `burst(position, count)` throws a ring of dust puffs anywhere (build / destroy feedback).
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSmoke {
    /// Emit the column (bursts work either way).
    #[export]
    emitting: bool,
    /// Puffs per second.
    #[export(range = (0.0, 30.0, 0.05))]
    rate: f32,
    /// Seconds a puff lives.
    #[export(range = (0.2, 30.0, 0.05))]
    lifetime: f32,
    /// Rise speed (m/s).
    #[export(range = (0.0, 10.0, 0.01))]
    rise_speed: f32,
    /// Random sideways speed (m/s).
    #[export(range = (0.0, 4.0, 0.01))]
    spread: f32,
    /// Puff radius at birth / death (m).
    #[export(range = (0.02, 4.0, 0.01))]
    start_size: f32,
    #[export(range = (0.02, 8.0, 0.01))]
    end_size: f32,
    /// Take the wind from PixelSky (cloud drift) and PixelWeather.
    #[export]
    use_sky_wind: bool,
    /// Wind (m/s, XZ) when `use_sky_wind` is off, else a multiplier source is the sky.
    #[export]
    wind: Vector2,
    /// Multiplies the wind.
    #[export(range = (0.0, 10.0, 0.01))]
    wind_strength: f32,
    /// Most puffs alive at once (column + bursts).
    #[export(range = (1.0, 1024.0, 1.0))]
    #[var(set = set_max_puffs)]
    max_puffs: i32,
    /// Bumps around each puff's rim (4 = blobby, 16 = fine cauliflower).
    #[export(range = (4.0, 16.0, 1.0))]
    #[var(set = set_detail)]
    detail: i32,
    #[export]
    #[var(set = set_smoke_color)]
    smoke_color: Color,
    #[export]
    #[var(set = set_soot_color)]
    soot_color: Color,
    /// Draw renderer outlines around the puffs (off by default: soft smoke reads better).
    #[export]
    #[var(set = set_outline)]
    outline: bool,

    #[export_group(name = "Fire")]
    /// Flame puffs at the base plus a flickering light.
    #[export]
    fire: bool,
    #[export]
    #[var(set = set_flame_color)]
    flame_color: Color,
    #[export(range = (0.0, 16.0, 0.01))]
    light_energy: f32,
    #[export(range = (0.1, 50.0, 0.1))]
    light_range: f32,
    /// Brightness of the glow pool the fire throws on the ground (0 = none).
    #[export(range = (0.0, 4.0, 0.01))]
    glow_intensity: f32,
    /// Light flicker amount (noise, like a real flame).
    #[export(range = (0.0, 1.0, 0.01))]
    flicker: f32,

    puffs: Vec<Puff>,
    emit_acc: f32,
    flame_acc: f32,
    mmi: Option<Gd<MultiMeshInstance3D>>,
    material: Option<Gd<ShaderMaterial>>,
    light: Option<Gd<PixelLightVolume>>,
    /// Last values pushed to `light` (visible, energy, range, flicker, glow, color).
    light_state: Option<(bool, f32, f32, f32, f32, Color)>,
    rng: Rng,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelSmoke {
    fn init(base: Base<Node3D>) -> Self {
        PixelSmoke {
            emitting: true,
            rate: 7.0,
            lifetime: 9.0,
            rise_speed: 1.1,
            spread: 0.12,
            start_size: 0.16,
            end_size: 1.1,
            use_sky_wind: true,
            wind: Vector2::new(0.4, 0.15),
            wind_strength: 1.0,
            max_puffs: 128,
            detail: 6,
            smoke_color: Color::from_rgb(0.6, 0.6, 0.64),
            soot_color: Color::from_rgb(0.33, 0.32, 0.35),
            outline: false,
            fire: false,
            flame_color: Color::from_rgb(1.0, 0.55, 0.15),
            light_energy: 0.2,
            light_range: 2.8,
            glow_intensity: 0.3,
            flicker: 0.35,
            puffs: Vec::new(),
            emit_acc: 0.0,
            flame_acc: 0.0,
            mmi: None,
            material: None,
            light: None,
            light_state: None,
            rng: Rng(0x5_30CE),
            base,
        }
    }

    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        self.rng = Rng(self.base().instance_id().to_i64() as u64);
        self.build();
        let mut light = PixelLightVolume::new_alloc();
        light.set_name("FireLight");
        light.set_position(Vector3::new(0.0, 0.4, 0.0));
        light.set("halo_size", &0.0f32.to_variant());
        light.set("volume_scale", &1.2f32.to_variant());
        light.set("volume_bands", &3i32.to_variant());
        let mut this = self.to_gd();
        add_internal(&mut this, &light);
        self.light = Some(light);
        self.refresh_pixel_settings();
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f64) {
        let dt = (delta as f32).min(0.1);
        self.simulate(dt);
        self.upload();
        self.update_light();
    }
}

impl PixelSmoke {
    fn build(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        free_node(self.mmi.take());
        // A 2x2 quad: the shader turns it into a camera-facing puff of radius = instance scale.
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(2.0, 2.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        mm.set_instance_count(self.max_puffs.max(1));
        mm.set_visible_instance_count(0);
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name("Puffs");
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Puffs are simulated in world space.
        mi.set_as_top_level(true);
        mi.set_global_transform(Transform3D::IDENTITY);
        if self.material.is_none() {
            self.material = Some(shader_material("puff.gdshader"));
        }
        if let Some(m) = &self.material {
            mi.set_material_override(m);
        }
        let mut this = self.to_gd();
        add_internal(&mut this, &mi);
        self.mmi = Some(mi);
        self.puffs.truncate(self.max_puffs.max(1) as usize);
        self.update_material();
    }

    fn update_material(&mut self) {
        let Some(mut m) = self.material.clone() else { return };
        m.set_shader_parameter("smoke_color", &self.smoke_color.to_variant());
        m.set_shader_parameter("soot_color", &self.soot_color.to_variant());
        m.set_shader_parameter("flame_color", &self.flame_color.to_variant());
        m.set_shader_parameter("flame_core", &self.flame_color.lerp(Color::from_rgb(1.0, 0.95, 0.6), 0.6).to_variant());
        m.set_shader_parameter("lumps", &(self.detail as f32).to_variant());
        m.set_shader_parameter("pt_flags", &object_flags(1, self.outline, 0, true).to_variant());
    }

    /// World wind for the puffs (m/s).
    fn wind_now(&self) -> Vector3 {
        let mut w = self.wind;
        if self.use_sky_wind
            && let Some(sky) = self.find_sky()
        {
            // The sky's wind is in cloud-noise units; dividing by the noise scale gives the
            // speed cloud shadows move at in m/s, so smoke drifts with the cloud shadows.
            let sw = sky.get("wind").try_to::<Vector2>().unwrap_or(Vector2::ZERO);
            let sc = sky.get("cloud_scale").try_to::<f32>().unwrap_or(0.02).max(1e-3);
            w = sw / sc;
        }
        if let Some(m) = weather::mix() {
            w *= m.p.wind_scale;
        }
        let w = w * self.wind_strength;
        Vector3::new(w.x, 0.0, w.y)
    }

    fn find_sky(&self) -> Option<Gd<Node>> {
        if !self.base().is_inside_tree() {
            return None;
        }
        let tree = self.base().get_tree();
        let mut nodes = tree.get_nodes_in_group("__pixel_sky");
        nodes.pop().or_else(|| {
            let parent = self.base().get_parent()?;
            parent.get_children().iter_shared().find(|n| n.is_class("PixelSky"))
        })
    }

    fn spawn(&mut self, pos: Vector3, vel: Vector3, life: f32, size0: f32, size1: f32, kind: u8) {
        if self.puffs.len() >= self.max_puffs.max(1) as usize {
            // Recycle the oldest puff.
            if let Some((i, _)) = self.puffs.iter().enumerate().max_by(|a, b| (a.1.age / a.1.life).total_cmp(&(b.1.age / b.1.life))) {
                self.puffs.swap_remove(i);
            }
        }
        let seed = self.rng.next_f32();
        self.puffs.push(Puff { pos, vel, age: 0.0, life, size0, size1, seed, kind });
    }

    fn simulate(&mut self, dt: f32) {
        let origin = self.base().get_global_position();
        if self.emitting && self.rate > 0.0 {
            self.emit_acc += dt * self.rate;
            while self.emit_acc >= 1.0 {
                self.emit_acc -= 1.0;
                let side = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.spread;
                let life = self.lifetime * (0.8 + 0.4 * self.rng.next_f32());
                let y0 = if self.fire { 0.45 } else { 0.0 };
                let s0 = self.start_size * (0.8 + 0.4 * self.rng.next_f32());
                self.spawn(origin + Vector3::new(0.0, y0, 0.0) + side * 0.5, side + Vector3::UP * self.rise_speed, life, s0, self.end_size, 0);
            }
        }
        if self.emitting && self.fire {
            self.flame_acc += dt * 7.0;
            while self.flame_acc >= 1.0 {
                self.flame_acc -= 1.0;
                let side = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.12;
                let s0 = 0.14 + 0.08 * self.rng.next_f32();
                let life = 0.45 + 0.2 * self.rng.next_f32();
                self.spawn(origin + side, side * 0.5 + Vector3::UP * 1.3, life, s0, 0.04, 1);
            }
        }
        // Strong sky wind would flatten the column; cap the drift relative to the rise.
        let mut wind = self.wind_now();
        let cap = (self.rise_speed * 1.2).max(0.2);
        if wind.length() > cap {
            wind = wind.normalized() * cap;
        }
        for p in &mut self.puffs {
            p.age += dt;
            let k = (p.age / p.life).clamp(0.0, 1.0);
            match p.kind {
                // Smoke: a continuous plume. The rise slows a little and the wind takes over
                // with height, and a slow per-puff sway (stronger as it ages) makes the column
                // meander and widen instead of stacking straight up.
                0 => {
                    let ph = p.age * 0.9 + p.seed * 37.0;
                    let sway = Vector3::new(ph.sin(), 0.0, (ph * 0.8 + 1.7).cos()) * (0.12 + 0.3 * k);
                    let target = Vector3::UP * self.rise_speed * (1.0 - 0.35 * k) + wind * (0.15 + 0.85 * k) + sway;
                    p.vel += (target - p.vel) * (1.5 * dt).min(1.0);
                }
                // Dust: drag, slight lift.
                2 => {
                    p.vel = p.vel * (1.0 - (3.0 * dt).min(1.0)) + Vector3::UP * 0.6 * dt + wind * dt * 0.5;
                }
                _ => {}
            }
            p.pos += p.vel * dt;
        }
        self.puffs.retain(|p| p.age < p.life);
    }

    fn upload(&mut self) {
        let Some(mi) = self.mmi.clone().filter(|m| m.is_instance_valid()) else { return };
        let Some(mut mm) = mi.get_multimesh() else { return };
        let n = self.puffs.len().min(mm.get_instance_count() as usize);
        let mut lo = Vector3::splat(f32::MAX);
        let mut hi = Vector3::splat(f32::MIN);
        for (i, p) in self.puffs.iter().take(n).enumerate() {
            let k = (p.age / p.life).clamp(0.0, 1.0);
            // Smoke grows steadily so the plume widens with height; flames shrink fast.
            let ease = if p.kind == 0 { k.sqrt() * 0.6 + k * 0.4 } else { 1.0 - (1.0 - k) * (1.0 - k) };
            let s = p.size0 + (p.size1 - p.size0) * ease;
            mm.set_instance_transform(i as i32, Transform3D::new(Basis::from_scale(Vector3::splat(s)), p.pos));
            mm.set_instance_custom_data(i as i32, Color::from_rgba(k, p.seed, p.kind as f32, 0.0));
            lo = lo.coord_min(p.pos - Vector3::splat(s));
            hi = hi.coord_max(p.pos + Vector3::splat(s));
        }
        mm.set_visible_instance_count(n as i32);
        if n > 0 {
            mm.set_custom_aabb(Aabb::new(lo, hi - lo));
        }
    }

    fn update_light(&mut self) {
        let fire = self.fire && self.emitting;
        let state = (fire, self.light_energy, self.light_range, self.flicker, self.glow_intensity, self.flame_color);
        if self.light_state == Some(state) {
            return;
        }
        let Some(l) = &mut self.light else { return };
        // The volume flickers itself (light in game, glow in the shader).
        l.set_visible(fire);
        l.set("energy", &state.1.to_variant());
        l.set("radius", &state.2.to_variant());
        l.set("flicker", &state.3.to_variant());
        l.set("volume_intensity", &state.4.to_variant());
        l.set("light_color", &state.5.to_variant());
        self.light_state = Some(state);
    }
}

#[godot_api]
impl PixelSmoke {
    /// Throws a ring of `count` dust puffs at a world position (build / destroy feedback).
    #[func]
    fn burst(&mut self, position: Vector3, count: i32) {
        let n = count.clamp(1, 64);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + self.rng.next_f32() * 0.4;
            let dir = Vector3::new(a.cos(), 0.15 + 0.3 * self.rng.next_f32(), a.sin());
            let speed = 1.5 + self.rng.next_f32() * 1.5;
            let s0 = self.start_size * (1.0 + self.rng.next_f32());
            let life = 0.9 + 0.5 * self.rng.next_f32();
            self.spawn(position + dir * 0.2, dir * speed, life, s0, self.end_size * 0.8, 2);
        }
    }

    /// Number of live puffs.
    #[func]
    fn get_puff_count(&self) -> i32 {
        self.puffs.len() as i32
    }

    /// Re-applies the metadata layer (called when the meta layer changes).
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = globals::meta_mask();
        if let Some(m) = &mut self.mmi {
            let layers = m.get_layer_mask() | mask;
            m.set_layer_mask(layers);
        }
    }

    #[func]
    fn set_max_puffs(&mut self, v: i32) {
        self.max_puffs = v.clamp(1, 1024);
        self.build();
        self.refresh_pixel_settings();
    }
    #[func]
    fn set_detail(&mut self, v: i32) {
        self.detail = v.clamp(4, 16);
        self.update_material();
    }
    #[func]
    fn set_smoke_color(&mut self, v: Color) {
        self.smoke_color = v;
        self.update_material();
    }
    #[func]
    fn set_soot_color(&mut self, v: Color) {
        self.soot_color = v;
        self.update_material();
    }
    #[func]
    fn set_flame_color(&mut self, v: Color) {
        self.flame_color = v;
        self.update_material();
    }
    #[func]
    fn set_outline(&mut self, v: bool) {
        self.outline = v;
        self.update_material();
    }
}
