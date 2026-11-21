use super::model::{BMesh, Edge, Face, edge};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub fn move_vertices(m: &mut BMesh, verts: &BTreeSet<u32>, d: Vec3) {
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            *p = *p + d;
        }
    }
}
pub fn scale_vertices(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, s: Vec3) {
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            *p = pivot + v3(d.x * s.x, d.y * s.y, d.z * s.z);
        }
    }
}
pub fn rotate_vertices_y(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, angle: f32) {
    let (s, c) = angle.sin_cos();
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let d = *p - pivot;
            *p = pivot + v3(d.x * c + d.z * s, d.y, -d.x * s + d.z * c);
        }
    }
}
pub fn extrude_selection(m: &mut BMesh, faces: &BTreeSet<usize>, distance: f32) -> BTreeSet<usize> {
    let faces: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    if faces.is_empty() {
        return faces;
    }
    let mut directed = BTreeSet::new();
    for f in &faces {
        for e in m.facets[*f].loop_edges() {
            directed.insert(e);
        }
    }
    let boundary: Vec<(u32, u32, usize)> = faces
        .iter()
        .flat_map(|f| {
            m.facets[*f]
                .loop_edges()
                .map(move |e| (e, *f))
                .collect::<Vec<_>>()
        })
        .filter(|((a, b), _)| !directed.contains(&(*b, *a)))
        .map(|((a, b), f)| (a, b, f))
        .collect();
    let mut normal_sum: BTreeMap<u32, Vec3> = BTreeMap::new();
    for f in &faces {
        let n = m.face_normal(*f);
        for v in &m.facets[*f].verts {
            let e = normal_sum.entry(*v).or_insert(Vec3::ZERO);
            *e = *e + n;
        }
    }
    let mut dup: HashMap<u32, u32> = HashMap::new();
    for (v, n) in &normal_sum {
        let p = m.positions[*v as usize] + n.unitized() * distance;
        dup.insert(*v, m.add_vert(p));
    }
    for (a, b, f) in boundary {
        let src = m.facets[f].clone();
        let side = src.with_verts(vec![a, b, dup[&b], dup[&a]]);
        m.facets.push(Face { glide: 0, ..side });
    }
    for f in &faces {
        for v in &mut m.facets[*f].verts {
            *v = dup[v];
        }
    }
    faces
}
pub fn inset_center(m: &mut BMesh, faces: &BTreeSet<usize>, amount: f32) -> BTreeSet<usize> {
    let t = amount.clamp(0.0, 0.9705);
    let mut out = BTreeSet::new();
    for &f in faces {
        if f >= m.facets.len() {
            continue;
        }
        let c = m.face_center(f);
        let old = m.facets[f].verts.clone();
        let inner: Vec<u32> = old
            .iter()
            .map(|v| {
                let p = m.positions[*v as usize];
                m.add_vert(p.lerp(c, t))
            })
            .collect();
        let n = old.len();
        let src = m.facets[f].clone();
        for i in 0..n {
            let j = (i + 1) % n;
            m.facets
                .push(src.with_verts(vec![old[i], old[j], inner[j], inner[i]]));
        }
        m.facets[f].verts = inner;
        out.insert(f);
    }
    out
}
pub fn discard_facets(m: &mut BMesh, faces: &BTreeSet<usize>) {
    let mut i = 0;
    m.facets.retain(|_| {
        let keep = !faces.contains(&i);
        i += 1;
        keep
    });
    m.compact();
}
pub fn delete_verts(m: &mut BMesh, verts: &BTreeSet<u32>) {
    let faces: BTreeSet<usize> = (0..m.facets.len())
        .filter(|f| m.facets[*f].verts.iter().any(|v| verts.contains(v)))
        .collect();
    discard_facets(m, &faces);
}
pub fn delete_edges(m: &mut BMesh, edges: &BTreeSet<Edge>) {
    let faces: BTreeSet<usize> = (0..m.facets.len())
        .filter(|f| {
            m.facets[*f]
                .loop_edges()
                .any(|(a, b)| edges.contains(&edge(a, b)))
        })
        .collect();
    discard_facets(m, &faces);
}
pub fn turnover_facets(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            face.flip_order();
        }
    }
}
pub fn harmonize_uprights(m: &mut BMesh) -> usize {
    let ef = m.edge_faces();
    let same_dir = |verts: &[u32], a: u32, b: u32| {
        (0..verts.len()).any(|i| verts[i] == a && verts[(i + 1) % verts.len()] == b)
    };
    let mut visited = vec![false; m.facets.len()];
    let mut to_flip = Vec::new();
    for seed in 0..m.facets.len() {
        if visited[seed] {
            continue;
        }
        let mut component = vec![seed];
        let mut flip: HashMap<usize, bool> = HashMap::from([(seed, false)]);
        visited[seed] = true;
        let mut queue = vec![seed];
        while let Some(fi) = queue.pop() {
            let verts = m.facets[fi].verts.clone();
            for i in 0..verts.len() {
                let (a, b) = (verts[i], verts[(i + 1) % verts.len()]);
                let Some(sharers) = ef.get(&edge(a, b)) else {
                    continue;
                };
                if sharers.len() != 2 {
                    continue;
                }
                for &nf in sharers {
                    if nf == fi || visited[nf] {
                        continue;
                    }
                    let same = same_dir(&m.facets[nf].verts, a, b);
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
        m.facets[*f].flip_order();
    }
    to_flip.len()
}
pub(crate) fn remap(m: &mut BMesh, map: &HashMap<u32, u32>) -> Vec<Option<u32>> {
    for f in &mut m.facets {
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
        let mut seen = BTreeSet::new();
        out.retain(|v| seen.insert(*v));
        f.verts = out;
    }
    m.facets.retain(|f| f.verts.len() >= 3);
    m.compact()
}
pub fn merge_vertices(m: &mut BMesh, verts: &BTreeSet<u32>) -> Option<u32> {
    let list: Vec<u32> = verts
        .iter()
        .copied()
        .filter(|v| (*v as usize) < m.positions.len())
        .collect();
    if list.len() < 2 {
        return list.first().copied();
    }
    let c = list
        .iter()
        .fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize])
        / list.len() as f32;
    let keep = list[0];
    m.positions[keep as usize] = c;
    let map: HashMap<u32, u32> = list.iter().map(|v| (*v, keep)).collect();
    remap(m, &map)[keep as usize]
}
pub fn braze_via_span(m: &mut BMesh, threshold: f32, only: Option<&BTreeSet<u32>>) -> usize {
    let th = threshold.max(0.0004);
    let cell = |p: Vec3| {
        (
            (p.x / th).floor() as i64,
            (p.y / th).floor() as i64,
            (p.z / th).floor() as i64,
        )
    };
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
                            if (m.positions[o as usize] - p).magnitude() <= th {
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
pub fn subdivide_selection(m: &mut BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let faces: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    let mut mids: HashMap<Edge, u32> = HashMap::new();
    for e in m.edges_of_faces(&faces) {
        let p = (m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5;
        mids.insert(e, m.add_vert(p));
    }
    let centers: Vec<(usize, u32)> = faces
        .iter()
        .map(|f| {
            let c = m.face_center(*f);
            (*f, m.add_vert(c))
        })
        .collect();
    let mut new_faces: Vec<Face> = Vec::new();
    let mut result_idx = Vec::new();
    let old = std::mem::take(&mut m.facets);
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
    m.facets = new_faces;
    result_idx.into_iter().collect()
}
pub fn mesh_fill_facets(m: &mut BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let tris = super::compile::face_triangles(m);
    let old = std::mem::take(&mut m.facets);
    let mut out = BTreeSet::new();
    for (fi, f) in old.into_iter().enumerate() {
        if faces.contains(&fi) && f.verts.len() > 3 && !tris[fi].is_empty() {
            let manual = f.by_hand_tex_coords().is_some();
            let painted = f.corner_colors().is_some();
            for t in &tris[fi] {
                out.insert(m.facets.len());
                let mut tf = f.with_verts(t.iter().map(|k| f.verts[*k]).collect());
                if manual {
                    tf.uv_manual = true;
                    tf.uv_coords = t.iter().map(|k| f.uv_coords[*k]).collect();
                }
                if painted {
                    tf.color_corners = t.iter().map(|k| f.color_corners[*k]).collect();
                }
                m.facets.push(tf);
            }
        } else {
            if faces.contains(&fi) {
                out.insert(m.facets.len());
            }
            m.facets.push(f);
        }
    }
    out
}
pub fn replicate_facets(m: &mut BMesh, faces: &BTreeSet<usize>, offset: Vec3) -> BTreeSet<usize> {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut out = BTreeSet::new();
    for f in faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect::<Vec<_>>()
    {
        let src = m.facets[f].clone();
        let verts = src
            .verts
            .iter()
            .map(|v| {
                *map.entry(*v).or_insert_with(|| {
                    let p = m.positions[*v as usize] + offset;
                    m.positions.push(p);
                    (m.positions.len() - 1) as u32
                })
            })
            .collect();
        out.insert(m.facets.len());
        m.facets.push(src.with_verts(verts));
    }
    out
}
pub fn mirror(m: &mut BMesh, axis: usize) {
    for p in &mut m.positions {
        match axis {
            0 => p.x = -p.x,
            1 => p.y = -p.y,
            _ => p.z = -p.z,
        }
    }
    for f in &mut m.facets {
        f.flip_order();
    }
}
pub fn assign_stuff(m: &mut BMesh, faces: &BTreeSet<usize>, slot: u16) {
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            face.stuff = slot;
        }
    }
}
pub fn set_smooth(m: &mut BMesh, faces: &BTreeSet<usize>, group: u16) {
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            face.glide = group;
        }
    }
}
pub fn assign_tint(m: &mut BMesh, faces: &BTreeSet<usize>, tint: [u8; 4]) {
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            face.tint = tint;
        }
    }
}
pub fn set_uv(m: &mut BMesh, faces: &BTreeSet<usize>, tex_coord: super::model::UvParams) {
    for f in faces {
        if let Some(face) = m.facets.get_mut(*f) {
            face.tex_coord = tex_coord;
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn harmonize_uprights_mends_reversed_facets() {
        let mut m = super::super::shapes::cuboid(v3(1.0, 1.0, 1.0));
        let v0 = m.volume();
        m.facets[1].flip_order();
        m.facets[4].flip_order();
        assert!(!m.is_closed());
        assert_eq!(harmonize_uprights(&mut m), 2);
        assert!(m.is_closed());
        assert!((m.volume() - v0).abs() < 0.0002);
        let mut m = super::super::shapes::cuboid(v3(1.0, 1.0, 1.0));
        for f in &mut m.facets {
            f.flip_order();
        }
        assert_eq!(harmonize_uprights(&mut m), 0);
        assert!(m.is_closed());
    }
    use super::super::shapes::*;
    use super::*;
    fn top_face(m: &BMesh) -> usize {
        (0..m.facets.len())
            .find(|f| m.face_normal(*f).y > 0.9)
            .unwrap()
    }
    #[test]
    fn extrude_top_of_box() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let sel = extrude_selection(&mut m, &BTreeSet::from([t]), 2.0);
        assert_eq!(m.facets.len(), 10);
        assert_eq!(m.positions.len(), 12);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 3.0).abs() < 0.0015);
        assert!((m.face_center(*sel.first().unwrap()).y - 3.0).abs() < 0.0002);
    }
    #[test]
    fn pull_out_neighboring_facets_as_sector() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m = {
            let mut s = m.clone();
            let t = top_face(&s);
            subdivide_selection(&mut s, &BTreeSet::from([t]));
            s
        };
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        let tops: BTreeSet<usize> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y > 0.9)
            .collect();
        assert_eq!(tops.len(), 4);
        extrude_selection(&mut m, &tops, 1.0);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 2.0).abs() < 0.0015, "{}", m.volume());
    }
    #[test]
    fn inset_keeps_area_and_closure() {
        let mut m = cuboid(v3(2.0, 1.0, 2.0));
        let a = m.total_area();
        let t = top_face(&m);
        inset_center(&mut m, &BTreeSet::from([t]), 0.5);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.total_area() - a).abs() < 0.0015);
        assert!((m.face_area(t) - 1.0).abs() < 0.0015);
    }
    #[test]
    fn delete_compacts_and_flip_inverts() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        discard_facets(&mut m, &BTreeSet::from([t]));
        assert_eq!(m.facets.len(), 5);
        assert_eq!(m.positions.len(), 8);
        assert!(!m.is_closed());
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<usize> = (0..6).collect();
        turnover_facets(&mut c, &all);
        assert!(c.volume() < 0.0);
        delete_verts(&mut c, &BTreeSet::from([0]));
        assert_eq!(c.facets.len(), 3);
    }
    #[test]
    fn weld_is_idempotent_and_merges_split_shapes() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let mut b = cuboid(v3(1.0, 1.0, 1.0));
        b.displace(v3(0.0, 1.0, 0.0));
        m.append_tail(&b);
        assert_eq!(braze_via_span(&mut m, 0.0015, None), 4);
        assert_eq!(m.positions.len(), 12);
        assert_eq!(braze_via_span(&mut m, 0.0015, None), 0);
    }
    #[test]
    fn merge_vertices_collapses_to_pyramid() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let top: BTreeSet<u32> = m.facets[t].verts.iter().copied().collect();
        merge_vertices(&mut m, &top);
        m.sanity_check().unwrap();
        assert_eq!(m.positions.len(), 5);
        assert_eq!(m.facets.len(), 5);
        assert!(m.is_closed());
        assert!((m.volume() - 1.0 / 3.0).abs() < 0.0015);
    }
    #[test]
    fn subdivide_and_triangulate_stay_watertight() {
        let mut m = tube(v3(2.0, 2.0, 2.0), 8, true);
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        let v = m.volume();
        subdivide_selection(&mut m, &all);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - v).abs() < 0.0015);
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        mesh_fill_facets(&mut m, &all);
        assert!(m.facets.iter().all(|f| f.verts.len() == 3));
        assert!(m.is_closed());
    }
    #[test]
    fn mirror_keeps_outward() {
        let mut m = wedge(v3(1.0, 1.0, 1.0));
        mirror(&mut m, 0);
        assert!(m.volume() > 0.0);
    }
}
