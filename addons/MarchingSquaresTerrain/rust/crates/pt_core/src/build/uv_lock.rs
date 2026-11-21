use std::collections::{BTreeMap, BTreeSet, HashMap};
use super::model::{BMesh, Face, edge, newell};
use super::uv::projection_frame;
use crate::math::*;
pub const NEIGHBOUR_DOT: f32 = 0.7074;
pub const SAME_AXIS_EPS: f32 = 0.0005;
pub const FLAT_EPS: f32 = 0.0005;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvFrame {
    pub origin: Vec3,
    pub upright: Vec3,
    pub t: Vec3,
    pub b: Vec3,
    pub uv0: Vec2,
    pub du: Vec2,
    pub dv: Vec2,
}
impl UvFrame {
    pub fn adapt(corners: &[Vec3], uvs: &[Vec2], face_normal: Vec3) -> Option<UvFrame> {
        let n = corners.len();
        if n < 3 || uvs.len() != n {
            return None;
        }
        let (t, b) = projection_frame(face_normal);
        let origin = corners.iter().fold(Vec3::ZERO, |a, p| a + *p) / n as f32;
        let uv0 = uvs.iter().fold(Vec2::ZERO, |a, p| a + *p) / n as f32;
        let mut f = UvFrame {
            origin,
            upright: face_normal.unitized(),
            t,
            b,
            uv0,
            du: Vec2::ZERO,
            dv: Vec2::ZERO,
        };
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
        f.du = v2(
            ((sxu[0] * syy - syu[0] * sxy) / det) as f32,
            ((sxu[1] * syy - syu[1] * sxy) / det) as f32,
        );
        f.dv = v2(
            ((syu[0] * sxx - sxu[0] * sxy) / det) as f32,
            ((syu[1] * sxx - sxu[1] * sxy) / det) as f32,
        );
        let fdet = (f.du.x * f.dv.y - f.du.y * f.dv.x).abs();
        if fdet <= 1e-12 || fdet < 1e-9 * (f.du.magnitude_quadratic() + f.dv.magnitude_quadratic())
        {
            return None;
        }
        Some(f)
    }
    fn local(&self, p: Vec3) -> Vec2 {
        let d = p - self.origin;
        v2(d.innerprod(self.t), d.innerprod(self.b))
    }
    pub fn uv_at(&self, p: Vec3) -> Vec2 {
        let l = self.local(p);
        self.uv0 + self.du * l.x + self.dv * l.y
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Manual {
        frame: UvFrame,
        uvs: Vec<Vec2>,
        dist: Vec<f32>,
    },
    Auto {
        normal: Vec3,
        axis: Vec3,
        dist: Vec<f32>,
        lift: Vec<f32>,
        adj: Vec<usize>,
    },
}
pub type Frames = BTreeMap<usize, Entry>;
fn corners_from(face: &Face, positions: &[Vec3]) -> Vec<Vec3> {
    face.verts
        .iter()
        .map(|v| positions.get(*v as usize).copied().unwrap_or(Vec3::ZERO))
        .collect()
}
fn pair_distances(c: &[Vec3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(c.len() * c.len() / 2);
    for i in 0..c.len() {
        for j in i + 1..c.len() {
            out.push((c[i] - c[j]).magnitude());
        }
    }
    out
}
fn same_distances(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= 0.0005 * y.max(1.0))
}
fn lifts(c: &[Vec3], n: Vec3) -> Vec<f32> {
    c.iter().map(|p| p.innerprod(n)).collect()
}
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
    high - low > FLAT_EPS * scale
}
fn start_axis(normal: Vec3, axis: Vec3) -> Vec3 {
    if axis == Vec3::ZERO {
        normal
    } else {
        axis.unitized()
    }
}
pub fn capture_drag_frames(m: &BMesh, verts: &BTreeSet<u32>) -> Frames {
    let mut out = Frames::new();
    for (fi, face) in m.facets.iter().enumerate() {
        if !face.verts.iter().any(|v| verts.contains(v)) {
            continue;
        }
        let corners = corners_from(face, &m.positions);
        let normal = newell(&corners);
        if normal.innerprod(normal) <= 0.0 {
            continue;
        }
        let dist = pair_distances(&corners);
        match face.by_hand_tex_coords() {
            None => {
                let unit = normal.unitized();
                out.insert(
                    fi,
                    Entry::Auto {
                        normal: unit,
                        axis: face.uv_axis,
                        dist,
                        lift: lifts(&corners, unit),
                        adj: Vec::new(),
                    },
                );
            }
            Some(uvs) => {
                if let Some(frame) = UvFrame::adapt(&corners, uvs, normal) {
                    out.insert(
                        fi,
                        Entry::Manual {
                            frame,
                            uvs: uvs.to_vec(),
                            dist,
                        },
                    );
                }
            }
        }
    }
    link_patches(m, &mut out);
    out
}
pub fn link_patches(m: &BMesh, frames: &mut Frames) {
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, e) in frames.iter_mut() {
        let Entry::Auto { adj, .. } = e else { continue };
        adj.clear();
        for (a, b) in m.facets[*fi].loop_edges() {
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
                if axis_of(&frames[&sharing[i]]).innerprod(axis_of(&frames[&sharing[j]]))
                    >= 1.0 - SAME_AXIS_EPS
                {
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
pub fn refit_after_drag(m: &mut BMesh, frames: &Frames) {
    let mut sources: Vec<usize> = Vec::new();
    let mut rigid_tilted: BTreeMap<usize, Vec3> = BTreeMap::new();
    for (&fi, entry) in frames {
        if fi >= m.facets.len() {
            continue;
        }
        let corners = corners_from(&m.facets[fi], &m.positions);
        match entry {
            Entry::Auto {
                normal,
                axis,
                dist,
                lift,
                ..
            } => {
                let rigid = same_distances(&pair_distances(&corners), dist);
                let mut axis = *axis;
                if axis == Vec3::ZERO && tilted(lift, *normal, &corners) {
                    if rigid {
                        rigid_tilted.insert(fi, newell(&corners).unitized());
                    } else {
                        axis = *normal;
                    }
                }
                m.facets[fi].uv_axis = axis;
                if axis != Vec3::ZERO {
                    sources.push(fi);
                }
            }
            Entry::Manual { frame, uvs, dist } => {
                let face = &mut m.facets[fi];
                if face.by_hand_tex_coords().is_none() {
                    continue;
                }
                face.uv_coords = if same_distances(&pair_distances(&corners), dist) {
                    uvs.clone()
                } else {
                    corners.iter().map(|p| frame.uv_at(*p)).collect()
                };
            }
        }
    }
    while let Some(fi) = sources.pop() {
        if rigid_tilted.is_empty() {
            break;
        }
        let Some(Entry::Auto { adj, .. }) = frames.get(&fi) else {
            continue;
        };
        let axis = m.facets[fi].uv_axis;
        for other in adj {
            let Some(n) = rigid_tilted.get(other) else {
                continue;
            };
            if n.innerprod(axis.unitized()) < NEIGHBOUR_DOT {
                continue;
            }
            rigid_tilted.remove(other);
            m.facets[*other].uv_axis = axis;
            sources.push(*other);
        }
    }
}
pub fn drag_manual_coords(m: &BMesh, frames: &Frames, start: bool) -> BTreeMap<usize, Vec<Vec2>> {
    let mut out = BTreeMap::new();
    for (&fi, e) in frames {
        let Entry::Manual { uvs, .. } = e else {
            continue;
        };
        if start {
            out.insert(fi, uvs.clone());
        } else if let Some(c) = m.facets.get(fi).and_then(|f| f.by_hand_tex_coords()) {
            out.insert(fi, c.to_vec());
        }
    }
    out
}
pub fn drag_axis_changes(m: &BMesh, frames: &Frames, start: bool) -> BTreeMap<usize, Vec3> {
    let mut out = BTreeMap::new();
    for (&fi, e) in frames {
        let Entry::Auto { axis, .. } = e else {
            continue;
        };
        let Some(f) = m.facets.get(fi) else { continue };
        if f.uv_axis != *axis {
            out.insert(fi, if start { *axis } else { f.uv_axis });
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::super::uv_edit::{AutoUv, face_corner_uvs, freeze_manual_uvs};
    use super::*;
    fn shut(a: &[Vec2], b: &[Vec2]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (*x - *y).magnitude() < 1e-4)
    }
    fn facet_alongside_upright(m: &BMesh, n: Vec3) -> usize {
        (0..m.facets.len())
            .find(|f| m.face_normal(*f).innerprod(n) > 0.9)
            .unwrap()
    }
    #[test]
    fn frame_round_trips_the_uvs() {
        let c = [
            v3(0.0, 0.0, 0.0),
            v3(2.0, 0.0, 0.0),
            v3(2.0, 0.0, 1.0),
            v3(0.0, 0.0, 1.0),
        ];
        let uv = [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 0.5), v2(0.0, 0.5)];
        let f = UvFrame::adapt(&c, &uv, Vec3::UP).unwrap();
        for i in 0..4 {
            assert!((f.uv_at(c[i]) - uv[i]).magnitude() < 1e-5);
        }
        assert!(
            UvFrame::adapt(
                &c,
                &[v2(0.0, 0.0), v2(1.0, 0.0), v2(2.0, 0.0), v2(3.0, 0.0)],
                Vec3::UP
            )
            .is_none()
        );
        assert!(UvFrame::adapt(&c[..2], &uv[..2], Vec3::UP).is_none());
    }
    #[test]
    fn manual_uvs_ride_along_a_rigid_move_and_stay_put_when_stretched() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = facet_alongside_upright(&m, Vec3::UP);
        freeze_manual_uvs(&mut m, &BTreeSet::from([top]), &AutoUv::default());
        let start_uvs = m.facets[top].uv_coords.clone();
        let verts: BTreeSet<u32> = m.facets[top].verts.iter().copied().collect();
        let frames = capture_drag_frames(&m, &verts);
        assert!(matches!(frames[&top], Entry::Manual { .. }));
        let mut moved = m.clone();
        for v in &verts {
            moved.positions[*v as usize].y += 0.5;
        }
        refit_after_drag(&mut moved, &frames);
        assert!(shut(&moved.facets[top].uv_coords, &start_uvs));
        let v0 = m.facets[top].verts[0];
        let mut stretched = m.clone();
        stretched.positions[v0 as usize].x += 0.5;
        refit_after_drag(&mut stretched, &frames);
        let Entry::Manual { frame, .. } = &frames[&top] else {
            unreachable!()
        };
        let want = frame.uv_at(stretched.positions[v0 as usize]);
        assert!((stretched.facets[top].uv_coords[0] - want).magnitude() < 1e-5);
        assert!((stretched.facets[top].uv_coords[0] - start_uvs[0]).magnitude() > 0.1);
        assert_eq!(
            drag_manual_coords(&stretched, &frames, true)[&top],
            start_uvs
        );
        assert_eq!(
            drag_manual_coords(&stretched, &frames, false)[&top],
            stretched.facets[top].uv_coords
        );
    }
    #[test]
    fn deformed_auto_face_freezes_its_projection_axis() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = facet_alongside_upright(&m, Vec3::UP);
        let v0 = m.facets[top].verts[0];
        let frames = capture_drag_frames(&m, &BTreeSet::from([v0]));
        assert_eq!(frames.len(), 3);
        let before = face_corner_uvs(&m, top, &AutoUv::default());
        m.positions[v0 as usize].y += 0.6;
        refit_after_drag(&mut m, &frames);
        assert_eq!(m.facets[top].uv_axis, Vec3::UP);
        let after = face_corner_uvs(&m, top, &AutoUv::default());
        let moved_ci = 0;
        for ci in 0..before.len() {
            if ci != moved_ci {
                assert!((before[ci] - after[ci]).magnitude() < 1e-5);
            }
        }
        let axes = drag_axis_changes(&m, &frames, false);
        assert_eq!(axes.get(&top), Some(&Vec3::UP));
        assert_eq!(
            drag_axis_changes(&m, &frames, true).get(&top),
            Some(&Vec3::ZERO)
        );
    }
    #[test]
    fn rigid_move_keeps_auto_axes_free() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        let frames = capture_drag_frames(&m, &all);
        for p in &mut m.positions {
            *p = *p + v3(0.3, 0.2, 0.1);
        }
        refit_after_drag(&mut m, &frames);
        assert!(m.facets.iter().all(|f| f.uv_axis == Vec3::ZERO));
        let frames = capture_drag_frames(&m, &all);
        for p in &mut m.positions {
            let (s, c) = (0.3f32.sin(), 0.3f32.cos());
            *p = v3(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
        }
        refit_after_drag(&mut m, &frames);
        assert!(m.facets.iter().all(|f| f.uv_axis == Vec3::ZERO));
    }
    #[test]
    fn coplanar_neighbours_are_linked() {
        let mut m = BMesh::default();
        for p in [
            v3(0.0, 0.0, 0.0),
            v3(1.0, 0.0, 0.0),
            v3(2.0, 0.0, 0.0),
            v3(0.0, 0.0, 1.0),
            v3(1.0, 0.0, 1.0),
            v3(2.0, 0.0, 1.0),
        ] {
            m.add_vert(p);
        }
        m.add_face_facing(vec![0, 1, 4, 3], Vec3::UP);
        m.add_face_facing(vec![1, 2, 5, 4], Vec3::UP);
        let frames = capture_drag_frames(&m, &BTreeSet::from([1]));
        let Entry::Auto { adj, .. } = &frames[&0] else {
            unreachable!()
        };
        assert_eq!(adj, &vec![1]);
        let mut m2 = m.clone();
        m2.positions[2].y += 0.2;
        let frames = capture_drag_frames(&m, &BTreeSet::from([2]));
        refit_after_drag(&mut m2, &frames);
        assert_eq!(m2.facets[1].uv_axis, Vec3::UP);
        assert_eq!(m2.facets[0].uv_axis, Vec3::ZERO);
    }
}
