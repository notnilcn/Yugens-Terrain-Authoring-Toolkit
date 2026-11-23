pub const LEGACY_MULTIMESH_PROPS: &[&str] = &[
    "multimesh",
    "mesh",
    "material_override",
    "material_overlay",
    "cast_shadow",
    "gi_mode",
    "gi_lightmap_scale",
    "gi_lightmap_texel_scale",
    "visibility_range_begin",
    "visibility_range_begin_margin",
    "visibility_range_end",
    "visibility_range_end_margin",
    "visibility_range_fade_mode",
    "lod_bias",
    "extra_cull_margin",
    "custom_aabb",
    "sorting_offset",
    "sorting_use_aabb_center",
    "ignore_occlusion_culling",
    "transparency",
];
pub fn is_legacy_multimesh_prop(name: &str) -> bool {
    LEGACY_MULTIMESH_PROPS.contains(&name) || name.starts_with("instance_shader_parameters/")
}
