use super::model::BMesh;
use super::poly2tri::Poly2Tri;
use super::shapes::{extrude_profile, ring};
use super::topo::{chain_loops, region_boundary};
use super::trim_walls::{collapse_points, polygon_solid};
use crate::math::*;
use std::collections::{BTreeSet, HashMap};
pub fn wedge(s: Vec3) -> BMesh {
    let x = s.x * 0.5;
    let tri = [v2(-x, 0.0), v2(x, 0.0), v2(0.0, s.y)];
    extrude_profile(
        &tri,
        v3(1.0, 0.0, 0.0),
        Vec3::UP,
        v3(0.0, 0.0, 1.0),
        v3(0.0, 0.0, -s.z * 0.5),
        s.z,
        0,
    )
}
const PHI: f32 = 1.618_034;
const ICOSAHEDRON: [[f32; 3]; 12] = [
    [-1.0, PHI, 0.0],
    [1.0, PHI, 0.0],
    [-1.0, -PHI, 0.0],
    [1.0, -PHI, 0.0],
    [0.0, -1.0, PHI],
    [0.0, 1.0, PHI],
    [0.0, -1.0, -PHI],
    [0.0, 1.0, -PHI],
    [PHI, 0.0, -1.0],
    [PHI, 0.0, 1.0],
    [-PHI, 0.0, -1.0],
    [-PHI, 0.0, 1.0],
];
const ICOSAHEDRON_FACES: [[u32; 3]; 20] = [
    [0, 11, 5],
    [0, 5, 1],
    [0, 1, 7],
    [0, 7, 10],
    [0, 10, 11],
    [1, 5, 9],
    [5, 11, 4],
    [11, 10, 2],
    [10, 7, 6],
    [7, 1, 8],
    [3, 9, 4],
    [3, 4, 2],
    [3, 2, 6],
    [3, 6, 8],
    [3, 8, 9],
    [4, 9, 5],
    [2, 4, 11],
    [6, 2, 10],
    [8, 6, 7],
    [9, 8, 1],
];
pub fn icosphere(s: Vec3, subdivisions: u32, smooth: bool) -> BMesh {
    let mut points: Vec<Vec3> = ICOSAHEDRON
        .iter()
        .map(|p| v3(p[0], p[1], p[2]).unitized())
        .collect();
    let mut faces: Vec<[u32; 3]> = ICOSAHEDRON_FACES.to_vec();
    for _ in 0..subdivisions.clamp(1, 5) {
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, points: &mut Vec<Vec3>| -> u32 {
            *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                points.push((points[a as usize] + points[b as usize]).unitized());
                points.len() as u32 - 1
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for &[a, b, c] in &faces {
            let ab = midpoint(a, b, &mut points);
            let bc = midpoint(b, c, &mut points);
            let ca = midpoint(c, a, &mut points);
            next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }
    let mut m = BMesh::default();
    let centre = v3(0.0, s.y * 0.5, 0.0);
    for p in &points {
        m.add_vert(centre + v3(p.x * s.x * 0.5, p.y * s.y * 0.5, p.z * s.z * 0.5));
    }
    for [a, b, c] in faces {
        let ctr = (points[a as usize] + points[b as usize] + points[c as usize]) / 3.0;
        let f = m.add_face_facing(
            vec![a, b, c],
            v3(ctr.x * s.y * s.z, ctr.y * s.x * s.z, ctr.z * s.x * s.y),
        );
        m.facets[f].glide = smooth as u16;
    }
    m
}
pub fn tube_slashes(s: Vec3, segments: u32, cuts: u32, smooth: bool) -> BMesh {
    let n = segments.max(3);
    let rows = cuts + 1;
    let pts = ring(n, s.x * 0.5, s.z * 0.5);
    let mut m = BMesh::default();
    for k in 0..=rows {
        let y = s.y * k as f32 / rows as f32;
        for p in &pts {
            m.add_vert(v3(p.x, y, p.y));
        }
    }
    let at = |k: u32, i: u32| k * n + i % n;
    m.add_face_facing((0..n).map(|i| at(0, i)).collect(), -Vec3::UP);
    m.add_face_facing((0..n).map(|i| at(rows, i)).collect(), Vec3::UP);
    for k in 0..rows {
        for i in 0..n {
            let mid = (pts[i as usize] + pts[((i + 1) % n) as usize]) * 0.5;
            let f = m.add_face_facing(
                vec![at(k, i), at(k, i + 1), at(k + 1, i + 1), at(k + 1, i)],
                v3(mid.x, 0.0, mid.y),
            );
            m.facets[f].glide = smooth as u16;
        }
    }
    m
}
pub fn conduit_slashes(s: Vec3, thickness: f32, segments: u32, cuts: u32, smooth: bool) -> BMesh {
    let n = segments.max(3);
    let rows = cuts + 1;
    let (rx, rz) = (s.x * 0.5, s.z * 0.5);
    let t = thickness.clamp(0.0105, rx.min(rz) * 0.9505);
    let outer = ring(n, rx, rz);
    let inner = ring(n, rx - t, rz - t);
    let mut m = BMesh::default();
    for pts in [&outer, &inner] {
        for k in 0..=rows {
            let y = s.y * k as f32 / rows as f32;
            for p in pts.iter() {
                m.add_vert(v3(p.x, y, p.y));
            }
        }
    }
    let at = |inner: bool, k: u32, i: u32| (inner as u32) * (rows + 1) * n + k * n + i % n;
    for i in 0..n {
        let j = i + 1;
        let mid = (outer[i as usize] + outer[(j % n) as usize]) * 0.5;
        let dir = v3(mid.x, 0.0, mid.y);
        for k in 0..rows {
            let a = m.add_face_facing(
                vec![
                    at(false, k, i),
                    at(false, k, j),
                    at(false, k + 1, j),
                    at(false, k + 1, i),
                ],
                dir,
            );
            m.facets[a].glide = smooth as u16;
            let b = m.add_face_facing(
                vec![
                    at(true, k, i),
                    at(true, k, j),
                    at(true, k + 1, j),
                    at(true, k + 1, i),
                ],
                -dir,
            );
            m.facets[b].glide = 2 * smooth as u16;
        }
        m.add_face_facing(
            vec![
                at(false, rows, i),
                at(false, rows, j),
                at(true, rows, j),
                at(true, rows, i),
            ],
            Vec3::UP,
        );
        m.add_face_facing(
            vec![
                at(false, 0, i),
                at(false, 0, j),
                at(true, 0, j),
                at(true, 0, i),
            ],
            -Vec3::UP,
        );
    }
    m
}
pub fn torus_partial(
    ring_radius: f32,
    tube_radius: f32,
    ring_segments: u32,
    tube_segments: u32,
    ring_deg: f32,
    tube_deg: f32,
) -> BMesh {
    let (hd, vd) = (ring_deg.clamp(0.0, 360.0), tube_deg.clamp(0.0, 360.0));
    if hd >= 360.0 && vd >= 360.0 {
        return super::prims::donut(ring_radius, tube_radius, ring_segments, tube_segments);
    }
    if hd <= 0.0 || vd <= 0.0 {
        return BMesh::default();
    }
    let (rs, ts) = (ring_segments.max(3), tube_segments.max(3));
    let tube = tube_radius.clamp(0.0015, ring_radius * 0.9505);
    let (full_ring, full_tube) = (hd >= 360.0, vd >= 360.0);
    let (cols, rows) = (rs + !full_ring as u32, ts + !full_tube as u32);
    let ua = |i: u32| (hd * i as f32 / rs as f32).to_radians();
    let va = |j: u32| (vd * j as f32 / ts as f32).to_radians();
    let mut m = BMesh::default();
    for i in 0..cols {
        let radial = v3(ua(i).cos(), 0.0, ua(i).sin());
        for j in 0..rows {
            let b = va(j);
            m.add_vert(radial * (ring_radius + b.cos() * tube) + Vec3::UP * (b.sin() * tube));
        }
    }
    let id = |i: u32, j: u32| (i % cols) * rows + (j % rows);
    for i in 0..rs {
        for j in 0..ts {
            let ids = vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            let centroid = ids
                .iter()
                .fold(Vec3::ZERO, |acc, v| acc + m.positions[*v as usize])
                / 4.0;
            let mid = (ua(i) + ua(i + 1)) * 0.5;
            let tube_centre = v3(mid.cos(), 0.0, mid.sin()) * ring_radius;
            let f = m.add_face_facing(ids, centroid - tube_centre);
            m.facets[f].glide = 1;
        }
    }
    m
}
pub fn arch_band(
    s: Vec3,
    thickness: f32,
    sides: u32,
    degrees: f32,
    end_caps: bool,
    smooth: bool,
) -> BMesh {
    let (rx, ry) = (s.x * 0.5, s.y);
    let t = thickness.clamp(0.0105, rx.min(ry) * 0.9505);
    let deg = degrees.clamp(1.0, 360.0);
    let n = sides.clamp(2, 200).max((deg / 179.0).ceil() as u32);
    let full = deg >= 360.0;
    let stations = if full { n } else { n + 1 };
    let (front, back) = (s.z * 0.5, -s.z * 0.5);
    let angle = |k: u32| (deg * k as f32 / n as f32).to_radians();
    let mut m = BMesh::default();
    for k in 0..stations {
        let a = angle(k);
        for (wx, wy) in [(rx, ry), (rx - t, ry - t)] {
            for z in [front, back] {
                m.add_vert(v3(a.cos() * wx, a.sin() * wy, z));
            }
        }
    }
    let at = |k: u32, inner: bool, back: bool| (k % stations) * 4 + 2 * inner as u32 + back as u32;
    for k in 0..n {
        let mid = (angle(k) + angle(k + 1)) * 0.5;
        let radial = v3(mid.cos() * ry, mid.sin() * rx, 0.0);
        let k1 = k + 1;
        let outer = m.add_face_facing(
            vec![
                at(k, false, false),
                at(k, false, true),
                at(k1, false, true),
                at(k1, false, false),
            ],
            radial,
        );
        m.facets[outer].glide = smooth as u16;
        let inner = m.add_face_facing(
            vec![
                at(k, true, false),
                at(k, true, true),
                at(k1, true, true),
                at(k1, true, false),
            ],
            -radial,
        );
        m.facets[inner].glide = 2 * smooth as u16;
        m.add_face_facing(
            vec![
                at(k, false, false),
                at(k1, false, false),
                at(k1, true, false),
                at(k, true, false),
            ],
            v3(0.0, 0.0, 1.0),
        );
        m.add_face_facing(
            vec![
                at(k, false, true),
                at(k1, false, true),
                at(k1, true, true),
                at(k, true, true),
            ],
            v3(0.0, 0.0, -1.0),
        );
    }
    if end_caps && !full {
        for (k, sign) in [(0u32, -1.0f32), (n, 1.0)] {
            let a = angle(k);
            let tangent = v3(-a.sin() * rx, a.cos() * ry, 0.0) * sign;
            m.add_face_facing(
                vec![
                    at(k, false, false),
                    at(k, true, false),
                    at(k, true, true),
                    at(k, false, true),
                ],
                tangent,
            );
        }
    }
    if let Some((lo, _)) = m.extent() {
        m.displace(v3(0.0, -lo.y, 0.0));
    }
    m
}
pub fn stair_count(height: f32, step_height: f32) -> u32 {
    let sh = step_height.max(0.0105);
    ((height / sh - 1e-4).ceil()).clamp(1.0, 256.0) as u32
}
pub fn polygon_form(outline: &[Vec2], holes: &[Vec<Vec2>], height: f32) -> Result<BMesh, String> {
    if !height.is_finite() {
        return Err("The extrusion height must be finite.".into());
    }
    let outline = collapse_points(outline, true, 0.0005);
    let holes: Vec<Vec<Vec2>> = holes
        .iter()
        .map(|h| collapse_points(h, true, 0.0005))
        .filter(|h| h.len() >= 3)
        .collect();
    if outline.len() < 3 {
        return Err("A polygon needs at least three points.".into());
    }
    if holes.is_empty() && height != 0.0 {
        return Ok(polygon_solid(&outline, height));
    }
    let to2 = |p: &Vec2| [p.x as f64, p.y as f64];
    let contour: Vec<[f64; 2]> = outline.iter().map(to2).collect();
    let holes2: Vec<Vec<[f64; 2]>> = holes.iter().map(|h| h.iter().map(to2).collect()).collect();
    let tris = Poly2Tri::mesh_fill(&contour, &holes2).map_err(|e| e.fault_text_blurb())?;
    let all: Vec<Vec2> = outline
        .iter()
        .chain(holes.iter().flatten())
        .copied()
        .collect();
    let k = all.len() as u32;
    let up = if height >= 0.0 { Vec3::UP } else { -Vec3::UP };
    let mut cap = BMesh::default();
    for p in &all {
        cap.add_vert(v3(p.x, height, p.y));
    }
    for t in tris.chunks(3) {
        cap.add_face_facing(vec![t[0] as u32, t[1] as u32, t[2] as u32], up);
    }
    let loops = merge_cap(&cap);
    let mut m = BMesh::default();
    for p in &all {
        m.add_vert(v3(p.x, 0.0, p.y));
    }
    if height == 0.0 {
        for lp in &loops {
            m.add_face(lp.clone());
        }
        return Ok(m);
    }
    for p in &all {
        m.add_vert(v3(p.x, height, p.y));
    }
    for lp in &loops {
        m.add_face(lp.iter().map(|v| v + k).collect());
        m.add_face(lp.iter().rev().copied().collect());
    }
    let all_faces: BTreeSet<usize> = (0..cap.facets.len()).collect();
    for (a, b) in region_boundary(&cap, &all_faces) {
        m.add_face(vec![a, b, b + k, a + k]);
    }
    Ok(m)
}
fn merge_cap(cap: &BMesh) -> Vec<Vec<u32>> {
    let edge_faces = cap.edge_faces();
    let mut owner = vec![usize::MAX; cap.facets.len()];
    let mut loops = Vec::new();
    for seed in 0..cap.facets.len() {
        if owner[seed] != usize::MAX {
            continue;
        }
        let id = loops.len();
        let mut region = BTreeSet::from([seed]);
        owner[seed] = id;
        let mut grew = true;
        while grew {
            grew = false;
            let candidates: BTreeSet<usize> = region
                .iter()
                .flat_map(|f| cap.facets[*f].loop_edges().collect::<Vec<_>>())
                .flat_map(|(a, b)| {
                    edge_faces
                        .get(&super::model::edge(a, b))
                        .cloned()
                        .unwrap_or_default()
                })
                .filter(|f| owner[*f] == usize::MAX)
                .collect();
            for c in candidates {
                region.insert(c);
                let ok = chain_loops(&region_boundary(cap, &region)).is_some_and(|l| l.len() == 1);
                if ok {
                    owner[c] = id;
                    grew = true;
                } else {
                    region.remove(&c);
                }
            }
        }
        let lp = chain_loops(&region_boundary(cap, &region))
            .and_then(|l| l.into_iter().next())
            .unwrap_or_default();
        loops.push(lp);
    }
    loops
}
pub fn ring_zone(pts: &[Vec2]) -> f32 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y)
        .sum::<f32>()
        * 0.5
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::shapes::{ShapeKind, ShapeSpec, produce};
    fn is_shut(m: &BMesh, name: &str) {
        m.sanity_check().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(m.is_closed(), "{name} not closed");
        assert!(m.volume() > 0.0, "{name} inside out: {}", m.volume());
    }
    fn square(h: f32) -> Vec<Vec2> {
        vec![v2(-h, -h), v2(h, -h), v2(h, h), v2(-h, h)]
    }
    #[test]
    fn prism_is_half_a_box() {
        let m = wedge(v3(2.0, 1.0, 3.0));
        is_shut(&m, "prism");
        assert_eq!(m.facets.len(), 5);
        assert!((m.volume() - 3.0).abs() < 1e-4);
        let (lo, hi) = m.extent().unwrap();
        assert!(lo.y.abs() < 1e-6 && (hi.y - 1.0).abs() < 1e-6);
    }
    #[test]
    fn icosphere_subdivides_and_closes() {
        for (sub, faces) in [(1u32, 80usize), (2, 320), (5, 20 * 4usize.pow(5))] {
            let m = icosphere(v3(2.0, 2.0, 2.0), sub, true);
            is_shut(&m, "icosphere");
            assert_eq!(m.facets.len(), faces);
            assert_eq!(m.positions.len(), faces / 2 + 2, "shared vertices");
            let (lo, hi) = m.extent().unwrap();
            assert!(lo.y.abs() < 1e-5 && (hi.y - 2.0).abs() < 1e-5);
        }
        let m = icosphere(v3(2.0, 2.0, 2.0), 3, true);
        let v = m.volume();
        assert!((v - 4.0 / 3.0 * std::f32::consts::PI).abs() < 0.1, "{v}");
        is_shut(
            &icosphere(v3(1.0, 3.0, 0.5), 2, false),
            "squashed icosphere",
        );
    }
    #[test]
    fn cuts_split_the_side_walls() {
        let s = v3(2.0, 3.0, 2.0);
        let c = tube_slashes(s, 8, 2, true);
        is_shut(&c, "cylinder cuts");
        assert_eq!(c.facets.len(), 2 + 8 * 3);
        let plain = crate::build::shapes::tube(s, 8, true);
        assert!((c.volume() - plain.volume()).abs() < 1e-3);
        let p = conduit_slashes(s, 0.3, 8, 3, true);
        is_shut(&p, "pipe cuts");
        assert_eq!(p.facets.len(), 8 * (2 * 4 + 2));
        let plain = crate::build::shapes::conduit(s, 0.3, 8, true);
        assert!((p.volume() - plain.volume()).abs() < 1e-3);
    }
    #[test]
    fn partial_torus_is_open_and_full_is_unchanged() {
        let full = torus_partial(1.05, 0.26, 12, 8, 360.0, 360.0);
        assert_eq!(full, crate::build::prims::donut(1.05, 0.26, 12, 8));
        let half = torus_partial(1.0, 0.25, 12, 8, 180.0, 360.0);
        half.sanity_check().unwrap();
        assert!(!half.is_closed());
        assert_eq!(half.facets.len(), 12 * 8);
        assert_eq!(half.positions.len(), 13 * 8);
        for f in 0..half.facets.len() {
            let c = half.face_center(f);
            let tc = v3(c.x, 0.0, c.z).unitized() * 1.0;
            assert!(
                half.face_normal(f).innerprod(c - tc) > 0.0,
                "face {f} inward"
            );
        }
        let slice = torus_partial(1.0, 0.25, 12, 8, 90.0, 120.0);
        slice.sanity_check().unwrap();
        assert!(
            torus_partial(1.0, 0.25, 12, 8, 0.0, 360.0)
                .facets
                .is_empty()
        );
    }
    #[test]
    fn arch_band_caps_and_full_ring() {
        let s = v3(4.0, 2.0, 1.0);
        let a = arch_band(s, 0.5, 8, 180.0, true, true);
        is_shut(&a, "arch band");
        assert_eq!(a.facets.len(), 8 * 4 + 2);
        let (lo, hi) = a.extent().unwrap();
        assert!(
            lo.y.abs() < 1e-5 && (hi.y - 2.0).abs() < 1e-4,
            "{lo:?} {hi:?}"
        );
        let open = arch_band(s, 0.5, 8, 180.0, false, true);
        open.sanity_check().unwrap();
        assert!(!open.is_closed());
        is_shut(&arch_band(s, 0.5, 12, 360.0, true, false), "arch ring");
        is_shut(
            &arch_band(s, 0.5, 2, 300.0, true, false),
            "arch with few sides",
        );
    }
    #[test]
    fn stair_count_has_no_sliver_step() {
        assert_eq!(stair_count(3.0, 0.2), 15);
        assert_eq!(stair_count(3.0, 0.25), 12);
        assert_eq!(stair_count(1.0, 0.3), 4);
        assert_eq!(stair_count(100.0, 0.01), 256);
        assert_eq!(stair_count(0.1, 5.0), 1);
    }
    #[test]
    fn stairs_by_height_spec() {
        let spec = ShapeSpec {
            sort: ShapeKind::Stairs,
            size: v3(1.0, 1.0, 2.0),
            step_height: 0.3,
            ..Default::default()
        };
        let eq = produce(&spec, None);
        is_shut(&eq, "equal steps");
        assert_eq!(eq, crate::build::shapes::steps(v3(1.0, 1.0, 2.0), 4));
        let uneq = produce(
            &ShapeSpec {
                equal_steps: false,
                ..spec
            },
            None,
        );
        is_shut(&uneq, "fixed steps");
        let v = 0.5 * (0.3 + 0.6 + 0.9 + 1.0);
        assert!((uneq.volume() - v).abs() < 1e-4, "{}", uneq.volume());
        let curved = produce(
            &ShapeSpec {
                sort: ShapeKind::CurvedStairs,
                step_height: 0.25,
                ..spec
            },
            None,
        );
        is_shut(&curved, "curved by height");
        let many = produce(
            &ShapeSpec {
                size: v3(1.0, 3.0, 2.0),
                step_height: 0.01,
                equal_steps: false,
                ..spec
            },
            None,
        );
        assert_eq!(
            many,
            crate::build::shapes::steps(v3(1.0, 3.0, 2.0), 256),
            "no giant top riser"
        );
    }
    #[test]
    fn spec_defaults_keep_old_output() {
        let s = v3(2.0, 3.0, 2.5);
        let g = |k| {
            produce(
                &ShapeSpec {
                    sort: k,
                    size: s,
                    ..Default::default()
                },
                None,
            )
        };
        assert_eq!(
            g(ShapeKind::Plane),
            crate::build::shapes::sheet(v2(2.0, 2.5), 13)
        );
        assert_eq!(
            g(ShapeKind::Cylinder),
            crate::build::shapes::tube(s, 13, true)
        );
        assert_eq!(g(ShapeKind::Arch), crate::build::shapes::bow(s, 0.2605, 13));
        assert_eq!(g(ShapeKind::Stairs), crate::build::shapes::steps(s, 7));
        let p = produce(
            &ShapeSpec {
                sort: ShapeKind::Plane,
                stretches: 3,
                segments_z: 2,
                ..Default::default()
            },
            None,
        );
        assert_eq!(p.facets.len(), 6);
        let sp = g(ShapeKind::Sprite);
        assert_eq!((sp.facets.len(), sp.positions.len()), (1, 4));
        let ab = produce(
            &ShapeSpec {
                sort: ShapeKind::Arch,
                arch_band: true,
                arch_degrees: 120.0,
                size: s,
                ..Default::default()
            },
            None,
        );
        is_shut(&ab, "arch band spec");
        let t = produce(
            &ShapeSpec {
                sort: ShapeKind::Torus,
                torus_arc: 270.0,
                size: s,
                ..Default::default()
            },
            None,
        );
        t.sanity_check().unwrap();
    }
    #[test]
    fn poly_without_holes_is_the_prism() {
        let sq = square(1.0);
        assert_eq!(
            polygon_form(&sq, &[], 2.0).unwrap(),
            polygon_solid(&sq, 2.0)
        );
    }
    #[test]
    fn poly_with_a_hole() {
        let outer = square(2.0);
        let hole = square(0.5);
        for (o, h) in [
            (outer.clone(), hole.clone()),
            (
                outer.iter().rev().copied().collect(),
                hole.iter().rev().copied().collect::<Vec<_>>(),
            ),
        ] {
            let m = polygon_form(&o, &[h], 1.5).unwrap();
            is_shut(&m, "poly with hole");
            assert!(
                (m.volume() - (16.0 - 1.0) * 1.5).abs() < 1e-3,
                "{}",
                m.volume()
            );
            let up: Vec<usize> = (0..m.facets.len())
                .filter(|f| m.face_normal(*f).y > 0.9)
                .collect();
            assert_eq!(up.len(), 2, "cap faces");
            let area: f32 = up.iter().map(|f| m.face_area(*f)).sum();
            assert!((area - 15.0).abs() < 1e-3);
            assert_eq!(m.facets.len() - 2 * up.len(), 8, "one wall per edge");
        }
    }
    #[test]
    fn poly_with_two_holes_and_flat_and_negative() {
        let outer = vec![
            v2(-3.0, -1.0),
            v2(3.0, -1.0),
            v2(3.0, 1.0),
            v2(0.0, 2.0),
            v2(-3.0, 1.0),
        ];
        let h1: Vec<Vec2> = square(0.4).iter().map(|p| *p + v2(-1.5, 0.0)).collect();
        let h2: Vec<Vec2> = square(0.4).iter().map(|p| *p + v2(1.5, 0.0)).collect();
        let area = ring_zone(&outer).abs() - 2.0 * 0.64;
        let m = polygon_form(&outer, &[h1.clone(), h2.clone()], 1.0).unwrap();
        is_shut(&m, "two holes");
        let caps = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y > 0.9)
            .count();
        assert!((2..=3).contains(&caps), "two-hole cap faces: {caps}");
        assert!((m.volume() - area).abs() < 1e-3);
        let flat = polygon_form(&outer, &[h1.clone()], 0.0).unwrap();
        flat.sanity_check().unwrap();
        assert!((0..flat.facets.len()).all(|f| flat.face_normal(f).y > 0.9));
        let down = polygon_form(&outer, &[h1, h2], -1.0).unwrap();
        is_shut(&down, "negative height");
        assert!(polygon_form(&square(1.0), &[square(2.0)], 1.0).is_err());
    }
}
