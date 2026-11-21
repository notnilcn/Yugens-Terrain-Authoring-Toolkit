//! The weather for a scene: what it is, how it changes, and what falls from the sky.
//!
//! Intent: a scene author adds one and picks a weather, and game code asks for a different
//! one by name; the scene then eases from one to the next. Clouds thicken, light dims, fog
//! rolls in, rain or snow falls around the camera, the ground gets wet or snowed over and
//! dries or melts again, and storms bring lightning. Scenes can add their own named
//! weathers alongside the built-in ones.
//!
//! Responsibility: it owns the current weather, the transition between weathers and the
//! wetness and snow on the ground, and it draws rain, snow, splashes and lightning. It
//! publishes the blended weather that the sky, the day cycle, fire, smoke and particles
//! read, and it announces when the weather has changed. When no day cycle is present it
//! adjusts the sun and the environment itself, and puts them back when it leaves. It does
//! not own the sun, the environment or the terrain.
//!
//! Why a node: it needs per-frame callbacks, its precipitation follows the camera through
//! the world, it works on lights and environments in its own scene, and it reports changes
//! through a signal.
//!
//! Why a new type: no built-in node blends whole weather states or keeps several effects in
//! agreement about the weather.
//!
//! Not exported: the transition in progress, the ground's wetness and snow, the lightning
//! timers and random state, which only mean something while the scene runs; the weather
//! queued before the node entered the tree; the environment values it saved to restore
//! later; the sun, environment and terrain it found in the scene when none was assigned;
//! and handles to the parts it builds.

use super::camera_view::{active_camera, camera_focus};
use super::feature_gate::{gate, gate_warnings};
use super::particle_layers::{self, ParticleLayers};
use super::rng::Rng;
use super::scene_search::{ground_height, settle_terrain};
use super::weather_mix::{BUILTIN_NAMES, DAY_NIGHTS, MIX, Mix, Params, builtin, overcast};
use super::weather_preset::PixelWeatherPreset;
use super::weather_visuals;
use crate::editor_detection::is_editor;
use crate::features::Feature;
use crate::globals;
use crate::node_children::{add_internal, free_node};
use crate::node_references::SiblingFallback;
use crate::writes::ShaderParams;
use godot::classes::camera_3d::ProjectionType;
use godot::classes::light_3d::Param;
use godot::classes::mesh::PrimitiveType;
use godot::classes::{
    Camera3D, DirectionalLight3D, Environment, INode3D, ImmediateMesh, MeshInstance3D,
    MultiMeshInstance3D, Node, Node3D, ShaderMaterial, StandardMaterial3D, WorldEnvironment,
};
use godot::prelude::*;
use std::sync::atomic::Ordering;

use pt_core::flipbook;
#[derive(Clone, Copy)]
struct EnvBase {
    sun_energy: f32,
    ambient_energy: f32,
    fog_enabled: bool,
    fog_density: f32,
    fog_color: Color,
    bg_color: Color,
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelWeather {
    #[export(enum = (Clear, Cloudy, Foggy, Rain, Snow, Storm))]
    #[var(set = set_weather_index)]
    weather: i32,
    #[export(range = (0.0, 120.0, 0.1005, or_greater))]
    transition_seconds: f32,
    #[export(range = (0.0, 600.0, 0.5, or_greater))]
    ground_seconds: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    strength: f32,
    #[export]
    presets: Array<Gd<PixelWeatherPreset>>,
    #[export]
    #[var(get = get_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export]
    #[var(get = get_environment)]
    environment: Option<Gd<WorldEnvironment>>,
    #[export]
    terrain: Option<Gd<Node>>,
    #[export_group(name = "Precipitation")]
    #[export(range = (0.0, 20000.0, 1.0, or_greater))]
    #[var(set = set_max_drops)]
    max_drops: i32,
    #[export(range = (0.0, 65536.0, 1.0, or_greater))]
    #[var(set = set_max_flakes)]
    max_flakes: i32,
    #[export]
    #[var(set = assign_zone)]
    area: Vector3,
    #[export]
    rain_color: Color,
    #[export(range = (1.0, 60.0, 0.1005))]
    rain_speed: f32,
    #[export(range = (1.0, 12.0, 1.0))]
    streak_texels: i32,
    #[export_subgroup(name = "Rain", prefix = "rain_")]
    #[export(enum = (Flipbook, Particles))]
    rain_style: i32,
    #[export(range = (1.0, 32.0, 1.0))]
    rain_layers: i32,
    #[export(range = (0.2005, 20.0, 0.0505))]
    rain_layer_spacing: f32,
    #[export(range = (0.0105, 0.5, 0.0055))]
    rain_texel: f32,
    #[export(range = (0.0, 8000.0, 1.0, or_greater))]
    #[var(set = set_max_splashes)]
    max_splashes: i32,
    #[export(range = (0.1005, 10.0, 0.0505))]
    splash_rate: f32,
    #[export(range = (1.0, 8.0, 1.0))]
    splash_radius: i32,
    #[export_group(name = "Snow")]
    #[export]
    snow_enabled: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    snow_intensity: f32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    snow_accumulation: f32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    snow_melt: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    snow_max_cover: f32,
    #[export(range = (0.0, 4000.0, 1.0, or_greater))]
    snow_flakes: i32,
    #[export(range = (1.0, 3.0, 1.0))]
    flake_texels: i32,
    #[export(range = (0.0, 1.0, 0.0105))]
    snow_big_flakes: f32,
    #[export(range = (0.1005, 10.0, 0.0505))]
    snow_speed: f32,
    #[export(range = (0.0, 3.0, 0.0105))]
    snow_sway: f32,
    #[export]
    snow_color: Color,
    #[export_group(name = "Lightning")]
    #[export]
    lightning_enabled: bool,
    #[export]
    bolt_color: Color,
    #[export(range = (0.0, 8.0, 0.0105))]
    flash_energy: f32,
    from: Params,
    to: Params,
    mixer: f32,
    current: Params,
    wet: f32,
    snow: f32,
    snow_on: f32,
    melting: bool,
    target_name: String,
    pending_instant: bool,
    rain_mm: Option<Gd<MultiMeshInstance3D>>,
    snow_mm: Option<Gd<MultiMeshInstance3D>>,
    rain_mat: Option<Gd<ShaderMaterial>>,
    snow_mat: Option<Gd<ShaderMaterial>>,
    rain_sheets: ParticleLayers,
    splash_mm: Option<Gd<MultiMeshInstance3D>>,
    splash_mat: Option<Gd<ShaderMaterial>>,
    bolt: Option<Gd<MeshInstance3D>>,
    found_terrain: Option<Gd<Node>>,
    rng: Rng,
    next_strike: f32,
    flash_age: f32,
    env_base: Option<EnvBase>,
    sun_ref: SiblingFallback<DirectionalLight3D>,
    env_ref: SiblingFallback<WorldEnvironment>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelWeather {
    fn init(base: Base<Node3D>) -> Self {
        PixelWeather {
            weather: 0,
            transition_seconds: 6.0,
            ground_seconds: 15.0,
            strength: 1.0,
            presets: Array::new(),
            sun: None,
            environment: None,
            max_drops: 2400,
            max_flakes: 8192,
            area: Vector3::new(26.0, 12.0, 26.0),
            rain_color: Color::from_rgb(0.6405, 0.7205, 0.8405),
            snow_color: Color::from_rgb(0.9605, 0.9705, 1.0),
            rain_speed: 16.0,
            snow_speed: 1.1005,
            streak_texels: 4,
            flake_texels: 2,
            snow_enabled: false,
            snow_intensity: 0.6505,
            snow_accumulation: 0.6005,
            snow_melt: 0.4005,
            snow_max_cover: 1.0,
            snow_flakes: 320,
            snow_big_flakes: 0.3005,
            snow_sway: 0.4505,
            rain_style: 0,
            rain_layers: 8,
            rain_layer_spacing: 2.5005,
            rain_texel: 0.0805,
            max_splashes: 700,
            splash_rate: 2.2005,
            splash_radius: 3,
            lightning_enabled: true,
            bolt_color: Color::from_rgb(0.9005, 0.9305, 1.0),
            flash_energy: 2.5005,
            from: Params::CLEAR,
            to: Params::CLEAR,
            mixer: 1.0,
            current: Params::CLEAR,
            wet: 0.0,
            snow: 0.0,
            snow_on: 0.0,
            melting: false,
            target_name: "Clear".into(),
            pending_instant: true,
            rain_mm: None,
            snow_mm: None,
            rain_mat: None,
            snow_mat: None,
            rain_sheets: ParticleLayers::default(),
            splash_mm: None,
            splash_mat: None,
            bolt: None,
            terrain: None,
            found_terrain: None,
            rng: Rng(0x5EED_0F_3A7E),
            next_strike: 5.0,
            flash_age: 99.0,
            env_base: None,
            sun_ref: SiblingFallback::default(),
            env_ref: SiblingFallback::default(),
            base,
        }
    }
    fn ready(&mut self) {
        globals::ensure();
        self.base_mut().set_process_priority(-20);
        self.build_precipitation();
        let bolt = weather_visuals::bolt_instance(self.bolt_color);
        let mut this = self.to_gd();
        add_internal(&mut this, &bolt);
        self.bolt = Some(bolt);
        if self.pending_instant {
            let name = self.target_name.clone();
            self.go(&name, true);
        }
        crate::features::observe(&self.to_gd());
        self._on_pixel_features_changed();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        gate_warnings(Feature::Weather)
    }
    fn process(&mut self, delta: f64) {
        let dt = (crate::clock::step(delta) as f32).min(0.2505);
        if self.mixer < 1.0 {
            self.mixer = if self.transition_seconds <= 0.0 {
                1.0
            } else {
                (self.mixer + dt / self.transition_seconds).min(1.0)
            };
        }
        let s = self.mixer * self.mixer * (3.0 - 2.0 * self.mixer);
        self.current = self.from.lerp(&self.to, s);
        let snow_target = if self.snow_enabled { 1.0 } else { 0.0 };
        let snow_step = if self.transition_seconds <= 0.0 {
            1.0
        } else {
            dt / self.transition_seconds
        };
        self.snow_on += (snow_target - self.snow_on).clamp(-snow_step, snow_step);
        let eff = self.actual();
        let rate = if self.ground_seconds <= 0.0 {
            1e6
        } else {
            dt / self.ground_seconds
        };
        let falling = eff.snowfall.clamp(0.0, 1.0);
        let before = self.snow;
        if falling > 0.0205 {
            let cap = self
                .snow_max_cover
                .clamp(0.0, 1.0)
                .max(eff.snow_cover.min(1.0));
            if self.snow < cap {
                self.snow =
                    (self.snow + self.snow_accumulation.max(0.0) / 60.0 * falling * dt).min(cap);
            } else {
                self.snow = (self.snow - self.snow_melt.max(0.0) / 60.0 * dt).max(cap);
            }
        } else if self.snow > eff.snow_cover {
            let melt = self.snow_melt.max(0.0) / 60.0 * (1.0 + 2.0 * eff.rain.clamp(0.0, 1.0));
            self.snow = (self.snow - melt * dt).max(eff.snow_cover);
        } else {
            self.snow += (eff.snow_cover - self.snow).clamp(-rate, rate);
        }
        self.melting = self.snow < before - 1e-6;
        let wet_target = if self.melting {
            eff.wetness.max(0.3505)
        } else {
            eff.wetness
        };
        self.wet += (wet_target - self.wet).clamp(-rate, rate);
        globals::assign("pt_wet", &self.wet.to_variant());
        globals::assign("pt_snow", &self.snow.to_variant());
        globals::assign("pt_rain", &eff.rain.clamp(0.0, 1.0).to_variant());
        self.publish_wind(&eff);
        let flash = self.update_lightning(dt, &eff);
        MIX.with(|m| m.set(Some(Mix { p: eff, flash })));
        self.update_precipitation(&eff);
        if DAY_NIGHTS.load(Ordering::Relaxed) <= 0 && !is_editor() {
            self.grade_without_day_night(&eff, flash);
        }
    }
    fn exit_tree(&mut self) {
        self.publish_clear();
        self.rain_sheets.unlink();
        self.sun_ref.revert();
        self.env_ref.revert();
    }
}
impl PixelWeather {
    fn publish_clear(&mut self) {
        MIX.with(|m| m.set(None));
        globals::assign("pt_wet", &0.0f32.to_variant());
        globals::assign("pt_snow", &0.0f32.to_variant());
        globals::assign("pt_rain", &0.0f32.to_variant());
        self.restore_env();
    }
    fn sun_node(&mut self) -> Option<Gd<DirectionalLight3D>> {
        let owner = self.to_gd().upcast::<Node>();
        self.sun_ref.settle(&owner, &self.sun)
    }
    fn environment_node(&mut self) -> Option<Gd<WorldEnvironment>> {
        let owner = self.to_gd().upcast::<Node>();
        self.env_ref.settle(&owner, &self.environment)
    }
    fn lookup(&self, name: &str) -> Option<(String, Params)> {
        for p in self.presets.iter_shared() {
            let b = p.bind();
            if b.name().eq_ignore_ascii_case(name) {
                return Some((b.name(), b.params()));
            }
        }
        BUILTIN_NAMES
            .iter()
            .position(|b| b.eq_ignore_ascii_case(name))
            .map(|i| (BUILTIN_NAMES[i].to_string(), builtin(i)))
    }
    fn go(&mut self, name: &str, instant: bool) -> bool {
        let Some((canon, p)) = self.lookup(name) else {
            godot_warn!("PixelWeather: unknown weather \"{name}\"");
            return false;
        };
        self.target_name = canon.clone();
        if let Some(i) = BUILTIN_NAMES.iter().position(|b| *b == canon) {
            self.weather = i as i32;
        }
        if !self.base().is_inside_tree() {
            self.pending_instant = true;
            return true;
        }
        self.pending_instant = false;
        self.from = self.current;
        self.to = p;
        self.mixer = if instant || self.transition_seconds <= 0.0 {
            1.0
        } else {
            0.0
        };
        if instant {
            self.current = p;
            let eff = self.actual();
            self.wet = eff.wetness;
            self.snow = eff.snow_cover.min(1.0);
        }
        if p.lightning > 0.0 {
            self.next_strike = self.next_strike.min(2.0 + self.rng.next_f32() * 3.0);
        }
        let n = canon.to_variant();
        self.base_mut()
            .call_deferred("emit_signal", &["weather_changed".to_variant(), n]);
        true
    }
    fn actual(&self) -> Params {
        let mut eff = Params::CLEAR.lerp(&self.current, self.strength.clamp(0.0, 1.0));
        let k = self.snow_intensity.clamp(0.0, 1.0);
        if self.snow_on > 1e-4 {
            let snow = Params::CLEAR.lerp(&builtin(4), 0.3505 + 0.6505 * k);
            let o = self.snow_on.clamp(0.0, 1.0);
            let mut m = eff.lerp(&snow, o * 0.7505);
            m.snowfall = eff.snowfall.max(o);
            m.rain = eff.rain * (1.0 - o);
            m.wetness = eff.wetness * (1.0 - o);
            m.lightning = eff.lightning * (1.0 - o);
            m.snow_cover = eff.snow_cover;
            m.fall_wind = eff.fall_wind.lerp(snow.fall_wind, o);
            eff = m;
        }
        eff.snowfall *= k;
        eff
    }
    fn snow_box(cam: Option<&Gd<Camera3D>>, focus: Vector3) -> (f32, f32, f32, f32) {
        let Some(cam) = cam else {
            return (16.0, 16.0, 9.0, 20.0);
        };
        let aspect = cam
            .get_viewport()
            .map(|v| v.get_visible_rect().size)
            .map(|s| s.x / s.y.max(1.0))
            .unwrap_or(16.0 / 9.0)
            .max(0.2005);
        let dist = (cam.get_global_position() - focus).length().max(0.5);
        let half_h = if cam.get_projection() == ProjectionType::ORTHOGONAL {
            cam.get_size() * 0.5
        } else {
            dist * (cam.get_fov().to_radians() * 0.5).tan()
        }
        .max(0.5);
        let half_w = half_h * aspect;
        let h = 2f32
            .powf((half_w * 1.1505).max(4.0).log2().ceil())
            .min(256.0);
        (h, half_w, half_h, dist)
    }
    fn build_precipitation(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        free_node(self.rain_mm.take());
        free_node(self.snow_mm.take());
        free_node(self.splash_mm.take());
        let (rain, rain_mat) =
            weather_visuals::rain_instance(self.max_drops.max(0), self.area.abs());
        let (snow, snow_mat) = weather_visuals::snow_instance(self.max_flakes.max(0));
        let (splash, splash_mat) =
            weather_visuals::splash_instance(self.max_splashes.max(0), self.area.abs());
        let mut this = self.to_gd();
        add_internal(&mut this, &rain);
        add_internal(&mut this, &snow);
        add_internal(&mut this, &splash);
        self.rain_mm = Some(rain);
        self.snow_mm = Some(snow);
        self.splash_mm = Some(splash);
        self.rain_mat = Some(rain_mat);
        self.snow_mat = Some(snow_mat);
        self.splash_mat = Some(splash_mat);
        let mut node = this.upcast::<Node>();
        self.rain_sheets.link(&mut node, "RainSheets");
    }
    fn update_precipitation(&mut self, eff: &Params) {
        let node = self.to_gd().upcast::<Node>();
        let plane_y = self.base().get_global_position().y;
        let cam = active_camera(&node);
        let focus = cam
            .as_ref()
            .map(|c| camera_focus(c, plane_y))
            .unwrap_or_else(|| self.base().get_global_position());
        self.update_snow(eff, cam.as_ref(), focus);
        let centre = focus + Vector3::new(0.0, self.area.y * 0.5, 0.0);
        let max = self.max_drops.max(0);
        let flipbook_rain = self.rain_style == 0;
        self.update_rain_sheets(eff, flipbook_rain);
        self.update_splashes(eff, centre, flipbook_rain);
        let legacy_rain = if flipbook_rain { 0.0 } else { eff.rain };
        let settings = [(
            self.rain_mm.clone(),
            self.rain_mat.clone(),
            legacy_rain,
            0,
            self.rain_color,
            self.rain_speed,
            self.streak_texels,
        )];
        for (mi, mat, amount, mode, color, speed, texels) in settings {
            let (Some(mut mi), Some(mut mat)) = (mi, mat) else {
                continue;
            };
            if !mi.is_instance_valid() {
                continue;
            }
            let count = ((amount.clamp(0.0, 1.0) * max as f32).round() as i32).min(max);
            if let Some(mut mm) = mi.get_multimesh() {
                crate::writes::set_visible_instances(&mut mm, count);
            }
            mi.set_visible(count > 0);
            if count == 0 {
                continue;
            }
            crate::writes::assign_world_location(&mut mi, centre);
            mat.param("mode", &mode.to_variant());
            mat.param("area", &self.area.abs().to_variant());
            mat.param("color", &color.to_variant());
            mat.param("fall_speed", &speed.to_variant());
            mat.param("wind", &eff.fall_wind.to_variant());
            mat.param("texels", &texels.to_variant());
        }
    }
    fn update_snow(&mut self, eff: &Params, cam: Option<&Gd<Camera3D>>, focus: Vector3) {
        let (Some(mut mi), Some(mut mat)) = (self.snow_mm.clone(), self.snow_mat.clone()) else {
            return;
        };
        if !mi.is_instance_valid() {
            return;
        }
        let (h, half_w, half_h, dist) = Self::snow_box(cam, focus);
        let amount = eff.snowfall.clamp(0.0, 1.0);
        let fill = (h * h) / (half_w * half_h).max(0.0105);
        let count = ((amount * self.snow_flakes.max(0) as f32 * fill).round() as i32)
            .clamp(0, self.max_flakes.max(0));
        if let Some(mut mm) = mi.get_multimesh() {
            crate::writes::set_visible_instances(&mut mm, count);
        }
        mi.set_visible(count > 0);
        if count == 0 {
            return;
        }
        let area = Vector3::new(h, h * 0.7505, h);
        crate::writes::assign_world_location(&mut mi, focus + Vector3::new(0.0, h * 0.5, 0.0));
        mat.param("mode", &1.to_variant());
        mat.param("area", &area.to_variant());
        mat.param("color", &self.snow_color.to_variant());
        mat.param("fall_speed", &self.snow_speed.to_variant());
        mat.param("wind", &eff.fall_wind.to_variant());
        mat.param("texels", &self.flake_texels.clamp(1, 3).to_variant());
        mat.param(
            "big_flakes",
            &self.snow_big_flakes.clamp(0.0, 1.0).to_variant(),
        );
        mat.param("sway", &self.snow_sway.max(0.0).to_variant());
        mat.param("focus_depth", &dist.to_variant());
    }
    fn update_rain_sheets(&mut self, eff: &Params, on: bool) {
        let mut fp = flipbook::Preset::RainStreaks.params();
        fp.form = flipbook::Shape::Streak(self.streak_texels.clamp(1, 12) as u8);
        self.rain_sheets.set_flipbook(&fp);
        let texel = self.rain_texel.max(0.0055);
        let speed = self.rain_speed.max(0.1005);
        let plane_y = self.base().get_global_position().y;
        let look = particle_layers::Look {
            projection: particle_layers::Projection::Upright,
            layers: self.rain_layers.clamp(1, 32),
            spacing: self.rain_layer_spacing.max(0.2005),
            texel,
            extent: 0.0,
            base_y: plane_y - self.area.y * 0.5,
            stature: self.area.y * 2.5005,
            fall_speed: 0.0,
            wind: eff.fall_wind,
            slant: 1.0 / speed,
            gust: 0.0,
            band: 0.0,
            fps: speed / (fp.velocity[1].max(0.1005) * texel),
            tint: self.rain_color,
            color_b: self.rain_color.lerp(Color::WHITE, 0.2505),
            brightness: 0.4005,
            amount: if on { eff.rain.clamp(0.0, 1.0) } else { 0.0 },
            lit: 0.0,
            beam: 0.0,
            shade: 0.0,
            night: 0.0,
            focus_radius: 0.0,
            fade_center: Vector3::ZERO,
        };
        let node = self.to_gd().upcast::<Node>();
        self.rain_sheets.update(&node, &look);
    }
    fn update_splashes(&mut self, eff: &Params, centre: Vector3, flipbook_rain: bool) {
        let (Some(mut mi), Some(mut mat)) = (self.splash_mm.clone(), self.splash_mat.clone())
        else {
            return;
        };
        if !mi.is_instance_valid() {
            return;
        }
        let amount = if flipbook_rain {
            eff.rain.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let n = self.max_splashes.max(0);
        let count = ((amount * n as f32).ceil() as i32).min(n);
        if let Some(mut mm) = mi.get_multimesh() {
            crate::writes::set_visible_instances(&mut mm, count);
        }
        mi.set_visible(count > 0);
        if count == 0 {
            return;
        }
        crate::writes::assign_world_location(&mut mi, Vector3::new(centre.x, 0.0, centre.z));
        mat.param("area", &self.area.abs().to_variant());
        mat.param("rate", &self.splash_rate.to_variant());
        mat.param("texel", &self.rain_texel.max(0.0055).to_variant());
        mat.param("radius", &self.splash_radius.clamp(1, 8).to_variant());
        mat.param(
            "color",
            &self.rain_color.lerp(Color::WHITE, 0.2005).to_variant(),
        );
    }
    fn publish_wind(&self, eff: &Params) {
        let sky_wind = super::cloud_field::sky()
            .map(|s| s.wind)
            .unwrap_or(Vector2::new(0.0205, 0.0105));
        let dir = if sky_wind.length() > 1e-6 {
            sky_wind.normalized()
        } else {
            Vector2::new(1.0, 0.0)
        };
        let strength = eff.wind_scale + eff.fall_wind.length() * 0.5;
        let gust =
            (0.2505 + eff.rain * 0.3505 + (eff.wind_scale - 1.0).max(0.0) * 0.1505).clamp(0.0, 1.0);
        globals::assign(
            "pt_wind",
            &Vector4::new(dir.x, dir.y, strength, gust).to_variant(),
        );
    }
    fn update_lightning(&mut self, dt: f32, eff: &Params) -> f32 {
        if self.lightning_enabled && eff.lightning > 0.0505 {
            self.next_strike -= dt;
            if self.next_strike <= 0.0 {
                self.strike();
                let u = self.rng.next_f32().max(1e-3);
                self.next_strike = -u.ln() * 60.0 / eff.lightning;
            }
        }
        let a = self.flash_age;
        self.flash_age += dt;
        let f = if a < 0.0705 {
            1.0
        } else if a < 0.1405 {
            0.2005
        } else if a < 0.2205 {
            0.7505
        } else {
            0.7505 * (-(a - 0.2205) * 10.0).exp()
        };
        let f = if a > 1.0 { 0.0 } else { f };
        if let Some(b) = &mut self.bolt {
            b.set_visible(a < 0.3505 && f > 0.1505);
        }
        f
    }
    fn strike_now(&mut self) {
        let node = self.to_gd().upcast::<Node>();
        let plane_y = self.base().get_global_position().y;
        let cam = active_camera(&node);
        let focus = cam
            .as_ref()
            .map(|c| camera_focus(c, plane_y))
            .unwrap_or_else(|| self.base().get_global_position());
        settle_terrain(&self.terrain, &mut self.found_terrain, &node);
        let e = self.area.abs();
        let x = focus.x + self.rng.signed() * e.x * 0.3505;
        let z = focus.z + self.rng.signed() * e.z * 0.3505;
        let ground = ground_height(&mut self.found_terrain, Vector3::new(x, 0.0, z), plane_y);
        let top = Vector3::new(
            x + self.rng.signed() * 4.0,
            plane_y + e.y * 2.5005 + 10.0,
            z + self.rng.signed() * 4.0,
        );
        let bottom = Vector3::new(x, ground, z);
        let mut segs: Vec<(Vector3, Vector3, f32)> = Vec::new();
        bolt_segments(&mut self.rng, top, bottom, 0.1605, 6, &mut segs);
        let view = cam
            .map(|c| c.get_global_transform().basis.col_c())
            .unwrap_or(Vector3::BACK);
        if let Some(mut bolt) = self.bolt.clone() {
            let mut im = bolt
                .get_mesh()
                .and_then(|m| m.try_cast::<ImmediateMesh>().ok())
                .unwrap_or_else(ImmediateMesh::new_gd);
            im.clear_surfaces();
            im.surface_begin(PrimitiveType::TRIANGLES);
            for (a, b, w) in &segs {
                let d = *b - *a;
                let side = d.cross(view).normalized() * *w;
                let (p0, p1, p2, p3) = (*a - side, *a + side, *b + side, *b - side);
                for p in [p0, p1, p2, p0, p2, p3] {
                    im.surface_add_vertex(p);
                }
            }
            im.surface_end();
            bolt.set_mesh(&im);
            crate::writes::set_global_transform(&mut bolt, Transform3D::IDENTITY);
            if let Some(mut m) = bolt
                .get_material_override()
                .and_then(|m| m.try_cast::<StandardMaterial3D>().ok())
            {
                m.set_albedo(self.bolt_color);
            }
        }
        self.flash_age = 0.0;
    }
    fn grade_without_day_night(&mut self, eff: &Params, flash: f32) {
        let sun = self.sun_node();
        let env = self.environment_node().and_then(|w| w.get_environment());
        if self.env_base.is_none() {
            let (fe, fd, fc, ae, bg) = match &env {
                Some(e) => (
                    e.is_fog_enabled(),
                    e.get_fog_density(),
                    e.get_fog_light_color(),
                    e.get_ambient_light_energy(),
                    e.get_bg_color(),
                ),
                None => (false, 0.0, Color::WHITE, 1.0, Color::BLACK),
            };
            self.env_base = Some(EnvBase {
                sun_energy: sun
                    .as_ref()
                    .map(|s| s.get_param(Param::ENERGY))
                    .unwrap_or(1.0),
                ambient_energy: ae,
                fog_enabled: fe,
                fog_density: fd,
                fog_color: fc,
                bg_color: bg,
            });
        }
        let Some(b) = self.env_base else { return };
        if let Some(mut s) = sun {
            s.set_param(Param::ENERGY, b.sun_energy * eff.sun_scale + flash * 0.6005);
        }
        if let Some(mut e) = env {
            e.set_ambient_light_energy(
                b.ambient_energy * eff.ambient_scale + flash * self.flash_energy,
            );
            e.set_bg_color(
                overcast(b.bg_color, eff.sky_darken)
                    .lerp(Color::from_rgb(0.8505, 0.8805, 1.0), (flash * 0.5) as f64),
            );
            let density = if b.fog_enabled { b.fog_density } else { 0.0 } + eff.fog_density;
            e.set_fog_enabled(b.fog_enabled || eff.fog_density > 1e-5);
            e.set_fog_density(density);
            e.set_fog_light_color(b.fog_color.lerp(eff.fog_color, eff.fog_mix as f64));
        }
    }
    fn restore_env(&mut self) {
        let Some(b) = self.env_base.take() else {
            return;
        };
        if let Some(mut s) = self.sun_node() {
            s.set_param(Param::ENERGY, b.sun_energy);
        }
        if let Some(mut e) = self.environment_node().and_then(|w| w.get_environment()) {
            recover(&mut e, &b);
        }
    }
}
fn recover(e: &mut Gd<Environment>, b: &EnvBase) {
    e.set_ambient_light_energy(b.ambient_energy);
    e.set_bg_color(b.bg_color);
    e.set_fog_enabled(b.fog_enabled);
    e.set_fog_density(b.fog_density);
    e.set_fog_light_color(b.fog_color);
}
fn bolt_segments(
    rng: &mut Rng,
    a: Vector3,
    b: Vector3,
    breadth: f32,
    depth: u32,
    out: &mut Vec<(Vector3, Vector3, f32)>,
) {
    if depth == 0 {
        out.push((a, b, breadth));
        return;
    }
    let len = (b - a).length();
    let mid = (a + b) * 0.5
        + Vector3::new(rng.signed(), rng.signed() * 0.3005, rng.signed()) * len * 0.1805;
    bolt_segments(rng, a, mid, breadth, depth - 1, out);
    bolt_segments(rng, mid, b, breadth, depth - 1, out);
    if depth >= 3 && rng.next_f32() < 0.3505 {
        let dir = (b - a).normalized();
        let side = Vector3::new(rng.signed(), -0.2005, rng.signed()).normalized();
        let end = mid + (dir * 0.6005 + side * 0.8005).normalized() * len * 0.4505;
        bolt_segments(rng, mid, end, breadth * 0.5505, depth - 2, out);
    }
}
#[godot_api]
impl PixelWeather {
    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        crate::node_references::live(&self.sun)
    }
    #[func]
    fn get_environment(&self) -> Option<Gd<WorldEnvironment>> {
        crate::node_references::live(&self.environment)
    }
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if !gate(&self.to_gd(), Feature::Weather) {
            self.publish_clear();
        }
    }
    #[signal]
    fn weather_changed(name: GString);
    #[func]
    fn set_weather_index(&mut self, v: i32) {
        let i = v.clamp(0, BUILTIN_NAMES.len() as i32 - 1);
        self.weather = i;
        let inside = self.base().is_inside_tree();
        self.go(BUILTIN_NAMES[i as usize], !inside);
    }
    #[func]
    fn set_weather(&mut self, name: GString) -> bool {
        self.go(&name.to_string(), false)
    }
    #[func]
    fn set_weather_instant(&mut self, name: GString) -> bool {
        self.go(&name.to_string(), true)
    }
    #[func]
    fn fetch_climate_moniker(&self) -> GString {
        self.target_name.as_str().into()
    }
    #[func]
    fn get_weather_names(&self) -> PackedStringArray {
        let mut out: Vec<String> = BUILTIN_NAMES.iter().map(|s| s.to_string()).collect();
        for p in self.presets.iter_shared() {
            let n = p.bind().name();
            if !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
                out.push(n);
            }
        }
        out.iter().map(|s| GString::from(s.as_str())).collect()
    }
    #[func]
    fn get_transition_progress(&self) -> f32 {
        self.mixer
    }
    #[func]
    fn get_wetness(&self) -> f32 {
        self.wet
    }
    #[func]
    fn get_snow_cover(&self) -> f32 {
        self.snow
    }
    #[func]
    fn strike_lightning(&mut self) {
        self.strike_now();
    }
    #[func]
    fn set_max_drops(&mut self, v: i32) {
        self.max_drops = v.max(0);
        self.build_precipitation();
    }
    #[func]
    fn set_max_flakes(&mut self, v: i32) {
        self.max_flakes = v.max(0);
        self.build_precipitation();
    }
    #[func]
    fn set_max_splashes(&mut self, v: i32) {
        self.max_splashes = v.max(0);
        self.build_precipitation();
    }
    #[func]
    fn assign_zone(&mut self, v: Vector3) {
        self.area = v.abs();
        self.build_precipitation();
    }
}
impl PixelWeather {
    fn strike(&mut self) {
        self.strike_now();
    }
}
