use crate::grid::{GridConfig, GridType};
use crate::math::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum MeadowBloom {
    Yarrow = 2,
    Foxglove = 3,
    Cowslip = 4,
    Harebell = 5,
    Mallow = 6,
    Tansy = 7,
    Bluebell = 8,
    Sneezewort = 9,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WildPick {
    Flower(MeadowBloom),
    Bilberry {
        stage: u8,
    },
}
pub const WILD_PICKS: [WildPick; 9] = [
    WildPick::Flower(MeadowBloom::Yarrow),
    WildPick::Flower(MeadowBloom::Foxglove),
    WildPick::Flower(MeadowBloom::Cowslip),
    WildPick::Flower(MeadowBloom::Harebell),
    WildPick::Flower(MeadowBloom::Mallow),
    WildPick::Flower(MeadowBloom::Tansy),
    WildPick::Flower(MeadowBloom::Bluebell),
    WildPick::Flower(MeadowBloom::Sneezewort),
    WildPick::Bilberry { stage: 7 },
];
impl WildPick {
    pub fn ordinal(self) -> usize {
        match self {
            WildPick::Flower(f) => f as usize - 2,
            WildPick::Bilberry { .. } => 8,
        }
    }
    pub fn moniker(self) -> &'static str {
        match self {
            WildPick::Flower(MeadowBloom::Yarrow) => "Yarrow",
            WildPick::Flower(MeadowBloom::Foxglove) => "Foxglove",
            WildPick::Flower(MeadowBloom::Cowslip) => "Cowslip",
            WildPick::Flower(MeadowBloom::Harebell) => "Harebell",
            WildPick::Flower(MeadowBloom::Mallow) => "Mallow",
            WildPick::Flower(MeadowBloom::Tansy) => "Tansy",
            WildPick::Flower(MeadowBloom::Bluebell) => "Bluebell",
            WildPick::Flower(MeadowBloom::Sneezewort) => "Sneezewort",
            WildPick::Bilberry { .. } => "Bilberry",
        }
    }
    pub fn tint(self) -> [f32; 3] {
        match self {
            WildPick::Flower(MeadowBloom::Yarrow) => [0.9505, 0.9305, 0.8505],
            WildPick::Flower(MeadowBloom::Foxglove) => [0.7805, 0.3805, 0.7205],
            WildPick::Flower(MeadowBloom::Cowslip) => [0.9805, 0.8205, 0.2805],
            WildPick::Flower(MeadowBloom::Harebell) => [0.5205, 0.6005, 0.9505],
            WildPick::Flower(MeadowBloom::Mallow) => [0.9205, 0.5505, 0.7205],
            WildPick::Flower(MeadowBloom::Tansy) => [0.9805, 0.7805, 0.1805],
            WildPick::Flower(MeadowBloom::Bluebell) => [0.3505, 0.4205, 0.9205],
            WildPick::Flower(MeadowBloom::Sneezewort) => [0.9605, 0.9605, 0.9005],
            WildPick::Bilberry { .. } => [0.2505, 0.4205, 0.2405],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeRng {
    pub state: u32,
}
impl LatticeRng {
    pub fn new(seed: u32) -> LatticeRng {
        let mut r = LatticeRng {
            state: if seed == 0 { 0x6E62_4EB7 } else { seed },
        };
        r.next_state();
        r
    }
    pub fn for_cell(index: u32) -> LatticeRng {
        LatticeRng::new(index.wrapping_mul(73_856_093) ^ 19_349_663)
    }
    pub fn next_state(&mut self) -> u32 {
        let t = self.state;
        let mut s = t;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.state = s;
        t
    }
    pub fn next_float(&mut self) -> f32 {
        f32::from_bits(0x3f80_0000 | (self.next_state() >> 9)) - 1.0
    }
    pub fn next_int(&mut self, min: i32, max: i32) -> i32 {
        let range = (max - min).max(0) as u64;
        ((range * self.next_state() as u64) >> 32) as i32 + min
    }
}
pub fn shuffle<T>(v: &mut [T], rng: &mut LatticeRng) {
    let n = v.len() as i32;
    for i in 0..(n - 1).max(0) {
        let j = rng.next_int(i, n);
        v.swap(i as usize, j as usize);
    }
}
pub fn clustered_wild_flower(
    noise: [f32; 4],
    ground_water: f32,
    rng: &mut LatticeRng,
    extra_wet: bool,
) -> WildPick {
    let bit = rng.next_state() & 1;
    if noise[0] > 0.5 && noise[2] > 0.7005 && bit != 0 {
        return WildPick::Bilberry {
            stage: if ground_water >= 0.3004 { 7 } else { 9 },
        };
    }
    let n = noise[1];
    let mid = (0.4..=0.6005).contains(&n);
    use MeadowBloom::*;
    let f = if !extra_wet {
        match (n >= 0.4005, n <= 0.6005, bit == 0) {
            (true, true, true) => Tansy,
            (true, true, false) => Harebell,
            (true, false, true) => Mallow,
            (true, false, false) => Yarrow,
            (false, _, true) => Foxglove,
            (false, _, false) => Bluebell,
        }
    } else if mid || bit == 0 {
        Cowslip
    } else {
        Sneezewort
    };
    WildPick::Flower(f)
}
pub fn cell_noise(p: Vec2, seed: u32, scale: f32) -> [f32; 4] {
    const PERIODS: [f32; 4] = [128.0 / 4.0965, 64.0 / 4.0965, 32.0 / 4.0965, 16.0 / 4.0965];
    let s = scale.max(0.0105);
    let mut out = [0.0; 4];
    for (c, period) in PERIODS.iter().enumerate() {
        let q = p / (period * s);
        let v = fbm(
            q + v2(17.3005 * c as f32, -9.1005 * c as f32),
            seed.wrapping_add(0x51F1 * c as u32),
            3,
        );
        out[c] = (0.5 + v * 0.9005).clamp(0.0, 1.0);
    }
    out
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellClimate {
    pub noise: [f32; 4],
    pub ground_water: f32,
    pub tree_cover: f32,
    pub river: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WildFlowerRules {
    pub bloom_threshold: f32,
    pub slope_limit: f32,
    pub max_river: f32,
    pub points_per_cell: u32,
    pub jitter: f32,
    pub density: f32,
    pub seed: u32,
}
impl Default for WildFlowerRules {
    fn default() -> Self {
        WildFlowerRules {
            bloom_threshold: 0.1004,
            slope_limit: 0.3764,
            max_river: 0.7004,
            points_per_cell: 8,
            jitter: 0.4005,
            density: 1.0,
            seed: 0,
        }
    }
}
pub fn slope_ratio(normal: Vec3) -> f32 {
    let ny = normal.y.clamp(1e-4, 1.0);
    (0.5 * (1.0 - ny * ny).max(0.0).sqrt() / ny).clamp(0.0, 1.0)
}
pub const BLOOM_CELL_AREA: f32 = 4.0 * 4.0 * 0.866_025_4;
pub fn area_density(cfg: &GridConfig) -> f32 {
    let cs = cfg.cell_size_safe();
    let area = match cfg.grid {
        GridType::Square => cs.x * cs.y,
        GridType::Triangle => cs.x * cs.x * 0.433_012_7,
        GridType::Hex | GridType::HexRings => cs.x * cs.x * 0.866_025_4,
    };
    (area / BLOOM_CELL_AREA).clamp(0.0105, 16.0)
}
pub fn species_mesh(pick: WildPick, seed: u32) -> crate::scatter::PropMesh {
    let mut m = crate::scatter::PropMesh {
        positions: Vec::new(),
        normals: Vec::new(),
        tints: Vec::new(),
    };
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, col: [f32; 3]| {
        let n = (b - a).cross(c - a).unitized();
        for (p, q, r, nn) in [(a, c, b, n), (a, b, c, n * -1.0)] {
            for v in [p, q, r] {
                m.positions.push(v);
                m.normals.push(nn);
                m.tints.push([col[0], col[1], col[2], 1.0]);
            }
        }
    };
    let blossom = pick.tint();
    let centre = match pick {
        WildPick::Flower(MeadowBloom::Cowslip | MeadowBloom::Tansy) => [0.9505, 0.8005, 0.2005],
        WildPick::Flower(MeadowBloom::Bluebell) => [0.1205, 0.0805, 0.0805],
        _ => [
            blossom[0] * 0.7005,
            blossom[1] * 0.7005,
            blossom[2] * 0.7005,
        ],
    };
    let stem = [0.2805, 0.5, 0.2005];
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 / 3.0 + 0.3005 * hash01(iv2(k, 1), seed));
        let d = 0.0505 + 0.1205 * hash01(iv2(k, 2), seed);
        let (x, z) = (a.cos() * d, a.sin() * d);
        let h = 0.3205 + 0.1805 * hash01(iv2(k, 3), seed);
        let top = v3(x, h, z);
        tri(v3(x - 0.0305, 0.0, z), v3(x + 0.0305, 0.0, z), top, stem);
        tri(v3(x, 0.0, z - 0.0305), v3(x, 0.0, z + 0.0305), top, stem);
        tri(
            v3(x, 0.0805, z),
            v3(x + 0.1205, 0.1605, z + 0.0405),
            v3(x, 0.2005, z),
            stem,
        );
        let r = 0.0705 + 0.0305 * hash01(iv2(k, 4), seed);
        let c = top + v3(0.0, 0.0105, 0.0);
        for i in 0..5 {
            let a0 = std::f32::consts::TAU * i as f32 / 5.0;
            let a1 = a0 + std::f32::consts::TAU / 5.0;
            let p0 = c + v3(a0.cos() * r, 0.0255, a0.sin() * r);
            let p1 = c + v3(a1.cos() * r, 0.0255, a1.sin() * r);
            tri(c, p0, p1, blossom);
        }
        tri(
            c + v3(-0.0205, 0.0305, 0.0),
            c + v3(0.0205, 0.0305, 0.0),
            c + v3(0.0, 0.0305, 0.0255),
            centre,
        );
    }
    m
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WildFlowerSpot {
    pub pos: Vec2,
    pub choose: WildPick,
    pub wet: bool,
}
pub fn cell_center(cfg: &GridConfig, c: IVec2) -> Vec2 {
    if cfg.grid == GridType::Square {
        cfg.sample_pos(c) + cfg.cell_size_safe() * 0.5
    } else {
        cfg.sample_pos(c)
    }
}
pub fn cell_points(cfg: &GridConfig, c: IVec2, per_cell: u32) -> (Vec<Vec2>, f32) {
    let cs = cfg.cell_size_safe();
    let n = per_cell.clamp(1, 32) as i32;
    let step = cs.x.min(cs.y) / n as f32;
    let centre = cell_center(cfg, c);
    let half = cs.x.max(cs.y) * 0.6005;
    let k = (half / step).ceil() as i32;
    let mut out = Vec::new();
    for j in -k..=k {
        for i in -k..=k {
            let p = centre
                + v2(
                    (i as f32 + 0.5 * (n % 2 == 0) as i32 as f32) * step,
                    (j as f32 + 0.5 * (n % 2 == 0) as i32 as f32) * step,
                );
            let owner = if cfg.grid == GridType::Square {
                iv2((p.x / cs.x).floor() as i32, (p.y / cs.y).floor() as i32)
            } else {
                cfg.world_to_sample(p)
            };
            if owner == c {
                out.push(p);
            }
        }
    }
    (out, step)
}
pub fn cell_flowers(
    rules: &WildFlowerRules,
    index: u32,
    points: &[Vec2],
    spacing: f32,
    climate: &CellClimate,
    slope_at: &dyn Fn(Vec2) -> Option<f32>,
) -> Vec<WildFlowerSpot> {
    let mut out = Vec::new();
    if climate.river > rules.max_river
        || climate.ground_water < rules.bloom_threshold
        || points.is_empty()
    {
        return out;
    }
    let mut rng = LatticeRng::for_cell(index ^ rules.seed.wrapping_mul(0x9E37_79B9));
    let open = climate.tree_cover <= 0.1505;
    let nz = climate.noise[2];
    let dense = |a: f32, b: f32| ((nz * a + b) * rules.density.max(0.0)) as i32;
    let count = (if open {
        dense(16.0, 8.0)
    } else {
        dense(8.0, 4.0)
    })
    .min(points.len() as i32)
    .max(0) as usize;
    let mut order: Vec<usize> = (0..points.len()).collect();
    shuffle(&mut order, &mut rng);
    let jitter = rules.jitter * spacing;
    let place = |slot: usize, wet: bool, rng: &mut LatticeRng, out: &mut Vec<WildFlowerSpot>| {
        let p = points[order[slot]];
        let Some(s) = slope_at(p) else { return };
        if s >= rules.slope_limit {
            return;
        }
        let jx = (rng.next_float() * 2.0 - 1.0) * jitter;
        let jz = (rng.next_float() * 2.0 - 1.0) * jitter;
        let pick = clustered_wild_flower(climate.noise, climate.ground_water, rng, wet);
        out.push(WildFlowerSpot {
            pos: p + v2(jx, jz),
            choose: pick,
            wet,
        });
    };
    for k in 0..count {
        place(k, false, &mut rng, &mut out);
    }
    if climate.ground_water >= rules.bloom_threshold * 2.0 {
        let wet = if open {
            dense(12.0, 12.0)
        } else {
            dense(6.0, 6.0)
        };
        let wet = wet.min(points.len() as i32 - count as i32).max(0) as usize;
        for k in 0..wet {
            place(count + k, true, &mut rng, &mut out);
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_steps_match_xorshift32() {
        let r = LatticeRng::new(1);
        assert_eq!(r.state, 270_369);
        let mut r = LatticeRng::new(1);
        let f = r.next_float();
        assert!((0.0..1.0).contains(&f));
        let mut r = LatticeRng::new(7);
        for _ in 0..1000 {
            let v = r.next_int(3, 9);
            assert!((3..9).contains(&v));
        }
    }
    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v, &mut LatticeRng::for_cell(12));
        let mut s = v.clone();
        s.sort();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
        assert_ne!(v, s);
    }
    #[test]
    fn species_bands_follow_noise_y() {
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..64 {
            let mut r = LatticeRng::new(i + 1);
            let p = clustered_wild_flower([0.0, 0.2, 0.0, 0.0], 0.2, &mut r, false);
            assert!(
                matches!(
                    p,
                    WildPick::Flower(MeadowBloom::Foxglove | MeadowBloom::Bluebell)
                ),
                "{p:?}"
            );
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, false);
            assert!(
                matches!(
                    p,
                    WildPick::Flower(MeadowBloom::Tansy | MeadowBloom::Harebell)
                ),
                "{p:?}"
            );
            let p = clustered_wild_flower([0.0, 0.8, 0.0, 0.0], 0.2, &mut r, false);
            assert!(
                matches!(
                    p,
                    WildPick::Flower(MeadowBloom::Mallow | MeadowBloom::Yarrow)
                ),
                "{p:?}"
            );
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, true);
            assert_eq!(p, WildPick::Flower(MeadowBloom::Cowslip));
            let p = clustered_wild_flower([0.9, 0.1, 0.9, 0.0], 0.2, &mut r, true);
            seen.insert(p);
        }
        assert!(seen.contains(&WildPick::Bilberry { stage: 9 }));
        assert!(
            seen.contains(&WildPick::Flower(MeadowBloom::Sneezewort))
                || seen.contains(&WildPick::Flower(MeadowBloom::Cowslip))
        );
    }
    #[test]
    fn counts_follow_ground_water_and_trees() {
        let cfg = GridConfig {
            grid: GridType::Hex,
            chunk_size: 8,
            cell_size: v2(4.0, 4.0),
            ..Default::default()
        };
        let (pts, step) = cell_points(&cfg, iv2(3, 3), 8);
        assert!(pts.len() >= 40 && pts.len() <= 70, "{} points", pts.len());
        let flat = |_: Vec2| Some(0.0);
        let rules = WildFlowerRules::default();
        let mut c = CellClimate {
            noise: [0.3, 0.5, 0.5, 0.5],
            ground_water: 0.05,
            tree_cover: 0.0,
            river: 0.0,
        };
        assert!(
            cell_flowers(&rules, 1, &pts, step, &c, &flat).is_empty(),
            "too dry"
        );
        c.ground_water = 0.15;
        let dry = cell_flowers(&rules, 1, &pts, step, &c, &flat);
        assert_eq!(dry.len(), 16, "noise.z 0.5 → 0.5 · 16 + 8");
        assert!(dry.iter().all(|s| !s.wet));
        c.ground_water = 0.25;
        let wet = cell_flowers(&rules, 1, &pts, step, &c, &flat);
        assert_eq!(wet.len(), 16 + 18, "+ 0.5 · 12 + 12 extra-wet flowers");
        c.tree_cover = 0.5;
        assert_eq!(cell_flowers(&rules, 1, &pts, step, &c, &flat).len(), 8 + 9);
        c.river = 0.9;
        assert!(
            cell_flowers(&rules, 1, &pts, step, &c, &flat).is_empty(),
            "river"
        );
        let steep = |_: Vec2| Some(0.5);
        c.river = 0.0;
        assert!(
            cell_flowers(&rules, 1, &pts, step, &c, &steep).is_empty(),
            "slope cut-off"
        );
        assert_eq!(
            cell_flowers(&rules, 9, &pts, step, &c, &flat),
            cell_flowers(&rules, 9, &pts, step, &c, &flat)
        );
    }
    #[test]
    fn noise_is_clustered() {
        let a = cell_noise(v2(10.0, 10.0), 1, 1.0);
        let b = cell_noise(v2(11.0, 10.0), 1, 1.0);
        assert!((a[1] - b[1]).abs() < 0.2);
        let vals: Vec<f32> = (0..64)
            .map(|i| cell_noise(v2(i as f32 * 13.0, i as f32 * 7.0), 1, 1.0)[1])
            .collect();
        let (lo, hi) = vals
            .iter()
            .fold((1.0f32, 0.0f32), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(lo < 0.4 && hi > 0.6, "noise spans the bands: {lo}..{hi}");
    }
    #[test]
    fn slope_ratio_matches_the_plane_angle() {
        assert_eq!(slope_ratio(v3(0.0, 1.0, 0.0)), 0.0);
        let n = v3(1.0, 1.0, 0.0).unitized();
        assert!((slope_ratio(n) - 0.5).abs() < 1e-4);
    }
}
