use super::model::{BMesh, Face, UvParams, edge};
use super::topo::chain_loops;
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub const METADATA_KEY: &str = "_modeler";
pub const SURFACE_SLOTS_KEY: &str = "surfaceSlots";
pub const MESH_KEY: &str = "mesh";
pub const VERSION_KEY: &str = "version";
pub const FORMAT_VERSION: i64 = 4;
pub const SURFACE_SLOT_LIMIT: i64 = 0x10000;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XAutoUv {
    pub use_world_space: bool,
    pub flip_u: bool,
    pub flip_v: bool,
    pub swap_uv: bool,
    pub magnify: [f32; 2],
    pub offset: [f32; 2],
    pub rotation: f32,
}
impl Default for XAutoUv {
    fn default() -> Self {
        XAutoUv {
            use_world_space: false,
            flip_u: false,
            flip_v: false,
            swap_uv: false,
            magnify: [1.0, 1.0],
            offset: [0.0, 0.0],
            rotation: 0.0,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XFace {
    pub ordinals: Vec<u32>,
    pub smoothing_group: i64,
    pub submesh_index: i64,
    pub texture_group: i64,
    pub manual_uv: bool,
    pub tex_coord: XAutoUv,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XMesh {
    pub version: i64,
    pub surface_slots: Vec<i64>,
    pub positions: Vec<[f32; 3]>,
    pub facets: Vec<XFace>,
    pub shared_vertices: Vec<Vec<u32>>,
    pub uv0: Vec<[f32; 2]>,
    pub tints: Option<Vec<[f32; 4]>>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XImportOptions {
    pub freeze_uvs: bool,
}
impl Default for XImportOptions {
    fn default() -> Self {
        XImportOptions { freeze_uvs: true }
    }
}
#[derive(Clone, Debug, Default)]
pub struct XImport {
    pub cage: BMesh,
    pub surface_of_slot: Vec<Option<usize>>,
    pub split_faces: usize,
    pub detached_faces: usize,
    pub dropped_triangles: usize,
}
fn rgba8(c: [f32; 4]) -> [u8; 4] {
    c.map(|v| {
        if v.is_nan() {
            0
        } else {
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    })
}
fn verify(x: &XMesh) -> Result<(), String> {
    if !(2..=FORMAT_VERSION).contains(&x.version) {
        return Err(format!(
            "Unsupported Modeler Modeler snapshot version {}.",
            x.version
        ));
    }
    let n = x.positions.len();
    if x.uv0.len() != n {
        return Err("The Modeler mesh has a uv0 array of the wrong length.".into());
    }
    if x.tints.as_ref().is_some_and(|c| c.len() != n) {
        return Err("The Modeler mesh has a colors array of the wrong length.".into());
    }
    if x.positions.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
        return Err("The Modeler mesh has invalid positions.".into());
    }
    if x.uv0.iter().any(|t| t.iter().any(|c| !c.is_finite())) {
        return Err("The Modeler mesh has invalid UVs.".into());
    }
    for (i, f) in x.facets.iter().enumerate() {
        if f.ordinals.len() % 3 != 0 || f.ordinals.iter().any(|r| *r as usize >= n) {
            return Err(format!("Modeler face {i} has invalid indices."));
        }
        if f.submesh_index < 0 || f.submesh_index >= SURFACE_SLOT_LIMIT || f.smoothing_group < 0 {
            return Err(format!("Modeler face {i} has invalid attributes."));
        }
    }
    if x.shared_vertices.iter().flatten().any(|r| *r as usize >= n) {
        return Err("The Modeler mesh has invalid shared vertices.".into());
    }
    if x.surface_slots
        .iter()
        .any(|s| *s < 0 || *s >= SURFACE_SLOT_LIMIT)
    {
        return Err("The Modeler mesh has invalid surface slots.".into());
    }
    Ok(())
}
fn oriented_fringe(tris: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
    for t in tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *counts.entry(edge(a, b)).or_default() += 1;
        }
    }
    tris.iter()
        .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
        .filter(|(a, b)| counts[&edge(*a, *b)] == 1)
        .collect()
}
fn outline(tris: &[[u32; 3]]) -> Option<Vec<u32>> {
    let loops = chain_loops(&oriented_fringe(tris))?;
    let used: BTreeSet<u32> = tris.iter().flatten().copied().collect();
    let [outline] = loops.as_slice() else {
        return None;
    };
    (outline.len() >= 3 && outline.len() == used.len()).then(|| outline.clone())
}
pub fn to_bmesh(x: &XMesh, opt: &XImportOptions) -> Result<XImport, String> {
    verify(x)?;
    let n = x.positions.len();
    let mut uv0 = x.uv0.clone();
    if x.version == 2 {
        let manual: BTreeSet<u32> = x
            .facets
            .iter()
            .filter(|f| f.manual_uv)
            .flat_map(|f| f.ordinals.iter().copied())
            .collect();
        for r in manual {
            uv0[r as usize][1] = 1.0 - uv0[r as usize][1];
        }
    }
    let mut vert_of = vec![u32::MAX; n];
    let mut out = XImport::default();
    let m = &mut out.cage;
    for g in &x.shared_vertices {
        let Some(first) = g.iter().find(|r| vert_of[**r as usize] == u32::MAX) else {
            continue;
        };
        let v = m.add_vert(Vec3::from_arr(x.positions[*first as usize]));
        for r in g {
            if vert_of[*r as usize] == u32::MAX {
                vert_of[*r as usize] = v;
            }
        }
    }
    for r in 0..n {
        if vert_of[r] == u32::MAX {
            vert_of[r] = m.add_vert(Vec3::from_arr(x.positions[r]));
        }
    }
    let mut directed: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut undirected: HashMap<(u32, u32), usize> = HashMap::new();
    for xf in &x.facets {
        let mut rec_of: BTreeMap<u32, u32> = BTreeMap::new();
        let mut tris = Vec::new();
        for t in xf.ordinals.chunks(3) {
            let v = [
                vert_of[t[0] as usize],
                vert_of[t[1] as usize],
                vert_of[t[2] as usize],
            ];
            if v[0] == v[1] || v[1] == v[2] || v[0] == v[2] {
                out.dropped_triangles += 1;
                continue;
            }
            for k in 0..3 {
                rec_of.entry(v[k]).or_insert(t[k]);
            }
            tris.push(v);
        }
        if tris.is_empty() {
            continue;
        }
        let loops: Vec<Vec<u32>> = match outline(&tris) {
            Some(l) => vec![l],
            None => {
                out.split_faces += 1;
                tris.iter().map(|t| t.to_vec()).collect()
            }
        };
        for mut lp in loops {
            let es: Vec<(u32, u32)> = (0..lp.len())
                .map(|i| (lp[i], lp[(i + 1) % lp.len()]))
                .collect();
            if es.iter().any(|e| {
                directed.contains(e) || undirected.get(&edge(e.0, e.1)).copied().unwrap_or(0) >= 2
            }) {
                out.detached_faces += 1;
                for v in &mut lp {
                    let rec = rec_of[v];
                    let nv = m.add_vert(m.positions[*v as usize]);
                    rec_of.insert(nv, rec);
                    *v = nv;
                }
            }
            for i in 0..lp.len() {
                let (a, b) = (lp[i], lp[(i + 1) % lp.len()]);
                directed.insert((a, b));
                *undirected.entry(edge(a, b)).or_default() += 1;
            }
            let recs: Vec<usize> = lp.iter().map(|v| rec_of[v] as usize).collect();
            let mut f = Face::new(lp);
            f.stuff = xf.submesh_index as u16;
            f.glide = xf.smoothing_group.min(u16::MAX as i64) as u16;
            f.tex_coord = UvParams {
                offset: v2(xf.tex_coord.offset[0], xf.tex_coord.offset[1]),
                rotation: xf.tex_coord.rotation,
                magnify: v2(xf.tex_coord.magnify[0], xf.tex_coord.magnify[1]),
                world: xf.tex_coord.use_world_space,
            };
            if xf.manual_uv || opt.freeze_uvs {
                f.uv_manual = true;
                f.uv_coords = recs
                    .iter()
                    .map(|r| v2(uv0[*r][0], 1.0 - uv0[*r][1]))
                    .collect();
            }
            if let Some(c) = &x.tints {
                let cc: Vec<[u8; 4]> = recs.iter().map(|r| rgba8(c[*r])).collect();
                if cc.iter().any(|c| *c != [255; 4]) {
                    f.color_corners = cc;
                }
            }
            m.facets.push(f);
        }
    }
    m.compact();
    m.sanity_check()?;
    let top = m
        .facets
        .iter()
        .map(|f| f.stuff as usize + 1)
        .max()
        .unwrap_or(0);
    out.surface_of_slot = vec![None; top];
    for (surface, slot) in x.surface_slots.iter().enumerate() {
        if let Some(s) = out.surface_of_slot.get_mut(*slot as usize)
            && s.is_none()
        {
            *s = Some(surface);
        }
    }
    Ok(out)
}
trait FromArr {
    fn from_arr(a: [f32; 3]) -> Self;
}
impl FromArr for Vec3 {
    fn from_arr(a: [f32; 3]) -> Vec3 {
        v3(a[0], a[1], a[2])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn block() -> XMesh {
        let positions: Vec<[f32; 3]> = vec![
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, 0.5, 0.5],
            [0.5, -0.5, 0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [0.5, 0.5, -0.5],
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
        ];
        let faces = (0..6u32)
            .map(|s| {
                let b = s * 4;
                XFace {
                    ordinals: vec![b, b + 1, b + 2, b, b + 2, b + 3],
                    submesh_index: (s == 5) as i64,
                    ..Default::default()
                }
            })
            .collect();
        let mut groups: BTreeMap<[i32; 3], Vec<u32>> = BTreeMap::new();
        for (i, p) in positions.iter().enumerate() {
            groups
                .entry(p.map(|c| (c * 2.0) as i32))
                .or_default()
                .push(i as u32);
        }
        let uv0 = (0..24)
            .map(|i| [(i % 2) as f32, (i / 2 % 2) as f32])
            .collect();
        XMesh {
            version: 4,
            surface_slots: vec![0, 1],
            positions,
            facets: faces,
            shared_vertices: groups.into_values().collect(),
            uv0,
            tints: None,
        }
    }
    #[test]
    fn cube_decodes_to_closed_six_quads() {
        let r = to_bmesh(&block(), &XImportOptions::default()).unwrap();
        let m = &r.cage;
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.facets.len(), 6);
        assert_eq!(m.positions.len(), 8);
        assert!(m.facets.iter().all(|f| f.verts.len() == 4));
        assert!(
            (m.volume() - 1.0).abs() < 1e-5,
            "outward winding: {}",
            m.volume()
        );
        assert_eq!(m.facets[5].stuff, 1);
        assert_eq!(r.surface_of_slot, vec![Some(0), Some(1)]);
        assert!(
            m.facets.iter().all(|f| f.by_hand_tex_coords().is_some()),
            "uv0 frozen by default"
        );
        assert_eq!(
            (r.split_faces, r.detached_faces, r.dropped_triangles),
            (0, 0, 0)
        );
    }
    #[test]
    fn manual_uvs_colors_and_smoothing_follow_records() {
        let mut x = block();
        x.facets[0].manual_uv = true;
        x.facets[0].smoothing_group = 3;
        x.uv0[2] = [0.25, 0.75];
        let mut colors = vec![[1.0; 4]; 24];
        colors[1] = [1.0, 0.0, 0.0, 0.5];
        x.tints = Some(colors);
        let r = to_bmesh(&x, &XImportOptions { freeze_uvs: false }).unwrap();
        let f = &r.cage.facets[0];
        assert_eq!(f.glide, 3);
        let uvs = f.by_hand_tex_coords().unwrap();
        let ci = f
            .verts
            .iter()
            .position(|v| r.cage.positions[*v as usize] == v3(0.5, 0.5, -0.5))
            .unwrap();
        assert_eq!(uvs[ci], v2(0.25, 0.25), "V-up uv0 -> Godot V-down");
        let cj = f
            .verts
            .iter()
            .position(|v| r.cage.positions[*v as usize] == v3(-0.5, 0.5, -0.5))
            .unwrap();
        assert_eq!(f.color_corners[cj], [255, 0, 0, 128]);
        assert!(
            r.cage.facets[1].by_hand_tex_coords().is_none()
                && r.cage.facets[1].color_corners.is_empty()
        );
        x.version = 2;
        let r = to_bmesh(&x, &XImportOptions { freeze_uvs: false }).unwrap();
        assert_eq!(
            r.cage.facets[0].by_hand_tex_coords().unwrap()[ci],
            v2(0.25, 0.75)
        );
    }
    #[test]
    fn fan_with_centre_vertex_stays_triangles() {
        let x = XMesh {
            version: 4,
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
                [0.5, 0.0, -0.5],
            ],
            facets: vec![XFace {
                ordinals: vec![0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4],
                ..Default::default()
            }],
            uv0: vec![[0.0; 2]; 5],
            ..Default::default()
        };
        let r = to_bmesh(&x, &XImportOptions::default()).unwrap();
        assert_eq!(r.cage.facets.len(), 4);
        assert_eq!(r.split_faces, 1);
        assert!((0..r.cage.facets.len()).all(|f| r.cage.face_normal(f).y > 0.9));
    }
    #[test]
    fn non_manifold_faces_are_detached() {
        let mut x = block();
        let dup = x.facets[0].clone();
        x.facets.push(dup);
        let r = to_bmesh(&x, &XImportOptions::default()).unwrap();
        r.cage.sanity_check().unwrap();
        assert_eq!(r.detached_faces, 1);
        assert_eq!(r.cage.facets.len(), 7);
    }
    #[test]
    fn damaged_metadata_is_an_error() {
        let mut x = block();
        x.facets[0].ordinals.push(99);
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = block();
        x.facets[0].ordinals = vec![0, 1, 200];
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = block();
        x.uv0.pop();
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = block();
        x.version = 9;
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = block();
        x.shared_vertices.push(vec![1000]);
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        assert!(
            to_bmesh(
                &XMesh {
                    version: 4,
                    ..Default::default()
                },
                &XImportOptions::default()
            )
            .unwrap()
            .cage
            .facets
            .is_empty()
        );
    }
}
