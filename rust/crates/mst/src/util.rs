//! Small helpers shared by the nodes: type conversions, mesh upload, shader loading.

use godot::classes::mesh::{ArrayCustomFormat, ArrayFormat, ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, Engine, Node, Shader, ShaderMaterial};
use godot::prelude::*;
use pt_core::math::{IVec2, Vec2 as PVec2, Vec3 as PVec3, iv2, v2};
use pt_core::mesh::MeshArrays;

pub const SHADER_DIR: &str = "res://addons/MarchingSquaresTerrain/shaders/";

pub fn is_editor() -> bool {
    Engine::singleton().is_editor_hint()
}

pub fn to_iv(v: Vector2i) -> IVec2 {
    iv2(v.x, v.y)
}
pub fn from_iv(v: IVec2) -> Vector2i {
    Vector2i::new(v.x, v.y)
}
pub fn to_v2(v: Vector2) -> PVec2 {
    v2(v.x, v.y)
}
pub fn from_v3(v: PVec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}

/// Packs the per-object flags read by `pixel_common.gdshaderinc` (`pt_flags`).
/// Layout: `pt_core::meta::object_flags` (palette and dither on).
pub fn object_flags(pixel_size: i32, outline: bool, outline_id: i32, snap: bool) -> i32 {
    pt_core::meta::object_flags(pixel_size, outline, outline_id, snap, true, true)
}

pub fn load_shader(name: &str) -> Option<Gd<Shader>> {
    try_load::<Shader>(&format!("{SHADER_DIR}{name}")).ok()
}

pub fn shader_material(name: &str) -> Gd<ShaderMaterial> {
    let mut m = ShaderMaterial::new_gd();
    if let Some(s) = load_shader(name) {
        m.set_shader(&s);
    } else {
        godot_error!("Marching Squares Terrain: missing shader {SHADER_DIR}{name}");
    }
    m
}

/// Uploads indexed mesh arrays (positions, normals, UV, UV2, CUSTOM0/1 as RGBA float).
pub fn build_array_mesh(m: &MeshArrays) -> Option<Gd<ArrayMesh>> {
    if m.indices.is_empty() {
        return None;
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let pos: PackedVector3Array = m.positions.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
    let nrm: PackedVector3Array = m.normals.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
    let uv: PackedVector2Array = m.uv.iter().map(|p| Vector2::new(p[0], p[1])).collect();
    let uv2: PackedVector2Array = m.uv2.iter().map(|p| Vector2::new(p[0], p[1])).collect();
    let c0: PackedFloat32Array = m.custom0.iter().flat_map(|c| c.iter().copied()).collect();
    let c1: PackedFloat32Array = m.custom1.iter().flat_map(|c| c.iter().copied()).collect();
    let idx: PackedInt32Array = m.indices.iter().copied().collect();
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
    arrays.set(ArrayType::TEX_UV2.ord() as usize, &uv2.to_variant());
    arrays.set(ArrayType::CUSTOM0.ord() as usize, &c0.to_variant());
    arrays.set(ArrayType::CUSTOM1.ord() as usize, &c1.to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
    let rgba = ArrayCustomFormat::RGBA_FLOAT.ord() as u64;
    let flags = (rgba << ArrayFormat::CUSTOM0_SHIFT.ord()) | (rgba << ArrayFormat::CUSTOM1_SHIFT.ord());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays_ex(PrimitiveType::TRIANGLES, &arrays)
        .flags(ArrayFormat::from_ord(flags))
        .done();
    Some(mesh)
}

/// Adds `child` as an internal (unsaved, hidden from the scene dock) child of `parent`.
pub fn add_internal(parent: &mut Gd<impl Inherits<Node>>, child: &Gd<impl Inherits<Node>>) {
    use godot::classes::node::InternalMode;
    parent.upcast_mut::<Node>().add_child_ex(child).internal(InternalMode::FRONT).done();
}

/// Frees a node if it is still valid.
pub fn free_node(n: Option<Gd<impl Inherits<Node>>>) {
    if let Some(n) = n {
        let mut n = n.upcast::<Node>();
        if n.is_instance_valid() {
            n.queue_free();
        }
    }
}

/// Spaces the CamelCase items of an enum hint ("HexRings,FlatWalls" -> "Hex Rings,Flat
/// Walls") so the inspector names enum values like the editor strips do. Call it from
/// `on_validate_property`; stored values are unchanged.
pub fn space_enum_hint(p: &mut godot::register::info::PropertyInfo) {
    if p.hint_info.hint != godot::register::info::PropertyHint::ENUM {
        return;
    }
    let src = p.hint_info.hint_string.to_string();
    let spaced = space_camel_list(&src);
    if spaced != src {
        p.hint_info.hint_string = spaced.as_str().into();
    }
}

fn space_camel_list(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 8);
    let mut prev: Option<char> = None;
    for c in src.chars() {
        if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase()) {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn spaces_camel_case_enum_items() {
        assert_eq!(super::space_camel_list("Square,Triangle,Hex,HexRings"), "Square,Triangle,Hex,Hex Rings");
        assert_eq!(super::space_camel_list("Anchor:0,LowRes:1"), "Anchor:0,Low Res:1");
    }
}
