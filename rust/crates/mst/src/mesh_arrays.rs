use godot::classes::ArrayMesh;
use godot::classes::mesh::{ArrayCustomFormat, ArrayFormat, ArrayType, PrimitiveType};
use godot::prelude::*;
use pt_core::mesh::MeshArrays;
pub fn build_array_mesh(m: &MeshArrays) -> Option<Gd<ArrayMesh>> {
    build_array_mesh_concat(&[m])
}
pub fn build_array_mesh_concat(parts: &[&MeshArrays]) -> Option<Gd<ArrayMesh>> {
    let verts: usize = parts.iter().map(|m| m.positions.len()).sum();
    let tris: usize = parts.iter().map(|m| m.ordinals.len()).sum();
    if tris == 0 {
        return None;
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let (mut pos, mut nrm) = (Vec::with_capacity(verts), Vec::with_capacity(verts));
    let (mut uv, mut uv2) = (Vec::with_capacity(verts), Vec::with_capacity(verts));
    let (mut c0, mut c1) = (Vec::with_capacity(verts * 4), Vec::with_capacity(verts * 4));
    let mut idx = Vec::with_capacity(tris);
    for m in parts {
        let base = pos.len() as i32;
        let n = m.positions.len();
        pos.extend(m.positions.iter().map(|p| Vector3::new(p[0], p[1], p[2])));
        let pad3 = |out: &mut Vec<Vector3>, src: &[[f32; 3]]| {
            out.extend(src.iter().take(n).map(|p| Vector3::new(p[0], p[1], p[2])));
            out.resize(base as usize + n, Vector3::UP);
        };
        pad3(&mut nrm, &m.normals);
        for (out, src) in [(&mut uv, &m.tex_coord), (&mut uv2, &m.uv2)] {
            out.extend(src.iter().take(n).map(|p| Vector2::new(p[0], p[1])));
            out.resize(base as usize + n, Vector2::ZERO);
        }
        for (out, src) in [(&mut c0, &m.custom0), (&mut c1, &m.custom1)] {
            out.extend(src.iter().take(n).flat_map(|c| c.iter().copied()));
            out.resize((base as usize + n) * 4, 0.0);
        }
        idx.extend(m.ordinals.iter().map(|i| i + base));
    }
    arrays.set(
        ArrayType::VERTEX.ord() as usize,
        &PackedVector3Array::from(pos.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::NORMAL.ord() as usize,
        &PackedVector3Array::from(nrm.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::TEX_UV.ord() as usize,
        &PackedVector2Array::from(uv.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::TEX_UV2.ord() as usize,
        &PackedVector2Array::from(uv2.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::CUSTOM0.ord() as usize,
        &PackedFloat32Array::from(c0.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::CUSTOM1.ord() as usize,
        &PackedFloat32Array::from(c1.as_slice()).to_variant(),
    );
    arrays.set(
        ArrayType::INDEX.ord() as usize,
        &PackedInt32Array::from(idx.as_slice()).to_variant(),
    );
    let rgba = ArrayCustomFormat::RGBA_FLOAT.ord() as u64;
    let flags =
        (rgba << ArrayFormat::CUSTOM0_SHIFT.ord()) | (rgba << ArrayFormat::CUSTOM1_SHIFT.ord());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays_ex(PrimitiveType::TRIANGLES, &arrays)
        .flags(ArrayFormat::from_ord(flags))
        .done();
    Some(mesh)
}
pub fn mesh_arrays_of(mesh: &Gd<godot::classes::Mesh>) -> Option<MeshArrays> {
    if mesh.get_surface_count() < 1 {
        return None;
    }
    let a = mesh.surface_get_arrays(0);
    let get = |t: ArrayType| a.get(t.ord() as usize).unwrap_or_default();
    let pos = get(ArrayType::VERTEX).try_to::<PackedVector3Array>().ok()?;
    let mut m = MeshArrays {
        positions: pos.as_slice().iter().map(|v| [v.x, v.y, v.z]).collect(),
        ..Default::default()
    };
    if let Ok(n) = get(ArrayType::NORMAL).try_to::<PackedVector3Array>() {
        m.normals = n.as_slice().iter().map(|v| [v.x, v.y, v.z]).collect();
    }
    if let Ok(v) = get(ArrayType::TEX_UV).try_to::<PackedVector2Array>() {
        m.tex_coord = v.as_slice().iter().map(|v| [v.x, v.y]).collect();
    }
    if let Ok(v) = get(ArrayType::TEX_UV2).try_to::<PackedVector2Array>() {
        m.uv2 = v.as_slice().iter().map(|v| [v.x, v.y]).collect();
    }
    for (t, out) in [
        (ArrayType::CUSTOM0, &mut m.custom0),
        (ArrayType::CUSTOM1, &mut m.custom1),
    ] {
        if let Ok(v) = get(t).try_to::<PackedFloat32Array>()
            && v.len() == m.positions.len() * 4
        {
            *out = v
                .as_slice()
                .chunks_exact(4)
                .map(|c| [c[0], c[1], c[2], c[3]])
                .collect();
        }
    }
    m.ordinals = match get(ArrayType::INDEX).try_to::<PackedInt32Array>() {
        Ok(i) if !i.is_empty() => i.as_slice().to_vec(),
        _ => (0..m.positions.len() as i32).collect(),
    };
    Some(m)
}
