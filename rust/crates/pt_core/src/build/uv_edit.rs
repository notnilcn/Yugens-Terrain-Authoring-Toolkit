//! Manual-UV operations for the UV editor: port of Unibuilder's `UBOpUV` (`core/ops/uv_ops.gd`,
//! MIT) onto the builder model. A face is *manual* when `uv_manual` is on and `uv_coords` holds
//! one UV per corner; every operation that moves UVs first bakes the automatic projection into
//! manual coordinates (`bake_manual`), exactly like Unibuilder.
//!
//! Corners are addressed as `(face, corner index in the loop)`.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{BMesh, newell};
use super::uv::{auto_uv, projection_frame};
use crate::math::*;

pub type Corner = (usize, usize);

/// 2D affine transform (Godot `Transform2D`): `p' = x * p.x + y * p.y + o`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub x: Vec2,
    pub y: Vec2,
    pub o: Vec2,
}

impl Affine2 {
    pub const IDENTITY: Affine2 = Affine2 { x: Vec2 { x: 1.0, y: 0.0 }, y: Vec2 { x: 0.0, y: 1.0 }, o: Vec2 { x: 0.0, y: 0.0 } };
    pub fn translate(t: Vec2) -> Affine2 {
        Affine2 { o: t, ..Self::IDENTITY }
    }
    pub fn rotate(a: f32) -> Affine2 {
        let (s, c) = a.sin_cos();
        Affine2 { x: v2(c, s), y: v2(-s, c), o: Vec2::ZERO }
    }
    pub fn scale(s: Vec2) -> Affine2 {
        Affine2 { x: v2(s.x, 0.0), y: v2(0.0, s.y), o: Vec2::ZERO }
    }
    pub fn apply(&self, p: Vec2) -> Vec2 {
        self.x * p.x + self.y * p.y + self.o
    }
    /// `self * other` (apply `other` first).
    pub fn then_after(&self, other: &Affine2) -> Affine2 {
        Affine2 { x: self.x * other.x.x + self.y * other.x.y, y: self.x * other.y.x + self.y * other.y.y, o: self.apply(other.o) }
    }
    /// Rotation by `rot` and scale `s` about `pivot` (scale applied first, as in Unibuilder's
    /// `_transform_about_center`).
    pub fn about(pivot: Vec2, rot: f32, s: Vec2) -> Affine2 {
        Self::translate(pivot).then_after(&Self::rotate(rot)).then_after(&Self::scale(s)).then_after(&Self::translate(-pivot))
    }
}

/// Axis-aligned UV rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn of(points: impl IntoIterator<Item = Vec2>) -> Option<Rect> {
        let mut it = points.into_iter();
        let first = it.next()?;
        Some(it.fold(Rect { min: first, max: first }, |r, p| Rect { min: v2(r.min.x.min(p.x), r.min.y.min(p.y)), max: v2(r.max.x.max(p.x), r.max.y.max(p.y)) }))
    }
}

/// Settings of the automatic projection (same as `compile::CompileOptions`).
#[derive(Clone, Copy, Debug)]
pub struct AutoUv {
    pub units_per_repeat: f32,
    pub world_offset: Vec3,
}

impl Default for AutoUv {
    fn default() -> Self {
        AutoUv { units_per_repeat: 1.0, world_offset: Vec3::ZERO }
    }
}

/// The UVs a face shows: its manual coordinates, else the automatic projection
/// (Unibuilder `UBUVUnwrap.face_uvs`).
pub fn face_uvs(m: &BMesh, f: usize, a: &AutoUv) -> Vec<Vec2> {
    let face = &m.faces[f];
    if let Some(c) = face.manual_uvs() {
        return c.to_vec();
    }
    let n = m.face_normal(f);
    face.verts
        .iter()
        .map(|v| {
            let p = m.positions[*v as usize];
            auto_uv(if face.uv.world { p + a.world_offset } else { p }, face.uv_normal(n), &face.uv, a.units_per_repeat)
        })
        .collect()
}

fn valid_faces(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<usize> {
    faces.iter().copied().filter(|f| *f < m.faces.len()).collect()
}

/// Every corner of every face in the set.
pub fn corners_of(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<Corner> {
    let mut out = BTreeSet::new();
    for f in valid_faces(m, faces) {
        for c in 0..m.faces[f].verts.len() {
            out.insert((f, c));
        }
    }
    out
}

/// Faces touched by a corner set.
pub fn faces_of(corners: &BTreeSet<Corner>) -> BTreeSet<usize> {
    corners.iter().map(|c| c.0).collect()
}

fn is_manual(m: &BMesh, f: usize) -> bool {
    m.faces.get(f).is_some_and(|x| x.manual_uvs().is_some())
}

/// Converts faces to manual UVs from their current automatic projection.
pub fn bake_manual(m: &mut BMesh, faces: &BTreeSet<usize>, a: &AutoUv) {
    for f in valid_faces(m, faces) {
        if is_manual(m, f) {
            continue;
        }
        let uvs = face_uvs(m, f, a);
        let face = &mut m.faces[f];
        face.uv_coords = uvs;
        face.uv_manual = true;
    }
}

/// Returns faces to the automatic projection.
pub fn clear_manual(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for f in valid_faces(m, faces) {
        m.faces[f].uv_manual = false;
        m.faces[f].uv_coords.clear();
    }
}

/// Applies an affine transform to the given corners' (manual) UVs.
pub fn transform_uvs(m: &mut BMesh, corners: &BTreeSet<Corner>, xf: &Affine2) {
    for &(f, c) in corners {
        if is_manual(m, f) && c < m.faces[f].uv_coords.len() {
            let p = m.faces[f].uv_coords[c];
            m.faces[f].uv_coords[c] = xf.apply(p);
        }
    }
}

/// Sets corners' UVs outright (a drag passes original + delta).
pub fn set_uvs(m: &mut BMesh, uvs: &BTreeMap<Corner, Vec2>) {
    for (&(f, c), uv) in uvs {
        if is_manual(m, f) && c < m.faces[f].uv_coords.len() {
            m.faces[f].uv_coords[c] = *uv;
        }
    }
}

/// Projects the set as one island along its average normal; makes the faces manual.
pub fn planar_project(m: &mut BMesh, faces: &BTreeSet<usize>, fit: bool) {
    let fs = valid_faces(m, faces);
    let mut normal = fs.iter().fold(Vec3::ZERO, |a, f| a + m.face_normal(*f));
    if normal.dot(normal) < 1e-10 {
        normal = Vec3::UP;
    }
    let normal = normal.normalized();
    let axis = if normal.x.abs() < 0.9 { v3(1.0, 0.0, 0.0) } else { Vec3::UP };
    let u = (axis - normal * axis.dot(normal)).normalized();
    let w = normal.cross(u);
    for f in &fs {
        let coords = m.faces[*f].verts.iter().map(|v| {
            let p = m.positions[*v as usize];
            v2(p.dot(u), p.dot(w))
        });
        let coords: Vec<Vec2> = coords.collect();
        m.faces[*f].uv_coords = coords;
        m.faces[*f].uv_manual = true;
    }
    if fit {
        fit_to_unit(m, faces, FitAxis::Both);
    }
}

/// Per-face dominant-axis projection; makes the faces manual.
pub fn box_project(m: &mut BMesh, faces: &BTreeSet<usize>, fit: bool) {
    for f in valid_faces(m, faces) {
        let (u, w) = projection_frame(m.face_normal(f));
        let coords: Vec<Vec2> = m.faces[f]
            .verts
            .iter()
            .map(|v| {
                let p = m.positions[*v as usize];
                v2(p.dot(u), p.dot(w))
            })
            .collect();
        m.faces[f].uv_coords = coords;
        m.faces[f].uv_manual = true;
    }
    if fit {
        fit_to_unit(m, faces, FitAxis::Both);
    }
}

/// Sew: corners of the set that share a 3D vertex move to their average UV.
pub fn weld_uvs(m: &mut BMesh, corners: &BTreeSet<Corner>) {
    let mut by_vertex: BTreeMap<u32, Vec<Corner>> = BTreeMap::new();
    for &(f, c) in corners {
        if is_manual(m, f) && c < m.faces[f].verts.len() {
            by_vertex.entry(m.faces[f].verts[c]).or_default().push((f, c));
        }
    }
    for keys in by_vertex.values() {
        if keys.len() < 2 {
            continue;
        }
        let avg = keys.iter().fold(Vec2::ZERO, |a, (f, c)| a + m.faces[*f].uv_coords[*c]) / keys.len() as f32;
        for (f, c) in keys {
            m.faces[*f].uv_coords[*c] = avg;
        }
    }
}

/// Bounding rectangle of the set's manual UVs.
pub fn uv_bounds(m: &BMesh, corners: &BTreeSet<Corner>) -> Option<Rect> {
    Rect::of(corners.iter().filter(|(f, c)| is_manual(m, *f) && *c < m.faces[*f].uv_coords.len()).map(|(f, c)| m.faces[*f].uv_coords[*c]))
}

/// Which extent `fit_to_unit` brings to exactly 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitAxis {
    Both,
    U,
    V,
}

/// Scales the set uniformly and moves its minimum corner to the origin.
pub fn fit_to_unit(m: &mut BMesh, faces: &BTreeSet<usize>, axis: FitAxis) {
    let corners = corners_of(m, faces);
    let Some(r) = uv_bounds(m, &corners) else { return };
    let size = r.size();
    let extent = match axis {
        FitAxis::Both => size.x.max(size.y),
        FitAxis::U => size.x,
        FitAxis::V => size.y,
    };
    let s = if extent > 1e-9 { 1.0 / extent } else { 1.0 };
    let xf = Affine2 { x: v2(s, 0.0), y: v2(0.0, s), o: r.min * -s };
    transform_uvs(m, &corners, &xf);
}

fn unit_lock_axis(lo: f32, hi: f32) -> f32 {
    let (a, b) = (-lo, 1.0 - hi);
    0.0f32.clamp(a.min(b), a.max(b))
}

/// The smallest translation that puts `r` inside the unit square.
pub fn unit_lock_offset(r: Rect) -> Vec2 {
    v2(unit_lock_axis(r.min.x, r.max.x), unit_lock_axis(r.min.y, r.max.y))
}

/// Scale factors capped so an island of `size` stays within the unit square.
pub fn unit_lock_factors(size: Vec2, factors: Vec2, uniform: bool) -> Vec2 {
    let mut k = [1.0f32, 1.0];
    for (a, (s, f)) in [(size.x, factors.x), (size.y, factors.y)].into_iter().enumerate() {
        let grown = (s * f).abs();
        let limit = s.max(1.0);
        if grown > limit && grown > 1e-12 {
            k[a] = limit / grown;
        }
    }
    if uniform {
        let mn = k[0].min(k[1]);
        k = [mn, mn];
    }
    v2(factors.x * k[0], factors.y * k[1])
}

/// Faces in the same UV island as `seed` (sharing a vertex at the same UV).
pub fn island_faces(m: &BMesh, seed: usize, eps: f32) -> BTreeSet<usize> {
    if seed >= m.faces.len() {
        return BTreeSet::new();
    }
    if !is_manual(m, seed) {
        return BTreeSet::from([seed]);
    }
    let mut by_vertex: BTreeMap<u32, Vec<(usize, Vec2)>> = BTreeMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        if let Some(c) = f.manual_uvs() {
            for (ci, v) in f.verts.iter().enumerate() {
                by_vertex.entry(*v).or_default().push((fi, c[ci]));
            }
        }
    }
    let mut out = BTreeSet::from([seed]);
    let mut queue = vec![seed];
    while let Some(fi) = queue.pop() {
        let f = &m.faces[fi];
        for (ci, v) in f.verts.iter().enumerate() {
            let uv = f.uv_coords[ci];
            for (nf, nuv) in by_vertex.get(v).map(|x| x.as_slice()).unwrap_or(&[]) {
                if !out.contains(nf) && nuv.distance(uv) <= eps {
                    out.insert(*nf);
                    queue.push(*nf);
                }
            }
        }
    }
    out
}

/// Copies UVs onto every same-arity target face; returns the count applied.
pub fn copy_uvs(m: &mut BMesh, coords: &[Vec2], faces: &BTreeSet<usize>) -> usize {
    let mut n = 0;
    for f in valid_faces(m, faces) {
        if m.faces[f].verts.len() == coords.len() {
            m.faces[f].uv_coords = coords.to_vec();
            m.faces[f].uv_manual = true;
            n += 1;
        }
    }
    n
}

fn face_world_area(m: &BMesh, f: usize) -> f32 {
    newell(&m.face_points(f)).length() * 0.5
}

/// UV-space area of a manual face (shoelace).
pub fn face_uv_area(m: &BMesh, f: usize) -> f32 {
    let Some(c) = m.faces.get(f).and_then(|x| x.manual_uvs()) else { return 0.0 };
    let mut a = 0.0;
    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p.x * q.y - p.y * q.x;
    }
    a.abs() * 0.5
}

/// Matches every touched island's texel density to the reference face.
pub fn texel_normalize(m: &mut BMesh, faces: &BTreeSet<usize>, reference: usize) {
    if reference >= m.faces.len() {
        return;
    }
    let (ref_world, ref_uv) = (face_world_area(m, reference), face_uv_area(m, reference));
    if ref_world <= 1e-12 || ref_uv <= 1e-12 {
        return;
    }
    let ref_density = (ref_uv / ref_world).sqrt();
    let mut done = BTreeSet::new();
    for f in valid_faces(m, faces) {
        if done.contains(&f) || !is_manual(m, f) {
            continue;
        }
        let island = island_faces(m, f, 1e-5);
        let (mut world, mut uv) = (0.0, 0.0);
        for i in &island {
            done.insert(*i);
            world += face_world_area(m, *i);
            uv += face_uv_area(m, *i);
        }
        if world <= 1e-12 || uv <= 1e-12 {
            continue;
        }
        let s = ref_density / (uv / world).sqrt();
        if (s - 1.0).abs() < 1e-6 {
            continue;
        }
        let corners = corners_of(m, &island);
        let Some(r) = uv_bounds(m, &corners) else { continue };
        transform_uvs(m, &corners, &Affine2::about(r.center(), 0.0, v2(s, s)));
    }
}

/// Transforms corners about the centre of their bounds (Unibuilder `_transform_about_center`):
/// bakes the faces first, optionally keeps the island inside the unit square (`lock`).
pub fn transform_about_center(m: &mut BMesh, corners: &BTreeSet<Corner>, scale: Vec2, rot: f32, lock: bool, a: &AutoUv) {
    bake_manual(m, &faces_of(corners), a);
    let Some(b) = uv_bounds(m, corners) else { return };
    let s = if lock { unit_lock_factors(b.size(), scale, (scale.x - scale.y).abs() < 1e-6) } else { scale };
    transform_uvs(m, corners, &Affine2::about(b.center(), rot, s));
    if lock && let Some(nb) = uv_bounds(m, corners) {
        let shift = unit_lock_offset(nb);
        if shift != Vec2::ZERO {
            transform_uvs(m, corners, &Affine2::translate(shift));
        }
    }
}

/// Point-in-polygon test (even-odd), for picking faces in UV space.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
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

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    fn all(m: &BMesh) -> BTreeSet<usize> {
        (0..m.faces.len()).collect()
    }

    #[test]
    fn bake_keeps_auto_uvs_and_clear_restores() {
        let mut m = cuboid(v3(2.0, 1.0, 3.0));
        let a = AutoUv::default();
        let before: Vec<Vec<Vec2>> = (0..m.faces.len()).map(|f| face_uvs(&m, f, &a)).collect();
        let fs = all(&m);
        bake_manual(&mut m, &fs, &a);
        assert!(m.faces.iter().all(|f| f.manual_uvs().is_some()));
        for f in 0..m.faces.len() {
            assert_eq!(face_uvs(&m, f, &a), before[f]);
        }
        clear_manual(&mut m, &fs);
        assert!(m.faces.iter().all(|f| !f.uv_manual));
    }

    #[test]
    fn fit_and_box_project_fill_the_unit_square() {
        let mut m = cuboid(v3(4.0, 2.0, 1.0));
        let faces = all(&m);
        box_project(&mut m, &faces, true);
        let r = uv_bounds(&m, &corners_of(&m, &faces)).unwrap();
        assert!(r.min.length() < 1e-5);
        assert!((r.size().x.max(r.size().y) - 1.0).abs() < 1e-5);
        planar_project(&mut m, &BTreeSet::from([0]), true);
        let r = uv_bounds(&m, &corners_of(&m, &BTreeSet::from([0]))).unwrap();
        assert!((r.size().x.max(r.size().y) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn transforms_flip_rotate_and_lock() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let f0 = BTreeSet::from([0]);
        box_project(&mut m, &f0, true);
        let c = corners_of(&m, &f0);
        let before = m.faces[0].uv_coords.clone();
        transform_about_center(&mut m, &c, v2(-1.0, 1.0), 0.0, false, &AutoUv::default());
        for (p, q) in before.iter().zip(&m.faces[0].uv_coords) {
            assert!((p.x + q.x - 1.0).abs() < 1e-5 && (p.y - q.y).abs() < 1e-5, "flip U mirrors about the centre");
        }
        transform_about_center(&mut m, &c, v2(3.0, 3.0), 0.0, true, &AutoUv::default());
        let r = uv_bounds(&m, &c).unwrap();
        assert!(r.min.x >= -1e-5 && r.max.x <= 1.0 + 1e-5, "locked scale stays in the unit square: {r:?}");
        let rot = Affine2::about(v2(0.5, 0.5), std::f32::consts::FRAC_PI_2, v2(1.0, 1.0));
        let p = rot.apply(v2(1.0, 0.5));
        assert!((p.x - 0.5).abs() < 1e-5 && (p.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn sew_island_copy_texel() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let faces = all(&m);
        box_project(&mut m, &faces, false);
        // Separate islands per face (box projection does not share UVs across the cube edges).
        let isl = island_faces(&m, 0, 1e-5);
        assert!(isl.contains(&0));
        let cs = corners_of(&m, &faces);
        weld_uvs(&mut m, &cs);
        // After sewing, every face shares its corner UVs with its neighbours: one island.
        assert_eq!(island_faces(&m, 0, 1e-5).len(), m.faces.len());
        let coords: Vec<Vec2> = (0..4).map(|i| v2(i as f32, 0.0)).collect();
        assert_eq!(copy_uvs(&mut m, &coords, &BTreeSet::from([1, 2])), 2);
        assert_eq!(m.faces[2].uv_coords, coords);
        // Texel density: a face scaled twice in UV space gets scaled back.
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        box_project(&mut m, &faces, true);
        let c1 = corners_of(&m, &BTreeSet::from([1]));
        transform_uvs(&mut m, &c1, &Affine2::scale(v2(2.0, 2.0)));
        texel_normalize(&mut m, &BTreeSet::from([0, 1]), 0);
        assert!((face_uv_area(&m, 1) - face_uv_area(&m, 0)).abs() < 1e-4);
    }

    #[test]
    fn flip_keeps_manual_uvs_on_their_corners() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        box_project(&mut m, &BTreeSet::from([0]), true);
        let pairs: Vec<(u32, Vec2)> = m.faces[0].verts.iter().copied().zip(m.faces[0].uv_coords.iter().copied()).collect();
        super::super::ops::flip_faces(&mut m, &BTreeSet::from([0]));
        for (v, uv) in m.faces[0].verts.iter().zip(&m.faces[0].uv_coords) {
            assert!(pairs.contains(&(*v, *uv)));
        }
        // A changed loop drops manual UVs (falls back to auto).
        let g = m.faces[0].with_verts(vec![0, 1, 2]);
        assert!(g.manual_uvs().is_none());
    }

    #[test]
    fn picks_points_in_polygons() {
        let sq = [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)];
        assert!(point_in_polygon(v2(0.5, 0.5), &sq));
        assert!(!point_in_polygon(v2(1.5, 0.5), &sq));
    }
}
