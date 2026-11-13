//! Grass fire / rain / regrowth simulation on a world-aligned texel lattice.
//!
//! The field covers a terrain with square texels (`texel` metres, terrain-local XZ), so it
//! works the same on every grid topology; texels remember which terrain cell they lie in
//! ([`FireField::from_store`]) so games can query and react per cell (hex first).
//!
//! Each texel holds grass fuel, heat (burning), char (burnt), wetness and a regrowth timer:
//!
//! * **Ignition / spread**: an unburnt texel catches fire with a hazard proportional to the
//!   heat of its 8 neighbours, weighted towards the wind (downwind spread is fast, upwind
//!   creeps) and scaled down by its own wetness; cliffs taller than `max_step` stop the front.
//!   Hot texels throw embers that start spot fires downwind.
//! * **Burning**: heat ramps up while the fuel burns away; the texel chars as fuel is
//!   consumed, then smoulders out. Rain and wetness douse heat.
//! * **Rain**: wets texels (up to the weather's ambient ground wetness and beyond while it
//!   rains); wet grass does not ignite and burnt ground regrows faster.
//! * **Regrowth**: after `regrow_delay` seconds without fire, fuel grows back and the char fades.
//!
//! Stepping is deterministic: the same seed, parameters, commands and step count always give
//! the same field (the random draws hash the seed, tick and texel index).

use crate::data::{TerrainStore, VOID_SLOT};
use crate::grid::GridType;
use crate::math::*;

/// Texels with less fuel than this never burn (bare ground, freshly burnt grass).
pub const FUEL_MIN: f32 = 0.15;
/// Heat a new spot fire starts with.
const SPARK_HEAT: f32 = 0.08;

#[derive(Clone, Debug, PartialEq)]
pub struct FireParams {
    /// Ignition hazard (per second) from one fully burning edge neighbour without wind.
    pub spread_rate: f32,
    /// Seconds a full-fuel texel burns at full heat.
    pub burn_time: f32,
    /// Seconds a fresh fire takes to reach full heat.
    pub ignite_time: f32,
    /// Wind in m/s (terrain-local XZ).
    pub wind: Vec2,
    /// How strongly the wind skews spread (per m/s): downwind weight `1 + k·|w|`, upwind
    /// weight `max(0, 1 - k·|w|)`.
    pub wind_influence: f32,
    /// Rain intensity 0..1 right now.
    pub rain: f32,
    /// Ground wetness the texels settle at (PixelWeather's wetness), 0..1.
    pub ambient_wet: f32,
    /// Heat lost per second under full rain.
    pub douse_rate: f32,
    /// Wetness gained per second under full rain.
    pub wet_rate: f32,
    /// Seconds a soaked texel takes to dry out (no rain).
    pub dry_time: f32,
    /// Wetness at which grass no longer ignites (spread chance falls linearly towards it).
    pub wet_block: f32,
    /// Seconds after the fire is out before burnt grass starts regrowing.
    pub regrow_delay: f32,
    /// Seconds burnt grass takes to grow back fully.
    pub regrow_time: f32,
    /// Regrowth speed-up at full wetness (`1 + boost · wet`).
    pub wet_regrow_boost: f32,
    /// Spot fires per second thrown by one fully burning texel.
    pub ember_rate: f32,
    /// How far embers fly downwind (texels per m/s of wind, plus a little random scatter).
    pub ember_distance: f32,
    /// Fire does not spread between texels whose ground differs by more than this (m).
    pub max_step: f32,
    pub seed: u32,
}

impl Default for FireParams {
    fn default() -> Self {
        FireParams {
            spread_rate: 0.9,
            burn_time: 3.5,
            ignite_time: 0.6,
            wind: Vec2::ZERO,
            wind_influence: 0.6,
            rain: 0.0,
            ambient_wet: 0.0,
            douse_rate: 0.45,
            wet_rate: 0.35,
            dry_time: 40.0,
            wet_block: 0.6,
            regrow_delay: 30.0,
            regrow_time: 40.0,
            wet_regrow_boost: 0.5,
            ember_rate: 0.006,
            ember_distance: 1.5,
            max_step: 1.2,
            seed: 1,
        }
    }
}

/// One texel of the field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireTexel {
    /// Grass fuel when fully grown (0 = no grass: never burns).
    pub fuel_max: f32,
    /// Current fuel 0..fuel_max.
    pub fuel: f32,
    /// Burning intensity 0..1 (0 = not burning).
    pub heat: f32,
    /// Char 0..1 (burnt grass / scorched ground).
    pub burnt: f32,
    /// Wetness 0..1.
    pub wet: f32,
    /// Seconds since the fire went out (regrowth timer).
    pub since_fire: f32,
    /// Ground height (terrain-local) for the cliff test and flame placement.
    pub height: f32,
}

impl FireTexel {
    pub fn is_burning(&self) -> bool {
        self.heat > 0.0
    }
    /// Grass growth 0..1 (1 = full height, or no grass at all).
    pub fn growth(&self) -> f32 {
        if self.fuel_max > 0.0 { (self.fuel / self.fuel_max).clamp(0.0, 1.0) } else { 1.0 }
    }
}

/// Aggregated state of one terrain cell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellFire {
    pub texels: u32,
    /// Texels burning right now.
    pub burning: u32,
    /// Largest heat.
    pub heat: f32,
    /// Mean char / wetness / growth over the cell's texels.
    pub burnt: f32,
    pub wet: f32,
    pub growth: f32,
    /// Fraction of the cell's texels with grass.
    pub fuel: f32,
}

/// What changed in one step (cells start / stop burning).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepEvents {
    pub ignited_cells: Vec<IVec2>,
    pub extinguished_cells: Vec<IVec2>,
    /// The whole field went from no fire to fire / from fire to none.
    pub fire_started: bool,
    pub fire_out: bool,
}

#[derive(Clone, Debug)]
pub struct FireField {
    /// Terrain-local XZ of the field's min corner.
    pub origin: Vec2,
    pub texel: f32,
    pub width: i32,
    pub height: i32,
    pub texels: Vec<FireTexel>,
    /// Index into `cells` per texel (`u32::MAX` = off the terrain).
    pub cell_of: Vec<u32>,
    /// Terrain cells (samples) touched by the field.
    pub cells: Vec<IVec2>,
    cell_burning: Vec<bool>,
    any_burning: bool,
    pub tick: u64,
    /// Burning texel indices after the last step.
    pub burning: Vec<u32>,
}

const N8: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];

fn rand01(seed: u32, tick: u64, i: u32, salt: u32) -> f32 {
    let h = hash_u32((seed as u64) << 40 ^ tick.wrapping_mul(0x9E37_79B9) ^ ((i as u64) << 8) ^ salt as u64);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

impl FireField {
    /// An empty (fuel-less) field of `width` × `height` texels.
    pub fn new(origin: Vec2, texel: f32, width: i32, height: i32) -> FireField {
        let (width, height) = (width.max(1), height.max(1));
        let n = (width * height) as usize;
        FireField {
            origin,
            texel: texel.max(0.01),
            width,
            height,
            texels: vec![FireTexel::default(); n],
            cell_of: vec![u32::MAX; n],
            cells: Vec::new(),
            cell_burning: Vec::new(),
            any_burning: false,
            tick: 0,
            burning: Vec::new(),
        }
    }

    /// A field covering `[min, max]` (terrain-local XZ) with grass fuel read from the terrain:
    /// texels whose cell's floor slot grows grass (`slot_has_grass`) and lies at or above
    /// `min_height` get the cell's grass density as fuel; the cell is remembered per texel.
    /// `height` gives the ground height at a terrain-local XZ (`None` = no ground).
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
        let mut texel = texel.max(0.05);
        let size = max - min;
        let dim = |t: f32| ((size.x / t).ceil().max(1.0) as i32, (size.y / t).ceil().max(1.0) as i32);
        // Coarsen the lattice until it fits the texture budget.
        while {
            let (w, h) = dim(texel);
            w > max_texels || h > max_texels
        } {
            texel *= 1.25;
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
                let Some(sample) = store.sample(s) else { continue };
                if sample.floor == VOID_SLOT {
                    continue;
                }
                let Some(hgt) = height(p) else { continue };
                let cell = if cfg.grid == GridType::Square { s } else { cfg.wrap_sample(s) };
                let next = f.cells.len() as u32;
                let ci = *index.entry(cell).or_insert_with(|| {
                    f.cells.push(cell);
                    next
                });
                f.cell_of[i] = ci;
                let t = &mut f.texels[i];
                t.height = hgt;
                let grass = slot_has_grass[(sample.floor as usize).min(15)] && hgt >= min_height;
                if grass {
                    t.fuel_max = sample.grass as f32 / 255.0;
                    t.fuel = t.fuel_max;
                }
            }
        }
        f.cell_burning = vec![false; f.cells.len()];
        f
    }

    pub fn len(&self) -> usize {
        self.texels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texels.is_empty()
    }

    /// Terrain-local XZ centre of texel `(x, y)`.
    pub fn texel_center(&self, x: i32, y: i32) -> Vec2 {
        self.origin + v2((x as f32 + 0.5) * self.texel, (y as f32 + 0.5) * self.texel)
    }

    pub fn texel_pos(&self, i: usize) -> Vec2 {
        self.texel_center(i as i32 % self.width, i as i32 / self.width)
    }

    /// Texel under a terrain-local XZ point.
    pub fn texel_at(&self, p: Vec2) -> Option<usize> {
        let x = ((p.x - self.origin.x) / self.texel).floor() as i32;
        let y = ((p.y - self.origin.y) / self.texel).floor() as i32;
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| (y * self.width + x) as usize)
    }

    /// Texel indices whose centre lies within `radius` of `p` (at least the texel under `p`).
    pub fn texels_in_radius(&self, p: Vec2, radius: f32) -> Vec<usize> {
        let r = radius.max(0.0);
        let x0 = ((p.x - r - self.origin.x) / self.texel).floor() as i32;
        let x1 = ((p.x + r - self.origin.x) / self.texel).floor() as i32;
        let y0 = ((p.y - r - self.origin.y) / self.texel).floor() as i32;
        let y1 = ((p.y + r - self.origin.y) / self.texel).floor() as i32;
        let mut out = Vec::new();
        for y in y0.max(0)..=y1.min(self.height - 1) {
            for x in x0.max(0)..=x1.min(self.width - 1) {
                if self.texel_center(x, y).distance(p) <= r {
                    out.push((y * self.width + x) as usize);
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

    /// Sets a texel's grass (fuel) directly, e.g. for fields built without a terrain.
    pub fn set_fuel(&mut self, i: usize, fuel: f32) {
        if let Some(t) = self.texels.get_mut(i) {
            t.fuel_max = fuel.clamp(0.0, 1.0);
            t.fuel = t.fuel_max;
        }
    }

    /// Lights every burnable texel within `radius` of `p`; returns how many caught. Wet texels
    /// catch too but are doused quickly by their own wetness.
    pub fn ignite(&mut self, p: Vec2, radius: f32) -> usize {
        let mut n = 0;
        for i in self.texels_in_radius(p, radius) {
            let t = &mut self.texels[i];
            if t.fuel > FUEL_MIN * t.fuel_max.max(1e-3) && t.fuel > 0.0 {
                t.heat = t.heat.max(0.35);
                t.since_fire = 0.0;
                n += 1;
            }
        }
        n
    }

    /// Lights every burnable texel of a terrain cell.
    pub fn ignite_cell(&mut self, cell: IVec2) -> usize {
        let Some(ci) = self.cells.iter().position(|c| *c == cell) else { return 0 };
        let mut n = 0;
        for i in 0..self.texels.len() {
            if self.cell_of[i] == ci as u32 {
                let t = &mut self.texels[i];
                if t.fuel > 0.0 && t.fuel > FUEL_MIN * t.fuel_max {
                    t.heat = t.heat.max(0.35);
                    t.since_fire = 0.0;
                    n += 1;
                }
            }
        }
        n
    }

    /// Puts out every texel within `radius` of `p` and wets it to at least `wet`; returns how
    /// many were burning.
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

    /// Puts out every fire (keeps the char).
    pub fn extinguish_all(&mut self) {
        for t in &mut self.texels {
            if t.heat > 0.0 {
                t.heat = 0.0;
                t.since_fire = 0.0;
            }
        }
    }

    /// Back to fully grown, dry, unburnt grass.
    pub fn reset(&mut self) {
        for t in &mut self.texels {
            t.fuel = t.fuel_max;
            t.heat = 0.0;
            t.burnt = 0.0;
            t.wet = 0.0;
            t.since_fire = 0.0;
        }
        self.burning.clear();
    }

    /// Aggregated state of a terrain cell (`None` if the field doesn't cover it).
    pub fn cell_state(&self, cell: IVec2) -> Option<CellFire> {
        let ci = self.cells.iter().position(|c| *c == cell)? as u32;
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

    /// Cells with at least one burning texel.
    pub fn burning_cells(&self) -> Vec<IVec2> {
        self.cell_burning.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| self.cells[i]).collect()
    }

    /// Advances the field by `dt` seconds (call at a fixed rate for determinism).
    pub fn step(&mut self, p: &FireParams, dt: f32) -> StepEvents {
        let dt = dt.max(0.0);
        self.tick = self.tick.wrapping_add(1);
        let (w, h) = (self.width, self.height);
        let heat0: Vec<f32> = self.texels.iter().map(|t| t.heat).collect();
        let wind_len = p.wind.length();
        let wind_dir = if wind_len > 1e-4 { p.wind / wind_len } else { Vec2::ZERO };
        let wk = p.wind_influence * wind_len;
        // Neighbour weights: direction from the neighbour towards this texel vs the wind.
        let mut nw = [0.0f32; 8];
        for (k, (dx, dy)) in N8.iter().enumerate() {
            let d = v2(-*dx as f32, -*dy as f32);
            let len = d.length();
            let along = (d / len).dot(wind_dir);
            nw[k] = (1.0 + wk * along).max(0.0) / len;
        }
        let dry = 1.0 / p.dry_time.max(0.01);
        let rain = p.rain.clamp(0.0, 1.0);
        let ambient = p.ambient_wet.clamp(0.0, 1.0);
        let mut sparks: Vec<usize> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut t = self.texels[i];
                // Wetness: rain soaks, otherwise dry out towards the ambient ground wetness.
                t.wet = (t.wet + rain * p.wet_rate * dt).min(1.0);
                if t.wet > ambient {
                    t.wet = (t.wet - dry * (1.0 - rain) * dt).max(ambient);
                } else {
                    t.wet = (t.wet + p.wet_rate * dt).min(ambient);
                }
                if t.heat > 0.0 {
                    if t.fuel > 0.0 {
                        // Rain stops the flames from building up again.
                        t.heat = (t.heat + (1.0 - rain) * dt / p.ignite_time.max(0.01)).min(1.0);
                        t.fuel = (t.fuel - t.fuel_max.max(0.05) * t.heat * dt / p.burn_time.max(0.05)).max(0.0);
                    } else {
                        // Smoulder out.
                        t.heat -= dt / (p.burn_time.max(0.05) * 0.4);
                    }
                    t.heat -= (p.douse_rate * rain + t.wet * 0.3) * dt;
                    if t.fuel_max > 0.0 {
                        t.burnt = t.burnt.max(1.0 - t.fuel / t.fuel_max);
                    }
                    if t.heat <= 0.0 {
                        t.heat = 0.0;
                        t.since_fire = 0.0;
                        t.burnt = t.burnt.max(0.35);
                    } else if p.ember_rate > 0.0 && t.heat > 0.5 && rand01(p.seed, self.tick, i as u32, 7) < p.ember_rate * t.heat * dt {
                        // Ember: a spot fire downwind (or a short hop anywhere in calm air).
                        let a = rand01(p.seed, self.tick, i as u32, 11) * std::f32::consts::TAU;
                        let jitter = v2(a.cos(), a.sin()) * (1.0 + rand01(p.seed, self.tick, i as u32, 13) * 1.5);
                        let reach = wind_dir * (2.0 + p.ember_distance * wind_len * (0.5 + rand01(p.seed, self.tick, i as u32, 17)));
                        let tx = x + (reach.x + jitter.x).round() as i32;
                        let ty = y + (reach.y + jitter.y).round() as i32;
                        if tx >= 0 && ty >= 0 && tx < w && ty < h {
                            sparks.push((ty * w + tx) as usize);
                        }
                    }
                } else if t.fuel_max > 0.0 {
                    if t.fuel < t.fuel_max {
                        // Regrowth after the fire, faster when wet.
                        let boost = 1.0 + p.wet_regrow_boost * t.wet;
                        t.since_fire += dt * boost;
                        if t.since_fire >= p.regrow_delay {
                            t.fuel = (t.fuel + t.fuel_max * dt * boost / p.regrow_time.max(0.01)).min(t.fuel_max);
                        }
                    }
                    if t.burnt > 0.0 && t.since_fire >= p.regrow_delay {
                        // Char fades as the grass comes back.
                        t.burnt = t.burnt.min(1.0 - t.growth()).max(0.0);
                    }
                    // Catch fire from burning neighbours.
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
                            if hn <= 0.0 || (self.texels[j].height - t.height).abs() > p.max_step {
                                continue;
                            }
                            hazard += hn * nw[k];
                        }
                        if hazard > 0.0 {
                            let dryness = 1.0 - (t.wet / p.wet_block.max(1e-3)).clamp(0.0, 1.0);
                            let chance = 1.0 - (-hazard * p.spread_rate * t.growth() * dryness * dt).exp();
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
            if t.heat <= 0.0 && t.fuel > FUEL_MIN * t.fuel_max && t.fuel_max > 0.0 && t.wet < p.wet_block {
                t.heat = SPARK_HEAT;
                t.since_fire = 0.0;
            }
        }
        self.collect_events()
    }

    fn collect_events(&mut self) -> StepEvents {
        let mut ev = StepEvents::default();
        self.burning.clear();
        let mut now = vec![false; self.cells.len()];
        for (i, t) in self.texels.iter().enumerate() {
            if t.is_burning() {
                self.burning.push(i as u32);
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
                ev.ignited_cells.push(self.cells[ci]);
            } else if was && !is {
                ev.extinguished_cells.push(self.cells[ci]);
            }
        }
        self.cell_burning = now;
        let any = !self.burning.is_empty();
        ev.fire_started = any && !self.any_burning;
        ev.fire_out = !any && self.any_burning;
        self.any_burning = any;
        ev
    }

    /// RGBA8 texture data, one pixel per texel, rows along +Z: R = heat, G = char,
    /// B = wetness, A = grass growth (255 where there is no grass).
    pub fn write_rgba8(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.texels.len() * 4);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        for t in &self.texels {
            out.extend_from_slice(&[q(t.heat), q(t.burnt), q(t.wet), q(t.growth())]);
        }
    }

    /// Total heat (a cheap fire size measure).
    pub fn total_heat(&self) -> f32 {
        self.burning.iter().map(|&i| self.texels[i as usize].heat).sum()
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

    fn run(f: &mut FireField, p: &FireParams, seconds: f32) -> Vec<StepEvents> {
        let dt = 0.1;
        (0..(seconds / dt).round() as i32).map(|_| f.step(p, dt)).collect()
    }

    fn burnt_count(f: &FireField) -> usize {
        f.texels.iter().filter(|t| t.burnt > 0.3).count()
    }

    #[test]
    fn fire_spreads_burns_out_and_regrows() {
        let mut f = meadow(24);
        let p = FireParams { regrow_delay: 30.0, regrow_time: 10.0, ember_rate: 0.0, ..Default::default() };
        assert!(f.ignite(v2(12.0, 12.0), 1.0) > 0);
        run(&mut f, &p, 4.0);
        assert!(f.burning.len() > 10, "fire should spread, burning {}", f.burning.len());
        run(&mut f, &p, 26.0);
        assert!(f.burning.is_empty(), "fire should burn out");
        assert!(burnt_count(&f) > 24 * 24 / 2, "most of the meadow burnt: {}", burnt_count(&f));
        let g: f32 = f.texels.iter().map(|t| t.growth()).sum::<f32>() / f.len() as f32;
        run(&mut f, &p, 36.0);
        let g2: f32 = f.texels.iter().map(|t| t.growth()).sum::<f32>() / f.len() as f32;
        assert!(g2 > g + 0.3, "grass regrows ({g} -> {g2})");
        run(&mut f, &p, 60.0);
        assert!(f.texels.iter().all(|t| t.growth() > 0.99 && t.burnt < 0.01), "fully regrown");
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
        let p = FireParams { ember_rate: 0.0, ..Default::default() };
        f.ignite(v2(3.0, 10.0), 1.5);
        run(&mut f, &p, 40.0);
        let right = (0..20).flat_map(|y| (11..20).map(move |x| (y * 20 + x) as usize)).filter(|&i| f.texels[i].burnt > 0.0).count();
        assert_eq!(right, 0, "fire jumped a 2-texel firebreak");
        assert!(burnt_count(&f) > 100);
    }

    #[test]
    fn wind_biases_spread_downwind() {
        let mut f = meadow(41);
        let p = FireParams { wind: v2(4.0, 0.0), wind_influence: 0.25, ember_rate: 0.0, ..Default::default() };
        f.ignite(v2(20.5, 20.5), 1.0);
        run(&mut f, &p, 5.0);
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
        let dry = FireParams { ember_rate: 0.0, ..Default::default() };
        f.ignite(v2(15.0, 15.0), 2.0);
        run(&mut f, &dry, 2.0);
        assert!(!f.burning.is_empty());
        let wet = FireParams { rain: 1.0, ..dry.clone() };
        let ev = run(&mut f, &wet, 3.0);
        assert!(f.burning.is_empty(), "rain puts the fire out");
        assert!(ev.iter().any(|e| e.fire_out));
        assert!(f.texels.iter().all(|t| t.wet > 0.5));
        // Soaked grass does not catch.
        f.ignite(v2(5.0, 5.0), 0.5);
        run(&mut f, &dry, 3.0);
        assert!(burnt_count(&f) < 30 * 30 / 4);
    }

    #[test]
    fn deterministic() {
        let p = FireParams { wind: v2(1.0, 2.0), seed: 9, ..Default::default() };
        let mut a = meadow(20);
        let mut b = meadow(20);
        a.ignite(v2(5.0, 5.0), 1.0);
        b.ignite(v2(5.0, 5.0), 1.0);
        run(&mut a, &p, 6.0);
        run(&mut b, &p, 6.0);
        assert_eq!(a.texels, b.texels);
        let (mut da, mut db) = (Vec::new(), Vec::new());
        a.write_rgba8(&mut da);
        b.write_rgba8(&mut db);
        assert_eq!(da, db);
        assert_eq!(da.len(), 20 * 20 * 4);
    }

    #[test]
    fn hex_cells_from_store_and_events() {
        let cfg = GridConfig { grid: GridType::HexRings, ring_radius: 2, cell_size: v2(2.0, 2.0), ..Default::default() };
        let mut store = TerrainStore::new(cfg);
        store.add_chunk(iv2(0, 0), MergeMode::Cubic);
        let mut grass = [false; 16];
        grass[0] = true;
        let h = |_p: Vec2| Some(0.0f32);
        let f0 = FireField::from_store(&store, v2(-6.0, -6.0), v2(6.0, 6.0), 0.5, &grass, -1.0, 256, h);
        assert_eq!(f0.width, 24);
        assert_eq!(f0.cells.len(), 19, "a radius-2 hex chunk has 19 cells");
        assert!(f0.texels.iter().filter(|t| t.fuel_max > 0.0).count() > 100);
        let mut f = f0.clone();
        let p = FireParams { ember_rate: 0.0, ..Default::default() };
        assert!(f.ignite_cell(iv2(0, 0)) > 0);
        let ev = f.step(&p, 0.1);
        assert!(ev.fire_started);
        assert!(ev.ignited_cells.contains(&iv2(0, 0)));
        assert!(f.burning_cells().contains(&iv2(0, 0)));
        let evs = run(&mut f, &p, 30.0);
        let ignited: usize = evs.iter().map(|e| e.ignited_cells.len()).sum::<usize>() + 1;
        assert!(ignited >= 10, "fire crossed hex cells: {ignited}");
        assert!(evs.iter().any(|e| e.fire_out));
        let s = f.cell_state(iv2(0, 0)).unwrap();
        assert!(s.burnt > 0.3 && s.burning == 0 && s.fuel > 0.9);
        // No grass slot: nothing burns.
        let f2 = FireField::from_store(&store, v2(-6.0, -6.0), v2(6.0, 6.0), 0.5, &[false; 16], -1.0, 256, h);
        assert!(f2.texels.iter().all(|t| t.fuel_max == 0.0));
        // The texture budget coarsens the lattice.
        let f3 = FireField::from_store(&store, v2(-6.0, -6.0), v2(6.0, 6.0), 0.05, &grass, -1.0, 64, h);
        assert!(f3.width <= 64 && f3.height <= 64);
    }

    #[test]
    fn cliffs_block_spread() {
        let mut f = meadow(20);
        for i in 0..f.len() {
            if i as i32 % 20 >= 10 {
                f.texels[i].height = 3.0;
            }
        }
        let p = FireParams { ember_rate: 0.0, ..Default::default() };
        f.ignite(v2(3.0, 10.0), 1.5);
        run(&mut f, &p, 40.0);
        assert!(f.texels.iter().enumerate().filter(|(i, _)| *i as i32 % 20 >= 10).all(|(_, t)| t.burnt == 0.0));
    }
}
