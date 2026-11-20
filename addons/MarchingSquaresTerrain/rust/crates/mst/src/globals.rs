//! Global shader parameters shared by every Marching Squares Terrain shader, plus the metadata layer.

use godot::classes::RenderingServer;
use godot::classes::rendering_server::GlobalShaderParameterType as T;
use godot::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Render layer (1-based) seen only by the metadata camera. Pixelized geometry is put on it.
pub const DEFAULT_META_LAYER: u32 = 20;
static META_LAYER: AtomicU32 = AtomicU32::new(DEFAULT_META_LAYER);

pub fn meta_layer() -> u32 {
    META_LAYER.load(Ordering::Relaxed)
}

pub fn meta_mask() -> u32 {
    1u32 << (meta_layer().clamp(1, 20) - 1)
}

/// Cull mask of the id camera: the meta layer plus an otherwise unused high bit
/// (`pt_core::meta::ID_CAMERA_BIT`). Shaders compare it exactly (`PT_IS_ID`).
pub fn id_mask() -> u32 {
    meta_mask() | (1u32 << pt_core::meta::ID_CAMERA_BIT)
}

pub fn set_meta_layer(layer: u32) {
    META_LAYER.store(layer.clamp(1, 20), Ordering::Relaxed);
    assign("pt_meta_mask", &meta_mask().to_variant());
    assign("pt_id_mask", &id_mask().to_variant());
}

/// Group every node that needs to re-apply its layers/flags when the meta layer changes.
pub const REFRESH_GROUP: &str = "marching_squares_terrain_refresh";

fn list() -> Vec<(&'static str, T, Variant)> {
    vec![
        (
            "pt_meta_mask",
            T::UINT,
            (1i64 << (DEFAULT_META_LAYER - 1)).to_variant(),
        ),
        (
            "pt_id_mask",
            T::UINT,
            ((1i64 << (DEFAULT_META_LAYER - 1)) | (1i64 << pt_core::meta::ID_CAMERA_BIT))
                .to_variant(),
        ),
        ("pt_meta_depth_range", T::FLOAT, 200.0f32.to_variant()),
        ("pt_texel_world", T::FLOAT, 0.0f32.to_variant()),
        (
            "pt_sun_dir",
            T::VEC3,
            Vector3::new(0.3005, 0.8005, 0.4005).to_variant(),
        ),
        (
            "pt_cloud_a",
            T::VEC4,
            Vector4::new(0.4505, 0.1205, 0.0205, 0.4505).to_variant(),
        ),
        (
            "pt_cloud_b",
            T::VEC4,
            Vector4::new(0.0, 0.0, 40.0, 0.0).to_variant(),
        ),
        ("pt_cloud_c", T::VEC4, Vector4::ZERO.to_variant()),
        ("pt_night", T::FLOAT, 0.0f32.to_variant()),
        // PixelDayNight: sun light 0..1 relative to daylight (fades god rays through dusk).
        ("pt_sun_light", T::FLOAT, 1.0f32.to_variant()),
        // PixelDayNight: 0 with the sun high .. 1 at sunset / blue hour (softens cloud shadows).
        ("pt_sun_low", T::FLOAT, 0.0f32.to_variant()),
        // MarchingSquaresTerrain distant LOD: (focus x, focus z, fade end distance, band; band 0
        // = off). Scatter props and grass dither out before their chunk turns low detail.
        ("pt_lod_fade", T::VEC4, Vector4::ZERO.to_variant()),
        // The camera end (x, z) of the full-detail segment that starts at the focus.
        ("pt_lod_eye", T::VEC2, Vector2::ZERO.to_variant()),
        ("pt_terrain_height", T::SAMPLER2D, Variant::nil()),
        ("pt_terrain_rect", T::VEC4, Vector4::ZERO.to_variant()),
        // Per-cell data (fog of war, tint, highlight) of the terrain using it; see
        // mst_cells.gdshaderinc and pt_core::celldata.
        ("pt_cell_data", T::SAMPLER2D, Variant::nil()),
        ("pt_cell_rect", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_cell_grid",
            T::VEC4,
            Vector4::new(0.0, 2.0, 2.0, 0.0).to_variant(),
        ),
        (
            "pt_cell_xform",
            T::VEC4,
            Vector4::new(0.0, 0.0, 1.0, 1.0).to_variant(),
        ),
        // Fog of war for water / props / objects: (mode: 2 = fog on, hidden dim, hidden
        // saturation, hatch) and the fog colour (mst_cells.gdshaderinc `pt_fog_surface`).
        ("pt_cell_fog", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_cell_fog_color",
            T::COLOR,
            Color::from_rgb(0.0505, 0.0605, 0.0905).to_variant(),
        ),
        // PixelWeather: ground wetness / snow cover 0..1 (pixel_weather.gdshaderinc).
        ("pt_wet", T::FLOAT, 0.0f32.to_variant()),
        ("pt_snow", T::FLOAT, 0.0f32.to_variant()),
        // PixelWater: (enabled, water level, time, 0) and caustics (strength, scale, bands, reach).
        ("pt_water_a", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_water_b",
            T::VEC4,
            Vector4::new(0.0, 0.6005, 3.0, 0.6005).to_variant(),
        ),
        // PixelDayNight: the background colour at the horizon (linear rgb, a = 1 while published);
        // PixelWater fades its far sea into it (water.gdshader `horizon_*`).
        ("pt_horizon", T::VEC4, Vector4::ZERO.to_variant()),
        // Cel lighting (PixelDayNight): (ramp on, wrap, rim width, rim back bias), ramp, rim.
        (
            "pt_light",
            T::VEC4,
            Vector4::new(0.0, 0.0, 0.3505, 0.6005).to_variant(),
        ),
        ("pt_light_ramp", T::SAMPLER2D, Variant::nil()),
        ("pt_rim", T::VEC4, Vector4::ZERO.to_variant()),
        // PixelWeather: rain intensity 0..1 and the wind (dir x, dir z, strength m/s, gustiness)
        // for pixel_wind.gdshaderinc (pixel_weather.gdshaderinc reads pt_rain).
        ("pt_rain", T::FLOAT, 0.0f32.to_variant()),
        (
            "pt_wind",
            T::VEC4,
            Vector4::new(0.8945, 0.4475, 1.0, 0.2505).to_variant(),
        ),
        // PixelRenderer god rays: the sheets' cloud mask for other effects (dust in the shafts,
        // particle_field.gdshader): (enabled, cloud threshold, softness, gap erosion) and
        // (cloud shadow strength, sun follow, 0, 0).
        ("pt_rays_a", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_rays_b",
            T::VEC4,
            Vector4::new(0.9005, 1.0, 0.0, 0.0).to_variant(),
        ),
        // PixelFireSystem: fire state texture, its world rect and (enabled, texel, w, h)
        // (pixel_fire.gdshaderinc, pt_core::fire).
        ("pt_fire_tex", T::SAMPLER2D, Variant::nil()),
        ("pt_fire_rect", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_fire_a",
            T::VEC4,
            Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant(),
        ),
        // Shared effect clock (clock.rs): every addon shader animates with it instead of TIME.
        ("pt_time", T::FLOAT, 0.0f32.to_variant()),
    ]
}

static REGISTERED: AtomicBool = AtomicBool::new(false);

/// Registers any missing global shader parameter (idempotent). The parameter list can only
/// be queried in the editor, so outside it registration happens exactly once per process.
pub fn ensure() {
    let editor = godot::classes::Engine::singleton().is_editor_hint();
    if !editor && REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut rs = RenderingServer::singleton();
    let existing = if editor {
        rs.global_shader_parameter_get_list()
    } else {
        Array::new()
    };
    for (name, ty, default) in list() {
        let sn = StringName::from(name);
        if !existing.contains(&sn) {
            rs.global_shader_parameter_add(&sn, ty, &default);
        }
    }
}

thread_local! {
    /// Last value written per global, so unchanged values never reach the RenderingServer
    /// (each write would wake the editor for another redraw; see `writes`).
    static LAST: RefCell<HashMap<&'static str, Variant>> = RefCell::new(HashMap::new());
}

thread_local! {
    /// Class of the last object (texture) written per global; diagnostics only, so the cache
    /// never keeps a texture alive.
    static OBJECTS: RefCell<HashMap<&'static str, String>> = RefCell::new(HashMap::new());
}

/// Drops the cached values (before the engine unloads; see `lib.rs`).
pub fn clear_cache() {
    LAST.with_borrow_mut(|m| m.clear());
    OBJECTS.with_borrow_mut(|m| m.clear());
}

/// Every global this addon has written in this process, by name: the value last written, or
/// the class name of a texture (`PixelDiagnostics.get_published_globals`). Unlike
/// `RenderingServer.global_shader_parameter_get` this also works on the headless (dummy)
/// renderer, so tests can compare the editor's published state with the game's.
pub fn freeze_frame() -> VarDictionary {
    let mut d = VarDictionary::new();
    LAST.with_borrow(|m| {
        for (k, v) in m {
            d.set(*k, v);
        }
    });
    OBJECTS.with_borrow(|m| {
        for (k, v) in m {
            d.set(*k, &format!("<{v}>").to_variant());
        }
    });
    d
}

/// Sets a global shader parameter, unless it already holds `value`.
pub fn assign(name: &'static str, value: &Variant) {
    // Textures are set rarely and must not be kept alive by the cache (it outlives the
    // scene): always write them.
    if value.get_type() == VariantType::OBJECT {
        LAST.with_borrow_mut(|m| m.remove(name));
        let class = value
            .try_to::<Gd<godot::classes::Object>>()
            .map(|o| o.get_class().to_string())
            .unwrap_or_default();
        OBJECTS.with_borrow_mut(|m| m.insert(name, class));
        RenderingServer::singleton()
            .global_shader_parameter_set(&crate::writes::moniker(name), value);
        return;
    }
    let changed = LAST.with_borrow_mut(|m| match m.get_mut(name) {
        Some(v) if v == value => false,
        Some(v) => {
            *v = value.clone();
            true
        }
        None => {
            m.insert(name, value.clone());
            true
        }
    });
    if value.is_nil() {
        OBJECTS.with_borrow_mut(|m| m.remove(name));
    }
    if changed {
        RenderingServer::singleton()
            .global_shader_parameter_set(&crate::writes::moniker(name), value);
    }
}
