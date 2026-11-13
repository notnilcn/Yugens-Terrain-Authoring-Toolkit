//! Triangle-soup builder shared by both meshers, plus normal generation and indexing.
//!
//! Vertex contract consumed by `mst_terrain.gdshader`:
//! * `UV`      – ridge/ledge factors (x = closeness to an upper terrace edge, y = to a cliff top).
//! * `UV2`     – floor texture coordinates (terrain-local XZ / cell size).
//! * `CUSTOM0` – `(mat_a, mat_b, mat_c, ridge_ledge_wall_slot)` texture slots, flat per cell.
//! * `CUSTOM1` – `(weight_a, weight_b, is_wall, grass)`; `weight_c = 1 - a - b`.
//!   `is_wall`: 1 = outer wall / cliff face, 0.75 = closing geometry inside a cell (ramp
//!   corner caps, terrace risers), 0.6 = low-detail wall of the distant LOD ring
//!   ([`crate::cells::LOD_WALL`], no rock strata), 0 = floor. Walls use `CUSTOM0.x` as
//!   their slot.
//! * `NORMAL`  – floors: smoothed by position. Walls: flat, or (cell grids,
//!   [`Soup::wall_smooth_cos`]) smoothed across wall faces meeting at a shallow angle.

use crate::math::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TVert {
    pub pos: Vec3,
    pub uv: Vec2,
    pub uv2: Vec2,
    pub custom0: [f32; 4],
    pub custom1: [f32; 4],
    pub floor: bool,
}

/// Floor-triangle range of one cell, used for grass scatter.
#[derive(Clone, Debug)]
pub struct CellTris {
    pub cell: IVec2,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Soup {
    pub verts: Vec<TVert>,
    pub cells: Vec<CellTris>,
    /// Cell modes: nominal area of one cell (grass density reference). 0 for square chunks,
    /// whose grass uses one jittered `n × n` grid per marching-squares cell.
    pub cell_area: f32,
    /// Floor triangles of cells just outside the chunk: they only contribute to the smoothed
    /// floor normals, so shared vertices on chunk borders get the same normal in both chunks
    /// (otherwise sloped floors show a lighting/crease seam along every chunk border).
    pub normal_only: Vec<TVert>,
    /// Some(cos): wall normals are averaged (angle-weighted) with the wall faces sharing the
    /// vertex position whose normals are within this cosine; None = flat wall normals.
    pub wall_smooth_cos: Option<f32>,
}

impl Soup {
    pub fn begin_cell(&mut self, cell: IVec2) {
        let n = self.verts.len();
        self.cells.push(CellTris { cell, start: n, end: n });
    }
    pub fn end_cell(&mut self) {
        let n = self.verts.len();
        if let Some(c) = self.cells.last_mut() {
            c.end = n;
        }
    }
    pub fn triangle_count(&self) -> usize {
        self.verts.len() / 3
    }
}

/// Final indexed mesh arrays.
#[derive(Clone, Debug, Default)]
pub struct MeshArrays {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub uv2: Vec<[f32; 2]>,
    pub custom0: Vec<[f32; 4]>,
    pub custom1: Vec<[f32; 4]>,
    pub indices: Vec<i32>,
    /// Non-indexed triangle positions for a concave collision shape.
    pub collision_faces: Vec<[f32; 3]>,
}

fn qkey(p: Vec3) -> (i32, i32, i32) {
    let s = 4096.0;
    ((p.x * s).round() as i32, (p.y * s).round() as i32, (p.z * s).round() as i32)
}

/// Interior angle at `p` of the triangle `p, q, r`.
fn corner_angle(p: Vec3, q: Vec3, r: Vec3) -> f32 {
    let (u, v) = (q - p, r - p);
    let d = u.length() * v.length();
    if d < 1e-12 { 0.0 } else { (u.dot(v) / d).clamp(-1.0, 1.0).acos() }
}

fn face_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    // Godot's front faces are clockwise when viewed from the front; the soup is wound so that
    // (b - a) x (c - a) points *into* the surface, hence the reversed cross product.
    (c - a).cross(b - a)
}

impl Soup {
    /// Smooth normals for floor vertices (shared by position), flat normals for walls,
    /// then de-duplicates identical vertices into an index buffer.
    pub fn build(&self) -> MeshArrays {
        let n = self.verts.len() / 3 * 3;
        let mut acc: HashMap<(i32, i32, i32), Vec3> = HashMap::new();
        let mut face_n = Vec::with_capacity(n / 3);
        for t in (0..n).step_by(3) {
            let (a, b, c) = (self.verts[t], self.verts[t + 1], self.verts[t + 2]);
            let fnrm = face_normal(a.pos, b.pos, c.pos);
            face_n.push(fnrm);
            for v in [a, b, c] {
                if v.floor {
                    let e = acc.entry(qkey(v.pos)).or_insert(Vec3::ZERO);
                    *e = *e + fnrm;
                }
            }
        }
        for t in self.normal_only.chunks_exact(3) {
            let fnrm = face_normal(t[0].pos, t[1].pos, t[2].pos);
            for v in t {
                if v.floor
                    && let Some(e) = acc.get_mut(&qkey(v.pos))
                {
                    *e = *e + fnrm;
                }
            }
        }
        // Wall faces per position: (unit face normal, corner angle).
        let mut walls: HashMap<(i32, i32, i32), Vec<(Vec3, f32)>> = HashMap::new();
        if self.wall_smooth_cos.is_some() {
            let own = self.verts[..n].chunks_exact(3).map(|t| (t, true));
            let extra = self.normal_only.chunks_exact(3).map(|t| (t, false));
            for (t, own) in own.chain(extra) {
                if t[0].floor {
                    continue;
                }
                let fnrm = face_normal(t[0].pos, t[1].pos, t[2].pos);
                if fnrm.length() < 1e-12 {
                    continue;
                }
                let un = fnrm.normalized();
                for k in 0..3 {
                    let key = qkey(t[k].pos);
                    let ang = corner_angle(t[k].pos, t[(k + 1) % 3].pos, t[(k + 2) % 3].pos);
                    if own {
                        walls.entry(key).or_default().push((un, ang));
                    } else if let Some(e) = walls.get_mut(&key) {
                        e.push((un, ang));
                    }
                }
            }
        }
        let wall_normal = |p: Vec3, fnrm: Vec3| -> Vec3 {
            let (Some(cos), Some(list)) = (self.wall_smooth_cos, walls.get(&qkey(p))) else { return fnrm };
            let mut sum = Vec3::ZERO;
            for (nrm, w) in list {
                if nrm.dot(fnrm) >= cos {
                    sum = sum + *nrm * *w;
                }
            }
            if sum.length() < 1e-6 { fnrm } else { sum.normalized() }
        };
        let mut out = MeshArrays::default();
        let mut dedupe: HashMap<[u32; 18], i32> = HashMap::new();
        for t in (0..n).step_by(3) {
            let fnrm = face_n[t / 3].normalized();
            for k in 0..3 {
                let v = self.verts[t + k];
                let nrm = if v.floor { acc.get(&qkey(v.pos)).copied().unwrap_or(fnrm).normalized() } else { wall_normal(v.pos, fnrm) };
                let key = [
                    v.pos.x.to_bits(),
                    v.pos.y.to_bits(),
                    v.pos.z.to_bits(),
                    nrm.x.to_bits(),
                    nrm.y.to_bits(),
                    nrm.z.to_bits(),
                    v.uv.x.to_bits(),
                    v.uv.y.to_bits(),
                    v.uv2.x.to_bits(),
                    v.uv2.y.to_bits(),
                    v.custom0[0].to_bits(),
                    v.custom0[1].to_bits(),
                    v.custom0[2].to_bits(),
                    v.custom0[3].to_bits(),
                    v.custom1[0].to_bits(),
                    v.custom1[1].to_bits(),
                    v.custom1[2].to_bits(),
                    v.custom1[3].to_bits(),
                ];
                let idx = *dedupe.entry(key).or_insert_with(|| {
                    out.positions.push([v.pos.x, v.pos.y, v.pos.z]);
                    out.normals.push([nrm.x, nrm.y, nrm.z]);
                    out.uv.push([v.uv.x, v.uv.y]);
                    out.uv2.push([v.uv2.x, v.uv2.y]);
                    out.custom0.push(v.custom0);
                    out.custom1.push(v.custom1);
                    (out.positions.len() - 1) as i32
                });
                out.indices.push(idx);
                out.collision_faces.push([v.pos.x, v.pos.y, v.pos.z]);
            }
        }
        out
    }
}
