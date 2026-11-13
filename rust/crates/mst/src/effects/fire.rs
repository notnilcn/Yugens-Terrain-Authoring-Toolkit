//! `PixelFireSystem` — grass fires that ignite, spread with the wind, char the ground, are put
//! out by rain and regrow (simulation: `pt_core::fire`).
//!
//! The node runs a [`FireField`] over its terrain at a fixed tick rate, publishes the field as
//! the `pt_fire_*` global shader parameters (read by `pixel_fire.gdshaderinc` in the terrain and
//! grass shaders: charred ground, embers, burning / shrinking / regrowing blades, wet patches)
//! and draws pixel flames (`fire_flame.gdshader`), soot smoke and rain steam (`puff.gdshader`,
//! like `PixelSmoke`) plus a few flickering `PixelLightVolume` fire lights.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::image::Format;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{INode3D, Image, ImageTexture, MultiMesh, MultiMeshInstance3D, Node, Node3D, QuadMesh, ShaderMaterial};
use godot::prelude::*;
use pt_core::fire::{FireField, FireParams, StepEvents};
use pt_core::math::{Vec2 as PVec2, v2};

use super::light_volume::PixelLightVolume;
use super::{Rng, weather};
use crate::globals;
use crate::terrain::MarchingSquaresTerrain;
use crate::util::*;

#[derive(Clone, Copy)]
struct Part {
    pos: Vector3,
    vel: Vector3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    seed: f32,
    kind: u8,
    heat: f32,
}

/// One MultiMesh of camera-facing particles.
#[derive(Default)]
struct PartSet {
    parts: Vec<Part>,
    mmi: Option<Gd<MultiMeshInstance3D>>,
    material: Option<Gd<ShaderMaterial>>,
    max: usize,
}

impl PartSet {
    fn build(&mut self, parent: &mut Gd<Node3D>, name: &str, shader: &str, max: usize) {
        free_node(self.mmi.take());
        self.max = max.max(1);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(2.0, 2.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        mm.set_instance_count(self.max as i32);
        mm.set_visible_instance_count(0);
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        mi.set_as_top_level(true);
        mi.set_global_transform(Transform3D::IDENTITY);
        if self.material.is_none() {
            self.material = Some(shader_material(shader));
        }
        if let Some(m) = &self.material {
            mi.set_material_override(m);
        }
        add_internal(parent, &mi);
        self.mmi = Some(mi);
        self.parts.truncate(self.max);
    }

    fn push(&mut self, p: Part) {
        if self.parts.len() >= self.max {
            // Recycle the oldest.
            if let Some((i, _)) = self.parts.iter().enumerate().max_by(|a, b| (a.1.age / a.1.life).total_cmp(&(b.1.age / b.1.life))) {
                self.parts.swap_remove(i);
            }
        }
        self.parts.push(p);
    }

    fn upload(&mut self) {
        let Some(mi) = self.mmi.clone().filter(|m| m.is_instance_valid()) else { return };
        let Some(mut mm) = mi.get_multimesh() else { return };
        let count = mm.get_instance_count() as usize;
        let n = self.parts.len().min(count);
        let mut lo = Vector3::splat(f32::MAX);
        let mut hi = Vector3::splat(f32::MIN);
        // One buffer upload: per instance a 3x4 row-major transform + the custom color.
        let mut buf = vec![0.0f32; count * 16];
        for (i, p) in self.parts.iter().take(n).enumerate() {
            let k = (p.age / p.life).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - k) * (1.0 - k);
            let s = (p.size0 + (p.size1 - p.size0) * ease).max(0.005);
            buf[i * 16..i * 16 + 16].copy_from_slice(&[s, 0.0, 0.0, p.pos.x, 0.0, s, 0.0, p.pos.y, 0.0, 0.0, s, p.pos.z, k, p.seed, p.kind as f32, p.heat]);
            lo = lo.coord_min(p.pos - Vector3::splat(s));
            hi = hi.coord_max(p.pos + Vector3::splat(s));
        }
        mm.set_buffer(&PackedFloat32Array::from(buf.as_slice()));
        mm.set_visible_instance_count(n as i32);
        if n > 0 {
            mm.set_custom_aabb(Aabb::new(lo, hi - lo));
        }
    }

    fn set_layers(&mut self, mask: u32) {
        if let Some(m) = &mut self.mmi {
            let layers = m.get_layer_mask() | mask;
            m.set_layer_mask(layers);
        }
    }
}

/// Grass fire, rain and regrowth for a `MarchingSquaresTerrain` (any grid type; hex first).
///
/// Add it as a child of the terrain (or anywhere: it then uses the scene's first terrain).
/// Grass (texture slots with grass, at or above the terrain's `grass_min_height`) is fuel.
/// Fire spreads blade to blade, faster downwind, throws embers that start spot fires, chars
/// the ground and shrinks the blades to black stubble; rain (PixelWeather, or `rain_override`)
/// douses it with puffs of steam and wets the grass, which then resists fire and regrows faster.
///
/// Game API: `ignite(position, radius)`, `ignite_cell(cell)`, `extinguish(position, radius)`,
/// `extinguish_all()`, `reset_fire()`, `get_state_at(position)`, `get_cell_state(cell)`,
/// `is_burning_at(position)`, `get_burning_cells()`; signals `fire_started`, `fire_out`,
/// `cell_ignited`, `cell_extinguished`. The simulation steps at `tick_rate` (deterministic for a
/// given `seed`); `step(seconds)` advances it manually.
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelFireSystem {
    /// Run the simulation (the fire freezes while off).
    #[export]
    simulate: bool,
    /// Run the fire in the editor too (for previewing with the ignite tool).
    #[export]
    preview_in_editor: bool,
    /// Size of one fire cell in metres (the lattice covers the whole terrain).
    #[export(range = (0.1, 4.0, 0.01))]
    #[var(set = set_texel_size)]
    texel_size: f32,
    /// Simulation steps per second.
    #[export(range = (1.0, 60.0, 1.0))]
    tick_rate: f32,
    #[export]
    seed: i32,

    #[export_group(name = "Spread")]
    /// Ignition rate from one fully burning neighbour (per second).
    #[export(range = (0.0, 10.0, 0.01))]
    spread_rate: f32,
    /// Seconds full grass burns.
    #[export(range = (0.1, 60.0, 0.1))]
    burn_time: f32,
    /// Spot fires per second thrown by one burning cell (embers).
    #[export(range = (0.0, 1.0, 0.001))]
    ember_rate: f32,
    /// How far embers fly downwind (cells per m/s of wind).
    #[export(range = (0.0, 10.0, 0.01))]
    ember_distance: f32,
    /// Fire does not climb steps taller than this (m).
    #[export(range = (0.0, 20.0, 0.05))]
    max_step: f32,
    /// Radius used by `ignite_here()` and the editor ignite tool (m).
    #[export(range = (0.1, 20.0, 0.1))]
    ignite_radius: f32,
    /// Editor: click the terrain to ignite (Shift+click extinguishes) while this node is selected.
    #[export]
    ignite_tool: bool,

    #[export_group(name = "Wind")]
    /// Take the wind from PixelSky (cloud drift) scaled by PixelWeather.
    #[export]
    use_sky_wind: bool,
    /// Wind in m/s (XZ) when `use_sky_wind` is off or there is no sky.
    #[export]
    wind: Vector2,
    /// Multiplies the wind.
    #[export(range = (0.0, 10.0, 0.01))]
    wind_strength: f32,
    /// How much the wind skews the spread.
    #[export(range = (0.0, 4.0, 0.01))]
    wind_influence: f32,

    #[export_group(name = "Rain")]
    /// Rain intensity 0..1; negative = from PixelWeather.
    #[export(range = (-1.0, 1.0, 0.01))]
    rain_override: f32,
    /// Heat lost per second in full rain.
    #[export(range = (0.0, 10.0, 0.01))]
    douse_rate: f32,
    /// Wetness gained per second in full rain.
    #[export(range = (0.0, 4.0, 0.01))]
    wet_rate: f32,
    /// Seconds soaked grass takes to dry.
    #[export(range = (0.1, 600.0, 0.1))]
    dry_time: f32,
    /// Wetness at which grass stops catching fire.
    #[export(range = (0.01, 1.0, 0.01))]
    wet_block: f32,

    #[export_group(name = "Regrowth")]
    /// Seconds after the fire before burnt grass regrows.
    #[export(range = (0.0, 600.0, 0.1))]
    regrow_delay: f32,
    /// Seconds burnt grass takes to grow back.
    #[export(range = (0.1, 600.0, 0.1))]
    regrow_time: f32,
    /// Regrowth speed-up of wet grass (1 + boost x wetness).
    #[export(range = (0.0, 10.0, 0.01))]
    wet_regrow_boost: f32,

    #[export_group(name = "Look")]
    /// Flame tongues per second per burning cell at full heat.
    #[export(range = (0.0, 40.0, 0.1))]
    flame_rate: f32,
    /// Flame tongue radius (m).
    #[export(range = (0.02, 4.0, 0.01))]
    flame_size: f32,
    /// Soot puffs per second per burning cell.
    #[export(range = (0.0, 10.0, 0.01))]
    smoke_rate: f32,
    /// Steam puffs per cell put out by rain.
    #[export(range = (0.0, 4.0, 0.01))]
    steam_amount: f32,
    #[export(range = (16.0, 8192.0, 1.0))]
    #[var(set = set_max_flames)]
    max_flames: i32,
    #[export(range = (16.0, 4096.0, 1.0))]
    #[var(set = set_max_smoke)]
    max_smoke: i32,
    #[export]
    #[var(set = set_core_color)]
    core_color: Color,
    #[export]
    #[var(set = set_flame_color)]
    flame_color: Color,
    #[export]
    #[var(set = set_rim_color)]
    rim_color: Color,
    #[export]
    #[var(set = set_smoke_color)]
    smoke_color: Color,
    #[export]
    #[var(set = set_steam_color)]
    steam_color: Color,
    /// Fire lights (flickering PixelLightVolumes on the biggest blazes).
    #[export(range = (0.0, 8.0, 1.0))]
    max_lights: i32,
    #[export(range = (0.0, 16.0, 0.01))]
    light_energy: f32,
    #[export(range = (0.1, 50.0, 0.1))]
    light_range: f32,
    #[export(range = (0.0, 4.0, 0.01))]
    glow_intensity: f32,

    field: Option<FireField>,
    /// Terrain global transform when the field was built.
    xf: Transform3D,
    acc: f32,
    flames: PartSet,
    smoke: PartSet,
    steam: PartSet,
    flame_acc: f32,
    smoke_acc: f32,
    lights: Vec<Gd<PixelLightVolume>>,
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
            spread_rate: 0.9,
            burn_time: 3.5,
            ember_rate: 0.006,
            ember_distance: 1.5,
            max_step: 1.2,
            ignite_radius: 0.8,
            ignite_tool: false,
            use_sky_wind: true,
            wind: Vector2::new(0.8, 0.3),
            wind_strength: 1.0,
            wind_influence: 0.6,
            rain_override: -1.0,
            douse_rate: 0.45,
            wet_rate: 0.35,
            dry_time: 40.0,
            wet_block: 0.6,
            regrow_delay: 30.0,
            regrow_time: 40.0,
            wet_regrow_boost: 0.5,
            flame_rate: 2.5,
            flame_size: 0.5,
            smoke_rate: 0.006,
            steam_amount: 1.0,
            max_flames: 2000,
            max_smoke: 200,
            core_color: Color::from_rgb(1.0, 0.93, 0.55),
            flame_color: Color::from_rgb(1.0, 0.62, 0.16),
            rim_color: Color::from_rgb(0.86, 0.2, 0.12),
            smoke_color: Color::from_rgb(0.5, 0.48, 0.5),
            steam_color: Color::from_rgb(0.5, 0.56, 0.74),
            max_lights: 4,
            light_energy: 0.4,
            light_range: 4.0,
            glow_intensity: 0.35,
            field: None,
            xf: Transform3D::IDENTITY,
            acc: 0.0,
            flames: PartSet::default(),
            smoke: PartSet::default(),
            steam: PartSet::default(),
            flame_acc: 0.0,
            smoke_acc: 0.0,
            lights: Vec::new(),
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
        self.to_gd().add_to_group("__pixel_fire");
        self.rng = Rng(self.seed as u64 ^ 0xF12E_0000);
        self.build_visuals();
        self.base_mut().set_process(true);
    }

    fn exit_tree(&mut self) {
        globals::set("pt_fire_a", &Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant());
    }

    fn process(&mut self, delta: f64) {
        let active = !is_editor() || self.preview_in_editor || self.ignite_tool;
        if !active {
            self.publish_off();
            return;
        }
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let dt = (delta as f32).min(0.1);
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

impl PixelFireSystem {
    fn terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        if let Some(p) = self.base().get_parent()
            && let Ok(t) = p.try_cast::<MarchingSquaresTerrain>()
        {
            return Some(t);
        }
        super::find_terrain(&self.to_gd().upcast()).and_then(|n| n.try_cast::<MarchingSquaresTerrain>().ok())
    }

    fn params(&self) -> FireParams {
        FireParams {
            spread_rate: self.spread_rate,
            burn_time: self.burn_time,
            ignite_time: 0.6,
            wind: v2(self.wind_now.x, self.wind_now.y),
            wind_influence: self.wind_influence * 0.25,
            rain: self.rain_now,
            ambient_wet: weather::mix().map(|m| m.p.wetness).unwrap_or(0.0).clamp(0.0, 1.0) * if self.rain_override >= 0.0 { 0.0 } else { 1.0 },
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

    /// Wind (m/s) and rain right now.
    fn sample_environment(&mut self) {
        let mut w = self.wind;
        if self.use_sky_wind
            && let Some(sky) = self.find_sky()
        {
            let sw = sky.get("wind").try_to::<Vector2>().unwrap_or(Vector2::ZERO);
            let sc = sky.get("cloud_scale").try_to::<f32>().unwrap_or(0.02).max(1e-3);
            w = sw / sc;
        }
        let mix = weather::mix();
        if let Some(m) = mix {
            w *= m.p.wind_scale;
        }
        self.wind_now = w * self.wind_strength;
        self.rain_now = if self.rain_override >= 0.0 { self.rain_override } else { mix.map(|m| m.p.rain).unwrap_or(0.0) }.clamp(0.0, 1.0);
    }

    fn find_sky(&self) -> Option<Gd<Node>> {
        if !self.base().is_inside_tree() {
            return None;
        }
        let mut nodes = self.base().get_tree().get_nodes_in_group("__pixel_sky");
        nodes.pop()
    }

    fn build_visuals(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        let mut this = self.to_gd().upcast::<Node3D>();
        self.flames.build(&mut this, "Flames", "fire_flame.gdshader", self.max_flames.max(16) as usize);
        self.smoke.build(&mut this, "Smoke", "puff.gdshader", self.max_smoke.max(16) as usize);
        self.steam.build(&mut this, "Steam", "puff.gdshader", (self.max_smoke.max(16) / 2) as usize);
        for l in self.lights.drain(..) {
            free_node(Some(l));
        }
        self.update_materials();
        self.refresh_pixel_settings();
    }

    fn update_materials(&mut self) {
        let flags = object_flags(1, false, 0, true).to_variant();
        if let Some(m) = &mut self.flames.material {
            m.set_shader_parameter("core_color", &self.core_color.to_variant());
            m.set_shader_parameter("flame_color", &self.flame_color.to_variant());
            m.set_shader_parameter("rim_color", &self.rim_color.to_variant());
            m.set_shader_parameter("ember_color", &self.flame_color.to_variant());
            m.set_shader_parameter("pt_flags", &flags);
        }
        if let Some(m) = &mut self.smoke.material {
            m.set_shader_parameter("smoke_color", &self.smoke_color.to_variant());
            m.set_shader_parameter("soot_color", &self.smoke_color.darkened(0.45).to_variant());
            m.set_shader_parameter("pt_flags", &flags);
        }
        if let Some(m) = &mut self.steam.material {
            m.set_shader_parameter("smoke_color", &self.steam_color.lightened(0.15).to_variant());
            m.set_shader_parameter("soot_color", &self.steam_color.to_variant());
            m.set_shader_parameter("dissolve_start", &0.2f32.to_variant());
            m.set_shader_parameter("pt_flags", &flags);
        }
    }

    fn publish_off(&mut self) {
        globals::set("pt_fire_a", &Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant());
    }

    /// Terrain-local XZ → world (with the ground height).
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
        // Steam where rain just put a fire out.
        if self.rain_now > 0.05 && self.steam_amount > 0.0 {
            let now: std::collections::HashSet<u32> = f.burning.iter().copied().collect();
            let out: Vec<u32> = prev.iter().copied().filter(|i| !now.contains(i)).collect();
            let spots: Vec<(PVec2, f32)> = out.iter().map(|&i| (f.texel_pos(i as usize), f.texels[i as usize].height)).collect();
            for (q, h) in spots {
                if self.rng.next_f32() < self.steam_amount * 0.6 {
                    self.spawn_steam(q, h);
                }
            }
        }
        // Steam while it rains on a burning fire.
        if let Some(f) = self.field.as_ref()
            && self.rain_now > 0.05
        {
            let n = f.burning.len();
            let k = ((n as f32 * self.rain_now * self.steam_amount * dt * 0.8) + self.rng.next_f32()) as usize;
            let picks: Vec<(PVec2, f32)> = (0..k.min(n))
                .map(|_| {
                    let i = f.burning[(self.rng.next_f32() * n as f32) as usize % n] as usize;
                    (f.texel_pos(i), f.texels[i].height)
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
        let started_at = if ev.fire_started { self.blaze_center() } else { None };
        for c in ev.ignited_cells {
            self.base_mut().emit_signal("cell_ignited", &[from_iv(c).to_variant()]);
        }
        for c in ev.extinguished_cells {
            self.base_mut().emit_signal("cell_extinguished", &[from_iv(c).to_variant()]);
        }
        if let Some(p) = started_at {
            self.base_mut().emit_signal("fire_started", &[p.to_variant()]);
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
            s += self.to_world(f.texel_pos(i as usize), t.height);
        }
        Some(s / f.burning.len() as f32)
    }

    fn spawn_steam(&mut self, q: PVec2, h: f32) {
        let j = v2(self.rng.signed(), self.rng.signed()) * self.texel_size * 0.5;
        let pos = self.to_world(q + j, h + 0.25);
        let seed = self.rng.next_f32();
        let w = Vector3::new(self.wind_now.x, 0.0, self.wind_now.y) * 0.15;
        let s0 = 0.25 + 0.2 * self.rng.next_f32();
        self.steam.push(Part { pos, vel: Vector3::UP * (0.5 + 0.4 * self.rng.next_f32()) + w, age: 0.0, life: 1.6 + self.rng.next_f32(), size0: s0, size1: s0 * 2.2, seed, kind: 0, heat: 0.0 });
    }

    fn upload_texture(&mut self) {
        let Some(f) = self.field.as_ref() else { return };
        f.write_rgba8(&mut self.bytes);
        let (w, h) = (f.width, f.height);
        let Some(img) = Image::create_from_data(w, h, false, Format::RGBA8, &PackedByteArray::from(self.bytes.as_slice())) else { return };
        let same = self.tex.as_ref().is_some_and(|t| t.get_width() == w && t.get_height() == h);
        if same {
            if let Some(t) = &mut self.tex {
                t.update(&img);
            }
        } else {
            self.tex = ImageTexture::create_from_image(&img);
            if let Some(t) = &self.tex {
                globals::set("pt_fire_tex", &t.to_variant());
            }
        }
        let o = self.to_world(f.origin, 0.0);
        let size = (w as f32 * f.texel, h as f32 * f.texel);
        globals::set("pt_fire_rect", &Vector4::new(o.x, o.z, 1.0 / size.0, 1.0 / size.1).to_variant());
        globals::set("pt_fire_a", &Vector4::new(1.0, f.texel, w as f32, h as f32).to_variant());
    }

    /// Fire lights on the biggest blazes: burning cells bucketed into light-range squares.
    fn update_lights(&mut self) {
        let want = self.max_lights.clamp(0, 8) as usize;
        let mut blazes: Vec<(Vector3, f32)> = Vec::new();
        if let Some(f) = self.field.as_ref()
            && want > 0
        {
            let cell = self.light_range.max(1.0) * 1.5;
            let mut buckets: std::collections::HashMap<(i32, i32), (Vector3, f32)> = std::collections::HashMap::new();
            for &i in &f.burning {
                let t = &f.texels[i as usize];
                let q = f.texel_pos(i as usize);
                let e = buckets.entry(((q.x / cell).floor() as i32, (q.y / cell).floor() as i32)).or_insert((Vector3::ZERO, 0.0));
                e.0 += Vector3::new(q.x, t.height, q.y) * t.heat;
                e.1 += t.heat;
            }
            blazes = buckets.into_values().filter(|b| b.1 > 0.05).map(|(s, w)| (s / w, w)).collect();
            blazes.sort_by(|a, b| b.1.total_cmp(&a.1));
            blazes.truncate(want);
        }
        while self.lights.len() < blazes.len() {
            let mut l = PixelLightVolume::new_alloc();
            l.set_name(&format!("FireLight{}", self.lights.len()));
            l.set("halo_size", &0.0f32.to_variant());
            l.set("volume_scale", &1.2f32.to_variant());
            l.set("volume_bands", &3i32.to_variant());
            l.set("flicker", &0.45f32.to_variant());
            l.set("light_color", &self.flame_color.to_variant());
            l.set("radius", &self.light_range.to_variant());
            l.set_as_top_level(true);
            let mut this = self.to_gd();
            add_internal(&mut this, &l);
            self.lights.push(l);
        }
        for (k, l) in self.lights.clone().iter_mut().enumerate() {
            match blazes.get(k) {
                Some((c, w)) => {
                    let pos = self.to_world(v2(c.x, c.z), c.y + 0.6);
                    l.set_global_position(pos);
                    let size = (w / 6.0).clamp(0.3, 1.0);
                    l.set("energy", &(self.light_energy * size).to_variant());
                    l.set("volume_intensity", &(self.glow_intensity * size).to_variant());
                    l.set("radius", &(self.light_range * (0.6 + 0.4 * size)).to_variant());
                    l.set_visible(true);
                }
                None => {
                    if l.is_visible() {
                        l.set_visible(false);
                    }
                }
            }
        }
    }

    fn animate(&mut self, dt: f32) {
        let wind = Vector3::new(self.wind_now.x, 0.0, self.wind_now.y);
        // Spawn flames, embers and soot from the burning cells.
        let picks: Vec<(PVec2, f32, f32)> = match self.field.as_ref() {
            Some(f) if !f.burning.is_empty() => {
                let total = f.total_heat();
                self.flame_acc += total * self.flame_rate * dt;
                self.smoke_acc += total * self.smoke_rate * dt;
                let n = f.burning.len();
                let cap = (self.flames.max as f32 * 0.6 * dt / 0.6).max(1.0);
                let k = (self.flame_acc.floor()).min(cap) as usize;
                self.flame_acc -= self.flame_acc.floor();
                let mut v = Vec::with_capacity(k);
                for _ in 0..k {
                    let i = f.burning[(self.rng.next_f32() * n as f32) as usize % n] as usize;
                    v.push((f.texel_pos(i), f.texels[i].height, f.texels[i].heat));
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
        let lean = wind * 0.25;
        for (q, h, heat) in picks {
            let j = v2(self.rng.signed(), self.rng.signed()) * texel * 0.6;
            let s0 = self.flame_size * (0.55 + 0.7 * heat) * (0.7 + 0.6 * self.rng.next_f32());
            let pos = self.to_world(q + j, h + s0 * 0.6);
            let seed = self.rng.next_f32();
            let vel = Vector3::UP * (0.8 + 0.8 * self.rng.next_f32()) * (0.6 + 0.6 * heat) + lean;
            let life = 0.45 + 0.4 * self.rng.next_f32();
            self.flames.push(Part { pos, vel, age: 0.0, life, size0: s0, size1: s0 * 0.35, seed, kind: 0, heat });
            if self.rng.next_f32() < 0.06 * heat {
                let v = Vector3::new(self.rng.signed() * 0.6, 2.0 + 1.5 * self.rng.next_f32(), self.rng.signed() * 0.6) + wind * 0.5;
                let seed = self.rng.next_f32();
                self.flames.push(Part { pos, vel: v, age: 0.0, life: 0.9 + 0.8 * self.rng.next_f32(), size0: 0.05, size1: 0.04, seed, kind: 1, heat });
            }
            if self.smoke_acc >= 1.0 {
                self.smoke_acc -= 1.0;
                let s0 = 0.2 + 0.15 * self.rng.next_f32();
                let pos = pos + Vector3::UP * 0.5;
                let seed = self.rng.next_f32();
                self.smoke.push(Part { pos, vel: Vector3::UP * 1.2 + wind * 0.2, age: 0.0, life: 2.5 + 1.5 * self.rng.next_f32(), size0: s0, size1: s0 * 3.0, seed, kind: 0, heat: 0.0 });
            }
        }
        self.smoke_acc = self.smoke_acc.min(4.0);
        let cap = (1.2f32).max(0.2);
        let mut drift = wind;
        if drift.length() > cap {
            drift = drift.normalized() * cap;
        }
        for p in &mut self.flames.parts {
            p.age += dt;
            if p.kind == 1 {
                p.vel += (wind * 0.5 - p.vel) * (0.8 * dt).min(1.0) + Vector3::DOWN * 1.2 * dt;
            }
            p.pos += p.vel * dt;
        }
        for set in [&mut self.smoke, &mut self.steam] {
            for p in &mut set.parts {
                p.age += dt;
                let k = (p.age / p.life).clamp(0.0, 1.0);
                let target = Vector3::UP * 1.1 * (1.0 - 0.5 * k) + drift * (0.3 + 0.7 * k);
                p.vel += (target - p.vel) * (1.5 * dt).min(1.0);
                p.pos += p.vel * dt;
            }
        }
        for set in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            set.parts.retain(|p| p.age < p.life);
            set.upload();
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
                d.set("cell", from_iv(f.cells[c as usize]));
            }
        }
        d
    }
}

#[godot_api]
impl PixelFireSystem {
    /// The first cells of a new fire caught (`position`: world centre of the blaze).
    #[signal]
    fn fire_started(position: Vector3);
    /// Every fire is out (burnt out or doused).
    #[signal]
    fn fire_out();
    /// A terrain cell started burning.
    #[signal]
    fn cell_ignited(cell: Vector2i);
    /// A terrain cell stopped burning.
    #[signal]
    fn cell_extinguished(cell: Vector2i);

    /// Rebuilds the fuel (grass) map from the terrain; keeps the fire state if the lattice
    /// did not change. Call after editing the terrain at runtime.
    #[func]
    fn refresh_fuel(&mut self) {
        let Some(t) = self.terrain() else { return };
        let min_h = t.get("grass_min_height").try_to::<f32>().unwrap_or(f32::MIN);
        let grass_on = t.get("grass_enabled").try_to::<bool>().unwrap_or(true);
        let xf = t.get_global_transform();
        let field = {
            let tb = t.bind();
            let slots = if grass_on { tb.texture_set_ref().map(|s| s.bind().has_grass()).unwrap_or([false; 16]) } else { [false; 16] };
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
            FireField::from_store(store, min, max, self.texel_size, &slots, min_h, 512, |p| tb.local_height_at(p))
        };
        let mut field = field;
        if let Some(old) = self.field.as_ref()
            && old.width == field.width
            && old.height == field.height
            && (old.texel - field.texel).abs() < 1e-5
        {
            for (n, o) in field.texels.iter_mut().zip(old.texels.iter()) {
                let fuel_max = n.fuel_max;
                let h = n.height;
                *n = *o;
                n.fuel_max = fuel_max;
                n.fuel = o.fuel.min(fuel_max);
                n.height = h;
            }
        }
        self.xf = xf;
        self.field = Some(field);
        self.upload_texture();
    }

    /// Lights the grass within `radius` metres of a world position. Returns the fire cells lit.
    #[func]
    fn ignite(&mut self, position: Vector3, radius: f32) -> i32 {
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let q = self.to_local(position);
        let n = self.field.as_mut().map(|f| f.ignite(q, radius)).unwrap_or(0) as i32;
        self.upload_texture();
        n
    }

    /// Lights every grass blade of a terrain cell (hex / triangle / square cell coordinates).
    #[func]
    fn ignite_cell(&mut self, cell: Vector2i) -> i32 {
        if self.field.is_none() {
            self.refresh_fuel();
        }
        let n = self.field.as_mut().map(|f| f.ignite_cell(to_iv(cell))).unwrap_or(0) as i32;
        self.upload_texture();
        n
    }

    /// Ignites `ignite_radius` around this node (editor button / quick test).
    #[func]
    fn ignite_here(&mut self) {
        let p = self.base().get_global_position();
        let r = self.ignite_radius;
        self.ignite(p, r);
    }

    /// Puts out fires within `radius` of a world position and wets the ground there.
    /// Returns how many fire cells were burning.
    #[func]
    fn extinguish(&mut self, position: Vector3, radius: f32) -> i32 {
        let q = self.to_local(position);
        let n = self.field.as_mut().map(|f| f.extinguish(q, radius, 0.7)).unwrap_or(0) as i32;
        self.upload_texture();
        n
    }

    /// Wets the ground within `radius` (0..1 wetness) without the need for rain.
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

    /// Puts out every fire (the char stays and regrows).
    #[func]
    fn extinguish_all(&mut self) {
        if let Some(f) = self.field.as_mut() {
            f.extinguish_all();
        }
        self.upload_texture();
    }

    /// Back to unburnt, dry grass everywhere; clears flames and smoke.
    #[func]
    fn reset_fire(&mut self) {
        if let Some(f) = self.field.as_mut() {
            f.reset();
        }
        for s in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            s.parts.clear();
            s.upload();
        }
        self.prev_burning.clear();
        self.upload_texture();
        self.update_lights();
    }

    /// Advances the simulation by `seconds` at the tick rate (also while `simulate` is off).
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

    /// State at a world position: burning, heat, burnt, wet, growth (0..1), grass, cell.
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
        self.field.as_ref().and_then(|f| f.texel_at(q).map(|i| f.texels[i].is_burning())).unwrap_or(false)
    }

    /// Aggregated state of a terrain cell: burning (texels), heat (max), burnt / wet / growth
    /// (means), grass (fraction of the cell with grass). Empty if the cell is unknown.
    #[func]
    fn get_cell_state(&self, cell: Vector2i) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(s) = self.field.as_ref().and_then(|f| f.cell_state(to_iv(cell))) {
            d.set("burning", s.burning as i64);
            d.set("heat", s.heat);
            d.set("burnt", s.burnt);
            d.set("wet", s.wet);
            d.set("growth", s.growth);
            d.set("grass", s.fuel);
        }
        d
    }

    /// Terrain cells burning right now.
    #[func]
    fn get_burning_cells(&self) -> Array<Vector2i> {
        self.field.as_ref().map(|f| f.burning_cells().into_iter().map(from_iv).collect()).unwrap_or_default()
    }

    /// Fire cells (texels) burning right now.
    #[func]
    fn get_burning_count(&self) -> i32 {
        self.field.as_ref().map(|f| f.burning.len() as i32).unwrap_or(0)
    }

    /// Fraction of the grass that is burnt (0..1).
    #[func]
    fn get_burnt_fraction(&self) -> f32 {
        let Some(f) = self.field.as_ref() else { return 0.0 };
        let (mut n, mut b) = (0.0f32, 0.0f32);
        for t in f.texels.iter().filter(|t| t.fuel_max > 0.0) {
            n += 1.0;
            b += t.burnt;
        }
        if n > 0.0 { b / n } else { 0.0 }
    }

    /// Live flame / smoke / steam particles.
    #[func]
    fn get_particle_count(&self) -> i32 {
        (self.flames.parts.len() + self.smoke.parts.len() + self.steam.parts.len()) as i32
    }

    /// The published state texture (R heat, G char, B wet, A growth).
    #[func]
    fn get_fire_texture(&self) -> Option<Gd<ImageTexture>> {
        self.tex.clone()
    }

    /// Re-applies the metadata layer (called when the meta layer changes).
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = globals::meta_mask();
        for s in [&mut self.flames, &mut self.smoke, &mut self.steam] {
            s.set_layers(mask);
        }
    }

    #[func]
    fn set_texel_size(&mut self, v: f32) {
        self.texel_size = v.clamp(0.1, 4.0);
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
    fn set_core_color(&mut self, v: Color) {
        self.core_color = v;
        self.update_materials();
    }
    #[func]
    fn set_flame_color(&mut self, v: Color) {
        self.flame_color = v;
        self.update_materials();
        for l in &mut self.lights {
            l.set("light_color", &v.to_variant());
        }
    }
    #[func]
    fn set_rim_color(&mut self, v: Color) {
        self.rim_color = v;
        self.update_materials();
    }
    #[func]
    fn set_smoke_color(&mut self, v: Color) {
        self.smoke_color = v;
        self.update_materials();
    }
    #[func]
    fn set_steam_color(&mut self, v: Color) {
        self.steam_color = v;
        self.update_materials();
    }
}
