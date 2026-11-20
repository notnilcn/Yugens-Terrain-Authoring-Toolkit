//! Static description of the pixel editor UI: toolbar sections and the attributes each one
//! shows in the bottom strip. Pure data; `plugin.rs` turns it into controls.

/// How an attribute is edited.
#[derive(Clone, Copy)]
pub enum Kind {
    Float {
        min: f64,
        max: f64,
        step: f64,
    },
    /// `greater`: the spin slider accepts values above `max` (the soft end of the drag range).
    Int {
        min: f64,
        max: f64,
        greater: bool,
    },
    Bool,
    Color,
    /// (label, stored integer value)
    Enum(&'static [(&'static str, i64)]),
    /// Button calling this method of the target (no arguments; the method records its own
    /// undo step), e.g. MarchingSquaresErosion `erode`.
    Action(&'static str),
    /// Progress of a background job: a progress bar fed by the `getter` method (0..1, < 0 =
    /// idle) and a button calling `cancel`. Shown only while the job runs; the section's
    /// Action buttons are disabled meanwhile (MarchingSquaresErosion `get_progress` / `abort`).
    Progress {
        getter: &'static str,
        cancel: &'static str,
    },
}

/// Where the value lives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// A property of the section's target node.
    Prop,
    /// A shader parameter of the `ShaderMaterial` in the named property of the target
    /// (PixelWater `stuff` / `cascade_stuff`).
    Shader(&'static str),
    /// Component 0/1/2 (x/y/z) of a Vector2/Vector3 property.
    Axis(u8),
}

#[derive(Clone, Copy)]
pub struct Field {
    pub label: &'static str,
    pub prop: &'static str,
    pub sort: Kind,
    pub store: Store,
}

const fn f(label: &'static str, prop: &'static str, min: f64, max: f64, step: f64) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Float { min, max, step },
        store: Store::Prop,
    }
}
const fn i(label: &'static str, prop: &'static str, min: f64, max: f64) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Int {
            min,
            max,
            greater: false,
        },
        store: Store::Prop,
    }
}
/// Int field whose `max` is only the drag range (typing a larger value is allowed).
const fn ig(label: &'static str, prop: &'static str, min: f64, max: f64) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Int {
            min,
            max,
            greater: true,
        },
        store: Store::Prop,
    }
}
const fn bx(label: &'static str, prop: &'static str) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Bool,
        store: Store::Prop,
    }
}
const fn cx(label: &'static str, prop: &'static str) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Color,
        store: Store::Prop,
    }
}
const fn e(
    label: &'static str,
    prop: &'static str,
    items: &'static [(&'static str, i64)],
) -> Field {
    Field {
        label,
        prop,
        sort: Kind::Enum(items),
        store: Store::Prop,
    }
}
const fn perform(label: &'static str, method: &'static str) -> Field {
    Field {
        label,
        prop: method,
        sort: Kind::Action(method),
        store: Store::Prop,
    }
}
const fn progress(label: &'static str, getter: &'static str, cancel: &'static str) -> Field {
    Field {
        label,
        prop: getter,
        sort: Kind::Progress { getter, cancel },
        store: Store::Prop,
    }
}
const fn shader(mut fd: Field) -> Field {
    fd.store = Store::Shader("stuff");
    fd
}
/// PixelWater `cascade_stuff` (`waterfall.gdshader`; created once the water has falls).
const fn fall_shader(mut fd: Field) -> Field {
    fd.store = Store::Shader("cascade_stuff");
    fd
}
const fn axle(mut fd: Field, a: u8) -> Field {
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
    Debug,
    GridFog,
    Erosion,
    WildFlowers,
    Fire,
}

pub struct Section {
    pub sec: Sec,
    pub label: &'static str,
    pub glyph: &'static str,
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
pub const WILD_FLOWERS_ICON: &str =
    "res://addons/MarchingSquaresTerrain/editor/icons/wild_flowers.svg";
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
pub const WIRE_TOOLTIP: &str = "Fill empty sun / environment references of Sky and Day/Night with the scene's first\nDirectionalLight3D / WorldEnvironment (undoable).";
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
            (
                "Caustics",
                "Caustic light pattern on the ground under the water.",
                16,
            ),
            ("Foam", "Shore foam and surge pulses.", 20),
            ("Colors", "Water and sky reflection colors.", 26),
            (
                "Waterfalls",
                "Raised water cells (pools) and the waterfalls where they spill over (hex / triangle terrains).",
                33,
            ),
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
            (
                "Snow",
                "Optional snow (off by default): snow on top of any weather, snowfall amount,\nhow fast the ground cover builds up and melts, flake count / size / fall.",
                4,
            ),
            ("Particles", "Rain drop area, speed and size.", 15),
            (
                "Rain",
                "Flipbook rain sheets and ground / water splashes.",
                21,
            ),
            ("Lightning", "Lightning frequency and flash.", 28),
            ("Colors", "Rain and bolt colors.", 30),
        ],
    ),
    (
        Sec::Particles,
        &[
            ("Field", "Preset, density and weather response.", 0),
            (
                "Layers",
                "World-anchored sheets: orientation, count, spacing, texel size, fall and wind.",
                4,
            ),
            (
                "Flipbook",
                "The baked particle animation (tiles in space, loops in time).",
                14,
            ),
            (
                "Look",
                "Colors, sunlight / god-ray shaft masks and time of day.",
                26,
            ),
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
            (
                "Rays",
                "On / off, ray alpha, colour and how many sheets a view ray may cross (distance, cut-off).",
                0,
            ),
            (
                "Sun",
                "Sun colour tint, day cycle and dawn/dusk (keeps the gaps open for a low sun), sun follow.",
                6,
            ),
            (
                "Sheets",
                "Sheet banding, spacing, stacking, stack adaptation, distance falloff and surface fade.",
                10,
            ),
            ("Clouds", "How the cloud gaps light the sheets.", 17),
            (
                "Depth Fade",
                "Camera start depth and the fade-in after it.",
                20,
            ),
        ],
    ),
    (
        Sec::Erosion,
        &[
            (
                "Basics",
                "Erode button (progress and Cancel while it runs), droplets per press, seed, the write-back mode (Auto / Raw / Terraced, level step, small-plateau cleanup) and the height raster.",
                0,
            ),
            ("Droplet", "The droplet erosion parameters.", 9),
        ],
    ),
    (
        Sec::WildFlowers,
        &[
            ("Placement", "Scatter / clear and the flower placement.", 0),
            (
                "Ground Water",
                "Ground water that decides where flowers grow.",
                9,
            ),
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
    PAGES
        .iter()
        .find(|p| p.0 == sec)
        .map(|p| p.1)
        .unwrap_or(&[])
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
        glyph: "renderer",
        tooltip: "Renderer\nPixelRenderer: mode (Anchor = full pipeline, Low Res = color only), macro-pixel size,\nresolution and camera snapping.",
        class: Some(RENDERER),
        group_start: true,
        fields: &[
            bx("Enabled", "active"),
            e("Mode", "mode", &[("Anchor", 0), ("Low Res", 1)]),
            ig("Pixel Size", "pixel_size", 1.0, 16.0),
            i("Fixed Height", "fixed_height", 0.0, 1080.0),
            bx("Texel Snap", "texel_snap"),
            bx("Sub-texel Scroll", "subtexel_scroll"),
            bx("Crisp Edges", "crisp_edges"),
            f("Depth Range", "depth_range", 1.0, 4000.0, 1.0),
            ig("Obj. Pixel", "object_pixel_size", 1.0, 16.0),
        ],
    },
    Section {
        sec: Sec::Outlines,
        label: "Outlines",
        glyph: "outlines",
        tooltip: "Outlines\nDepth/normal edge detection, outline darkening and highlights.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            bx("Outlines", "outlines"),
            bx("Background", "outline_background"),
            f("Depth Thr.", "depth_threshold", 0.0105, 10.0, 0.0105),
            f("Normal Thr.", "normal_threshold", 0.0, 2.0, 0.0105),
            f("Darken", "outline_darken", 0.0, 1.0, 0.0105),
            f("Highlight", "highlight", 0.0, 1.0, 0.0105),
            bx("Wide Ids", "extended_ids"),
            bx("Id Edges", "id_edges"),
            bx("Obj. Outline", "object_outline"),
            f("Fold Limit", "fold_limit", 0.0, 20.0, 0.1005),
            bx("Ledge Walls", "ledge_walls"),
            i("Auto Id From", "auto_id_first", 1.0, 253.0),
            bx("Auto Focus", "auto_focus"),
        ],
    },
    Section {
        sec: Sec::Color,
        label: "Color / Palette",
        glyph: "palette",
        tooltip: "Color / Palette\nPalette quantisation, ordered dithering and color grading.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            bx("Use Palette", "palette_enabled"),
            f("Mix", "palette_mix", 0.0, 1.0, 0.0105),
            bx("Dither", "dither_enabled"),
            f("Spread", "dither_spread", 0.0, 0.5, 0.0015),
            e("Matrix", "dither_size", &[("Bayer 4", 4), ("Bayer 8", 8)]),
            f("Saturation", "saturation", 0.0, 2.0, 0.0105),
            f("Contrast", "contrast", 0.0, 2.0, 0.0105),
            f("Brightness", "brightness", -1.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::GodRays,
        label: "God Rays",
        glyph: "god_rays",
        tooltip: "God Rays\nWorld-anchored sun shafts after robert-k's god rays (godrays.gdshader): light\nsheets along the sun beam stack up the sunlight let through the PixelSky cloud gaps.\nRay Alpha 1 = the default look.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[
            bx("God Rays", "god_rays"),
            f("Ray Alpha", "ray_alpha", 0.0, 4.0, 0.0105),
            cx("Color", "beam_tint"),
            i("Max Sheets", "ray_count", 2.0, 52.0),
            f("Ray Depth", "ray_depth", 1.0, 4000.0, 1.0),
            f("Reach", "ray_reach", 0.0, 1000.0, 0.1005),
            bx("Sun Color", "sun_color"),
            bx("Day Cycle", "day_cycle"),
            f("Dawn/Dusk", "dawn_dusk", 0.0, 1.0, 0.0105),
            f("Sun Follow", "sun_follow", 0.0, 1.0, 0.0105),
            i("Bands", "sheet_bands", 1.0, 16.0),
            f("Gap", "sheet_gap", 0.0505, 50.0, 0.0105),
            f("Energy", "sheet_energy", 0.0, 10.0, 0.0105),
            f("Max Stack", "sheet_stack_max", 0.0, 10.0, 0.0105),
            f("Falloff", "sheet_falloff", 0.0, 1.0, 0.0015),
            f("Surface Fade", "sheet_depth_fade", 0.0, 1000.0, 0.1005),
            f("Adaptation", "eye_adaptation", 0.0, 10.0, 0.0105),
            f("Cloud Soft.", "cloud_softness", 0.0, 0.5, 0.0004),
            f("Cloud Occl.", "cloud_occlusion", 0.0, 1.0, 0.0105),
            f("Gap Erode", "gap_erode", 0.0, 0.9905, 0.0105),
            f("Start Depth", "start_depth", 0.0, 100.0, 0.1005),
            f("Depth Fade", "depth_fade", 0.0, 100.0, 0.1005),
            f("Depth Power", "depth_fade_power", 0.0505, 8.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Sky,
        label: "Sky & Clouds",
        glyph: "sky",
        tooltip: "Sky & Clouds\nPixelSky: cloud shadows on the ground and the optional cloud deck. Its clouds drive\nthe shared cloud field (pt_cloud_* globals) that terrain, grass, water and god rays read;\nwith several skies, `cloud_camera_scope` picks the one for each camera.",
        class: Some("PixelSky"),
        group_start: true,
        fields: &[
            bx("Clouds", "clouds_enabled"),
            f("Coverage", "coverage", 0.0, 1.0, 0.0105),
            f("Softness", "softness", 0.0, 0.5, 0.0055),
            f("Scale", "cloud_scale", 0.0015, 0.2005, 0.0015),
            axle(f("Wind X", "wind", -1.0, 1.0, 0.0015), 0),
            axle(f("Wind Y", "wind", -1.0, 1.0, 0.0015), 1),
            f("Height", "cloud_height", 1.0, 500.0, 0.5),
            f("Shadow", "shadow_strength", 0.0, 1.0, 0.0105),
            i("Bands", "bands", 0.0, 8.0),
            bx("Deck", "deck_enabled"),
            f("Deck Size", "deck_size", 10.0, 4000.0, 1.0),
            f("Opacity", "deck_opacity", 0.0, 1.0, 0.0105),
            cx("Color", "deck_color"),
            cx("Shade", "deck_shade_color"),
        ],
    },
    Section {
        sec: Sec::DayNight,
        label: "Day / Night",
        glyph: "day_night",
        tooltip: "Day / Night\nPixelDayNight: sun arc, sky/ambient grading, fog, cel light ramp and rim light over the day.",
        class: Some("PixelDayNight"),
        group_start: false,
        fields: &[
            f("Time", "time_of_day", 0.0, 1.0, 0.0015),
            f("Override", "time_override", -1.0, 1.0, 0.0015),
            bx("Running", "running"),
            f("Day Length", "day_length_seconds", 1.0, 3600.0, 1.0),
            f("Time Scale", "time_scale", 0.0, 100.0, 0.1005),
            f("Latitude", "latitude_tilt_deg", 0.0, 80.0, 0.5),
            f("Azimuth", "azimuth_deg", -180.0, 180.0, 0.5),
            bx("Shadows", "sun_shadows"),
            f("Sun Energy", "sun_energy_scale", 0.0, 4.0, 0.0105),
            f("Ambient", "ambient_energy_scale", 0.0, 4.0, 0.0105),
            bx("Fog", "apply_fog"),
            f("Fog Day", "fog_density", 0.0, 0.1005, 0.0005),
            f("Fog Night", "fog_night_density", 0.0, 0.1005, 0.0005),
            e(
                "Light Ramp",
                "light_ramp_preset",
                &[
                    ("Off", 0),
                    ("Classic", 1),
                    ("Soft", 2),
                    ("Hard", 3),
                    ("Warm Shadows", 4),
                    ("Custom", 5),
                ],
            ),
            f("Light Wrap", "light_wrap", -0.9005, 1.0, 0.0105),
            f("Rim", "rim_strength", 0.0, 2.0, 0.0105),
            f("Rim Width", "rim_width", 0.0, 1.0, 0.0105),
            f("Rim Back", "rim_backlight", 0.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Weather,
        label: "Weather",
        glyph: "weather",
        tooltip: "Weather\nPixelWeather: blends Clear / Cloudy / Foggy / Rain / Snow / Storm (clouds, light,\nfog, wet ground, pixel rain, lightning) and optional snow (Snow page: falling flakes,\nground cover that builds up and melts). API: set_weather(\"rain\").",
        class: Some("PixelWeather"),
        group_start: false,
        fields: &[
            e(
                "Weather",
                "weather",
                &[
                    ("Clear", 0),
                    ("Cloudy", 1),
                    ("Foggy", 2),
                    ("Rain", 3),
                    ("Snow", 4),
                    ("Storm", 5),
                ],
            ),
            f("Transition", "transition_seconds", 0.0, 120.0, 0.1005),
            f("Ground", "ground_seconds", 0.0, 600.0, 0.5),
            f("Strength", "strength", 0.0, 1.0, 0.0105),
            bx("Snow", "snow_enabled"),
            f("Intensity", "snow_intensity", 0.0, 1.0, 0.0105),
            f("Accum./min", "snow_accumulation", 0.0, 10.0, 0.0105),
            f("Melt/min", "snow_melt", 0.0, 10.0, 0.0105),
            f("Max Cover", "snow_max_cover", 0.0, 1.0, 0.0105),
            i("Flakes", "snow_flakes", 0.0, 4000.0),
            i("Flake Size", "flake_texels", 1.0, 3.0),
            f("Big Flakes", "snow_big_flakes", 0.0, 1.0, 0.0105),
            f("Fall Speed", "snow_speed", 0.1005, 10.0, 0.0505),
            f("Sway", "snow_sway", 0.0, 3.0, 0.0105),
            cx("Flake Color", "snow_color"),
            i("Max Drops", "max_drops", 0.0, 20000.0),
            axle(f("Area X", "area", 1.0, 500.0, 0.5), 0),
            axle(f("Area Y", "area", 1.0, 200.0, 0.5), 1),
            axle(f("Area Z", "area", 1.0, 500.0, 0.5), 2),
            f("Rain Speed", "rain_speed", 1.0, 60.0, 0.1005),
            i("Streak", "streak_texels", 1.0, 12.0),
            e(
                "Rain Style",
                "rain_style",
                &[("Flipbook", 0), ("Particles", 1)],
            ),
            i("Rain Layers", "rain_layers", 1.0, 32.0),
            f("Layer Gap", "rain_layer_spacing", 0.2005, 20.0, 0.0505),
            f("Rain Texel", "rain_texel", 0.0105, 0.5, 0.0055),
            i("Splashes", "max_splashes", 0.0, 8000.0),
            f("Splash/s", "splash_rate", 0.1005, 10.0, 0.0505),
            i("Splash R.", "splash_radius", 1.0, 8.0),
            bx("Lightning", "lightning_enabled"),
            f("Flash", "flash_energy", 0.0, 8.0, 0.0105),
            cx("Rain", "rain_color"),
            cx("Bolt", "bolt_color"),
        ],
    },
    Section {
        sec: Sec::Water,
        label: "Water",
        glyph: "water",
        tooltip: "Water\nPixelWater: depth-banded color, foam and waves; raised pools and waterfalls\non hex / triangle terrains (Waterfalls page).",
        class: Some("PixelWater"),
        group_start: false,
        fields: &[
            axle(f("Size X", "size", 1.0, 4000.0, 0.5), 0),
            axle(f("Size Z", "size", 1.0, 4000.0, 0.5), 1),
            i("Subdiv.", "subdivisions", 0.0, 256.0),
            shader(f("Depth", "depth_scale", 0.1005, 32.0, 0.1005)),
            shader(i("Depth Bands", "depth_bands", 1.0, 8.0)),
            shader(f("Rim Dither", "rim_dither", 0.0, 1.0, 0.0105)),
            shader(f("Swell H.", "swell_height", 0.0, 2.0, 0.0055)),
            shader(f("Swell Len.", "swell_length", 1.0, 80.0, 0.1005)),
            shader(f("Steepness", "swell_steepness", 0.0, 1.0, 0.0105)),
            shader(f("Swell Speed", "swell_speed", 0.0, 4.0, 0.0105)),
            shader(f("Chop H.", "wave_height", 0.0, 1.0, 0.0055)),
            shader(f("Grazing", "grazing_strength", 0.0, 1.0, 0.0105)),
            shader(f("Reflection", "reflection_strength", 0.0, 1.0, 0.0105)),
            shader(i("Refl. Bands", "reflection_bands", 1.0, 6.0)),
            shader(f("Turq. Depth", "turquoise_depth", 0.0, 8.0, 0.0105)),
            shader(f("Sparkle", "sparkle", 0.0, 2.0, 0.0105)),
            f("Caustics", "caustics_strength", 0.0, 2.0, 0.0105),
            f("Caust. Scale", "caustics_scale", 0.0505, 4.0, 0.0105),
            i("Caust. Bands", "caustics_bands", 1.0, 6.0),
            f("Caust. Reach", "caustics_reach", 0.0, 4.0, 0.0105),
            shader(f("Foam Width", "foam_width", 0.0, 2.0, 0.0105)),
            shader(f("Foam Speed", "foam_speed", 0.0, 4.0, 0.0105)),
            shader(f("Surge/s", "foam_pulse_speed", 0.0, 4.0, 0.0105)),
            shader(f("Surge Pause", "foam_dead_time", 0.0, 0.9505, 0.0105)),
            shader(f("Surge", "foam_surge", 0.0, 6.0, 0.0105)),
            shader(i("Bands", "bands", 1.0, 8.0)),
            shader(cx("Shallow", "shallow_color")),
            shader(cx("Deep", "deep_color")),
            shader(cx("Turquoise", "turquoise_color")),
            shader(cx("Grazing", "grazing_color")),
            shader(cx("Sun Side", "sky_toward_sun")),
            shader(cx("Away Side", "sky_away_from_sun")),
            shader(cx("Foam", "foam_color")),
            i("Fall Levels", "waterfall_min_levels", 1.0, 64.0),
            f("Level H.", "waterfall_level_height", 0.0, 10.0, 0.0105),
            fall_shader(f("Fall Speed", "fall_speed", 0.0, 12.0, 0.0505)),
            fall_shader(f("Streaks/m", "streak_density", 0.1005, 16.0, 0.1005)),
            fall_shader(f("Streak Len.", "streak_length", 0.0505, 4.0, 0.0105)),
            fall_shader(f("Base Foam", "base_foam", 0.0, 1.0, 0.0105)),
            fall_shader(f("Spray Rate", "spray_rate", 0.0, 4.0, 0.0105)),
            fall_shader(f("Spray Rise", "spray_rise", 0.0, 6.0, 0.0105)),
            fall_shader(cx("Fall Water", "water_color")),
            fall_shader(cx("Fall Streak", "streak_color")),
            fall_shader(cx("Fall Foam", "foam_color")),
        ],
    },
    Section {
        sec: Sec::Fireflies,
        label: "Fireflies",
        glyph: "fireflies",
        tooltip: "Fireflies\nPixelFireflies: a blinking, wandering swarm that fades in at night.",
        class: Some("PixelFireflies"),
        group_start: false,
        fields: &[
            i("Count", "count", 0.0, 4096.0),
            axle(f("Area X", "extents", 0.1005, 500.0, 0.1005), 0),
            axle(f("Area Y", "extents", 0.1005, 100.0, 0.1005), 1),
            axle(f("Area Z", "extents", 0.1005, 500.0, 0.1005), 2),
            i("Seed", "seed", 0.0, 99999.0),
            cx("Color", "tint"),
            f("Size", "size", 0.0105, 2.0, 0.0105),
            f("Wander", "wander", 0.0, 4.0, 0.0105),
            f("Blink", "blink_speed", 0.0, 10.0, 0.0105),
            f("Glow", "energy", 0.0, 8.0, 0.0105),
            f("Night Only", "night_only", 0.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Particles,
        label: "Particle Field",
        glyph: "particles",
        tooltip: "Particle Field\nPixelParticleField: a prerendered, tiling and looping flipbook of pixel particles on\nworld-anchored layers (dust motes in the sunbeams, rain, snow, embers, leaves); stays put\nunder ortho and perspective camera moves.",
        class: Some("PixelParticleField"),
        group_start: false,
        fields: &[
            e(
                "Preset",
                "preset",
                &[
                    ("Dust Motes", 0),
                    ("Rain", 1),
                    ("Snow", 2),
                    ("Embers", 3),
                    ("Leaves", 4),
                ],
            ),
            bx("Emitting", "emitting"),
            f("Density", "density", 0.0, 1.0, 0.0105),
            e(
                "Weather",
                "weather_response",
                &[("Always", 0), ("Fair Weather", 1), ("Rain", 2), ("Snow", 3)],
            ),
            e("Projection", "projection", &[("Ground", 0), ("Upright", 1)]),
            i("Layers", "layers", 1.0, 64.0),
            f("Spacing", "spacing", 0.0505, 20.0, 0.0105),
            f("Texel", "texel_size", 0.0105, 1.0, 0.0055),
            f("Extent", "extent", 0.0, 500.0, 0.5),
            f("Height", "stature", 0.5, 200.0, 0.5),
            f("Band", "band", 0.0, 50.0, 0.1005),
            f("Fall", "fall_speed", -10.0, 30.0, 0.0105),
            axle(f("Wind X", "wind", -20.0, 20.0, 0.0105), 0),
            axle(f("Wind Y", "wind", -20.0, 20.0, 0.0105), 1),
            i("Tile", "tile_size", 16.0, 256.0),
            i("Frames", "frame_count", 4.0, 128.0),
            i("Count", "particle_count", 1.0, 2000.0),
            i("Lifetime", "lifetime", 1.0, 128.0),
            axle(f("Velocity X", "velocity", -16.0, 16.0, 0.0105), 0),
            axle(f("Velocity Y", "velocity", -16.0, 16.0, 0.0105), 1),
            f("Drift", "drift", 0.0, 4.0, 0.0105),
            f("Sway", "sway", 0.0, 8.0, 0.1005),
            i("Fade", "fade_frames", 0.0, 64.0),
            i("Streak", "streak_length", 0.0, 12.0),
            i("Seed", "seed", 0.0, 99999.0),
            f("FPS", "fps", 0.0, 60.0, 0.1005),
            cx("Color", "tint"),
            cx("Color B", "color_b"),
            f("Brightness", "brightness", 0.0, 8.0, 0.0105),
            f("Sunlit", "sunlit", 0.0, 1.0, 0.0105),
            f("Beam", "beam", 0.0, 1.0, 0.0105),
            f("Shade", "shade", 0.0, 1.0, 0.0105),
            f("Night", "night", -1.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Birds,
        label: "Birds",
        glyph: "birds",
        tooltip: "Birds\nPixelBirds: a boids flock of 2-frame flapping pixel birds that circle,\nland on high ground and scatter when the camera comes near.",
        class: Some("PixelBirds"),
        group_start: false,
        fields: &[
            i("Count", "count", 1.0, 256.0),
            f("Radius", "radius", 1.0, 200.0, 0.1005),
            f("Altitude", "altitude", 0.0, 100.0, 0.1005),
            f("Speed", "speed", 0.1005, 30.0, 0.1005),
            f("Neighbours", "neighbor_distance", 0.1005, 20.0, 0.0505),
            f("Separation", "separation", 0.0, 4.0, 0.0105),
            f("Alignment", "alignment", 0.0, 4.0, 0.0105),
            f("Cohesion", "cohesion", 0.0, 4.0, 0.0105),
            f("Steer", "steer_rate", 0.1005, 10.0, 0.0505),
            f("Flaps/s", "flap_speed", 0.0, 20.0, 0.1005),
            f("Size", "size", 0.0505, 4.0, 0.0105),
            f("Land Every", "land_interval", 1.0, 600.0, 0.5),
            f("Perch", "perch_seconds", 0.5, 600.0, 0.5),
            f("Scare Dist.", "scare_distance", 0.0, 100.0, 0.1005),
            bx("Landing", "landing"),
            bx("Outline", "outline"),
            bx("In Editor", "preview_in_editor"),
            cx("Color", "tint"),
        ],
    },
    Section {
        sec: Sec::Smoke,
        label: "Smoke & Fire",
        glyph: "smoke",
        tooltip: "Smoke & Fire\nPixelSmoke: tumbling outlined puffs drifting with the sky wind, optional flames\nwith a flickering light; burst(position, count) for build/destroy puffs.",
        class: Some("PixelSmoke"),
        group_start: false,
        fields: &[
            f("Rate", "rate", 0.0, 30.0, 0.0505),
            f("Lifetime", "lifetime", 0.2005, 30.0, 0.0505),
            f("Rise", "rise_speed", 0.0, 10.0, 0.0105),
            f("Spread", "spread", 0.0, 4.0, 0.0105),
            f("Start Size", "start_size", 0.0205, 4.0, 0.0105),
            f("End Size", "end_size", 0.0205, 8.0, 0.0105),
            f("Wind", "wind_strength", 0.0, 10.0, 0.0105),
            i("Max Puffs", "max_puffs", 1.0, 1024.0),
            i("Detail", "detail", 4.0, 16.0),
            f("Light", "light_energy", 0.0, 16.0, 0.0105),
            f("Light Range", "light_range", 0.1005, 50.0, 0.1005),
            f("Flicker", "flicker", 0.0, 1.0, 0.0105),
            bx("Emitting", "emitting"),
            bx("Sky Wind", "use_sky_wind"),
            bx("Outline", "outline"),
            bx("Fire", "fire"),
            cx("Smoke", "fumes_tint"),
            cx("Soot", "ash_tint"),
            cx("Flame", "fire_tint"),
        ],
    },
    Section {
        sec: Sec::Lights,
        label: "Light Volumes",
        glyph: "light_volume",
        tooltip: "Light Volumes\nPixelLightVolume: stepped point light with a dithered light pool and a small source orb.\nSelect one to edit it; the first in the scene is used otherwise.",
        class: Some("PixelLightVolume"),
        group_start: false,
        fields: &[
            f("Radius", "radius", 0.1005, 100.0, 0.1005),
            cx("Color", "lamp_tint"),
            f("Energy", "energy", 0.0, 16.0, 0.0105),
            f("Flicker", "flicker", 0.0, 1.0, 0.0105),
            bx("Shadows", "shadows"),
            bx("Glow", "glow_enabled"),
            f("Pool Scale", "pool_scale", 0.0505, 2.0, 0.0105),
            f("Pool Energy", "pool_energy", 0.0, 4.0, 0.0105),
            f("Glow Radius", "glow_radius", 0.0, 4.0, 0.0105),
            f("Glow Energy", "glow_energy", 0.0, 4.0, 0.0105),
            i("Glow Steps", "glow_steps", 1.0, 8.0),
            f("Night Boost", "night_boost", 0.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Object,
        label: "Pixel Object",
        glyph: "object",
        tooltip: "Pixel Object\nPixelObject: per-object pixel size, outline id/color, palette and dither\n(-1 / Inherit = parent PixelObject, then the renderer's Object Defaults).",
        class: Some("PixelObject"),
        group_start: true,
        fields: &[
            ig("Pixel Size", "pixel_size", -1.0, 16.0),
            e("Outline", "outline", INHERIT_TRI),
            i("Outline Id", "outline_id", -1.0, 253.0),
            e("Palette", "palette", INHERIT_TRI),
            e("Dither", "dither", INHERIT_TRI),
            bx("Own Color", "supersede_contour_tint"),
            cx("Outline Col.", "contour_tint"),
            bx("Texel Snap", "snap_to_texel"),
            bx("Convert", "convert_materials"),
            i("Bands", "bands", 1.0, 8.0),
            f("Shade Floor", "shade_floor", 0.0, 1.0, 0.0105),
            f("Rim", "rim_strength", 0.0, 1.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::Debug,
        label: "Debug Views",
        glyph: "debug",
        tooltip: "Debug Views\nShow the renderer's intermediate buffers (normals, depth, pixel size, outline flags),\nwhy each outline is drawn (red silhouette, blue id edge, yellow crease, green suppressed\nfold / ledge) and the god-ray sheet stack.",
        class: Some(RENDERER),
        group_start: false,
        fields: &[e(
            "View",
            "debug_view",
            &[
                ("Final", 0),
                ("Normals", 1),
                ("Depth", 2),
                ("Pixel Size", 3),
                ("Outline Ids", 4),
                ("Raw Color", 5),
                ("Outline Cause", 6),
                ("Shafts", 7),
            ],
        )],
    },
    Section {
        sec: Sec::GridFog,
        label: "Grid / Fog",
        glyph: "grid",
        tooltip: "Grid / Fog\nMarchingSquaresTerrain overlays: cell grid lines (editor / in game), fog of war\nlook and terrain debug views (slope, contours, slot ids, chunk borders, cell ids).",
        class: Some("MarchingSquaresTerrain"),
        group_start: true,
        fields: &[
            bx("Game Grid", "show_grid"),
            bx("Editor Grid", "show_grid_in_editor"),
            cx("Grid Color", "grid_color"),
            f("Grid Width", "grid_width", 0.0055, 2.0, 0.0055),
            f("Grid Fade", "grid_fade_distance", 0.0, 2000.0, 1.0),
            bx("Chunk Lines", "grid_chunk_lines"),
            bx("Game Fog", "fog_enabled"),
            bx("Editor Fog", "fog_in_editor"),
            f("Fog Fade s", "fog_transition", 0.0, 5.0, 0.0105),
            cx("Fog Color", "fog_color"),
            cx("Fog Dither", "fog_color_alt"),
            f("Hidden Dim", "fog_hidden_dim", 0.0, 1.0, 0.0105),
            f("Hatch", "fog_hatch", 0.0, 1.0, 0.0105),
            f("Softness", "fog_softness", 0.0, 0.5, 0.0105),
            e(
                "Debug View",
                "debug_view",
                &[
                    ("Off", 0),
                    ("Slope", 1),
                    ("Height Contours", 2),
                    ("Slot Ids", 3),
                    ("Chunk Borders", 4),
                    ("Cell Ids", 5),
                    ("Cell Data", 6),
                ],
            ),
        ],
    },
    Section {
        sec: Sec::Erosion,
        label: "Erosion",
        glyph: EROSION_ICON,
        tooltip: "Erosion\nMarchingSquaresErosion: hydraulic droplet erosion of the parent terrain.\nErode runs one batch of droplets (one undo step); each press continues with a new seed.",
        class: Some(EROSION),
        group_start: false,
        fields: &[
            perform("Erode", "erode"),
            progress("Eroding", "get_progress", "abort"),
            i("Droplets", "iterations", 100.0, 500000.0),
            i("Seed", "seed", -2147483648.0, 2147483647.0),
            e("Mode", "mode", &[("Auto", 0), ("Raw", 1), ("Terraced", 2)]),
            f("Level Step", "level_step", 0.0, 16.0, 0.0505),
            i("Min Plateau", "min_plateau_cells", 0.0, 64.0),
            f("Height Scale", "height_scale", 0.0, 256.0, 0.1005),
            i("Px / Cell", "pixels_per_cell", 1.0, 8.0),
            i("Radius", "splash_radius", 2.0, 8.0),
            f("Momentum", "momentum", 0.0, 1.0, 0.0015),
            f("Load", "load_limit", 0.0, 32.0, 0.0105),
            f("Min Load", "min_load", 0.0, 1.0, 0.0015),
            f("Carve", "carve_rate", 0.0, 1.0, 0.0105),
            f("Drop", "drop_rate", 0.0, 1.0, 0.0105),
            f("Dry", "dry_rate", 0.0, 1.0, 0.0015),
            f("Gravity", "gravity", 0.0, 32.0, 0.1005),
            i("Steps", "droplet_steps", 1.0, 256.0),
            f("Volume", "start_volume", 0.0, 8.0, 0.0105),
            f("Speed", "start_momentum", 0.0, 8.0, 0.0105),
        ],
    },
    Section {
        sec: Sec::WildFlowers,
        label: "Wild Flowers",
        glyph: WILD_FLOWERS_ICON,
        tooltip: "Wild Flowers\nMarchingSquaresWildFlowers: clustered wild flowers on the parent terrain's\nscatter layer (8 species + bilberry, from cell noise and ground water; none under water).\nScatter replaces the previous flowers, Clear removes them (undoable).",
        class: Some(WILD_FLOWERS),
        group_start: false,
        fields: &[
            perform("Scatter", "scatter_flowers"),
            perform("Clear", "clear_flowers"),
            i("Seed", "seed", -2147483648.0, 2147483647.0),
            f("Density", "density", 0.0, 2.0, 0.0105),
            f("Patch Scale", "patch_scale", 0.0505, 8.0, 0.0105),
            f("Threshold", "bloom_threshold", 0.0, 1.0, 0.0105),
            f("Max Slope", "max_slope", 0.0, 1.0, 0.0055),
            i("Points/Cell", "points_per_cell", 2.0, 32.0),
            f("Jitter", "jitter", 0.0, 1.0, 0.0105),
            f("Ground Water", "ground_water", 0.0, 1.0, 0.0105),
            f("Variation", "ground_water_variation", 0.0, 1.0, 0.0105),
            f("Shore Wet", "shore_wetness", 0.0, 1.0, 0.0105),
            f("Water Level", "water_level", -256.0, 256.0, 0.1005),
            f("Shore Dist.", "shore_distance", 0.0105, 64.0, 0.0105),
            bx("Trees Shade", "trees_shade"),
        ],
    },
    super::fire_section::FIRE_SECTION,
];

/// Icon name (in `ICON_DIR`) for a node class.
pub fn icon_for(class: &str) -> &'static str {
    match class {
        "PixelDiagnostics" => "debug",
        "PixelPalette" => "palette",
        "PixelSunShafts" => "god_rays",
        c => SECTIONS
            .iter()
            .find(|s| s.class == Some(c))
            .map(|s| s.glyph)
            .unwrap_or("debug"),
    }
}

pub fn slice_ui(sec: Sec) -> &'static Section {
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
    "PixelPalette",
    EROSION,
    WILD_FLOWERS,
    FIRE,
];

/// Nodes listed in the "Pixel World" panel / menu: (class, label, unique per scene).
pub const ADDABLE: &[(&str, &str, bool)] = &[
    (RENDERER, "Renderer", true),
    ("PixelSky", "Sky & Clouds", true),
    ("PixelDayNight", "Day / Night", true),
    ("PixelWeather", "Weather", true),
    ("PixelWater", "Water", true),
    ("PixelFireflies", "Fireflies", false),
    ("PixelParticleField", "Particle Field", false),
    ("PixelBirds", "Birds", false),
    ("PixelSmoke", "Smoke & Fire", false),
    ("PixelLightVolume", "Light Volume", false),
    ("PixelSunShafts", "Sun Shafts", false),
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
    let s = slice_ui(sec);
    match s.class {
        Some(c) => SECTIONS
            .iter()
            .find(|x| x.class == Some(c))
            .map(|x| x.sec)
            .unwrap_or(sec),
        None => sec,
    }
}

/// Strip tabs of the node edited by `sec`: every page of every section of that class, in
/// toolbar order (e.g. the renderer: Renderer, Outlines, Color, god-ray pages, Debug).
pub fn class_tabs(sec: Sec) -> Vec<Tab> {
    let class = slice_ui(sec).class;
    let mut tabs = Vec::new();
    for s in SECTIONS.iter().filter(|s| {
        if class.is_some() {
            s.class == class
        } else {
            s.sec == sec
        }
    }) {
        let p = pages(s.sec);
        if p.is_empty() {
            tabs.push(Tab {
                label: s.label,
                tooltip: s.tooltip,
                sec: s.sec,
                page: 0,
            });
        } else {
            for (k, pg) in p.iter().enumerate() {
                tabs.push(Tab {
                    label: pg.0,
                    tooltip: pg.1,
                    sec: s.sec,
                    page: k,
                });
            }
        }
    }
    tabs
}

/// The strip label of `class.prop` (node properties only; vector axes drop their " X" / " Y" /
/// " Z" suffix). The inspector shows the same label (`inspector_labels.rs`), so every property
/// has one name in both places.
pub fn strip_label(class: &str, prop: &str) -> Option<&'static str> {
    SECTIONS
        .iter()
        .filter(|s| s.class == Some(class))
        .flat_map(|s| s.fields.iter())
        .find_map(|fd| {
            if fd.prop != prop || matches!(fd.sort, Kind::Action(_) | Kind::Progress { .. }) {
                return None;
            }
            match fd.store {
                Store::Prop => Some(fd.label),
                Store::Axis(_) => Some(
                    fd.label
                        .trim_end_matches(" X")
                        .trim_end_matches(" Y")
                        .trim_end_matches(" Z")
                        .trim_end_matches('.'),
                ),
                Store::Shader(_) => None,
            }
        })
}

/// Classes with at least one strip field (inspector label plugin).
pub fn has_strip_fields(class: &str) -> bool {
    SECTIONS.iter().any(|s| {
        s.class == Some(class)
            && s.fields
                .iter()
                .any(|fd| matches!(fd.store, Store::Prop | Store::Axis(_)))
    })
}

/// Extra explanation shown in a strip control's tooltip (fields whose label alone says little).
pub fn field_hint(prop: &str) -> Option<&'static str> {
    Some(match prop {
        "eye_adaptation" => {
            "Eye adaptation of the light-sheet stack (0 = off: raw stack).\nA view across the beams passes many sheets and would turn into a flat, milky veil;\nthe stack is scaled by min(1, Adaptation / potential stack), so views along the sun\nare unchanged and side-on views keep distinct shafts. Default 1.6."
        }
        _ => return None,
    })
}
