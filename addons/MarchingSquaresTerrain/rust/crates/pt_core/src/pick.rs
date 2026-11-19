//! Ray picking helpers for the diagnostics object picker (slab AABB test + Möller-Trumbore).
//! All inputs are in the same (local) space; `t` is the ray parameter.

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Entry parameter of the ray into the box `[min, max]` (0 when the origin is inside), or None.
pub fn ray_aabb(origin: V3, dir: V3, min: V3, max: V3) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if origin[i] < min[i] || origin[i] > max[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir[i];
        let (mut a, mut b) = ((min[i] - origin[i]) * inv, (max[i] - origin[i]) * inv);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// Ray / triangle intersection (double sided). Returns the ray parameter.
pub fn ray_triangle(origin: V3, dir: V3, a: V3, b: V3, c: V3) -> Option<f32> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, a);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t >= 0.0).then_some(t)
}

/// Nearest hit over a triangle soup (`faces.len() % 3 == 0`).
pub fn ray_faces(origin: V3, dir: V3, faces: &[V3]) -> Option<f32> {
    faces.chunks_exact(3).filter_map(|t| ray_triangle(origin, dir, t[0], t[1], t[2])).fold(None, |best, t| Some(best.map_or(t, |b: f32| b.min(t))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_hits_and_misses() {
        assert_eq!(ray_aabb([-5.0, 0.5, 0.5], [1.0, 0.0, 0.0], [0.0; 3], [1.0; 3]), Some(5.0));
        assert_eq!(ray_aabb([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], [0.0; 3], [1.0; 3]), Some(0.0));
        assert_eq!(ray_aabb([-5.0, 2.0, 0.5], [1.0, 0.0, 0.0], [0.0; 3], [1.0; 3]), None);
        assert_eq!(ray_aabb([5.0, 0.5, 0.5], [1.0, 0.0, 0.0], [0.0; 3], [1.0; 3]), None);
    }

    #[test]
    fn triangle_hits_and_misses() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let t = ray_triangle([0.2, 0.2, 3.0], [0.0, 0.0, -1.0], a, b, c).unwrap();
        assert!((t - 3.0).abs() < 1e-6);
        assert!(ray_triangle([0.8, 0.8, 3.0], [0.0, 0.0, -1.0], a, b, c).is_none());
        assert!(ray_triangle([0.2, 0.2, 3.0], [0.0, 0.0, 1.0], a, b, c).is_none());
        let faces = [a, b, c, [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]];
        assert!((ray_faces([0.1, 0.1, 3.0], [0.0, 0.0, -1.0], &faces).unwrap() - 2.0).abs() < 1e-6);
    }
}
