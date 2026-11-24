use crate::cells::{EdgeParams, EdgeStyle};
use crate::grid::{GridConfig, GridType};
use crate::math::*;
use std::collections::HashSet;
pub type Selection = Vec<(IVec2, f32)>;
pub fn merge(sel: &mut Selection, fp: &[(IVec2, f32)]) {
    for (s, w) in fp {
        match sel.iter_mut().find(|(c, _)| c == s) {
            Some(e) => e.1 = e.1.max(*w),
            None => sel.push((*s, *w)),
        }
    }
}
pub fn deduct(sel: &mut Selection, fp: &[(IVec2, f32)]) -> bool {
    let n = sel.len();
    sel.retain(|(c, _)| !fp.iter().any(|(s, _)| s == c));
    sel.len() != n
}
pub fn drag_height(origin: Vec3, dir: Vec3, base: Vec3) -> Option<f32> {
    let n = v3(dir.x, 0.0, dir.z);
    let nd = n.innerprod(dir);
    if n.magnitude() < 1e-3 || nd.abs() < 1e-6 {
        return None;
    }
    let t = n.innerprod(base - origin) / nd;
    (t > 0.0).then(|| origin.y + dir.y * t)
}
pub const DRAG_PX_PER_STEP: f32 = 16.0;
pub const DRAG_MIN_HORIZONTAL: f32 = 0.2005;
pub fn drag_rate_limit(step: f32) -> f32 {
    step.abs().max(1e-3) / DRAG_PX_PER_STEP
}
pub fn drag_offset(
    origin: Vec3,
    dir: Vec3,
    base: Vec3,
    pixels: f32,
    screen_per_pixel: f32,
    max_per_pixel: f32,
) -> f32 {
    let cap = pixels.abs() * max_per_pixel;
    let plane = (v3(dir.x, 0.0, dir.z).magnitude() > DRAG_MIN_HORIZONTAL)
        .then(|| drag_height(origin, dir, base))
        .flatten()
        .map(|y| y - base.y)
        .filter(|o| o.is_finite() && *o * pixels >= 0.0);
    let raw = plane.unwrap_or(pixels * screen_per_pixel);
    raw.clamp(-cap, cap)
}
pub fn stature_stride(edges: &EdgeParams) -> Option<f32> {
    matches!(edges.style, EdgeStyle::Terraces | EdgeStyle::HexPrism)
        .then_some(edges.terrace_step)
        .filter(|s| *s > 1e-4)
}
pub fn snap_offset(offset: f32, flatten: Option<f32>, step: Option<f32>) -> f32 {
    let Some(step) = step else { return offset };
    let snap = |v: f32| (v / step).round() * step;
    match flatten {
        Some(level) => snap(level + offset) - level,
        None => snap(offset),
    }
}
pub fn sample_polygon(cfg: &GridConfig, s: IVec2) -> Vec<Vec2> {
    if cfg.grid != GridType::Square {
        return cfg.cell_corners(s);
    }
    let h = cfg.cell_size_safe() * 0.5;
    let o = cfg.sample_pos(s) - h;
    let (x, z) = (v2(h.x * 2.0, 0.0), v2(0.0, h.y * 2.0));
    vec![o, o + x, o + x + z, o + z]
}
pub fn rim(cfg: &GridConfig, sel: &[(IVec2, f32)]) -> Vec<(IVec2, Vec2, Vec2)> {
    let set: HashSet<IVec2> = sel.iter().map(|(s, _)| *s).collect();
    let mut out = Vec::new();
    for (s, _) in sel {
        let poly = sample_polygon(cfg, *s);
        for (e, n) in cfg.edge_neighbors(*s).into_iter().enumerate() {
            if set.contains(&n) {
                continue;
            }
            let (i, j) = cfg.edge_corners(*s, e);
            out.push((*s, poly[i], poly[j]));
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::brush::{Brush, footprint, op_offset};
    use crate::terrain_store::{MergeMode, TerrainStore};
    fn store(grid: GridType) -> TerrainStore {
        let mut s = TerrainStore::new(GridConfig {
            grid,
            chunk_size: 8,
            ring_radius: 3,
            ..Default::default()
        });
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        s
    }
    #[test]
    fn merge_keeps_strongest_weight_and_subtract_removes() {
        let mut sel = vec![(iv2(0, 0), 0.3)];
        merge(&mut sel, &[(iv2(0, 0), 0.8), (iv2(1, 0), 0.5)]);
        merge(&mut sel, &[(iv2(0, 0), 0.1)]);
        assert_eq!(sel, vec![(iv2(0, 0), 0.8), (iv2(1, 0), 0.5)]);
        assert!(deduct(&mut sel, &[(iv2(0, 0), 1.0)]));
        assert!(!deduct(&mut sel, &[(iv2(5, 5), 1.0)]));
        assert_eq!(sel, vec![(iv2(1, 0), 0.5)]);
    }
    #[test]
    fn drag_height_follows_the_mouse_ray() {
        let o = v3(0.0, 5.0, 10.0);
        let d = v3(0.0, -0.2, -1.0).unitized();
        let h = drag_height(o, d, v3(0.0, 0.0, 0.0)).unwrap();
        assert!((h - 3.0).abs() < 1e-4, "{h}");
        assert_eq!(
            drag_height(o, v3(0.0, -1.0, 0.0), v3(0.0, 0.0, 0.0)),
            None,
            "top-down view"
        );
    }
    fn camera_ray(dist: f32, pitch_deg: f32, px: f32) -> (Vec3, Vec3, f32) {
        let (vh, half) = (720.0, (75.0f32.to_radians() * 0.5).tan());
        let p = pitch_deg.to_radians();
        let (fwd, up) = (v3(0.0, -p.sin(), -p.cos()), v3(0.0, p.cos(), -p.sin()));
        let origin = -fwd * dist;
        let dir = (fwd + up * (px / (vh * 0.5) * half)).unitized();
        (origin, dir, 2.0 * dist * half / vh)
    }
    fn offset_at(dist: f32, pitch: f32, px: f32, step: f32) -> f32 {
        let (o, d, spp) = camera_ray(dist, pitch, px);
        drag_offset(o, d, Vec3::ZERO, px, spp, drag_rate_limit(step))
    }
    #[test]
    fn drag_offset_needs_a_minimum_screen_distance_per_step() {
        let step = 0.5;
        for dist in [5.0, 10.0, 20.0, 40.0, 80.0, 200.0] {
            for pitch in [10.0, 30.0, 45.0, 60.0, 75.0, 85.0, 89.0, 90.0] {
                let mut last = 0.0;
                for px in [1.0, 2.0, 5.0, 10.0, 16.0, 30.0, 60.0, 120.0, 300.0] {
                    let o = offset_at(dist, pitch, px, step);
                    let down = offset_at(dist, pitch, -px, step);
                    assert!(
                        o.is_finite() && o > 0.0 && down < 0.0,
                        "d={dist} p={pitch} px={px}: {o} / {down}"
                    );
                    assert!(
                        o / px <= step / DRAG_PX_PER_STEP + 1e-6,
                        "d={dist} p={pitch} px={px}: {} per px",
                        o / px
                    );
                    assert!(
                        -down / px <= step / DRAG_PX_PER_STEP + 1e-6,
                        "d={dist} p={pitch} px={px}: down"
                    );
                    assert!(o >= last, "d={dist} p={pitch} px={px}: monotonic");
                    last = o;
                }
                assert!(
                    offset_at(dist, pitch, 30.0, step) <= 2.0 * step,
                    "d={dist} p={pitch}"
                );
            }
        }
        let far = offset_at(60.0, 60.0, 32.0, step);
        assert!((far - 2.0 * step).abs() < 1e-4, "{far}");
        for (dist, pitch) in [(5.0, 20.0), (10.0, 30.0), (8.0, 45.0)] {
            for px in [3.0, 12.0, 40.0] {
                let (o, d, _) = camera_ray(dist, pitch, px);
                let plane = drag_height(o, d, Vec3::ZERO).unwrap();
                let got = offset_at(dist, pitch, px, step);
                assert!(
                    (got - plane).abs() < 1e-5,
                    "d={dist} p={pitch} px={px}: {got} vs plane {plane}"
                );
            }
        }
        let snapped = |px| snap_offset(offset_at(80.0, 70.0, px, step), None, Some(step));
        assert_eq!(snapped(7.0), 0.0);
        assert_eq!(snapped(10.0), step);
        assert_eq!(snapped(26.0), 2.0 * step);
        assert_eq!(
            drag_offset(Vec3::ZERO, v3(0.0, -1.0, 0.0), Vec3::ZERO, 0.0, 1.0, 1.0),
            0.0,
            "no travel, no offset"
        );
    }
    #[test]
    fn boundary_outlines_the_selection_only() {
        for grid in [GridType::HexRings, GridType::Square] {
            let s = store(grid);
            let c = s.config.chunk_cells(iv2(0, 0))[10];
            let one = vec![(c, 1.0)];
            let n = s.config.edge_neighbors(c).len();
            assert_eq!(rim(&s.config, &one).len(), n, "{grid:?}");
            let two = vec![(c, 1.0), (s.config.edge_neighbors(c)[0], 0.5)];
            assert_eq!(rim(&s.config, &two).len(), 2 * n - 2, "{grid:?}");
            let p = sample_polygon(&s.config, c);
            let (i, j) = s.config.edge_corners(c, 0);
            let q = sample_polygon(&s.config, two[1].0);
            assert!(
                q.iter().any(|v| v.span(p[i]) < 1e-4) && q.iter().any(|v| v.span(p[j]) < 1e-4),
                "{grid:?}"
            );
        }
    }
    #[test]
    fn snapping_lands_on_steps() {
        assert_eq!(snap_offset(0.7, None, None), 0.7);
        assert!((snap_offset(0.7, None, Some(0.5)) - 0.5).abs() < 1e-6);
        assert!((snap_offset(0.9, Some(0.2), Some(0.5)) - 0.8).abs() < 1e-6);
    }
    #[test]
    fn dragging_raises_every_selected_cell() {
        for grid in [GridType::HexRings, GridType::Square] {
            let mut s = store(grid);
            let b = Brush {
                radius: 2.5,
                falloff: false,
                ..Default::default()
            };
            let cells = s.config.chunk_cells(iv2(0, 0));
            let (pa, pb) = (
                s.config.sample_pos(cells[0]),
                s.config.sample_pos(cells[cells.len() - 1]),
            );
            let mut sel = Selection::new();
            merge(&mut sel, &footprint(&s.config, &b, pa));
            let first = sel.len();
            merge(&mut sel, &footprint(&s.config, &b, pb));
            sel.retain(|(c, _)| s.stature(*c).is_some());
            assert!(
                sel.len() > first && first > 0,
                "{grid:?}: selection accumulates"
            );
            let base: Vec<(IVec2, f32)> = sel
                .iter()
                .map(|(c, _)| (*c, s.stature(*c).unwrap()))
                .collect();
            op_offset(&mut s, &sel, &base, 2.0, None);
            for (c, _) in &sel {
                assert!(
                    (s.stature(*c).unwrap() - 2.0).abs() < 1e-6,
                    "{grid:?}: {c:?} raised"
                );
            }
            let outside = cells[cells.len() / 2];
            assert!(!sel.iter().any(|(c, _)| *c == outside));
            assert_eq!(
                s.stature(outside),
                Some(0.0),
                "{grid:?}: unselected cell untouched"
            );
        }
    }
}
