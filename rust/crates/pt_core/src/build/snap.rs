//! Snapping: square grid steps and terrain cells (centres / corners in terrain-local XZ).

use crate::grid::GridConfig;
use crate::math::*;

pub fn snap_value(v: f32, step: f32) -> f32 {
    if step <= 1e-6 { v } else { (v / step).round() * step }
}

pub fn snap_vec(p: Vec3, step: f32) -> Vec3 {
    v3(snap_value(p.x, step), snap_value(p.y, step), snap_value(p.z, step))
}

/// Centre of the cell containing `p` (terrain-local XZ). Square grids snap to cell centres
/// (between samples) too.
pub fn cell_center(cfg: &GridConfig, p: Vec2) -> (IVec2, Vec2) {
    if cfg.grid.is_cell_mode() {
        let c = cfg.world_to_sample(p);
        (c, cfg.sample_pos(c))
    } else {
        let cs = cfg.cell_size_safe();
        let c = iv2((p.x / cs.x).floor() as i32, (p.y / cs.y).floor() as i32);
        (c, v2((c.x as f32 + 0.5) * cs.x, (c.y as f32 + 0.5) * cs.y))
    }
}

/// Nearest cell corner (or cell centre, if closer and `allow_center`) to `p`.
pub fn cell_point(cfg: &GridConfig, p: Vec2, allow_center: bool) -> Vec2 {
    let (c, center) = cell_center(cfg, p);
    let mut best = if allow_center { center } else { v2(f32::MAX, f32::MAX) };
    for k in cfg.cell_corners(c) {
        if k.distance(p) < best.distance(p) {
            best = k;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::GridType;

    #[test]
    fn hex_center_and_corner() {
        let cfg = GridConfig { grid: GridType::Hex, cell_size: v2(2.0, 2.0), ..Default::default() };
        let c = iv2(2, 3);
        let ctr = cfg.sample_pos(c);
        assert_eq!(cell_center(&cfg, ctr + v2(0.3, -0.2)).0, c);
        let corner = cfg.cell_corners(c)[1];
        assert!(cell_point(&cfg, corner + v2(0.05, 0.05), true).distance(corner) < 1e-5);
        assert!((snap_value(1.26, 0.25) - 1.25).abs() < 1e-6);
    }
}
