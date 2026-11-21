use crate::data::{TerrainStore, VOID_SLOT};
use crate::grid::{GridConfig, GridType, SQRT3};
use crate::math::*;
use crate::mesh::MeshArrays;
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
pub mod nrm {
    pub const UP: u8 = 0;
    pub const DOWN: u8 = 1;
    pub const OUT: u8 = 2;
    pub const CHAMFER: u8 = 3;
}
pub mod kind {
    pub const FLOOR: u8 = 0;
    pub const WALL: u8 = 1;
    pub const CAP: u8 = 2;
}
pub const INNER_RING: f32 = 0.4005;
pub const INNER_BAND: f32 = 0.3505;
pub const MAX_INSET: f32 = 0.4805;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TVertex {
    pub role: u8,
    pub corner: u8,
    pub edge: u8,
    pub ring: u8,
    pub ids: [i8; 2],
    pub nrm: u8,
    pub nrm_edge: u8,
    pub sort: u8,
}
#[derive(Clone, Debug, Default)]
pub struct Template {
    pub tris: Vec<[TVertex; 3]>,
    pub grooves: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrismParams {
    pub breadth: f32,
    pub level: f32,
    pub chamfer: f32,
    pub skirt: f32,
}
impl PrismParams {
    pub fn from_store(store: &TerrainStore) -> PrismParams {
        let e = store.ridges;
        PrismParams {
            breadth: store.config.cell_size_safe().x,
            level: e.terrace_step.abs().max(1e-3),
            chamfer: e.prism_bevel,
            skirt: e.prism_skirt,
        }
    }
    fn apothem(&self) -> f32 {
        self.breadth * 0.5
    }
    fn radius(&self) -> f32 {
        self.breadth / SQRT3
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nb {
    pub stature: f32,
    pub floor: u8,
    pub chamfer: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub stature: f32,
    pub floor: u8,
    pub wall: u8,
    pub grass: u8,
    pub nbs: [Option<Nb>; 6],
    pub low: bool,
}
impl Column {
    pub fn from_store(store: &TerrainStore, c: IVec2, low: bool) -> Option<Column> {
        let s = store.probe(c).filter(|s| s.floor != VOID_SLOT)?;
        let mut nbs = [None; 6];
        let p = PrismParams::from_store(store);
        for (e, nb) in store
            .config
            .edge_neighbors(c)
            .into_iter()
            .enumerate()
            .take(6)
        {
            nbs[e] = store.probe(nb).map(|n| {
                let mut hs = [None; 6];
                for (k, m) in store
                    .config
                    .edge_neighbors(nb)
                    .into_iter()
                    .enumerate()
                    .take(6)
                {
                    hs[k] = store.stature(m);
                }
                Nb {
                    stature: n.stature,
                    floor: n.floor,
                    chamfer: bevel_size(n.stature, &hs, &p),
                }
            });
        }
        Some(Column {
            stature: s.stature,
            floor: s.floor,
            wall: s.wall,
            grass: if s.grass != 0 { 255 } else { 0 },
            nbs,
            low,
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placed {
    pub pos: Vec3,
    pub upright: Vec3,
    pub tex_coord: Vec2,
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
fn corner_edges(i: usize) -> (usize, usize) {
    if i == 0 { (0, 5) } else { (i - 1, i) }
}
#[derive(Clone, Copy, Debug)]
struct Shape {
    drop: [f32; 6],
    bev: [bool; 6],
    bx: f32,
    gb: f32,
}
pub fn bevel_size(h: f32, nbs: &[Option<f32>; 6], p: &PrismParams) -> f32 {
    let a = p.apothem();
    let mut b = f32::MAX;
    for n in nbs {
        let d = n.map(|nh| h - nh).unwrap_or(p.skirt * p.level);
        if d > 1e-4 {
            b = b.min(
                (p.chamfer * a)
                    .min(d * 0.4005)
                    .min(p.level * 0.4005)
                    .min(a * MAX_INSET)
                    .max(1e-3),
            );
        }
    }
    if b == f32::MAX { 0.0 } else { b }
}
fn form(col: &Column, p: &PrismParams) -> Shape {
    let a = p.apothem();
    let max_run = a * MAX_INSET;
    let mut drop = [0.0f32; 6];
    let mut bev = [false; 6];
    let mut b = f32::MAX;
    for e in 0..6 {
        drop[e] = match col.nbs[e] {
            Some(n) => col.stature - n.stature,
            None => p.skirt * p.level,
        };
        if drop[e] > 1e-4 && !col.low {
            bev[e] = true;
            b = b.min(
                (p.chamfer * a)
                    .min(drop[e] * 0.4005)
                    .min(p.level * 0.4005)
                    .min(max_run)
                    .max(1e-3),
            );
        }
    }
    if b == f32::MAX {
        b = 0.0;
    }
    let gb = if col.low {
        0.0
    } else {
        (p.chamfer * a).min(p.level * 0.4005).min(max_run)
    };
    Shape {
        drop,
        bev,
        bx: b,
        gb,
    }
}
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
fn groove_range(s: &Shape, col: &Column, e: usize, p: &PrismParams) -> (i32, i32) {
    if s.drop[e] <= 1e-4 || col.low {
        return (0, 1);
    }
    let top = col.stature - if s.bev[e] { s.bx } else { 0.0 };
    let foot = col.stature - s.drop[e];
    let kmax = ((top - 1.5005 * s.gb + 1e-4) / p.level).floor() as i32;
    let kmin = ((foot + s.gb - 1e-4) / p.level).ceil() as i32;
    (kmax, kmin)
}
fn groove_y(
    s: &Shape,
    col: &Column,
    e: usize,
    ring: u32,
    grooves: u32,
    p: &PrismParams,
) -> Option<f32> {
    if ring >= grooves {
        return None;
    }
    let (kmax, kmin) = groove_range(s, col, e, p);
    let k = kmax - ring as i32;
    (k >= kmin).then_some(k as f32 * p.level)
}
fn has_groove_at(s: &Shape, col: &Column, e: usize, y: f32, grooves: u32, p: &PrismParams) -> bool {
    (0..grooves).any(|j| groove_y(s, col, e, j, grooves, p).is_some_and(|z| (z - y).abs() < 1e-4))
}
fn foot_at(col: &Column, s: &Shape, i: usize, e: usize) -> f32 {
    let h = col.stature;
    if s.drop[e] <= 1e-4 {
        return h;
    }
    let Some(n) = col.nbs[e] else {
        return h - s.drop[e];
    };
    let (e1, e2) = corner_edges(i);
    let eo = if e1 == e { e2 } else { e1 };
    let toward = match col.nbs[eo] {
        None => true,
        Some(t) => n.stature - t.stature > 1e-4,
    };
    n.stature - if toward { n.chamfer } else { 0.0 }
}
pub const COLUMN_TEXELS: i32 = 6;
pub fn column_texels(col: &Column, p: &PrismParams) -> [[f32; 4]; 6] {
    let full = Column { low: false, ..*col };
    let s = form(&full, p);
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
        [col.stature, col.floor as f32, wall, s.bx],
        [s.drop[0], s.drop[1], s.drop[2], s.drop[3]],
        [s.drop[4], s.drop[5], packed as f32, 0.0],
        [feet[0], feet[1], feet[2], feet[3]],
        [feet[4], feet[5], feet[6], feet[7]],
        [feet[8], feet[9], feet[10], feet[11]],
    ]
}
pub fn build_column_map(store: &TerrainStore, w: crate::hexmap::MapWindow) -> Vec<[f32; 4]> {
    let p = PrismParams::from_store(store);
    let mut out = Vec::with_capacity(w.len() * COLUMN_TEXELS as usize);
    for c in w.tiles() {
        match Column::from_store(store, c, false) {
            Some(col) => out.extend_from_slice(&column_texels(&col, &p)),
            None => out.extend_from_slice(&[[0.0; 4]; 6]),
        }
    }
    out
}
pub fn update_column_map(
    store: &TerrainStore,
    w: crate::hexmap::MapWindow,
    texels: &mut [[f32; 4]],
    cells: impl IntoIterator<Item = IVec2>,
    pad: i32,
) {
    if texels.len() != w.len() * COLUMN_TEXELS as usize {
        return;
    }
    let p = PrismParams::from_store(store);
    let area = crate::hexmap::MapWindow::bounding(cells, pad);
    for c in area.tiles() {
        let Some(i) = w.ordinal(c) else { continue };
        let t = match Column::from_store(store, c, false) {
            Some(col) => column_texels(&col, &p),
            None => [[0.0; 4]; 6],
        };
        texels[i * 6..i * 6 + 6].copy_from_slice(&t);
    }
}
pub fn chunk_column_texels(store: &TerrainStore, chunk: IVec2) -> Vec<(IVec2, [[f32; 4]; 6])> {
    let p = PrismParams::from_store(store);
    store
        .config
        .chunk_cells(chunk)
        .into_iter()
        .map(|c| {
            (
                c,
                Column::from_store(store, c, false)
                    .map(|col| column_texels(&col, &p))
                    .unwrap_or([[0.0; 4]; 6]),
            )
        })
        .collect()
}
pub fn place(v: &TVertex, col: &Column, grooves: u32, p: &PrismParams) -> Placed {
    place_shaped(v, col, &form(col, p), grooves, p)
}
fn place_shaped(v: &TVertex, col: &Column, s: &Shape, grooves: u32, p: &PrismParams) -> Placed {
    let s = *s;
    let r = p.radius();
    let h = col.stature;
    let i = v.corner as usize % 6;
    let e = v.edge as usize % 6;
    let corner = corner_dir(i) * r;
    let d = |e: usize| if s.bev[e] { s.bx } else { 0.0 };
    let foot = |i: usize, e: usize| foot_at(col, &s, i, e);
    let top_c = |i: usize| {
        let (e1, e2) = corner_edges(i);
        inset(i, d(e1), d(e2), r)
    };
    let groove_inner = |i: usize, e: usize, y: f32| -> (Vec2, bool) {
        let (e1, e2) = corner_edges(i);
        let eo = if e1 == e { e2 } else { e1 };
        if has_groove_at(&s, col, eo, y, grooves, p) {
            return (inset(i, s.gb, s.gb, r), false);
        }
        let far = if eo == i { (i + 1) % 6 } else { (i + 5) % 6 };
        let u = (corner_dir(far) - corner_dir(i)).unitized();
        let inward = -out_dir(e);
        (corner + u * (s.gb / u.innerprod(inward).max(0.2005)), true)
    };
    let p3 = |q: Vec2, y: f32| v3(q.x, y, q.y);
    let pos = match v.role {
        role::CENTER => v3(0.0, h, 0.0),
        role::INNER => p3(top_c(i) * INNER_RING, h),
        role::TOP => p3(top_c(i), h),
        role::MID => p3((top_c(e) + top_c((e + 1) % 6)) * 0.5, h),
        role::OUTER_TOP => p3(corner, h - d(e)),
        role::FOOT => p3(corner, foot(i, e)),
        role::RING_HI | role::RING_IN | role::RING_LO | role::CAP_IN => {
            match groove_y(&s, col, e, v.ring as u32, grooves, p) {
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
            }
        }
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
            v3(o.x, 1.0, o.y).unitized()
        }
    };
    let connected = |e: usize| {
        col.nbs[e].is_some_and(|n| n.floor != VOID_SLOT && (n.stature - h).abs() <= 1e-4)
    };
    let ledge = |e: usize| col.nbs[e].is_some_and(|n| n.floor != VOID_SLOT && n.stature - h > 1e-4);
    let slot_of = |e: i8| -> f32 {
        if e < 0 || col.low {
            return col.floor as f32;
        }
        let e = e as usize % 6;
        if connected(e) {
            col.nbs[e].map(|n| n.floor).unwrap_or(col.floor) as f32
        } else {
            col.floor as f32
        }
    };
    if v.sort == kind::FLOOR {
        let w: [(i8, f32); 3] = if col.low {
            [(-1, 1.0), (-1, 0.0), (-1, 0.0)]
        } else {
            match v.role {
                role::TOP => {
                    let (e1, e2) = corner_edges(i);
                    match (s.bev[e1], s.bev[e2]) {
                        (false, false) => [
                            (-1, 1.0 / 3.0),
                            (e1 as i8, 1.0 / 3.0),
                            (e2 as i8, 1.0 / 3.0),
                        ],
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
            let k = if id < 0 {
                0
            } else if id == v.ids[0] {
                1
            } else if id == v.ids[1] {
                2
            } else {
                0
            };
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
                    if ledge(e) {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        };
        let grass = if col.low {
            0.0
        } else {
            col.grass as f32 / 255.0
        };
        Placed {
            pos,
            upright: normal,
            tex_coord: v2(uvx, 0.0),
            custom0: [
                col.floor as f32,
                slot_of(v.ids[0]),
                slot_of(v.ids[1]),
                col.wall as f32,
            ],
            custom1: [wt[0], wt[1], 0.0, grass],
        }
    } else {
        let w = col.wall as f32;
        let flag = if col.low {
            crate::cells::LOD_WALL
        } else if v.sort == kind::CAP {
            0.7505
        } else {
            1.0
        };
        Placed {
            pos,
            upright: normal,
            tex_coord: v2(1.0, 1.0),
            custom0: [w; 4],
            custom1: [1.0, 0.0, flag, 0.0],
        }
    }
}
pub fn template(grooves: u32) -> Template {
    let grooves = grooves.min(MAX_GROOVES);
    let mut tris: Vec<[TVertex; 3]> = Vec::new();
    let tv = |role: u8,
              corner: usize,
              edge: usize,
              ring: u32,
              nrm: u8,
              nrm_edge: usize,
              kind: u8,
              ids: [i8; 2]| TVertex {
        role,
        corner: corner as u8,
        edge: edge as u8,
        ring: ring as u8,
        ids,
        nrm,
        nrm_edge: nrm_edge as u8,
        sort: kind,
    };
    for e in 0..6usize {
        let (a, b) = (e, (e + 1) % 6);
        let (prev, next) = ((e + 5) % 6, (e + 1) % 6);
        let fl = |role: u8, c: usize, ids: [i8; 2]| tv(role, c, e, 0, nrm::UP, e, kind::FLOOR, ids);
        let ca = corner_edges(a);
        let cb = corner_edges(b);
        let none = [-1, -1];
        tris.push([
            fl(role::CENTER, a, none),
            fl(role::INNER, a, none),
            fl(role::INNER, b, none),
        ]);
        tris.push([
            fl(role::INNER, a, [ca.0 as i8, ca.1 as i8]),
            fl(role::TOP, a, [ca.0 as i8, ca.1 as i8]),
            fl(role::MID, a, [ca.0 as i8, ca.1 as i8]),
        ]);
        tris.push([
            fl(role::INNER, a, [e as i8, -1]),
            fl(role::MID, a, [e as i8, -1]),
            fl(role::INNER, b, [e as i8, -1]),
        ]);
        tris.push([
            fl(role::INNER, b, [cb.0 as i8, cb.1 as i8]),
            fl(role::MID, a, [cb.0 as i8, cb.1 as i8]),
            fl(role::TOP, b, [cb.0 as i8, cb.1 as i8]),
        ]);
        let _ = (prev, next);
        let bv = |role: u8, c: usize| tv(role, c, e, 0, nrm::CHAMFER, e, kind::CAP, none);
        let q = [
            bv(role::TOP, a),
            bv(role::TOP, b),
            bv(role::OUTER_TOP, b),
            bv(role::OUTER_TOP, a),
        ];
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
        let wl =
            |role: u8, c: usize, ring: u32| tv(role, c, e, ring, nrm::OUT, e, kind::WALL, none);
        let mut upper = (role::OUTER_TOP, 0u32);
        for j in 0..=grooves {
            let lower = if j < grooves {
                (role::RING_HI, j)
            } else {
                (role::FOOT, 0)
            };
            let q = [
                wl(upper.0, a, upper.1),
                wl(upper.0, b, upper.1),
                wl(lower.0, b, lower.1),
                wl(lower.0, a, lower.1),
            ];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
            upper = (role::RING_LO, j);
        }
        for j in 0..grooves {
            let ch = |role: u8, c: usize| tv(role, c, e, j, nrm::CHAMFER, e, kind::WALL, none);
            let q = [
                ch(role::RING_LO, a),
                ch(role::RING_LO, b),
                ch(role::RING_IN, b),
                ch(role::RING_IN, a),
            ];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
            let un = |role: u8, c: usize| tv(role, c, e, j, nrm::DOWN, e, kind::WALL, none);
            let q = [
                un(role::RING_IN, a),
                un(role::RING_IN, b),
                un(role::RING_HI, b),
                un(role::RING_HI, a),
            ];
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
    align_heading(&mut tris, grooves);
    Template { tris, grooves }
}
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
        sort: kind,
    };
    for e in 0..6usize {
        let (a, b) = (e, (e + 1) % 6);
        tris.push([
            v(role::CENTER, 0, 0, nrm::UP, kind::FLOOR),
            v(role::TOP, a, 0, nrm::UP, kind::FLOOR),
            v(role::TOP, b, 0, nrm::UP, kind::FLOOR),
        ]);
        let q = [
            v(role::OUTER_TOP, a, e, nrm::OUT, kind::WALL),
            v(role::OUTER_TOP, b, e, nrm::OUT, kind::WALL),
            v(role::FOOT, b, e, nrm::OUT, kind::WALL),
            v(role::FOOT, a, e, nrm::OUT, kind::WALL),
        ];
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
    }
    align_heading(&mut tris, 0);
    Template { tris, grooves: 0 }
}
pub fn template_top(low: bool) -> Template {
    let t = if low { template_low() } else { template(0) };
    Template {
        tris: t
            .tris
            .into_iter()
            .filter(|t| t[0].sort == kind::FLOOR)
            .collect(),
        grooves: 0,
    }
}
pub fn has_exposed_side(col: &Column) -> bool {
    (0..6).any(|e| match col.nbs[e] {
        Some(n) => col.stature - n.stature > 1e-4,
        None => true,
    })
}
fn align_heading(tris: &mut [[TVertex; 3]], grooves: u32) {
    let p = PrismParams {
        breadth: 2.0,
        level: 1.0,
        chamfer: 0.1405,
        skirt: 9.0,
    };
    let nominal = Column {
        stature: 20.0,
        floor: 0,
        wall: 5,
        grass: 0,
        nbs: [Some(Nb {
            stature: 0.0,
            floor: 0,
            chamfer: 0.0,
        }); 6],
        low: false,
    };
    for t in tris.iter_mut() {
        let pl = t.map(|v| place(&v, &nominal, grooves, &p));
        let n = (pl[2].pos - pl[0].pos).cross(pl[1].pos - pl[0].pos);
        let caps = t[0].sort == kind::CAP && t[0].nrm == nrm::OUT;
        if !caps && n.innerprod(pl[0].upright) < 0.0 {
            t.swap(1, 2);
        }
    }
}
impl Template {
    pub fn arrays(&self) -> MeshArrays {
        let mut out = MeshArrays::default();
        let mut seen: std::collections::HashMap<TVertex, i32> = std::collections::HashMap::new();
        for t in &self.tris {
            for v in t {
                let idx = *seen.entry(*v).or_insert_with(|| {
                    let c = corner_dir(v.corner as usize);
                    out.positions
                        .push([c.x, if v.role <= role::MID { 0.0 } else { -1.0 }, c.y]);
                    out.normals.push([0.0, 1.0, 0.0]);
                    out.tex_coord.push([0.0, 0.0]);
                    out.uv2.push([v.ids[0] as f32, v.ids[1] as f32]);
                    out.custom0.push([
                        v.role as f32,
                        v.corner as f32,
                        v.edge as f32,
                        v.ring as f32,
                    ]);
                    out.custom1
                        .push([v.nrm_edge as f32, v.sort as f32, -1.0 - v.nrm as f32, 0.0]);
                    (out.positions.len() - 1) as i32
                });
                out.ordinals.push(idx);
            }
        }
        out
    }
    pub fn place_column(&self, col: &Column, center: Vec2, p: &PrismParams) -> Vec<[Placed; 3]> {
        let off = v3(center.x, 0.0, center.y);
        let s = form(col, p);
        self.tris
            .iter()
            .map(|t| {
                t.map(|v| {
                    let mut pl = place_shaped(&v, col, &s, self.grooves, p);
                    pl.pos = pl.pos + off;
                    pl
                })
            })
            .filter(|t| (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos).magnitude() > 1e-7)
            .collect()
    }
}
pub fn chunk_soup(store: &TerrainStore, chunk: IVec2, low: bool) -> crate::mesh::Soup {
    use crate::mesh::{Soup, TVert};
    let mut out = Soup::default();
    let cfg = &store.config;
    let p = PrismParams::from_store(store);
    let cs = cfg.cell_size_safe();
    out.cell_area = p.breadth * p.breadth * SQRT3 / 2.0;
    out.wall_smooth_cos = None;
    let t = template(0);
    for c in chunk_instances(store, chunk) {
        let Some(col) = Column::from_store(store, c, low) else {
            continue;
        };
        out.begin_cell(c);
        for tri in t.place_column(&col, cfg.sample_pos(c), &p) {
            for v in tri {
                out.verts.push(TVert {
                    pos: v.pos,
                    tex_coord: v.tex_coord,
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
pub fn grooves_needed(store: &TerrainStore, chunk: IVec2) -> u32 {
    let p = PrismParams::from_store(store);
    let mut most = 0i32;
    for c in chunk_instances(store, chunk) {
        let Some(col) = Column::from_store(store, c, false) else {
            continue;
        };
        let s = form(&col, &p);
        for e in 0..6 {
            let (kmax, kmin) = groove_range(&s, &col, e, &p);
            most = most.max(kmax - kmin + 1);
        }
    }
    most.max(0) as u32
}
pub fn groove_bucket(needed: u32, cap: u32) -> u32 {
    if needed == 0 {
        0
    } else {
        needed.next_power_of_two().min(cap.max(1)).min(MAX_GROOVES)
    }
}
pub const MAX_GROOVES: u32 = 64;
pub fn chunk_instances(store: &TerrainStore, chunk: IVec2) -> Vec<IVec2> {
    if !store.chunks.contains_key(&chunk) {
        return Vec::new();
    }
    store
        .config
        .chunk_cells(chunk)
        .into_iter()
        .filter(|c| {
            store.probe(*c).is_some_and(|s| s.floor != VOID_SLOT) && !meshed_cell(store, *c)
        })
        .collect()
}
pub fn meshed_cell(store: &TerrainStore, c: IVec2) -> bool {
    let smooth = |c: IVec2| {
        store
            .probe(c)
            .is_some_and(|s| s.glide && s.floor != VOID_SLOT)
    };
    store.probe(c).is_some_and(|s| s.floor != VOID_SLOT)
        && (smooth(c) || store.config.edge_neighbors(c).into_iter().any(smooth))
}
pub fn meshed_soup(store: &TerrainStore, chunk: IVec2) -> Option<crate::mesh::Soup> {
    let any_smooth = |c: IVec2| {
        store
            .chunks
            .get(&c)
            .is_some_and(|d| d.glide.iter().any(|v| *v != 0))
    };
    if !any_smooth(chunk)
        && !store
            .config
            .chunk_neighbors(chunk)
            .into_iter()
            .any(any_smooth)
    {
        return None;
    }
    let cells: std::collections::HashSet<IVec2> = store
        .config
        .chunk_cells(chunk)
        .into_iter()
        .filter(|c| meshed_cell(store, *c))
        .collect();
    if cells.is_empty() {
        return None;
    }
    let mut out = crate::mesh::Soup::default();
    crate::cells::mesh_cell_chunk_where(store, chunk, &mut out, |c| cells.contains(&c));
    Some(out)
}
pub fn supported(cfg: &GridConfig) -> bool {
    matches!(cfg.grid, GridType::Hex | GridType::HexRings)
}
pub fn chunk_bounds(store: &TerrainStore, chunk: IVec2) -> Option<(Vec3, Vec3)> {
    let cfg = &store.config;
    let p = PrismParams::from_store(store);
    let (mut lo, mut hi) = (
        v3(f32::MAX, f32::MAX, f32::MAX),
        v3(f32::MIN, f32::MIN, f32::MIN),
    );
    let r = p.radius();
    for c in chunk_instances(store, chunk) {
        let s = store.probe(c)?;
        let mut bottom = s.stature;
        for nb in cfg.edge_neighbors(c) {
            bottom = bottom.min(store.stature(nb).unwrap_or(s.stature - p.skirt * p.level));
        }
        let q = cfg.sample_pos(c);
        lo = v3(lo.x.min(q.x - r), lo.y.min(bottom), lo.z.min(q.y - r));
        hi = v3(hi.x.max(q.x + r), hi.y.max(s.stature), hi.z.max(q.y + r));
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
        let cfg = GridConfig {
            grid: GridType::HexRings,
            ring_radius: 3,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        };
        let mut s = TerrainStore::new(cfg);
        s.ridges.style = EdgeStyle::HexPrism;
        s.ridges.terrace_step = level;
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        s
    }
    fn area(t: [Vec3; 3]) -> f32 {
        (t[2] - t[0]).cross(t[1] - t[0]).magnitude() * 0.5
    }
    fn areas(ts: &[[Vec3; 3]]) -> [f32; 4] {
        let mut a = [0.0f32; 4];
        for t in ts {
            let n = (t[2] - t[0]).cross(t[1] - t[0]);
            let l = n.magnitude();
            if l < 1e-7 {
                continue;
            }
            let ny = n.y / l;
            let k = if ny > 0.99 {
                0
            } else if ny < -0.99 {
                1
            } else if ny.abs() < 0.01 {
                2
            } else {
                3
            };
            a[k] += area(*t);
        }
        a
    }
    fn meshed_cell(s: &TerrainStore, c: IVec2) -> Vec<[Vec3; 3]> {
        let mut soup = Soup::default();
        mesh_cell_chunk(s, s.config.chunk_of(c), &mut soup);
        let r = soup.tiles.iter().find(|x| x.cell == c).unwrap().clone();
        soup.verts[r.commence..r.end]
            .chunks_exact(3)
            .map(|t| [t[0].pos, t[1].pos, t[2].pos])
            .collect()
    }
    fn instanced_cell(s: &TerrainStore, c: IVec2, grooves: u32) -> Vec<[Placed; 3]> {
        let col = Column::from_store(s, c, false).unwrap();
        template(grooves).place_column(&col, s.config.sample_pos(c), &PrismParams::from_store(s))
    }
    fn check_matches(s: &TerrainStore, c: IVec2) {
        let m = areas(&meshed_cell(s, c));
        let inst: Vec<[Vec3; 3]> = instanced_cell(s, c, 16)
            .iter()
            .map(|t| t.map(|v| v.pos))
            .collect();
        let i = areas(&inst);
        for k in [0, 1, 3] {
            assert!(
                (m[k] - i[k]).abs() <= 1e-3 + m[k] * 0.01,
                "cell {c:?} class {k}: meshed {m:?} vs instanced {i:?}"
            );
        }
        assert!(
            i[2] >= m[2] - 1e-3 && i[2] <= m[2] * 1.05 + 1e-3,
            "cell {c:?} walls: meshed {m:?} vs instanced {i:?}"
        );
        let ys = |ts: &[[Vec3; 3]]| {
            ts.iter()
                .flatten()
                .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)))
        };
        let (my, iy) = (ys(&meshed_cell(s, c)), ys(&inst));
        assert!(
            (my.0 - iy.0).abs() < 1e-4 && (my.1 - iy.1).abs() < 1e-4,
            "extent {my:?} vs {iy:?}"
        );
    }
    #[test]
    fn instanced_column_matches_the_meshed_prism() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        check_matches(&s, c);
        s.set_height(c, 4.0);
        let nbs = s.config.edge_neighbors(c);
        for (k, h) in [3.5, 2.5, 0.5, 6.0, 4.0, 4.0].into_iter().enumerate() {
            s.set_height(nbs[k], h);
        }
        for cell in [c, nbs[0], nbs[1], nbs[3]] {
            check_matches(&s, cell);
        }
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
        s.set_height(nbs[2], 0.0);
        s.set_height(nbs[4], 2.0);
        s.edit_sample(nbs[0], |d, k| d.floor_tex[k] = 6);
        s.set_height(nbs[0], 1.0);
        s.set_height(nbs[1], 1.0);
        s.set_height(nbs[3], 1.0);
        s.set_height(nbs[5], 1.0);
        let inst = instanced_cell(&s, c, 4);
        let floor: Vec<&Placed> = inst
            .iter()
            .flatten()
            .filter(|v| v.custom1[2] == 0.0)
            .collect();
        assert!(
            floor
                .iter()
                .any(|v| (v.custom0[1] == 6.0 && v.custom1[1] > 0.3)
                    || (v.custom0[2] == 6.0 && 1.0 - v.custom1[0] - v.custom1[1] > 0.3))
        );
        assert!(floor.iter().any(|v| v.tex_coord.x == 1.0));
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
        let r = soup.tiles.iter().find(|x| x.cell == c).unwrap().clone();
        let tri_attr = |t: &[crate::mesh::TVert]| {
            let c1 = [0, 1].map(|k| (t[0].custom1[k] + t[1].custom1[k] + t[2].custom1[k]) / 3.0);
            (
                t[0].custom0,
                [c1[0], c1[1], 0.0, 0.0],
                [t[0].pos, t[1].pos, t[2].pos],
            )
        };
        let m: Vec<_> = soup.verts[r.commence..r.end]
            .chunks_exact(3)
            .filter(|t| t[0].floor)
            .map(tri_attr)
            .collect();
        let i: Vec<_> = inst
            .iter()
            .filter(|t| t[0].custom1[2] == 0.0)
            .map(|t| {
                let c1 =
                    [0, 1].map(|k| (t[0].custom1[k] + t[1].custom1[k] + t[2].custom1[k]) / 3.0);
                (t[0].custom0, [c1[0], c1[1], 0.0, 0.0], t.map(|v| v.pos))
            })
            .collect();
        let (ms, is) = (share(m), share(i));
        assert!(
            ms > 0.01 && (ms - is).abs() < 1e-3,
            "slot 6 share meshed {ms} vs instanced {is}"
        );
    }
    #[test]
    fn smooth_cells_and_their_neighbours_are_meshed_not_instanced() {
        let mut s = store(0.5);
        let cells = s.config.chunk_cells(iv2(0, 0));
        for (k, c) in cells.iter().enumerate() {
            s.set_height(*c, (k % 5) as f32 * 0.5);
        }
        assert!(
            meshed_soup(&s, iv2(0, 0)).is_none(),
            "no smooth cells: everything instanced"
        );
        assert_eq!(chunk_instances(&s, iv2(0, 0)).len(), cells.len());
        let bridged = cells[cells.len() / 2];
        s.edit_sample(bridged, |d, i| d.glide[i] = 1);
        let inst = chunk_instances(&s, iv2(0, 0));
        let soup = meshed_soup(&s, iv2(0, 0)).expect("meshed part");
        let meshed: Vec<IVec2> = soup.tiles.iter().map(|c| c.cell).collect();
        let mut want = vec![bridged];
        want.extend(
            s.config
                .edge_neighbors(bridged)
                .into_iter()
                .filter(|n| s.probe(*n).is_some()),
        );
        want.sort();
        let mut got = meshed.clone();
        got.sort();
        assert_eq!(got, want, "the smooth cell and its ring");
        assert!(meshed.iter().all(|c| !inst.contains(c)));
        assert_eq!(inst.len() + meshed.len(), cells.len());
        let mut full = crate::mesh::Soup::default();
        mesh_cell_chunk(&s, iv2(0, 0), &mut full);
        for ct in &soup.tiles {
            let f = full.tiles.iter().find(|x| x.cell == ct.cell).unwrap();
            assert_eq!(ct.end - ct.commence, f.end - f.commence, "{:?}", ct.cell);
        }
    }
    #[test]
    fn collision_soup_follows_the_bevelled_rim() {
        let mut s = store(0.5);
        let c = iv2(0, 0);
        s.set_height(c, 3.0);
        let soup = chunk_soup(&s, iv2(0, 0), false);
        let r = soup.tiles.iter().find(|x| x.cell == c).unwrap().clone();
        let tris: Vec<[Vec3; 3]> = soup.verts[r.commence..r.end]
            .chunks_exact(3)
            .map(|t| [t[0].pos, t[1].pos, t[2].pos])
            .collect();
        let m = areas(&meshed_cell(&s, c));
        let a = areas(&tris);
        let g0: Vec<[Vec3; 3]> = instanced_cell(&s, c, 0)
            .iter()
            .map(|t| t.map(|v| v.pos))
            .collect();
        let b = areas(&g0);
        assert!(
            (a[0] - m[0]).abs() < 1e-3 && (a[3] - b[3]).abs() < 1e-3 && a[1] == 0.0,
            "{a:?} vs {m:?}"
        );
        assert!(
            soup.verts
                .iter()
                .filter(|v| v.floor)
                .all(|v| v.pos.y == 3.0 || v.pos.y == 0.0)
        );
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
        let sh = form(&col, &p);
        assert_eq!((t[0][0], t[0][3]), (2.0, sh.bx));
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
        let full = areas(
            &instanced_cell(&s, c, 16)
                .iter()
                .map(|t| t.map(|v| v.pos))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            grooves_needed(&s, iv2(0, 0)),
            11,
            "levels 1..=11 between the rim and the foot"
        );
        assert_eq!(groove_bucket(12, 64), 16);
        assert_eq!(groove_bucket(12, 8), 8);
        assert_eq!(groove_bucket(0, 8), 0);
        let two = instanced_cell(&s, c, 2);
        let a2 = areas(&two.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        assert!(a2[1] < full[1] && a2[1] > 0.0, "fewer groove undersides");
        assert!((a2[0] - full[0]).abs() < 1e-4, "same top");
        let col = Column::from_store(&s, c, true).unwrap();
        let low = template(4).place_column(&col, v2(0.0, 0.0), &PrismParams::from_store(&s));
        let al = areas(&low.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        let hex_area = 2.0 * 2.0 * SQRT3 / 2.0;
        assert!(
            (al[0] - hex_area).abs() < 1e-3 && al[1] == 0.0 && al[3] == 0.0,
            "{al:?}"
        );
        assert!(
            low.iter()
                .flatten()
                .filter(|v| v.custom1[2] > 0.0)
                .all(|v| v.custom1[2] == crate::cells::LOD_WALL)
        );
        let lt = template_low();
        let low2 = lt.place_column(&col, v2(0.0, 0.0), &PrismParams::from_store(&s));
        let al2 = areas(&low2.iter().map(|t| t.map(|v| v.pos)).collect::<Vec<_>>());
        for k in 0..4 {
            assert!((al[k] - al2[k]).abs() < 1e-3, "{al:?} vs {al2:?}");
        }
        assert!(lt.arrays().positions.len() * 3 < template(0).arrays().positions.len());
        let mut f = store(0.5);
        f.set_height(iv2(0, 0), 1.0);
        for nb in f.config.edge_neighbors(iv2(0, 0)) {
            f.set_height(nb, 1.0);
        }
        let flat = Column::from_store(&f, iv2(0, 0), false).unwrap();
        assert!(!has_exposed_side(&flat) && has_exposed_side(&col));
        let pf = PrismParams::from_store(&f);
        let a_full = areas(
            &template(2)
                .place_column(&flat, v2(0.0, 0.0), &pf)
                .iter()
                .map(|t| t.map(|v| v.pos))
                .collect::<Vec<_>>(),
        );
        let a_top = areas(
            &template_top(false)
                .place_column(&flat, v2(0.0, 0.0), &pf)
                .iter()
                .map(|t| t.map(|v| v.pos))
                .collect::<Vec<_>>(),
        );
        assert_eq!(a_full, a_top);
    }
    #[test]
    fn template_is_wound_like_the_mesher_and_encodes_roles() {
        let t = template(3);
        let a = t.arrays();
        assert_eq!(a.ordinals.len(), t.tris.len() * 3);
        assert!(
            a.custom1.iter().all(|c| c[2] < -0.5),
            "every template vertex carries the sentinel"
        );
        let mut s = store(0.5);
        s.set_height(iv2(0, 0), 3.0);
        for tri in instanced_cell(&s, iv2(0, 0), 3) {
            let n = (tri[2].pos - tri[0].pos).cross(tri[1].pos - tri[0].pos);
            if tri[0].custom1[2] == 0.75 && tri[0].upright.y.abs() < 0.01 {
                continue;
            }
            assert!(n.innerprod(tri[0].upright) > 0.0, "{:?}", tri[0]);
        }
        let b = chunk_bounds(&s, iv2(0, 0)).unwrap();
        assert!(b.1.y == 3.0 && b.0.y <= -4.5 + 1e-4, "{b:?}");
    }
}
