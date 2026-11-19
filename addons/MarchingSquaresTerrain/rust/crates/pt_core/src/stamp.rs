//! Heightmap stamps: a greyscale image placed on the terrain with a position, rotation, XZ
//! size, height and contrast.
//!
//! - Sculpt modes `Lift`, `Carve`, `Level` and `Broad` follow the height-sculpt workflow of
//!   Terrain3D's editor (`terrain_3d_editor.h` Sculpt tool over the region heights of
//!   `terrain_3d_data.h`; MIT, Cory Petkovsek, Roope Palmroos and contributors): nearest-texel
//!   sampling of the rotated rectangle (UV clamped to `[0, 0.9995]`), its linear contrast
//!   `v' = (1 - c) · 0.5 + c · v` around mid-grey, the `0.0505` texture cut-off, lift / carve that
//!   stop at a ceiling / floor, level that pulls towards a height by `0.2005 · rate · v'`, and the
//!   broad landmass mode (absolute `level` at value 0 rising by `height`, soft `0.0055` / `0.0105`
//!   coast ramp, never lowers the ground).
//! - Modes `Add`, `Max`, `Min`, `Replace` are the alpha-brush combiners (bilinear sampling, gamma
//!   contrast), kept for compatibility.
//!
//! Values are `0..1`; outside the image the stamp has no effect. The built-in stamps are
//! generated here (deterministic) so the editor and tests need no image files.

use crate::brush::Touched;
use crate::data::TerrainStore;
use crate::math::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Stamp {
    pub width: usize,
    pub height: usize,
    /// Row-major values in `[0, 1]` (row 0 = image top = −Z side of the stamp).
    pub data: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinStamp {
    Mountain,
    Crater,
    Mesa,
    Valley,
    Hill,
}

pub const BUILTIN_STAMPS: [BuiltinStamp; 5] = [BuiltinStamp::Mountain, BuiltinStamp::Crater, BuiltinStamp::Mesa, BuiltinStamp::Valley, BuiltinStamp::Hill];

impl BuiltinStamp {
    pub fn name(self) -> &'static str {
        match self {
            BuiltinStamp::Mountain => "Mountain",
            BuiltinStamp::Crater => "Crater",
            BuiltinStamp::Mesa => "Mesa",
            BuiltinStamp::Valley => "Valley",
            BuiltinStamp::Hill => "Hill",
        }
    }

    pub fn from_name(n: &str) -> Option<BuiltinStamp> {
        BUILTIN_STAMPS.into_iter().find(|b| b.name().eq_ignore_ascii_case(n))
    }
}

/// How a stamp combines with the ground it covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SculptMode {
    /// `h += v * height`.
    #[default]
    Add = 0,
    /// `h = max(h, base + v * height)` where the stamp is non-zero.
    Max = 1,
    /// `h = min(h, base + v * height)` where the stamp is non-zero (carving with negative height).
    Min = 2,
    /// `h = base + v * height` over the whole stamp square (soft 8 % border).
    Replace = 3,
    /// Lift: `h = min(h + height · v', ceiling)` while `h < ceiling`.
    Lift = 4,
    /// Carve: `h = max(h - height · v', floor)` while `h > floor`.
    Carve = 5,
    /// Level: `h += 0.2005 · height · (level - h) · clamp(v', 0.0505, 1)`.
    Level = 6,
    /// Broad landmass stamp: absolute heights from `level` (value 0) rising by `height`
    /// (value 1) that only ever raise the ground (see [`broad_level`]).
    Broad = 7,
}

impl SculptMode {
    pub fn from_i64(v: i64) -> SculptMode {
        match v {
            1 => SculptMode::Max,
            2 => SculptMode::Min,
            3 => SculptMode::Replace,
            4 => SculptMode::Lift,
            5 => SculptMode::Carve,
            6 => SculptMode::Level,
            7 => SculptMode::Broad,
            _ => SculptMode::Add,
        }
    }

    /// The four sculpting modes with a level / floor / ceiling.
    pub fn is_sculpt(self) -> bool {
        matches!(self, SculptMode::Lift | SculptMode::Carve | SculptMode::Level | SculptMode::Broad)
    }
}

/// The alpha-brush combiners (`STAMP_MODES_ALL` adds the sculpting modes; the Stamp tool lists all).
pub const STAMP_MODES: [&str; 4] = ["Add", "Max", "Min", "Replace"];
/// Every mode, indexed by `SculptMode as usize`.
pub const STAMP_MODES_ALL: [&str; 8] = ["Add", "Max", "Min", "Replace", "Lift", "Carve", "Level", "Broad"];

/// Limits of the sculpting modes (`level`, `floor` and `ceiling` heights).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SculptLimits {
    /// Level target / broad height at value 0.
    pub level: f32,
    pub floor: f32,
    pub ceiling: f32,
}

impl SculptLimits {
    /// Level = the stamp's `base`, no floor or ceiling.
    pub fn open(p: &StampParams) -> SculptLimits {
        SculptLimits { level: p.base, floor: f32::MIN, ceiling: f32::MAX }
    }
}

/// The sculpting modes' contrast: lerp from flat mid-grey (`c = 0`) to the image (`c = 1`);
/// `c > 1` steepens. The editor passes `2 × currentContrast` (UI 0.5 = image as is).
pub fn sculpt_contrast(v: f32, c: f32) -> f32 {
    (1.0 - c) * 0.5 + c * v
}

/// Broad landmass branch: new absolute height for a texel value `v`, current height `cur`,
/// `min_at0` = the height at value 0 and `range` = the height gained at value 1.
/// Never returns less than `cur`; `None` = texel below the 0.0025 cut-off (no change).
pub fn broad_level(v: f32, contrast: f32, cur: f32, min_at0: f32, range: f32) -> Option<f32> {
    let v = if v < 0.0025 { 0.0 } else { v };
    if v <= 0.0 {
        return None;
    }
    let k = if v < 0.0505 { v / 0.1005 } else { 0.5 };
    let v = ((1.0 - contrast) * k + contrast * v).max(0.0);
    let fade = if v < 0.0105 { v / 0.0105 } else { 1.0 };
    let lvl = if v < 0.0055 { (v / 0.0055).clamp(0.0, 1.0) * min_at0 } else { (v - 0.0055) * range + min_at0 };
    let new = lvl * fade;
    Some(if min_at0 < new || min_at0 < cur { ((cur - min_at0).max(0.0) + new).max(cur) } else { new.max(cur) })
}

/// Placement of a stamp (terrain-local).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampParams {
    pub center: Vec2,
    /// Degrees around Y (same convention as [`crate::brush::Brush::rotation`]).
    pub rotation: f32,
    /// World extent along the stamp's own X and Z.
    pub size: Vec2,
    /// Height of a value of 1 (negative carves).
    pub height: f32,
    /// Gamma applied to the values: `> 1` sharper / thinner, `< 1` fuller.
    pub contrast: f32,
    pub mode: SculptMode,
    /// Height level for Max / Min / Replace (usually the terrain height at the centre).
    pub base: f32,
}

impl Default for StampParams {
    fn default() -> Self {
        StampParams { center: v2(0.0, 0.0), rotation: 0.0, size: v2(16.0, 16.0), height: 6.0, contrast: 1.0, mode: SculptMode::Add, base: 0.0 }
    }
}

impl StampParams {
    /// Terrain-local point → stamp UV (`0..1` inside).
    pub fn uv(&self, p: Vec2) -> Vec2 {
        let l = (p - self.center).rotated(-self.rotation.to_radians());
        v2(l.x / self.size.x.max(0.01) + 0.5, l.y / self.size.y.max(0.01) + 0.5)
    }

    /// Stamp UV → terrain-local point.
    pub fn point(&self, uv: Vec2) -> Vec2 {
        self.center + v2((uv.x - 0.5) * self.size.x, (uv.y - 0.5) * self.size.y).rotated(self.rotation.to_radians())
    }

    /// Corners of the stamp square (terrain-local), for gizmos and bounds.
    pub fn corners(&self) -> [Vec2; 4] {
        [self.point(v2(0.0, 0.0)), self.point(v2(1.0, 0.0)), self.point(v2(1.0, 1.0)), self.point(v2(0.0, 1.0))]
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        let c = self.corners();
        let min = c.iter().fold(v2(f32::MAX, f32::MAX), |a, p| v2(a.x.min(p.x), a.y.min(p.y)));
        let max = c.iter().fold(v2(f32::MIN, f32::MIN), |a, p| v2(a.x.max(p.x), a.y.max(p.y)));
        (min, max)
    }
}

impl Stamp {
    pub fn new(width: usize, height: usize, data: Vec<f32>) -> Stamp {
        assert_eq!(data.len(), width * height, "stamp data size");
        Stamp { width, height, data }
    }

    /// From 8-bit greyscale bytes (row-major).
    pub fn from_gray8(width: usize, height: usize, bytes: &[u8]) -> Option<Stamp> {
        (bytes.len() == width * height && width > 1 && height > 1).then(|| Stamp::new(width, height, bytes.iter().map(|b| *b as f32 / 255.0).collect()))
    }

    pub fn to_gray8(&self) -> Vec<u8> {
        self.data.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
    }

    fn at(&self, x: usize, y: usize) -> f32 {
        self.data[y.min(self.height - 1) * self.width + x.min(self.width - 1)]
    }

    /// Bilinear sample at UV; `None` outside `[0, 1]²`.
    pub fn sample(&self, uv: Vec2) -> Option<f32> {
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return None;
        }
        let fx = uv.x * (self.width - 1) as f32;
        let fy = uv.y * (self.height - 1) as f32;
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let a = self.at(x0, y0) + (self.at(x0 + 1, y0) - self.at(x0, y0)) * tx;
        let b = self.at(x0, y0 + 1) + (self.at(x0 + 1, y0 + 1) - self.at(x0, y0 + 1)) * tx;
        Some(a + (b - a) * ty)
    }

    /// Sculpting texel lookup: nearest texel, UV clamped to `[0, 0.9995]`;
    /// `None` outside the stamp rectangle.
    pub fn sample_nearest(&self, uv: Vec2) -> Option<f32> {
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return None;
        }
        let (u, v) = (uv.x.clamp(0.0, 0.9995), uv.y.clamp(0.0, 0.9995));
        Some(self.at((self.width as f32 * u) as usize, (self.height as f32 * v) as usize))
    }

    /// Sculpting modes: new height of a sample at `at` currently at `h`.
    pub fn apply_height_sculpt(&self, p: &StampParams, lim: &SculptLimits, at: Vec2, h: f32) -> f32 {
        let Some(v) = self.sample_nearest(p.uv(at)) else { return h };
        let c = p.contrast;
        match p.mode {
            SculptMode::Broad => match broad_level(v, c, h, lim.level, p.height) {
                Some(n) => n.min(lim.ceiling).max(lim.floor),
                None => h,
            },
            _ if v <= 0.0505 => h,
            SculptMode::Level => {
                let f = sculpt_contrast(v, c).clamp(0.0505, 1.0);
                // `rate01 · 0.2005 · rate_mult · (level - h) · f`; capped at 1 so a big one-shot
                // amount lands on the level instead of overshooting it.
                h + (0.2005 * p.height.abs() * f).min(1.0) * (lim.level - h)
            }
            SculptMode::Lift if h < lim.ceiling => (h + p.height.abs() * sculpt_contrast(v, c).max(0.0)).min(lim.ceiling),
            SculptMode::Carve if h > lim.floor => (h - p.height.abs() * sculpt_contrast(v, c).max(0.0)).max(lim.floor),
            _ => h,
        }
    }

    /// Shaped value (contrast applied) of the stamp at a terrain-local point.
    pub fn value_at(&self, p: &StampParams, at: Vec2) -> Option<f32> {
        let v = self.sample(p.uv(at))?;
        Some(v.clamp(0.0, 1.0).powf(p.contrast.clamp(0.05, 20.0)))
    }

    /// New height of a sample at `at` currently at `h` (unchanged outside the stamp).
    /// Sculpting modes use [`SculptLimits::open`].
    pub fn apply_height(&self, p: &StampParams, at: Vec2, h: f32) -> f32 {
        if p.mode.is_sculpt() {
            return self.apply_height_sculpt(p, &SculptLimits::open(p), at, h);
        }
        let Some(v) = self.value_at(p, at) else { return h };
        let target = p.base + v * p.height;
        match p.mode {
            SculptMode::Add => h + v * p.height,
            SculptMode::Max if v > 1e-4 => h.max(target),
            SculptMode::Min if v > 1e-4 => h.min(target),
            SculptMode::Replace => {
                let uv = p.uv(at);
                let edge = uv.x.min(1.0 - uv.x).min(uv.y).min(1.0 - uv.y);
                let w = (edge / 0.08).clamp(0.0, 1.0);
                let w = w * w * (3.0 - 2.0 * w);
                h + (target - h) * w
            }
            _ => h,
        }
    }

    /// Generates a built-in stamp of `size`² pixels.
    pub fn builtin(kind: BuiltinStamp, size: usize) -> Stamp {
        let n = size.max(8);
        let mut data = Vec::with_capacity(n * n);
        for y in 0..n {
            for x in 0..n {
                let u = v2(x as f32 / (n - 1) as f32 * 2.0 - 1.0, y as f32 / (n - 1) as f32 * 2.0 - 1.0);
                data.push(builtin_value(kind, u).clamp(0.0, 1.0));
            }
        }
        Stamp::new(n, n, data)
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Built-in stamp shapes over `u ∈ [-1, 1]²` (all fade to 0 before the square's edge).
fn builtin_value(kind: BuiltinStamp, u: Vec2) -> f32 {
    let r = u.length();
    let ang = u.y.atan2(u.x);
    match kind {
        BuiltinStamp::Mountain => {
            // Ridged peak: radial profile with angular ridges and a little fbm.
            let ridges = 0.12 * (ang * 5.0 + 1.3).sin() * r + 0.07 * (ang * 9.0).sin() * r;
            let rr = (r + ridges).max(0.0);
            let base = (1.0 - rr).max(0.0).powf(1.7);
            let n = fbm(u * 3.5, 11, 4) * 0.12 * (1.0 - r).max(0.0);
            (base + n) * (1.0 - smoothstep(0.85, 1.0, r))
        }
        BuiltinStamp::Crater => {
            let rim = 0.62 + 0.04 * (ang * 7.0).sin();
            let inside = 0.18 + 0.82 * (r / rim).clamp(0.0, 1.0).powf(3.0);
            let outside = (1.0 - (r - rim) / (0.95 - rim)).clamp(0.0, 1.0).powf(2.2);
            let v = if r < rim { inside } else { outside };
            v * (1.0 - smoothstep(0.9, 1.0, r))
        }
        BuiltinStamp::Mesa => {
            let edge = 0.5 + 0.08 * fbm(v2(ang.cos(), ang.sin()) * 2.0, 5, 3);
            let top = 1.0 - smoothstep(edge - 0.04, edge + 0.08, r);
            let apron = 0.22 * (1.0 - smoothstep(edge, 0.95, r));
            top.max(apron) * (1.0 - smoothstep(0.92, 1.0, r))
        }
        BuiltinStamp::Valley => {
            // A meandering channel along the stamp's Z axis, faded towards both ends.
            let centre = 0.25 * (u.y * 2.4).sin() + 0.08 * (u.y * 6.0 + 0.7).sin();
            let d = (u.x - centre).abs();
            let channel = (-(d / 0.22).powi(2)).exp();
            let ends = 1.0 - smoothstep(0.7, 1.0, u.y.abs());
            channel * ends * (1.0 - smoothstep(0.85, 1.0, u.x.abs()))
        }
        BuiltinStamp::Hill => {
            let t = (1.0 - r).max(0.0);
            t * t * (3.0 - 2.0 * t)
        }
    }
}

/// Applies a stamp to every sample under it. One call = one undo step for the editor.
pub fn op_stamp(store: &mut TerrainStore, stamp: &Stamp, p: &StampParams) -> Touched {
    op_stamp_limited(store, stamp, p, &SculptLimits::open(p))
}

/// [`op_stamp`] with an explicit level / floor / ceiling for the sculpting modes.
pub fn op_stamp_limited(store: &mut TerrainStore, stamp: &Stamp, p: &StampParams, lim: &SculptLimits) -> Touched {
    let (min, max) = p.bounds();
    let mut t = Touched::new();
    for s in store.config.samples_in_rect(min, max) {
        let Some(h) = store.height(s) else { continue };
        let at = store.config.sample_pos(s);
        let nh = if p.mode.is_sculpt() { stamp.apply_height_sculpt(p, lim, at, h) } else { stamp.apply_height(p, at, h) };
        if (nh - h).abs() > 1e-6 && store.set_height(s, nh) {
            for c in store.chunks_affected_by(s) {
                t.insert(c);
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::{GridConfig, GridType};

    #[test]
    fn builtins_fade_to_zero_at_the_border() {
        for k in BUILTIN_STAMPS {
            let s = Stamp::builtin(k, 64);
            for i in 0..64 {
                for (x, y) in [(i, 0), (i, 63), (0, i), (63, i)] {
                    assert!(s.data[y * 64 + x] < 0.02, "{k:?} border {x},{y} = {}", s.data[y * 64 + x]);
                }
            }
            let peak = s.data.iter().copied().fold(0.0, f32::max);
            assert!(peak > 0.7, "{k:?} peak {peak}");
        }
        let c = Stamp::builtin(BuiltinStamp::Crater, 65);
        assert!(c.data[32 * 65 + 32] < 0.3, "crater centre is a bowl");
    }

    #[test]
    fn uv_round_trip_and_rotation() {
        let p = StampParams { center: v2(10.0, 4.0), rotation: 60.0, size: v2(8.0, 4.0), ..Default::default() };
        for uv in [v2(0.0, 0.0), v2(0.3, 0.9), v2(1.0, 0.5)] {
            let back = p.uv(p.point(uv));
            assert!((back - uv).length() < 1e-4);
        }
        assert!((p.uv(p.center) - v2(0.5, 0.5)).length() < 1e-5);
    }

    #[test]
    fn modes_combine_heights() {
        let s = Stamp::new(2, 2, vec![1.0; 4]);
        let mut p = StampParams { size: v2(10.0, 10.0), height: 4.0, base: 1.0, ..Default::default() };
        assert_eq!(s.apply_height(&p, v2(0.0, 0.0), 2.0), 6.0);
        p.mode = SculptMode::Max;
        assert_eq!(s.apply_height(&p, v2(0.0, 0.0), 2.0), 5.0);
        assert_eq!(s.apply_height(&p, v2(0.0, 0.0), 9.0), 9.0);
        p.mode = SculptMode::Min;
        assert_eq!(s.apply_height(&p, v2(0.0, 0.0), 9.0), 5.0);
        p.mode = SculptMode::Replace;
        assert!((s.apply_height(&p, v2(0.0, 0.0), 9.0) - 5.0).abs() < 1e-5);
        assert_eq!(s.apply_height(&p, v2(50.0, 0.0), 9.0), 9.0, "outside");
    }

    #[test]
    fn lift_and_carve_respect_limits() {
        let s = Stamp::new(2, 2, vec![1.0; 4]);
        let mut p = StampParams { size: v2(10.0, 10.0), height: 2.0, contrast: 1.0, mode: SculptMode::Lift, ..Default::default() };
        let lim = SculptLimits { level: 0.0, floor: -1.0, ceiling: 3.0 };
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), 2.0);
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 2.5), 3.0, "stops at the ceiling");
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 4.0), 4.0, "above the ceiling: untouched");
        p.mode = SculptMode::Carve;
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), -1.0, "stops at the floor");
        // Contrast 0 = flat mid-grey: half the amount wherever the texel is above 0.0505.
        p.contrast = 0.0;
        p.mode = SculptMode::Lift;
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), 1.0);
        let faint = Stamp::new(2, 2, vec![0.04; 4]);
        assert_eq!(faint.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), 0.0, "0.0505 cut-off");
    }

    #[test]
    fn level_pulls_towards_the_target() {
        let s = Stamp::new(2, 2, vec![1.0; 4]);
        let p = StampParams { size: v2(10.0, 10.0), height: 1.0, contrast: 1.0, mode: SculptMode::Level, ..Default::default() };
        let lim = SculptLimits { level: 5.0, floor: f32::MIN, ceiling: f32::MAX };
        assert!((s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0) - 1.0025).abs() < 1e-6, "0.2005 · 1 · (5 - 0) · 1");
        assert!((s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 10.0) - 8.9975).abs() < 1e-6);
    }

    #[test]
    fn broad_mode_only_raises() {
        // Value 1 → min_at0 + (1 - 0.0055) · range.
        assert!((broad_level(1.0, 1.0, 0.0, 2.0, 10.0).unwrap() - 11.945).abs() < 1e-4);
        assert_eq!(broad_level(0.001, 1.0, 3.0, 2.0, 10.0), None, "below 0.0025 = no change");
        assert!(broad_level(0.5, 1.0, 50.0, 2.0, 10.0).unwrap() >= 50.0, "never lowers");
        let lo = broad_level(0.003, 1.0, 0.0, 2.0, 10.0).unwrap();
        assert!(lo > 0.0 && lo < 2.0, "coast ramp {lo}");
    }

    #[test]
    fn nearest_sampling_clamps_at_the_border() {
        let s = Stamp::new(2, 2, vec![0.0, 1.0, 0.25, 0.75]);
        assert_eq!(s.sample_nearest(v2(0.49, 0.0)), Some(0.0));
        assert_eq!(s.sample_nearest(v2(0.5, 0.0)), Some(1.0));
        assert_eq!(s.sample_nearest(v2(1.0, 1.0)), Some(0.75), "UV clamps to 0.9995");
        assert_eq!(s.sample_nearest(v2(1.01, 0.5)), None);
    }

    #[test]
    fn op_stamp_raises_a_hex_terrain() {
        let mut st = TerrainStore::new(GridConfig { grid: GridType::Hex, chunk_size: 8, cell_size: v2(2.0, 2.0), ..Default::default() });
        st.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        let stamp = Stamp::builtin(BuiltinStamp::Hill, 32);
        let p = StampParams { center: v2(8.0, 7.0), size: v2(12.0, 12.0), height: 5.0, ..Default::default() };
        let t = op_stamp(&mut st, &stamp, &p);
        assert!(t.contains(&iv2(0, 0)));
        let c = st.config.world_to_sample(v2(8.0, 7.0));
        assert!(st.height(c).unwrap() > 4.0);
        assert_eq!(st.height(st.config.global_of(iv2(0, 0), iv2(0, 0))), Some(0.0));
    }
}
