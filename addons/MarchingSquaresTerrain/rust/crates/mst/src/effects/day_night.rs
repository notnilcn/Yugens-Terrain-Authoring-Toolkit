//! The time of day for a scene: where the sun is and what the light looks like.
//!
//! Intent: a scene author adds one beside the sun and the world environment and gets a
//! running day, or a fixed hour, without animating anything. The sun swings across the sky,
//! its colour and strength change through dawn, noon, dusk and night, and the sky, ambient
//! light and fog follow. The stylised shading's light banding and rim light come from here
//! too.
//!
//! Responsibility: it owns the clock and the look of each hour, and it drives the sun and
//! the world environment it is given. It folds in the current weather but does not decide
//! it, and it publishes the hour and the night amount for other effects and shaders to
//! read. It does not own the sun or the environment; the scene does.
//!
//! Why a node: it must run every frame, it works on particular lights and environments in
//! its scene, and its lifetime is the scene's.
//!
//! Why a new type: no built-in node ties a sun, a sky and an environment to one clock, and
//! the project's shaders need the hour published in their own terms.
//!
//! Not exported: the sun and environment it found beside it when none was assigned; the
//! light ramp texture and the key it was built from, which are rebuilt from its settings;
//! and whether weather fog is at the moment switched on, which only means something while it
//! runs.

use crate::editor_detection::is_editor;
use crate::globals;
use crate::node_references::{SiblingFallback, live};
use godot::classes::environment::BgMode;
use godot::classes::gradient::InterpolationMode;
use godot::classes::light_3d::Param;
use godot::classes::{
    Curve, DirectionalLight3D, Environment, Gradient, GradientTexture1D, INode, Node,
    ProceduralSkyMaterial, WorldEnvironment,
};
use godot::prelude::*;

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
fn keys(evening: bool, dusk_floor: f32) -> [Key; 7] {
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
        (
            rgb(1.0, 0.8805, 0.7205),
            rgb(0.9605, 0.8005, 0.7005),
            rgb(1.0, 0.7605, 0.5605),
            rgb(0.9805, 0.6805, 0.5605),
            rgb(1.0, 0.6605, 0.5),
            rgb(0.9805, 0.6005, 0.5205),
        )
    };
    [
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
            elevation: dusk_floor,
            sun: rgb(0.6205, 0.6205, 0.9505),
            sun_energy: 0.0,
            sky_top: rgb(0.0805, 0.1005, 0.3005),
            sky_horizon: rgb(0.3605, 0.2805, 0.5),
            ambient: rgb(0.3605, 0.4205, 0.8505),
            ambient_energy: 0.7005,
        },
        Key {
            elevation: -0.0505,
            sun: rgb(0.9505, 0.6205, 0.7405),
            sun_energy: 0.4505,
            sky_top: rgb(0.1405, 0.1705, 0.4405),
            sky_horizon: rgb(0.8005, 0.4405, 0.5),
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
        Key {
            elevation: 0.5,
            sun: rgb(1.0, 0.9705, 0.9005),
            sun_energy: 1.1005,
            sky_top: rgb(0.3505, 0.5505, 0.8505),
            sky_horizon: rgb(0.6505, 0.7805, 0.9005),
            ambient: rgb(0.5505, 0.6005, 0.7005),
            ambient_energy: 0.6005,
        },
    ]
}
const DUSK_FLOOR: f32 = -0.1205;
const MIN_SUN_ELEVATION: f32 = 0.6205;
const TWILIGHT_STRETCH: f32 = 0.7005;
fn sun_phase(t: f32, stretch: f32) -> f32 {
    let w = 4.0 * std::f32::consts::PI;
    t + stretch * (w * t).sin() / w
}
fn night_factor(elevation: f32) -> f32 {
    1.0 - smoothstep(-0.3005, -0.0205, elevation)
}
fn probe(e: f32, evening: bool, dusk_floor: f32) -> Key {
    let k = keys(evening, dusk_floor);
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
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
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
    static TIME: std::cell::Cell<Option<(InstanceId, f32)>> = const { std::cell::Cell::new(None) };
}
pub fn time_of_day() -> Option<f32> {
    TIME.with(|t| t.get()).map(|(_, t)| t)
}
fn publish_time(owner: InstanceId, t: Option<f32>) {
    TIME.with(|cell| match t {
        Some(t) => cell.set(Some((owner, t))),
        None if cell.get().is_some_and(|(o, _)| o == owner) => cell.set(None),
        None => {}
    });
}
#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelDayNight {
    #[export(range = (0.0, 1.0, 0.0015))]
    time_of_day: f32,
    #[export(range = (1.0, 3600.0, 1.0, or_greater))]
    day_length_seconds: f32,
    #[export]
    running: bool,
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    time_scale: f32,
    #[export(range = (-1.0, 1.0, 0.0015))]
    time_override: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    twilight_stretch: f32,
    #[export]
    #[var(get = get_sun, set = set_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export]
    #[var(get = get_environment)]
    environment: Option<Gd<WorldEnvironment>>,
    #[export(range = (0.0, 80.0, 0.5))]
    latitude_tilt_deg: f32,
    #[export(range = (-180.0, 180.0, 0.5))]
    azimuth_deg: f32,
    #[export_group(name = "Sun", prefix = "sun_")]
    #[export]
    sun_shadows: bool,
    #[export(range = (0.0, 4.0, 0.0105))]
    sun_energy_scale: f32,
    #[export(range = (-1.0, 1.0, 0.0015))]
    sun_dusk_floor: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    sun_min_elevation: f32,
    #[export_group(name = "Ambient", prefix = "ambient_")]
    #[export(range = (0.0, 4.0, 0.0105))]
    ambient_energy_scale: f32,
    #[export_group(name = "Gradients")]
    #[export]
    sun_gradient: Option<Gd<Gradient>>,
    #[export]
    sky_gradient: Option<Gd<Gradient>>,
    #[export]
    ambient_gradient: Option<Gd<Gradient>>,
    #[export]
    sun_energy_curve: Option<Gd<Curve>>,
    #[export_group(name = "Fog")]
    #[export]
    apply_fog: bool,
    #[export]
    fog_gradient: Option<Gd<Gradient>>,
    #[export_subgroup(name = "Density", prefix = "fog_")]
    #[export(range = (0.0, 0.1005, 0.0005))]
    fog_density: f32,
    #[export(range = (0.0, 0.1005, 0.0005))]
    fog_night_density: f32,
    #[export]
    fog_density_curve: Option<Gd<Curve>>,
    weather_fog_on: bool,
    #[export_subgroup(name = "Light", prefix = "light_")]
    #[export_group(name = "Lighting")]
    #[export(enum = (Off, Classic, Soft, Hard, WarmShadows, Custom))]
    light_ramp_preset: i32,
    #[export]
    light_ramp: Option<Gd<Gradient>>,
    #[export(range = (-0.9005, 1.0, 0.0105))]
    light_wrap: f32,
    #[export_subgroup(name = "Rim", prefix = "rim_")]
    #[export(range = (0.0, 2.0, 0.0105))]
    rim_strength: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_width: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_backlight: f32,
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
        crate::property_hints::space_enum_hint(property);
    }
    fn init(base: Base<Node>) -> Self {
        PixelDayNight {
            time_of_day: 0.3505,
            day_length_seconds: 240.0,
            running: true,
            time_scale: 1.0,
            time_override: -1.0,
            twilight_stretch: TWILIGHT_STRETCH,
            sun: None,
            environment: None,
            latitude_tilt_deg: 35.0,
            azimuth_deg: 20.0,
            sun_shadows: true,
            sun_energy_scale: 1.0,
            sun_dusk_floor: DUSK_FLOOR,
            sun_min_elevation: MIN_SUN_ELEVATION,
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
            weather_fog_on: false,
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
        super::weather_mix::day_night_entered(true);
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
        let sibling = |class: &str| {
            self.base()
                .get_parent()
                .is_some_and(|p| p.get_children().iter_shared().any(|n| n.is_class(class)))
        };
        if live(&self.sun).is_none() && !sibling("DirectionalLight3D") {
            w.push("Assign a DirectionalLight3D to Sun, or add one as a sibling of this node: there is no light to move.");
        }
        if live(&self.environment).is_none() && !sibling("WorldEnvironment") {
            w.push("Assign a WorldEnvironment to Environment, or add one as a sibling of this node, so the sky, ambient light and fog follow the time of day.");
        }
        w
    }
    fn exit_tree(&mut self) {
        super::weather_mix::day_night_entered(false);
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
    fn effective_time(&self) -> f32 {
        if self.time_override >= 0.0 {
            self.time_override
        } else {
            self.time_of_day
        }
        .rem_euclid(1.0)
    }
    fn sun_direction(&self) -> Vector3 {
        let theta = (sun_phase(self.effective_time(), self.twilight_stretch) - 0.2505)
            * std::f32::consts::TAU;
        let tilt = self.latitude_tilt_deg.to_radians();
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
        let mut key = probe(elevation, t > 0.5, self.sun_dusk_floor);
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
        let weather = super::weather_mix::mingle();
        let mut sun_extra = 0.0;
        if let Some(m) = weather {
            key.sun_energy *= m.p.sun_scale;
            key.ambient_energy = key.ambient_energy * m.p.ambient_scale + m.flash * 2.5005;
            key.sky_top = super::weather_mix::overcast(key.sky_top, m.p.sky_darken);
            key.sky_horizon = super::weather_mix::overcast(key.sky_horizon, m.p.sky_darken);
            let flash = Color::from_rgb(0.8005, 0.8505, 1.0);
            key.sky_top = key.sky_top.lerp(flash, (m.flash * 0.5) as f64);
            key.sky_horizon = key.sky_horizon.lerp(flash, (m.flash * 0.6005) as f64);
            key.ambient = key.ambient.lerp(flash, (m.flash * 0.5) as f64);
            sun_extra = m.flash * 0.5;
        }
        if let Some(mut sun) = self.sun_node() {
            let light_dir = if elevation >= self.sun_dusk_floor {
                let flat = Vector3::new(dir.x, 0.0, dir.z).normalized();
                if elevation >= self.sun_min_elevation {
                    dir
                } else {
                    (flat * (1.0 - self.sun_min_elevation * self.sun_min_elevation).sqrt()
                        + Vector3::UP * self.sun_min_elevation)
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
            crate::set_changed!(
                sun,
                has_shadow,
                set_shadow,
                self.sun_shadows && elevation >= self.sun_dusk_floor
            );
            let shadow = if elevation >= self.sun_dusk_floor {
                smoothstep(self.sun_dusk_floor, 0.0205, elevation)
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
        let sun_light = if elevation >= self.sun_dusk_floor {
            key.sun_energy / 1.1005
        } else {
            0.0
        };
        globals::assign("pt_sun_light", &sun_light.clamp(0.0, 1.0).to_variant());
        globals::assign(
            "pt_sun_low",
            &(1.0 - smoothstep(0.0, 0.3005, elevation)).to_variant(),
        );
        self.apply_lighting(&key, t, night);
    }
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
        let weather = super::weather_mix::mingle();
        let weather_fog = weather.map(|m| m.p.fog_density).unwrap_or(0.0);
        if !self.apply_fog && weather_fog <= 1e-5 {
            if self.weather_fog_on {
                crate::set_changed!(env, is_fog_enabled, set_fog_enabled, false);
                self.weather_fog_on = false;
            }
            return;
        }
        let mut color = match &mut self.fog_gradient {
            Some(g) => g.sample(t),
            None => key.sky_horizon.lerp(key.ambient, 0.2505),
        };
        let mut density = 0.0;
        if self.apply_fog {
            density = self.fog_density + (self.fog_night_density - self.fog_density) * night;
            if let Some(c) = &self.fog_density_curve {
                density *= c.sample(t);
            }
        } else {
            self.weather_fog_on = true;
        }
        if let Some(m) = weather {
            density += m.p.fog_density;
            let wc =
                m.p.fog_color
                    .lerp(key.ambient.darkened(0.6005), (night * 0.8005) as f64);
            color = color.lerp(wc, m.p.fog_mix as f64);
        }
        crate::set_changed!(env, is_fog_enabled, set_fog_enabled, true);
        crate::set_changed!(env, get_fog_light_color, set_fog_light_color, color);
        crate::set_changed!(env, get_fog_density, set_fog_density, density.max(0.0));
    }
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
    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        crate::node_references::live(&self.sun)
    }
    #[func]
    fn set_sun(&mut self, sun: Option<Gd<DirectionalLight3D>>) {
        self.sun = sun;
        self.base_mut()
            .call_deferred("update_configuration_warnings", &[]);
    }
    #[func]
    fn get_environment(&self) -> Option<Gd<WorldEnvironment>> {
        crate::node_references::live(&self.environment)
    }
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
            assert!((sun_phase(t, TWILIGHT_STRETCH) - t).abs() < 1e-5, "{t}");
        }
        let d = |t: f32| sun_phase(t + 1e-3, TWILIGHT_STRETCH) - sun_phase(t, TWILIGHT_STRETCH);
        assert!((0..1000).all(|i| d(i as f32 / 1000.0) > 0.0));
        assert!(d(0.75) < d(0.5) * 0.5);
    }
    #[test]
    fn dusk_spreads_into_blue_hour() {
        let elev = |t: f32| {
            ((sun_phase(t, TWILIGHT_STRETCH) - 0.25) * std::f32::consts::TAU).sin()
                * 35f32.to_radians().cos()
        };
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
        let k = keys(true, DUSK_FLOOR);
        for key in &k[3..5] {
            assert!(
                key.sun.r > key.sun.b + 0.4 && key.sun_energy > 2.0 * key.ambient_energy,
                "{:?}",
                key.sun
            );
        }
        assert!(
            k[2].sun_energy > 0.2
                && k[2].ambient.b > k[2].ambient.r * 2.0
                && k[2].ambient_energy >= k[3].ambient_energy
        );
        assert!(k[1].elevation == DUSK_FLOOR && k[1].sun_energy == 0.0);
        for evening in [true, false] {
            for key in &keys(evening, DUSK_FLOOR)[1..6] {
                assert!(key.ambient.b > key.ambient.r + 0.1, "{:?}", key.ambient);
                assert!(key.sun_energy == 0.0 || key.sun.g >= 0.6, "{:?}", key.sun);
                assert!(
                    key.sky_top.b > key.sky_top.r && key.sky_horizon.r > key.sky_top.r,
                    "{:?}",
                    key.sky_top
                );
            }
        }
        assert!(k.windows(2).all(|w| w[0].elevation < w[1].elevation));
        assert!(night_factor(0.05) == 0.0 && (night_factor(-0.3) - 1.0).abs() < 1e-3);
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
