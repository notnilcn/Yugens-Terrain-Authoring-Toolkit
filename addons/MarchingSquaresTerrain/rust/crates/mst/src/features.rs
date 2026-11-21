use crate::editor_detection::is_editor;
use godot::classes::scene_tree::GroupCallFlags;
use godot::classes::{Engine, ProjectSettings, SceneTree};
use godot::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
pub const GROUP: &str = "marching_squares_terrain_features";
pub const CALLBACK: &str = "_on_pixel_features_changed";
const PREFIX: &str = "marching_squares_terrain/features/";
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    Clouds,
    GodRays,
    Outlines,
    OutlineIds,
    Water,
    Weather,
    Birds,
    Fireflies,
    Smoke,
    Fire,
    Particles,
    LightVolumes,
    SunShafts,
    Grass,
}
pub struct Info {
    pub feature: Feature,
    pub token: &'static str,
    pub label: &'static str,
    pub default: bool,
    pub hint: &'static str,
}
const fn facts(
    feature: Feature,
    key: &'static str,
    label: &'static str,
    default: bool,
    hint: &'static str,
) -> Info {
    Info {
        feature,
        token: key,
        label,
        default,
        hint,
    }
}
pub const ALL: &[Info] = &[
    facts(
        Feature::Clouds,
        "clouds",
        "Clouds",
        false,
        "PixelSky cloud shadows and cloud deck (cloud noise in every ground, prop and water shader).",
    ),
    facts(
        Feature::GodRays,
        "god_rays",
        "God Rays",
        false,
        "PixelRenderer light-sheet god rays (a ray march per low-res pixel).",
    ),
    facts(
        Feature::Outlines,
        "outlines",
        "Outlines",
        true,
        "PixelRenderer silhouettes and crease highlights.",
    ),
    facts(
        Feature::OutlineIds,
        "outline_ids",
        "Outline Ids",
        true,
        "PixelRenderer id pass: outline ids above 7, hover / selection highlights, per-object palette and dither switches (one extra low-res scene pass).",
    ),
    facts(
        Feature::Water,
        "water",
        "Water",
        false,
        "PixelWater surfaces, waterfalls and caustics on the ground.",
    ),
    facts(
        Feature::Weather,
        "weather",
        "Weather",
        false,
        "PixelWeather rain, snow, lightning, wetness and wind.",
    ),
    facts(
        Feature::Birds,
        "birds",
        "Birds",
        false,
        "PixelBirds flocks (boids simulation).",
    ),
    facts(
        Feature::Fireflies,
        "fireflies",
        "Fireflies",
        false,
        "PixelFireflies glow swarms.",
    ),
    facts(
        Feature::Smoke,
        "smoke",
        "Smoke",
        false,
        "PixelSmoke plumes (simulated puffs).",
    ),
    facts(
        Feature::Fire,
        "fire",
        "Fire",
        false,
        "PixelFireSystem spreading grass fire (simulation, flames, glow).",
    ),
    facts(
        Feature::Particles,
        "particles",
        "Particle Fields",
        false,
        "PixelParticleField flipbook layers (dust, pollen, falling leaves).",
    ),
    facts(
        Feature::LightVolumes,
        "light_volumes",
        "Light Volumes",
        false,
        "PixelLightVolume dithered light pools and source orbs.",
    ),
    facts(
        Feature::SunShafts,
        "sun_shafts",
        "Sun Shafts",
        false,
        "PixelSunShafts shadow-cut light sheets (a few large transparent quads lit through the sun's shadow map).",
    ),
    facts(
        Feature::Grass,
        "grass",
        "Grass",
        true,
        "MarchingSquaresTerrain grass blades.",
    ),
];
fn ordinal(f: Feature) -> usize {
    ALL.iter().position(|i| i.feature == f).unwrap_or(0)
}
pub fn info_of(f: Feature) -> &'static Info {
    &ALL[ordinal(f)]
}
pub fn for_category(class: &str) -> Option<Feature> {
    Some(match class {
        "PixelSky" => Feature::Clouds,
        "PixelWater" => Feature::Water,
        "PixelWeather" => Feature::Weather,
        "PixelBirds" => Feature::Birds,
        "PixelFireflies" => Feature::Fireflies,
        "PixelSmoke" => Feature::Smoke,
        "PixelFireSystem" => Feature::Fire,
        "PixelParticleField" => Feature::Particles,
        "PixelLightVolume" => Feature::LightVolumes,
        "PixelSunShafts" => Feature::SunShafts,
        _ => return None,
    })
}
pub fn by_key(key: &str) -> Option<&'static Info> {
    ALL.iter().find(|i| i.token == key)
}
fn setting(i: &Info) -> String {
    format!("{PREFIX}{}", i.token)
}
static MASK: AtomicU32 = AtomicU32::new(0);
static LOADED: AtomicBool = AtomicBool::new(false);
fn read_mask() -> u32 {
    let ps = ProjectSettings::singleton();
    ALL.iter().enumerate().fold(0, |m, (b, i)| {
        let key = setting(i);
        let on = if ps.has_setting(&key) {
            ps.get_setting(&key).try_to::<bool>().unwrap_or(i.default)
        } else {
            i.default
        };
        if on { m | (1 << b) } else { m }
    })
}
fn mask() -> u32 {
    if !LOADED.swap(true, Ordering::Relaxed) {
        MASK.store(read_mask(), Ordering::Relaxed);
    }
    MASK.load(Ordering::Relaxed)
}
pub fn active(f: Feature) -> bool {
    mask() & (1 << ordinal(f)) != 0
}
pub fn register_settings() {
    let mut ps = ProjectSettings::singleton();
    for i in ALL {
        let key = setting(i);
        if !ps.has_setting(&key) {
            ps.set_setting(&key, &i.default.to_variant());
        }
        ps.set_initial_value(&key, &i.default.to_variant());
        let mut d = VarDictionary::new();
        d.set("name", key.as_str());
        d.set("type", VariantType::BOOL.ord());
        d.set("hint_string", i.hint);
        ps.add_property_info(&d);
        ps.set_as_basic(&key, true);
    }
    LOADED.store(false, Ordering::Relaxed);
}
pub fn assign_active(f: Feature, on: bool) {
    let i = info_of(f);
    let mut ps = ProjectSettings::singleton();
    ps.set_setting(&setting(i), &on.to_variant());
    if is_editor() {
        ps.save();
    }
    reingest();
}
pub fn reingest() {
    let old = MASK.load(Ordering::Relaxed);
    let new = read_mask();
    MASK.store(new, Ordering::Relaxed);
    LOADED.store(true, Ordering::Relaxed);
    if old != new {
        notify();
    }
}
pub fn notify() {
    let Some(tree) = Engine::singleton()
        .get_main_loop()
        .and_then(|m| m.try_cast::<SceneTree>().ok())
    else {
        return;
    };
    let mut tree = tree;
    tree.call_group_flags(GroupCallFlags::DEFERRED, GROUP, CALLBACK, &[]);
}
pub fn observe(node: &Gd<impl Inherits<Node>>) {
    let mut n = node.clone().upcast::<Node>();
    if !n.is_in_group(GROUP) {
        n.add_to_group(GROUP);
    }
}
pub fn warning(f: Feature) -> Option<String> {
    if active(f) {
        return None;
    }
    let i = info_of(f);
    Some(format!(
        "Turn \"{}\" on in the 3D toolbar under Pixel World > Features (or Project Settings > Marching Squares Terrain > Features): it is off for this project, so this node draws nothing.",
        i.label
    ))
}
#[derive(GodotClass)]
#[class(no_init, base = Object)]
pub struct PixelFeatures {
    base: Base<Object>,
}
#[godot_api]
impl PixelFeatures {
    #[func]
    fn is_enabled(key: GString) -> bool {
        by_key(&key.to_string()).is_some_and(|i| active(i.feature))
    }
    #[func]
    fn assign_active(key: GString, on: bool) {
        match by_key(&key.to_string()) {
            Some(i) => assign_active(i.feature, on),
            None => {
                godot_error!("PixelFeatures: unknown feature \"{key}\" (see get_feature_keys())")
            }
        }
    }
    #[func]
    fn get_feature_keys() -> PackedStringArray {
        ALL.iter().map(|i| GString::from(i.token)).collect()
    }
    #[func]
    fn get_feature_label(key: GString) -> GString {
        by_key(&key.to_string())
            .map(|i| GString::from(i.label))
            .unwrap_or_default()
    }
    #[func]
    fn get_time() -> f32 {
        crate::clock::now()
    }
    #[func]
    fn is_animating() -> bool {
        crate::clock::running()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_table_is_consistent() {
        assert!(ALL.len() <= 32, "the enabled mask is a u32");
        for (i, a) in ALL.iter().enumerate() {
            assert_eq!(ordinal(a.feature), i, "{} listed twice", a.token);
            assert!(
                ALL.iter().filter(|b| b.token == a.token).count() == 1,
                "duplicate key {}",
                a.token
            );
            assert!(
                a.token.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "key {} is not snake_case",
                a.token
            );
        }
        for class in [
            "PixelSky",
            "PixelWater",
            "PixelWeather",
            "PixelBirds",
            "PixelFireflies",
            "PixelSmoke",
            "PixelFireSystem",
            "PixelParticleField",
            "PixelLightVolume",
        ] {
            let f = for_category(class).expect(class);
            assert!(!info_of(f).default, "{class}: expensive effects are opt-in");
        }
        assert!(for_category("PixelRenderer").is_none());
    }
}
