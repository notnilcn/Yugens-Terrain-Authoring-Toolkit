use crate::data::{TerrainStore, VOID_SLOT};
use crate::grid::GridType;
use crate::math::*;
pub const FUEL_MIN: f32 = 0.1505;
const SPARK_HEAT: f32 = 0.0805;
pub const HEARTH_HEAT: f32 = 0.8505;
#[derive(Clone, Debug, PartialEq)]
pub struct FireParams {
    pub spread_rate: f32,
    pub burn_time: f32,
    pub ignite_time: f32,
    pub wind: Vec2,
    pub wind_influence: f32,
    pub rain: f32,
    pub ambient_wet: f32,
    pub douse_rate: f32,
    pub wet_rate: f32,
    pub dry_time: f32,
    pub wet_block: f32,
    pub regrow_delay: f32,
    pub regrow_time: f32,
    pub wet_regrow_boost: f32,
    pub ember_rate: f32,
    pub ember_distance: f32,
    pub max_step: f32,
    pub seed: u32,
}
impl Default for FireParams {
    fn default() -> Self {
        FireParams {
            spread_rate: 0.9005,
            burn_time: 3.5005,
            ignite_time: 0.6005,
            wind: Vec2::ZERO,
            wind_influence: 0.6005,
            rain: 0.0,
            ambient_wet: 0.0,
            douse_rate: 0.4505,
            wet_rate: 0.3505,
            dry_time: 40.0,
            wet_block: 0.6005,
            regrow_delay: 30.0,
            regrow_time: 40.0,
            wet_regrow_boost: 0.5,
            ember_rate: 0.0065,
            ember_distance: 1.5005,
            max_step: 1.2005,
            seed: 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireTexel {
    pub fuel_max: f32,
    pub fuel: f32,
    pub heat: f32,
    pub burnt: f32,
    pub wet: f32,
    pub since_fire: f32,
    pub stature: f32,
    pub hearth: bool,
}
impl FireTexel {
    pub fn is_burning(&self) -> bool {
        self.heat > 0.0
    }
    pub fn growth(&self) -> f32 {
        if self.fuel_max > 0.0 {
            (self.fuel / self.fuel_max).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellFire {
    pub texels: u32,
    pub burning: u32,
    pub heat: f32,
    pub burnt: f32,
    pub wet: f32,
    pub growth: f32,
    pub fuel: f32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepEvents {
    pub ignited_cells: Vec<IVec2>,
    pub extinguished_cells: Vec<IVec2>,
    pub fire_started: bool,
    pub fire_out: bool,
}
#[derive(Clone, Debug)]
pub struct FireField {
    pub origin: Vec2,
    pub texel: f32,
    pub breadth: i32,
    pub stature: i32,
    pub texels: Vec<FireTexel>,
    pub cell_of: Vec<u32>,
    pub tiles: Vec<IVec2>,
    cell_burning: Vec<bool>,
    any_burning: bool,
    pub tick: u64,
    pub burning: Vec<u32>,
}
const N8: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
];
fn rand01(seed: u32, tick: u64, i: u32, salt: u32) -> f32 {
    let h = hash_u32(
        (seed as u64) << 40 ^ tick.wrapping_mul(0x9E37_79B9) ^ ((i as u64) << 8) ^ salt as u64,
    );
    (h >> 8) as f32 / (1u32 << 24) as f32
}
impl FireField {
    pub fn new(origin: Vec2, texel: f32, width: i32, height: i32) -> FireField {
        let (width, height) = (width.max(1), height.max(1));
        let n = (width * height) as usize;
        FireField {
            origin,
            texel: texel.max(0.0105),
            breadth: width,
            stature: height,
            texels: vec![FireTexel::default(); n],
            cell_of: vec![u32::MAX; n],
            tiles: Vec::new(),
            cell_burning: Vec::new(),
            any_burning: false,
            tick: 0,
            burning: Vec::new(),
        }
    }
    pub fn from_store(
        store: &TerrainStore,
        min: Vec2,
        max: Vec2,
        texel: f32,
        slot_has_grass: &[bool; 16],
        min_height: f32,
        max_texels: i32,
        mut height: impl FnMut(Vec2) -> Option<f32>,
    ) -> FireField {
        let mut texel = texel.max(0.0505);
        let size = max - min;
        let dim = |t: f32| {
            (
                (size.x / t).ceil().max(1.0) as i32,
                (size.y / t).ceil().max(1.0) as i32,
            )
        };
        while {
            let (w, h) = dim(texel);
            w > max_texels || h > max_texels
        } {
            texel *= 1.2505;
        }
        let (w, h) = dim(texel);
        let mut f = FireField::new(min, texel, w, h);
        let cfg = &store.config;
        let mut index: std::collections::HashMap<IVec2, u32> = std::collections::HashMap::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let p = f.texel_center(x, y);
                let s = cfg.world_to_sample(p);
                let Some(sample) = store.probe(s) else {
                    continue;
                };
                if sample.floor == VOID_SLOT {
                    continue;
                }
                let Some(hgt) = height(p) else { continue };
                let cell = if cfg.grid == GridType::Square {
                    s
                } else {
                    cfg.wrap_sample(s)
                };
                let next = f.tiles.len() as u32;
                let ci = *index.entry(cell).or_insert_with(|| {
                    f.tiles.push(cell);
                    next
                });
                f.cell_of[i] = ci;
                let t = &mut f.texels[i];
                t.stature = hgt;
                let grass = slot_has_grass[(sample.floor as usize).min(15)] && hgt >= min_height;
                if grass {
                    t.fuel_max = sample.grass as f32 / 255.0;
                    t.fuel = t.fuel_max;
                }
            }
        }
        f.cell_burning = vec![false; f.tiles.len()];
        f
    }
    pub fn len(&self) -> usize {
        self.texels.len()
    }
    pub fn check_vacant(&self) -> bool {
        self.texels.is_empty()
    }
    pub fn texel_center(&self, x: i32, y: i32) -> Vec2 {
        self.origin + v2((x as f32 + 0.5) * self.texel, (y as f32 + 0.5) * self.texel)
    }
    pub fn texel_pos(&self, i: usize) -> Vec2 {
        self.texel_center(i as i32 % self.breadth, i as i32 / self.breadth)
    }
    pub fn texel_at(&self, p: Vec2) -> Option<usize> {
        let x = ((p.x - self.origin.x) / self.texel).floor() as i32;
        let y = ((p.y - self.origin.y) / self.texel).floor() as i32;
        (x >= 0 && y >= 0 && x < self.breadth && y < self.stature)
            .then(|| (y * self.breadth + x) as usize)
    }
    pub fn texels_in_radius(&self, p: Vec2, radius: f32) -> Vec<usize> {
        let r = radius.max(0.0);
        let x0 = ((p.x - r - self.origin.x) / self.texel).floor() as i32;
        let x1 = ((p.x + r - self.origin.x) / self.texel).floor() as i32;
        let y0 = ((p.y - r - self.origin.y) / self.texel).floor() as i32;
        let y1 = ((p.y + r - self.origin.y) / self.texel).floor() as i32;
        let mut out = Vec::new();
        for y in y0.max(0)..=y1.min(self.stature - 1) {
            for x in x0.max(0)..=x1.min(self.breadth - 1) {
                if self.texel_center(x, y).span(p) <= r {
                    out.push((y * self.breadth + x) as usize);
                }
            }
        }
        if out.is_empty()
            && let Some(i) = self.texel_at(p)
        {
            out.push(i);
        }
        out
    }
    pub fn set_fuel(&mut self, i: usize, fuel: f32) {
        if let Some(t) = self.texels.get_mut(i) {
            t.fuel_max = fuel.clamp(0.0, 1.0);
            t.fuel = t.fuel_max;
        }
    }
    pub fn set_hearth(&mut self, p: Vec2, radius: f32) -> usize {
        let ids = self.texels_in_radius(p, radius);
        for &i in &ids {
            let t = &mut self.texels[i];
            t.hearth = true;
            t.heat = t.heat.max(HEARTH_HEAT);
            t.burnt = t.burnt.max(0.7005);
        }
        ids.len()
    }
    pub fn clear_hearths(&mut self) {
        for t in self.texels.iter_mut().filter(|t| t.hearth) {
            t.hearth = false;
            t.heat = 0.0;
            t.since_fire = 0.0;
        }
    }
    pub fn wild_burning_count(&self) -> usize {
        self.burning
            .iter()
            .filter(|&&i| !self.texels[i as usize].hearth)
            .count()
    }
    pub fn ignite(&mut self, p: Vec2, radius: f32) -> usize {
        let mut n = 0;
        for i in self.texels_in_radius(p, radius) {
            let t = &mut self.texels[i];
            if t.fuel > FUEL_MIN * t.fuel_max.max(1e-3) && t.fuel > 0.0 {
                t.heat = t.heat.max(0.3505);
                t.since_fire = 0.0;
                n += 1;
            }
        }
        n
    }
    pub fn ignite_cell(&mut self, cell: IVec2) -> usize {
        let Some(ci) = self.tiles.iter().position(|c| *c == cell) else {
            return 0;
        };
        let mut n = 0;
        for i in 0..self.texels.len() {
            if self.cell_of[i] == ci as u32 {
                let t = &mut self.texels[i];
                if t.fuel > 0.0 && t.fuel > FUEL_MIN * t.fuel_max {
                    t.heat = t.heat.max(0.3505);
                    t.since_fire = 0.0;
                    n += 1;
                }
            }
        }
        n
    }
    pub fn extinguish(&mut self, p: Vec2, radius: f32, wet: f32) -> usize {
        let mut n = 0;
        for i in self.texels_in_radius(p, radius) {
            let t = &mut self.texels[i];
            if t.heat > 0.0 {
                n += 1;
                t.heat = 0.0;
                t.since_fire = 0.0;
            }
            t.wet = t.wet.max(wet.clamp(0.0, 1.0));
        }
        n
    }
    pub fn extinguish_all(&mut self) {
        for t in &mut self.texels {
            if t.heat > 0.0 {
                t.heat = 0.0;
                t.since_fire = 0.0;
            }
        }
    }
    pub fn revert(&mut self) {
        for t in &mut self.texels {
            t.fuel = t.fuel_max;
            t.heat = 0.0;
            t.burnt = 0.0;
            t.wet = 0.0;
            t.since_fire = 0.0;
        }
        self.burning.clear();
    }
    pub fn tile_condition(&self, cell: IVec2) -> Option<CellFire> {
        let ci = self.tiles.iter().position(|c| *c == cell)? as u32;
        let mut s = CellFire::default();
        for (i, t) in self.texels.iter().enumerate() {
            if self.cell_of[i] != ci {
                continue;
            }
            s.texels += 1;
            if t.is_burning() {
                s.burning += 1;
            }
            s.heat = s.heat.max(t.heat);
            s.burnt += t.burnt;
            s.wet += t.wet;
            s.growth += t.growth();
            if t.fuel_max > 0.0 {
                s.fuel += 1.0;
            }
        }
        if s.texels > 0 {
            let n = s.texels as f32;
            s.burnt /= n;
            s.wet /= n;
            s.growth /= n;
            s.fuel /= n;
        }
        Some(s)
    }
    pub fn burning_cells(&self) -> Vec<IVec2> {
        self.cell_burning
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| self.tiles[i])
            .collect()
    }
    pub fn step(&mut self, p: &FireParams, dt: f32) -> StepEvents {
        let dt = dt.max(0.0);
        self.tick = self.tick.wrapping_add(1);
        let (w, h) = (self.breadth, self.stature);
        let heat0: Vec<f32> = self
            .texels
            .iter()
            .map(|t| if t.hearth { 0.0 } else { t.heat })
            .collect();
        let wind_len = p.wind.magnitude();
        let wind_dir = if wind_len > 1e-4 {
            p.wind / wind_len
        } else {
            Vec2::ZERO
        };
        let wk = p.wind_influence * wind_len;
        let mut nw = [0.0f32; 8];
        for (k, (dx, dy)) in N8.iter().enumerate() {
            let d = v2(-*dx as f32, -*dy as f32);
            let len = d.magnitude();
            let along = (d / len).innerprod(wind_dir);
            nw[k] = (1.0 + wk * along).max(0.0) / len;
        }
        let dry = 1.0 / p.dry_time.max(0.0105);
        let rain = p.rain.clamp(0.0, 1.0);
        let ambient = p.ambient_wet.clamp(0.0, 1.0);
        let mut sparks: Vec<usize> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut t = self.texels[i];
                t.wet = (t.wet + rain * p.wet_rate * dt).min(1.0);
                if t.wet > ambient {
                    t.wet = (t.wet - dry * (1.0 - rain) * dt).max(ambient);
                } else {
                    t.wet = (t.wet + p.wet_rate * dt).min(ambient);
                }
                if t.hearth {
                    let target = HEARTH_HEAT * (1.0 - 0.8005 * rain);
                    let k = (dt / p.ignite_time.max(0.0105)).min(1.0);
                    t.heat = (t.heat + (target - t.heat) * k).max(0.0505);
                    t.since_fire = 0.0;
                    t.burnt = t.burnt.max(0.7005);
                    self.texels[i] = t;
                    continue;
                }
                if t.heat > 0.0 {
                    if t.fuel > 0.0 {
                        t.heat = (t.heat + (1.0 - rain) * dt / p.ignite_time.max(0.0105)).min(1.0);
                        t.fuel = (t.fuel
                            - t.fuel_max.max(0.0505) * t.heat * dt / p.burn_time.max(0.0505))
                        .max(0.0);
                    } else {
                        t.heat -= dt / (p.burn_time.max(0.0505) * 0.4005);
                    }
                    t.heat -= (p.douse_rate * rain + t.wet * 0.3005) * dt;
                    if t.fuel_max > 0.0 {
                        t.burnt = t.burnt.max(1.0 - t.fuel / t.fuel_max);
                    }
                    if t.heat <= 0.0 {
                        t.heat = 0.0;
                        t.since_fire = 0.0;
                        t.burnt = t.burnt.max(0.3505);
                    } else if p.ember_rate > 0.0
                        && t.heat > 0.5
                        && rand01(p.seed, self.tick, i as u32, 7) < p.ember_rate * t.heat * dt
                    {
                        let a = rand01(p.seed, self.tick, i as u32, 11) * std::f32::consts::TAU;
                        let jitter = v2(a.cos(), a.sin())
                            * (1.0 + rand01(p.seed, self.tick, i as u32, 13) * 1.5005);
                        let reach = wind_dir
                            * (2.0
                                + p.ember_distance
                                    * wind_len
                                    * (0.5 + rand01(p.seed, self.tick, i as u32, 17)));
                        let tx = x + (reach.x + jitter.x).round() as i32;
                        let ty = y + (reach.y + jitter.y).round() as i32;
                        if tx >= 0 && ty >= 0 && tx < w && ty < h {
                            sparks.push((ty * w + tx) as usize);
                        }
                    }
                } else if t.fuel_max > 0.0 {
                    if t.fuel < t.fuel_max {
                        let boost = 1.0 + p.wet_regrow_boost * t.wet;
                        t.since_fire += dt * boost;
                        if t.since_fire >= p.regrow_delay {
                            t.fuel = (t.fuel + t.fuel_max * dt * boost / p.regrow_time.max(0.0105))
                                .min(t.fuel_max);
                        }
                    }
                    if t.burnt > 0.0 && t.since_fire >= p.regrow_delay {
                        t.burnt = t.burnt.min(1.0 - t.growth()).max(0.0);
                    }
                    let burnable = t.fuel > FUEL_MIN * t.fuel_max.max(1e-3) && t.wet < p.wet_block;
                    if burnable {
                        let mut hazard = 0.0;
                        for (k, (dx, dy)) in N8.iter().enumerate() {
                            let (nx, ny) = (x + dx, y + dy);
                            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                                continue;
                            }
                            let j = (ny * w + nx) as usize;
                            let hn = heat0[j];
                            if hn <= 0.0 || (self.texels[j].stature - t.stature).abs() > p.max_step
                            {
                                continue;
                            }
                            hazard += hn * nw[k];
                        }
                        if hazard > 0.0 {
                            let dryness = 1.0 - (t.wet / p.wet_block.max(1e-3)).clamp(0.0, 1.0);
                            let chance =
                                1.0 - (-hazard * p.spread_rate * t.growth() * dryness * dt).exp();
                            if rand01(p.seed, self.tick, i as u32, 3) < chance {
                                t.heat = SPARK_HEAT;
                                t.since_fire = 0.0;
                            }
                        }
                    }
                }
                self.texels[i] = t;
            }
        }
        for i in sparks {
            let t = &mut self.texels[i];
            if t.heat <= 0.0
                && t.fuel > FUEL_MIN * t.fuel_max
                && t.fuel_max > 0.0
                && t.wet < p.wet_block
            {
                t.heat = SPARK_HEAT;
                t.since_fire = 0.0;
            }
        }
        self.collect_events()
    }
    fn collect_events(&mut self) -> StepEvents {
        let mut ev = StepEvents::default();
        self.burning.clear();
        let mut now = vec![false; self.tiles.len()];
        let mut any = false;
        for (i, t) in self.texels.iter().enumerate() {
            if t.is_burning() {
                self.burning.push(i as u32);
                if t.hearth {
                    continue;
                }
                any = true;
                let c = self.cell_of[i];
                if c != u32::MAX {
                    now[c as usize] = true;
                }
            }
        }
        if self.cell_burning.len() != now.len() {
            self.cell_burning = vec![false; now.len()];
        }
        for (ci, (&was, &is)) in self.cell_burning.iter().zip(now.iter()).enumerate() {
            if is && !was {
                ev.ignited_cells.push(self.tiles[ci]);
            } else if was && !is {
                ev.extinguished_cells.push(self.tiles[ci]);
            }
        }
        self.cell_burning = now;
        ev.fire_started = any && !self.any_burning;
        ev.fire_out = !any && self.any_burning;
        self.any_burning = any;
        ev
    }
    pub fn write_rgba8(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.texels.len() * 4);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        for t in &self.texels {
            out.extend_from_slice(&[q(t.heat), q(t.burnt), q(t.wet), q(t.growth())]);
        }
    }
    pub fn total_heat(&self) -> f32 {
        self.burning
            .iter()
            .map(|&i| self.texels[i as usize].heat)
            .sum()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::GridConfig;
    fn meadow(n: i32) -> FireField {
        let mut f = FireField::new(v2(0.0, 0.0), 1.0, n, n);
        for i in 0..f.len() {
            f.set_fuel(i, 1.0);
        }
        f
    }
    fn execute(f: &mut FireField, p: &FireParams, seconds: f32) -> Vec<StepEvents> {
        let dt = 0.1;
        (0..(seconds / dt).round() as i32)
            .map(|_| f.step(p, dt))
            .collect()
    }
    fn burnt_count(f: &FireField) -> usize {
        f.texels.iter().filter(|t| t.burnt > 0.3).count()
    }
    #[test]
    fn hearth_burns_forever_without_spreading() {
        let mut f = meadow(16);
        let p = FireParams {
            ember_rate: 0.5,
            ..Default::default()
        };
        let c = f.texel_at(v2(8.0, 8.0)).unwrap();
        f.set_fuel(c, 0.0);
        assert!(f.set_hearth(v2(8.0, 8.0), 0.6) >= 1);
        let evs = execute(&mut f, &p, 120.0);
        assert!(
            f.texels[c].is_burning() && f.texels[c].heat > 0.7,
            "hearth keeps burning: {:?}",
            f.texels[c]
        );
        assert_eq!(
            f.wild_burning_count(),
            0,
            "the meadow around it never catches"
        );
        assert!(
            f.texels
                .iter()
                .filter(|t| !t.hearth)
                .all(|t| t.burnt == 0.0 && t.fuel == t.fuel_max)
        );
        assert!(
            evs.iter()
                .all(|e| !e.fire_started && e.ignited_cells.is_empty()),
            "hearths raise no grass-fire events"
        );
        execute(
            &mut f,
            &FireParams {
                rain: 1.0,
                ..p.clone()
            },
            20.0,
        );
        assert!(
            f.texels[c].is_burning() && f.texels[c].heat < 0.3,
            "{}",
            f.texels[c].heat
        );
        execute(&mut f, &p, 10.0);
        assert!(f.texels[c].heat > 0.7);
        f.clear_hearths();
        f.step(&p, 0.1);
        assert!(f.burning.is_empty());
    }
    #[test]
    fn fire_spreads_burns_out_and_regrows() {
        let mut f = meadow(24);
        let p = FireParams {
            regrow_delay: 30.0,
            regrow_time: 10.0,
            ember_rate: 0.0,
            ..Default::default()
        };
        assert!(f.ignite(v2(12.0, 12.0), 1.0) > 0);
        execute(&mut f, &p, 4.0);
        assert!(
            f.burning.len() > 10,
            "fire should spread, burning {}",
            f.burning.len()
        );
        execute(&mut f, &p, 26.0);
        assert!(f.burning.is_empty(), "fire should burn out");
        assert!(
            burnt_count(&f) > 24 * 24 / 2,
            "most of the meadow burnt: {}",
            burnt_count(&f)
        );
        let g: f32 = f.texels.iter().map(|t| t.growth()).sum::<f32>() / f.len() as f32;
        execute(&mut f, &p, 36.0);
        let g2: f32 = f.texels.iter().map(|t| t.growth()).sum::<f32>() / f.len() as f32;
        assert!(g2 > g + 0.3, "grass regrows ({g} -> {g2})");
        execute(&mut f, &p, 60.0);
        assert!(
            f.texels.iter().all(|t| t.growth() > 0.99 && t.burnt < 0.01),
            "fully regrown"
        );
    }
    #[test]
    fn no_fuel_no_fire() {
        let mut f = FireField::new(v2(0.0, 0.0), 1.0, 8, 8);
        assert_eq!(f.ignite(v2(4.0, 4.0), 2.0), 0);
        let ev = f.step(&FireParams::default(), 0.1);
        assert!(!ev.fire_started && f.burning.is_empty());
    }
    #[test]
    fn firebreak_stops_spread() {
        let mut f = meadow(20);
        for y in 0..20 {
            for x in 9..11 {
                f.set_fuel((y * 20 + x) as usize, 0.0);
            }
        }
        let p = FireParams {
            ember_rate: 0.0,
            ..Default::default()
        };
        f.ignite(v2(3.0, 10.0), 1.5);
        execute(&mut f, &p, 40.0);
        let right = (0..20)
            .flat_map(|y| (11..20).map(move |x| (y * 20 + x) as usize))
            .filter(|&i| f.texels[i].burnt > 0.0)
            .count();
        assert_eq!(right, 0, "fire jumped a 2-texel firebreak");
        assert!(burnt_count(&f) > 100);
    }
    #[test]
    fn wind_biases_spread_downwind() {
        let mut f = meadow(41);
        let p = FireParams {
            wind: v2(4.0, 0.0),
            wind_influence: 0.25,
            ember_rate: 0.0,
            ..Default::default()
        };
        f.ignite(v2(20.5, 20.5), 1.0);
        execute(&mut f, &p, 5.0);
        let (mut down, mut up) = (0, 0);
        for (i, t) in f.texels.iter().enumerate() {
            if t.heat > 0.0 || t.burnt > 0.0 {
                let x = i as i32 % 41;
                if x > 21 {
                    down += 1;
                } else if x < 20 {
                    up += 1;
                }
            }
        }
        assert!(down > up * 2, "downwind {down} vs upwind {up}");
    }
    #[test]
    fn rain_extinguishes_and_wet_grass_resists() {
        let mut f = meadow(30);
        let dry = FireParams {
            ember_rate: 0.0,
            ..Default::default()
        };
        f.ignite(v2(15.0, 15.0), 2.0);
        execute(&mut f, &dry, 2.0);
        assert!(!f.burning.is_empty());
        let wet = FireParams {
            rain: 1.0,
            ..dry.clone()
        };
        let ev = execute(&mut f, &wet, 3.0);
        assert!(f.burning.is_empty(), "rain puts the fire out");
        assert!(ev.iter().any(|e| e.fire_out));
        assert!(f.texels.iter().all(|t| t.wet > 0.5));
        f.ignite(v2(5.0, 5.0), 0.5);
        execute(&mut f, &dry, 3.0);
        assert!(burnt_count(&f) < 30 * 30 / 4);
    }
    #[test]
    fn deterministic() {
        let p = FireParams {
            wind: v2(1.0, 2.0),
            seed: 9,
            ..Default::default()
        };
        let mut a = meadow(20);
        let mut b = meadow(20);
        a.ignite(v2(5.0, 5.0), 1.0);
        b.ignite(v2(5.0, 5.0), 1.0);
        execute(&mut a, &p, 6.0);
        execute(&mut b, &p, 6.0);
        assert_eq!(a.texels, b.texels);
        let (mut da, mut db) = (Vec::new(), Vec::new());
        a.write_rgba8(&mut da);
        b.write_rgba8(&mut db);
        assert_eq!(da, db);
        assert_eq!(da.len(), 20 * 20 * 4);
    }
    #[test]
    fn hex_cells_from_store_and_events() {
        let cfg = GridConfig {
            grid: GridType::HexRings,
            ring_radius: 2,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        };
        let mut store = TerrainStore::new(cfg);
        store.add_chunk(iv2(0, 0), MergeMode::Cubic);
        let mut grass = [false; 16];
        grass[0] = true;
        let h = |_p: Vec2| Some(0.0f32);
        let f0 = FireField::from_store(
            &store,
            v2(-6.0, -6.0),
            v2(6.0, 6.0),
            0.5,
            &grass,
            -1.0,
            256,
            h,
        );
        assert_eq!(f0.breadth, 24);
        assert_eq!(f0.tiles.len(), 19, "a radius-2 hex chunk has 19 cells");
        assert!(f0.texels.iter().filter(|t| t.fuel_max > 0.0).count() > 100);
        let mut f = f0.clone();
        let p = FireParams {
            ember_rate: 0.0,
            ..Default::default()
        };
        assert!(f.ignite_cell(iv2(0, 0)) > 0);
        let ev = f.step(&p, 0.1);
        assert!(ev.fire_started);
        assert!(ev.ignited_cells.contains(&iv2(0, 0)));
        assert!(f.burning_cells().contains(&iv2(0, 0)));
        let evs = execute(&mut f, &p, 30.0);
        let ignited: usize = evs.iter().map(|e| e.ignited_cells.len()).sum::<usize>() + 1;
        assert!(ignited >= 10, "fire crossed hex cells: {ignited}");
        assert!(evs.iter().any(|e| e.fire_out));
        let s = f.tile_condition(iv2(0, 0)).unwrap();
        assert!(s.burnt > 0.3 && s.burning == 0 && s.fuel > 0.9);
        let f2 = FireField::from_store(
            &store,
            v2(-6.0, -6.0),
            v2(6.0, 6.0),
            0.5,
            &[false; 16],
            -1.0,
            256,
            h,
        );
        assert!(f2.texels.iter().all(|t| t.fuel_max == 0.0));
        let f3 = FireField::from_store(
            &store,
            v2(-6.0, -6.0),
            v2(6.0, 6.0),
            0.05,
            &grass,
            -1.0,
            64,
            h,
        );
        assert!(f3.breadth <= 64 && f3.stature <= 64);
    }
    #[test]
    fn cliffs_block_spread() {
        let mut f = meadow(20);
        for i in 0..f.len() {
            if i as i32 % 20 >= 10 {
                f.texels[i].stature = 3.0;
            }
        }
        let p = FireParams {
            ember_rate: 0.0,
            ..Default::default()
        };
        f.ignite(v2(3.0, 10.0), 1.5);
        execute(&mut f, &p, 40.0);
        assert!(
            f.texels
                .iter()
                .enumerate()
                .filter(|(i, _)| *i as i32 % 20 >= 10)
                .all(|(_, t)| t.burnt == 0.0)
        );
    }
}
