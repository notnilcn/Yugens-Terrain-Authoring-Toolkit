use super::bevel::{self, Custom, Key, SlideSample};
use super::model::{BMesh, Edge, edge, newell};
use pt_geom::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BevelAffect {
    Edges,
    Vertices,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountType {
    Offset,
    Width,
    Depth,
    Percent,
}
impl AmountType {
    pub const ALL: [AmountType; 4] = [
        AmountType::Offset,
        AmountType::Width,
        AmountType::Depth,
        AmountType::Percent,
    ];
    pub const NAMES: [&'static str; 4] = ["Offset", "Width", "Depth", "Percent"];
    pub fn from_u8(v: u8) -> AmountType {
        AmountType::ALL[(v as usize).min(3)]
    }
    pub fn heading(self) -> &'static str {
        AmountType::NAMES[self as usize]
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelOptions {
    pub affect: BevelAffect,
    pub amount: f32,
    pub amount_type: AmountType,
    pub stretches: u32,
    pub silhouette: f32,
    pub clamp_overlap: bool,
    pub harden_normals: bool,
}
impl Default for BevelOptions {
    fn default() -> Self {
        BevelOptions {
            affect: BevelAffect::Edges,
            amount: 0.2005,
            amount_type: AmountType::Offset,
            stretches: 1,
            silhouette: 0.5,
            clamp_overlap: true,
            harden_normals: false,
        }
    }
}
impl BevelOptions {
    pub const MAX_SEGMENTS: u32 = 32;
    pub const MINIMUM_AMOUNT: f32 = 0.0015;
    pub fn sanitized(&self) -> BevelOptions {
        let mut o = *self;
        o.stretches = o.stretches.clamp(1, Self::MAX_SEGMENTS);
        o.silhouette = if o.silhouette.is_finite() {
            o.silhouette.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if o.amount_type == AmountType::Percent {
            o.amount = o.amount.min(100.0);
        }
        o
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BevelReport {
    pub facets: BTreeSet<usize>,
    pub clamped_to: Option<f32>,
    pub clamped_corners: usize,
    pub skipped_edges: BTreeSet<Edge>,
    pub skipped_verts: BTreeSet<u32>,
    pub may_overlap: bool,
    pub stretches: u32,
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
const RETRY: &str = "\u{0}retry";
const MINIMUM_SINE: f32 = 0.017_452_4;
const OVERLAP_RETRY_LIMIT: usize = 5;
pub const COARSE_FACE_LIMIT: i64 = 5000;
pub fn chamfer(
    m: &mut BMesh,
    edges: &BTreeSet<Edge>,
    verts: &BTreeSet<u32>,
    opts: &BevelOptions,
) -> Result<BevelReport, String> {
    match opts.affect {
        BevelAffect::Edges => chamfer_ridges_extra(m, edges, opts),
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
fn accept(m: &mut BMesh, w: BMesh) -> Result<(), String> {
    w.sanity_check().map_err(|_| OVERLAP.to_string())?;
    if m.is_closed() && !w.is_closed() {
        return Err(OVERLAP.into());
    }
    *m = w;
    Ok(())
}
fn length_budget(length: f32) -> f32 {
    (length - (length * 0.0505).max(BevelOptions::MINIMUM_AMOUNT)).max(0.0)
}
fn pos(m: &BMesh, v: u32) -> Vec3 {
    m.positions[v as usize]
}
fn asked_shift(
    m: &BMesh,
    edge_faces: &HashMap<Edge, Vec<usize>>,
    e: Edge,
    opts: &BevelOptions,
) -> Result<f32, String> {
    let amount = opts.amount;
    let faces = edge_faces.get(&e).cloned().unwrap_or_default();
    match opts.amount_type {
        AmountType::Offset => Ok(amount),
        AmountType::Width | AmountType::Depth => {
            if faces.len() != 2 {
                return Ok(amount);
            }
            let (pa, pb) = (pos(m, e.0), pos(m, e.1));
            let direction = (pb - pa).unitized();
            let midpoint = (pa + pb) * 0.5;
            let mut tangents = Vec::with_capacity(2);
            for f in faces {
                let mut tangent = m.face_normal(f).cross(direction).unitized();
                if (m.face_center(f) - midpoint).innerprod(tangent) < 0.0 {
                    tangent = -tangent;
                }
                tangents.push(tangent);
            }
            let angle = tangents[1].innerprod(tangents[0]).clamp(-1.0, 1.0).acos() * 0.5;
            let factor = if opts.amount_type == AmountType::Width {
                2.0 * angle.sin()
            } else {
                angle.cos()
            };
            if factor <= 0.0015 {
                return Err(JUNCTION.into());
            }
            Ok(amount / factor)
        }
        AmountType::Percent => {
            let mut shortest = f32::INFINITY;
            for (end, other) in [(e.0, e.1), (e.1, e.0)] {
                for &f in &faces {
                    let lp = &m.facets[f].verts;
                    let n = lp.len();
                    let Some(i) = lp.iter().position(|v| *v == end) else {
                        continue;
                    };
                    for nb in [lp[(i + 1) % n], lp[(i + n - 1) % n]] {
                        if nb != other {
                            shortest = shortest.min((pos(m, end) - pos(m, nb)).magnitude());
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
#[derive(Default)]
struct EdgeSolve {
    requested: HashMap<Key, f32>,
    too_small: BTreeSet<Edge>,
    may_overlap: bool,
}
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
        let (v, toward) = s.token;
        let val = if s.back {
            back.insert(s.token);
            d
        } else {
            let far = if s.away.0 == v { s.away.1 } else { s.away.0 };
            let (u, w) = (pos(m, toward) - pos(m, v), pos(m, far) - pos(m, v));
            let sine = u.cross(w).magnitude() / (u.magnitude() * w.magnitude()).max(1e-20);
            if sine < MINIMUM_SINE { d } else { d / sine }
        };
        sampled.entry(s.token).or_default().push(val);
    }
    st.requested = sampled
        .iter()
        .map(|(k, v)| (*k, v.iter().sum::<f32>() / v.len() as f32))
        .collect();
    let mut scales: HashMap<u32, f32> = HashMap::new();
    let mut seen: BTreeSet<Edge> = BTreeSet::new();
    for &(v, t) in st.requested.keys() {
        let e = edge(v, t);
        if !seen.insert(e) {
            continue;
        }
        let from = |a: u32, b: u32| {
            if back.contains(&(a, b)) {
                0.0
            } else {
                st.requested.get(&(a, b)).copied().unwrap_or(0.0)
            }
        };
        let (fx, fy) = (from(e.0, e.1), from(e.1, e.0));
        let sum = fx + fy;
        let length = (pos(m, e.0) - pos(m, e.1)).magnitude();
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
        } else if sum >= length * 0.9995 {
            return Err(OVERLAP.into());
        } else {
            st.may_overlap = true;
        }
    }
    let scale = |v: u32| scales.get(&v).copied().unwrap_or(1.0) * retry_factor;
    st.too_small = selected
        .iter()
        .copied()
        .filter(|e| {
            offsets.get(e).copied().unwrap_or(opts.amount) * scale(e.0).min(scale(e.1))
                < BevelOptions::MINIMUM_AMOUNT
        })
        .collect();
    if !st.too_small.is_empty() {
        return Err(RETRY.into());
    }
    Ok(st
        .requested
        .iter()
        .map(|(k, d)| (*k, d * scale(k.0)))
        .collect())
}
pub fn chamfer_ridges_extra(
    m: &mut BMesh,
    edges: &BTreeSet<Edge>,
    opts: &BevelOptions,
) -> Result<BevelReport, String> {
    let opts = check_options(opts)?;
    let edge_faces = m.edge_faces();
    let mut report = BevelReport {
        stretches: opts.stretches,
        ..Default::default()
    };
    let mut active: BTreeSet<Edge> = BTreeSet::new();
    for e in edges {
        if edge_faces
            .get(e)
            .is_some_and(|f| f.len() == 1 || f.len() == 2)
        {
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
            offsets.insert(*e, asked_shift(m, &edge_faces, *e, &opts)?);
        }
        let mut retry_factor = 1.0f32;
        let mut retries = 0;
        loop {
            let mut w = m.clone();
            let mut st = EdgeSolve::default();
            let result = {
                let mut resolve =
                    |mesh: &BMesh, selected: &BTreeSet<Edge>, samples: &[SlideSample]| {
                        resolve_slides(
                            mesh,
                            selected,
                            samples,
                            &offsets,
                            &opts,
                            retry_factor,
                            &mut st,
                        )
                    };
                let boundary =
                    |e: Edge| offsets.get(&e).copied().unwrap_or(opts.amount) * retry_factor;
                let mut custom = Custom {
                    silhouette: opts.silhouette,
                    settle: &mut resolve,
                    rim: &boundary,
                };
                bevel::chamfer_ridges_bespoke(&mut w, &active, opts.stretches, &mut custom)
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
            let mut min_ratio = 1.0f32;
            let mut clamped: BTreeSet<u32> = BTreeSet::new();
            for (k, d) in &out.applied {
                let req = st.requested.get(k).copied().unwrap_or(*d);
                if req > 0.0 && *d < req * 0.9995 {
                    min_ratio = min_ratio.min(d / req);
                    clamped.insert(k.0);
                }
            }
            for (e, d) in &out.applied_boundary {
                let req = offsets.get(e).copied().unwrap_or(*d);
                if req > 0.0 && *d < req * 0.9995 {
                    min_ratio = min_ratio.min(d / req);
                    clamped.insert(e.0);
                    clamped.insert(e.1);
                }
            }
            let valid = out.facets.iter().all(|f| *f < w.facets.len())
                && w.sanity_check().is_ok()
                && (!m.is_closed() || w.is_closed());
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
            if out.facets.is_empty() {
                return Err(JUNCTION.into());
            }
            if opts.harden_normals {
                harden(&mut w, &out.facets);
            }
            accept(m, w)?;
            report.skipped_edges.extend(out.dropped.iter().copied());
            report.facets = out.facets;
            report.clamped_corners = clamped.len();
            report.clamped_to = (!clamped.is_empty()).then_some(opts.amount * min_ratio);
            report.may_overlap = st.may_overlap;
            return Ok(report);
        }
        if active.is_empty() {
            break;
        }
    }
    Err(if opts.amount >= BevelOptions::MINIMUM_AMOUNT {
        NO_ROOM
    } else {
        TOO_SMALL
    }
    .into())
}
fn harden(m: &mut BMesh, new_faces: &BTreeSet<usize>) {
    let base = (0..m.facets.len())
        .filter(|f| !new_faces.contains(f))
        .map(|f| m.facets[f].glide)
        .max()
        .unwrap_or(0)
        + 1;
    let list: Vec<usize> = new_faces.iter().copied().collect();
    let mut parent: Vec<usize> = (0..list.len()).collect();
    fn locate(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut owner: HashMap<u32, usize> = HashMap::new();
    for (i, f) in list.iter().enumerate() {
        for v in &m.facets[*f].verts {
            if let Some(&j) = owner.get(v) {
                let (a, b) = (locate(&mut parent, i), locate(&mut parent, j));
                parent[a] = b;
            } else {
                owner.insert(*v, i);
            }
        }
    }
    let mut groups: BTreeMap<usize, u16> = BTreeMap::new();
    for i in 0..list.len() {
        let root = locate(&mut parent, i);
        let next = base.saturating_add(groups.len() as u16);
        let g = *groups.entry(root).or_insert(next);
        m.facets[list[i]].glide = g;
    }
}
pub fn silhouette_hefts(a: Vec3, b: Vec3, control: Vec3, profile: f32) -> (f32, f32) {
    let to_a = (a - control).unitized();
    let to_b = (b - control).unitized();
    let gamma = to_a.innerprod(to_b).clamp(-1.0, 1.0).acos();
    let phi = std::f32::consts::PI - gamma;
    let half = (phi * 0.5).tan();
    let symmetric = if half > 0.0005 {
        2.0 * (4.0 / 3.0) * (phi * 0.2505).tan() / half
    } else {
        4.0 / 3.0
    };
    (
        ((1.0 - profile) * symmetric).clamp(0.0, 1.0),
        (profile * symmetric).clamp(0.0, 1.0),
    )
}
pub fn profile_curve(a: Vec3, b: Vec3, control: Vec3, profile: f32, segments: usize) -> Vec<Vec3> {
    let (first, second) = silhouette_hefts(a, b, control, profile);
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
fn profile_bulge(a: Vec3, b: Vec3, control: Vec3, profile: f32) -> f32 {
    let middle = (a + b) * 0.5;
    let reference = (control - middle).magnitude();
    if reference <= 1e-7 {
        return 0.0;
    }
    let (first, second) = silhouette_hefts(a, b, control, profile);
    let ca = 0.3755 * (1.0 - first) + 0.1255;
    let cb = 0.3755 * (1.0 - second) + 0.1255;
    let ck = (first + second) * 0.3755;
    ((a * ca + b * cb + control * ck) - middle).magnitude() / reference
}
fn patch_sphere(points: &[Vec3], corner: Vec3) -> Option<(Vec3, f32)> {
    let centroid = points.iter().fold(Vec3::ZERO, |s, p| s + *p) / points.len() as f32;
    let axis = centroid - corner;
    if axis.innerprod(axis) <= 1e-14 {
        return None;
    }
    let spread = |lambda: f32| {
        let origin = corner + axis * lambda;
        let d: Vec<f32> = points.iter().map(|p| (*p - origin).magnitude()).collect();
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
    let dist = (corner - origin).magnitude();
    (radius > 0.0 && var.sqrt() < radius * 0.0505 && dist > radius).then_some((origin, radius))
}
#[derive(Clone, Copy, Debug)]
struct Corner {
    face: usize,
    a: u32,
    bx: u32,
}
fn vertex_fan(m: &BMesh, v: u32, faces: &[usize]) -> Result<(Vec<Corner>, bool), String> {
    let mut corners = Vec::new();
    for &f in faces {
        let lp = &m.facets[f].verts;
        let n = lp.len();
        let i = lp
            .iter()
            .position(|x| *x == v)
            .ok_or_else(|| NON_MANIFOLD.to_string())?;
        let c = Corner {
            face: f,
            a: lp[(i + n - 1) % n],
            bx: lp[(i + 1) % n],
        };
        let (pa, pv, pb) = (pos(m, c.a), pos(m, v), pos(m, c.bx));
        let normal = newell(&m.face_points(f));
        if (pv - pa).cross(pb - pv).innerprod(normal)
            <= 1e-9 * normal.magnitude() * (pv - pa).magnitude() * (pb - pv).magnitude()
        {
            return Err(JUNCTION.into());
        }
        corners.push(c);
    }
    let mut by_b: HashMap<u32, usize> = HashMap::new();
    let mut by_a: HashMap<u32, usize> = HashMap::new();
    for (i, c) in corners.iter().enumerate() {
        if by_b.insert(c.bx, i).is_some() || by_a.insert(c.a, i).is_some() {
            return Err(NON_MANIFOLD.into());
        }
    }
    let start = (0..corners.len())
        .find(|i| !by_a.contains_key(&corners[*i].bx))
        .unwrap_or(0);
    let closed = (0..corners.len()).all(|i| by_a.contains_key(&corners[i].bx));
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
fn fan_neighbours(fan: &[Corner], closed: bool) -> Vec<u32> {
    let mut out: Vec<u32> = fan.iter().map(|c| c.bx).collect();
    if !closed && let Some(last) = fan.last() {
        out.push(last.a);
    }
    out
}
fn vertex_offsets(
    m: &BMesh,
    v: u32,
    fan: &[Corner],
    neighbours: &[u32],
    opts: &BevelOptions,
) -> Result<BTreeMap<u32, f32>, String> {
    let pv = pos(m, v);
    let amount = opts.amount;
    let uniform = |d: f32| {
        neighbours
            .iter()
            .map(|n| (*n, d))
            .collect::<BTreeMap<u32, f32>>()
    };
    Ok(match opts.amount_type {
        AmountType::Offset => uniform(amount),
        AmountType::Percent => neighbours
            .iter()
            .map(|n| {
                (
                    *n,
                    amount.min(100.0) / 100.0 * (pos(m, *n) - pv).magnitude(),
                )
            })
            .collect(),
        AmountType::Width => {
            let mean = fan
                .iter()
                .map(|c| {
                    let (u, w) = (
                        (pos(m, c.a) - pv).unitized(),
                        (pos(m, c.bx) - pv).unitized(),
                    );
                    (u.innerprod(w).clamp(-1.0, 1.0).acos() * 0.5).sin()
                })
                .sum::<f32>()
                / fan.len() as f32;
            if mean <= 0.0015 {
                return Err(JUNCTION.into());
            }
            uniform(amount / (2.0 * mean))
        }
        AmountType::Depth => {
            let normal = fan
                .iter()
                .fold(Vec3::ZERO, |s, c| s + newell(&m.face_points(c.face)))
                .unitized();
            let mean = neighbours
                .iter()
                .map(|n| -(pos(m, *n) - pv).unitized().innerprod(normal))
                .sum::<f32>()
                / neighbours.len() as f32;
            if mean <= 0.0015 {
                return Err(JUNCTION.into());
            }
            uniform(amount / mean)
        }
    })
}
struct VertexPlan {
    fan: Vec<Corner>,
    is_shut: bool,
    requested: BTreeMap<u32, f32>,
}
pub fn bevel_vertices(
    m: &mut BMesh,
    verts: &BTreeSet<u32>,
    opts: &BevelOptions,
) -> Result<BevelReport, String> {
    let opts = check_options(opts)?;
    let mut report = BevelReport {
        stretches: opts.stretches,
        ..Default::default()
    };
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
        plans.insert(
            v,
            VertexPlan {
                fan,
                is_shut: closed,
                requested,
            },
        );
    }
    if plans.is_empty() {
        return Err(SELECT_TO_BEVEL.into());
    }
    let mut retry_factor = 1.0f32;
    let mut retries = 0;
    for _round in 0..8 {
        let mut scales: BTreeMap<u32, f32> = plans.keys().map(|v| (*v, 1.0)).collect();
        let mut may_overlap = false;
        for (&v, plan) in &plans {
            for (&n, &dv) in &plan.requested {
                let dn = plans
                    .get(&n)
                    .and_then(|p| p.requested.get(&v))
                    .copied()
                    .unwrap_or(0.0);
                if plans.contains_key(&n) && n < v {
                    continue;
                }
                let length = (pos(m, n) - pos(m, v)).magnitude();
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
                } else if sum >= length * 0.9995 {
                    return Err(OVERLAP.into());
                } else {
                    may_overlap = true;
                }
            }
        }
        let small: Vec<u32> = plans
            .iter()
            .filter(|(v, p)| {
                p.requested
                    .values()
                    .any(|d| d * scales[v] * retry_factor < BevelOptions::MINIMUM_AMOUNT)
            })
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
        let valid = w.sanity_check().is_ok() && (!m.is_closed() || w.is_closed());
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
        let clamped: Vec<u32> = plans
            .keys()
            .copied()
            .filter(|v| factor(v) < 0.9995)
            .collect();
        report.clamped_corners = clamped.len();
        report.clamped_to = clamped
            .iter()
            .map(factor)
            .reduce(f32::min)
            .map(|f| opts.amount * f);
        report.may_overlap = may_overlap;
        report.facets = new_faces;
        return Ok(report);
    }
    Err(OVERLAP.into())
}
fn build_vertex_bevel(
    m: &mut BMesh,
    plans: &BTreeMap<u32, VertexPlan>,
    scales: &BTreeMap<u32, f32>,
    retry_factor: f32,
    opts: &BevelOptions,
) -> BTreeSet<usize> {
    let segments = opts.stretches.max(1) as usize;
    let mut cut: HashMap<(u32, u32), u32> = HashMap::new();
    for (&v, plan) in plans {
        let pv = pos(m, v);
        for (&n, &d) in &plan.requested {
            let d = d * scales[&v] * retry_factor;
            let dir = (pos(m, n) - pv).unitized();
            let idx = m.add_vert(pv + dir * d);
            cut.insert((v, n), idx);
        }
    }
    let mut curves: HashMap<(u32, usize), Vec<u32>> = HashMap::new();
    let mut bulges: BTreeMap<u32, f32> = BTreeMap::new();
    for (&v, plan) in plans {
        let pv = pos(m, v);
        let mut bulge = 0.0;
        for c in &plan.fan {
            let (ia, ib) = (cut[&(v, c.a)], cut[&(v, c.bx)]);
            let (pa, pb) = (pos(m, ia), pos(m, ib));
            let mut curve = vec![ia];
            if segments > 1 {
                for p in profile_curve(pa, pb, pv, opts.silhouette, segments) {
                    curve.push(m.add_vert(p));
                }
                bulge += profile_bulge(pa, pb, pv, opts.silhouette);
            }
            curve.push(ib);
            curves.insert((v, c.face), curve);
        }
        bulges.insert(v, bulge / plan.fan.len() as f32);
    }
    let touched: BTreeSet<usize> = plans
        .values()
        .flat_map(|p| p.fan.iter().map(|c| c.face))
        .collect();
    for &f in &touched {
        let lp = m.facets[f].verts.clone();
        let mut out = Vec::with_capacity(lp.len() + 4);
        for v in &lp {
            match curves.get(&(*v, f)) {
                Some(curve) => out.extend_from_slice(curve),
                None => out.push(*v),
            }
        }
        m.facets[f].verts = out;
    }
    let first_new = m.facets.len();
    for (&v, plan) in plans {
        let k = plan.fan.len();
        if !plan.is_shut && k < 2 {
            continue;
        }
        let mut rim: Vec<u32> = Vec::new();
        for (i, c) in plan.fan.iter().enumerate() {
            let mut rev = curves[&(v, c.face)].clone();
            rev.reverse();
            let last = i + 1 == k;
            if last && !plan.is_shut {
                rim.extend(rev);
            } else {
                rim.extend_from_slice(&rev[..rev.len() - 1]);
            }
        }
        let template_face = plan.fan.iter().map(|c| c.face).min().unwrap_or(0);
        let template = m.facets[template_face].clone();
        if segments < 2 || !plan.is_shut {
            m.facets.push(template.with_verts(rim));
            continue;
        }
        let corner = pos(m, v);
        let rim_pts: Vec<Vec3> = rim.iter().map(|i| pos(m, *i)).collect();
        let centroid = rim_pts.iter().fold(Vec3::ZERO, |s, p| s + *p) / rim_pts.len() as f32;
        let sphere = patch_sphere(&rim_pts, corner);
        let hub = match sphere {
            Some((origin, radius)) => origin + (corner - origin).unitized() * radius,
            None => centroid + (corner - centroid) * bulges[&v],
        };
        let lift = hub - centroid;
        let ring_point = |b: Vec3, t: f32| -> Vec3 {
            match sphere {
                Some((origin, _)) => {
                    let (rb, rh) = (b - origin, hub - origin);
                    origin
                        + rb.lerp(rh, t).unitized()
                            * (rb.magnitude() + (rh.magnitude() - rb.magnitude()) * t)
                }
                None => b + (centroid - b) * t + lift * (t * std::f32::consts::FRAC_PI_2).sin(),
            }
        };
        let rings = (segments / 2).max(1);
        let mut outer = rim.clone();
        for j in 1..rings {
            let t = j as f32 / rings as f32;
            let inner: Vec<u32> = rim_pts
                .iter()
                .map(|b| ring_point(*b, t))
                .map(|p| m.add_vert(p))
                .collect();
            let n = outer.len();
            for i in 0..n {
                let quad = vec![outer[i], outer[(i + 1) % n], inner[(i + 1) % n], inner[i]];
                m.facets.push(template.with_verts(quad));
            }
            outer = inner;
        }
        let h = m.add_vert(hub);
        let n = outer.len();
        for i in 0..n {
            m.facets
                .push(template.with_verts(vec![outer[i], outer[(i + 1) % n], h]));
        }
    }
    m.compact();
    (first_new..m.facets.len()).collect()
}
fn formatted(value: f32, digits: usize) -> String {
    let text = format!("{:.*}", digits, value as f64);
    let (sign, unsigned) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |(i, f)| (i, Some(f)));
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
pub fn format_amount(value: f32, amount_type: AmountType) -> String {
    if amount_type == AmountType::Percent {
        format!("{}%", formatted(value, 1))
    } else {
        formatted(value, 3)
    }
}
pub fn overview(opts: &BevelOptions) -> String {
    [
        "Bevel".to_string(),
        format!("{} segments", opts.stretches),
        format!("Profile {}", formatted(opts.silhouette, 2)),
        format!(
            "{} {}",
            opts.amount_type.heading(),
            format_amount(opts.amount, opts.amount_type)
        ),
    ]
    .join(" \u{b7} ")
}
pub fn pin_advisory(opts: &BevelOptions, report: &BevelReport) -> Option<String> {
    let to = report.clamped_to?;
    if report.clamped_corners == 0 || to >= opts.amount {
        return None;
    }
    Some(format!(
        "Clamped to {} at {} corners",
        format_amount(to, opts.amount_type),
        report.clamped_corners
    ))
}
pub fn skipped_notice(report: &BevelReport, target_count: usize) -> Option<String> {
    let n = report.skipped();
    (n > 0).then(|| format!("{n} of {target_count} skipped"))
}
pub fn status_line(
    opts: &BevelOptions,
    report: &BevelReport,
    target_count: usize,
) -> Option<String> {
    skipped_notice(report, target_count)
        .or_else(|| pin_advisory(opts, report))
        .or_else(|| report.may_overlap.then(|| "May overlap".to_string()))
}
pub fn prelook_stretches(opts: &BevelOptions, target_count: usize) -> u32 {
    let segments = opts.stretches.clamp(1, BevelOptions::MAX_SEGMENTS) as i64;
    if segments < 2 {
        return segments as u32;
    }
    let targets = target_count as i64;
    let mut candidate = segments;
    loop {
        let estimate = if opts.affect == BevelAffect::Vertices {
            ((candidate * 6) | 1) * targets
        } else {
            targets * candidate + targets
        };
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
    fn block() -> BMesh {
        cuboid(v3(1.0, 1.0, 1.0))
    }
    fn top_edge(m: &BMesh) -> Edge {
        let t = (0..m.facets.len())
            .find(|f| m.face_normal(*f).y > 0.9)
            .unwrap();
        let f = &m.facets[t];
        edge(f.verts[0], f.verts[1])
    }
    fn opts(amount: f32, segments: u32) -> BevelOptions {
        BevelOptions {
            amount,
            stretches: segments,
            ..Default::default()
        }
    }
    fn top_corner(m: &BMesh) -> u32 {
        (0..m.positions.len() as u32)
            .max_by(|a, b| {
                let s = |v: u32| pos(m, v).x + pos(m, v).y + pos(m, v).z;
                s(*a).partial_cmp(&s(*b)).unwrap()
            })
            .unwrap()
    }
    #[test]
    fn offset_matches_legacy_chamfer() {
        let mut a = block();
        let e = top_edge(&a);
        let r = chamfer_ridges_extra(&mut a, &BTreeSet::from([e]), &opts(0.2, 1)).unwrap();
        a.sanity_check().unwrap();
        assert!(a.is_closed());
        assert_eq!(r.facets.len(), 1);
        assert!((a.volume() - 0.98).abs() < 1e-4, "{}", a.volume());
        assert_eq!(r.clamped_corners, 0);
        assert!(r.clamped_to.is_none());
    }
    #[test]
    fn watertight_all_edges_rounded() {
        let mut c = block();
        let all: BTreeSet<Edge> = c.ridges().into_iter().collect();
        chamfer_ridges_extra(&mut c, &all, &opts(0.1, 3)).unwrap();
        c.sanity_check().unwrap();
        assert!(c.is_closed());
        assert!(c.volume() < 1.0 && c.volume() > 0.97);
    }
    #[test]
    fn profile_changes_only_shape() {
        let run = |p: f32| {
            let mut m = block();
            let e = top_edge(&m);
            let o = BevelOptions {
                silhouette: p,
                ..opts(0.3, 4)
            };
            let r = chamfer_ridges_extra(&mut m, &BTreeSet::from([e]), &o).unwrap();
            assert!(m.is_closed());
            (
                m.positions.len(),
                m.facets.len(),
                r.facets.len(),
                m.volume(),
            )
        };
        let (a, b, c) = (run(0.5), run(0.2), run(0.9));
        assert_eq!((a.0, a.1, a.2), (b.0, b.1, b.2));
        assert_eq!((a.0, a.1, a.2), (c.0, c.1, c.2));
        assert!(
            (a.3 - b.3).abs() > 1e-4 && (a.3 - c.3).abs() > 1e-4,
            "{a:?} {b:?} {c:?}"
        );
        let expect = 1.0 - 0.09 * (1.0 - std::f32::consts::FRAC_PI_4);
        assert!((a.3 - expect).abs() < 4e-3, "{} vs {expect}", a.3);
    }
    #[test]
    fn percent_amount_uses_neighbour_edge() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let e = m
            .ridges()
            .into_iter()
            .find(|(a, b)| {
                let (pa, pb) = (pos(&m, *a), pos(&m, *b));
                pa.y > 0.4 && pb.y > 0.4 && (pa - pb).x.abs() > 1.5
            })
            .unwrap();
        let o = BevelOptions {
            amount: 25.0,
            amount_type: AmountType::Percent,
            ..opts(0.0, 1)
        };
        chamfer_ridges_extra(&mut m, &BTreeSet::from([e]), &o).unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - (2.0 - 0.0625)).abs() < 1e-4, "{}", m.volume());
        assert_eq!(
            BevelOptions { amount: 150.0, ..o }.sanitized().amount,
            100.0
        );
    }
    #[test]
    fn width_and_depth_on_right_angle() {
        let width = |t: AmountType| {
            let mut m = block();
            let e = top_edge(&m);
            let r = chamfer_ridges_extra(
                &mut m,
                &BTreeSet::from([e]),
                &BevelOptions {
                    amount: 0.2,
                    amount_type: t,
                    ..opts(0.0, 1)
                },
            )
            .unwrap();
            let f = *r.facets.iter().next().unwrap();
            let p = m.face_points(f);
            (0..p.len())
                .map(|i| (p[i] - p[(i + 1) % p.len()]).magnitude())
                .fold(f32::MAX, f32::min)
        };
        assert!((width(AmountType::Width) - 0.2).abs() < 1e-4);
        assert!((width(AmountType::Depth) - 0.4).abs() < 1e-4);
    }
    #[test]
    fn pin_account_plus_overrun_rejection() {
        let mut m = block();
        let all: BTreeSet<Edge> = m.ridges().into_iter().collect();
        let r = chamfer_ridges_extra(&mut m, &all, &opts(0.8, 1)).unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.clamped_corners, 8);
        let to = r.clamped_to.unwrap();
        assert!((to - 0.475).abs() < 1e-3, "{to}");
        let note = pin_advisory(&opts(0.8, 1), &r).unwrap();
        assert_eq!(note, "Clamped to 0.475 at 8 corners");
        let left = (1.0 - 2.0 * to) * (1.0 - 2.0 * to);
        let faces = (0..m.facets.len())
            .filter(|f| !r.facets.contains(f) && (m.face_area(*f) - left).abs() < 1e-4)
            .count();
        assert_eq!(faces, 6);
        let mut n = block();
        let before = n.clone();
        let o = BevelOptions {
            clamp_overlap: false,
            ..opts(0.8, 1)
        };
        let err = chamfer_ridges_extra(&mut n, &all, &o).unwrap_err();
        assert_eq!(err, OVERLAP);
        assert_eq!(n, before);
    }
    #[test]
    fn clamped_amount_matches_slide() {
        let mut m = block();
        let e = top_edge(&m);
        let r = chamfer_ridges_extra(&mut m, &BTreeSet::from([e]), &opts(1.5, 1)).unwrap();
        assert!(m.is_closed());
        let to = r.clamped_to.unwrap();
        assert!((to - 0.95).abs() < 1e-3, "{to}");
        assert!(
            (m.volume() - (1.0 - to * to / 2.0)).abs() < 1e-3,
            "{}",
            m.volume()
        );
    }
    #[test]
    fn stiffen_clusters_fresh_facets() {
        let mut m = block();
        let e = top_edge(&m);
        let r = chamfer_ridges_extra(
            &mut m,
            &BTreeSet::from([e]),
            &BevelOptions {
                harden_normals: true,
                ..opts(0.2, 3)
            },
        )
        .unwrap();
        let g = m.facets[*r.facets.iter().next().unwrap()].glide;
        assert!(g > 0);
        assert!(r.facets.iter().all(|f| m.facets[*f].glide == g));
        assert!(
            (0..m.facets.len())
                .filter(|f| !r.facets.contains(f))
                .all(|f| m.facets[f].glide != g)
        );
        let mut n = block();
        let r = chamfer_ridges_extra(&mut n, &BTreeSet::from([e]), &opts(0.2, 3)).unwrap();
        assert!(r.facets.iter().all(|f| n.facets[*f].glide == 0));
    }
    #[test]
    fn invalid_amount_and_empty_selection() {
        let mut m = block();
        let before = m.clone();
        assert_eq!(
            chamfer_ridges_extra(&mut m, &BTreeSet::from([top_edge(&before)]), &opts(0.0, 1))
                .unwrap_err(),
            SET_AMOUNT
        );
        assert_eq!(
            chamfer_ridges_extra(&mut m, &BTreeSet::new(), &opts(0.2, 1)).unwrap_err(),
            SELECT_TO_BEVEL
        );
        assert_eq!(m, before);
    }
    #[test]
    fn vertex_chamfer_cube_corner() {
        let mut m = block();
        let v = top_corner(&m);
        let o = BevelOptions {
            affect: BevelAffect::Vertices,
            ..opts(0.3, 1)
        };
        let r = chamfer(&mut m, &BTreeSet::new(), &BTreeSet::from([v]), &o).unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.facets.len(), 1);
        let f = *r.facets.iter().next().unwrap();
        assert_eq!(m.facets[f].verts.len(), 3);
        assert!(m.face_normal(f).innerprod(v3(1.0, 1.0, 1.0).unitized()) > 0.99);
        assert!(
            (m.volume() - (1.0 - 0.027 / 6.0)).abs() < 1e-5,
            "{}",
            m.volume()
        );
        assert_eq!(m.facets.len(), 7);
    }
    #[test]
    fn vertex_round_cube_corner() {
        for segments in [2, 3, 4] {
            let mut m = block();
            let v = top_corner(&m);
            let o = BevelOptions {
                affect: BevelAffect::Vertices,
                ..opts(0.3, segments)
            };
            let r = bevel_vertices(&mut m, &BTreeSet::from([v]), &o).unwrap();
            m.sanity_check().unwrap();
            assert!(m.is_closed(), "segments {segments}");
            assert!(r.facets.len() > 1);
            assert!(
                m.volume() > 1.0 - 0.027 / 6.0 && m.volume() < 1.0,
                "{}",
                m.volume()
            );
        }
    }
    #[test]
    fn vertex_bevel_all_corners_and_clamp() {
        let mut m = block();
        let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
        let o = BevelOptions {
            affect: BevelAffect::Vertices,
            ..opts(0.8, 1)
        };
        let r = bevel_vertices(&mut m, &all, &o).unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert_eq!(r.facets.len(), 8);
        assert_eq!(r.clamped_corners, 8);
        assert!((r.clamped_to.unwrap() - 0.475).abs() < 1e-3);
        let mut n = block();
        let before = n.clone();
        assert_eq!(
            bevel_vertices(
                &mut n,
                &all,
                &BevelOptions {
                    clamp_overlap: false,
                    ..o
                }
            )
            .unwrap_err(),
            OVERLAP
        );
        assert_eq!(n, before);
    }
    #[test]
    fn vertex_bevel_open_border() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let vf = m.vert_faces();
        let corner = (0..m.positions.len()).find(|v| vf[*v].len() == 1).unwrap() as u32;
        let side = (0..m.positions.len()).find(|v| vf[*v].len() == 2).unwrap() as u32;
        let area = m.total_area();
        let o = BevelOptions {
            affect: BevelAffect::Vertices,
            ..opts(0.25, 1)
        };
        let r = bevel_vertices(&mut m, &BTreeSet::from([corner, side]), &o).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(
            r.facets.len(),
            1,
            "border side vertex gets a cap, corner none"
        );
        assert!(
            (m.total_area() - (area - 0.03125)).abs() < 1e-4,
            "{}",
            m.total_area()
        );
        assert!(m.facets.iter().all(|f| f.verts.len() >= 3));
    }
    #[test]
    fn vertex_bevel_refuses_reflex_corner() {
        let mut m = BMesh::default();
        let pts = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ];
        let ids: Vec<u32> = pts
            .iter()
            .map(|(x, z)| m.add_vert(v3(*x, 0.0, *z)))
            .collect();
        m.add_face_facing(ids.clone(), v3(0.0, 1.0, 0.0));
        let before = m.clone();
        let o = BevelOptions {
            affect: BevelAffect::Vertices,
            ..opts(0.2, 1)
        };
        assert_eq!(
            bevel_vertices(&mut m, &BTreeSet::from([ids[3]]), &o).unwrap_err(),
            JUNCTION
        );
        assert_eq!(m, before);
    }
    #[test]
    fn hud_strings() {
        let o = BevelOptions {
            stretches: 3,
            ..opts(0.2, 3)
        };
        assert_eq!(
            overview(&o),
            "Bevel \u{b7} 3 segments \u{b7} Profile 0.50 \u{b7} Offset 0.200"
        );
        let p = BevelOptions {
            amount: 12.5,
            amount_type: AmountType::Percent,
            silhouette: 0.25,
            ..o
        };
        assert_eq!(
            overview(&p),
            "Bevel \u{b7} 3 segments \u{b7} Profile 0.25 \u{b7} Percent 12.5%"
        );
        let r = BevelReport {
            skipped_edges: BTreeSet::from([(0, 1), (1, 2)]),
            ..Default::default()
        };
        assert_eq!(status_line(&o, &r, 12).unwrap(), "2 of 12 skipped");
        assert_eq!(formatted(1234.5, 1), "1,234.5");
    }
    #[test]
    fn rough_prelook_stretches() {
        let e = BevelOptions {
            stretches: 32,
            ..Default::default()
        };
        assert_eq!(prelook_stretches(&e, 10), 32);
        assert_eq!(prelook_stretches(&e, 1000), 4);
        assert_eq!(prelook_stretches(&e, 3000), 1);
        let v = BevelOptions {
            affect: BevelAffect::Vertices,
            ..e
        };
        assert_eq!(prelook_stretches(&v, 100), 8);
        assert_eq!(
            prelook_stretches(&BevelOptions { stretches: 1, ..e }, 1_000_000),
            1
        );
    }
    #[test]
    fn profile_weights_right_angle_is_circular() {
        let (a, b) = silhouette_hefts(v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), Vec3::ZERO, 0.5);
        assert!((a - 0.553_5).abs() < 1e-4 && (a - b).abs() < 1e-6);
        let mid = profile_curve(v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), Vec3::ZERO, 0.5, 2)[0];
        assert!(((mid - v3(1.0, 1.0, 0.0)).magnitude() - 1.0).abs() < 1e-3);
    }
    fn top_rim(m: &BMesh) -> BTreeSet<Edge> {
        let t = (0..m.facets.len())
            .find(|f| m.face_normal(*f).y > 0.9)
            .unwrap();
        m.facets[t].loop_edges().map(|(a, b)| edge(a, b)).collect()
    }
    fn top_area(m: &BMesh) -> f32 {
        (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y > 0.999)
            .map(|f| m.face_area(f))
            .sum()
    }
    #[test]
    fn rim_bevel_insets_the_face() {
        for seg in [1u32, 3] {
            let mut m = block();
            let rim = top_rim(&m);
            let r = chamfer_ridges_extra(&mut m, &rim, &opts(0.2, seg)).unwrap();
            m.sanity_check().unwrap();
            assert!(m.is_closed());
            assert!(
                (top_area(&m) - 0.36).abs() < 1e-4,
                "seg {seg}: {}",
                top_area(&m)
            );
            assert!(
                r.facets.len() >= 4 * seg as usize,
                "seg {seg}: {}",
                r.facets.len()
            );
            if seg == 1 {
                let expect = 1.0 - 4.0 * 0.02 + 4.0 * 0.008 / 3.0;
                assert!(
                    (m.volume() - expect).abs() < 1e-4,
                    "{} vs {expect}",
                    m.volume()
                );
            }
        }
    }
    #[test]
    fn all_edges_segments_add_faces() {
        let count = |seg: u32| {
            let mut c = block();
            let all: BTreeSet<Edge> = c.ridges().into_iter().collect();
            let r = chamfer_ridges_extra(&mut c, &all, &opts(0.2, seg)).unwrap();
            c.sanity_check().unwrap();
            assert!(c.is_closed());
            let squares = (0..c.facets.len())
                .filter(|f| !r.facets.contains(f) && (c.face_area(*f) - 0.36).abs() < 1e-4)
                .count();
            assert_eq!(squares, 6);
            (r.facets.len(), c.volume())
        };
        let (f1, v1) = count(1);
        let (f3, v3) = count(3);
        assert_eq!(f1, 12 + 8);
        assert!(f3 > f1 && v3 > v1, "{f1} {v1} {f3} {v3}");
    }
    #[test]
    fn rim_profile_changes_only_shape() {
        let run = |p: f32| {
            let mut m = block();
            let rim = top_rim(&m);
            chamfer_ridges_extra(
                &mut m,
                &rim,
                &BevelOptions {
                    silhouette: p,
                    ..opts(0.2, 4)
                },
            )
            .unwrap();
            assert!(m.is_closed());
            (m.positions.len(), m.facets.len(), m.volume())
        };
        let (a, b) = (run(0.5), run(0.15));
        assert_eq!((a.0, a.1), (b.0, b.1));
        assert!((a.2 - b.2).abs() > 1e-4);
    }
    #[test]
    fn rim_width_amount() {
        let mut m = block();
        let rim = top_rim(&m);
        chamfer_ridges_extra(
            &mut m,
            &rim,
            &BevelOptions {
                amount: 0.2,
                amount_type: AmountType::Width,
                ..opts(0.0, 1)
            },
        )
        .unwrap();
        let side = 1.0 - 2.0 * 0.2 / 2f32.sqrt();
        assert!(
            (top_area(&m) - side * side).abs() < 1e-4,
            "{}",
            top_area(&m)
        );
    }
    fn vopts(amount: f32, t: AmountType, segments: u32) -> BevelOptions {
        BevelOptions {
            affect: BevelAffect::Vertices,
            amount,
            amount_type: t,
            ..opts(0.0, segments)
        }
    }
    fn corner_cap(o: &BevelOptions, size: Vec3) -> (Vec3, Vec<Vec3>) {
        let mut m = cuboid(size);
        let v = top_corner(&m);
        let corner = pos(&m, v);
        let r = bevel_vertices(&mut m, &BTreeSet::from([v]), o).unwrap();
        assert!(m.is_closed());
        let f = *r.facets.iter().next().unwrap();
        (corner, m.face_points(f))
    }
    #[test]
    fn vertex_amount_types() {
        let (_, p) = corner_cap(&vopts(0.3, AmountType::Width, 1), v3(1.0, 1.0, 1.0));
        assert!(((p[0] - p[1]).magnitude() - 0.3).abs() < 1e-4);
        let (corner, p) = corner_cap(&vopts(0.1, AmountType::Depth, 1), v3(1.0, 1.0, 1.0));
        let n = v3(1.0, 1.0, 1.0).unitized();
        assert!(((corner - p[0]).innerprod(n) - 0.1).abs() < 1e-4);
        let (c2, p) = corner_cap(&vopts(20.0, AmountType::Percent, 1), v3(2.0, 1.0, 1.0));
        let mut d: Vec<f32> = p.iter().map(|q| (c2 - *q).magnitude()).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            (d[0] - 0.2).abs() < 1e-4 && (d[1] - 0.2).abs() < 1e-4 && (d[2] - 0.4).abs() < 1e-4,
            "{d:?}"
        );
    }
    #[test]
    fn vertex_profile_and_harden() {
        let run = |p: f32, harden: bool| {
            let mut m = block();
            let v = top_corner(&m);
            let o = BevelOptions {
                silhouette: p,
                harden_normals: harden,
                ..vopts(0.3, AmountType::Offset, 4)
            };
            let r = bevel_vertices(&mut m, &BTreeSet::from([v]), &o).unwrap();
            assert!(m.is_closed());
            let groups: BTreeSet<u16> = r.facets.iter().map(|f| m.facets[*f].glide).collect();
            (m.positions.len(), m.facets.len(), m.volume(), groups)
        };
        let (a, b) = (run(0.5, false), run(0.2, false));
        assert_eq!((a.0, a.1), (b.0, b.1));
        assert!((a.2 - b.2).abs() > 1e-5);
        assert_eq!(a.3, BTreeSet::from([0]));
        assert_eq!(run(0.5, true).3, BTreeSet::from([1]));
    }
}
