//! UV lock: textures stay put while faces are dragged (move / rotate / scale of elements).
//! Port of Unibuilder's drag UV handling (`core/ops/uv_ops.gd`: `drag_uv_frames`,
//! `link_drag_patches`, `refit_drag_uvs`, `drag_uv_coords`, `drag_uv_axes`) and of
//! `core/uv_frame.gd` (`UBUVFrame`, MIT).
//!
//! * Manual-UV faces: a rigid move keeps the coordinates (the texture rides along); a face that
//!   is stretched is refitted through the affine plane ↔ UV map captured at the start, so the
//!   texture stays fixed in space under the moved corners.
//! * Automatic-UV faces: a face that tilts while being deformed freezes its projection axis
//!   (`Face::uv_axis`) at the start normal, so it does not jump to another projection. A face
//!   that tilts rigidly (rotated) re-projects, unless it is texture-continuous with a frozen
//!   neighbour, which it then follows.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::model::{BMesh, Face, edge, newell};
use super::uv::projection_frame;
use crate::math::*;

/// Dot-product floor for a rigid face to follow its neighbours' fixed axis.
pub const FOLLOW_DOT: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Dot-product margin under which two start projection axes count as the same.
pub const TILT_EPS: f32 = 1e-5;
/// Corner travel spread along the start normal, times coordinate scale, that counts as a tilt.
pub const PLANE_EPS: f32 = 1e-5;

/// The affine map between a face's plane and its UV space (`UBUVFrame`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvFrame {
    /// Corner mean.
    pub origin: Vec3,
    pub normal: Vec3,
    /// In-plane orthonormal basis.
    pub t: Vec3,
    pub b: Vec3,
    /// UV at the corner mean.
    pub uv0: Vec2,
    /// d(uv) per unit along `t` / `b`.
    pub du: Vec2,
    pub dv: Vec2,
}

impl UvFrame {
    /// Least-squares fit of the frame from corner positions and their UVs (`None` for
    /// degenerate faces or UV maps).
    pub fn fit(corners: &[Vec3], uvs: &[Vec2], face_normal: Vec3) -> Option<UvFrame> {
        let n = corners.len();
        if n < 3 || uvs.len() != n {
            return None;
        }
        let (t, b) = projection_frame(face_normal);
        let origin = corners.iter().fold(Vec3::ZERO, |a, p| a + *p) / n as f32;
        let uv0 = uvs.iter().fold(Vec2::ZERO, |a, p| a + *p) / n as f32;
        let mut f = UvFrame { origin, normal: face_normal.normalized(), t, b, uv0, du: Vec2::ZERO, dv: Vec2::ZERO };
        let (mut sxx, mut sxy, mut syy) = (0.0f64, 0.0f64, 0.0f64);
        let (mut sxu, mut syu) = ([0.0f64; 2], [0.0f64; 2]);
        for i in 0..n {
            let l = f.local(corners[i]);
            let d = uvs[i] - uv0;
            let (lx, ly) = (l.x as f64, l.y as f64);
            sxx += lx * lx;
            sxy += lx * ly;
            syy += ly * ly;
            sxu[0] += d.x as f64 * lx;
            sxu[1] += d.y as f64 * lx;
            syu[0] += d.x as f64 * ly;
            syu[1] += d.y as f64 * ly;
        }
        let det = sxx * syy - sxy * sxy;
        let span = sxx + syy;
        if span < 1e-18 || det < 1e-9 * span * span {
            return None;
        }
        f.du = v2(((sxu[0] * syy - syu[0] * sxy) / det) as f32, ((sxu[1] * syy - syu[1] * sxy) / det) as f32);
        f.dv = v2(((syu[0] * sxx - sxu[0] * sxy) / det) as f32, ((syu[1] * sxx - sxu[1] * sxy) / det) as f32);
        // The UV map must be invertible (a face squashed to a line in UV space is not).
        let fdet = (f.du.x * f.dv.y - f.du.y * f.dv.x).abs();
        if fdet <= 1e-12 || fdet < 1e-9 * (f.du.length_squared() + f.dv.length_squared()) {
            return None;
        }
        Some(f)
    }

    fn local(&self, p: Vec3) -> Vec2 {
        let d = p - self.origin;
        v2(d.dot(self.t), d.dot(self.b))
    }

    /// The UV the map assigns at a position (its out-of-plane component is ignored).
    pub fn uv_at(&self, p: Vec3) -> Vec2 {
        let l = self.local(p);
        self.uv0 + self.du * l.x + self.dv * l.y
    }
}

/// Start state of one dragged face.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// Manual UVs: the fitted frame and the start coordinates.
    Manual { frame: UvFrame, uvs: Vec<Vec2>, dist: Vec<f32> },
    /// Automatic UVs: start unit normal, start projection axis, pair distances, the corners'
    /// heights along the normal, and the texture-continuous neighbours (`adj`).
    Auto { normal: Vec3, axis: Vec3, dist: Vec<f32>, lift: Vec<f32>, adj: Vec<usize> },
}

/// Drag start state for every face touching a dragged vertex.
pub type Frames = BTreeMap<usize, Entry>;

fn corners_from(face: &Face, positions: &[Vec3]) -> Vec<Vec3> {
    face.verts.iter().map(|v| positions.get(*v as usize).copied().unwrap_or(Vec3::ZERO)).collect()
}

fn pair_distances(c: &[Vec3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(c.len() * c.len() / 2);
    for i in 0..c.len() {
        for j in i + 1..c.len() {
            out.push((c[i] - c[j]).length());
        }
    }
    out
}

fn same_distances(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-5 * y.max(1.0))
}

fn lifts(c: &[Vec3], n: Vec3) -> Vec<f32> {
    c.iter().map(|p| p.dot(n)).collect()
}

/// True when the corners left the start plane unevenly (the face tilted or bent).
fn tilted(start_lift: &[f32], normal: Vec3, c: &[Vec3]) -> bool {
    if start_lift.len() != c.len() {
        return false;
    }
    let now = lifts(c, normal);
    let (mut low, mut high, mut scale) = (f32::INFINITY, f32::NEG_INFINITY, 1.0f32);
    for i in 0..now.len() {
        let travel = now[i] - start_lift[i];
        low = low.min(travel);
        high = high.max(travel);
        scale = scale.max(c[i].x.abs().max(c[i].y.abs()).max(c[i].z.abs()));
    }
    high - low > PLANE_EPS * scale
}

fn start_axis(normal: Vec3, axis: Vec3) -> Vec3 {
    if axis == Vec3::ZERO { normal } else { axis.normalized() }
}

/// Captures the start state of every face touching `verts` (`positions` = the start mesh's).
pub fn drag_uv_frames(m: &BMesh, verts: &BTreeSet<u32>) -> Frames {
    let mut out = Frames::new();
    for (fi, face) in m.faces.iter().enumerate() {
        if !face.verts.iter().any(|v| verts.contains(v)) {
            continue;
        }
        let corners = corners_from(face, &m.positions);
        let normal = newell(&corners);
        if normal.dot(normal) <= 0.0 {
            continue;
        }
        let dist = pair_distances(&corners);
        match face.manual_uvs() {
            None => {
                let unit = normal.normalized();
                out.insert(fi, Entry::Auto { normal: unit, axis: face.uv_axis, dist, lift: lifts(&corners, unit), adj: Vec::new() });
            }
            Some(uvs) => {
                if let Some(frame) = UvFrame::fit(&corners, uvs, normal) {
                    out.insert(fi, Entry::Manual { frame, uvs: uvs.to_vec(), dist });
                }
            }
        }
    }
    link_drag_patches(m, &mut out);
    out
}

/// Links each automatic entry to its neighbours across an edge that share its start
/// projection axis (texture-continuous patches).
pub fn link_drag_patches(m: &BMesh, frames: &mut Frames) {
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, e) in frames.iter_mut() {
        let Entry::Auto { adj, .. } = e else { continue };
        adj.clear();
        for (a, b) in m.faces[*fi].loop_edges() {
            by_edge.entry(edge(a, b)).or_default().push(*fi);
        }
    }
    let axis_of = |e: &Entry| match e {
        Entry::Auto { normal, axis, .. } => start_axis(*normal, *axis),
        Entry::Manual { .. } => Vec3::ZERO,
    };
    let mut links: Vec<(usize, usize)> = Vec::new();
    for sharing in by_edge.values() {
        for i in 0..sharing.len() {
            for j in i + 1..sharing.len() {
                if axis_of(&frames[&sharing[i]]).dot(axis_of(&frames[&sharing[j]])) >= 1.0 - TILT_EPS {
                    links.push((sharing[i], sharing[j]));
                }
            }
        }
    }
    links.sort_unstable();
    for (a, b) in links {
        for (x, y) in [(a, b), (b, a)] {
            if let Some(Entry::Auto { adj, .. }) = frames.get_mut(&x) {
                adj.push(y);
            }
        }
    }
}

/// Refits the dragged faces' UVs against the mesh's current positions.
pub fn refit_drag_uvs(m: &mut BMesh, frames: &Frames) {
    let mut sources: Vec<usize> = Vec::new();
    let mut rigid_tilted: BTreeMap<usize, Vec3> = BTreeMap::new();
    for (&fi, entry) in frames {
        if fi >= m.faces.len() {
            continue;
        }
        let corners = corners_from(&m.faces[fi], &m.positions);
        match entry {
            Entry::Auto { normal, axis, dist, lift, .. } => {
                let rigid = same_distances(&pair_distances(&corners), dist);
                let mut axis = *axis;
                if axis == Vec3::ZERO && tilted(lift, *normal, &corners) {
                    if rigid {
                        rigid_tilted.insert(fi, newell(&corners).normalized());
                    } else {
                        axis = *normal;
                    }
                }
                m.faces[fi].uv_axis = axis;
                if axis != Vec3::ZERO {
                    sources.push(fi);
                }
            }
            Entry::Manual { frame, uvs, dist } => {
                let face = &mut m.faces[fi];
                if face.manual_uvs().is_none() {
                    continue;
                }
                face.uv_coords = if same_distances(&pair_distances(&corners), dist) { uvs.clone() } else { corners.iter().map(|p| frame.uv_at(*p)).collect() };
            }
        }
    }
    // Rigidly tilted faces follow a texture-continuous neighbour that kept a fixed axis.
    while let Some(fi) = sources.pop() {
        if rigid_tilted.is_empty() {
            break;
        }
        let Some(Entry::Auto { adj, .. }) = frames.get(&fi) else { continue };
        let axis = m.faces[fi].uv_axis;
        for other in adj {
            let Some(n) = rigid_tilted.get(other) else { continue };
            if n.dot(axis.normalized()) < FOLLOW_DOT {
                continue;
            }
            rigid_tilted.remove(other);
            m.faces[*other].uv_axis = axis;
            sources.push(*other);
        }
    }
}

/// The manual faces' coordinates at the start (`start`) or now, for an undo entry.
pub fn drag_uv_coords(m: &BMesh, frames: &Frames, start: bool) -> BTreeMap<usize, Vec<Vec2>> {
    let mut out = BTreeMap::new();
    for (&fi, e) in frames {
        let Entry::Manual { uvs, .. } = e else { continue };
        if start {
            out.insert(fi, uvs.clone());
        } else if let Some(c) = m.faces.get(fi).and_then(|f| f.manual_uvs()) {
            out.insert(fi, c.to_vec());
        }
    }
    out
}

/// The automatic faces' projection axes at the start (`start`) or now, where they changed.
pub fn drag_uv_axes(m: &BMesh, frames: &Frames, start: bool) -> BTreeMap<usize, Vec3> {
    let mut out = BTreeMap::new();
    for (&fi, e) in frames {
        let Entry::Auto { axis, .. } = e else { continue };
        let Some(f) = m.faces.get(fi) else { continue };
        if f.uv_axis != *axis {
            out.insert(fi, if start { *axis } else { f.uv_axis });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::super::uv_edit::{AutoUv, bake_manual, face_uvs};
    use super::*;

    fn close(a: &[Vec2], b: &[Vec2]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (*x - *y).length() < 1e-4)
    }

    fn face_with_normal(m: &BMesh, n: Vec3) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).dot(n) > 0.9).unwrap()
    }

    #[test]
    fn frame_round_trips_the_uvs() {
        let c = [v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(2.0, 0.0, 1.0), v3(0.0, 0.0, 1.0)];
        let uv = [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 0.5), v2(0.0, 0.5)];
        let f = UvFrame::fit(&c, &uv, Vec3::UP).unwrap();
        for i in 0..4 {
            assert!((f.uv_at(c[i]) - uv[i]).length() < 1e-5);
        }
        // Squashed UVs (a line) cannot be inverted.
        assert!(UvFrame::fit(&c, &[v2(0.0, 0.0), v2(1.0, 0.0), v2(2.0, 0.0), v2(3.0, 0.0)], Vec3::UP).is_none());
        assert!(UvFrame::fit(&c[..2], &uv[..2], Vec3::UP).is_none());
    }

    #[test]
    fn manual_uvs_ride_along_a_rigid_move_and_stay_put_when_stretched() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = face_with_normal(&m, Vec3::UP);
        bake_manual(&mut m, &BTreeSet::from([top]), &AutoUv::default());
        let start_uvs = m.faces[top].uv_coords.clone();
        let verts: BTreeSet<u32> = m.faces[top].verts.iter().copied().collect();
        let frames = drag_uv_frames(&m, &verts);
        assert!(matches!(frames[&top], Entry::Manual { .. }));
        // Rigid: lift the whole top face.
        let mut moved = m.clone();
        for v in &verts {
            moved.positions[*v as usize].y += 0.5;
        }
        refit_drag_uvs(&mut moved, &frames);
        assert!(close(&moved.faces[top].uv_coords, &start_uvs));
        // Stretch: move one top vertex outwards in the plane -> UVs follow the old map.
        let v0 = m.faces[top].verts[0];
        let mut stretched = m.clone();
        stretched.positions[v0 as usize].x += 0.5;
        refit_drag_uvs(&mut stretched, &frames);
        let Entry::Manual { frame, .. } = &frames[&top] else { unreachable!() };
        let want = frame.uv_at(stretched.positions[v0 as usize]);
        assert!((stretched.faces[top].uv_coords[0] - want).length() < 1e-5);
        assert!((stretched.faces[top].uv_coords[0] - start_uvs[0]).length() > 0.1);
        // Undo entry data.
        assert_eq!(drag_uv_coords(&stretched, &frames, true)[&top], start_uvs);
        assert_eq!(drag_uv_coords(&stretched, &frames, false)[&top], stretched.faces[top].uv_coords);
    }

    #[test]
    fn deformed_auto_face_freezes_its_projection_axis() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = face_with_normal(&m, Vec3::UP);
        let v0 = m.faces[top].verts[0];
        let frames = drag_uv_frames(&m, &BTreeSet::from([v0]));
        assert_eq!(frames.len(), 3);
        let before = face_uvs(&m, top, &AutoUv::default());
        // Lift one corner of the top: it bends, its axis stays UP.
        m.positions[v0 as usize].y += 0.6;
        refit_drag_uvs(&mut m, &frames);
        assert_eq!(m.faces[top].uv_axis, Vec3::UP);
        let after = face_uvs(&m, top, &AutoUv::default());
        // The untouched corners keep their UVs (same projection).
        let moved_ci = 0;
        for ci in 0..before.len() {
            if ci != moved_ci {
                assert!((before[ci] - after[ci]).length() < 1e-5);
            }
        }
        let axes = drag_uv_axes(&m, &frames, false);
        assert_eq!(axes.get(&top), Some(&Vec3::UP));
        assert_eq!(drag_uv_axes(&m, &frames, true).get(&top), Some(&Vec3::ZERO));
    }

    #[test]
    fn rigid_move_keeps_auto_axes_free() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        let frames = drag_uv_frames(&m, &all);
        for p in &mut m.positions {
            *p = *p + v3(0.3, 0.2, 0.1);
        }
        refit_drag_uvs(&mut m, &frames);
        assert!(m.faces.iter().all(|f| f.uv_axis == Vec3::ZERO));
        // Rotating the whole box tilts every face rigidly: they re-project (no axis).
        let frames = drag_uv_frames(&m, &all);
        for p in &mut m.positions {
            let (s, c) = (0.3f32.sin(), 0.3f32.cos());
            *p = v3(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
        }
        refit_drag_uvs(&mut m, &frames);
        assert!(m.faces.iter().all(|f| f.uv_axis == Vec3::ZERO));
    }

    #[test]
    fn coplanar_neighbours_are_linked() {
        // Two coplanar quads sharing an edge.
        let mut m = BMesh::default();
        for p in [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(1.0, 0.0, 1.0), v3(2.0, 0.0, 1.0)] {
            m.add_vert(p);
        }
        m.add_face_facing(vec![0, 1, 4, 3], Vec3::UP);
        m.add_face_facing(vec![1, 2, 5, 4], Vec3::UP);
        let frames = drag_uv_frames(&m, &BTreeSet::from([1]));
        let Entry::Auto { adj, .. } = &frames[&0] else { unreachable!() };
        assert_eq!(adj, &vec![1]);
        // A rigidly tilted face follows a deformed (frozen) neighbour.
        let mut m2 = m.clone();
        m2.positions[2].y += 0.2; // bend face 1 (non-rigid) -> frozen axis UP
        let frames = drag_uv_frames(&m, &BTreeSet::from([2]));
        refit_drag_uvs(&mut m2, &frames);
        assert_eq!(m2.faces[1].uv_axis, Vec3::UP);
        assert_eq!(m2.faces[0].uv_axis, Vec3::ZERO);
    }
}
