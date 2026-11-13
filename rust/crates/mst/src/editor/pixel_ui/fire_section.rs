//! Pixel World section for `PixelFireSystem` (grass fire / rain / regrowth), registered by
//! `sections.rs` (`Sec::Fire`, `FIRE_PAGES`, `HANDLED`, `ADDABLE`).

use super::sections::{Field, Kind, Page, Sec, Section, Store};

pub const FIRE: &str = "PixelFireSystem";
pub const FIRE_ICON: &str = "res://addons/MarchingSquaresTerrain/editor/icons/pixel/fire.svg";

const fn f(label: &'static str, prop: &'static str, min: f64, max: f64, step: f64) -> Field {
    Field { label, prop, kind: Kind::Float { min, max, step }, store: Store::Prop }
}
const fn i(label: &'static str, prop: &'static str, min: f64, max: f64) -> Field {
    Field { label, prop, kind: Kind::Int { min, max }, store: Store::Prop }
}
const fn b(label: &'static str, prop: &'static str) -> Field {
    Field { label, prop, kind: Kind::Bool, store: Store::Prop }
}
const fn c(label: &'static str, prop: &'static str) -> Field {
    Field { label, prop, kind: Kind::Color, store: Store::Prop }
}
const fn act(label: &'static str, method: &'static str) -> Field {
    Field { label, prop: method, kind: Kind::Action(method), store: Store::Prop }
}

pub const FIRE_PAGES: &[Page] = &[
    ("Fire", "Ignite tool (click the terrain; Shift + click puts fires out), quick actions and the simulation.", 0),
    ("Spread", "How fast fire spreads, burns and throws embers.", 9),
    ("Wind & Rain", "Wind skew, rain dousing and wetness.", 14),
    ("Regrowth", "When and how fast burnt grass grows back.", 21),
    ("Look", "Flames, smoke, steam and fire lights.", 24),
];

pub const FIRE_SECTION: Section = Section {
    sec: Sec::Fire,
    label: "Grass Fire",
    icon: FIRE_ICON,
    tooltip: "Grass Fire\nPixelFireSystem: fire spreads through the parent terrain's grass with the wind, chars the\nground and shrinks the blades; rain puts it out with steam, and burnt grass regrows.\nEnable Ignite Tool and click the terrain to start a fire in the editor (Shift + click douses).",
    class: Some(FIRE),
    group_start: false,
    fields: &[
        b("Ignite Tool", "ignite_tool"),
        f("Ignite Radius", "ignite_radius", 0.1, 20.0, 0.1),
        act("Ignite Here", "ignite_here"),
        act("Put Out", "extinguish_all"),
        act("Reset", "reset_fire"),
        b("Simulate", "simulate"),
        b("In Editor", "preview_in_editor"),
        f("Cell Size", "texel_size", 0.1, 4.0, 0.01),
        i("Seed", "seed", -2147483648.0, 2147483647.0),
        f("Spread", "spread_rate", 0.0, 10.0, 0.01),
        f("Burn Time", "burn_time", 0.1, 60.0, 0.1),
        f("Embers", "ember_rate", 0.0, 1.0, 0.001),
        f("Ember Dist.", "ember_distance", 0.0, 10.0, 0.01),
        f("Max Step", "max_step", 0.0, 20.0, 0.05),
        b("Sky Wind", "use_sky_wind"),
        f("Wind Str.", "wind_strength", 0.0, 10.0, 0.01),
        f("Wind Skew", "wind_influence", 0.0, 4.0, 0.01),
        f("Rain", "rain_override", -1.0, 1.0, 0.01),
        f("Douse", "douse_rate", 0.0, 10.0, 0.01),
        f("Wet Rate", "wet_rate", 0.0, 4.0, 0.01),
        f("Wet Block", "wet_block", 0.01, 1.0, 0.01),
        f("Regrow After", "regrow_delay", 0.0, 600.0, 0.1),
        f("Regrow Time", "regrow_time", 0.1, 600.0, 0.1),
        f("Wet Boost", "wet_regrow_boost", 0.0, 10.0, 0.01),
        f("Flames", "flame_rate", 0.0, 40.0, 0.1),
        f("Flame Size", "flame_size", 0.02, 4.0, 0.01),
        f("Smoke", "smoke_rate", 0.0, 10.0, 0.01),
        f("Steam", "steam_amount", 0.0, 4.0, 0.01),
        i("Lights", "max_lights", 0.0, 8.0),
        f("Light", "light_energy", 0.0, 16.0, 0.01),
        f("Glow", "glow_intensity", 0.0, 4.0, 0.01),
        c("Core", "core_color"),
        c("Flame", "flame_color"),
        c("Rim", "rim_color"),
        c("Smoke Col.", "smoke_color"),
        c("Steam Col.", "steam_color"),
    ],
};
