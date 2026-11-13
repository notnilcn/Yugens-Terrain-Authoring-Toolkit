//! Edge loops / rings, loop insertion, bridging, object mirror and selective vertex weld.
//!
//! Ports of Unibuilder (MIT, Leafshade Consulting Ltd): `core/topology.gd` (UBTopology),
//! `core/select_queries.gd` (edge_loop, edge_ring, face_loop, grow/shrink, boundary edges),
//! `core/ops/insert_loop.gd`, `core/ops/bridge_edges.gd`, `core/ops/mirror_object.gd`,
//! `core/ops/weld_vertices.gd` and `UBOpUtils.insert_edge_points` (`core/ops/op_utils.gd`).
//! `insert_edge_points` interpolates manual UVs and per-corner colours like the original; the
//! other ops here drop them on rebuilt faces (they fall back to the automatic projection).

use super::model::{BMesh, Edge, edge, newell};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Adjacency cache (UBTopology): edge -> bordering faces, vertex -> touching edges.
pub struct Topo {
    pub edge_faces: BTreeMap<Edge, Vec<usize>>,
    pub vertex_edges: Vec<Vec<Edge>>,
}

impl Topo {
    pub fn build(m: &BMesh) -> Topo {
        let n = m.positions.len();
        let mut edge_faces: BTreeMap<Edge, Vec<usize>> = BTreeMap::new();
        for (fi, f) in m.faces.iter().enumerate() {
            for (a, b) in f.loop_edges() {
                if (a as usize) < n && (b as usize) < n {
                    edge_faces.entry(edge(a, b)).or_default().push(fi);
                }
            }
        }
        let mut vertex_edges = vec![Vec::new(); n];
        for e in edge_faces.keys() {
            vertex_edges[e.0 as usize].push(*e);
            vertex_edges[e.1 as usize].push(*e);
        }
        Topo { edge_faces, vertex_edges }
    }
    fn faces_of(&self, e: Edge) -> &[usize] {
        self.edge_faces.get(&e).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Edges used by exactly one face.
pub fn boundary_edges(t: &Topo) -> BTreeSet<Edge> {
    t.edge_faces.iter().filter(|(_, f)| f.len() == 1).map(|(e, _)| *e).collect()
}

/// Edge loop through `start`: continues straight through every 4-valent vertex.
pub fn edge_loop(t: &Topo, start: Edge) -> Vec<Edge> {
    let mut lp = vec![start];
    if !t.edge_faces.contains_key(&start) {
        return lp;
    }
    let mut visited = BTreeSet::from([start]);
    for start_vertex in [start.0, start.1] {
        let mut e = start;
        let mut pivot = start_vertex;
        loop {
            let Some(next) = loop_continuation(t, e, pivot) else { break };
            if !visited.insert(next) {
                break;
            }
            lp.push(next);
            pivot = if next.0 == pivot { next.1 } else { next.0 };
            e = next;
        }
    }
    lp
}

fn loop_continuation(t: &Topo, e: Edge, pivot: u32) -> Option<Edge> {
    let ve = &t.vertex_edges[pivot as usize];
    if ve.len() != 4 {
        return None;
    }
    let ef: BTreeSet<usize> = t.faces_of(e).iter().copied().collect();
    let mut candidate = None;
    for e2 in ve {
        if *e2 == e {
            continue;
        }
        let shares = t.faces_of(*e2).iter().any(|f| ef.contains(f));
        if !shares {
            if candidate.is_some() {
                return None;
            }
            candidate = Some(*e2);
        }
    }
    candidate
}

/// Opposite edge of a quad, `None` for other faces.
fn opposite_edge(m: &BMesh, fi: usize, e: Edge) -> Option<Edge> {
    let v = &m.faces[fi].verts;
    if v.len() != 4 {
        return None;
    }
    (0..4).find(|i| edge(v[*i], v[(i + 1) % 4]) == e).map(|i| edge(v[(i + 2) % 4], v[(i + 3) % 4]))
}

/// Edge ring through `start`: across quads through their opposite edges, both ways. Stops at
/// boundaries, non-quads and when it closes.
pub fn edge_ring(m: &BMesh, t: &Topo, start: Edge) -> Vec<Edge> {
    let mut ring = vec![start];
    let mut visited = BTreeSet::from([start]);
    for &f0 in t.faces_of(start) {
        let mut e = start;
        let mut fi = f0;
        loop {
            let Some(opp) = opposite_edge(m, fi, e) else { break };
            if !visited.insert(opp) {
                break;
            }
            ring.push(opp);
            let Some(nf) = t.faces_of(opp).iter().copied().find(|f| *f != fi) else { break };
            fi = nf;
            e = opp;
        }
    }
    ring
}

/// Chain of quads crossed perpendicular to `direction_edge` (one of `start_face`'s edges).
pub fn face_loop(m: &BMesh, t: &Topo, start_face: usize, direction_edge: Edge) -> BTreeSet<usize> {
    let mut out = BTreeSet::from([start_face]);
    face_walk(m, t, start_face, direction_edge, &mut out);
    if let Some(f) = t.faces_of(direction_edge).iter().copied().find(|f| *f != start_face) {
        out.insert(f);
        face_walk(m, t, f, direction_edge, &mut out);
    }
    out
}

fn face_walk(m: &BMesh, t: &Topo, mut fi: usize, mut entry: Edge, out: &mut BTreeSet<usize>) {
    loop {
        let Some(opp) = opposite_edge(m, fi, entry) else { return };
        let Some(nf) = t.faces_of(opp).iter().copied().find(|f| *f != fi) else { return };
        if !out.insert(nf) {
            return;
        }
        entry = opp;
        fi = nf;
    }
}

/// `shrink_faces`-style helpers of select_queries: grow / shrink vertex and edge selections.
pub fn grow_vertices(t: &Topo, sel: &BTreeSet<u32>) -> BTreeSet<u32> {
    let mut out = sel.clone();
    for v in sel {
        if let Some(es) = t.vertex_edges.get(*v as usize) {
            for e in es {
                out.insert(e.0);
                out.insert(e.1);
            }
        }
    }
    out
}

pub fn shrink_vertices(t: &Topo, sel: &BTreeSet<u32>) -> BTreeSet<u32> {
    sel.iter()
        .copied()
        .filter(|v| t.vertex_edges.get(*v as usize).is_some_and(|es| es.iter().all(|e| sel.contains(&if e.0 == *v { e.1 } else { e.0 }))))
        .collect()
}

pub fn grow_edges(t: &Topo, sel: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    let mut out = sel.clone();
    for e in sel {
        if !t.edge_faces.contains_key(e) {
            continue;
        }
        for v in [e.0, e.1] {
            out.extend(t.vertex_edges[v as usize].iter().copied());
        }
    }
    out
}

pub fn shrink_edges(t: &Topo, sel: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    sel.iter()
        .copied()
        .filter(|e| t.edge_faces.contains_key(e) && [e.0, e.1].iter().all(|v| t.vertex_edges[*v as usize].iter().all(|e2| e2 == e || sel.contains(e2))))
        .collect()
}

/// Keeps a face only if every edge-adjacent face is selected.
pub fn shrink_faces(m: &BMesh, t: &Topo, sel: &BTreeSet<usize>) -> BTreeSet<usize> {
    sel.iter()
        .copied()
        .filter(|fi| *fi < m.faces.len() && m.faces[*fi].loop_edges().all(|(a, b)| t.faces_of(edge(a, b)).iter().all(|nf| nf == fi || sel.contains(nf))))
        .collect()
}

/// `UBOpUtils.insert_edge_points`: splices points into every face (except `skip`) along the
/// edges they lie on. Points are listed from the edge's low to high vertex. Manual UVs and
/// per-corner colours of the faces are interpolated along the split edge (by the projection of
/// each new point onto it), like the original.
pub fn insert_edge_points(m: &mut BMesh, points: &BTreeMap<Edge, Vec<u32>>, skip: &BTreeSet<usize>) {
    let positions = &m.positions;
    for (fi, f) in m.faces.iter_mut().enumerate() {
        if skip.contains(&fi) {
            continue;
        }
        let n = f.verts.len();
        let has_uv = f.uv_manual && f.uv_coords.len() == n;
        let has_col = f.color_corners.len() == n;
        let mut out = Vec::with_capacity(n + 2);
        let mut out_uv = Vec::new();
        let mut out_col = Vec::new();
        let mut changed = false;
        for i in 0..n {
            let (a, b) = (f.verts[i], f.verts[(i + 1) % n]);
            out.push(a);
            if has_uv {
                out_uv.push(f.uv_coords[i]);
            }
            if has_col {
                out_col.push(f.color_corners[i]);
            }
            let key = edge(a, b);
            let Some(pts) = points.get(&key) else { continue };
            changed = true;
            let inserted: Vec<u32> = if a == key.0 { pts.clone() } else { pts.iter().rev().copied().collect() };
            out.extend(inserted.iter().copied());
            if !(has_uv || has_col) {
                continue;
            }
            let pa = positions[a as usize];
            let ab = positions[b as usize] - pa;
            let len2 = ab.dot(ab);
            for vi in inserted {
                let t = if len2 > 1e-12 { ((positions[vi as usize] - pa).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.5 };
                if has_uv {
                    out_uv.push(f.uv_coords[i].lerp(f.uv_coords[(i + 1) % n], t));
                }
                if has_col {
                    out_col.push(lerp_color(f.color_corners[i], f.color_corners[(i + 1) % n], t));
                }
            }
        }
        if changed {
            f.verts = out;
            if has_uv {
                f.uv_coords = out_uv;
            }
            if has_col {
                f.color_corners = out_col;
            }
        }
    }
}

/// Per-channel lerp of two 8-bit colours (Godot `Color.lerp`, rounded back to bytes).
pub fn lerp_color(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let mut o = [0u8; 4];
    for k in 0..4 {
        o[k] = (a[k] as f32 + (b[k] as f32 - a[k] as f32) * t).round().clamp(0.0, 255.0) as u8;
    }
    o
}

/// Inserts `cuts` evenly spaced edge loops along the ring through `start_edge` (UBOpInsertLoop).
/// Returns the new edges running along the inserted loops.
pub fn insert_loop(m: &mut BMesh, start_edge: Edge, cuts: u32) -> BTreeSet<Edge> {
    let cuts = cuts.max(1);
    let t = Topo::build(m);
    if !t.edge_faces.contains_key(&start_edge) {
        return BTreeSet::new();
    }
    let ring = edge_ring(m, &t, start_edge);
    let ring_set: BTreeSet<Edge> = ring.iter().copied().collect();
    let mut points: BTreeMap<Edge, Vec<u32>> = BTreeMap::new();
    for e in &ring {
        let (pa, pb) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
        let list = (1..=cuts).map(|s| m.add_vert(pa.lerp(pb, s as f32 / (cuts + 1) as f32))).collect();
        points.insert(*e, list);
    }
    let seq = |from: u32, to: u32| -> Vec<u32> {
        let key = edge(from, to);
        let list = &points[&key];
        let mut s = vec![from];
        if from == key.0 {
            s.extend(list.iter().copied());
        } else {
            s.extend(list.iter().rev().copied());
        }
        s.push(to);
        s
    };
    let mut new_edges = BTreeSet::new();
    let original = m.faces.len();
    for fi in 0..original {
        let v = m.faces[fi].verts.clone();
        if v.len() != 4 {
            continue;
        }
        let hits: Vec<usize> = (0..4).filter(|i| ring_set.contains(&edge(v[*i], v[(i + 1) % 4]))).collect();
        if hits.len() != 2 || hits[1] - hits[0] != 2 {
            continue;
        }
        let i0 = hits[0];
        let (v0, v1, v2, v3) = (v[i0], v[(i0 + 1) % 4], v[(i0 + 2) % 4], v[(i0 + 3) % 4]);
        let a = seq(v0, v1);
        let b = seq(v3, v2);
        let template = m.faces[fi].clone();
        m.faces[fi].verts = vec![a[0], a[1], b[1], b[0]];
        for j in 1..=cuts as usize {
            m.faces.push(template.with_verts(vec![a[j], a[j + 1], b[j + 1], b[j]]));
            new_edges.insert(edge(a[j], b[j]));
        }
    }
    insert_edge_points(m, &points, &BTreeSet::new());
    new_edges
}

fn traversal(m: &BMesh, fi: usize, e: Edge) -> Option<(u32, u32)> {
    m.faces[fi].loop_edges().find(|(a, b)| edge(*a, *b) == e)
}

fn degenerate(m: &BMesh, lp: &[u32]) -> bool {
    let pts: Vec<Vec3> = lp.iter().map(|v| m.positions[*v as usize]).collect();
    let mut scale: f32 = 0.0;
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            scale = scale.max((pts[i] - pts[j]).length());
        }
    }
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            if (pts[i] - pts[j]).length() <= 1e-6 * scale {
                return true;
            }
        }
    }
    newell(&pts).length() <= 1e-6 * scale * scale
}

fn bowtie(m: &BMesh, lp: &[u32]) -> bool {
    let n = lp.len();
    let p = |i: usize| m.positions[lp[i] as usize];
    let turns: Vec<Vec3> = (0..n).map(|i| (p((i + 1) % n) - p(i)).cross(p((i + n - 1) % n) - p(i))).collect();
    let total = turns.iter().fold(Vec3::ZERO, |a, b| a + *b);
    total.dot(total) <= 0.0 || turns.iter().any(|t| t.dot(total) < 0.0)
}

fn side_ok(m: &BMesh, t: &Topo, u: u32, v: u32) -> bool {
    let sharers = t.faces_of(edge(u, v));
    match sharers {
        [] => true,
        [f] => traversal(m, *f, edge(u, v)) == Some((v, u)),
        _ => false,
    }
}

/// Bridges two boundary edges with a quad (UBOpBridgeEdges). Returns the new face.
pub fn bridge_edges(m: &mut BMesh, e1: Edge, e2: Edge) -> Option<usize> {
    let t = Topo::build(m);
    let (f1, f2) = (t.faces_of(e1), t.faces_of(e2));
    if f1.len() != 1 || f2.len() != 1 {
        return None;
    }
    if e1.0 == e2.0 || e1.0 == e2.1 || e1.1 == e2.0 || e1.1 == e2.1 {
        return None;
    }
    let (a, b) = traversal(m, f1[0], e1)?;
    let (mut c, mut d) = traversal(m, f2[0], e2)?;
    if bowtie(m, &[b, a, d, c]) {
        std::mem::swap(&mut c, &mut d);
    }
    if !side_ok(m, &t, a, d) || !side_ok(m, &t, c, b) {
        return None;
    }
    if degenerate(m, &[b, a, d, c]) {
        return None;
    }
    let src = m.faces[f1[0]].clone();
    m.faces.push(src.with_verts(vec![b, a, d, c]));
    Some(m.faces.len() - 1)
}

/// Bridges the selected boundary edges pairwise: two edges directly, longer selections as
/// two chains of equal length (first half / second half of the chained boundary runs).
pub fn bridge_selected(m: &mut BMesh, edges: &BTreeSet<Edge>) -> BTreeSet<usize> {
    let t = Topo::build(m);
    let open: Vec<Edge> = edges.iter().copied().filter(|e| t.faces_of(*e).len() == 1).collect();
    let mut out = BTreeSet::new();
    if open.len() == 2 {
        out.extend(bridge_edges(m, open[0], open[1]));
        return out;
    }
    // Split into connected runs, then pair runs edge by edge.
    let runs = edge_runs(&open);
    if runs.len() == 2 && runs[0].len() == runs[1].len() {
        let (r0, mut r1) = (runs[0].clone(), runs[1].clone());
        // Pair each edge with the nearest-ordered one of the other run.
        let mid = |e: Edge, m: &BMesh| (m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5;
        let d_fwd = (mid(r0[0], m) - mid(r1[0], m)).length();
        let d_rev = (mid(r0[0], m) - mid(*r1.last().unwrap(), m)).length();
        if d_rev < d_fwd {
            r1.reverse();
        }
        for (a, b) in r0.iter().zip(r1.iter()) {
            out.extend(bridge_edges(m, *a, *b));
        }
    }
    out
}

/// Connected runs of edges, each ordered along its chain.
pub fn edge_runs(edges: &[Edge]) -> Vec<Vec<Edge>> {
    let mut left: BTreeSet<Edge> = edges.iter().copied().collect();
    let mut runs = Vec::new();
    while let Some(&seed) = left.iter().next() {
        left.remove(&seed);
        let mut run = std::collections::VecDeque::from([seed]);
        let (mut head, mut tail) = (seed.0, seed.1);
        loop {
            let Some(&n) = left.iter().find(|e| e.0 == tail || e.1 == tail) else { break };
            left.remove(&n);
            tail = if n.0 == tail { n.1 } else { n.0 };
            run.push_back(n);
        }
        loop {
            let Some(&n) = left.iter().find(|e| e.0 == head || e.1 == head) else { break };
            left.remove(&n);
            head = if n.0 == head { n.1 } else { n.0 };
            run.push_front(n);
        }
        runs.push(run.into_iter().collect());
    }
    runs
}

/// Merges the given vertices within `threshold` of each other into their averages
/// (UBOpWeldVertices: union-find clusters, representative = lowest index).
pub fn weld_vertices(m: &mut BMesh, verts: &BTreeSet<u32>, threshold: f32) -> usize {
    let idx: Vec<u32> = verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect();
    if idx.len() < 2 {
        return 0;
    }
    let mut parent: HashMap<u32, u32> = idx.iter().map(|v| (*v, *v)).collect();
    fn find(p: &mut HashMap<u32, u32>, mut v: u32) -> u32 {
        while p[&v] != v {
            let g = p[&p[&v]];
            p.insert(v, g);
            v = g;
        }
        v
    }
    for i in 0..idx.len() {
        for j in i + 1..idx.len() {
            let (a, b) = (idx[i], idx[j]);
            if (m.positions[a as usize] - m.positions[b as usize]).length() <= threshold {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                parent.insert(rb, ra);
            }
        }
    }
    let mut clusters: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for v in &idx {
        let r = find(&mut parent, *v);
        clusters.entry(r).or_default().push(*v);
    }
    let mut map = HashMap::new();
    for members in clusters.values_mut() {
        if members.len() < 2 {
            continue;
        }
        members.sort();
        let rep = members[0];
        let c = members.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / members.len() as f32;
        for v in members.iter() {
            map.insert(*v, rep);
        }
        m.positions[rep as usize] = c;
    }
    let n = map.values().len() - map.iter().filter(|(k, v)| k == v).count();
    if n > 0 {
        super::ops::remap(m, &map);
    }
    n
}

/// Adds a copy of the whole mesh mirrored across a local axis plane (UBOpMirrorObject), welding
/// the seam vertices that lie on the plane.
pub fn mirror_object(m: &mut BMesh, axis: usize, weld: bool, threshold: f32) {
    let n0 = m.positions.len() as u32;
    let f0 = m.faces.len();
    let flip = |p: Vec3| match axis {
        0 => v3(-p.x, p.y, p.z),
        1 => v3(p.x, -p.y, p.z),
        _ => v3(p.x, p.y, -p.z),
    };
    for i in 0..n0 as usize {
        let p = flip(m.positions[i]);
        m.positions.push(p);
    }
    for fi in 0..f0 {
        let mut copy = m.faces[fi].clone();
        for v in &mut copy.verts {
            *v += n0;
        }
        copy.reverse();
        m.faces.push(copy);
    }
    if weld {
        let on = |p: Vec3| match axis {
            0 => p.x,
            1 => p.y,
            _ => p.z,
        }
        .abs()
            <= threshold;
        let near: BTreeSet<u32> = (0..m.positions.len() as u32).filter(|v| on(m.positions[*v as usize])).collect();
        if near.len() >= 2 {
            weld_vertices(m, &near, threshold * 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::shapes::{cuboid, plane};
    use super::*;

    #[test]
    fn loop_and_ring_on_a_grid() {
        let m = plane(v2(3.0, 3.0), 3);
        let t = Topo::build(&m);
        // An interior edge: both ends 4-valent along the middle.
        let interior: Vec<Edge> = t.edge_faces.iter().filter(|(_, f)| f.len() == 2).map(|(e, _)| *e).collect();
        let e = interior[0];
        let lp = edge_loop(&t, e);
        assert_eq!(lp.len(), 3, "a straight line across the 3x3 grid: {lp:?}");
        let ring = edge_ring(&m, &t, e);
        assert_eq!(ring.len(), 4, "{ring:?}");
        assert_eq!(boundary_edges(&t).len(), 12);
        let fl = face_loop(&m, &t, t.edge_faces[&e][0], e);
        assert_eq!(fl.len(), 3);
    }

    #[test]
    fn insert_loop_around_box() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let e = m.edges().into_iter().find(|(a, b)| (m.positions[*a as usize].y - m.positions[*b as usize].y).abs() > 0.5).unwrap();
        let new = insert_loop(&mut m, e, 1);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(new.len(), 4, "loop around the four sides");
        assert_eq!(m.faces.len(), 10);
        assert!((m.volume() - 1.0).abs() < 1e-4);
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let e = c.edges()[0];
        insert_loop(&mut c, e, 3);
        c.validate().unwrap();
        assert!(c.is_closed());
        assert_eq!(c.faces.len(), 6 + 4 * 3);
    }

    #[test]
    fn bridge_two_planes() {
        let mut m = plane(v2(1.0, 1.0), 1);
        let mut other = plane(v2(1.0, 1.0), 1);
        other.translate(v3(0.0, 0.0, 3.0));
        m.append(&other);
        let t = Topo::build(&m);
        let pick = |m: &BMesh, z: f32| t.edge_faces.keys().copied().find(|e| (m.positions[e.0 as usize].z - z).abs() < 1e-4 && (m.positions[e.1 as usize].z - z).abs() < 1e-4).unwrap();
        let (e1, e2) = (pick(&m, 0.5), pick(&m, 2.5));
        let f = bridge_edges(&mut m, e1, e2).unwrap();
        m.validate().unwrap();
        assert!(m.face_normal(f).y > 0.99, "bridge keeps the planes' facing");
        assert!((m.face_area(f) - 2.0).abs() < 1e-4);
        assert!(bridge_edges(&mut m, e1, e2).is_none(), "already bridged");
    }

    #[test]
    fn mirror_welds_the_seam() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.translate(v3(0.5, 0.0, 0.0));
        let all: BTreeSet<usize> = [(0..m.faces.len()).find(|f| m.face_normal(*f).x < -0.9).unwrap()].into();
        super::super::ops::delete_faces(&mut m, &all);
        mirror_object(&mut m, 0, true, 1e-3);
        m.validate().unwrap();
        assert_eq!(m.positions.len(), 12);
        assert_eq!(m.faces.len(), 10);
        assert!(m.is_closed());
        assert!((m.volume() - 2.0).abs() < 1e-4);
        let mut w = BMesh { positions: vec![v3(0.0, 0.0, 0.0), v3(0.0005, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0)], faces: vec![] };
        w.add_face(vec![1, 2, 3]);
        w.add_face(vec![0, 3, 2]);
        assert_eq!(weld_vertices(&mut w, &BTreeSet::from([0, 1]), 1e-3), 1);
    }
}
