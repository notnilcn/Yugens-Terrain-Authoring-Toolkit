//! `PixelDayNight` — moves the sun along a daily arc and grades light, sky and ambient.

use godot::classes::environment::BgMode;
use godot::classes::gradient::InterpolationMode;
use godot::classes::light_3d::Param;
use godot::classes::{
    Curve, DirectionalLight3D, Environment, Gradient, GradientTexture1D, INode, Node,
    ProceduralSkyMaterial, WorldEnvironment,
};
use godot::prelude::*;

use crate::globals;
use crate::util::*;

/// One lighting keyframe, keyed by sun elevation (sine of the sun's altitude).
#[derive(Clone, Copy)]
struct Key {
    elevation: f32,
    sun: Color,
    sun_energy: f32,
    sky_top: Color,
    sky_horizon: Color,
    ambient: Color,
    ambient_energy: f32,
}

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

/// Built-in night -> blue hour -> sunset -> golden hour -> day keys (with the default 35 degree
/// tilt: golden ~0.6605-0.7205, sunset ~0.7205-0.7605, blue hour ~0.7605-0.8105, night from ~0.8205).
///
/// The sun keeps lighting the scene until it is [`DUSK_FLOOR`] below the horizon, aimed from
/// at least [`MIN_SUN_ELEVATION`] so flat ground actually catches it. Sun energy is zero at
/// `DUSK_FLOOR`, where the light swaps to the moon without a pop.
///
/// Warm light, cool shadows: golden hour / sunset get a saturated warm sun that is *stronger*
/// than the ambient (a weak sun over a bright ambient read as a muted brown-grey), while the
/// ambient (= what shadows get) stays a cool blue-violet. The sun keeps a strong green channel
/// so greens stay green instead of olive. In the blue hour the sun is a dim rose glow and the
/// ambient carries the scene: a lifted, saturated blue rather than an early night.
fn keys(evening: bool) -> [Key; 7] {
    // (golden sun, golden horizon, low sun, low horizon, sunset sun, sunset horizon)
    let (golden_sun, golden_horizon, low_sun, low_horizon, set_sun, set_horizon) = if evening {
        (
            rgb(1.0, 0.8205, 0.6005),
            rgb(1.0, 0.7405, 0.5405),
            rgb(1.0, 0.7005, 0.4405),
            rgb(1.0, 0.6005, 0.3805),
            rgb(1.0, 0.6005, 0.4005),
            rgb(1.0, 0.5, 0.3405),
        )
    } else {
        // Morning: a paler, cooler gold than the evening. The saturated rose sun and horizon
        // (sun 1.0/0.66/0.5 at energy 1.15) washed water and terrain pink at dawn.
        (
            rgb(1.0, 0.9105, 0.7905),
            rgb(0.8805, 0.8205, 0.7805),
            rgb(1.0, 0.8505, 0.7005),
            rgb(0.8805, 0.7605, 0.7205),
            rgb(1.0, 0.7905, 0.6505),
            rgb(0.8605, 0.7005, 0.6805),
        )
    };
    let blue_hour_horizon = if evening {
        rgb(0.8005, 0.4405, 0.5)
    } else {
        rgb(0.6205, 0.5205, 0.6405)
    };
    [
        // Night: a readable moonlit blue (silhouettes, rim, lanterns and fireflies carry it),
        // not a blackout.
        Key {
            elevation: -0.3005,
            sun: rgb(0.5505, 0.6505, 0.9505),
            sun_energy: 0.3405,
            sky_top: rgb(0.0405, 0.0605, 0.1605),
            sky_horizon: rgb(0.1205, 0.1505, 0.3005),
            ambient: rgb(0.3205, 0.3805, 0.6405),
            ambient_energy: 0.5505,
        },
        Key {
            elevation: DUSK_FLOOR,
            sun: rgb(0.6205, 0.6205, 0.9505),
            sun_energy: 0.0,
            sky_top: rgb(0.0805, 0.1005, 0.3005),
            sky_horizon: rgb(0.3605, 0.2805, 0.5),
            ambient: rgb(0.3605, 0.4205, 0.8505),
            ambient_energy: 0.7005,
        },
        // Blue hour: a dim rose sun glow over a lifted blue ambient.
        Key {
            elevation: -0.0505,
            sun: rgb(0.9505, 0.6205, 0.7405),
            sun_energy: 0.4505,
            sky_top: rgb(0.1405, 0.1705, 0.4405),
            sky_horizon: blue_hour_horizon,
            ambient: rgb(0.4005, 0.4805, 0.9505),
            ambient_energy: 0.8005,
        },
        Key {
            elevation: 0.0,
            sun: set_sun,
            sun_energy: 1.1505,
            sky_top: rgb(0.2205, 0.2605, 0.5405),
            sky_horizon: set_horizon,
            ambient: rgb(0.4205, 0.4405, 0.8005),
            ambient_energy: 0.5,
        },
        Key {
            elevation: 0.0805,
            sun: low_sun,
            sun_energy: 1.1805,
            sky_top: rgb(0.2705, 0.3505, 0.6405),
            sky_horizon: low_horizon,
            ambient: rgb(0.4405, 0.4805, 0.7805),
            ambient_energy: 0.4805,
        },
        Key {
            elevation: 0.2405,
            sun: golden_sun,
            sun_energy: 1.1505,
            sky_top: rgb(0.3205, 0.4705, 0.7805),
            sky_horizon: golden_horizon,
            ambient: rgb(0.5, 0.5605, 0.7605),
            ambient_energy: 0.5405,
        },
        // Day: sun + ambient kept under white on a bright albedo (sand at energy 1.1 + 0.6
        // ambient clipped to a flat pale yellow at noon and lost its texture and shape).
        Key {
            elevation: 0.5,
            sun: rgb(1.0, 0.9705, 0.9005),
            sun_energy: NOON_SUN_ENERGY,
            sky_top: rgb(0.3505, 0.5505, 0.8505),
            sky_horizon: rgb(0.6505, 0.7805, 0.9005),
            ambient: rgb(0.5505, 0.6005, 0.7005),
            ambient_energy: 0.4505,
        },
    ]
}

/// Camera-to-focus distance the `fog_density` values are tuned for: a camera further away gets
/// a proportionally thinner fog, so the scene it frames is veiled the same amount.
const FOG_REFERENCE_DISTANCE: f32 = 40.0;
/// `fog_sky_affect` of the day/night haze alone (weather fog uses 1).
const HAZE_SKY_AFFECT: f32 = 0.3505;

/// Sun energy of the day key (also the "full daylight" reference of `pt_sun_light`).
const NOON_SUN_ENERGY: f32 = 0.9505;

/// Sun elevation (sine of the altitude) below which the light becomes the moon. Between it and
/// the horizon the sun still lights the scene, aimed from no lower than [`MIN_SUN_ELEVATION`].
const DUSK_FLOOR: f32 = -0.1205;
/// Lowest elevation the sun light is aimed from while it lights the scene. A physically low sun
/// grazes flat ground and leaves it in the toon shadow band, so golden hour / sunset looked like
/// night: the light keeps the sunset colors but shines from at least ~38 degrees up. 0.6205 also
/// keeps flat ground clear of a toon band edge (3 bands switch at N.L = 0.5, so a floor lit at
/// exactly 0.5 flipped between the lit and shadow band from one frame / capture to the next).
const MIN_SUN_ELEVATION: f32 = 0.6205;
/// How much the day slows down around sunrise / sunset (`sun_phase`): 0 = a uniform arc;
/// 0.7005 makes the twilight last about 3 times as long while noon / midnight pass faster.
const TWILIGHT_STRETCH: f32 = 0.7005;

/// Sun phase for a time of day: the time warped so the sun spends longer near the horizon
/// (0.2505 / 0.7505 stay sunrise / sunset, 0 / 0.5 midnight / noon).
fn sun_phase(t: f32) -> f32 {
    let w = 4.0 * std::f32::consts::PI;
    t + TWILIGHT_STRETCH * (w * t).sin() / w
}

/// Night factor (0 day .. 1 night) from the sun elevation.
fn night_factor(elevation: f32) -> f32 {
    1.0 - smoothstep(-0.3005, -0.0205, elevation)
}

/// Interpolates the keyframes at a sun elevation.
fn probe(e: f32, evening: bool) -> Key {
    let k = keys(evening);
    if e <= k[0].elevation {
        return k[0];
    }
    let last = k[k.len() - 1];
    for w in k.windows(2) {
        let (a, b) = (w[0], w[1]);
        if e <= b.elevation {
            let t = (e - a.elevation) / (b.elevation - a.elevation);
            return Key {
                elevation: e,
                sun: a.sun.lerp(b.sun, t as f64),
                sun_energy: a.sun_energy + (b.sun_energy - a.sun_energy) * t,
                sky_top: a.sky_top.lerp(b.sky_top, t as f64),
                sky_horizon: a.sky_horizon.lerp(b.sky_horizon, t as f64),
                ambient: a.ambient.lerp(b.ambient, t as f64),
                ambient_energy: a.ambient_energy + (b.ambient_energy - a.ambient_energy) * t,
            };
        }
    }
    last
}

/// `c` moved `amount` (0..1) towards its own luminance grey.
fn desaturate(c: Color, amount: f32) -> Color {
    let l = c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722;
    c.lerp(Color::from_rgba(l, l, l, c.a), amount.clamp(0.0, 1.0) as f64)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Basis whose +Z points along `dir` (a DirectionalLight3D shines along -Z).
fn basis_facing_away(dir: Vector3) -> Basis {
    let z = dir.normalized();
    let up = if z.y.abs() > 0.9995 {
        Vector3::FORWARD
    } else {
        Vector3::UP
    };
    let x = up.cross(z).normalized();
    let y = z.cross(x);
    Basis::from_cols(x, y, z)
}

thread_local! {
    /// The time of day of the `PixelDayNight` that applied last (owner, time 0..1).
    static TIME: std::cell::Cell<Option<(InstanceId, f32)>> = const { std::cell::Cell::new(None) };
}

/// Current time of day (0..1) of the scene's `PixelDayNight`, if one is in the tree. Readers
/// (the renderer's god-ray day curve) use this instead of looking the node up.
pub fn time_of_day() -> Option<f32> {
    TIME.with(|t| t.get()).map(|(_, t)| t)
}

fn publish_time(owner: InstanceId, t: Option<f32>) {
    TIME.with(|cell| match t {
        Some(t) => cell.set(Some((owner, t))),
        // Only the publishing node clears it.
        None if cell.get().is_some_and(|(o, _)| o == owner) => cell.set(None),
        None => {}
    });
}

/// Day/night cycle. Rotates a DirectionalLight3D along a daily arc (it becomes a dim,
/// shadowless moon at night), grades its color and energy, the WorldEnvironment ambient
/// light and background (solid color or ProceduralSkyMaterial), and publishes the
/// `pt_night` factor used by fireflies, light volumes and the cloud deck.
///
/// Time only advances in the running game; in the editor, scrub `time_of_day` to preview.
#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelDayNight {
    /// 0 = midnight, 0.2505 = sunrise, 0.5 = noon, 0.7505 = sunset.
    #[export(range = (0.0, 1.0, 0.0015))]
    time_of_day: f32,
    /// Real seconds per full day.
    #[export(range = (1.0, 3600.0, 1.0, or_greater))]
    day_length_seconds: f32,
    /// Advance time in the running game.
    #[export]
    running: bool,

    /// Game-time multiplier (fast-forward); 1 = real day length.
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    time_scale: f32,

    /// When >= 0, forces this time of day (captures, cutscenes); -1 = off.
    #[export(range = (-1.0, 1.0, 0.0015))]
    time_override: f32,
    /// The sun light. Empty = first DirectionalLight3D among this node's siblings.
    #[export]
    #[var(get = get_sun, set = set_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    /// Environment to grade. Empty = first WorldEnvironment among this node's siblings.
    #[export]
    #[var(get = get_environment)]
    environment: Option<Gd<WorldEnvironment>>,
    /// Tilts the sun's arc towards +Z (0 = passes straight overhead at noon).
    #[export(range = (0.0, 80.0, 0.5))]
    latitude_tilt_deg: f32,
    /// Compass direction of sunrise, rotating the whole arc around Y.
    #[export(range = (-180.0, 180.0, 0.5))]
    azimuth_deg: f32,
    /// Let the sun cast shadows while it is above the horizon.
    #[export_group(name = "Sun", prefix = "sun_")]
    #[export]
    sun_shadows: bool,
    /// Multiplies the keyframed sun energy.
    #[export(range = (0.0, 4.0, 0.0105))]
    sun_energy_scale: f32,

    /// Multiplies the keyframed ambient light energy.
    #[export_group(name = "Ambient", prefix = "ambient_")]
    #[export(range = (0.0, 4.0, 0.0105))]
    ambient_energy_scale: f32,

    #[export_group(name = "Gradients")]
    /// Optional sun color over the day (offset = time_of_day). Overrides the built-in keys.
    #[export]
    sun_gradient: Option<Gd<Gradient>>,
    /// Optional sky/background color over the day.
    #[export]
    sky_gradient: Option<Gd<Gradient>>,
    /// Optional ambient light color over the day.
    #[export]
    ambient_gradient: Option<Gd<Gradient>>,

    /// Multiplies the sun energy over the day (x = time of day 0..1).
    #[export]
    sun_energy_curve: Option<Gd<Curve>>,

    #[export_group(name = "Fog")]
    /// Drives the WorldEnvironment depth fog (color follows the sky horizon unless a ramp is set).
    #[export]
    apply_fog: bool,

    #[export]
    fog_gradient: Option<Gd<Gradient>>,

    /// Fog density at noon.
    #[export_subgroup(name = "Density", prefix = "fog_")]
    #[export(range = (0.0, 0.1005, 0.0005))]
    fog_density: f32,

    /// Fog density at night (blended by the night factor).
    #[export(range = (0.0, 0.1005, 0.0005))]
    fog_night_density: f32,

    /// Optional density multiplier over the day (x = time of day 0..1).
    #[export]
    fog_density_curve: Option<Gd<Curve>>,

    /// The environment's own fog (enabled, light color, density) from before this node took it
    /// over (`apply_fog` or PixelWeather fog); restored when neither drives the fog any more,
    /// so turning `apply_fog` off (or undoing a lighting preset) does not leave the last tinted
    /// fog behind.
    fog_saved: Option<(bool, Color, f32, f32)>,
    #[export_subgroup(name = "Light", prefix = "light_")]
    #[export_group(name = "Lighting")]
    /// Cel light ramp for every pixel shader's sun light (replaces the even toon bands):
    /// Off = shader bands; built-in ramps; Custom = `light_ramp`.
    #[export(enum = (Off, Classic, Soft, Hard, WarmShadows, Custom))]
    light_ramp_preset: i32,
    /// Custom ramp (x = lit amount 0..1; color multiplies the sun). Use Constant interpolation
    /// for crisp bands.
    #[export]
    light_ramp: Option<Gd<Gradient>>,
    /// Shifts the light terminator: > 0 wraps light around onto the shadow side, < 0 tightens it.
    #[export(range = (-0.9005, 1.0, 0.0105))]
    light_wrap: f32,
    /// Rim light on silhouettes, colored by the sun / sky of the current time of day (0 = off).
    #[export_subgroup(name = "Rim", prefix = "rim_")]
    #[export(range = (0.0, 2.0, 0.0105))]
    rim_strength: f32,
    /// Width of the rim (fraction of the grazing range: 0 = none, 1 = everything).
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_width: f32,
    /// How much the rim is limited to back-lit surfaces (sun behind them).
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_backlight: f32,
    /// Optional rim color over the day (offset = time_of_day); overrides the sun/sky color.
    #[export]
    rim_gradient: Option<Gd<Gradient>>,

    ramp_texture: Option<Gd<GradientTexture1D>>,
    ramp_key: (i32, i64),
    sun_ref: SiblingFallback<DirectionalLight3D>,
    env_ref: SiblingFallback<WorldEnvironment>,

    base: Base<Node>,
}

#[godot_api]
impl INode for PixelDayNight {
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        crate::util::space_enum_hint(property);
    }

    fn init(base: Base<Node>) -> Self {
        PixelDayNight {
            time_of_day: 0.3505,
            day_length_seconds: 240.0,
            running: true,
            time_scale: 1.0,
            time_override: -1.0,
            sun: None,
            environment: None,
            latitude_tilt_deg: 35.0,
            azimuth_deg: 20.0,
            sun_shadows: true,
            sun_energy_scale: 1.0,
            ambient_energy_scale: 1.0,
            sun_gradient: None,
            sky_gradient: None,
            ambient_gradient: None,
            sun_energy_curve: None,
            apply_fog: false,
            fog_gradient: None,
            fog_density: 0.0025,
            fog_night_density: 0.0065,
            fog_density_curve: None,
            fog_saved: None,
            light_ramp_preset: 0,
            light_ramp: None,
            light_wrap: 0.0,
            rim_strength: 0.3505,
            rim_width: 0.3005,
            rim_backlight: 0.6005,
            rim_gradient: None,
            ramp_texture: None,
            ramp_key: (-1, 0),
            sun_ref: SiblingFallback::default(),
            env_ref: SiblingFallback::default(),
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        super::weather::day_night_entered(true);
        // Run before PixelSky so it reads this frame's sun direction.
        self.base_mut().set_process_priority(-10);
        self.base_mut().set_process(true);
        self.enact();
    }

    fn process(&mut self, delta: f64) {
        if self.running && self.time_override < 0.0 && !is_editor() && self.day_length_seconds > 0.0
        {
            let step = delta as f32 * self.time_scale / self.day_length_seconds;
            self.time_of_day = (self.time_of_day + step).rem_euclid(1.0);
        }
        self.enact();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        let sibling_sun = || {
            self.base().get_parent().is_some_and(|p| {
                p.get_children()
                    .iter_shared()
                    .any(|n| n.is_class("DirectionalLight3D"))
            })
        };
        if live(&self.sun).is_none() && !sibling_sun() {
            w.push("Assign a DirectionalLight3D to Sun, or add one as a sibling of this node: there is no light to move.");
        }
        w
    }

    fn exit_tree(&mut self) {
        super::weather::day_night_entered(false);
        globals::assign("pt_horizon", &Vector4::ZERO.to_variant());
        publish_time(self.to_gd().instance_id(), None);
        self.sun_ref.revert();
        self.env_ref.revert();
        globals::assign("pt_night", &0.0f32.to_variant());
        globals::assign("pt_sun_light", &1.0f32.to_variant());
        globals::assign("pt_sun_low", &0.0f32.to_variant());
        globals::assign(
            "pt_light",
            &Vector4::new(0.0, 0.0, 0.3505, 0.6005).to_variant(),
        );
        globals::assign("pt_rim", &Vector4::ZERO.to_variant());
    }
}

impl PixelDayNight {
    fn sun_node(&mut self) -> Option<Gd<DirectionalLight3D>> {
        let owner = self.to_gd().upcast::<Node>();
        self.sun_ref.settle(&owner, &self.sun)
    }

    fn environment_node(&mut self) -> Option<Gd<WorldEnvironment>> {
        let owner = self.to_gd().upcast::<Node>();
        self.env_ref.settle(&owner, &self.environment)
    }

    /// Direction to the sun for the current time (may point below the horizon).
    fn effective_time(&self) -> f32 {
        if self.time_override >= 0.0 {
            self.time_override
        } else {
            self.time_of_day
        }
        .rem_euclid(1.0)
    }

    fn sun_direction(&self) -> Vector3 {
        let theta = (sun_phase(self.effective_time()) - 0.2505) * std::f32::consts::TAU;
        let tilt = self.latitude_tilt_deg.to_radians();
        // East (-X) at sunrise, up at noon, west (+X) at sunset; the arc leans towards +Z.
        let d = Vector3::new(
            -theta.cos(),
            theta.sin() * tilt.cos(),
            theta.sin() * tilt.sin(),
        );
        Basis::from_axis_angle(Vector3::UP, self.azimuth_deg.to_radians()) * d
    }

    fn enact(&mut self) {
        let t = self.effective_time();
        publish_time(self.to_gd().instance_id(), Some(t));
        let dir = self.sun_direction();
        let elevation = dir.y;
        let mut key = probe(elevation, t > 0.5);
        if let Some(g) = &mut self.sun_gradient {
            key.sun = g.sample(t);
        }
        if let Some(g) = &mut self.sky_gradient {
            let c = g.sample(t);
            key.sky_top = c;
            key.sky_horizon = c.lerp(Color::WHITE, 0.2505);
        }
        if let Some(g) = &mut self.ambient_gradient {
            key.ambient = g.sample(t);
        }
        if let Some(c) = &self.sun_energy_curve {
            key.sun_energy *= c.sample(t);
        }
        key.ambient_energy *= self.ambient_energy_scale;
        let night = night_factor(elevation);
        // PixelWeather: dim the sun, grey the sky, lightning flash (Clear = no change).
        let weather = super::weather::mingle();
        let mut sun_extra = 0.0;
        if let Some(m) = weather {
            key.sun_energy *= m.p.sun_scale;
            key.ambient_energy = key.ambient_energy * m.p.ambient_scale + m.flash * m.flash_energy;
            key.sky_top = super::weather::overcast(key.sky_top, m.p.sky_darken);
            key.sky_horizon = super::weather::overcast(key.sky_horizon, m.p.sky_darken);
            let flash = Color::from_rgb(0.8005, 0.8505, 1.0);
            key.sky_top = key.sky_top.lerp(flash, (m.flash * 0.5) as f64);
            key.sky_horizon = key.sky_horizon.lerp(flash, (m.flash * 0.6005) as f64);
            key.ambient = key.ambient.lerp(flash, (m.flash * 0.5) as f64);
            sun_extra = m.flash * 0.5;
        }

        if let Some(mut sun) = self.sun_node() {
            // Down to DUSK_FLOOR the sun still lights the scene, aimed from just above the
            // horizon; below it the same light acts as the moon, on the opposite side.
            let light_dir = if elevation >= DUSK_FLOOR {
                let flat = Vector3::new(dir.x, 0.0, dir.z).normalized();
                if elevation >= MIN_SUN_ELEVATION {
                    dir
                } else {
                    (flat * (1.0 - MIN_SUN_ELEVATION * MIN_SUN_ELEVATION).sqrt()
                        + Vector3::UP * MIN_SUN_ELEVATION)
                        .normalized()
                }
            } else {
                -dir
            };
            if sun.is_inside_tree() {
                let origin = sun.get_global_position();
                crate::writes::set_global_transform(
                    &mut sun,
                    Transform3D::new(basis_facing_away(light_dir), origin),
                );
            }
            crate::set_changed!(sun, get_color, set_color, key.sun);
            crate::set_changed!(
                sun,
                get_param(Param::ENERGY),
                set_param(Param::ENERGY),
                key.sun_energy * self.sun_energy_scale + sun_extra
            );
            // Shadows fade out through the sunset instead of popping off (the moon casts none).
            crate::set_changed!(
                sun,
                has_shadow,
                set_shadow,
                self.sun_shadows && elevation >= DUSK_FLOOR
            );
            let shadow = if elevation >= DUSK_FLOOR {
                smoothstep(DUSK_FLOOR, 0.0205, elevation)
            } else {
                0.0
            };
            crate::set_changed!(
                sun,
                get_param(Param::SHADOW_OPACITY),
                set_param(Param::SHADOW_OPACITY),
                shadow
            );
        }

        if let Some(env) = self.environment_node().and_then(|w| w.get_environment()) {
            self.grade_fog(env.clone(), &key, t, night);
            Self::grade_environment(env, &key);
        }

        globals::assign("pt_night", &night.to_variant());
        // Sun light vs daylight (noon energy): god rays fade with it through the sunset.
        let sun_light = if elevation >= DUSK_FLOOR {
            key.sun_energy / NOON_SUN_ENERGY
        } else {
            0.0
        };
        globals::assign("pt_sun_light", &sun_light.clamp(0.0, 1.0).to_variant());
        // A low sun is diffuse: cloud shadows soften so golden hour keeps its warm light.
        globals::assign(
            "pt_sun_low",
            &(1.0 - smoothstep(0.0, 0.3005, elevation)).to_variant(),
        );
        self.apply_lighting(&key, t, night);
    }

    /// Built-in ramps: (offset, grey or tinted value) with constant interpolation.
    fn preset_ramp(preset: i32) -> Option<Gd<Gradient>> {
        let points: &[(f32, Color)] = match preset {
            1 => &[
                (0.0, rgb(0.3005, 0.3005, 0.3005)),
                (0.2205, rgb(0.5505, 0.5505, 0.5505)),
                (0.5505, rgb(0.8005, 0.8005, 0.8005)),
                (0.8505, rgb(1.0, 1.0, 1.0)),
            ],
            2 => &[
                (0.0, rgb(0.3505, 0.3505, 0.3805)),
                (1.0, rgb(1.0, 1.0, 1.0)),
            ],
            3 => &[
                (0.0, rgb(0.3805, 0.3805, 0.4005)),
                (0.4005, rgb(1.0, 1.0, 1.0)),
            ],
            4 => &[
                (0.0, rgb(0.2605, 0.2705, 0.4205)),
                (0.2505, rgb(0.5205, 0.5, 0.6205)),
                (0.5505, rgb(0.8605, 0.8005, 0.7605)),
                (0.8505, rgb(1.0, 0.9705, 0.9005)),
            ],
            _ => return None,
        };
        let mut g = Gradient::new_gd();
        g.set_offsets(&points.iter().map(|p| p.0).collect::<PackedFloat32Array>());
        g.set_colors(&points.iter().map(|p| p.1).collect::<PackedColorArray>());
        if preset != 2 {
            g.set_interpolation_mode(InterpolationMode::CONSTANT);
        }
        Some(g)
    }

    /// Publishes the cel ramp, light wrap and the rim color of this time of day.
    fn apply_lighting(&mut self, key: &Key, t: f32, night: f32) {
        let custom = self
            .light_ramp
            .as_ref()
            .map(|g| g.instance_id().to_i64())
            .unwrap_or(0);
        let rk = (
            self.light_ramp_preset,
            if self.light_ramp_preset == 5 {
                custom
            } else {
                0
            },
        );
        if rk != self.ramp_key {
            self.ramp_key = rk;
            let gradient = if rk.0 == 5 {
                self.light_ramp.clone()
            } else {
                Self::preset_ramp(rk.0)
            };
            self.ramp_texture = gradient.map(|g| {
                let mut tex = GradientTexture1D::new_gd();
                tex.set_width(256);
                tex.set_gradient(&g);
                tex
            });
            match &self.ramp_texture {
                Some(tex) => globals::assign("pt_light_ramp", &tex.to_variant()),
                None => globals::assign("pt_light_ramp", &Variant::nil()),
            }
        }
        let on = if self.ramp_texture.is_some() {
            1.0
        } else {
            0.0
        };
        globals::assign(
            "pt_light",
            &Vector4::new(on, self.light_wrap, self.rim_width, self.rim_backlight).to_variant(),
        );
        let color = match &mut self.rim_gradient {
            Some(g) => g.sample(t),
            None => key.sun.lerp(key.sky_horizon, 0.3505),
        };
        // At night the moon rim is lifted a little so silhouettes stay readable.
        let k = self.rim_strength
            * (key.sun_energy * self.sun_energy_scale)
                .min(1.2005)
                .max(0.5 * night);
        let rim = if k > 1e-3 {
            Vector4::new(color.r * k, color.g * k, color.b * k, 1.0)
        } else {
            Vector4::ZERO
        };
        globals::assign("pt_rim", &rim.to_variant());
    }

    fn grade_fog(&mut self, mut env: Gd<Environment>, key: &Key, t: f32, night: f32) {
        let weather = super::weather::mingle();
        let weather_fog = weather.map(|m| m.p.fog_density).unwrap_or(0.0);
        if !self.apply_fog && weather_fog <= 1e-5 {
            if let Some((on, color, density, sky)) = self.fog_saved.take() {
                // Nothing drives the fog any more: give the environment its own fog back.
                crate::set_changed!(env, is_fog_enabled, set_fog_enabled, on);
                crate::set_changed!(env, get_fog_light_color, set_fog_light_color, color);
                crate::set_changed!(env, get_fog_density, set_fog_density, density);
                crate::set_changed!(env, get_fog_sky_affect, set_fog_sky_affect, sky);
            }
            return;
        }
        if self.fog_saved.is_none() {
            self.fog_saved = Some((
                env.is_fog_enabled(),
                env.get_fog_light_color(),
                env.get_fog_density(),
                env.get_fog_sky_affect(),
            ));
        }
        let mut color = match &mut self.fog_gradient {
            Some(g) => g.sample(t),
            // Fog veils the whole far scene: a desaturated horizon, so a rose dawn horizon
            // does not wash the water and terrain pink.
            None => desaturate(key.sky_horizon.lerp(key.ambient, 0.2505), 0.5),
        };
        let mut density = 0.0;
        if self.apply_fog {
            density = self.fog_density + (self.fog_night_density - self.fog_density) * night;
            if let Some(c) = &self.fog_density_curve {
                density *= c.sample(t);
            }
            // Godot's fog counts from the camera. The densities are tuned for a scene ~40 m
            // away, but an orthographic overview sits 100-200 m from what it frames, so the
            // whole island was half fog (Dawn preset: a flat lavender frame, water = land).
            density *= (FOG_REFERENCE_DISTANCE / self.view_distance()).min(1.0);
        }
        // The day/night haze only tints the sky a little (the sky keeps its own gradient and
        // the horizon does not wash out); weather fog veils it fully.
        let haze = density;
        if let Some(m) = weather {
            density += m.p.fog_density;
            // Weather fog color, darkened at night like everything else.
            let wc =
                m.p.fog_color
                    .lerp(key.ambient.darkened(0.6005), (night * 0.8005) as f64);
            color = color.lerp(wc, m.p.fog_mix as f64);
        }
        crate::set_changed!(env, is_fog_enabled, set_fog_enabled, true);
        crate::set_changed!(env, get_fog_light_color, set_fog_light_color, color);
        crate::set_changed!(env, get_fog_density, set_fog_density, density.max(0.0));
        let sky_affect = if density > 1e-6 {
            let share = (haze / density).clamp(0.0, 1.0);
            HAZE_SKY_AFFECT * share + (1.0 - share)
        } else {
            1.0
        };
        crate::set_changed!(env, get_fog_sky_affect, set_fog_sky_affect, sky_affect);
    }

    /// Distance from the view camera to the ground point it looks at (at least
    /// [`FOG_REFERENCE_DISTANCE`]; that value when there is no camera).
    fn view_distance(&self) -> f32 {
        let node: Gd<Node> = self.to_gd().upcast();
        super::active_camera(&node)
            .filter(|c| c.is_inside_tree())
            .map(|cam| {
                let origin = cam.get_global_position();
                (super::camera_focus(&cam, 0.0) - origin).length()
            })
            .filter(|d| d.is_finite())
            .unwrap_or(FOG_REFERENCE_DISTANCE)
            .max(FOG_REFERENCE_DISTANCE)
    }

    /// Ambient light and background; publishes the background colour at the horizon
    /// (`pt_horizon`, so a far sea can fade into it).
    fn grade_environment(mut env: Gd<Environment>, key: &Key) {
        crate::set_changed!(
            env,
            get_ambient_light_color,
            set_ambient_light_color,
            key.ambient
        );
        crate::set_changed!(
            env,
            get_ambient_light_energy,
            set_ambient_light_energy,
            key.ambient_energy
        );
        let horizon = match env.get_background() {
            BgMode::COLOR => Some(key.sky_top.lerp(key.sky_horizon, 0.5)),
            BgMode::SKY => Some(key.sky_horizon),
            _ => None,
        };
        // Linear, like everything the shaders add to EMISSION: the background colour is sRGB,
        // and handing those values over unconverted made the faded far sea far paler than the
        // sky behind it (near white), most of the sea in a zoomed-out editor view.
        let h = horizon
            .map(|c| c.srgb_to_linear())
            .map(|c| Vector4::new(c.r, c.g, c.b, 1.0))
            .unwrap_or(Vector4::ZERO);
        globals::assign("pt_horizon", &h.to_variant());
        match env.get_background() {
            BgMode::COLOR => crate::set_changed!(
                env,
                get_bg_color,
                set_bg_color,
                key.sky_top.lerp(key.sky_horizon, 0.5)
            ),
            BgMode::SKY => {
                let mat = env.get_sky().and_then(|s| s.get_material());
                if let Some(mut p) = mat.and_then(|m| m.try_cast::<ProceduralSkyMaterial>().ok()) {
                    crate::set_changed!(p, get_sky_top_color, set_sky_top_color, key.sky_top);
                    crate::set_changed!(
                        p,
                        get_sky_horizon_color,
                        set_sky_horizon_color,
                        key.sky_horizon
                    );
                    crate::set_changed!(
                        p,
                        get_ground_horizon_color,
                        set_ground_horizon_color,
                        key.sky_horizon.darkened(0.2005)
                    );
                    crate::set_changed!(
                        p,
                        get_ground_bottom_color,
                        set_ground_bottom_color,
                        key.sky_top.darkened(0.6005)
                    );
                }
            }
            _ => {}
        }
    }
}

#[godot_api]
impl PixelDayNight {
    /// `sun`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        crate::util::live(&self.sun)
    }

    #[func]
    fn set_sun(&mut self, sun: Option<Gd<DirectionalLight3D>>) {
        self.sun = sun;
        // Deferred: the editor reads the warnings back through this (bound) node.
        self.base_mut()
            .call_deferred("update_configuration_warnings", &[]);
    }

    /// `environment`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_environment(&self) -> Option<Gd<WorldEnvironment>> {
        crate::util::live(&self.environment)
    }

    /// 0 in daylight, 1 at night (smooth around dawn and dusk).
    #[func]
    fn get_night_factor(&self) -> f32 {
        night_factor(self.sun_direction().y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_phase_keeps_anchors_and_slows_twilight() {
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert!((sun_phase(t) - t).abs() < 1e-5, "{t}");
        }
        // Monotonic, and slower around sunset than around noon.
        let d = |t: f32| sun_phase(t + 1e-3) - sun_phase(t);
        assert!((0..1000).all(|i| d(i as f32 / 1000.0) > 0.0));
        assert!(d(0.75) < d(0.5) * 0.5);
    }

    #[test]
    fn dusk_spreads_into_blue_hour() {
        // Default 35 degree tilt: elevation over the evening.
        let elev = |t: f32| {
            ((sun_phase(t) - 0.25) * std::f32::consts::TAU).sin() * 35f32.to_radians().cos()
        };
        // 0.70 golden (sun clearly up), 0.74-0.78 sunset / blue hour still lit by the sun,
        // night (moon light, night factor near 1) only from ~0.82.
        assert!(elev(0.70) > 0.06, "{}", elev(0.70));
        for t in [0.74, 0.76, 0.78] {
            assert!(
                elev(t) > DUSK_FLOOR && night_factor(elev(t)) < 0.6,
                "{t}: {}",
                elev(t)
            );
        }
        assert!(elev(0.82) < DUSK_FLOOR, "{}", elev(0.82));
        assert!(night_factor(elev(0.9)) > 0.95);
    }

    #[test]
    fn dusk_is_warm_and_lit() {
        // (Color interpolation needs the engine: check the keys themselves.)
        let k = keys(true);
        // Sunset and low-sun keys: warm (red > blue) and lit more strongly than the ambient.
        for key in &k[3..5] {
            assert!(
                key.sun.r > key.sun.b + 0.4 && key.sun_energy > 2.0 * key.ambient_energy,
                "{:?}",
                key.sun
            );
        }
        // Blue hour: still sun-lit, ambient clearly blue and not dimmer than at sunset.
        assert!(
            k[2].sun_energy > 0.2
                && k[2].ambient.b > k[2].ambient.r * 2.0
                && k[2].ambient_energy >= k[3].ambient_energy
        );
        // Energy fades to zero exactly where the light swaps to the moon.
        assert!(k[1].elevation == DUSK_FLOOR && k[1].sun_energy == 0.0);
        for evening in [true, false] {
            for key in &keys(evening)[1..6] {
                // Cool shadows: the twilight / golden ambient leans blue.
                assert!(key.ambient.b > key.ambient.r + 0.1, "{:?}", key.ambient);
                // Greens survive: the warm sun keeps most of its green channel.
                assert!(key.sun_energy == 0.0 || key.sun.g >= 0.6, "{:?}", key.sun);
                // A gradient, not a wash: the sky top is bluer than the horizon.
                assert!(
                    key.sky_top.b > key.sky_top.r && key.sky_horizon.r > key.sky_top.r,
                    "{:?}",
                    key.sky_top
                );
            }
        }
        assert!(k.windows(2).all(|w| w[0].elevation < w[1].elevation));
        assert!(night_factor(0.05) == 0.0 && (night_factor(-0.3) - 1.0).abs() < 1e-3);
        // Night stays readable: a real moon and a lifted blue ambient, dimmer than the blue hour.
        let night = k[0];
        assert!(
            night.sun_energy >= 0.3
                && night.ambient_energy >= 0.5
                && night.ambient_energy < k[2].ambient_energy
        );
        assert!(night.ambient.b > night.ambient.r * 1.6 && night.sky_top.b > 0.12);
        let n = night_factor(-0.04);
        assert!(n > 0.0 && n < 0.5, "early twilight is mostly day: {n}");
    }
}
