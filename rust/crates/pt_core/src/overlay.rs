//! Analytic cell math used by the terrain shader's grid overlay, brush cursor, cell data
//! lookups and debug views (`mst_terrain_overlay.gdshaderinc`). This is the CPU reference of
//! those GLSL functions: the tests pin them to `GridConfig::world_to_sample` /
//! `chunk_of`, so a change on either side shows up in `cargo test`.

use crate::celldata::data_cell;
use crate::grid::{GridConfig, GridType, SQRT3, to_lower_res};
use crate::math::*;

/// Shader grid code (`mst_grid.x`): 0 square, 1 triangle, 2 hex (odd-r), 3 hex rings (axial).
pub fn grid_code(g: GridType) -> i32 {
    g as i32
}

fn round_half_up(x: f32) -> f32 {
    (x + 0.5).floor()
}

fn axial_round(q: f32, r: f32) -> IVec2 {
    let s = -q - r;
    let (mut rq, mut rr, rs) = (round_half_up(q), round_half_up(r), round_half_up(s));
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    iv2(rq as i32, rr as i32)
}

/// GLSL `mst_cell_of`: the data cell containing terrain-local XZ `p`.
pub fn shader_cell(grid: GridType, cs: Vec2, p: Vec2) -> IVec2 {
    let cs = v2(cs.x.max(0.01), cs.y.max(0.01));
    match grid {
        GridType::Square => iv2((p.x / cs.x).floor() as i32, (p.y / cs.y).floor() as i32),
        GridType::Triangle => {
            let h = cs.x * 0.866_025_4;
            let beta = p.y / h;
            let alpha = p.x / cs.x - beta * 0.5;
            let (i, j) = (alpha.floor(), beta.floor());
            let o = if (alpha - i) + (beta - j) < 1.0 { 0 } else { 1 };
            iv2(2 * i as i32 + o, j as i32)
        }
        GridType::Hex | GridType::HexRings => {
            let radius = cs.x / SQRT3;
            let a = axial_round((p.x * SQRT3 / 3.0 - p.y / 3.0) / radius, p.y * 2.0 / 3.0 / radius);
            if grid == GridType::Hex { axial_to_offset(a) } else { a }
        }
    }
}

/// GLSL `mst_cell_center` (cells) / lower corner (square).
pub fn shader_cell_center(grid: GridType, cs: Vec2, c: IVec2) -> Vec2 {
    let cfg = GridConfig { grid, cell_size: cs, ..Default::default() };
    match grid {
        GridType::Square => v2((c.x as f32 + 0.5) * cs.x, (c.y as f32 + 0.5) * cs.y),
        _ => cfg.sample_pos(c),
    }
}

/// GLSL `mst_edge_distance`: world distance from `p` to the nearest cell edge of its cell.
pub fn edge_distance(grid: GridType, cs: Vec2, p: Vec2) -> f32 {
    let cs = v2(cs.x.max(0.01), cs.y.max(0.01));
    let fd = |x: f32| {
        let f = x - x.floor();
        f.min(1.0 - f)
    };
    match grid {
        GridType::Square => (fd(p.x / cs.x) * cs.x).min(fd(p.y / cs.y) * cs.y),
        GridType::Triangle => {
            let h = cs.x * 0.866_025_4;
            let beta = p.y / h;
            let alpha = p.x / cs.x - beta * 0.5;
            fd(alpha).min(fd(beta)).min(fd(alpha + beta)) * h
        }
        GridType::Hex | GridType::HexRings => {
            let c = shader_cell_center(grid, cs, shader_cell(grid, cs, p));
            let v = p - c;
            let m = v.x.abs().max(v.dot(v2(0.5, 0.866_025_4)).abs()).max(v.dot(v2(-0.5, 0.866_025_4)).abs());
            cs.x * 0.5 - m
        }
    }
}

/// GLSL `mst_chunk_of`: chunk owning a data cell (`stride` = `GridConfig::chunk_stride`,
/// `radius` = ring radius). Square data cells are marching-squares cells.
pub fn shader_chunk(grid: GridType, stride: IVec2, radius: i32, c: IVec2) -> IVec2 {
    match grid {
        GridType::HexRings => to_lower_res(c.x, c.y, radius.max(1)),
        _ => iv2(div_floor(c.x, stride.x.max(1)), div_floor(c.y, stride.y.max(1))),
    }
}

fn axial_to_offset(a: IVec2) -> IVec2 {
    iv2(a.x + div_floor(a.y, 2), a.y)
}

/// Consistency helper for tests and tools: shader cell == `celldata::data_cell`.
pub fn matches_data_cell(cfg: &GridConfig, p: Vec2) -> bool {
    shader_cell(cfg.grid, cfg.cell_size_safe(), p) == data_cell(cfg, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts() -> Vec<Vec2> {
        let mut v = Vec::new();
        for j in -40..40 {
            for i in -40..40 {
                v.push(v2(i as f32 * 0.537 + 0.013, j as f32 * 0.611 + 0.007));
            }
        }
        v
    }

    #[test]
    fn shader_cell_matches_world_to_sample() {
        for g in [GridType::Square, GridType::Triangle, GridType::Hex, GridType::HexRings] {
            for cs in [v2(2.0, 2.0), v2(6.0, 6.0), v2(1.5, 2.5)] {
                let cfg = GridConfig { grid: g, cell_size: cs, ..Default::default() };
                for p in pts() {
                    assert!(matches_data_cell(&cfg, p), "{g:?} {cs:?} {p:?}: {:?} vs {:?}", shader_cell(g, cs, p), data_cell(&cfg, p));
                }
            }
        }
    }

    #[test]
    fn edge_distance_is_bounded_and_zero_on_edges() {
        for g in [GridType::Square, GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let cs = v2(2.0, 2.0);
            let cfg = GridConfig { grid: g, cell_size: cs, ..Default::default() };
            for p in pts() {
                let d = edge_distance(g, cs, p);
                assert!((-1e-3..=1.0 + 1e-3).contains(&d), "{g:?} {p:?} {d}");
            }
            // Cell corners lie on edges; centres are an inradius away.
            let c = iv2(3, 2);
            let corners = if g == GridType::Square { vec![v2(6.0, 4.0)] } else { cfg.cell_corners(c) };
            for k in corners {
                assert!(edge_distance(g, cs, k + v2(1e-4, 1e-4)).abs() < 1e-2, "{g:?} corner {k:?}");
            }
            if g.is_cell_mode() {
                let ctr = cfg.sample_pos(c);
                let expect = if g == GridType::Triangle { cs.x * 0.866_025_4 / 3.0 } else { cs.x * 0.5 };
                assert!((edge_distance(g, cs, ctr) - expect).abs() < 1e-3, "{g:?}");
            }
        }
    }

    #[test]
    fn shader_chunk_matches_chunk_of() {
        for g in [GridType::Triangle, GridType::Hex, GridType::HexRings] {
            let cfg = GridConfig { grid: g, chunk_size: 6, ring_radius: 2, ..Default::default() };
            for q in -20..20 {
                for r in -20..20 {
                    let c = iv2(q, r);
                    assert_eq!(shader_chunk(g, cfg.chunk_stride(), cfg.ring_radius, c), cfg.chunk_of(c), "{g:?} {c:?}");
                }
            }
        }
        // Square: data cell (x, z) lies in chunk floor(x / n).
        let cfg = GridConfig { chunk_size: 8, ..Default::default() };
        assert_eq!(shader_chunk(GridType::Square, cfg.chunk_stride(), 1, iv2(-1, 8)), iv2(-1, 1));
    }
}
