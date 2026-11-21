use super::weather_mix::{BUILTIN_NAMES, Params, builtin};
use godot::classes::IResource;
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelWeatherPreset {
    #[export]
    preset_name: GString,
    #[export_group(name = "Clouds")]
    #[export(range = (-1.0, 1.0, 0.0105))]
    cloud_coverage: f32,
    #[export(range = (-0.5, 0.5, 0.0055))]
    cloud_softness: f32,
    #[export(range = (-1.0, 1.0, 0.0105))]
    cloud_shadow: f32,
    #[export(range = (-1.0, 1.0, 0.0105))]
    deck_opacity: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    deck_darken: f32,
    #[export(range = (0.0, 8.0, 0.0105))]
    wind_scale: f32,
    #[export_group(name = "Light")]
    #[export(range = (0.0, 2.0, 0.0105))]
    sun_scale: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    ambient_scale: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    sky_darken: f32,
    #[export_group(name = "Fog")]
    #[export(range = (0.0, 0.1005, 0.0004))]
    fog_density: f32,
    #[export]
    fog_color: Color,
    #[export(range = (0.0, 1.0, 0.0105))]
    fog_color_mix: f32,
    #[export_group(name = "Ground")]
    #[export(range = (0.0, 1.0, 0.0105))]
    wetness: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    snow_cover: f32,
    #[export_group(name = "Precipitation")]
    #[export(range = (0.0, 1.0, 0.0105))]
    rain: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    snowfall: f32,
    #[export]
    fall_wind: Vector2,
    #[export_group(name = "Lightning", prefix = "lightning_")]
    #[export(range = (0.0, 60.0, 0.1005))]
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
        p.ingest(&Params::CLEAR);
        p
    }
}
impl PixelWeatherPreset {
    fn ingest(&mut self, p: &Params) {
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
    pub fn name(&self) -> String {
        self.preset_name.to_string()
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
    #[func]
    fn from_builtin(name: GString) -> Gd<PixelWeatherPreset> {
        let n = name.to_string();
        let i = BUILTIN_NAMES
            .iter()
            .position(|b| b.eq_ignore_ascii_case(&n))
            .unwrap_or(0);
        let mut p = PixelWeatherPreset::new_gd();
        {
            let mut b = p.bind_mut();
            b.preset_name = BUILTIN_NAMES[i].into();
            b.ingest(&builtin(i));
        }
        p
    }
}
