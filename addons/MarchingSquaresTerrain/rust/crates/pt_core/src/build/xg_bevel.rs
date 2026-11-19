//! Xogot Modeler's Bevel Mode options on top of the builder bevel (`bevel.rs`), plus the vertex
//! bevel Xogot never shipped a port of.
//!
//! Ported from xogot-rust (MIT, Xibbon Inc. / port authors):
//! - `modeler_kit/src/mesh_bevel.rs`: `MeshBevelOptions` (amount type, profile, clamp overlap,
//!   harden normals, guards), `requested_offset` (offset / width / depth / percent),
//!   `solve_edge_samples` (corner offsets `d / sin α`, mean of the samples of one edge point,
//!   per-vertex clamp factors from `length_budget`), `solve_edges` (overlap retry that halves
//!   every clamp factor, clamped-corner report), `profile_weights` / `make_profile` (the profile
//!   curve and its bulge), `patch_center` / `spread` (hub of a round corner patch),
//!   `smoothing_site` (hardened normals: one smoothing group per bevel cluster).
//! - `editor_core/src/bevel_mode.rs`: `summary`, `clamp_notice`, `omitted_notice`, `format`,
//!   `formatted`, `amount_type_title`, `preview_segments` (coarse preview under 5000 faces).
//!
//! The edge strips and corner patches themselves are still built by the port in
//! `bevel.rs` (its slide/arc representation), fed with the distances computed here.
//!
//! Vertex bevel: Xogot's `solveVertices` (Swift `MeshBevel.solveVertices`) was never ported by
//! xogot-rust and its decompiled form is not readable enough to port, so `bevel_vertices` is
//! this repo's own vertex chamfer, built from the same pieces as the edge path: per-edge cut
//! points at the resolved amount, the same clamp budget, Xogot's profile curve for the rounded
//! face corners and Xogot's `patch_center` for the hub of the rounded cap.

use super::bevel::{self, Custom, Key, SlideSample};
use super::model::{BMesh, Edge, edge, newell};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Which elements the bevel acts on (Xogot `MeshBevelAffect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BevelAffect {
    Edges,
    Vertices,
}

/// How the amount is measured (Xogot `MeshBevelAmountType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountType {
    /// Distance from the bevelled edge (vertex bevel: along each edge).
    Offset,
    /// Width of the new face (edge bevel: across the chamfer; vertex bevel: cap side length).
    Width,
    /// Depth of the cut, measured from the old edge / vertex.
    Depth,
    /// Percent (0–100) of the adjacent edge length.
    Percent,
}

impl AmountType {
    pub const ALL: [AmountType; 4] = [AmountType::Offset, AmountType::Width, AmountType::Depth, AmountType::Percent];
    pub const NAMES: [&'static str; 4] = ["Offset", "Width", "Depth", "Percent"];
    pub fn from_u8(v: u8) -> AmountType {
        AmountType::ALL[(v as usize).min(3)]
    }
    /// `amount_type_title`.
    pub fn title(self) -> &'static str {
        AmountType::NAMES[self as usize]
    }
}

/// Bevel settings (Xogot `MeshBevelOptions`; material / UV policies are the builder's own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelOptions {
    pub affect: BevelAffect,
    pub amount: f32,
    pub amount_type: AmountType,
    /// 1..=32 (1 = chamfer).
    pub segments: u32,
    /// 0..=1, 0.5 = circular.
    pub profile: f32,
    /// Shrink the amount where it would cross a neighbouring vertex (else refuse the bevel).
    pub clamp_overlap: bool,
    /// New faces get their own smoothing group per connected bevel (else they keep the group of
    /// the face they came from).
    pub harden_normals: bool,
}

impl Default for BevelOptions {
    fn default() -> Self {
        BevelOptions {
            affect: BevelAffect::Edges,
            amount: 0.2,
            amount_type: AmountType::Offset,
            segments: 1,
            profile: 0.5,
            clamp_overlap: true,
            harden_normals: false,
        }
    }
}

impl BevelOptions {
    /// `MeshBevelOptions.segmentRange`.
    pub const MAX_SEGMENTS: u32 = 32;
    /// `MeshBevelOptions.minimumAmount`.
    pub const MINIMUM_AMOUNT: f32 = 0.001;

    /// The options with every field in range: segments 1..=32, profile 0..=1, percent ≤ 100.
    pub fn sanitized(&self) -> BevelOptions {
        let mut o = *self;
        o.segments = o.segments.clamp(1, Self::MAX_SEGMENTS);
        o.profile = if o.profile.is_finite() { o.profile.clamp(0.0, 1.0) } else { 0.5 };
        if o.amount_type == AmountType::Percent {
            o.amount = o.amount.min(100.0);
        }
        o
    }
}

/// What a bevel did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BevelReport {
    /// The new faces (strips, corner patches, vertex caps).
    pub faces: BTreeSet<usize>,
    /// The smallest amount actually used, in the option's units, when clamping reduced it.
    pub clamped_to: Option<f32>,
    /// How many vertices had their amount reduced.
    pub clamped_corners: usize,
    /// Selected edges that were left alone (no room, missing, or a reflex corner that can't be
    /// mitred).
    pub skipped_edges: BTreeSet<Edge>,
    /// Selected vertices that were left alone (no faces, or too short edges).
    pub skipped_verts: BTreeSet<u32>,
    /// Clamping was off and the amount came within 5% of a neighbouring vertex.
    pub may_overlap: bool,
    /// The segment count used.
    pub segments: u32,
}

impl BevelReport {
    pub fn skipped(&self) -> usize {
        self.skipped_edges.len() + self.skipped_verts.len()
    }
}

const SET_AMOUNT: &str = "Set the amount above zero.";
const SELECT_TO_BEVEL: &str = "Select edges, faces, or vertices to bevel.";
const NO_ROOM: &str = "No room: a short edge next to the selection limits the bevel.";
const TOO_SMALL: &str = "Too small";
const OVERLAP: &str = "The bevel would make faces overlap. Use a smaller amount.";
const JUNCTION: &str = "This bevel junction is not supported. Change the selection.";
const NON_MANIFOLD: &str = "The mesh has degenerate or non-manifold topology.";
/// Internal: some edges/vertices fell below the minimum amount; drop them and solve again.
const RETRY: &str = "\u{0}retry";
/// `MINIMUM_SINE` (sin 1°): flatter corners slide by the plain offset.
const MINIMUM_SINE: f32 = 0.017_452_4;
const OVERLAP_RETRY_LIMIT: usize = 5;
/// `BevelSession::COARSE_FACE_LIMIT`.
pub const COARSE_FACE_LIMIT: i64 = 5000;

// ---------------------------------------------------------------- entry points

/// Bevels the elements `opts.affect` names: `edges` for an edge bevel, `verts` for a vertex
/// bevel. On error the mesh is unchanged.
pub fn bevel(m: &mut BMesh, edges: &BTreeSet<Edge>, verts: &BTreeSet<u32>, opts: &BevelOptions) -> Result<BevelReport, String> {
    match opts.affect {
        BevelAffect::Edges => bevel_edges_ex(m, edges, opts),
        BevelAffect::Vertices => bevel_vertices(m, verts, opts),
    }
}

fn check_options(opts: &BevelOptions) -> Result<BevelOptions, String> {
    let o = opts.sanitized();
    if !o.amount.is_finite() || o.amount < BevelOptions::MINIMUM_AMOUNT {
        return Err(SET_AMOUNT.into());
    }
    Ok(o)
}

/// Commits `w` into `m` if it is a valid result (and stays closed when `m` was closed).
fn accept(m: &mut BMesh, w: BMesh) -> Result<(), String> {
    w.validate().map_err(|_| OVERLAP.to_string())?;
    if m.is_closed() && !w.is_closed() {
        return Err(OVERLAP.into());
    }
    *m = w;
    Ok(())
}

/// `lengthBudget`: how much of an edge two bevels may use together, leaving a 5% (≥ 0.001) gap.
fn length_budget(length: f32) -> f32 {
    (length - (length * 0.05).max(BevelOptions::MINIMUM_AMOUNT)).max(0.0)
}

fn pos(m: &BMesh, v: u32) -> Vec3 {
    m.positions[v as usize]
}

// ---------------------------------------------------------------- edge bevel

/// Port of `requested_offset`: the offset (distance from the edge, in each face) that the amount
/// type asks for on `e`. Open edges measure Width / Depth as Offset.
fn requested_offset(m: &BMesh, edge_faces: &HashMap<Edge, Vec<usize>>, e: Edge, opts: &BevelOptions) -> Result<f32, String> {
    let amount = opts.amount;
    let faces = edge_faces.get(&e).cloned().unwrap_or_default();
    match opts.amount_type {
        AmountType::Offset => Ok(amount),
        AmountType::Width | AmountType::Depth => {
            if faces.len() != 2 {
                return Ok(amount);
            }
            let (pa, pb) = (pos(m, e.0), pos(m, e.1));
            let direction = (pb - pa).normalized();
            let midpoint = (pa + pb) * 0.5;
            let mut tangents = Vec::with_capacity(2);
            for f in faces {
                let mut tangent = m.face_normal(f).cross(direction).normalized();
                if (m.face_center(f) - midpoint).dot(tangent) < 0.0 {
                    tangent = -tangent;
                }
                tangents.push(tangent);
            }
            let angle = tangents[1].dot(tangents[0]).clamp(-1.0, 1.0).acos() * 0.5;
            let factor = if opts.amount_type == AmountType::Width { 2.0 * angle.sin() } else { angle.cos() };
            if factor <= 0.001 {
                return Err(JUNCTION.into());
            }
            Ok(amount / factor)
        }
        AmountType::Percent => {
            let mut shortest = f32::INFINITY;
            for (end, other) in [(e.0, e.1), (e.1, e.0)] {
                for &f in &faces {
                    let lp = &m.faces[f].verts;
                    let n = lp.len();
                    let Some(i) = lp.iter().position(|v| *v == end) else { continue };
                    for nb in [lp[(i + 1) % n], lp[(i + n - 1) % n]] {
                        if nb != other {
                            shortest = shortest.min((pos(m, end) - pos(m, nb)).length());
                        }
                    }
                }
            }
            if !shortest.is_finite() {
                return Err(JUNCTION.into());
            }
            Ok(amount.min(100.0) / 100.0 * shortest)
        }
    }
}

/// Clamp state of one edge-bevel attempt.
#[derive(Default)]
struct EdgeSolve {
    requested: HashMap<Key, f32>,
    too_small: BTreeSet<Edge>,
    may_overlap: bool,
}

/// Port of the distance half of `solve_edge_samples`: per-slide distances (`offset / sin α`,
/// averaged per slide), per-vertex clamp factors from `length_budget`, too-small edges.
fn resolve_slides(
    m: &BMesh,
    selected: &BTreeSet<Edge>,
    samples: &[SlideSample],
    offsets: &BTreeMap<Edge, f32>,
    opts: &BevelOptions,
    retry_factor: f32,
    st: &mut EdgeSolve,
) -> Result<HashMap<Key, f32>, String> {
    let mut sampled: BTreeMap<Key, Vec<f32>> = BTreeMap::new();
    let mut back: BTreeSet<Key> = BTreeSet::new();
    for s in samples {
        let d = offsets.get(&s.away).copied().unwrap_or(opts.amount);
        let (v, toward) = s.key;
        let val = if s.back {
            back.insert(s.key);
            d
        } else {
            let far = if s.away.0 == v { s.away.1 } else { s.away.0 };
            let (u, w) = (pos(m, toward) - pos(m, v), pos(m, far) - pos(m, v));
            let sine = u.cross(w).length() / (u.length() * w.length()).max(1e-20);
            if sine < MINIMUM_SINE { d } else { d / sine }
        };
        sampled.entry(s.key).or_default().push(val);
    }
    st.requested = sampled.iter().map(|(k, v)| (*k, v.iter().sum::<f32>() / v.len() as f32)).collect();
    let mut scales: HashMap<u32, f32> = HashMap::new();
    let mut seen: BTreeSet<Edge> = BTreeSet::new();
    for &(v, t) in st.requested.keys() {
        let e = edge(v, t);
        if !seen.insert(e) {
            continue;
        }
        let from = |a: u32, b: u32| if back.contains(&(a, b)) { 0.0 } else { st.requested.get(&(a, b)).copied().unwrap_or(0.0) };
        let (fx, fy) = (from(e.0, e.1), from(e.1, e.0));
        let sum = fx + fy;
        let length = (pos(m, e.0) - pos(m, e.1)).length();
        let budget = length_budget(length);
        if sum <= budget || sum <= 0.0 {
            continue;
        }
        if opts.clamp_overlap {
            let limit = budget / sum;
            for (vert, f) in [(e.0, fx), (e.1, fy)] {
                if f > 0.0 {
                    let s = scales.entry(vert).or_insert(1.0);
                    *s = s.min(limit);
                }
            }
        } else if sum >= length * 0.999 {
            return Err(OVERLAP.into());
        } else {
            st.may_overlap = true;
        }
    }
    let scale = |v: u32| scales.get(&v).copied().unwrap_or(1.0) * retry_factor;
    st.too_small = selected.iter().copied().filter(|e| offsets.get(e).copied().unwrap_or(opts.amount) * scale(e.0).min(scale(e.1)) < BevelOptions::MINIMUM_AMOUNT).collect();
    if !st.too_small.is_empty() {
        return Err(RETRY.into());
    }
    Ok(st.requested.iter().map(|(k, d)| (*k, d * scale(k.0))).collect())
}

/// Edge bevel with Xogot's Bevel Mode options (amount type, profile, clamp overlap, harden
/// normals). Port of `MeshBevel.solve` / `solve_edges` onto the builder bevel: targets that end
/// up below the minimum amount are dropped and the rest solved again (up to four rounds), and a
/// result that would overlap is retried with every clamp factor halved (clamping on).
/// On error the mesh is unchanged.
pub fn bevel_edges_ex(m: &mut BMesh, edges: &BTreeSet<Edge>, opts: &BevelOptions) -> Result<BevelReport, String> {
    let opts = check_options(opts)?;
    let edge_faces = m.edge_faces();
    let mut report = BevelReport { segments: opts.segments, ..Default::default() };
    let mut active: BTreeSet<Edge> = BTreeSet::new();
    for e in edges {
        if edge_faces.get(e).is_some_and(|f| f.len() == 1 || f.len() == 2) {
            active.insert(*e);
        } else {
            report.skipped_edges.insert(*e);
        }
    }
    if active.is_empty() {
        return Err(SELECT_TO_BEVEL.into());
    }
    for _round in 0..4 {
        let mut offsets: BTreeMap<Edge, f32> = BTreeMap::new();
        for e in &active {
            offsets.insert(*e, requested_offset(m, &edge_faces, *e, &opts)?);
        }
        let mut retry_factor = 1.0f32;
        let mut retries = 0;
        loop {
            let mut w = m.clone();
            let mut st = EdgeSolve::default();
            let result = {
                let mut resolve = |mesh: &BMesh, selected: &BTreeSet<Edge>, samples: &[SlideSample]| resolve_slides(mesh, selected, samples, &offsets, &opts, retry_factor, &mut st);
                let boundary = |e: Edge| offsets.get(&e).copied().unwrap_or(opts.amount) * retry_factor;
                let mut custom = Custom { profile: opts.profile, resolve: &mut resolve, boundary: &boundary };
                bevel::bevel_edges_custom(&mut w, &active, opts.segments, &mut custom)
            };
            let out = match result {
                Ok(out) => out,
                Err(e) if e == RETRY => {
                    for e in &st.too_small {
                        active.remove(e);
                        report.skipped_edges.insert(*e);
                    }
                    break;
                }
                Err(e) => return Err(e),
            };
            // What was actually applied, against what was asked for.
            let mut min_ratio = 1.0f32;
            let mut clamped: BTreeSet<u32> = BTreeSet::new();
            for (k, d) in &out.applied {
                let req = st.requested.get(k).copied().unwrap_or(*d);
                if req > 0.0 && *d < req * 0.999 {
                    min_ratio = min_ratio.min(d / req);
                    clamped.insert(k.0);
                }
            }
            for (e, d) in &out.applied_boundary {
                let req = offsets.get(e).copied().unwrap_or(*d);
                if req > 0.0 && *d < req * 0.999 {
                    min_ratio = min_ratio.min(d / req);
                    clamped.insert(e.0);
                    clamped.insert(e.1);
                }
            }
            let valid = out.faces.iter().all(|f| *f < w.faces.len()) && w.validate().is_ok() && (!m.is_closed() || w.is_closed());
            if !opts.clamp_overlap && !clamped.is_empty() {
                return Err(OVERLAP.into());
            }
            if !valid {
                if opts.clamp_overlap && retries < OVERLAP_RETRY_LIMIT {
                    retries += 1;
                    retry_factor *= 0.5;
                    continue;
                }
                return Err(OVERLAP.into());
            }
            if out.faces.is_empty() {
                // Every edge was dropped (reflex corners that can't be mitred): nothing done.
                return Err(JUNCTION.into());
            }
            if opts.harden_normals {
                harden(&mut w, &out.faces);
            }
            accept(m, w)?;
            report.skipped_edges.extend(out.dropped.iter().copied());
            report.faces = out.faces;
            report.clamped_corners = clamped.len();
            report.clamped_to = (!clamped.is_empty()).then_some(opts.amount * min_ratio);
            report.may_overlap = st.may_overlap;
            return Ok(report);
        }
        // Some targets fell below the minimum amount: solve again without them.
        if active.is_empty() {
            break;
        }
    }
    Err(if opts.amount >= BevelOptions::MINIMUM_AMOUNT { NO_ROOM } else { TOO_SMALL }.into())
}

/// Port of `smoothing_site` with hardened normals: every connected group of new faces gets its
/// own smoothing group, one above the highest group already in use.
fn harden(m: &mut BMesh, new_faces: &BTreeSet<usize>) {
    let base = (0..m.faces.len()).filter(|f| !new_faces.contains(f)).map(|f| m.faces[f].smooth).max().unwrap_or(0) + 1;
    let list: Vec<usize> = new_faces.iter().copied().collect();
    let mut parent: Vec<usize> = (0..list.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut owner: HashMap<u32, usize> = HashMap::new();
    for (i, f) in list.iter().enumerate() {
        for v in &m.faces[*f].verts {
            if let Some(&j) = owner.get(v) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            } else {
                owner.insert(*v, i);
            }
        }
    }
    let mut groups: BTreeMap<usize, u16> = BTreeMap::new();
    for i in 0..list.len() {
        let root = find(&mut parent, i);
        let next = base.saturating_add(groups.len() as u16);
        let g = *groups.entry(root).or_insert(next);
        m.faces[list[i]].smooth = g;
    }
}

// ---------------------------------------------------------------- profile (shared with bevel.rs)

/// Port of `profile_weights`: the two curve-handle fractions of a round corner from `a` to `b`
/// around `control`. At `profile == 0.5` both are the circular-arc handle.
pub fn profile_weights(a: Vec3, b: Vec3, control: Vec3, profile: f32) -> (f32, f32) {
    let to_a = (a - control).normalized();
    let to_b = (b - control).normalized();
    let gamma = to_a.dot(to_b).clamp(-1.0, 1.0).acos();
    let phi = std::f32::consts::PI - gamma;
    let half = (phi * 0.5).tan();
    let symmetric = if half > 0.0001 { 2.0 * (4.0 / 3.0) * (phi * 0.25).tan() / half } else { 4.0 / 3.0 };
    (((1.0 - profile) * symmetric).clamp(0.0, 1.0), (profile * symmetric).clamp(0.0, 1.0))
}

/// Port of the curve in `make_profile`: the `segments - 1` interior points of the cubic from
/// `a` to `b` whose handles lean towards `control` by `profile_weights`.
pub fn profile_curve(a: Vec3, b: Vec3, control: Vec3, profile: f32, segments: usize) -> Vec<Vec3> {
    let (first, second) = profile_weights(a, b, control, profile);
    let n = segments as f32;
    (1..segments)
        .map(|j| {
            let t = j as f32 / n;
            let s = 1.0 - t;
            let (b0, b1, b2, b3) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
            let ck = b2 * second + first * b1;
            let ca = b0 + b1 * (1.0 - first);
            let cb = b3 + b2 * (1.0 - second);
            a * ca + b * cb + control * ck
        })
        .collect()
}

/// Port of the `bulge` of `make_profile`: how far the curve's middle leans from the chord
/// towards `control` (0 = straight, 1 = through the control point).
fn profile_bulge(a: Vec3, b: Vec3, control: Vec3, profile: f32) -> f32 {
    let middle = (a + b) * 0.5;
    let reference = (control - middle).length();
    if reference <= 1e-7 {
        return 0.0;
    }
    let (first, second) = profile_weights(a, b, control, profile);
    let ca = 0.375 * (1.0 - first) + 0.125;
    let cb = 0.375 * (1.0 - second) + 0.125;
    let ck = (first + second) * 0.375;
    ((a * ca + b * cb + control * ck) - middle).length() / reference
}

/// Port of `patch_center` / `spread`: the sphere through the rim of a round corner patch, when
/// the rim is close enough to one (`(origin, radius)`), searched along the axis from the corner
/// through the rim centroid.
fn patch_sphere(points: &[Vec3], corner: Vec3) -> Option<(Vec3, f32)> {
    let centroid = points.iter().fold(Vec3::ZERO, |s, p| s + *p) / points.len() as f32;
    let axis = centroid - corner;
    if axis.dot(axis) <= 1e-14 {
        return None;
    }
    let spread = |lambda: f32| {
        let origin = corner + axis * lambda;
        let d: Vec<f32> = points.iter().map(|p| (*p - origin).length()).collect();
        let mean = d.iter().sum::<f32>() / d.len() as f32;
        let var = d.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / d.len() as f32;
        (var, mean)
    };
    let (mut low, mut high) = (1.0f32, 6.0f32);
    for _ in 0..60 {
        let third = (high - low) / 3.0;
        let (a, b) = (low + third, high - third);
        if spread(a).0 <= spread(b).0 {
            high = b;
        } else {
            low = a;
        }
    }
    let lambda = (low + high) * 0.5;
    let (var, radius) = spread(lambda);
    let origin = corner + axis * lambda;
    let dist = (corner - origin).length();
    (radius > 0.0 && var.sqrt() < radius * 0.05 && dist > radius).then_some((origin, radius))
}

// ---------------------------------------------------------------- vertex bevel

/// One face corner at a bevelled vertex: `face`, in-neighbour `a` (previous in the loop) and
/// out-neighbour `b` (next).
#[derive(Clone, Copy, Debug)]
struct Corner {
    face: usize,
    a: u32,
    b: u32,
}

/// The corners around `v` in walking order (each next corner shares the previous one's `a`
/// edge), and whether the fan closes. Refuses bow-tie / non-manifold fans and reflex corners.
fn vertex_fan(m: &BMesh, v: u32, faces: &[usize]) -> Result<(Vec<Corner>, bool), String> {
    let mut corners = Vec::new();
    for &f in faces {
        let lp = &m.faces[f].verts;
        let n = lp.len();
        let i = lp.iter().position(|x| *x == v).ok_or_else(|| NON_MANIFOLD.to_string())?;
        let c = Corner { face: f, a: lp[(i + n - 1) % n], b: lp[(i + 1) % n] };
        // Reflex (or flat-folded) corner: the cut would fold the face.
        let (pa, pv, pb) = (pos(m, c.a), pos(m, v), pos(m, c.b));
        let normal = newell(&m.face_points(f));
        if (pv - pa).cross(pb - pv).dot(normal) <= 1e-9 * normal.length() * (pv - pa).length() * (pb - pv).length() {
            return Err(JUNCTION.into());
        }
        corners.push(c);
    }
    let mut by_b: HashMap<u32, usize> = HashMap::new();
    let mut by_a: HashMap<u32, usize> = HashMap::new();
    for (i, c) in corners.iter().enumerate() {
        if by_b.insert(c.b, i).is_some() || by_a.insert(c.a, i).is_some() {
            return Err(NON_MANIFOLD.into());
        }
    }
    // Start at a corner nobody walks into (open fan), else at the first.
    let start = (0..corners.len()).find(|i| !by_a.contains_key(&corners[*i].b)).unwrap_or(0);
    let closed = (0..corners.len()).all(|i| by_a.contains_key(&corners[i].b));
    let mut order = vec![start];
    let mut cur = start;
    while let Some(&next) = by_b.get(&corners[cur].a) {
        if next == start {
            break;
        }
        if order.contains(&next) {
            return Err(NON_MANIFOLD.into());
        }
        order.push(next);
        cur = next;
    }
    if order.len() != corners.len() {
        return Err(NON_MANIFOLD.into());
    }
    if closed && corners.len() < 3 {
        return Err(JUNCTION.into());
    }
    Ok((order.into_iter().map(|i| corners[i]).collect(), closed))
}

/// Neighbours of `v` along its fan: each corner's `b`, then (open fans) the last corner's `a`.
fn fan_neighbours(fan: &[Corner], closed: bool) -> Vec<u32> {
    let mut out: Vec<u32> = fan.iter().map(|c| c.b).collect();
    if !closed && let Some(last) = fan.last() {
        out.push(last.a);
    }
    out
}

/// Distance along each edge from `v` that the amount type asks for.
fn vertex_offsets(m: &BMesh, v: u32, fan: &[Corner], neighbours: &[u32], opts: &BevelOptions) -> Result<BTreeMap<u32, f32>, String> {
    let pv = pos(m, v);
    let amount = opts.amount;
    let uniform = |d: f32| neighbours.iter().map(|n| (*n, d)).collect::<BTreeMap<u32, f32>>();
    Ok(match opts.amount_type {
        AmountType::Offset => uniform(amount),
        AmountType::Percent => neighbours.iter().map(|n| (*n, amount.min(100.0) / 100.0 * (pos(m, *n) - pv).length())).collect(),
        AmountType::Width => {
            // Cap side = chord 2·d·sin(α/2) across each face corner.
            let mean = fan
                .iter()
                .map(|c| {
                    let (u, w) = ((pos(m, c.a) - pv).normalized(), (pos(m, c.b) - pv).normalized());
                    (u.dot(w).clamp(-1.0, 1.0).acos() * 0.5).sin()
                })
                .sum::<f32>()
                / fan.len() as f32;
            if mean <= 0.001 {
                return Err(JUNCTION.into());
            }
            uniform(amount / (2.0 * mean))
        }
        AmountType::Depth => {
            // Depth of the cut below the vertex, along its (area-weighted) normal.
            let normal = fan.iter().fold(Vec3::ZERO, |s, c| s + newell(&m.face_points(c.face))).normalized();
            let mean = neighbours.iter().map(|n| -(pos(m, *n) - pv).normalized().dot(normal)).sum::<f32>() / neighbours.len() as f32;
            if mean <= 0.001 {
                return Err(JUNCTION.into());
            }
            uniform(amount / mean)
        }
    })
}

struct VertexPlan {
    fan: Vec<Corner>,
    closed: bool,
    /// Requested distance along the edge to each neighbour.
    requested: BTreeMap<u32, f32>,
}

/// Vertex bevel: every selected vertex is cut off by a new face across its edges at the
/// resolved amount (chamfer), or — with `segments > 1` — the face corners are rounded with
/// Xogot's profile curve and the cap becomes a domed patch of `segments / 2` rings around a hub
/// (`patch_center`). Border vertices get an open fan: their corners are cut and, with two or
/// more faces, a flat cap closes the cut. Reflex corners, bow-tie fans and two-face closed fans
/// are refused. On error the mesh is unchanged.
pub fn bevel_vertices(m: &mut BMesh, verts: &BTreeSet<u32>, opts: &BevelOptions) -> Result<BevelReport, String> {
    let opts = check_options(opts)?;
    let mut report = BevelReport { segments: opts.segments, ..Default::default() };
    let vert_faces = m.vert_faces();
    let mut plans: BTreeMap<u32, VertexPlan> = BTreeMap::new();
    for &v in verts {
        let Some(faces) = vert_faces.get(v as usize).filter(|f| !f.is_empty()) else {
            report.skipped_verts.insert(v);
            continue;
        };
        let (fan, closed) = vertex_fan(m, v, faces)?;
        let neighbours = fan_neighbours(&fan, closed);
        let requested = vertex_offsets(m, v, &fan, &neighbours, &opts)?;
        plans.insert(v, VertexPlan { fan, closed, requested });
    }
    if plans.is_empty() {
        return Err(SELECT_TO_BEVEL.into());
    }
    let mut retry_factor = 1.0f32;
    let mut retries = 0;
    for _round in 0..8 {
        // Clamp factors per vertex (same budget as the edge path).
        let mut scales: BTreeMap<u32, f32> = plans.keys().map(|v| (*v, 1.0)).collect();
        let mut may_overlap = false;
        for (&v, plan) in &plans {
            for (&n, &dv) in &plan.requested {
                let dn = plans.get(&n).and_then(|p| p.requested.get(&v)).copied().unwrap_or(0.0);
                if plans.contains_key(&n) && n < v {
                    continue; // each shared edge once
                }
                let length = (pos(m, n) - pos(m, v)).length();
                let budget = length_budget(length);
                let sum = dv + dn;
                if sum <= budget {
                    continue;
                }
                if opts.clamp_overlap {
                    let limit = if sum > 0.0 { budget / sum } else { 0.0 };
                    for (x, d) in [(v, dv), (n, dn)] {
                        if d > 0.0
                            && let Some(s) = scales.get_mut(&x)
                        {
                            *s = s.min(limit);
                        }
                    }
                } else if sum >= length * 0.999 {
                    return Err(OVERLAP.into());
                } else {
                    may_overlap = true;
                }
            }
        }
        let small: Vec<u32> = plans
            .iter()
            .filter(|(v, p)| p.requested.values().any(|d| d * scales[v] * retry_factor < BevelOptions::MINIMUM_AMOUNT))
            .map(|(v, _)| *v)
            .collect();
        if !small.is_empty() {
            for v in small {
                plans.remove(&v);
                report.skipped_verts.insert(v);
            }
            if plans.is_empty() {
                return Err(NO_ROOM.into());
            }
            continue;
        }
        let mut w = m.clone();
        let new_faces = build_vertex_bevel(&mut w, &plans, &scales, retry_factor, &opts);
        let valid = w.validate().is_ok() && (!m.is_closed() || w.is_closed());
        if !valid {
            if opts.clamp_overlap && retries < OVERLAP_RETRY_LIMIT {
                retries += 1;
                retry_factor *= 0.5;
                continue;
            }
            return Err(OVERLAP.into());
        }
        if opts.harden_normals {
            harden(&mut w, &new_faces);
        }
        accept(m, w)?;
        let factor = |v: &u32| scales[v] * retry_factor;
        let clamped: Vec<u32> = plans.keys().copied().filter(|v| factor(v) < 0.999).collect();
        report.clamped_corners = clamped.len();
        report.clamped_to = clamped.iter().map(factor).reduce(f32::min).map(|f| opts.amount * f);
        report.may_overlap = may_overlap;
        report.faces = new_faces;
        return Ok(report);
    }
    Err(OVERLAP.into())
}

/// Builds the vertex bevel into `m` (cut points, rounded corners, caps). Returns the new faces.
fn build_vertex_bevel(m: &mut BMesh, plans: &BTreeMap<u32, VertexPlan>, scales: &BTreeMap<u32, f32>, retry_factor: f32, opts: &BevelOptions) -> BTreeSet<usize> {
    let segments = opts.segments.max(1) as usize;
    // Cut point on edge (v, n).
    let mut cut: HashMap<(u32, u32), u32> = HashMap::new();
    for (&v, plan) in plans {
        let pv = pos(m, v);
        for (&n, &d) in &plan.requested {
            let d = d * scales[&v] * retry_factor;
            let dir = (pos(m, n) - pv).normalized();
            let idx = m.add_vert(pv + dir * d);
            cut.insert((v, n), idx);
        }
    }
    // Corner curve per (v, face): from the cut on the `a` edge to the cut on the `b` edge.
    let mut curves: HashMap<(u32, usize), Vec<u32>> = HashMap::new();
    let mut bulges: BTreeMap<u32, f32> = BTreeMap::new();
    for (&v, plan) in plans {
        let pv = pos(m, v);
        let mut bulge = 0.0;
        for c in &plan.fan {
            let (ia, ib) = (cut[&(v, c.a)], cut[&(v, c.b)]);
            let (pa, pb) = (pos(m, ia), pos(m, ib));
            let mut curve = vec![ia];
            if segments > 1 {
                for p in profile_curve(pa, pb, pv, opts.profile, segments) {
                    curve.push(m.add_vert(p));
                }
                bulge += profile_bulge(pa, pb, pv, opts.profile);
            }
            curve.push(ib);
            curves.insert((v, c.face), curve);
        }
        bulges.insert(v, bulge / plan.fan.len() as f32);
    }
    // Rebuild every touched face.
    let touched: BTreeSet<usize> = plans.values().flat_map(|p| p.fan.iter().map(|c| c.face)).collect();
    for &f in &touched {
        let lp = m.faces[f].verts.clone();
        let mut out = Vec::with_capacity(lp.len() + 4);
        for v in &lp {
            match curves.get(&(*v, f)) {
                Some(curve) => out.extend_from_slice(curve),
                None => out.push(*v),
            }
        }
        m.faces[f].verts = out;
    }
    // Caps.
    let first_new = m.faces.len();
    for (&v, plan) in plans {
        let k = plan.fan.len();
        if !plan.closed && k < 2 {
            continue; // a single border face: the cut corner is the whole bevel
        }
        let mut rim: Vec<u32> = Vec::new();
        for (i, c) in plan.fan.iter().enumerate() {
            let mut rev = curves[&(v, c.face)].clone();
            rev.reverse();
            let last = i + 1 == k;
            if last && !plan.closed {
                rim.extend(rev);
            } else {
                rim.extend_from_slice(&rev[..rev.len() - 1]);
            }
        }
        let template_face = plan.fan.iter().map(|c| c.face).min().unwrap_or(0);
        let template = m.faces[template_face].clone();
        if segments < 2 || !plan.closed {
            m.faces.push(template.with_verts(rim));
            continue;
        }
        // Domed patch: rings from the rim to a hub.
        let corner = pos(m, v);
        let rim_pts: Vec<Vec3> = rim.iter().map(|i| pos(m, *i)).collect();
        let centroid = rim_pts.iter().fold(Vec3::ZERO, |s, p| s + *p) / rim_pts.len() as f32;
        let sphere = patch_sphere(&rim_pts, corner);
        let hub = match sphere {
            Some((origin, radius)) => origin + (corner - origin).normalized() * radius,
            None => centroid + (corner - centroid) * bulges[&v],
        };
        let lift = hub - centroid;
        let ring_point = |b: Vec3, t: f32| -> Vec3 {
            match sphere {
                Some((origin, _)) => {
                    let (rb, rh) = (b - origin, hub - origin);
                    origin + rb.lerp(rh, t).normalized() * (rb.length() + (rh.length() - rb.length()) * t)
                }
                None => b + (centroid - b) * t + lift * (t * std::f32::consts::FRAC_PI_2).sin(),
            }
        };
        let rings = (segments / 2).max(1);
        let mut outer = rim.clone();
        for j in 1..rings {
            let t = j as f32 / rings as f32;
            let inner: Vec<u32> = rim_pts.iter().map(|b| ring_point(*b, t)).map(|p| m.add_vert(p)).collect();
            let n = outer.len();
            for i in 0..n {
                let quad = vec![outer[i], outer[(i + 1) % n], inner[(i + 1) % n], inner[i]];
                m.faces.push(template.with_verts(quad));
            }
            outer = inner;
        }
        let h = m.add_vert(hub);
        let n = outer.len();
        for i in 0..n {
            m.faces.push(template.with_verts(vec![outer[i], outer[(i + 1) % n], h]));
        }
    }
    m.compact();
    (first_new..m.faces.len()).collect()
}

// ---------------------------------------------------------------- Bevel Mode HUD helpers

/// Port of `formatted`: fixed fraction digits with `,` thousands grouping (en-US).
fn formatted(value: f32, digits: usize) -> String {
    let text = format!("{:.*}", digits, value as f64);
    let (sign, unsigned) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (integer, fraction) = unsigned.split_once('.').map_or((unsigned, None), |(i, f)| (i, Some(f)));
    let mut grouped = String::new();
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    match fraction {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Port of `BevelSession::format`: "12.5%" for percent, else three decimals.
pub fn format_amount(value: f32, amount_type: AmountType) -> String {
    if amount_type == AmountType::Percent { format!("{}%", formatted(value, 1)) } else { formatted(value, 3) }
}

/// Port of `BevelSession::summary`: "Bevel · 3 segments · Profile 0.50 · Offset 0.200".
pub fn summary(opts: &BevelOptions) -> String {
    [
        "Bevel".to_string(),
        format!("{} segments", opts.segments),
        format!("Profile {}", formatted(opts.profile, 2)),
        format!("{} {}", opts.amount_type.title(), format_amount(opts.amount, opts.amount_type)),
    ]
    .join(" \u{b7} ")
}

/// Port of `BevelSession::clamp_notice`: "Clamped to 0.150 at 4 corners".
pub fn clamp_notice(opts: &BevelOptions, report: &BevelReport) -> Option<String> {
    let to = report.clamped_to?;
    if report.clamped_corners == 0 || to >= opts.amount {
        return None;
    }
    Some(format!("Clamped to {} at {} corners", format_amount(to, opts.amount_type), report.clamped_corners))
}

/// Port of `BevelSession::omitted_notice`: "2 of 12 skipped".
pub fn skipped_notice(report: &BevelReport, target_count: usize) -> Option<String> {
    let n = report.skipped();
    (n > 0).then(|| format!("{n} of {target_count} skipped"))
}

/// The HUD status line: skipped, clamped or may-overlap notice (Xogot `status_line`, success
/// branch), else `None`.
pub fn status_line(opts: &BevelOptions, report: &BevelReport, target_count: usize) -> Option<String> {
    skipped_notice(report, target_count)
        .or_else(|| clamp_notice(opts, report))
        .or_else(|| report.may_overlap.then(|| "May overlap".to_string()))
}

/// Port of `BevelSession::preview_segments`: the most segments (down to 1) whose estimated new
/// face count stays within `COARSE_FACE_LIMIT`, for a quick live preview of `target_count`
/// selected edges / vertices.
pub fn preview_segments(opts: &BevelOptions, target_count: usize) -> u32 {
    let segments = opts.segments.clamp(1, BevelOptions::MAX_SEGMENTS) as i64;
    if segments < 2 {
        return segments as u32;
    }
    let targets = target_count as i64;
    let mut candidate = segments;
    loop {
        let estimate = if opts.affect == BevelAffect::Vertices { ((candidate * 6) | 1) * targets } else { targets * candidate + targets };
        if estimate <= COARSE_FACE_LIMIT {
            return candidate as u32;
        }
        if candidate <= 2 {
            return 1;
        }
        candidate -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::super::shapes::*;
    use super::*;

    fn cube() -> BMesh {
        cuboid(v3(1.0, 1.0, 1.0))
    }

    fn top_edge(m: &BMesh) -> Edge {
        let t = (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap();
        let f = &m.faces[t];
        edge(f.verts[0], f.verts[1])
    }

    fn opts(amount: f32, segments: u32) -> BevelOptions {
        BevelOptions { amount, segments, ..Default::default() }
    }

    /// Corner vertex with the largest x+y+z.
    fn top_corner(m: &BMesh) -> u32 {
        (0..m.positions.len() as u32).max_by(|a, b| {
            let s = |v: u32| pos(m, v).x + pos(m, v).y + pos(m, v).z;
            s(*a).partial_cmp(&s(*b)).unwrap()
        }).unwrap()
    }

    #[test]
    fn offset_matches_legacy_chamfer() {
        let mut a = cube();
        let e = top_edge(&a);
        let r = bevel_edges_ex(&mut a, &BTreeSet::from([e]), &opts(0.2, 1)).unwrap();
        a.validate().unwrap();
        assert!(a.is_closed());
        assert_eq!(r.faces.len(), 1);
        assert!((a.volume() - 0.98).abs() < 1e-4, "{}", a.volume());
        assert_eq!(r.clamped_corners, 0);
        assert!(r.clamped_to.is_none());
    }

    #[test]
    fn watertight_all_edges_rounded() {
        let mut c = cube();
        let all: BTreeSet<Edge> = c.edges().into_iter().collect();
        bevel_edges_ex(&mut c, &all, &opts(0.1, 3)).unwrap();
        c.validate().unwrap();
        assert!(c.is_closed());
        assert!(c.volume() < 1.0 && c.volume() > 0.97);
    }

    #[test]
    fn profile_changes_only_shape() {
        let run = |p: f32| {
            let mut m = cube();
            let e = top_edge(&m);
            let o = BevelOptions { profile: p, ..opts(0.3, 4) };
            let r = bevel_edges_ex(&mut m, &BTreeSet::from([e]), &o).unwrap();
            assert!(m.is_closed());
            (m.positions.len(), m.faces.len(), r.faces.len(), m.volume())
        };
        let (a, b, c) = (run(0.5), run(0.2), run(0.9));
        assert_eq!((a.0, a.1, a.2), (b.0, b.1, b.2));
        assert_eq!((a.0, a.1, a.2), (c.0, c.1, c.2));
        assert!((a.3 - b.3).abs() > 1e-4 && (a.3 - c.3).abs() > 1e-4, "{a:?} {b:?} {c:?}");
        // Circular profile removes about (1 - π/4)·r² of the unit-length edge.
        let expect = 1.0 - 0.09 * (1.0 - std::f32::consts::FRAC_PI_4);
        assert!((a.3 - expect).abs() < 4e-3, "{} vs {expect}", a.3);
    }

    #[test]
    fn percent_amount_uses_neighbour_edge() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        // A top edge along x (length 2): its neighbour edges are length 1 (y and z).
        let e = m.edges().into_iter().find(|(a, b)| {
            let (pa, pb) = (pos(&m, *a), pos(&m, *b));
            pa.y > 0.4 && pb.y > 0.4 && (pa - pb).x.abs() > 1.5
        }).unwrap();
        let o = BevelOptions { amount: 25.0, amount_type: AmountType::Percent, ..opts(0.0, 1) };
        bevel_edges_ex(&mut m, &BTreeSet::from([e]), &o).unwrap();
        assert!(m.is_closed());
        // 25% of 1 = 0.25 offset on a 2-long edge: removes 2·0.25²/2.
        assert!((m.volume() - (2.0 - 0.0625)).abs() < 1e-4, "{}", m.volume());
        // Percent above 100 is capped.
        assert_eq!(BevelOptions { amount: 150.0, ..o }.sanitized().amount, 100.0);
    }

    #[test]
    fn width_and_depth_on_right_angle() {
        let width = |t: AmountType| {
            let mut m = cube();
            let e = top_edge(&m);
            let r = bevel_edges_ex(&mut m, &BTreeSet::from([e]), &BevelOptions { amount: 0.2, amount_type: t, ..opts(0.0, 1) }).unwrap();
            let f = *r.faces.iter().next().unwrap();
            // Chamfer face width = its short side.
            let p = m.face_points(f);
            (0..p.len()).map(|i| (p[i] - p[(i + 1) % p.len()]).length()).fold(f32::MAX, f32::min)
        };
        assert!((width(AmountType::Width) - 0.2).abs() < 1e-4);
        // Depth 0.2 at 90° = offset 0.2/cos45 → width 0.2·√2/cos45 = 0.4.
        assert!((width(AmountType::Depth) - 0.4).abs() < 1e-4);
    }

    #[test]
    fn clamp_report_and_overlap_refusal() {
        let mut m = cube();
        let all: BTreeSet<Edge> = m.edges().into_iter().collect();
        let r = bevel_edges_ex(&mut m, &all, &opts(0.8, 1)).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.clamped_corners, 8);
        let to = r.clamped_to.unwrap();
        assert!((to - 0.475).abs() < 1e-3, "{to}");
        let note = clamp_notice(&opts(0.8, 1), &r).unwrap();
        assert_eq!(note, "Clamped to 0.475 at 8 corners");
        // The reported amount is the applied one: every original face is inset to a square of
        // side 1 - 2·to.
        let left = (1.0 - 2.0 * to) * (1.0 - 2.0 * to);
        let faces = (0..m.faces.len()).filter(|f| !r.faces.contains(f) && (m.face_area(*f) - left).abs() < 1e-4).count();
        assert_eq!(faces, 6);

        let mut n = cube();
        let before = n.clone();
        let o = BevelOptions { clamp_overlap: false, ..opts(0.8, 1) };
        let err = bevel_edges_ex(&mut n, &all, &o).unwrap_err();
        assert_eq!(err, OVERLAP);
        assert_eq!(n, before);
    }

    #[test]
    fn clamped_amount_matches_slide() {
        let mut m = cube();
        let e = top_edge(&m);
        let r = bevel_edges_ex(&mut m, &BTreeSet::from([e]), &opts(1.5, 1)).unwrap();
        assert!(m.is_closed());
        let to = r.clamped_to.unwrap();
        assert!((to - 0.95).abs() < 1e-3, "{to}");
        // Removed prism: to²/2 per unit length.
        assert!((m.volume() - (1.0 - to * to / 2.0)).abs() < 1e-3, "{}", m.volume());
    }

    #[test]
    fn harden_groups_new_faces() {
        let mut m = cube();
        let e = top_edge(&m);
        let r = bevel_edges_ex(&mut m, &BTreeSet::from([e]), &BevelOptions { harden_normals: true, ..opts(0.2, 3) }).unwrap();
        let g = m.faces[*r.faces.iter().next().unwrap()].smooth;
        assert!(g > 0);
        assert!(r.faces.iter().all(|f| m.faces[*f].smooth == g));
        assert!((0..m.faces.len()).filter(|f| !r.faces.contains(f)).all(|f| m.faces[f].smooth != g));
        let mut n = cube();
        let r = bevel_edges_ex(&mut n, &BTreeSet::from([e]), &opts(0.2, 3)).unwrap();
        assert!(r.faces.iter().all(|f| n.faces[*f].smooth == 0));
    }

    #[test]
    fn invalid_amount_and_empty_selection() {
        let mut m = cube();
        let before = m.clone();
        assert_eq!(bevel_edges_ex(&mut m, &BTreeSet::from([top_edge(&before)]), &opts(0.0, 1)).unwrap_err(), SET_AMOUNT);
        assert_eq!(bevel_edges_ex(&mut m, &BTreeSet::new(), &opts(0.2, 1)).unwrap_err(), SELECT_TO_BEVEL);
        assert_eq!(m, before);
    }

    #[test]
    fn vertex_chamfer_cube_corner() {
        let mut m = cube();
        let v = top_corner(&m);
        let o = BevelOptions { affect: BevelAffect::Vertices, ..opts(0.3, 1) };
        let r = bevel(&mut m, &BTreeSet::new(), &BTreeSet::from([v]), &o).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.faces.len(), 1);
        let f = *r.faces.iter().next().unwrap();
        assert_eq!(m.faces[f].verts.len(), 3);
        assert!(m.face_normal(f).dot(v3(1.0, 1.0, 1.0).normalized()) > 0.99);
        assert!((m.volume() - (1.0 - 0.027 / 6.0)).abs() < 1e-5, "{}", m.volume());
        assert_eq!(m.faces.len(), 7);
    }

    #[test]
    fn vertex_round_cube_corner() {
        for segments in [2, 3, 4] {
            let mut m = cube();
            let v = top_corner(&m);
            let o = BevelOptions { affect: BevelAffect::Vertices, ..opts(0.3, segments) };
            let r = bevel_vertices(&mut m, &BTreeSet::from([v]), &o).unwrap();
            m.validate().unwrap();
            assert!(m.is_closed(), "segments {segments}");
            assert!(r.faces.len() > 1);
            // Rounded removes less than the chamfer.
            assert!(m.volume() > 1.0 - 0.027 / 6.0 && m.volume() < 1.0, "{}", m.volume());
        }
    }

    #[test]
    fn vertex_bevel_all_corners_and_clamp() {
        let mut m = cube();
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        let o = BevelOptions { affect: BevelAffect::Vertices, ..opts(0.8, 1) };
        let r = bevel_vertices(&mut m, &all, &o).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.faces.len(), 8);
        assert_eq!(r.clamped_corners, 8);
        assert!((r.clamped_to.unwrap() - 0.475).abs() < 1e-3);
        let mut n = cube();
        let before = n.clone();
        assert_eq!(bevel_vertices(&mut n, &all, &BevelOptions { clamp_overlap: false, ..o }).unwrap_err(), OVERLAP);
        assert_eq!(n, before);
    }

    #[test]
    fn vertex_bevel_open_border() {
        // Plane 2x2 with 2x2 cells: a corner vertex (one face) and an edge-midpoint (two faces).
        let mut m = plane(v2(2.0, 2.0), 2);
        let vf = m.vert_faces();
        let corner = (0..m.positions.len()).find(|v| vf[*v].len() == 1).unwrap() as u32;
        let side = (0..m.positions.len()).find(|v| vf[*v].len() == 2).unwrap() as u32;
        let area = m.total_area();
        let o = BevelOptions { affect: BevelAffect::Vertices, ..opts(0.25, 1) };
        let r = bevel_vertices(&mut m, &BTreeSet::from([corner, side]), &o).unwrap();
        m.validate().unwrap();
        assert_eq!(r.faces.len(), 1, "border side vertex gets a cap, corner none");
        // The corner loses a 0.25² / 2 triangle, the side cut is re-covered by its cap.
        assert!((m.total_area() - (area - 0.03125)).abs() < 1e-4, "{}", m.total_area());
        assert!(m.faces.iter().all(|f| f.verts.len() >= 3));
    }

    #[test]
    fn vertex_bevel_refuses_reflex_corner() {
        // L-shaped flat polygon, its reflex vertex.
        let mut m = BMesh::default();
        let pts = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        let ids: Vec<u32> = pts.iter().map(|(x, z)| m.add_vert(v3(*x, 0.0, *z))).collect();
        m.add_face_facing(ids.clone(), v3(0.0, 1.0, 0.0));
        let before = m.clone();
        let o = BevelOptions { affect: BevelAffect::Vertices, ..opts(0.2, 1) };
        assert_eq!(bevel_vertices(&mut m, &BTreeSet::from([ids[3]]), &o).unwrap_err(), JUNCTION);
        assert_eq!(m, before);
    }

    #[test]
    fn hud_strings() {
        let o = BevelOptions { segments: 3, ..opts(0.2, 3) };
        assert_eq!(summary(&o), "Bevel \u{b7} 3 segments \u{b7} Profile 0.50 \u{b7} Offset 0.200");
        let p = BevelOptions { amount: 12.5, amount_type: AmountType::Percent, profile: 0.25, ..o };
        assert_eq!(summary(&p), "Bevel \u{b7} 3 segments \u{b7} Profile 0.25 \u{b7} Percent 12.5%");
        let r = BevelReport { skipped_edges: BTreeSet::from([(0, 1), (1, 2)]), ..Default::default() };
        assert_eq!(status_line(&o, &r, 12).unwrap(), "2 of 12 skipped");
        assert_eq!(formatted(1234.5, 1), "1,234.5");
    }

    #[test]
    fn coarse_preview_segments() {
        let e = BevelOptions { segments: 32, ..Default::default() };
        assert_eq!(preview_segments(&e, 10), 32);
        assert_eq!(preview_segments(&e, 1000), 4); // 1000·5 = 5000
        assert_eq!(preview_segments(&e, 3000), 1);
        let v = BevelOptions { affect: BevelAffect::Vertices, ..e };
        assert_eq!(preview_segments(&v, 100), 8); // (48|1)·100 = 4900
        assert_eq!(preview_segments(&BevelOptions { segments: 1, ..e }, 1_000_000), 1);
    }

    #[test]
    fn profile_weights_right_angle_is_circular() {
        let (a, b) = profile_weights(v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), Vec3::ZERO, 0.5);
        assert!((a - 0.552_284_8).abs() < 1e-4 && (a - b).abs() < 1e-6);
        let mid = profile_curve(v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), Vec3::ZERO, 0.5, 2)[0];
        // Circle of radius 1 around (1,1): midpoint at distance 1.
        assert!(((mid - v3(1.0, 1.0, 0.0)).length() - 1.0).abs() < 1e-3);
    }

    fn top_rim(m: &BMesh) -> BTreeSet<Edge> {
        let t = (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap();
        m.faces[t].loop_edges().map(|(a, b)| edge(a, b)).collect()
    }

    fn top_area(m: &BMesh) -> f32 {
        (0..m.faces.len()).filter(|f| m.face_normal(*f).y > 0.999).map(|f| m.face_area(f)).sum()
    }

    #[test]
    fn rim_bevel_insets_the_face() {
        for seg in [1u32, 3] {
            let mut m = cube();
            let rim = top_rim(&m);
            let r = bevel_edges_ex(&mut m, &rim, &opts(0.2, seg)).unwrap();
            m.validate().unwrap();
            assert!(m.is_closed());
            assert!((top_area(&m) - 0.36).abs() < 1e-4, "seg {seg}: {}", top_area(&m));
            assert!(r.faces.len() >= 4 * seg as usize, "seg {seg}: {}", r.faces.len());
            if seg == 1 {
                // A 45° chamfer of 0.2 around the rim: four prisms 0.02 each, minus the mitred
                // corner overlaps (four pyramids of 0.2³/3 counted twice).
                let expect = 1.0 - 4.0 * 0.02 + 4.0 * 0.008 / 3.0;
                assert!((m.volume() - expect).abs() < 1e-4, "{} vs {expect}", m.volume());
            }
        }
    }

    #[test]
    fn all_edges_segments_add_faces() {
        let count = |seg: u32| {
            let mut c = cube();
            let all: BTreeSet<Edge> = c.edges().into_iter().collect();
            let r = bevel_edges_ex(&mut c, &all, &opts(0.2, seg)).unwrap();
            c.validate().unwrap();
            assert!(c.is_closed());
            // Every original face is inset to a 0.6 square.
            let squares = (0..c.faces.len()).filter(|f| !r.faces.contains(f) && (c.face_area(*f) - 0.36).abs() < 1e-4).count();
            assert_eq!(squares, 6);
            (r.faces.len(), c.volume())
        };
        let (f1, v1) = count(1);
        let (f3, v3) = count(3);
        assert_eq!(f1, 12 + 8);
        assert!(f3 > f1 && v3 > v1, "{f1} {v1} {f3} {v3}");
    }

    #[test]
    fn rim_profile_changes_only_shape() {
        let run = |p: f32| {
            let mut m = cube();
            let rim = top_rim(&m);
            bevel_edges_ex(&mut m, &rim, &BevelOptions { profile: p, ..opts(0.2, 4) }).unwrap();
            assert!(m.is_closed());
            (m.positions.len(), m.faces.len(), m.volume())
        };
        let (a, b) = (run(0.5), run(0.15));
        assert_eq!((a.0, a.1), (b.0, b.1));
        assert!((a.2 - b.2).abs() > 1e-4);
    }

    #[test]
    fn rim_width_amount() {
        let mut m = cube();
        let rim = top_rim(&m);
        bevel_edges_ex(&mut m, &rim, &BevelOptions { amount: 0.2, amount_type: AmountType::Width, ..opts(0.0, 1) }).unwrap();
        // Width 0.2 across a 90° edge = offset 0.2/√2 on each face.
        let side = 1.0 - 2.0 * 0.2 / 2f32.sqrt();
        assert!((top_area(&m) - side * side).abs() < 1e-4, "{}", top_area(&m));
    }

    fn vopts(amount: f32, t: AmountType, segments: u32) -> BevelOptions {
        BevelOptions { affect: BevelAffect::Vertices, amount, amount_type: t, ..opts(0.0, segments) }
    }

    /// The single cap of a cube-corner chamfer, as its points.
    fn corner_cap(o: &BevelOptions, size: Vec3) -> (Vec3, Vec<Vec3>) {
        let mut m = cuboid(size);
        let v = top_corner(&m);
        let corner = pos(&m, v);
        let r = bevel_vertices(&mut m, &BTreeSet::from([v]), o).unwrap();
        assert!(m.is_closed());
        let f = *r.faces.iter().next().unwrap();
        (corner, m.face_points(f))
    }

    #[test]
    fn vertex_amount_types() {
        // Width: cap side = amount.
        let (_, p) = corner_cap(&vopts(0.3, AmountType::Width, 1), v3(1.0, 1.0, 1.0));
        assert!(((p[0] - p[1]).length() - 0.3).abs() < 1e-4);
        // Depth: cap plane that far below the corner.
        let (corner, p) = corner_cap(&vopts(0.1, AmountType::Depth, 1), v3(1.0, 1.0, 1.0));
        let n = v3(1.0, 1.0, 1.0).normalized();
        assert!(((corner - p[0]).dot(n) - 0.1).abs() < 1e-4);
        // Percent: of each edge, so a 2x1x1 box cuts twice as far along x.
        let (c2, p) = corner_cap(&vopts(20.0, AmountType::Percent, 1), v3(2.0, 1.0, 1.0));
        let mut d: Vec<f32> = p.iter().map(|q| (c2 - *q).length()).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((d[0] - 0.2).abs() < 1e-4 && (d[1] - 0.2).abs() < 1e-4 && (d[2] - 0.4).abs() < 1e-4, "{d:?}");
    }

    #[test]
    fn vertex_profile_and_harden() {
        let run = |p: f32, harden: bool| {
            let mut m = cube();
            let v = top_corner(&m);
            let o = BevelOptions { profile: p, harden_normals: harden, ..vopts(0.3, AmountType::Offset, 4) };
            let r = bevel_vertices(&mut m, &BTreeSet::from([v]), &o).unwrap();
            assert!(m.is_closed());
            let groups: BTreeSet<u16> = r.faces.iter().map(|f| m.faces[*f].smooth).collect();
            (m.positions.len(), m.faces.len(), m.volume(), groups)
        };
        let (a, b) = (run(0.5, false), run(0.2, false));
        assert_eq!((a.0, a.1), (b.0, b.1));
        assert!((a.2 - b.2).abs() > 1e-5);
        assert_eq!(a.3, BTreeSet::from([0]));
        assert_eq!(run(0.5, true).3, BTreeSet::from([1]));
    }
}
