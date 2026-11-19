//! Reads meshes saved by the Xogot Modeler (`mesh.meta["_xogot_modeler"]`) into builder meshes.
//!
//! Port of xogot-rust (MIT, Xibbon / xogot-rust port): the envelope layout of
//! `xogot_modeler/src/shared/document.rs` (`METADATA_KEY`, `SURFACE_SLOTS_KEY`, `MESH_KEY`,
//! `VERSION_KEY`, `envelope`, `prepare`), the snapshot fields of
//! `modeler_kit/src/editable_mesh.rs` (`to_value` / `from_value` / `decode_face`,
//! `FORMAT_VERSION`, `SnapshotCodec::mesh` with the v2/v3 migrations) and the face outline
//! walk of `Face::directed_perimeter` / `MeshExporter::export_polygons`.
//!
//! Xogot stores a mesh as *records* (one position per face corner); `sharedVertices` groups the
//! records that are one logical vertex, and every face is a triangle list over its records. The
//! builder stores shared vertices plus n-gon loops, so each Xogot face becomes one n-gon whose
//! loop is recovered from its triangles' outline (faces whose outline is not one clean loop are
//! kept as their triangles).
//!
//! The Godot side reads the metadata Dictionary into [`XMesh`] (Xogot writes its integers as
//! floats, e.g. `version = 4.0`, so accept both) and calls [`to_bmesh`].

use super::model::{BMesh, Face, UvParams, edge};
use super::topo::chain_loops;
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Mesh-resource metadata key holding the editable source.
pub const METADATA_KEY: &str = "_xogot_modeler";
/// Envelope entry mapping each compiled surface to its material slot (submesh index).
pub const SURFACE_SLOTS_KEY: &str = "surfaceSlots";
/// Envelope entry holding the snapshot dictionary of the mesh.
pub const MESH_KEY: &str = "mesh";
/// Envelope entry holding the snapshot format version.
pub const VERSION_KEY: &str = "version";
/// The snapshot format version Xogot writes (`FORMAT_VERSION`); 2 and 3 are migrated.
pub const FORMAT_VERSION: i64 = 4;
/// Surface slots at or above this are treated as damaged metadata (`SURFACE_SLOT_LIMIT`).
pub const SURFACE_SLOT_LIMIT: i64 = 0x10000;

/// A face's automatic-UV settings (Xogot `AutoUnwrap`, the `uv` dictionary of a face).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XAutoUv {
    pub use_world_space: bool,
    pub flip_u: bool,
    pub flip_v: bool,
    pub swap_uv: bool,
    pub scale: [f32; 2],
    pub offset: [f32; 2],
    /// Degrees (as in xogot-rust `mesh_uv.rs`).
    pub rotation: f32,
}

impl Default for XAutoUv {
    fn default() -> Self {
        XAutoUv { use_world_space: false, flip_u: false, flip_v: false, swap_uv: false, scale: [1.0, 1.0], offset: [0.0, 0.0], rotation: 0.0 }
    }
}

/// One Xogot face (`faces[i]` of the snapshot).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XFace {
    /// Triangle list over records (`indices`), counter-clockwise seen from outside.
    pub indices: Vec<u32>,
    /// `smoothingGroup`: 0 = flat.
    pub smoothing_group: i64,
    /// `submeshIndex`: the material slot.
    pub submesh_index: i64,
    /// `textureGroup` (not used by the builder; kept for callers).
    pub texture_group: i64,
    /// `manualUV`: the face's records carry hand-edited `uv0`.
    pub manual_uv: bool,
    pub uv: XAutoUv,
}

/// Plain-data copy of the `_xogot_modeler` envelope (filled by the Godot side).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XMesh {
    /// `version` of the envelope (2, 3 or 4).
    pub version: i64,
    /// `surfaceSlots`: material slot of each compiled surface of the ArrayMesh.
    pub surface_slots: Vec<i64>,
    /// `mesh.positions`: one per record.
    pub positions: Vec<[f32; 3]>,
    pub faces: Vec<XFace>,
    /// `mesh.sharedVertices`: records that are one logical vertex.
    pub shared_vertices: Vec<Vec<u32>>,
    /// `mesh.uv0`: one per record, V pointing **up** (Xogot writes `ARRAY_TEX_UV = (u, 1 - v)`,
    /// `document.rs` `SurfaceArrays`); `to_bmesh` flips it into the builder's Godot V-down UVs.
    pub uv0: Vec<[f32; 2]>,
    /// `mesh.colors`: one RGBA (0..1) per record, if present.
    pub colors: Option<Vec<[f32; 4]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XImportOptions {
    /// Keep the stored `uv0` of every face as manual UVs (exactly what Xogot showed). When off,
    /// only `manualUV` faces keep their UVs and the others use the builder's automatic
    /// projection with the face's offset / rotation / scale / world settings, which does not
    /// match Xogot's projection exactly but follows later edits.
    pub freeze_uvs: bool,
}

impl Default for XImportOptions {
    fn default() -> Self {
        XImportOptions { freeze_uvs: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct XImport {
    pub mesh: BMesh,
    /// `surface_of_slot[slot]` = the ArrayMesh surface index whose material belongs to builder
    /// material slot `slot` (None when no surface uses it). Builder face materials are the Xogot
    /// `submeshIndex`; `surfaceSlots` maps surface → slot, this is the inverse.
    pub surface_of_slot: Vec<Option<usize>>,
    /// Faces whose triangles did not outline one loop and were kept as separate triangles.
    pub split_faces: usize,
    /// Faces that would have made the mesh non-manifold and were given their own vertices.
    pub detached_faces: usize,
    /// Degenerate triangles dropped.
    pub dropped_triangles: usize,
}

fn rgba8(c: [f32; 4]) -> [u8; 4] {
    c.map(|v| if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0).round() as u8 })
}

/// Validates the arrays (never panics on damaged metadata).
fn check(x: &XMesh) -> Result<(), String> {
    if !(2..=FORMAT_VERSION).contains(&x.version) {
        return Err(format!("Unsupported Xogot Modeler snapshot version {}.", x.version));
    }
    let n = x.positions.len();
    if x.uv0.len() != n {
        return Err("The Xogot mesh has a uv0 array of the wrong length.".into());
    }
    if x.colors.as_ref().is_some_and(|c| c.len() != n) {
        return Err("The Xogot mesh has a colors array of the wrong length.".into());
    }
    if x.positions.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
        return Err("The Xogot mesh has invalid positions.".into());
    }
    if x.uv0.iter().any(|t| t.iter().any(|c| !c.is_finite())) {
        return Err("The Xogot mesh has invalid UVs.".into());
    }
    for (i, f) in x.faces.iter().enumerate() {
        if f.indices.len() % 3 != 0 || f.indices.iter().any(|r| *r as usize >= n) {
            return Err(format!("Xogot face {i} has invalid indices."));
        }
        if f.submesh_index < 0 || f.submesh_index >= SURFACE_SLOT_LIMIT || f.smoothing_group < 0 {
            return Err(format!("Xogot face {i} has invalid attributes."));
        }
    }
    if x.shared_vertices.iter().flatten().any(|r| *r as usize >= n) {
        return Err("The Xogot mesh has invalid shared vertices.".into());
    }
    if x.surface_slots.iter().any(|s| *s < 0 || *s >= SURFACE_SLOT_LIMIT) {
        return Err("The Xogot mesh has invalid surface slots.".into());
    }
    Ok(())
}

/// Outline of a triangle list as directed edges (port of xogot-rust
/// `modeler_kit/src/editable_mesh.rs` `Face::directed_perimeter`: edges used once).
fn directed_perimeter(tris: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
    for t in tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *counts.entry(edge(a, b)).or_default() += 1;
        }
    }
    tris.iter().flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]).filter(|(a, b)| counts[&edge(*a, *b)] == 1).collect()
}

/// The face's outline as one loop, when its triangles outline exactly one loop through every
/// vertex they use (port of the loop walk in xogot-rust `MeshExporter::export_polygons`, with
/// the extra "no interior vertex" check so fans around a centre vertex do not lose it).
fn outline(tris: &[[u32; 3]]) -> Option<Vec<u32>> {
    let loops = chain_loops(&directed_perimeter(tris))?;
    let used: BTreeSet<u32> = tris.iter().flatten().copied().collect();
    (loops.len() == 1 && loops[0].len() >= 3 && loops[0].len() == used.len()).then(|| loops.into_iter().next().unwrap())
}

/// Builds the builder mesh from a decoded Xogot snapshot.
pub fn to_bmesh(x: &XMesh, opt: &XImportOptions) -> Result<XImport, String> {
    check(x)?;
    let n = x.positions.len();
    // v2 stored manual UVs with V pointing down (`SnapshotCodec::migrate_version2`): bring them
    // to the v3+ V-up convention first; everything is flipped to Godot's V-down below.
    let mut uv0 = x.uv0.clone();
    if x.version == 2 {
        let manual: BTreeSet<u32> = x.faces.iter().filter(|f| f.manual_uv).flat_map(|f| f.indices.iter().copied()).collect();
        for r in manual {
            uv0[r as usize][1] = 1.0 - uv0[r as usize][1];
        }
    }
    // Record -> logical vertex: one vertex per shared group, ungrouped records stand alone.
    let mut vert_of = vec![u32::MAX; n];
    let mut out = XImport::default();
    let m = &mut out.mesh;
    for g in &x.shared_vertices {
        let Some(first) = g.iter().find(|r| vert_of[**r as usize] == u32::MAX) else { continue };
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
    for xf in &x.faces {
        // Triangles over logical vertices, plus the record each vertex came from in this face.
        let mut rec_of: BTreeMap<u32, u32> = BTreeMap::new();
        let mut tris = Vec::new();
        for t in xf.indices.chunks(3) {
            let v = [vert_of[t[0] as usize], vert_of[t[1] as usize], vert_of[t[2] as usize]];
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
            // A loop that reuses a directed edge, or a third face on an edge, would break
            // `validate`: give it its own vertex copies.
            let es: Vec<(u32, u32)> = (0..lp.len()).map(|i| (lp[i], lp[(i + 1) % lp.len()])).collect();
            if es.iter().any(|e| directed.contains(e) || undirected.get(&edge(e.0, e.1)).copied().unwrap_or(0) >= 2) {
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
            f.material = xf.submesh_index as u16;
            f.smooth = xf.smoothing_group.min(u16::MAX as i64) as u16;
            f.uv = UvParams {
                offset: v2(xf.uv.offset[0], xf.uv.offset[1]),
                rotation: xf.uv.rotation,
                scale: v2(xf.uv.scale[0], xf.uv.scale[1]),
                world: xf.uv.use_world_space,
            };
            if xf.manual_uv || opt.freeze_uvs {
                f.uv_manual = true;
                f.uv_coords = recs.iter().map(|r| v2(uv0[*r][0], 1.0 - uv0[*r][1])).collect();
            }
            if let Some(c) = &x.colors {
                let cc: Vec<[u8; 4]> = recs.iter().map(|r| rgba8(c[*r])).collect();
                if cc.iter().any(|c| *c != [255; 4]) {
                    f.color_corners = cc;
                }
            }
            m.faces.push(f);
        }
    }
    m.compact();
    m.validate()?;
    let top = m.faces.iter().map(|f| f.material as usize + 1).max().unwrap_or(0);
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

    /// The default cube as Xogot stores it (positions / faces of the `cube_basic` parity dump):
    /// four records per side, each side two triangles `[0,1,2, 0,2,3]`.
    pub fn cube() -> XMesh {
        let positions: Vec<[f32; 3]> = vec![
            [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
            [-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5],
            [-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5],
            [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5],
        ];
        let faces = (0..6u32)
            .map(|s| {
                let b = s * 4;
                XFace { indices: vec![b, b + 1, b + 2, b, b + 2, b + 3], submesh_index: (s == 5) as i64, ..Default::default() }
            })
            .collect();
        // Group records by position (what `rebuild_shared_vertices` produces).
        let mut groups: BTreeMap<[i32; 3], Vec<u32>> = BTreeMap::new();
        for (i, p) in positions.iter().enumerate() {
            groups.entry(p.map(|c| (c * 2.0) as i32)).or_default().push(i as u32);
        }
        let uv0 = (0..24).map(|i| [(i % 2) as f32, (i / 2 % 2) as f32]).collect();
        XMesh { version: 4, surface_slots: vec![0, 1], positions, faces, shared_vertices: groups.into_values().collect(), uv0, colors: None }
    }

    #[test]
    fn cube_decodes_to_closed_six_quads() {
        let r = to_bmesh(&cube(), &XImportOptions::default()).unwrap();
        let m = &r.mesh;
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.faces.len(), 6);
        assert_eq!(m.positions.len(), 8);
        assert!(m.faces.iter().all(|f| f.verts.len() == 4));
        assert!((m.volume() - 1.0).abs() < 1e-5, "outward winding: {}", m.volume());
        assert_eq!(m.faces[5].material, 1);
        assert_eq!(r.surface_of_slot, vec![Some(0), Some(1)]);
        assert!(m.faces.iter().all(|f| f.manual_uvs().is_some()), "uv0 frozen by default");
        assert_eq!((r.split_faces, r.detached_faces, r.dropped_triangles), (0, 0, 0));
    }

    #[test]
    fn manual_uvs_colors_and_smoothing_follow_records() {
        let mut x = cube();
        x.faces[0].manual_uv = true;
        x.faces[0].smoothing_group = 3;
        x.uv0[2] = [0.25, 0.75];
        let mut colors = vec![[1.0; 4]; 24];
        colors[1] = [1.0, 0.0, 0.0, 0.5];
        x.colors = Some(colors);
        let r = to_bmesh(&x, &XImportOptions { freeze_uvs: false }).unwrap();
        let f = &r.mesh.faces[0];
        assert_eq!(f.smooth, 3);
        let uvs = f.manual_uvs().unwrap();
        // Record 2 is the corner at (0.5, 0.5, -0.5).
        let ci = f.verts.iter().position(|v| r.mesh.positions[*v as usize] == v3(0.5, 0.5, -0.5)).unwrap();
        assert_eq!(uvs[ci], v2(0.25, 0.25), "V-up uv0 -> Godot V-down");
        let cj = f.verts.iter().position(|v| r.mesh.positions[*v as usize] == v3(-0.5, 0.5, -0.5)).unwrap();
        assert_eq!(f.color_corners[cj], [255, 0, 0, 128]);
        assert!(r.mesh.faces[1].manual_uvs().is_none() && r.mesh.faces[1].color_corners.is_empty());
        // v2 flips V of manual records.
        x.version = 2;
        let r = to_bmesh(&x, &XImportOptions { freeze_uvs: false }).unwrap();
        assert_eq!(r.mesh.faces[0].manual_uvs().unwrap()[ci], v2(0.25, 0.75));
    }

    #[test]
    fn fan_with_centre_vertex_stays_triangles() {
        // A quad fanned around an interior centre record: no single loop covers the centre.
        let x = XMesh {
            version: 4,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.5, 0.0, -0.5]],
            faces: vec![XFace { indices: vec![0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4], ..Default::default() }],
            uv0: vec![[0.0; 2]; 5],
            ..Default::default()
        };
        let r = to_bmesh(&x, &XImportOptions::default()).unwrap();
        assert_eq!(r.mesh.faces.len(), 4);
        assert_eq!(r.split_faces, 1);
        assert!((0..r.mesh.faces.len()).all(|f| r.mesh.face_normal(f).y > 0.9));
    }

    #[test]
    fn non_manifold_faces_are_detached() {
        let mut x = cube();
        let dup = x.faces[0].clone();
        x.faces.push(dup);
        let r = to_bmesh(&x, &XImportOptions::default()).unwrap();
        r.mesh.validate().unwrap();
        assert_eq!(r.detached_faces, 1);
        assert_eq!(r.mesh.faces.len(), 7);
    }

    #[test]
    fn damaged_metadata_is_an_error() {
        let mut x = cube();
        x.faces[0].indices.push(99);
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = cube();
        x.faces[0].indices = vec![0, 1, 200];
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = cube();
        x.uv0.pop();
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = cube();
        x.version = 9;
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        let mut x = cube();
        x.shared_vertices.push(vec![1000]);
        assert!(to_bmesh(&x, &XImportOptions::default()).is_err());
        assert!(to_bmesh(&XMesh { version: 4, ..Default::default() }, &XImportOptions::default()).unwrap().mesh.faces.is_empty());
    }
}
