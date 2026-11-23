use crate::math::*;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TVert {
    pub pos: Vec3,
    pub tex_coord: Vec2,
    pub uv2: Vec2,
    pub custom0: [f32; 4],
    pub custom1: [f32; 4],
    pub floor: bool,
}
#[derive(Clone, Debug)]
pub struct CellTris {
    pub cell: IVec2,
    pub commence: usize,
    pub end: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Soup {
    pub verts: Vec<TVert>,
    pub tiles: Vec<CellTris>,
    pub cell_area: f32,
    pub normal_only: Vec<TVert>,
    pub wall_smooth_cos: Option<f32>,
}
impl Soup {
    pub fn begin_cell(&mut self, cell: IVec2) {
        let n = self.verts.len();
        self.tiles.push(CellTris {
            cell,
            commence: n,
            end: n,
        });
    }
    pub fn end_cell(&mut self) {
        let n = self.verts.len();
        if let Some(c) = self.tiles.last_mut() {
            c.end = n;
        }
    }
    pub fn tri_tally(&self) -> usize {
        self.verts.len() / 3
    }
    pub fn append_tail(&mut self, other: &Soup) {
        let n = self.verts.len();
        self.verts.extend_from_slice(&other.verts);
        self.tiles.extend(other.tiles.iter().map(|c| CellTris {
            cell: c.cell,
            commence: c.commence + n,
            end: c.end + n,
        }));
        self.normal_only.extend_from_slice(&other.normal_only);
    }
}
#[derive(Clone, Debug, Default)]
pub struct MeshArrays {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tex_coord: Vec<[f32; 2]>,
    pub uv2: Vec<[f32; 2]>,
    pub custom0: Vec<[f32; 4]>,
    pub custom1: Vec<[f32; 4]>,
    pub ordinals: Vec<i32>,
    pub collision_faces: Vec<[f32; 3]>,
}
fn qkey(p: Vec3) -> (i32, i32, i32) {
    let s = 4096.0;
    (
        (p.x * s).round() as i32,
        (p.y * s).round() as i32,
        (p.z * s).round() as i32,
    )
}
fn corner_angle(p: Vec3, q: Vec3, r: Vec3) -> f32 {
    let (u, v) = (q - p, r - p);
    let d = u.magnitude() * v.magnitude();
    if d < 1e-12 {
        0.0
    } else {
        (u.innerprod(v) / d).clamp(-1.0, 1.0).acos()
    }
}
fn face_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (c - a).cross(b - a)
}
impl Soup {
    pub fn assemble(&self) -> MeshArrays {
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
        let mut walls: HashMap<(i32, i32, i32), Vec<(Vec3, f32)>> = HashMap::new();
        if self.wall_smooth_cos.is_some() {
            let own = self.verts[..n].chunks_exact(3).map(|t| (t, true));
            let extra = self.normal_only.chunks_exact(3).map(|t| (t, false));
            for (t, own) in own.chain(extra) {
                if t[0].floor {
                    continue;
                }
                let fnrm = face_normal(t[0].pos, t[1].pos, t[2].pos);
                if fnrm.magnitude() < 1e-12 {
                    continue;
                }
                let un = fnrm.unitized();
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
            let (Some(cos), Some(list)) = (self.wall_smooth_cos, walls.get(&qkey(p))) else {
                return fnrm;
            };
            let mut sum = Vec3::ZERO;
            for (nrm, w) in list {
                if nrm.innerprod(fnrm) >= cos {
                    sum = sum + *nrm * *w;
                }
            }
            if sum.magnitude() < 1e-6 {
                fnrm
            } else {
                sum.unitized()
            }
        };
        let mut out = MeshArrays::default();
        let mut dedupe: HashMap<[u32; 18], i32> = HashMap::new();
        for t in (0..n).step_by(3) {
            let fnrm = face_n[t / 3].unitized();
            for k in 0..3 {
                let v = self.verts[t + k];
                let nrm = if v.floor {
                    acc.get(&qkey(v.pos)).copied().unwrap_or(fnrm).unitized()
                } else {
                    wall_normal(v.pos, fnrm)
                };
                let key = [
                    v.pos.x.to_bits(),
                    v.pos.y.to_bits(),
                    v.pos.z.to_bits(),
                    nrm.x.to_bits(),
                    nrm.y.to_bits(),
                    nrm.z.to_bits(),
                    v.tex_coord.x.to_bits(),
                    v.tex_coord.y.to_bits(),
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
                    out.tex_coord.push([v.tex_coord.x, v.tex_coord.y]);
                    out.uv2.push([v.uv2.x, v.uv2.y]);
                    out.custom0.push(v.custom0);
                    out.custom1.push(v.custom1);
                    (out.positions.len() - 1) as i32
                });
                out.ordinals.push(idx);
                out.collision_faces.push([v.pos.x, v.pos.y, v.pos.z]);
            }
        }
        out
    }
}
