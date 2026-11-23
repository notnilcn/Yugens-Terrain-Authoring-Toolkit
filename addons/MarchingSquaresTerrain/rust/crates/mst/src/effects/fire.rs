//! Wildfire for the terrain: grass that catches, burns, spreads and grows back.
//!
//! Intent: a scene author adds one to a terrain and gets fire as a living part of the
//! world. Game code lights it at a point or on a tile, puts it out, douses it or asks what
//! is burning; the fire then spreads through grass with the wind, is slowed and put out by
//! rain, leaves scorched ground and lets it regrow, and announces when a blaze starts, when
//! it dies out and which tiles caught or went out.
//!
//! Responsibility: it owns the burning state of the ground and how that state changes over
//! time, and it draws the flames, smoke, steam and firelight that go with it. It publishes
//! the burn state for the terrain's shading to read. It does not own the terrain or its
//! grass, it does not decide what burning means for anything standing in it, and it reads
//! wind and rain from the sky and weather effects rather than deciding them.
//!
//! Why a node: it belongs to one terrain in one scene, it needs per-frame callbacks to
//! advance the simulation, it has a place in the world for its visuals, and it reports what
//! happens through signals other nodes connect to.
//!
//! Why a new type: no built-in node simulates spreading fire, and the rules depend on the
//! terrain's own tiles and grass, which only this project knows about.
//!
//! The terrain it burns is handed to it by the scene; it never goes looking for one, and
//! without one it has nothing to burn. Its firelight lamps come from a lamp scene it is
//! handed, instanced at run time because how many blazes burn, and where, is only known once
//! the fire spreads.
//!
//! Not exported: the live burn simulation and its particles, accumulators and random state,
//! which only mean something while the fire runs; the wind, rain and terrain placement it
//! last read; buffers and textures rebuilt from that state; and handles to the visual parts
//! it builds for itself.

use super::feature_gate::{gate, gate_warnings};
use super::fire_lights::{FireLightLook, FireLights};
use super::fire_particles::{Part, PartSet};
use super::rng::Rng;
use super::weather_mix;
use crate::editor_detection::is_editor;
use crate::features::Feature;
use crate::globals;
use crate::math_conversions::{from_iv, to_iv};
use crate::object_flags::object_flags;
use crate::terrain::MarchingSquaresTerrain;
use crate::shader_param_cache::ShaderParams;
use godot::classes::image::Format;
use godot::classes::{INode3D, Image, ImageTexture, Node3D};
use godot::prelude::*;
use pt_core::fire::{FireField, FireParams, StepEvents};
use pt_core::math::{Vec2 as PVec2, v2};

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelFireSystem {
    #[export]
    simulate: bool,
    #[export]
    preview_in_editor: bool,
    #[export(range = (0.1005, 4.0, 0.0105))]
    #[var(set = assign_texture_px_measure)]
    texel_size: f32,
    #[export(range = (1.0, 60.0, 1.0))]
    tick_rate: f32,
    #[export]
    seed: i32,
    #[export]
    terrain: Option<Gd<MarchingSquaresTerrain>>,
    #[export_group(name = "Spread")]
    #[export(range = (0.0, 10.0, 0.0105))]
    spread_rate: f32,
    #[export(range = (0.1005, 60.0, 0.1005))]
    burn_time: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    ignite_time: f32,
    #[export(range = (0.0, 1.0, 0.0015))]
    ember_rate: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    ember_distance: f32,
    #[export(range = (0.0, 20.0, 0.0505))]
    max_step: f32,
    #[export(range = (0.1005, 20.0, 0.1005))]
    ignite_radius: f32,
    #[export]
    ignite_tool: bool,
    #[export]
    #[var(set = set_hearths)]
    hearths: PackedVector3Array,
    #[export(range = (0.1005, 4.0, 0.0105))]
    #[var(set = set_hearth_radius)]
    hearth_radius: f32,
    #[export_group(name = "Wind")]
    #[export]
    use_sky_wind: bool,
    #[export]
    wind: Vector2,
    #[export(range = (0.0, 10.0, 0.0105))]
    wind_strength: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    wind_influence: f32,
    #[export_group(name = "Rain")]
    #[export(range = (-1.0, 1.0, 0.0105))]
    rain_override: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    douse_rate: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    wet_rate: f32,
    #[export(range = (0.1005, 600.0, 0.1005))]
    dry_time: f32,
    #[export(range = (0.0105, 1.0, 0.0105))]
    wet_block: f32,
    #[export_group(name = "Regrowth")]
    #[export(range = (0.0, 600.0, 0.1005))]
    regrow_delay: f32,
    #[export(range = (0.1005, 600.0, 0.1005))]
    regrow_time: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    wet_regrow_boost: f32,
    #[export_group(name = "Look")]
    #[export(range = (0.0, 40.0, 0.1005))]
    flame_rate: f32,
    #[export(range = (0.0205, 4.0, 0.0105))]
    flame_size: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    smoke_rate: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    steam_amount: f32,
    #[export(range = (16.0, 8192.0, 1.0))]
    #[var(set = set_max_flames)]
    max_flames: i32,
    #[export(range = (16.0, 4096.0, 1.0))]
    #[var(set = set_max_smoke)]
    max_smoke: i32,
    #[export]
    #[var(set = assign_inner_tint)]
    inner_tint: Color,
    #[export]
    #[var(set = assign_fire_tint)]
    fire_tint: Color,
    #[export]
    #[var(set = assign_brim_tint)]
    brim_tint: Color,
    #[export]
    #[var(set = assign_fumes_tint)]
    fumes_tint: Color,
    #[export]
    #[var(set = assign_vapor_tint)]
    vapor_tint: Color,
    #[export(range = (0.0, 8.0, 1.0))]
    max_lights: i32,
    #[export(range = (0.0, 16.0, 0.0105))]
    light_energy: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_light_flicker)]
    light_flicker: f32,
    #[export(range = (0.1005, 50.0, 0.1005))]
    light_range: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    glow_intensity: f32,
    #[export]
    firelight_scene: Option<Gd<PackedScene>>,
    #[export_group(name = "Flames")]
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_scatter_texels: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_cool_scale: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_heat_scale: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_random_scale_min: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_random_scale_range: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flame_rise_speed: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flame_rise_jitter: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_wind_lean: f32,
    #[export(range = (0.0105, 10.0, 0.0105))]
    flame_life_seconds: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    flame_life_jitter_seconds: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    flame_end_scale: f32,
    #[export_group(name = "Sparks")]
    #[export(range = (0.0, 1.0, 0.0015))]
    spark_chance_per_heat: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    spark_spread_speed: f32,
    #[export(range = (0.0, 20.0, 0.0105))]
    spark_rise_speed: f32,
    #[export(range = (0.0, 20.0, 0.0105))]
    spark_rise_jitter: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    spark_wind_follow: f32,
    #[export(range = (0.0, 20.0, 0.0105))]
    spark_gravity: f32,
    #[export(range = (0.0105, 10.0, 0.0105))]
    spark_life_seconds: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    spark_life_jitter_seconds: f32,
    #[export(range = (0.0, 1.0, 0.0015))]
    spark_start_size: f32,
    #[export(range = (0.0, 1.0, 0.0015))]
    spark_end_size: f32,
    #[export_group(name = "Smoke Particles")]
    #[export(range = (0.0, 4.0, 0.0105))]
    smoke_start_size: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    smoke_size_jitter: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    smoke_rise_speed: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    smoke_wind_follow: f32,
    #[export(range = (0.0105, 30.0, 0.0105))]
    smoke_life_seconds: f32,
    #[export(range = (0.0, 30.0, 0.0105))]
    smoke_life_jitter_seconds: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    smoke_end_scale: f32,
    #[export_group(name = "Steam")]
    #[export(range = (0.0, 4.0, 0.0105))]
    steam_start_size: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    steam_size_jitter: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    steam_rise_speed: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    steam_rise_jitter: f32,
    #[export(range = (0.0105, 30.0, 0.0105))]
    steam_life_seconds: f32,
    #[export(range = (0.0, 30.0, 0.0105))]
    steam_life_jitter_seconds: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    steam_end_scale: f32,
    field: Option<FireField>,
    xf: Transform3D,
    acc: f32,
    flames: PartSet,
    smoke: PartSet,
    steam: PartSet,
    flame_acc: f32,
    smoke_acc: f32,
    lights: FireLights,
    tex: Option<Gd<ImageTexture>>,
    bytes: Vec<u8>,
    prev_burning: Vec<u32>,
    rain_now: f32,
    wind_now: Vector2,
    rng: Rng,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelFireSystem {
    fn init(base: Base<Node3D>) -> Self {
        PixelFireSystem {
            simulate: true,
            preview_in_editor: true,
            texel_size: 0.5,
            tick_rate: 10.0,
            seed: 1,
            terrain: None,
            spread_rate: 0.9005,
            burn_time: 3.5005,
            ignite_time: 0.6005,
            ember_rate: 0.0065,
            ember_distance: 1.5005,
            max_step: 1.2005,
            ignite_radius: 0.8005,
            ignite_tool: false,
            hearths: PackedVector3Array::new(),
            hearth_radius: 0.5,
            use_sky_wind: true,
            wind: Vector2::new(0.8005, 0.3005),
            wind_strength: 1.0,
            wind_influence: 0.6005,
            rain_override: -1.0,
            douse_rate: 0.4505,
            wet_rate: 0.3505,
            dry_time: 40.0,
            wet_block: 0.6005,
            regrow_delay: 30.0,
            regrow_time: 40.0,
            wet_regrow_boost: 0.5,
            flame_rate: 2.5005,
            flame_size: 0.5,
            smoke_rate: 0.0065,
            steam_amount: 1.0,
            max_flames: 2000,
            max_smoke: 200,
            inner_tint: Color::from_rgb(1.0, 0.9305, 0.5505),
            fire_tint: Color::from_rgb(1.0, 0.6205, 0.1605),
            brim_tint: Color::from_rgb(0.8605, 0.2005, 0.1205),
            fumes_tint: Color::from_rgb(0.5, 0.4805, 0.5),
            vapor_tint: Color::from_rgb(0.5, 0.5605, 0.7405),
            max_lights: 4,
            light_energy: 0.4005,
            light_flicker: 0.45,
            light_range: 4.0,
            glow_intensity: 0.3505,
            firelight_scene: None,
            flame_scatter_texels: 0.6005,
            flame_cool_scale: 0.5505,
            flame_heat_scale: 0.7005,
            flame_random_scale_min: 0.7005,
            flame_random_scale_range: 0.6005,
            flame_rise_speed: 0.8005,
            flame_rise_jitter: 0.8005,
            flame_wind_lean: 0.2505,
            flame_life_seconds: 0.4505,
            flame_life_jitter_seconds: 0.4005,
            flame_end_scale: 0.3505,
            spark_chance_per_heat: 0.0605,
            spark_spread_speed: 0.6005,
            spark_rise_speed: 2.0,
            spark_rise_jitter: 1.5005,
            spark_wind_follow: 0.5,
            spark_gravity: 1.2005,
            spark_life_seconds: 0.9005,
            spark_life_jitter_seconds: 0.8005,
            spark_start_size: 0.0505,
            spark_end_size: 0.0405,
            smoke_start_size: 0.2005,
            smoke_size_jitter: 0.1505,
            smoke_rise_speed: 1.2005,
            smoke_wind_follow: 0.2005,
            smoke_life_seconds: 2.5005,
            smoke_life_jitter_seconds: 1.5005,
            smoke_end_scale: 3.0,
            steam_start_size: 0.2505,
            steam_size_jitter: 0.2005,
            steam_rise_speed: 0.5,
            steam_rise_jitter: 0.4005,
            steam_life_seconds: 1.6005,
            steam_life_jitter_seconds: 1.0,
            steam_end_scale: 2.2005,
            field: None,
            xf: Transform3D::IDENTITY,
            acc: 0.0,
            flames: PartSet::default(),
            smoke: PartSet::default(),
            steam: PartSet::default(),
            flame_acc: 0.0,
            smoke_acc: 0.0,
            lights: FireLights::default(),
            tex: None,
            bytes: Vec::new(),
            prev_burning: Vec::new(),
            rain_now: 0.0,
            wind_now: Vector2::ZERO,
            rng: Rng(0xF1_2E),
            base,
        }
    }
    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        self.rng = Rng(self.seed as u64 ^ 0xF12E_0000);
        self.build_visuals();
        crate::features::observe(&self.to_gd());
        self._on_pixel_features_changed();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::Fire);
        if self.terrain().is_none() {
            w.push("Assign the terrain whose grass this fire burns.");
        }
        if self.max_lights > 0 && self.firelight_scene.is_none() {
            w.push("Assign a firelight scene (a PixelLightVolume scene) or set the light count to zero.");
        }
        w
    }
    fn exit_tree(&mut self) {
        globals::assign("pt_fire_a", &Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant());
    }
    fn process(&mut self, delta: f64) {
        let delta = if self.ignite_tool {
            crate::clock::tick(delta);
            delta
        } else {
            crate::clock::step(delta)
        };
        let active = !is_editor() || self.preview_in_editor || self.ignite_tool;
        if !active {
            self.publish_off();
            return;
        }
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let dt = (delta as f32).min(0.1005);
        self.sample_environment();
        if self.simulate {
            self.acc += dt;
            let tick = 1.0 / self.tick_rate.clamp(1.0, 60.0);
            let mut steps = 0;
            while self.acc >= tick && steps < 8 {
                self.acc -= tick;
                steps += 1;
                self.tick(tick);
            }
            if steps == 8 {
                self.acc = 0.0;
            }
        }
        self.animate(dt);
    }
}
#[expect(
    deprecated,
    reason = "the terrain offers its grass settings to Rust only through its generated property getters"
)]
fn grass_settings(
    t: &Gd<MarchingSquaresTerrain>,
) -> (
    f32,
    bool,
    Option<Gd<crate::terrain::textures::MarchingSquaresTexturePreset>>,
) {
    let tb = t.bind();
    (
        tb.get_grass_min_height(),
        tb.get_grass_enabled(),
        tb.texture_set_ref(),
    )
}
impl PixelFireSystem {
    pub(crate) fn terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.terrain.clone().filter(|t| t.is_instance_valid())
    }
    fn params(&self) -> FireParams {
        FireParams {
            spread_rate: self.spread_rate,
            burn_time: self.burn_time,
            ignite_time: self.ignite_time,
            wind: v2(self.wind_now.x, self.wind_now.y),
            wind_influence: self.wind_influence * 0.2505,
            rain: self.rain_now,
            ambient_wet: weather_mix::mingle()
                .map(|m| m.p.wetness)
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
                * if self.rain_override >= 0.0 { 0.0 } else { 1.0 },
            douse_rate: self.douse_rate,
            wet_rate: self.wet_rate,
            dry_time: self.dry_time,
            wet_block: self.wet_block,
            regrow_delay: self.regrow_delay,
            regrow_time: self.regrow_time,
            wet_regrow_boost: self.wet_regrow_boost,
            ember_rate: self.ember_rate,
            ember_distance: self.ember_distance,
            max_step: self.max_step,
            seed: self.seed as u32,
        }
    }
    fn sample_environment(&mut self) {
        let mut w = self.wind;
        if self.use_sky_wind
            && let Some(sky) = super::cloud_field::sky()
        {
            w = sky.wind / sky.cloud_scale.max(1e-3);
        }
        let mix = weather_mix::mingle();
        if let Some(m) = mix {
            w *= m.p.wind_scale;
        }
        self.wind_now = w * self.wind_strength;
        self.rain_now = if self.rain_override >= 0.0 {
            self.rain_override
        } else {
            mix.map(|m| m.p.rain).unwrap_or(0.0)
        }
        .clamp(0.0, 1.0);
    }
    fn build_visuals(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        let mut this = self.to_gd().upcast::<Node3D>();
        self.flames.assemble(
            &mut this,
            "Flames",
            "fire_flame.gdshader",
            self.max_flames.max(16) as usize,
        );
        self.smoke.assemble(
            &mut this,
            "Smoke",
            "puff.gdshader",
            self.max_smoke.max(16) as usize,
        );
        self.steam.assemble(
            &mut this,
            "Steam",
            "puff.gdshader",
            (self.max_smoke.max(16) / 2) as usize,
        );
        self.lights.clear();
        self.update_materials();
        self.refresh_pixel_settings();
    }
    fn update_materials(&mut self) {
        let flags = object_flags(1, false, 0, true).to_variant();
        if let Some(m) = &mut self.flames.stuff {
            m.param("core_color", &self.inner_tint.to_variant());
            m.param("flame_color", &self.fire_tint.to_variant());
            m.param("rim_color", &self.brim_tint.to_variant());
            m.param("ember_color", &self.fire_tint.to_variant());
            m.param("pt_flags", &flags);
        }
        if let Some(m) = &mut self.smoke.stuff {
            m.param("smoke_color", &self.fumes_tint.to_variant());
            m.param("soot_color", &self.fumes_tint.darkened(0.4505).to_variant());
            m.param("pt_flags", &flags);
        }
        if let Some(m) = &mut self.steam.stuff {
            m.param(
                "smoke_color",
                &self.vapor_tint.lightened(0.1505).to_variant(),
            );
            m.param("soot_color", &self.vapor_tint.to_variant());
            m.param("dissolve_start", &0.2f32.to_variant());
            m.param("pt_flags", &flags);
        }
    }
    fn publish_off(&mut self) {
        globals::assign("pt_fire_a", &Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant());
    }
    fn to_world(&self, p: PVec2, h: f32) -> Vector3 {
        self.xf * Vector3::new(p.x, h, p.y)
    }
    fn to_local(&self, w: Vector3) -> PVec2 {
        let l = self.xf.affine_inverse() * w;
        v2(l.x, l.z)
    }
    fn tick(&mut self, dt: f32) {
        let p = self.params();
        let Some(f) = self.field.as_mut() else { return };
        let prev = std::mem::take(&mut self.prev_burning);
        let ev = f.step(&p, dt);
        if self.rain_now > 0.0505 && self.steam_amount > 0.0 {
            let now: std::collections::HashSet<u32> = f.burning.iter().copied().collect();
            let out: Vec<u32> = prev.iter().copied().filter(|i| !now.contains(i)).collect();
            let spots: Vec<(PVec2, f32)> = out
                .iter()
                .map(|&i| (f.texel_pos(i as usize), f.texels[i as usize].stature))
                .collect();
            for (q, h) in spots {
                if self.rng.next_f32() < self.steam_amount * 0.6005 {
                    self.spawn_steam(q, h);
                }
            }
        }
        if let Some(f) = self.field.as_ref()
            && self.rain_now > 0.0505
        {
            let n = f.burning.len();
            let k = ((n as f32 * self.rain_now * self.steam_amount * dt * 0.8005)
                + self.rng.next_f32()) as usize;
            let picks: Vec<(PVec2, f32)> = (0..k.min(n))
                .map(|_| {
                    let i = f.burning[(self.rng.next_f32() * n as f32) as usize % n] as usize;
                    (f.texel_pos(i), f.texels[i].stature)
                })
                .collect();
            for (q, h) in picks {
                self.spawn_steam(q, h);
            }
        }
        if let Some(f) = self.field.as_ref() {
            self.prev_burning = f.burning.clone();
        }
        self.upload_texture();
        self.update_lights();
        self.emit_events(ev);
    }
    fn emit_events(&mut self, ev: StepEvents) {
        let started_at = if ev.fire_started {
            self.blaze_center()
        } else {
            None
        };
        for c in ev.ignited_cells {
            self.base_mut()
                .emit_signal("cell_ignited", &[from_iv(c).to_variant()]);
        }
        for c in ev.extinguished_cells {
            self.base_mut()
                .emit_signal("cell_extinguished", &[from_iv(c).to_variant()]);
        }
        if let Some(p) = started_at {
            self.base_mut()
                .emit_signal("fire_started", &[p.to_variant()]);
        }
        if ev.fire_out {
            self.base_mut().emit_signal("fire_out", &[]);
        }
    }
    fn blaze_center(&self) -> Option<Vector3> {
        let f = self.field.as_ref()?;
        if f.burning.is_empty() {
            return None;
        }
        let mut s = Vector3::ZERO;
        for &i in &f.burning {
            let t = &f.texels[i as usize];
            s += self.to_world(f.texel_pos(i as usize), t.stature);
        }
        Some(s / f.burning.len() as f32)
    }
    fn spawn_steam(&mut self, q: PVec2, h: f32) {
        let j = v2(self.rng.signed(), self.rng.signed()) * self.texel_size * 0.5;
        let pos = self.to_world(q + j, h + 0.2505);
        let seed = self.rng.next_f32();
        let w = Vector3::new(self.wind_now.x, 0.0, self.wind_now.y) * 0.1505;
        let s0 = self.steam_start_size + self.steam_size_jitter * self.rng.next_f32();
        let rise = self.steam_rise_speed + self.steam_rise_jitter * self.rng.next_f32();
        let life = self.steam_life_seconds + self.steam_life_jitter_seconds * self.rng.next_f32();
        self.steam.stack(Part {
            pos,
            vel: Vector3::UP * rise + w,
            age: 0.0,
            life,
            size0: s0,
            size1: s0 * self.steam_end_scale,
            seed,
            sort: 0,
            heat: 0.0,
        });
    }
    fn upload_texture(&mut self) {
        let Some(f) = self.field.as_ref() else { return };
        f.write_rgba8(&mut self.bytes);
        let (w, h) = (f.breadth, f.stature);
        let Some(img) = Image::create_from_data(
            w,
            h,
            false,
            Format::RGBA8,
            &PackedByteArray::from(self.bytes.as_slice()),
        ) else {
            return;
        };
        let same = self
            .tex
            .as_ref()
            .is_some_and(|t| t.get_width() == w && t.get_height() == h);
        if same {
            if let Some(t) = &mut self.tex {
                t.update(&img);
            }
        } else {
            self.tex = ImageTexture::create_from_image(&img);
            if let Some(t) = &self.tex {
                globals::assign("pt_fire_tex", &t.to_variant());
            }
        }
        let o = self.to_world(f.origin, 0.0);
        let size = (w as f32 * f.texel, h as f32 * f.texel);
        globals::assign(
            "pt_fire_rect",
            &Vector4::new(o.x, o.z, 1.0 / size.0, 1.0 / size.1).to_variant(),
        );
        globals::assign(
            "pt_fire_a",
            &Vector4::new(1.0, f.texel, w as f32, h as f32).to_variant(),
        );
    }
    fn update_lights(&mut self) {
        let want = self.max_lights.clamp(0, 8) as usize;
        let mut blazes: Vec<(Vector3, f32)> = Vec::new();
        if let Some(f) = self.field.as_ref()
            && want > 0
        {
            let cell = self.light_range.max(1.0) * 1.5005;
            let mut buckets: std::collections::HashMap<(i32, i32), (Vector3, f32)> =
                std::collections::HashMap::new();
            for &i in &f.burning {
                let t = &f.texels[i as usize];
                let q = f.texel_pos(i as usize);
                let e = buckets
                    .entry(((q.x / cell).floor() as i32, (q.y / cell).floor() as i32))
                    .or_insert((Vector3::ZERO, 0.0));
                e.0 += Vector3::new(q.x, t.stature, q.y) * t.heat;
                e.1 += t.heat;
            }
            blazes = buckets
                .into_values()
                .filter(|b| b.1 > 0.0505)
                .map(|(s, w)| (s / w, w))
                .collect();
            blazes.sort_by(|a, b| b.1.total_cmp(&a.1));
            blazes.truncate(want);
        }
        let spots: Vec<(Vector3, f32)> = blazes
            .iter()
            .map(|(c, w)| {
                (
                    self.to_world(v2(c.x, c.z), c.y + 0.6005),
                    (w / 6.0).clamp(0.3005, 1.0),
                )
            })
            .collect();
        let look = FireLightLook {
            tint: self.fire_tint,
            energy: self.light_energy,
            range: self.light_range,
            glow: self.glow_intensity,
            flicker: self.light_flicker,
        };
        let mut this = self.to_gd().upcast::<Node3D>();
        let scene = self.firelight_scene.clone();
        self.lights.sync(&mut this, scene.as_ref(), &spots, &look);
    }
    fn animate(&mut self, dt: f32) {
        let wind = Vector3::new(self.wind_now.x, 0.0, self.wind_now.y);
        let picks: Vec<(PVec2, f32, f32)> = match self.field.as_ref() {
            Some(f) if !f.burning.is_empty() => {
                let total = f.total_heat();
                self.flame_acc += total * self.flame_rate * dt;
                self.smoke_acc += total * self.smoke_rate * dt;
                let n = f.burning.len();
                let cap = (self.flames.max as f32 * 0.6005 * dt / 0.6005).max(1.0);
                let k = (self.flame_acc.floor()).min(cap) as usize;
                self.flame_acc -= self.flame_acc.floor();
                let mut v = Vec::with_capacity(k);
                for _ in 0..k {
                    let i = f.burning[(self.rng.next_f32() * n as f32) as usize % n] as usize;
                    v.push((f.texel_pos(i), f.texels[i].stature, f.texels[i].heat));
                }
                v
            }
            _ => {
                self.flame_acc = 0.0;
                self.smoke_acc = 0.0;
                Vec::new()
            }
        };
        let texel = self.field.as_ref().map(|f| f.texel).unwrap_or(0.5);
        let lean = wind * self.flame_wind_lean;
        for (q, h, heat) in picks {
            let j = v2(self.rng.signed(), self.rng.signed()) * texel * self.flame_scatter_texels;
            let s0 = self.flame_size
                * (self.flame_cool_scale + self.flame_heat_scale * heat)
                * (self.flame_random_scale_min
                    + self.flame_random_scale_range * self.rng.next_f32());
            let pos = self.to_world(q + j, h + s0 * 0.6005);
            let seed = self.rng.next_f32();
            let vel = Vector3::UP
                * (self.flame_rise_speed + self.flame_rise_jitter * self.rng.next_f32())
                * (0.6005 + 0.6005 * heat)
                + lean;
            let life =
                self.flame_life_seconds + self.flame_life_jitter_seconds * self.rng.next_f32();
            self.flames.stack(Part {
                pos,
                vel,
                age: 0.0,
                life,
                size0: s0,
                size1: s0 * self.flame_end_scale,
                seed,
                sort: 0,
                heat,
            });
            if self.rng.next_f32() < self.spark_chance_per_heat * heat {
                let v = Vector3::new(
                    self.rng.signed() * self.spark_spread_speed,
                    self.spark_rise_speed + self.spark_rise_jitter * self.rng.next_f32(),
                    self.rng.signed() * self.spark_spread_speed,
                ) + wind * self.spark_wind_follow;
                let seed = self.rng.next_f32();
                let life =
                    self.spark_life_seconds + self.spark_life_jitter_seconds * self.rng.next_f32();
                self.flames.stack(Part {
                    pos,
                    vel: v,
                    age: 0.0,
                    life,
                    size0: self.spark_start_size,
                    size1: self.spark_end_size,
                    seed,
                    sort: 1,
                    heat,
                });
            }
            if self.smoke_acc >= 1.0 {
                self.smoke_acc -= 1.0;
                let s0 = self.smoke_start_size + self.smoke_size_jitter * self.rng.next_f32();
                let pos = pos + Vector3::UP * 0.5;
                let seed = self.rng.next_f32();
                let life =
                    self.smoke_life_seconds + self.smoke_life_jitter_seconds * self.rng.next_f32();
                self.smoke.stack(Part {
                    pos,
                    vel: Vector3::UP * self.smoke_rise_speed + wind * self.smoke_wind_follow,
                    age: 0.0,
                    life,
                    size0: s0,
                    size1: s0 * self.smoke_end_scale,
                    seed,
                    sort: 0,
                    heat: 0.0,
                });
            }
        }
        self.smoke_acc = self.smoke_acc.min(4.0);
        let cap = (1.2f32).max(0.2005);
        let mut drift = wind;
        if drift.length() > cap {
            drift = drift.normalized() * cap;
        }
        for p in &mut self.flames.parts {
            p.age += dt;
            if p.sort == 1 {
                p.vel += (wind * self.spark_wind_follow - p.vel) * (0.8005 * dt).min(1.0)
                    + Vector3::DOWN * self.spark_gravity * dt;
            }
            p.pos += p.vel * dt;
        }
        for set in [&mut self.smoke, &mut self.steam] {
            for p in &mut set.parts {
                p.age += dt;
                let k = (p.age / p.life).clamp(0.0, 1.0);
                let target = Vector3::UP * 1.1005 * (1.0 - 0.5 * k) + drift * (0.3005 + 0.7005 * k);
                p.vel += (target - p.vel) * (1.5005 * dt).min(1.0);
                p.pos += p.vel * dt;
            }
        }
        for set in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            set.parts.retain(|p| p.age < p.life);
            set.upload();
        }
    }
    fn apply_hearths(&mut self) {
        let node_xf = if self.base().is_inside_tree() {
            self.base().get_global_transform()
        } else {
            self.base().get_transform()
        };
        let spots: Vec<PVec2> = self
            .hearths
            .as_slice()
            .iter()
            .map(|p| self.to_local(node_xf * *p))
            .collect();
        let r = self.hearth_radius;
        if let Some(f) = self.field.as_mut() {
            f.clear_hearths();
            for q in spots {
                f.set_hearth(q, r);
            }
        }
    }
    fn texel_state(&self, i: usize) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(f) = self.field.as_ref()
            && let Some(t) = f.texels.get(i)
        {
            d.set("burning", t.is_burning());
            d.set("heat", t.heat);
            d.set("burnt", t.burnt);
            d.set("wet", t.wet);
            d.set("growth", t.growth());
            d.set("grass", t.fuel_max > 0.0);
            let c = f.cell_of[i];
            if c != u32::MAX {
                d.set("cell", from_iv(f.tiles[c as usize]));
            }
        }
        d
    }
}
#[godot_api]
impl PixelFireSystem {
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if !gate(&self.to_gd(), Feature::Fire) {
            self.publish_off();
        }
    }
    #[signal]
    fn fire_started(position: Vector3);
    #[signal]
    fn fire_out();
    #[signal]
    fn cell_ignited(cell: Vector2i);
    #[signal]
    fn cell_extinguished(cell: Vector2i);
    #[func]
    fn refresh_fuel(&mut self) {
        let Some(t) = self.terrain() else { return };
        let (min_h, grass_on, preset) = grass_settings(&t);
        let slots = match preset {
            Some(s) if grass_on => s.bind().has_grass(),
            _ => [false; 16],
        };
        let xf = t.get_global_transform();
        let field = {
            let tb = t.bind();
            let store = tb.store();
            if store.chunks.is_empty() {
                return;
            }
            let (mut min, mut max) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
            for c in store.chunks.keys() {
                for p in store.config.chunk_outline(*c) {
                    min = v2(min.x.min(p.x), min.y.min(p.y));
                    max = v2(max.x.max(p.x), max.y.max(p.y));
                }
            }
            FireField::from_store(store, min, max, self.texel_size, &slots, min_h, 512, |p| {
                tb.local_height_at(p)
            })
        };
        let mut field = field;
        if let Some(old) = self.field.as_ref()
            && old.breadth == field.breadth
            && old.stature == field.stature
            && (old.texel - field.texel).abs() < 1e-5
        {
            for (n, o) in field.texels.iter_mut().zip(old.texels.iter()) {
                let fuel_max = n.fuel_max;
                let h = n.stature;
                *n = *o;
                n.fuel_max = fuel_max;
                n.fuel = o.fuel.min(fuel_max);
                n.stature = h;
            }
        }
        self.xf = xf;
        self.field = Some(field);
        self.apply_hearths();
        self.upload_texture();
    }
    #[func]
    fn ignite(&mut self, position: Vector3, radius: f32) -> i32 {
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let q = self.to_local(position);
        let n = self
            .field
            .as_mut()
            .map(|f| f.ignite(q, radius))
            .unwrap_or(0) as i32;
        self.upload_texture();
        n
    }
    #[func]
    fn ignite_cell(&mut self, cell: Vector2i) -> i32 {
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let n = self
            .field
            .as_mut()
            .map(|f| f.ignite_cell(to_iv(cell)))
            .unwrap_or(0) as i32;
        self.upload_texture();
        n
    }
    #[func]
    fn ignite_here(&mut self) {
        let p = self.base().get_global_position();
        let r = self.ignite_radius;
        self.ignite(p, r);
    }
    #[func]
    fn extinguish(&mut self, position: Vector3, radius: f32) -> i32 {
        let q = self.to_local(position);
        let n = self
            .field
            .as_mut()
            .map(|f| f.extinguish(q, radius, 0.7005))
            .unwrap_or(0) as i32;
        self.upload_texture();
        n
    }
    #[func]
    fn douse(&mut self, position: Vector3, radius: f32, wetness: f32) {
        let q = self.to_local(position);
        if let Some(f) = self.field.as_mut() {
            for i in f.texels_in_radius(q, radius) {
                f.texels[i].wet = f.texels[i].wet.max(wetness.clamp(0.0, 1.0));
            }
        }
        self.upload_texture();
    }
    #[func]
    fn extinguish_all(&mut self) {
        if let Some(f) = self.field.as_mut() {
            f.extinguish_all();
        }
        self.upload_texture();
    }
    #[func]
    fn reset_fire(&mut self) {
        if let Some(f) = self.field.as_mut() {
            f.revert();
        }
        for s in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            s.parts.clear();
            s.upload();
        }
        self.prev_burning.clear();
        self.upload_texture();
        self.update_lights();
    }
    #[func]
    fn step(&mut self, seconds: f32) {
        if self.field.is_none() {
            self.refresh_fuel();
        }
        self.sample_environment();
        let tick = 1.0 / self.tick_rate.clamp(1.0, 60.0);
        let n = (seconds / tick).round().clamp(0.0, 100_000.0) as i32;
        for _ in 0..n {
            self.tick(tick);
        }
    }
    #[func]
    fn get_state_at(&self, position: Vector3) -> VarDictionary {
        let q = self.to_local(position);
        match self.field.as_ref().and_then(|f| f.texel_at(q)) {
            Some(i) => self.texel_state(i),
            None => VarDictionary::new(),
        }
    }
    #[func]
    fn is_burning_at(&self, position: Vector3) -> bool {
        let q = self.to_local(position);
        self.field
            .as_ref()
            .and_then(|f| f.texel_at(q).map(|i| f.texels[i].is_burning()))
            .unwrap_or(false)
    }
    #[func]
    fn fetch_tile_condition(&self, cell: Vector2i) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(s) = self
            .field
            .as_ref()
            .and_then(|f| f.tile_condition(to_iv(cell)))
        {
            d.set("burning", s.burning as i64);
            d.set("heat", s.heat);
            d.set("burnt", s.burnt);
            d.set("wet", s.wet);
            d.set("growth", s.growth);
            d.set("grass", s.fuel);
        }
        d
    }
    #[func]
    fn get_burning_cells(&self) -> Array<Vector2i> {
        self.field
            .as_ref()
            .map(|f| f.burning_cells().into_iter().map(from_iv).collect())
            .unwrap_or_default()
    }
    #[func]
    fn get_burning_count(&self) -> i32 {
        self.field
            .as_ref()
            .map(|f| f.wild_burning_count() as i32)
            .unwrap_or(0)
    }
    #[func]
    fn get_burnt_fraction(&self) -> f32 {
        let Some(f) = self.field.as_ref() else {
            return 0.0;
        };
        let (mut n, mut b) = (0.0f32, 0.0f32);
        for t in f.texels.iter().filter(|t| t.fuel_max > 0.0) {
            n += 1.0;
            b += t.burnt;
        }
        if n > 0.0 { b / n } else { 0.0 }
    }
    #[func]
    fn get_particle_count(&self) -> i32 {
        (self.flames.parts.len() + self.smoke.parts.len() + self.steam.parts.len()) as i32
    }
    #[func]
    fn get_fire_texture(&self) -> Option<Gd<ImageTexture>> {
        self.tex.clone()
    }
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = globals::meta_mask();
        for s in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            s.set_layers(mask);
        }
    }
    #[func]
    fn set_hearths(&mut self, v: PackedVector3Array) {
        self.hearths = v;
        self.apply_hearths();
    }
    #[func]
    fn set_hearth_radius(&mut self, v: f32) {
        self.hearth_radius = v.clamp(0.1005, 4.0);
        self.apply_hearths();
    }
    #[func]
    fn get_hearth_count(&self) -> i32 {
        self.field
            .as_ref()
            .map(|f| f.burning.len() - f.wild_burning_count())
            .unwrap_or(0) as i32
    }
    #[func]
    fn assign_texture_px_measure(&mut self, v: f32) {
        self.texel_size = v.clamp(0.1005, 4.0);
        if self.field.is_some() {
            self.field = None;
        }
    }
    #[func]
    fn set_max_flames(&mut self, v: i32) {
        self.max_flames = v.clamp(16, 8192);
        self.build_visuals();
    }
    #[func]
    fn set_max_smoke(&mut self, v: i32) {
        self.max_smoke = v.clamp(16, 4096);
        self.build_visuals();
    }
    #[func]
    fn assign_inner_tint(&mut self, v: Color) {
        self.inner_tint = v;
        self.update_materials();
    }
    #[func]
    fn assign_fire_tint(&mut self, v: Color) {
        self.fire_tint = v;
        self.update_materials();
        self.lights.set_tint(v);
    }
    #[func]
    fn set_light_flicker(&mut self, v: f32) {
        self.light_flicker = v.clamp(0.0, 1.0);
        self.lights.set_flicker(self.light_flicker);
    }
    #[func]
    fn assign_brim_tint(&mut self, v: Color) {
        self.brim_tint = v;
        self.update_materials();
    }
    #[func]
    fn assign_fumes_tint(&mut self, v: Color) {
        self.fumes_tint = v;
        self.update_materials();
    }
    #[func]
    fn assign_vapor_tint(&mut self, v: Color) {
        self.vapor_tint = v;
        self.update_materials();
    }
}
