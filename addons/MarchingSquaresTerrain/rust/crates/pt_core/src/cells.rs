use crate::data::{TerrainStore, VOID_SLOT};
use crate::math::*;
use crate::mesh::{Soup, TVert};
use std::collections::HashMap;
use std::rc::Rc;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EdgeStyle {
    #[default]
    FlatWalls = 0,
    Terraces = 1,
    Slopes = 2,
    HexPrism = 3,
}
impl EdgeStyle {
    pub fn from_i32(v: i32) -> EdgeStyle {
        match v {
            1 => EdgeStyle::Terraces,
            2 => EdgeStyle::Slopes,
            3 => EdgeStyle::HexPrism,
            _ => EdgeStyle::FlatWalls,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeParams {
    pub style: EdgeStyle,
    pub terrace_step: f32,
    pub prism_bevel: f32,
    pub prism_skirt: f32,
}
impl Default for EdgeParams {
    fn default() -> Self {
        EdgeParams {
            style: EdgeStyle::FlatWalls,
            terrace_step: 0.5,
            prism_bevel: 0.1405,
            prism_skirt: 9.0,
        }
    }
}
impl EdgeParams {
    pub fn wedge(&self) -> bool {
        self.style == EdgeStyle::HexPrism
    }
    fn level(&self) -> f32 {
        self.terrace_step.abs().max(1e-3)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RampKind {
    #[default]
    Linear,
    Terrace,
    Slope,
    Bevel,
}
const TR: f32 = 0.0805;
const HALF_TERRACE_PROFILE: [(f32, f32); 4] = [
    (0.0, 0.0),
    (2.0 * TR, 2.0 / 3.0),
    (1.0 - TR, 2.0 / 3.0),
    (1.0, 1.0),
];
const LINEAR_PROFILE: [(f32, f32); 2] = [(0.0, 0.0), (1.0, 1.0)];
const SLOPE_RUN: f32 = 0.3505;
impl RampKind {
    fn silhouette(self) -> &'static [(f32, f32)] {
        match self {
            RampKind::Terrace => &HALF_TERRACE_PROFILE,
            _ => &LINEAR_PROFILE,
        }
    }
    fn rock(self, u0: f32, u1: f32) -> bool {
        match self {
            RampKind::Linear => false,
            RampKind::Slope | RampKind::Bevel => true,
            RampKind::Terrace => (u1 - u0).abs() > 1e-6,
        }
    }
}
fn classify(
    edges: EdgeParams,
    h: f32,
    nh: f32,
    merge: f32,
    max_run: f32,
) -> Option<(RampKind, f32, f32)> {
    let step = h - nh;
    if step.abs() <= 1e-4 {
        return None;
    }
    if edges.wedge() {
        let b = prism_bevel(edges, step, max_run);
        return (step > 0.0).then_some((RampKind::Bevel, h - b, b));
    }
    if edges.style == EdgeStyle::Terraces && step.abs() <= edges.terrace_step + 1e-4 {
        return Some((RampKind::Terrace, (h + nh) * 0.5, max_run));
    }
    if step < 0.0 {
        return None;
    }
    if step <= merge {
        return Some((RampKind::Linear, nh, step.min(max_run)));
    }
    if edges.style == EdgeStyle::Slopes {
        return Some((RampKind::Slope, nh, (step * SLOPE_RUN).min(max_run)));
    }
    None
}
fn prism_bevel(edges: EdgeParams, step: f32, max_run: f32) -> f32 {
    let apothem = max_run / RAMP_MAX_INSET;
    (edges.prism_bevel * apothem)
        .min(step * 0.4005)
        .min(edges.level() * 0.4005)
        .min(max_run)
        .max(1e-3)
}
fn prism_grooves(edges: EdgeParams, top: f32, bottom: f32, b: f32) -> Vec<f32> {
    let l = edges.level();
    let mut out = Vec::new();
    let mut k = ((bottom + b - 1e-4) / l).ceil();
    while k * l <= top - b * 1.5005 + 1e-4 && out.len() < 256 {
        out.push(k * l);
        k += 1.0;
    }
    out
}
struct Ctx<'a> {
    store: &'a TerrainStore,
    cs: Vec2,
    corner_cache: HashMap<IVec2, Vec<f32>>,
    geom_cache: HashMap<IVec2, Rc<Geom>>,
}
fn token(p: Vec2) -> (i32, i32) {
    ((p.x * 1024.0).round() as i32, (p.y * 1024.0).round() as i32)
}
impl<'a> Ctx<'a> {
    fn new(store: &'a TerrainStore) -> Self {
        Ctx {
            store,
            cs: store.config.cell_size_safe(),
            corner_cache: HashMap::new(),
            geom_cache: HashMap::new(),
        }
    }
    fn glide(&self, c: IVec2) -> bool {
        self.store.probe(c).map(|s| s.glide).unwrap_or(false)
    }
    fn corner_heights(&mut self, c: IVec2) -> Vec<f32> {
        if let Some(v) = self.corner_cache.get(&c) {
            return v.clone();
        }
        let cfg = &self.store.config;
        let corners = cfg.cell_corners(c);
        let h = self.store.stature(c).unwrap_or(0.0);
        let mut out = vec![h; corners.len()];
        if self.glide(c) {
            for (i, p) in corners.iter().enumerate() {
                let k = token(*p);
                let (mut sum, mut n) = (0.0, 0);
                for cell in self.cells_around_corner(c, k) {
                    if let Some(s) = self.store.probe(cell)
                        && s.glide
                    {
                        sum += s.stature;
                        n += 1;
                    }
                }
                if n > 0 {
                    out[i] = sum / n as f32;
                }
            }
        }
        self.corner_cache.insert(c, out.clone());
        out
    }
    fn cells_around_corner(&self, start: IVec2, k: (i32, i32)) -> Vec<IVec2> {
        let cfg = &self.store.config;
        let mut result = Vec::new();
        let mut visited = vec![start];
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            let corners = cfg.cell_corners(cell);
            let Some(local) = corners.iter().position(|p| token(*p) == k) else {
                continue;
            };
            result.push(cell);
            for (edge, nb) in cfg.edge_neighbors(cell).into_iter().enumerate() {
                let (a, b) = cfg.edge_corners(cell, edge);
                if a != local && b != local {
                    continue;
                }
                if visited.contains(&nb) {
                    continue;
                }
                visited.push(nb);
                if self.store.probe(nb).is_some() {
                    queue.push_back(nb);
                }
            }
        }
        result
    }
    fn merge_of(&self, c: IVec2) -> f32 {
        let chunk = self.store.config.chunk_of(c);
        self.store
            .chunks
            .get(&chunk)
            .map(|d| d.merge_mode.threshold())
            .unwrap_or(0.0)
    }
    fn geom(&mut self, c: IVec2) -> Option<Rc<Geom>> {
        if let Some(g) = self.geom_cache.get(&c) {
            return Some(g.clone());
        }
        let s = self.store.probe(c)?;
        let prism = self.store.ridges.wedge();
        let cfg = &self.store.config;
        let center = cfg.sample_pos(c);
        let corners = cfg.cell_corners(c);
        let n = corners.len();
        let nbs = cfg.edge_neighbors(c);
        let edges: Vec<(usize, usize)> = (0..nbs.len()).map(|e| cfg.edge_corners(c, e)).collect();
        let merge = self.merge_of(c);
        let ch = if s.glide {
            self.corner_heights(c)
        } else {
            vec![s.stature; n]
        };
        let mut inward = Vec::with_capacity(edges.len());
        let mut apothem = f32::MAX;
        for &(a, b) in &edges {
            let d = corners[b] - corners[a];
            let mut nrm = v2(-d.y, d.x).unitized();
            if nrm.innerprod(center - corners[a]) < 0.0 {
                nrm = -nrm;
            }
            apothem = apothem.min(nrm.innerprod(center - corners[a]));
            inward.push(nrm);
        }
        let max_run = apothem * RAMP_MAX_INSET;
        let mut ramp = vec![None; edges.len()];
        let mut kind = vec![RampKind::Linear; edges.len()];
        let mut run = vec![0.0f32; edges.len()];
        if prism && !s.glide {
            let ep = self.store.ridges;
            let drops: Vec<Option<f32>> = nbs
                .iter()
                .map(|nb| match self.store.probe(*nb) {
                    Some(ns) if ns.floor != VOID_SLOT => Some(s.stature - ns.stature),
                    Some(_) => None,
                    None => Some(ep.prism_skirt * ep.level()),
                })
                .map(|d| d.filter(|d| *d > 1e-4))
                .collect();
            let b = drops
                .iter()
                .flatten()
                .map(|d| prism_bevel(ep, *d, max_run))
                .fold(f32::MAX, f32::min);
            for (e, d) in drops.iter().enumerate() {
                if d.is_some() {
                    ramp[e] = Some(s.stature - b);
                    kind[e] = RampKind::Bevel;
                    run[e] = b;
                }
            }
        }
        for (e, nb) in nbs.iter().enumerate() {
            if prism {
                break;
            }
            let Some(ns) = self.store.probe(*nb) else {
                continue;
            };
            if s.glide || ns.glide || ns.floor == VOID_SLOT {
                continue;
            }
            if let Some((k, foot, r)) =
                classify(self.store.ridges, s.stature, ns.stature, merge, max_run)
            {
                ramp[e] = Some(foot);
                kind[e] = k;
                run[e] = r;
            }
        }
        let mut corner_edges = vec![(usize::MAX, usize::MAX); n];
        for (e, &(a, b)) in edges.iter().enumerate() {
            for i in [a, b] {
                if corner_edges[i].0 == usize::MAX {
                    corner_edges[i].0 = e;
                } else {
                    corner_edges[i].1 = e;
                }
            }
        }
        let inset = (0..n)
            .map(|i| {
                let (e1, e2) = corner_edges[i];
                let (d1, d2) = (run[e1], run[e2]);
                if d1 == 0.0 && d2 == 0.0 {
                    return corners[i];
                }
                let (n1, n2) = (inward[e1], inward[e2]);
                let det = n1.x * n2.y - n1.y * n2.x;
                if det.abs() < 1e-6 {
                    return corners[i] + n1 * d1.max(d2);
                }
                corners[i] + v2((d1 * n2.y - d2 * n1.y) / det, (n1.x * d2 - n2.x * d1) / det)
            })
            .collect();
        let g = Rc::new(Geom {
            cell: c,
            midpoint: center,
            corners,
            inset,
            ridges: edges,
            corner_edges,
            nbs,
            ramp,
            sort: kind,
            inward,
            apothem,
            h: s.stature,
            ch,
            glide: s.glide,
            floor: s.floor,
            wall: s.wall,
            grass: s.grass,
        });
        self.geom_cache.insert(c, g.clone());
        Some(g)
    }
    fn relation(&mut self, g: &Geom, e: usize) -> Rel {
        let nb = g.nbs[e];
        let Some(ng) = self.geom(nb) else {
            if self.store.ridges.wedge() {
                let base = g.h - self.store.ridges.prism_skirt * self.store.ridges.level();
                let mut pts: Vec<(f32, f32, f32)> = g
                    .silhouette(e)
                    .into_iter()
                    .map(|(t, h)| (t, h, base))
                    .collect();
                pts.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-5);
                return Rel {
                    exists: true,
                    nb_floor: g.floor,
                    pts,
                    ..Default::default()
                };
            }
            return Rel::default();
        };
        let (a, _) = g.ridges[e];
        let Some(f) = ng.nbs.iter().position(|x| *x == g.cell) else {
            return Rel::default();
        };
        let mine = g.silhouette(e);
        let (na, _) = ng.ridges[f];
        let mut theirs = ng.silhouette(f);
        let same_dir = token(ng.corners[na]) == token(g.corners[a]);
        if !same_dir {
            theirs = theirs
                .into_iter()
                .rev()
                .map(|(t, h)| (1.0 - t, h))
                .collect();
        }
        let mut ts: Vec<f32> = mine
            .iter()
            .chain(theirs.iter())
            .map(|p| p.0)
            .chain([0.5])
            .collect();
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
        let pts: Vec<(f32, f32, f32)> = ts
            .iter()
            .map(|&t| (t, eval_profile(&mine, t), eval_profile(&theirs, t)))
            .collect();
        let max_up = pts.iter().map(|p| p.1 - p.2).fold(f32::MIN, f32::max);
        let max_down = pts.iter().map(|p| p.2 - p.1).fold(f32::MIN, f32::max);
        let nb_void = ng.floor == VOID_SLOT;
        let my_face = g.ramp[e].is_some() && matches!(g.sort[e], RampKind::Slope | RampKind::Bevel);
        let their_face =
            ng.ramp[f].is_some() && matches!(ng.sort[f], RampKind::Slope | RampKind::Bevel);
        let my_band_face = my_face && g.sort[e] == RampKind::Slope;
        Rel {
            exists: true,
            connected: !nb_void && !my_face && !their_face && max_up.max(max_down) <= BAND_MIN_STEP,
            ridge: my_band_face || (g.ramp[e].is_none() && max_up > BAND_MIN_STEP),
            ledge: !nb_void && (their_face || max_down > BAND_MIN_STEP),
            nb_floor: ng.floor,
            pts,
        }
    }
    fn emit_cell(&mut self, c: IVec2, out: &mut Soup, walls: bool) {
        let Some(g) = self.geom(c) else { return };
        if g.floor == VOID_SLOT {
            return;
        }
        let rels: Vec<Rel> = (0..g.nbs.len()).map(|e| self.relation(&g, e)).collect();
        let n = g.corners.len();
        let slot_of = |e: i8| -> u8 {
            if e < 0 {
                return g.floor;
            }
            let r = &rels[e as usize];
            if r.exists && r.connected {
                r.nb_floor
            } else {
                g.floor
            }
        };
        let em = Emitter {
            cs: self.cs,
            wall: g.wall,
            grass: g.grass,
            own: g.floor,
        };
        let center_h = if g.glide {
            g.ch.iter().sum::<f32>() / n as f32
        } else {
            g.h
        };
        let o = FV {
            p: v3(g.midpoint.x, center_h, g.midpoint.y),
            tex_coord: Vec2::ZERO,
            w: OWN,
        };
        let band = |r: &Rel| {
            v2(
                if r.ledge { 1.0 } else { 0.0 },
                if r.ridge { 1.0 } else { 0.0 },
            )
        };
        let top: Vec<FV> = (0..n)
            .map(|i| {
                let (e1, e2) = g.corner_edges[i];
                let p = g.inset[i];
                let (r1, r2) = (g.ramp[e1].is_some(), g.ramp[e2].is_some());
                let w = match (r1, r2) {
                    (false, false) => corner_w(e1, e2),
                    (true, false) => [(-1, 0.5), (e2 as i8, 0.5), (-1, 0.0)],
                    (false, true) => [(-1, 0.5), (e1 as i8, 0.5), (-1, 0.0)],
                    (true, true) => OWN,
                };
                let uv = max2(band(&rels[e1]), band(&rels[e2]));
                FV {
                    p: v3(p.x, g.ch[i], p.y),
                    tex_coord: uv,
                    w,
                }
            })
            .collect();
        let inner: Vec<FV> = top
            .iter()
            .map(|t| FV {
                p: o.p.lerp(t.p, INNER_RING),
                tex_coord: t.tex_coord * INNER_BAND,
                w: OWN,
            })
            .collect();
        for (e, &(a, b)) in g.ridges.iter().enumerate() {
            let ramp = g.ramp[e].is_some();
            let mid = FV {
                p: (top[a].p + top[b].p) * 0.5,
                tex_coord: band(&rels[e]),
                w: if ramp {
                    OWN
                } else {
                    [(-1, 0.5), (e as i8, 0.5), (-1, 0.0)]
                },
            };
            em.floor_tri(out, &slot_of, o, inner[a], inner[b]);
            em.floor_tri(out, &slot_of, inner[a], top[a], mid);
            em.floor_tri(out, &slot_of, inner[a], mid, inner[b]);
            em.floor_tri(out, &slot_of, inner[b], mid, top[b]);
            if let Some(f) = g.ramp[e] {
                let kind = g.sort[e];
                let foot = |i: usize| {
                    let (e1, e2) = g.corner_edges[i];
                    FV {
                        p: v3(g.corners[i].x, f, g.corners[i].y),
                        tex_coord: Vec2::ZERO,
                        w: corner_w(e1, e2),
                    }
                };
                let (fa, fb) = (foot(a), foot(b));
                let fm = FV {
                    p: (fa.p + fb.p) * 0.5,
                    tex_coord: Vec2::ZERO,
                    w: [(-1, 0.5), (e as i8, 0.5), (-1, 0.0)],
                };
                let row = |s: f32, u: f32| -> [FV; 3] {
                    [(top[a], fa), (mid, fm), (top[b], fb)].map(|(t, fo)| {
                        let xz = t.p.xz().lerp(fo.p.xz(), s);
                        FV {
                            p: v3(xz.x, t.p.y + (fo.p.y - t.p.y) * u, xz.y),
                            tex_coord: t.tex_coord * (1.0 - s),
                            w: lerp_w(t.w, fo.w, s),
                        }
                    })
                };
                let outward = if f < g.h { -g.inward[e] } else { g.inward[e] };
                for w in kind.silhouette().windows(2) {
                    let ((s0, u0), (s1, u1)) = (w[0], w[1]);
                    let (r0, r1) = (row(s0, u0), row(s1, u1));
                    if kind.rock(u0, u1) {
                        let tag = if kind == RampKind::Slope { 1.0 } else { 0.7505 };
                        for t in [
                            [r0[0], r1[0], r1[1]],
                            [r0[0], r1[1], r0[1]],
                            [r0[1], r1[1], r0[2]],
                            [r0[2], r1[1], r1[2]],
                        ] {
                            em.wall_tri(out, t.map(|v| v.p), outward, tag);
                        }
                    } else {
                        em.floor_tri(out, &slot_of, r0[0], r1[0], r1[1]);
                        em.floor_tri(out, &slot_of, r0[0], r1[1], r0[1]);
                        em.floor_tri(out, &slot_of, r0[1], r1[1], r0[2]);
                        em.floor_tri(out, &slot_of, r0[2], r1[1], r1[2]);
                    }
                }
            }
        }
        if !walls {
            return;
        }
        for i in 0..n {
            let (e1, e2) = g.corner_edges[i];
            if let (Some(f1), Some(f2)) = (g.ramp[e1], g.ramp[e2])
                && ((f1 - f2).abs() > 1e-4 || g.sort[e1] != g.sort[e2])
            {
                let (p1, p2) = (g.sort[e1].silhouette(), g.sort[e2].silhouette());
                let mut ss: Vec<f32> = p1.iter().chain(p2.iter()).map(|p| p.0).collect();
                ss.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                ss.dedup_by(|x, y| (*x - *y).abs() < 1e-5);
                let h = g.ch[i];
                let at = |s: f32| {
                    let xz = g.inset[i].lerp(g.corners[i], s);
                    (
                        xz,
                        h + (f1 - h) * eval_profile(p1, s),
                        h + (f2 - h) * eval_profile(p2, s),
                    )
                };
                let mut pts: Vec<(Vec2, f32, f32)> = Vec::new();
                for w in ss.windows(2) {
                    let (q0, q1) = (at(w[0]), at(w[1]));
                    pts.push(q0);
                    let (d0, d1) = (q0.1 - q0.2, q1.1 - q1.2);
                    if d0 * d1 < 0.0 {
                        let k = d0 / (d0 - d1);
                        pts.push(at(w[0] + (w[1] - w[0]) * k));
                    }
                }
                pts.push(at(1.0));
                let v = |q: (Vec2, f32, f32), hh: f32| v3(q.0.x, hh, q.0.y);
                for w in pts.windows(2) {
                    let (q0, q1) = (w[0], w[1]);
                    let quad = [v(q0, q0.1), v(q1, q1.1), v(q1, q1.2), v(q0, q0.2)];
                    for t in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                        if (t[2] - t[0]).cross(t[1] - t[0]).magnitude() > 1e-8 {
                            em.cap(out, t);
                            em.cap(out, [t[0], t[2], t[1]]);
                        }
                    }
                }
            }
        }
        let ep = self.store.ridges;
        let gb = (ep.prism_bevel * g.apothem)
            .min(ep.level() * 0.4005)
            .min(g.apothem * RAMP_MAX_INSET);
        let grooves: Vec<Vec<f32>> = rels
            .iter()
            .map(|r| {
                if !ep.wedge() || !r.exists || r.pts.is_empty() {
                    return Vec::new();
                }
                let top = r.pts.iter().map(|p| p.1).fold(f32::MAX, f32::min);
                let bottom = r.pts.iter().map(|p| p.2).fold(f32::MIN, f32::max);
                prism_grooves(ep, top, bottom, gb)
            })
            .collect();
        for (e, r) in rels.iter().enumerate() {
            if !r.exists {
                continue;
            }
            let (a, b) = g.ridges[e];
            let (ca, cb) = (g.corners[a], g.corners[b]);
            for &y in &grooves[e] {
                self.emit_groove(&g, &em, out, e, y, gb, &grooves);
            }
            let mut bands: Vec<(f32, f32)> = Vec::new();
            let mut top = f32::MAX;
            for &y in grooves[e].iter().rev() {
                bands.push((top, y));
                top = y - gb;
            }
            bands.push((top, f32::MIN));
            for (bt, bb) in bands {
                let pts: Vec<(f32, f32, f32)> = r
                    .pts
                    .iter()
                    .map(|&(t, x, n)| (t, x.min(bt), n.max(bb)))
                    .collect();
                for w in pts.windows(2) {
                    let ((t0, x0, n0), (t1, x1, n1)) = (w[0], w[1]);
                    let (d0, d1) = (x0 - n0, x1 - n1);
                    if d0 <= 1e-4 && d1 <= 1e-4 {
                        continue;
                    }
                    let (mut s0, mut s1) = ((t0, x0, n0), (t1, x1, n1));
                    if d0 < 0.0 || d1 < 0.0 {
                        let k = d0 / (d0 - d1);
                        let tc = t0 + (t1 - t0) * k;
                        let hc = x0 + (x1 - x0) * k;
                        if d0 < 0.0 {
                            s0 = (tc, hc, hc);
                        } else {
                            s1 = (tc, hc, hc);
                        }
                    }
                    let qa = ca.lerp(cb, s0.0);
                    let qb = ca.lerp(cb, s1.0);
                    em.peel(out, g.midpoint, (qa, s0.1, s0.2), (qb, s1.1, s1.2));
                }
            }
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "the groove emitter needs the geometry, the emitter, the output soup, the edge index and its profile bounds together; a parameter struct would have no other user"
    )]
    fn emit_groove(
        &self,
        g: &Geom,
        em: &Emitter,
        out: &mut Soup,
        e: usize,
        y: f32,
        b: f32,
        grooves: &[Vec<f32>],
    ) {
        let (ia, ib) = g.ridges[e];
        let inner = |i: usize| -> (Vec2, bool) {
            let (e1, e2) = g.corner_edges[i];
            let eo = if e1 == e { e2 } else { e1 };
            if grooves[eo].iter().any(|z| (z - y).abs() < 1e-4) {
                let (n1, n2) = (g.inward[e1], g.inward[e2]);
                let det = n1.x * n2.y - n1.y * n2.x;
                if det.abs() > 1e-6 {
                    return (
                        g.corners[i] + v2((b * n2.y - b * n1.y) / det, (n1.x * b - n2.x * b) / det),
                        false,
                    );
                }
            }
            let (oa, ob) = g.ridges[eo];
            let far = if oa == i { ob } else { oa };
            let u = (g.corners[far] - g.corners[i]).unitized();
            (
                g.corners[i] + u * (b / u.innerprod(g.inward[e]).max(0.2005)),
                true,
            )
        };
        let (pa, cap_a) = inner(ia);
        let (pb, cap_b) = inner(ib);
        let (ca, cb) = (g.corners[ia], g.corners[ib]);
        let p = |q: Vec2, h: f32| v3(q.x, h, q.y);
        let chamfer = [p(ca, y - b), p(cb, y - b), p(pb, y), p(pa, y)];
        for t in [
            [chamfer[0], chamfer[1], chamfer[2]],
            [chamfer[0], chamfer[2], chamfer[3]],
        ] {
            em.wall_tri(out, t, -g.inward[e], 1.0);
        }
        let under = [p(pa, y), p(pb, y), p(cb, y), p(ca, y)];
        for t in [
            [under[0], under[1], under[2]],
            [under[0], under[2], under[3]],
        ] {
            em.tri_facing(out, t, v3(0.0, -1.0, 0.0), 1.0);
        }
        for (c, q, cap) in [(ca, pa, cap_a), (cb, pb, cap_b)] {
            if cap {
                let t = [p(c, y - b), p(q, y), p(c, y)];
                em.cap(out, t);
                em.cap(out, [t[0], t[2], t[1]]);
            }
        }
    }
}
const OWN: [(i8, f32); 3] = [(-1, 1.0), (-1, 0.0), (-1, 0.0)];
const RAMP_MAX_INSET: f32 = 0.4805;
const WALL_SMOOTH_COS: f32 = 0.4505;
const INNER_RING: f32 = 0.4005;
const INNER_BAND: f32 = 0.3505;
const BAND_MIN_STEP: f32 = 0.5;
fn corner_w(e1: usize, e2: usize) -> [(i8, f32); 3] {
    [
        (-1, 1.0 / 3.0),
        (e1 as i8, 1.0 / 3.0),
        (e2 as i8, 1.0 / 3.0),
    ]
}
fn lerp_w(a: [(i8, f32); 3], b: [(i8, f32); 3], s: f32) -> [(i8, f32); 3] {
    let mut acc: Vec<(i8, f32)> = vec![(-1, 0.0)];
    for (src, k) in [(a, 1.0 - s), (b, s)] {
        for (id, w) in src {
            if w <= 0.0 || k <= 0.0 {
                continue;
            }
            match acc.iter_mut().find(|x| x.0 == id) {
                Some(x) => x.1 += w * k,
                None => acc.push((id, w * k)),
            }
        }
    }
    acc[1..].sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = [(-1, 0.0); 3];
    for (k, x) in acc.iter().enumerate() {
        if k < 3 {
            out[k] = *x;
        } else {
            out[0].1 += x.1;
        }
    }
    out
}
fn max2(a: Vec2, b: Vec2) -> Vec2 {
    v2(a.x.max(b.x), a.y.max(b.y))
}
struct Geom {
    cell: IVec2,
    midpoint: Vec2,
    corners: Vec<Vec2>,
    inset: Vec<Vec2>,
    ridges: Vec<(usize, usize)>,
    corner_edges: Vec<(usize, usize)>,
    nbs: Vec<IVec2>,
    ramp: Vec<Option<f32>>,
    sort: Vec<RampKind>,
    inward: Vec<Vec2>,
    apothem: f32,
    h: f32,
    ch: Vec<f32>,
    glide: bool,
    floor: u8,
    wall: u8,
    grass: u8,
}
impl Geom {
    fn silhouette(&self, e: usize) -> Vec<(f32, f32)> {
        let (a, b) = self.ridges[e];
        if self.glide {
            return vec![(0.0, self.ch[a]), (1.0, self.ch[b])];
        }
        if let Some(f) = self.ramp[e] {
            return vec![(0.0, f), (1.0, f)];
        }
        let len = (self.corners[b] - self.corners[a]).magnitude().max(1e-6);
        let other = |i: usize| {
            let (e1, e2) = self.corner_edges[i];
            if e1 == e { e2 } else { e1 }
        };
        let mut v = Vec::with_capacity(12);
        let (oa, ob) = (other(a), other(b));
        match self.ramp[oa] {
            Some(f) => {
                let d = (self.inset[a] - self.corners[a]).magnitude() / len;
                for &(s, u) in self.sort[oa].silhouette().iter().rev() {
                    v.push(((1.0 - s) * d, self.h + (f - self.h) * u));
                }
            }
            None => v.push((0.0, self.h)),
        }
        match self.ramp[ob] {
            Some(f) => {
                let d = (self.inset[b] - self.corners[b]).magnitude() / len;
                for &(s, u) in self.sort[ob].silhouette() {
                    v.push((1.0 - (1.0 - s) * d, self.h + (f - self.h) * u));
                }
            }
            None => v.push((1.0, self.h)),
        }
        v
    }
}
fn eval_profile(p: &[(f32, f32)], t: f32) -> f32 {
    if t <= p[0].0 {
        return p[0].1;
    }
    for w in p.windows(2) {
        if t <= w[1].0 {
            let span = w[1].0 - w[0].0;
            if span < 1e-6 {
                return w[1].1;
            }
            return w[0].1 + (w[1].1 - w[0].1) * (t - w[0].0) / span;
        }
    }
    p[p.len() - 1].1
}
#[derive(Default)]
struct Rel {
    exists: bool,
    connected: bool,
    ridge: bool,
    ledge: bool,
    nb_floor: u8,
    pts: Vec<(f32, f32, f32)>,
}
#[derive(Clone, Copy)]
struct FV {
    p: Vec3,
    tex_coord: Vec2,
    w: [(i8, f32); 3],
}
struct Emitter {
    cs: Vec2,
    wall: u8,
    grass: u8,
    own: u8,
}
impl Emitter {
    fn floor_tri(&self, out: &mut Soup, slot_of: &dyn Fn(i8) -> u8, a: FV, b: FV, c: FV) {
        let mut ids: [i8; 2] = [-1, -1];
        for v in [a, b, c] {
            for (e, w) in v.w {
                if e >= 0 && w > 0.0 && !ids.contains(&e) {
                    if ids[0] < 0 {
                        ids[0] = e;
                    } else if ids[1] < 0 {
                        ids[1] = e;
                    }
                }
            }
        }
        let slots = [self.own, slot_of(ids[0]), slot_of(ids[1])];
        let (mut b, mut c) = (b, c);
        if (c.p - a.p).cross(b.p - a.p).y < 0.0 {
            std::mem::swap(&mut b, &mut c);
        }
        for v in [a, b, c] {
            let mut wt = [0.0f32; 3];
            for (e, w) in v.w {
                let k = if e < 0 {
                    0
                } else if e == ids[0] {
                    1
                } else if e == ids[1] {
                    2
                } else {
                    0
                };
                wt[k] += w;
            }
            out.verts.push(TVert {
                pos: v.p,
                tex_coord: v.tex_coord,
                uv2: v2(v.p.x / self.cs.x, v.p.z / self.cs.y),
                custom0: [
                    slots[0] as f32,
                    slots[1] as f32,
                    slots[2] as f32,
                    self.wall as f32,
                ],
                custom1: [wt[0], wt[1], 0.0, self.grass as f32 / 255.0],
                floor: true,
            });
        }
    }
    fn wall_v(&self, out: &mut Soup, p: Vec3, kind: f32) {
        let w = self.wall as f32;
        out.verts.push(TVert {
            pos: p,
            tex_coord: v2(1.0, 1.0),
            uv2: v2(p.x / self.cs.x, p.z / self.cs.y),
            custom0: [w, w, w, w],
            custom1: [1.0, 0.0, kind, 0.0],
            floor: false,
        });
    }
    fn wall_tri(&self, out: &mut Soup, t: [Vec3; 3], outward: Vec2, kind: f32) {
        let n = (t[2] - t[0]).cross(t[1] - t[0]);
        if n.magnitude() <= 1e-8 {
            return;
        }
        let t = if v2(n.x, n.z).innerprod(outward) < 0.0 {
            [t[0], t[2], t[1]]
        } else {
            t
        };
        for p in t {
            self.wall_v(out, p, kind);
        }
    }
    fn tri_facing(&self, out: &mut Soup, t: [Vec3; 3], dir: Vec3, kind: f32) {
        let n = (t[2] - t[0]).cross(t[1] - t[0]);
        if n.magnitude() <= 1e-8 {
            return;
        }
        let t = if n.innerprod(dir) < 0.0 {
            [t[0], t[2], t[1]]
        } else {
            t
        };
        for p in t {
            self.wall_v(out, p, kind);
        }
    }
    fn cap(&self, out: &mut Soup, t: [Vec3; 3]) {
        for p in t {
            self.wall_v(out, p, 0.7505);
        }
    }
    fn peel(&self, out: &mut Soup, center: Vec2, a: (Vec2, f32, f32), b: (Vec2, f32, f32)) {
        self.strip_kind(out, center, a, b, 1.0);
    }
    fn strip_kind(
        &self,
        out: &mut Soup,
        center: Vec2,
        a: (Vec2, f32, f32),
        b: (Vec2, f32, f32),
        kind: f32,
    ) {
        let (mut a, mut b) = (a, b);
        let dir = b.0 - a.0;
        if v2(dir.y, -dir.x).innerprod((a.0 + b.0) * 0.5 - center) < 0.0 {
            std::mem::swap(&mut a, &mut b);
        }
        let a_low = v3(a.0.x, a.2, a.0.y);
        let b_low = v3(b.0.x, b.2, b.0.y);
        let a_high = v3(a.0.x, a.1, a.0.y);
        let b_high = v3(b.0.x, b.1, b.0.y);
        for t in [[a_low, b_high, a_high], [a_low, b_low, b_high]] {
            if (t[2] - t[0]).cross(t[1] - t[0]).magnitude() > 1e-8 {
                for p in t {
                    self.wall_v(out, p, kind);
                }
            }
        }
    }
}
pub fn mesh_cell_chunk(store: &TerrainStore, chunk: IVec2, out: &mut Soup) {
    mesh_cell_chunk_where(store, chunk, out, |_| true);
}
pub fn mesh_cell_chunk_where(
    store: &TerrainStore,
    chunk: IVec2,
    out: &mut Soup,
    keep: impl Fn(IVec2) -> bool,
) {
    let cfg = &store.config;
    if !store.chunks.contains_key(&chunk) {
        return;
    }
    let mut ctx = Ctx::new(store);
    out.cell_area = cell_area(cfg, chunk);
    out.wall_smooth_cos = if store.ridges.wedge() {
        None
    } else {
        Some(WALL_SMOOTH_COS)
    };
    let cells: Vec<IVec2> = cfg
        .chunk_cells(chunk)
        .into_iter()
        .filter(|c| keep(*c))
        .collect();
    let mut outside: Vec<IVec2> = Vec::new();
    for &g in &cells {
        out.begin_cell(g);
        ctx.emit_cell(g, out, true);
        out.end_cell();
        for nb in cfg.edge_neighbors(g) {
            if (cfg.chunk_of(nb) != chunk || !keep(nb)) && !outside.contains(&nb) {
                outside.push(nb);
            }
        }
    }
    let mut extra = Soup::default();
    for nb in outside {
        ctx.emit_cell(nb, &mut extra, true);
    }
    out.normal_only = extra.verts;
}
pub const LOD_WALL: f32 = 0.6005;
pub fn mesh_cell_chunk_low(store: &TerrainStore, chunk: IVec2, out: &mut Soup) {
    mesh_cell_chunk_flat(store, chunk, out, false, LOD_WALL);
}
pub fn mesh_cell_chunk_flat(
    store: &TerrainStore,
    chunk: IVec2,
    out: &mut Soup,
    grass: bool,
    wall_kind: f32,
) {
    let cfg = &store.config;
    if !store.chunks.contains_key(&chunk) {
        return;
    }
    let cs = cfg.cell_size_safe();
    out.cell_area = cell_area(cfg, chunk);
    out.wall_smooth_cos = None;
    let ep = store.ridges;
    for c in cfg.chunk_cells(chunk) {
        out.begin_cell(c);
        if let Some(s) = store.probe(c).filter(|s| s.floor != VOID_SLOT) {
            let em = Emitter {
                cs,
                wall: s.wall,
                grass: if grass { s.grass } else { 0 },
                own: s.floor,
            };
            let own = |_e: i8| s.floor;
            let center = cfg.sample_pos(c);
            let corners = cfg.cell_corners(c);
            let fv = |p: Vec2| FV {
                p: v3(p.x, s.stature, p.y),
                tex_coord: Vec2::ZERO,
                w: OWN,
            };
            for i in 1..corners.len().saturating_sub(1) {
                em.floor_tri(
                    out,
                    &own,
                    fv(corners[0]),
                    fv(corners[i]),
                    fv(corners[i + 1]),
                );
            }
            for (e, nb) in cfg.edge_neighbors(c).into_iter().enumerate() {
                let base = match store.probe(nb) {
                    Some(ns) => ns.stature,
                    None if ep.wedge() => s.stature - ep.prism_skirt * ep.level(),
                    None => continue,
                };
                if s.stature - base <= 1e-4 {
                    continue;
                }
                let (a, b) = cfg.edge_corners(c, e);
                em.strip_kind(
                    out,
                    center,
                    (corners[a], s.stature, base),
                    (corners[b], s.stature, base),
                    wall_kind,
                );
            }
        }
        out.end_cell();
    }
}
fn cell_area(cfg: &crate::grid::GridConfig, chunk: IVec2) -> f32 {
    let c = cfg.global_of(chunk, iv2(0, 0));
    let k = cfg.cell_corners(c);
    let mut a = 0.0;
    for i in 0..k.len() {
        let (p, q) = (k[i], k[(i + 1) % k.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    (a * 0.5).abs()
}
pub fn decoration_spots(store: &TerrainStore, c: IVec2, min_height: f32) -> Vec<Vec3> {
    decoration_spots_ex(store, c, min_height, &|_, _| false)
        .into_iter()
        .map(|(_, p)| p)
        .collect()
}
pub fn decoration_spots_ex(
    store: &TerrainStore,
    c: IVec2,
    min_height: f32,
    skip: &dyn Fn(IVec2, Vec3) -> bool,
) -> Vec<(IVec2, Vec3)> {
    let Some(s) = store.probe(c) else {
        return Vec::new();
    };
    if s.floor == VOID_SLOT {
        return Vec::new();
    }
    let cfg = &store.config;
    let center = cfg.sample_pos(c);
    let corners = cfg.cell_corners(c);
    let mut spots = vec![center];
    if matches!(
        cfg.grid,
        crate::grid::GridType::Hex | crate::grid::GridType::HexRings
    ) {
        for e in 0..corners.len() {
            let (a, b) = cfg.edge_corners(c, e);
            spots.push(center + ((corners[a] + corners[b]) * 0.5 - center) * (2.0 / 3.0));
        }
    }
    let small_apothem = cfg.cell_size_safe().x / 6.0;
    spots
        .into_iter()
        .filter_map(|p| {
            let h = surface_height(store, p)?;
            if h <= min_height {
                return None;
            }
            let even = (0..6).all(|k| {
                let a = k as f32 * std::f32::consts::FRAC_PI_3;
                let q = p + v2(a.cos(), a.sin()) * (small_apothem * 0.5);
                surface_height(store, q).is_some_and(|hq| (hq - h).abs() < 1e-3)
            });
            let spot = v3(p.x, h, p.y);
            let small = cfg.world_to_small_hex(p);
            (even && !skip(small, spot)).then_some((small, spot))
        })
        .collect()
}
pub fn surface_height(store: &TerrainStore, p: Vec2) -> Option<f32> {
    let c = store.config.world_to_sample(p);
    cell_surface(store, c)?.height_at(p)
}
#[derive(Clone, Debug)]
pub enum CellSurface {
    Flat(f32),
    Tris(Box<[[Vec3; 3]]>, f32),
}
impl CellSurface {
    pub fn height_at(&self, p: Vec2) -> Option<f32> {
        match self {
            CellSurface::Flat(h) => Some(*h),
            CellSurface::Tris(tris, h) => {
                let mut best: Option<f32> = None;
                for t in tris.iter() {
                    if let Some(y) = height_in_tri(t[0], t[1], t[2], p) {
                        best = Some(best.map_or(y, |b: f32| b.max(y)));
                    }
                }
                Some(best.unwrap_or(*h))
            }
        }
    }
}
pub fn cell_surface(store: &TerrainStore, c: IVec2) -> Option<CellSurface> {
    let s = store.probe(c)?;
    if s.floor == VOID_SLOT {
        return None;
    }
    let mut ctx = Ctx::new(store);
    if !s.glide {
        let merge = ctx.merge_of(c);
        let ramps = store.config.edge_neighbors(c).into_iter().any(|nb| {
            (store.ridges.wedge() && store.probe(nb).is_none())
                || store.probe(nb).is_some_and(|ns| {
                    (store.ridges.wedge() || !ns.glide)
                        && ns.floor != VOID_SLOT
                        && classify(store.ridges, s.stature, ns.stature, merge, f32::MAX).is_some()
                })
        });
        if !ramps {
            return Some(CellSurface::Flat(s.stature));
        }
    }
    let mut soup = Soup::default();
    ctx.emit_cell(c, &mut soup, false);
    let tris: Vec<[Vec3; 3]> = soup
        .verts
        .chunks_exact(3)
        .map(|t| [t[0].pos, t[1].pos, t[2].pos])
        .filter(|t| {
            let (v0, v1) = (t[1].xz() - t[0].xz(), t[2].xz() - t[0].xz());
            (v0.x * v1.y - v1.x * v0.y).abs() >= 1e-10
        })
        .collect();
    Some(CellSurface::Tris(tris.into_boxed_slice(), s.stature))
}
#[derive(Default)]
pub struct SurfaceCache {
    tiles: std::cell::RefCell<HashMap<IVec2, Option<CellSurface>>>,
}
impl SurfaceCache {
    const MAX_CELLS: usize = 1 << 18;
    pub fn stature(&self, store: &TerrainStore, p: Vec2) -> Option<f32> {
        let c = store.config.world_to_sample(p);
        let mut cells = self.tiles.borrow_mut();
        if let Some(s) = cells.get(&c) {
            return s.as_ref()?.height_at(p);
        }
        if cells.len() >= Self::MAX_CELLS {
            cells.clear();
        }
        let s = cell_surface(store, c);
        let h = s.as_ref().and_then(|s| s.height_at(p));
        cells.insert(c, s);
        h
    }
    pub fn purge(&self) {
        self.tiles.borrow_mut().clear();
    }
    pub fn invalidate_chunks<'a>(
        &self,
        store: &TerrainStore,
        chunks: impl IntoIterator<Item = &'a IVec2>,
    ) {
        let set: std::collections::HashSet<IVec2> = chunks.into_iter().copied().collect();
        if set.is_empty() {
            return;
        }
        let cfg = &store.config;
        let near: std::collections::HashSet<IVec2> =
            set.iter().flat_map(|c| cfg.chunk_neighbors(*c)).collect();
        self.tiles.borrow_mut().retain(|c, _| {
            let ch = cfg.chunk_of(*c);
            if set.contains(&ch) {
                return false;
            }
            !near.contains(&ch)
                || !cfg
                    .edge_neighbors(*c)
                    .into_iter()
                    .any(|n| set.contains(&cfg.chunk_of(n)))
        });
    }
}
fn height_in_tri(a: Vec3, b: Vec3, c: Vec3, p: Vec2) -> Option<f32> {
    let (v0, v1, v2_) = (b.xz() - a.xz(), c.xz() - a.xz(), p - a.xz());
    let den = v0.x * v1.y - v1.x * v0.y;
    if den.abs() < 1e-10 {
        return None;
    }
    let u = (v2_.x * v1.y - v1.x * v2_.y) / den;
    let v = (v0.x * v2_.y - v2_.x * v0.y) / den;
    let eps = 1e-4;
    if u < -eps || v < -eps || u + v > 1.0 + eps {
        return None;
    }
    Some(a.y + (b.y - a.y) * u + (c.y - a.y) * v)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grass::{GrassParams, scatter};
    use crate::grid::{GridConfig, GridType, SQRT3};
    fn store(grid: GridType) -> TerrainStore {
        let mut s = TerrainStore::new(GridConfig {
            grid,
            chunk_size: 4,
            ring_radius: 3,
            ..Default::default()
        });
        s.add_chunk(iv2(0, 0), MergeMode::Cubic);
        s
    }
    fn hex(merge: MergeMode) -> (TerrainStore, IVec2) {
        let mut s = store(GridType::HexRings);
        s.chunks.get_mut(&iv2(0, 0)).unwrap().merge_mode = merge;
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        (s, c)
    }
    fn cage(s: &TerrainStore) -> Soup {
        let mut soup = Soup::default();
        mesh_cell_chunk(s, iv2(0, 0), &mut soup);
        soup
    }
    fn cell_soup<'a>(soup: &'a Soup, c: IVec2) -> &'a [TVert] {
        let ct = soup.tiles.iter().find(|x| x.cell == c).unwrap();
        &soup.verts[ct.commence..ct.end]
    }
    fn wall_area(v: &[TVert], kind: f32) -> f32 {
        v.chunks_exact(3)
            .filter(|t| !t[0].floor && (t[0].custom1[2] - kind).abs() < 1e-3)
            .map(|t| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).magnitude() * 0.5)
            .sum()
    }
    #[test]
    fn flat_cells_emit_only_tops_facing_up() {
        for g in [GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let s = store(g);
            let soup = cage(&s);
            assert!(soup.verts.iter().all(|v| v.floor), "{g:?}");
            let m = soup.assemble();
            assert!(m.normals.iter().all(|n| n[1] > 0.99), "{g:?} normals up");
        }
    }
    #[test]
    fn cliff_cell_gets_outward_walls_of_full_area_and_bands() {
        for g in [GridType::Hex, GridType::HexRings, GridType::Triangle] {
            let mut s = store(g);
            let local = if g == GridType::HexRings {
                iv2(3, 3)
            } else {
                iv2(2, 1)
            };
            let c = s.config.global_of(iv2(0, 0), local);
            s.set_height(c, 3.0);
            let soup = cage(&s);
            let v = cell_soup(&soup, c);
            let corners = s.config.cell_corners(c);
            let perim: f32 = (0..corners.len())
                .map(|i| (corners[(i + 1) % corners.len()] - corners[i]).magnitude())
                .sum();
            assert!(
                (wall_area(v, 1.0) - perim * 3.0).abs() < 1e-3,
                "{g:?} wall area {}",
                wall_area(v, 1.0)
            );
            let center = s.config.sample_pos(c);
            let m = soup.assemble();
            for (i, n) in m.normals.iter().enumerate() {
                if m.custom1[i][2] > 0.9 {
                    let p = m.positions[i];
                    assert!(
                        v2(p[0] - center.x, p[2] - center.y).innerprod(v2(n[0], n[2])) > 0.0,
                        "{g:?} wall faces inward"
                    );
                }
            }
            assert!(v.iter().any(|x| x.floor && x.tex_coord.y > 0.99));
            let nb = s.config.edge_neighbors(c)[0];
            assert!(
                cell_soup(&soup, nb)
                    .iter()
                    .any(|x| x.floor && x.tex_coord.x > 0.99),
                "{g:?} ledge"
            );
        }
    }
    #[test]
    fn ramps_are_floor_inside_the_higher_cell_and_meet_the_lower_floor() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.set_height(c, 1.0);
        s.edit_sample(c, |d, i| d.floor_tex[i] = 4);
        let soup = cage(&s);
        let v = cell_soup(&soup, c);
        assert!(v.iter().all(|x| x.floor), "no walls: ramps on all sides");
        assert!(
            v.iter().all(|x| x.custom0[0] == 4.0),
            "ramp uses the higher cell's floor slot"
        );
        let center = s.config.sample_pos(c);
        let r = s.config.hex_radius();
        assert!(
            v.iter()
                .all(|x| (x.pos.xz() - center).magnitude() <= r + 1e-4)
        );
        for nb in s.config.edge_neighbors(c) {
            assert!(
                cell_soup(&soup, nb)
                    .iter()
                    .all(|x| x.floor && x.pos.y.abs() < 1e-5)
            );
        }
        let corners = s.config.cell_corners(c);
        let mid = (corners[0] + corners[1]) * 0.5;
        assert!((surface_height(&s, center).unwrap() - 1.0).abs() < 1e-4);
        assert!(surface_height(&s, center.lerp(mid, 0.999)).unwrap() < 0.05);
        let h_half = surface_height(&s, center.lerp(mid, 0.8)).unwrap();
        assert!(h_half > 0.05 && h_half < 0.95, "on the ramp: {h_half}");
        let gp = GrassParams {
            slot_has_grass: [true; 16],
            ..Default::default()
        };
        let blades = scatter(&soup, &gp);
        assert!(
            blades.iter().any(|b| b.pos.y > 0.05 && b.pos.y < 0.95),
            "grass on ramps"
        );
    }
    #[test]
    fn mixed_steps_close_with_strips_and_wedges() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(c, 2.0);
        s.set_height(nbs[1], 2.0);
        s.set_height(nbs[2], 1.0);
        s.set_height(nbs[3], 1.0);
        s.set_height(nbs[4], 0.8);
        let soup = cage(&s);
        let center = s.config.sample_pos(c);
        let w = s.config.cell_size.x;
        for j in 0..40 {
            for i in 0..40 {
                let p = center
                    + v2(
                        (i as f32 / 39.0 - 0.5) * 3.0 * w,
                        (j as f32 / 39.0 - 0.5) * 3.0 * w,
                    );
                let hit = soup.verts.chunks_exact(3).any(|t| {
                    t[0].floor && height_in_tri(t[0].pos, t[1].pos, t[2].pos, p).is_some()
                });
                assert!(hit, "hole at {p:?}");
            }
        }
        let mut ctx = Ctx::new(&s);
        let g = ctx.geom(c).unwrap();
        let mut expect = 0.0;
        for e in 0..6 {
            let r = ctx.relation(&g, e);
            let (a, b) = g.ridges[e];
            let len = (g.corners[b] - g.corners[a]).magnitude();
            for wdw in r.pts.windows(2) {
                let (d0, d1) = (
                    (wdw[0].1 - wdw[0].2).max(0.0),
                    (wdw[1].1 - wdw[1].2).max(0.0),
                );
                if (wdw[0].1 - wdw[0].2) * (wdw[1].1 - wdw[1].2) < 0.0 {
                    let (x0, x1) = (wdw[0].1 - wdw[0].2, wdw[1].1 - wdw[1].2);
                    let k = x0 / (x0 - x1);
                    let dt = wdw[1].0 - wdw[0].0;
                    expect += 0.5 * (d0 * k * dt + d1 * (1.0 - k) * dt) * len;
                } else {
                    expect += 0.5 * (d0 + d1) * (wdw[1].0 - wdw[0].0) * len;
                }
            }
        }
        let got = wall_area(cell_soup(&soup, c), 1.0);
        assert!(
            expect > 0.5 && (got - expect).abs() < 1e-3,
            "strips {got} vs {expect}"
        );
    }
    #[test]
    fn slots_blend_across_flat_borders_but_not_cliffs() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        let nb = s.config.edge_neighbors(c)[0];
        s.edit_sample(nb, |d, i| d.floor_tex[i] = 3);
        let soup = cage(&s);
        let blended = cell_soup(&soup, c)
            .iter()
            .any(|v| v.custom0[1] == 3.0 && v.custom1[1] > 0.4);
        assert!(blended, "edge midpoint mixes in the neighbour slot");
        s.set_height(nb, 5.0);
        let soup = cage(&s);
        assert!(
            cell_soup(&soup, c)
                .iter()
                .filter(|v| v.floor)
                .all(|v| v.custom0[..3].iter().all(|m| *m == 0.0))
        );
    }
    #[test]
    fn hex_grass_covers_the_whole_cell() {
        let s = store(GridType::HexRings);
        let soup = cage(&s);
        let gp = GrassParams {
            subdivisions: 3,
            slot_has_grass: [true; 16],
            ..Default::default()
        };
        let cells = soup.tiles.len() as f32;
        let per_cell = scatter(&soup, &gp).len() as f32 / cells;
        assert!((per_cell - 9.0).abs() < 0.9, "{per_cell} blades per cell");
        assert!((soup.cell_area - SQRT3 * 0.5 * 4.0).abs() < 1e-3);
    }
    #[test]
    fn floor_normals_match_across_chunk_seams() {
        let mut s = store(GridType::HexRings);
        s.add_chunk(iv2(1, 0), MergeMode::Cubic);
        for ch in [iv2(0, 0), iv2(1, 0)] {
            for c in s.config.chunk_cells(ch) {
                let p = s.config.sample_pos(c);
                s.set_height(c, (p.x * 0.7).sin() * 0.8 + p.y * 0.1);
                s.edit_sample(c, |d, i| d.glide[i] = 1);
            }
        }
        let build = |ch: IVec2| {
            let mut soup = Soup::default();
            mesh_cell_chunk(&s, ch, &mut soup);
            soup.assemble()
        };
        let (a, b) = (build(iv2(0, 0)), build(iv2(1, 0)));
        let key = |p: [f32; 3]| {
            (
                (p[0] * 512.0).round() as i32,
                (p[1] * 512.0).round() as i32,
                (p[2] * 512.0).round() as i32,
            )
        };
        let mut shared = 0;
        for (i, p) in a.positions.iter().enumerate() {
            if a.custom1[i][2] > 0.5 {
                continue;
            }
            for (j, q) in b.positions.iter().enumerate() {
                if b.custom1[j][2] < 0.5 && key(*p) == key(*q) {
                    shared += 1;
                    let (n, m) = (a.normals[i], b.normals[j]);
                    let d = n[0] * m[0] + n[1] * m[1] + n[2] * m[2];
                    assert!(d > 0.9999, "seam normal mismatch at {p:?}: {n:?} vs {m:?}");
                }
            }
        }
        assert!(shared > 4, "chunks share border vertices ({shared})");
    }
    #[test]
    fn smooth_cells_follow_their_plane() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        let nbs = s.config.edge_neighbors(c);
        for (k, nb) in std::iter::once(c).chain(nbs.iter().copied()).enumerate() {
            s.set_height(nb, k as f32 * 0.2);
            s.edit_sample(nb, |d, i| d.glide[i] = 1);
        }
        let soup = cage(&s);
        let center = s.config.sample_pos(c);
        let corners = s.config.cell_corners(c);
        let ctx_h = surface_height(&s, corners[0].lerp(center, 0.01)).unwrap();
        let expect = (0.0 + 0.2 * 6.0 + 0.2 * 1.0) / 3.0;
        assert!((ctx_h - expect).abs() < 0.02, "{ctx_h} vs {expect}");
        assert!(!soup.verts.is_empty());
    }
    const STYLES: [EdgeStyle; 4] = [
        EdgeStyle::FlatWalls,
        EdgeStyle::Terraces,
        EdgeStyle::Slopes,
        EdgeStyle::HexPrism,
    ];
    fn seg_hits(p: Vec3, q: Vec3, t: &[TVert]) -> bool {
        let (a, b, c) = (t[0].pos, t[1].pos, t[2].pos);
        let d = q - p;
        let (e1, e2) = (b - a, c - a);
        let h = d.cross(e2);
        let det = e1.innerprod(h);
        if det.abs() < 1e-9 {
            return false;
        }
        let f = 1.0 / det;
        let s = p - a;
        let u = f * s.innerprod(h);
        let qv = s.cross(e1);
        let v = f * d.innerprod(qv);
        let tt = f * e2.innerprod(qv);
        let eps = 1e-4;
        u >= -eps && v >= -eps && u + v <= 1.0 + eps && (-eps..=1.0 + eps).contains(&tt)
    }
    fn bumpy(grid: GridType, style: EdgeStyle, seed: u32) -> TerrainStore {
        let mut s = store(grid);
        s.ridges.style = style;
        s.chunks.get_mut(&iv2(0, 0)).unwrap().merge_mode = MergeMode::Polyhedron;
        for (k, c) in s.config.chunk_cells(iv2(0, 0)).into_iter().enumerate() {
            let h = (hash01(c, seed) * 8.0).floor() * 0.5;
            s.set_height(c, h);
            if k % 11 == 5 {
                s.edit_sample(c, |d, i| d.floor_tex[i] = 4);
            }
        }
        s
    }
    fn bumpy_bridged(grid: GridType, style: EdgeStyle, seed: u32) -> TerrainStore {
        let mut s = bumpy(grid, style, seed);
        for c in s.config.chunk_cells(iv2(0, 0)) {
            if hash01(c, seed + 99) < 0.4 {
                s.edit_sample(c, |d, i| d.glide[i] = 1);
            }
        }
        s
    }
    #[test]
    fn every_style_is_watertight() {
        for grid in [GridType::HexRings, GridType::Triangle] {
            for style in STYLES {
                for seed in [1, 7, 23, 101, 107, 123] {
                    let s = if seed > 100 {
                        bumpy_bridged(grid, style, seed)
                    } else {
                        bumpy(grid, style, seed)
                    };
                    let soup = cage(&s);
                    let cells = s.config.chunk_cells(iv2(0, 0));
                    let mid = cells
                        .iter()
                        .map(|c| s.config.sample_pos(*c))
                        .fold(Vec2::ZERO, |a, p| a + p)
                        * (1.0 / cells.len() as f32);
                    let w = s.config.cell_size.x;
                    let reach = if grid == GridType::HexRings {
                        1.6 * w
                    } else {
                        1.2 * w
                    };
                    let (mut misses, mut tested) = (0, 0);
                    for k in 0..4000u32 {
                        let r = |j: u32| hash01(iv2(k as i32, j as i32), seed) * 2.0 - 1.0;
                        let a = mid + v2(r(1), r(2)) * reach;
                        let b = a + v2(r(3), r(4)) * (0.6 * w);
                        let inner = |x: Vec2| {
                            let c = s.config.world_to_sample(x);
                            s.probe(c).is_some()
                                && s.config
                                    .edge_neighbors(c)
                                    .iter()
                                    .all(|n| s.probe(*n).is_some())
                        };
                        if !inner(a) || !inner(b) || !inner(a.lerp(b, 0.5)) {
                            continue;
                        }
                        tested += 1;
                        let (p, q) = (v3(a.x, 6.0, a.y), v3(b.x, -2.0, b.y));
                        if !soup.verts.chunks_exact(3).any(|t| seg_hits(p, q, t)) {
                            if misses < 3 {
                                let c = s.config.world_to_sample(a);
                                let hs: Vec<(IVec2, f32)> = s
                                    .config
                                    .edge_neighbors(c)
                                    .iter()
                                    .map(|n| (*n, s.stature(*n).unwrap_or(-9.0)))
                                    .collect();
                                eprintln!(
                                    "miss {a:?}->{b:?} cell {c:?} h {:?} nbs {hs:?}",
                                    s.stature(c)
                                );
                            }
                            misses += 1;
                        }
                    }
                    assert!(tested > 500, "{grid:?}: {tested} segments");
                    assert_eq!(
                        misses, 0,
                        "{grid:?} {style:?} seed {seed}: {misses} segments slipped through"
                    );
                }
            }
        }
    }
    #[test]
    fn height_queries_match_the_mesh() {
        for (style, bridged) in STYLES.iter().flat_map(|st| [(*st, false), (*st, true)]) {
            let s = if bridged {
                bumpy_bridged(GridType::HexRings, style, 5)
            } else {
                bumpy(GridType::HexRings, style, 5)
            };
            let soup = cage(&s);
            let center = s
                .config
                .sample_pos(s.config.global_of(iv2(0, 0), iv2(3, 3)));
            let w = s.config.cell_size.x;
            for k in 0..600u32 {
                let r = |j: u32| hash01(iv2(k as i32, 100 + j as i32), 3) * 2.0 - 1.0;
                let p = center + v2(r(1), r(2)) * (1.8 * w);
                let mut best: Option<f32> = None;
                for t in soup.verts.chunks_exact(3) {
                    if let Some(h) = height_in_tri(t[0].pos, t[1].pos, t[2].pos, p) {
                        best = Some(best.map_or(h, |b: f32| b.max(h)));
                    }
                }
                let got = surface_height(&s, p).unwrap();
                let want = best.unwrap();
                assert!(
                    (got - want).abs() < 2e-3,
                    "{style:?} at {p:?}: query {got} vs mesh {want}"
                );
            }
        }
    }
    #[test]
    fn terraces_step_down_in_treads_with_rock_risers() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.ridges.style = EdgeStyle::Terraces;
        s.ridges.terrace_step = 1.25;
        s.set_height(c, 1.2);
        let soup = cage(&s);
        let v = cell_soup(&soup, c);
        assert!(wall_area(v, 0.75) > 0.1, "rock risers");
        assert!(wall_area(v, 1.0) < 1e-6, "no cliff walls on a terrace step");
        let center = s.config.sample_pos(c);
        let nb = s.config.edge_neighbors(c)[0];
        let nc = s.config.sample_pos(nb);
        let mut levels: Vec<f32> = Vec::new();
        for i in 0..=400 {
            let h = surface_height(&s, center.lerp(nc, i as f32 / 400.0)).unwrap();
            if levels.last().is_none_or(|l| (l - h).abs() > 1e-3) {
                levels.push(h);
            }
        }
        for want in [1.2, 0.8, 0.4, 0.0] {
            assert!(
                levels.iter().any(|l| (l - want).abs() < 1e-3),
                "plateau {want} in {levels:?}"
            );
        }
        assert!(
            levels.windows(2).all(|w| w[1] <= w[0] + 1e-4),
            "monotonic descent {levels:?}"
        );
        assert!(wall_area(cell_soup(&soup, nb), 0.75) > 0.01);
        assert!(
            cell_soup(&soup, nb)
                .iter()
                .any(|x| x.floor && (x.pos.y - 0.4).abs() < 1e-4)
        );
        let gp = GrassParams {
            slot_has_grass: [true; 16],
            subdivisions: 4,
            ..Default::default()
        };
        let blades = scatter(&soup, &gp);
        assert!(
            blades.iter().any(|b| (b.pos.y - 0.4).abs() < 1e-3),
            "grass on the lower tread"
        );
        assert!(
            blades.iter().any(|b| (b.pos.y - 0.8).abs() < 1e-3),
            "grass on the upper tread"
        );
        s.set_height(c, 3.0);
        let soup = cage(&s);
        assert!(wall_area(cell_soup(&soup, c), 1.0) > 1.0, "cliff");
    }
    #[test]
    fn slopes_turn_cliffs_into_sloped_rock_faces() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.ridges.style = EdgeStyle::Slopes;
        s.set_height(c, 3.0);
        let soup = cage(&s);
        let v = cell_soup(&soup, c);
        let faces: Vec<&[TVert]> = v
            .chunks_exact(3)
            .filter(|t| !t[0].floor && t[0].custom1[2] > 0.9)
            .collect();
        assert!(!faces.is_empty());
        for t in &faces {
            let n = (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).unitized();
            assert!(n.y > 0.1, "battered, not vertical: {n:?}");
        }
        assert!(
            v.iter().any(|x| x.floor && x.tex_coord.y > 0.99),
            "ridge band on the cliff top"
        );
        let nb = s.config.edge_neighbors(c)[0];
        assert!(
            cell_soup(&soup, nb)
                .iter()
                .any(|x| x.floor && x.tex_coord.x > 0.99),
            "ledge band at the foot"
        );
        let center = s.config.sample_pos(c);
        assert!(
            v.iter()
                .all(|x| (x.pos.xz() - center).magnitude() <= s.config.hex_radius() + 1e-4)
        );
        assert!(cell_soup(&soup, nb).iter().all(|x| x.pos.y.abs() < 1e-5));
        s.set_height(c, 1.0);
        let soup = cage(&s);
        assert!(cell_soup(&soup, c).iter().all(|x| x.floor));
    }
    #[test]
    fn cliff_corners_share_smoothed_wall_normals() {
        let (mut s, c) = hex(MergeMode::Cubic);
        s.set_height(c, 3.0);
        let m = cage(&s).assemble();
        let corners = s.config.cell_corners(c);
        for k in corners {
            let at: Vec<[f32; 3]> = (0..m.positions.len())
                .filter(|&i| {
                    m.custom1[i][2] > 0.9
                        && (v2(m.positions[i][0], m.positions[i][2]) - k).magnitude() < 1e-4
                })
                .map(|i| m.normals[i])
                .collect();
            assert!(at.len() >= 2, "corner has wall verts");
            for n in &at {
                let d = n[0] * at[0][0] + n[1] * at[0][1] + n[2] * at[0][2];
                assert!(d > 0.9999, "one normal per wall corner: {at:?}");
            }
            let center = s.config.sample_pos(c);
            let out = (k - center).unitized();
            assert!(out.innerprod(v2(at[0][0], at[0][2])) > 0.99);
        }
    }
    #[test]
    fn wall_normals_match_across_chunk_seams() {
        let mut s = store(GridType::HexRings);
        s.add_chunk(iv2(1, 0), MergeMode::Cubic);
        let (x, y) = s
            .config
            .chunk_cells(iv2(0, 0))
            .into_iter()
            .find_map(|x| {
                s.config
                    .edge_neighbors(x)
                    .into_iter()
                    .find(|y| s.config.chunk_of(*y) == iv2(1, 0))
                    .map(|y| (x, y))
            })
            .unwrap();
        s.set_height(x, 2.0);
        s.set_height(y, 2.0);
        let build = |ch: IVec2| {
            let mut soup = Soup::default();
            mesh_cell_chunk(&s, ch, &mut soup);
            soup.assemble()
        };
        let (a, b) = (build(iv2(0, 0)), build(iv2(1, 0)));
        let key = |p: [f32; 3]| {
            (
                (p[0] * 512.0).round() as i32,
                (p[1] * 512.0).round() as i32,
                (p[2] * 512.0).round() as i32,
            )
        };
        let walls = |m: &crate::mesh::MeshArrays| -> HashMap<(i32, i32, i32), [f32; 3]> {
            (0..m.positions.len())
                .filter(|&i| m.custom1[i][2] > 0.9)
                .map(|i| (key(m.positions[i]), m.normals[i]))
                .collect()
        };
        let (wa, wb) = (walls(&a), walls(&b));
        let mut shared = 0;
        for (k, n) in &wa {
            if let Some(m) = wb.get(k) {
                shared += 1;
                assert!(
                    n[0] * m[0] + n[1] * m[1] + n[2] * m[2] > 0.9999,
                    "seam wall normal {n:?} vs {m:?}"
                );
            }
        }
        assert!(shared >= 2, "walls meet on the seam ({shared})");
    }
    #[test]
    fn hex_prism_columns_are_flat_topped_with_grooves_per_level() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.ridges.style = EdgeStyle::HexPrism;
        s.ridges.terrace_step = 0.5;
        s.set_height(c, 3.0);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(nbs[0], 0.5);
        s.set_height(nbs[3], 0.25);
        let soup = cage(&s);
        let v = cell_soup(&soup, c);
        let tris: Vec<&[TVert]> = v.chunks_exact(3).collect();
        let nrm = |t: &[TVert]| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).unitized();
        for t in tris.iter().filter(|t| t[0].floor) {
            assert!(t.iter().all(|x| (x.pos.y - 3.0).abs() < 1e-5), "flat top");
        }
        let walls: Vec<Vec3> = tris
            .iter()
            .filter(|t| !t[0].floor && t[0].custom1[2] > 0.9)
            .map(|t| nrm(t))
            .collect();
        assert!(walls.iter().any(|n| n.y.abs() < 1e-3), "vertical sides");
        assert!(
            walls.iter().any(|n| (n.y - 0.7071).abs() < 0.02),
            "chamfers"
        );
        assert!(walls.iter().any(|n| n.y < -0.99), "groove undersides");
        assert!(
            walls
                .iter()
                .all(|n| n.y.abs() < 1e-3 || (n.y - 0.7071).abs() < 0.02 || n.y < -0.99),
            "{walls:?}"
        );
        let center = s.config.sample_pos(c);
        let corners = s.config.cell_corners(c);
        let edge_out = |e: usize| {
            let (a, b) = s.config.edge_corners(c, e);
            ((corners[a] + corners[b]) * 0.5 - center).unitized()
        };
        let under_levels = |e_out: Vec2| -> Vec<f32> {
            let mut ys: Vec<f32> = tris
                .iter()
                .filter(|t| !t[0].floor && nrm(t).y < -0.99)
                .filter(|t| {
                    ((t[0].pos.xz() + t[1].pos.xz() + t[2].pos.xz()) * (1.0 / 3.0) - center)
                        .unitized()
                        .innerprod(e_out)
                        > 0.9
                })
                .map(|t| (t[0].pos.y * 100.0).round() / 100.0)
                .collect();
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ys.dedup();
            ys
        };
        let e0 = (0..6)
            .find(|&e| s.config.edge_neighbors(c)[e] == nbs[0])
            .unwrap();
        assert_eq!(
            under_levels(edge_out(e0)),
            vec![1.0, 1.5, 2.0, 2.5],
            "wall down to the 0.5 neighbour"
        );
        assert_eq!(
            under_levels(edge_out((e0 + 1) % 6)),
            vec![0.5, 1.0, 1.5, 2.0, 2.5]
        );
        assert!(
            v.iter()
                .all(|x| (x.pos.xz() - center).magnitude() <= s.config.hex_radius() + 1e-4)
        );
        assert!(
            cell_soup(&soup, nbs[3])
                .iter()
                .filter(|x| x.floor)
                .all(|x| (x.pos.y - 0.25).abs() < 1e-5)
        );
        assert!(soup.wall_smooth_cos.is_none());
        assert!((surface_height(&s, center).unwrap() - 3.0).abs() < 1e-4);
        let (a, b) = s.config.edge_corners(c, (e0 + 1) % 6);
        let rim = center.lerp((corners[a] + corners[b]) * 0.5, 0.999);
        assert!(surface_height(&s, rim).unwrap() < 3.0 - 0.05);
    }
    #[test]
    fn hex_prism_bridge_cells_slope_between_flat_columns() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.ridges.style = EdgeStyle::HexPrism;
        s.ridges.terrace_step = 0.5;
        let nbs = s.config.edge_neighbors(c);
        let (lo, hi) = (nbs[0], nbs[3]);
        s.set_height(lo, 0.0);
        s.set_height(hi, 2.0);
        for (cell, h) in [(lo, 0.0), (c, 1.0), (hi, 2.0)] {
            s.set_height(cell, h);
            s.edit_sample(cell, |d, i| d.glide[i] = 1);
        }
        let soup = cage(&s);
        let floors: Vec<f32> = cell_soup(&soup, c)
            .iter()
            .filter(|v| v.floor)
            .map(|v| v.pos.y)
            .collect();
        let (min, max) = floors
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!(
            max - min > 0.5,
            "bridged prism cell is a slope ({min}..{max})"
        );
        let (pl, ph) = (s.config.sample_pos(lo), s.config.sample_pos(hi));
        let center = s.config.sample_pos(c);
        let (yl, yh) = (
            surface_height(&s, center.lerp(pl, 0.45)).unwrap(),
            surface_height(&s, center.lerp(ph, 0.45)).unwrap(),
        );
        assert!(yl < 1.0 - 0.1 && yh > 1.0 + 0.1, "{yl} .. {yh}");
        let other = nbs[1];
        s.set_height(other, 3.0);
        let soup = cage(&s);
        assert!(
            cell_soup(&soup, other)
                .iter()
                .filter(|v| v.floor)
                .all(|v| v.pos.y <= 3.0 + 1e-5)
        );
        assert!(
            cell_soup(&soup, other)
                .iter()
                .any(|v| v.floor && (v.pos.y - 3.0).abs() < 1e-5)
        );
    }
    #[test]
    fn surface_cache_matches_surface_height_and_follows_edits() {
        for style in [EdgeStyle::HexPrism, EdgeStyle::FlatWalls, EdgeStyle::Slopes] {
            let (mut s, c) = hex(MergeMode::Polyhedron);
            s.ridges.style = style;
            s.set_height(c, 2.0);
            let cache = SurfaceCache::default();
            let center = s.config.sample_pos(c);
            let probe: Vec<Vec2> = (0..400)
                .map(|i| center + v2((i % 20) as f32 * 0.17 - 1.7, (i / 20) as f32 * 0.17 - 1.7))
                .collect();
            for p in &probe {
                assert_eq!(
                    cache.stature(&s, *p),
                    surface_height(&s, *p),
                    "{style:?} at {p:?}"
                );
                assert_eq!(cache.stature(&s, *p), surface_height(&s, *p));
            }
            s.set_height(c, 0.5);
            cache.invalidate_chunks(&s, &[s.config.chunk_of(c)]);
            for p in &probe {
                assert_eq!(
                    cache.stature(&s, *p),
                    surface_height(&s, *p),
                    "{style:?} after edit at {p:?}"
                );
            }
            cache.purge();
            assert_eq!(cache.stature(&s, center), Some(0.5));
        }
    }
    #[test]
    fn decoration_spots_are_the_seven_small_hexes_on_even_dry_ground() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.set_height(c, 2.0);
        s.ridges.style = EdgeStyle::HexPrism;
        let spots = decoration_spots(&s, c, 0.0);
        assert_eq!(spots.len(), 7);
        let center = s.config.sample_pos(c);
        let w = s.config.cell_size.x;
        for p in &spots[1..] {
            assert!(
                ((p.xz() - center).magnitude() - w / 3.0).abs() < 1e-4,
                "ring at a third of the width"
            );
            assert!((p.y - 2.0).abs() < 1e-5);
        }
        for nb in s.config.edge_neighbors(c) {
            let dir = (s.config.sample_pos(nb) - center).unitized();
            assert!(
                spots[1..]
                    .iter()
                    .any(|p| (p.xz() - center).unitized().innerprod(dir) > 0.9999)
            );
        }
        s.ridges.style = EdgeStyle::FlatWalls;
        s.set_height(c, 1.0);
        let spots = decoration_spots(&s, c, -5.0);
        assert_eq!(spots.len(), 1, "only the centre stays flat: {spots:?}");
        assert!(decoration_spots(&s, c, 1.5).is_empty());
    }
    #[test]
    fn hex_prism_border_columns_get_a_skirt() {
        let mut s = store(GridType::HexRings);
        s.ridges.style = EdgeStyle::HexPrism;
        s.ridges.terrace_step = 0.5;
        s.ridges.prism_skirt = 4.0;
        let soup = cage(&s);
        let low = soup
            .verts
            .iter()
            .filter(|x| !x.floor)
            .map(|x| x.pos.y)
            .fold(f32::MAX, f32::min);
        assert!((low + 2.0).abs() < 1e-4, "skirt 4 levels deep: {low}");
        let (_, c) = hex(MergeMode::Cubic);
        assert!(cell_soup(&soup, c).iter().all(|x| x.floor));
    }
    #[test]
    fn low_detail_cells_are_flat_tops_with_plain_walls_and_no_grass() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.ridges.style = EdgeStyle::HexPrism;
        s.ridges.terrace_step = 0.5;
        s.ridges.prism_skirt = 4.0;
        s.set_height(c, 3.0);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(nbs[0], 1.0);
        for i in 0..s.config.chunk_cells(iv2(0, 0)).len() {
            let cell = s.config.chunk_cells(iv2(0, 0))[i];
            s.edit_sample(cell, |d, k| d.grass[k] = 255);
        }
        let full = cage(&s);
        let mut low = Soup::default();
        mesh_cell_chunk_low(&s, iv2(0, 0), &mut low);
        assert!(
            low.tri_tally() * 3 < full.tri_tally(),
            "{} vs {}",
            low.tri_tally(),
            full.tri_tally()
        );
        let v = cell_soup(&low, c);
        for t in v.chunks_exact(3).filter(|t| t[0].floor) {
            for x in t {
                assert!((x.pos.y - 3.0).abs() < 1e-5);
                assert_eq!(x.custom1[0], 1.0);
                assert_eq!(x.custom1[3], 0.0, "no grass flag");
                assert_eq!(x.tex_coord, Vec2::ZERO, "no bands");
            }
        }
        let walls: Vec<&[TVert]> = v.chunks_exact(3).filter(|t| !t[0].floor).collect();
        assert!(
            walls
                .iter()
                .all(|t| t.iter().all(|x| (x.custom1[2] - LOD_WALL).abs() < 1e-6))
        );
        let edge = s.config.hex_radius();
        let area = wall_area(v, LOD_WALL);
        assert!((area - edge * (2.0 + 5.0 * 3.0)).abs() < 1e-3, "{area}");
        assert!(
            scatter(
                &low,
                &GrassParams {
                    slot_has_grass: [true; 16],
                    ..Default::default()
                }
            )
            .is_empty()
        );
        let bottom = low
            .verts
            .iter()
            .filter(|x| !x.floor)
            .map(|x| x.pos.y)
            .fold(f32::MAX, f32::min);
        assert!((bottom + 2.0).abs() < 1e-4, "{bottom}");
        let mut ls = Soup::default();
        s.ridges.style = EdgeStyle::FlatWalls;
        mesh_cell_chunk_low(&s, iv2(0, 0), &mut ls);
        assert!(
            ls.verts
                .iter()
                .filter(|x| !x.floor)
                .all(|x| x.pos.y >= -1e-5),
            "no skirt outside HexPrism"
        );
    }
}
