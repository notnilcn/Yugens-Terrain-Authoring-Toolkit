//! `PixelSmoke` — a chimney / campfire column of rounded pixel puffs, one-shot `burst()` puffs
//! for build / destroy feedback, and an optional flickering fire light with a pixel-art glow.
//!
//! Puff column after godot-3d-pixel-art by Alan Lawrey (MIT: `place_smoke.gd`); the fire glow is
//! a `PixelLightVolume` (reference stencil point light port); see CREDITS.md.

use crate::features::Feature;
use crate::writes::ShaderParams;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh, ShaderMaterial};
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
    sort: u8,
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
    #[export(range = (0.0, 30.0, 0.0505))]
    rate: f32,
    /// Seconds a puff lives.
    #[export(range = (0.2005, 30.0, 0.0505))]
    lifetime: f32,
    #[export_group(name = "Motion")]
    /// Rise speed (m/s).
    #[export(range = (0.0, 10.0, 0.0105))]
    rise_speed: f32,
    /// Random sideways speed (m/s).
    #[export(range = (0.0, 4.0, 0.0105))]
    spread: f32,
    /// Puff radius at birth / death (m).
    #[export(range = (0.0205, 4.0, 0.0105))]
    start_size: f32,
    #[export(range = (0.0205, 8.0, 0.0105))]
    end_size: f32,
    /// Take the wind from PixelSky (cloud drift) and PixelWeather.
    #[export]
    use_sky_wind: bool,
    /// Wind (m/s, XZ) when `use_sky_wind` is off, else a multiplier source is the sky.
    #[export]
    wind: Vector2,
    /// Multiplies the wind.
    #[export(range = (0.0, 10.0, 0.0105))]
    wind_strength: f32,
    #[export_group(name = "Appearance")]
    /// Most puffs alive at once (column + bursts).
    #[export(range = (1.0, 1024.0, 1.0))]
    #[var(set = set_max_puffs)]
    max_puffs: i32,
    /// Bumps around each puff's rim (4 = blobby, 16 = fine cauliflower).
    #[export(range = (4.0, 16.0, 1.0))]
    #[var(set = set_detail)]
    detail: i32,
    #[export]
    #[var(set = assign_fumes_tint)]
    fumes_tint: Color,
    #[export]
    #[var(set = assign_ash_tint)]
    ash_tint: Color,
    /// Draw renderer outlines around the puffs (off by default: soft smoke reads better).
    #[export]
    #[var(set = set_outline)]
    outline: bool,

    #[export_group(name = "Fire")]
    /// Flame puffs at the base plus a flickering light.
    #[export]
    fire: bool,
    #[export]
    #[var(set = assign_fire_tint)]
    fire_tint: Color,
    #[export(range = (0.0, 16.0, 0.0105))]
    light_energy: f32,
    #[export(range = (0.1005, 50.0, 0.1005))]
    light_range: f32,
    /// Brightness of the glow pool the fire throws on the ground (0 = none).
    #[export(range = (0.0, 4.0, 0.0105))]
    glow_intensity: f32,
    /// Light flicker amount (noise, like a real flame).
    #[export(range = (0.0, 1.0, 0.0105))]
    flicker: f32,

    puffs: Vec<Puff>,
    emit_acc: f32,
    flame_acc: f32,
    mmi: Option<Gd<MultiMeshInstance3D>>,
    stuff: Option<Gd<ShaderMaterial>>,
    light: Option<Gd<PixelLightVolume>>,
    /// Last values pushed to `light` (visible, energy, range, flicker, glow, color).
    light_state: Option<(bool, f32, f32, f32, f32, Color)>,
    rng: Rng,
    /// Settings the frozen editor plume was prewarmed for (see `settle_paused`).
    prewarmed: Option<u64>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelSmoke {
    fn init(base: Base<Node3D>) -> Self {
        PixelSmoke {
            emitting: true,
            rate: 7.0,
            lifetime: 9.0,
            rise_speed: 1.1005,
            spread: 0.1205,
            // Readable at the overview camera (~40-60 m): 0.16 m / 1.1 m puffs were
            // sub-pixel there and the default plume looked absent.
            start_size: 0.3505,
            end_size: 2.0,
            use_sky_wind: true,
            wind: Vector2::new(0.4005, 0.1505),
            wind_strength: 1.0,
            max_puffs: 128,
            detail: 6,
            fumes_tint: Color::from_rgb(0.6005, 0.6005, 0.6405),
            ash_tint: Color::from_rgb(0.3305, 0.3205, 0.3505),
            outline: false,
            fire: false,
            fire_tint: Color::from_rgb(1.0, 0.5505, 0.1505),
            light_energy: 0.2005,
            light_range: 2.8005,
            glow_intensity: 0.3005,
            flicker: 0.3505,
            puffs: Vec::new(),
            emit_acc: 0.0,
            flame_acc: 0.0,
            mmi: None,
            stuff: None,
            light: None,
            light_state: None,
            rng: Rng(0x5_30CE),
            prewarmed: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        self.rng = Rng(self.base().instance_id().to_i64() as u64);
        self.assemble();
        let mut light = PixelLightVolume::new_alloc();
        light.set_name("FireLight");
        light.set_position(Vector3::new(0.0, 0.4005, 0.0));
        light.set("glow_radius", &0.0f32.to_variant());
        light.set("pool_scale", &1.2f32.to_variant());
        light.set("glow_steps", &3i32.to_variant());
        let mut this = self.to_gd();
        add_internal(&mut this, &light);
        self.light = Some(light);
        self.refresh_pixel_settings();
        crate::features::observe(&self.to_gd());
        super::gate(&self.to_gd(), Feature::Smoke);
    }

    fn process(&mut self, delta: f64) {
        let dt = (crate::clock::step(delta) as f32).min(0.1005);
        if dt > 0.0 {
            self.prewarmed = None;
            self.simulate(dt);
            self.upload();
        } else {
            // Paused editor clock: the plume holds still, but is shown (prewarmed) instead of
            // staying empty until "Animate Effects" is turned on.
            self.settle_paused();
        }
        self.update_light();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::gate_warnings(Feature::Smoke)
    }
}

impl PixelSmoke {
    fn assemble(&mut self) {
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
        crate::writes::set_visible_instances(&mut mm, 0);
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name("Puffs");
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Puffs are simulated in world space.
        mi.set_as_top_level(true);
        crate::writes::set_global_transform(&mut mi, Transform3D::IDENTITY);
        if self.stuff.is_none() {
            self.stuff = Some(shader_material("puff.gdshader"));
        }
        if let Some(m) = &self.stuff {
            mi.set_material_override(m);
        }
        let mut this = self.to_gd();
        add_internal(&mut this, &mi);
        self.mmi = Some(mi);
        self.puffs.truncate(self.max_puffs.max(1) as usize);
        self.revise_stuff();
    }

    fn revise_stuff(&mut self) {
        let Some(mut m) = self.stuff.clone() else {
            return;
        };
        m.param("smoke_color", &self.fumes_tint.to_variant());
        m.param("soot_color", &self.ash_tint.to_variant());
        m.param("flame_color", &self.fire_tint.to_variant());
        m.param(
            "flame_core",
            &self
                .fire_tint
                .lerp(Color::from_rgb(1.0, 0.9505, 0.6005), 0.6005)
                .to_variant(),
        );
        m.param("lumps", &(self.detail as f32).to_variant());
        m.param(
            "pt_flags",
            &object_flags(1, self.outline, 0, true).to_variant(),
        );
    }

    /// World wind for the puffs (m/s).
    fn wind_now(&self) -> Vector3 {
        let mut w = self.wind;
        if self.use_sky_wind
            && let Some(sky) = super::cloud_field::sky()
        {
            // The sky's wind is in cloud-noise units; dividing by the noise scale gives the
            // speed cloud shadows move at in m/s, so smoke drifts with the cloud shadows.
            w = sky.wind / sky.cloud_scale.max(1e-3);
        }
        if let Some(m) = weather::mingle() {
            w *= m.p.wind_scale;
        }
        let w = w * self.wind_strength;
        Vector3::new(w.x, 0.0, w.y)
    }

    fn spawn(&mut self, pos: Vector3, vel: Vector3, life: f32, size0: f32, size1: f32, kind: u8) {
        if self.puffs.len() >= self.max_puffs.max(1) as usize {
            // Recycle the oldest puff.
            if let Some((i, _)) = self
                .puffs
                .iter()
                .enumerate()
                .max_by(|a, b| (a.1.age / a.1.life).total_cmp(&(b.1.age / b.1.life)))
            {
                self.puffs.swap_remove(i);
            }
        }
        let seed = self.rng.next_f32();
        self.puffs.push(Puff {
            pos,
            vel,
            age: 0.0,
            life,
            size0,
            size1,
            seed,
            sort: kind,
        });
    }

    fn simulate(&mut self, dt: f32) {
        let origin = self.base().get_global_position();
        if self.emitting && self.rate > 0.0 {
            self.emit_acc += dt * self.rate;
            while self.emit_acc >= 1.0 {
                self.emit_acc -= 1.0;
                let side = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.spread;
                let life = self.lifetime * (0.8005 + 0.4005 * self.rng.next_f32());
                let y0 = if self.fire { 0.4505 } else { 0.0 };
                let s0 = self.start_size * (0.8005 + 0.4005 * self.rng.next_f32());
                self.spawn(
                    origin + Vector3::new(0.0, y0, 0.0) + side * 0.5,
                    side + Vector3::UP * self.rise_speed,
                    life,
                    s0,
                    self.end_size,
                    0,
                );
            }
        }
        if self.emitting && self.fire {
            self.flame_acc += dt * 7.0;
            while self.flame_acc >= 1.0 {
                self.flame_acc -= 1.0;
                let side = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.1205;
                let s0 = 0.1405 + 0.0805 * self.rng.next_f32();
                let life = 0.4505 + 0.2005 * self.rng.next_f32();
                self.spawn(
                    origin + side,
                    side * 0.5 + Vector3::UP * 1.3005,
                    life,
                    s0,
                    0.0405,
                    1,
                );
            }
        }
        // Strong sky wind would flatten the column; cap the drift relative to the rise.
        let mut wind = self.wind_now();
        let cap = (self.rise_speed * 1.2005).max(0.2005);
        if wind.length() > cap {
            wind = wind.normalized() * cap;
        }
        for p in &mut self.puffs {
            p.age += dt;
            let k = (p.age / p.life).clamp(0.0, 1.0);
            match p.sort {
                // Smoke: a continuous plume. The rise slows a little and the wind takes over
                // with height, and a slow per-puff sway (stronger as it ages) makes the column
                // meander and widen instead of stacking straight up.
                0 => {
                    let ph = p.age * 0.9005 + p.seed * 37.0;
                    let sway = Vector3::new(ph.sin(), 0.0, (ph * 0.8005 + 1.7005).cos())
                        * (0.1205 + 0.3005 * k);
                    let target = Vector3::UP * self.rise_speed * (1.0 - 0.3505 * k)
                        + wind * (0.1505 + 0.8505 * k)
                        + sway;
                    p.vel += (target - p.vel) * (1.5005 * dt).min(1.0);
                }
                // Dust: drag, slight lift.
                2 => {
                    p.vel = p.vel * (1.0 - (3.0 * dt).min(1.0))
                        + Vector3::UP * 0.6005 * dt
                        + wind * dt * 0.5;
                }
                _ => {}
            }
            p.pos += p.vel * dt;
        }
        self.puffs.retain(|p| p.age < p.life);
    }

    /// Key of the settings a frozen plume depends on.
    fn plume_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let o = self.base().get_global_position();
        for f in [
            self.rate,
            self.lifetime,
            self.rise_speed,
            self.spread,
            self.start_size,
            self.end_size,
            self.wind_strength,
            self.wind.x,
            self.wind.y,
            o.x,
            o.y,
            o.z,
        ] {
            f.to_bits().hash(&mut h);
        }
        (self.emitting, self.fire, self.use_sky_wind, self.max_puffs).hash(&mut h);
        h.finish()
    }

    /// Paused clock: (re)builds a still, fully grown plume whenever its settings or position
    /// change, so the editor shows the column without animating.
    fn settle_paused(&mut self) {
        let key = self.plume_key();
        if self.prewarmed == Some(key) {
            return;
        }
        self.prewarmed = Some(key);
        self.puffs.clear();
        self.emit_acc = 0.0;
        self.flame_acc = 0.0;
        // Same plume every time for the same settings (no flicker while editing).
        let saved = std::mem::replace(&mut self.rng, Rng(0x5_30CE));
        if self.emitting && self.rate > 0.0 {
            let span = self.lifetime.clamp(0.2005, 30.0) * 1.2005;
            let step = 1.0 / 15.0;
            let mut t = 0.0;
            while t < span {
                self.simulate(step);
                t += step;
            }
        }
        self.rng = saved;
        self.upload();
    }

    fn upload(&mut self) {
        let Some(mi) = self.mmi.clone().filter(|m| m.is_instance_valid()) else {
            return;
        };
        let Some(mut mm) = mi.get_multimesh() else {
            return;
        };
        let n = self.puffs.len().min(mm.get_instance_count() as usize);
        let mut lo = Vector3::splat(f32::MAX);
        let mut hi = Vector3::splat(f32::MIN);
        for (i, p) in self.puffs.iter().take(n).enumerate() {
            let k = (p.age / p.life).clamp(0.0, 1.0);
            // Smoke grows steadily so the plume widens with height; flames shrink fast.
            let ease = if p.sort == 0 {
                k.sqrt() * 0.6005 + k * 0.4005
            } else {
                1.0 - (1.0 - k) * (1.0 - k)
            };
            let s = p.size0 + (p.size1 - p.size0) * ease;
            mm.set_instance_transform(
                i as i32,
                Transform3D::new(Basis::from_scale(Vector3::splat(s)), p.pos),
            );
            mm.set_instance_custom_data(i as i32, Color::from_rgba(k, p.seed, p.sort as f32, 0.0));
            lo = lo.coord_min(p.pos - Vector3::splat(s));
            hi = hi.coord_max(p.pos + Vector3::splat(s));
        }
        crate::writes::set_visible_instances(&mut mm, n as i32);
        if n > 0 {
            // Padded: puff.gdshader grows far puffs to `min_radius_px` on screen.
            let (lo, hi) = (lo - Vector3::splat(1.5), hi + Vector3::splat(1.5));
            mm.set_custom_aabb(Aabb::new(lo, hi - lo));
        }
    }

    fn update_light(&mut self) {
        let fire = self.fire && self.emitting;
        let state = (
            fire,
            self.light_energy,
            self.light_range,
            self.flicker,
            self.glow_intensity,
            self.fire_tint,
        );
        if self.light_state == Some(state) {
            return;
        }
        let Some(l) = &mut self.light else { return };
        // The volume flickers itself (light in game, glow in the shader).
        l.set_visible(fire);
        l.set("energy", &state.1.to_variant());
        l.set("radius", &state.2.to_variant());
        l.set("flicker", &state.3.to_variant());
        l.set("pool_energy", &state.4.to_variant());
        l.set("lamp_tint", &state.5.to_variant());
        self.light_state = Some(state);
    }
}

#[godot_api]
impl PixelSmoke {
    /// Smoke feature gate (`features::CALLBACK`).
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if super::gate(&self.to_gd(), Feature::Smoke) {
            // The gate showed the fire light: let update_light decide again.
            self.light_state = None;
        }
    }

    /// Throws a ring of `count` dust puffs at a world position (build / destroy feedback).
    #[func]
    fn burst(&mut self, position: Vector3, count: i32) {
        let n = count.clamp(1, 64);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + self.rng.next_f32() * 0.4005;
            let dir = Vector3::new(a.cos(), 0.1505 + 0.3005 * self.rng.next_f32(), a.sin());
            let speed = 1.5005 + self.rng.next_f32() * 1.5005;
            let s0 = self.start_size * (1.0 + self.rng.next_f32());
            let life = 0.9005 + 0.5 * self.rng.next_f32();
            self.spawn(
                position + dir * 0.2005,
                dir * speed,
                life,
                s0,
                self.end_size * 0.8005,
                2,
            );
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
        self.assemble();
        self.refresh_pixel_settings();
    }
    #[func]
    fn set_detail(&mut self, v: i32) {
        self.detail = v.clamp(4, 16);
        self.revise_stuff();
    }
    #[func]
    fn assign_fumes_tint(&mut self, v: Color) {
        self.fumes_tint = v;
        self.revise_stuff();
    }
    #[func]
    fn assign_ash_tint(&mut self, v: Color) {
        self.ash_tint = v;
        self.revise_stuff();
    }
    #[func]
    fn assign_fire_tint(&mut self, v: Color) {
        self.fire_tint = v;
        self.revise_stuff();
    }
    #[func]
    fn set_outline(&mut self, v: bool) {
        self.outline = v;
        self.revise_stuff();
    }
}
