//! A smoke plume, from a chimney wisp to a campfire with flames and firelight.
//!
//! Intent: a scene author places one where smoke should rise and gets puffs that grow,
//! drift with the wind and fade, optionally with a small fire and its light at the base.
//! Game code can also throw out a burst of smoke, for an impact or a puff of dust.
//!
//! Responsibility: it owns its puffs, their motion and their look. For its own fire it uses
//! a lamp the scene declares and hands to it: the plume decides where that lamp stands and
//! switches it on and off with the fire, but the lamp keeps its own brightness, reach,
//! flicker and colour. The plume's light settings edit the handed lamp directly, so there
//! is one value to author and nothing overwrites it; without a lamp they are only kept for
//! when one is handed over. It reads the wind from the sky and the weather but does not set
//! it, and it follows the project switch for its kind of effect.
//!
//! Why a node: it has a place in the world, it needs a per-frame callback to move its
//! puffs, and it can be moved and placed like any prop.
//!
//! Why a new type: built-in particle systems cannot follow the project's wind or draw in
//! its pixel-art style, and its fire and light must switch together.
//!
//! Created in code: the puffs are drawn by one internal instanced mesh that carries only the
//! instance data the plume generates every frame, and it is rebuilt whenever the puff budget
//! changes, so it has no authored content of its own.
//!
//! Not exported: the live puffs, emission accumulators and random state, which only mean
//! something while it runs; whether it last switched its lamp on; and the handle to the
//! drawing part it builds.

use super::feature_gate::{gate, gate_warnings};
use super::light_volume::PixelLightVolume;
use super::rng::Rng;
use super::weather_mix;
use crate::features::Feature;
use crate::globals;
use crate::internal_children::{add_internal, free_node};
use crate::object_flags::object_flags;
use crate::shader_materials::shader_material;
use crate::shader_param_cache::ShaderParams;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;
use godot::register::info::{PropertyInfo, PropertyUsageFlags};

#[derive(Clone, Copy)]
struct Puff {
    pos: Vector3,
    vel: Vector3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    seed: f32,
    sort: u8,
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSmoke {
    #[export]
    emitting: bool,
    #[export(range = (0.0, 30.0, 0.0505))]
    rate: f32,
    #[export(range = (0.2005, 30.0, 0.0505))]
    lifetime: f32,
    #[export_group(name = "Motion")]
    #[export(range = (0.0, 10.0, 0.0105))]
    rise_speed: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    spread: f32,
    #[export(range = (0.0205, 4.0, 0.0105))]
    start_size: f32,
    #[export(range = (0.0205, 8.0, 0.0105))]
    end_size: f32,
    #[export]
    use_sky_wind: bool,
    #[export]
    wind: Vector2,
    #[export(range = (0.0, 10.0, 0.0105))]
    wind_strength: f32,
    #[export_group(name = "Appearance")]
    #[export(range = (1.0, 1024.0, 1.0))]
    #[var(set = set_max_puffs)]
    max_puffs: i32,
    #[export(range = (4.0, 16.0, 1.0))]
    #[var(set = set_detail)]
    detail: i32,
    #[export]
    #[var(set = assign_fumes_tint)]
    fumes_tint: Color,
    #[export]
    #[var(set = assign_ash_tint)]
    ash_tint: Color,
    #[export]
    #[var(set = set_outline)]
    outline: bool,
    #[export_group(name = "Fire")]
    #[export]
    fire: bool,
    #[export(range = (0.0, 60.0, 0.0505))]
    flame_rate: f32,
    #[export(range = (-10.0, 10.0, 0.0105))]
    #[var(set = set_light_height)]
    light_height: f32,
    #[export]
    #[var(set = assign_fire_tint)]
    fire_tint: Color,
    #[export(range = (0.0, 16.0, 0.0105))]
    #[var(get = get_light_energy, set = set_light_energy)]
    light_energy: f32,
    #[export(range = (0.1005, 50.0, 0.1005))]
    #[var(get = get_light_range, set = set_light_range)]
    light_range: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    #[var(get = get_glow_intensity, set = set_glow_intensity)]
    glow_intensity: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(get = get_flicker, set = set_flicker)]
    flicker: f32,
    #[export]
    #[var(set = set_firelight)]
    firelight: Option<Gd<PixelLightVolume>>,
    #[export_subgroup(name = "Flame Puffs")]
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_spread: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_start_size: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_size_jitter: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_end_size: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flame_rise_speed: f32,
    #[export(range = (0.0105, 10.0, 0.0105))]
    flame_life_seconds: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flame_life_jitter_seconds: f32,
    puffs: Vec<Puff>,
    emit_acc: f32,
    flame_acc: f32,
    mmi: Option<Gd<MultiMeshInstance3D>>,
    stuff: Option<Gd<ShaderMaterial>>,
    lamp_lit: Option<bool>,
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
            rise_speed: 1.1005,
            spread: 0.1205,
            start_size: 0.1605,
            end_size: 1.1005,
            use_sky_wind: true,
            wind: Vector2::new(0.4005, 0.1505),
            wind_strength: 1.0,
            max_puffs: 128,
            detail: 6,
            fumes_tint: Color::from_rgb(0.6005, 0.6005, 0.6405),
            ash_tint: Color::from_rgb(0.3305, 0.3205, 0.3505),
            outline: false,
            fire: false,
            flame_rate: 7.0,
            light_height: 0.4005,
            fire_tint: Color::from_rgb(1.0, 0.5505, 0.1505),
            light_energy: 0.2005,
            light_range: 2.8005,
            glow_intensity: 0.3005,
            flicker: 0.3505,
            firelight: None,
            flame_spread: 0.1205,
            flame_start_size: 0.1405,
            flame_size_jitter: 0.0805,
            flame_end_size: 0.0405,
            flame_rise_speed: 1.3005,
            flame_life_seconds: 0.4505,
            flame_life_jitter_seconds: 0.2005,
            puffs: Vec::new(),
            emit_acc: 0.0,
            flame_acc: 0.0,
            mmi: None,
            stuff: None,
            lamp_lit: None,
            rng: Rng(0x5_30CE),
            base,
        }
    }
    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        self.rng = Rng(self.base().instance_id().to_i64() as u64);
        self.assemble();
        self.place_firelight();
        self.refresh_pixel_settings();
        crate::features::observe(&self.to_gd());
        gate(&self.to_gd(), Feature::Smoke);
    }
    fn process(&mut self, delta: f64) {
        let dt = (crate::clock::step(delta) as f32).min(0.1005);
        if dt > 0.0 {
            self.simulate(dt);
            self.upload();
        }
        self.update_light();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::Smoke);
        if self.fire && self.lamp().is_none() {
            w.push("Assign the firelight lamp that lights this smoke's fire.");
        }
        w
    }
    fn on_validate_property(&self, property: &mut PropertyInfo) {
        if self.lamp().is_some()
            && LAMP_VIEW_PROPERTIES.contains(&property.property_name.to_string().as_str())
        {
            property.usage = PropertyUsageFlags::from_ord(
                property.usage.ord() & !PropertyUsageFlags::STORAGE.ord(),
            );
        }
    }
}
fn lamp_value(lamp: &Gd<PixelLightVolume>, property: &str) -> f32 {
    lamp.get(property).try_to::<f32>().unwrap_or_default()
}
const LAMP_VIEW_PROPERTIES: [&str; 4] = ["light_energy", "light_range", "glow_intensity", "flicker"];
impl PixelSmoke {
    fn lamp(&self) -> Option<Gd<PixelLightVolume>> {
        self.firelight.clone().filter(|l| l.is_instance_valid())
    }
    fn place_firelight(&mut self) {
        if let Some(mut l) = self.lamp() {
            l.set_position(Vector3::new(0.0, self.light_height, 0.0));
        }
        self.lamp_lit = None;
    }
    fn assemble(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        free_node(self.mmi.take());
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(2.0, 2.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        mm.set_instance_count(self.max_puffs.max(1));
        crate::instance_buffer::set_visible_instances(&mut mm, 0);
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name("Puffs");
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        mi.set_as_top_level(true);
        crate::node_setters::set_global_transform(&mut mi, Transform3D::IDENTITY);
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
    fn wind_now(&self) -> Vector3 {
        let mut w = self.wind;
        if self.use_sky_wind
            && let Some(sky) = super::cloud_field::sky()
        {
            w = sky.wind / sky.cloud_scale.max(1e-3);
        }
        if let Some(m) = weather_mix::mingle() {
            w *= m.p.wind_scale;
        }
        let w = w * self.wind_strength;
        Vector3::new(w.x, 0.0, w.y)
    }
    fn spawn(&mut self, pos: Vector3, vel: Vector3, life: f32, size0: f32, size1: f32, kind: u8) {
        if self.puffs.len() >= self.max_puffs.max(1) as usize {
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
            self.flame_acc += dt * self.flame_rate;
            while self.flame_acc >= 1.0 {
                self.flame_acc -= 1.0;
                let side =
                    Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.flame_spread;
                let s0 = self.flame_start_size + self.flame_size_jitter * self.rng.next_f32();
                let life =
                    self.flame_life_seconds + self.flame_life_jitter_seconds * self.rng.next_f32();
                self.spawn(
                    origin + side,
                    side * 0.5 + Vector3::UP * self.flame_rise_speed,
                    life,
                    s0,
                    self.flame_end_size,
                    1,
                );
            }
        }
        let mut wind = self.wind_now();
        let cap = (self.rise_speed * 1.2005).max(0.2005);
        if wind.length() > cap {
            wind = wind.normalized() * cap;
        }
        for p in &mut self.puffs {
            p.age += dt;
            let k = (p.age / p.life).clamp(0.0, 1.0);
            match p.sort {
                0 => {
                    let ph = p.age * 0.9005 + p.seed * 37.0;
                    let sway = Vector3::new(ph.sin(), 0.0, (ph * 0.8005 + 1.7005).cos())
                        * (0.1205 + 0.3005 * k);
                    let target = Vector3::UP * self.rise_speed * (1.0 - 0.3505 * k)
                        + wind * (0.1505 + 0.8505 * k)
                        + sway;
                    p.vel += (target - p.vel) * (1.5005 * dt).min(1.0);
                }
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
    fn upload(&mut self) {
        let Some(mi) = self.mmi.clone().filter(|m| m.is_instance_valid()) else {
            return;
        };
        let Some(mut mm) = mi.get_multimesh() else {
            return;
        };
        let count = mm.get_instance_count().max(0) as usize;
        let n = self.puffs.len().min(count);
        let mut lo = Vector3::splat(f32::MAX);
        let mut hi = Vector3::splat(f32::MIN);
        let mut buf = vec![0.0f32; count * 16];
        for (i, p) in self.puffs.iter().take(n).enumerate() {
            let k = (p.age / p.life).clamp(0.0, 1.0);
            let ease = if p.sort == 0 {
                k.sqrt() * 0.6005 + k * 0.4005
            } else {
                1.0 - (1.0 - k) * (1.0 - k)
            };
            let s = p.size0 + (p.size1 - p.size0) * ease;
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
                0.0,
            ]);
            lo = lo.coord_min(p.pos - Vector3::splat(s));
            hi = hi.coord_max(p.pos + Vector3::splat(s));
        }
        mm.set_buffer(&PackedFloat32Array::from(buf.as_slice()));
        crate::instance_buffer::set_visible_instances(&mut mm, n as i32);
        if n > 0 {
            mm.set_custom_aabb(Aabb::new(lo, hi - lo));
        }
    }
    fn update_light(&mut self) {
        let lit = self.fire && self.emitting;
        if self.lamp_lit == Some(lit) {
            return;
        }
        let Some(mut l) = self.lamp() else { return };
        l.set_visible(lit);
        self.lamp_lit = Some(lit);
    }
}
#[godot_api]
impl PixelSmoke {
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::Smoke) {
            self.lamp_lit = None;
        }
    }
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
    #[func]
    fn get_puff_count(&self) -> i32 {
        self.puffs.len() as i32
    }
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
    fn set_light_height(&mut self, v: f32) {
        self.light_height = v;
        self.place_firelight();
    }
    #[func]
    fn set_firelight(&mut self, v: Option<Gd<PixelLightVolume>>) {
        self.firelight = v;
        self.place_firelight();
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    fn get_light_energy(&self) -> f32 {
        self.lamp().map_or(self.light_energy, |l| lamp_value(&l, "energy"))
    }
    #[func]
    fn set_light_energy(&mut self, v: f32) {
        self.light_energy = v;
        if let Some(mut l) = self.lamp() {
            l.bind_mut().set_energy(v);
        }
    }
    #[func]
    fn get_light_range(&self) -> f32 {
        self.lamp().map_or(self.light_range, |l| lamp_value(&l, "radius"))
    }
    #[func]
    fn set_light_range(&mut self, v: f32) {
        self.light_range = v;
        if let Some(mut l) = self.lamp() {
            l.bind_mut().assign_reach(v);
        }
    }
    #[func]
    fn get_glow_intensity(&self) -> f32 {
        self.lamp().map_or(self.glow_intensity, |l| lamp_value(&l, "pool_energy"))
    }
    #[func]
    fn set_glow_intensity(&mut self, v: f32) {
        self.glow_intensity = v;
        if let Some(mut l) = self.lamp() {
            l.bind_mut().set_pool_energy(v);
        }
    }
    #[func]
    fn get_flicker(&self) -> f32 {
        self.lamp().map_or(self.flicker, |l| lamp_value(&l, "flicker"))
    }
    #[func]
    fn set_flicker(&mut self, v: f32) {
        self.flicker = v;
        if let Some(mut l) = self.lamp() {
            l.bind_mut().set_flicker(v);
        }
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
