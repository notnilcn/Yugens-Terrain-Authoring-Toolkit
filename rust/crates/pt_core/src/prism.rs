//! GPU-instanced hex prism columns (the `HexPrism` edge style drawn as one MultiMesh instance
//! per cell instead of a meshed chunk; BitCraft's "one instanced column stack per large hex").
//!
//! Every instance draws the same **template** mesh: a hexagonal column whose vertices only
//! carry *roles* (top centre, inner ring, inset top corner, edge midpoint, bevel foot, groove
//! rings, wall foot) plus the corner / edge / ring they belong to. The vertex shader
//! (`mst_prism_instanced.gdshaderinc`, a port of [`place`]) reads the column's precomputed
//! shape ([`column_texels`]: height, bevel, drops, neighbour floors, side feet; 3–4 texel
//! fetches per vertex) from the column map and moves each role to its place, so a column is
//! rebuilt from data alone: the bevelled rim towards lower
//! neighbours, vertical sides down to each lower neighbour (or `prism_skirt` levels on the
//! terrain border), the chamfer + underside groove at every whole level of an exposed side
//! (`grooves` rings per side; the node picks per chunk the count its tallest side needs,
//! [`grooves_needed`]), floor-slot blending with
//! equal-height neighbours and the ledge band at the foot of higher neighbours. Roles that a
//! cell does not need collapse onto a neighbouring vertex (zero-area triangles), so one
//! template fits every column. The result matches [`crate::cells::mesh_cell_chunk`] for
//! HexPrism (same rim, bevel, groove and blend rules; tested below). [`chunk_soup`] places the
//! groove-less columns on the CPU for collision and grass.
//!
//! Template vertex encoding (all floats, read by the shader):
//! * `CUSTOM0 = (role, corner, edge, ring)`
//! * `CUSTOM1 = (normal_edge, kind, -1 - normal_kind, 0)` — `CUSTOM1.z < -0.5` marks a
//!   template vertex (terrain meshes never write a negative wall flag);
//! * `UV2 = (blend_edge_0, blend_edge_1)` (-1 = none): the neighbours a floor triangle blends.
//!
//! Instances: identity transforms, `INSTANCE_CUSTOM = (q, r, low, rings)` (the cell's grid
//! coordinates; `low = 1` draws the distant-LOD column: flat top, plain walls).

use crate::data::{TerrainStore, VOID_SLOT};
use crate::grid::{GridConfig, GridType, SQRT3};
use crate::math::*;
use crate::mesh::MeshArrays;

/// Vertex roles (`CUSTOM0.x`).
pub mod role {
    pub const CENTER: u8 = 0;
    pub const INNER: u8 = 1;
    pub const TOP: u8 = 2;
    pub const MID: u8 = 3;
    pub const OUTER_TOP: u8 = 4;
    pub const RING_HI: u8 = 5;
    pub const RING_IN: u8 = 6;
    pub const RING_LO: u8 = 7;
    pub const CAP_IN: u8 = 8;
    pub const FOOT: u8 = 9;
}

/// Normal kinds (`-1 - CUSTOM1.z`).
pub mod nrm {
    pub const UP: u8 = 0;
    pub const DOWN: u8 = 1;
    pub const OUT: u8 = 2;
    /// 45° chamfer facing out of `normal_edge` (top bevel and groove chamfers).
    pub const CHAMFER: u8 = 3;
}

/// Surface kinds (`CUSTOM1.y`) → the terrain shader's `CUSTOM1.z` wall flag.
pub mod kind {
    pub const FLOOR: u8 = 0;
    pub const WALL: u8 = 1;
    /// Bevels and groove caps (closing geometry, 0.75 like the mesher's caps).
    pub const CAP: u8 = 2;
}

/// Inner, unblended part of a cell top (= `cells::INNER_RING`).
pub const INNER_RING: f32 = 0.4;
/// Ledge factor at the inner ring (= `cells::INNER_BAND`).
pub const INNER_BAND: f32 = 0.35;
/// Ramps / bevels run at most this fraction of the apothem (= `cells::RAMP_MAX_INSET`).
pub const MAX_INSET: f32 = 0.48;

/// One template vertex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TVertex {
    pub role: u8,
    pub corner: u8,
    pub edge: u8,
    pub ring: u8,
    pub ids: [i8; 2],
    pub nrm: u8,
    pub nrm_edge: u8,
    pub kind: u8,
}

/// The column template: triangles of [`TVertex`] (wound as Godot front faces).
#[derive(Clone, Debug, Default)]
pub struct Template {
    pub tris: Vec<[TVertex; 3]>,
    pub grooves: u32,
}

/// Prism shape parameters (from the terrain: cell width, `level_height`, `prism_bevel`,
/// `prism_skirt`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrismParams {
    /// Flat-to-flat cell width (`cell_size.x`).
    pub width: f32,
    pub level: f32,
    /// Bevel / groove size as a fraction of the apothem.
    pub bevel: f32,
    /// Border skirt depth in levels.
    pub skirt: f32,
}

impl PrismParams {
    pub fn from_store(store: &TerrainStore) -> PrismParams {
        let e = store.edges;
        PrismParams { width: store.config.cell_size_safe().x, level: e.terrace_step.abs().max(1e-3), bevel: e.prism_bevel, skirt: e.prism_skirt }
    }
    fn apothem(&self) -> f32 {
        self.width * 0.5
    }
    fn radius(&self) -> f32 {
        self.width / SQRT3
    }
}

/// A neighbour as the column sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nb {
    pub height: f32,
    pub floor: u8,
    /// The neighbour's own top bevel size (0 = none): a wall down to it follows the
    /// neighbour's rim where that rim is bevelled at the shared corner.
    pub bevel: f32,
}

/// Everything the vertex shader reads for one column (own texel + the six neighbours).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub height: f32,
    pub floor: u8,
    pub wall: u8,
    pub grass: u8,
    pub nbs: [Option<Nb>; 6],
    pub low: bool,
}

impl Column {
    /// Column of cell `c` read from the store (`None` for missing or void cells).
    pub fn from_store(store: &TerrainStore, c: IVec2, low: bool) -> Option<Column> {
        let s = store.sample(c).filter(|s| s.floor != VOID_SLOT)?;
        let mut nbs = [None; 6];
        let p = PrismParams::from_store(store);
        for (e, nb) in store.config.edge_neighbors(c).into_iter().enumerate().take(6) {
            nbs[e] = store.sample(nb).map(|n| {
                let mut hs = [None; 6];
                for (k, m) in store.config.edge_neighbors(nb).into_iter().enumerate().take(6) {
                    hs[k] = store.height(m);
                }
                Nb { height: n.height, floor: n.floor, bevel: bevel_size(n.height, &hs, &p) }
            });
        }
        Some(Column { height: s.height, floor: s.floor, wall: s.wall, grass: if s.grass != 0 { 255 } else { 0 }, nbs, low })
    }
}

/// A placed template vertex (terrain-local position relative to the cell centre + the terrain
/// shader's vertex attributes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placed {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub custom0: [f32; 4],
    pub custom1: [f32; 4],
}

fn corner_dir(i: usize) -> Vec2 {
    let a = (-30.0f32 + 60.0 * (i % 6) as f32).to_radians();
    v2(a.cos(), a.sin())
}

fn out_dir(e: usize) -> Vec2 {
    let a = (60.0f32 * (e % 6) as f32).to_radians();
    v2(a.cos(), a.sin())
}

/// The two edges meeting at corner `i`, in the mesher's order (`cells::Geom::corner_edges`).
fn corner_edges(i: usize) -> (usize, usize) {
    if i == 0 { (0, 5) } else { (i - 1, i) }
}

/// Per-column values shared by all its vertices (computed once per vertex in the shader).
#[derive(Clone, Copy, Debug)]
struct Shape {
    /// Drop to each neighbour (> 0 = exposed side; missing = skirt).
    drop: [f32; 6],
    bev: [bool; 6],
    /// Top bevel size and groove size.
    b: f32,
    gb: f32,
}

/// Top bevel size of a column at `h` with neighbour heights `nbs` (`None` = missing: skirt):
/// the nominal bevel, shrunk on small drops so the chamfer never reaches the lower top
/// (`cells::prism_bevel`, one size per column from its smallest drop). 0 without a drop.
pub fn bevel_size(h: f32, nbs: &[Option<f32>; 6], p: &PrismParams) -> f32 {
    let a = p.apothem();
    let mut b = f32::MAX;
    for n in nbs {
        let d = n.map(|nh| h - nh).unwrap_or(p.skirt * p.level);
        if d > 1e-4 {
            b = b.min((p.bevel * a).min(d * 0.4).min(p.level * 0.4).min(a * MAX_INSET).max(1e-3));
        }
    }
    if b == f32::MAX { 0.0 } else { b }
}

fn shape(col: &Column, p: &PrismParams) -> Shape {
    let a = p.apothem();
    let max_run = a * MAX_INSET;
    let mut drop = [0.0f32; 6];
    let mut bev = [false; 6];
    let mut b = f32::MAX;
    for e in 0..6 {
        drop[e] = match col.nbs[e] {
            Some(n) => col.height - n.height,
            None => p.skirt * p.level,
        };
        if drop[e] > 1e-4 && !col.low {
            bev[e] = true;
            b = b.min((p.bevel * a).min(drop[e] * 0.4).min(p.level * 0.4).min(max_run).max(1e-3));
        }
    }
    if b == f32::MAX {
        b = 0.0;
    }
    let gb = if col.low { 0.0 } else { (p.bevel * a).min(p.level * 0.4).min(max_run) };
    Shape { drop, bev, b, gb }
}

/// Inset of corner `i` by `d1` / `d2` along the inward normals of its two edges.
fn inset(i: usize, d1: f32, d2: f32, r: f32) -> Vec2 {
    let c = corner_dir(i) * r;
    let (e1, e2) = corner_edges(i);
    if d1 == 0.0 && d2 == 0.0 {
        return c;
    }
    let (n1, n2) = (-out_dir(e1), -out_dir(e2));
    let det = n1.x * n2.y - n1.y * n2.x;
    c + v2((d1 * n2.y - d2 * n1.y) / det, (n1.x * d2 - n2.x * d1) / det)
}

/// Groove levels of edge `e`, top-down: `(k_max, k_min)` (`k·level` for `k_max ≥ k ≥ k_min`).
fn groove_range(s: &Shape, col: &Column, e: usize, p: &PrismParams) -> (i32, i32) {
    if s.drop[e] <= 1e-4 || col.low {
        return (0, 1);
    }
    let top = col.height - if s.bev[e] { s.b } else { 0.0 };
    let foot = col.height - s.drop[e];
    let kmax = ((top - 1.5 * s.gb + 1e-4) / p.level).floor() as i32;
    let kmin = ((foot + s.gb - 1e-4) / p.level).ceil() as i32;
    (kmax, kmin)
}

/// Height of groove `ring` on edge `e`, or `None` when that groove does not exist.
fn groove_y(s: &Shape, col: &Column, e: usize, ring: u32, grooves: u32, p: &PrismParams) -> Option<f32> {
    if ring >= grooves {
        return None;
    }
    let (kmax, kmin) = groove_range(s, col, e, p);
    let k = kmax - ring as i32;
    (k >= kmin).then_some(k as f32 * p.level)
}

/// Whether edge `e` has a groove at height `y`.
fn has_groove_at(s: &Shape, col: &Column, e: usize, y: f32, grooves: u32, p: &PrismParams) -> bool {
    (0..grooves).any(|j| groove_y(s, col, e, j, grooves, p).is_some_and(|z| (z - y).abs() < 1e-4))
}

/// Foot of side `e` at corner `i`: the neighbour's top, or its bevel foot where the
/// neighbour is bevelled at this corner (towards the third cell there), so the wall never
/// leaves a notch above a bevelled rim (the mesher follows the neighbour's profile). `h` for
/// sides that are not exposed.
fn foot_at(col: &Column, s: &Shape, i: usize, e: usize) -> f32 {
    let h = col.height;
    if s.drop[e] <= 1e-4 {
        return h;
    }
    let Some(n) = col.nbs[e] else { return h - s.drop[e] };
    let (e1, e2) = corner_edges(i);
    let eo = if e1 == e { e2 } else { e1 };
    let toward = match col.nbs[eo] {
        None => true,
        Some(t) => n.height - t.height > 1e-4,
    };
    n.height - if toward { n.bevel } else { 0.0 }
}

/// Texels per cell in the column map ([`column_texels`]).
pub const COLUMN_TEXELS: i32 = 6;

/// The per-column values the vertex shader reads (computed once on the CPU, so a vertex does
/// 3–4 texel fetches instead of reading and re-deriving all six neighbours):
/// * `T0 = (height, floor slot, wall slot + 16·grass, bevel)` (full-detail bevel);
/// * `T1 = drop[0..4]`, `T2 = (drop[4], drop[5], neighbour floors, 0)` — drops to each
///   neighbour (> 0 = exposed; missing neighbours = the skirt), neighbour floor slots packed
///   4 bits per edge (15 = missing or void);
/// * `T3..T5` = the 12 side feet `foot(corner i, its edges)` at index `2i + (edge == first)`.
pub fn column_texels(col: &Column, p: &PrismParams) -> [[f32; 4]; 6] {
    let full = Column { low: false, ..*col };
    let s = shape(&full, p);
    let mut packed: u32 = 0;
    for e in 0..6 {
        let code = col.nbs[e].map(|n| n.floor.min(15)).unwrap_or(15) as u32;
        packed |= code << (4 * e);
    }
    let wall = col.wall as f32 + if col.grass != 0 { 16.0 } else { 0.0 };
    let mut feet = [0.0f32; 12];
    for i in 0..6 {
        let (e1, e2) = corner_edges(i);
        feet[2 * i] = foot_at(&full, &s, i, e1);
        feet[2 * i + 1] = foot_at(&full, &s, i, e2);
    }
    [
        [col.height, col.floor as f32, wall, s.b],
        [s.drop[0], s.drop[1], s.drop[2], s.drop[3]],
        [s.drop[4], s.drop[5], packed as f32, 0.0],
        [feet[0], feet[1], feet[2], feet[3]],
        [feet[4], feet[5], feet[6], feet[7]],
        [feet[8], feet[9], feet[10], feet[11]],
    ]
}

/// Column map of a window: [`COLUMN_TEXELS`] texels per cell, texel `(x·6 + k, y)` for cell
/// `origin + (x, y)` (zero for missing / void cells, which get no instance).
pub fn build_column_map(store: &TerrainStore, w: crate::hexmap::MapWindow) -> Vec<[f32; 4]> {
    let p = PrismParams::from_store(store);
    let mut out = Vec::with_capacity(w.len() * COLUMN_TEXELS as usize);
    for c in w.cells() {
        match Column::from_store(store, c, false) {
            Some(col) => out.extend_from_slice(&column_texels(&col, &p)),
            None => out.extend_from_slice(&[[0.0; 4]; 6]),
        }
    }
    out
}

/// Rewrites, in a [`build_column_map`] buffer of window `w`, the texels of the cells within
/// `pad` cells of `cells` (a column's shape reads its neighbours, so an edit reaches one ring
/// further). Lets a height edit refresh only the touched area instead of the whole map.
pub fn update_column_map(store: &TerrainStore, w: crate::hexmap::MapWindow, texels: &mut [[f32; 4]], cells: impl IntoIterator<Item = IVec2>, pad: i32) {
    if texels.len() != w.len() * COLUMN_TEXELS as usize {
        return;
    }
    let p = PrismParams::from_store(store);
    let area = crate::hexmap::MapWindow::bounding(cells, pad);
    for c in area.cells() {
        let Some(i) = w.index(c) else { continue };
        let t = match Column::from_store(store, c, false) {
            Some(col) => column_texels(&col, &p),
            None => [[0.0; 4]; 6],
        };
        texels[i * 6..i * 6 + 6].copy_from_slice(&t);
    }
}

/// Places one template vertex for a column (the CPU reference of the vertex shader).
pub fn place(v: &TVertex, col: &Column, grooves: u32, p: &PrismParams) -> Placed {
    place_shaped(v, col, &shape(col, p), grooves, p)
}

fn place_shaped(v: &TVertex, col: &Column, s: &Shape, grooves: u32, p: &PrismParams) -> Placed {
    let s = *s;
    let r = p.radius();
    let h = col.height;
    let i = v.corner as usize % 6;
    let e = v.edge as usize % 6;
    let corner = corner_dir(i) * r;
    let d = |e: usize| if s.bev[e] { s.b } else { 0.0 };
    let foot = |i: usize, e: usize| foot_at(col, &s, i, e);
    let top_c = |i: usize| {
        let (e1, e2) = corner_edges(i);
        inset(i, d(e1), d(e2), r)
    };
    // Groove inner point at corner `i` of edge `e` and whether it needs a cap.
    let groove_inner = |i: usize, e: usize, y: f32| -> (Vec2, bool) {
        let (e1, e2) = corner_edges(i);
        let eo = if e1 == e { e2 } else { e1 };
        if has_groove_at(&s, col, eo, y, grooves, p) {
            return (inset(i, s.gb, s.gb, r), false);
        }
        // Along the other edge, away from this corner.
        let far = if eo == i { (i + 1) % 6 } else { (i + 5) % 6 };
        let u = (corner_dir(far) - corner_dir(i)).normalized();
        let inward = -out_dir(e);
        (corner + u * (s.gb / u.dot(inward).max(0.2)), true)
    };
    let p3 = |q: Vec2, y: f32| v3(q.x, y, q.y);
    let pos = match v.role {
        role::CENTER => v3(0.0, h, 0.0),
        role::INNER => p3(top_c(i) * INNER_RING, h),
        role::TOP => p3(top_c(i), h),
        role::MID => p3((top_c(e) + top_c((e + 1) % 6)) * 0.5, h),
        role::OUTER_TOP => p3(corner, h - d(e)),
        role::FOOT => p3(corner, foot(i, e)),
        role::RING_HI | role::RING_IN | role::RING_LO | role::CAP_IN => match groove_y(&s, col, e, v.ring as u32, grooves, p) {
            None => p3(corner, foot(i, e)),
            Some(y) => match v.role {
                role::RING_HI => p3(corner, y),
                role::RING_LO => p3(corner, y - s.gb),
                role::RING_IN => p3(groove_inner(i, e, y).0, y),
                _ => {
                    let (q, cap) = groove_inner(i, e, y);
                    if cap { p3(q, y) } else { p3(corner, y) }
                }
            },
        },
        _ => v3(0.0, h, 0.0),
    };
    let ne = v.nrm_edge as usize % 6;
    let normal = match v.nrm {
        nrm::UP => v3(0.0, 1.0, 0.0),
        nrm::DOWN => v3(0.0, -1.0, 0.0),
        nrm::OUT => {
            let o = out_dir(ne);
            v3(o.x, 0.0, o.y)
        }
        _ => {
            let o = out_dir(ne);
            v3(o.x, 1.0, o.y).normalized()
        }
    };
    // Floor attributes: blend weights / slots (mesher's corner_w / mid rules) and ledge bands.
    let connected = |e: usize| col.nbs[e].is_some_and(|n| n.floor != VOID_SLOT && (n.height - h).abs() <= 1e-4);
    let ledge = |e: usize| col.nbs[e].is_some_and(|n| n.floor != VOID_SLOT && n.height - h > 1e-4);
    let slot_of = |e: i8| -> f32 {
        if e < 0 || col.low {
            return col.floor as f32;
        }
        let e = e as usize % 6;
        if connected(e) { col.nbs[e].map(|n| n.floor).unwrap_or(col.floor) as f32 } else { col.floor as f32 }
    };
    if v.kind == kind::FLOOR {
        // Weights over (own, edge a, edge b) for this vertex.
        let w: [(i8, f32); 3] = if col.low {
            [(-1, 1.0), (-1, 0.0), (-1, 0.0)]
        } else {
            match v.role {
                role::TOP => {
                    let (e1, e2) = corner_edges(i);
                    match (s.bev[e1], s.bev[e2]) {
                        (false, false) => [(-1, 1.0 / 3.0), (e1 as i8, 1.0 / 3.0), (e2 as i8, 1.0 / 3.0)],
                        (true, false) => [(-1, 0.5), (e2 as i8, 0.5), (-1, 0.0)],
                        (false, true) => [(-1, 0.5), (e1 as i8, 0.5), (-1, 0.0)],
                        (true, true) => [(-1, 1.0), (-1, 0.0), (-1, 0.0)],
                    }
                }
                role::MID if !s.bev[e] => [(-1, 0.5), (e as i8, 0.5), (-1, 0.0)],
                _ => [(-1, 1.0), (-1, 0.0), (-1, 0.0)],
            }
        };
        let mut wt = [0.0f32; 3];
        for (id, x) in w {
            let k = if id < 0 { 0 } else if id == v.ids[0] { 1 } else if id == v.ids[1] { 2 } else { 0 };
            wt[k] += x;
        }
        let band = |i: usize| {
            let (e1, e2) = corner_edges(i);
            if ledge(e1) || ledge(e2) { 1.0 } else { 0.0 }
        };
        let uvx = if col.low {
            0.0
        } else {
            match v.role {
                role::TOP => band(i),
                role::INNER => band(i) * INNER_BAND,
                role::MID => {
                    if ledge(e) { 1.0 } else { 0.0 }
                }
                _ => 0.0,
            }
        };
        let grass = if col.low { 0.0 } else { col.grass as f32 / 255.0 };
        Placed {
            pos,
            normal,
            uv: v2(uvx, 0.0),
            custom0: [col.floor as f32, slot_of(v.ids[0]), slot_of(v.ids[1]), col.wall as f32],
            custom1: [wt[0], wt[1], 0.0, grass],
        }
    } else {
        let w = col.wall as f32;
        let flag = if col.low { crate::cells::LOD_WALL } else if v.kind == kind::CAP { 0.75 } else { 1.0 };
        Placed { pos, normal, uv: v2(1.0, 1.0), custom0: [w; 4], custom1: [1.0, 0.0, flag, 0.0] }
    }
}

/// Builds the column template with `grooves` groove rings per side (0 = plain walls).
pub fn template(grooves: u32) -> Template {
    let grooves = grooves.min(MAX_GROOVES);
    let mut tris: Vec<[TVertex; 3]> = Vec::new();
    let tv = |role: u8, corner: usize, edge: usize, ring: u32, nrm: u8, nrm_edge: usize, kind: u8, ids: [i8; 2]| TVertex {
        role,
        corner: corner as u8,
        edge: edge as u8,
        ring: ring as u8,
        ids,
        nrm,
        nrm_edge: nrm_edge as u8,
        kind,
    };
    for e in 0..6usize {
        let (a, b) = (e, (e + 1) % 6);
        let (prev, next) = ((e + 5) % 6, (e + 1) % 6);
        // Top: centre fan to the inner ring, rim triangles through the edge midpoint.
        let fl = |role: u8, c: usize, ids: [i8; 2]| tv(role, c, e, 0, nrm::UP, e, kind::FLOOR, ids);
        let ca = corner_edges(a);
        let cb = corner_edges(b);
        let none = [-1, -1];
        tris.push([fl(role::CENTER, a, none), fl(role::INNER, a, none), fl(role::INNER, b, none)]);
        tris.push([fl(role::INNER, a, [ca.0 as i8, ca.1 as i8]), fl(role::TOP, a, [ca.0 as i8, ca.1 as i8]), fl(role::MID, a, [ca.0 as i8, ca.1 as i8])]);
        tris.push([fl(role::INNER, a, [e as i8, -1]), fl(role::MID, a, [e as i8, -1]), fl(role::INNER, b, [e as i8, -1])]);
        tris.push([fl(role::INNER, b, [cb.0 as i8, cb.1 as i8]), fl(role::MID, a, [cb.0 as i8, cb.1 as i8]), fl(role::TOP, b, [cb.0 as i8, cb.1 as i8])]);
        let _ = (prev, next);
        // Top bevel.
        let bv = |role: u8, c: usize| tv(role, c, e, 0, nrm::CHAMFER, e, kind::CAP, none);
        let q = [bv(role::TOP, a), bv(role::TOP, b), bv(role::OUTER_TOP, b), bv(role::OUTER_TOP, a)];
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
        // Wall segments between the grooves.
        let wl = |role: u8, c: usize, ring: u32| tv(role, c, e, ring, nrm::OUT, e, kind::WALL, none);
        let mut upper = (role::OUTER_TOP, 0u32);
        for j in 0..=grooves {
            let lower = if j < grooves { (role::RING_HI, j) } else { (role::FOOT, 0) };
            let q = [wl(upper.0, a, upper.1), wl(upper.0, b, upper.1), wl(lower.0, b, lower.1), wl(lower.0, a, lower.1)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
            upper = (role::RING_LO, j);
        }
        // Grooves: chamfer of the prism below, underside of the prism above, corner caps.
        for j in 0..grooves {
            let ch = |role: u8, c: usize| tv(role, c, e, j, nrm::CHAMFER, e, kind::WALL, none);
            let q = [ch(role::RING_LO, a), ch(role::RING_LO, b), ch(role::RING_IN, b), ch(role::RING_IN, a)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
            let un = |role: u8, c: usize| tv(role, c, e, j, nrm::DOWN, e, kind::WALL, none);
            let q = [un(role::RING_IN, a), un(role::RING_IN, b), un(role::RING_HI, b), un(role::RING_HI, a)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
            for c in [a, b] {
                let (e1, e2) = corner_edges(c);
                let eo = if e1 == e { e2 } else { e1 };
                let cp = |role: u8| tv(role, c, e, j, nrm::OUT, eo, kind::CAP, none);
                let t = [cp(role::RING_LO), cp(role::CAP_IN), cp(role::RING_HI)];
                tris.push(t);
                tris.push([t[0], t[2], t[1]]);
            }
        }
    }
    orient(&mut tris, grooves);
    Template { tris, grooves }
}

/// The distant-LOD column template (draw with `low = 1`): a flat top fan over the whole
/// hexagon and one plain wall per side, a quarter of the full template's vertices.
pub fn template_low() -> Template {
    let mut tris: Vec<[TVertex; 3]> = Vec::new();
    let v = |role: u8, corner: usize, edge: usize, nrm: u8, kind: u8| TVertex {
        role,
        corner: corner as u8,
        edge: edge as u8,
        ring: 0,
        ids: [-1, -1],
        nrm,
        nrm_edge: edge as u8,
        kind,
    };
    for e in 0..6usize {
        let (a, b) = (e, (e + 1) % 6);
        // Floor roles of a low column ignore the edge: share them (7 top vertices in all).
        tris.push([v(role::CENTER, 0, 0, nrm::UP, kind::FLOOR), v(role::TOP, a, 0, nrm::UP, kind::FLOOR), v(role::TOP, b, 0, nrm::UP, kind::FLOOR)]);
        let q = [
            v(role::OUTER_TOP, a, e, nrm::OUT, kind::WALL),
            v(role::OUTER_TOP, b, e, nrm::OUT, kind::WALL),
            v(role::FOOT, b, e, nrm::OUT, kind::WALL),
            v(role::FOOT, a, e, nrm::OUT, kind::WALL),
        ];
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
    }
    orient(&mut tris, 0);
    Template { tris, grooves: 0 }
}

/// Top-only template (floor triangles of [`template`] or, `low`, of [`template_low`]) for
/// columns with no exposed side ([`has_exposed_side`] false): their walls, bevels and grooves
/// all collapse, so drawing them is wasted vertex and setup work.
pub fn template_top(low: bool) -> Template {
    let t = if low { template_low() } else { template(0) };
    Template { tris: t.tris.into_iter().filter(|t| t[0].kind == kind::FLOOR).collect(), grooves: 0 }
}

/// Whether a column has a side above a neighbour (or on the terrain border): it needs the
/// full template; otherwise [`template_top`] draws it exactly.
pub fn has_exposed_side(col: &Column) -> bool {
    (0..6).any(|e| match col.nbs[e] {
        Some(n) => col.height - n.height > 1e-4,
        None => true,
    })
}

/// Winds every triangle as a Godot front face towards its normal, using a column where
/// every role is in general position (all sides exposed, every groove present). Groove caps
/// are double-sided and left alone.
fn orient(tris: &mut [[TVertex; 3]], grooves: u32) {
    let p = PrismParams { width: 2.0, level: 1.0, bevel: 0.14, skirt: 9.0 };
    let nominal = Column {
        height: 20.0,
        floor: 0,
        wall: 5,
        grass: 0,
        nbs: [Some(Nb { height: 0.0, floor: 0, bevel: 0.0 }); 6],
        low: false,
    };
    for t in tris.iter_mut() {
        let pl = t.map(|v| place(&v, &nominal, grooves, &p));
        let n = (pl[2].pos - pl[0].pos).cross(pl[1].pos - pl[0].pos);
        let caps = t[0].kind == kind::CAP && t[0].nrm == nrm::OUT;
        if !caps && n.dot(pl[0].normal) < 0.0 {
            t.swap(1, 2);
        }
    }
}

impl Template {
    /// The template as mesh arrays for an `ArrayMesh` (see the module docs for the encoding).
    pub fn arrays(&self) -> MeshArrays {
        let mut out = MeshArrays::default();
        let mut seen: std::collections::HashMap<TVertex, i32> = std::collections::HashMap::new();
        for t in &self.tris {
            for v in t {
                let idx = *seen.entry(*v).or_insert_with(|| {
                    // Positions only matter for the mesh AABB (instances set a custom one).
                    let c = corner_dir(v.corner as usize);
                    out.positions.push([c.x, if v.role <= role::MID { 0.0 } else { -1.0 }, c.y]);
                    out.normals.push([0.0, 1.0, 0.0]);
                    out.uv.push([0.0, 0.0]);
                    out.uv2.push([v.ids[0] as f32, v.ids[1] as f32]);
                    out.custom0.push([v.role as f32, v.corner as f32, v.edge as f32, v.ring as f32]);
                    out.custom1.push([v.nrm_edge as f32, v.kind as f32, -1.0 - v.nrm as f32, 0.0]);
                    (out.positions.len() - 1) as i32
                });
                out.indices.push(idx);
            }
        }
        out
    }

    /// Triangles of one column placed on the CPU (terrain-local, at the cell centre `center`),
    /// skipping collapsed ones. Reference for the shader and for tests.
    pub fn place_column(&self, col: &Column, center: Vec2, p: &PrismParams) -> Vec<[Placed; 3]> {
        let off = v3(center.x, 0.0, center.y);
        let s = shape(col, p);
        self.tris
            .iter()
            .map(|t| {
                t.map(|v| {
                    let mut pl = place_shaped(&v, col, &s, self.grooves, p);
                    pl.pos = pl.pos + off;
                    pl
                })
            })
            .filter(|t| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).length() > 1e-7)
            .collect()
    }
}

/// The surface the instanced columns of a chunk draw, as a mesher soup (no grooves): flat
/// tops inset by the bevels, bevel chamfers and sides, with the same floor attributes as the
/// shader. Used for the chunk's collision and grass (so both follow the bevelled rim exactly
/// like the meshed HexPrism) without meshing the full surface; `low` = distant LOD columns.
pub fn chunk_soup(store: &TerrainStore, chunk: IVec2, low: bool) -> crate::mesh::Soup {
    use crate::mesh::{Soup, TVert};
    let mut out = Soup::default();
    let cfg = &store.config;
    let p = PrismParams::from_store(store);
    let cs = cfg.cell_size_safe();
    out.cell_area = p.width * p.width * SQRT3 / 2.0;
    out.wall_smooth_cos = None;
    let t = template(0);
    for c in chunk_instances(store, chunk) {
        let Some(col) = Column::from_store(store, c, low) else { continue };
        out.begin_cell(c);
        for tri in t.place_column(&col, cfg.sample_pos(c), &p) {
            for v in tri {
                out.verts.push(TVert {
                    pos: v.pos,
                    uv: v.uv,
                    uv2: v2(v.pos.x / cs.x, v.pos.z / cs.y),
                    custom0: v.custom0,
                    custom1: v.custom1,
                    floor: v.custom1[2] == 0.0,
                });
            }
        }
        out.end_cell();
    }
    out
}

/// Most grooves any exposed side of a chunk's columns has (the template ring count that
/// draws every groove of the chunk).
pub fn grooves_needed(store: &TerrainStore, chunk: IVec2) -> u32 {
    let p = PrismParams::from_store(store);
    let mut most = 0i32;
    for c in chunk_instances(store, chunk) {
        let Some(col) = Column::from_store(store, c, false) else { continue };
        let s = shape(&col, &p);
        for e in 0..6 {
            let (kmax, kmin) = groove_range(&s, &col, e, &p);
            most = most.max(kmax - kmin + 1);
        }
    }
    most.max(0) as u32
}

/// Template ring count for `needed` grooves under a cap: the next power of two (few distinct
/// templates), at most `cap`.
pub fn groove_bucket(needed: u32, cap: u32) -> u32 {
    if needed == 0 { 0 } else { needed.next_power_of_two().min(cap.max(1)).min(MAX_GROOVES) }
}

/// Largest template ring count.
pub const MAX_GROOVES: u32 = 64;

/// Cells of a chunk that get a column instance (stored, non-void cells).
pub fn chunk_instances(store: &TerrainStore, chunk: IVec2) -> Vec<IVec2> {
    if !store.chunks.contains_key(&chunk) {
        return Vec::new();
    }
    store.config.chunk_cells(chunk).into_iter().filter(|c| store.sample(*c).is_some_and(|s| s.floor != VOID_SLOT)).collect()
}

/// Whether instanced columns can draw this grid (both hex layouts).
pub fn supported(cfg: &GridConfig) -> bool {
    matches!(cfg.grid, GridType::Hex | GridType::HexRings)
}

/// Terrain-local bounds `(min, max)` of a chunk's columns, including skirts and the lowest
/// neighbour each column reaches down to.
pub fn chunk_bounds(store: &TerrainStore, chunk: IVec2) -> Option<(Vec3, Vec3)> {
    let cfg = &store.config;
    let p = PrismParams::from_store(store);
    let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
    let r = p.radius();
    for c in chunk_instances(store, chunk) {
        let s = store.sample(c)?;
        let mut bottom = s.height;
        for nb in cfg.edge_neighbors(c) {
            bottom = bottom.min(store.height(nb).unwrap_or(s.height - p.skirt * p.level));
        }
        let q = cfg.sample_pos(c);
        lo = v3(lo.x.min(q.x - r), lo.y.min(bottom), lo.z.min(q.y - r));
        hi = v3(hi.x.max(q.x + r), hi.y.max(s.height), hi.z.max(q.y + r));
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells::{EdgeStyle, mesh_cell_chunk};
    use crate::data::MergeMode;
    use crate::mesh::Soup;

    fn store(level: f32) -> TerrainStore {
        let cfg = GridConfig { grid: GridType::HexRings, ring_radius: 3, cell_size: v2(2.0, 2.0), ..Default::default() };
        let mut s = TerrainStore::new(cfg);
        s.edges.style = EdgeStyle::HexPrism;
        s.edges.terrace_step = level;
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        s
    }

    fn area(t: [Vec3; 3]) -> f32 {
        (t[2] - t[0]).cross(t[1] - t[0]).length() * 0.5
    }

    /// Up-facing, down-facing, vertical and sloped area of a triangle set.
    fn areas(ts: &[[Vec3; 3]]) -> [f32; 4] {
        let mut a = [0.0f32; 4];
        for t in ts {
            let n = (t[2] - t[0]).cross(t[1] - t[0]);
            let l = n.length();
            if l < 1e-7 {
                continue;
            }
            let ny = n.y / l;
            let k = if ny > 0.99 { 0 } else if ny < -0.99 { 1 } else if ny.abs() < 0.01 { 2 } else { 3 };
            a[k] += area(*t);
        }
        a
    }

    fn meshed_cell(s: &TerrainStore, c: IVec2) -> Vec<[Vec3; 3]> {
        let mut soup = Soup::default();
        mesh_cell_chunk(s, s.config.chunk_of(c), &mut soup);
        let r = soup.cells.iter().find(|x| x.cell == c).unwrap().clone();
        soup.verts[r.start..r.end].chunks_exact(3).map(|t| [t[0].pos, t[1].pos, t[2].pos]).collect()
    }

    fn instanced_cell(s: &TerrainStore, c: IVec2, grooves: u32) -> Vec<[Placed; 3]> {
        let col = Column::from_store(s, c, false).unwrap();
        template(grooves).place_column(&col, s.config.sample_pos(c), &PrismParams::from_store(s))
    }

    fn check_matches(s: &TerrainStore, c: IVec2) {
        let m = areas(&meshed_cell(s, c));
        let inst: Vec<[Vec3; 3]> = instanced_cell(s, c, 16).iter().map(|t| t.map(|v| v.pos)).collect();
        let i = areas(&inst);
        for k in [0, 1, 3] {
            assert!((m[k] - i[k]).abs() <= 1e-3 + m[k] * 0.01, "cell {c:?} class {k}: meshed {m:?} vs instanced {i:?}");
        }
        // Walls: the instanced foot runs straight between the corners, so next to a bevelled
        // neighbour rim it reaches a little below the rim profile the mesher follows (buried
        // in the neighbour column, never a gap).
        assert!(i[2] >= m[2] - 1e-3 && i[2] <= m[2] * 1.05 + 1e-3, "cell {c:?} walls: meshed {m:?} vs instanced {i:?}");
        // Same vertical extent.
        let ys = |ts: &[[Vec3; 3]]| ts.iter().flatten().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let (my, iy) = (ys(&meshed_cell(s, c)), ys(&inst));
        assert!((my.0 - iy.0).abs() < 1e-4 && (my.1 - iy.1).abs() < 1e-4, "extent {my:?} vs {iy:?}");
    }

    #[test]
    fn instanced_column_matches_the_meshed_prism() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        // Flat: just the top.
        check_matches(&s, c);
        // A raised column over mixed neighbours: 1 level, 3 levels, 7 levels down, one higher.
        s.set_height(c, 4.0);
        let nbs = s.config.edge_neighbors(c);
        for (k, h) in [3.5, 2.5, 0.5, 6.0, 4.0, 4.0].into_iter().enumerate() {
            s.set_height(nbs[k], h);
        }
        for cell in [c, nbs[0], nbs[1], nbs[3]] {
            check_matches(&s, cell);
        }
        // Border column (skirt) and a pit.
        let edge = iv2(3, 0);
        s.set_height(edge, 1.0);
        check_matches(&s, edge);
        s.set_height(iv2(-1, 2), -3.0);
        check_matches(&s, iv2(-1, 1));
    }

    #[test]
    fn instanced_floor_attributes_match_the_mesher() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        s.set_height(c, 1.0);
        let nbs = s.config.edge_neighbors(c);
        s.set_height(nbs[2], 0.0); // lower: bevel, no blend
        s.set_height(nbs[4], 2.0); // higher: ledge band on our rim
        s.edit_sample(nbs[0], |d, k| d.floor_tex[k] = 6); // equal height, other slot: blends
        s.set_height(nbs[0], 1.0);
        s.set_height(nbs[1], 1.0);
        s.set_height(nbs[3], 1.0);
        s.set_height(nbs[5], 1.0);
        let inst = instanced_cell(&s, c, 4);
        let floor: Vec<&Placed> = inst.iter().flatten().filter(|v| v.custom1[2] == 0.0).collect();
        // Slot 6 shows up with weight on the rim towards neighbour 0 only.
        assert!(floor.iter().any(|v| (v.custom0[1] == 6.0 && v.custom1[1] > 0.3) || (v.custom0[2] == 6.0 && 1.0 - v.custom1[0] - v.custom1[1] > 0.3)));
        // Ledge band: UV.x = 1 on the rim next to the higher neighbour 4.
        assert!(floor.iter().any(|v| v.uv.x == 1.0));
        // Same weighted slot usage as the meshed floor (area-weighted share of slot 6).
        let share = |ts: Vec<([f32; 4], [f32; 4], [Vec3; 3])>| {
            let (mut tot, mut six) = (0.0f32, 0.0f32);
            for (c0, c1, t) in ts {
                let a = area(t);
                tot += a;
                let w = [c1[0], c1[1], (1.0 - c1[0] - c1[1]).max(0.0)];
                for k in 0..3 {
                    if c0[k] == 6.0 {
                        six += a * w[k];
                    }
                }
            }
            six / tot
        };
        let mut soup = Soup::default();
        mesh_cell_chunk(&s, iv2(0, 0), &mut soup);
        let r = soup.cells.iter().find(|x| x.cell == c).unwrap().clone();
        // Per-triangle average attributes (flat slots per triangle on both sides).
        let tri_attr = |t: &[crate::mesh::TVert]| {
            let c1 = [0, 1].map(|k| (t[0].custom1[k] + t[1].custom1[k] + t[2].custom1[k]) / 3.0);
            (t[0].custom0, [c1[0], c1[1], 0.0, 0.0], [t[0].pos, t[1].pos, t[2].pos])
        };
        let m: Vec<_> = soup.verts[r.start..r.end].chunks_exact(3).filter(|t| t[0].floor).map(tri_attr).collect();
        let i: Vec<_> = inst
            .iter()
            .filter(|t| t[0].custom1[2] == 0.0)
            .map(|t| {
                let c1 = [0, 1].map(|k| (t[0].custom1[k] + t[1].custom1[k] + t[2].custom1[k]) / 3.0);
                (t[0].custom0, [c1[0], c1[1], 0.0, 0.0], t.map(|v| v.pos))
            })
            .collect();
        let (ms, is) = (share(m), share(i));
        assert!(ms > 0.01 && (ms - is).abs() < 1e-3, "slot 6 share meshed {ms} vs instanced {is}");
    }

    #[test]
    fn collision_soup_follows_the_bevelled_rim() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        s.set_height(c, 3.0);
        let soup = chunk_soup(&s, iv2(0, 0), false);
        let r = soup.cells.iter().find(|x| x.cell == c).unwrap().clone();
        let tris: Vec<[Vec3; 3]> = soup.verts[r.start..r.end].chunks_exact(3).map(|t| [t[0].pos, t[1].pos, t[2].pos]).collect();
        let m = areas(&meshed_cell(&s, c));
        let a = areas(&tris);
        // Same inset top as the meshed prism; the bevel is the groove-less template's.
        let g0: Vec<[Vec3; 3]> = instanced_cell(&s, c, 0).iter().map(|t| t.map(|v| v.pos)).collect();
        let b = areas(&g0);
        assert!((a[0] - m[0]).abs() < 1e-3 && (a[3] - b[3]).abs() < 1e-3 && a[1] == 0.0, "{a:?} vs {m:?}");
        assert!(soup.verts.iter().filter(|v| v.floor).all(|v| v.pos.y == 3.0 || v.pos.y == 0.0));
    }

    #[test]
    fn column_texels_encode_the_shape() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        s.set_height(c, 2.0);
        let nbs = s.config.edge_neighbors(c);
        s.edit_sample(nbs[1], |d, k| d.floor_tex[k] = 9);
        s.set_height(nbs[1], 2.0);
        let col = Column::from_store(&s, c, false).unwrap();
        let p = PrismParams::from_store(&s);
        let t = column_texels(&col, &p);
        let sh = shape(&col, &p);
        assert_eq!((t[0][0], t[0][3]), (2.0, sh.b));
        assert_eq!(t[1][0], 2.0);
        assert_eq!(((t[2][2] as u32) >> 4) & 15, 9, "neighbour 1 floor");
        for i in 0..6 {
            let (e1, e2) = corner_edges(i);
            assert_eq!(t[3 + i / 2][(2 * i) % 4], foot_at(&col, &sh, i, e1));
            assert_eq!(t[3 + i / 2][(2 * i + 1) % 4], foot_at(&col, &sh, i, e2));
        }
        let w = crate::hexmap::chunks_window(&s, 1);
        assert_eq!(build_column_map(&s, w).len(), w.len() * 6);
    }

    #[test]
    fn partial_column_map_update_matches_a_rebuild() {
        let mut s = store(0.5);
        let w = crate::hexmap::chunks_window(&s, 0);
        let mut texels = build_column_map(&s, w);
        let c = iv2(1, 0);
        s.set_height(c, 3.0);
        update_column_map(&s, w, &mut texels, [c], 2);
        assert_eq!(texels, build_column_map(&s, w));
    }

    #[test]
    fn groove_limit_and_low_detail() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        s.set_height(c, 6.0);
        // 12 levels down to every neighbour: a 2-groove template keeps the top two grooves.
        let full = areas(&instanced_cell(&s, c, 16).iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        assert_eq!(grooves_needed(&s, iv2(0, 0)), 11, "levels 1..=11 between the rim and the foot");
        assert_eq!(groove_bucket(12, 64), 16);
        assert_eq!(groove_bucket(12, 8), 8);
        assert_eq!(groove_bucket(0, 8), 0);
        let two = instanced_cell(&s, c, 2);
        let a2 = areas(&two.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        assert!(a2[1] < full[1] && a2[1] > 0.0, "fewer groove undersides");
        assert!((a2[0] - full[0]).abs() < 1e-4, "same top");
        // Low detail: flat top over the whole hexagon, plain LOD walls, no grooves or bevel.
        let col = Column::from_store(&s, c, true).unwrap();
        let low = template(4).place_column(&col, v2(0.0, 0.0), &PrismParams::from_store(&s));
        let al = areas(&low.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        let hex_area = 2.0 * 2.0 * SQRT3 / 2.0;
        assert!((al[0] - hex_area).abs() < 1e-3 && al[1] == 0.0 && al[3] == 0.0, "{al:?}");
        assert!(low.iter().flatten().filter(|v| v.custom1[2] > 0.0).all(|v| v.custom1[2] == crate::cells::LOD_WALL));
        // The dedicated low template draws the same surface with far fewer vertices.
        let lt = template_low();
        let low2 = lt.place_column(&col, v2(0.0, 0.0), &PrismParams::from_store(&s));
        let al2 = areas(&low2.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        for k in 0..4 {
            assert!((al[k] - al2[k]).abs() < 1e-3, "{al:?} vs {al2:?}");
        }
        assert!(lt.arrays().positions.len() * 3 < template(0).arrays().positions.len());
        // Top-only template = the full column's top on a column without exposed sides.
        let mut f = store(0.5);
        f.set_height(iv2(0, 0), 1.0);
        for nb in f.config.edge_neighbors(iv2(0, 0)) {
            f.set_height(nb, 1.0);
        }
        let flat = Column::from_store(&f, iv2(0, 0), false).unwrap();
        assert!(!has_exposed_side(&flat) && has_exposed_side(&col));
        let pf = PrismParams::from_store(&f);
        let a_full = areas(&template(2).place_column(&flat, v2(0.0, 0.0), &pf).iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        let a_top = areas(&template_top(false).place_column(&flat, v2(0.0, 0.0), &pf).iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        assert_eq!(a_full, a_top);
    }

    #[test]
    fn template_is_wound_like_the_mesher_and_encodes_roles() {
        let t = template(3);
        let a = t.arrays();
        assert_eq!(a.indices.len(), t.tris.len() * 3);
        assert!(a.custom1.iter().all(|c| c[2] < -0.5), "every template vertex carries the sentinel");
        // Every non-cap triangle of a raised column faces along its normal (Godot front face).
        let mut s = store(0.5);
        s.set_height(iv2(0, 0), 3.0);
        for tri in instanced_cell(&s, iv2(0, 0), 3) {
            let n = (tri[2].pos - tri[0].pos).cross(tri[1].pos - tri[0].pos);
            if tri[0].custom1[2] == 0.75 && tri[0].normal.y.abs() < 0.01 {
                continue; // double-sided groove caps
            }
            assert!(n.dot(tri[0].normal) > 0.0, "{:?}", tri[0]);
        }
        let b = chunk_bounds(&s, iv2(0, 0)).unwrap();
        assert!(b.1.y == 3.0 && b.0.y <= -4.5 + 1e-4, "{b:?}");
    }
}
