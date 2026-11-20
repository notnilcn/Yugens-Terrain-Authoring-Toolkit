//! Element and object operations: edge division, quad diagonal turning, un-sharing vertices,
//! chord connection, block merging, pivot transforms and the helpers they use (loop planes,
//! chord tests, colour capture, real-edge filtering).
//!
//! Modelled on cyclopsLevelBuilder's mesh editing (MIT, Mark McKay): the block transform and
//! merge commands (`commands/mesh/cmd_transform_vertices.gd`, `cmd_merge_blocks.gd`), the vertex
//! and edge handling of `math/general_mesh.gd`, and the planar loop tests of `math/loop_2d.gd`.

use super::loops::{Topo, interleave_ridge_points};
use super::model::{BMesh, Edge, edge, newell};
use super::tri::mesh_fill;
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet};

/// Affine transform (basis columns + origin), the `Transform3D` that maps one mesh's local
/// space into another's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
    pub origin: Vec3,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        x: v3(1.0, 0.0, 0.0),
        y: v3(0.0, 1.0, 0.0),
        z: v3(0.0, 0.0, 1.0),
        origin: Vec3::ZERO,
    };
    pub fn enact(&self, p: Vec3) -> Vec3 {
        self.x * p.x + self.y * p.y + self.z * p.z + self.origin
    }
    pub fn determinant(&self) -> f32 {
        self.x.innerprod(self.y.cross(self.z))
    }
    /// Inverse-transpose of the basis applied to a normal (`basis.inverse().transposed() * n`).
    fn upright(&self, n: Vec3) -> Vec3 {
        let d = self.determinant();
        (self.y.cross(self.z) * n.x + self.z.cross(self.x) * n.y + self.x.cross(self.y) * n.z) / d
    }
}

/// Bakes a transform into the positions; fixed UV projection axes follow it, and a mirroring
/// transform reverses every loop so faces stay outward.
pub fn bake_transform(m: &mut BMesh, xf: &Affine) {
    for p in &mut m.positions {
        *p = xf.enact(*p);
    }
    let det = xf.determinant();
    if det.abs() > 2e-12 {
        for f in &mut m.facets {
            if f.uv_axis != Vec3::ZERO {
                f.uv_axis = xf.upright(f.uv_axis).unitized();
            }
        }
    }
    if det < 0.0 {
        for f in &mut m.facets {
            f.flip_order();
        }
    }
}

/// Centre of the axis-aligned bounds (mesh-local), zero when empty.
pub fn bounds_midpoint(m: &BMesh) -> Vec3 {
    m.extent()
        .map(|(lo, hi)| (lo + hi) * 0.5)
        .unwrap_or(Vec3::ZERO)
}

/// Appends `source` (transformed by `source_to_target`) to `target`. `slot_map` maps each source
/// material slot to a target slot (index = source slot; slots past its end keep their index).
/// Returns the new faces.
pub fn merge_blocks(
    addressee: &mut BMesh,
    source: &BMesh,
    source_to_target: &Affine,
    slot_map: &[u16],
) -> BTreeSet<usize> {
    let mut src = source.clone();
    bake_transform(&mut src, source_to_target);
    let base = addressee.positions.len() as u32;
    addressee.positions.extend(src.positions.iter().copied());
    let first = addressee.facets.len();
    for mut f in src.facets {
        for v in &mut f.verts {
            *v += base;
        }
        f.stuff = slot_map.get(f.stuff as usize).copied().unwrap_or(f.stuff);
        addressee.facets.push(f);
    }
    (first..addressee.facets.len()).collect()
}

/// The keys that are real edges of the mesh.
pub fn existing_edges(t: &Topo, edges: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    edges
        .iter()
        .copied()
        .filter(|e| t.edge_faces.contains_key(e))
        .collect()
}

/// Splits every selected edge into `segments` (≥ 2) equal parts in every face that uses it.
/// Returns the new vertices.
pub fn divide_edges(m: &mut BMesh, edges: &BTreeSet<Edge>, segments: u32) -> BTreeSet<u32> {
    let segments = segments.max(2);
    let edges = existing_edges(&Topo::assemble(m), edges);
    let mut points: BTreeMap<Edge, Vec<u32>> = BTreeMap::new();
    let mut out = BTreeSet::new();
    for e in edges {
        let (pa, pb) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
        let list: Vec<u32> = (1..segments)
            .map(|s| m.add_vert(pa.lerp(pb, s as f32 / segments as f32)))
            .collect();
        out.extend(list.iter().copied());
        points.insert(e, list);
    }
    if !points.is_empty() {
        interleave_ridge_points(m, &points, &BTreeSet::new());
    }
    out
}

/// Rotates each selected face's loop by one corner, which turns a quad's triangulation diagonal
/// (manual UVs and corner colours rotate with it). Faces with fewer than four corners are left
/// alone. Returns how many faces changed.
pub fn rotate_face_loop(m: &mut BMesh, faces: &BTreeSet<usize>) -> usize {
    let mut n_changed = 0;
    for &fi in faces {
        let Some(f) = m.facets.get_mut(fi) else {
            continue;
        };
        let n = f.verts.len();
        if n < 4 {
            continue;
        }
        f.verts.rotate_left(1);
        if f.uv_manual && f.uv_coords.len() == n {
            f.uv_coords.rotate_left(1);
        }
        if f.color_corners.len() == n {
            f.color_corners.rotate_left(1);
        }
        n_changed += 1;
    }
    n_changed
}

/// Un-shares the selected vertices so each touching face gets its own copy (the first face keeps
/// the original). Returns the new vertices.
pub fn unshare_vertices(m: &mut BMesh, verts: &BTreeSet<u32>) -> BTreeSet<u32> {
    let vf = m.vert_faces();
    let mut out = BTreeSet::new();
    for &v in verts {
        let Some(touching) = vf.get(v as usize) else {
            continue;
        };
        for &fi in touching.iter().skip(1) {
            let copy = m.add_vert(m.positions[v as usize]);
            out.insert(copy);
            for x in &mut m.facets[fi].verts {
                if *x == v {
                    *x = copy;
                }
            }
        }
    }
    out
}

/// A loop projected onto its best-fit plane.
pub(crate) struct LoopPlane {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub pts: Vec<Vec2>,
    /// Winding sign of `pts` (+1 counter-clockwise in the u/v plane).
    pub align_heading: f32,
    /// Largest corner distance from the centroid: the length scale for tolerances.
    pub magnify: f32,
}

impl LoopPlane {
    pub fn to_3d(&self, p: Vec2) -> Vec3 {
        self.origin + self.u * p.x + self.v * p.y
    }
    pub fn dir_2d(&self, d: Vec3) -> Vec2 {
        v2(d.innerprod(self.u), d.innerprod(self.v))
    }
    /// `LoopPlane.is_concave`: some corner turns against the loop's winding.
    pub fn is_concave(&self) -> bool {
        let n = self.pts.len();
        let eps = 2e-9 * self.magnify * self.magnify;
        (0..n).any(|i| {
            let (a, b, c) = (
                self.pts[(i + n - 1) % n],
                self.pts[i],
                self.pts[(i + 1) % n],
            );
            self.align_heading * cross_2d(b - a, c - b) < -eps
        })
    }
}

pub(crate) fn sign_of_zone_planar(pts: &[Vec2]) -> f32 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].x * pts[(i + 1) % n].y - pts[i].y * pts[(i + 1) % n].x)
        .sum::<f32>()
        * 0.5
}

/// `None` for degenerate loops.
pub(crate) fn loop_plane(m: &BMesh, lp: &[u32]) -> Option<LoopPlane> {
    let n = lp.len();
    if n < 3 {
        return None;
    }
    let pts3: Vec<Vec3> = lp.iter().map(|v| m.positions[*v as usize]).collect();
    let c = pts3.iter().fold(Vec3::ZERO, |a, p| a + *p) / n as f32;
    let normal = newell(&pts3);
    if normal.innerprod(normal) <= 0.0 {
        return None;
    }
    let normal = normal.unitized();
    let axis = if normal.x.abs() < 0.9005 {
        v3(1.0, 0.0, 0.0)
    } else {
        Vec3::UP
    };
    let u = (axis - normal * axis.innerprod(normal)).unitized();
    let v = normal.cross(u);
    let mut scale = 0.0f32;
    let pts: Vec<Vec2> = pts3
        .iter()
        .map(|p| {
            scale = scale.max((*p - c).magnitude());
            let d = *p - c;
            v2(d.innerprod(u), d.innerprod(v))
        })
        .collect();
    let area = sign_of_zone_planar(&pts);
    if area.abs() <= 2e-12 * scale * scale {
        return None;
    }
    Some(LoopPlane {
        origin: c,
        u,
        v,
        pts,
        align_heading: area.signum(),
        magnify: scale,
    })
}

pub(crate) fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// True when segments ab and cd properly cross.
pub(crate) fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2, eps: f32) -> bool {
    let d1 = cross_2d(b - a, c - a);
    let d2 = cross_2d(b - a, d - a);
    let d3 = cross_2d(d - c, a - c);
    let d4 = cross_2d(d - c, b - c);
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
        && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

fn closest_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let l2 = ab.innerprod(ab);
    if l2 <= 0.0 {
        return a;
    }
    a + ab * ((p - a).innerprod(ab) / l2).clamp(0.0, 1.0)
}

/// Even-odd point in polygon (`Geometry2D.is_point_in_polygon`).
pub(crate) fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Corners `i` and `j` can be joined through the polygon's interior.
fn chord_inside(lp: &LoopPlane, i: usize, j: usize) -> bool {
    let pts = &lp.pts;
    let n = pts.len();
    let (a, b) = (pts[i], pts[j]);
    let len_eps = 0.0004 * lp.magnify;
    if a.span(b) <= len_eps {
        return false;
    }
    let cross_eps = 2e-9 * lp.magnify * lp.magnify;
    for k in 0..n {
        let k1 = (k + 1) % n;
        if k == i || k == j || k1 == i || k1 == j {
            continue;
        }
        if segments_cross(a, b, pts[k], pts[k1], cross_eps) {
            return false;
        }
    }
    for k in 0..n {
        if k == i || k == j {
            continue;
        }
        if closest_on_segment(pts[k], a, b).span(pts[k]) <= len_eps {
            return false;
        }
    }
    point_in_polygon((a + b) * 0.5, pts)
}

/// Position-based colour sampler over one painted face.
struct ColorField {
    pts: Vec<Vec3>,
    cols: Vec<[u8; 4]>,
    tris: Vec<[usize; 3]>,
    by_id: BTreeMap<u32, [u8; 4]>,
}

impl ColorField {
    /// `None` when the face carries no per-corner paint.
    fn record(m: &BMesh, fi: usize) -> Option<ColorField> {
        let f = &m.facets[fi];
        let cols = f.corner_colors()?.to_vec();
        let pts = m.face_points(fi);
        let mut by_id = BTreeMap::new();
        for (i, v) in f.verts.iter().enumerate() {
            by_id.entry(*v).or_insert(cols[i]);
        }
        let tris = mesh_fill(&pts, newell(&pts));
        if tris.is_empty() {
            return None;
        }
        Some(ColorField {
            pts,
            cols,
            tris,
            by_id,
        })
    }

    fn color_at(&self, p: Vec3) -> [u8; 4] {
        let mut best = -1.0e30f32;
        let mut best_col = [255u8; 4];
        for t in &self.tris {
            let a = self.pts[t[0]];
            let ab = self.pts[t[1]] - a;
            let ac = self.pts[t[2]] - a;
            let ap = p - a;
            let (d00, d01, d11, d20, d21) = (
                ab.innerprod(ab),
                ab.innerprod(ac),
                ac.innerprod(ac),
                ap.innerprod(ab),
                ap.innerprod(ac),
            );
            let denom = d00 * d11 - d01 * d01;
            if denom.abs() < 2e-12 {
                continue;
            }
            let v = (d11 * d20 - d01 * d21) / denom;
            let w = (d00 * d21 - d01 * d20) / denom;
            let u = 1.0 - v - w;
            let fit = u.min(v.min(w));
            if fit > best {
                best = fit;
                let (u, v, w) = (u.max(0.0), v.max(0.0), w.max(0.0));
                let s = u + v + w;
                if s < 2e-12 {
                    continue;
                }
                let (ca, cb, cc) = (self.cols[t[0]], self.cols[t[1]], self.cols[t[2]]);
                for k in 0..4 {
                    best_col[k] = ((ca[k] as f32 * u + cb[k] as f32 * v + cc[k] as f32 * w) / s)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
        best_col
    }

    fn colors_for_loop(&self, m: &BMesh, lp: &[u32]) -> Vec<[u8; 4]> {
        lp.iter()
            .map(|v| {
                self.by_id
                    .get(v)
                    .copied()
                    .unwrap_or_else(|| self.color_at(m.positions[*v as usize]))
            })
            .collect()
    }
}

/// Adds a vertex at the midpoint of each selected edge and splits every face that has exactly
/// two of them with a chord between the two (when the chord runs inside the face). Neighbouring
/// faces get the midpoints spliced in. Returns the new chords.
pub fn join_ridges(m: &mut BMesh, edges: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    let edges = existing_edges(&Topo::assemble(m), edges);
    let mut mids: BTreeMap<Edge, Vec<u32>> = BTreeMap::new();
    for e in edges {
        let p = (m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5;
        mids.insert(e, vec![m.add_vert(p)]);
    }
    let mut chords = BTreeSet::new();
    if mids.is_empty() {
        return chords;
    }
    let mut split = BTreeSet::new();
    let original = m.facets.len();
    for fi in 0..original {
        let verts = m.facets[fi].verts.clone();
        let n = verts.len();
        let mut expanded = Vec::with_capacity(n + 2);
        let mut mid_pos = Vec::new();
        for i in 0..n {
            expanded.push(verts[i]);
            if let Some(mv) = mids.get(&edge(verts[i], verts[(i + 1) % n])) {
                mid_pos.push(expanded.len());
                expanded.push(mv[0]);
            }
        }
        if mid_pos.len() != 2 {
            continue;
        }
        let (a, b) = (mid_pos[0], mid_pos[1]);
        let Some(lp) = loop_plane(m, &expanded) else {
            continue;
        };
        if !chord_inside(&lp, a, b) {
            continue;
        }
        let chord = edge(expanded[a], expanded[b]);
        if !chords.insert(chord) {
            continue;
        }
        split.insert(fi);
        let k = expanded.len();
        let first: Vec<u32> = expanded[a..=b].to_vec();
        let mut second = Vec::new();
        let mut i = b;
        loop {
            second.push(expanded[i % k]);
            if i % k == a {
                break;
            }
            i += 1;
        }
        let field = ColorField::record(m, fi);
        let mut extra = m.facets[fi].with_verts(second.clone());
        let mut kept = m.facets[fi].with_verts(first.clone());
        if let Some(field) = field {
            extra.color_corners = field.colors_for_loop(m, &second);
            kept.color_corners = field.colors_for_loop(m, &first);
        }
        m.facets[fi] = kept;
        m.facets.push(extra);
    }
    interleave_ridge_points(
        m,
        &mids,
        &split
            .iter()
            .copied()
            .chain(original..m.facets.len())
            .collect(),
    );
    chords
}

#[cfg(test)]
mod tests {
    use super::super::ops;
    use super::super::shapes::cuboid;
    use super::*;

    fn top_face(m: &BMesh) -> usize {
        (0..m.facets.len())
            .find(|f| m.face_normal(*f).y > 0.9)
            .unwrap()
    }

    #[test]
    fn divide_edges_keeps_mesh_closed_and_lerps_colours() {
        let mut m = cuboid(v3(2.0, 2.0, 2.0));
        let t = top_face(&m);
        let n = m.facets[t].verts.len();
        m.facets[t].color_corners = (0..n)
            .map(|i| {
                if i == 0 {
                    [0, 0, 0, 0]
                } else {
                    [255, 255, 255, 255]
                }
            })
            .collect();
        let (a, b) = (m.facets[t].verts[0], m.facets[t].verts[1]);
        let new = divide_edges(&mut m, &BTreeSet::from([edge(a, b), (9998, 9999)]), 4);
        assert_eq!(new.len(), 3);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.facets[t].verts.len(), 7);
        let c = &m.facets[t].color_corners;
        assert_eq!(c.len(), 7);
        // Corner 0 is black, corner 4 (old corner 1) white; the inserted ones ramp between.
        assert!(c[1][0] > 0 && c[1][0] < c[2][0] && c[2][0] < c[3][0] && c[3][0] < 255);
        assert!((m.volume() - 8.0).abs() < 0.0015);
    }

    #[test]
    fn rotate_face_loop_rotates_quads_only() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top_face(&m);
        let before = m.facets[t].verts.clone();
        m.facets[t].uv_manual = true;
        m.facets[t].uv_coords = vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)];
        assert_eq!(rotate_face_loop(&mut m, &BTreeSet::from([t])), 1);
        assert_eq!(
            m.facets[t].verts,
            vec![before[1], before[2], before[3], before[0]]
        );
        assert_eq!(m.facets[t].uv_coords[0], v2(1.0, 0.0));
        assert!(m.is_closed());
        // A triangle is untouched.
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        let tris = ops::mesh_fill_facets(&mut m, &all);
        assert_eq!(rotate_face_loop(&mut m, &tris), 0);
    }

    #[test]
    fn unshare_vertices_gives_each_face_a_copy() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let new = unshare_vertices(&mut m, &BTreeSet::from([0]));
        assert_eq!(new.len(), 2);
        assert_eq!(m.positions.len(), 10);
        let users = m.facets.iter().filter(|f| f.verts.contains(&0)).count();
        assert_eq!(users, 1);
        assert!(!m.is_closed());
        m.sanity_check().unwrap();
    }

    #[test]
    fn connect_edges_splits_the_face_between_two_midpoints() {
        let mut m = cuboid(v3(2.0, 2.0, 2.0));
        let t = top_face(&m);
        let v = m.facets[t].verts.clone();
        m.facets[t].color_corners = vec![[255, 0, 0, 255]; 4];
        let chords = join_ridges(
            &mut m,
            &BTreeSet::from([edge(v[0], v[1]), edge(v[2], v[3])]),
        );
        assert_eq!(chords.len(), 1);
        assert_eq!(m.facets.len(), 7);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 8.0).abs() < 0.0015);
        let tops: Vec<usize> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y > 0.9)
            .collect();
        assert_eq!(tops.len(), 2);
        for f in tops {
            assert!((m.face_area(f) - 2.0).abs() < 0.0015);
            assert_eq!(m.facets[f].color_corners, vec![[255, 0, 0, 255]; 4]);
        }
    }

    #[test]
    fn merge_blocks_bakes_transform_and_maps_slots() {
        let mut a = cuboid(v3(1.0, 1.0, 1.0));
        let mut b = cuboid(v3(1.0, 1.0, 1.0));
        for f in &mut b.facets {
            f.stuff = 1;
        }
        // Mirror + move: winding must be reversed so the copy stays outward.
        let xf = Affine {
            x: v3(-1.0, 0.0, 0.0),
            origin: v3(5.0, 0.0, 0.0),
            ..Affine::IDENTITY
        };
        let new = merge_blocks(&mut a, &b, &xf, &[0, 3]);
        assert_eq!(new.len(), 6);
        assert_eq!(a.facets.len(), 12);
        a.sanity_check().unwrap();
        assert!(a.is_closed());
        assert!((a.volume() - 2.0).abs() < 0.0015);
        assert!(new.iter().all(|f| a.facets[*f].stuff == 3));
        let (lo, hi) = a.extent().unwrap();
        assert!((hi.x - 5.5).abs() < 0.0002 || (hi.x - 5.0).abs() < 0.0002);
        assert!(lo.x <= 0.0);
    }

    #[test]
    fn bounds_midpoint_and_triangulate_carry_corners() {
        let mut m = cuboid(v3(2.0, 2.0, 2.0));
        let c = bounds_midpoint(&m);
        m.displace(-c);
        assert!(bounds_midpoint(&m).magnitude() < 0.0002);
        let t = top_face(&m);
        m.facets[t].color_corners = vec![
            [10, 20, 30, 40],
            [50, 60, 70, 80],
            [90, 100, 110, 120],
            [1, 2, 3, 4],
        ];
        let corner_of: BTreeMap<u32, [u8; 4]> = m.facets[t]
            .verts
            .iter()
            .copied()
            .zip(m.facets[t].color_corners.iter().copied())
            .collect();
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        let tris = ops::mesh_fill_facets(&mut m, &all);
        assert_eq!(tris.len(), 12);
        assert!(m.is_closed());
        let painted: Vec<&super::super::model::Face> = m
            .facets
            .iter()
            .filter(|f| !f.color_corners.is_empty())
            .collect();
        assert_eq!(painted.len(), 2);
        for f in painted {
            for (i, v) in f.verts.iter().enumerate() {
                assert_eq!(f.color_corners[i], corner_of[v]);
            }
        }
    }
}
