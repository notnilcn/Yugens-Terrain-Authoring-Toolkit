//! Project-wide feature toggles: opt in to the expensive parts of the addon.
//!
//! Each toggle is a project setting (`marching_squares_terrain/features/<key>`, listed in
//! Project Settings and in the 3D toolbar's *Pixel World > Features* menu). A feature that is
//! off behaves as if its nodes were not in the scene: they build nothing, process nothing and
//! publish neutral shader globals, but keep their saved settings, so turning the feature back
//! on restores them. Resource-intensive effects are off by default; the core look (outlines,
//! outline ids, grass) is on.
//!
//! Gated nodes join [`GROUP`] and implement [`CALLBACK`] (`_on_pixel_features_changed`), which
//! is called (deferred) whenever a toggle changes, from the editor menu, from Project Settings
//! or from a script through the `PixelFeatures` class. Only the node knows how to switch its
//! own feature off; this module only stores the toggles and tells the nodes.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use godot::classes::{Engine, ProjectSettings, SceneTree};
use godot::classes::scene_tree::GroupCallFlags;
use godot::prelude::*;

use crate::util::is_editor;

/// Group of the nodes gated by a feature.
pub const GROUP: &str = "marching_squares_terrain_features";
/// Method every node in [`GROUP`] implements.
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
    Grass,
}

pub struct Info {
    pub feature: Feature,
    /// Setting key under `marching_squares_terrain/features/` (also the script name).
    pub key: &'static str,
    pub label: &'static str,
    pub default: bool,
    /// What the feature draws and what switching it off saves.
    pub hint: &'static str,
}

const fn info(feature: Feature, key: &'static str, label: &'static str, default: bool, hint: &'static str) -> Info {
    Info { feature, key, label, default, hint }
}

/// Every feature, in menu order. The index is the feature's bit in the enabled mask.
pub const ALL: &[Info] = &[
    info(Feature::Clouds, "clouds", "Clouds", false, "PixelSky cloud shadows and cloud deck (cloud noise in every ground, prop and water shader)."),
    info(Feature::GodRays, "god_rays", "God Rays", false, "PixelRenderer light-sheet god rays (a ray march per low-res pixel)."),
    info(Feature::Outlines, "outlines", "Outlines", true, "PixelRenderer silhouettes and crease highlights."),
    info(Feature::OutlineIds, "outline_ids", "Outline Ids", true, "PixelRenderer id pass: outline ids above 7, hover / selection highlights, per-object palette and dither switches (one extra low-res scene pass)."),
    info(Feature::Water, "water", "Water", false, "PixelWater surfaces, waterfalls and caustics on the ground."),
    info(Feature::Weather, "weather", "Weather", false, "PixelWeather rain, snow, lightning, wetness and wind."),
    info(Feature::Birds, "birds", "Birds", false, "PixelBirds flocks (boids simulation)."),
    info(Feature::Fireflies, "fireflies", "Fireflies", false, "PixelFireflies glow swarms."),
    info(Feature::Smoke, "smoke", "Smoke", false, "PixelSmoke plumes (simulated puffs)."),
    info(Feature::Fire, "fire", "Fire", false, "PixelFireSystem spreading grass fire (simulation, flames, glow)."),
    info(Feature::Particles, "particles", "Particle Fields", false, "PixelParticleField flipbook layers (dust, pollen, falling leaves)."),
    info(Feature::LightVolumes, "light_volumes", "Light Volumes", false, "PixelLightVolume dithered light pools and halos."),
    info(Feature::Grass, "grass", "Grass", true, "MarchingSquaresTerrain grass blades."),
];

fn index(f: Feature) -> usize {
    ALL.iter().position(|i| i.feature == f).unwrap_or(0)
}

pub fn info_of(f: Feature) -> &'static Info {
    &ALL[index(f)]
}

/// The feature that gates nodes of `class` (the Pixel World menu turns it on when such a node
/// is added).
pub fn for_class(class: &str) -> Option<Feature> {
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
        _ => return None,
    })
}

pub fn by_key(key: &str) -> Option<&'static Info> {
    ALL.iter().find(|i| i.key == key)
}

fn setting(i: &Info) -> String {
    format!("{PREFIX}{}", i.key)
}

static MASK: AtomicU32 = AtomicU32::new(0);
static LOADED: AtomicBool = AtomicBool::new(false);

fn read_mask() -> u32 {
    let ps = ProjectSettings::singleton();
    ALL.iter().enumerate().fold(0, |m, (b, i)| {
        let key = setting(i);
        let on = if ps.has_setting(&key) { ps.get_setting(&key).try_to::<bool>().unwrap_or(i.default) } else { i.default };
        if on { m | (1 << b) } else { m }
    })
}

fn mask() -> u32 {
    if !LOADED.swap(true, Ordering::Relaxed) {
        MASK.store(read_mask(), Ordering::Relaxed);
    }
    MASK.load(Ordering::Relaxed)
}

/// Whether `f` is on for this project.
pub fn enabled(f: Feature) -> bool {
    mask() & (1 << index(f)) != 0
}

/// Registers the settings (with their defaults, so Project Settings lists them and only
/// changed values are written to project.godot). Called once at extension init.
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

/// Turns `f` on or off and tells the gated nodes. In the editor the change is saved to
/// project.godot; at runtime it lasts for the session (a game's options menu can call it).
pub fn set_enabled(f: Feature, on: bool) {
    let i = info_of(f);
    let mut ps = ProjectSettings::singleton();
    ps.set_setting(&setting(i), &on.to_variant());
    if is_editor() {
        ps.save();
    }
    reload();
}

/// Re-reads the settings (after Project Settings was edited) and notifies on a change.
pub fn reload() {
    let old = MASK.load(Ordering::Relaxed);
    let new = read_mask();
    MASK.store(new, Ordering::Relaxed);
    LOADED.store(true, Ordering::Relaxed);
    if old != new {
        notify();
    }
}

/// Calls [`CALLBACK`] on every gated node (deferred: the caller may be a bound node).
pub fn notify() {
    let Some(tree) = Engine::singleton().get_main_loop().and_then(|m| m.try_cast::<SceneTree>().ok()) else { return };
    let mut tree = tree;
    tree.call_group_flags(GroupCallFlags::DEFERRED, GROUP, CALLBACK, &[]);
}

/// Adds a gated node to [`GROUP`]; call from `ready`.
pub fn watch(node: &Gd<impl Inherits<Node>>) {
    let mut n = node.clone().upcast::<Node>();
    if !n.is_in_group(GROUP) {
        n.add_to_group(GROUP);
    }
}

/// Configuration warning for a node whose feature is off (empty when it is on).
pub fn warning(f: Feature) -> Option<String> {
    if enabled(f) {
        return None;
    }
    let i = info_of(f);
    Some(format!(
        "\"{}\" is off for this project, so this node draws nothing. Turn it on in the 3D toolbar under Pixel World > Features (or Project Settings > Marching Squares Terrain > Features).",
        i.label
    ))
}

/// Script access to the feature toggles (static methods only), e.g. for a game's graphics
/// options: `PixelFeatures.set_enabled("god_rays", false)`.
#[derive(GodotClass)]
#[class(no_init, base = Object)]
pub struct PixelFeatures {
    base: Base<Object>,
}

#[godot_api]
impl PixelFeatures {
    /// Whether the feature `key` (see `get_feature_keys`) is on.
    #[func]
    fn is_enabled(key: GString) -> bool {
        by_key(&key.to_string()).is_some_and(|i| enabled(i.feature))
    }

    /// Turns the feature `key` on or off. In a running game this lasts for the session.
    #[func]
    fn set_enabled(key: GString, on: bool) {
        match by_key(&key.to_string()) {
            Some(i) => set_enabled(i.feature, on),
            None => godot_error!("PixelFeatures: unknown feature \"{key}\" (see get_feature_keys())"),
        }
    }

    /// Keys of every feature, in menu order.
    #[func]
    fn get_feature_keys() -> PackedStringArray {
        ALL.iter().map(|i| GString::from(i.key)).collect()
    }

    /// Display name of the feature `key`.
    #[func]
    fn get_feature_label(key: GString) -> GString {
        by_key(&key.to_string()).map(|i| GString::from(i.label)).unwrap_or_default()
    }

    /// Seconds on the shared effect clock (`pt_time` in the shaders; wraps every hour).
    #[func]
    fn get_time() -> f32 {
        crate::clock::now()
    }

    /// True when the editor animates effects (always true in a running game).
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
            assert_eq!(index(a.feature), i, "{} listed twice", a.key);
            assert!(ALL.iter().filter(|b| b.key == a.key).count() == 1, "duplicate key {}", a.key);
            assert!(a.key.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "key {} is not snake_case", a.key);
        }
        for class in ["PixelSky", "PixelWater", "PixelWeather", "PixelBirds", "PixelFireflies", "PixelSmoke", "PixelFireSystem", "PixelParticleField", "PixelLightVolume"] {
            let f = for_class(class).expect(class);
            assert!(!info_of(f).default, "{class}: expensive effects are opt-in");
        }
        assert!(for_class("PixelRenderer").is_none());
    }
}
