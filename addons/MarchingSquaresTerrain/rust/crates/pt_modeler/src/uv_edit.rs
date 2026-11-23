use std::collections::{BTreeMap, BTreeSet};
use super::model::{BMesh, newell};
use super::uv::{projection_frame, self_auto_tex_coord};
use pt_geom::math::*;
pub type Corner = (usize, usize);
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub x: Vec2,
    pub y: Vec2,
    pub o: Vec2,
}
impl Affine2 {
    pub const IDENTITY: Affine2 = Affine2 {
        x: Vec2 { x: 1.0, y: 0.0 },
        y: Vec2 { x: 0.0, y: 1.0 },
        o: Vec2 { x: 0.0, y: 0.0 },
    };
    pub fn displace(t: Vec2) -> Affine2 {
        Affine2 {
            o: t,
            ..Self::IDENTITY
        }
    }
    pub fn spin(a: f32) -> Affine2 {
        let (s, c) = a.sin_cos();
        Affine2 {
            x: v2(c, s),
            y: v2(-s, c),
            o: Vec2::ZERO,
        }
    }
    pub fn magnify(s: Vec2) -> Affine2 {
        Affine2 {
            x: v2(s.x, 0.0),
            y: v2(0.0, s.y),
            o: Vec2::ZERO,
        }
    }
    pub fn enact(&self, p: Vec2) -> Vec2 {
        self.x * p.x + self.y * p.y + self.o
    }
    pub fn then_after(&self, other: &Affine2) -> Affine2 {
        Affine2 {
            x: self.x * other.x.x + self.y * other.x.y,
            y: self.x * other.y.x + self.y * other.y.y,
            o: self.enact(other.o),
        }
    }
    pub fn about(pivot: Vec2, rot: f32, s: Vec2) -> Affine2 {
        Self::displace(pivot)
            .then_after(&Self::spin(rot))
            .then_after(&Self::magnify(s))
            .then_after(&Self::displace(-pivot))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}
impl Rect {
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    pub fn midpoint(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn of(points: impl IntoIterator<Item = Vec2>) -> Option<Rect> {
        let mut it = points.into_iter();
        let first = it.next()?;
        Some(it.fold(
            Rect {
                min: first,
                max: first,
            },
            |r, p| Rect {
                min: v2(r.min.x.min(p.x), r.min.y.min(p.y)),
                max: v2(r.max.x.max(p.x), r.max.y.max(p.y)),
            },
        ))
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AutoUv {
    pub units_per_repeat: f32,
    pub world_offset: Vec3,
}
impl Default for AutoUv {
    fn default() -> Self {
        AutoUv {
            units_per_repeat: 1.0,
            world_offset: Vec3::ZERO,
        }
    }
}
pub fn face_corner_uvs(m: &BMesh, f: usize, a: &AutoUv) -> Vec<Vec2> {
    let face = &m.facets[f];
    if let Some(c) = face.by_hand_tex_coords() {
        return c.to_vec();
    }
    let n = m.face_normal(f);
    face.verts
        .iter()
        .map(|v| {
            let p = m.positions[*v as usize];
            self_auto_tex_coord(
                if face.tex_coord.world {
                    p + a.world_offset
                } else {
                    p
                },
                face.uv_normal(n),
                &face.tex_coord,
                a.units_per_repeat,
            )
        })
        .collect()
}
fn valid_faces(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<usize> {
    faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect()
}
pub fn corners_of(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<Corner> {
    let mut out = BTreeSet::new();
    for f in valid_faces(m, faces) {
        for c in 0..m.facets[f].verts.len() {
            out.insert((f, c));
        }
    }
    out
}
pub fn faces_of(corners: &BTreeSet<Corner>) -> BTreeSet<usize> {
    corners.iter().map(|c| c.0).collect()
}
fn is_manual(m: &BMesh, f: usize) -> bool {
    m.facets
        .get(f)
        .is_some_and(|x| x.by_hand_tex_coords().is_some())
}
pub fn freeze_manual_uvs(m: &mut BMesh, faces: &BTreeSet<usize>, a: &AutoUv) {
    for f in valid_faces(m, faces) {
        if is_manual(m, f) {
            continue;
        }
        let uvs = face_corner_uvs(m, f, a);
        let face = &mut m.facets[f];
        face.uv_coords = uvs;
        face.uv_manual = true;
    }
}
pub fn drop_manual(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for f in valid_faces(m, faces) {
        m.facets[f].uv_manual = false;
        m.facets[f].uv_coords.clear();
    }
}
pub fn transform_uvs(m: &mut BMesh, corners: &BTreeSet<Corner>, xf: &Affine2) {
    for &(f, c) in corners {
        if is_manual(m, f) && c < m.facets[f].uv_coords.len() {
            let p = m.facets[f].uv_coords[c];
            m.facets[f].uv_coords[c] = xf.enact(p);
        }
    }
}
pub fn set_uvs(m: &mut BMesh, uvs: &BTreeMap<Corner, Vec2>) {
    for (&(f, c), uv) in uvs {
        if is_manual(m, f) && c < m.facets[f].uv_coords.len() {
            m.facets[f].uv_coords[c] = *uv;
        }
    }
}
pub fn planar_project(m: &mut BMesh, faces: &BTreeSet<usize>, fit: bool) {
    let fs = valid_faces(m, faces);
    let mut normal = fs.iter().fold(Vec3::ZERO, |a, f| a + m.face_normal(*f));
    if normal.innerprod(normal) < 1e-10 {
        normal = Vec3::UP;
    }
    let normal = normal.unitized();
    let axis = if normal.x.abs() < 0.9005 {
        v3(1.0, 0.0, 0.0)
    } else {
        Vec3::UP
    };
    let u = (axis - normal * axis.innerprod(normal)).unitized();
    let w = normal.cross(u);
    for f in &fs {
        let coords = m.facets[*f].verts.iter().map(|v| {
            let p = m.positions[*v as usize];
            v2(p.innerprod(u), p.innerprod(w))
        });
        let coords: Vec<Vec2> = coords.collect();
        m.facets[*f].uv_coords = coords;
        m.facets[*f].uv_manual = true;
    }
    if fit {
        fit_to_unit(m, faces, FitAxis::Both);
    }
}
pub fn box_project(m: &mut BMesh, faces: &BTreeSet<usize>, fit: bool) {
    for f in valid_faces(m, faces) {
        let (u, w) = projection_frame(m.face_normal(f));
        let coords: Vec<Vec2> = m.facets[f]
            .verts
            .iter()
            .map(|v| {
                let p = m.positions[*v as usize];
                v2(p.innerprod(u), p.innerprod(w))
            })
            .collect();
        m.facets[f].uv_coords = coords;
        m.facets[f].uv_manual = true;
    }
    if fit {
        fit_to_unit(m, faces, FitAxis::Both);
    }
}
pub fn weld_uvs(m: &mut BMesh, corners: &BTreeSet<Corner>) {
    let mut by_vertex: BTreeMap<u32, Vec<Corner>> = BTreeMap::new();
    for &(f, c) in corners {
        if is_manual(m, f) && c < m.facets[f].verts.len() {
            by_vertex
                .entry(m.facets[f].verts[c])
                .or_default()
                .push((f, c));
        }
    }
    for keys in by_vertex.values() {
        if keys.len() < 2 {
            continue;
        }
        let avg = keys
            .iter()
            .fold(Vec2::ZERO, |a, (f, c)| a + m.facets[*f].uv_coords[*c])
            / keys.len() as f32;
        for (f, c) in keys {
            m.facets[*f].uv_coords[*c] = avg;
        }
    }
}
pub fn uv_bounds(m: &BMesh, corners: &BTreeSet<Corner>) -> Option<Rect> {
    Rect::of(
        corners
            .iter()
            .filter(|(f, c)| is_manual(m, *f) && *c < m.facets[*f].uv_coords.len())
            .map(|(f, c)| m.facets[*f].uv_coords[*c]),
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitAxis {
    Both,
    U,
    V,
}
pub fn fit_to_unit(m: &mut BMesh, faces: &BTreeSet<usize>, axis: FitAxis) {
    let corners = corners_of(m, faces);
    let Some(r) = uv_bounds(m, &corners) else {
        return;
    };
    let size = r.size();
    let extent = match axis {
        FitAxis::Both => size.x.max(size.y),
        FitAxis::U => size.x,
        FitAxis::V => size.y,
    };
    let s = if extent > 1e-9 { 1.0 / extent } else { 1.0 };
    let xf = Affine2 {
        x: v2(s, 0.0),
        y: v2(0.0, s),
        o: r.min * -s,
    };
    transform_uvs(m, &corners, &xf);
}
fn unit_lock_axis(lo: f32, hi: f32) -> f32 {
    let (a, b) = (-lo, 1.0 - hi);
    0.0f32.clamp(a.min(b), a.max(b))
}
pub fn unit_lock_offset(r: Rect) -> Vec2 {
    v2(
        unit_lock_axis(r.min.x, r.max.x),
        unit_lock_axis(r.min.y, r.max.y),
    )
}
pub fn unit_lock_factors(size: Vec2, factors: Vec2, uniform: bool) -> Vec2 {
    let mut k = [1.0f32, 1.0];
    for (a, (s, f)) in [(size.x, factors.x), (size.y, factors.y)]
        .into_iter()
        .enumerate()
    {
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
pub fn island_faces(m: &BMesh, seed: usize, eps: f32) -> BTreeSet<usize> {
    if seed >= m.facets.len() {
        return BTreeSet::new();
    }
    if !is_manual(m, seed) {
        return BTreeSet::from([seed]);
    }
    let mut by_vertex: BTreeMap<u32, Vec<(usize, Vec2)>> = BTreeMap::new();
    for (fi, f) in m.facets.iter().enumerate() {
        if let Some(c) = f.by_hand_tex_coords() {
            for (ci, v) in f.verts.iter().enumerate() {
                by_vertex.entry(*v).or_default().push((fi, c[ci]));
            }
        }
    }
    let mut out = BTreeSet::from([seed]);
    let mut queue = vec![seed];
    while let Some(fi) = queue.pop() {
        let f = &m.facets[fi];
        for (ci, v) in f.verts.iter().enumerate() {
            let uv = f.uv_coords[ci];
            for (nf, nuv) in by_vertex.get(v).map(|x| x.as_slice()).unwrap_or(&[]) {
                if !out.contains(nf) && nuv.span(uv) <= eps {
                    out.insert(*nf);
                    queue.push(*nf);
                }
            }
        }
    }
    out
}
pub fn copy_uvs(m: &mut BMesh, coords: &[Vec2], faces: &BTreeSet<usize>) -> usize {
    let mut n = 0;
    for f in valid_faces(m, faces) {
        if m.facets[f].verts.len() == coords.len() {
            m.facets[f].uv_coords = coords.to_vec();
            m.facets[f].uv_manual = true;
            n += 1;
        }
    }
    n
}
fn face_world_area(m: &BMesh, f: usize) -> f32 {
    newell(&m.face_points(f)).magnitude() * 0.5
}
pub fn face_uv_area(m: &BMesh, f: usize) -> f32 {
    let Some(c) = m.facets.get(f).and_then(|x| x.by_hand_tex_coords()) else {
        return 0.0;
    };
    let mut a = 0.0;
    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p.x * q.y - p.y * q.x;
    }
    a.abs() * 0.5
}
pub fn texel_normalize(m: &mut BMesh, faces: &BTreeSet<usize>, reference: usize) {
    if reference >= m.facets.len() {
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
        let island = island_faces(m, f, 0.0005);
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
        let Some(r) = uv_bounds(m, &corners) else {
            continue;
        };
        transform_uvs(m, &corners, &Affine2::about(r.midpoint(), 0.0, v2(s, s)));
    }
}
pub fn transform_about_bounds(
    m: &mut BMesh,
    corners: &BTreeSet<Corner>,
    scale: Vec2,
    rot: f32,
    lock: bool,
    a: &AutoUv,
) {
    freeze_manual_uvs(m, &faces_of(corners), a);
    let Some(b) = uv_bounds(m, corners) else {
        return;
    };
    let s = if lock {
        unit_lock_factors(b.size(), scale, (scale.x - scale.y).abs() < 1e-6)
    } else {
        scale
    };
    transform_uvs(m, corners, &Affine2::about(b.midpoint(), rot, s));
    if lock && let Some(nb) = uv_bounds(m, corners) {
        let shift = unit_lock_offset(nb);
        if shift != Vec2::ZERO {
            transform_uvs(m, corners, &Affine2::displace(shift));
        }
    }
}
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
        (0..m.facets.len()).collect()
    }
    #[test]
    fn freeze_keeps_auto_uvs_and_drop_restores() {
        let mut m = cuboid(v3(2.0, 1.0, 3.0));
        let a = AutoUv::default();
        let before: Vec<Vec<Vec2>> = (0..m.facets.len())
            .map(|f| face_corner_uvs(&m, f, &a))
            .collect();
        let fs = all(&m);
        freeze_manual_uvs(&mut m, &fs, &a);
        assert!(m.facets.iter().all(|f| f.by_hand_tex_coords().is_some()));
        for f in 0..m.facets.len() {
            assert_eq!(face_corner_uvs(&m, f, &a), before[f]);
        }
        drop_manual(&mut m, &fs);
        assert!(m.facets.iter().all(|f| !f.uv_manual));
    }
    #[test]
    fn fit_and_box_project_fill_the_unit_square() {
        let mut m = cuboid(v3(4.0, 2.0, 1.0));
        let faces = all(&m);
        box_project(&mut m, &faces, true);
        let r = uv_bounds(&m, &corners_of(&m, &faces)).unwrap();
        assert!(r.min.magnitude() < 1e-5);
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
        let before = m.facets[0].uv_coords.clone();
        transform_about_bounds(&mut m, &c, v2(-1.0, 1.0), 0.0, false, &AutoUv::default());
        for (p, q) in before.iter().zip(&m.facets[0].uv_coords) {
            assert!(
                (p.x + q.x - 1.0).abs() < 1e-5 && (p.y - q.y).abs() < 1e-5,
                "flip U mirrors about the centre"
            );
        }
        transform_about_bounds(&mut m, &c, v2(3.0, 3.0), 0.0, true, &AutoUv::default());
        let r = uv_bounds(&m, &c).unwrap();
        assert!(
            r.min.x >= -1e-5 && r.max.x <= 1.0 + 1e-5,
            "locked scale stays in the unit square: {r:?}"
        );
        let rot = Affine2::about(v2(0.5, 0.5), std::f32::consts::FRAC_PI_2, v2(1.0, 1.0));
        let p = rot.enact(v2(1.0, 0.5));
        assert!((p.x - 0.5).abs() < 1e-5 && (p.y - 1.0).abs() < 1e-5);
    }
    #[test]
    fn sew_island_copy_texel() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let faces = all(&m);
        box_project(&mut m, &faces, false);
        let isl = island_faces(&m, 0, 1e-5);
        assert!(isl.contains(&0));
        let cs = corners_of(&m, &faces);
        weld_uvs(&mut m, &cs);
        assert_eq!(island_faces(&m, 0, 1e-5).len(), m.facets.len());
        let coords: Vec<Vec2> = (0..4).map(|i| v2(i as f32, 0.0)).collect();
        assert_eq!(copy_uvs(&mut m, &coords, &BTreeSet::from([1, 2])), 2);
        assert_eq!(m.facets[2].uv_coords, coords);
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        box_project(&mut m, &faces, true);
        let c1 = corners_of(&m, &BTreeSet::from([1]));
        transform_uvs(&mut m, &c1, &Affine2::magnify(v2(2.0, 2.0)));
        texel_normalize(&mut m, &BTreeSet::from([0, 1]), 0);
        assert!((face_uv_area(&m, 1) - face_uv_area(&m, 0)).abs() < 1e-4);
    }
    #[test]
    fn flip_keeps_manual_uvs_on_their_corners() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        box_project(&mut m, &BTreeSet::from([0]), true);
        let pairs: Vec<(u32, Vec2)> = m.facets[0]
            .verts
            .iter()
            .copied()
            .zip(m.facets[0].uv_coords.iter().copied())
            .collect();
        super::super::selection_edits::turnover_facets(&mut m, &BTreeSet::from([0]));
        for (v, uv) in m.facets[0].verts.iter().zip(&m.facets[0].uv_coords) {
            assert!(pairs.contains(&(*v, *uv)));
        }
        let g = m.facets[0].with_verts(vec![0, 1, 2]);
        assert!(g.by_hand_tex_coords().is_none());
    }
    #[test]
    fn picks_points_in_polygons() {
        let sq = [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)];
        assert!(point_in_polygon(v2(0.5, 0.5), &sq));
        assert!(!point_in_polygon(v2(1.5, 0.5), &sq));
    }
}
