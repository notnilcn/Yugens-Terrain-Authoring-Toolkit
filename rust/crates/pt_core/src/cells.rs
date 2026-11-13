//! Mesher for the cell modes (triangle, hex, hex rings).
//!
//! Each cell is a flat polygon at its own height. Edges towards a neighbour that is lower by
//! at most the chunk's merge threshold become *ramps* built inside the higher cell (its top
//! polygon is inset; the ramp is floor-textured, blended and grassed, its foot lies on the
//! shared edge at the neighbour's height). Cells flagged smooth (bridge tool /
//! `set_sample_smooth`) become planes through the averaged heights of neighbouring smooth
//! cells. Every edge is then closed by vertical strips between the two cells' surface
//! profiles along it, emitted by whichever side is higher, so ramps, cliffs and smooth
//! planes always meet without cracks.
//!
//! Top polygons are split into an inner fan and an outer rim so floor slots blend across
//! cell borders (corner = ⅓ of each of the three cells, edge midpoint = ½ / ½, inner ring =
//! own slot only; neighbours across a cliff never blend), and so the rim carries the
//! ridge (`UV.y`, cliff top) and ledge (`UV.x`, cliff foot) band factors.
//!
//! [`EdgeStyle`] (the terrain's `hex_edge_style`) picks the profile of those in-cell slopes:
//! straight ramps + vertical cliffs (*FlatWalls*), stepped terraces for steps of up to one
//! level (*Terraces*, catlikecoding Hex Map style: 3 rock risers, 2 grassed treads, split at
//! the shared edge so each cell meshes half: the higher one steps down to the mid height, the
//! lower one steps up to it), or straight ramps + battered (sloped) rock faces for cliffs
//! (*Slopes*). Every ramp is a profile `u(s)` from the cell's own height to the height on the
//! shared edge across its run (`s` = 0 at the inset top edge, 1 on the shared edge), so ramps
//! meeting at a corner share the same rows on the corner diagonal, side profiles feed the edge
//! strips, and caps along the diagonal close two different profiles.
//!
//! *HexPrism* (BitCraft-style column stacks, the `kg_hex` game look) drops ramps, terraces and
//! smooth planes: every cell is a flat-topped prism column and every step is an exposed prism
//! side. The column reads as a stack of one-level prisms: its top rim is bevelled (a narrow
//! rock chamfer, built like a ramp of run `b`), and every whole level `k · terrace_step` inside
//! a wall gets the chamfer + underside groove of the prism below it. Edges on the terrain
//! border get a skirt `prism_skirt` levels deep, so the columns never look hollow.

use crate::data::{TerrainStore, VOID_SLOT};
use crate::math::*;
use crate::mesh::{Soup, TVert};
use std::collections::HashMap;
use std::rc::Rc;

/// How cell grids close height steps between neighbouring cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EdgeStyle {
    /// Ramps for steps up to the merge threshold, vertical cliff walls above it.
    #[default]
    FlatWalls = 0,
    /// Steps of up to one level (`EdgeParams::terrace_step`) become stepped terraces shared by
    /// both cells; larger steps ramp (up to the merge threshold) or are cliffs.
    Terraces = 1,
    /// Ramps for small steps; cliffs become steep, sloped rock faces inside the higher cell.
    Slopes = 2,
    /// Stacked hex prism columns: flat tops, vertical sides, a bevelled groove per level.
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

/// Edge meshing options of the cell grids (`TerrainStore::edges`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeParams {
    pub style: EdgeStyle,
    /// Terraces: largest height step that becomes a terrace ("one level"). HexPrism: height of
    /// one prism of a column (grooves sit at whole multiples of it).
    pub terrace_step: f32,
    /// HexPrism: bevel / groove size as a fraction of the cell apothem.
    pub prism_bevel: f32,
    /// HexPrism: depth of the skirt on the terrain border, in levels (`terrace_step`).
    pub prism_skirt: f32,
}

impl Default for EdgeParams {
    fn default() -> Self {
        EdgeParams { style: EdgeStyle::FlatWalls, terrace_step: 0.5, prism_bevel: 0.14, prism_skirt: 9.0 }
    }
}

impl EdgeParams {
    pub fn prism(&self) -> bool {
        self.style == EdgeStyle::HexPrism
    }
    fn level(&self) -> f32 {
        self.terrace_step.abs().max(1e-3)
    }
}

/// Profile of one in-cell slope (see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RampKind {
    /// Straight, floor-textured ramp.
    #[default]
    Linear,
    /// Half of a terrace (rock risers + floor tread); the other half is in the neighbour.
    Terrace,
    /// Straight, wall-textured cliff face.
    Slope,
    /// HexPrism: the rock chamfer on a column's top rim (run = drop = the bevel size).
    Bevel,
}

/// A full terrace (catlikecoding: 2 terraces per slope) is 3 risers of `TR` and 2 treads
/// across both cells, symmetric about the shared edge. Each cell meshes one half, from its own
/// height (s = 0) to the mid height on the shared edge (s = 1): a riser of 2/3 of its half
/// drop, a tread, and half of the middle riser.
const TR: f32 = 0.08;
const HALF_TERRACE_PROFILE: [(f32, f32); 4] = [(0.0, 0.0), (2.0 * TR, 2.0 / 3.0), (1.0 - TR, 2.0 / 3.0), (1.0, 1.0)];
const LINEAR_PROFILE: [(f32, f32); 2] = [(0.0, 0.0), (1.0, 1.0)];
/// Slopes style: horizontal run of a cliff face per unit of height (before the inset limit).
const SLOPE_RUN: f32 = 0.35;

impl RampKind {
    /// `(s, u)`: fraction of the run from the top edge, fraction of the drop.
    fn profile(self) -> &'static [(f32, f32)] {
        match self {
            RampKind::Terrace => &HALF_TERRACE_PROFILE,
            _ => &LINEAR_PROFILE,
        }
    }
    /// Segment `u0 -> u1` of this profile is drawn as rock (wall) instead of floor.
    fn rock(self, u0: f32, u1: f32) -> bool {
        match self {
            RampKind::Linear => false,
            RampKind::Slope | RampKind::Bevel => true,
            RampKind::Terrace => (u1 - u0).abs() > 1e-6,
        }
    }
}

/// Classifies the step from a cell at `h` to a (non-void, non-smooth) neighbour at `nh`:
/// `Some((kind, height on the shared edge, run))` for an in-cell slope, `None` for a flat
/// edge or a vertical wall. Terraces go both ways (up and down, symmetric, so both cells
/// agree); ramps and sloped faces only exist in the higher cell (its chunk's `merge`).
fn classify(edges: EdgeParams, h: f32, nh: f32, merge: f32, max_run: f32) -> Option<(RampKind, f32, f32)> {
    let step = h - nh;
    if step.abs() <= 1e-4 {
        return None;
    }
    if edges.prism() {
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

/// HexPrism bevel size for a drop of `step` (`max_run` = 0.48 apothem): the nominal bevel,
/// shrunk on small steps so the chamfer never reaches the lower cell's top.
fn prism_bevel(edges: EdgeParams, step: f32, max_run: f32) -> f32 {
    let apothem = max_run / RAMP_MAX_INSET;
    (edges.prism_bevel * apothem).min(step * 0.4).min(edges.level() * 0.4).min(max_run).max(1e-3)
}

/// HexPrism: whole levels `y` with a groove `[y - b, y]` inside a wall running from `top`
/// (its lowest point along the edge) down to `bottom` (its highest).
fn prism_grooves(edges: EdgeParams, top: f32, bottom: f32, b: f32) -> Vec<f32> {
    let l = edges.level();
    let mut out = Vec::new();
    let mut k = ((bottom + b - 1e-4) / l).ceil();
    while k * l <= top - b * 1.5 + 1e-4 && out.len() < 256 {
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

fn key(p: Vec2) -> (i32, i32) {
    ((p.x * 1024.0).round() as i32, (p.y * 1024.0).round() as i32)
}

impl<'a> Ctx<'a> {
    fn new(store: &'a TerrainStore) -> Self {
        Ctx { store, cs: store.config.cell_size_safe(), corner_cache: HashMap::new(), geom_cache: HashMap::new() }
    }

    fn smooth(&self, c: IVec2) -> bool {
        !self.store.edges.prism() && self.store.sample(c).map(|s| s.smooth).unwrap_or(false)
    }

    /// Corner heights of a cell: flat unless the cell is smooth, in which case every corner
    /// averages the smooth cells sharing it (so neighbouring smooth cells meet exactly).
    fn corner_heights(&mut self, c: IVec2) -> Vec<f32> {
        if let Some(v) = self.corner_cache.get(&c) {
            return v.clone();
        }
        let cfg = &self.store.config;
        let corners = cfg.cell_corners(c);
        let h = self.store.height(c).unwrap_or(0.0);
        let mut out = vec![h; corners.len()];
        if self.smooth(c) {
            for (i, p) in corners.iter().enumerate() {
                let k = key(*p);
                let (mut sum, mut n) = (0.0, 0);
                for cell in self.cells_around_corner(c, k) {
                    if let Some(s) = self.store.sample(cell)
                        && s.smooth
                        && !self.store.edges.prism() {
                            sum += s.height;
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
            let Some(local) = corners.iter().position(|p| key(*p) == k) else { continue };
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
                if self.store.sample(nb).is_some() {
                    queue.push_back(nb);
                }
            }
        }
        result
    }

    fn merge_of(&self, c: IVec2) -> f32 {
        let chunk = self.store.config.chunk_of(c);
        self.store.chunks.get(&chunk).map(|d| d.merge_mode.threshold()).unwrap_or(0.0)
    }

    /// Classifies a cell: insets its top polygon for ramps towards lower neighbours.
    fn geom(&mut self, c: IVec2) -> Option<Rc<Geom>> {
        if let Some(g) = self.geom_cache.get(&c) {
            return Some(g.clone());
        }
        let mut s = self.store.sample(c)?;
        let prism = self.store.edges.prism();
        if prism {
            s.smooth = false;
        }
        let cfg = &self.store.config;
        let center = cfg.sample_pos(c);
        let corners = cfg.cell_corners(c);
        let n = corners.len();
        let nbs = cfg.edge_neighbors(c);
        let edges: Vec<(usize, usize)> = (0..nbs.len()).map(|e| cfg.edge_corners(c, e)).collect();
        let merge = self.merge_of(c);
        let ch = if s.smooth { self.corner_heights(c) } else { vec![s.height; n] };
        // Inward unit normals and apothems per edge.
        let mut inward = Vec::with_capacity(edges.len());
        let mut apothem = f32::MAX;
        for &(a, b) in &edges {
            let d = corners[b] - corners[a];
            let mut nrm = v2(-d.y, d.x).normalized();
            if nrm.dot(center - corners[a]) < 0.0 {
                nrm = -nrm;
            }
            apothem = apothem.min(nrm.dot(center - corners[a]));
            inward.push(nrm);
        }
        let max_run = apothem * RAMP_MAX_INSET;
        let mut ramp = vec![None; edges.len()];
        let mut kind = vec![RampKind::Linear; edges.len()];
        let mut run = vec![0.0f32; edges.len()];
        // HexPrism: every lower (or missing) neighbour gets a bevel, one size per cell (from its
        // smallest drop) so all chamfers of a column meet in one rim.
        if prism {
            let ep = self.store.edges;
            let drops: Vec<Option<f32>> = nbs
                .iter()
                .map(|nb| match self.store.sample(*nb) {
                    Some(ns) if ns.floor != VOID_SLOT => Some(s.height - ns.height),
                    Some(_) => None,
                    None => Some(ep.prism_skirt * ep.level()),
                })
                .map(|d| d.filter(|d| *d > 1e-4))
                .collect();
            let b = drops.iter().flatten().map(|d| prism_bevel(ep, *d, max_run)).fold(f32::MAX, f32::min);
            for (e, d) in drops.iter().enumerate() {
                if d.is_some() {
                    ramp[e] = Some(s.height - b);
                    kind[e] = RampKind::Bevel;
                    run[e] = b;
                }
            }
        }
        for (e, nb) in nbs.iter().enumerate() {
            if prism {
                break;
            }
            let Some(ns) = self.store.sample(*nb) else { continue };
            if s.smooth || ns.smooth || ns.floor == VOID_SLOT {
                continue;
            }
            if let Some((k, foot, r)) = classify(self.store.edges, s.height, ns.height, merge, max_run) {
                ramp[e] = Some(foot);
                kind[e] = k;
                run[e] = r;
            }
        }
        // Inset corner = intersection of the two (offset) edge lines meeting there.
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
            center,
            corners,
            inset,
            edges,
            corner_edges,
            nbs,
            ramp,
            kind,
            inward,
            apothem,
            h: s.height,
            ch,
            smooth: s.smooth,
            floor: s.floor,
            wall: s.wall,
            grass: s.grass,
        });
        self.geom_cache.insert(c, g.clone());
        Some(g)
    }

    /// Relation of cell `c` to the neighbour across edge `e`: both surface profiles along the
    /// shared edge, sampled at the union of their breakpoints.
    fn relation(&mut self, g: &Geom, e: usize) -> Rel {
        let nb = g.nbs[e];
        let Some(ng) = self.geom(nb) else {
            if self.store.edges.prism() {
                // Terrain border: a skirt down to `prism_skirt` levels below the top.
                let base = g.h - self.store.edges.prism_skirt * self.store.edges.level();
                let mut pts: Vec<(f32, f32, f32)> = g.profile(e).into_iter().map(|(t, h)| (t, h, base)).collect();
                pts.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-5);
                return Rel { exists: true, nb_floor: g.floor, pts, ..Default::default() };
            }
            return Rel::default();
        };
        let (a, _) = g.edges[e];
        let Some(f) = ng.nbs.iter().position(|x| *x == g.cell) else {
            return Rel::default();
        };
        let mine = g.profile(e);
        let (na, _) = ng.edges[f];
        let mut theirs = ng.profile(f);
        // Map the neighbour's parameter onto ours (its edge may run the other way).
        let same_dir = key(ng.corners[na]) == key(g.corners[a]);
        if !same_dir {
            theirs = theirs.into_iter().rev().map(|(t, h)| (1.0 - t, h)).collect();
        }
        // 0.5: both tops (and ramp feet) have a vertex at the edge midpoint; avoid T-junctions.
        let mut ts: Vec<f32> = mine.iter().chain(theirs.iter()).map(|p| p.0).chain([0.5]).collect();
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
        let pts: Vec<(f32, f32, f32)> = ts.iter().map(|&t| (t, eval_profile(&mine, t), eval_profile(&theirs, t))).collect();
        let max_up = pts.iter().map(|p| p.1 - p.2).fold(f32::MIN, f32::max);
        let max_down = pts.iter().map(|p| p.2 - p.1).fold(f32::MIN, f32::max);
        let nb_void = ng.floor == VOID_SLOT;
        // A sloped cliff face (Slopes style) still separates the two floors like a wall.
        let my_face = g.ramp[e].is_some() && matches!(g.kind[e], RampKind::Slope | RampKind::Bevel);
        let their_face = ng.ramp[f].is_some() && matches!(ng.kind[f], RampKind::Slope | RampKind::Bevel);
        // Prism tops stay clean up to the bevel (no rock ridge band on the rim).
        let my_band_face = my_face && g.kind[e] == RampKind::Slope;
        Rel {
            exists: true,
            connected: !nb_void && !my_face && !their_face && max_up.max(max_down) <= BAND_MIN_STEP,
            ridge: my_band_face || (g.ramp[e].is_none() && max_up > BAND_MIN_STEP),
            ledge: !nb_void && (their_face || max_down > BAND_MIN_STEP),
            nb_floor: ng.floor,
            pts,
        }
    }

    /// Emits one cell: top polygon, ramps, ramp corner wedges and the vertical strips
    /// closing every edge where this cell's surface is above its neighbour's.
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
            if r.exists && r.connected { r.nb_floor } else { g.floor }
        };
        let em = Emitter { cs: self.cs, wall: g.wall, grass: g.grass, own: g.floor };

        // Surface heights of the top polygon.
        let center_h = if g.smooth { g.ch.iter().sum::<f32>() / n as f32 } else { g.h };
        let o = FV { p: v3(g.center.x, center_h, g.center.y), uv: Vec2::ZERO, w: OWN };
        // Inset corners with their blend weights / band factors.
        let band = |r: &Rel| v2(if r.ledge { 1.0 } else { 0.0 }, if r.ridge { 1.0 } else { 0.0 });
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
                // Ramps carry no band; sloped cliff faces (Slopes) keep the ridge on their top.
                let uv = max2(band(&rels[e1]), band(&rels[e2]));
                FV { p: v3(p.x, g.ch[i], p.y), uv, w }
            })
            .collect();
        let inner: Vec<FV> = top
            .iter()
            .map(|t| FV { p: o.p.lerp(t.p, INNER_RING), uv: t.uv * INNER_BAND, w: OWN })
            .collect();
        for (e, &(a, b)) in g.edges.iter().enumerate() {
            let ramp = g.ramp[e].is_some();
            let mid = FV {
                p: (top[a].p + top[b].p) * 0.5,
                uv: band(&rels[e]),
                w: if ramp { OWN } else { [(-1, 0.5), (e as i8, 0.5), (-1, 0.0)] },
            };
            em.floor_tri(out, &slot_of, o, inner[a], inner[b]);
            em.floor_tri(out, &slot_of, inner[a], top[a], mid);
            em.floor_tri(out, &slot_of, inner[a], mid, inner[b]);
            em.floor_tri(out, &slot_of, inner[b], mid, top[b]);
            if let Some(f) = g.ramp[e] {
                let kind = g.kind[e];
                let foot = |i: usize| {
                    let (e1, e2) = g.corner_edges[i];
                    FV { p: v3(g.corners[i].x, f, g.corners[i].y), uv: Vec2::ZERO, w: corner_w(e1, e2) }
                };
                let (fa, fb) = (foot(a), foot(b));
                let fm = FV { p: (fa.p + fb.p) * 0.5, uv: Vec2::ZERO, w: [(-1, 0.5), (e as i8, 0.5), (-1, 0.0)] };
                // One row (corner a, edge midpoint, corner b) per profile breakpoint.
                let row = |s: f32, u: f32| -> [FV; 3] {
                    [(top[a], fa), (mid, fm), (top[b], fb)].map(|(t, fo)| {
                        let xz = t.p.xz().lerp(fo.p.xz(), s);
                        FV { p: v3(xz.x, t.p.y + (fo.p.y - t.p.y) * u, xz.y), uv: t.uv * (1.0 - s), w: lerp_w(t.w, fo.w, s) }
                    })
                };
                // Rock faces look towards the lower side: out of the cell for ramps down,
                // into it for the lower half of a terrace.
                let outward = if f < g.h { -g.inward[e] } else { g.inward[e] };
                for w in kind.profile().windows(2) {
                    let ((s0, u0), (s1, u1)) = (w[0], w[1]);
                    let (r0, r1) = (row(s0, u0), row(s1, u1));
                    if kind.rock(u0, u1) {
                        let tag = if kind == RampKind::Slope { 1.0 } else { 0.75 };
                        for t in [[r0[0], r1[0], r1[1]], [r0[0], r1[1], r0[1]], [r0[1], r1[1], r0[2]], [r0[2], r1[1], r1[2]]] {
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
        // Two ramps of different depth or profile meeting at a corner leave a vertical sliver
        // on the corner diagonal (inset corner -> outer corner) between their profiles.
        for i in 0..n {
            let (e1, e2) = g.corner_edges[i];
            if let (Some(f1), Some(f2)) = (g.ramp[e1], g.ramp[e2])
                && ((f1 - f2).abs() > 1e-4 || g.kind[e1] != g.kind[e2])
            {
                let (p1, p2) = (g.kind[e1].profile(), g.kind[e2].profile());
                let mut ss: Vec<f32> = p1.iter().chain(p2.iter()).map(|p| p.0).collect();
                ss.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                ss.dedup_by(|x, y| (*x - *y).abs() < 1e-5);
                let h = g.ch[i];
                let at = |s: f32| {
                    let xz = g.inset[i].lerp(g.corners[i], s);
                    (xz, h + (f1 - h) * eval_profile(p1, s), h + (f2 - h) * eval_profile(p2, s))
                };
                // Split segments where the two profiles cross.
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
                        if (t[2] - t[0]).cross(t[1] - t[0]).length() > 1e-8 {
                            em.cap(out, t);
                            em.cap(out, [t[0], t[2], t[1]]);
                        }
                    }
                }
            }
        }
        // Vertical strips where this cell is above its neighbour along an edge. HexPrism: the
        // wall is cut at every whole level by a prism groove (chamfer + underside).
        let ep = self.store.edges;
        let gb = (ep.prism_bevel * g.apothem).min(ep.level() * 0.4).min(g.apothem * RAMP_MAX_INSET);
        let grooves: Vec<Vec<f32>> = rels
            .iter()
            .map(|r| {
                if !ep.prism() || !r.exists || r.pts.is_empty() {
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
            let (a, b) = g.edges[e];
            let (ca, cb) = (g.corners[a], g.corners[b]);
            for &y in &grooves[e] {
                self.emit_groove(&g, &em, out, e, y, gb, &grooves);
            }
            // Wall bands between the grooves, top to bottom: (clip top, clip bottom).
            let mut bands: Vec<(f32, f32)> = Vec::new();
            let mut top = f32::MAX;
            for &y in grooves[e].iter().rev() {
                bands.push((top, y));
                top = y - gb;
            }
            bands.push((top, f32::MIN));
            for (bt, bb) in bands {
                let pts: Vec<(f32, f32, f32)> = r.pts.iter().map(|&(t, x, n)| (t, x.min(bt), n.max(bb))).collect();
                for w in pts.windows(2) {
                    let ((t0, x0, n0), (t1, x1, n1)) = (w[0], w[1]);
                    let (d0, d1) = (x0 - n0, x1 - n1);
                    if d0 <= 1e-4 && d1 <= 1e-4 {
                        continue;
                    }
                    // Clip the segment to the part where we are higher.
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
                    em.strip(out, g.center, (qa, s0.1, s0.2), (qb, s1.1, s1.2));
                }
            }
        }
    }

    /// HexPrism groove at level `y` on edge `e`: the chamfer of the prism below (from the wall
    /// at `y - b` in to the groove line at `y`) and the underside of the prism above. At a
    /// corner the groove line meets the next edge's groove, or ends on the next edge's line
    /// with a small closing cap.
    #[allow(clippy::too_many_arguments)]
    fn emit_groove(&self, g: &Geom, em: &Emitter, out: &mut Soup, e: usize, y: f32, b: f32, grooves: &[Vec<f32>]) {
        let (ia, ib) = g.edges[e];
        let inner = |i: usize| -> (Vec2, bool) {
            let (e1, e2) = g.corner_edges[i];
            let eo = if e1 == e { e2 } else { e1 };
            if grooves[eo].iter().any(|z| (z - y).abs() < 1e-4) {
                let (n1, n2) = (g.inward[e1], g.inward[e2]);
                let det = n1.x * n2.y - n1.y * n2.x;
                if det.abs() > 1e-6 {
                    return (g.corners[i] + v2((b * n2.y - b * n1.y) / det, (n1.x * b - n2.x * b) / det), false);
                }
            }
            let (oa, ob) = g.edges[eo];
            let far = if oa == i { ob } else { oa };
            let u = (g.corners[far] - g.corners[i]).normalized();
            (g.corners[i] + u * (b / u.dot(g.inward[e]).max(0.2)), true)
        };
        let (pa, cap_a) = inner(ia);
        let (pb, cap_b) = inner(ib);
        let (ca, cb) = (g.corners[ia], g.corners[ib]);
        let p = |q: Vec2, h: f32| v3(q.x, h, q.y);
        let chamfer = [p(ca, y - b), p(cb, y - b), p(pb, y), p(pa, y)];
        for t in [[chamfer[0], chamfer[1], chamfer[2]], [chamfer[0], chamfer[2], chamfer[3]]] {
            em.wall_tri(out, t, -g.inward[e], 1.0);
        }
        let under = [p(pa, y), p(pb, y), p(cb, y), p(ca, y)];
        for t in [[under[0], under[1], under[2]], [under[0], under[2], under[3]]] {
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
/// Ramps run at most this fraction of the cell apothem into the higher cell. Below 0.5 so
/// two ramps on the edges either side of a flat hex edge never overlap along it.
const RAMP_MAX_INSET: f32 = 0.48;
/// Wall faces meeting at less than ~63 degrees (hex corners, terrace risers) share smoothed
/// normals, so cliff columns shade as one rounded rock face instead of flat panels with a
/// crease at every corner; sharper turns (triangle corners, wall vs floor) keep their crease.
const WALL_SMOOTH_COS: f32 = 0.45;
/// Inner, unblended part of a cell top (fraction of the centre → corner distance).
const INNER_RING: f32 = 0.4;
/// Ridge/ledge factor at the inner ring (so bands reach a little further in).
const INNER_BAND: f32 = 0.35;
/// Height difference along an edge above which it counts as a cliff (bands, no blending).
const BAND_MIN_STEP: f32 = 0.5;

fn corner_w(e1: usize, e2: usize) -> [(i8, f32); 3] {
    [(-1, 1.0 / 3.0), (e1 as i8, 1.0 / 3.0), (e2 as i8, 1.0 / 3.0)]
}

/// Blend weights `(1 - s)·a + s·b`, merged by neighbour id (own first, at most two edges).
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
            out[0].1 += x.1; // more than two neighbours: fold the rest into "own"
        }
    }
    out
}

fn max2(a: Vec2, b: Vec2) -> Vec2 {
    v2(a.x.max(b.x), a.y.max(b.y))
}

struct Geom {
    cell: IVec2,
    center: Vec2,
    corners: Vec<Vec2>,
    /// Top polygon corners (corners moved inward by adjacent ramps).
    inset: Vec<Vec2>,
    edges: Vec<(usize, usize)>,
    corner_edges: Vec<(usize, usize)>,
    nbs: Vec<IVec2>,
    /// Ramp foot height per edge (ramp down into that neighbour).
    ramp: Vec<Option<f32>>,
    /// Profile of each ramp (meaningful where `ramp` is set).
    kind: Vec<RampKind>,
    /// Inward unit normal per edge (XZ).
    inward: Vec<Vec2>,
    apothem: f32,
    h: f32,
    /// Top surface height at each corner.
    ch: Vec<f32>,
    smooth: bool,
    floor: u8,
    wall: u8,
    grass: u8,
}

impl Geom {
    /// Piecewise-linear surface height along edge `e`, parameter 0 at corner a, 1 at b.
    fn profile(&self, e: usize) -> Vec<(f32, f32)> {
        let (a, b) = self.edges[e];
        if self.smooth {
            return vec![(0.0, self.ch[a]), (1.0, self.ch[b])];
        }
        if let Some(f) = self.ramp[e] {
            return vec![(0.0, f), (1.0, f)];
        }
        let len = (self.corners[b] - self.corners[a]).length().max(1e-6);
        let other = |i: usize| {
            let (e1, e2) = self.corner_edges[i];
            if e1 == e { e2 } else { e1 }
        };
        // A ramp on the other edge at a corner runs along this edge from the corner (foot,
        // s = 1) to the inset corner (top, s = 0) with its profile.
        let mut v = Vec::with_capacity(12);
        let (oa, ob) = (other(a), other(b));
        match self.ramp[oa] {
            Some(f) => {
                let d = (self.inset[a] - self.corners[a]).length() / len;
                for &(s, u) in self.kind[oa].profile().iter().rev() {
                    v.push(((1.0 - s) * d, self.h + (f - self.h) * u));
                }
            }
            None => v.push((0.0, self.h)),
        }
        match self.ramp[ob] {
            Some(f) => {
                let d = (self.inset[b] - self.corners[b]).length() / len;
                for &(s, u) in self.kind[ob].profile() {
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
    /// `(t, my height, neighbour height)` at the union of both profiles' breakpoints.
    pts: Vec<(f32, f32, f32)>,
}

/// Floor vertex under construction: blend weights refer to "own" (-1) or an edge's neighbour.
#[derive(Clone, Copy)]
struct FV {
    p: Vec3,
    uv: Vec2,
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
        // Up to two neighbour edges take part in one triangle (slots b and c).
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
                let k = if e < 0 { 0 } else if e == ids[0] { 1 } else if e == ids[1] { 2 } else { 0 };
                wt[k] += w;
            }
            out.verts.push(TVert {
                pos: v.p,
                uv: v.uv,
                uv2: v2(v.p.x / self.cs.x, v.p.z / self.cs.y),
                custom0: [slots[0] as f32, slots[1] as f32, slots[2] as f32, self.wall as f32],
                custom1: [wt[0], wt[1], 0.0, self.grass as f32 / 255.0],
                floor: true,
            });
        }
    }

    fn wall_v(&self, out: &mut Soup, p: Vec3, kind: f32) {
        let w = self.wall as f32;
        out.verts.push(TVert {
            pos: p,
            uv: v2(1.0, 1.0),
            uv2: v2(p.x / self.cs.x, p.z / self.cs.y),
            custom0: [w, w, w, w],
            custom1: [1.0, 0.0, kind, 0.0],
            floor: false,
        });
    }

    /// Wall-textured triangle wound to face `outward` (XZ direction).
    fn wall_tri(&self, out: &mut Soup, t: [Vec3; 3], outward: Vec2, kind: f32) {
        let n = (t[2] - t[0]).cross(t[1] - t[0]);
        if n.length() <= 1e-8 {
            return;
        }
        let t = if v2(n.x, n.z).dot(outward) < 0.0 { [t[0], t[2], t[1]] } else { t };
        for p in t {
            self.wall_v(out, p, kind);
        }
    }

    /// Wall-textured triangle wound to face `dir`.
    fn tri_facing(&self, out: &mut Soup, t: [Vec3; 3], dir: Vec3, kind: f32) {
        let n = (t[2] - t[0]).cross(t[1] - t[0]);
        if n.length() <= 1e-8 {
            return;
        }
        let t = if n.dot(dir) < 0.0 { [t[0], t[2], t[1]] } else { t };
        for p in t {
            self.wall_v(out, p, kind);
        }
    }

    /// Closing geometry inside a cell (marked 0.75: wall-textured, not an outer wall).
    fn cap(&self, out: &mut Soup, t: [Vec3; 3]) {
        for p in t {
            self.wall_v(out, p, 0.75);
        }
    }

    /// Vertical strip on the edge line between `(qa, top, bottom)` and `(qb, top, bottom)`,
    /// facing away from `center`.
    fn strip(&self, out: &mut Soup, center: Vec2, a: (Vec2, f32, f32), b: (Vec2, f32, f32)) {
        self.strip_kind(out, center, a, b, 1.0);
    }

    /// [`Emitter::strip`] with an explicit wall kind (`CUSTOM1.z`).
    fn strip_kind(&self, out: &mut Soup, center: Vec2, a: (Vec2, f32, f32), b: (Vec2, f32, f32), kind: f32) {
        let (mut a, mut b) = (a, b);
        let dir = b.0 - a.0;
        if v2(dir.y, -dir.x).dot((a.0 + b.0) * 0.5 - center) < 0.0 {
            std::mem::swap(&mut a, &mut b);
        }
        let a_low = v3(a.0.x, a.2, a.0.y);
        let b_low = v3(b.0.x, b.2, b.0.y);
        let a_high = v3(a.0.x, a.1, a.0.y);
        let b_high = v3(b.0.x, b.1, b.0.y);
        // Clockwise as seen from outside (Godot front faces); skip degenerate halves.
        for t in [[a_low, b_high, a_high], [a_low, b_low, b_high]] {
            if (t[2] - t[0]).cross(t[1] - t[0]).length() > 1e-8 {
                for p in t {
                    self.wall_v(out, p, kind);
                }
            }
        }
    }
}

/// Meshes one cell-mode chunk into `out`.
pub fn mesh_cell_chunk(store: &TerrainStore, chunk: IVec2, out: &mut Soup) {
    let cfg = &store.config;
    if !store.chunks.contains_key(&chunk) {
        return;
    }
    let mut ctx = Ctx::new(store);
    out.cell_area = cell_area(cfg, chunk);
    // Prism columns are faceted: flat side and bevel normals (crisp prism edges).
    out.wall_smooth_cos = if store.edges.prism() { None } else { Some(WALL_SMOOTH_COS) };
    let cells = cfg.chunk_cells(chunk);
    let mut outside: Vec<IVec2> = Vec::new();
    for &g in &cells {
        out.begin_cell(g);
        ctx.emit_cell(g, out, true);
        out.end_cell();
        for nb in cfg.edge_neighbors(g) {
            if cfg.chunk_of(nb) != chunk && !outside.contains(&nb) {
                outside.push(nb);
            }
        }
    }
    // Every cell sharing a corner with a chunk cell is one of its edge neighbours: their
    // floors and walls make the border vertex normals identical on both sides of the seam.
    let mut extra = Soup::default();
    for nb in outside {
        ctx.emit_cell(nb, &mut extra, true);
    }
    out.normal_only = extra.verts;
}

/// Wall kind (`CUSTOM1.z`) of low-detail walls: still a wall (> 0.5) for the shader, but
/// below the 0.7 cut that enables rock strata, so distant columns shade as plain rock.
pub const LOD_WALL: f32 = 0.6;

/// Meshes one cell-mode chunk at low detail (the distant ring): one flat polygon per cell at
/// its height (own floor slot, no blending, no ridge / ledge bands, no grass flag), and one
/// plain vertical quad per edge down to each lower neighbour, marked [`LOD_WALL`]. No ramps,
/// terraces, smooth planes, prism bevels or grooves; HexPrism keeps its border skirt so the
/// columns never look hollow. Seams with full-detail chunks are exact for HexPrism and for
/// cliffs; next to ramps or smooth planes the plain walls only approximate the full surface.
pub fn mesh_cell_chunk_low(store: &TerrainStore, chunk: IVec2, out: &mut Soup) {
    mesh_cell_chunk_flat(store, chunk, out, false, LOD_WALL);
}

/// [`mesh_cell_chunk_low`] with the cells' grass flags kept and walls marked `wall_kind`: the
/// collision / grass surface of GPU-instanced prism chunks ([`crate::prism`]), which are
/// drawn by the instanced columns rather than by this mesh.
pub fn mesh_cell_chunk_flat(store: &TerrainStore, chunk: IVec2, out: &mut Soup, grass: bool, wall_kind: f32) {
    let cfg = &store.config;
    if !store.chunks.contains_key(&chunk) {
        return;
    }
    let cs = cfg.cell_size_safe();
    out.cell_area = cell_area(cfg, chunk);
    out.wall_smooth_cos = None;
    let ep = store.edges;
    for c in cfg.chunk_cells(chunk) {
        out.begin_cell(c);
        if let Some(s) = store.sample(c).filter(|s| s.floor != VOID_SLOT) {
            let em = Emitter { cs, wall: s.wall, grass: if grass { s.grass } else { 0 }, own: s.floor };
            let own = |_e: i8| s.floor;
            let center = cfg.sample_pos(c);
            let corners = cfg.cell_corners(c);
            let fv = |p: Vec2| FV { p: v3(p.x, s.height, p.y), uv: Vec2::ZERO, w: OWN };
            for i in 1..corners.len().saturating_sub(1) {
                em.floor_tri(out, &own, fv(corners[0]), fv(corners[i]), fv(corners[i + 1]));
            }
            for (e, nb) in cfg.edge_neighbors(c).into_iter().enumerate() {
                let base = match store.sample(nb) {
                    Some(ns) => ns.height,
                    None if ep.prism() => s.height - ep.prism_skirt * ep.level(),
                    None => continue,
                };
                if s.height - base <= 1e-4 {
                    continue;
                }
                let (a, b) = cfg.edge_corners(c, e);
                em.strip_kind(out, center, (corners[a], s.height, base), (corners[b], s.height, base), wall_kind);
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

/// Top-decoration spots of a hex cell read as a kg_hex *large* hex: the 7 whole *small* hexes
/// it contains (the small lattice is the large one scaled by 1/3 in axial space: the centre and
/// its 6 neighbours at a third of the cell width), as terrain-local positions on the surface.
/// A spot is skipped where the ground under it is uneven (ramp, terrace, slope, smooth plane)
/// or at / below `min_height` (submerged); the whole cell is skipped when it is void. Paved /
/// built-on cells are the game's business. Triangle grids give the centre only.
pub fn decoration_spots(store: &TerrainStore, c: IVec2, min_height: f32) -> Vec<Vec3> {
    decoration_spots_ex(store, c, min_height, &|_, _| false).into_iter().map(|(_, p)| p).collect()
}

/// [`decoration_spots`] with the small hex of each spot (kg_hex `SmallHex` coordinates on the
/// 3× finer axial lattice; centre spot = `3·cell` on hex rings) and a game filter: `skip(small,
/// spot)` drops paved / built-on / flooded small hexes.
pub fn decoration_spots_ex(store: &TerrainStore, c: IVec2, min_height: f32, skip: &dyn Fn(IVec2, Vec3) -> bool) -> Vec<(IVec2, Vec3)> {
    let Some(s) = store.sample(c) else { return Vec::new() };
    if s.floor == VOID_SLOT {
        return Vec::new();
    }
    let cfg = &store.config;
    let center = cfg.sample_pos(c);
    let corners = cfg.cell_corners(c);
    let mut spots = vec![center];
    if matches!(cfg.grid, crate::grid::GridType::Hex | crate::grid::GridType::HexRings) {
        for e in 0..corners.len() {
            let (a, b) = cfg.edge_corners(c, e);
            spots.push(center + ((corners[a] + corners[b]) * 0.5 - center) * (2.0 / 3.0));
        }
    }
    // Probe the small hex around each spot (half its apothem out) for an even surface.
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

/// Height of the meshed surface (tops, ramps, smooth planes) of the cell under a
/// terrain-local XZ point; `None` outside the terrain or over void cells.
pub fn surface_height(store: &TerrainStore, p: Vec2) -> Option<f32> {
    let c = store.config.world_to_sample(p);
    let s = store.sample(c)?;
    if s.floor == VOID_SLOT {
        return None;
    }
    let mut ctx = Ctx::new(store);
    // Fast path (the common case for raycasts / props): a flat cell without ramps.
    if !s.smooth {
        let merge = ctx.merge_of(c);
        let ramps = store.config.edge_neighbors(c).into_iter().any(|nb| {
            (store.edges.prism() && store.sample(nb).is_none())
                || store.sample(nb).is_some_and(|ns| {
                !ns.smooth && ns.floor != VOID_SLOT && classify(store.edges, s.height, ns.height, merge, f32::MAX).is_some()
            })
        });
        if !ramps {
            return Some(s.height);
        }
    }
    let mut soup = Soup::default();
    ctx.emit_cell(c, &mut soup, false);
    let mut best: Option<f32> = None;
    for t in soup.verts.chunks_exact(3) {
        if let Some(h) = height_in_tri(t[0].pos, t[1].pos, t[2].pos, p) {
            best = Some(best.map_or(h, |b: f32| b.max(h)));
        }
    }
    Some(best.unwrap_or(s.height))
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
        let mut s = TerrainStore::new(GridConfig { grid, chunk_size: 4, ring_radius: 3, ..Default::default() });
        s.add_chunk(iv2(0, 0), MergeMode::Cubic);
        s
    }

    fn hex(merge: MergeMode) -> (TerrainStore, IVec2) {
        let mut s = store(GridType::HexRings);
        s.chunks.get_mut(&iv2(0, 0)).unwrap().merge_mode = merge;
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        (s, c)
    }

    fn mesh(s: &TerrainStore) -> Soup {
        let mut soup = Soup::default();
        mesh_cell_chunk(s, iv2(0, 0), &mut soup);
        soup
    }

    fn cell_soup<'a>(soup: &'a Soup, c: IVec2) -> &'a [TVert] {
        let ct = soup.cells.iter().find(|x| x.cell == c).unwrap();
        &soup.verts[ct.start..ct.end]
    }

    /// Area of vertical (wall) triangles.
    fn wall_area(v: &[TVert], kind: f32) -> f32 {
        v.chunks_exact(3)
            .filter(|t| !t[0].floor && (t[0].custom1[2] - kind).abs() < 1e-3)
            .map(|t| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).length() * 0.5)
            .sum()
    }

    #[test]
    fn flat_cells_emit_only_tops_facing_up() {
        for g in [GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let s = store(g);
            let soup = mesh(&s);
            assert!(soup.verts.iter().all(|v| v.floor), "{g:?}");
            let m = soup.build();
            assert!(m.normals.iter().all(|n| n[1] > 0.99), "{g:?} normals up");
        }
    }

    #[test]
    fn cliff_cell_gets_outward_walls_of_full_area_and_bands() {
        for g in [GridType::Hex, GridType::HexRings, GridType::Triangle] {
            let mut s = store(g);
            let local = if g == GridType::HexRings { iv2(3, 3) } else { iv2(2, 1) };
            let c = s.config.global_of(iv2(0, 0), local);
            s.set_height(c, 3.0);
            let soup = mesh(&s);
            let v = cell_soup(&soup, c);
            let corners = s.config.cell_corners(c);
            let perim: f32 = (0..corners.len()).map(|i| (corners[(i + 1) % corners.len()] - corners[i]).length()).sum();
            assert!((wall_area(v, 1.0) - perim * 3.0).abs() < 1e-3, "{g:?} wall area {}", wall_area(v, 1.0));
            let center = s.config.sample_pos(c);
            let m = soup.build();
            for (i, n) in m.normals.iter().enumerate() {
                if m.custom1[i][2] > 0.9 {
                    let p = m.positions[i];
                    assert!(v2(p[0] - center.x, p[2] - center.y).dot(v2(n[0], n[2])) > 0.0, "{g:?} wall faces inward");
                }
            }
            // Cliff top: ridge band on the rim; neighbours get a ledge band at the cliff foot.
            assert!(v.iter().any(|x| x.floor && x.uv.y > 0.99));
            let nb = s.config.edge_neighbors(c)[0];
            assert!(cell_soup(&soup, nb).iter().any(|x| x.floor && x.uv.x > 0.99), "{g:?} ledge");
        }
    }

    #[test]
    fn ramps_are_floor_inside_the_higher_cell_and_meet_the_lower_floor() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.set_height(c, 1.0);
        s.edit_sample(c, |d, i| d.floor_tex[i] = 4);
        let soup = mesh(&s);
        let v = cell_soup(&soup, c);
        assert!(v.iter().all(|x| x.floor), "no walls: ramps on all sides");
        assert!(v.iter().all(|x| x.custom0[0] == 4.0), "ramp uses the higher cell's floor slot");
        // Everything stays inside the raised hexagon.
        let center = s.config.sample_pos(c);
        let r = s.config.hex_radius();
        assert!(v.iter().all(|x| (x.pos.xz() - center).length() <= r + 1e-4));
        // Lower neighbours: flat tops only at height 0.
        for nb in s.config.edge_neighbors(c) {
            assert!(cell_soup(&soup, nb).iter().all(|x| x.floor && x.pos.y.abs() < 1e-5));
        }
        // The surface descends from the top to the foot on the shared edge.
        let corners = s.config.cell_corners(c);
        let mid = (corners[0] + corners[1]) * 0.5;
        assert!((surface_height(&s, center).unwrap() - 1.0).abs() < 1e-4);
        assert!(surface_height(&s, center.lerp(mid, 0.999)).unwrap() < 0.05);
        let h_half = surface_height(&s, center.lerp(mid, 0.8)).unwrap();
        assert!(h_half > 0.05 && h_half < 0.95, "on the ramp: {h_half}");
        // Ramps get grass like floors.
        let gp = GrassParams { slot_has_grass: [true; 16], ..Default::default() };
        let blades = scatter(&soup, &gp);
        assert!(blades.iter().any(|b| b.pos.y > 0.05 && b.pos.y < 0.95), "grass on ramps");
    }

    #[test]
    fn mixed_steps_close_with_strips_and_wedges() {
        // Cell at 2 between a 1-lower (ramp) and 2-lower (wall) neighbour and an equal one.
        let (mut s, c) = hex(MergeMode::Polyhedron);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(c, 2.0);
        s.set_height(nbs[1], 2.0);
        s.set_height(nbs[2], 1.0);
        s.set_height(nbs[3], 1.0);
        s.set_height(nbs[4], 0.8);
        let soup = mesh(&s);
        // Top-down coverage: every point of the chunk interior is under some floor triangle.
        let center = s.config.sample_pos(c);
        let w = s.config.cell_size.x;
        for j in 0..40 {
            for i in 0..40 {
                let p = center + v2((i as f32 / 39.0 - 0.5) * 3.0 * w, (j as f32 / 39.0 - 0.5) * 3.0 * w);
                let hit = soup.verts.chunks_exact(3).any(|t| t[0].floor && height_in_tri(t[0].pos, t[1].pos, t[2].pos, p).is_some());
                assert!(hit, "hole at {p:?}");
            }
        }
        // Vertical closing: the gaps between neighbouring surfaces along every edge of `c`
        // are covered by strips (area check against the profile integral).
        let mut ctx = Ctx::new(&s);
        let g = ctx.geom(c).unwrap();
        let mut expect = 0.0;
        for e in 0..6 {
            let r = ctx.relation(&g, e);
            let (a, b) = g.edges[e];
            let len = (g.corners[b] - g.corners[a]).length();
            for wdw in r.pts.windows(2) {
                let (d0, d1) = ((wdw[0].1 - wdw[0].2).max(0.0), (wdw[1].1 - wdw[1].2).max(0.0));
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
        assert!(expect > 0.5 && (got - expect).abs() < 1e-3, "strips {got} vs {expect}");
    }

    #[test]
    fn slots_blend_across_flat_borders_but_not_cliffs() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        let nb = s.config.edge_neighbors(c)[0];
        s.edit_sample(nb, |d, i| d.floor_tex[i] = 3);
        let soup = mesh(&s);
        let blended = cell_soup(&soup, c).iter().any(|v| v.custom0[1] == 3.0 && v.custom1[1] > 0.4);
        assert!(blended, "edge midpoint mixes in the neighbour slot");
        s.set_height(nb, 5.0);
        let soup = mesh(&s);
        assert!(cell_soup(&soup, c).iter().filter(|v| v.floor).all(|v| v.custom0[..3].iter().all(|m| *m == 0.0)));
    }

    #[test]
    fn hex_grass_covers_the_whole_cell() {
        let s = store(GridType::HexRings);
        let soup = mesh(&s);
        let gp = GrassParams { subdivisions: 3, slot_has_grass: [true; 16], ..Default::default() };
        let cells = soup.cells.len() as f32;
        let per_cell = scatter(&soup, &gp).len() as f32 / cells;
        assert!((per_cell - 9.0).abs() < 0.9, "{per_cell} blades per cell");
        assert!((soup.cell_area - SQRT3 * 0.5 * 4.0).abs() < 1e-3);
    }

    #[test]
    fn floor_normals_match_across_chunk_seams() {
        let mut s = store(GridType::HexRings);
        s.add_chunk(iv2(1, 0), MergeMode::Cubic);
        // A smooth, bumpy beach spanning both chunks.
        for ch in [iv2(0, 0), iv2(1, 0)] {
            for c in s.config.chunk_cells(ch) {
                let p = s.config.sample_pos(c);
                s.set_height(c, (p.x * 0.7).sin() * 0.8 + p.y * 0.1);
                s.edit_sample(c, |d, i| d.smooth[i] = 1);
            }
        }
        let build = |ch: IVec2| {
            let mut soup = Soup::default();
            mesh_cell_chunk(&s, ch, &mut soup);
            soup.build()
        };
        let (a, b) = (build(iv2(0, 0)), build(iv2(1, 0)));
        let key = |p: [f32; 3]| ((p[0] * 512.0).round() as i32, (p[1] * 512.0).round() as i32, (p[2] * 512.0).round() as i32);
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
            s.edit_sample(nb, |d, i| d.smooth[i] = 1);
        }
        let soup = mesh(&s);
        let center = s.config.sample_pos(c);
        let corners = s.config.cell_corners(c);
        let ctx_h = surface_height(&s, corners[0].lerp(center, 0.01)).unwrap();
        // Corner height = mean of the three smooth cells sharing it.
        let expect = (0.0 + 0.2 * 6.0 + 0.2 * 1.0) / 3.0;
        assert!((ctx_h - expect).abs() < 0.02, "{ctx_h} vs {expect}");
        assert!(!soup.verts.is_empty());
    }

    const STYLES: [EdgeStyle; 4] = [EdgeStyle::FlatWalls, EdgeStyle::Terraces, EdgeStyle::Slopes, EdgeStyle::HexPrism];

    /// Segment / triangle intersection (two-sided, with a small tolerance on the edges).
    fn seg_hits(p: Vec3, q: Vec3, t: &[TVert]) -> bool {
        let (a, b, c) = (t[0].pos, t[1].pos, t[2].pos);
        let d = q - p;
        let (e1, e2) = (b - a, c - a);
        let h = d.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-9 {
            return false;
        }
        let f = 1.0 / det;
        let s = p - a;
        let u = f * s.dot(h);
        let qv = s.cross(e1);
        let v = f * d.dot(qv);
        let tt = f * e2.dot(qv);
        let eps = 1e-4;
        u >= -eps && v >= -eps && u + v <= 1.0 + eps && (-eps..=1.0 + eps).contains(&tt)
    }

    /// Random terraced heights (multiples of 0.5: ramps, terraces and cliffs) on one chunk.
    fn bumpy(grid: GridType, style: EdgeStyle, seed: u32) -> TerrainStore {
        let mut s = store(grid);
        s.edges.style = style;
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

    #[test]
    fn every_style_is_watertight() {
        // Slanted segments from above the terrain to below it must cross the mesh somewhere:
        // a crack between ramps, terraces, caps and walls lets some of them through.
        for grid in [GridType::HexRings, GridType::Triangle] {
            for style in STYLES {
                for seed in [1, 7, 23] {
                    let s = bumpy(grid, style, seed);
                    let soup = mesh(&s);
                    let cells = s.config.chunk_cells(iv2(0, 0));
                    let mid = cells.iter().map(|c| s.config.sample_pos(*c)).fold(Vec2::ZERO, |a, p| a + p) * (1.0 / cells.len() as f32);
                    let w = s.config.cell_size.x;
                    let reach = if grid == GridType::HexRings { 1.6 * w } else { 1.2 * w };
                    let (mut misses, mut tested) = (0, 0);
                    for k in 0..4000u32 {
                        let r = |j: u32| hash01(iv2(k as i32, j as i32), seed) * 2.0 - 1.0;
                        let a = mid + v2(r(1), r(2)) * reach;
                        let b = a + v2(r(3), r(4)) * (0.6 * w);
                        // Stay away from the open chunk border.
                        let inner = |x: Vec2| {
                            let c = s.config.world_to_sample(x);
                            s.sample(c).is_some() && s.config.edge_neighbors(c).iter().all(|n| s.sample(*n).is_some())
                        };
                        if !inner(a) || !inner(b) || !inner(a.lerp(b, 0.5)) {
                            continue;
                        }
                        tested += 1;
                        let (p, q) = (v3(a.x, 6.0, a.y), v3(b.x, -2.0, b.y));
                        if !soup.verts.chunks_exact(3).any(|t| seg_hits(p, q, t)) {
                            if misses < 3 {
                                let c = s.config.world_to_sample(a);
                                let hs: Vec<(IVec2, f32)> = s.config.edge_neighbors(c).iter().map(|n| (*n, s.height(*n).unwrap_or(-9.0))).collect();
                                eprintln!("miss {a:?}->{b:?} cell {c:?} h {:?} nbs {hs:?}", s.height(c));
                            }
                            misses += 1;
                        }
                    }
                    assert!(tested > 500, "{grid:?}: {tested} segments");
                    assert_eq!(misses, 0, "{grid:?} {style:?} seed {seed}: {misses} segments slipped through");
                }
            }
        }
    }

    #[test]
    fn height_queries_match_the_mesh() {
        for style in STYLES {
            let s = bumpy(GridType::HexRings, style, 5);
            let soup = mesh(&s);
            let center = s.config.sample_pos(s.config.global_of(iv2(0, 0), iv2(3, 3)));
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
                assert!((got - want).abs() < 2e-3, "{style:?} at {p:?}: query {got} vs mesh {want}");
            }
        }
    }

    #[test]
    fn terraces_step_down_in_treads_with_rock_risers() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.edges.style = EdgeStyle::Terraces;
        s.edges.terrace_step = 1.25;
        s.set_height(c, 1.2);
        let soup = mesh(&s);
        let v = cell_soup(&soup, c);
        // Risers are rock (closing geometry), treads are floor; no outer cliff walls.
        assert!(wall_area(v, 0.75) > 0.1, "rock risers");
        assert!(wall_area(v, 1.0) < 1e-6, "no cliff walls on a terrace step");
        // Walking from the centre to a neighbour's centre: top, a tread in each cell, foot.
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
            assert!(levels.iter().any(|l| (l - want).abs() < 1e-3), "plateau {want} in {levels:?}");
        }
        assert!(levels.windows(2).all(|w| w[1] <= w[0] + 1e-4), "monotonic descent {levels:?}");
        // The lower half (and its rock riser) lives in the neighbour.
        assert!(wall_area(cell_soup(&soup, nb), 0.75) > 0.01);
        assert!(cell_soup(&soup, nb).iter().any(|x| x.floor && (x.pos.y - 0.4).abs() < 1e-4));
        // Treads carry grass; a step of more than one level ramps or is a cliff.
        let gp = GrassParams { slot_has_grass: [true; 16], subdivisions: 4, ..Default::default() };
        let blades = scatter(&soup, &gp);
        assert!(blades.iter().any(|b| (b.pos.y - 0.4).abs() < 1e-3), "grass on the lower tread");
        assert!(blades.iter().any(|b| (b.pos.y - 0.8).abs() < 1e-3), "grass on the upper tread");
        s.set_height(c, 3.0);
        let soup = mesh(&s);
        assert!(wall_area(cell_soup(&soup, c), 1.0) > 1.0, "cliff");
    }

    #[test]
    fn slopes_turn_cliffs_into_sloped_rock_faces() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.edges.style = EdgeStyle::Slopes;
        s.set_height(c, 3.0);
        let soup = mesh(&s);
        let v = cell_soup(&soup, c);
        let faces: Vec<&[TVert]> = v.chunks_exact(3).filter(|t| !t[0].floor && t[0].custom1[2] > 0.9).collect();
        assert!(!faces.is_empty());
        for t in &faces {
            let n = (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).normalized();
            assert!(n.y > 0.1, "battered, not vertical: {n:?}");
        }
        assert!(v.iter().any(|x| x.floor && x.uv.y > 0.99), "ridge band on the cliff top");
        let nb = s.config.edge_neighbors(c)[0];
        assert!(cell_soup(&soup, nb).iter().any(|x| x.floor && x.uv.x > 0.99), "ledge band at the foot");
        // Everything stays inside the raised cell; neighbours stay flat.
        let center = s.config.sample_pos(c);
        assert!(v.iter().all(|x| (x.pos.xz() - center).length() <= s.config.hex_radius() + 1e-4));
        assert!(cell_soup(&soup, nb).iter().all(|x| x.pos.y.abs() < 1e-5));
        // Small steps still ramp.
        s.set_height(c, 1.0);
        let soup = mesh(&s);
        assert!(cell_soup(&soup, c).iter().all(|x| x.floor));
    }

    #[test]
    fn cliff_corners_share_smoothed_wall_normals() {
        let (mut s, c) = hex(MergeMode::Cubic);
        s.set_height(c, 3.0);
        let m = mesh(&s).build();
        let corners = s.config.cell_corners(c);
        for k in corners {
            let at: Vec<[f32; 3]> = (0..m.positions.len())
                .filter(|&i| m.custom1[i][2] > 0.9 && (v2(m.positions[i][0], m.positions[i][2]) - k).length() < 1e-4)
                .map(|i| m.normals[i])
                .collect();
            assert!(at.len() >= 2, "corner has wall verts");
            for n in &at {
                let d = n[0] * at[0][0] + n[1] * at[0][1] + n[2] * at[0][2];
                assert!(d > 0.9999, "one normal per wall corner: {at:?}");
            }
            // The shared normal points out through the corner.
            let center = s.config.sample_pos(c);
            let out = (k - center).normalized();
            assert!(out.dot(v2(at[0][0], at[0][2])) > 0.99);
        }
    }

    #[test]
    fn wall_normals_match_across_chunk_seams() {
        // Two raised cells side by side, one in each chunk: their cliff walls meet at corners
        // on the chunk seam, which both chunk meshes must shade identically.
        let mut s = store(GridType::HexRings);
        s.add_chunk(iv2(1, 0), MergeMode::Cubic);
        let (x, y) = s
            .config
            .chunk_cells(iv2(0, 0))
            .into_iter()
            .find_map(|x| s.config.edge_neighbors(x).into_iter().find(|y| s.config.chunk_of(*y) == iv2(1, 0)).map(|y| (x, y)))
            .unwrap();
        s.set_height(x, 2.0);
        s.set_height(y, 2.0);
        let build = |ch: IVec2| {
            let mut soup = Soup::default();
            mesh_cell_chunk(&s, ch, &mut soup);
            soup.build()
        };
        let (a, b) = (build(iv2(0, 0)), build(iv2(1, 0)));
        let key = |p: [f32; 3]| ((p[0] * 512.0).round() as i32, (p[1] * 512.0).round() as i32, (p[2] * 512.0).round() as i32);
        let walls = |m: &crate::mesh::MeshArrays| -> HashMap<(i32, i32, i32), [f32; 3]> {
            (0..m.positions.len()).filter(|&i| m.custom1[i][2] > 0.9).map(|i| (key(m.positions[i]), m.normals[i])).collect()
        };
        let (wa, wb) = (walls(&a), walls(&b));
        let mut shared = 0;
        for (k, n) in &wa {
            if let Some(m) = wb.get(k) {
                shared += 1;
                assert!(n[0] * m[0] + n[1] * m[1] + n[2] * m[2] > 0.9999, "seam wall normal {n:?} vs {m:?}");
            }
        }
        assert!(shared >= 2, "walls meet on the seam ({shared})");
    }

    #[test]
    fn hex_prism_columns_are_flat_topped_with_grooves_per_level() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.edges.style = EdgeStyle::HexPrism;
        s.edges.terrace_step = 0.5;
        // A 0.5 step would ramp in FlatWalls (Polyhedron 1.3); prisms always step.
        s.set_height(c, 3.0);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(nbs[0], 0.5);
        // Smooth flags are ignored: every prism top is flat.
        s.edit_sample(nbs[3], |d, i| d.smooth[i] = 1);
        s.set_height(nbs[3], 0.25);
        let soup = mesh(&s);
        let v = cell_soup(&soup, c);
        let tris: Vec<&[TVert]> = v.chunks_exact(3).collect();
        let nrm = |t: &[TVert]| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).normalized();
        // Floors: flat at the column top.
        for t in tris.iter().filter(|t| t[0].floor) {
            assert!(t.iter().all(|x| (x.pos.y - 3.0).abs() < 1e-5), "flat top");
        }
        // Walls: vertical sides, 45 degree chamfers (top bevel + one per level), undersides.
        let walls: Vec<Vec3> = tris.iter().filter(|t| !t[0].floor && t[0].custom1[2] > 0.9).map(|t| nrm(t)).collect();
        assert!(walls.iter().any(|n| n.y.abs() < 1e-3), "vertical sides");
        assert!(walls.iter().any(|n| (n.y - 0.7071).abs() < 0.02), "chamfers");
        assert!(walls.iter().any(|n| n.y < -0.99), "groove undersides");
        assert!(walls.iter().all(|n| n.y.abs() < 1e-3 || (n.y - 0.7071).abs() < 0.02 || n.y < -0.99), "{walls:?}");
        // One groove per whole level strictly inside each wall: towards the 0-high neighbours
        // at 0.5, 1.0, ... 2.5 (the top level 3.0 is the rim bevel).
        let center = s.config.sample_pos(c);
        let corners = s.config.cell_corners(c);
        let edge_out = |e: usize| {
            let (a, b) = s.config.edge_corners(c, e);
            ((corners[a] + corners[b]) * 0.5 - center).normalized()
        };
        let under_levels = |e_out: Vec2| -> Vec<f32> {
            let mut ys: Vec<f32> = tris
                .iter()
                .filter(|t| !t[0].floor && nrm(t).y < -0.99)
                .filter(|t| ((t[0].pos.xz() + t[1].pos.xz() + t[2].pos.xz()) * (1.0 / 3.0) - center).normalized().dot(e_out) > 0.9)
                .map(|t| (t[0].pos.y * 100.0).round() / 100.0)
                .collect();
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ys.dedup();
            ys
        };
        let e0 = (0..6).find(|&e| s.config.edge_neighbors(c)[e] == nbs[0]).unwrap();
        assert_eq!(under_levels(edge_out(e0)), vec![1.0, 1.5, 2.0, 2.5], "wall down to the 0.5 neighbour");
        assert_eq!(under_levels(edge_out((e0 + 1) % 6)), vec![0.5, 1.0, 1.5, 2.0, 2.5]);
        // Everything stays inside the column's hexagon; the smooth neighbour stays flat.
        assert!(v.iter().all(|x| (x.pos.xz() - center).length() <= s.config.hex_radius() + 1e-4));
        assert!(cell_soup(&soup, nbs[3]).iter().filter(|x| x.floor).all(|x| (x.pos.y - 0.25).abs() < 1e-5));
        // Faceted: wall normals are flat (no smoothing across prism corners).
        assert!(soup.wall_smooth_cos.is_none());
        // Height queries see the flat top and the bevel.
        assert!((surface_height(&s, center).unwrap() - 3.0).abs() < 1e-4);
        let (a, b) = s.config.edge_corners(c, (e0 + 1) % 6);
        let rim = center.lerp((corners[a] + corners[b]) * 0.5, 0.999);
        assert!(surface_height(&s, rim).unwrap() < 3.0 - 0.05);
    }

    #[test]
    fn decoration_spots_are_the_seven_small_hexes_on_even_dry_ground() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.set_height(c, 2.0);
        // A prism column: flat top, all seven small hexes decoratable.
        s.edges.style = EdgeStyle::HexPrism;
        let spots = decoration_spots(&s, c, 0.0);
        assert_eq!(spots.len(), 7);
        let center = s.config.sample_pos(c);
        let w = s.config.cell_size.x;
        for p in &spots[1..] {
            assert!(((p.xz() - center).length() - w / 3.0).abs() < 1e-4, "ring at a third of the width");
            assert!((p.y - 2.0).abs() < 1e-5);
        }
        // kg_hex: small hexes scaled x3 in axial space -> the ring is the small-hex neighbour lattice.
        for nb in s.config.edge_neighbors(c) {
            let dir = (s.config.sample_pos(nb) - center).normalized();
            assert!(spots[1..].iter().any(|p| (p.xz() - center).normalized().dot(dir) > 0.9999));
        }
        // Ramps make the outer small hexes uneven (FlatWalls, 1 m step < Polyhedron 1.3).
        s.edges.style = EdgeStyle::FlatWalls;
        s.set_height(c, 1.0);
        let spots = decoration_spots(&s, c, -5.0);
        assert_eq!(spots.len(), 1, "only the centre stays flat: {spots:?}");
        // Submerged cells get nothing.
        assert!(decoration_spots(&s, c, 1.5).is_empty());
    }

    #[test]
    fn hex_prism_border_columns_get_a_skirt() {
        let mut s = store(GridType::HexRings);
        s.edges.style = EdgeStyle::HexPrism;
        s.edges.terrace_step = 0.5;
        s.edges.prism_skirt = 4.0;
        let soup = mesh(&s);
        let low = soup.verts.iter().filter(|x| !x.floor).map(|x| x.pos.y).fold(f32::MAX, f32::min);
        assert!((low + 2.0).abs() < 1e-4, "skirt 4 levels deep: {low}");
        // Interior cells of a flat chunk emit no walls.
        let (_, c) = hex(MergeMode::Cubic);
        assert!(cell_soup(&soup, c).iter().all(|x| x.floor));
    }

    #[test]
    fn low_detail_cells_are_flat_tops_with_plain_walls_and_no_grass() {
        let (mut s, c) = hex(MergeMode::Polyhedron);
        s.edges.style = EdgeStyle::HexPrism;
        s.edges.terrace_step = 0.5;
        s.edges.prism_skirt = 4.0;
        s.set_height(c, 3.0);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(nbs[0], 1.0);
        for i in 0..s.config.chunk_cells(iv2(0, 0)).len() {
            let cell = s.config.chunk_cells(iv2(0, 0))[i];
            s.edit_sample(cell, |d, k| d.grass[k] = 255);
        }
        let full = mesh(&s);
        let mut low = Soup::default();
        mesh_cell_chunk_low(&s, iv2(0, 0), &mut low);
        // Far fewer triangles than the full prism mesh (no rims, bevels, grooves).
        assert!(low.triangle_count() * 3 < full.triangle_count(), "{} vs {}", low.triangle_count(), full.triangle_count());
        let v = cell_soup(&low, c);
        // Flat top at the cell height, own slot only.
        for t in v.chunks_exact(3).filter(|t| t[0].floor) {
            for x in t {
                assert!((x.pos.y - 3.0).abs() < 1e-5);
                assert_eq!(x.custom1[0], 1.0);
                assert_eq!(x.custom1[3], 0.0, "no grass flag");
                assert_eq!(x.uv, Vec2::ZERO, "no bands");
            }
        }
        // Walls: vertical, LOD-marked (no strata), area = edge length x drop.
        let walls: Vec<&[TVert]> = v.chunks_exact(3).filter(|t| !t[0].floor).collect();
        assert!(walls.iter().all(|t| t.iter().all(|x| (x.custom1[2] - LOD_WALL).abs() < 1e-6)));
        let edge = s.config.hex_radius();
        let area = wall_area(v, LOD_WALL);
        assert!((area - edge * (2.0 + 5.0 * 3.0)).abs() < 1e-3, "{area}");
        // No grass on low-detail chunks, whatever the cell flags say.
        assert!(scatter(&low, &GrassParams { slot_has_grass: [true; 16], ..Default::default() }).is_empty());
        // The border keeps the HexPrism skirt (4 levels of 0.5 below the flat tops at 0).
        let bottom = low.verts.iter().filter(|x| !x.floor).map(|x| x.pos.y).fold(f32::MAX, f32::min);
        assert!((bottom + 2.0).abs() < 1e-4, "{bottom}");
        // Other styles have no skirt: walls never go below the lowest cell (0).
        let mut ls = Soup::default();
        s.edges.style = EdgeStyle::FlatWalls;
        mesh_cell_chunk_low(&s, iv2(0, 0), &mut ls);
        assert!(ls.verts.iter().filter(|x| !x.floor).all(|x| x.pos.y >= -1e-5), "no skirt outside HexPrism");
    }
}
