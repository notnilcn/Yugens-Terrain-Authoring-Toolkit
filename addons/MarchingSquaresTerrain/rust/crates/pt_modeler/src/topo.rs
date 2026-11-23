use super::model::{BMesh, Edge, Face, edge, newell};
use pt_geom::math::*;
use std::collections::{BTreeSet, HashMap};
pub fn directed_owner(m: &BMesh) -> HashMap<(u32, u32), usize> {
    let mut o = HashMap::new();
    for (fi, f) in m.facets.iter().enumerate() {
        for e in f.loop_edges() {
            o.insert(e, fi);
        }
    }
    o
}
pub fn chamfer_ridges(m: &mut BMesh, edges: &BTreeSet<Edge>, amount: f32) -> BTreeSet<usize> {
    super::bevel::chamfer_ridges(m, edges, amount, 1, false)
}
pub fn region_boundary(m: &BMesh, region: &BTreeSet<usize>) -> Vec<(u32, u32)> {
    let mut directed = BTreeSet::new();
    for f in region {
        for e in m.facets[*f].loop_edges() {
            directed.insert(e);
        }
    }
    region
        .iter()
        .flat_map(|f| m.facets[*f].loop_edges().collect::<Vec<_>>())
        .filter(|(a, b)| !directed.contains(&(*b, *a)))
        .collect()
}
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
pub fn fuse_facets(m: &mut BMesh, faces: &BTreeSet<usize>) -> Option<usize> {
    let region: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    if region.len() < 2 {
        return None;
    }
    let loops = chain_loops(&region_boundary(m, &region))?;
    if loops.len() != 1 || loops[0].len() < 3 {
        return None;
    }
    let first = *region.first()?;
    let face = m.facets[first].with_verts(loops.into_iter().next()?);
    let mut i = 0;
    m.facets.retain(|_| {
        let keep = !region.contains(&i);
        i += 1;
        keep
    });
    m.facets.push(face);
    m.compact();
    Some(m.facets.len() - 1)
}
pub fn pull_out_exposed_ridges(
    m: &mut BMesh,
    edges: &BTreeSet<Edge>,
    distance: f32,
    offset: Vec3,
) -> BTreeSet<Edge> {
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
        let e = (m.positions[b as usize] - m.positions[a as usize]).unitized();
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
        let p = m.positions[v as usize] + dir[&v].unitized() * distance + offset;
        dup.insert(v, m.add_vert(p));
    }
    let mut out = BTreeSet::new();
    for (a, b, f) in open {
        let src = m.facets[f].clone();
        m.facets.push(Face {
            glide: 0,
            ..src.with_verts(vec![b, a, dup[&a], dup[&b]])
        });
        out.insert(edge(dup[&a], dup[&b]));
    }
    out
}
pub fn connect_verts(m: &mut BMesh, a: u32, b: u32) -> Option<(usize, usize)> {
    let fi = (0..m.facets.len()).find(|f| {
        let v = &m.facets[*f].verts;
        let (Some(i), Some(j)) = (
            v.iter().position(|x| *x == a),
            v.iter().position(|x| *x == b),
        ) else {
            return false;
        };
        let n = v.len();
        i != j && (i + 1) % n != j && (j + 1) % n != i
    })?;
    let f = m.facets[fi].clone();
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
    m.facets[fi] = f.with_verts(walk(i, j));
    m.facets.push(f.with_verts(walk(j, i)));
    Some((fi, m.facets.len() - 1))
}
pub fn connect_selected(m: &mut BMesh, verts: &BTreeSet<u32>) -> usize {
    let mut made = 0;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 250 {
            return made;
        }
        let mut pair = None;
        'f: for f in &m.facets {
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
pub fn split_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, cuts: u32) -> BTreeSet<u32> {
    let cuts = cuts.clamp(1, 60);
    let mut inserted: HashMap<Edge, Vec<u32>> = HashMap::new();
    let mut out = BTreeSet::new();
    for &(a, b) in edges {
        if (a as usize) >= m.positions.len() || (b as usize) >= m.positions.len() {
            continue;
        }
        let (pa, pb) = (m.positions[a as usize], m.positions[b as usize]);
        let vs: Vec<u32> = (1..=cuts)
            .map(|k| m.add_vert(pa.lerp(pb, k as f32 / (cuts + 1) as f32)))
            .collect();
        out.extend(vs.iter().copied());
        inserted.insert((a, b), vs);
    }
    for f in &mut m.facets {
        let mut lp = Vec::with_capacity(f.verts.len() + 2);
        for (a, b) in f.loop_edges() {
            lp.push(a);
            if let Some(vs) = inserted.get(&edge(a, b)) {
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
pub fn flood_cavities(m: &mut BMesh, only: Option<&BTreeSet<u32>>) -> BTreeSet<usize> {
    let owner = directed_owner(m);
    let open: Vec<(u32, u32)> = owner
        .keys()
        .filter(|(a, b)| !owner.contains_key(&(*b, *a)))
        .map(|(a, b)| (*b, *a))
        .collect();
    let Some(loops) = chain_loops(&open) else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::new();
    for lp in loops {
        if lp.len() < 3 || only.is_some_and(|s| !lp.iter().all(|v| s.contains(v))) {
            continue;
        }
        let src = owner
            .get(&(lp[1], lp[0]))
            .map(|f| m.facets[*f].clone())
            .unwrap_or_else(|| Face::new(Vec::new()));
        m.facets.push(Face {
            glide: 0,
            ..src.with_verts(lp)
        });
        out.insert(m.facets.len() - 1);
    }
    out
}
pub fn rotate_vertices(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, axis: Vec3, angle: f32) {
    let k = axis.unitized();
    let (s, c) = angle.sin_cos();
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            let r = d * c + k.cross(d) * s + k * (k.innerprod(d) * (1.0 - c));
            *p = pivot + r;
        }
    }
}
pub fn coplanar_faces(m: &BMesh, seed: usize, max_angle_deg: f32) -> BTreeSet<usize> {
    let Some(_) = m.facets.get(seed) else {
        return BTreeSet::new();
    };
    let n0 = m.face_normal(seed);
    let p0 = m.face_center(seed);
    let cos = max_angle_deg.to_radians().cos();
    let ef = m.edge_faces();
    let mut out = BTreeSet::from([seed]);
    let mut stack = vec![seed];
    while let Some(f) = stack.pop() {
        for (a, b) in m.facets[f].loop_edges() {
            for &g in ef.get(&edge(a, b)).into_iter().flatten() {
                if out.contains(&g) {
                    continue;
                }
                let n = m.face_normal(g);
                let dist = (m.face_center(g) - p0).innerprod(n0).abs();
                if n.innerprod(n0) >= cos && dist < 0.0014 {
                    out.insert(g);
                    stack.push(g);
                }
            }
        }
    }
    out
}
pub fn similar_faces(m: &BMesh, seed: usize) -> BTreeSet<usize> {
    let Some(f) = m.facets.get(seed) else {
        return BTreeSet::new();
    };
    let slot = f.stuff;
    (0..m.facets.len())
        .filter(|i| m.facets[*i].stuff == slot)
        .collect()
}
pub fn unlink_facets(m: &mut BMesh, faces: &BTreeSet<usize>) {
    let inside: BTreeSet<u32> = m.verts_of_faces(faces);
    let outside: BTreeSet<u32> = (0..m.facets.len())
        .filter(|f| !faces.contains(f))
        .flat_map(|f| m.facets[f].verts.clone())
        .collect();
    let mut map: HashMap<u32, u32> = HashMap::new();
    for v in inside.intersection(&outside) {
        let p = m.positions[*v as usize];
        map.insert(*v, m.add_vert(p));
    }
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            for v in &mut face.verts {
                if let Some(n) = map.get(v) {
                    *v = *n;
                }
            }
        }
    }
}
pub fn loop_normal(m: &BMesh, lp: &[u32]) -> Vec3 {
    let pts: Vec<Vec3> = lp.iter().map(|v| m.positions[*v as usize]).collect();
    newell(&pts)
}
#[cfg(test)]
mod tests {
    use super::super::shapes::*;
    use super::*;
    fn top_face(m: &BMesh) -> usize {
        (0..m.facets.len())
            .find(|f| m.face_normal(*f).y > 0.9)
            .unwrap()
    }
    #[test]
    fn bevel_one_box_edge_is_a_clean_chamfer() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let e = {
            let f = &m.facets[t];
            edge(f.verts[0], f.verts[1])
        };
        let strips = chamfer_ridges(&mut m, &BTreeSet::from([e]), 0.2);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(strips.len(), 1);
        assert_eq!(m.facets.len(), 7);
        assert_eq!(m.positions.len(), 10);
        assert!((m.volume() - (1.0 - 0.02)).abs() < 0.0015, "{}", m.volume());
        assert_eq!(m.facets.iter().filter(|f| f.verts.len() == 5).count(), 2);
    }
    #[test]
    fn bevel_top_rim_and_all_edges() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let rim: BTreeSet<Edge> = m.facets[t].loop_edges().map(|(a, b)| edge(a, b)).collect();
        chamfer_ridges(&mut m, &rim, 0.1);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(
            m.facets.len(),
            14,
            "top + 4 sides + bottom + 4 strips + 4 corner patches"
        );
        assert!(m.volume() < 1.0 && m.volume() > 0.97, "{}", m.volume());
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<Edge> = c.ridges().into_iter().collect();
        chamfer_ridges(&mut c, &all, 0.1);
        c.sanity_check().unwrap();
        assert!(c.is_closed());
        assert!(
            (c.volume() - (1.0 - 8.0 * 0.001 / 6.0)).abs() < 0.0015,
            "{}",
            c.volume()
        );
    }
    #[test]
    fn bevel_on_cylinder_side_edge() {
        let mut m = tube(v3(2.0, 2.0, 2.0), 8, false);
        let side: Edge = m
            .ridges()
            .into_iter()
            .find(|(a, b)| (m.positions[*a as usize].y - m.positions[*b as usize].y).abs() > 1.0)
            .unwrap();
        chamfer_ridges(&mut m, &BTreeSet::from([side]), 0.1);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
    }
    #[test]
    fn merge_two_quads_of_subdivided_top() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        super::super::selection_edits::subdivide_selection(&mut m, &BTreeSet::from([t]));
        let tops: BTreeSet<usize> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y > 0.9)
            .collect();
        assert_eq!(tops.len(), 4);
        let a = m.total_area();
        let f = fuse_facets(&mut m, &tops).unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.face_area(f) - 1.0).abs() < 0.0015);
        assert!((m.total_area() - a).abs() < 0.0015);
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        assert!(fuse_facets(&mut m, &all).is_none());
    }
    #[test]
    fn extrude_plane_edge_outwards() {
        let mut m = sheet(v2(2.0, 2.0), 1);
        let e = m
            .ridges()
            .into_iter()
            .find(|(a, b)| m.positions[*a as usize].x > 0.9 && m.positions[*b as usize].x > 0.9)
            .unwrap();
        let out = pull_out_exposed_ridges(&mut m, &BTreeSet::from([e]), 1.0, Vec3::ZERO);
        m.sanity_check().unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(m.facets.len(), 2);
        assert!((m.total_area() - 6.0).abs() < 0.0015);
        assert!(
            m.face_normal(1).y > 0.99,
            "extruded face keeps the plane's facing"
        );
    }
    #[test]
    fn connect_split_and_fill() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let v = m.facets[t].verts.clone();
        connect_verts(&mut m, v[0], v[2]).unwrap();
        assert_eq!(m.facets.len(), 7);
        assert!(m.is_closed());
        assert!(
            connect_verts(&mut m, v[0], v[1]).is_none(),
            "adjacent verts have no face to split"
        );
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let e = c.ridges()[0];
        let nv = split_edges(&mut c, &BTreeSet::from([e]), 2);
        assert_eq!(nv.len(), 2);
        c.sanity_check().unwrap();
        assert!(c.is_closed());
        assert!((c.volume() - 1.0).abs() < 0.0002);
        let t = top_face(&c);
        super::super::selection_edits::discard_facets(&mut c, &BTreeSet::from([t]));
        assert!(!c.is_closed());
        let filled = flood_cavities(&mut c, None);
        assert_eq!(filled.len(), 1);
        c.sanity_check().unwrap();
        assert!(c.is_closed());
        assert!((c.volume() - 1.0).abs() < 0.0002);
    }
    #[test]
    fn rotate_quarter_turn_and_coplanar() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        rotate_vertices(
            &mut m,
            &all,
            Vec3::ZERO,
            Vec3::UP,
            std::f32::consts::FRAC_PI_2,
        );
        let (lo, hi) = m.extent().unwrap();
        assert!((hi.z - lo.z - 2.0).abs() < 0.0015 && (hi.x - lo.x - 1.0).abs() < 0.0015);
        let mut p = sheet(v2(2.0, 2.0), 3);
        assert_eq!(coplanar_faces(&p, 0, 1.0).len(), 9);
        p.positions[0].y = 0.5;
        assert!(coplanar_faces(&p, 4, 1.0).len() < 9);
    }
    #[test]
    fn unlink_splits_facets() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        unlink_facets(&mut m, &BTreeSet::from([t]));
        assert_eq!(m.positions.len(), 12);
        assert!(!m.is_closed());
        m.sanity_check().unwrap();
    }
}
