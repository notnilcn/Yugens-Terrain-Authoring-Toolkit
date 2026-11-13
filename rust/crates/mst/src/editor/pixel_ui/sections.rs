//! Static description of the pixelizer editor UI: toolbar sections and the attributes each one
//! shows in the bottom strip. Pure data; `plugin.rs` turns it into controls.

/// How an attribute is edited.
#[derive(Clone, Copy)]
pub enum Kind {
    Float { min: f64, max: f64, step: f64 },
    Int { min: f64, max: f64 },
    Bool,
    Color,
    /// (label, stored integer value)
    Enum(&'static [(&'static str, i64)]),
    /// Button calling this method of the target (no arguments; the method records its own
    /// undo step), e.g. MarchingSquaresErosion `erode`.
    Action(&'static str),
    /// Progress of a background job: a progress bar fed by the `getter` method (0..1, < 0 =
    /// idle) and a button calling `cancel`. Shown only while the job runs; the section's
    /// Action buttons are disabled meanwhile (MarchingSquaresErosion `get_progress` / `cancel`).
    Progress { getter: &'static str, cancel: &'static str },
}

/// Where the value lives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// A property of the section's target node.
    Prop,
    /// A shader parameter of the `ShaderMaterial` in the named property of the target
    /// (PixelWater `material` / `waterfall_material`).
    Shader(&'static str),
    /// Component 0/1/2 (x/y/z) of a Vector2/Vector3 property.
    Axis(u8),
}

#[derive(Clone, Copy)]
pub struct Field {
    pub label: &'static str,
    pub prop: &'static str,
    pub kind: Kind,
    pub store: Store,
}

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
const fn e(label: &'static str, prop: &'static str, items: &'static [(&'static str, i64)]) -> Field {
    Field { label, prop, kind: Kind::Enum(items), store: Store::Prop }
}
const fn act(label: &'static str, method: &'static str) -> Field {
    Field { label, prop: method, kind: Kind::Action(method), store: Store::Prop }
}
const fn progress(label: &'static str, getter: &'static str, cancel: &'static str) -> Field {
    Field { label, prop: getter, kind: Kind::Progress { getter, cancel }, store: Store::Prop }
}
const fn shader(mut fd: Field) -> Field {
    fd.store = Store::Shader("material");
    fd
}
/// PixelWater `waterfall_material` (`waterfall.gdshader`; created once the water has falls).
const fn fall_shader(mut fd: Field) -> Field {
    fd.store = Store::Shader("waterfall_material");
    fd
}
const fn axis(mut fd: Field, a: u8) -> Field {
    fd.store = Store::Axis(a);
    fd
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sec {
    Renderer,
    Outlines,
    Color,
    GodRays,
    Sky,
    DayNight,
    Weather,
    Water,
    Fireflies,
    Particles,
    Birds,
    Smoke,
    Lights,
    Object,
    Camera,
    Debug,
    GridFog,
    Erosion,
    WildFlowers,
    Fire,
}

pub struct Section {
    pub sec: Sec,
    pub label: &'static str,
    pub icon: &'static str,
    pub tooltip: &'static str,
    /// Node class edited by this section.
    pub class: Option<&'static str>,
    /// Draw a separator in the toolbar before this button.
    pub group_start: bool,
    pub fields: &'static [Field],
}

/// Inheritable tri-state (PixelObject / PixelObjectProfile).
const INHERIT_TRI: &[(&str, i64)] = &[("Inherit", -1), ("Off", 0), ("On", 1)];

pub const ICON_DIR: &str = "res://addons/MarchingSquaresTerrain/editor/icons/pixel/";
/// Terrain feature node icons (outside `ICON_DIR`: `widgets::icon` loads `res://` names as is).
pub const EROSION_ICON: &str = "res://addons/MarchingSquaresTerrain/editor/icons/erosion.svg";
pub const WILD_FLOWERS_ICON: &str = "res://addons/MarchingSquaresTerrain/editor/icons/wild_flowers.svg";
pub const EROSION: &str = "MarchingSquaresErosion";
pub const WILD_FLOWERS: &str = "MarchingSquaresWildFlowers";
pub use super::fire_section::FIRE;

/// Classes that live under a `MarchingSquaresTerrain` (added as a child of the selected /
/// first terrain instead of the scene root).
pub fn is_terrain_child(class: &str) -> bool {
    matches!(class, EROSION | WILD_FLOWERS | FIRE)
}

/// "Wire refs" action, shared by the Pixel World panel and the toolbar "Pixel World" menu.
pub const WIRE_LABEL: &str = "Wire Sun / Env Refs";
pub const WIRE_TOOLTIP: &str =
    "Fill empty sun / environment references of Sky and Day/Night with the scene's first\nDirectionalLight3D / WorldEnvironment (undoable).";
/// Labels of the "Pixel World" toolbar menu / panel sections.
pub const PRESET_HEADER: &str = "Lighting Preset";
/// Strip pages of the sections with many attributes: (page name, tooltip, index of its first
/// field). A page runs to the next page's first field. Sections not listed have one page.
pub type Page = (&'static str, &'static str, usize);
pub static PAGES: &[(Sec, &[Page])] = &[
    (Sec::Fire, super::fire_section::FIRE_PAGES),
    (
        Sec::Water,
        &[
            ("Surface", "Size, mesh detail and depth bands.", 0),
            ("Waves", "Gerstner swell and chop.", 6),
            ("Light", "Grazing light, reflections and sparkle.", 11),
            ("Caustics", "Caustic light pattern on the ground under the water.", 16),
            ("Foam", "Shore foam and surge pulses.", 20),
            ("Colors", "Water and sky reflection colors.", 26),
            ("Waterfalls", "Raised water cells (pools) and the waterfalls where they spill over (hex / triangle terrains).", 33),
        ],
    ),
    (
        Sec::DayNight,
        &[
            ("Time", "Time of day, day length and sun path.", 0),
            ("Sun & Fog", "Sun energy, ambient light and fog.", 8),
            ("Rim Light", "Cel light wrap and rim light.", 14),
        ],
    ),
    (
        Sec::Weather,
        &[
            ("Weather", "Storm amount, transition and ground effects.", 0),
            ("Snow", "Optional snow (off by default): snow on top of any weather, snowfall amount,\nhow fast the ground cover builds up and melts, flake count / size / fall.", 4),
            ("Particles", "Rain drop area, speed and size.", 15),
            ("Rain", "Flipbook rain sheets and ground / water splashes.", 21),
            ("Lightning", "Lightning frequency and flash.", 28),
            ("Colors", "Rain and bolt colors.", 30),
        ],
    ),
    (
        Sec::Particles,
        &[
            ("Field", "Preset, density and weather response.", 0),
            ("Layers", "World-anchored sheets: orientation, count, spacing, texel size, fall and wind.", 4),
            ("Flipbook", "The baked particle animation (tiles in space, loops in time).", 14),
            ("Look", "Colors, sunlight / god-ray shaft masks and time of day.", 26),
        ],
    ),
    (
        Sec::Birds,
        &[
            ("Flock", "Flock size, area and speed.", 0),
            ("Steering", "Boids steering and wing beat.", 4),
            ("Landing", "Perching and scaring.", 11),
            ("Look", "Outline, editor preview and color.", 15),
        ],
    ),
    (
        Sec::Smoke,
        &[
            ("Emission", "Puff rate, life, rise and size.", 0),
            ("Fire Light", "Detail and the flickering fire light.", 8),
            ("Options", "Emitting, wind and outline.", 12),
            ("Colors", "Fire, smoke, soot and flame colors.", 15),
        ],
    ),
    (
        Sec::GodRays,
        &[
            ("Rays", "On / off, strength, color and the cloud threshold source (Stable3D day curve or PixelSky coverage, Dawn/Dusk keeps the gaps open for a low sun).", 0),
            ("Sheets", "Light-sheet spacing, stacking, stack adaptation and distance fades.", 7),
            ("Clouds", "How the cloud gaps light the sheets.", 15),
            ("Near / Far", "Camera near fade and depth cutoff.", 19),
        ],
    ),
    (
        Sec::Erosion,
        &[
            ("Basics", "Erode button (progress and Cancel while it runs), droplets per press, seed, the write-back mode (Auto / Raw / Terraced, level step, small-plateau cleanup) and the height raster.", 0),
            ("Droplet", "Minor Deity's droplet parameters.", 9),
        ],
    ),
    (
        Sec::WildFlowers,
        &[
            ("Placement", "Scatter / clear and Minor Deity's flower placement.", 0),
            ("Ground Water", "Ground water that decides where flowers grow.", 9),
        ],
    ),
    (
        Sec::GridFog,
        &[
            ("Grid", "Shader grid overlay.", 0),
            ("Fog of War", "Fog of war and cell data.", 6),
        ],
    ),
];

/// Pages of `sec` (empty = one page).
pub fn pages(sec: Sec) -> &'static [Page] {
    PAGES.iter().find(|p| p.0 == sec).map(|p| p.1).unwrap_or(&[])
}

/// Field index range of page `page` of a section with `n` fields.
pub fn page_range(sec: Sec, page: usize, n: usize) -> std::ops::Range<usize> {
    let p = pages(sec);
    if p.is_empty() {
        return 0..n;
    }
    let page = page.min(p.len() - 1);
    let end = p.get(page + 1).map(|x| x.2).unwrap_or(n);
    p[page].2.min(n)..end.min(n)
}

/// Name of the viewport toolbar toggle (every hint refers to it by this name).
pub const PREVIEW_LABEL: &str = "Pixel Preview";

pub const RENDERER: &str = "PixelRenderer";

pub static SECTIONS: &[Section] = &[
    Section {
        sec: Sec::Renderer,
        label: "Renderer",
        icon: "renderer",
        tooltip: "Renderer\nPixelRenderer: mode (Anchor = full pipeline, Low Res = color only), macro-pixel size,\nresolution and camera snapping.",
        class: Some(RENDERER),
        group_start: true,
        fields: &[
            b("Enabled", "enabled"),
            e("Mode", "mode", &[("Anchor", 0), ("Low Res", 1)]),
            i("Pixel Size", "pixel_size", 1.0, 16.0),
            i("Fixed Height", "fixed_height", 0.0, 1080.0),
            b("Snap Camera", "snap_camera"),
            b("Smooth Pan", "smooth_scroll"),
            b("Sharp Scale", "sharp_upscale"),
            f("Depth Range", "depth_range", 1.0, 4000.0, 1.0),
            i("Obj. Pixel", "object_pixel_size", 1.0, 4.0),
        ],
    },
    Section {
        sec: Sec::Outlines,
        label: "Outlines",
        icon: "outlines",
        tooltip: "Outlines\nDepth/normal edge detection, outline darkening and highlights.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            b("Outlines", "outlines"),
            b("Background", "outline_background"),
            f("Depth Thr.", "depth_threshold", 0.01, 10.0, 0.01),
            f("Normal Thr.", "normal_threshold", 0.0, 2.0, 0.01),
            f("Darken", "outline_darken", 0.0, 1.0, 0.01),
            f("Highlight", "highlight", 0.0, 1.0, 0.01),
            b("Wide Ids", "extended_ids"),
            b("Id Edges", "id_edges"),
            b("Obj. Outline", "object_outline"),
            f("Fold Limit", "fold_limit", 0.0, 20.0, 0.1),
            b("Ledge Walls", "ledge_walls"),
            i("Auto Id From", "auto_id_first", 1.0, 253.0),
            b("Auto Focus", "auto_focus"),
        ],
    },
    Section {
        sec: Sec::Color,
        label: "Color / Palette",
        icon: "palette",
        tooltip: "Color / Palette\nPalette quantisation, ordered dithering and color grading.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            b("Use Palette", "palette_enabled"),
            f("Mix", "palette_mix", 0.0, 1.0, 0.01),
            b("Dither", "dither_enabled"),
            f("Spread", "dither_spread", 0.0, 0.5, 0.001),
            e("Matrix", "dither_size", &[("Bayer 4", 4), ("Bayer 8", 8)]),
            f("Saturation", "saturation", 0.0, 2.0, 0.01),
            f("Contrast", "contrast", 0.0, 2.0, 0.01),
            f("Brightness", "brightness", -1.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::GodRays,
        label: "God Rays",
        icon: "god_rays",
        tooltip: "God Rays\nStable3D light sheets: world-anchored sheets along the sun beam stack up the sunlight\nlet through the PixelSky cloud gaps. Defaults = Stable3D's default material (Strength 1).",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            b("God Rays", "god_rays"),
            f("Strength", "ray_strength", 0.0, 4.0, 0.01),
            c("Color", "ray_color"),
            b("Sun Color", "ray_use_sun_color"),
            b("Day Cycle", "ray_day_cycle"),
            f("Dawn/Dusk", "ray_dawn_dusk", 0.0, 1.0, 0.01),
            i("Bands", "ray_bands", 1.0, 16.0),
            i("Max Sheets", "ray_samples", 2.0, 48.0),
            f("Spacing", "ray_sheet_spacing", 0.05, 50.0, 0.01),
            f("Quantum", "ray_sheet_quantum", 0.0, 10.0, 0.01),
            f("Max Stack", "ray_sheet_max_stack", 0.0, 10.0, 0.01),
            f("Adaptation", "ray_adaptation", 0.0, 10.0, 0.01),
            f("Falloff", "ray_sheet_distance_falloff", 0.0, 1.0, 0.001),
            f("Surface Fade", "ray_sheet_surface_fade", 0.0, 1000.0, 0.1),
            f("Max Dist.", "ray_max_distance", 1.0, 4000.0, 1.0),
            f("Cloud Soft.", "ray_sheet_cloud_softness", 0.0, 0.5, 0.0005),
            f("Gap Erosion", "ray_gap_erosion", 0.0, 0.99, 0.01),
            f("Cloud Shadow", "ray_cloud_shadow_strength", 0.0, 1.0, 0.01),
            f("Sun Follow", "ray_sun_follow", 0.0, 1.0, 0.01),
            f("Near Start", "ray_near_fade_start", 0.0, 100.0, 0.1),
            f("Near Fade", "ray_near_fade", 0.0, 100.0, 0.1),
            f("Near Power", "ray_near_fade_power", 0.05, 8.0, 0.01),
            f("Depth Cutoff", "ray_depth_cutoff", 0.0, 1000.0, 0.1),
        ],
    },
    Section {
        sec: Sec::Sky,
        label: "Sky & Clouds",
        icon: "sky",
        tooltip: "Sky & Clouds\nPixelSky: cloud shadows on the ground and the optional cloud deck. Its clouds drive\nthe shared cloud field (pt_cloud_* globals) that terrain, grass, water and god rays read;\nwith several skies, `cloud_camera_scope` picks the one for each camera.",
        class: Some("PixelSky"),
        group_start: true,
        fields: &[
            b("Clouds", "clouds_enabled"),
            f("Coverage", "coverage", 0.0, 1.0, 0.01),
            f("Softness", "softness", 0.0, 0.5, 0.005),
            f("Scale", "cloud_scale", 0.001, 0.2, 0.001),
            axis(f("Wind X", "wind", -1.0, 1.0, 0.001), 0),
            axis(f("Wind Y", "wind", -1.0, 1.0, 0.001), 1),
            f("Height", "cloud_height", 1.0, 500.0, 0.5),
            f("Shadow", "shadow_strength", 0.0, 1.0, 0.01),
            i("Bands", "bands", 0.0, 8.0),
            b("Deck", "deck_enabled"),
            f("Deck Size", "deck_size", 10.0, 4000.0, 1.0),
            f("Opacity", "deck_opacity", 0.0, 1.0, 0.01),
            c("Color", "deck_color"),
            c("Shade", "deck_shade_color"),
        ],
    },
    Section {
        sec: Sec::DayNight,
        label: "Day / Night",
        icon: "day_night",
        tooltip: "Day / Night\nPixelDayNight: sun arc, sky/ambient grading, fog, cel light ramp and rim light over the day.",
        class: Some("PixelDayNight"),
        group_start: false,
        fields: &[
            f("Time", "time_of_day", 0.0, 1.0, 0.001),
            f("Override", "time_override", -1.0, 1.0, 0.001),
            b("Running", "running"),
            f("Day Length", "day_length_seconds", 1.0, 3600.0, 1.0),
            f("Time Scale", "time_scale", 0.0, 100.0, 0.1),
            f("Latitude", "latitude_tilt_deg", 0.0, 80.0, 0.5),
            f("Azimuth", "azimuth_deg", -180.0, 180.0, 0.5),
            b("Shadows", "sun_shadows"),
            f("Sun Energy", "sun_energy_scale", 0.0, 4.0, 0.01),
            f("Ambient", "ambient_energy_scale", 0.0, 4.0, 0.01),
            b("Fog", "apply_fog"),
            f("Fog Day", "fog_density", 0.0, 0.1, 0.0001),
            f("Fog Night", "fog_night_density", 0.0, 0.1, 0.0001),
            e("Light Ramp", "light_ramp_preset", &[("Off", 0), ("Classic", 1), ("Soft", 2), ("Hard", 3), ("Warm Shadows", 4), ("Custom", 5)]),
            f("Light Wrap", "light_wrap", -0.9, 1.0, 0.01),
            f("Rim", "rim_strength", 0.0, 2.0, 0.01),
            f("Rim Width", "rim_width", 0.0, 1.0, 0.01),
            f("Rim Back", "rim_backlight", 0.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::Weather,
        label: "Weather",
        icon: "weather",
        tooltip: "Weather\nPixelWeather: blends Clear / Cloudy / Foggy / Rain / Snow / Storm (clouds, light,\nfog, wet ground, pixel rain, lightning) and optional snow (Snow page: falling flakes,\nground cover that builds up and melts). API: set_weather(\"rain\").",
        class: Some("PixelWeather"),
        group_start: false,
        fields: &[
            e("Weather", "weather", &[("Clear", 0), ("Cloudy", 1), ("Foggy", 2), ("Rain", 3), ("Snow", 4), ("Storm", 5)]),
            f("Transition", "transition_seconds", 0.0, 120.0, 0.1),
            f("Ground", "ground_seconds", 0.0, 600.0, 0.5),
            f("Strength", "strength", 0.0, 1.0, 0.01),
            b("Snow", "snow_enabled"),
            f("Intensity", "snow_intensity", 0.0, 1.0, 0.01),
            f("Accum./min", "snow_accumulation", 0.0, 10.0, 0.01),
            f("Melt/min", "snow_melt", 0.0, 10.0, 0.01),
            f("Max Cover", "snow_max_cover", 0.0, 1.0, 0.01),
            i("Flakes", "snow_flakes", 0.0, 4000.0),
            i("Flake Size", "flake_texels", 1.0, 3.0),
            f("Big Flakes", "snow_big_flakes", 0.0, 1.0, 0.01),
            f("Fall Speed", "snow_speed", 0.1, 10.0, 0.05),
            f("Sway", "snow_sway", 0.0, 3.0, 0.01),
            c("Flake Color", "snow_color"),
            i("Max Drops", "max_drops", 0.0, 20000.0),
            axis(f("Area X", "area", 1.0, 500.0, 0.5), 0),
            axis(f("Area Y", "area", 1.0, 200.0, 0.5), 1),
            axis(f("Area Z", "area", 1.0, 500.0, 0.5), 2),
            f("Rain Speed", "rain_speed", 1.0, 60.0, 0.1),
            i("Streak", "streak_texels", 1.0, 12.0),
            e("Rain Style", "rain_style", &[("Flipbook", 0), ("Particles", 1)]),
            i("Rain Layers", "rain_layers", 1.0, 32.0),
            f("Layer Gap", "rain_layer_spacing", 0.2, 20.0, 0.05),
            f("Rain Texel", "rain_texel", 0.01, 0.5, 0.005),
            i("Splashes", "max_splashes", 0.0, 8000.0),
            f("Splash/s", "splash_rate", 0.1, 10.0, 0.05),
            i("Splash R.", "splash_radius", 1.0, 8.0),
            b("Lightning", "lightning_enabled"),
            f("Flash", "flash_energy", 0.0, 8.0, 0.01),
            c("Rain", "rain_color"),
            c("Bolt", "bolt_color"),
        ],
    },
    Section {
        sec: Sec::Water,
        label: "Water",
        icon: "water",
        tooltip: "Water\nPixelWater: depth-banded color, foam and waves; raised pools and waterfalls\non hex / triangle terrains (Waterfalls page).",
        class: Some("PixelWater"),
        group_start: false,
        fields: &[
            axis(f("Size X", "size", 1.0, 4000.0, 0.5), 0),
            axis(f("Size Z", "size", 1.0, 4000.0, 0.5), 1),
            i("Subdiv.", "subdivisions", 0.0, 256.0),
            shader(f("Depth", "depth_scale", 0.1, 32.0, 0.1)),
            shader(i("Depth Bands", "depth_bands", 1.0, 8.0)),
            shader(f("Rim Dither", "rim_dither", 0.0, 1.0, 0.01)),
            shader(f("Swell H.", "swell_height", 0.0, 2.0, 0.005)),
            shader(f("Swell Len.", "swell_length", 1.0, 80.0, 0.1)),
            shader(f("Steepness", "swell_steepness", 0.0, 1.0, 0.01)),
            shader(f("Swell Speed", "swell_speed", 0.0, 4.0, 0.01)),
            shader(f("Chop H.", "wave_height", 0.0, 1.0, 0.005)),
            shader(f("Grazing", "grazing_strength", 0.0, 1.0, 0.01)),
            shader(f("Reflection", "reflection_strength", 0.0, 1.0, 0.01)),
            shader(i("Refl. Bands", "reflection_bands", 1.0, 6.0)),
            shader(f("Turq. Depth", "turquoise_depth", 0.0, 8.0, 0.01)),
            shader(f("Sparkle", "sparkle", 0.0, 2.0, 0.01)),
            f("Caustics", "caustics_strength", 0.0, 2.0, 0.01),
            f("Caust. Scale", "caustics_scale", 0.05, 4.0, 0.01),
            i("Caust. Bands", "caustics_bands", 1.0, 6.0),
            f("Caust. Reach", "caustics_reach", 0.0, 4.0, 0.01),
            shader(f("Foam Width", "foam_width", 0.0, 2.0, 0.01)),
            shader(f("Foam Speed", "foam_speed", 0.0, 4.0, 0.01)),
            shader(f("Surge/s", "foam_pulse_speed", 0.0, 4.0, 0.01)),
            shader(f("Surge Pause", "foam_dead_time", 0.0, 0.95, 0.01)),
            shader(f("Surge", "foam_surge", 0.0, 6.0, 0.01)),
            shader(i("Bands", "bands", 1.0, 8.0)),
            shader(c("Shallow", "shallow_color")),
            shader(c("Deep", "deep_color")),
            shader(c("Turquoise", "turquoise_color")),
            shader(c("Grazing", "grazing_color")),
            shader(c("Sun Side", "sky_toward_sun")),
            shader(c("Away Side", "sky_away_from_sun")),
            shader(c("Foam", "foam_color")),
            i("Fall Levels", "waterfall_min_levels", 1.0, 64.0),
            f("Level H.", "waterfall_level_height", 0.0, 10.0, 0.01),
            fall_shader(f("Fall Speed", "fall_speed", 0.0, 12.0, 0.05)),
            fall_shader(f("Streaks/m", "streak_density", 0.1, 16.0, 0.1)),
            fall_shader(f("Streak Len.", "streak_length", 0.05, 4.0, 0.01)),
            fall_shader(f("Base Foam", "base_foam", 0.0, 1.0, 0.01)),
            fall_shader(f("Spray Rate", "spray_rate", 0.0, 4.0, 0.01)),
            fall_shader(f("Spray Rise", "spray_rise", 0.0, 6.0, 0.01)),
            fall_shader(c("Fall Water", "water_color")),
            fall_shader(c("Fall Streak", "streak_color")),
            fall_shader(c("Fall Foam", "foam_color")),
        ],
    },
    Section {
        sec: Sec::Fireflies,
        label: "Fireflies",
        icon: "fireflies",
        tooltip: "Fireflies\nPixelFireflies: a blinking, wandering swarm that fades in at night.",
        class: Some("PixelFireflies"),
        group_start: false,
        fields: &[
            i("Count", "count", 0.0, 4096.0),
            axis(f("Area X", "extents", 0.1, 500.0, 0.1), 0),
            axis(f("Area Y", "extents", 0.1, 100.0, 0.1), 1),
            axis(f("Area Z", "extents", 0.1, 500.0, 0.1), 2),
            i("Seed", "seed", 0.0, 99999.0),
            c("Color", "color"),
            f("Size", "size", 0.01, 2.0, 0.01),
            f("Wander", "wander", 0.0, 4.0, 0.01),
            f("Blink", "blink_speed", 0.0, 10.0, 0.01),
            f("Glow", "energy", 0.0, 8.0, 0.01),
            f("Night Only", "night_only", 0.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::Particles,
        label: "Particle Field",
        icon: "particles",
        tooltip: "Particle Field\nPixelParticleField: a prerendered, tiling and looping flipbook of pixel particles on\nworld-anchored layers (dust motes in the sunbeams, rain, snow, embers, leaves); stays put\nunder ortho and perspective camera moves.",
        class: Some("PixelParticleField"),
        group_start: false,
        fields: &[
            e("Preset", "preset", &[("Dust Motes", 0), ("Rain", 1), ("Snow", 2), ("Embers", 3), ("Leaves", 4)]),
            b("Emitting", "emitting"),
            f("Density", "density", 0.0, 1.0, 0.01),
            e("Weather", "weather_response", &[("Always", 0), ("Fair Weather", 1), ("Rain", 2), ("Snow", 3)]),
            e("Projection", "projection", &[("Ground", 0), ("Upright", 1)]),
            i("Layers", "layers", 1.0, 64.0),
            f("Spacing", "spacing", 0.05, 20.0, 0.01),
            f("Texel", "texel_size", 0.01, 1.0, 0.005),
            f("Extent", "extent", 0.0, 500.0, 0.5),
            f("Height", "height", 0.5, 200.0, 0.5),
            f("Band", "band", 0.0, 50.0, 0.1),
            f("Fall", "fall_speed", -10.0, 30.0, 0.01),
            axis(f("Wind X", "wind", -20.0, 20.0, 0.01), 0),
            axis(f("Wind Y", "wind", -20.0, 20.0, 0.01), 1),
            i("Tile", "tile_size", 16.0, 256.0),
            i("Frames", "frame_count", 4.0, 128.0),
            i("Count", "particle_count", 1.0, 2000.0),
            i("Lifetime", "lifetime", 1.0, 128.0),
            axis(f("Velocity X", "velocity", -16.0, 16.0, 0.01), 0),
            axis(f("Velocity Y", "velocity", -16.0, 16.0, 0.01), 1),
            f("Drift", "drift", 0.0, 4.0, 0.01),
            f("Sway", "sway", 0.0, 8.0, 0.1),
            i("Fade", "fade_frames", 0.0, 64.0),
            i("Streak", "streak_length", 0.0, 12.0),
            i("Seed", "seed", 0.0, 99999.0),
            f("FPS", "fps", 0.0, 60.0, 0.1),
            c("Color", "color"),
            c("Color B", "color_b"),
            f("Brightness", "brightness", 0.0, 8.0, 0.01),
            f("Sunlit", "sunlit", 0.0, 1.0, 0.01),
            f("Beam", "beam", 0.0, 1.0, 0.01),
            f("Shade", "shade", 0.0, 1.0, 0.01),
            f("Night", "night", -1.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::Birds,
        label: "Birds",
        icon: "birds",
        tooltip: "Birds\nPixelBirds: a boids flock of 2-frame flapping pixel birds that circle,\nland on high ground and scatter when the camera comes near.",
        class: Some("PixelBirds"),
        group_start: false,
        fields: &[
            i("Count", "count", 1.0, 256.0),
            f("Radius", "radius", 1.0, 200.0, 0.1),
            f("Altitude", "altitude", 0.0, 100.0, 0.1),
            f("Speed", "speed", 0.1, 30.0, 0.1),
            f("Neighbours", "neighbor_distance", 0.1, 20.0, 0.05),
            f("Separation", "separation", 0.0, 4.0, 0.01),
            f("Alignment", "alignment", 0.0, 4.0, 0.01),
            f("Cohesion", "cohesion", 0.0, 4.0, 0.01),
            f("Turn", "turn_rate", 0.1, 10.0, 0.05),
            f("Flaps/s", "flap_speed", 0.0, 20.0, 0.1),
            f("Size", "size", 0.05, 4.0, 0.01),
            f("Land Every", "land_interval", 1.0, 600.0, 0.5),
            f("Perch", "perch_seconds", 0.5, 600.0, 0.5),
            f("Scare Dist.", "scare_distance", 0.0, 100.0, 0.1),
            b("Landing", "landing"),
            b("Outline", "outline"),
            b("In Editor", "preview_in_editor"),
            c("Color", "color"),
        ],
    },
    Section {
        sec: Sec::Smoke,
        label: "Smoke & Fire",
        icon: "smoke",
        tooltip: "Smoke & Fire\nPixelSmoke: tumbling outlined puffs drifting with the sky wind, optional flames\nwith a flickering light; burst(position, count) for build/destroy puffs.",
        class: Some("PixelSmoke"),
        group_start: false,
        fields: &[
            f("Rate", "rate", 0.0, 30.0, 0.05),
            f("Lifetime", "lifetime", 0.2, 30.0, 0.05),
            f("Rise", "rise_speed", 0.0, 10.0, 0.01),
            f("Spread", "spread", 0.0, 4.0, 0.01),
            f("Start Size", "start_size", 0.02, 4.0, 0.01),
            f("End Size", "end_size", 0.02, 8.0, 0.01),
            f("Wind", "wind_strength", 0.0, 10.0, 0.01),
            i("Max Puffs", "max_puffs", 1.0, 1024.0),
            i("Detail", "detail", 4.0, 16.0),
            f("Light", "light_energy", 0.0, 16.0, 0.01),
            f("Light Range", "light_range", 0.1, 50.0, 0.1),
            f("Flicker", "flicker", 0.0, 1.0, 0.01),
            b("Emitting", "emitting"),
            b("Sky Wind", "use_sky_wind"),
            b("Outline", "outline"),
            b("Fire", "fire"),
            c("Smoke", "smoke_color"),
            c("Soot", "soot_color"),
            c("Flame", "flame_color"),
        ],
    },
    Section {
        sec: Sec::Lights,
        label: "Light Volumes",
        icon: "light_volume",
        tooltip: "Light Volumes\nPixelLightVolume: banded point light with a dithered light pool and a source halo.\nSelect one to edit it; the first in the scene is used otherwise.",
        class: Some("PixelLightVolume"),
        group_start: false,
        fields: &[
            f("Radius", "radius", 0.1, 100.0, 0.1),
            c("Color", "light_color"),
            f("Energy", "energy", 0.0, 16.0, 0.01),
            f("Flicker", "flicker", 0.0, 1.0, 0.01),
            b("Shadows", "shadows"),
            b("Volume", "volume_enabled"),
            f("Vol. Scale", "volume_scale", 0.05, 2.0, 0.01),
            f("Intensity", "volume_intensity", 0.0, 4.0, 0.01),
            f("Halo Size", "halo_size", 0.0, 4.0, 0.01),
            f("Halo", "halo_intensity", 0.0, 4.0, 0.01),
            i("Bands", "volume_bands", 1.0, 8.0),
            f("Night Boost", "night_boost", 0.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::Object,
        label: "Pixel Object",
        icon: "object",
        tooltip: "Pixel Object\nPixelObject: per-object pixel size, outline id/color, palette and dither\n(-1 / Inherit = parent PixelObject, then the renderer's Object Defaults).",
        class: Some("PixelObject"),
        group_start: true,
        fields: &[
            i("Pixel Size", "pixel_size", -1.0, 4.0),
            e("Outline", "outline", INHERIT_TRI),
            i("Outline Id", "outline_id", -1.0, 253.0),
            e("Palette", "palette", INHERIT_TRI),
            e("Dither", "dither", INHERIT_TRI),
            b("Own Color", "override_outline_color"),
            c("Outline Col.", "outline_color"),
            b("Texel Snap", "snap_to_texel"),
            b("Convert", "convert_materials"),
            i("Bands", "bands", 1.0, 8.0),
            f("Shade Floor", "shade_floor", 0.0, 1.0, 0.01),
            f("Rim", "rim_strength", 0.0, 1.0, 0.01),
        ],
    },
    Section {
        sec: Sec::Camera,
        label: "Orbit Camera",
        icon: "camera",
        tooltip: "Orbit Camera\nPixelOrbitCamera: RTS camera (WASD pan, Q/E rotate, wheel zoom).\nOrthographic keeps pixels world-locked.",
        class: Some("PixelOrbitCamera"),
        group_start: false,
        fields: &[
            b("Ortho", "orthographic"),
            f("Ortho Size", "ortho_size", 0.5, 500.0, 0.1),
            f("Distance", "distance", 1.0, 1000.0, 0.1),
            f("Yaw", "yaw_deg", -360.0, 360.0, 0.5),
            f("Pitch", "pitch_deg", 0.0, 90.0, 0.5),
            f("Yaw Snap", "yaw_snap_deg", 0.0, 180.0, 1.0),
            f("Pan Speed", "pan_speed", 0.0, 10.0, 0.01),
            f("Smoothing", "smoothing", 0.0, 60.0, 0.1),
            b("Edge Pan", "edge_pan"),
        ],
    },
    Section {
        sec: Sec::Debug,
        label: "Debug Views",
        icon: "debug",
        tooltip: "Debug Views\nShow the renderer's intermediate buffers (normals, depth, pixel size, outline flags),\nwhy each outline is drawn (red silhouette, blue id edge, yellow crease, green suppressed\nfold / ledge) and the god-ray sheet stack.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[e(
            "View",
            "debug_view",
            &[("Final", 0), ("Normals", 1), ("Depth", 2), ("Pixel Size", 3), ("Outline Ids", 4), ("Raw Color", 5), ("Outline Cause", 6), ("God Rays", 7)],
        )],
    },
    Section {
        sec: Sec::GridFog,
        label: "Grid / Fog",
        icon: "grid",
        tooltip: "Grid / Fog\nMarchingSquaresTerrain overlays: cell grid lines (editor / in game), fog of war\nlook and terrain debug views (slope, contours, slot ids, chunk borders, cell ids).",
        class: Some("MarchingSquaresTerrain"),
        group_start: true,
        fields: &[
            b("Game Grid", "show_grid"),
            b("Editor Grid", "show_grid_in_editor"),
            c("Grid Color", "grid_color"),
            f("Grid Width", "grid_width", 0.005, 2.0, 0.005),
            f("Grid Fade", "grid_fade_distance", 0.0, 2000.0, 1.0),
            b("Chunk Lines", "grid_chunk_lines"),
            b("Game Fog", "fog_enabled"),
            b("Editor Fog", "fog_in_editor"),
            f("Fog Fade s", "fog_transition", 0.0, 5.0, 0.01),
            c("Fog Color", "fog_color"),
            c("Fog Dither", "fog_color_alt"),
            f("Hidden Dim", "fog_hidden_dim", 0.0, 1.0, 0.01),
            f("Hatch", "fog_hatch", 0.0, 1.0, 0.01),
            f("Softness", "fog_softness", 0.0, 0.5, 0.01),
            e(
                "Debug View",
                "debug_view",
                &[("Off", 0), ("Slope", 1), ("Height Contours", 2), ("Slot Ids", 3), ("Chunk Borders", 4), ("Cell Ids", 5), ("Cell Data", 6)],
            ),
        ],
    },
    Section {
        sec: Sec::Erosion,
        label: "Erosion",
        icon: EROSION_ICON,
        tooltip: "Erosion\nMarchingSquaresErosion: Minor Deity's hydraulic droplet erosion of the parent terrain.\nErode runs one batch of droplets (one undo step); each press continues with a new seed.",
        class: Some(EROSION),
        group_start: false,
        fields: &[
            act("Erode", "erode"),
            progress("Eroding", "get_progress", "cancel"),
            i("Droplets", "iterations", 100.0, 500000.0),
            i("Seed", "seed", -2147483648.0, 2147483647.0),
            e("Mode", "mode", &[("Auto", 0), ("Raw", 1), ("Terraced", 2)]),
            f("Level Step", "level_step", 0.0, 16.0, 0.05),
            i("Min Plateau", "min_plateau_cells", 0.0, 64.0),
            f("Height Scale", "height_scale", 0.0, 256.0, 0.1),
            i("Px / Cell", "pixels_per_cell", 1.0, 8.0),
            i("Radius", "erosion_radius", 2.0, 8.0),
            f("Inertia", "inertia", 0.0, 1.0, 0.001),
            f("Capacity", "sediment_capacity_factor", 0.0, 32.0, 0.01),
            f("Min Capacity", "min_sediment_capacity", 0.0, 1.0, 0.001),
            f("Erode Speed", "erode_speed", 0.0, 1.0, 0.01),
            f("Deposit", "deposit_speed", 0.0, 1.0, 0.01),
            f("Evaporate", "evaporate_speed", 0.0, 1.0, 0.001),
            f("Gravity", "gravity", 0.0, 32.0, 0.1),
            i("Lifetime", "max_droplet_lifetime", 1.0, 256.0),
            f("Water", "initial_water_volume", 0.0, 8.0, 0.01),
            f("Speed", "initial_speed", 0.0, 8.0, 0.01),
        ],
    },
    Section {
        sec: Sec::WildFlowers,
        label: "Wild Flowers",
        icon: WILD_FLOWERS_ICON,
        tooltip: "Wild Flowers\nMarchingSquaresWildFlowers: Minor Deity's clustered wild flowers on the parent terrain's\nscatter layer (8 species + bilberry, from cell noise and ground water; none under water).\nScatter replaces the previous flowers, Clear removes them (undoable).",
        class: Some(WILD_FLOWERS),
        group_start: false,
        fields: &[
            act("Scatter", "scatter_flowers"),
            act("Clear", "clear_flowers"),
            i("Seed", "seed", -2147483648.0, 2147483647.0),
            f("Density", "density", 0.0, 2.0, 0.01),
            f("Patch Scale", "patch_scale", 0.05, 8.0, 0.01),
            f("Threshold", "flower_threshold", 0.0, 1.0, 0.01),
            f("Max Slope", "max_slope", 0.0, 1.0, 0.005),
            i("Points/Cell", "points_per_cell", 2.0, 32.0),
            f("Jitter", "jitter", 0.0, 1.0, 0.01),
            f("Ground Water", "ground_water", 0.0, 1.0, 0.01),
            f("Variation", "ground_water_variation", 0.0, 1.0, 0.01),
            f("Shore Wet", "shore_wetness", 0.0, 1.0, 0.01),
            f("Water Level", "water_level", -256.0, 256.0, 0.1),
            f("Shore Dist.", "shore_distance", 0.01, 64.0, 0.01),
            b("Trees Shade", "trees_shade"),
        ],
    },
    super::fire_section::FIRE_SECTION,
];

/// Icon name (in `ICON_DIR`) for a node class.
pub fn icon_for(class: &str) -> &'static str {
    match class {
        "PixelDiagnostics" => "debug",
        "PixelPalette" => "palette",
        c => SECTIONS.iter().find(|s| s.class == Some(c)).map(|s| s.icon).unwrap_or("debug"),
    }
}

pub fn section(sec: Sec) -> &'static Section {
    SECTIONS.iter().find(|s| s.sec == sec).expect("section")
}

/// Section shown first when a node of `class` is selected.
pub fn default_section(class: &str) -> Sec {
    match class {
        "PixelSky" => Sec::Sky,
        "PixelDayNight" => Sec::DayNight,
        "PixelWater" => Sec::Water,
        "PixelWeather" => Sec::Weather,
        "PixelBirds" => Sec::Birds,
        "PixelSmoke" => Sec::Smoke,
        "PixelFireflies" => Sec::Fireflies,
        "PixelParticleField" => Sec::Particles,
        "PixelLightVolume" => Sec::Lights,
        "PixelObject" => Sec::Object,
        "PixelOrbitCamera" => Sec::Camera,
        "PixelPalette" => Sec::Color,
        EROSION => Sec::Erosion,
        WILD_FLOWERS => Sec::WildFlowers,
        FIRE => Sec::Fire,
        _ => Sec::Renderer,
    }
}

/// Classes this plugin handles.
pub const HANDLED: &[&str] = &[
    RENDERER,
    "PixelObject",
    "PixelSky",
    "PixelDayNight",
    "PixelWater",
    "PixelWeather",
    "PixelBirds",
    "PixelSmoke",
    "PixelFireflies",
    "PixelParticleField",
    "PixelLightVolume",
    "PixelOrbitCamera",
    "PixelPalette",
    EROSION,
    WILD_FLOWERS,
    FIRE,
];

/// Nodes listed in the "Pixel World" panel / menu: (class, label, unique per scene).
pub const ADDABLE: &[(&str, &str, bool)] = &[
    (RENDERER, "Renderer", true),
    ("PixelOrbitCamera", "Orbit Camera", true),
    ("PixelSky", "Sky & Clouds", true),
    ("PixelDayNight", "Day / Night", true),
    ("PixelWeather", "Weather", true),
    ("PixelWater", "Water", true),
    ("PixelFireflies", "Fireflies", false),
    ("PixelParticleField", "Particle Field", false),
    ("PixelBirds", "Birds", false),
    ("PixelSmoke", "Smoke & Fire", false),
    ("PixelLightVolume", "Light Volume", false),
    ("PixelObject", "Pixel Object", false),
    ("PixelDiagnostics", "Diagnostics", true),
    (EROSION, "Erosion", false),
    (WILD_FLOWERS, "Wild Flowers", false),
    (FIRE, "Grass Fire", false),
];

/// One strip tab of a node: a section's page (or the whole section when it has no pages).
pub struct Tab {
    pub label: &'static str,
    pub tooltip: &'static str,
    pub sec: Sec,
    pub page: usize,
}

/// First section editing `sec`'s class: the node's single left-toolbar entry. Every Pixel*
/// node has one toolbar button; its sections and pages are strip tabs (`class_tabs`).
pub fn head(sec: Sec) -> Sec {
    let s = section(sec);
    match s.class {
        Some(c) => SECTIONS.iter().find(|x| x.class == Some(c)).map(|x| x.sec).unwrap_or(sec),
        None => sec,
    }
}

/// Strip tabs of the node edited by `sec`: every page of every section of that class, in
/// toolbar order (e.g. the renderer: Renderer, Outlines, Color, god-ray pages, Debug).
pub fn class_tabs(sec: Sec) -> Vec<Tab> {
    let class = section(sec).class;
    let mut tabs = Vec::new();
    for s in SECTIONS.iter().filter(|s| if class.is_some() { s.class == class } else { s.sec == sec }) {
        let p = pages(s.sec);
        if p.is_empty() {
            tabs.push(Tab { label: s.label, tooltip: s.tooltip, sec: s.sec, page: 0 });
        } else {
            for (k, pg) in p.iter().enumerate() {
                tabs.push(Tab { label: pg.0, tooltip: pg.1, sec: s.sec, page: k });
            }
        }
    }
    tabs
}

/// The strip label of `class.prop` (node properties only; vector axes drop their " X" / " Y" /
/// " Z" suffix). The inspector shows the same label (`inspector_labels.rs`), so every property
/// has one name in both places.
pub fn strip_label(class: &str, prop: &str) -> Option<&'static str> {
    SECTIONS.iter().filter(|s| s.class == Some(class)).flat_map(|s| s.fields.iter()).find_map(|fd| {
        if fd.prop != prop || matches!(fd.kind, Kind::Action(_) | Kind::Progress { .. }) {
            return None;
        }
        match fd.store {
            Store::Prop => Some(fd.label),
            Store::Axis(_) => Some(fd.label.trim_end_matches(" X").trim_end_matches(" Y").trim_end_matches(" Z").trim_end_matches('.')),
            Store::Shader(_) => None,
        }
    })
}

/// Classes with at least one strip field (inspector label plugin).
pub fn has_strip_fields(class: &str) -> bool {
    SECTIONS.iter().any(|s| s.class == Some(class) && s.fields.iter().any(|fd| matches!(fd.store, Store::Prop | Store::Axis(_))))
}

/// Extra explanation shown in a strip control's tooltip (fields whose label alone says little).
pub fn field_hint(prop: &str) -> Option<&'static str> {
    Some(match prop {
        "ray_adaptation" => "Eye adaptation of the light-sheet stack (0 = off: raw Stable3D stack).\nA view across the beams passes many sheets and would turn into a flat, milky veil;\nthe stack is scaled by min(1, Adaptation / potential stack), so views along the sun\nare unchanged and side-on views keep distinct shafts. Default 1.6.",
        _ => return None,
    })
}
