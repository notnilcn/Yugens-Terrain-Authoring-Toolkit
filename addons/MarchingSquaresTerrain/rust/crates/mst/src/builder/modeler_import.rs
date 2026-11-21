use godot::builtin::VariantType;
use godot::prelude::*;
use pt_core::build::modeler_meta::{
    MESH_KEY, METADATA_KEY, SURFACE_SLOTS_KEY, VERSION_KEY, XAutoUv, XFace, XMesh,
};
pub fn envelope(mesh: &Gd<godot::classes::Mesh>) -> Option<VarDictionary> {
    if !mesh.has_meta(METADATA_KEY) {
        return None;
    }
    mesh.get_meta(METADATA_KEY).try_to::<VarDictionary>().ok()
}
fn num(v: &Variant) -> Option<f64> {
    match v.get_type() {
        VariantType::INT => v.try_to::<i64>().ok().map(|i| i as f64),
        VariantType::FLOAT => v.try_to::<f64>().ok(),
        VariantType::BOOL => v.try_to::<bool>().ok().map(|b| b as i64 as f64),
        _ => None,
    }
}
fn integer(d: &VarDictionary, k: &str) -> Option<i64> {
    d.get(k).as_ref().and_then(num).map(|f| f.round() as i64)
}
fn truthy(d: &VarDictionary, k: &str) -> bool {
    d.get(k).is_some_and(|v| {
        v.try_to::<bool>()
            .ok()
            .or_else(|| num(&v).map(|f| f != 0.0))
            .unwrap_or(false)
    })
}
fn digits(v: &Variant) -> Option<Vec<f64>> {
    match v.get_type() {
        VariantType::ARRAY => v
            .try_to::<VarArray>()
            .ok()
            .map(|a| a.iter_shared().filter_map(|x| num(&x)).collect()),
        VariantType::PACKED_INT32_ARRAY => v
            .try_to::<PackedInt32Array>()
            .ok()
            .map(|a| a.as_slice().iter().map(|x| *x as f64).collect()),
        VariantType::PACKED_INT64_ARRAY => v
            .try_to::<PackedInt64Array>()
            .ok()
            .map(|a| a.as_slice().iter().map(|x| *x as f64).collect()),
        VariantType::PACKED_FLOAT32_ARRAY => v
            .try_to::<PackedFloat32Array>()
            .ok()
            .map(|a| a.as_slice().iter().map(|x| *x as f64).collect()),
        VariantType::PACKED_FLOAT64_ARRAY => v
            .try_to::<PackedFloat64Array>()
            .ok()
            .map(|a| a.as_slice().to_vec()),
        _ => None,
    }
}
fn ordinals(v: &Variant) -> Result<Vec<u32>, String> {
    let n = digits(v).ok_or("The Modeler mesh has a face without indices.")?;
    n.into_iter()
        .map(|f| {
            if f >= 0.0 && f.is_finite() {
                Ok(f.round() as u32)
            } else {
                Err("The Modeler mesh has a negative index.".to_string())
            }
        })
        .collect()
}
fn pair(d: &VarDictionary, k: &str, default: [f32; 2]) -> [f32; 2] {
    match d.get(k) {
        Some(v) => {
            if let Ok(p) = v.try_to::<Vector2>() {
                return [p.x, p.y];
            }
            match digits(&v) {
                Some(n) if n.len() >= 2 => [n[0] as f32, n[1] as f32],
                _ => default,
            }
        }
        None => default,
    }
}
fn self_auto_tex_coord(v: Option<Variant>) -> XAutoUv {
    let mut uv = XAutoUv::default();
    let Some(d) = v.and_then(|v| v.try_to::<VarDictionary>().ok()) else {
        return uv;
    };
    uv.use_world_space = truthy(&d, "useWorldSpace");
    uv.flip_u = truthy(&d, "flipU");
    uv.flip_v = truthy(&d, "flipV");
    uv.swap_uv = truthy(&d, "swapUV");
    uv.magnify = pair(&d, "scale", uv.magnify);
    uv.offset = pair(&d, "offset", uv.offset);
    uv.rotation = d.get("rotation").as_ref().and_then(num).unwrap_or(0.0) as f32;
    uv
}
pub fn scan_in(env: &VarDictionary) -> Result<XMesh, String> {
    let version = integer(env, VERSION_KEY).ok_or("The Modeler metadata has no version.")?;
    let mesh = env
        .get(MESH_KEY)
        .and_then(|v| v.try_to::<VarDictionary>().ok())
        .ok_or("The Modeler metadata has no mesh.")?;
    let surface_slots = env
        .get(SURFACE_SLOTS_KEY)
        .and_then(|v| digits(&v))
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.round() as i64)
        .collect();
    let positions: Vec<[f32; 3]> = mesh
        .get("positions")
        .and_then(|v| v.try_to::<PackedVector3Array>().ok())
        .ok_or("The Modeler mesh has no positions.")?
        .as_slice()
        .iter()
        .map(|p| [p.x, p.y, p.z])
        .collect();
    let uv0: Vec<[f32; 2]> = match mesh
        .get("uv0")
        .and_then(|v| v.try_to::<PackedVector2Array>().ok())
    {
        Some(a) => a.as_slice().iter().map(|p| [p.x, p.y]).collect(),
        None => vec![[0.0, 0.0]; positions.len()],
    };
    let colors = mesh
        .get("colors")
        .and_then(|v| v.try_to::<PackedColorArray>().ok())
        .map(|a| a.as_slice().iter().map(|c| [c.r, c.g, c.b, c.a]).collect());
    let mut faces = Vec::new();
    let list = mesh
        .get("faces")
        .and_then(|v| v.try_to::<VarArray>().ok())
        .ok_or("The Modeler mesh has no faces.")?;
    for f in list.iter_shared() {
        let d = f
            .try_to::<VarDictionary>()
            .map_err(|_| "The Modeler mesh has a damaged face.".to_string())?;
        faces.push(XFace {
            ordinals: ordinals(&d.get("indices").unwrap_or_default())?,
            smoothing_group: integer(&d, "smoothingGroup").unwrap_or(0),
            submesh_index: integer(&d, "submeshIndex").unwrap_or(0),
            texture_group: integer(&d, "textureGroup").unwrap_or(0),
            manual_uv: truthy(&d, "manualUV"),
            tex_coord: self_auto_tex_coord(d.get("uv")),
        });
    }
    let mut shared_vertices = Vec::new();
    if let Some(groups) = mesh
        .get("sharedVertices")
        .and_then(|v| v.try_to::<VarArray>().ok())
    {
        for g in groups.iter_shared() {
            shared_vertices.push(ordinals(&g)?);
        }
    }
    Ok(XMesh {
        version,
        surface_slots,
        positions,
        facets: faces,
        shared_vertices,
        uv0,
        tints: colors,
    })
}
