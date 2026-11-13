//! Import (triangle soup -> editable n-gons) and export (Wavefront OBJ) of builder meshes.

use super::compile::CompileOptions;
use super::model::{BMesh, Face, edge};
use super::ops::weld_by_distance;
use super::topo::{chain_loops, region_boundary};
use super::uv::auto_uv;
use crate::math::*;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

/// Builds a mesh from Godot-wound (clockwise front) indexed triangles, one material slot per
/// triangle. Coincident vertices are welded (split UV/normal seams join up again) and adjacent
/// coplanar triangles of the same slot are dissolved into n-gons.
pub fn from_triangles(positions: &[Vec3], tris: &[[u32; 3]], slots: &[u16], weld: f32) -> BMesh {
    let mut m = BMesh { positions: positions.to_vec(), faces: Vec::new() };
    for (i, t) in tris.iter().enumerate() {
        if t.iter().any(|v| *v as usize >= positions.len()) || t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        let mut f = Face::new(vec![t[0], t[2], t[1]]);
        f.material = slots.get(i).copied().unwrap_or(0);
        m.faces.push(f);
    }
    weld_by_distance(&mut m, weld.max(1e-6), None);
    // Drop degenerate (zero-area) and duplicate faces that welding can produce.
    let mut seen = BTreeSet::new();
    m.faces.retain(|f| {
        let mut k = f.verts.clone();
        k.sort();
        seen.insert(k)
    });
    let keep: Vec<bool> = (0..m.faces.len()).map(|f| m.face_area(f) > 1e-9).collect();
    let mut i = 0;
    m.faces.retain(|_| {
        i += 1;
        keep[i - 1]
    });
    // Two triangles running the same edge in the same direction would make the mesh invalid
    // (non-manifold source): keep the first.
    let mut directed = BTreeSet::new();
    m.faces.retain(|f| {
        let es: Vec<(u32, u32)> = f.loop_edges().collect();
        if es.iter().any(|e| directed.contains(e)) {
            return false;
        }
        directed.extend(es);
        true
    });
    m.compact();
    dissolve_coplanar(&mut m, 0.5);
    m
}

/// Merges groups of edge-connected faces that share a plane (within `max_angle_deg`) and a
/// material into single faces where their outline is one loop.
pub fn dissolve_coplanar(m: &mut BMesh, max_angle_deg: f32) {
    let cos = max_angle_deg.to_radians().cos();
    let n = m.faces.len();
    let normals: Vec<Vec3> = (0..n).map(|f| m.face_normal(f)).collect();
    let centers: Vec<Vec3> = (0..n).map(|f| m.face_center(f)).collect();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
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
        let same = m.faces[a].material == m.faces[b].material
            && normals[a].dot(normals[b]) >= cos
            && (centers[b] - centers[a]).dot(normals[a]).abs() < 1e-4;
        if same {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[ra] = rb;
            }
        }
    }
    let mut groups: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    for f in 0..n {
        let r = find(&mut parent, f);
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
            merged.push(m.faces[*g.first().unwrap()].with_verts(loops[0].clone()));
            for f in &g {
                consumed[*f] = true;
            }
        }
    }
    let old = std::mem::take(&mut m.faces);
    m.faces = old.into_iter().enumerate().filter(|(i, _)| !consumed[*i]).map(|(_, f)| f).collect();
    m.faces.extend(merged);
    remove_collinear(m);
    m.compact();
}

/// Drops vertices that are straight-through points of every face using them (valence-2
/// collinear vertices left behind by dissolving triangles).
fn remove_collinear(m: &mut BMesh) {
    let vf = m.vert_faces();
    let edges = m.edges();
    let mut deg = vec![0usize; m.positions.len()];
    for (a, b) in &edges {
        deg[*a as usize] += 1;
        deg[*b as usize] += 1;
    }
    let mut drop = BTreeSet::new();
    for f in &m.faces {
        let n = f.verts.len();
        for i in 0..n {
            let (u, v, w) = (f.verts[(i + n - 1) % n], f.verts[i], f.verts[(i + 1) % n]);
            if deg[v as usize] != 2 || vf[v as usize].len() > 2 {
                continue;
            }
            let (a, b) = (m.positions[v as usize] - m.positions[u as usize], m.positions[w as usize] - m.positions[v as usize]);
            if a.cross(b).length() < 1e-6 * (a.length() * b.length()).max(1e-12) {
                drop.insert(v);
            }
        }
    }
    if drop.is_empty() {
        return;
    }
    for f in &mut m.faces {
        if f.verts.iter().filter(|v| !drop.contains(v)).count() >= 3 {
            f.verts.retain(|v| !drop.contains(v));
        }
    }
    let _ = edge;
}

/// Wavefront OBJ text. Faces keep their n-gon loops (OBJ is CCW-front like `BMesh`), UVs are
/// the compiled auto-UVs (V flipped to OBJ's bottom-up convention), normals are flat per face.
/// `material_names[slot]` names each `usemtl` (default `slot_N`).
pub fn to_obj(m: &BMesh, opt: &CompileOptions, object_name: &str, mtllib: Option<&str>, material_names: &[String]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Marching Squares Terrain builder mesh");
    if let Some(l) = mtllib {
        let _ = writeln!(s, "mtllib {l}");
    }
    let _ = writeln!(s, "o {}", if object_name.is_empty() { "builder" } else { object_name });
    for p in &m.positions {
        let _ = writeln!(s, "v {} {} {}", p.x, p.y, p.z);
    }
    let mut order: Vec<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].verts.len() >= 3).collect();
    order.sort_by_key(|f| m.faces[*f].material);
    let mut vt = 0usize;
    let mut body = String::new();
    let mut uvs = String::new();
    let mut nrm = String::new();
    let mut cur: Option<u16> = None;
    for (k, &fi) in order.iter().enumerate() {
        let f = &m.faces[fi];
        if cur != Some(f.material) {
            cur = Some(f.material);
            let name = material_names.get(f.material as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("slot_{}", f.material));
            let _ = writeln!(body, "usemtl {name}");
        }
        let n = m.face_normal(fi);
        let _ = writeln!(nrm, "vn {} {} {}", n.x, n.y, n.z);
        let mut line = String::from("f");
        let manual = f.manual_uvs();
        for (ci, v) in f.verts.iter().enumerate() {
            let p = m.positions[*v as usize];
            let up = if f.uv.world { p + opt.world_offset } else { p };
            // Manual (UV editor) coordinates win, like Unibuilder's `export_obj.gd`.
            let uv = manual.map_or_else(|| auto_uv(up, f.uv_normal(n), &f.uv, opt.units_per_repeat), |c| c[ci]);
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
    use super::super::compile::compile;
    use super::super::shapes::*;
    use super::*;

    /// Compiled (split-vertex, Godot-wound) triangles of a mesh, like a MeshInstance3D has.
    fn soup(m: &BMesh) -> (Vec<Vec3>, Vec<[u32; 3]>, Vec<u16>) {
        let c = compile(m, &CompileOptions::default());
        let (mut p, mut t, mut s) = (Vec::new(), Vec::new(), Vec::new());
        for surf in &c.surfaces {
            let base = p.len() as u32;
            p.extend(surf.positions.iter().map(|q| v3(q[0], q[1], q[2])));
            for tri in surf.indices.chunks(3) {
                t.push([base + tri[0] as u32, base + tri[1] as u32, base + tri[2] as u32]);
                s.push(surf.material);
            }
        }
        (p, t, s)
    }

    #[test]
    fn triangulated_box_converts_back_to_six_quads() {
        let src = cuboid(v3(2.0, 1.0, 1.0));
        let (p, t, s) = soup(&src);
        assert_eq!(t.len(), 12);
        let m = from_triangles(&p, &t, &s, 1e-4);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.faces.len(), 6);
        assert_eq!(m.positions.len(), 8);
        assert!((m.volume() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn convert_keeps_material_borders_and_curves() {
        let mut src = cuboid(v3(1.0, 1.0, 1.0));
        src.faces[0].material = 3;
        let (p, t, s) = soup(&src);
        let m = from_triangles(&p, &t, &s, 1e-4);
        assert_eq!(m.faces.len(), 6);
        assert_eq!(m.faces.iter().filter(|f| f.material == 3).count(), 1);
        let cyl = cylinder(v3(1.0, 1.0, 1.0), 10, false);
        let (p, t, s) = soup(&cyl);
        let m = from_triangles(&p, &t, &s, 1e-4);
        assert!(m.is_closed());
        assert_eq!(m.faces.len(), 12);
    }

    #[test]
    fn obj_export_has_ngons_and_materials() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.faces[2].material = 1;
        let obj = to_obj(&m, &CompileOptions::default(), "box", Some("box.mtl"), &["stone".into()]);
        assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 8);
        assert_eq!(obj.lines().filter(|l| l.starts_with("f ")).count(), 6);
        assert_eq!(obj.lines().filter(|l| l.starts_with("vt ")).count(), 24);
        assert!(obj.contains("usemtl stone") && obj.contains("usemtl slot_1") && obj.contains("mtllib box.mtl"));
        assert!(obj.lines().filter(|l| l.starts_with("f ")).all(|l| l.split_whitespace().count() == 5));
        let mtl = to_mtl(&[("stone".into(), [0.5, 0.5, 0.5], Some("stone.png".into()))]);
        assert!(mtl.contains("newmtl stone") && mtl.contains("map_Kd stone.png"));
    }
}
