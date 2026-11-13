//! Element-transform math for the rotate / scale gizmos, proportional ("soft") selection
//! weights and vertex snapping.
//!
//! Port of Unibuilder's `core/transform_math.gd` (UBTransformMath), `core/soft_select.gd`
//! (UBSoftSelect) and `editor/vertex_snap.gd` (UBVertexSnap) — MIT, Leafshade Consulting Ltd.
//! Unibuilder passes `{vertex: weight}` dictionaries; here they are `BTreeMap<u32, f32>`.

use super::model::BMesh;
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet};

/// Per-vertex drag weights (1 for the selection, falloff for soft neighbours).
pub type Weights = BTreeMap<u32, f32>;

/// Weight 1 for every vertex of `verts`.
pub fn hard_weights(verts: &BTreeSet<u32>) -> Weights {
    verts.iter().map(|v| (*v, 1.0)).collect()
}

/// Rotates `p` about the unit `axis` by `angle` (Rodrigues; same as Godot's `Basis(axis, angle)`).
pub fn rotate_vec(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let k = axis.normalized();
    let (s, c) = angle.sin_cos();
    p * c + k.cross(p) * s + k * (k.dot(p) * (1.0 - c))
}

/// Orthonormal orientation frame (columns x, y, z) used by the scale gizmo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame {
    pub const IDENTITY: Frame = Frame { x: v3(1.0, 0.0, 0.0), y: v3(0.0, 1.0, 0.0), z: v3(0.0, 0.0, 1.0) };
    /// Coordinates of `d` in this (orthonormal) frame: `inverse(orientation) * d`.
    pub fn to_local(&self, d: Vec3) -> Vec3 {
        v3(d.dot(self.x), d.dot(self.y), d.dot(self.z))
    }
    pub fn to_world(&self, l: Vec3) -> Vec3 {
        self.x * l.x + self.y * l.y + self.z * l.z
    }
}

fn mul(a: Vec3, b: Vec3) -> Vec3 {
    v3(a.x * b.x, a.y * b.y, a.z * b.z)
}

/// `rotate_positions`: rotates the given vertices about `pivot` around `axis` by `angle`.
pub fn rotate_positions(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, axis: Vec3, angle: f32) {
    if axis.dot(axis) < 1e-12 || angle.abs() < 1e-9 {
        return;
    }
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            *p = pivot + rotate_vec(*p - pivot, axis, angle);
        }
    }
}

/// `scale_positions`: scales the given vertices about `pivot` by per-axis `factors` of `orientation`.
pub fn scale_positions(m: &mut BMesh, verts: &BTreeSet<u32>, pivot: Vec3, orientation: &Frame, factors: Vec3) {
    for v in verts {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let local = orientation.to_local(*p - pivot);
            *p = pivot + orientation.to_world(mul(local, factors));
        }
    }
}

/// `translate_weighted`: translates each vertex by `offset` scaled by its weight.
pub fn translate_weighted(m: &mut BMesh, weights: &Weights, offset: Vec3) {
    for (v, w) in weights {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            *p = *p + offset * *w;
        }
    }
}

/// `rotate_weighted`: soft neighbours sweep a fraction of the arc on their own circle.
pub fn rotate_weighted(m: &mut BMesh, weights: &Weights, pivot: Vec3, axis: Vec3, angle: f32) {
    if axis.dot(axis) < 1e-12 || angle.abs() < 1e-9 {
        return;
    }
    for (v, w) in weights {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            *p = pivot + rotate_vec(*p - pivot, axis, angle * *w);
        }
    }
}

/// `scale_weighted`: factors blended towards 1 by each vertex's weight.
pub fn scale_weighted(m: &mut BMesh, weights: &Weights, pivot: Vec3, orientation: &Frame, factors: Vec3) {
    for (v, w) in weights {
        if let Some(p) = m.positions.get_mut(*v as usize) {
            let f = v3(1.0, 1.0, 1.0).lerp(factors, *w);
            let local = orientation.to_local(*p - pivot);
            *p = pivot + orientation.to_world(mul(local, f));
        }
    }
}

fn snapped(v: f32, step: f32) -> f32 {
    if step > 0.0 { (v / step + 0.5).floor() * step } else { v }
}

fn comp(v: Vec3, k: usize) -> f32 {
    match k {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// `snap_offset_absolute`: the free move `offset` adjusted so `reference + offset` lands on the
/// grid `origin + k * step` (along `axis` only when one is given).
pub fn snap_offset_absolute(reference: Vec3, offset: Vec3, step: f32, origin: Vec3, axis: Vec3) -> Vec3 {
    if step <= 0.0 {
        return offset;
    }
    if axis.dot(axis) < 1e-12 {
        return snap_point(reference + offset, step, origin) - reference;
    }
    let n = axis.normalized();
    let a = v3(n.x.abs(), n.y.abs(), n.z.abs());
    let k = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
    let target = snapped(comp(reference, k) + comp(offset, k) - comp(origin, k), step) + comp(origin, k);
    n * ((target - comp(reference, k)) / comp(n, k))
}

/// `snap_point`: `p` snapped to the nearest point of the lattice `origin + k * step`.
pub fn snap_point(p: Vec3, step: f32, origin: Vec3) -> Vec3 {
    if step <= 0.0 {
        return p;
    }
    let d = p - origin;
    origin + v3(snapped(d.x, step), snapped(d.y, step), snapped(d.z, step))
}

/// Godot's `wrapf(v, -PI, PI)`.
pub fn wrap_pi(v: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let r = (v + std::f32::consts::PI).rem_euclid(tau);
    r - std::f32::consts::PI
}

/// Snaps an angle / factor to `step` (no-op for `step <= 0`).
pub fn snap_step(v: f32, step: f32) -> f32 {
    snapped(v, step)
}

// ------------------------------------------------------------------ soft selection

/// Falloff curves (UBSoftSelect.Falloff).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Falloff {
    #[default]
    Smooth = 0,
    Sphere,
    Root,
    InverseSquare,
    Sharp,
    Linear,
    Constant,
}

impl Falloff {
    pub const ALL: [Falloff; 7] = [Falloff::Smooth, Falloff::Sphere, Falloff::Root, Falloff::InverseSquare, Falloff::Sharp, Falloff::Linear, Falloff::Constant];
    pub fn from_i64(v: i64) -> Falloff {
        Self::ALL.get(v.max(0) as usize).copied().unwrap_or_default()
    }
    pub fn name(self) -> &'static str {
        ["Smooth", "Sphere", "Root", "Inverse Square", "Sharp", "Linear", "Constant"][self as usize]
    }
}

/// Weight for a normalised distance `t` (0 at the selection, 1 at the radius).
pub fn falloff_weight(t: f32, falloff: Falloff) -> f32 {
    let x = (1.0 - t).clamp(0.0, 1.0);
    match falloff {
        Falloff::Sphere => (x * (2.0 - x)).sqrt(),
        Falloff::Root => x.sqrt(),
        Falloff::InverseSquare => x * (2.0 - x),
        Falloff::Sharp => x * x,
        Falloff::Linear => x,
        Falloff::Constant => 1.0,
        Falloff::Smooth => x * x * (3.0 - 2.0 * x),
    }
}

/// Per-vertex drag weights for a proportional drag: 1 for `selected`, falloff by distance to the
/// nearest selected vertex within `radius` (distances measured after `xf`, e.g. in world space).
pub fn soft_weights(positions: &[Vec3], selected: &BTreeSet<u32>, radius: f32, xf: &dyn Fn(Vec3) -> Vec3, falloff: Falloff) -> Weights {
    let mut out: Weights = selected.iter().copied().filter(|v| (*v as usize) < positions.len()).map(|v| (v, 1.0)).collect();
    if radius <= 0.0 || out.is_empty() {
        return out;
    }
    let anchors: Vec<Vec3> = out.keys().map(|v| xf(positions[*v as usize])).collect();
    for (vi, p) in positions.iter().enumerate() {
        let vi = vi as u32;
        if out.contains_key(&vi) {
            continue;
        }
        let wp = xf(*p);
        let best = anchors.iter().map(|a| (wp - *a).length()).fold(radius, f32::min);
        if best < radius {
            let w = falloff_weight(best / radius, falloff);
            if w > 0.0 {
                out.insert(vi, w);
            }
        }
    }
    out
}

// ------------------------------------------------------------------ vertex snap

/// Screen distance (px) that arms a vertex snap.
pub const SNAP_PX: f32 = 22.0;

/// `nearest_target`: index of the target projecting nearest `cursor` within `max_px`.
pub fn nearest_target(targets: &[Vec3], project: &dyn Fn(Vec3) -> Option<Vec2>, cursor: Vec2, max_px: f32) -> Option<usize> {
    let mut best = None;
    let mut best_d = max_px;
    for (i, t) in targets.iter().enumerate() {
        let Some(s) = project(*t) else { continue };
        let d = s.distance(cursor);
        if d < best_d {
            best_d = d;
            best = Some(i);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn rotate_and_scale_about_pivot() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        rotate_positions(&mut m, &all, v3(0.0, 0.5, 0.0), Vec3::UP, std::f32::consts::FRAC_PI_2);
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi.z - lo.z - 2.0).abs() < 1e-4 && (hi.x - lo.x - 1.0).abs() < 1e-4);
        assert!((m.volume() - 2.0).abs() < 1e-3);
        scale_positions(&mut m, &all, v3(0.0, 0.0, 0.0), &Frame::IDENTITY, v3(1.0, 2.0, 0.5));
        assert!((m.volume() - 2.0).abs() < 1e-3);
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi.y - lo.y - 2.0).abs() < 1e-4 && lo.y.abs() < 1e-4);
    }

    #[test]
    fn weighted_ops_blend_by_weight() {
        let mut m = BMesh::default();
        m.add_vert(v3(1.0, 0.0, 0.0));
        m.add_vert(v3(2.0, 0.0, 0.0));
        let w: Weights = [(0, 1.0), (1, 0.5)].into_iter().collect();
        translate_weighted(&mut m, &w, v3(0.0, 2.0, 0.0));
        assert!(close(m.positions[0], v3(1.0, 2.0, 0.0)) && close(m.positions[1], v3(2.0, 1.0, 0.0)));
        let mut m2 = BMesh { positions: vec![v3(1.0, 0.0, 0.0), v3(1.0, 0.0, 0.0)], faces: vec![] };
        rotate_weighted(&mut m2, &w, Vec3::ZERO, Vec3::UP, std::f32::consts::PI);
        assert!(close(m2.positions[0], v3(-1.0, 0.0, 0.0)));
        assert!(close(m2.positions[1], v3(0.0, 0.0, -1.0)), "{:?}", m2.positions[1]);
        let mut m3 = BMesh { positions: vec![v3(1.0, 1.0, 1.0), v3(1.0, 1.0, 1.0)], faces: vec![] };
        scale_weighted(&mut m3, &w, Vec3::ZERO, &Frame::IDENTITY, v3(3.0, 3.0, 3.0));
        assert!(close(m3.positions[0], v3(3.0, 3.0, 3.0)) && close(m3.positions[1], v3(2.0, 2.0, 2.0)));
    }

    #[test]
    fn absolute_snap_lands_on_grid() {
        let o = snap_offset_absolute(v3(0.3, 0.0, 0.0), v3(0.6, 0.2, 0.0), 0.5, Vec3::ZERO, Vec3::ZERO);
        assert!(close(v3(0.3, 0.0, 0.0) + o, v3(1.0, 0.0, 0.0)));
        let o = snap_offset_absolute(v3(0.3, 0.1, 0.0), v3(0.6, 0.0, 0.0), 0.5, Vec3::ZERO, v3(1.0, 0.0, 0.0));
        assert!(close(o, v3(0.7, 0.0, 0.0)), "{o:?}");
        assert!(close(snap_point(v3(0.26, -0.24, 1.0), 0.25, v3(0.1, 0.0, 0.0)), v3(0.35, -0.25, 1.0)));
        assert!((wrap_pi(3.0 * std::f32::consts::PI) - std::f32::consts::PI).abs() < 1e-4 || (wrap_pi(3.0 * std::f32::consts::PI) + std::f32::consts::PI).abs() < 1e-4);
        assert!((wrap_pi(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn soft_weights_fall_off() {
        let pos = vec![v3(0.0, 0.0, 0.0), v3(0.5, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(3.0, 0.0, 0.0)];
        let w = soft_weights(&pos, &BTreeSet::from([0]), 2.0, &|p| p, Falloff::Linear);
        assert_eq!(w[&0], 1.0);
        assert!((w[&1] - 0.75).abs() < 1e-5 && (w[&2] - 0.5).abs() < 1e-5);
        assert!(!w.contains_key(&3));
        assert_eq!(falloff_weight(0.0, Falloff::Smooth), 1.0);
        assert_eq!(falloff_weight(1.0, Falloff::Smooth), 0.0);
        assert!((falloff_weight(0.5, Falloff::Sharp) - 0.25).abs() < 1e-6);
        assert_eq!(soft_weights(&pos, &BTreeSet::from([0]), 0.0, &|p| p, Falloff::Smooth).len(), 1);
    }

    #[test]
    fn nearest_target_picks_closest_in_range() {
        let t = [v3(0.0, 0.0, 0.0), v3(10.0, 0.0, 0.0), v3(100.0, 0.0, 0.0)];
        let proj = |p: Vec3| Some(v2(p.x, p.y));
        assert_eq!(nearest_target(&t, &proj, v2(8.0, 0.0), SNAP_PX), Some(1));
        assert_eq!(nearest_target(&t, &proj, v2(60.0, 0.0), SNAP_PX), None);
    }
}
