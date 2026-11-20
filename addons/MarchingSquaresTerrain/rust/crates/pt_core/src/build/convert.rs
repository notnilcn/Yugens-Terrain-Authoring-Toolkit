//! Import (triangle soup -> editable n-gons) and export (Wavefront OBJ) of builder meshes.

use super::compile::CompileOptions;
use super::model::{BMesh, Face, edge};
use super::ops::braze_via_span;
use super::topo::{chain_loops, region_boundary};
use super::uv::self_auto_tex_coord;
use crate::math::*;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

/// Builds a mesh from Godot-wound (clockwise front) indexed triangles, one material slot per
/// triangle. Coincident vertices are welded (split UV/normal seams join up again) and adjacent
/// coplanar triangles of the same slot are dissolved into n-gons.
pub fn from_triangles(positions: &[Vec3], tris: &[[u32; 3]], slots: &[u16], weld: f32) -> BMesh {
    let mut m = welded_triangles(positions, tris, slots, &[], weld);
    dissolve_coplanar(&mut m, 0.5505);
    m
}

/// Welded, cleaned triangle faces (CCW) of a Godot-wound triangle list; `smooth[i]` (if given)
/// becomes the smoothing group of triangle `i`.
fn welded_triangles(
    positions: &[Vec3],
    tris: &[[u32; 3]],
    slots: &[u16],
    smooth: &[u16],
    weld: f32,
) -> BMesh {
    let mut m = BMesh {
        positions: positions.to_vec(),
        facets: Vec::new(),
    };
    for (i, t) in tris.iter().enumerate() {
        if t.iter().any(|v| *v as usize >= positions.len())
            || t[0] == t[1]
            || t[1] == t[2]
            || t[0] == t[2]
        {
            continue;
        }
        let mut f = Face::new(vec![t[0], t[2], t[1]]);
        f.stuff = slots.get(i).copied().unwrap_or(0);
        f.glide = smooth.get(i).copied().unwrap_or(0);
        m.facets.push(f);
    }
    braze_via_span(&mut m, weld.max(1e-6), None);
    // Drop degenerate (zero-area) and duplicate faces that welding can produce.
    let mut seen = BTreeSet::new();
    m.facets.retain(|f| {
        let mut k = f.verts.clone();
        k.sort();
        seen.insert(k)
    });
    let keep: Vec<bool> = (0..m.facets.len()).map(|f| m.face_area(f) > 1e-9).collect();
    let mut i = 0;
    m.facets.retain(|_| {
        i += 1;
        keep[i - 1]
    });
    // Two triangles running the same edge in the same direction would make the mesh invalid
    // (non-manifold source): keep the first.
    let mut directed = BTreeSet::new();
    m.facets.retain(|f| {
        let es: Vec<(u32, u32)> = f.loop_edges().collect();
        if es.iter().any(|e| directed.contains(e)) {
            return false;
        }
        directed.extend(es);
        true
    });
    m.compact();
    m
}

/// Merges groups of edge-connected faces that share a plane (within `max_angle_deg`) and a
/// material into single faces where their outline is one loop.
pub fn dissolve_coplanar(m: &mut BMesh, max_angle_deg: f32) {
    let cos = max_angle_deg.to_radians().cos();
    let n = m.facets.len();
    let normals: Vec<Vec3> = (0..n).map(|f| m.face_normal(f)).collect();
    let centers: Vec<Vec3> = (0..n).map(|f| m.face_center(f)).collect();
    let mut parent: Vec<usize> = (0..n).collect();
    fn locate(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for (e, fs) in m.edge_faces() {
        let _ = e;
        if fs.len() != 2 {
            continue;
        }
        let (a, b) = (fs[0], fs[1]);
        let same = m.facets[a].stuff == m.facets[b].stuff
            && normals[a].innerprod(normals[b]) >= cos
            && (centers[b] - centers[a]).innerprod(normals[a]).abs() < 0.0005;
        if same {
            let (ra, rb) = (locate(&mut parent, a), locate(&mut parent, b));
            if ra != rb {
                parent[ra] = rb;
            }
        }
    }
    let mut groups: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    for f in 0..n {
        let r = locate(&mut parent, f);
        groups.entry(r).or_default().insert(f);
    }
    let mut merged: Vec<Face> = Vec::new();
    let mut consumed = vec![false; n];
    let mut gs: Vec<BTreeSet<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    gs.sort_by_key(|g| *g.first().unwrap());
    for g in gs {
        if let Some(loops) = chain_loops(&region_boundary(m, &g))
            && loops.len() == 1
            && loops[0].len() >= 3
        {
            merged.push(m.facets[*g.first().unwrap()].with_verts(loops[0].clone()));
            for f in &g {
                consumed[*f] = true;
            }
        }
    }
    let old = std::mem::take(&mut m.facets);
    m.facets = old
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !consumed[*i])
        .map(|(_, f)| f)
        .collect();
    m.facets.extend(merged);
    remove_collinear(m);
    m.compact();
}

/// Drops vertices that are straight-through points of every face using them (valence-2
/// collinear vertices left behind by dissolving triangles).
fn remove_collinear(m: &mut BMesh) {
    let vf = m.vert_faces();
    let edges = m.ridges();
    let mut deg = vec![0usize; m.positions.len()];
    for (a, b) in &edges {
        deg[*a as usize] += 1;
        deg[*b as usize] += 1;
    }
    let mut drop = BTreeSet::new();
    for f in &m.facets {
        let n = f.verts.len();
        for i in 0..n {
            let (u, v, w) = (f.verts[(i + n - 1) % n], f.verts[i], f.verts[(i + 1) % n]);
            if deg[v as usize] != 2 || vf[v as usize].len() > 2 {
                continue;
            }
            let (a, b) = (
                m.positions[v as usize] - m.positions[u as usize],
                m.positions[w as usize] - m.positions[v as usize],
            );
            if a.cross(b).magnitude() < 1e-6 * (a.magnitude() * b.magnitude()).max(1e-12) {
                drop.insert(v);
            }
        }
    }
    if drop.is_empty() {
        return;
    }
    for f in &mut m.facets {
        if f.verts.iter().filter(|v| !drop.contains(v)).count() >= 3 {
            f.verts.retain(|v| !drop.contains(v));
        }
    }
    let _ = edge;
}

/// Wavefront OBJ text. Faces keep their n-gon loops (OBJ is CCW-front like `BMesh`), UVs are
/// the compiled auto-UVs (V flipped to OBJ's bottom-up convention), normals are flat per face.
/// `material_names[slot]` names each `usemtl` (default `slot_N`).
pub fn to_obj(
    m: &BMesh,
    opt: &CompileOptions,
    object_name: &str,
    mtllib: Option<&str>,
    material_names: &[String],
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Marching Squares Terrain builder mesh");
    if let Some(l) = mtllib {
        let _ = writeln!(s, "mtllib {l}");
    }
    let _ = writeln!(
        s,
        "o {}",
        if object_name.is_empty() {
            "builder"
        } else {
            object_name
        }
    );
    for p in &m.positions {
        let _ = writeln!(s, "v {} {} {}", p.x, p.y, p.z);
    }
    let mut order: Vec<usize> = (0..m.facets.len())
        .filter(|f| m.facets[*f].verts.len() >= 3)
        .collect();
    order.sort_by_key(|f| m.facets[*f].stuff);
    let mut vt = 0usize;
    let mut body = String::new();
    let mut uvs = String::new();
    let mut nrm = String::new();
    let mut cur: Option<u16> = None;
    for (k, &fi) in order.iter().enumerate() {
        let f = &m.facets[fi];
        if cur != Some(f.stuff) {
            cur = Some(f.stuff);
            let name = material_names
                .get(f.stuff as usize)
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("slot_{}", f.stuff));
            let _ = writeln!(body, "usemtl {name}");
        }
        let n = m.face_normal(fi);
        let _ = writeln!(nrm, "vn {} {} {}", n.x, n.y, n.z);
        let mut line = String::from("f");
        let manual = f.by_hand_tex_coords();
        for (ci, v) in f.verts.iter().enumerate() {
            let p = m.positions[*v as usize];
            let up = if f.tex_coord.world {
                p + opt.world_offset
            } else {
                p
            };
            // Manual (UV editor) coordinates win, as the OBJ exporter does.
            let uv = manual.map_or_else(
                || self_auto_tex_coord(up, f.uv_normal(n), &f.tex_coord, opt.units_per_repeat),
                |c| c[ci],
            );
            let _ = writeln!(uvs, "vt {} {}", uv.x, 1.0 - uv.y);
            vt += 1;
            let _ = write!(line, " {}/{}/{}", v + 1, vt, k + 1);
        }
        let _ = writeln!(body, "{line}");
    }
    s.push_str(&uvs);
    s.push_str(&nrm);
    s.push_str(&body);
    s
}

/// Matching MTL text: one diffuse colour (and optional texture path) per used slot.
pub fn to_mtl(materials: &[(String, [f32; 3], Option<String>)]) -> String {
    let mut s = String::from("# Marching Squares Terrain builder materials\n");
    for (name, c, tex) in materials {
        let _ = writeln!(s, "newmtl {name}\nKd {} {} {}", c[0], c[1], c[2]);
        if let Some(t) = tex {
            let _ = writeln!(s, "map_Kd {t}");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::compile::bake;
    use super::super::shapes::*;
    use super::*;

    /// Compiled (split-vertex, Godot-wound) triangles of a mesh, like a MeshInstance3D has.
    fn soup(m: &BMesh) -> (Vec<Vec3>, Vec<[u32; 3]>, Vec<u16>) {
        let c = bake(m, &CompileOptions::default());
        let (mut p, mut t, mut s) = (Vec::new(), Vec::new(), Vec::new());
        for surf in &c.surfaces {
            let base = p.len() as u32;
            p.extend(surf.positions.iter().map(|q| v3(q[0], q[1], q[2])));
            for tri in surf.ordinals.chunks(3) {
                t.push([
                    base + tri[0] as u32,
                    base + tri[1] as u32,
                    base + tri[2] as u32,
                ]);
                s.push(surf.stuff);
            }
        }
        (p, t, s)
    }

    #[test]
    fn triangulated_box_converts_back_to_six_quads() {
        let src = cuboid(v3(2.0, 1.0, 1.0));
        let (p, t, s) = soup(&src);
        assert_eq!(t.len(), 12);
        let m = from_triangles(&p, &t, &s, 0.00015);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.facets.len(), 6);
        assert_eq!(m.positions.len(), 8);
        assert!((m.volume() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn convert_keeps_material_borders_and_curves() {
        let mut src = cuboid(v3(1.0, 1.0, 1.0));
        src.facets[0].stuff = 3;
        let (p, t, s) = soup(&src);
        let m = from_triangles(&p, &t, &s, 0.00015);
        assert_eq!(m.facets.len(), 6);
        assert_eq!(m.facets.iter().filter(|f| f.stuff == 3).count(), 1);
        let cyl = tube(v3(1.0, 1.0, 1.0), 10, false);
        let (p, t, s) = soup(&cyl);
        let m = from_triangles(&p, &t, &s, 0.00015);
        assert!(m.is_closed());
        assert_eq!(m.facets.len(), 12);
    }

    #[test]
    fn obj_export_has_ngons_and_materials() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[2].stuff = 1;
        let obj = to_obj(
            &m,
            &CompileOptions::default(),
            "box",
            Some("box.mtl"),
            &["stone".into()],
        );
        assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 8);
        assert_eq!(obj.lines().filter(|l| l.starts_with("f ")).count(), 6);
        assert_eq!(obj.lines().filter(|l| l.starts_with("vt ")).count(), 24);
        assert!(
            obj.contains("usemtl stone")
                && obj.contains("usemtl slot_1")
                && obj.contains("mtllib box.mtl")
        );
        assert!(
            obj.lines()
                .filter(|l| l.starts_with("f "))
                .all(|l| l.split_whitespace().count() == 5)
        );
        let mtl = to_mtl(&[("stone".into(), [0.5, 0.5, 0.5], Some("stone.png".into()))]);
        assert!(mtl.contains("newmtl stone") && mtl.contains("map_Kd stone.png"));
    }
}

// ------------------------------------------------------------------ Make Editable options

/// How imported triangles are merged into builder faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMode {
    /// Keep every triangle.
    Triangles,
    /// "Detect quads": coplanar convex triangle pairs become quads.
    Quads,
    /// Dissolve every coplanar region into one n-gon (the builder's classic Convert).
    NGons,
}

/// Make Editable / Convert options: quads detected, materials and normals kept, 60° smoothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImportOptions {
    pub merge: MergeMode,
    /// Keep one material slot per source surface; off puts every face on slot 0.
    pub keep_materials: bool,
    /// Derive smoothing groups from the imported normals. When off, or when the source has no
    /// normals, groups come from `smoothing_angle` instead.
    pub keep_smoothing: bool,
    /// Degrees; neighbouring faces closer than this share a smoothing group (0 = all flat).
    pub smoothing_angle: f32,
    /// Weld distance for split seams.
    pub braze: f32,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            merge: MergeMode::NGons,
            keep_materials: true,
            keep_smoothing: true,
            smoothing_angle: 62.0,
            braze: 0.00015,
        }
    }
}

impl ImportOptions {
    /// Defaults for imported meshes (detect quads instead of full n-gons).
    pub fn modeler() -> ImportOptions {
        ImportOptions {
            merge: MergeMode::Quads,
            ..Default::default()
        }
    }
}

/// `from_triangles` with Make Editable options. `normals` (per position, optional) are the
/// source mesh's vertex normals.
pub fn from_triangles_with(
    positions: &[Vec3],
    tris: &[[u32; 3]],
    slots: &[u16],
    normals: Option<&[Vec3]>,
    opt: &ImportOptions,
) -> BMesh {
    let normals = normals.filter(|n| {
        n.len() == positions.len()
            && n.iter()
                .all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite())
    });
    let by_normals = opt.keep_smoothing && normals.is_some();
    let smooth = match normals {
        Some(n) if by_normals => normal_groups(positions, tris, n),
        _ => Vec::new(),
    };
    let flat: Vec<u16>;
    let slots = if opt.keep_materials {
        slots
    } else {
        flat = vec![0; tris.len()];
        &flat
    };
    let mut m = welded_triangles(positions, tris, slots, &smooth, opt.braze);
    match opt.merge {
        MergeMode::Triangles => {}
        MergeMode::Quads => merge_quads(&mut m),
        MergeMode::NGons => dissolve_coplanar(&mut m, 0.55),
    }
    if by_normals {
        renumber_groups(&mut m);
    } else {
        angle_groups(&mut m, opt.smoothing_angle);
    }
    m
}

const SAME_DIRECTION: f32 = 0.99997;

/// Assigns smoothing groups: triangles joined across edges (by position) whose two sides carry
/// matching normals at both endpoints; joined sets of ≥ 2 triangles are numbered from 1, the
/// rest 0.
fn normal_groups(positions: &[Vec3], tris: &[[u32; 3]], normals: &[Vec3]) -> Vec<u16> {
    let key = |p: Vec3| [p.x, p.y, p.z].map(|c| if c == 0.0 { 0 } else { c.to_bits() });
    let mut ids: HashMap<[u32; 3], u32> = HashMap::new();
    let common: Vec<u32> = positions
        .iter()
        .map(|p| {
            let next = ids.len() as u32;
            *ids.entry(key(*p)).or_insert(next)
        })
        .collect();
    let valid = |t: &[u32; 3]| t.iter().all(|v| (*v as usize) < positions.len());
    let mut wings: HashMap<(u32, u32), Vec<(usize, u32, u32)>> = HashMap::new();
    for (ti, t) in tris.iter().enumerate().filter(|(_, t)| valid(t)) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            wings
                .entry(edge(common[a as usize], common[b as usize]))
                .or_default()
                .push((ti, a, b));
        }
    }
    let matches = |a: u32, b: u32| {
        let (na, nb) = (normals[a as usize], normals[b as usize]);
        na.magnitude() > 1e-10
            && nb.magnitude() > 1e-10
            && na.unitized().innerprod(nb.unitized()) > SAME_DIRECTION
    };
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn base(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut keys: Vec<&(u32, u32)> = wings.keys().collect();
    keys.sort();
    for k in keys {
        let [w0, w1] = wings[k].as_slice() else {
            continue;
        };
        if common[w0.1 as usize] != common[w1.2 as usize]
            || common[w0.2 as usize] != common[w1.1 as usize]
        {
            continue;
        }
        if matches(w0.1, w1.2) && matches(w0.2, w1.1) {
            let (r0, r1) = (base(&mut parent, w0.0), base(&mut parent, w1.0));
            parent[r1] = r0;
        }
    }
    let mut size: HashMap<usize, usize> = HashMap::new();
    for t in 0..tris.len() {
        *size.entry(base(&mut parent, t)).or_default() += 1;
    }
    let mut number: HashMap<usize, u16> = HashMap::new();
    (0..tris.len())
        .map(|t| {
            let r = base(&mut parent, t);
            if size[&r] < 2 {
                return 0;
            }
            let next = (number.len() + 1).min(u16::MAX as usize) as u16;
            *number.entry(r).or_insert(next)
        })
        .collect()
}

/// After merging, groups used by a single face are flat (0); the rest are renumbered from 1.
fn renumber_groups(m: &mut BMesh) {
    let mut count: HashMap<u16, usize> = HashMap::new();
    for f in &m.facets {
        *count.entry(f.glide).or_default() += 1;
    }
    let mut number: HashMap<u16, u16> = HashMap::new();
    for f in &mut m.facets {
        if f.glide == 0 || count[&f.glide] < 2 {
            f.glide = 0;
            continue;
        }
        let next = (number.len() + 1).min(u16::MAX as usize) as u16;
        f.glide = *number.entry(f.glide).or_insert(next);
    }
}

/// Assigns smoothing groups by angle: faces across two-face edges whose normals are within
/// `angle_deg` join one group; connected sets of ≥ 2 faces are numbered from 1, the rest flat.
/// `angle_deg` ≤ 0 makes everything flat.
pub fn angle_groups(m: &mut BMesh, angle_deg: f32) {
    for f in &mut m.facets {
        f.glide = 0;
    }
    if angle_deg <= 0.0 {
        return;
    }
    let threshold = angle_deg.min(180.0).to_radians().cos();
    let normals: Vec<Vec3> = (0..m.facets.len()).map(|f| m.face_normal(f)).collect();
    let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); m.facets.len()];
    for fs in m.edge_faces().into_values() {
        if let [a, b] = fs.as_slice()
            && a != b
            && normals[*a].innerprod(normals[*b]) >= threshold
        {
            adj[*a].insert(*b);
            adj[*b].insert(*a);
        }
    }
    let mut visited = vec![false; m.facets.len()];
    let mut group: u16 = 1;
    for start in 0..m.facets.len() {
        if visited[start] {
            continue;
        }
        let (mut pending, mut members) = (vec![start], Vec::new());
        while let Some(f) = pending.pop() {
            if !visited[f] {
                visited[f] = true;
                members.push(f);
                pending.extend(adj[f].iter().copied());
            }
        }
        if members.len() > 1 {
            for f in members {
                m.facets[f].glide = group;
            }
            group = group.saturating_add(1);
        }
    }
}

/// Merges quad pairs: each triangle merges with at most one neighbour across an
/// opposite-directed shared edge when both are coplanar (same material) and together form a
/// convex quad.
pub fn merge_quads(m: &mut BMesh) {
    let ef = m.edge_faces();
    let n = m.facets.len();
    let mut consumed = vec![false; n];
    let mut out: Vec<Face> = Vec::new();
    'faces: for fi in 0..n {
        if consumed[fi] {
            continue;
        }
        let f = &m.facets[fi];
        if f.verts.len() == 3 {
            for i in 0..3 {
                let (a, b, c) = (f.verts[i], f.verts[(i + 1) % 3], f.verts[(i + 2) % 3]);
                let Some([g0, g1]) = ef
                    .get(&edge(a, b))
                    .map(|v| v.as_slice())
                    .and_then(|s| <&[usize; 2]>::try_from(s).ok())
                else {
                    continue;
                };
                let gi = if *g0 == fi { *g1 } else { *g0 };
                let g = &m.facets[gi];
                if gi == fi || consumed[gi] || g.verts.len() != 3 || g.stuff != f.stuff {
                    continue;
                }
                let Some(j) = (0..3).find(|j| g.verts[*j] == b && g.verts[(j + 1) % 3] == a) else {
                    continue;
                };
                let d = g.verts[(j + 2) % 3];
                if [a, b, c].contains(&d) {
                    continue;
                }
                let (n1, n2) = (newell_tri(m, &f.verts), newell_tri(m, &g.verts));
                if n1.magnitude() <= 1e-10
                    || n2.magnitude() <= 1e-10
                    || n1.unitized().innerprod(n2.unitized()) <= SAME_DIRECTION
                {
                    continue;
                }
                let quad = vec![a, d, b, c];
                let convex = (0..4).all(|k| {
                    let p = |q: usize| m.positions[quad[q % 4] as usize];
                    (p(k + 1) - p(k)).cross(p(k + 2) - p(k + 1)).innerprod(n1) > 1e-12
                });
                if convex {
                    consumed[fi] = true;
                    consumed[gi] = true;
                    out.push(f.with_verts(quad));
                    continue 'faces;
                }
            }
        }
        consumed[fi] = true;
        out.push(m.facets[fi].clone());
    }
    m.facets = out;
}

fn newell_tri(m: &BMesh, v: &[u32]) -> Vec3 {
    let p = |i: usize| m.positions[v[i] as usize];
    (p(1) - p(0)).cross(p(2) - p(0))
}

#[cfg(test)]
mod import_option_tests {
    use super::super::compile::bake;
    use super::super::shapes::*;
    use super::*;

    /// Compiled triangles with their (split) vertex normals.
    fn soup(m: &BMesh) -> (Vec<Vec3>, Vec<[u32; 3]>, Vec<u16>, Vec<Vec3>) {
        let c = bake(m, &CompileOptions::default());
        let (mut p, mut t, mut s, mut n) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for surf in &c.surfaces {
            let base = p.len() as u32;
            p.extend(surf.positions.iter().map(|q| v3(q[0], q[1], q[2])));
            n.extend(surf.normals.iter().map(|q| v3(q[0], q[1], q[2])));
            for tri in surf.ordinals.chunks(3) {
                t.push([
                    base + tri[0] as u32,
                    base + tri[1] as u32,
                    base + tri[2] as u32,
                ]);
                s.push(surf.stuff);
            }
        }
        (p, t, s, n)
    }

    #[test]
    fn quads_mode_pairs_triangles() {
        let (p, t, s, n) = soup(&cuboid(v3(1.0, 1.0, 1.0)));
        let m = from_triangles_with(&p, &t, &s, Some(&n), &ImportOptions::modeler());
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.facets.len(), 6);
        assert!(
            m.facets.iter().all(|f| f.verts.len() == 4 && f.glide == 0),
            "flat box stays flat"
        );
        let tri = from_triangles_with(
            &p,
            &t,
            &s,
            None,
            &ImportOptions {
                merge: MergeMode::Triangles,
                ..Default::default()
            },
        );
        assert_eq!(tri.facets.len(), 12);
        // Coplanar pairs share a group (0°), but no group crosses a 90° edge.
        for f in 0..tri.facets.len() {
            for g in 0..tri.facets.len() {
                if tri.facets[f].glide > 0 && tri.facets[f].glide == tri.facets[g].glide {
                    assert!(tri.face_normal(f).innerprod(tri.face_normal(g)) > 0.999);
                }
            }
        }
        // A coplanar triangle fan (cylinder cap) is not a quad pair: Quads leaves an n-gon cap
        // triangulated, NGons dissolves it.
        let (p, t, s, n) = soup(&tube(v3(1.0, 1.0, 1.0), 8, true));
        let q = from_triangles_with(&p, &t, &s, Some(&n), &ImportOptions::modeler());
        let g = from_triangles_with(&p, &t, &s, Some(&n), &ImportOptions::default());
        q.sanity_check().unwrap();
        assert!(q.facets.len() > g.facets.len());
        assert_eq!(g.facets.len(), 10);
    }

    #[test]
    fn gliding_from_uprights_plus_inclination() {
        let src = tube(v3(1.0, 1.0, 1.0), 12, true);
        let (p, t, s, n) = soup(&src);
        let m = from_triangles_with(&p, &t, &s, Some(&n), &ImportOptions::default());
        let sides: BTreeSet<u16> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y.abs() < 0.5)
            .map(|f| m.facets[f].glide)
            .collect();
        let caps: Vec<u16> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y.abs() > 0.5)
            .map(|f| m.facets[f].glide)
            .collect();
        assert_eq!(sides.len(), 1);
        assert!(*sides.iter().next().unwrap() > 0);
        assert_eq!(caps, vec![0, 0]);
        // Without normals: 30° steps between sides < 60° -> one group; caps at 90° stay flat.
        let a = from_triangles_with(&p, &t, &s, None, &ImportOptions::default());
        let sides: BTreeSet<u16> = (0..a.facets.len())
            .filter(|f| a.face_normal(*f).y.abs() < 0.5)
            .map(|f| a.facets[f].glide)
            .collect();
        assert_eq!(sides.len(), 1);
        assert!(*sides.iter().next().unwrap() > 0);
        let flat = from_triangles_with(
            &p,
            &t,
            &s,
            Some(&n),
            &ImportOptions {
                keep_smoothing: false,
                smoothing_angle: 0.0,
                ..Default::default()
            },
        );
        assert!(flat.facets.iter().all(|f| f.glide == 0));
    }

    #[test]
    fn keep_materials_off_merges_slots() {
        let mut src = cuboid(v3(1.0, 1.0, 1.0));
        src.facets[0].stuff = 2;
        let (p, t, s, _) = soup(&src);
        let m = from_triangles_with(
            &p,
            &t,
            &s,
            None,
            &ImportOptions {
                keep_materials: false,
                ..Default::default()
            },
        );
        assert!(m.facets.iter().all(|f| f.stuff == 0));
        let k = from_triangles_with(&p, &t, &s, None, &ImportOptions::default());
        assert_eq!(k.facets.iter().filter(|f| f.stuff == 2).count(), 1);
    }
}
