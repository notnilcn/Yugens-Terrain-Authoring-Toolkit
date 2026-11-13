//! Knife: cuts a drawn polygon into a face as its own sub-face.
//!
//! Port of Unibuilder's `core/ops/cut_face.gd` (UBOpCutFace) and the lattice / edge snapping
//! helpers of `editor/cut_tool.gd` (UBCutTool), MIT, Leafshade Consulting Ltd. A polygon
//! strictly inside the face becomes an inner face stitched to the outer ring; a polygon whose
//! points touch the face's vertices or edges splits the face into regions along it.

use super::loops::insert_edge_points;
use super::model::{BMesh, Edge, edge};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::TAU;

const EPS: f32 = 1e-4;
const TOK_EDGE: i64 = 1_000_000;
const TOK_INT: i64 = 2_000_000;

fn cross2(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn area(pts: &[Vec2]) -> f32 {
    let n = pts.len();
    (0..n).map(|i| cross2(pts[i], pts[(i + 1) % n])).sum::<f32>() * 0.5
}

/// Godot's `Geometry2D.is_point_in_polygon` (even-odd).
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[derive(Clone, Copy, Debug)]
enum Class {
    Inside(Vec2),
    Vertex(usize, Vec2),
    Edge(usize, f32, Vec2),
}

impl Class {
    fn p2(&self) -> Vec2 {
        match *self {
            Class::Inside(p) | Class::Vertex(_, p) | Class::Edge(_, _, p) => p,
        }
    }
}

/// In-plane frame (u, w) of a normal: `axis` = X unless the normal is close to it.
pub fn plane_frame(normal: Vec3) -> (Vec3, Vec3) {
    let axis = if normal.x.abs() < 0.9 { v3(1.0, 0.0, 0.0) } else { Vec3::UP };
    let u = (axis - normal * axis.dot(normal)).normalized();
    (u, normal.cross(u))
}

/// Cuts `points` (on the face plane, any winding) into face `face_index`. Returns the new inner
/// face's index on success.
pub fn cut_face(m: &mut BMesh, face_index: usize, points: &[Vec3]) -> Option<usize> {
    if face_index >= m.faces.len() {
        return None;
    }
    let mut pts: Vec<Vec3> = Vec::new();
    for i in 0..points.len() {
        if (points[i] - points[(i + 1) % points.len()]).length() >= EPS {
            pts.push(points[i]);
        }
    }
    if pts.len() < 3 {
        return None;
    }
    let normal = m.face_normal(face_index);
    let (u, w) = plane_frame(normal);
    let mut outer_ids = m.faces[face_index].verts.clone();
    let origin = outer_ids.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / outer_ids.len() as f32;
    let to2 = |p: Vec3| {
        let r = p - origin;
        v2(r.dot(u), r.dot(w))
    };
    let mut outer2: Vec<Vec2> = outer_ids.iter().map(|v| to2(m.positions[*v as usize])).collect();
    let outer_ccw = area(&outer2) >= 0.0;
    if !outer_ccw {
        outer_ids.reverse();
        outer2.reverse();
    }
    let cut2: Vec<Vec2> = pts.iter().map(|p| to2(*p)).collect();
    let (mut lo, mut hi) = (v2(f32::INFINITY, f32::INFINITY), v2(f32::NEG_INFINITY, f32::NEG_INFINITY));
    for q in &cut2 {
        lo = v2(lo.x.min(q.x), lo.y.min(q.y));
        hi = v2(hi.x.max(q.x), hi.y.max(q.y));
    }
    let extent = (hi - lo).length();
    if area(&cut2).abs() <= 1e-6 * extent * extent {
        return None;
    }
    let cls = classify(&cut2, &outer2);
    if cls.iter().any(|c| !matches!(c, Class::Inside(_))) {
        return apply_boundary(m, face_index, &pts, &cls, &outer_ids, &outer2, outer_ccw);
    }
    let mut inner_ids: Vec<u32> = pts.iter().map(|p| m.add_vert(*p)).collect();
    let mut inner2: Vec<Vec2> = cut2.clone();
    if area(&inner2) < 0.0 {
        inner_ids.reverse();
        inner2.reverse();
    }
    let mut inner_loop = inner_ids.clone();
    if !outer_ccw {
        inner_loop.reverse();
    }
    let mut polys = if is_convex(&outer2) {
        stitch(&outer_ids, &outer2, &inner_ids, &inner2)
    } else {
        let p = stitch_bridged(&outer_ids, &outer2, &inner_ids, &inner2);
        if p.is_empty() { stitch(&outer_ids, &outer2, &inner_ids, &inner2) } else { p }
    };
    if !outer_ccw {
        for p in &mut polys {
            p.reverse();
        }
    }
    let template = m.faces[face_index].clone();
    m.faces[face_index].verts = polys[0].clone();
    for p in &polys[1..] {
        m.faces.push(template.with_verts(p.clone()));
    }
    m.faces.push(template.with_verts(inner_loop));
    Some(m.faces.len() - 1)
}

fn classify(cut2: &[Vec2], outer2: &[Vec2]) -> Vec<Class> {
    let n = outer2.len();
    cut2.iter()
        .map(|&p| {
            if let Some(i) = (0..n).find(|i| p.distance(outer2[*i]) < EPS) {
                return Class::Vertex(i, outer2[i]);
            }
            for i in 0..n {
                let a = outer2[i];
                let seg = outer2[(i + 1) % n] - a;
                let len2 = seg.length_squared();
                if len2 < EPS * EPS {
                    continue;
                }
                let t = (p - a).dot(seg) / len2;
                if t <= 0.0 || t >= 1.0 {
                    continue;
                }
                let on = a + seg * t;
                if p.distance(on) < EPS {
                    return Class::Edge(i, t, on);
                }
            }
            Class::Inside(p)
        })
        .collect()
}

fn dedup_cyclic(lp: &[i64]) -> Vec<i64> {
    let n = lp.len();
    (0..n).filter(|i| lp[*i] != lp[(i + 1) % n]).map(|i| lp[i]).collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_boundary(m: &mut BMesh, face_index: usize, pts: &[Vec3], cls: &[Class], outer_ids: &[u32], outer2: &[Vec2], outer_ccw: bool) -> Option<usize> {
    let n = outer_ids.len();
    let mut edge_groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (pi, c) in cls.iter().enumerate() {
        if let Class::Edge(e, _, _) = c {
            edge_groups.entry(*e).or_default().push(pi);
        }
    }
    let t_of = |pi: usize| if let Class::Edge(_, t, _) = cls[pi] { t } else { 0.0 };
    for g in edge_groups.values_mut() {
        g.sort_by(|a, b| t_of(*a).total_cmp(&t_of(*b)));
    }
    let mut p_tok: Vec<i64> = Vec::new();
    let mut p2: Vec<Vec2> = Vec::new();
    for i in 0..n {
        p_tok.push(i as i64);
        p2.push(outer2[i]);
        if let Some(g) = edge_groups.get(&i) {
            for pi in g {
                p_tok.push(TOK_EDGE + *pi as i64);
                p2.push(cls[*pi].p2());
            }
        }
    }
    let planned_pos: HashMap<i64, usize> = p_tok.iter().enumerate().map(|(k, t)| (*t, k)).collect();
    let mut q_tok: Vec<i64> = Vec::new();
    let mut q2: Vec<Vec2> = Vec::new();
    for (pi, c) in cls.iter().enumerate() {
        q_tok.push(match c {
            Class::Vertex(i, _) => *i as i64,
            Class::Edge(..) => TOK_EDGE + pi as i64,
            Class::Inside(_) => TOK_INT + pi as i64,
        });
        q2.push(c.p2());
    }
    if area(&q2) < 0.0 {
        q_tok.reverse();
        q2.reverse();
    }
    let (mut kt, mut k2) = (Vec::new(), Vec::new());
    for j in 0..q_tok.len() {
        if q_tok[j] != q_tok[(j + 1) % q_tok.len()] {
            kt.push(q_tok[j]);
            k2.push(q2[j]);
        }
    }
    let (q_tok, q2) = (kt, k2);
    if q_tok.len() < 3 {
        return None;
    }
    let mut seen = BTreeSet::new();
    if !q_tok.iter().all(|t| seen.insert(*t)) {
        return None;
    }
    let touch_j: Vec<usize> = (0..q_tok.len()).filter(|j| planned_pos.contains_key(&q_tok[*j])).collect();
    let mm = touch_j.len();
    let np = p_tok.len();
    let nq = q_tok.len();
    let mut regions: Vec<Vec<i64>> = Vec::new();
    if mm == 1 {
        let j0 = touch_j[0];
        let p0 = planned_pos[&q_tok[j0]];
        let (bx, by) = find_bridge(&p2, &q2, Some(p0), Some(j0), p2[p0], q2[j0])?;
        let mut half_a = Vec::new();
        let mut i = p0;
        loop {
            half_a.push(p_tok[i]);
            if i == bx {
                break;
            }
            i = (i + 1) % np;
        }
        i = by;
        loop {
            half_a.push(q_tok[i]);
            if i == j0 {
                break;
            }
            i = (i + nq - 1) % nq;
        }
        let mut half_b = Vec::new();
        i = bx;
        loop {
            half_b.push(p_tok[i]);
            if i == p0 {
                break;
            }
            i = (i + 1) % np;
        }
        i = j0;
        loop {
            half_b.push(q_tok[i]);
            if i == by {
                break;
            }
            i = (i + nq - 1) % nq;
        }
        for half in [half_a, half_b] {
            let clean = dedup_cyclic(&half);
            if clean.len() < 3 {
                return None;
            }
            regions.push(clean);
        }
    } else {
        let mut arc_sum = 0;
        for k in 0..mm {
            let pa = planned_pos[&q_tok[touch_j[k]]];
            let pb = planned_pos[&q_tok[touch_j[(k + 1) % mm]]];
            let mut region = Vec::new();
            let mut i = pa;
            loop {
                region.push(p_tok[i]);
                if i == pb {
                    break;
                }
                i = (i + 1) % np;
            }
            arc_sum += region.len() - 1;
            i = (touch_j[(k + 1) % mm] + nq - 1) % nq;
            while i != touch_j[k] {
                region.push(q_tok[i]);
                i = (i + nq - 1) % nq;
            }
            if region.len() >= 3 {
                regions.push(region);
            }
        }
        if arc_sum != np || regions.is_empty() {
            return None;
        }
    }
    for r in &regions {
        let mut d = BTreeSet::new();
        if !r.iter().all(|t| d.insert(*t)) {
            return None;
        }
    }
    let mut new_ids: HashMap<i64, u32> = HashMap::new();
    let mut edge_points: BTreeMap<Edge, Vec<u32>> = BTreeMap::new();
    for (e, g) in &edge_groups {
        let (a_id, b_id) = (outer_ids[*e], outer_ids[(*e + 1) % n]);
        let mut along = Vec::new();
        for pi in g {
            let p = m.positions[a_id as usize].lerp(m.positions[b_id as usize], t_of(*pi));
            let vid = m.add_vert(p);
            new_ids.insert(TOK_EDGE + *pi as i64, vid);
            along.push(vid);
        }
        let key = edge(a_id, b_id);
        if a_id != key.0 {
            along.reverse();
        }
        edge_points.insert(key, along);
    }
    let used: BTreeSet<i64> = q_tok.iter().copied().collect();
    for (pi, c) in cls.iter().enumerate() {
        if matches!(c, Class::Inside(_)) && used.contains(&(TOK_INT + pi as i64)) {
            new_ids.insert(TOK_INT + pi as i64, m.add_vert(pts[pi]));
        }
    }
    if !edge_points.is_empty() {
        insert_edge_points(m, &edge_points, &BTreeSet::from([face_index]));
    }
    let ids = |toks: &[i64]| -> Vec<u32> { toks.iter().map(|t| if *t < TOK_EDGE { outer_ids[*t as usize] } else { new_ids[t] }).collect() };
    let mut polys: Vec<Vec<u32>> = regions.iter().map(|r| ids(r)).collect();
    let mut inner_loop = ids(&q_tok);
    if !outer_ccw {
        for p in &mut polys {
            p.reverse();
        }
        inner_loop.reverse();
    }
    let template = m.faces[face_index].clone();
    m.faces[face_index].verts = polys[0].clone();
    for p in &polys[1..] {
        m.faces.push(template.with_verts(p.clone()));
    }
    m.faces.push(template.with_verts(inner_loop));
    Some(m.faces.len() - 1)
}

fn is_convex(pts: &[Vec2]) -> bool {
    let n = pts.len();
    (0..n).all(|i| {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        cross2(b - a, c - b) >= -1e-9
    })
}

fn stitch_bridged(outer_ids: &[u32], outer2: &[Vec2], inner_ids: &[u32], inner2: &[Vec2]) -> Vec<Vec<u32>> {
    let (p, q) = (outer2.len(), inner2.len());
    let Some(b1) = find_bridge(outer2, inner2, None, None, Vec2::ZERO, Vec2::ZERO) else { return Vec::new() };
    let Some(b2) = find_bridge(outer2, inner2, Some(b1.0), Some(b1.1), outer2[b1.0], inner2[b1.1]) else { return Vec::new() };
    let mut half_a = Vec::new();
    let mut i = b1.0;
    loop {
        half_a.push(outer_ids[i]);
        if i == b2.0 {
            break;
        }
        i = (i + 1) % p;
    }
    i = b2.1;
    loop {
        half_a.push(inner_ids[i]);
        if i == b1.1 {
            break;
        }
        i = (i + q - 1) % q;
    }
    let mut half_b = Vec::new();
    i = b2.0;
    loop {
        half_b.push(outer_ids[i]);
        if i == b1.0 {
            break;
        }
        i = (i + 1) % p;
    }
    i = b1.1;
    loop {
        half_b.push(inner_ids[i]);
        if i == b2.1 {
            break;
        }
        i = (i + q - 1) % q;
    }
    vec![half_a, half_b]
}

fn find_bridge(outer2: &[Vec2], inner2: &[Vec2], skip_o: Option<usize>, skip_i: Option<usize>, other_from: Vec2, other_to: Vec2) -> Option<(usize, usize)> {
    for oi in 0..outer2.len() {
        if Some(oi) == skip_o {
            continue;
        }
        for ij in 0..inner2.len() {
            if Some(ij) == skip_i {
                continue;
            }
            if !diagonal_ok(outer2, inner2, outer2[oi], inner2[ij], oi, ij) {
                continue;
            }
            if skip_o.is_some() && segments_cross(outer2[oi], inner2[ij], other_from, other_to) {
                continue;
            }
            return Some((oi, ij));
        }
    }
    None
}

fn diagonal_ok(outer2: &[Vec2], inner2: &[Vec2], from: Vec2, to: Vec2, o_idx: usize, i_idx: usize) -> bool {
    let po = outer2.len();
    for i in 0..po {
        if i == o_idx || (i + 1) % po == o_idx {
            continue;
        }
        if segments_cross(from, to, outer2[i], outer2[(i + 1) % po]) {
            return false;
        }
    }
    let pi = inner2.len();
    for i in 0..pi {
        if i == i_idx || (i + 1) % pi == i_idx {
            continue;
        }
        if segments_cross(from, to, inner2[i], inner2[(i + 1) % pi]) {
            return false;
        }
    }
    let mid = (from + to) * 0.5;
    point_in_polygon(mid, outer2) && !point_in_polygon(mid, inner2)
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let d1 = cross2(b - a, c - a);
    let d2 = cross2(b - a, d - a);
    let d3 = cross2(d - c, a - c);
    let d4 = cross2(d - c, b - c);
    ((d1 > 1e-9 && d2 < -1e-9) || (d1 < -1e-9 && d2 > 1e-9)) && ((d3 > 1e-9 && d4 < -1e-9) || (d3 < -1e-9 && d4 > 1e-9))
}

fn angle(v: Vec2) -> f64 {
    (v.y as f64).atan2(v.x as f64)
}

/// Unwrapped polar angles of a loop around `c`, starting at its smallest angle.
fn rotated_unwrapped(ids: &[u32], pts: &[Vec2], c: Vec2) -> (Vec<u32>, Vec<f64>) {
    let n = ids.len();
    let raw: Vec<f64> = pts.iter().map(|p| angle(*p - c)).collect();
    let min_i = (0..n).fold(0, |b, i| if raw[i] < raw[b] { i } else { b });
    let mut out_ids = Vec::with_capacity(n);
    let mut angles: Vec<f64> = Vec::with_capacity(n);
    for k in 0..n {
        let i = (min_i + k) % n;
        let mut a = raw[i];
        while let Some(last) = angles.last()
            && a < *last
        {
            a += TAU;
        }
        out_ids.push(ids[i]);
        angles.push(a);
    }
    (out_ids, angles)
}

fn ang(angles: &[f64], k: usize) -> f64 {
    let n = angles.len();
    angles[k % n] + TAU * (k / n) as f64
}

/// Fan-stitches a convex outer ring to an inner loop by polar angle (one poly per inner edge).
fn stitch(outer_ids: &[u32], outer2: &[Vec2], inner_ids: &[u32], inner2: &[Vec2]) -> Vec<Vec<u32>> {
    let c = inner2.iter().fold(Vec2::ZERO, |a, b| a + *b) / inner2.len() as f32;
    let (oi, oa) = rotated_unwrapped(outer_ids, outer2, c);
    let (hi, mut ha) = rotated_unwrapped(inner_ids, inner2, c);
    while ha[0] < oa[0] {
        for a in &mut ha {
            *a += TAU;
        }
    }
    while ha[0] >= oa[0] + TAU {
        for a in &mut ha {
            *a -= TAU;
        }
    }
    let (p_count, q_count) = (oi.len(), hi.len());
    let mut a = 0;
    for i in 1..p_count {
        if oa[i] <= ha[0] {
            a = i;
        }
    }
    let a_end = a + p_count;
    let mut polys = Vec::new();
    for b in 0..q_count {
        let mut poly = vec![oi[a % p_count]];
        while a < a_end && ang(&oa, a + 1) <= ang(&ha, b + 1) {
            a += 1;
            poly.push(oi[a % p_count]);
        }
        poly.push(hi[(b + 1) % q_count]);
        poly.push(hi[b]);
        polys.push(poly);
    }
    polys
}

// ------------------------------------------------------------------ cut tool snapping

/// `lattice_frame`: the cut's 2D lattice in the face plane, anchored at a face vertex.
pub fn lattice_frame(n: Vec3, anchor: Vec3) -> (Vec3, Vec3, Vec3) {
    let (u, w) = plane_frame(n);
    (anchor, u, w)
}

fn snapped(v: f32, step: f32) -> f32 {
    if step > 0.0 { (v / step + 0.5).floor() * step } else { v }
}

/// `snap_to_lattice`: `p` snapped to the nearest lattice crossing.
pub fn snap_to_lattice(p: Vec3, frame: (Vec3, Vec3, Vec3), step: f32) -> Vec3 {
    let (anchor, u, w) = frame;
    let d = p - anchor;
    anchor + u * snapped(d.dot(u), step) + w * snapped(d.dot(w), step)
}

/// Closest point of segment `a`-`b` to `p`.
pub fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let e = b - a;
    let l2 = e.dot(e);
    if l2 < 1e-12 {
        return a;
    }
    a + e * ((p - a).dot(e) / l2).clamp(0.0, 1.0)
}

/// `snap_on_edge`: the point of edge `a`-`b` nearest `p`, snapped along the edge onto the lattice.
pub fn snap_on_edge(p: Vec3, a: Vec3, b: Vec3, frame: (Vec3, Vec3, Vec3), step: f32) -> Vec3 {
    let e = b - a;
    if e.dot(e) < 1e-12 {
        return a;
    }
    let cp = closest_on_segment(p, a, b);
    if step <= 0.0 {
        return cp;
    }
    let (anchor, u, w) = frame;
    let axis = if e.dot(u).abs() >= e.dot(w).abs() { u } else { w };
    let ek = e.dot(axis);
    if ek.abs() < 1e-9 {
        return cp;
    }
    let target = snapped((cp - anchor).dot(axis), step);
    let t = (target - (a - anchor).dot(axis)) / ek;
    a + e * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::super::shapes::{cuboid, plane};
    use super::*;

    fn top(m: &BMesh) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap()
    }

    #[test]
    fn inner_square_on_a_box_top() {
        let mut m = cuboid(v3(2.0, 1.0, 2.0));
        let t = top(&m);
        let pts = [v3(-0.5, 1.0, -0.5), v3(0.5, 1.0, -0.5), v3(0.5, 1.0, 0.5), v3(-0.5, 1.0, 0.5)];
        let a = m.total_area();
        let inner = cut_face(&mut m, t, &pts).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.total_area() - a).abs() < 1e-4);
        assert!((m.face_area(inner) - 1.0).abs() < 1e-4);
        assert!(m.face_normal(inner).y > 0.99, "inner face keeps the facing");
        assert_eq!(m.faces.len(), 6 + 4, "4 ring quads (one replaces the top) + inner");
    }

    #[test]
    fn cut_across_edges_splits_the_face() {
        let mut m = plane(v2(2.0, 2.0), 1);
        // A triangle from the middle of one edge to the middle of the opposite edge and a point inside.
        let pts = [v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, -0.5), v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 0.5)];
        let inner = cut_face(&mut m, 0, &pts).unwrap();
        m.validate().unwrap();
        assert!((m.total_area() - 4.0).abs() < 1e-4, "{}", m.total_area());
        assert!((m.face_area(inner) - 1.0).abs() < 1e-4);
        assert!(m.faces.iter().enumerate().all(|(f, _)| m.face_normal(f).y > 0.99));
        assert_eq!(m.faces.len(), 3);
    }

    #[test]
    fn lattice_and_edge_snaps() {
        let f = lattice_frame(Vec3::UP, Vec3::ZERO);
        let s = snap_to_lattice(v3(0.26, 0.0, 0.74), f, 0.5);
        assert!((s - v3(0.5, 0.0, 0.5)).length() < 1e-5, "{s:?}");
        let e = snap_on_edge(v3(0.3, 0.0, 0.2), v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0), f, 0.25);
        assert!((e - v3(0.25, 0.0, 0.0)).length() < 1e-5, "{e:?}");
    }
}
