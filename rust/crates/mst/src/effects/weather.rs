//! `PixelWeather` + `PixelWeatherPreset` — blended weather states (clouds, light, fog, ground
//! wetness / snow, pixel rain and snow, lightning).
//!
//! Design after the ideas in docs/port/03-refs-features.md §1.5 / #5 (Enviro-style
//! weather types with blended transitions). The weather does not own the lighting: it publishes
//! a [`Mix`] of *offsets and multipliers* (Clear = identity) that `PixelSky` and `PixelDayNight`
//! fold into their own values every frame, so the sky/day-night settings stay authoritative and
//! nothing is written back into the scene. Without a `PixelDayNight` the weather grades the
//! sun / environment itself, in the running game only.

use std::cell::Cell;
use std::sync::atomic::{AtomicI32, Ordering};

use godot::classes::base_material_3d::{CullMode, ShadingMode};
use godot::classes::camera_3d::ProjectionType;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::light_3d::Param;
use godot::classes::mesh::PrimitiveType;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    Camera3D, DirectionalLight3D, Environment, INode3D, IResource, ImmediateMesh, MeshInstance3D, MultiMesh, MultiMeshInstance3D, Node,
    Node3D, QuadMesh, ShaderMaterial, StandardMaterial3D, WorldEnvironment,
};
use godot::prelude::*;

use super::particle_field::{self, ParticleLayers};
use super::{Rng, active_camera, camera_focus, find_terrain, ground_height};
use pt_core::flipbook;
use crate::globals;
use crate::util::*;

/// Every blended weather quantity. Sky/light values are offsets or multipliers on top of
/// `PixelSky` / `PixelDayNight`, so `Params::CLEAR` changes nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Added to PixelSky coverage / softness / shadow strength / deck opacity.
    pub coverage: f32,
    pub softness: f32,
    pub shadow: f32,
    pub deck_opacity: f32,
    /// 0..1: cloud deck towards storm grey.
    pub deck_darken: f32,
    /// Multiplies the cloud drift (and smoke wind).
    pub wind_scale: f32,
    /// Multipliers on the sun and ambient energy.
    pub sun_scale: f32,
    pub ambient_scale: f32,
    /// 0..1: sky / background towards an overcast grey.
    pub sky_darken: f32,
    /// Added to the fog density; fog color blended by `fog_mix`.
    pub fog_density: f32,
    pub fog_color: Color,
    pub fog_mix: f32,
    /// Ground targets (pt_wet / pt_snow follow them over `ground_seconds`).
    pub wetness: f32,
    pub snow_cover: f32,
    /// Precipitation density 0..1 (fraction of `max_drops`).
    pub rain: f32,
    pub snowfall: f32,
    /// Horizontal drift of falling rain / snow (m/s).
    pub fall_wind: Vector2,
    /// Strikes per minute.
    pub lightning: f32,
}

impl Params {
    pub const CLEAR: Params = Params {
        coverage: 0.0,
        softness: 0.0,
        shadow: 0.0,
        deck_opacity: 0.0,
        deck_darken: 0.0,
        wind_scale: 1.0,
        sun_scale: 1.0,
        ambient_scale: 1.0,
        sky_darken: 0.0,
        fog_density: 0.0,
        fog_color: Color { r: 0.75, g: 0.78, b: 0.82, a: 1.0 },
        fog_mix: 0.0,
        wetness: 0.0,
        snow_cover: 0.0,
        rain: 0.0,
        snowfall: 0.0,
        fall_wind: Vector2::ZERO,
        lightning: 0.0,
    };

    pub fn lerp(&self, o: &Params, t: f32) -> Params {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Params {
            coverage: l(self.coverage, o.coverage),
            softness: l(self.softness, o.softness),
            shadow: l(self.shadow, o.shadow),
            deck_opacity: l(self.deck_opacity, o.deck_opacity),
            deck_darken: l(self.deck_darken, o.deck_darken),
            wind_scale: l(self.wind_scale, o.wind_scale),
            sun_scale: l(self.sun_scale, o.sun_scale),
            ambient_scale: l(self.ambient_scale, o.ambient_scale),
            sky_darken: l(self.sky_darken, o.sky_darken),
            fog_density: l(self.fog_density, o.fog_density),
            // Fog color only matters while fog is mixed in; blend it towards the side that has fog.
            fog_color: self.fog_color.lerp(o.fog_color, t as f64),
            fog_mix: l(self.fog_mix, o.fog_mix),
            wetness: l(self.wetness, o.wetness),
            snow_cover: l(self.snow_cover, o.snow_cover),
            rain: l(self.rain, o.rain),
            snowfall: l(self.snowfall, o.snowfall),
            fall_wind: self.fall_wind.lerp(o.fall_wind, t),
            lightning: l(self.lightning, o.lightning),
        }
    }
}

/// Snowflake instances allocated once (only `visible_instance_count` of them are drawn).
const SNOW_INSTANCES: i32 = 8192;

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

/// Built-in weather names, in `PixelWeather.weather` enum order.
pub const BUILTIN_NAMES: [&str; 6] = ["Clear", "Cloudy", "Foggy", "Rain", "Snow", "Storm"];

/// Built-in weather states (values after Enviro's Clear / Cloudy / Foggy / Rain / Snow types).
pub fn builtin(i: usize) -> Params {
    let c = Params::CLEAR;
    match i {
        1 => Params {
            coverage: 0.28,
            softness: 0.05,
            shadow: 0.12,
            deck_opacity: 0.15,
            deck_darken: 0.2,
            wind_scale: 1.4,
            sun_scale: 0.75,
            ambient_scale: 0.95,
            sky_darken: 0.3,
            fog_density: 0.002,
            fog_color: rgb(0.7, 0.74, 0.8),
            fog_mix: 0.3,
            ..c
        },
        2 => Params {
            coverage: 0.12,
            softness: 0.1,
            shadow: -0.25,
            deck_darken: 0.1,
            wind_scale: 0.4,
            sun_scale: 0.55,
            ambient_scale: 1.1,
            sky_darken: 0.5,
            fog_density: 0.03,
            fog_color: rgb(0.76, 0.79, 0.82),
            fog_mix: 0.85,
            wetness: 0.15,
            ..c
        },
        3 => Params {
            coverage: 0.45,
            softness: 0.1,
            shadow: 0.2,
            deck_opacity: 0.3,
            deck_darken: 0.45,
            wind_scale: 2.0,
            sun_scale: 0.45,
            ambient_scale: 0.8,
            sky_darken: 0.65,
            fog_density: 0.01,
            fog_color: rgb(0.5, 0.55, 0.62),
            fog_mix: 0.65,
            wetness: 1.0,
            rain: 0.7,
            fall_wind: Vector2::new(1.5, 0.6),
            ..c
        },
        4 => Params {
            coverage: 0.35,
            softness: 0.12,
            shadow: 0.0,
            deck_opacity: 0.2,
            deck_darken: 0.1,
            wind_scale: 0.8,
            sun_scale: 0.65,
            ambient_scale: 1.0,
            sky_darken: 0.45,
            fog_density: 0.006,
            fog_color: rgb(0.86, 0.89, 0.95),
            fog_mix: 0.5,
            snow_cover: 0.8,
            // Scaled by PixelWeather.snow_intensity (0.65 by default).
            snowfall: 1.0,
            fall_wind: Vector2::new(0.6, 0.25),
            ..c
        },
        5 => Params {
            coverage: 0.6,
            softness: 0.15,
            shadow: 0.3,
            deck_opacity: 0.4,
            deck_darken: 0.65,
            wind_scale: 3.5,
            sun_scale: 0.3,
            ambient_scale: 0.5,
            sky_darken: 0.8,
            fog_density: 0.014,
            fog_color: rgb(0.34, 0.37, 0.44),
            fog_mix: 0.7,
            wetness: 1.0,
            rain: 1.0,
            fall_wind: Vector2::new(3.5, 1.2),
            lightning: 7.0,
            ..c
        },
        _ => c,
    }
}

/// What the sky / day-night nodes fold in this frame.
#[derive(Clone, Copy, Debug)]
pub struct Mix {
    pub p: Params,
    /// Lightning flash 0..1.
    pub flash: f32,
}

thread_local! {
    static MIX: Cell<Option<Mix>> = const { Cell::new(None) };
}
static DAY_NIGHTS: AtomicI32 = AtomicI32::new(0);

/// The current weather mix, if a `PixelWeather` is active.
pub fn mix() -> Option<Mix> {
    MIX.with(|m| m.get())
}

/// `PixelDayNight` registers itself so the weather knows who grades the lighting.
pub fn day_night_entered(on: bool) {
    DAY_NIGHTS.fetch_add(if on { 1 } else { -1 }, Ordering::Relaxed);
}

/// Color towards an overcast grey of similar brightness.
pub fn overcast(c: Color, amount: f32) -> Color {
    let l = c.r * 0.3 + c.g * 0.55 + c.b * 0.15;
    c.lerp(Color::from_rgb(l * 0.82, l * 0.86, l * 0.95), amount.clamp(0.0, 1.0) as f64)
}

// ---------------------------------------------------------------------------------------------

/// One weather state for `PixelWeather`. Sky and light values are offsets / multipliers on top
/// of the scene's `PixelSky` and `PixelDayNight` (all zero / one = no change), so a preset
/// works with any time of day.
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelWeatherPreset {
    /// Name used by `PixelWeather.set_weather()`; a preset named like a built-in replaces it.
    #[export]
    preset_name: GString,

    #[export_group(name = "Clouds")]
    /// Added to PixelSky coverage.
    #[export(range = (-1.0, 1.0, 0.01))]
    cloud_coverage: f32,
    #[export(range = (-0.5, 0.5, 0.005))]
    cloud_softness: f32,
    /// Added to the cloud shadow strength.
    #[export(range = (-1.0, 1.0, 0.01))]
    cloud_shadow: f32,
    #[export(range = (-1.0, 1.0, 0.01))]
    deck_opacity: f32,
    /// Darkens the cloud deck towards storm grey.
    #[export(range = (0.0, 1.0, 0.01))]
    deck_darken: f32,
    /// Multiplies the cloud drift (also blows smoke).
    #[export(range = (0.0, 8.0, 0.01))]
    wind_scale: f32,

    #[export_group(name = "Light")]
    #[export(range = (0.0, 2.0, 0.01))]
    sun_scale: f32,
    #[export(range = (0.0, 2.0, 0.01))]
    ambient_scale: f32,
    /// Sky / background towards overcast grey.
    #[export(range = (0.0, 1.0, 0.01))]
    sky_darken: f32,

    #[export_group(name = "Fog")]
    /// Added to the environment fog density.
    #[export(range = (0.0, 0.1, 0.0005))]
    fog_density: f32,
    #[export]
    fog_color: Color,
    /// How much the fog takes `fog_color` (0 = day/night fog color).
    #[export(range = (0.0, 1.0, 0.01))]
    fog_color_mix: f32,

    #[export_group(name = "Ground")]
    /// Ground wetness target (darker ground, puddles, rain rings on water).
    #[export(range = (0.0, 1.0, 0.01))]
    wetness: f32,
    /// Snow cover target on floors.
    #[export(range = (0.0, 1.0, 0.01))]
    snow_cover: f32,

    #[export_group(name = "Precipitation")]
    /// Rain density (fraction of PixelWeather.max_drops).
    #[export(range = (0.0, 1.0, 0.01))]
    rain: f32,
    /// Snowfall density (scaled by PixelWeather.snow_intensity).
    #[export(range = (0.0, 1.0, 0.01))]
    snowfall: f32,
    /// Horizontal drift of falling drops / flakes (m/s).
    #[export]
    fall_wind: Vector2,

    #[export_group(name = "Lightning", prefix = "lightning_")]
    /// Lightning strikes per minute (0 = none).
    #[export(range = (0.0, 60.0, 0.1))]
    lightning_per_minute: f32,

    base: Base<Resource>,
}

#[godot_api]
impl IResource for PixelWeatherPreset {
    fn init(base: Base<Resource>) -> Self {
        let mut p = PixelWeatherPreset {
            preset_name: "Custom".into(),
            cloud_coverage: 0.0,
            cloud_softness: 0.0,
            cloud_shadow: 0.0,
            deck_opacity: 0.0,
            deck_darken: 0.0,
            wind_scale: 1.0,
            sun_scale: 1.0,
            ambient_scale: 1.0,
            sky_darken: 0.0,
            fog_density: 0.0,
            fog_color: Params::CLEAR.fog_color,
            fog_color_mix: 0.0,
            wetness: 0.0,
            snow_cover: 0.0,
            rain: 0.0,
            snowfall: 0.0,
            fall_wind: Vector2::ZERO,
            lightning_per_minute: 0.0,
            base,
        };
        p.load(&Params::CLEAR);
        p
    }
}

impl PixelWeatherPreset {
    fn load(&mut self, p: &Params) {
        self.cloud_coverage = p.coverage;
        self.cloud_softness = p.softness;
        self.cloud_shadow = p.shadow;
        self.deck_opacity = p.deck_opacity;
        self.deck_darken = p.deck_darken;
        self.wind_scale = p.wind_scale;
        self.sun_scale = p.sun_scale;
        self.ambient_scale = p.ambient_scale;
        self.sky_darken = p.sky_darken;
        self.fog_density = p.fog_density;
        self.fog_color = p.fog_color;
        self.fog_color_mix = p.fog_mix;
        self.wetness = p.wetness;
        self.snow_cover = p.snow_cover;
        self.rain = p.rain;
        self.snowfall = p.snowfall;
        self.fall_wind = p.fall_wind;
        self.lightning_per_minute = p.lightning;
    }

    pub fn params(&self) -> Params {
        Params {
            coverage: self.cloud_coverage,
            softness: self.cloud_softness,
            shadow: self.cloud_shadow,
            deck_opacity: self.deck_opacity,
            deck_darken: self.deck_darken,
            wind_scale: self.wind_scale,
            sun_scale: self.sun_scale,
            ambient_scale: self.ambient_scale,
            sky_darken: self.sky_darken,
            fog_density: self.fog_density,
            fog_color: self.fog_color,
            fog_mix: self.fog_color_mix,
            wetness: self.wetness,
            snow_cover: self.snow_cover,
            rain: self.rain,
            snowfall: self.snowfall,
            fall_wind: self.fall_wind,
            lightning: self.lightning_per_minute,
        }
    }
}

#[godot_api]
impl PixelWeatherPreset {
    /// A new preset holding the values of a built-in weather ("Clear", "Cloudy", "Foggy",
    /// "Rain", "Snow", "Storm"); unknown names give Clear.
    #[func]
    fn from_builtin(name: GString) -> Gd<PixelWeatherPreset> {
        let n = name.to_string();
        let i = BUILTIN_NAMES.iter().position(|b| b.eq_ignore_ascii_case(&n)).unwrap_or(0);
        let mut p = PixelWeatherPreset::new_gd();
        {
            let mut b = p.bind_mut();
            b.preset_name = BUILTIN_NAMES[i].into();
            b.load(&builtin(i));
        }
        p
    }
}

// ---------------------------------------------------------------------------------------------

/// Values captured from the environment when the weather grades it itself (no PixelDayNight).
#[derive(Clone, Copy)]
struct EnvBase {
    sun_energy: f32,
    ambient_energy: f32,
    fog_enabled: bool,
    fog_density: f32,
    fog_color: Color,
    bg_color: Color,
}

/// Weather for the pixel world: blends `PixelWeatherPreset`s over `transition_seconds` and
/// drives the cloud field (through `PixelSky`), light / sky / fog (through `PixelDayNight`),
/// ground wetness and snow (`pt_wet` / `pt_snow` in the terrain, grass and water shaders), rain
/// (prerendered flipbook streak sheets anchored in the world, see `PixelParticleField`, plus
/// pixel splash rings on the ground and the water), optional snow (pixel flakes in a box sized
/// with the view, ground cover that builds up and melts; off by default), lightning (flash +
/// bolt) and the global wind (`pt_wind`, `pt_rain`).
///
/// Place the node at about ground height: the precipitation box is centred where the camera
/// looks at that height. Change the weather with `weather` (built-ins) or `set_weather(name)`
/// (also custom presets in `presets`).
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelWeather {
    /// Current (target) built-in weather.
    #[export(enum = (Clear, Cloudy, Foggy, Rain, Snow, Storm))]
    #[var(set = set_weather_index)]
    weather: i32,
    /// Seconds to blend from one weather to the next.
    #[export(range = (0.0, 120.0, 0.1, or_greater))]
    transition_seconds: f32,
    /// Seconds for the ground to soak / dry or to whiten / thaw.
    #[export(range = (0.0, 600.0, 0.5, or_greater))]
    ground_seconds: f32,
    /// Overall weather intensity (0 = always clear).
    #[export(range = (0.0, 1.0, 0.01))]
    strength: f32,
    /// Custom weather states. A preset named like a built-in replaces it; others are reachable
    /// through `set_weather(name)`.
    #[export]
    presets: Array<Gd<PixelWeatherPreset>>,
    /// Light to flash and dim. Empty = first DirectionalLight3D among this node's siblings.
    #[export]
    sun: Option<Gd<DirectionalLight3D>>,
    /// Environment graded when there is no PixelDayNight (running game only).
    #[export]
    environment: Option<Gd<WorldEnvironment>>,

    #[export_group(name = "Precipitation")]
    /// Drops / flakes at density 1.
    #[export(range = (0.0, 20000.0, 1.0, or_greater))]
    #[var(set = set_max_drops)]
    max_drops: i32,
    /// Half-size of the box around the camera focus that holds the rain (snow sizes its own box with the view).
    #[export]
    #[var(set = set_area)]
    area: Vector3,
    #[export]
    rain_color: Color,
    /// Rain fall speed (m/s).
    #[export(range = (1.0, 60.0, 0.1))]
    rain_speed: f32,
    /// Rain streak length in low-res texels.
    #[export(range = (1.0, 12.0, 1.0))]
    streak_texels: i32,
    /// Rain look: Flipbook = prerendered streak sheets on world-anchored upright layers (one
    /// texture fetch per texel, `particle_field.gdshader`) plus pixel splashes on the ground and
    /// the water; Particles = the older per-drop MultiMesh streaks.
    #[export_subgroup(name = "Rain", prefix = "rain_")]
    #[export(enum = (Flipbook, Particles))]
    rain_style: i32,
    /// Flipbook rain: number of upright streak sheets and their spacing (m).
    #[export(range = (1.0, 32.0, 1.0))]
    rain_layers: i32,
    #[export(range = (0.2, 20.0, 0.05))]
    rain_layer_spacing: f32,
    /// Flipbook rain: world size of one streak / splash texel (m).
    #[export(range = (0.01, 0.5, 0.005))]
    rain_texel: f32,
    /// Ground / water splashes at full rain (0 = none).
    #[export(range = (0.0, 8000.0, 1.0, or_greater))]
    #[var(set = set_max_splashes)]
    max_splashes: i32,
    /// Splashes per second per splash slot.
    #[export(range = (0.1, 10.0, 0.05))]
    splash_rate: f32,
    /// Largest splash ring radius (texels of `rain_texel`).
    #[export(range = (1.0, 8.0, 1.0))]
    splash_radius: i32,

    /// Optional snow, off by default. `snow_enabled` lets it snow on top of any weather (the sky
    /// greys towards the Snow weather with the intensity); the Snow weather snows on its own.
    /// While snow falls the ground cover (`pt_snow`) builds up on upward faces at
    /// `snow_accumulation` and melts back at `snow_melt` once it stops.
    #[export_group(name = "Snow")]
    #[export]
    snow_enabled: bool,
    /// Snowfall amount: scales every weather's snowfall (and the cover growth).
    #[export(range = (0.0, 1.0, 0.01))]
    snow_intensity: f32,
    /// Cover gained per minute at full snowfall (1 = everything flat is white).
    #[export(range = (0.0, 10.0, 0.01, or_greater))]
    snow_accumulation: f32,
    /// Cover lost per minute once the snow stops (faster in rain).
    #[export(range = (0.0, 10.0, 0.01, or_greater))]
    snow_melt: f32,
    /// Most cover the snowfall builds up.
    #[export(range = (0.0, 1.0, 0.01))]
    snow_max_cover: f32,
    /// Flakes on screen at full snowfall (the snow box follows the view, so zooming out does
    /// not thicken it into noise).
    #[export(range = (0.0, 4000.0, 1.0, or_greater))]
    snow_flakes: i32,
    /// Size of the big flakes in low-res texels (3 = plus-shaped).
    #[export(range = (1.0, 3.0, 1.0))]
    flake_texels: i32,
    /// Fraction of flakes drawn `flake_texels` wide; the rest are single texels.
    #[export(range = (0.0, 1.0, 0.01))]
    snow_big_flakes: f32,
    /// Fall speed (m/s).
    #[export(range = (0.1, 10.0, 0.05))]
    snow_speed: f32,
    /// Sideways sway of each flake (m).
    #[export(range = (0.0, 3.0, 0.01))]
    snow_sway: f32,
    #[export]
    snow_color: Color,

    #[export_group(name = "Lightning")]
    #[export]
    lightning_enabled: bool,
    #[export]
    bolt_color: Color,
    /// Extra ambient energy at the peak of a flash.
    #[export(range = (0.0, 8.0, 0.01))]
    flash_energy: f32,

    from: Params,
    to: Params,
    blend: f32,
    current: Params,
    wet: f32,
    snow: f32,
    /// 0..1 ramp of the `snow_enabled` overlay (follows `transition_seconds`).
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
    terrain: Option<Gd<Node>>,
    rng: Rng,
    next_strike: f32,
    flash_age: f32,
    env_base: Option<EnvBase>,
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
            area: Vector3::new(26.0, 12.0, 26.0),
            rain_color: Color::from_rgb(0.64, 0.72, 0.84),
            snow_color: Color::from_rgb(0.96, 0.97, 1.0),
            rain_speed: 16.0,
            snow_speed: 1.1,
            streak_texels: 4,
            flake_texels: 2,
            snow_enabled: false,
            snow_intensity: 0.65,
            snow_accumulation: 0.6,
            snow_melt: 0.4,
            snow_max_cover: 1.0,
            snow_flakes: 320,
            snow_big_flakes: 0.3,
            snow_sway: 0.45,
            rain_style: 0,
            rain_layers: 8,
            rain_layer_spacing: 2.5,
            rain_texel: 0.08,
            max_splashes: 700,
            splash_rate: 2.2,
            splash_radius: 3,
            lightning_enabled: true,
            bolt_color: Color::from_rgb(0.9, 0.93, 1.0),
            flash_energy: 2.5,
            from: Params::CLEAR,
            to: Params::CLEAR,
            blend: 1.0,
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
            rng: Rng(0x5EED_0F_3A7E),
            next_strike: 5.0,
            flash_age: 99.0,
            env_base: None,
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        // Before PixelDayNight (-10) and PixelSky (0), so they fold in this frame's mix.
        self.base_mut().set_process_priority(-20);
        self.build_precipitation();
        let mut bolt = MeshInstance3D::new_alloc();
        bolt.set_name("Bolt");
        bolt.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Color pass only: the bolt opts out of the metadata layer.
        let layers = bolt.get_layer_mask() & !globals::meta_mask();
        bolt.set_layer_mask(layers);
        bolt.set_as_top_level(true);
        bolt.set_visible(false);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_cull_mode(CullMode::DISABLED);
        mat.set_albedo(self.bolt_color);
        bolt.set_material_override(&mat);
        bolt.set_mesh(&ImmediateMesh::new_gd());
        let mut this = self.to_gd();
        add_internal(&mut this, &bolt);
        self.bolt = Some(bolt);
        if self.pending_instant {
            let name = self.target_name.clone();
            self.go(&name, true);
        }
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f64) {
        let dt = (delta as f32).min(0.25);
        if self.blend < 1.0 {
            self.blend = if self.transition_seconds <= 0.0 { 1.0 } else { (self.blend + dt / self.transition_seconds).min(1.0) };
        }
        let s = self.blend * self.blend * (3.0 - 2.0 * self.blend);
        self.current = self.from.lerp(&self.to, s);
        let snow_target = if self.snow_enabled { 1.0 } else { 0.0 };
        let snow_step = if self.transition_seconds <= 0.0 { 1.0 } else { dt / self.transition_seconds };
        self.snow_on += (snow_target - self.snow_on).clamp(-snow_step, snow_step);
        let eff = self.effective();

        // Ground follows its target at a fixed rate (soaks / dries); snow builds up while it
        // falls and melts back once it stops (wetting the ground while it melts).
        let rate = if self.ground_seconds <= 0.0 { 1e6 } else { dt / self.ground_seconds };
        let falling = eff.snowfall.clamp(0.0, 1.0);
        let before = self.snow;
        if falling > 0.02 {
            let cap = self.snow_max_cover.clamp(0.0, 1.0).max(eff.snow_cover.min(1.0));
            if self.snow < cap {
                self.snow = (self.snow + self.snow_accumulation.max(0.0) / 60.0 * falling * dt).min(cap);
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
        let wet_target = if self.melting { eff.wetness.max(0.35) } else { eff.wetness };
        self.wet += (wet_target - self.wet).clamp(-rate, rate);
        globals::set("pt_wet", &self.wet.to_variant());
        globals::set("pt_snow", &self.snow.to_variant());
        globals::set("pt_rain", &eff.rain.clamp(0.0, 1.0).to_variant());
        self.publish_wind(&eff);

        let flash = self.update_lightning(dt, &eff);
        MIX.with(|m| m.set(Some(Mix { p: eff, flash })));
        self.update_precipitation(&eff);
        if DAY_NIGHTS.load(Ordering::Relaxed) <= 0 && !is_editor() {
            self.grade_without_day_night(&eff, flash);
        }
    }

    fn exit_tree(&mut self) {
        MIX.with(|m| m.set(None));
        globals::set("pt_wet", &0.0f32.to_variant());
        globals::set("pt_snow", &0.0f32.to_variant());
        globals::set("pt_rain", &0.0f32.to_variant());
        self.rain_sheets.detach();
        self.restore_env();
    }
}

impl PixelWeather {
    fn sibling<T: GodotClass + Inherits<Node>>(&self, explicit: &Option<Gd<T>>) -> Option<Gd<T>> {
        if let Some(n) = explicit
            && n.is_instance_valid()
        {
            return Some(n.clone());
        }
        let parent = self.base().get_parent()?;
        parent.get_children().iter_shared().find_map(|n| n.try_cast::<T>().ok())
    }

    /// Looks a weather up by name: custom presets first, then the built-ins.
    fn lookup(&self, name: &str) -> Option<(String, Params)> {
        for p in self.presets.iter_shared() {
            let b = p.bind();
            if b.preset_name.to_string().eq_ignore_ascii_case(name) {
                return Some((b.preset_name.to_string(), b.params()));
            }
        }
        BUILTIN_NAMES.iter().position(|b| b.eq_ignore_ascii_case(name)).map(|i| (BUILTIN_NAMES[i].to_string(), builtin(i)))
    }

    /// Starts blending to `name` (instantly when `instant` or outside the tree).
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
        self.blend = if instant || self.transition_seconds <= 0.0 { 1.0 } else { 0.0 };
        if instant {
            self.current = p;
            let eff = self.effective();
            self.wet = eff.wetness;
            self.snow = eff.snow_cover.min(1.0);
        }
        if p.lightning > 0.0 {
            self.next_strike = self.next_strike.min(2.0 + self.rng.next_f32() * 3.0);
        }
        let n = canon.to_variant();
        self.base_mut().call_deferred("emit_signal", &["weather_changed".to_variant(), n]);
        true
    }

    /// The weather actually shown: the current blend scaled by `strength`, with the optional
    /// `snow_enabled` overlay folded in, and every snowfall scaled by `snow_intensity`.
    fn effective(&self) -> Params {
        let mut eff = Params::CLEAR.lerp(&self.current, self.strength.clamp(0.0, 1.0));
        let k = self.snow_intensity.clamp(0.0, 1.0);
        if self.snow_on > 1e-4 {
            // Snow on top of any weather: the sky greys towards the Snow weather (more with a
            // heavier snowfall), snow replaces rain, the standing cover stays the weather's own.
            let snow = Params::CLEAR.lerp(&builtin(4), 0.35 + 0.65 * k);
            let o = self.snow_on.clamp(0.0, 1.0);
            let mut m = eff.lerp(&snow, o * 0.75);
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

    /// Half-size of the snow box: about the camera view at the focus, rounded up to a power of
    /// two so the flakes' world wrap only changes at zoom steps. Also returns the view's half
    /// width / height (m) at the focus and the camera distance to it.
    fn snow_box(cam: Option<&Gd<Camera3D>>, focus: Vector3) -> (f32, f32, f32, f32) {
        let Some(cam) = cam else { return (16.0, 16.0, 9.0, 20.0) };
        let aspect = cam.get_viewport().map(|v| v.get_visible_rect().size).map(|s| s.x / s.y.max(1.0)).unwrap_or(16.0 / 9.0).max(0.2);
        let dist = (cam.get_global_position() - focus).length().max(0.5);
        let half_h = if cam.get_projection() == ProjectionType::ORTHOGONAL {
            cam.get_size() * 0.5
        } else {
            dist * (cam.get_fov().to_radians() * 0.5).tan()
        }
        .max(0.5);
        let half_w = half_h * aspect;
        let h = 2f32.powf((half_w * 1.15).max(4.0).log2().ceil()).min(256.0);
        (h, half_w, half_h, dist)
    }

    fn precip_instance(&self, name: &str, snow: bool) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(1.0, 1.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        let n = if snow { SNOW_INSTANCES } else { self.max_drops.max(0) };
        mm.set_instance_count(n);
        let mut rng = Rng(if snow { 0xF1A_4E5 } else { 0xD30_95 });
        for i in 0..n {
            mm.set_instance_transform(i, Transform3D::IDENTITY);
            mm.set_instance_custom_data(i, Color::from_rgba(rng.next_f32(), rng.next_f32(), rng.next_f32(), rng.next_f32()));
        }
        mm.set_visible_instance_count(0);
        // The snow box follows the view (up to 256 m); the shader clips flakes itself.
        let e = if snow { Vector3::splat(260.0) } else { self.area.abs() };
        mm.set_custom_aabb(Aabb::new(-e - Vector3::splat(2.0), (e + Vector3::splat(2.0)) * 2.0));
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Color pass only: precipitation opts out of the metadata layer (no outlines on drops).
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        let mat = shader_material("precip.gdshader");
        mi.set_material_override(&mat);
        (mi, mat)
    }

    fn build_precipitation(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        free_node(self.rain_mm.take());
        free_node(self.snow_mm.take());
        free_node(self.splash_mm.take());
        let (rain, rain_mat) = self.precip_instance("Rain", false);
        let (snow, snow_mat) = self.precip_instance("Snow", true);
        let (splash, splash_mat) = self.splash_instance();
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
        self.rain_sheets.attach(&mut node, "RainSheets");
    }

    fn update_precipitation(&mut self, eff: &Params) {
        let node = self.to_gd().upcast::<Node>();
        let plane_y = self.base().get_global_position().y;
        let cam = active_camera(&node);
        let focus = cam.as_ref().map(|c| camera_focus(c, plane_y)).unwrap_or_else(|| self.base().get_global_position());
        self.update_snow(eff, cam.as_ref(), focus);
        // Box bottom a little under the focus plane; the shader clips drops at the ground.
        let centre = focus + Vector3::new(0.0, self.area.y * 0.5, 0.0);
        let max = self.max_drops.max(0);
        let flipbook_rain = self.rain_style == 0;
        self.update_rain_sheets(eff, flipbook_rain);
        self.update_splashes(eff, centre, flipbook_rain);
        let legacy_rain = if flipbook_rain { 0.0 } else { eff.rain };
        let settings = [(self.rain_mm.clone(), self.rain_mat.clone(), legacy_rain, 0, self.rain_color, self.rain_speed, self.streak_texels)];
        for (mi, mat, amount, mode, color, speed, texels) in settings {
            let (Some(mut mi), Some(mut mat)) = (mi, mat) else { continue };
            if !mi.is_instance_valid() {
                continue;
            }
            let count = ((amount.clamp(0.0, 1.0) * max as f32).round() as i32).min(max);
            if let Some(mut mm) = mi.get_multimesh() {
                mm.set_visible_instance_count(count);
            }
            mi.set_visible(count > 0);
            if count == 0 {
                continue;
            }
            mi.set_global_position(centre);
            mat.set_shader_parameter("mode", &mode.to_variant());
            mat.set_shader_parameter("area", &self.area.abs().to_variant());
            mat.set_shader_parameter("color", &color.to_variant());
            mat.set_shader_parameter("fall_speed", &speed.to_variant());
            mat.set_shader_parameter("wind", &eff.fall_wind.to_variant());
            mat.set_shader_parameter("texels", &texels.to_variant());
        }
    }

    /// Snowflakes: a box about the size of the view around the focus, filled so that about
    /// `snow_flakes` flakes show on screen at full snowfall at any zoom.
    fn update_snow(&mut self, eff: &Params, cam: Option<&Gd<Camera3D>>, focus: Vector3) {
        let (Some(mut mi), Some(mut mat)) = (self.snow_mm.clone(), self.snow_mat.clone()) else { return };
        if !mi.is_instance_valid() {
            return;
        }
        let (h, half_w, half_h, dist) = Self::snow_box(cam, focus);
        let amount = eff.snowfall.clamp(0.0, 1.0);
        // Flakes in the box per flake in the view's cross-section.
        let fill = (h * h) / (half_w * half_h).max(0.01);
        let count = ((amount * self.snow_flakes.max(0) as f32 * fill).round() as i32).clamp(0, SNOW_INSTANCES);
        if let Some(mut mm) = mi.get_multimesh() {
            mm.set_visible_instance_count(count);
        }
        mi.set_visible(count > 0);
        if count == 0 {
            return;
        }
        let area = Vector3::new(h, h * 0.75, h);
        mi.set_global_position(focus + Vector3::new(0.0, h * 0.5, 0.0));
        mat.set_shader_parameter("mode", &1.to_variant());
        mat.set_shader_parameter("area", &area.to_variant());
        mat.set_shader_parameter("color", &self.snow_color.to_variant());
        mat.set_shader_parameter("fall_speed", &self.snow_speed.to_variant());
        mat.set_shader_parameter("wind", &eff.fall_wind.to_variant());
        mat.set_shader_parameter("texels", &self.flake_texels.clamp(1, 3).to_variant());
        mat.set_shader_parameter("big_flakes", &self.snow_big_flakes.clamp(0.0, 1.0).to_variant());
        mat.set_shader_parameter("sway", &self.snow_sway.max(0.0).to_variant());
        mat.set_shader_parameter("focus_depth", &dist.to_variant());
    }

    fn splash_instance(&self) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(1.0, 1.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        let n = self.max_splashes.max(0);
        mm.set_instance_count(n);
        let mut rng = Rng(0x5_9A5B);
        for i in 0..n {
            mm.set_instance_transform(i, Transform3D::IDENTITY);
            mm.set_instance_custom_data(i, Color::from_rgba(rng.next_f32(), rng.next_f32(), rng.next_f32(), rng.next_f32()));
        }
        mm.set_visible_instance_count(0);
        let e = self.area.abs();
        mm.set_custom_aabb(Aabb::new(Vector3::new(-e.x - 2.0, -200.0, -e.z - 2.0), Vector3::new(e.x * 2.0 + 4.0, 400.0, e.z * 2.0 + 4.0)));
        let mut mi = MultiMeshInstance3D::new_alloc();
        mi.set_name("Splashes");
        mi.set_multimesh(&mm);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Color pass only, like the drops.
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        let mat = shader_material("rain_splash.gdshader");
        mi.set_material_override(&mat);
        (mi, mat)
    }

    /// Flipbook rain: upright streak sheets around the camera focus. Rain intensity thins out
    /// whole streaks, the fall speed sets the flipbook rate and the fall wind slants and drifts
    /// the streaks.
    fn update_rain_sheets(&mut self, eff: &Params, on: bool) {
        let mut fp = flipbook::Preset::RainStreaks.params();
        fp.shape = flipbook::Shape::Streak(self.streak_texels.clamp(1, 12) as u8);
        self.rain_sheets.set_flipbook(&fp);
        let texel = self.rain_texel.max(0.005);
        let speed = self.rain_speed.max(0.1);
        let plane_y = self.base().get_global_position().y;
        let look = particle_field::Look {
            projection: particle_field::Projection::Upright,
            layers: self.rain_layers.clamp(1, 32),
            spacing: self.rain_layer_spacing.max(0.2),
            texel,
            extent: 0.0,
            base_y: plane_y - self.area.y * 0.5,
            height: self.area.y * 2.5,
            fall_speed: 0.0,
            wind: eff.fall_wind,
            slant: 1.0 / speed,
            gust: 0.0,
            band: 0.0,
            // The baked streaks fall `velocity.y` texels per frame.
            fps: speed / (fp.velocity[1].max(0.1) * texel),
            color: self.rain_color,
            color_b: self.rain_color.lerp(Color::WHITE, 0.25),
            // Additive: streaks lighten what is behind them, like the reference rain shader.
            brightness: 0.4,
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
        let (Some(mut mi), Some(mut mat)) = (self.splash_mm.clone(), self.splash_mat.clone()) else { return };
        if !mi.is_instance_valid() {
            return;
        }
        let amount = if flipbook_rain { eff.rain.clamp(0.0, 1.0) } else { 0.0 };
        let n = self.max_splashes.max(0);
        let count = ((amount * n as f32).ceil() as i32).min(n);
        if let Some(mut mm) = mi.get_multimesh() {
            mm.set_visible_instance_count(count);
        }
        mi.set_visible(count > 0);
        if count == 0 {
            return;
        }
        mi.set_global_position(Vector3::new(centre.x, 0.0, centre.z));
        mat.set_shader_parameter("area", &self.area.abs().to_variant());
        mat.set_shader_parameter("rate", &self.splash_rate.to_variant());
        mat.set_shader_parameter("texel", &self.rain_texel.max(0.005).to_variant());
        mat.set_shader_parameter("radius", &self.splash_radius.clamp(1, 8).to_variant());
        mat.set_shader_parameter("color", &self.rain_color.lerp(Color::WHITE, 0.2).to_variant());
    }

    /// `pt_wind` (pixel_wind.gdshaderinc): horizontal wind direction (x, z), strength (m/s:
    /// a light breeze scaled by the weather's wind) and gustiness 0..1.
    fn publish_wind(&self, eff: &Params) {
        let sky_wind = self
            .base()
            .get_parent()
            .and_then(|p| p.get_children().iter_shared().find(|n| n.is_class("PixelSky")))
            .and_then(|s| s.get("wind").try_to::<Vector2>().ok())
            .unwrap_or(Vector2::new(0.02, 0.01));
        let dir = if sky_wind.length() > 1e-6 { sky_wind.normalized() } else { Vector2::new(1.0, 0.0) };
        let strength = eff.wind_scale + eff.fall_wind.length() * 0.5;
        let gust = (0.25 + eff.rain * 0.35 + (eff.wind_scale - 1.0).max(0.0) * 0.15).clamp(0.0, 1.0);
        globals::set("pt_wind", &Vector4::new(dir.x, dir.y, strength, gust).to_variant());
    }

    /// Advances the strike timer and the flash envelope; returns the flash level 0..1.
    fn update_lightning(&mut self, dt: f32, eff: &Params) -> f32 {
        if self.lightning_enabled && eff.lightning > 0.05 {
            self.next_strike -= dt;
            if self.next_strike <= 0.0 {
                self.strike();
                // Exponential gaps (Poisson strikes) around the preset rate.
                let u = self.rng.next_f32().max(1e-3);
                self.next_strike = -u.ln() * 60.0 / eff.lightning;
            }
        }
        // Age before advancing, so the frame of a strike always shows the full flash.
        let a = self.flash_age;
        self.flash_age += dt;
        // Double flicker, then a fast decay.
        let f = if a < 0.07 {
            1.0
        } else if a < 0.14 {
            0.2
        } else if a < 0.22 {
            0.75
        } else {
            0.75 * (-(a - 0.22) * 10.0).exp()
        };
        let f = if a > 1.0 { 0.0 } else { f };
        if let Some(b) = &mut self.bolt {
            b.set_visible(a < 0.35 && f > 0.15);
        }
        f
    }

    /// Fires one lightning strike now: a jagged, branching bolt near the camera focus and a flash.
    fn strike_now(&mut self) {
        let node = self.to_gd().upcast::<Node>();
        let plane_y = self.base().get_global_position().y;
        let cam = active_camera(&node);
        let focus = cam.as_ref().map(|c| camera_focus(c, plane_y)).unwrap_or_else(|| self.base().get_global_position());
        if self.terrain.as_ref().is_none_or(|t| !t.is_instance_valid()) {
            self.terrain = find_terrain(&node);
        }
        let e = self.area.abs();
        let x = focus.x + self.rng.signed() * e.x * 0.35;
        let z = focus.z + self.rng.signed() * e.z * 0.35;
        let ground = ground_height(&mut self.terrain, Vector3::new(x, 0.0, z), plane_y);
        let top = Vector3::new(x + self.rng.signed() * 4.0, plane_y + e.y * 2.5 + 10.0, z + self.rng.signed() * 4.0);
        let bottom = Vector3::new(x, ground, z);
        let mut segs: Vec<(Vector3, Vector3, f32)> = Vec::new();
        bolt_segments(&mut self.rng, top, bottom, 0.16, 6, &mut segs);
        let view = cam.map(|c| c.get_global_transform().basis.col_c()).unwrap_or(Vector3::BACK);
        if let Some(mut bolt) = self.bolt.clone() {
            let mut im = bolt.get_mesh().and_then(|m| m.try_cast::<ImmediateMesh>().ok()).unwrap_or_else(ImmediateMesh::new_gd);
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
            bolt.set_global_transform(Transform3D::IDENTITY);
            if let Some(mut m) = bolt.get_material_override().and_then(|m| m.try_cast::<StandardMaterial3D>().ok()) {
                m.set_albedo(self.bolt_color);
            }
        }
        self.flash_age = 0.0;
    }

    /// Lighting without a PixelDayNight (game only): scale the captured base values.
    fn grade_without_day_night(&mut self, eff: &Params, flash: f32) {
        let sun = self.sibling(&self.sun);
        let env = self.sibling(&self.environment).and_then(|w| w.get_environment());
        if self.env_base.is_none() {
            let (fe, fd, fc, ae, bg) = match &env {
                Some(e) => (e.is_fog_enabled(), e.get_fog_density(), e.get_fog_light_color(), e.get_ambient_light_energy(), e.get_bg_color()),
                None => (false, 0.0, Color::WHITE, 1.0, Color::BLACK),
            };
            self.env_base = Some(EnvBase {
                sun_energy: sun.as_ref().map(|s| s.get_param(Param::ENERGY)).unwrap_or(1.0),
                ambient_energy: ae,
                fog_enabled: fe,
                fog_density: fd,
                fog_color: fc,
                bg_color: bg,
            });
        }
        let Some(b) = self.env_base else { return };
        if let Some(mut s) = sun {
            s.set_param(Param::ENERGY, b.sun_energy * eff.sun_scale + flash * 0.6);
        }
        if let Some(mut e) = env {
            e.set_ambient_light_energy(b.ambient_energy * eff.ambient_scale + flash * self.flash_energy);
            e.set_bg_color(overcast(b.bg_color, eff.sky_darken).lerp(Color::from_rgb(0.85, 0.88, 1.0), (flash * 0.5) as f64));
            let density = if b.fog_enabled { b.fog_density } else { 0.0 } + eff.fog_density;
            e.set_fog_enabled(b.fog_enabled || eff.fog_density > 1e-5);
            e.set_fog_density(density);
            e.set_fog_light_color(b.fog_color.lerp(eff.fog_color, eff.fog_mix as f64));
        }
    }

    fn restore_env(&mut self) {
        let Some(b) = self.env_base.take() else { return };
        if let Some(mut s) = self.sibling(&self.sun) {
            s.set_param(Param::ENERGY, b.sun_energy);
        }
        if let Some(mut e) = self.sibling(&self.environment).and_then(|w| w.get_environment()) {
            restore(&mut e, &b);
        }
    }
}

fn restore(e: &mut Gd<Environment>, b: &EnvBase) {
    e.set_ambient_light_energy(b.ambient_energy);
    e.set_bg_color(b.bg_color);
    e.set_fog_enabled(b.fog_enabled);
    e.set_fog_density(b.fog_density);
    e.set_fog_light_color(b.fog_color);
}

/// Recursive midpoint displacement with side branches: (start, end, half width) segments.
fn bolt_segments(rng: &mut Rng, a: Vector3, b: Vector3, width: f32, depth: u32, out: &mut Vec<(Vector3, Vector3, f32)>) {
    if depth == 0 {
        out.push((a, b, width));
        return;
    }
    let len = (b - a).length();
    let mid = (a + b) * 0.5 + Vector3::new(rng.signed(), rng.signed() * 0.3, rng.signed()) * len * 0.18;
    bolt_segments(rng, a, mid, width, depth - 1, out);
    bolt_segments(rng, mid, b, width, depth - 1, out);
    if depth >= 3 && rng.next_f32() < 0.35 {
        let dir = (b - a).normalized();
        let side = Vector3::new(rng.signed(), -0.2, rng.signed()).normalized();
        let end = mid + (dir * 0.6 + side * 0.8).normalized() * len * 0.45;
        bolt_segments(rng, mid, end, width * 0.55, depth - 2, out);
    }
}

#[godot_api]
impl PixelWeather {
    /// Emitted (deferred) when a new weather is requested.
    #[signal]
    fn weather_changed(name: GString);

    #[func]
    fn set_weather_index(&mut self, v: i32) {
        let i = v.clamp(0, BUILTIN_NAMES.len() as i32 - 1);
        self.weather = i;
        let inside = self.base().is_inside_tree();
        self.go(BUILTIN_NAMES[i as usize], !inside);
    }

    /// Blends to the weather called `name` (custom presets first, then Clear / Cloudy / Foggy /
    /// Rain / Snow / Storm, case-insensitive) over `transition_seconds`. Returns false if unknown.
    #[func]
    fn set_weather(&mut self, name: GString) -> bool {
        self.go(&name.to_string(), false)
    }

    /// Switches to `name` immediately (no transition; the ground is set at once too).
    #[func]
    fn set_weather_instant(&mut self, name: GString) -> bool {
        self.go(&name.to_string(), true)
    }

    /// Name of the current (target) weather.
    #[func]
    fn get_weather_name(&self) -> GString {
        self.target_name.as_str().into()
    }

    /// Every weather name `set_weather()` accepts: built-ins, then custom presets.
    #[func]
    fn get_weather_names(&self) -> PackedStringArray {
        let mut out: Vec<String> = BUILTIN_NAMES.iter().map(|s| s.to_string()).collect();
        for p in self.presets.iter_shared() {
            let n = p.bind().preset_name.to_string();
            if !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
                out.push(n);
            }
        }
        out.iter().map(|s| GString::from(s.as_str())).collect()
    }

    /// 0..1 progress of the current transition.
    #[func]
    fn get_transition_progress(&self) -> f32 {
        self.blend
    }

    /// Current ground wetness / snow cover (what pt_wet / pt_snow are set to).
    #[func]
    fn get_wetness(&self) -> f32 {
        self.wet
    }
    #[func]
    fn get_snow_cover(&self) -> f32 {
        self.snow
    }

    /// Triggers a lightning strike now (any weather).
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
    fn set_max_splashes(&mut self, v: i32) {
        self.max_splashes = v.max(0);
        self.build_precipitation();
    }

    #[func]
    fn set_area(&mut self, v: Vector3) {
        self.area = v.abs();
        self.build_precipitation();
    }
}

impl PixelWeather {
    fn strike(&mut self) {
        self.strike_now();
    }
}
