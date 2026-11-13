//! Grid topologies. Every terrain stores one *sample* per grid site:
//!
//! * `Square`   – samples are lattice **vertices**; marching squares meshes the cells between them.
//! * `Triangle` – samples are equilateral triangle **cells** (rhombus addressing `x = 2i + orient`).
//! * `Hex`      – samples are pointy-top hex **cells** in odd-r offset coordinates.
//! * `HexRings` – samples are pointy-top hex cells in **axial** coordinates, grouped into hexagonal
//!   chunk disks (the "hexmod" layout used by the game's server); optional torus wrap.
//!
//! All positions are in the terrain's local XZ plane.

use crate::math::*;

pub const SQRT3: f32 = 1.732_050_8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GridType {
    #[default]
    Square = 0,
    Triangle = 1,
    Hex = 2,
    HexRings = 3,
}

impl GridType {
    pub fn from_i64(v: i64) -> GridType {
        match v {
            1 => GridType::Triangle,
            2 => GridType::Hex,
            3 => GridType::HexRings,
            _ => GridType::Square,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            GridType::Square => "square",
            GridType::Triangle => "triangle",
            GridType::Hex => "hex",
            GridType::HexRings => "hex_rings",
        }
    }
    pub fn is_cell_mode(self) -> bool {
        self != GridType::Square
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GridConfig {
    pub grid: GridType,
    /// Cells per chunk edge (square/triangle rows/hex). Triangle chunks hold `2 * size` columns.
    pub chunk_size: i32,
    pub cell_size: Vec2,
    /// Disk radius of a hex-rings chunk.
    pub ring_radius: i32,
    /// Hex-rings torus wrap in chunks (0 = no wrap).
    pub wrap: IVec2,
    /// Hex-rings torus wrap as an **axial cell period** `P`: cell `(q, r)` is the same cell as
    /// `(q mod P, r mod P)`, exactly like kg_hex `Torus::wrap_large` (`P = 19k` with
    /// `ring_radius = 2`). Must be a multiple of the chunk area `3R(R+1)+1` so whole chunks wrap
    /// onto whole chunks (see [`GridConfig::wrap_period_valid`]). 0 = off. Takes precedence over
    /// `wrap`.
    pub wrap_period: i32,
}

impl Default for GridConfig {
    fn default() -> Self {
        GridConfig { grid: GridType::Square, chunk_size: 32, cell_size: v2(2.0, 2.0), ring_radius: 4, wrap: iv2(0, 0), wrap_period: 0 }
    }
}

/// kg_hex: a large (terrain) hex is the small-hex lattice scaled by this in axial space.
pub const SMALL_PER_LARGE_AXIS: i32 = 3;

const HEX_EVEN: [IVec2; 6] = [iv2(1, 0), iv2(0, 1), iv2(-1, 1), iv2(-1, 0), iv2(-1, -1), iv2(0, -1)];
const HEX_ODD: [IVec2; 6] = [iv2(1, 0), iv2(1, 1), iv2(0, 1), iv2(-1, 0), iv2(0, -1), iv2(1, -1)];
const AXIAL: [IVec2; 6] = [iv2(1, 0), iv2(0, 1), iv2(-1, 1), iv2(-1, 0), iv2(0, -1), iv2(1, -1)];

fn floor_div(a: i32, b: i32) -> i32 {
    debug_assert!(b > 0);
    a.div_euclid(b)
}

impl GridConfig {
    pub fn cell_size_safe(&self) -> Vec2 {
        v2(self.cell_size.x.max(0.01), self.cell_size.y.max(0.01))
    }

    /// Bounding box of samples stored per chunk.
    pub fn samples_per_chunk(&self) -> IVec2 {
        let n = self.chunk_size.max(1);
        match self.grid {
            GridType::Square => iv2(n + 1, n + 1),
            GridType::Hex => iv2(n, n),
            GridType::Triangle => iv2(2 * n, n),
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                iv2(2 * r + 1, 2 * r + 1)
            }
        }
    }

    /// Distance between chunk origins in samples (square chunks overlap by one border row).
    pub fn chunk_stride(&self) -> IVec2 {
        let s = self.samples_per_chunk();
        if self.grid == GridType::Square { s - iv2(1, 1) } else { s }
    }

    /// Every (chunk, local) slot holding a copy of `sample`: square border vertices are
    /// duplicated into the chunks to the left/top; other grids have exactly one owner.
    pub fn sample_slots(&self, sample: IVec2) -> Vec<(IVec2, IVec2)> {
        let ch = self.chunk_of(sample);
        let l = self.local_of(sample);
        let mut v = vec![(ch, l)];
        if self.grid == GridType::Square {
            let n = self.chunk_stride();
            if l.x == 0 {
                v.push((ch + iv2(-1, 0), iv2(n.x, l.y)));
            }
            if l.y == 0 {
                v.push((ch + iv2(0, -1), iv2(l.x, n.y)));
            }
            if l.x == 0 && l.y == 0 {
                v.push((ch + iv2(-1, -1), iv2(n.x, n.y)));
            }
        }
        v
    }

    pub fn has_local(&self, local: IVec2) -> bool {
        let s = self.samples_per_chunk();
        if local.x < 0 || local.y < 0 || local.x >= s.x || local.y >= s.y {
            return false;
        }
        if self.grid == GridType::HexRings {
            let r = self.ring_radius.max(1);
            let (dq, dr) = (local.x - r, local.y - r);
            return dq.abs() <= r && dr.abs() <= r && (dq + dr).abs() <= r;
        }
        true
    }

    pub fn chunk_of(&self, sample: IVec2) -> IVec2 {
        match self.grid {
            GridType::HexRings => {
                let s = self.wrap_sample(sample);
                to_lower_res(s.x, s.y, self.ring_radius.max(1))
            }
            _ => {
                let s = self.chunk_stride();
                iv2(floor_div(sample.x, s.x), floor_div(sample.y, s.y))
            }
        }
    }

    pub fn local_of(&self, sample: IVec2) -> IVec2 {
        match self.grid {
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                let s = self.wrap_sample(sample);
                let chunk = to_lower_res(s.x, s.y, r);
                let c = chunk_center_hex(chunk.x, chunk.y, r);
                iv2(s.x - c.x + r, s.y - c.y + r)
            }
            _ => {
                let s = self.chunk_stride();
                iv2(sample.x.rem_euclid(s.x), sample.y.rem_euclid(s.y))
            }
        }
    }

    pub fn global_of(&self, chunk: IVec2, local: IVec2) -> IVec2 {
        match self.grid {
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                let c = chunk_center_hex(chunk.x, chunk.y, r);
                iv2(c.x + local.x - r, c.y + local.y - r)
            }
            _ => chunk * self.chunk_stride() + local,
        }
    }

    /// Global coordinates of every sample a chunk owns (local row-major order: `local.y`
    /// outer, `local.x` inner; hex-rings chunks skip the corners of their bounding box).
    pub fn chunk_cells(&self, chunk: IVec2) -> Vec<IVec2> {
        let size = self.samples_per_chunk();
        let mut v = Vec::with_capacity((size.x * size.y) as usize);
        for y in 0..size.y {
            for x in 0..size.x {
                if self.has_local(iv2(x, y)) {
                    v.push(self.global_of(chunk, iv2(x, y)));
                }
            }
        }
        v
    }

    /// The sample at the centre of a chunk (hex rings: the disk centre).
    pub fn chunk_center_cell(&self, chunk: IVec2) -> IVec2 {
        match self.grid {
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                chunk_center_hex(chunk.x, chunk.y, r)
            }
            _ => self.global_of(chunk, self.samples_per_chunk() / 2),
        }
    }

    /// Canonicalizes a hex-rings sample into the wrapped lap (identity elsewhere).
    pub fn wrap_sample(&self, sample: IVec2) -> IVec2 {
        if self.grid != GridType::HexRings {
            return sample;
        }
        let r = self.ring_radius.max(1);
        if self.wrap_period_valid() {
            // Canonical cell = the same offset from the chunk centre wrapped into [0, P)² (the
            // kg_hex torus slot rule), so a chunk's disk stays together across the seam.
            let p = self.wrap_period;
            let chunk = to_lower_res(sample.x, sample.y, r);
            let c = chunk_center_hex(chunk.x, chunk.y, r);
            let w = iv2(c.x.rem_euclid(p), c.y.rem_euclid(p));
            return w + (sample - c);
        }
        if self.wrap.x <= 0 || self.wrap.y <= 0 {
            return sample;
        }
        let chunk = to_lower_res(sample.x, sample.y, r);
        let wrapped = iv2(chunk.x.rem_euclid(self.wrap.x), chunk.y.rem_euclid(self.wrap.y));
        if wrapped == chunk {
            return sample;
        }
        let c = chunk_center_hex(chunk.x, chunk.y, r);
        let w = chunk_center_hex(wrapped.x, wrapped.y, r);
        iv2(w.x + sample.x - c.x, w.y + sample.y - c.y)
    }

    /// Chunk area in cells (hex rings: `3R(R+1)+1`, 19 for the game's radius-2 patches).
    pub fn chunk_area(&self) -> i32 {
        match self.grid {
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                3 * r * (r + 1) + 1
            }
            _ => {
                let s = self.chunk_stride();
                s.x * s.y
            }
        }
    }

    /// Whether `wrap_period` is active: hex rings, `P > 0` and a multiple of the chunk area.
    pub fn wrap_period_valid(&self) -> bool {
        self.grid == GridType::HexRings && self.wrap_period > 0 && self.wrap_period % self.chunk_area() == 0
    }

    /// Canonical coordinate of a chunk under the active wrap (identity without wrap).
    pub fn wrap_chunk(&self, chunk: IVec2) -> IVec2 {
        if self.grid != GridType::HexRings {
            return chunk;
        }
        self.chunk_of(chunk_center_hex(chunk.x, chunk.y, self.ring_radius.max(1)))
    }

    /// A chunk's cells in kg_hex spiral order (`Hex::range`: centre, then ring `k` starting at
    /// `centre + (0, -k)` and walking E, SE, SW, W, NW, NE), which is the order of
    /// `kg_hex::patch::LARGE_LOCAL` for radius 2. Hex rings only (other grids: row-major).
    pub fn chunk_cells_spiral(&self, chunk: IVec2) -> Vec<IVec2> {
        if self.grid != GridType::HexRings {
            return self.chunk_cells(chunk);
        }
        let r = self.ring_radius.max(1);
        spiral(chunk_center_hex(chunk.x, chunk.y, r), r)
    }

    /// Small (kg_hex `SmallHex`) lattice: `SMALL_PER_LARGE_AXIS`× finer than the hex-rings
    /// cells in axial space, sharing the origin. Terrain-local XZ centre of a small hex.
    pub fn small_hex_pos(&self, s: IVec2) -> Vec2 {
        let k = SMALL_PER_LARGE_AXIS as f32;
        let sp = self.hex_spacing() / k;
        v2(sp.x * (s.x as f32 + 0.5 * s.y as f32), sp.y * s.y as f32)
    }

    /// Small hex containing a terrain-local XZ point.
    pub fn world_to_small_hex(&self, p: Vec2) -> IVec2 {
        let radius = self.hex_radius() / SMALL_PER_LARGE_AXIS as f32;
        axial_round((p.x * SQRT3 / 3.0 - p.y / 3.0) / radius, p.y * 2.0 / 3.0 / radius)
    }

    /// The cell owning a small hex (kg_hex `SmallHex::large`: `div_floor(q + 1, 3)` per axis).
    pub fn small_hex_owner(&self, s: IVec2) -> IVec2 {
        let k = SMALL_PER_LARGE_AXIS;
        iv2(floor_div(s.x + k / 2, k), floor_div(s.y + k / 2, k))
    }

    fn hex_spacing(&self) -> Vec2 {
        let cs = self.cell_size_safe();
        v2(cs.x, cs.x * 0.866_025_4)
    }

    /// Circumradius of a hex cell.
    pub fn hex_radius(&self) -> f32 {
        self.cell_size_safe().x / SQRT3
    }

    /// Terrain-local XZ position of a sample (vertex for square, cell center otherwise).
    pub fn sample_pos(&self, s: IVec2) -> Vec2 {
        let cs = self.cell_size_safe();
        match self.grid {
            GridType::Square => v2(s.x as f32 * cs.x, s.y as f32 * cs.y),
            GridType::Hex => {
                let sp = self.hex_spacing();
                v2(sp.x * (s.x as f32 + 0.5 * (s.y & 1) as f32), sp.y * s.y as f32)
            }
            GridType::HexRings => {
                let sp = self.hex_spacing();
                v2(sp.x * (s.x as f32 + 0.5 * s.y as f32), sp.y * s.y as f32)
            }
            GridType::Triangle => {
                let c = self.tri_corners(s);
                (c[0] + c[1] + c[2]) / 3.0
            }
        }
    }

    /// Sample nearest to / containing a terrain-local XZ point.
    pub fn world_to_sample(&self, p: Vec2) -> IVec2 {
        let cs = self.cell_size_safe();
        match self.grid {
            GridType::Square => iv2((p.x / cs.x).round() as i32, (p.y / cs.y).round() as i32),
            GridType::Hex => {
                // Exact nearest-center search over the 3x3 offset neighbourhood.
                let sp = self.hex_spacing();
                let row = (p.y / sp.y).round() as i32;
                let col = (p.x / sp.x - 0.5 * (row & 1) as f32).round() as i32;
                let mut best = iv2(col, row);
                let mut bd = f32::MAX;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let c = iv2(col + dx, row + dy);
                        let d = (self.sample_pos(c) - p).length_squared();
                        if d < bd {
                            bd = d;
                            best = c;
                        }
                    }
                }
                best
            }
            GridType::HexRings => {
                let radius = self.hex_radius();
                let q = (p.x * SQRT3 / 3.0 - p.y / 3.0) / radius;
                let r = p.y * 2.0 / 3.0 / radius;
                axial_round(q, r)
            }
            GridType::Triangle => {
                let h = self.tri_row_height();
                let beta = p.y / h;
                let alpha = p.x / cs.x - beta * 0.5;
                let i = alpha.floor() as i32;
                let j = beta.floor() as i32;
                let a = alpha - i as f32;
                let b = beta - j as f32;
                iv2(2 * i + if a + b < 1.0 { 0 } else { 1 }, j)
            }
        }
    }

    pub fn world_to_cell_unwrapped(&self, p: Vec2) -> IVec2 {
        self.world_to_sample(p)
    }

    fn tri_row_height(&self) -> f32 {
        self.cell_size_safe().x * 0.866_025_4
    }

    fn tri_lattice(&self, i: i32, j: i32) -> Vec2 {
        let s = self.cell_size_safe().x;
        v2(i as f32 * s + j as f32 * s * 0.5, j as f32 * self.tri_row_height())
    }

    fn tri_corners(&self, c: IVec2) -> [Vec2; 3] {
        let i = c.x.div_euclid(2);
        let j = c.y;
        if c.x & 1 == 0 {
            [self.tri_lattice(i, j), self.tri_lattice(i + 1, j), self.tri_lattice(i, j + 1)]
        } else {
            [self.tri_lattice(i + 1, j + 1), self.tri_lattice(i + 1, j), self.tri_lattice(i, j + 1)]
        }
    }

    /// Corner positions (terrain-local XZ) of a cell-mode cell, in edge order: edge `k`
    /// joins `corners[edge_corners(k).0]` and `corners[edge_corners(k).1]`.
    pub fn cell_corners(&self, c: IVec2) -> Vec<Vec2> {
        match self.grid {
            GridType::Triangle => self.tri_corners(c).to_vec(),
            GridType::Hex | GridType::HexRings => {
                let center = self.sample_pos(c);
                let r = self.hex_radius();
                (0..6)
                    .map(|i| {
                        let a = (-30.0f32 + 60.0 * i as f32).to_radians();
                        center + v2(r * a.cos(), r * a.sin())
                    })
                    .collect()
            }
            GridType::Square => {
                let cs = self.cell_size_safe();
                let o = v2(c.x as f32 * cs.x, c.y as f32 * cs.y);
                vec![o, o + v2(cs.x, 0.0), o + v2(cs.x, cs.y), o + v2(0.0, cs.y)]
            }
        }
    }

    /// Edge neighbours of a cell-mode cell, indexed by edge.
    pub fn edge_neighbors(&self, c: IVec2) -> Vec<IVec2> {
        match self.grid {
            GridType::Hex => {
                let t = if c.y & 1 == 1 { &HEX_ODD } else { &HEX_EVEN };
                t.iter().map(|d| c + *d).collect()
            }
            GridType::HexRings => AXIAL.iter().map(|d| c + *d).collect(),
            GridType::Triangle => {
                if c.x & 1 == 0 {
                    vec![iv2(c.x + 1, c.y), iv2(c.x - 1, c.y), iv2(c.x + 1, c.y - 1)]
                } else {
                    vec![iv2(c.x - 1, c.y), iv2(c.x - 1, c.y + 1), iv2(c.x + 1, c.y)]
                }
            }
            GridType::Square => vec![iv2(c.x + 1, c.y), iv2(c.x, c.y + 1), iv2(c.x - 1, c.y), iv2(c.x, c.y - 1)],
        }
    }

    /// Corner index pair carrying edge `edge`.
    pub fn edge_corners(&self, _c: IVec2, edge: usize) -> (usize, usize) {
        match self.grid {
            GridType::Triangle => [(1, 2), (0, 2), (0, 1)][edge],
            GridType::Hex | GridType::HexRings => (edge, (edge + 1) % 6),
            GridType::Square => [(1, 2), (2, 3), (3, 0), (0, 1)][edge],
        }
    }

    /// Neighbouring samples used by the smooth tool (4/6/3 connectivity).
    pub fn sample_neighbors(&self, s: IVec2) -> Vec<IVec2> {
        match self.grid {
            GridType::Square => vec![iv2(s.x + 1, s.y), iv2(s.x - 1, s.y), iv2(s.x, s.y + 1), iv2(s.x, s.y - 1)],
            _ => self.edge_neighbors(s),
        }
    }

    /// All samples whose position may lie inside the terrain-local rectangle `[min, max]`.
    pub fn samples_in_rect(&self, min: Vec2, max: Vec2) -> Vec<IVec2> {
        let cs = self.cell_size_safe();
        let mut out = Vec::new();
        match self.grid {
            GridType::Square => {
                let x0 = (min.x / cs.x).floor() as i32;
                let x1 = (max.x / cs.x).ceil() as i32;
                let z0 = (min.y / cs.y).floor() as i32;
                let z1 = (max.y / cs.y).ceil() as i32;
                for z in z0..=z1 {
                    for x in x0..=x1 {
                        out.push(iv2(x, z));
                    }
                }
            }
            GridType::Hex => {
                let sp = self.hex_spacing();
                let r0 = (min.y / sp.y).floor() as i32 - 1;
                let r1 = (max.y / sp.y).ceil() as i32 + 1;
                let c0 = (min.x / sp.x).floor() as i32 - 1;
                let c1 = (max.x / sp.x).ceil() as i32 + 1;
                for r in r0..=r1 {
                    for c in c0..=c1 {
                        out.push(iv2(c, r));
                    }
                }
            }
            GridType::HexRings => {
                let sp = self.hex_spacing();
                let r0 = (min.y / sp.y).floor() as i32 - 1;
                let r1 = (max.y / sp.y).ceil() as i32 + 1;
                for r in r0..=r1 {
                    let q0 = (min.x / sp.x - 0.5 * r as f32).floor() as i32 - 1;
                    let q1 = (max.x / sp.x - 0.5 * r as f32).ceil() as i32 + 1;
                    for q in q0..=q1 {
                        out.push(iv2(q, r));
                    }
                }
            }
            GridType::Triangle => {
                let h = self.tri_row_height();
                let j0 = (min.y / h).floor() as i32 - 1;
                let j1 = (max.y / h).ceil() as i32 + 1;
                for j in j0..=j1 {
                    let i0 = (min.x / cs.x - j as f32 * 0.5).floor() as i32 - 1;
                    let i1 = (max.x / cs.x - j as f32 * 0.5).ceil() as i32 + 1;
                    for i in i0..=i1 {
                        out.push(iv2(2 * i, j));
                        out.push(iv2(2 * i + 1, j));
                    }
                }
            }
        }
        out
    }

    /// Whole-cell selection used by "grid aligned" painting: a hex disk of radius `size - 1`
    /// (hex modes), a lattice hexagon of side `size` (triangle), or a square block (square).
    pub fn grid_aligned(&self, center: Vec2, size: i32) -> Vec<IVec2> {
        let size = size.max(1);
        match self.grid {
            GridType::Hex => {
                let c = self.world_to_sample(center);
                let ca = offset_to_axial(c);
                disk(ca, size - 1).into_iter().map(axial_to_offset).collect()
            }
            GridType::HexRings => disk(self.world_to_sample(center), size - 1),
            GridType::Triangle => {
                let cs = self.cell_size_safe();
                let h = self.tri_row_height();
                let beta = center.y / h;
                let alpha = center.x / cs.x - beta * 0.5;
                // nearest lattice point
                let (i0, j0) = (alpha.floor() as i32, beta.floor() as i32);
                let mut best = iv2(i0, j0);
                let mut bd = f32::MAX;
                for di in 0..2 {
                    for dj in 0..2 {
                        let p = self.tri_lattice(i0 + di, j0 + dj);
                        let d = (p - center).length_squared();
                        if d < bd {
                            bd = d;
                            best = iv2(i0 + di, j0 + dj);
                        }
                    }
                }
                let n = size;
                let mut out = Vec::new();
                let hd = |a: IVec2, b: IVec2| {
                    let d = a - b;
                    (d.x.abs() + d.y.abs() + (d.x + d.y).abs()) / 2
                };
                for i in (best.x - n - 1)..=(best.x + n + 1) {
                    for j in (best.y - n - 1)..=(best.y + n + 1) {
                        let a = [iv2(i, j), iv2(i + 1, j), iv2(i, j + 1)];
                        if a.iter().all(|p| hd(*p, best) <= n) {
                            out.push(iv2(2 * i, j));
                        }
                        let b = [iv2(i + 1, j + 1), iv2(i + 1, j), iv2(i, j + 1)];
                        if b.iter().all(|p| hd(*p, best) <= n) {
                            out.push(iv2(2 * i + 1, j));
                        }
                    }
                }
                out
            }
            GridType::Square => {
                let c = self.world_to_sample(center);
                let r = size - 1;
                let mut out = Vec::new();
                for z in -r..=r {
                    for x in -r..=r {
                        out.push(c + iv2(x, z));
                    }
                }
                out
            }
        }
    }

    /// Terrain-local XZ outline of a chunk (for gizmos / chunk manager).
    pub fn chunk_outline(&self, chunk: IVec2) -> Vec<Vec2> {
        match self.grid {
            GridType::Square | GridType::Hex | GridType::Triangle => {
                let s = self.samples_per_chunk();
                let a = self.sample_pos(self.global_of(chunk, iv2(0, 0)));
                let b = self.sample_pos(self.global_of(chunk, iv2(s.x - 1, 0)));
                let c = self.sample_pos(self.global_of(chunk, iv2(s.x - 1, s.y - 1)));
                let d = self.sample_pos(self.global_of(chunk, iv2(0, s.y - 1)));
                if self.grid == GridType::Square {
                    let cs = self.cell_size_safe();
                    let n = self.chunk_stride().x as f32;
                    let o = v2(chunk.x as f32 * n * cs.x, chunk.y as f32 * n * cs.y);
                    vec![o, o + v2(n * cs.x, 0.0), o + v2(n * cs.x, n * cs.y), o + v2(0.0, n * cs.y)]
                } else {
                    vec![a, b, c, d]
                }
            }
            GridType::HexRings => {
                let r = self.ring_radius.max(1);
                let center = chunk_center_hex(chunk.x, chunk.y, r);
                (0..6)
                    .map(|i| self.sample_pos(center + AXIAL[i] * iv2(r, r)))
                    .collect()
            }
        }
    }

    /// Terrain-local centre of a chunk (mean of its outline).
    pub fn chunk_center_pos(&self, chunk: IVec2) -> Vec2 {
        let o = self.chunk_outline(chunk);
        let n = o.len().max(1) as f32;
        o.iter().fold(Vec2::ZERO, |a, p| a + *p) / n
    }

    /// Distant low-detail ring: the chunks whose centre lies farther than `distance` from the
    /// terrain-local `focus` (XZ). Chunks already in `prev` stay low until they come
    /// `hysteresis` closer than `distance`, so a focus hovering on the boundary does not make
    /// chunks flip every frame. `distance <= 0` disables the ring (empty set).
    pub fn lod_low_chunks(
        &self,
        chunks: impl IntoIterator<Item = IVec2>,
        focus: Vec2,
        distance: f32,
        hysteresis: f32,
        prev: &std::collections::HashSet<IVec2>,
    ) -> std::collections::HashSet<IVec2> {
        if distance <= 0.0 {
            return Default::default();
        }
        chunks
            .into_iter()
            .filter(|c| {
                let d = (self.chunk_center_pos(*c) - focus).length();
                d > if prev.contains(c) { distance - hysteresis.max(0.0) } else { distance }
            })
            .collect()
    }

    /// Chunk containing a terrain-local point.
    pub fn chunk_at(&self, p: Vec2) -> IVec2 {
        match self.grid {
            GridType::Square => {
                let cs = self.cell_size_safe();
                let n = self.chunk_size.max(1) as f32;
                iv2((p.x / (cs.x * n)).floor() as i32, (p.y / (cs.y * n)).floor() as i32)
            }
            _ => self.chunk_of(self.world_to_sample(p)),
        }
    }

    /// Chunks adjacent to a chunk (used when a sample edit can change a neighbour's mesh).
    pub fn chunk_neighbors(&self, chunk: IVec2) -> Vec<IVec2> {
        match self.grid {
            GridType::HexRings => AXIAL.iter().map(|d| self.wrap_chunk(chunk + *d)).collect(),
            _ => {
                let mut v = Vec::with_capacity(8);
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        if dx != 0 || dz != 0 {
                            v.push(chunk + iv2(dx, dz));
                        }
                    }
                }
                v
            }
        }
    }
}

pub fn offset_to_axial(c: IVec2) -> IVec2 {
    iv2(c.x - (c.y - (c.y & 1)).div_euclid(2), c.y)
}
pub fn axial_to_offset(a: IVec2) -> IVec2 {
    iv2(a.x + (a.y - (a.y & 1)).div_euclid(2), a.y)
}
pub fn axial_distance(a: IVec2, b: IVec2) -> i32 {
    let d = a - b;
    (d.x.abs() + d.y.abs() + (d.x + d.y).abs()) / 2
}
pub fn disk(center: IVec2, radius: i32) -> Vec<IVec2> {
    let mut out = Vec::new();
    if radius < 0 {
        return out;
    }
    for dq in -radius..=radius {
        for dr in (-radius).max(-dq - radius)..=radius.min(-dq + radius) {
            out.push(center + iv2(dq, dr));
        }
    }
    out
}

/// Hex disk in kg_hex / hexx spiral order (centre, then each ring from `(0, -k)` walking the
/// `AXIAL` directions E, SE, SW, W, NW, NE).
pub fn spiral(center: IVec2, radius: i32) -> Vec<IVec2> {
    let mut out = vec![center];
    for k in 1..=radius.max(0) {
        let mut h = center + iv2(0, -k);
        for d in AXIAL {
            for _ in 0..k {
                out.push(h);
                h = h + d;
            }
        }
    }
    out
}

fn axial_round(q: f32, r: f32) -> IVec2 {
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    iv2(rq as i32, rr as i32)
}

/// Low-res chunk hex owning a fine hex (hexx `to_lower_res`).
pub fn to_lower_res(q: i32, r: i32, radius: i32) -> IVec2 {
    let s = -q - r;
    let area = 3 * radius * (radius + 1) + 1;
    let shift = 3 * radius + 2;
    let a = floor_div(r + shift * q, area);
    let b = floor_div(s + shift * r, area);
    let c = floor_div(q + shift * s, area);
    iv2(floor_div(1 + a - b, 3), floor_div(1 + b - c, 3))
}

/// Center hex of a chunk (hexx `to_higher_res`).
pub fn chunk_center_hex(cq: i32, cr: i32, radius: i32) -> IVec2 {
    let s = -cq - cr;
    iv2(cq * (radius + 1) - radius * s, cr * (radius + 1) - radius * cq)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(grid: GridType) -> GridConfig {
        GridConfig { grid, chunk_size: 8, ring_radius: 2, ..Default::default() }
    }

    #[test]
    fn chunk_local_global_roundtrip() {
        for g in [GridType::Square, GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let c = cfg(g);
            for y in -20..20 {
                for x in -20..20 {
                    let s = iv2(x, y);
                    let ch = c.chunk_of(s);
                    let l = c.local_of(s);
                    assert!(c.has_local(l), "{g:?} {s:?} local {l:?}");
                    assert_eq!(c.global_of(ch, l), s, "{g:?}");
                }
            }
        }
    }

    #[test]
    fn hex_rings_disk_has_19_cells_for_radius_2() {
        let c = cfg(GridType::HexRings);
        let s = c.samples_per_chunk();
        let mut n = 0;
        for y in 0..s.y {
            for x in 0..s.x {
                if c.has_local(iv2(x, y)) {
                    n += 1;
                }
            }
        }
        assert_eq!(n, 19);
    }

    #[test]
    fn world_to_sample_inverts_sample_pos() {
        for g in [GridType::Square, GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let c = cfg(g);
            for y in -6..6 {
                for x in -6..6 {
                    let s = iv2(x, y);
                    assert_eq!(c.world_to_sample(c.sample_pos(s)), s, "{g:?}");
                }
            }
        }
    }

    #[test]
    fn neighbors_are_symmetric() {
        for g in [GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let c = cfg(g);
            for y in -4..4 {
                for x in -4..4 {
                    let s = iv2(x, y);
                    for n in c.edge_neighbors(s) {
                        assert!(c.edge_neighbors(n).contains(&s), "{g:?} {s:?} -> {n:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn shared_edges_share_corner_positions() {
        for g in [GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let c = cfg(g);
            let s = iv2(3, 2);
            let corners = c.cell_corners(s);
            for (e, n) in c.edge_neighbors(s).into_iter().enumerate() {
                let (a, b) = c.edge_corners(s, e);
                let nc = c.cell_corners(n);
                for p in [corners[a], corners[b]] {
                    assert!(nc.iter().any(|q| (*q - p).length() < 1e-3), "{g:?} edge {e}");
                }
            }
        }
    }

    #[test]
    fn wrap_maps_into_lap() {
        let mut c = cfg(GridType::HexRings);
        c.wrap = iv2(3, 3);
        let far = c.global_of(iv2(4, -1), iv2(2, 2));
        let w = c.wrap_sample(far);
        let ch = c.chunk_of(w);
        assert!((0..3).contains(&ch.x) && (0..3).contains(&ch.y));
    }

    /// kg_hex `patch::LARGE_LOCAL` (radius-2 spiral) literally.
    const LARGE_LOCAL: [(i32, i32); 19] = [
        (0, 0), (0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0),
        (0, -2), (1, -2), (2, -2), (2, -1), (2, 0), (1, 1), (0, 2), (-1, 2), (-2, 2), (-2, 1), (-2, 0), (-1, -1),
    ];

    #[test]
    fn spiral_order_matches_kg_hex_large_local() {
        let c = cfg(GridType::HexRings);
        let s = c.chunk_cells_spiral(iv2(0, 0));
        let expect: Vec<IVec2> = LARGE_LOCAL.iter().map(|&(q, r)| iv2(q, r)).collect();
        assert_eq!(s, expect);
        // Same set as the row-major listing, for any chunk.
        let mut a = c.chunk_cells_spiral(iv2(2, -3));
        let mut b = c.chunk_cells(iv2(2, -3));
        a.sort_by_key(|v| (v.x, v.y));
        b.sort_by_key(|v| (v.x, v.y));
        assert_eq!(a, b);
    }

    #[test]
    fn kg_hex_pixel_and_patch_conventions() {
        // Game scale: large hex 6 units flat-to-flat, ring 2 chunk == PatchCoord.
        let c = GridConfig { grid: GridType::HexRings, ring_radius: 2, cell_size: v2(6.0, 6.0), ..Default::default() };
        let r = 6.0 / SQRT3;
        for (q, rr) in [(0, 0), (1, 0), (0, 1), (-3, 5), (7, -2)] {
            // kg_hex pixel: x = R(√3 q + √3/2 r), y = 1.5 R r; 2D y -> 3D +Z.
            let p = c.sample_pos(iv2(q, rr));
            let e = v2(r * (SQRT3 * q as f32 + SQRT3 / 2.0 * rr as f32), 1.5 * r * rr as f32);
            assert!((p - e).length() < 1e-4, "{q},{rr}");
        }
        // PatchCoord::center_large = (3q - 2s, 3r - 2q); unit steps move it by (5,-2), (2,3).
        assert_eq!(c.chunk_center_cell(iv2(1, 0)), iv2(5, -2));
        assert_eq!(c.chunk_center_cell(iv2(0, 1)), iv2(2, 3));
        // Neighbour order == kg_hex DIRS (E, SE, SW, W, NW, NE).
        assert_eq!(c.edge_neighbors(iv2(0, 0)), vec![iv2(1, 0), iv2(0, 1), iv2(-1, 1), iv2(-1, 0), iv2(0, -1), iv2(1, -1)]);
        // Small hexes: 2 units flat-to-flat, owner rule div_floor(q + 1, 3).
        assert!((c.small_hex_pos(iv2(3, 0)) - c.sample_pos(iv2(1, 0))).length() < 1e-4);
        assert!((c.small_hex_pos(iv2(1, 0)).x - 2.0).abs() < 1e-4);
        for (s, o) in [((0, 0), (0, 0)), ((1, 1), (0, 0)), ((2, 0), (1, 0)), ((-1, -1), (0, 0)), ((-2, 4), (-1, 1))] {
            assert_eq!(c.small_hex_owner(iv2(s.0, s.1)), iv2(o.0, o.1), "{s:?}");
        }
        for q in -7..7 {
            for rr in -7..7 {
                let s = iv2(q, rr);
                assert_eq!(c.world_to_small_hex(c.small_hex_pos(s)), s);
            }
        }
    }

    #[test]
    fn axial_period_wrap_matches_kg_hex_torus() {
        // k = 1: P = 19 large hexes on both axial axes.
        let mut c = GridConfig { grid: GridType::HexRings, ring_radius: 2, cell_size: v2(6.0, 6.0), ..Default::default() };
        c.wrap_period = 19;
        assert!(c.wrap_period_valid());
        for q in -25..25 {
            for r in -25..25 {
                let a = iv2(q, r);
                let w = c.wrap_sample(a);
                // Same cell after wrapping by the game's period vectors.
                assert_eq!(c.wrap_sample(a + iv2(19, 0)), w);
                assert_eq!(c.wrap_sample(a + iv2(0, 19)), w);
                // Canonical: the canonical cell's chunk is its own canonical chunk, and its
                // (q, r) mod P equals the input's.
                assert_eq!(c.wrap_sample(w), w);
                assert_eq!(iv2(w.x.rem_euclid(19), w.y.rem_euclid(19)), iv2(q.rem_euclid(19), r.rem_euclid(19)));
            }
        }
        // 19 distinct canonical chunks (= kg_hex slot_count for k = 1).
        let mut chunks: Vec<IVec2> = Vec::new();
        for q in 0..19 {
            for r in 0..19 {
                let ch = c.chunk_of(iv2(q, r));
                if !chunks.contains(&ch) {
                    chunks.push(ch);
                }
            }
        }
        assert_eq!(chunks.len(), 19);
        assert!(chunks.iter().all(|ch| c.wrap_chunk(*ch) == *ch));
        c.wrap_period = 20;
        assert!(!c.wrap_period_valid());
    }

    #[test]
    fn lod_ring_selects_far_chunks_with_hysteresis() {
        let cfg = GridConfig { grid: GridType::HexRings, ring_radius: 2, cell_size: v2(2.0, 2.0), ..Default::default() };
        let chunks: Vec<IVec2> = (-3..=3).flat_map(|q| (-3..=3).map(move |r| iv2(q, r))).collect();
        let none = std::collections::HashSet::new();
        assert!(cfg.lod_low_chunks(chunks.clone(), Vec2::ZERO, 0.0, 1.0, &none).is_empty(), "0 = off");
        // The chunk centre of (0, 0) is the origin cell.
        assert!(cfg.chunk_center_pos(iv2(0, 0)).length() < 1e-4);
        let low = cfg.lod_low_chunks(chunks.clone(), Vec2::ZERO, 20.0, 4.0, &none);
        for c in &chunks {
            let d = cfg.chunk_center_pos(*c).length();
            assert_eq!(low.contains(c), d > 20.0, "{c:?} at {d}");
        }
        assert!(!low.is_empty() && low.len() < chunks.len());
        // Moving the focus a little towards a low chunk keeps it low (hysteresis) ...
        let far = *low.iter().min_by(|a, b| cfg.chunk_center_pos(**a).length().partial_cmp(&cfg.chunk_center_pos(**b).length()).unwrap()).unwrap();
        let p = cfg.chunk_center_pos(far);
        let nudge = p.normalized() * (p.length() - 20.0 + 1.0);
        let again = cfg.lod_low_chunks(chunks.clone(), nudge, 20.0, 4.0, &low);
        assert!(again.contains(&far));
        // ... without hysteresis it would flip back to full detail.
        assert!(!cfg.lod_low_chunks(chunks, nudge, 20.0, 4.0, &none).contains(&far));
    }
}
