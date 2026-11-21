use crate::math::*;
use crate::mesh::Soup;
#[derive(Clone, Debug)]
pub struct GrassParams {
    pub subdivisions: u32,
    pub slot_has_grass: [bool; 16],
    pub ledge_threshold: f32,
    pub min_height: f32,
    pub seed: u32,
}
impl Default for GrassParams {
    fn default() -> Self {
        let mut s = [false; 16];
        for v in s.iter_mut().take(6) {
            *v = true;
        }
        GrassParams {
            subdivisions: 3,
            slot_has_grass: s,
            ledge_threshold: 0.5,
            min_height: f32::MIN,
            seed: 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassInstance {
    pub pos: Vec3,
    pub upright: Vec3,
    pub pocket: u8,
    pub rand: f32,
}
pub fn scatter(soup: &Soup, p: &GrassParams) -> Vec<GrassInstance> {
    let mut out = Vec::new();
    let n = p.subdivisions.max(1) as usize;
    for cell in &soup.tiles {
        let tris: Vec<usize> = (cell.commence..cell.end)
            .step_by(3)
            .filter(|&i| i + 2 < cell.end && soup.verts[i].floor)
            .collect();
        if tris.is_empty() {
            continue;
        }
        let (mut min, mut max) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
        for &t in &tris {
            for k in 0..3 {
                let q = soup.verts[t + k].pos.xz();
                min = v2(min.x.min(q.x), min.y.min(q.y));
                max = v2(max.x.max(q.x), max.y.max(q.y));
            }
        }
        let size = max - min;
        let (nx, nz, step) = if soup.cell_area > 0.0 {
            let s = (soup.cell_area / (n * n) as f32).sqrt();
            (
                (size.x / s).ceil().max(1.0) as usize,
                (size.y / s).ceil().max(1.0) as usize,
                v2(s, s),
            )
        } else {
            (n, n, v2(size.x / n as f32, size.y / n as f32))
        };
        let (ridge_cut, ledge_cut) = if soup.cell_area > 0.0 {
            (0.4005, p.ledge_threshold.min(0.4005))
        } else {
            (0.0, p.ledge_threshold)
        };
        for j in 0..nz {
            for i in 0..nx {
                let k = (j * nx + i) as u32;
                let jx = hash01(cell.cell, p.seed.wrapping_mul(31).wrapping_add(k * 2));
                let jz = hash01(cell.cell, p.seed.wrapping_mul(31).wrapping_add(k * 2 + 1));
                let q = v2(
                    min.x + (i as f32 + jx) * step.x,
                    min.y + (j as f32 + jz) * step.y,
                );
                for &t in &tris {
                    let (a, b, c) = (soup.verts[t], soup.verts[t + 1], soup.verts[t + 2]);
                    let (v0, v1, v2_) = (
                        c.pos.xz() - a.pos.xz(),
                        b.pos.xz() - a.pos.xz(),
                        q - a.pos.xz(),
                    );
                    let (d00, d01, d11) = (v0.innerprod(v0), v0.innerprod(v1), v1.innerprod(v1));
                    let den = d00 * d11 - d01 * d01;
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let (d02, d12) = (v0.innerprod(v2_), v1.innerprod(v2_));
                    let u = (d11 * d02 - d01 * d12) / den;
                    let v = (d00 * d12 - d01 * d02) / den;
                    if u < -1e-5 || v < -1e-5 || u + v > 1.0 + 1e-5 {
                        continue;
                    }
                    let wa = 1.0 - u - v;
                    let pos = a.pos * wa + b.pos * v + c.pos * u;
                    let uv = a.tex_coord * wa + b.tex_coord * v + c.tex_coord * u;
                    let mask = a.custom1[3] * wa + b.custom1[3] * v + c.custom1[3] * u;
                    let w_a = a.custom1[0] * wa + b.custom1[0] * v + c.custom1[0] * u;
                    let w_b = a.custom1[1] * wa + b.custom1[1] * v + c.custom1[1] * u;
                    let w_c = 1.0 - w_a - w_b;
                    let slot = if w_a >= w_b && w_a >= w_c {
                        a.custom0[0]
                    } else if w_b >= w_c {
                        a.custom0[1]
                    } else {
                        a.custom0[2]
                    } as usize;
                    let on_band = uv.y > ridge_cut || uv.x > ledge_cut;
                    if mask > 0.5
                        && pos.y >= p.min_height
                        && !on_band
                        && slot < 16
                        && p.slot_has_grass[slot]
                    {
                        let nrm = (c.pos - a.pos).cross(b.pos - a.pos).unitized();
                        out.push(GrassInstance {
                            pos,
                            upright: nrm,
                            pocket: slot as u8,
                            rand: hash01(cell.cell, p.seed ^ (k + 977)),
                        });
                    }
                    break;
                }
            }
        }
    }
    out
}
#[derive(Clone, Debug, Default)]
pub struct WaterCull {
    pub tiles: std::collections::HashMap<IVec2, f32>,
    pub footprints: Vec<Vec<Vec2>>,
    pub blocked: std::collections::HashSet<IVec2>,
}
impl WaterCull {
    const DEPTH_MARGIN: f32 = 0.0505;
    pub fn check_vacant(&self) -> bool {
        self.tiles.is_empty() && self.footprints.is_empty() && self.blocked.is_empty()
    }
    pub fn hides_prop(&self, cfg: &crate::grid::GridConfig, pos: Vec3) -> bool {
        if self.in_footprint(pos.xz()) {
            return true;
        }
        let c = cfg.world_to_sample(pos.xz());
        self.blocked.contains(&c)
            || (cfg.grid.is_cell_mode() && self.tiles.get(&c).is_some_and(|l| *l > pos.y + 1e-3))
    }
    pub fn in_footprint(&self, p: Vec2) -> bool {
        self.footprints.iter().any(|poly| {
            let n = poly.len();
            let mut inside = false;
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + n - 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                {
                    inside = !inside;
                }
            }
            inside
        })
    }
    pub fn covers(&self, cfg: &crate::grid::GridConfig, pos: Vec3) -> bool {
        if self.in_footprint(pos.xz())
            || (!self.blocked.is_empty() && self.blocked.contains(&cfg.world_to_sample(pos.xz())))
        {
            return true;
        }
        if self.tiles.is_empty() || !cfg.grid.is_cell_mode() {
            return false;
        }
        let p = pos.xz();
        let c = cfg.world_to_sample(p);
        if self
            .tiles
            .get(&c)
            .is_some_and(|l| *l > pos.y - Self::DEPTH_MARGIN)
        {
            return true;
        }
        let cs = cfg.cell_size_safe();
        let size = cs.x.min(cs.y);
        let near = |reach: f32, test: &dyn Fn(f32) -> bool| {
            (0..8).any(|i| {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let n = cfg.world_to_sample(p + v2(a.cos(), a.sin()) * reach);
                n != c && self.tiles.get(&n).is_some_and(|l| test(*l))
            })
        };
        near(size * 0.4505, &|l| l > pos.y + Self::BANK)
            || near(size * 0.3005, &|l| l > pos.y - Self::BANK)
    }
    const BANK: f32 = 0.5;
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ChunkData, MergeMode};
    use crate::grid::GridConfig;
    use crate::square::{BlendMode, mesh_square_chunk};
    #[test]
    fn flat_grass_chunk_is_fully_covered_and_masks_work() {
        let cfg = GridConfig {
            chunk_size: 4,
            ..Default::default()
        };
        let mut d = ChunkData::new(cfg.samples_per_chunk(), 0, 5, MergeMode::Polyhedron);
        let mut soup = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut soup);
        let p = GrassParams::default();
        assert_eq!(scatter(&soup, &p).len(), 16 * 9);
        d.grass.iter_mut().for_each(|g| *g = 0);
        let mut soup = Soup::default();
        mesh_square_chunk(&cfg, iv2(0, 0), &d, BlendMode(0), &mut soup);
        assert!(scatter(&soup, &p).is_empty());
    }
    #[test]
    fn water_cull_covers_pool_cells_and_the_waterfall_foot() {
        use crate::grid::GridType;
        let cfg = GridConfig {
            grid: GridType::HexRings,
            ring_radius: 2,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        };
        let pool = iv2(0, 0);
        let mut w = WaterCull::default();
        w.tiles.insert(pool, 9.85);
        let centre = cfg.sample_pos(pool);
        assert!(w.covers(&cfg, v3(centre.x, 9.5, centre.y)));
        assert!(!w.covers(&cfg, v3(centre.x, 10.0, centre.y)));
        let nb = cfg.edge_neighbors(pool)[0];
        let nc = cfg.sample_pos(nb);
        let near = centre + (nc - centre) * 0.6;
        assert_eq!(cfg.world_to_sample(near), nb);
        assert!(w.covers(&cfg, v3(near.x, 7.5, near.y)));
        let far = centre + (nc - centre) * 1.4;
        assert!(!w.covers(&cfg, v3(far.x, 7.5, far.y)));
        let edge = centre + (nc - centre) * 0.55;
        assert!(w.covers(&cfg, v3(edge.x, 10.0, edge.y)));
        assert!(!w.covers(&cfg, v3(edge.x, 12.5, edge.y)));
        assert!(!w.covers(&cfg, v3(far.x, 10.0, far.y)));
        assert!(!w.covers(&GridConfig::default(), v3(0.0, 0.0, 0.0)));
        let mut f = WaterCull::default();
        f.footprints.push(vec![
            v2(-1.0, -1.0),
            v2(1.0, -1.0),
            v2(1.0, 1.0),
            v2(-1.0, 1.0),
        ]);
        assert!(!f.check_vacant());
        assert!(f.covers(&GridConfig::default(), v3(0.5, 3.0, -0.5)));
        assert!(!f.covers(&GridConfig::default(), v3(1.5, 3.0, 0.0)));
    }
}
