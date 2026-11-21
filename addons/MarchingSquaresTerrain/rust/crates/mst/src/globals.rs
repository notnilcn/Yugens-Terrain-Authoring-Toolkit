//! The values every pixel-aware shader in the project reads, kept in one engine-wide table
//! because shaders on unrelated objects must agree on them without being wired to the node
//! that sets them.
use godot::classes::RenderingServer;
use godot::classes::rendering_server::GlobalShaderParameterType as T;
use godot::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
pub const DEFAULT_META_LAYER: u32 = 20;
static META_LAYER: AtomicU32 = AtomicU32::new(DEFAULT_META_LAYER);
pub fn meta_layer() -> u32 {
    META_LAYER.load(Ordering::Relaxed)
}
pub fn meta_mask() -> u32 {
    1u32 << (meta_layer().clamp(1, 20) - 1)
}
pub fn id_mask() -> u32 {
    meta_mask() | (1u32 << pt_core::meta::ID_CAMERA_BIT)
}
pub fn set_meta_layer(layer: u32) {
    META_LAYER.store(layer.clamp(1, 20), Ordering::Relaxed);
    assign("pt_meta_mask", &meta_mask().to_variant());
    assign("pt_id_mask", &id_mask().to_variant());
}
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
        ("pt_sun_light", T::FLOAT, 1.0f32.to_variant()),
        ("pt_sun_low", T::FLOAT, 0.0f32.to_variant()),
        ("pt_lod_fade", T::VEC4, Vector4::ZERO.to_variant()),
        ("pt_lod_eye", T::VEC2, Vector2::ZERO.to_variant()),
        ("pt_terrain_height", T::SAMPLER2D, Variant::nil()),
        ("pt_terrain_rect", T::VEC4, Vector4::ZERO.to_variant()),
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
        ("pt_cell_fog", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_cell_fog_color",
            T::COLOR,
            Color::from_rgb(0.0505, 0.0605, 0.0905).to_variant(),
        ),
        ("pt_wet", T::FLOAT, 0.0f32.to_variant()),
        ("pt_snow", T::FLOAT, 0.0f32.to_variant()),
        ("pt_water_a", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_water_b",
            T::VEC4,
            Vector4::new(0.0, 0.6005, 3.0, 0.6005).to_variant(),
        ),
        ("pt_horizon", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_light",
            T::VEC4,
            Vector4::new(0.0, 0.0, 0.3505, 0.6005).to_variant(),
        ),
        ("pt_light_ramp", T::SAMPLER2D, Variant::nil()),
        ("pt_rim", T::VEC4, Vector4::ZERO.to_variant()),
        ("pt_rain", T::FLOAT, 0.0f32.to_variant()),
        (
            "pt_wind",
            T::VEC4,
            Vector4::new(0.8945, 0.4475, 1.0, 0.2505).to_variant(),
        ),
        ("pt_rays_a", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_rays_b",
            T::VEC4,
            Vector4::new(0.9005, 1.0, 0.0, 0.0).to_variant(),
        ),
        ("pt_fire_tex", T::SAMPLER2D, Variant::nil()),
        ("pt_fire_rect", T::VEC4, Vector4::ZERO.to_variant()),
        (
            "pt_fire_a",
            T::VEC4,
            Vector4::new(0.0, 1.0, 1.0, 1.0).to_variant(),
        ),
        ("pt_time", T::FLOAT, 0.0f32.to_variant()),
    ]
}
static REGISTERED: AtomicBool = AtomicBool::new(false);
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
    static LAST: RefCell<HashMap<&'static str, Variant>> = RefCell::new(HashMap::new());
}
thread_local! {
    static OBJECTS: RefCell<HashMap<&'static str, String>> = RefCell::new(HashMap::new());
}
pub fn clear_cache() {
    LAST.with_borrow_mut(|m| m.clear());
    OBJECTS.with_borrow_mut(|m| m.clear());
}
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
pub fn assign(name: &'static str, value: &Variant) {
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
