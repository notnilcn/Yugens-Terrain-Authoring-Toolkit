//! Further topology operations: edge bevel (chamfer), face merge, edge extrude, vertex connect,
//! edge split, hole fill, arbitrary-axis rotation and coplanar selection.
//!
//! Like `ops`, every function mutates a `BMesh` in place and leaves it valid (callers validate
//! and snapshot for undo); they return what the editor should select afterwards.

use super::model::{BMesh, Edge, Face, edge, newell};
use crate::math::*;
use std::collections::{BTreeSet, HashMap};

/// Directed edge -> owning face.
pub fn directed_owner(m: &BMesh) -> HashMap<(u32, u32), usize> {
    let mut o = HashMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        for e in f.loop_edges() {
            o.insert(e, fi);
        }
    }
    o
}

/// Chamfers edges by `amount` (one segment): delegates to the Unibuilder port in `bevel.rs`.
pub fn bevel_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, amount: f32) -> BTreeSet<usize> {
    super::bevel::bevel_edges(m, edges, amount, 1, false)
}

/// Boundary loops (directed edges whose reverse is not in `region`) of a face region.
pub fn region_boundary(m: &BMesh, region: &BTreeSet<usize>) -> Vec<(u32, u32)> {
    let mut directed = BTreeSet::new();
    for f in region {
        for e in m.faces[*f].loop_edges() {
            directed.insert(e);
        }
    }
    region
        .iter()
        .flat_map(|f| m.faces[*f].loop_edges().collect::<Vec<_>>())
        .filter(|(a, b)| !directed.contains(&(*b, *a)))
        .collect()
}

/// Chains directed edges into loops. `None` if a vertex has two outgoing edges.
pub fn chain_loops(edges: &[(u32, u32)]) -> Option<Vec<Vec<u32>>> {
    let mut next: HashMap<u32, u32> = HashMap::new();
    for (a, b) in edges {
        if next.insert(*a, *b).is_some() {
            return None;
        }
    }
    let mut loops = Vec::new();
    let mut seen = BTreeSet::new();
    let mut starts: Vec<u32> = next.keys().copied().collect();
    starts.sort();
    for s in starts {
        if seen.contains(&s) {
            continue;
        }
        let mut lp = vec![s];
        seen.insert(s);
        let mut cur = s;
        loop {
            let n = *next.get(&cur)?;
            if n == s {
                break;
            }
            if !seen.insert(n) {
                return None;
            }
            lp.push(n);
            cur = n;
        }
        loops.push(lp);
    }
    Some(loops)
}

/// Merges a connected face region into one n-gon (attributes of its first face). Fails (None)
/// when the region's outline is not a single loop (holes, disjoint parts).
pub fn merge_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> Option<usize> {
    let region: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    if region.len() < 2 {
        return None;
    }
    let loops = chain_loops(&region_boundary(m, &region))?;
    if loops.len() != 1 || loops[0].len() < 3 {
        return None;
    }
    let first = *region.first()?;
    let face = m.faces[first].with_verts(loops.into_iter().next()?);
    let mut i = 0;
    m.faces.retain(|_| {
        let keep = !region.contains(&i);
        i += 1;
        keep
    });
    m.faces.push(face);
    m.compact();
    Some(m.faces.len() - 1)
}

/// Extrudes open (boundary) edges outward in the plane of their face by `distance`, plus
/// `offset`. Returns the new outer edges.
pub fn extrude_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, distance: f32, offset: Vec3) -> BTreeSet<Edge> {
    let owner = directed_owner(m);
    let mut open: Vec<(u32, u32, usize)> = Vec::new();
    for &(a, b) in edges {
        match (owner.get(&(a, b)), owner.get(&(b, a))) {
            (Some(f), None) => open.push((a, b, *f)),
            (None, Some(f)) => open.push((b, a, *f)),
            _ => {}
        }
    }
    let mut dir: HashMap<u32, Vec3> = HashMap::new();
    for &(a, b, f) in &open {
        let e = (m.positions[b as usize] - m.positions[a as usize]).normalized();
        let out = e.cross(m.face_normal(f));
        for v in [a, b] {
            let s = dir.entry(v).or_insert(Vec3::ZERO);
            *s = *s + out;
        }
    }
    let mut dup: HashMap<u32, u32> = HashMap::new();
    let mut verts: Vec<u32> = dir.keys().copied().collect();
    verts.sort();
    for v in verts {
        let p = m.positions[v as usize] + dir[&v].normalized() * distance + offset;
        dup.insert(v, m.add_vert(p));
    }
    let mut out = BTreeSet::new();
    for (a, b, f) in open {
        let src = m.faces[f].clone();
        m.faces.push(Face { smooth: 0, ..src.with_verts(vec![b, a, dup[&a], dup[&b]]) });
        out.insert(edge(dup[&a], dup[&b]));
    }
    out
}

/// Splits the face containing both `a` and `b` along the new edge a–b. Returns the two faces.
pub fn connect_verts(m: &mut BMesh, a: u32, b: u32) -> Option<(usize, usize)> {
    let fi = (0..m.faces.len()).find(|f| {
        let v = &m.faces[*f].verts;
        let (Some(i), Some(j)) = (v.iter().position(|x| *x == a), v.iter().position(|x| *x == b)) else { return false };
        let n = v.len();
        i != j && (i + 1) % n != j && (j + 1) % n != i
    })?;
    let f = m.faces[fi].clone();
    let n = f.verts.len();
    let i = f.verts.iter().position(|x| *x == a)?;
    let j = f.verts.iter().position(|x| *x == b)?;
    let walk = |from: usize, to: usize| {
        let mut l = vec![f.verts[from]];
        let mut k = from;
        while k != to {
            k = (k + 1) % n;
            l.push(f.verts[k]);
        }
        l
    };
    m.faces[fi] = f.with_verts(walk(i, j));
    m.faces.push(f.with_verts(walk(j, i)));
    Some((fi, m.faces.len() - 1))
}

/// Connects every pair of selected vertices that share a face (in selection order around each
/// face). Returns the number of new edges.
pub fn connect_selected(m: &mut BMesh, verts: &BTreeSet<u32>) -> usize {
    let mut made = 0;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 256 {
            return made;
        }
        let mut pair = None;
        'f: for f in &m.faces {
            let n = f.verts.len();
            let sel: Vec<usize> = (0..n).filter(|i| verts.contains(&f.verts[*i])).collect();
            for x in 0..sel.len() {
                for y in x + 1..sel.len() {
                    let (i, j) = (sel[x], sel[y]);
                    if (i + 1) % n != j && (j + 1) % n != i {
                        pair = Some((f.verts[i], f.verts[j]));
                        break 'f;
                    }
                }
            }
        }
        match pair.and_then(|(a, b)| connect_verts(m, a, b)) {
            Some(_) => made += 1,
            None => return made,
        }
    }
}

/// Inserts `cuts` evenly spaced vertices into each edge (in every face using it). Returns the
/// new vertices.
pub fn split_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, cuts: u32) -> BTreeSet<u32> {
    let cuts = cuts.clamp(1, 64);
    let mut inserted: HashMap<Edge, Vec<u32>> = HashMap::new();
    let mut out = BTreeSet::new();
    for &(a, b) in edges {
        if (a as usize) >= m.positions.len() || (b as usize) >= m.positions.len() {
            continue;
        }
        let (pa, pb) = (m.positions[a as usize], m.positions[b as usize]);
        let vs: Vec<u32> = (1..=cuts).map(|k| m.add_vert(pa.lerp(pb, k as f32 / (cuts + 1) as f32))).collect();
        out.extend(vs.iter().copied());
        inserted.insert((a, b), vs);
    }
    for f in &mut m.faces {
        let mut lp = Vec::with_capacity(f.verts.len() + 2);
        for (a, b) in f.loop_edges() {
            lp.push(a);
            if let Some(vs) = inserted.get(&edge(a, b)) {
                // Stored low -> high; reverse when walking high -> low.
                if a < b {
                    lp.extend(vs.iter().copied());
                } else {
                    lp.extend(vs.iter().rev().copied());
                }
            }
        }
        f.verts = lp;
    }
    out
}

/// Fills boundary loops. With `only` given, fills loops whose vertices all belong to it.
/// Returns the new faces.
pub fn fill_holes(m: &mut BMesh, only: Option<&BTreeSet<u32>>) -> BTreeSet<usize> {
    let owner = directed_owner(m);
    let open: Vec<(u32, u32)> = owner.keys().filter(|(a, b)| !owner.contains_key(&(*b, *a))).map(|(a, b)| (*b, *a)).collect();
    let Some(loops) = chain_loops(&open) else { return BTreeSet::new() };
    let mut out = BTreeSet::new();
    for lp in loops {
        if lp.len() < 3 || only.is_some_and(|s| !lp.iter().all(|v| s.contains(v))) {
            continue;
        }
        // Attributes of a face along the hole.
        let src = owner.get(&(lp[1], lp[0])).map(|f| m.faces[*f].clone()).unwrap_or_else(|| Face::new(Vec::new()));
        m.faces.push(Face { smooth: 0, ..src.with_verts(lp) });
        out.insert(m.faces.len() - 1);
    }
    out
}

/// Rotates `verts` about an axis through `pivot` (Rodrigues).
pub fn rotate_verts(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, axis: Vec3, angle: f32) {
    let k = axis.normalized();
    let (s, c) = angle.sin_cos();
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            let r = d * c + k.cross(d) * s + k * (k.dot(d) * (1.0 - c));
            *p = pivot + r;
        }
    }
}

/// Faces reachable from `seed` across edges whose neighbour is within `max_angle_deg` of the
/// seed's normal and lies in the same plane.
pub fn coplanar_faces(m: &BMesh, seed: usize, max_angle_deg: f32) -> BTreeSet<usize> {
    let Some(_) = m.faces.get(seed) else { return BTreeSet::new() };
    let n0 = m.face_normal(seed);
    let p0 = m.face_center(seed);
    let cos = max_angle_deg.to_radians().cos();
    let ef = m.edge_faces();
    let mut out = BTreeSet::from([seed]);
    let mut stack = vec![seed];
    while let Some(f) = stack.pop() {
        for (a, b) in m.faces[f].loop_edges() {
            for &g in ef.get(&edge(a, b)).into_iter().flatten() {
                if out.contains(&g) {
                    continue;
                }
                let n = m.face_normal(g);
                let dist = (m.face_center(g) - p0).dot(n0).abs();
                if n.dot(n0) >= cos && dist < 1e-3 {
                    out.insert(g);
                    stack.push(g);
                }
            }
        }
    }
    out
}

/// Faces with the same material slot as `seed` (select similar).
pub fn similar_faces(m: &BMesh, seed: usize) -> BTreeSet<usize> {
    let Some(f) = m.faces.get(seed) else { return BTreeSet::new() };
    let slot = f.material;
    (0..m.faces.len()).filter(|i| m.faces[*i].material == slot).collect()
}

/// Detaches faces from the rest of the mesh (own copies of shared vertices).
pub fn detach_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    let inside: BTreeSet<u32> = m.verts_of_faces(faces);
    let outside: BTreeSet<u32> = (0..m.faces.len()).filter(|f| !faces.contains(f)).flat_map(|f| m.faces[f].verts.clone()).collect();
    let mut map: HashMap<u32, u32> = HashMap::new();
    for v in inside.intersection(&outside) {
        let p = m.positions[*v as usize];
        map.insert(*v, m.add_vert(p));
    }
    for f in faces {
        if let Some(face) = m.faces.get_mut(*f) {
            for v in &mut face.verts {
                if let Some(n) = map.get(v) {
                    *v = *n;
                }
            }
        }
    }
}

/// Signed area normal of the hole loop (for tests / orientation checks).
pub fn loop_normal(m: &BMesh, lp: &[u32]) -> Vec3 {
    let pts: Vec<Vec3> = lp.iter().map(|v| m.positions[*v as usize]).collect();
    newell(&pts)
}

#[cfg(test)]
mod tests {
    use super::super::shapes::*;
    use super::*;

    fn top_face(m: &BMesh) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap()
    }

    #[test]
    fn bevel_one_box_edge_is_a_clean_chamfer() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let e = { let f = &m.faces[t]; edge(f.verts[0], f.verts[1]) };
        let strips = bevel_edges(&mut m, &BTreeSet::from([e]), 0.2);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(strips.len(), 1);
        assert_eq!(m.faces.len(), 7);
        assert_eq!(m.positions.len(), 10);
        assert!((m.volume() - (1.0 - 0.02)).abs() < 1e-4, "{}", m.volume());
        // The two end caps became pentagons.
        assert_eq!(m.faces.iter().filter(|f| f.verts.len() == 5).count(), 2);
    }

    #[test]
    fn bevel_top_rim_and_all_edges() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let rim: BTreeSet<Edge> = m.faces[t].loop_edges().map(|(a, b)| edge(a, b)).collect();
        bevel_edges(&mut m, &rim, 0.1);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.faces.len(), 14, "top + 4 sides + bottom + 4 strips + 4 corner patches");
        assert!(m.volume() < 1.0 && m.volume() > 0.97, "{}", m.volume());

        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<Edge> = c.edges().into_iter().collect();
        bevel_edges(&mut c, &all, 0.1);
        c.validate().unwrap();
        assert!(c.is_closed());
        // Unibuilder: a vertex whose every edge is bevelled only loses its corner (both slides run
        // along the bevelled edges themselves, so the strips collapse): 8 corner tetrahedra.
        assert!((c.volume() - (1.0 - 8.0 * 0.001 / 6.0)).abs() < 1e-4, "{}", c.volume());
    }

    #[test]
    fn bevel_on_cylinder_side_edge() {
        let mut m = cylinder(v3(2.0, 2.0, 2.0), 8, false);
        let side: Edge = m.edges().into_iter().find(|(a, b)| (m.positions[*a as usize].y - m.positions[*b as usize].y).abs() > 1.0).unwrap();
        bevel_edges(&mut m, &BTreeSet::from([side]), 0.1);
        m.validate().unwrap();
        assert!(m.is_closed());
    }

    #[test]
    fn merge_two_quads_of_subdivided_top() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        super::super::ops::subdivide_faces(&mut m, &BTreeSet::from([t]));
        let tops: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.face_normal(*f).y > 0.9).collect();
        assert_eq!(tops.len(), 4);
        let a = m.total_area();
        let f = merge_faces(&mut m, &tops).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.face_area(f) - 1.0).abs() < 1e-4);
        assert!((m.total_area() - a).abs() < 1e-4);
        // Merging the whole closed box has no single outline.
        let all: BTreeSet<usize> = (0..m.faces.len()).collect();
        assert!(merge_faces(&mut m, &all).is_none());
    }

    #[test]
    fn extrude_plane_edge_outwards() {
        let mut m = plane(v2(2.0, 2.0), 1);
        let e = m.edges().into_iter().find(|(a, b)| m.positions[*a as usize].x > 0.9 && m.positions[*b as usize].x > 0.9).unwrap();
        let out = extrude_edges(&mut m, &BTreeSet::from([e]), 1.0, Vec3::ZERO);
        m.validate().unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(m.faces.len(), 2);
        assert!((m.total_area() - 6.0).abs() < 1e-4);
        assert!(m.face_normal(1).y > 0.99, "extruded face keeps the plane's facing");
    }

    #[test]
    fn connect_split_and_fill() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let v = m.faces[t].verts.clone();
        connect_verts(&mut m, v[0], v[2]).unwrap();
        assert_eq!(m.faces.len(), 7);
        assert!(m.is_closed());
        assert!(connect_verts(&mut m, v[0], v[1]).is_none(), "adjacent verts have no face to split");

        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let e = c.edges()[0];
        let nv = split_edges(&mut c, &BTreeSet::from([e]), 2);
        assert_eq!(nv.len(), 2);
        c.validate().unwrap();
        assert!(c.is_closed());
        assert!((c.volume() - 1.0).abs() < 1e-5);

        let t = top_face(&c);
        super::super::ops::delete_faces(&mut c, &BTreeSet::from([t]));
        assert!(!c.is_closed());
        let filled = fill_holes(&mut c, None);
        assert_eq!(filled.len(), 1);
        c.validate().unwrap();
        assert!(c.is_closed());
        assert!((c.volume() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rotate_quarter_turn_and_coplanar() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        rotate_verts(&mut m, &all, Vec3::ZERO, Vec3::UP, std::f32::consts::FRAC_PI_2);
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi.z - lo.z - 2.0).abs() < 1e-4 && (hi.x - lo.x - 1.0).abs() < 1e-4);
        let mut p = plane(v2(2.0, 2.0), 3);
        assert_eq!(coplanar_faces(&p, 0, 1.0).len(), 9);
        p.positions[0].y = 0.5;
        assert!(coplanar_faces(&p, 4, 1.0).len() < 9);
    }

    #[test]
    fn detach_separates_faces() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        detach_faces(&mut m, &BTreeSet::from([t]));
        assert_eq!(m.positions.len(), 12);
        assert!(!m.is_closed());
        m.validate().unwrap();
    }
}
