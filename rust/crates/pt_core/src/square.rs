//! Marching-squares mesher for `GridType::Square` — a faithful port of Yūgen's
//! `MarchingSquaresTerrainCell` (cases 0–18), with the texture encoding reduced to
//! slot indices + blend weights (see `mesh.rs` for the vertex contract).

use crate::data::{ChunkData, MergeMode, TerrainStore};
use crate::grid::GridConfig;
use crate::math::*;
use crate::mesh::{Soup, TVert};

/// Texture blending: 0 = smooth, 1 = hard squares (corner A), 2 = hard per-vertex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlendMode(pub u8);

/// Merge threshold scaled by cell and chunk size, exactly like the original addon.
pub fn scaled_merge_threshold(cfg: &GridConfig, mode: MergeMode) -> f32 {
    let cs = cfg.cell_size_safe();
    let cell_scale = ((cs.x + cs.y) / 4.0).clamp(0.3, 1.0);
    let d = (cfg.chunk_size + 1) / 33;
    let dims_scale = ((d + d) as f32 / 2.0).clamp(0.5, 2.0);
    mode.threshold() * dims_scale * cell_scale
}

struct Mats {
    boundary: bool,
    min_h: f32,
    max_h: f32,
    min_idx: usize,
    max_idx: usize,
    floor: [u8; 4],
    wall: [u8; 4],
    grass: [u8; 4],
    floor_triple: [u8; 3],
    wall_triple: [u8; 3],
    blend: u8,
}

fn triple(src: &[u8; 4], priority: &[usize]) -> [u8; 3] {
    let mut order: Vec<u8> = Vec::with_capacity(4);
    for &p in priority {
        if !order.contains(&src[p]) {
            order.push(src[p]);
        }
    }
    let mut counts: Vec<(u8, usize, usize)> = Vec::new();
    for (i, &t) in src.iter().enumerate() {
        match counts.iter_mut().find(|c| c.0 == t) {
            Some(c) => c.1 += 1,
            None => counts.push((t, 1, i)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    for (t, _, _) in counts {
        if !order.contains(&t) {
            order.push(t);
        }
    }
    let a = order[0];
    let b = *order.get(1).unwrap_or(&a);
    let c = *order.get(2).unwrap_or(&b);
    [a, b, c]
}

impl Mats {
    fn bilinear(x: f32, z: f32) -> [f32; 4] {
        [(1.0 - x) * (1.0 - z), x * (1.0 - z), (1.0 - x) * z, x * z]
    }

    fn vertex_mat(&self, src: &[u8; 4], x: f32, y: f32, z: f32) -> u8 {
        if self.blend == 1 {
            return src[0];
        }
        if self.boundary {
            let range = (self.max_h - self.min_h).max(1e-6);
            let t = ((y - self.min_h) / range).clamp(0.0, 1.0);
            return if t < 0.5 { src[self.min_idx] } else { src[self.max_idx] };
        }
        let w = Self::bilinear(x, z);
        let mut best = (src[0], -1.0f32);
        for i in 0..4 {
            let total: f32 = (0..4).filter(|&j| src[j] == src[i]).map(|j| w[j]).sum();
            if total > best.1 + 1e-6 {
                best = (src[i], total);
            }
        }
        best.0
    }

    fn one_hot(tr: &[u8; 3], m: u8) -> (f32, f32) {
        if m == tr[0] {
            (1.0, 0.0)
        } else if m == tr[1] {
            (0.0, 1.0)
        } else if m == tr[2] {
            (0.0, 0.0)
        } else {
            (1.0, 0.0)
        }
    }

    fn encode(&self, x: f32, y: f32, z: f32, floor: bool) -> ([f32; 4], [f32; 4]) {
        let (src, tr) = if floor { (&self.floor, &self.floor_triple) } else { (&self.wall, &self.wall_triple) };
        let m = self.vertex_mat(src, x, y, z);
        let (wa, wb) = if floor && !self.boundary && self.blend == 0 {
            let w = Self::bilinear(x, z);
            let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
            for i in 0..4 {
                if src[i] == tr[0] {
                    a += w[i];
                } else if src[i] == tr[1] {
                    b += w[i];
                } else if src[i] == tr[2] {
                    c += w[i];
                }
            }
            let t = a + b + c;
            if t > 1e-3 { (a / t, b / t) } else { (1.0, 0.0) }
        } else {
            Self::one_hot(tr, m)
        };
        let rl = self.vertex_mat(&self.wall, x, y, z);
        let w = Self::bilinear(x, z);
        let grass: f32 = (0..4).map(|i| w[i] * self.grass[i] as f32 / 255.0).sum();
        (
            [tr[0] as f32, tr[1] as f32, tr[2] as f32, rl as f32],
            [wa, wb, if floor { 0.0 } else { 1.0 }, grass],
        )
    }
}

struct Cell<'a> {
    out: &'a mut Soup,
    mats: Mats,
    origin: Vec2,
    cs: Vec2,
    h: [f32; 4],
    rotation: u8,
    ay: f32,
    by: f32,
    cy: f32,
    dy: f32,
    ab: bool,
    bd: bool,
    cd: bool,
    ac: bool,
    merge: f32,
    floor_mode: bool,
}

impl<'a> Cell<'a> {
    fn set_rotation(&mut self, r: u8) {
        let [a, b, c, d] = self.h;
        (self.ay, self.by, self.dy, self.cy) = match r {
            1 => (b, d, c, a),
            2 => (d, c, a, b),
            3 => (c, a, b, d),
            _ => (a, b, d, c),
        };
        let m = self.merge;
        self.ab = (self.ay - self.by).abs() < m;
        self.bd = (self.by - self.dy).abs() < m;
        self.cd = (self.cy - self.dy).abs() < m;
        self.ac = (self.ay - self.cy).abs() < m;
        self.rotation = r;
    }
    fn rotate(&mut self, r: i32) {
        self.set_rotation(((4 + r + self.rotation as i32) % 4) as u8);
    }
    fn higher(&self, a: f32, b: f32) -> bool {
        a - b > self.merge
    }
    fn lower(&self, a: f32, b: f32) -> bool {
        a - b < -self.merge
    }
    fn merged(&self, a: f32, b: f32) -> bool {
        (a - b).abs() < self.merge
    }
    fn floor(&mut self) {
        self.floor_mode = true;
    }
    fn wall(&mut self) {
        self.floor_mode = false;
    }

    fn p(&mut self, mut x: f32, y: f32, mut z: f32, u: f32, v: f32) {
        for _ in 0..self.rotation {
            let t = x;
            x = 1.0 - z;
            z = t;
        }
        let uv = if self.floor_mode { v2(u, v) } else { v2(1.0, 1.0) };
        let pos = v3(self.origin.x + x * self.cs.x, y, self.origin.y + z * self.cs.y);
        let (c0, c1) = self.mats.encode(x, y, z, self.floor_mode);
        self.out.verts.push(TVert {
            pos,
            uv,
            uv2: v2(pos.x / self.cs.x, pos.z / self.cs.y),
            custom0: c0,
            custom1: c1,
            floor: self.floor_mode,
        });
    }

    fn generate(&mut self) {
        if self.ab && self.ac && self.bd && self.cd {
            self.full_floor();
            return;
        }
        for rot in 0..4u8 {
            self.set_rotation(rot);
            let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
            let (ab, bd, cd, ac) = (self.ab, self.bd, self.cd, self.ac);
            let found = if self.higher(ay, by) && self.higher(ay, cy) && bd && cd {
                self.outer_corner(true, true, false, -1.0);
                true
            } else if self.higher(ay, cy) && self.higher(by, dy) && ab && cd {
                self.edge(true, true, 0.0, 1.0);
                true
            } else if self.higher(ay, by) && self.higher(ay, cy) && self.higher(by, dy) && cd {
                self.edge(true, true, 0.5, 1.0);
                let b = self.by;
                self.outer_corner(false, true, true, b);
                true
            } else if self.higher(by, ay) && self.higher(ay, cy) && self.higher(by, dy) && cd {
                self.edge(true, true, 0.0, 0.5);
                self.rotate(1);
                let c = self.cy;
                self.outer_corner(false, true, true, c);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.lower(dy, by) && self.lower(dy, cy) && self.merged(by, cy) {
                self.inner_corner(true, false, false, false, false);
                let (b, c) = (self.by, self.cy);
                self.diagonal_floor(b, c, true, true);
                self.rotate(2);
                self.inner_corner(true, false, false, false, false);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.lower(dy, by) && self.lower(dy, cy) && self.higher(by, cy) {
                self.inner_corner(true, false, true, false, false);
                let c = self.cy;
                self.diagonal_floor(c, c, true, true);
                self.rotate(2);
                self.inner_corner(true, false, true, false, false);
                self.rotate(-1);
                self.outer_corner(false, true, false, -1.0);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && bd && cd {
                self.inner_corner(true, true, false, false, false);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.higher(dy, by) && self.higher(dy, cy) && self.merged(by, cy) {
                self.inner_corner(true, false, false, false, false);
                let (b, c) = (self.by, self.cy);
                self.diagonal_floor(b, c, true, false);
                self.rotate(2);
                self.outer_corner(false, true, false, -1.0);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.lower(dy, cy) && bd {
                self.c9();
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.lower(dy, by) && cd {
                self.c10();
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.higher(dy, cy) && bd {
                self.inner_corner(true, false, true, true, false);
                self.rotate(1);
                self.edge(false, true, 0.0, 1.0);
                true
            } else if self.lower(ay, by) && self.lower(ay, cy) && self.higher(dy, by) && cd {
                self.inner_corner(true, false, true, false, true);
                self.rotate(2);
                self.edge(false, true, 0.0, 1.0);
                true
            } else if self.lower(ay, by) && self.lower(by, dy) && self.lower(dy, cy) && self.higher(cy, ay) {
                self.inner_corner(true, false, true, false, true);
                self.rotate(2);
                self.edge(false, true, 0.0, 0.5);
                self.rotate(1);
                let c = self.cy;
                self.outer_corner(false, true, true, c);
                true
            } else if self.lower(ay, cy) && self.lower(cy, dy) && self.lower(dy, by) && self.higher(by, ay) {
                self.inner_corner(true, false, true, true, false);
                self.rotate(1);
                self.edge(false, true, 0.5, 1.0);
                let b = self.by;
                self.outer_corner(false, true, true, b);
                true
            } else if self.lower(ay, by) && self.lower(by, cy) && self.lower(cy, dy) {
                self.inner_corner(true, false, true, false, true);
                self.rotate(2);
                self.edge(false, true, 0.5, 1.0);
                let b = self.by;
                self.outer_corner(false, true, true, b);
                true
            } else if self.lower(ay, cy) && self.lower(cy, by) && self.lower(by, dy) {
                self.inner_corner(true, false, true, true, false);
                self.rotate(1);
                self.edge(false, true, 0.0, 0.5);
                self.rotate(1);
                let c = self.cy;
                self.outer_corner(false, true, true, c);
                true
            } else if ab && bd && cd && self.higher(ay, cy) {
                self.c17();
                true
            } else if ab && ac && cd && self.higher(by, dy) {
                self.c18();
                true
            } else {
                false
            };
            if found {
                return;
            }
        }
        self.set_rotation(0);
        self.full_floor();
    }

    fn full_floor(&mut self) {
        self.floor();
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let ey = (ay + by + cy + dy) / 4.0;
        self.p(0.0, ay, 0.0, 0.0, 0.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(0.5, ey, 0.5, 0.0, 0.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.5, ey, 0.5, 0.0, 0.0);
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(0.5, ey, 0.5, 0.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(0.0, ay, 0.0, 0.0, 0.0);
        self.p(0.5, ey, 0.5, 0.0, 0.0);
    }

    fn outer_corner(&mut self, floor_below: bool, floor_above: bool, flatten_bottom: bool, bottom: f32) {
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let eb = if flatten_bottom { bottom } else { by };
        let ec = if flatten_bottom { bottom } else { cy };
        if floor_above {
            self.floor();
            self.p(0.0, ay, 0.0, 0.0, 0.0);
            self.p(0.5, ay, 0.0, 0.0, 1.0);
            self.p(0.0, ay, 0.5, 0.0, 1.0);
        }
        self.wall();
        self.p(0.0, ec, 0.5, 0.0, 0.0);
        self.p(0.0, ay, 0.5, 0.0, 1.0);
        self.p(0.5, eb, 0.0, 1.0, 0.0);
        self.p(0.5, ay, 0.0, 1.0, 1.0);
        self.p(0.5, eb, 0.0, 1.0, 0.0);
        self.p(0.0, ay, 0.5, 0.0, 1.0);
        if floor_below {
            self.floor();
            self.p(1.0, dy, 1.0, 0.0, 0.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(1.0, by, 0.0, 0.0, 0.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(0.0, cy, 0.5, 1.0, 0.0);
            self.p(0.5, by, 0.0, 1.0, 0.0);
            self.p(1.0, by, 0.0, 0.0, 0.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(0.5, by, 0.0, 1.0, 0.0);
        }
    }

    fn edge(&mut self, floor_below: bool, floor_above: bool, a_x: f32, b_x: f32) {
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let ea = if self.ab { ay } else { ay.min(by) };
        let eb = if self.ab { by } else { ay.min(by) };
        let ec = if self.cd { cy } else { cy.max(dy) };
        let ed = if self.cd { dy } else { cy.max(dy) };
        let s = |c: bool| if c { 1.0 } else { 0.0 };
        if floor_above {
            self.floor();
            let ua = if b_x < 1.0 { -1.0 } else { s(a_x > 0.0) };
            let ub = if a_x > 0.0 { -1.0 } else { s(b_x < 1.0) };
            self.p(a_x, ea, 0.0, s(a_x > 0.0), 0.0);
            self.p(b_x, eb, 0.0, s(b_x < 1.0), 0.0);
            self.p(0.0, ea, 0.5, ua, 1.0);
            self.p(1.0, eb, 0.5, ub, 1.0);
            self.p(0.0, ea, 0.5, ua, 1.0);
            self.p(b_x, eb, 0.0, s(b_x < 1.0), 0.0);
        }
        self.wall();
        self.p(0.0, ec, 0.5, 0.0, 0.0);
        self.p(0.0, ea, 0.5, 0.0, 1.0);
        self.p(1.0, ed, 0.5, 1.0, 0.0);
        self.p(1.0, eb, 0.5, 1.0, 1.0);
        self.p(1.0, ed, 0.5, 1.0, 0.0);
        self.p(0.0, ea, 0.5, 0.0, 1.0);
        if floor_below {
            self.floor();
            self.p(0.0, cy, 0.5, 1.0, 0.0);
            self.p(1.0, dy, 0.5, 1.0, 0.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(1.0, dy, 1.0, 0.0, 0.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(1.0, dy, 0.5, 1.0, 0.0);
        }
    }

    fn inner_corner(&mut self, lower_floor: bool, full_upper: bool, flatten: bool, bd_floor: bool, cd_floor: bool) {
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let cb = if flatten { by.min(cy) } else { by };
        let cc = if flatten { by.min(cy) } else { cy };
        if lower_floor {
            self.floor();
            self.p(0.0, ay, 0.0, 0.0, 0.0);
            self.p(0.5, ay, 0.0, 1.0, 0.0);
            self.p(0.0, ay, 0.5, 1.0, 0.0);
        }
        self.wall();
        self.p(0.0, ay, 0.5, 1.0, 0.0);
        self.p(0.5, ay, 0.0, 0.0, 0.0);
        self.p(0.0, cc, 0.5, 1.0, 1.0);
        self.p(0.5, cb, 0.0, 0.0, 1.0);
        self.p(0.0, cc, 0.5, 1.0, 1.0);
        self.p(0.5, ay, 0.0, 0.0, 0.0);
        self.floor();
        if full_upper {
            self.p(1.0, dy, 1.0, 0.0, 0.0);
            self.p(0.0, cc, 1.0, 0.0, 0.0);
            self.p(1.0, cb, 0.0, 0.0, 0.0);
            self.p(0.0, cc, 1.0, 0.0, 0.0);
            self.p(0.0, cc, 0.5, 0.0, 1.0);
            self.p(0.5, cb, 0.0, 0.0, 1.0);
            self.p(1.0, cb, 0.0, 0.0, 0.0);
            self.p(0.0, cc, 1.0, 0.0, 0.0);
            self.p(0.5, cb, 0.0, 0.0, 1.0);
        }
        if cd_floor {
            self.p(1.0, by, 0.0, 0.0, 0.0);
            self.p(0.0, by, 0.5, 1.0, 1.0);
            self.p(0.5, by, 0.0, 0.0, 1.0);
            self.p(1.0, by, 0.0, 0.0, 0.0);
            self.p(1.0, by, 0.5, 1.0, -1.0);
            self.p(0.0, by, 0.5, 1.0, 1.0);
        }
        if bd_floor {
            self.p(0.0, cy, 0.5, 0.0, 1.0);
            self.p(0.5, cy, 0.0, 1.0, 1.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(0.5, cy, 1.0, 1.0, -1.0);
            self.p(0.0, cy, 1.0, 0.0, 0.0);
            self.p(0.5, cy, 0.0, 1.0, 1.0);
        }
    }

    fn diagonal_floor(&mut self, b_y: f32, c_y: f32, a_cliff: bool, d_cliff: bool) {
        self.floor();
        let (au, av) = if a_cliff { (0.0, 1.0) } else { (1.0, 0.0) };
        let (du, dv) = if d_cliff { (0.0, 1.0) } else { (1.0, 0.0) };
        self.p(1.0, b_y, 0.0, 0.0, 0.0);
        self.p(0.0, c_y, 1.0, 0.0, 0.0);
        self.p(0.5, b_y, 0.0, au, av);
        self.p(0.0, c_y, 1.0, 0.0, 0.0);
        self.p(0.0, c_y, 0.5, au, av);
        self.p(0.5, b_y, 0.0, au, av);
        self.p(1.0, b_y, 0.0, 0.0, 0.0);
        self.p(1.0, b_y, 0.5, du, dv);
        self.p(0.0, c_y, 1.0, 0.0, 0.0);
        self.p(0.0, c_y, 1.0, 0.0, 0.0);
        self.p(1.0, b_y, 0.5, du, dv);
        self.p(0.5, c_y, 1.0, du, dv);
    }

    fn c9(&mut self) {
        self.inner_corner(true, false, true, false, false);
        let (by, cy, dy) = (self.by, self.cy, self.dy);
        self.floor();
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.5, dy, 1.0, 1.0, 0.0);
        self.p(1.0, (by + dy) / 2.0, 0.5, 0.0, 0.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(1.0, (by + dy) / 2.0, 0.5, 0.0, 0.0);
        self.p(0.5, by, 0.0, 0.0, 1.0);
        self.p(0.5, by, 0.0, 0.0, 1.0);
        self.p(1.0, (by + dy) / 2.0, 0.5, 0.0, 0.0);
        self.p(0.0, by, 0.5, 1.0, 1.0);
        self.p(0.5, dy, 1.0, 1.0, 0.0);
        self.p(0.0, by, 0.5, 1.0, 1.0);
        self.p(1.0, (by + dy) / 2.0, 0.5, 0.0, 0.0);
        self.wall();
        self.p(0.0, by, 0.5, 0.0, 0.0);
        self.p(0.5, dy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 0.5, 0.0, 0.0);
        self.p(0.5, cy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 0.5, 0.0, 0.0);
        self.p(0.5, dy, 1.0, 0.0, 0.0);
        self.floor();
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 0.5, 0.0, 1.0);
        self.p(0.5, cy, 1.0, 0.0, 1.0);
    }

    fn c10(&mut self) {
        self.inner_corner(true, false, true, false, false);
        let (by, cy, dy) = (self.by, self.cy, self.dy);
        let m = (dy + cy) / 2.0;
        self.floor();
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.5, m, 1.0, 0.0, 0.0);
        self.p(1.0, dy, 0.5, 0.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 0.5, 0.0, 0.0);
        self.p(0.5, m, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 0.5, 0.0, 0.0);
        self.p(0.5, cy, 0.0, 0.0, 0.0);
        self.p(0.5, m, 1.0, 0.0, 0.0);
        self.p(1.0, dy, 0.5, 0.0, 0.0);
        self.p(0.5, m, 1.0, 0.0, 0.0);
        self.p(0.5, cy, 0.0, 0.0, 0.0);
        self.wall();
        self.p(0.5, cy, 0.0, 0.0, 0.0);
        self.p(0.5, by, 0.0, 0.0, 0.0);
        self.p(1.0, dy, 0.5, 0.0, 0.0);
        self.p(1.0, by, 0.5, 0.0, 0.0);
        self.p(1.0, dy, 0.5, 0.0, 0.0);
        self.p(0.5, by, 0.0, 0.0, 0.0);
        self.floor();
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(1.0, by, 0.5, 0.0, 0.0);
        self.p(0.5, by, 0.0, 0.0, 0.0);
    }

    fn c17(&mut self) {
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let e = (by + dy) / 2.0;
        self.floor();
        self.p(0.0, ay, 0.0, 0.0, 0.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(1.0, e, 0.5, 0.0, 0.0);
        self.p(1.0, e, 0.5, 0.0, 1.0);
        self.p(0.0, ay, 0.5, 0.0, 1.0);
        self.p(0.0, ay, 0.0, 0.0, 0.0);
        self.wall();
        self.p(0.0, cy, 0.5, 0.0, 0.0);
        self.p(0.0, ay, 0.5, 0.0, 1.0);
        self.p(1.0, e, 0.5, 1.0, 0.0);
        self.floor();
        self.p(0.0, cy, 0.5, 1.0, 0.0);
        self.p(1.0, e, 0.5, 1.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(1.0, e, 0.5, 0.0, 0.0);
    }

    fn c18(&mut self) {
        let (ay, by, cy, dy) = (self.ay, self.by, self.cy, self.dy);
        let e = (ay + cy) / 2.0;
        self.floor();
        self.p(0.0, ay, 0.0, 0.0, 0.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.p(0.0, e, 0.5, 0.0, 0.0);
        self.p(1.0, by, 0.5, 0.0, 1.0);
        self.p(0.0, e, 0.5, 0.0, 1.0);
        self.p(1.0, by, 0.0, 0.0, 0.0);
        self.wall();
        self.p(1.0, by, 0.5, 1.0, 1.0);
        self.p(1.0, dy, 0.5, 1.0, 0.0);
        self.p(0.0, e, 0.5, 0.0, 0.0);
        self.floor();
        self.p(0.0, e, 0.5, 1.0, 0.0);
        self.p(1.0, dy, 0.5, 1.0, 0.0);
        self.p(1.0, dy, 1.0, 0.0, 0.0);
        self.p(0.0, cy, 1.0, 0.0, 0.0);
        self.p(0.0, e, 0.5, 0.0, 0.0);
        self.p(1.0, dy, 1.0, 0.0, 0.0);
    }
}

/// Meshes one square chunk (cells `0..n` in both axes) into `out`.
pub fn mesh_square_chunk(cfg: &GridConfig, chunk: crate::math::IVec2, data: &ChunkData, blend: BlendMode, out: &mut Soup) {
    let n = cfg.chunk_stride();
    let merge = scaled_merge_threshold(cfg, data.merge_mode);
    for z in 0..n.y {
        for x in 0..n.x {
            let idx = [
                data.index(iv2(x, z)),
                data.index(iv2(x + 1, z)),
                data.index(iv2(x, z + 1)),
                data.index(iv2(x + 1, z + 1)),
            ];
            let corners = idx.map(|i| Corner { h: data.heights[i], floor: data.floor_tex[i], wall: data.wall_tex[i], grass: data.grass[i] });
            let g = cfg.global_of(chunk, iv2(x, z));
            out.begin_cell(g);
            mesh_cell(cfg, g, corners, merge, blend, out);
            out.end_cell();
        }
    }
}

/// Meshes one chunk of `store` and, into `out.normal_only`, the ring of cells just outside it
/// (read from the neighbouring chunks). Their floors only feed the smoothed normals, so a
/// vertex on a chunk border gets the same normal in both chunks — meshing from one chunk alone
/// leaves a lighting step (and a crease highlight in the pixel renderer) along every border.
pub fn mesh_square_chunk_in(store: &TerrainStore, chunk: IVec2, data: &ChunkData, blend: BlendMode, out: &mut Soup) {
    let cfg = &store.config;
    mesh_square_chunk(cfg, chunk, data, blend, out);
    let n = cfg.chunk_stride();
    let mut extra = Soup::default();
    for z in -1..=n.y {
        for x in -1..=n.x {
            if (0..n.x).contains(&x) && (0..n.y).contains(&z) {
                continue;
            }
            let g = cfg.global_of(chunk, iv2(x, z));
            let samples = [g, g + iv2(1, 0), g + iv2(0, 1), g + iv2(1, 1)].map(|s| store.sample(s));
            let [Some(a), Some(b), Some(c), Some(d)] = samples else { continue };
            let Some(owner) = store.chunks.get(&cfg.chunk_of(g)) else { continue };
            let merge = scaled_merge_threshold(cfg, owner.merge_mode);
            let corners = [a, b, c, d].map(|s| Corner { h: s.height, floor: s.floor, wall: s.wall, grass: s.grass });
            mesh_cell(cfg, g, corners, merge, blend, &mut extra);
        }
    }
    out.normal_only = extra.verts;
}

#[derive(Clone, Copy)]
struct Corner {
    h: f32,
    floor: u8,
    wall: u8,
    grass: u8,
}

/// Emits one marching-squares cell whose min corner is global sample `g`
/// (corners in A, B, C, D order: (0,0), (1,0), (0,1), (1,1)).
fn mesh_cell(cfg: &GridConfig, g: IVec2, corners: [Corner; 4], merge: f32, blend: BlendMode, out: &mut Soup) {
    let cs = cfg.cell_size_safe();
    let h = corners.map(|c| c.h);
    let floor = corners.map(|c| c.floor);
    let wall = corners.map(|c| c.wall);
    let grass = corners.map(|c| c.grass);
    let (mut min_idx, mut max_idx) = (0, 0);
    for i in 0..4 {
        if h[i] < h[min_idx] {
            min_idx = i;
        }
        if h[i] > h[max_idx] {
            max_idx = i;
        }
    }
    let boundary = h[max_idx] - h[min_idx] > merge;
    let prio_buf = [max_idx, min_idx];
    let prio: &[usize] = if boundary { &prio_buf } else { &[] };
    let mats = Mats {
        boundary,
        min_h: h[min_idx],
        max_h: h[max_idx],
        min_idx,
        max_idx,
        floor,
        wall,
        grass,
        floor_triple: triple(&floor, prio),
        wall_triple: triple(&wall, prio),
        blend: blend.0,
    };
    let mut cell = Cell {
        out,
        mats,
        origin: v2(g.x as f32 * cs.x, g.y as f32 * cs.y),
        cs,
        h,
        rotation: 0,
        ay: 0.0,
        by: 0.0,
        cy: 0.0,
        dy: 0.0,
        ab: false,
        bd: false,
        cd: false,
        ac: false,
        merge,
        floor_mode: true,
    };
    cell.set_rotation(0);
    cell.generate();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ChunkData;
    use crate::grid::GridConfig;

    fn chunk_with(f: impl Fn(i32, i32) -> f32) -> (GridConfig, ChunkData) {
        let cfg = GridConfig { chunk_size: 6, ..Default::default() };
        let mut d = ChunkData::new(cfg.samples_per_chunk(), 0, 5, MergeMode::Polyhedron);
        for z in 0..7 {
            for x in 0..7 {
                let i = d.index(iv2(x, z));
                d.heights[i] = f(x, z);
            }
        }
        (cfg, d)
    }

    #[test]
    fn flat_chunk_is_all_floor_quads() {
        let (cfg, d) = chunk_with(|_, _| 1.0);
        let mut s = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut s);
        assert_eq!(s.triangle_count(), 6 * 6 * 4);
        assert!(s.verts.iter().all(|v| v.floor && v.pos.y == 1.0));
        let m = s.build();
        assert!(m.normals.iter().all(|n| n[1] > 0.99), "flat floors face up");
    }

    #[test]
    fn cliffs_generate_walls_and_every_case_is_closed() {
        // Random-ish terrace field exercising many cases; each cell must emit whole triangles.
        let (cfg, d) = chunk_with(|x, z| (((x * 7 + z * 13) % 5) as f32) * 2.0);
        let mut s = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut s);
        assert_eq!(s.verts.len() % 3, 0);
        assert!(s.verts.iter().any(|v| !v.floor), "walls present");
        for c in &s.cells {
            assert_eq!((c.end - c.start) % 3, 0);
            assert!(c.end > c.start);
        }
    }

    #[test]
    fn plateau_walls_face_outward() {
        let (cfg, d) = chunk_with(|x, z| if (2..=4).contains(&x) && (2..=4).contains(&z) { 6.0 } else { 0.0 });
        let mut s = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut s);
        let m = s.build();
        let c = v2(6.0, 6.0);
        let mut walls = 0;
        for (i, n) in m.normals.iter().enumerate() {
            if m.custom1[i][2] > 0.5 {
                walls += 1;
                let p = m.positions[i];
                assert!(v2(p[0] - c.x, p[2] - c.y).dot(v2(n[0], n[2])) > 0.0, "wall normal faces into the plateau");
            }
        }
        assert!(walls > 0);
    }

    #[test]
    fn floor_normals_point_up_on_slopes() {
        let (cfg, d) = chunk_with(|x, _| x as f32 * 0.5);
        let mut s = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut s);
        let m = s.build();
        assert!(m.normals.iter().all(|n| n[1] > 0.5));
    }

    #[test]
    fn chunk_border_normals_match_across_chunks() {
        // A slope across two chunks: the shared border vertices must get identical normals.
        let cfg = GridConfig { chunk_size: 4, ..Default::default() };
        let mut store = TerrainStore::new(cfg.clone());
        for c in [iv2(0, 0), iv2(1, 0)] {
            store.add_chunk(c, MergeMode::Polyhedron);
        }
        for z in 0..=4 {
            for x in 0..=8 {
                // Curved slope (x^2) so a one-sided normal at the border differs visibly.
                store.set_height(iv2(x, z), (x * x) as f32 * 0.05);
            }
        }
        let normals_at_border = |c: IVec2| {
            let mut s = Soup::default();
            mesh_square_chunk_in(&store, c, &store.chunks[&c], BlendMode(0), &mut s);
            let m = s.build();
            let bx = 4.0 * cfg.cell_size_safe().x;
            let mut v: Vec<([i32; 2], [f32; 3])> = m
                .positions
                .iter()
                .zip(&m.normals)
                .filter(|(p, _)| (p[0] - bx).abs() < 1e-4)
                .map(|(p, n)| ([(p[2] * 100.0) as i32, (p[1] * 100.0) as i32], *n))
                .collect();
            v.sort_by_key(|e| e.0);
            v.dedup_by_key(|e| e.0);
            v
        };
        let (a, b) = (normals_at_border(iv2(0, 0)), normals_at_border(iv2(1, 0)));
        assert_eq!(a.len(), b.len());
        assert!(!a.is_empty());
        for ((pa, na), (pb, nb)) in a.iter().zip(&b) {
            assert_eq!(pa, pb);
            for k in 0..3 {
                assert!((na[k] - nb[k]).abs() < 1e-5, "border normal seam at {pa:?}: {na:?} vs {nb:?}");
            }
        }
    }
}
