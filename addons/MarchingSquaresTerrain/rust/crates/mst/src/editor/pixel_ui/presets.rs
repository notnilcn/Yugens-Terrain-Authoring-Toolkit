//! Lighting presets for the World section: values for PixelDayNight, PixelSky, PixelWeather and
//! the renderer's god rays, applied in one undo action to whichever of those nodes exist.
//! Noon keeps the defaults (ray alpha 1, cloud-threshold day curve). The day curve is
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
    pub moniker: &'static str,
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
        moniker: "Dawn",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.2705)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.0045)),
            (DN, "fog_night_density", F(0.0065)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.3505)),
            (SKY, "softness", F(0.1505)),
            (SKY, "shadow_strength", F(0.3505)),
            (SKY, "wind", V2(0.0205, 0.0105)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.7005)),
            (SKY, "deck_color", C(1.0, 0.8605, 0.7605)),
            (SKY, "deck_shade_color", C(0.6005, 0.5, 0.6605)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.8005, 0.6005)),
            (R, "ray_alpha", F(0.5)),
            // The day curve closes the clouds around sunrise / sunset: follow the sky.
            (R, "day_cycle", B(false)),
        ],
    },
    Preset {
        moniker: "Noon",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.5)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(false)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.4205)),
            (SKY, "softness", F(0.1205)),
            (SKY, "shadow_strength", F(0.4505)),
            (SKY, "wind", V2(0.0205, 0.0105)),
            (SKY, "deck_enabled", B(false)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.9705, 0.8805)),
            (R, "ray_alpha", F(1.0)),
            (R, "day_cycle", B(true)),
        ],
    },
    Preset {
        moniker: "Dusk",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.7305)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.0035)),
            (DN, "fog_night_density", F(0.0065)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.4505)),
            (SKY, "softness", F(0.1405)),
            (SKY, "shadow_strength", F(0.4005)),
            (SKY, "wind", V2(0.0205, 0.0105)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.8005)),
            (SKY, "deck_color", C(1.0, 0.7605, 0.6205)),
            (SKY, "deck_shade_color", C(0.4205, 0.3805, 0.6005)),
            (R, "god_rays", B(true)),
            (R, "ray_color", C(1.0, 0.7005, 0.4505)),
            (R, "ray_alpha", F(0.5)),
            (R, "day_cycle", B(false)),
        ],
    },
    Preset {
        moniker: "Night",
        values: &[
            (WX, "weather", I(0)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.9505)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(1.0)),
            (DN, "ambient_energy_scale", F(1.0)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.0045)),
            (DN, "fog_night_density", F(0.0085)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.3005)),
            (SKY, "softness", F(0.1505)),
            (SKY, "shadow_strength", F(0.2005)),
            (SKY, "wind", V2(0.0155, 0.0085)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.5)),
            (SKY, "deck_color", C(0.5, 0.5505, 0.7005)),
            (SKY, "deck_shade_color", C(0.2005, 0.2205, 0.3505)),
            (R, "god_rays", B(false)),
        ],
    },
    Preset {
        moniker: "Storm",
        values: &[
            (WX, "weather", I(5)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.4505)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(0.4505)),
            (DN, "ambient_energy_scale", F(0.8005)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.0125)),
            (DN, "fog_night_density", F(0.0165)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.8505)),
            (SKY, "softness", F(0.2505)),
            (SKY, "shadow_strength", F(0.7505)),
            (SKY, "wind", V2(0.0805, 0.0405)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.9505)),
            (SKY, "deck_color", C(0.6205, 0.6405, 0.7005)),
            (SKY, "deck_shade_color", C(0.3505, 0.3705, 0.4505)),
            (R, "god_rays", B(false)),
        ],
    },
    Preset {
        // Optional snow look: the Snow weather (falling flakes, ground cover building up) under
        // a cool, soft overcast day. Any other preset clears it again (the cover melts back).
        moniker: "Snow",
        values: &[
            (WX, "weather", I(4)),
            (WX, "snow_enabled", B(false)),
            (DN, "time_of_day", F(0.4205)),
            (DN, "time_override", F(-1.0)),
            (DN, "sun_energy_scale", F(0.9005)),
            (DN, "ambient_energy_scale", F(1.1005)),
            (DN, "apply_fog", B(true)),
            (DN, "fog_density", F(0.0065)),
            (DN, "fog_night_density", F(0.0105)),
            (SKY, "clouds_enabled", B(true)),
            (SKY, "coverage", F(0.6205)),
            (SKY, "softness", F(0.2005)),
            (SKY, "shadow_strength", F(0.2505)),
            (SKY, "wind", V2(0.0255, 0.0125)),
            (SKY, "deck_enabled", B(true)),
            (SKY, "deck_opacity", F(0.8005)),
            (SKY, "deck_color", C(0.8605, 0.9005, 0.9605)),
            (SKY, "deck_shade_color", C(0.5505, 0.6005, 0.7205)),
            (R, "god_rays", B(false)),
        ],
    },
];
