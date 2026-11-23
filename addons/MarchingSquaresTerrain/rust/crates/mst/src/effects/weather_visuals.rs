//! Builds the drawing parts for falling rain, snow, splashes and lightning.
//!
//! These parts are created in code because each one is only a carrier for instance and
//! geometry data the weather generates: their capacity and covered area follow the
//! weather's settings and they are thrown away and rebuilt whenever those change, so a
//! node declared in a scene would hold nothing an author could set and would be replaced
//! wholesale on the first change.

use super::rng::Rng;
use crate::globals;
use crate::shader_materials::shader_material;
use crate::instance_buffer::InstanceBuffer;
use godot::classes::base_material_3d::{CullMode, ShadingMode};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    ImmediateMesh, MeshInstance3D, MultiMesh, MultiMeshInstance3D, QuadMesh, ShaderMaterial,
    StandardMaterial3D,
};
use godot::prelude::*;
pub fn rain_instance(count: i32, area: Vector3) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
    seeded_quads(
        "Rain",
        "precip.gdshader",
        count,
        0xD30_95,
        Aabb::new(
            -area - Vector3::splat(2.0),
            (area + Vector3::splat(2.0)) * 2.0,
        ),
    )
}
pub fn snow_instance(count: i32) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
    let e = Vector3::splat(260.0);
    seeded_quads(
        "Snow",
        "precip.gdshader",
        count,
        0xF1A_4E5,
        Aabb::new(-e - Vector3::splat(2.0), (e + Vector3::splat(2.0)) * 2.0),
    )
}
pub fn splash_instance(count: i32, area: Vector3) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
    seeded_quads(
        "Splashes",
        "rain_splash.gdshader",
        count,
        0x5_9A5B,
        Aabb::new(
            Vector3::new(-area.x - 2.0, -200.0, -area.z - 2.0),
            Vector3::new(area.x * 2.0 + 4.0, 400.0, area.z * 2.0 + 4.0),
        ),
    )
}
pub fn bolt_instance(color: Color) -> Gd<MeshInstance3D> {
    let mut bolt = MeshInstance3D::new_alloc();
    bolt.set_name("Bolt");
    bolt.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    let layers = bolt.get_layer_mask() & !globals::meta_mask();
    bolt.set_layer_mask(layers);
    bolt.set_as_top_level(true);
    bolt.set_visible(false);
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_shading_mode(ShadingMode::UNSHADED);
    mat.set_cull_mode(CullMode::DISABLED);
    mat.set_albedo(color);
    bolt.set_material_override(&mat);
    bolt.set_mesh(&ImmediateMesh::new_gd());
    bolt
}
fn seeded_quads(
    name: &str,
    shader: &str,
    count: i32,
    seed: u64,
    bounds: Aabb,
) -> (Gd<MultiMeshInstance3D>, Gd<ShaderMaterial>) {
    let mut quad = QuadMesh::new_gd();
    quad.set_size(Vector2::new(1.0, 1.0));
    let mut mm = MultiMesh::new_gd();
    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
    mm.set_use_custom_data(true);
    mm.set_mesh(&quad);
    mm.set_instance_count(count);
    let mut rng = Rng(seed);
    let mut buffer = InstanceBuffer::default();
    for _ in 0..count {
        let custom = Color::from_rgba(
            rng.next_f32(),
            rng.next_f32(),
            rng.next_f32(),
            rng.next_f32(),
        );
        buffer.stack(Transform3D::IDENTITY, custom);
    }
    buffer.upload(&mut mm, bounds);
    crate::instance_buffer::set_visible_instances(&mut mm, 0);
    let mut mi = MultiMeshInstance3D::new_alloc();
    mi.set_name(name);
    mi.set_multimesh(&mm);
    mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    let layers = mi.get_layer_mask() & !globals::meta_mask();
    mi.set_layer_mask(layers);
    mi.set_as_top_level(true);
    let mat = shader_material(shader);
    mi.set_material_override(&mat);
    (mi, mat)
}
