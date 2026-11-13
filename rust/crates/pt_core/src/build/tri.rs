//! Polygon triangulation (ear clipping on the face plane, fan fallback for degenerate input).

use crate::math::*;

/// Orthonormal tangent pair `(u, v)` with `u × v = n`.
pub fn plane_basis(n: Vec3) -> (Vec3, Vec3) {
    let n = n.normalized();
    let a = if n.y.abs() < 0.9 { Vec3::UP } else { v3(1.0, 0.0, 0.0) };
    let u = a.cross(n).normalized();
    let v = n.cross(u);
    (u, v)
}

fn cross2(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let eps = -1e-7;
    cross2(a, b, p) >= eps && cross2(b, c, p) >= eps && cross2(c, a, p) >= eps
}

/// Triangulates a 2D loop wound counter-clockwise. Returns corner index triples (CCW).
pub fn triangulate_2d(pts: &[Vec2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }
    // Normalise winding.
    let area: f32 = (0..n).map(|i| cross2(Vec2::ZERO, pts[i], pts[(i + 1) % n])).sum();
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let scale = pts.iter().fold(0.0f32, |m, p| m.max(p.x.abs()).max(p.y.abs())).max(1e-6);
    let eps = 1e-9 * scale * scale;
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross2(a, b, c) <= eps {
                continue; // reflex or collinear
            }
            let blocked = idx.iter().any(|&j| j != ia && j != ib && j != ic && pts[j] != a && pts[j] != b && pts[j] != c && in_triangle(pts[j], a, b, c));
            if blocked {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // Degenerate (collinear runs / self-touching): drop a collinear vertex if any,
            // otherwise fan the rest.
            if let Some(i) = (0..m).find(|&i| cross2(pts[idx[(i + m - 1) % m]], pts[idx[i]], pts[idx[(i + 1) % m]]).abs() <= eps) {
                idx.remove(i);
                continue;
            }
            for k in 1..idx.len() - 1 {
                out.push([idx[0], idx[k], idx[k + 1]]);
            }
            idx.clear();
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    if area < 0.0 {
        for t in &mut out {
            t.swap(1, 2);
        }
    }
    out
}

/// Triangulates a planar 3D loop (wound CCW around `normal`). Returns corner index triples
/// with the same winding as the loop.
pub fn triangulate(points: &[Vec3], normal: Vec3) -> Vec<[usize; 3]> {
    if points.len() == 3 {
        return vec![[0, 1, 2]];
    }
    let (u, v) = plane_basis(normal);
    let pts: Vec<Vec2> = points.iter().map(|p| v2(p.dot(u), p.dot(v))).collect();
    let tris = triangulate_2d(&pts);
    // `triangulate_2d` preserves the input winding; it is CCW around `normal` by construction.
    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(pts: &[Vec2], tris: &[[usize; 3]]) -> f32 {
        tris.iter().map(|t| cross2(pts[t[0]], pts[t[1]], pts[t[2]]) * 0.5).sum()
    }

    #[test]
    fn concave_l_and_u_cover_exact_area() {
        let l = [v2(0.0, 0.0), v2(2.0, 0.0), v2(2.0, 1.0), v2(1.0, 1.0), v2(1.0, 2.0), v2(0.0, 2.0)];
        let t = triangulate_2d(&l);
        assert_eq!(t.len(), 4);
        assert!((area(&l, &t) - 3.0).abs() < 1e-5);
        let u = [v2(0.0, 0.0), v2(3.0, 0.0), v2(3.0, 2.0), v2(2.0, 2.0), v2(2.0, 1.0), v2(1.0, 1.0), v2(1.0, 2.0), v2(0.0, 2.0)];
        let t = triangulate_2d(&u);
        assert_eq!(t.len(), 6);
        assert!((area(&u, &t) - 5.0).abs() < 1e-5);
        assert!(t.iter().all(|t| cross2(u[t[0]], u[t[1]], u[t[2]]) > 0.0));
    }

    #[test]
    fn clockwise_input_keeps_winding() {
        let sq = [v2(0.0, 0.0), v2(0.0, 1.0), v2(1.0, 1.0), v2(1.0, 0.0)];
        let t = triangulate_2d(&sq);
        assert!((area(&sq, &t) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn collinear_points_are_ok() {
        let p = [v2(0.0, 0.0), v2(1.0, 0.0), v2(2.0, 0.0), v2(2.0, 1.0), v2(0.0, 1.0)];
        let t = triangulate_2d(&p);
        assert!((area(&p, &t) - 2.0).abs() < 1e-5);
    }
}
