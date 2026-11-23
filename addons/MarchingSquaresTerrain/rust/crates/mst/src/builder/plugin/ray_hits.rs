use super::*;
pub(super) fn closest_on_axis(p: Vector3, a: Vector3, o: Vector3, d: Vector3) -> Option<f32> {
    let b = a.dot(d);
    let w = p - o;
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-4 {
        return None;
    }
    Some((b * d.dot(w) - a.dot(w)) / denom)
}
pub(super) fn plane_hit(p: Vector3, n: Vector3, o: Vector3, d: Vector3) -> Option<Vector3> {
    let den = n.dot(d);
    if den.abs() < 1e-6 {
        return None;
    }
    let t = n.dot(p - o) / den;
    (t > 0.0).then(|| o + d * t)
}
