//! Topology and attribute operations. All operations mutate a `BMesh` in place and keep it
//! valid; callers snapshot before (`blob::encode`) for undo. Functions that change the face
//! list return the indices of the faces the result should select.

use super::model::{BMesh, Edge, Face, edge};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub fn translate_verts(m: &mut BMesh, verts: &BTreeSet<u32>, d: Vec3) {
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            *p = *p + d;
        }
    }
}

/// Scales `verts` about `pivot` per axis.
pub fn scale_verts(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, s: Vec3) {
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            *p = pivot + v3(d.x * s.x, d.y * s.y, d.z * s.z);
        }
    }
}

/// Rotates `verts` about a vertical axis through `pivot` by `angle` radians.
pub fn rotate_verts_y(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, angle: f32) {
    let (s, c) = angle.sin_cos();
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            *p = pivot + v3(d.x * c + d.z * s, d.y, -d.x * s + d.z * c);
        }
    }
}

/// Extrudes a face region along the averaged face normals of each vertex. Faces that share an
/// edge move together (no inner walls); side quads are added along the region's outline.
/// Returns the moved (cap) faces.
pub fn extrude_faces(m: &mut BMesh, faces: &BTreeSet<usize>, distance: f32) -> BTreeSet<usize> {
    let faces: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    if faces.is_empty() {
        return faces;
    }
    // Directed boundary edges: edges of the region whose reverse is not in the region.
    let mut directed = BTreeSet::new();
    for f in &faces {
        for e in m.faces[*f].loop_edges() {
            directed.insert(e);
        }
    }
    let boundary: Vec<(u32, u32, usize)> = faces
        .iter()
        .flat_map(|f| m.faces[*f].loop_edges().map(move |e| (e, *f)).collect::<Vec<_>>())
        .filter(|((a, b), _)| !directed.contains(&(*b, *a)))
        .map(|((a, b), f)| (a, b, f))
        .collect();
    let mut normal_sum: BTreeMap<u32, Vec3> = BTreeMap::new();
    for f in &faces {
        let n = m.face_normal(*f);
        for v in &m.faces[*f].verts {
            let e = normal_sum.entry(*v).or_insert(Vec3::ZERO);
            *e = *e + n;
        }
    }
    let mut dup: HashMap<u32, u32> = HashMap::new();
    for (v, n) in &normal_sum {
        let p = m.positions[*v as usize] + n.normalized() * distance;
        dup.insert(*v, m.add_vert(p));
    }
    for (a, b, f) in boundary {
        let src = m.faces[f].clone();
        let side = src.with_verts(vec![a, b, dup[&b], dup[&a]]);
        m.faces.push(Face { smooth: 0, ..side });
    }
    for f in &faces {
        for v in &mut m.faces[*f].verts {
            *v = dup[v];
        }
    }
    faces
}

/// Insets every face individually by `amount` (0..1 of the way to its centre). The original
/// face becomes the inner face; a ring of quads fills the gap. Returns the inner faces.
pub fn inset_faces(m: &mut BMesh, faces: &BTreeSet<usize>, amount: f32) -> BTreeSet<usize> {
    let t = amount.clamp(0.0, 0.99);
    let mut out = BTreeSet::new();
    for &f in faces {
        if f >= m.faces.len() {
            continue;
        }
        let c = m.face_center(f);
        let old = m.faces[f].verts.clone();
        let inner: Vec<u32> = old.iter().map(|v| {
            let p = m.positions[*v as usize];
            m.add_vert(p.lerp(c, t))
        }).collect();
        let n = old.len();
        let src = m.faces[f].clone();
        for i in 0..n {
            let j = (i + 1) % n;
            m.faces.push(src.with_verts(vec![old[i], old[j], inner[j], inner[i]]));
        }
        m.faces[f].verts = inner;
        out.insert(f);
    }
    out
}

/// Removes faces (and vertices nothing else uses).
pub fn delete_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    let mut i = 0;
    m.faces.retain(|_| {
        let keep = !faces.contains(&i);
        i += 1;
        keep
    });
    m.compact();
}

/// Removes every face touching one of `verts`.
pub fn delete_verts(m: &mut BMesh, verts: &BTreeSet<u32>) {
    let faces: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].verts.iter().any(|v| verts.contains(v))).collect();
    delete_faces(m, &faces);
}

/// Removes every face using one of `edges`.
pub fn delete_edges(m: &mut BMesh, edges: &BTreeSet<Edge>) {
    let faces: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].loop_edges().any(|(a, b)| edges.contains(&edge(a, b)))).collect();
    delete_faces(m, &faces);
}

pub fn flip_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            face.reverse();
        }
    }
}

/// Makes face windings consistent within each connected part of the mesh (port of
/// Unibuilder's `UBOpConformNormals`): walks the faces across manifold edges, marks every
/// neighbour that runs a shared edge in the same direction as needing the opposite flip, and
/// per connected part keeps the majority orientation. Returns the number of faces flipped.
pub fn conform_normals(m: &mut BMesh) -> usize {
    let ef = m.edge_faces();
    let same_dir = |verts: &[u32], a: u32, b: u32| (0..verts.len()).any(|i| verts[i] == a && verts[(i + 1) % verts.len()] == b);
    let mut visited = vec![false; m.faces.len()];
    let mut to_flip = Vec::new();
    for seed in 0..m.faces.len() {
        if visited[seed] {
            continue;
        }
        let mut component = vec![seed];
        let mut flip: HashMap<usize, bool> = HashMap::from([(seed, false)]);
        visited[seed] = true;
        let mut queue = vec![seed];
        while let Some(fi) = queue.pop() {
            let verts = m.faces[fi].verts.clone();
            for i in 0..verts.len() {
                let (a, b) = (verts[i], verts[(i + 1) % verts.len()]);
                let Some(sharers) = ef.get(&edge(a, b)) else { continue };
                if sharers.len() != 2 {
                    continue;
                }
                for &nf in sharers {
                    if nf == fi || visited[nf] {
                        continue;
                    }
                    let same = same_dir(&m.faces[nf].verts, a, b);
                    let f = flip[&fi];
                    flip.insert(nf, if same { !f } else { f });
                    visited[nf] = true;
                    component.push(nf);
                    queue.push(nf);
                }
            }
        }
        let flips = component.iter().filter(|f| flip[f]).count();
        let invert = flips * 2 > component.len();
        to_flip.extend(component.into_iter().filter(|f| flip[f] != invert));
    }
    for f in &to_flip {
        m.faces[*f].reverse();
    }
    to_flip.len()
}

/// Re-indexes faces through `map` (old vertex -> new vertex), dropping repeated corners and
/// faces left with fewer than three, then compacts.
pub(crate) fn remap(m: &mut BMesh, map: &HashMap<u32, u32>) -> Vec<Option<u32>> {
    for f in &mut m.faces {
        let mut out: Vec<u32> = Vec::with_capacity(f.verts.len());
        for v in &f.verts {
            let nv = *map.get(v).unwrap_or(v);
            if out.last() != Some(&nv) {
                out.push(nv);
            }
        }
        while out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
        // A collapsed loop may still revisit a vertex (bow-tie); keep the first occurrence run.
        let mut seen = BTreeSet::new();
        out.retain(|v| seen.insert(*v));
        f.verts = out;
    }
    m.faces.retain(|f| f.verts.len() >= 3);
    m.compact()
}

/// Merges `verts` into one vertex at their centroid. Returns the merged vertex index.
pub fn merge_verts(m: &mut BMesh, verts: &BTreeSet<u32>) -> Option<u32> {
    let list: Vec<u32> = verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect();
    if list.len() < 2 {
        return list.first().copied();
    }
    let c = list.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / list.len() as f32;
    let keep = list[0];
    m.positions[keep as usize] = c;
    let map: HashMap<u32, u32> = list.iter().map(|v| (*v, keep)).collect();
    remap(m, &map)[keep as usize]
}

/// Welds vertices closer than `threshold` (only among `only` if given). Returns verts removed.
pub fn weld_by_distance(m: &mut BMesh, threshold: f32, only: Option<&BTreeSet<u32>>) -> usize {
    let th = threshold.max(1e-6);
    let cell = |p: Vec3| ((p.x / th).floor() as i64, (p.y / th).floor() as i64, (p.z / th).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
    let mut map: HashMap<u32, u32> = HashMap::new();
    for i in 0..m.positions.len() as u32 {
        if only.is_some_and(|s| !s.contains(&i)) {
            continue;
        }
        let p = m.positions[i as usize];
        let k = cell(p);
        let mut target = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(l) = grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        for &o in l {
                            if (m.positions[o as usize] - p).length() <= th {
                                target = Some(o);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        match target {
            Some(o) => {
                map.insert(i, o);
            }
            None => grid.entry(k).or_default().push(i),
        }
    }
    let n = map.len();
    if n > 0 {
        remap(m, &map);
    }
    n
}

/// Splits every selected face into quads around its centre (edges are split at their midpoints
/// in neighbouring faces too, keeping the mesh watertight). Returns the new faces.
pub fn subdivide_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let faces: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    let mut mids: HashMap<Edge, u32> = HashMap::new();
    for e in m.edges_of_faces(&faces) {
        let p = (m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5;
        mids.insert(e, m.add_vert(p));
    }
    let centers: Vec<(usize, u32)> = faces.iter().map(|f| {
        let c = m.face_center(*f);
        (*f, m.add_vert(c))
    }).collect();
    let mut new_faces: Vec<Face> = Vec::new();
    let mut result_idx = Vec::new();
    let old = std::mem::take(&mut m.faces);
    let cmap: HashMap<usize, u32> = centers.into_iter().collect();
    for (fi, f) in old.into_iter().enumerate() {
        if let Some(&c) = cmap.get(&fi) {
            let n = f.verts.len();
            for i in 0..n {
                let v = f.verts[i];
                let next = mids[&edge(v, f.verts[(i + 1) % n])];
                let prev = mids[&edge(f.verts[(i + n - 1) % n], v)];
                result_idx.push(new_faces.len());
                new_faces.push(f.with_verts(vec![v, next, c, prev]));
            }
        } else {
            // Insert midpoints of split edges into untouched neighbours.
            let mut loop_ = Vec::with_capacity(f.verts.len() * 2);
            for (a, b) in f.loop_edges() {
                loop_.push(a);
                if let Some(mv) = mids.get(&edge(a, b)) {
                    loop_.push(*mv);
                }
            }
            new_faces.push(f.with_verts(loop_));
        }
    }
    m.faces = new_faces;
    result_idx.into_iter().collect()
}

/// Splits faces into triangles (fan-free ear clipping). Port of Unibuilder
/// `core/ops/triangulate_faces.gd`: manual UVs and per-corner colours follow their corners.
pub fn triangulate_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let tris = super::compile::face_triangles(m);
    let old = std::mem::take(&mut m.faces);
    let mut out = BTreeSet::new();
    for (fi, f) in old.into_iter().enumerate() {
        if faces.contains(&fi) && f.verts.len() > 3 && !tris[fi].is_empty() {
            let manual = f.manual_uvs().is_some();
            let painted = f.corner_colors().is_some();
            for t in &tris[fi] {
                out.insert(m.faces.len());
                let mut tf = f.with_verts(t.iter().map(|k| f.verts[*k]).collect());
                if manual {
                    tf.uv_manual = true;
                    tf.uv_coords = t.iter().map(|k| f.uv_coords[*k]).collect();
                }
                if painted {
                    tf.color_corners = t.iter().map(|k| f.color_corners[*k]).collect();
                }
                m.faces.push(tf);
            }
        } else {
            if faces.contains(&fi) {
                out.insert(m.faces.len());
            }
            m.faces.push(f);
        }
    }
    out
}

/// Duplicates faces as a detached copy (own vertices). Returns the copies.
pub fn duplicate_faces(m: &mut BMesh, faces: &BTreeSet<usize>, offset: Vec3) -> BTreeSet<usize> {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut out = BTreeSet::new();
    for f in faces.iter().copied().filter(|f| *f < m.faces.len()).collect::<Vec<_>>() {
        let src = m.faces[f].clone();
        let verts = src.verts.iter().map(|v| *map.entry(*v).or_insert_with(|| {
            let p = m.positions[*v as usize] + offset;
            m.positions.push(p);
            (m.positions.len() - 1) as u32
        })).collect();
        out.insert(m.faces.len());
        m.faces.push(src.with_verts(verts));
    }
    out
}

/// Mirrors the whole mesh across the plane `x = 0` (or y/z for `axis` 1/2), keeping it outward.
pub fn mirror(m: &mut BMesh, axis: usize) {
    for p in &mut m.positions {
        match axis {
            0 => p.x = -p.x,
            1 => p.y = -p.y,
            _ => p.z = -p.z,
        }
    }
    for f in &mut m.faces {
        f.reverse();
    }
}

pub fn set_material(m: &mut BMesh, faces: &BTreeSet<usize>, slot: u16) {
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            face.material = slot;
        }
    }
}

pub fn set_smooth(m: &mut BMesh, faces: &BTreeSet<usize>, group: u16) {
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            face.smooth = group;
        }
    }
}

pub fn set_color(m: &mut BMesh, faces: &BTreeSet<usize>, color: [u8; 4]) {
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            face.color = color;
        }
    }
}

pub fn set_uv(m: &mut BMesh, faces: &BTreeSet<usize>, uv: super::model::UvParams) {
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            face.uv = uv;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn conform_normals_repairs_flipped_faces() {
        let mut m = super::super::shapes::cuboid(v3(1.0, 1.0, 1.0));
        let v0 = m.volume();
        m.faces[1].reverse();
        m.faces[4].reverse();
        assert!(!m.is_closed());
        assert_eq!(conform_normals(&mut m), 2);
        assert!(m.is_closed());
        assert!((m.volume() - v0).abs() < 1e-5);
        // Majority wins: a fully inverted box stays consistent (inside-out) but consistent.
        let mut m = super::super::shapes::cuboid(v3(1.0, 1.0, 1.0));
        for f in &mut m.faces {
            f.reverse();
        }
        assert_eq!(conform_normals(&mut m), 0);
        assert!(m.is_closed());
    }

    use super::super::shapes::*;
    use super::*;

    fn top_face(m: &BMesh) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap()
    }

    #[test]
    fn extrude_top_of_box() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let sel = extrude_faces(&mut m, &BTreeSet::from([t]), 2.0);
        assert_eq!(m.faces.len(), 10);
        assert_eq!(m.positions.len(), 12);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 3.0).abs() < 1e-4);
        assert!((m.face_center(*sel.first().unwrap()).y - 3.0).abs() < 1e-5);
    }

    #[test]
    fn extrude_adjacent_faces_as_region() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m = {
            let mut s = m.clone();
            let t = top_face(&s);
            subdivide_faces(&mut s, &BTreeSet::from([t]));
            s
        };
        m.validate().unwrap();
        assert!(m.is_closed());
        let tops: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.face_normal(*f).y > 0.9).collect();
        assert_eq!(tops.len(), 4);
        extrude_faces(&mut m, &tops, 1.0);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 2.0).abs() < 1e-4, "{}", m.volume());
    }

    #[test]
    fn inset_keeps_area_and_closure() {
        let mut m = cuboid(v3(2.0, 1.0, 2.0));
        let a = m.total_area();
        let t = top_face(&m);
        inset_faces(&mut m, &BTreeSet::from([t]), 0.5);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.total_area() - a).abs() < 1e-4);
        assert!((m.face_area(t) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn delete_compacts_and_flip_inverts() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        delete_faces(&mut m, &BTreeSet::from([t]));
        assert_eq!(m.faces.len(), 5);
        assert_eq!(m.positions.len(), 8);
        assert!(!m.is_closed());
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<usize> = (0..6).collect();
        flip_faces(&mut c, &all);
        assert!(c.volume() < 0.0);
        delete_verts(&mut c, &BTreeSet::from([0]));
        assert_eq!(c.faces.len(), 3);
    }

    #[test]
    fn weld_is_idempotent_and_merges_split_shapes() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let mut b = cuboid(v3(1.0, 1.0, 1.0));
        b.translate(v3(0.0, 1.0, 0.0));
        m.append(&b);
        assert_eq!(weld_by_distance(&mut m, 1e-3, None), 4);
        assert_eq!(m.positions.len(), 12);
        assert_eq!(weld_by_distance(&mut m, 1e-3, None), 0);
    }

    #[test]
    fn merge_verts_collapses_to_pyramid() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let top: BTreeSet<u32> = m.faces[t].verts.iter().copied().collect();
        merge_verts(&mut m, &top);
        m.validate().unwrap();
        assert_eq!(m.positions.len(), 5);
        assert_eq!(m.faces.len(), 5);
        assert!(m.is_closed());
        assert!((m.volume() - 1.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn subdivide_and_triangulate_stay_watertight() {
        let mut m = cylinder(v3(2.0, 2.0, 2.0), 8, true);
        let all: BTreeSet<usize> = (0..m.faces.len()).collect();
        let v = m.volume();
        subdivide_faces(&mut m, &all);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - v).abs() < 1e-3);
        let all: BTreeSet<usize> = (0..m.faces.len()).collect();
        triangulate_faces(&mut m, &all);
        assert!(m.faces.iter().all(|f| f.verts.len() == 3));
        assert!(m.is_closed());
    }

    #[test]
    fn mirror_keeps_outward() {
        let mut m = wedge(v3(1.0, 1.0, 1.0));
        mirror(&mut m, 0);
        assert!(m.volume() > 0.0);
    }
}
