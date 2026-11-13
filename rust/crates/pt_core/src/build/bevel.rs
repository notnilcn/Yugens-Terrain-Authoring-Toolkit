//! Edge bevel: chamfers (1 segment) or rounds (`segments` strips) the selected edges.
//!
//! Port of Unibuilder's `core/ops/bevel_edges.gd` (UBOpBevelEdges, MIT, Leafshade Consulting
//! Ltd). Each face corner at a bevelled edge slides `distance` along its *other* edge; the slid
//! points of the two faces sharing a bevelled edge are joined by quarter-circle arcs (one per
//! end), the arcs are bridged by strips, reflex corners slide backwards (`plan_reflex`), the
//! holes left at the bevelled vertices are patched and open (boundary) edges get a single
//! flat strip. Manual-UV wrapping and per-corner colour blending of the original are dropped
//! (`BMesh` uses projected UVs and one colour per face).

use super::loops::Topo;
use super::model::{BMesh, Edge, edge, newell};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Key = (u32, u32);

fn pair(a: u32, b: u32) -> Key {
    (a.min(b), a.max(b))
}

/// Bevels `edges` by `distance` over `segments` strips. `smooth` puts the new faces in a fresh
/// smoothing group. Returns the new faces (strips and corner patches).
pub fn bevel_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, distance: f32, segments: u32, smooth: bool) -> BTreeSet<usize> {
    let segments = segments.max(1) as usize;
    if distance <= 0.0 {
        return BTreeSet::new();
    }
    let topo = Topo::build(m);
    let edge_set: BTreeSet<Edge> = edges.iter().copied().filter(|e| topo.edge_faces.contains_key(e)).collect();
    let mut selected: BTreeSet<Edge> = edge_set.iter().copied().filter(|e| topo.edge_faces[e].len() == 2).collect();
    let back_keys = plan_reflex(m, &topo, &mut selected);
    let first_new_face_initial = m.faces.len();
    if selected.is_empty() {
        bevel_boundary(m, &edge_set, distance);
        m.compact();
        return (first_new_face_initial..m.faces.len()).collect();
    }
    let pre_boundary: BTreeSet<Edge> = topo.edge_faces.iter().filter(|(_, f)| f.len() == 1).map(|(e, _)| *e).collect();

    let mut traverses_xy: BTreeMap<Edge, BTreeMap<usize, bool>> = BTreeMap::new();
    for e in &selected {
        let mut mm = BTreeMap::new();
        for &fi in &topo.edge_faces[e] {
            for (a, b) in m.faces[fi].loop_edges() {
                if a == e.0 && b == e.1 {
                    mm.insert(fi, true);
                } else if a == e.1 && b == e.0 {
                    mm.insert(fi, false);
                }
            }
        }
        traverses_xy.insert(*e, mm);
    }

    let mut slide_vert: HashMap<Key, u32> = HashMap::new();
    let mut corner_for_edge: HashMap<(usize, u32, u32), u32> = HashMap::new();
    for fi in 0..m.faces.len() {
        let verts = m.faces[fi].verts.clone();
        let n = verts.len();
        let mut out = Vec::with_capacity(n + 2);
        let mut changed = false;
        for i in 0..n {
            let v = verts[i];
            let prev = verts[(i + n - 1) % n];
            let next = verts[(i + 1) % n];
            let sel_prev = selected.contains(&edge(prev, v));
            let sel_next = selected.contains(&edge(v, next));
            if !sel_prev && !sel_next {
                out.push(v);
                continue;
            }
            changed = true;
            if sel_prev && sel_next {
                let v1 = slide(m, &mut slide_vert, v, prev, distance, &back_keys);
                let v2 = slide(m, &mut slide_vert, v, next, distance, &back_keys);
                out.push(v1);
                out.push(v2);
                corner_for_edge.insert((fi, v, prev), v1);
                corner_for_edge.insert((fi, v, next), v2);
            } else if sel_next {
                let s = slide(m, &mut slide_vert, v, prev, distance, &back_keys);
                out.push(s);
                corner_for_edge.insert((fi, v, next), s);
            } else {
                let s = slide(m, &mut slide_vert, v, next, distance, &back_keys);
                out.push(s);
                corner_for_edge.insert((fi, v, prev), s);
            }
        }
        if changed {
            m.faces[fi].verts = out;
        }
    }

    let mut arcs: HashMap<Key, Vec<u32>> = HashMap::new();
    let mut edge_arcs: BTreeMap<Edge, (Vec<u32>, Vec<u32>, usize, usize)> = BTreeMap::new();
    for e in &selected {
        let mm = &traverses_xy[e];
        let fis: Vec<usize> = mm.keys().copied().collect();
        if fis.len() != 2 {
            continue;
        }
        let (f1, f2) = if mm[&fis[0]] { (fis[0], fis[1]) } else { (fis[1], fis[0]) };
        let get = |k: (usize, u32, u32)| corner_for_edge.get(&k).copied();
        let (Some(a1), Some(b1), Some(a2), Some(b2)) = (get((f1, e.0, e.1)), get((f1, e.1, e.0)), get((f2, e.0, e.1)), get((f2, e.1, e.0))) else {
            continue;
        };
        let on_x = slide_vert.get(&(e.0, e.1)).copied();
        let on_y = slide_vert.get(&(e.1, e.0)).copied();
        let (x1_on, x2_on) = (on_x == Some(a1), on_x == Some(a2));
        let (y1_on, y2_on) = (on_y == Some(b1), on_y == Some(b2));
        let twisted = (x1_on && y2_on && !x2_on && !y1_on) || (x2_on && y1_on && !x1_on && !y2_on);
        let segs = if twisted { 1 } else { segments };
        let arc_x = shared_arc(m, &mut arcs, e.0, a1, a2, segs);
        let arc_y = shared_arc(m, &mut arcs, e.1, b1, b2, segs);
        edge_arcs.insert(*e, (arc_x, arc_y, f1, f2));
    }

    let mut origin: HashMap<u32, u32> = HashMap::new();
    for (key, v) in &slide_vert {
        origin.insert(*v, key.0);
    }
    let edge_slide = |v: u32, toward: u32| -> Option<u32> {
        if v == toward || selected.contains(&edge(v, toward)) {
            return None;
        }
        slide_vert.get(&(v, toward)).copied()
    };
    let mut cut_pairs: BTreeSet<Key> = BTreeSet::new();
    for fi in 0..m.faces.len() {
        let verts = m.faces[fi].verts.clone();
        let n = verts.len();
        let mut out = Vec::with_capacity(n + 4);
        let mut changed = false;
        let org = |v: u32| origin.get(&v).copied().unwrap_or(v);
        for i in 0..n {
            let v = verts[i];
            let prev = verts[(i + n - 1) % n];
            let next = verts[(i + 1) % n];
            let (ov, oprev, onext) = (org(v), org(prev), org(next));
            let (mut s_prev, mut s_next) = (None, None);
            if ov == v {
                s_prev = edge_slide(v, oprev);
                s_next = edge_slide(v, onext);
            }
            if let (Some(sp), Some(sn)) = (s_prev, s_next) {
                cut_pairs.insert(pair(sp, sn));
                let mut arc = arcs.get(&pair(sp, sn)).cloned().unwrap_or_else(|| vec![sp, sn]);
                if arc[0] != sp {
                    arc.reverse();
                }
                out.extend(arc);
                changed = true;
            } else {
                out.push(v);
                if let Some(sn) = s_next
                    && sn != next
                {
                    out.push(sn);
                    changed = true;
                }
            }
            if onext == ov {
                continue;
            }
            if let Some(sfn) = edge_slide(onext, ov)
                && sfn != v
                && sfn != next
            {
                let next2 = verts[(i + 2) % n];
                let onext2 = org(next2);
                if onext != next || edge_slide(next, onext2).is_none() {
                    out.push(sfn);
                    changed = true;
                }
            }
        }
        if changed {
            m.faces[fi].verts = out;
        }
    }

    let first_new_face = m.faces.len();
    let kept: BTreeSet<u32> = m.faces.iter().flat_map(|f| f.verts.iter().copied()).collect();
    for (e, (arc_x, arc_y, f1, _f2)) in &edge_arcs {
        let mid_x = straight_mid(m, arc_x, e.0, &kept, &cut_pairs);
        let mid_y = straight_mid(m, arc_y, e.1, &kept, &cut_pairs);
        for i in 0..segments {
            let mut raw = vec![arc_at(arc_y, i), arc_at(arc_x, i)];
            if let Some(mx) = mid_x
                && arc_at(arc_x, i) != arc_at(arc_x, i + 1)
            {
                raw.push(mx);
            }
            raw.push(arc_at(arc_x, i + 1));
            raw.push(arc_at(arc_y, i + 1));
            if let Some(my) = mid_y
                && arc_at(arc_y, i) != arc_at(arc_y, i + 1)
            {
                raw.push(my);
            }
            let lp = dedup_loop(&raw);
            if lp.len() < 3 {
                continue;
            }
            let strip = m.faces[*f1].with_verts(lp);
            m.faces.push(strip);
        }
    }
    fill_new_holes(m, &pre_boundary);
    bevel_boundary(m, &edge_set, distance);
    if smooth {
        let group = m.faces[..first_new_face].iter().map(|f| f.smooth + 1).max().unwrap_or(1).max(1);
        for f in &mut m.faces[first_new_face..] {
            f.smooth = group;
        }
    }
    m.compact();
    (first_new_face..m.faces.len()).collect()
}

/// Bevels open (single-face) edges of `edge_set` with one flat strip each.
fn bevel_boundary(m: &mut BMesh, edge_set: &BTreeSet<Edge>, distance: f32) {
    for e in edge_set {
        let topo = Topo::build(m);
        let Some(sharers) = topo.edge_faces.get(e) else { continue };
        if sharers.len() != 1 {
            continue;
        }
        let fi = sharers[0];
        let verts = m.faces[fi].verts.clone();
        let n = verts.len();
        let Some(ai) = (0..n).find(|i| edge(verts[*i], verts[(i + 1) % n]) == *e) else { continue };
        let (a, b) = (verts[ai], verts[(ai + 1) % n]);
        let prev = verts[(ai + n - 1) % n];
        let next = verts[(ai + 2) % n];
        if prev == b || next == a {
            continue;
        }
        let (pa, pb) = (m.positions[a as usize], m.positions[b as usize]);
        let da = m.positions[prev as usize] - pa;
        let db = m.positions[next as usize] - pb;
        if da.length() < 1e-6 || db.length() < 1e-6 {
            continue;
        }
        let a2 = m.add_vert(pa + da.normalized() * distance.min(da.length() * 0.45));
        let b2 = m.add_vert(pb + db.normalized() * distance.min(db.length() * 0.45));
        let out: Vec<u32> = verts.iter().map(|v| if *v == a { a2 } else if *v == b { b2 } else { *v }).collect();
        m.faces[fi].verts = out;
        let strip = m.faces[fi].with_verts(vec![a, b, b2, a2]);
        m.faces.push(strip);
    }
}

fn is_straight(d1: Vec3, d2: Vec3) -> bool {
    let c = d1.cross(d2);
    c.dot(c) <= 1e-12 * d1.dot(d1) * d2.dot(d2) && d1.dot(d2) < 0.0
}

fn straight_mid(m: &BMesh, arc: &[u32], corner: u32, kept: &BTreeSet<u32>, cut_pairs: &BTreeSet<Key>) -> Option<u32> {
    if arc.len() != 2 || !kept.contains(&corner) || arc.contains(&corner) || cut_pairs.contains(&pair(arc[0], arc[1])) {
        return None;
    }
    let c = m.positions[corner as usize];
    is_straight(m.positions[arc[0] as usize] - c, m.positions[arc[1] as usize] - c).then_some(corner)
}

fn arc_at(arc: &[u32], i: usize) -> u32 {
    arc[i.min(arc.len() - 1)]
}

fn dedup_loop(lp: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(lp.len());
    for v in lp {
        if out.last() != Some(v) {
            out.push(*v);
        }
    }
    while out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

fn shared_arc(m: &mut BMesh, arcs: &mut HashMap<Key, Vec<u32>>, corner: u32, i1: u32, i2: u32, segments: usize) -> Vec<u32> {
    let key = pair(i1, i2);
    let mut arc = match arcs.get(&key) {
        Some(a) => a.clone(),
        None => {
            let c = m.positions[corner as usize];
            let a = arc_points(m, c, i1, i2, segments);
            arcs.insert(key, a.clone());
            a
        }
    };
    if arc[0] != i1 {
        arc.reverse();
    }
    arc
}

fn arc_points(m: &mut BMesh, c: Vec3, i1: u32, i2: u32, segments: usize) -> Vec<u32> {
    if i1 == i2 {
        return vec![i1];
    }
    let mut pts = vec![i1];
    let (p1, p2) = (m.positions[i1 as usize], m.positions[i2 as usize]);
    if segments > 1 && !is_straight(p1 - c, p2 - c) {
        let mid = p1 + p2 - c;
        let (a, b) = (p1 - mid, p2 - mid);
        for s in 1..segments {
            let t = s as f32 / segments as f32 * std::f32::consts::FRAC_PI_2;
            pts.push(m.add_vert(mid + a * t.cos() + b * t.sin()));
        }
    }
    pts.push(i2);
    pts
}

/// Drops selected edges whose reflex corner cannot be mitred; returns the slide keys that must
/// slide backwards, mapped to the selected edge's far end.
fn plan_reflex(m: &BMesh, topo: &Topo, selected: &mut BTreeSet<Edge>) -> HashMap<Key, u32> {
    let normals: Vec<Vec3> = (0..m.faces.len()).map(|f| if m.faces[f].verts.len() >= 3 { newell(&m.face_points(f)) } else { Vec3::ZERO }).collect();
    loop {
        let mut uses: HashMap<Key, u32> = HashMap::new();
        let mut back: BTreeMap<Key, (Edge, u32, usize)> = BTreeMap::new();
        for (fi, f) in m.faces.iter().enumerate() {
            let verts = &f.verts;
            let n = verts.len();
            for i in 0..n {
                let v = verts[i];
                let prev = verts[(i + n - 1) % n];
                let next = verts[(i + 1) % n];
                let sel_prev = selected.contains(&edge(prev, v));
                let sel_next = selected.contains(&edge(v, next));
                if !sel_prev && !sel_next {
                    continue;
                }
                if sel_prev && sel_next {
                    *uses.entry((v, prev)).or_default() += 1;
                    *uses.entry((v, next)).or_default() += 1;
                    continue;
                }
                let toward = if sel_next { prev } else { next };
                let sel_end = if sel_next { next } else { prev };
                let key = (v, toward);
                *uses.entry(key).or_default() += 1;
                let normal = normals[fi];
                if normal.dot(normal) < 1e-20 {
                    continue;
                }
                let pv = m.positions[v as usize];
                let along = if sel_next { m.positions[next as usize] - pv } else { pv - m.positions[prev as usize] };
                let sl = m.positions[toward as usize] - pv;
                let inward = normal.cross(along);
                if sl.dot(inward) < -1e-6 * sl.length() * inward.length() {
                    back.insert(key, (edge(v, sel_end), sel_end, fi));
                }
            }
        }
        let mut dropped = false;
        for (key, (e, _, fi)) in &back {
            if !mitre_ok(m, topo, &uses, *key, *fi) {
                selected.remove(e);
                dropped = true;
            }
        }
        if !dropped {
            return back.into_iter().map(|(k, (_, end, _))| (k, end)).collect();
        }
        if selected.is_empty() {
            return HashMap::new();
        }
    }
}

fn mitre_ok(m: &BMesh, topo: &Topo, uses: &HashMap<Key, u32>, key: Key, fi: usize) -> bool {
    if uses.get(&key).copied().unwrap_or(0) != 1 {
        return false;
    }
    let Some(sharers) = topo.edge_faces.get(&edge(key.0, key.1)) else { return false };
    if sharers.len() != 2 {
        return false;
    }
    let gi = if sharers[1] == fi { sharers[0] } else { sharers[1] };
    if gi == fi {
        return false;
    }
    let gv = &m.faces[gi].verts;
    let n = gv.len();
    let Some(i) = gv.iter().position(|v| *v == key.0) else { return false };
    let prev = gv[(i + n - 1) % n];
    let next = gv[(i + 1) % n];
    let other = if prev == key.1 { next } else { prev };
    uses.contains_key(&(key.0, other))
}

fn slide(m: &mut BMesh, slide_vert: &mut HashMap<Key, u32>, v: u32, toward: u32, distance: f32, back_keys: &HashMap<Key, u32>) -> u32 {
    let key = (v, toward);
    if let Some(s) = slide_vert.get(&key) {
        return *s;
    }
    let pv = m.positions[v as usize];
    let pt = m.positions[toward as usize];
    let mut d = distance.min((pv - pt).length() * 0.45);
    let mut dir = (pt - pv).normalized();
    if let Some(&sel_end) = back_keys.get(&key) {
        d = d.min((pv - m.positions[sel_end as usize]).length() * 0.45);
        dir = -dir;
    }
    let idx = m.add_vert(pv + dir * d);
    slide_vert.insert(key, idx);
    idx
}

/// Patches closed boundary loops that the bevel opened (loops touching no pre-existing
/// boundary edge, with unambiguous chaining).
fn fill_new_holes(m: &mut BMesh, pre_boundary: &BTreeSet<Edge>) {
    let topo = Topo::build(m);
    let mut directed: BTreeMap<u32, u32> = BTreeMap::new();
    let mut owner: HashMap<u32, usize> = HashMap::new();
    let mut ambiguous: BTreeSet<u32> = BTreeSet::new();
    for (fi, f) in m.faces.iter().enumerate() {
        for (a, b) in f.loop_edges() {
            if topo.edge_faces.get(&edge(a, b)).is_some_and(|s| s.len() == 1) {
                if directed.insert(a, b).is_some() {
                    ambiguous.insert(a);
                }
                owner.insert(a, fi);
            }
        }
    }
    let mut visited: BTreeSet<u32> = BTreeSet::new();
    let starts: Vec<u32> = directed.keys().copied().collect();
    for start in starts {
        if visited.contains(&start) {
            continue;
        }
        let mut lp = vec![start];
        let mut qualifies = !ambiguous.contains(&start);
        let mut cur = directed[&start];
        let mut closed = false;
        let mut guard = directed.len() + 1;
        while guard > 0 {
            guard -= 1;
            if pre_boundary.contains(&edge(*lp.last().unwrap(), cur)) {
                qualifies = false;
            }
            if cur == start {
                closed = true;
                break;
            }
            if ambiguous.contains(&cur) || !directed.contains_key(&cur) {
                qualifies = false;
                break;
            }
            lp.push(cur);
            cur = directed[&cur];
        }
        visited.extend(lp.iter().copied());
        if !closed || !qualifies || lp.len() < 3 {
            continue;
        }
        lp.reverse();
        let src = owner.get(&start).copied().unwrap_or(0);
        let mut patch = m.faces[src].with_verts(lp);
        patch.smooth = 0;
        m.faces.push(patch);
    }
}

#[cfg(test)]
mod tests {
    use super::super::shapes::*;
    use super::*;

    fn top_face(m: &BMesh) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap()
    }

    #[test]
    fn chamfer_one_box_edge() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let e = {
            let f = &m.faces[t];
            edge(f.verts[0], f.verts[1])
        };
        let new = bevel_edges(&mut m, &BTreeSet::from([e]), 0.2, 1, false);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(new.len(), 1);
        assert_eq!(m.faces.len(), 7);
        assert!((m.volume() - (1.0 - 0.02)).abs() < 1e-4, "{}", m.volume());
    }

    #[test]
    fn rounded_bevel_has_segments() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let e = {
            let f = &m.faces[t];
            edge(f.verts[0], f.verts[1])
        };
        let new = bevel_edges(&mut m, &BTreeSet::from([e]), 0.2, 4, true);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(new.len(), 4);
        assert!(new.iter().all(|f| m.faces[*f].smooth > 0));
        // Quarter-round removes (1 - pi/4) r^2 per unit length: between chamfer and none.
        let v = m.volume();
        assert!(v > 1.0 - 0.02 && v < 1.0 - 0.2 * 0.2 * (1.0 - std::f32::consts::FRAC_PI_4) + 1e-3, "{v}");
    }

    #[test]
    fn bevel_rim_and_all_edges() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let rim: BTreeSet<Edge> = m.faces[t].loop_edges().map(|(a, b)| edge(a, b)).collect();
        bevel_edges(&mut m, &rim, 0.1, 2, false);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!(m.volume() < 1.0 && m.volume() > 0.97, "{}", m.volume());

        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<Edge> = c.edges().into_iter().collect();
        bevel_edges(&mut c, &all, 0.1, 1, false);
        c.validate().unwrap();
        assert!(c.is_closed());
        assert!((c.volume() - (1.0 - 8.0 * 0.001 / 6.0)).abs() < 1e-4, "{}", c.volume());
    }

    #[test]
    fn bevel_open_edge_of_plane() {
        let mut m = plane(v2(2.0, 2.0), 1);
        let e = m.edges()[0];
        let new = bevel_edges(&mut m, &BTreeSet::from([e]), 0.25, 1, false);
        m.validate().unwrap();
        assert_eq!(new.len(), 1);
        assert_eq!(m.faces.len(), 2);
        assert!((m.total_area() - 4.0).abs() < 1e-4);
    }

    #[test]
    fn bevel_cylinder_side_edge() {
        let mut m = cylinder(v3(2.0, 2.0, 2.0), 8, false);
        let side: Edge = m.edges().into_iter().find(|(a, b)| (m.positions[*a as usize].y - m.positions[*b as usize].y).abs() > 1.0).unwrap();
        bevel_edges(&mut m, &BTreeSet::from([side]), 0.1, 3, false);
        m.validate().unwrap();
        assert!(m.is_closed());
    }
}
