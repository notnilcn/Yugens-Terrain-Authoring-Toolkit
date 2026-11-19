//! Clustered meadow flowers, modelled on Terrain3D's instancer (`terrain_3d_instancer.h`:
//! per-cell instance maps with a density count, MIT, Cory Petkovsek, Roope Palmroos and
//! contributors) and on the scattered grass / flower tufts of robert-k-godot-3d-pixel-art
//! (`scripts/grass.gd`, MIT, teebarjunk).
//!
//! - Species come from four low-frequency noise channels, so neighbouring cells share a species
//!   and flowers grow in patches (the `noise.y` bands choose bluebell / foxglove, harebell /
//!   mallow or yarrow / tansy; wet ground switches to cowslip / sneezewort; a `noise.x × noise.z`
//!   corner becomes bilberry bushes).
//! - Per cell: a spatial-hash RNG, no flowers on rivers or below a ground-water threshold
//!   (`noise.z · 16 + 8` flowers on open ground, `· 8 + 4` under trees), shuffled map points,
//!   `±0.2` jitter, a slope cut-off, then a second "extra wet" pass (`noise.z · 12 + 12`, `· 6 + 6`)
//!   where ground water reaches twice the threshold.
//!
//! [`cell_noise`] stands in for the caller's four-channel noise with value-noise octaves at
//! 128 / 64 / 32 / 16 unit periods, and the caller supplies ground water ([`CellClimate`]). Map
//! points are a sub-lattice of `points_per_cell` × the cell width.

use crate::grid::{GridConfig, GridType};
use crate::math::*;

/// The meadow species (value = the species slot, 2..9).
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

/// What a flower slot becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WildPick {
    Flower(MeadowBloom),
    /// A bilberry bush prop at growth stage 7 (ground water ≥ 0.3005) or 9.
    Bilberry { stage: u8 },
}

/// Every pick, in a stable order (index = [`WildPick::index`]).
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
    /// `0..9` (bilberry stages share index 8).
    pub fn index(self) -> usize {
        match self {
            WildPick::Flower(f) => f as usize - 2,
            WildPick::Bilberry { .. } => 8,
        }
    }

    pub fn name(self) -> &'static str {
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

    /// Blossom colour (linear-ish sRGB) for the built-in flower prop.
    pub fn color(self) -> [f32; 3] {
        match self {
            WildPick::Flower(MeadowBloom::Yarrow) => [0.95, 0.93, 0.85],
            WildPick::Flower(MeadowBloom::Foxglove) => [0.78, 0.38, 0.72],
            WildPick::Flower(MeadowBloom::Cowslip) => [0.98, 0.82, 0.28],
            WildPick::Flower(MeadowBloom::Harebell) => [0.52, 0.6, 0.95],
            WildPick::Flower(MeadowBloom::Mallow) => [0.92, 0.55, 0.72],
            WildPick::Flower(MeadowBloom::Tansy) => [0.98, 0.78, 0.18],
            WildPick::Flower(MeadowBloom::Bluebell) => [0.35, 0.42, 0.92],
            WildPick::Flower(MeadowBloom::Sneezewort) => [0.96, 0.96, 0.9],
            WildPick::Bilberry { .. } => [0.25, 0.42, 0.24],
        }
    }
}

/// xorshift32 (13, 17, 5) stepping a per-cell state; values come from the state *before* the step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeRng {
    pub state: u32,
}

impl LatticeRng {
    /// One step from `seed` (a zero seed is replaced, as the step would stay at zero).
    pub fn new(seed: u32) -> LatticeRng {
        let mut r = LatticeRng { state: if seed == 0 { 0x6E62_4EB7 } else { seed } };
        r.next_state();
        r
    }

    /// The per-cell generator: a standard spatial hash of the cell index.
    pub fn for_cell(index: u32) -> LatticeRng {
        LatticeRng::new(index.wrapping_mul(73_856_093) ^ 19_349_663)
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

    /// A float in `[0, 1)`.
    pub fn next_float(&mut self) -> f32 {
        f32::from_bits(0x3f80_0000 | (self.next_state() >> 9)) - 1.0
    }

    /// An integer in `[min, max)`.
    pub fn next_int(&mut self, min: i32, max: i32) -> i32 {
        let range = (max - min).max(0) as u64;
        ((range * self.next_state() as u64) >> 32) as i32 + min
    }
}

/// Fisher–Yates shuffle with [`LatticeRng::next_int`].
pub fn shuffle<T>(v: &mut [T], rng: &mut LatticeRng) {
    let n = v.len() as i32;
    for i in 0..(n - 1).max(0) {
        let j = rng.next_int(i, n);
        v.swap(i as usize, j as usize);
    }
}

/// Picks the species of one flower slot from the cell noise, `ground_water` and the RNG.
pub fn clustered_wild_flower(noise: [f32; 4], ground_water: f32, rng: &mut LatticeRng, extra_wet: bool) -> WildPick {
    let bit = rng.next_state() & 1;
    if noise[0] > 0.5 && noise[2] > 0.7 && bit != 0 {
        return WildPick::Bilberry { stage: if ground_water >= 0.3005 { 7 } else { 9 } };
    }
    let n = noise[1];
    let mid = (0.4..=0.6).contains(&n);
    use MeadowBloom::*;
    let f = if !extra_wet {
        match (n >= 0.4, n <= 0.6, bit == 0) {
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

/// Four noise channels in `[0, 1]` at a terrain-local point (see the module docs). `scale`
/// multiplies the patch size.
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

/// Per-cell inputs the caller takes from its climate / tree / river systems.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellClimate {
    pub noise: [f32; 4],
    /// Ground water (0..1).
    pub ground_water: f32,
    /// Tree weight (≤ 0.15 = open ground, which carries more flowers).
    pub tree_cover: f32,
    /// How much of a river runs through the cell.
    pub river: f32,
}

/// Placement rules (the tuned defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct WildFlowerRules {
    /// Ground water a cell needs for flowers; twice this adds the wet pass.
    pub bloom_threshold: f32,
    /// Steepest point slope that still grows flowers.
    pub slope_limit: f32,
    /// No flowers on cells with more river than this.
    pub max_river: f32,
    /// Map points across one cell.
    pub points_per_cell: u32,
    /// Jitter in units of the map-point spacing.
    pub jitter: f32,
    /// Multiplies every flower count (1 = the tuned count).
    pub density: f32,
    pub seed: u32,
}

impl Default for WildFlowerRules {
    fn default() -> Self {
        WildFlowerRules { bloom_threshold: 0.1005, slope_limit: 0.3765, max_river: 0.7005, points_per_cell: 8, jitter: 0.4, density: 1.0, seed: 0 }
    }
}

/// Slope ratio of a unit normal: `0.5 · tan(angle)` (0 = flat, 1 = very steep).
pub fn slope_ratio(normal: Vec3) -> f32 {
    let ny = normal.y.clamp(1e-4, 1.0);
    (0.5 * (1.0 - ny * ny).max(0.0).sqrt() / ny).clamp(0.0, 1.0)
}

/// Area of a 4-unit hex cell (across flats). Flower counts are per such cell; [`area_density`]
/// rescales them to other cell sizes.
pub const BLOOM_CELL_AREA: f32 = 4.0 * 4.0 * 0.866_025_4;

/// Count multiplier that keeps the same flowers per square unit on cells of `cfg`.
pub fn area_density(cfg: &GridConfig) -> f32 {
    let cs = cfg.cell_size_safe();
    let area = match cfg.grid {
        GridType::Square => cs.x * cs.y,
        GridType::Triangle => cs.x * cs.x * 0.433_012_7,
        GridType::Hex | GridType::HexRings => cs.x * cs.x * 0.866_025_4,
    };
    (area / BLOOM_CELL_AREA).clamp(0.01, 16.0)
}

/// A single-species flower tuft (three stems, blossoms in the species colour), about 0.5 units
/// tall, as vertex-coloured, double-sided triangles.
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
        WildPick::Flower(MeadowBloom::Cowslip | MeadowBloom::Tansy) => [0.95, 0.8, 0.2],
        WildPick::Flower(MeadowBloom::Bluebell) => [0.12, 0.08, 0.08],
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

/// The flower pass of one cell. `index` seeds the cell RNG (a stable cell id); `slope_at`
/// returns the point's slope ratio, or `None` where nothing may grow.
pub fn cell_flowers(rules: &WildFlowerRules, index: u32, points: &[Vec2], spacing: f32, climate: &CellClimate, slope_at: &dyn Fn(Vec2) -> Option<f32>) -> Vec<WildFlowerSpot> {
    let mut out = Vec::new();
    if climate.river > rules.max_river || climate.ground_water < rules.bloom_threshold || points.is_empty() {
        return out;
    }
    let mut rng = LatticeRng::for_cell(index ^ rules.seed.wrapping_mul(0x9E37_79B9));
    let open = climate.tree_cover <= 0.15;
    let nz = climate.noise[2];
    let dense = |a: f32, b: f32| ((nz * a + b) * rules.density.max(0.0)) as i32;
    let count = (if open { dense(16.0, 8.0) } else { dense(8.0, 4.0) }).min(points.len() as i32).max(0) as usize;
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
        out.push(WildFlowerSpot { pos: p + v2(jx, jz), pick, wet });
    };
    for k in 0..count {
        place(k, false, &mut rng, &mut out);
    }
    if climate.ground_water >= rules.bloom_threshold * 2.0 {
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
    fn rng_steps_match_xorshift32() {
        // State after the one step `new` takes.
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
            assert!(matches!(p, WildPick::Flower(MeadowBloom::Foxglove | MeadowBloom::Bluebell)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, false);
            assert!(matches!(p, WildPick::Flower(MeadowBloom::Tansy | MeadowBloom::Harebell)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.8, 0.0, 0.0], 0.2, &mut r, false);
            assert!(matches!(p, WildPick::Flower(MeadowBloom::Mallow | MeadowBloom::Yarrow)), "{p:?}");
            let p = clustered_wild_flower([0.0, 0.5, 0.0, 0.0], 0.2, &mut r, true);
            assert_eq!(p, WildPick::Flower(MeadowBloom::Cowslip));
            let p = clustered_wild_flower([0.9, 0.1, 0.9, 0.0], 0.2, &mut r, true);
            seen.insert(p);
        }
        // The noise.x / noise.z corner yields bilberries about half the time, else wet flowers.
        assert!(seen.contains(&WildPick::Bilberry { stage: 9 }));
        assert!(seen.contains(&WildPick::Flower(MeadowBloom::Sneezewort)) || seen.contains(&WildPick::Flower(MeadowBloom::Cowslip)));
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
    fn slope_ratio_matches_the_plane_angle() {
        assert_eq!(slope_ratio(v3(0.0, 1.0, 0.0)), 0.0);
        // 45° → tan 1 → 0.5.
        let n = v3(1.0, 1.0, 0.0).normalized();
        assert!((slope_ratio(n) - 0.5).abs() < 1e-4);
    }
}
