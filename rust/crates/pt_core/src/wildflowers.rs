//! Clustered wild flowers — port of Minor Deity's flower placement (MIT, Re cOg Mission):
//!
//! - `BurstUtils.GetClusteredWildFlower` (`ghidra-decomp/BurstUtils.c`): picks the species
//!   from the cell's four low-frequency noise channels, so neighbouring cells share species and
//!   flowers grow in patches (noise.y bands choose knapweed/cornflower, St John's wort/chicory
//!   or poppy/chamomile; wet ground switches to bouncing betty/goldenrod; a noise.x × noise.z
//!   corner becomes bilberry bushes);
//! - the flower pass of `UpdateDoodadsJob.Execute` (`ghidra-decomp/UpdateDoodadsJob.c`): per
//!   cell RNG `Random(hexIndex · 123456 + 123)`, no flowers on rivers (`riverStrength01 > 0.7`)
//!   or below `flowerThreshold` ground water, `noise.z · 16 + 8` flowers on open ground
//!   (`· 8 + 4` under trees), shuffled map points, `±0.2` jitter, `DisallowDoodad`'s slope
//!   cut-off (`slope255 ≥ 96`), then a second "extra wet" pass (`noise.z · 12 + 12`, `· 6 + 6`)
//!   where ground water reaches twice the threshold;
//! - `IListExtensions.Shuffle` (Fisher–Yates with `NextInt(i, n)`) and the xorshift32 random
//!   generator the Burst code inlines;
//! - `CalculatePointSlopesParallelJob`: `slope01 = clamp(0.5 · |Δh| / distance)`.
//!
//! Minor Deity reads the noise from a 2048² RGBA noise texture (`kitFox_128-64-32-16`) sampled
//! at `position · 4.096`, and ground water from its climate simulation. Neither is ported:
//! [`cell_noise`] stands in with four value-noise octaves at the texture's periods
//! (128 / 64 / 32 / 16 px ⇒ 31.25 / 15.6 / 7.8 / 3.9 world units), and the caller supplies
//! ground water ([`CellClimate`]). Map points are a sub-lattice of `points_per_cell` × the
//! cell width (Minor Deity's hexes hold ~8 map points across).

use crate::grid::{GridConfig, GridType};
use crate::math::*;

/// `WildFlowerTypes` (value = the game's enum value).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum WildFlower {
    BouncingBetty = 2,
    Knapweed = 3,
    Chamomile = 4,
    Chicory = 5,
    Poppy = 6,
    StJohnsWort = 7,
    Cornflower = 8,
    /// `GoldenRod` and `Sunroot` share value 9 in the game.
    GoldenRod = 9,
}

/// What a flower slot becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WildPick {
    Flower(WildFlower),
    /// `WildGrassTypes.BushBillberry` at growth stage 7 (ground water ≥ 0.3) or 9.
    Bilberry { stage: u8 },
}

/// Every pick, in a stable order (index = [`WildPick::index`]).
pub const WILD_PICKS: [WildPick; 9] = [
    WildPick::Flower(WildFlower::BouncingBetty),
    WildPick::Flower(WildFlower::Knapweed),
    WildPick::Flower(WildFlower::Chamomile),
    WildPick::Flower(WildFlower::Chicory),
    WildPick::Flower(WildFlower::Poppy),
    WildPick::Flower(WildFlower::StJohnsWort),
    WildPick::Flower(WildFlower::Cornflower),
    WildPick::Flower(WildFlower::GoldenRod),
    WildPick::Bilberry { stage: 7 },
];

impl WildPick {
    /// `0..9` (bilberry stages share index 8).
    pub fn index(self) -> usize {
        match self {
            WildPick::Flower(f) => f as usize - 2,
            WildPick::Bilberry { .. } => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            WildPick::Flower(WildFlower::BouncingBetty) => "Bouncing Betty",
            WildPick::Flower(WildFlower::Knapweed) => "Knapweed",
            WildPick::Flower(WildFlower::Chamomile) => "Chamomile",
            WildPick::Flower(WildFlower::Chicory) => "Chicory",
            WildPick::Flower(WildFlower::Poppy) => "Poppy",
            WildPick::Flower(WildFlower::StJohnsWort) => "St John's Wort",
            WildPick::Flower(WildFlower::Cornflower) => "Cornflower",
            WildPick::Flower(WildFlower::GoldenRod) => "Goldenrod",
            WildPick::Bilberry { .. } => "Bilberry",
        }
    }

    /// Blossom colour (linear-ish sRGB) for the built-in flower prop.
    pub fn color(self) -> [f32; 3] {
        match self {
            WildPick::Flower(WildFlower::BouncingBetty) => [0.95, 0.78, 0.86],
            WildPick::Flower(WildFlower::Knapweed) => [0.72, 0.36, 0.78],
            WildPick::Flower(WildFlower::Chamomile) => [0.98, 0.97, 0.88],
            WildPick::Flower(WildFlower::Chicory) => [0.55, 0.66, 0.98],
            WildPick::Flower(WildFlower::Poppy) => [0.92, 0.18, 0.12],
            WildPick::Flower(WildFlower::StJohnsWort) => [1.0, 0.84, 0.2],
            WildPick::Flower(WildFlower::Cornflower) => [0.25, 0.42, 0.95],
            WildPick::Flower(WildFlower::GoldenRod) => [0.96, 0.72, 0.1],
            WildPick::Bilberry { .. } => [0.25, 0.42, 0.24],
        }
    }
}

/// `Unity.Mathematics.Random` as inlined by Burst: xorshift32 (13, 17, 5), values from the
/// state *before* the step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurstRandom {
    pub state: u32,
}

impl BurstRandom {
    /// `new Random(seed)` (one step; a zero seed is replaced as Unity asserts non-zero).
    pub fn new(seed: u32) -> BurstRandom {
        let mut r = BurstRandom { state: if seed == 0 { 0x6E62_4EB7 } else { seed } };
        r.next_state();
        r
    }

    /// The per-cell generator of `UpdateDoodadsJob`: `hexIndex · 123456 + 123`.
    pub fn for_cell(index: u32) -> BurstRandom {
        BurstRandom::new(index.wrapping_mul(123_456).wrapping_add(123))
    }

    /// Advances and returns the previous state.
    pub fn next_state(&mut self) -> u32 {
        let t = self.state;
        let mut s = t;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.state = s;
        t
    }

    /// `NextFloat()` in `[0, 1)`.
    pub fn next_float(&mut self) -> f32 {
        f32::from_bits(0x3f80_0000 | (self.next_state() >> 9)) - 1.0
    }

    /// `NextInt(min, max)`, `[min, max)`.
    pub fn next_int(&mut self, min: i32, max: i32) -> i32 {
        let range = (max - min).max(0) as u64;
        ((range * self.next_state() as u64) >> 32) as i32 + min
    }
}

/// `IListExtensions.Shuffle`.
pub fn shuffle<T>(v: &mut [T], rng: &mut BurstRandom) {
    let n = v.len() as i32;
    for i in 0..(n - 1).max(0) {
        let j = rng.next_int(i, n);
        v.swap(i as usize, j as usize);
    }
}

/// `GetClusteredWildFlower(noises, climate, random, extraWet)`; `ground_water` = `climate.y`.
pub fn clustered_wild_flower(noise: [f32; 4], ground_water: f32, rng: &mut BurstRandom, extra_wet: bool) -> WildPick {
    let bit = rng.next_state() & 1;
    if noise[0] > 0.5 && noise[2] > 0.7 && bit != 0 {
        return WildPick::Bilberry { stage: if ground_water >= 0.3 { 7 } else { 9 } };
    }
    let n = noise[1];
    let mid = (0.4..=0.6).contains(&n);
    use WildFlower::*;
    let f = if !extra_wet {
        match (n >= 0.4, n <= 0.6, bit == 0) {
            (true, true, true) => StJohnsWort,
            (true, true, false) => Chicory,
            (true, false, true) => Poppy,
            (true, false, false) => Chamomile,
            (false, _, true) => Knapweed,
            (false, _, false) => Cornflower,
        }
    } else if mid || bit == 0 {
        BouncingBetty
    } else {
        GoldenRod
    };
    WildPick::Flower(f)
}

/// Four noise channels in `[0, 1]` at a terrain-local point (stand-in for Minor Deity's noise
/// texture; see the module docs). `scale` multiplies the patch size.
pub fn cell_noise(p: Vec2, seed: u32, scale: f32) -> [f32; 4] {
    const PERIODS: [f32; 4] = [128.0 / 4.096, 64.0 / 4.096, 32.0 / 4.096, 16.0 / 4.096];
    let s = scale.max(0.01);
    let mut out = [0.0; 4];
    for (c, period) in PERIODS.iter().enumerate() {
        let q = p / (period * s);
        let v = fbm(q + v2(17.3 * c as f32, -9.1 * c as f32), seed.wrapping_add(0x51F1 * c as u32), 3);
        // fbm is roughly [-1, 1] with most mass near 0; stretch to cover [0, 1] like a texture.
        out[c] = (0.5 + v * 0.9).clamp(0.0, 1.0);
    }
    out
}

/// Per-cell inputs Minor Deity takes from its climate / tree / river systems.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellClimate {
    pub noise: [f32; 4],
    /// `climate.y` (0..1).
    pub ground_water: f32,
    /// `treeWeights.x + treeWeights.y` (≤ 0.15 = open ground, more flowers).
    pub tree_cover: f32,
    /// `riverStrength01`.
    pub river: f32,
}

/// Placement rules (Minor Deity values as defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct WildFlowerRules {
    /// `HexManager.flowerThreshold` (ground water needed for flowers; twice this for the wet pass).
    pub flower_threshold: f32,
    /// `DisallowDoodad` cut-off on the point slope (`slope255 ≥ 96`).
    pub max_slope01: f32,
    /// No flowers on cells with more river than this.
    pub max_river: f32,
    /// Map points across one cell.
    pub points_per_cell: u32,
    /// Jitter in units of the map-point spacing (Minor Deity: ±0.2 world units at ~0.5 spacing).
    pub jitter: f32,
    /// Multiplies every flower count (1 = Minor Deity).
    pub density: f32,
    pub seed: u32,
}

impl Default for WildFlowerRules {
    fn default() -> Self {
        WildFlowerRules { flower_threshold: 0.1, max_slope01: 96.0 / 255.0, max_river: 0.7, points_per_cell: 8, jitter: 0.4, density: 1.0, seed: 0 }
    }
}

/// `slope01` of `CalculatePointSlopesParallelJob` from a unit normal.
pub fn slope01(normal: Vec3) -> f32 {
    let ny = normal.y.clamp(1e-4, 1.0);
    (0.5 * (1.0 - ny * ny).max(0.0).sqrt() / ny).clamp(0.0, 1.0)
}

/// Area of a Minor Deity hex (`HexMetrics.hexBoundSize` 4 ⇒ 4 units across flats). Flower
/// counts are per hex there; [`area_density`] rescales them to other cell sizes.
pub const MD_HEX_AREA: f32 = 4.0 * 4.0 * 0.866_025_4;

/// Count multiplier that keeps Minor Deity's flowers per square unit on cells of `cfg`.
pub fn area_density(cfg: &GridConfig) -> f32 {
    let cs = cfg.cell_size_safe();
    let area = match cfg.grid {
        GridType::Square => cs.x * cs.y,
        GridType::Triangle => cs.x * cs.x * 0.433_012_7,
        GridType::Hex | GridType::HexRings => cs.x * cs.x * 0.866_025_4,
    };
    (area / MD_HEX_AREA).clamp(0.01, 16.0)
}

/// A single-species flower tuft (three stems, blossoms in the species colour), about 0.5 units
/// tall, as vertex-coloured, double-sided triangles. Stand-in for Minor Deity's flower meshes.
pub fn species_mesh(pick: WildPick, seed: u32) -> crate::scatter::PropMesh {
    let mut m = crate::scatter::PropMesh { positions: Vec::new(), normals: Vec::new(), colors: Vec::new() };
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, col: [f32; 3]| {
        let n = (b - a).cross(c - a).normalized();
        for (p, q, r, nn) in [(a, c, b, n), (a, b, c, n * -1.0)] {
            for v in [p, q, r] {
                m.positions.push(v);
                m.normals.push(nn);
                m.colors.push([col[0], col[1], col[2], 1.0]);
            }
        }
    };
    let blossom = pick.color();
    let centre = match pick {
        WildPick::Flower(WildFlower::Chamomile | WildFlower::BouncingBetty) => [0.95, 0.8, 0.2],
        WildPick::Flower(WildFlower::Poppy) => [0.12, 0.08, 0.08],
        _ => [blossom[0] * 0.7, blossom[1] * 0.7, blossom[2] * 0.7],
    };
    let stem = [0.28, 0.5, 0.2];
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 / 3.0 + 0.3 * hash01(iv2(k, 1), seed));
        let d = 0.05 + 0.12 * hash01(iv2(k, 2), seed);
        let (x, z) = (a.cos() * d, a.sin() * d);
        let h = 0.32 + 0.18 * hash01(iv2(k, 3), seed);
        let top = v3(x, h, z);
        tri(v3(x - 0.03, 0.0, z), v3(x + 0.03, 0.0, z), top, stem);
        tri(v3(x, 0.0, z - 0.03), v3(x, 0.0, z + 0.03), top, stem);
        // Leaf.
        tri(v3(x, 0.08, z), v3(x + 0.12, 0.16, z + 0.04), v3(x, 0.2, z), stem);
        // Blossom: a slightly cupped 5-petal fan facing up.
        let r = 0.07 + 0.03 * hash01(iv2(k, 4), seed);
        let c = top + v3(0.0, 0.01, 0.0);
        for i in 0..5 {
            let a0 = std::f32::consts::TAU * i as f32 / 5.0;
            let a1 = a0 + std::f32::consts::TAU / 5.0;
            let p0 = c + v3(a0.cos() * r, 0.025, a0.sin() * r);
            let p1 = c + v3(a1.cos() * r, 0.025, a1.sin() * r);
            tri(c, p0, p1, blossom);
        }
        tri(c + v3(-0.02, 0.03, 0.0), c + v3(0.02, 0.03, 0.0), c + v3(0.0, 0.03, 0.025), centre);
    }
    m
}

/// One placed flower.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WildFlowerSpot {
    pub pos: Vec2,
    pub pick: WildPick,
    /// The extra-wet pass placed it.
    pub wet: bool,
}

/// Centre of a cell (square grids address cells by their lower vertex).
pub fn cell_center(cfg: &GridConfig, c: IVec2) -> Vec2 {
    if cfg.grid == GridType::Square { cfg.sample_pos(c) + cfg.cell_size_safe() * 0.5 } else { cfg.sample_pos(c) }
}

/// Map points of a cell: a `points_per_cell`-across lattice clipped to the cell.
pub fn cell_points(cfg: &GridConfig, c: IVec2, per_cell: u32) -> (Vec<Vec2>, f32) {
    let cs = cfg.cell_size_safe();
    let n = per_cell.clamp(1, 32) as i32;
    let step = cs.x.min(cs.y) / n as f32;
    let centre = cell_center(cfg, c);
    let half = cs.x.max(cs.y) * 0.6;
    let k = (half / step).ceil() as i32;
    let mut out = Vec::new();
    for j in -k..=k {
        for i in -k..=k {
            let p = centre + v2((i as f32 + 0.5 * (n % 2 == 0) as i32 as f32) * step, (j as f32 + 0.5 * (n % 2 == 0) as i32 as f32) * step);
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

/// `UpdateDoodadsJob` flower pass for one cell. `index` seeds the cell RNG (a stable cell id);
/// `slope_at` returns the point's `slope01`, or `None` where nothing may grow.
pub fn cell_flowers(rules: &WildFlowerRules, index: u32, points: &[Vec2], spacing: f32, climate: &CellClimate, slope_at: &dyn Fn(Vec2) -> Option<f32>) -> Vec<WildFlowerSpot> {
    let mut out = Vec::new();
    if climate.river > rules.max_river || climate.ground_water < rules.flower_threshold || points.is_empty() {
        return out;
    }
    let mut rng = BurstRandom::for_cell(index ^ rules.seed.wrapping_mul(0x9E37_79B9));
    let open = climate.tree_cover <= 0.15;
    let nz = climate.noise[2];
    let dense = |a: f32, b: f32| ((nz * a + b) * rules.density.max(0.0)) as i32;
    let count = (if open { dense(16.0, 8.0) } else { dense(8.0, 4.0) }).min(points.len() as i32).max(0) as usize;
    let mut order: Vec<usize> = (0..points.len()).collect();
    shuffle(&mut order, &mut rng);
    let jitter = rules.jitter * spacing;
    let place = |slot: usize, wet: bool, rng: &mut BurstRandom, out: &mut Vec<WildFlowerSpot>| {
        let p = points[order[slot]];
        let Some(s) = slope_at(p) else { return };
        if s >= rules.max_slope01 {
            return;
        }
        let jx = (rng.next_float() * 2.0 - 1.0) * jitter;
        let jz = (rng.next_float() * 2.0 - 1.0) * jitter;
        let pick = clustered_wild_flower(climate.noise, climate.ground_water, rng, wet);
        out.push(WildFlowerSpot { pos: p + v2(jx, jz), pick, wet });
    };
    for k in 0..count {
        place(k, false, &mut rng, &mut out);
    }
    if climate.ground_water >= rules.flower_threshold * 2.0 {
        let wet = if open { dense(12.0, 12.0) } else { dense(6.0, 6.0) };
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
    fn random_matches_unity_xorshift() {
        // Unity.Mathematics.Random(1): state after the constructor step.
        let r = BurstRandom::new(1);
        assert_eq!(r.state, 270_369);
        let mut r = BurstRandom::new(1);
        let f = r.next_float();
        assert!((0.0..1.0).contains(&f));
        let mut r = BurstRandom::new(7);
        for _ in 0..1000 {
            let v = r.next_int(3, 9);
            assert!((3..9).contains(&v));
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v, &mut BurstRandom::for_cell(12));
        let mut s = v.clone();
        s.sort();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
        assert_ne!(v, s);
    }

    #[test]
    fn species_bands_follow_noise_y() {
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..64 {
            let mut r = BurstRandom::new(i + 1);
            let p = clustered_wild_flower([0.0, 0.2, 0.0, 0.0], 0.2, &mut r, false);
            assert!(matches!(p, WildPick::Flower(WildFlower::Knapweed | WildFlower::Cornflower)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, false);
            assert!(matches!(p, WildPick::Flower(WildFlower::StJohnsWort | WildFlower::Chicory)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.8, 0.0, 0.0], 0.2, &mut r, false);
            assert!(matches!(p, WildPick::Flower(WildFlower::Poppy | WildFlower::Chamomile)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, true);
            assert_eq!(p, WildPick::Flower(WildFlower::BouncingBetty));
            let p = clustered_wild_flower([0.9, 0.1, 0.9, 0.0], 0.2, &mut r, true);
            seen.insert(p);
        }
        // The noise.x / noise.z corner yields bilberries about half the time, else wet flowers.
        assert!(seen.contains(&WildPick::Bilberry { stage: 9 }));
        assert!(seen.contains(&WildPick::Flower(WildFlower::GoldenRod)) || seen.contains(&WildPick::Flower(WildFlower::BouncingBetty)));
    }

    #[test]
    fn counts_follow_ground_water_and_trees() {
        let cfg = GridConfig { grid: GridType::Hex, chunk_size: 8, cell_size: v2(4.0, 4.0), ..Default::default() };
        let (pts, step) = cell_points(&cfg, iv2(3, 3), 8);
        assert!(pts.len() >= 40 && pts.len() <= 70, "{} points", pts.len());
        let flat = |_: Vec2| Some(0.0);
        let rules = WildFlowerRules::default();
        let mut c = CellClimate { noise: [0.3, 0.5, 0.5, 0.5], ground_water: 0.05, tree_cover: 0.0, river: 0.0 };
        assert!(cell_flowers(&rules, 1, &pts, step, &c, &flat).is_empty(), "too dry");
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
        assert!(cell_flowers(&rules, 1, &pts, step, &c, &flat).is_empty(), "river");
        let steep = |_: Vec2| Some(0.5);
        c.river = 0.0;
        assert!(cell_flowers(&rules, 1, &pts, step, &c, &steep).is_empty(), "slope cut-off");
        // Deterministic.
        assert_eq!(cell_flowers(&rules, 9, &pts, step, &c, &flat), cell_flowers(&rules, 9, &pts, step, &c, &flat));
    }

    #[test]
    fn noise_is_clustered() {
        // Neighbouring cells (4 units apart) mostly agree on the species band; far cells vary.
        let a = cell_noise(v2(10.0, 10.0), 1, 1.0);
        let b = cell_noise(v2(11.0, 10.0), 1, 1.0);
        assert!((a[1] - b[1]).abs() < 0.2);
        let vals: Vec<f32> = (0..64).map(|i| cell_noise(v2(i as f32 * 13.0, i as f32 * 7.0), 1, 1.0)[1]).collect();
        let (lo, hi) = vals.iter().fold((1.0f32, 0.0f32), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(lo < 0.4 && hi > 0.6, "noise spans the bands: {lo}..{hi}");
    }

    #[test]
    fn slope01_matches_the_job() {
        assert_eq!(slope01(v3(0.0, 1.0, 0.0)), 0.0);
        // 45° → tan 1 → 0.5.
        let n = v3(1.0, 1.0, 0.0).normalized();
        assert!((slope01(n) - 0.5).abs() < 1e-4);
    }
}
