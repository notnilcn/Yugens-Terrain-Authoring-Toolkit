//! Picking helpers. Screen-space queries take a `project` closure (point → screen position and
//! depth, `None` if behind the camera) so this module stays engine-free.

use super::compile::face_triangles;
use super::model::{BMesh, Edge};
use crate::math::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub face: usize,
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Möller–Trumbore; returns `t` (two-sided).
pub fn ray_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-6 || u + v > 1.0 + 1e-6 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-6).then_some(t)
}

/// Nearest face hit by a ray (mesh-local). Back faces are hit too unless `front_only`.
pub fn ray_mesh(m: &BMesh, o: Vec3, d: Vec3, front_only: bool) -> Option<RayHit> {
    let tris = face_triangles(m);
    let mut best: Option<RayHit> = None;
    for (fi, f) in m.faces.iter().enumerate() {
        let n = m.face_normal(fi);
        if front_only && n.dot(d) > 0.0 {
            continue;
        }
        for t in &tris[fi] {
            let p = |k: usize| m.positions[f.verts[t[k]] as usize];
            if let Some(tt) = ray_triangle(o, d, p(0), p(1), p(2))
                && best.is_none_or(|b| tt < b.t)
            {
                best = Some(RayHit { face: fi, t: tt, point: o + d * tt, normal: n });
            }
        }
    }
    best
}

/// Closest vertex within `max_px` of `mouse` (ties broken by depth).
pub fn nearest_vertex(m: &BMesh, project: &dyn Fn(Vec3) -> Option<(Vec2, f32)>, mouse: Vec2, max_px: f32) -> Option<u32> {
    let mut best: Option<(f32, f32, u32)> = None;
    for (i, p) in m.positions.iter().enumerate() {
        let Some((s, depth)) = project(*p) else { continue };
        let dist = s.distance(mouse);
        if dist > max_px {
            continue;
        }
        let key = (dist.round(), depth);
        if best.is_none_or(|b| key.0 < b.0 || (key.0 == b.0 && key.1 < b.1)) {
            best = Some((key.0, key.1, i as u32));
        }
    }
    best.map(|b| b.2)
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    ((a + ab * t).distance(p), t)
}

/// Closest edge within `max_px` of `mouse`.
pub fn nearest_edge(m: &BMesh, project: &dyn Fn(Vec3) -> Option<(Vec2, f32)>, mouse: Vec2, max_px: f32) -> Option<Edge> {
    let mut best: Option<(f32, f32, Edge)> = None;
    for e in m.edges() {
        let (Some((a, da)), Some((b, db))) = (project(m.positions[e.0 as usize]), project(m.positions[e.1 as usize])) else { continue };
        let (dist, t) = seg_dist(mouse, a, b);
        if dist > max_px {
            continue;
        }
        let depth = da + (db - da) * t;
        let key = (dist.round(), depth);
        if best.is_none_or(|bb| key.0 < bb.0 || (key.0 == bb.0 && key.1 < bb.1)) {
            best = Some((key.0, key.1, e));
        }
    }
    best.map(|b| b.2)
}

/// Vertices projecting inside the screen rectangle `[lo, hi]`.
/// Unibuilder `UBRayAccel.occluded_before`: true when the ray hits a face before `limit`
/// (back faces skipped with `cull_back`).
pub fn occluded_before(m: &BMesh, o: Vec3, d: Vec3, limit: f32, cull_back: bool) -> bool {
    if limit <= 0.0 {
        return false;
    }
    ray_mesh(m, o, d, cull_back).is_some_and(|h| h.t < limit)
}

/// Unibuilder `UBPicking._occluded`: whether a local point is hidden from a camera at local
/// `eye` behind another face of the mesh (`T_EPS` = 1e-3 relative / absolute slack).
pub fn point_occluded(m: &BMesh, eye: Vec3, p: Vec3, cull_back: bool) -> bool {
    const T_EPS: f32 = 1e-3;
    let v = p - eye;
    let t = v.length();
    if t < 1e-6 {
        return false;
    }
    let d = v * (1.0 / t);
    occluded_before(m, eye, d, t - (t * T_EPS).max(T_EPS), cull_back)
}

pub fn verts_in_rect(m: &BMesh, project: &dyn Fn(Vec3) -> Option<(Vec2, f32)>, lo: Vec2, hi: Vec2) -> BTreeSet<u32> {
    (0..m.positions.len() as u32)
        .filter(|i| project(m.positions[*i as usize]).is_some_and(|(s, _)| s.x >= lo.x && s.x <= hi.x && s.y >= lo.y && s.y <= hi.y))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    #[test]
    fn far_corner_is_occluded_near_corner_is_not() {
        let m = cuboid(v3(1.0, 1.0, 1.0));
        let (lo, hi) = m.bounds().unwrap();
        let eye = v3(5.0, 5.0, 5.0);
        let near = (0..m.positions.len()).find(|i| m.positions[*i] == hi).unwrap();
        let far = (0..m.positions.len()).find(|i| m.positions[*i] == lo).unwrap();
        assert!(!point_occluded(&m, eye, m.positions[near], true));
        assert!(point_occluded(&m, eye, m.positions[far], true));
    }

    #[test]
    fn ray_hits_top_of_box() {
        let m = cuboid(v3(2.0, 1.0, 2.0));
        let h = ray_mesh(&m, v3(0.3, 5.0, 0.2), v3(0.0, -1.0, 0.0), true).unwrap();
        assert!((h.point.y - 1.0).abs() < 1e-5);
        assert!(h.normal.y > 0.9);
        assert!(ray_mesh(&m, v3(5.0, 5.0, 0.0), v3(0.0, -1.0, 0.0), false).is_none());
    }

    #[test]
    fn screen_queries_with_top_down_projection() {
        let m = cuboid(v3(2.0, 1.0, 2.0));
        // Orthographic top-down: screen = (x, z) * 100, depth = -y.
        let proj = |p: Vec3| Some((v2(p.x * 100.0, p.z * 100.0), -p.y));
        let v = nearest_vertex(&m, &proj, v2(98.0, 101.0), 8.0).unwrap();
        assert!(m.positions[v as usize].y > 0.5, "prefers the nearer (top) vertex");
        let e = nearest_edge(&m, &proj, v2(0.0, 99.0), 5.0).unwrap();
        let (a, b) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
        assert!((a.z - 1.0).abs() < 1e-5 && (b.z - 1.0).abs() < 1e-5);
        assert_eq!(verts_in_rect(&m, &proj, v2(0.0, 0.0), v2(200.0, 200.0)).len(), 2);
    }
}
