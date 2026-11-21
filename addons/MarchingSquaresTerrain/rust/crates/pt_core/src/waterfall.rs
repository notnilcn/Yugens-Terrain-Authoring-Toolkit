use crate::data::{TerrainStore, VOID_SLOT};
use crate::math::*;
use std::collections::HashMap;
pub type WaterMap = HashMap<IVec2, f32>;
pub const KIND_POOL: f32 = 0.0;
pub const KIND_FALL: f32 = 1.0;
pub const KIND_FOAM: f32 = 2.0;
pub const KIND_SPRAY: f32 = 3.0;
pub const KIND_BASIN: f32 = 4.0;
pub const FAN_SEGMENTS: usize = 10;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FallParams {
    pub min_drop: f32,
    pub sea: Option<f32>,
    pub lip: f32,
    pub foam_reach: f32,
    pub spray_density: f32,
    pub sheet_inset: f32,
    pub shore: f32,
}
impl Default for FallParams {
    fn default() -> Self {
        FallParams {
            min_drop: 2.5005,
            sea: None,
            lip: 0.1805,
            foam_reach: 1.1005,
            spray_density: 1.6005,
            sheet_inset: 0.2005,
            shore: 0.5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    pub cell: IVec2,
    pub neighbor: IVec2,
    pub edge: usize,
    pub a: Vec2,
    pub b: Vec2,
    pub out: Vec2,
    pub top: f32,
    pub bottom: f32,
    pub into_water: bool,
    pub waterfall: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterVert {
    pub pos: Vec3,
    pub upright: Vec3,
    pub tex_coord: Vec2,
    pub uv2: Vec2,
    pub sort: f32,
}
pub fn skin(
    store: &TerrainStore,
    water: &WaterMap,
    sea: Option<f32>,
    c: IVec2,
) -> Option<(f32, bool)> {
    let s = store.probe(c)?;
    if s.floor == VOID_SLOT {
        return None;
    }
    if let Some(w) = water.get(&c)
        && *w > s.stature
    {
        return Some((*w, true));
    }
    match sea {
        Some(l) if s.stature < l => Some((l, true)),
        _ => Some((s.stature, false)),
    }
}
pub fn find_falls(store: &TerrainStore, water: &WaterMap, p: &FallParams) -> Vec<Fall> {
    let cfg = &store.config;
    let mut cells: Vec<&IVec2> = water.keys().collect();
    cells.sort_by_key(|c| (c.y, c.x));
    let mut out = Vec::new();
    for &c in cells {
        let Some((top, true)) = skin(store, water, p.sea, c) else {
            continue;
        };
        let center = cfg.sample_pos(c);
        let corners = cfg.cell_corners(c);
        for (e, nb) in cfg.edge_neighbors(c).into_iter().enumerate() {
            let Some((bottom, wet)) = skin(store, water, p.sea, nb) else {
                continue;
            };
            let drop = top - bottom;
            if drop <= 0.0505 {
                continue;
            }
            let (ia, ib) = cfg.edge_corners(c, e);
            let (a, b) = (corners[ia], corners[ib]);
            let mid = (a + b) * 0.5;
            let out_dir = (mid - center).unitized();
            out.push(Fall {
                cell: c,
                neighbor: nb,
                edge: e,
                a,
                b,
                out: out_dir,
                top,
                bottom,
                into_water: wet,
                waterfall: drop >= p.min_drop - 1e-4,
            });
        }
    }
    out
}
pub fn build_water_mesh(
    store: &TerrainStore,
    water: &WaterMap,
    falls: &[Fall],
    p: &FallParams,
) -> Vec<WaterVert> {
    let cfg = &store.config;
    let mut v: Vec<WaterVert> = Vec::new();
    let up = v3(0.0, 1.0, 0.0);
    let mut cells: Vec<(&IVec2, &f32)> = water.iter().collect();
    cells.sort_by_key(|(c, _)| (c.y, c.x));
    for (&c, _) in cells {
        let Some((lvl, true)) = skin(store, water, p.sea, c) else {
            continue;
        };
        let center = cfg.sample_pos(c);
        let k = cfg.cell_corners(c);
        for i in 0..k.len() {
            let (q0, q1) = (k[i], k[(i + 1) % k.len()]);
            tri_up(
                &mut v,
                [
                    v3(center.x, lvl, center.y),
                    v3(q0.x, lvl, q0.y),
                    v3(q1.x, lvl, q1.y),
                ],
                KIND_POOL,
            );
        }
        if p.shore > 0.0 {
            for (e, nb) in cfg.edge_neighbors(c).into_iter().enumerate() {
                if water.contains_key(&nb) {
                    continue;
                }
                let Some((nh, false)) = skin(store, water, p.sea, nb) else {
                    continue;
                };
                if nh <= lvl {
                    continue;
                }
                let (ia, ib) = cfg.edge_corners(c, e);
                let (a, b) = (k[ia], k[ib]);
                let apothem = (((a + b) * 0.5) - center).magnitude().max(1e-5);
                let push = |q: Vec2| q + (q - center) * (p.shore / apothem);
                let (oa, ob) = (push(a), push(b));
                let y = lvl;
                tri_up(
                    &mut v,
                    [v3(a.x, y, a.y), v3(b.x, y, b.y), v3(ob.x, y, ob.y)],
                    KIND_POOL,
                );
                tri_up(
                    &mut v,
                    [v3(a.x, y, a.y), v3(ob.x, y, ob.y), v3(oa.x, y, oa.y)],
                    KIND_POOL,
                );
            }
        }
    }
    for f in falls {
        let center = cfg.sample_pos(f.cell);
        let apothem = (((f.a + f.b) * 0.5) - center).magnitude().max(1e-5);
        let push = |q: Vec2, d: f32| -> Vec2 { q + (q - center) * (d / apothem) };
        let ground = if f.into_water {
            store.stature(f.neighbor).unwrap_or(f.bottom).min(f.bottom)
        } else {
            f.bottom
        };
        let drop = f.top - f.bottom;
        let full = f.top - ground;
        let curl = (full * 0.1205).min(0.3505).min(drop * 0.5);
        let base_out = p.lip + (full * 0.0505).min(0.4005);
        let mut rows = [(0.02f32, f.top), (p.lip, f.top - curl), (base_out, ground)];
        if f.into_water && f.bottom > ground + 1e-5 {
            let (y1, y2) = (rows[1].1, ground);
            let t = (f.bottom - y2) / (y1 - y2);
            rows[2] = (base_out + (p.lip - base_out) * t, f.bottom);
        }
        let base_out = rows[2].0;
        let (ia, ib) = cfg.edge_corners(f.cell, f.edge);
        let shares = |corner: usize| {
            falls.iter().any(|g| {
                g.cell == f.cell && g.edge != f.edge && {
                    let (ga, gb) = cfg.edge_corners(g.cell, g.edge);
                    ga == corner || gb == corner
                }
            })
        };
        let inset = p.sheet_inset.clamp(0.0, 0.4505);
        let ta = if shares(ia) { 0.0 } else { inset };
        let tb = if shares(ib) { 1.0 } else { 1.0 - inset };
        let (ea, eb) = (f.a.lerp(f.b, ta), f.a.lerp(f.b, tb));
        let bed = store.stature(f.cell).unwrap_or(f.top).min(f.top);
        let len_full = (f.b - f.a).magnitude();
        let along0 = (f.a.x * 0.3705 + f.a.y * 0.6105).rem_euclid(17.0);
        if f.top - bed > 0.0105 {
            let n = v3(f.out.x, 0.2505, f.out.y).unitized();
            for (q0, q1, u0, u1) in [(f.a, ea, 0.0, ta), (eb, f.b, tb, 1.0)] {
                if (q1 - q0).magnitude() < 1e-4 {
                    continue;
                }
                let (p0, p1) = (push(q0, 0.0205), push(q1, 0.0205));
                let rv = |q: Vec2, u: f32, y: f32| WaterVert {
                    pos: v3(q.x, y, q.y),
                    upright: n,
                    tex_coord: v2(along0 + u * len_full, y),
                    uv2: v2(0.0, f.top - f.bottom),
                    sort: KIND_FALL,
                };
                let quad = [
                    rv(p0, u0, f.top),
                    rv(p1, u1, f.top),
                    rv(p1, u1, bed),
                    rv(p0, u0, bed),
                ];
                push_tri(&mut v, [quad[0], quad[1], quad[2]], f.out);
                push_tri(&mut v, [quad[0], quad[2], quad[3]], f.out);
            }
        }
        let (fa, fb) = (ea, eb);
        let len = (fb - fa).magnitude();
        let along0 = along0 + ta * len_full;
        let mut pts: Vec<[WaterVert; 2]> = Vec::new();
        for &(d, y) in &rows {
            let (qa, qb) = (push(fa, d), push(fb, d));
            let s = (f.top - y) / drop.max(1e-4);
            let n = v3(f.out.x, 0.2505, f.out.y).unitized();
            pts.push([
                WaterVert {
                    pos: v3(qa.x, y, qa.y),
                    upright: n,
                    tex_coord: v2(along0, y),
                    uv2: v2(s, drop),
                    sort: KIND_FALL,
                },
                WaterVert {
                    pos: v3(qb.x, y, qb.y),
                    upright: n,
                    tex_coord: v2(along0 + len, y),
                    uv2: v2(s, drop),
                    sort: KIND_FALL,
                },
            ]);
        }
        for w in pts.windows(2) {
            let ([a0, b0], [a1, b1]) = (w[0], w[1]);
            push_tri(&mut v, [a0, b0, b1], f.out);
            push_tri(&mut v, [a0, b1, a1], f.out);
        }
        if !f.waterfall {
            continue;
        }
        let fy = f.bottom + if f.into_water { 0.1205 } else { 0.0505 };
        let kind = if f.into_water { KIND_FOAM } else { KIND_BASIN };
        let mid = (fa + fb) * 0.5;
        let half = (fb - fa).magnitude() * 0.5;
        let centre = push(mid, base_out - 0.1005);
        let side = (fb - fa).unitized();
        let radius = half + p.foam_reach * 0.6005;
        let (rx, ry) = (radius, p.foam_reach.max(0.2005));
        let fv = |q: Vec2, o: f32| WaterVert {
            pos: v3(q.x, fy, q.y),
            upright: up,
            tex_coord: q,
            uv2: v2(o, drop),
            sort: kind,
        };
        let rim = |i: usize| {
            let a = std::f32::consts::PI * i as f32 / FAN_SEGMENTS as f32;
            centre + side * (a.cos() * rx) + f.out * (a.sin() * ry)
        };
        for i in 0..FAN_SEGMENTS {
            push_tri_up(
                &mut v,
                [fv(centre, 0.0), fv(rim(i), 1.0), fv(rim(i + 1), 1.0)],
            );
        }
        let n = ((len * p.spray_density).ceil() as usize).max(1) + 2;
        for i in 0..n {
            let h = hash01(
                iv2(f.cell.x * 7 + f.edge as i32, f.cell.y * 13 + i as i32),
                91,
            );
            let h2 = hash01(
                iv2(i as i32 * 5 + 1, f.cell.x - f.cell.y + f.edge as i32),
                33,
            );
            let t = (i as f32 + 0.5) / n as f32;
            let q = fa.lerp(fb, 0.1505 + 0.7005 * t)
                + f.out * (base_out + 0.1005 + h * ry * 0.4505)
                + side * ((h2 - 0.5) * 0.3005);
            let c = v3(q.x, f.bottom + 0.1005, q.y);
            let size = 0.3005 + 0.2505 * h2 + (drop * 0.0205).min(0.2005);
            let corners = [v2(-1.0, -1.0), v2(1.0, -1.0), v2(1.0, 1.0), v2(-1.0, 1.0)];
            let sv = |k: usize| WaterVert {
                pos: c,
                upright: up,
                tex_coord: v2(h, size),
                uv2: corners[k],
                sort: KIND_SPRAY,
            };
            v.extend([sv(0), sv(1), sv(2), sv(0), sv(2), sv(3)]);
        }
    }
    v
}
fn tri_up(v: &mut Vec<WaterVert>, t: [Vec3; 3], kind: f32) {
    let n = (t[2] - t[0]).cross(t[1] - t[0]);
    let t = if n.y < 0.0 { [t[0], t[2], t[1]] } else { t };
    for p in t {
        v.push(WaterVert {
            pos: p,
            upright: v3(0.0, 1.0, 0.0),
            tex_coord: p.xz(),
            uv2: Vec2::ZERO,
            sort: kind,
        });
    }
}
fn push_tri_up(v: &mut Vec<WaterVert>, t: [WaterVert; 3]) {
    let n = (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos);
    v.extend(if n.y < 0.0 { [t[0], t[2], t[1]] } else { t });
}
fn push_tri(v: &mut Vec<WaterVert>, t: [WaterVert; 3], out: Vec2) {
    let n = (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos);
    let t = if v2(n.x, n.z).innerprod(out) < 0.0 {
        [t[0], t[2], t[1]]
    } else {
        t
    };
    v.extend(t);
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::{GridConfig, GridType};
    fn store() -> TerrainStore {
        let mut s = TerrainStore::new(GridConfig {
            grid: GridType::HexRings,
            ring_radius: 3,
            ..Default::default()
        });
        s.add_chunk(iv2(0, 0), MergeMode::Cubic);
        s
    }
    #[test]
    fn falls_need_a_lower_neighbour_and_respect_the_threshold() {
        let mut s = store();
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        let nbs = s.config.edge_neighbors(c);
        for (i, nb) in nbs.iter().enumerate() {
            s.set_height(*nb, [4.0, 4.0, 4.0, 4.0, 1.0, 3.0][i]);
        }
        s.set_height(c, 3.5);
        let water: WaterMap = [(c, 3.9)].into_iter().collect();
        let p = FallParams {
            min_drop: 2.5,
            ..Default::default()
        };
        let falls = find_falls(&s, &water, &p);
        assert_eq!(falls.len(), 2);
        let big = falls.iter().find(|f| f.neighbor == nbs[4]).unwrap();
        assert!(
            big.waterfall
                && !big.into_water
                && (big.top - 3.9).abs() < 1e-5
                && (big.bottom - 1.0).abs() < 1e-5
        );
        let small = falls.iter().find(|f| f.neighbor == nbs[5]).unwrap();
        assert!(!small.waterfall);
        let d = s.config.sample_pos(nbs[4]) - s.config.sample_pos(c);
        assert!(big.out.innerprod(d.unitized()) > 0.99);
    }
    #[test]
    fn a_continuous_sheet_does_not_fall_into_itself_and_lands_on_the_sea() {
        let mut s = store();
        let cells = s.config.chunk_cells(iv2(0, 0));
        for c in &cells {
            s.set_height(*c, -1.0);
        }
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        let nb = s.config.edge_neighbors(c)[0];
        s.set_height(c, 5.0);
        s.set_height(nb, 5.0);
        let water: WaterMap = [(c, 5.4), (nb, 5.4)].into_iter().collect();
        let p = FallParams {
            sea: Some(0.0),
            ..Default::default()
        };
        let falls = find_falls(&s, &water, &p);
        assert!(
            falls.iter().all(|f| f.neighbor != nb && f.neighbor != c),
            "no fall between pools"
        );
        assert_eq!(falls.len(), 10, "both pools spill on their other 5 edges");
        assert!(
            falls
                .iter()
                .all(|f| f.into_water && f.waterfall && f.bottom.abs() < 1e-5)
        );
        let mesh = build_water_mesh(&s, &water, &falls, &p);
        assert_eq!(mesh.len() % 3, 0);
        let kinds = |k: f32| mesh.iter().filter(|v| v.sort == k).count();
        assert_eq!(kinds(KIND_POOL), 2 * 6 * 3);
        assert!(
            kinds(KIND_FALL) > 0
                && kinds(KIND_FOAM) == 10 * FAN_SEGMENTS * 3
                && kinds(KIND_SPRAY) > 0
        );
        assert_eq!(
            kinds(KIND_BASIN),
            0,
            "falls into the sea foam, no landing basin"
        );
        let fall: Vec<&WaterVert> = mesh.iter().filter(|v| v.sort == KIND_FALL).collect();
        let (lo, hi) = fall.iter().fold((f32::MAX, f32::MIN), |(a, b), v| {
            (a.min(v.pos.y), b.max(v.pos.y))
        });
        assert!((hi - 5.4).abs() < 1e-5 && lo.abs() < 1e-5, "{lo}..{hi}");
        for t in mesh.chunks_exact(3) {
            let n = (t[2].pos - t[0].pos).cross(t[1].pos - t[0].pos);
            match t[0].sort {
                k if k == KIND_FALL => assert!(v2(n.x, n.z).innerprod(t[0].upright.xz()) >= -1e-6),
                k if k == KIND_POOL || k == KIND_FOAM || k == KIND_BASIN => assert!(n.y > 0.0),
                _ => {}
            }
        }
    }
    #[test]
    fn neighbouring_fall_sheets_of_one_cell_meet_at_the_corner() {
        let mut s = store();
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        s.set_height(c, 4.0);
        let water: WaterMap = [(c, 4.3)].into_iter().collect();
        let p = FallParams::default();
        let falls = find_falls(&s, &water, &p);
        assert_eq!(falls.len(), 6);
        let mesh = build_water_mesh(&s, &water, &falls, &p);
        let key = |v: &WaterVert| {
            (
                (v.pos.x * 1000.0).round() as i32,
                (v.pos.y * 1000.0).round() as i32,
                (v.pos.z * 1000.0).round() as i32,
            )
        };
        let mut count: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (i, t) in mesh.chunks_exact(3).enumerate() {
            for v in t {
                if v.sort == KIND_FALL && v.pos.y < 0.01 {
                    let e = count.entry(key(v)).or_default();
                    if !e.contains(&(i / 2)) {
                        e.push(i / 2);
                    }
                }
            }
        }
        assert_eq!(count.len(), 6, "six shared base corners");
    }
    #[test]
    fn a_single_spilling_edge_is_a_ribbon_with_a_closed_lip() {
        let mut s = store();
        let c = s.config.global_of(iv2(0, 0), iv2(3, 3));
        let nbs = s.config.edge_neighbors(c);
        for (i, nb) in nbs.iter().enumerate() {
            s.set_height(*nb, if i == 2 { 1.0 } else { 4.0 });
        }
        s.set_height(c, 3.5);
        let water: WaterMap = [(c, 4.0)].into_iter().collect();
        let p = FallParams::default();
        let falls = find_falls(&s, &water, &p);
        assert_eq!(falls.len(), 1);
        let f = falls[0];
        let mesh = build_water_mesh(&s, &water, &falls, &p);
        let edge = f.b - f.a;
        let along = |v: &WaterVert| (v.pos.xz() - f.a).innerprod(edge) / edge.innerprod(edge);
        let low: Vec<f32> = mesh
            .iter()
            .filter(|v| v.sort == KIND_FALL && v.pos.y < 3.4)
            .map(along)
            .collect();
        let (lo, hi) = low
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
        assert!(
            lo > 0.05 && lo < p.sheet_inset + 1e-3 && hi < 0.95 && hi > 1.0 - p.sheet_inset - 1e-3,
            "{lo}..{hi}"
        );
        let lip: Vec<f32> = mesh
            .iter()
            .filter(|v| v.sort == KIND_FALL && v.pos.y >= 3.5 - 1e-4)
            .map(along)
            .collect();
        let (lo, hi) = lip
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
        assert!(lo.abs() < 0.03 && (hi - 1.0).abs() < 0.03, "{lo}..{hi}");
        assert!(f.waterfall && !f.into_water);
        let basin: Vec<&WaterVert> = mesh.iter().filter(|v| v.sort == KIND_BASIN).collect();
        assert_eq!(basin.len(), FAN_SEGMENTS * 3);
        assert!(mesh.iter().all(|v| v.sort != KIND_FOAM));
        let centre = (f.a + f.b) * 0.5;
        assert!(
            basin
                .iter()
                .all(|v| (v.pos.xz() - centre).innerprod(f.out) > -1e-3
                    && (v.pos.y - 1.0505).abs() < 1e-4)
        );
        let spray: Vec<&WaterVert> = mesh.iter().filter(|v| v.sort == KIND_SPRAY).collect();
        assert!(
            !spray.is_empty()
                && spray
                    .iter()
                    .all(|v| v.pos.y < 1.5 && (v.pos.xz() - centre).innerprod(f.out) > 0.0)
        );
    }
}
