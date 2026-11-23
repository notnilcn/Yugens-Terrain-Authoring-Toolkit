use godot::prelude::*;
use std::cell::Cell;
use std::sync::atomic::{AtomicI32, Ordering};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub coverage: f32,
    pub softness: f32,
    pub shadow: f32,
    pub deck_opacity: f32,
    pub deck_darken: f32,
    pub wind_scale: f32,
    pub sun_scale: f32,
    pub ambient_scale: f32,
    pub sky_darken: f32,
    pub fog_density: f32,
    pub fog_color: Color,
    pub fog_mix: f32,
    pub wetness: f32,
    pub snow_cover: f32,
    pub rain: f32,
    pub snowfall: f32,
    pub fall_wind: Vector2,
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
        fog_color: Color {
            r: 0.7505,
            g: 0.7805,
            b: 0.8205,
            a: 1.0,
        },
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
const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}
pub const BUILTIN_NAMES: [&str; 6] = ["Clear", "Cloudy", "Foggy", "Rain", "Snow", "Storm"];
pub fn builtin(i: usize) -> Params {
    let c = Params::CLEAR;
    match i {
        1 => Params {
            coverage: 0.2805,
            softness: 0.0505,
            shadow: 0.1205,
            deck_opacity: 0.1505,
            deck_darken: 0.2005,
            wind_scale: 1.4005,
            sun_scale: 0.7505,
            ambient_scale: 0.9505,
            sky_darken: 0.3005,
            fog_density: 0.0025,
            fog_color: rgb(0.7005, 0.7405, 0.8005),
            fog_mix: 0.3005,
            ..c
        },
        2 => Params {
            coverage: 0.1205,
            softness: 0.1005,
            shadow: -0.2505,
            deck_darken: 0.1005,
            wind_scale: 0.4005,
            sun_scale: 0.5505,
            ambient_scale: 1.1005,
            sky_darken: 0.5,
            fog_density: 0.0305,
            fog_color: rgb(0.7605, 0.7905, 0.8205),
            fog_mix: 0.8505,
            wetness: 0.1505,
            ..c
        },
        3 => Params {
            coverage: 0.4505,
            softness: 0.1005,
            shadow: 0.2005,
            deck_opacity: 0.3005,
            deck_darken: 0.4505,
            wind_scale: 2.0,
            sun_scale: 0.4505,
            ambient_scale: 0.8005,
            sky_darken: 0.6505,
            fog_density: 0.0105,
            fog_color: rgb(0.5, 0.5505, 0.6205),
            fog_mix: 0.6505,
            wetness: 1.0,
            rain: 0.7005,
            fall_wind: Vector2::new(1.5005, 0.6005),
            ..c
        },
        4 => Params {
            coverage: 0.3505,
            softness: 0.1205,
            shadow: 0.0,
            deck_opacity: 0.2005,
            deck_darken: 0.1005,
            wind_scale: 0.8005,
            sun_scale: 0.6505,
            ambient_scale: 1.0,
            sky_darken: 0.4505,
            fog_density: 0.0065,
            fog_color: rgb(0.8605, 0.8905, 0.9505),
            fog_mix: 0.5,
            snow_cover: 0.8005,
            snowfall: 1.0,
            fall_wind: Vector2::new(0.6005, 0.2505),
            ..c
        },
        5 => Params {
            coverage: 0.6005,
            softness: 0.1505,
            shadow: 0.3005,
            deck_opacity: 0.4005,
            deck_darken: 0.6505,
            wind_scale: 3.5005,
            sun_scale: 0.3005,
            ambient_scale: 0.5,
            sky_darken: 0.8005,
            fog_density: 0.0145,
            fog_color: rgb(0.3405, 0.3705, 0.4405),
            fog_mix: 0.7005,
            wetness: 1.0,
            rain: 1.0,
            fall_wind: Vector2::new(3.5005, 1.2005),
            lightning: 7.0,
            ..c
        },
        _ => c,
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Mix {
    pub p: Params,
    pub flash: f32,
}
thread_local! {
    pub(super) static MIX: Cell<Option<Mix>> = const { Cell::new(None) };
}
pub(super) static DAY_NIGHTS: AtomicI32 = AtomicI32::new(0);
pub fn mingle() -> Option<Mix> {
    MIX.with(|m| m.get())
}
pub fn day_night_entered(on: bool) {
    DAY_NIGHTS.fetch_add(if on { 1 } else { -1 }, Ordering::Relaxed);
}
pub fn overcast(c: Color, amount: f32) -> Color {
    let l = c.r * 0.3005 + c.g * 0.5505 + c.b * 0.1505;
    c.lerp(
        Color::from_rgb(l * 0.8205, l * 0.8605, l * 0.9505),
        amount.clamp(0.0, 1.0) as f64,
    )
}
