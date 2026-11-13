//! Lighting presets for the World section: values for PixelDayNight, PixelSky, PixelWeather and
//! the renderer's god rays, applied in one undo action to whichever of those nodes exist.
//! Noon keeps Stable3D's defaults (strength 1, cloud-threshold day curve). The day curve is
//! overcast around sunrise / sunset, so Dawn / Dusk take the gaps from PixelSky's coverage
//! instead, at half strength: the low sun crosses many more sheets.

use godot::prelude::*;

pub enum V {
    F(f32),
    I(i64),
    B(bool),
    C(f32, f32, f32),
    V2(f32, f32),
}

impl V {
    pub fn to_variant(&self) -> Variant {
        match *self {
            V::F(x) => x.to_variant(),
            V::I(x) => x.to_variant(),
            V::B(x) => x.to_variant(),
            V::C(r, g, b) => Color::from_rgb(r, g, b).to_variant(),
            V::V2(x, y) => Vector2::new(x, y).to_variant(),
        }
    }
}

pub struct Preset {
    pub name: &'static str,
    /// (class, property, value)
    pub values: &'static [(&'static str, &'static str, V)],
}

use V::*;

const DN: &str = "PixelDayNight";
const SKY: &str = "PixelSky";
const R: &str = super::sections::RENDERER;
/// PixelWeather `weather` values (built-in order: Clear, Cloudy, Foggy, Rain, Snow, Storm).
const WX: &str = "PixelWeather";

pub static PRESETS: &[Preset] = &[
    Preset {
        name: "Dawn",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.27)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.004)),
            (DN, "fog_night_density", F(0.006)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.35)),
            (SKY, "softness", F(0.15)),
            (SKY, "shadow_strength", F(0.35)),
            (SKY, "wind", V2(0.02, 0.01)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.7)),
            (SKY, "deck_color", C(1.0, 0.86, 0.76)),
            (SKY, "deck_shade_color", C(0.6, 0.5, 0.66)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.8, 0.6)),
            (R, "ray_strength", F(0.5)),
            // Stable3D's day curve closes the clouds around sunrise / sunset: follow the sky.
            (R, "ray_day_cycle", B(false)),
        ],
    },
    Preset {
        name: "Noon",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.5)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(false)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.42)),
            (SKY, "softness", F(0.12)),
            (SKY, "shadow_strength", F(0.45)),
            (SKY, "wind", V2(0.02, 0.01)),
            (SKY, "deck_enabled", B(false)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.97, 0.88)),
            (R, "ray_strength", F(1.0)),
            (R, "ray_day_cycle", B(true)),
        ],
    },
    Preset {
        name: "Dusk",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.73)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.003)),
            (DN, "fog_night_density", F(0.006)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.45)),
            (SKY, "softness", F(0.14)),
            (SKY, "shadow_strength", F(0.4)),
            (SKY, "wind", V2(0.02, 0.01)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.8)),
            (SKY, "deck_color", C(1.0, 0.76, 0.62)),
            (SKY, "deck_shade_color", C(0.42, 0.38, 0.6)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.7, 0.45)),
            (R, "ray_strength", F(0.5)),
            (R, "ray_day_cycle", B(false)),
        ],
    },
    Preset {
        name: "Night",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.95)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.004)),
            (DN, "fog_night_density", F(0.008)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.3)),
            (SKY, "softness", F(0.15)),
            (SKY, "shadow_strength", F(0.2)),
            (SKY, "wind", V2(0.015, 0.008)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.5)),
            (SKY, "deck_color", C(0.5, 0.55, 0.7)),
            (SKY, "deck_shade_color", C(0.2, 0.22, 0.35)),
            (R, "god_rays", B(false)),
        ],
    },
    Preset {
        name: "Storm",
        values: &[
            (WX, "weather", I(5)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.45)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(0.45)),
            (DN, "ambient_energy_scale", F(0.8)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.012)),
            (DN, "fog_night_density", F(0.016)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.85)),
            (SKY, "softness", F(0.25)),
            (SKY, "shadow_strength", F(0.75)),
            (SKY, "wind", V2(0.08, 0.04)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.95)),
            (SKY, "deck_color", C(0.62, 0.64, 0.7)),
            (SKY, "deck_shade_color", C(0.35, 0.37, 0.45)),
            (R, "god_rays", B(false)),
        ],
    },
    Preset {
        // Optional snow look: the Snow weather (falling flakes, ground cover building up) under
        // a cool, soft overcast day. Any other preset clears it again (the cover melts back).
        name: "Snow",
        values: &[
            (WX, "weather", I(4)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.42)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(0.9)),
            (DN, "ambient_energy_scale", F(1.1)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.006)),
            (DN, "fog_night_density", F(0.01)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.62)),
            (SKY, "softness", F(0.2)),
            (SKY, "shadow_strength", F(0.25)),
            (SKY, "wind", V2(0.025, 0.012)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.8)),
            (SKY, "deck_color", C(0.86, 0.9, 0.96)),
            (SKY, "deck_shade_color", C(0.55, 0.6, 0.72)),
            (R, "god_rays", B(false)),
        ],
    },
];
