//! Extra shape generators ported from Unibuilder's `core/primitives.gd` (UBPrimitives, MIT,
//! Leafshade Consulting Ltd): torus, door frame (flat or arched), curved stairs, moulding trim
//! (six profiles) and swept walls along a polyline (`make_trim_path` + `wall_runs` of
//! `editor/shapes/shape_tool.gd`).
//!
//! Unibuilder winds faces clockwise seen from outside and centres shapes on the origin; these
//! ports wind counter-clockwise (the `BMesh` convention, see `model.rs`) and the callers in
//! `shapes.rs` move them so their bottom sits on `y = 0`. Baked manual UVs are dropped.

use super::model::BMesh;
use crate::math::*;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Unibuilder's `_signed_area` (about the centroid).
fn signed_area(pts: &[Vec2]) -> f32 {
    if pts.is_empty() {
        return 0.0;
    }
    let c = pts.iter().fold(Vec2::ZERO, |a, b| a + *b) / pts.len() as f32;
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i] - c, pts[(i + 1) % n] - c);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5
}

/// `make_extruded_profile`: a (z, y) profile extruded along X by `width`, centred.
pub fn extruded_profile(profile: &[Vec2], width: f32) -> BMesh {
    let mut pts = profile.to_vec();
    if signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    let n = pts.len() as u32;
    let hw = width * 0.5;
    let mut m = BMesh::default();
    for p in &pts {
        m.add_vert(v3(hw, p.y, p.x));
    }
    for p in &pts {
        m.add_vert(v3(-hw, p.y, p.x));
    }
    // Unibuilder loops reversed (clockwise -> counter-clockwise from outside).
    m.add_face((0..n).rev().collect());
    m.add_face((0..n).map(|i| n + i).collect());
    for i in 0..n {
        let j = (i + 1) % n;
        m.add_face(vec![i, j, n + j, n + i]);
    }
    m
}

/// `_smooth_arc_walls`: side faces (index ≥ 2) whose every vertex passes `on_arc` get group 1.
fn smooth_arc_walls(m: &mut BMesh, on_arc: impl Fn(Vec3) -> bool) {
    for fi in 2..m.faces.len() {
        if m.faces[fi].verts.iter().all(|v| on_arc(m.positions[*v as usize])) {
            m.faces[fi].smooth = 1;
        }
    }
}

/// `make_door`: a wall (span along Z, `thickness` along X) with a doorway reaching the floor.
/// Centred on the origin like Unibuilder's.
#[allow(clippy::too_many_arguments)]
pub fn make_door(span: f32, height: f32, thickness: f32, pediment_height: f32, side_width: f32, arched: bool, sides: u32, smooth: bool) -> BMesh {
    let w2 = span * 0.5;
    let y0 = -height * 0.5;
    let ped = pediment_height.clamp(0.01, height * 0.95);
    let side = side_width.clamp(0.01, w2 * 0.95);
    let ow = w2 - side;
    let oh = height - ped;
    let mut profile = vec![v2(-w2, y0), v2(-w2, y0 + height), v2(w2, y0 + height), v2(w2, y0), v2(ow, y0)];
    let b = ow.min(oh);
    let spring = y0 + oh - b;
    if arched {
        let sides = sides.max(2);
        if oh > b + 1e-6 {
            profile.push(v2(ow, spring));
        }
        for i in 1..sides {
            let t = PI * i as f32 / sides as f32;
            profile.push(v2(ow * t.cos(), spring + b * t.sin()));
        }
        if oh > b + 1e-6 {
            profile.push(v2(-ow, spring));
        }
    } else {
        profile.push(v2(ow, y0 + oh));
        profile.push(v2(-ow, y0 + oh));
    }
    profile.push(v2(-ow, y0));
    let mut m = extruded_profile(&profile, thickness);
    if arched && smooth {
        smooth_arc_walls(&mut m, |p| (v2(p.z / ow, (p.y - spring) / b).length() - 1.0).abs() <= 1e-3);
    }
    m
}

/// `make_torus`: ring in the XZ plane, centred on the origin.
pub fn make_torus(ring_radius: f32, tube_radius: f32, ring_segments: u32, tube_segments: u32) -> BMesh {
    let rs = ring_segments.max(3);
    let ts = tube_segments.max(3);
    let tube = tube_radius.clamp(0.001, ring_radius * 0.95);
    let mut m = BMesh::default();
    for i in 0..rs {
        let a = TAU * i as f32 / rs as f32;
        let center = v3(a.cos() * ring_radius, 0.0, a.sin() * ring_radius);
        let radial = v3(a.cos(), 0.0, a.sin());
        for j in 0..ts {
            let b = TAU * j as f32 / ts as f32;
            m.add_vert(center + radial * (b.cos() * tube) + Vec3::UP * (b.sin() * tube));
        }
    }
    for i in 0..rs {
        let i2 = (i + 1) % rs;
        for j in 0..ts {
            let j2 = (j + 1) % ts;
            let ids = vec![i * ts + j, i2 * ts + j, i2 * ts + j2, i * ts + j2];
            let centroid = ids.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / 4.0;
            let (a0, a1) = (TAU * i as f32 / rs as f32, TAU * (i + 1) as f32 / rs as f32);
            let chord_mid = (v3(a0.cos(), 0.0, a0.sin()) + v3(a1.cos(), 0.0, a1.sin())) * (ring_radius * 0.5);
            let f = m.add_face_facing(ids, centroid - chord_mid);
            m.faces[f].smooth = 1;
        }
    }
    m
}

/// `make_curved_stairs`: the polar variant of straight stairs, sweeping `arc_deg` around +Y
/// from +X towards +Z. Centred vertically on the origin.
pub fn make_curved_stairs(inner_radius: f32, outer_radius: f32, height: f32, steps: u32, arc_deg: f32) -> BMesh {
    let arc_deg = arc_deg.clamp(5.0, 355.0);
    let steps = steps.max(1).max((arc_deg / 179.0).ceil() as u32);
    let inner = inner_radius.clamp(0.001, outer_radius * 0.95);
    let arc = arc_deg.to_radians();
    let y0 = -height * 0.5;
    let dh = height / steps as f32;
    let radii = [inner, outer_radius];
    let mut m = BMesh::default();
    let mut cache: HashMap<(usize, u32, u32), u32> = HashMap::new();
    let mut vert = |m: &mut BMesh, ring: usize, k: u32, level: u32| -> u32 {
        *cache.entry((ring, k, level)).or_insert_with(|| {
            let a = arc * k as f32 / steps as f32;
            let r = radii[ring];
            m.add_vert(v3(a.cos() * r, y0 + level as f32 * dh, a.sin() * r))
        })
    };
    for i in 0..steps {
        let mid = arc * (i as f32 + 0.5) / steps as f32;
        let radial = v3(mid.cos(), 0.0, mid.sin());
        let in0 = vert(&mut m, 0, i, 0);
        let out0 = vert(&mut m, 1, i, 0);
        let in1 = vert(&mut m, 0, i + 1, 0);
        let out1 = vert(&mut m, 1, i + 1, 0);
        let tin0 = vert(&mut m, 0, i, i + 1);
        let tout0 = vert(&mut m, 1, i, i + 1);
        let tin1 = vert(&mut m, 0, i + 1, i + 1);
        let tout1 = vert(&mut m, 1, i + 1, i + 1);
        m.add_face_facing(vec![in0, out0, out1, in1], -Vec3::UP);
        m.add_face_facing(vec![tin0, tout0, tout1, tin1], Vec3::UP);
        let mut wall_out = vec![out0, out1, tout1, tout0];
        let mut wall_in = vec![in0, in1, tin1, tin0];
        if i > 0 {
            wall_out.push(vert(&mut m, 1, i, i));
            wall_in.push(vert(&mut m, 0, i, i));
        }
        m.add_face_facing(wall_out, radial);
        m.add_face_facing(wall_in, -radial);
        let a0 = arc * i as f32 / steps as f32;
        let tangent = v3(-a0.sin(), 0.0, a0.cos());
        if i == 0 {
            m.add_face_facing(vec![in0, out0, tout0, tin0], -tangent);
        } else {
            let pin = vert(&mut m, 0, i, i);
            let pout = vert(&mut m, 1, i, i);
            m.add_face_facing(vec![pin, pout, tout0, tin0], -tangent);
        }
    }
    let end_tangent = v3(-arc.sin(), 0.0, arc.cos());
    let e = [vert(&mut m, 0, steps, 0), vert(&mut m, 1, steps, 0), vert(&mut m, 1, steps, steps), vert(&mut m, 0, steps, steps)];
    m.add_face_facing(e.to_vec(), end_tangent);
    m
}

/// `fit_xz`: centres the XZ footprint on the origin with its larger side spanning `extent`.
pub fn fit_xz(m: &mut BMesh, extent: f32) {
    let Some((lo, hi)) = m.bounds() else { return };
    let span = (hi.x - lo.x).max(hi.z - lo.z);
    if span <= 1e-6 {
        return;
    }
    let k = extent.max(1e-6) / span;
    let (cx, cz) = ((lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5);
    for p in &mut m.positions {
        *p = v3((p.x - cx) * k, p.y, (p.z - cz) * k);
    }
}

/// Trim cross-sections (UBPrimitives.TrimProfile).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TrimProfile {
    Flat = 0,
    #[default]
    Chamfer,
    Round,
    Cove,
    Ogee,
    Stepped,
}

impl TrimProfile {
    pub const NAMES: [&'static str; 6] = ["Flat", "Chamfer", "Round", "Cove", "Ogee", "Stepped"];
    pub fn from_u8(v: u8) -> TrimProfile {
        [TrimProfile::Flat, TrimProfile::Chamfer, TrimProfile::Round, TrimProfile::Cove, TrimProfile::Ogee, TrimProfile::Stepped][(v as usize).min(5)]
    }
    pub fn curved(self) -> bool {
        matches!(self, TrimProfile::Round | TrimProfile::Cove | TrimProfile::Ogee)
    }
}

/// Profile points with the arc ids each point belongs to (`_prof_append`).
struct Profile {
    pts: Vec<Vec2>,
    arcs: Vec<Vec<u32>>,
}

impl Profile {
    fn push(&mut self, p: Vec2, arc: Option<u32>) {
        let tol = 1e-5 * p.length().max(1.0);
        if self.pts.last().is_none_or(|l| l.distance(p) > tol) {
            self.pts.push(p);
            self.arcs.push(arc.into_iter().collect());
        } else if let Some(a) = arc {
            let last = self.arcs.last_mut().unwrap();
            if !last.contains(&a) {
                last.push(a);
            }
        }
    }
}

/// `trim_profile`: the (out, up) cross-section filling `depth` x `height`.
fn trim_profile(profile: TrimProfile, depth: f32, height: f32, segs: u32) -> Profile {
    let d = depth.max(1e-4);
    let h = height.max(1e-4);
    let segs = segs.clamp(1, 32);
    let mut p = Profile { pts: Vec::new(), arcs: Vec::new() };
    p.push(v2(0.0, 0.0), None);
    p.push(v2(0.0, h), None);
    let t = |k: u32| FRAC_PI_2 * k as f32 / segs as f32;
    match profile {
        TrimProfile::Chamfer => {
            let c = d.min(h * 0.5);
            p.push(v2(d - c, h), None);
            p.push(v2(d, h - c), None);
        }
        TrimProfile::Round => {
            let r = d.min(h * 0.5);
            for k in 0..=segs {
                p.push(v2(d - r + r * t(k).sin(), h - r + r * t(k).cos()), Some(0));
            }
        }
        TrimProfile::Cove => {
            for k in 0..=segs {
                p.push(v2(d - d * t(k).cos(), h - h * t(k).sin()), Some(0));
            }
        }
        TrimProfile::Ogee => {
            let k2 = (h * 0.5).min(d * 1.5);
            let mx = d * 0.5;
            let my = h - k2 * 0.5;
            for i in 0..=segs {
                p.push(v2(mx * t(i).sin(), my + k2 * 0.5 * t(i).cos()), Some(0));
            }
            for i in 0..=segs {
                p.push(v2(d - mx * t(i).cos(), my - k2 * 0.5 * t(i).sin()), Some(1));
            }
        }
        TrimProfile::Stepped => {
            p.push(v2(d * 0.5, h), None);
            p.push(v2(d * 0.5, h * 0.6), None);
            p.push(v2(d, h * 0.6), None);
        }
        TrimProfile::Flat => p.push(v2(d, h), None),
    }
    p.push(v2(d, 0.0), None);
    p
}

fn share_arc(a: &[u32], b: &[u32]) -> bool {
    a.iter().any(|x| b.contains(x))
}

/// `make_trim` (with `_emit_profile_solid` and `_FRAME_TURN`): a straight moulding strip running
/// along Z (`length`), its profile across X (`across` wide, out from the -X side), `depth` x
/// `height`. Centred on the origin in X / Z, bottom at y = 0.
#[allow(clippy::too_many_arguments)]
pub fn make_trim(length: f32, across: f32, depth: f32, height: f32, profile: TrimProfile, segs: u32, upright: bool, flip: bool, invert: bool, smooth: bool) -> BMesh {
    let prof = trim_profile(profile, depth, height, segs);
    let (x0, x1) = (-across * 0.5, across * 0.5);
    let mut pts: Vec<Vec2> = prof
        .pts
        .iter()
        .map(|p| {
            let a = p.x;
            let mut b = p.y;
            if invert {
                b = height.max(1e-4) - b;
            }
            if upright { v2(if flip { x1 - b } else { x0 + b }, a) } else { v2(if flip { x1 - a } else { x0 + a }, b) }
        })
        .collect();
    let mut arcs = prof.arcs;
    if signed_area(&pts) < 0.0 {
        pts.reverse();
        arcs.reverse();
    }
    // _FRAME_TURN = [(0,0,-1), X]: local x -> -Z, local z (= profile x) -> X.
    let map = |lx: f32, y: f32, lz: f32| v3(lz, y, -lx);
    let (x_lo, x_hi) = (-length * 0.5, length * 0.5);
    let n = pts.len() as u32;
    let mut m = BMesh::default();
    let mut cache: HashMap<[i64; 3], u32> = HashMap::new();
    let mut vid = |m: &mut BMesh, p: Vec3| -> u32 {
        let k = [(p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64];
        *cache.entry(k).or_insert_with(|| m.add_vert(p))
    };
    let right: Vec<u32> = pts.iter().map(|p| vid(&mut m, map(x_hi, p.y, p.x))).collect();
    let left: Vec<u32> = pts.iter().map(|p| vid(&mut m, map(x_lo, p.y, p.x))).collect();
    // Unibuilder loops reversed.
    m.add_face(right.iter().rev().copied().collect());
    m.add_face(left.clone());
    for i in 0..n as usize {
        let j = (i + 1) % n as usize;
        let f = m.add_face(vec![right[i], right[j], left[j], left[i]]);
        if smooth && share_arc(&arcs[i], &arcs[j]) {
            m.faces[f].smooth = 1;
        }
    }
    let lo_y = m.bounds().map(|b| b.0.y).unwrap_or(0.0);
    m.translate(v3(0.0, -lo_y, 0.0));
    m
}

/// `make_trim_path`: sweeps a trim profile along `points` with mitred corners. `normals[i]` is
/// the outward side of segment i (horizontal). `out_offset` shifts the profile outwards.
#[allow(clippy::too_many_arguments)]
pub fn make_trim_path(points: &[Vec3], normals: &[Vec3], closed: bool, up: Vec3, depth: f32, height: f32, profile: TrimProfile, segs: u32, hang: bool, invert: bool, offset: f32, smooth: bool, out_offset: f32) -> BMesh {
    let mut m = BMesh::default();
    let n = points.len();
    let seg_count = if closed { n } else { n.saturating_sub(1) };
    if n < 2 || normals.len() < seg_count {
        return m;
    }
    let prof = trim_profile(profile, depth, height, segs);
    let (mut pts, mut arcs) = (prof.pts, prof.arcs);
    if signed_area(&pts) < 0.0 {
        pts.reverse();
        arcs.reverse();
    }
    let h = height.max(1e-4);
    let count = pts.len();
    let mut rings: Vec<Vec<u32>> = Vec::with_capacity(n);
    for i in 0..n {
        let mt = if closed || (i > 0 && i < n - 1) {
            let n0 = normals[(i + seg_count - 1) % seg_count];
            let n1 = normals[i % seg_count];
            let d = 1.0 + n0.dot(n1);
            if d > 1e-4 { (n0 + n1) / d } else { n1 }
        } else if i == 0 {
            normals[0]
        } else {
            normals[seg_count - 1]
        };
        let ring = pts
            .iter()
            .map(|p| {
                let mut b = if invert { h - p.y } else { p.y };
                if hang {
                    b = -b;
                }
                m.add_vert(points[i] + mt * (p.x + out_offset) + up * (b + offset))
            })
            .collect();
        rings.push(ring);
    }
    let b_sign = if invert { -1.0 } else { 1.0 } * if hang { -1.0 } else { 1.0 };
    for i in 0..seg_count {
        let (r0, r1) = (rings[i].clone(), rings[(i + 1) % n].clone());
        let side = normals[i];
        for k in 0..count {
            let k2 = (k + 1) % count;
            let e = pts[k2] - pts[k];
            let outward = side * e.y + up * (-e.x * b_sign);
            let f = m.add_face_facing(vec![r0[k], r0[k2], r1[k2], r1[k]], outward);
            if smooth && share_arc(&arcs[k], &arcs[k2]) {
                m.faces[f].smooth = 1;
            }
        }
    }
    if !closed {
        let d0 = (points[1] - points[0]).normalized();
        let d1 = (points[n - 1] - points[n - 2]).normalized();
        m.add_face_facing(rings[0].clone(), -d0);
        m.add_face_facing(rings[n - 1].clone(), d1);
    }
    m
}

/// `wall_runs`: splits a wall's corners into runs that each sweep cleanly (folds and corners
/// too sharp for the wall width start a new run).
pub fn wall_runs(pts: &[Vec3], normals: &[Vec3], closed: bool, width: f32) -> Vec<(Vec<Vec3>, Vec<Vec3>)> {
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let mut folds: Vec<usize> = Vec::new();
    let range: Vec<usize> = if closed { (0..n).collect() } else { (1..n.saturating_sub(1)).collect() };
    for i in range {
        let n0 = normals[(i + seg_count - 1) % seg_count];
        let n1 = normals[i % seg_count];
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let d = 1.0 + n0.dot(n1);
        if d <= 1e-4 {
            folds.push(i);
            continue;
        }
        let dir_in = (pts[i] - prev).normalized();
        let reach = ((n0 + n1) / d).dot(dir_in).abs() * width;
        if reach >= (pts[i] - prev).length().min((pts[i] - next).length()) {
            folds.push(i);
        }
    }
    if folds.is_empty() {
        return vec![(pts.to_vec(), normals.to_vec())];
    }
    let (seq_pts, seq_ns, mut cuts): (Vec<Vec3>, Vec<Vec3>, Vec<usize>) = if closed {
        let s = folds[0];
        let sp = (0..=n).map(|k| pts[(s + k) % n]).collect();
        let sn = (0..n).map(|k| normals[(s + k) % n]).collect();
        let c = folds.iter().map(|f| (f + n - s) % n).filter(|k| *k > 0).collect();
        (sp, sn, c)
    } else {
        (pts.to_vec(), normals.to_vec(), folds)
    };
    cuts.push(seq_pts.len() - 1);
    let mut runs = Vec::new();
    let mut start = 0;
    for c in cuts {
        runs.push((seq_pts[start..=c].to_vec(), seq_ns[start..c].to_vec()));
        start = c;
    }
    runs
}

/// Wall side (Unibuilder's "Side" parameter): which side of the drawn line the wall occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WallSide {
    #[default]
    Centre,
    Left,
    Right,
}

/// The `Kind.WALL` branch of `UBShapeTool.build_shape`: a wall of `thickness` x `height` swept
/// along the polyline `raw` (closed loops need ≥ 3 points).
pub fn make_wall(raw: &[Vec3], closed: bool, thickness: f32, height: f32, side: WallSide) -> BMesh {
    let up = Vec3::UP;
    let mut pts: Vec<Vec3> = Vec::new();
    for p in raw {
        if pts.last().is_none_or(|l| (*l - *p).length() > 1e-4) {
            pts.push(*p);
        }
    }
    if closed && pts.len() > 1 && (pts[0] - *pts.last().unwrap()).length() <= 1e-4 {
        pts.pop();
    }
    if pts.len() < 2 || (closed && pts.len() < 3) {
        return BMesh::default();
    }
    let thickness = thickness.max(0.001);
    let seg_count = if closed { pts.len() } else { pts.len() - 1 };
    let normals: Vec<Vec3> = (0..seg_count).map(|i| (pts[(i + 1) % pts.len()] - pts[i]).normalized().cross(up).normalized()).collect();
    let out_offset = match side {
        WallSide::Centre => -thickness * 0.5,
        WallSide::Left => -thickness,
        WallSide::Right => 0.0,
    };
    let height = height.max(0.001);
    let runs = wall_runs(&pts, &normals, closed, out_offset.abs().max((thickness + out_offset).abs()));
    if runs.len() == 1 {
        return make_trim_path(&pts, &normals, closed, up, thickness, height, TrimProfile::Flat, 1, false, false, 0.0, false, out_offset);
    }
    let mut merged = BMesh::default();
    for (rp, rn) in runs {
        merged.append(&make_trim_path(&rp, &rn, false, up, thickness, height, TrimProfile::Flat, 1, false, false, 0.0, false, out_offset));
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(m: &BMesh, name: &str) {
        m.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(m.is_closed(), "{name} not closed");
        assert!(m.volume() > 0.0, "{name} inside out: {}", m.volume());
    }

    #[test]
    fn torus_volume() {
        let m = make_torus(1.0, 0.25, 32, 16);
        check(&m, "torus");
        let exact = 2.0 * PI * PI * 0.25 * 0.25;
        assert!((m.volume() - exact).abs() / exact < 0.05, "{} vs {exact}", m.volume());
    }

    #[test]
    fn doors_are_closed() {
        let flat = make_door(2.0, 2.5, 0.3, 0.5, 0.6, false, 8, false);
        check(&flat, "door");
        let expect = 0.3 * (2.0 * 2.5 - 0.8 * 2.0);
        assert!((flat.volume() - expect).abs() < 1e-3, "{} vs {expect}", flat.volume());
        let arched = make_door(2.0, 2.5, 0.3, 0.5, 0.6, true, 8, true);
        check(&arched, "arched door");
        assert!(arched.volume() > flat.volume(), "the arch fills the top corners");
        assert!(arched.faces.iter().any(|f| f.smooth == 1));
    }

    #[test]
    fn curved_stairs_closed() {
        let m = make_curved_stairs(0.5, 1.5, 1.0, 8, 90.0);
        check(&m, "curved stairs");
        let (lo, hi) = m.bounds().unwrap();
        assert!((hi.y - lo.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn trims_of_every_profile() {
        for p in 0..6 {
            let pr = TrimProfile::from_u8(p);
            for (up, fl, inv) in [(false, false, false), (true, true, false), (false, false, true)] {
                let m = make_trim(2.0, 0.3, 0.2, 0.3, pr, 4, up, fl, inv, true);
                check(&m, TrimProfile::NAMES[p as usize]);
                assert!(m.bounds().unwrap().0.y.abs() < 1e-5);
            }
        }
    }

    #[test]
    fn walls_open_and_closed() {
        let line = [v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0)];
        let w = make_wall(&line, false, 0.2, 2.0, WallSide::Centre);
        check(&w, "wall");
        assert!((w.volume() - 4.0 * 0.2 * 2.0).abs() < 1e-3, "{}", w.volume());
        let room = [v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0), v3(4.0, 0.0, 3.0), v3(0.0, 0.0, 3.0)];
        let r = make_wall(&room, true, 0.2, 2.0, WallSide::Centre);
        check(&r, "room");
        assert!((r.volume() - 2.0 * 0.2 * 14.0).abs() < 1e-2, "{}", r.volume());
        let l = make_wall(&room[..3], false, 0.2, 2.0, WallSide::Left);
        check(&l, "L wall");
    }
}
