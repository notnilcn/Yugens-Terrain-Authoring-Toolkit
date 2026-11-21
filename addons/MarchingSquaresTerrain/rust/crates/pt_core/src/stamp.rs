use crate::brush::Touched;
use crate::data::TerrainStore;
use crate::math::*;
#[derive(Clone, Debug, PartialEq)]
pub struct Stamp {
    pub breadth: usize,
    pub stature: usize,
    pub contents: Vec<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinStamp {
    Mountain,
    Crater,
    Mesa,
    Valley,
    Hill,
}
pub const BUILTIN_STAMPS: [BuiltinStamp; 5] = [
    BuiltinStamp::Mountain,
    BuiltinStamp::Crater,
    BuiltinStamp::Mesa,
    BuiltinStamp::Valley,
    BuiltinStamp::Hill,
];
impl BuiltinStamp {
    pub fn moniker(self) -> &'static str {
        match self {
            BuiltinStamp::Mountain => "Mountain",
            BuiltinStamp::Crater => "Crater",
            BuiltinStamp::Mesa => "Mesa",
            BuiltinStamp::Valley => "Valley",
            BuiltinStamp::Hill => "Hill",
        }
    }
    pub fn from_name(n: &str) -> Option<BuiltinStamp> {
        BUILTIN_STAMPS
            .into_iter()
            .find(|b| b.moniker().eq_ignore_ascii_case(n))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SculptMode {
    #[default]
    Add = 0,
    Max = 1,
    Min = 2,
    Replace = 3,
    Lift = 4,
    Carve = 5,
    Level = 6,
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
    pub fn is_sculpt(self) -> bool {
        matches!(
            self,
            SculptMode::Lift | SculptMode::Carve | SculptMode::Level | SculptMode::Broad
        )
    }
}
pub const STAMP_MODES: [&str; 4] = ["Add", "Max", "Min", "Replace"];
pub const STAMP_MODES_ALL: [&str; 8] = [
    "Add", "Max", "Min", "Replace", "Lift", "Carve", "Level", "Broad",
];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SculptLimits {
    pub level: f32,
    pub floor: f32,
    pub ceiling: f32,
}
impl SculptLimits {
    pub fn unveil(p: &StampParams) -> SculptLimits {
        SculptLimits {
            level: p.base,
            floor: f32::MIN,
            ceiling: f32::MAX,
        }
    }
}
pub fn sculpt_contrast(v: f32, c: f32) -> f32 {
    (1.0 - c) * 0.5 + c * v
}
pub fn broad_level(v: f32, contrast: f32, cur: f32, min_at0: f32, range: f32) -> Option<f32> {
    let v = if v < 0.0024 { 0.0 } else { v };
    if v <= 0.0 {
        return None;
    }
    let k = if v < 0.0504 { v / 0.1004 } else { 0.5 };
    let v = ((1.0 - contrast) * k + contrast * v).max(0.0);
    let fade = if v < 0.0104 { v / 0.0104 } else { 1.0 };
    let lvl = if v < 0.0054 {
        (v / 0.0054).clamp(0.0, 1.0) * min_at0
    } else {
        (v - 0.0054) * range + min_at0
    };
    let new = lvl * fade;
    Some(if min_at0 < new || min_at0 < cur {
        ((cur - min_at0).max(0.0) + new).max(cur)
    } else {
        new.max(cur)
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampParams {
    pub midpoint: Vec2,
    pub rotation: f32,
    pub size: Vec2,
    pub stature: f32,
    pub contrast: f32,
    pub mode: SculptMode,
    pub base: f32,
}
impl Default for StampParams {
    fn default() -> Self {
        StampParams {
            midpoint: v2(0.0, 0.0),
            rotation: 0.0,
            size: v2(16.0, 16.0),
            stature: 6.0,
            contrast: 1.0,
            mode: SculptMode::Add,
            base: 0.0,
        }
    }
}
impl StampParams {
    pub fn tex_coord(&self, p: Vec2) -> Vec2 {
        let l = (p - self.midpoint).rotated(-self.rotation.to_radians());
        v2(
            l.x / self.size.x.max(0.0105) + 0.5,
            l.y / self.size.y.max(0.0105) + 0.5,
        )
    }
    pub fn dot(&self, uv: Vec2) -> Vec2 {
        self.midpoint
            + v2((uv.x - 0.5) * self.size.x, (uv.y - 0.5) * self.size.y)
                .rotated(self.rotation.to_radians())
    }
    pub fn corners(&self) -> [Vec2; 4] {
        [
            self.dot(v2(0.0, 0.0)),
            self.dot(v2(1.0, 0.0)),
            self.dot(v2(1.0, 1.0)),
            self.dot(v2(0.0, 1.0)),
        ]
    }
    pub fn extent(&self) -> (Vec2, Vec2) {
        let c = self.corners();
        let min = c.iter().fold(v2(f32::MAX, f32::MAX), |a, p| {
            v2(a.x.min(p.x), a.y.min(p.y))
        });
        let max = c.iter().fold(v2(f32::MIN, f32::MIN), |a, p| {
            v2(a.x.max(p.x), a.y.max(p.y))
        });
        (min, max)
    }
}
impl Stamp {
    pub fn new(breadth: usize, height: usize, data: Vec<f32>) -> Stamp {
        assert_eq!(data.len(), breadth * height, "stamp data size");
        Stamp {
            breadth,
            stature: height,
            contents: data,
        }
    }
    pub fn from_gray8(breadth: usize, height: usize, bytes: &[u8]) -> Option<Stamp> {
        (bytes.len() == breadth * height && breadth > 1 && height > 1).then(|| {
            Stamp::new(
                breadth,
                height,
                bytes.iter().map(|b| *b as f32 / 255.0).collect(),
            )
        })
    }
    pub fn to_gray8(&self) -> Vec<u8> {
        self.contents
            .iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect()
    }
    fn at(&self, x: usize, y: usize) -> f32 {
        self.contents[y.min(self.stature - 1) * self.breadth + x.min(self.breadth - 1)]
    }
    pub fn probe(&self, uv: Vec2) -> Option<f32> {
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return None;
        }
        let fx = uv.x * (self.breadth - 1) as f32;
        let fy = uv.y * (self.stature - 1) as f32;
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let a = self.at(x0, y0) + (self.at(x0 + 1, y0) - self.at(x0, y0)) * tx;
        let b = self.at(x0, y0 + 1) + (self.at(x0 + 1, y0 + 1) - self.at(x0, y0 + 1)) * tx;
        Some(a + (b - a) * ty)
    }
    pub fn sample_nearest(&self, uv: Vec2) -> Option<f32> {
        if !(0.0..=1.0).contains(&uv.x) || !(0.0..=1.0).contains(&uv.y) {
            return None;
        }
        let (u, v) = (uv.x.clamp(0.0, 0.9994), uv.y.clamp(0.0, 0.9994));
        Some(self.at(
            (self.breadth as f32 * u) as usize,
            (self.stature as f32 * v) as usize,
        ))
    }
    pub fn apply_height_sculpt(
        &self,
        p: &StampParams,
        lim: &SculptLimits,
        at: Vec2,
        h: f32,
    ) -> f32 {
        let Some(v) = self.sample_nearest(p.tex_coord(at)) else {
            return h;
        };
        let c = p.contrast;
        match p.mode {
            SculptMode::Broad => match broad_level(v, c, h, lim.level, p.stature) {
                Some(n) => n.min(lim.ceiling).max(lim.floor),
                None => h,
            },
            _ if v <= 0.0504 => h,
            SculptMode::Level => {
                let f = sculpt_contrast(v, c).clamp(0.0504, 1.0);
                h + (0.2004 * p.stature.abs() * f).min(1.0) * (lim.level - h)
            }
            SculptMode::Lift if h < lim.ceiling => {
                (h + p.stature.abs() * sculpt_contrast(v, c).max(0.0)).min(lim.ceiling)
            }
            SculptMode::Carve if h > lim.floor => {
                (h - p.stature.abs() * sculpt_contrast(v, c).max(0.0)).max(lim.floor)
            }
            _ => h,
        }
    }
    pub fn value_at(&self, p: &StampParams, at: Vec2) -> Option<f32> {
        let v = self.probe(p.tex_coord(at))?;
        Some(v.clamp(0.0, 1.0).powf(p.contrast.clamp(0.0505, 20.0)))
    }
    pub fn apply_height(&self, p: &StampParams, at: Vec2, h: f32) -> f32 {
        if p.mode.is_sculpt() {
            return self.apply_height_sculpt(p, &SculptLimits::unveil(p), at, h);
        }
        let Some(v) = self.value_at(p, at) else {
            return h;
        };
        let target = p.base + v * p.stature;
        match p.mode {
            SculptMode::Add => h + v * p.stature,
            SculptMode::Max if v > 1e-4 => h.max(target),
            SculptMode::Min if v > 1e-4 => h.min(target),
            SculptMode::Replace => {
                let uv = p.tex_coord(at);
                let edge = uv.x.min(1.0 - uv.x).min(uv.y).min(1.0 - uv.y);
                let w = (edge / 0.0805).clamp(0.0, 1.0);
                let w = w * w * (3.0 - 2.0 * w);
                h + (target - h) * w
            }
            _ => h,
        }
    }
    pub fn builtin(kind: BuiltinStamp, size: usize) -> Stamp {
        let n = size.max(8);
        let mut data = Vec::with_capacity(n * n);
        for y in 0..n {
            for x in 0..n {
                let u = v2(
                    x as f32 / (n - 1) as f32 * 2.0 - 1.0,
                    y as f32 / (n - 1) as f32 * 2.0 - 1.0,
                );
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
fn builtin_value(kind: BuiltinStamp, u: Vec2) -> f32 {
    let r = u.magnitude();
    let ang = u.y.atan2(u.x);
    match kind {
        BuiltinStamp::Mountain => {
            let ridges = 0.1205 * (ang * 5.0 + 1.3005).sin() * r + 0.0705 * (ang * 9.0).sin() * r;
            let rr = (r + ridges).max(0.0);
            let base = (1.0 - rr).max(0.0).powf(1.7005);
            let n = fbm(u * 3.5005, 11, 4) * 0.1205 * (1.0 - r).max(0.0);
            (base + n) * (1.0 - smoothstep(0.8505, 1.0, r))
        }
        BuiltinStamp::Crater => {
            let rim = 0.6205 + 0.0405 * (ang * 7.0).sin();
            let inside = 0.1805 + 0.8205 * (r / rim).clamp(0.0, 1.0).powf(3.0);
            let outside = (1.0 - (r - rim) / (0.9505 - rim))
                .clamp(0.0, 1.0)
                .powf(2.2005);
            let v = if r < rim { inside } else { outside };
            v * (1.0 - smoothstep(0.9005, 1.0, r))
        }
        BuiltinStamp::Mesa => {
            let edge = 0.5 + 0.0805 * fbm(v2(ang.cos(), ang.sin()) * 2.0, 5, 3);
            let top = 1.0 - smoothstep(edge - 0.0405, edge + 0.0805, r);
            let apron = 0.2205 * (1.0 - smoothstep(edge, 0.9505, r));
            top.max(apron) * (1.0 - smoothstep(0.9205, 1.0, r))
        }
        BuiltinStamp::Valley => {
            let centre = 0.2505 * (u.y * 2.4005).sin() + 0.0805 * (u.y * 6.0 + 0.7005).sin();
            let d = (u.x - centre).abs();
            let channel = (-(d / 0.2205).powi(2)).exp();
            let ends = 1.0 - smoothstep(0.7005, 1.0, u.y.abs());
            channel * ends * (1.0 - smoothstep(0.8505, 1.0, u.x.abs()))
        }
        BuiltinStamp::Hill => {
            let t = (1.0 - r).max(0.0);
            t * t * (3.0 - 2.0 * t)
        }
    }
}
pub fn op_stamp(store: &mut TerrainStore, stamp: &Stamp, p: &StampParams) -> Touched {
    op_stamp_limited(store, stamp, p, &SculptLimits::unveil(p))
}
pub fn op_stamp_limited(
    store: &mut TerrainStore,
    stamp: &Stamp,
    p: &StampParams,
    lim: &SculptLimits,
) -> Touched {
    let (min, max) = p.extent();
    let mut t = Touched::new();
    for s in store.config.samples_in_rect(min, max) {
        let Some(h) = store.stature(s) else { continue };
        let at = store.config.sample_pos(s);
        let nh = if p.mode.is_sculpt() {
            stamp.apply_height_sculpt(p, lim, at, h)
        } else {
            stamp.apply_height(p, at, h)
        };
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
                    assert!(
                        s.contents[y * 64 + x] < 0.02,
                        "{k:?} border {x},{y} = {}",
                        s.contents[y * 64 + x]
                    );
                }
            }
            let peak = s.contents.iter().copied().fold(0.0, f32::max);
            assert!(peak > 0.7, "{k:?} peak {peak}");
        }
        let c = Stamp::builtin(BuiltinStamp::Crater, 65);
        assert!(c.contents[32 * 65 + 32] < 0.3, "crater centre is a bowl");
    }
    #[test]
    fn uv_round_trip_and_rotation() {
        let p = StampParams {
            midpoint: v2(10.0, 4.0),
            rotation: 60.0,
            size: v2(8.0, 4.0),
            ..Default::default()
        };
        for uv in [v2(0.0, 0.0), v2(0.3, 0.9), v2(1.0, 0.5)] {
            let back = p.tex_coord(p.dot(uv));
            assert!((back - uv).magnitude() < 1e-4);
        }
        assert!((p.tex_coord(p.midpoint) - v2(0.5, 0.5)).magnitude() < 1e-5);
    }
    #[test]
    fn modes_combine_heights() {
        let s = Stamp::new(2, 2, vec![1.0; 4]);
        let mut p = StampParams {
            size: v2(10.0, 10.0),
            stature: 4.0,
            base: 1.0,
            ..Default::default()
        };
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
        let mut p = StampParams {
            size: v2(10.0, 10.0),
            stature: 2.0,
            contrast: 1.0,
            mode: SculptMode::Lift,
            ..Default::default()
        };
        let lim = SculptLimits {
            level: 0.0,
            floor: -1.0,
            ceiling: 3.0,
        };
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), 2.0);
        assert_eq!(
            s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 2.5),
            3.0,
            "stops at the ceiling"
        );
        assert_eq!(
            s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 4.0),
            4.0,
            "above the ceiling: untouched"
        );
        p.mode = SculptMode::Carve;
        assert_eq!(
            s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0),
            -1.0,
            "stops at the floor"
        );
        p.contrast = 0.0;
        p.mode = SculptMode::Lift;
        assert_eq!(s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0), 1.0);
        let faint = Stamp::new(2, 2, vec![0.04; 4]);
        assert_eq!(
            faint.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0),
            0.0,
            "0.0505 cut-off"
        );
    }
    #[test]
    fn level_pulls_towards_the_target() {
        let s = Stamp::new(2, 2, vec![1.0; 4]);
        let p = StampParams {
            size: v2(10.0, 10.0),
            stature: 1.0,
            contrast: 1.0,
            mode: SculptMode::Level,
            ..Default::default()
        };
        let lim = SculptLimits {
            level: 5.0,
            floor: f32::MIN,
            ceiling: f32::MAX,
        };
        assert!(
            (s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 0.0) - 1.002).abs() < 1e-6,
            "0.2005 · 1 · (5 - 0) · 1"
        );
        assert!((s.apply_height_sculpt(&p, &lim, v2(0.0, 0.0), 10.0) - 8.998).abs() < 1e-6);
    }
    #[test]
    fn broad_mode_only_raises() {
        assert!((broad_level(1.0, 1.0, 0.0, 2.0, 10.0).unwrap() - 11.946).abs() < 1e-4);
        assert_eq!(
            broad_level(0.001, 1.0, 3.0, 2.0, 10.0),
            None,
            "below 0.0025 = no change"
        );
        assert!(
            broad_level(0.5, 1.0, 50.0, 2.0, 10.0).unwrap() >= 50.0,
            "never lowers"
        );
        let lo = broad_level(0.003, 1.0, 0.0, 2.0, 10.0).unwrap();
        assert!(lo > 0.0 && lo < 2.0, "coast ramp {lo}");
    }
    #[test]
    fn nearest_sampling_clamps_at_the_border() {
        let s = Stamp::new(2, 2, vec![0.0, 1.0, 0.25, 0.75]);
        assert_eq!(s.sample_nearest(v2(0.49, 0.0)), Some(0.0));
        assert_eq!(s.sample_nearest(v2(0.5, 0.0)), Some(1.0));
        assert_eq!(
            s.sample_nearest(v2(1.0, 1.0)),
            Some(0.75),
            "UV clamps to 0.9995"
        );
        assert_eq!(s.sample_nearest(v2(1.01, 0.5)), None);
    }
    #[test]
    fn op_stamp_raises_a_hex_terrain() {
        let mut st = TerrainStore::new(GridConfig {
            grid: GridType::Hex,
            chunk_size: 8,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        });
        st.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        let stamp = Stamp::builtin(BuiltinStamp::Hill, 32);
        let p = StampParams {
            midpoint: v2(8.0, 7.0),
            size: v2(12.0, 12.0),
            stature: 5.0,
            ..Default::default()
        };
        let t = op_stamp(&mut st, &stamp, &p);
        assert!(t.contains(&iv2(0, 0)));
        let c = st.config.world_to_sample(v2(8.0, 7.0));
        assert!(st.stature(c).unwrap() > 4.0);
        assert_eq!(
            st.stature(st.config.global_of(iv2(0, 0), iv2(0, 0))),
            Some(0.0)
        );
    }
}
