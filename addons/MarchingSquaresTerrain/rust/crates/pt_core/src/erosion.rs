use crate::brush::Touched;
use crate::data::TerrainStore;
use crate::math::*;
#[derive(Clone, Debug, PartialEq)]
pub struct HydraulicSettings {
    pub seed: i32,
    pub splash_radius: i32,
    pub momentum: f32,
    pub load_limit: f32,
    pub min_load: f32,
    pub carve_rate: f32,
    pub drop_rate: f32,
    pub dry_rate: f32,
    pub gravity: f32,
    pub droplet_steps: i32,
    pub start_volume: f32,
    pub start_momentum: f32,
}
impl Default for HydraulicSettings {
    fn default() -> Self {
        HydraulicSettings {
            seed: 0,
            splash_radius: 4,
            momentum: 0.0504,
            load_limit: 4.0004,
            min_load: 0.0104,
            carve_rate: 0.3004,
            drop_rate: 0.3004,
            dry_rate: 0.0104,
            gravity: 4.0004,
            droplet_steps: 32,
            start_volume: 1.0004,
            start_momentum: 1.0004,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceSample {
    pub stature: f32,
    pub slope_x: f32,
    pub slope_z: f32,
}
#[derive(Clone, Debug)]
pub struct XorShift32(pub u32);
impl XorShift32 {
    pub fn new(seed: i32) -> XorShift32 {
        let s = hash_u32(seed as u32 as u64 ^ 0x5EED_E205);
        XorShift32(if s == 0 { 0x6D2B_79F5 } else { s })
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    pub fn band(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + ((self.next_u32() as u64 * (hi - lo) as u64) >> 32) as i32
    }
}
pub struct Eroder {
    pub params: HydraulicSettings,
    pub breadth: usize,
    pub stature: usize,
    prng: XorShift32,
    disk: Vec<(i32, i32, f32)>,
}
impl Eroder {
    pub fn new(params: HydraulicSettings, width: usize, height: usize) -> Eroder {
        let r = params.splash_radius.clamp(1, 16);
        let mut disk = Vec::new();
        for y in -r..=r {
            for x in -r..=r {
                let d2 = (x * x + y * y) as f32;
                if d2 < (r * r) as f32 {
                    disk.push((x, y, 1.0 - d2.sqrt() / r as f32));
                }
            }
        }
        Eroder {
            prng: XorShift32::new(params.seed),
            params,
            breadth: width,
            stature: height,
            disk,
        }
    }
    pub fn surface_sample(&self, map: &[f32], pos_x: f32, pos_y: f32) -> SurfaceSample {
        let (cx, cy) = (pos_x as i32, pos_y as i32);
        let (x, y) = (pos_x - cx as f32, pos_y - cy as f32);
        let i = cy as usize * self.breadth + cx as usize;
        let nw = map[i];
        let ne = map[i + 1];
        let sw = map[i + self.breadth];
        let se = map[i + self.breadth + 1];
        SurfaceSample {
            stature: nw * (1.0 - x) * (1.0 - y)
                + ne * x * (1.0 - y)
                + sw * (1.0 - x) * y
                + se * x * y,
            slope_x: (ne - nw) * (1.0 - y) + (se - sw) * y,
            slope_z: (sw - nw) * (1.0 - x) + (se - ne) * x,
        }
    }
    fn brush(&self, cx: i32, cy: i32, out: &mut Vec<(usize, f32)>) {
        out.clear();
        let mut sum = 0.0;
        for &(ox, oy, w) in &self.disk {
            let (x, y) = (cx + ox, cy + oy);
            if x >= 0 && y >= 0 && (x as usize) < self.breadth && (y as usize) < self.stature {
                sum += w;
                out.push((y as usize * self.breadth + x as usize, w));
            }
        }
        if sum > 0.0 {
            for e in out.iter_mut() {
                e.1 /= sum;
            }
        }
    }
    pub fn erode(&mut self, map: &mut [f32], iterations: u32) {
        assert_eq!(map.len(), self.breadth * self.stature, "erosion map size");
        if self.breadth < 3 || self.stature < 3 {
            return;
        }
        let p = self.params.clone();
        let (max_x, max_y) = ((self.breadth - 1) as f32, (self.stature - 1) as f32);
        let mut brush = Vec::new();
        for _ in 0..iterations {
            let mut pos_x = self.prng.band(0, self.breadth as i32 - 1) as f32;
            let mut pos_y = self.prng.band(0, self.stature as i32 - 1) as f32;
            let (mut dir_x, mut dir_y) = (0.0f32, 0.0f32);
            let mut speed = p.start_momentum;
            let mut volume = p.start_volume;
            let mut load = 0.0f32;
            for _ in 0..p.droplet_steps.max(0) {
                let (node_x, node_y) = (pos_x as i32, pos_y as i32);
                let droplet = node_y as usize * self.breadth + node_x as usize;
                let (off_x, off_y) = (pos_x - node_x as f32, pos_y - node_y as f32);
                let s = self.surface_sample(map, pos_x, pos_y);
                dir_x = dir_x * p.momentum - s.slope_x * (1.0 - p.momentum);
                dir_y = dir_y * p.momentum - s.slope_z * (1.0 - p.momentum);
                let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
                if len != 0.0 {
                    dir_x /= len;
                    dir_y /= len;
                }
                pos_x += dir_x;
                pos_y += dir_y;
                if (dir_x == 0.0 && dir_y == 0.0)
                    || pos_x < 0.0
                    || pos_x >= max_x
                    || pos_y < 0.0
                    || pos_y >= max_y
                {
                    break;
                }
                let new_height = self.surface_sample(map, pos_x, pos_y).stature;
                let delta_height = new_height - s.stature;
                let capacity = (-delta_height * speed * volume * p.load_limit).max(p.min_load);
                if load > capacity || delta_height > 0.0 {
                    let amount = if delta_height > 0.0 {
                        delta_height.min(load)
                    } else {
                        (load - capacity) * p.drop_rate
                    };
                    load -= amount;
                    map[droplet] += amount * (1.0 - off_x) * (1.0 - off_y);
                    map[droplet + 1] += amount * off_x * (1.0 - off_y);
                    map[droplet + self.breadth] += amount * (1.0 - off_x) * off_y;
                    map[droplet + self.breadth + 1] += amount * off_x * off_y;
                } else {
                    let amount = ((capacity - load) * p.carve_rate).min(-delta_height);
                    self.brush(node_x, node_y, &mut brush);
                    for &(i, w) in &brush {
                        let weighed = amount * w;
                        let d = if map[i] < weighed { map[i] } else { weighed };
                        map[i] -= d;
                        load += d;
                    }
                }
                speed = (speed * speed + delta_height * p.gravity).max(0.0).sqrt();
                volume *= 1.0 - p.dry_rate;
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ErosionMode {
    #[default]
    Auto = 0,
    Raw = 1,
    Terraced = 2,
}
impl ErosionMode {
    pub fn from_i32(v: i32) -> ErosionMode {
        match v {
            1 => ErosionMode::Raw,
            2 => ErosionMode::Terraced,
            _ => ErosionMode::Auto,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct StoreErosion {
    pub params: HydraulicSettings,
    pub iterations: u32,
    pub pixels_per_cell: u32,
    pub height_scale: f32,
    pub region: Option<(Vec2, Vec2)>,
    pub mode: ErosionMode,
    pub level_step: f32,
    pub min_plateau_cells: u32,
}
impl Default for StoreErosion {
    fn default() -> Self {
        StoreErosion {
            params: HydraulicSettings::default(),
            iterations: 30_000,
            pixels_per_cell: 2,
            height_scale: 0.0,
            region: None,
            mode: ErosionMode::Auto,
            level_step: 0.0,
            min_plateau_cells: 4,
        }
    }
}
impl StoreErosion {
    pub fn terraced(&self, store: &TerrainStore) -> bool {
        match self.mode {
            ErosionMode::Raw => false,
            ErosionMode::Terraced => true,
            ErosionMode::Auto => store.config.grid.is_cell_mode(),
        }
    }
    pub fn step(&self, store: &TerrainStore) -> f32 {
        let s = if self.level_step > 0.0 {
            self.level_step
        } else {
            store.ridges.terrace_step.abs()
        };
        if s > 1e-3 { s } else { 0.5 }
    }
}
pub const PROGRESS_BATCH: u32 = 2000;
pub fn erode_store(store: &mut TerrainStore, e: &StoreErosion) -> Touched {
    erode_store_with(store, e, &mut |_| true)
}
pub fn erode_store_with(
    store: &mut TerrainStore,
    e: &StoreErosion,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Touched {
    let mut touched = Touched::new();
    let cfg = store.config.clone();
    let chunks = store.chunk_coords();
    if chunks.is_empty() {
        return touched;
    }
    let (mut min, mut max) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
    for c in &chunks {
        for p in cfg.chunk_outline(*c) {
            min = v2(min.x.min(p.x), min.y.min(p.y));
            max = v2(max.x.max(p.x), max.y.max(p.y));
        }
    }
    if let Some((a, b)) = e.region {
        min = v2(min.x.max(a.x.min(b.x)), min.y.max(a.y.min(b.y)));
        max = v2(max.x.min(a.x.max(b.x)), max.y.min(a.y.max(b.y)));
    }
    if max.x <= min.x || max.y <= min.y {
        return touched;
    }
    let cs = cfg.cell_size_safe();
    let step = cs.x.min(cs.y) / e.pixels_per_cell.clamp(1, 8) as f32;
    let w = (((max.x - min.x) / step).floor() as usize + 1).clamp(3, 4096);
    let h = (((max.y - min.y) / step).floor() as usize + 1).clamp(3, 4096);
    let mut owner: Vec<Option<IVec2>> = Vec::with_capacity(w * h);
    let mut raw = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let p = min + v2(x as f32 * step, y as f32 * step);
            let s = cfg.world_to_sample(p);
            match store.stature(s) {
                Some(hh) => {
                    owner.push(Some(s));
                    raw.push(hh);
                }
                None => {
                    owner.push(None);
                    raw.push(f32::NAN);
                }
            }
        }
    }
    let (lo, hi) = raw
        .iter()
        .filter(|v| v.is_finite())
        .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    if lo > hi {
        return touched;
    }
    let scale = if e.height_scale > 0.0 {
        e.height_scale
    } else {
        (hi - lo).max(1.0)
    };
    let norm: Vec<f32> = raw
        .iter()
        .map(|v| if v.is_finite() { (v - lo) / scale } else { 0.0 })
        .collect();
    let mut map = vec![0.0; w * h];
    let passes = if cfg.grid == crate::grid::GridType::Square && e.pixels_per_cell <= 1 {
        0
    } else {
        1 + e.pixels_per_cell.min(4) / 2
    };
    map.copy_from_slice(&norm);
    for _ in 0..passes {
        let src = map.clone();
        for y in 0..h {
            for x in 0..w {
                let (mut s, mut n) = (0.0, 0.0);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                        if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                            s += src[yy as usize * w + xx as usize];
                            n += 1.0;
                        }
                    }
                }
                map[y * w + x] = s / n;
            }
        }
    }
    let before = map.clone();
    let mut eroder = Eroder::new(e.params.clone(), w, h);
    let mut done = 0;
    while done < e.iterations {
        let n = PROGRESS_BATCH.min(e.iterations - done);
        eroder.erode(&mut map, n);
        done += n;
        if !progress(done as f32 / e.iterations.max(1) as f32) {
            return touched;
        }
    }
    let mut acc: std::collections::HashMap<IVec2, (f32, u32)> = std::collections::HashMap::new();
    for i in 0..w * h {
        if let Some(s) = owner[i] {
            let a = acc.entry(s).or_insert((0.0, 0));
            a.0 += map[i] - before[i];
            a.1 += 1;
        }
    }
    let mut deltas: std::collections::HashMap<IVec2, f32> = acc
        .into_iter()
        .map(|(s, (d, n))| (s, d / n as f32 * scale))
        .collect();
    if e.terraced(store) {
        deltas = terrace_deltas(store, &deltas, e.step(store));
        remove_small_plateaus(store, &mut deltas, e.min_plateau_cells as usize);
    }
    let mut keys: Vec<IVec2> = deltas.keys().copied().collect();
    keys.sort();
    for s in keys {
        let delta = deltas[&s];
        if delta.abs() < 1e-5 {
            continue;
        }
        let Some(old) = store.stature(s) else {
            continue;
        };
        if store.set_height(s, old + delta) {
            for c in store.chunks_affected_by(s) {
                touched.insert(c);
            }
        }
    }
    touched
}
pub fn terrace_deltas(
    store: &TerrainStore,
    raw: &std::collections::HashMap<IVec2, f32>,
    step: f32,
) -> std::collections::HashMap<IVec2, f32> {
    use std::collections::HashMap;
    let cfg = &store.config;
    let nbs = |s: IVec2| -> Vec<IVec2> {
        cfg.sample_neighbors(s)
            .into_iter()
            .filter(|n| store.stature(*n).is_some())
            .collect()
    };
    let mut d = raw.clone();
    for _ in 0..2 {
        let src = d.clone();
        for (s, v) in d.iter_mut() {
            let ns = nbs(*s);
            if ns.is_empty() {
                continue;
            }
            let mean = ns
                .iter()
                .map(|n| src.get(n).copied().unwrap_or(0.0))
                .sum::<f32>()
                / ns.len() as f32;
            *v = src[s] * 0.5 + mean * 0.5;
        }
    }
    let q: HashMap<IVec2, f32> = d
        .iter()
        .map(|(s, v)| (*s, (v / step).round() * step))
        .collect();
    let new_h =
        |s: IVec2| -> Option<f32> { Some(store.stature(s)? + q.get(&s).copied().unwrap_or(0.0)) };
    let mut out = HashMap::with_capacity(q.len());
    for (s, dq) in &q {
        if *dq == 0.0 {
            continue;
        }
        let (Some(old), Some(mut h)) = (store.stature(*s), new_h(*s)) else {
            continue;
        };
        let mut hs: Vec<f32> = nbs(*s).into_iter().filter_map(new_h).collect();
        if hs.len() >= 3 {
            hs.sort_by(f32::total_cmp);
            let (lo, hi) = (hs[1], hs[hs.len() - 2]);
            if h < lo {
                h = lo.min(old);
            } else if h > hi {
                h = hi.max(old);
            }
        }
        if (h - old).abs() > 1e-5 {
            out.insert(*s, h - old);
        }
    }
    out
}
pub fn remove_small_plateaus(
    store: &TerrainStore,
    deltas: &mut std::collections::HashMap<IVec2, f32>,
    min_cells: usize,
) {
    use std::collections::{HashMap, HashSet, VecDeque};
    if min_cells < 2 {
        return;
    }
    const EPS: f32 = 1e-4;
    let cfg = &store.config;
    let nbs = |s: IVec2| -> Vec<IVec2> {
        cfg.sample_neighbors(s)
            .into_iter()
            .filter(|n| store.stature(*n).is_some())
            .collect()
    };
    for _ in 0..4 {
        let new_h = |s: IVec2, d: &HashMap<IVec2, f32>| -> Option<f32> {
            Some(store.stature(s)? + d.get(&s).copied().unwrap_or(0.0))
        };
        let mut seeds: Vec<IVec2> = Vec::new();
        let mut seen_seed = HashSet::new();
        let mut changed: Vec<IVec2> = deltas
            .iter()
            .filter(|(_, v)| v.abs() > EPS)
            .map(|(s, _)| *s)
            .collect();
        changed.sort();
        for s in changed {
            for t in std::iter::once(s).chain(nbs(s)) {
                if seen_seed.insert(t) {
                    seeds.push(t);
                }
            }
        }
        let mut visited: HashSet<IVec2> = HashSet::new();
        let mut edits: Vec<(Vec<IVec2>, f32)> = Vec::new();
        for seed in seeds {
            if visited.contains(&seed) {
                continue;
            }
            let Some(h) = new_h(seed, deltas) else {
                continue;
            };
            let mut comp = vec![seed];
            let mut in_comp: HashSet<IVec2> = HashSet::from([seed]);
            let mut queue = VecDeque::from([seed]);
            let mut border: HashSet<IVec2> = HashSet::new();
            let mut big = false;
            while let Some(s) = queue.pop_front() {
                for n in nbs(s) {
                    if in_comp.contains(&n) || border.contains(&n) {
                        continue;
                    }
                    let Some(nh) = new_h(n, deltas) else { continue };
                    if (nh - h).abs() <= EPS {
                        in_comp.insert(n);
                        comp.push(n);
                        queue.push_back(n);
                        if comp.len() >= min_cells {
                            big = true;
                            break;
                        }
                    } else {
                        border.insert(n);
                    }
                }
                if big {
                    break;
                }
            }
            visited.extend(comp.iter().copied());
            if big || border.is_empty() {
                continue;
            }
            let bh: Vec<f32> = border.iter().filter_map(|b| new_h(*b, deltas)).collect();
            let (bmin, bmax) = bh
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            let column = bmax < h - EPS;
            let pit = bmin > h + EPS;
            if !column && !pit {
                continue;
            }
            let old_comp: Vec<f32> = comp.iter().filter_map(|s| store.stature(*s)).collect();
            let old_border: Vec<f32> = border.iter().filter_map(|s| store.stature(*s)).collect();
            let was = if column {
                let lo = old_comp.iter().copied().fold(f32::MAX, f32::min);
                old_border.iter().all(|b| *b < lo - EPS)
            } else {
                let hi = old_comp.iter().copied().fold(f32::MIN, f32::max);
                old_border.iter().all(|b| *b > hi + EPS)
            };
            if was {
                continue;
            }
            edits.push((comp, if column { bmax } else { bmin }));
        }
        if edits.is_empty() {
            break;
        }
        for (comp, target) in edits {
            for s in comp {
                let Some(old) = store.stature(s) else {
                    continue;
                };
                let d = target - old;
                if d.abs() > 1e-5 {
                    deltas.insert(s, d);
                } else {
                    deltas.remove(&s);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::{GridConfig, GridType};
    fn taper(n: usize) -> Vec<f32> {
        let c = (n as f32 - 1.0) * 0.5;
        (0..n * n)
            .map(|i| {
                let (x, y) = ((i % n) as f32, (i / n) as f32);
                (1.0 - ((x - c).hypot(y - c) / c)).max(0.0)
                    + 0.05 * ((x * 0.7).sin() * (y * 0.5).cos())
            })
            .map(|v| v.max(0.0))
            .collect()
    }
    #[test]
    fn defaults_are_the_tuned_bundle() {
        let p = HydraulicSettings::default();
        assert_eq!((p.splash_radius, p.droplet_steps), (4, 32));
        assert_eq!(
            (p.momentum, p.load_limit, p.min_load),
            (0.0504, 4.0004, 0.0104)
        );
        assert_eq!(
            (p.carve_rate, p.drop_rate, p.dry_rate, p.gravity),
            (0.3004, 0.3004, 0.0104, 4.0004)
        );
        assert_eq!((p.start_volume, p.start_momentum), (1.0004, 1.0004));
    }
    #[test]
    fn surface_is_bilinear() {
        let e = Eroder::new(HydraulicSettings::default(), 2, 2);
        let map = [0.0, 1.0, 2.0, 3.0];
        let s = e.surface_sample(&map, 0.5, 0.5);
        assert!((s.stature - 1.5).abs() < 1e-6);
        assert!((s.slope_x - 1.0).abs() < 1e-6 && (s.slope_z - 2.0).abs() < 1e-6);
    }
    #[test]
    fn brush_weights_normalise_inside_and_at_the_border() {
        let e = Eroder::new(HydraulicSettings::default(), 16, 16);
        let mut b = Vec::new();
        for (x, y) in [(8, 8), (0, 0), (15, 3)] {
            e.brush(x, y, &mut b);
            let s: f32 = b.iter().map(|v| v.1).sum();
            assert!((s - 1.0).abs() < 1e-5, "{x},{y}: {s}");
        }
        e.brush(8, 8, &mut b);
        assert_eq!(b.len(), 45, "radius 4 disk (d² < 16, corners excluded)");
    }
    #[test]
    fn erosion_is_deterministic_and_roughly_conserves_material() {
        let n = 64;
        let base = taper(n);
        let mut a = base.clone();
        let mut b = base.clone();
        Eroder::new(HydraulicSettings::default(), n, n).erode(&mut a, 4000);
        Eroder::new(HydraulicSettings::default(), n, n).erode(&mut b, 4000);
        assert_eq!(a, b);
        assert!(a.iter().all(|v| v.is_finite() && *v >= -1e-4));
        let changed = a
            .iter()
            .zip(&base)
            .filter(|(x, y)| (*x - *y).abs() > 1e-4)
            .count();
        assert!(changed > n * n / 8, "erosion changed {changed} nodes");
        let (s0, s1): (f32, f32) = (base.iter().sum(), a.iter().sum());
        assert!(s1 <= s0 + 1e-3 && s1 > s0 * 0.8, "{s0} → {s1}");
        let c = n / 2 * n + n / 2;
        assert!(a[c] <= base[c] + 1e-4);
    }
    #[test]
    fn erodes_a_hex_store() {
        let mut st = TerrainStore::new(GridConfig {
            grid: GridType::Hex,
            chunk_size: 12,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        });
        st.add_chunk_with_heights(iv2(0, 0), MergeMode::Polyhedron, |_, p| {
            (12.0 - p.span(v2(12.0, 10.0))).max(0.0) + (p.x * 0.9).sin()
        });
        let before: Vec<f32> = st
            .chunk_coords()
            .iter()
            .flat_map(|c| st.chunks[c].heights.clone())
            .collect();
        let t = erode_store(
            &mut st,
            &StoreErosion {
                iterations: 3000,
                ..Default::default()
            },
        );
        assert!(t.contains(&iv2(0, 0)));
        let after: Vec<f32> = st
            .chunk_coords()
            .iter()
            .flat_map(|c| st.chunks[c].heights.clone())
            .collect();
        assert!(after.iter().all(|v| v.is_finite()));
        assert!(after.iter().zip(&before).any(|(a, b)| (a - b).abs() > 1e-3));
    }
    #[test]
    fn terraced_mode_writes_whole_steps_without_pillars() {
        let mk = || {
            let mut st = TerrainStore::new(GridConfig {
                grid: GridType::HexRings,
                ring_radius: 6,
                cell_size: v2(2.0, 2.0),
                ..Default::default()
            });
            st.ridges.terrace_step = 0.5;
            for c in [
                iv2(0, 0),
                iv2(1, 0),
                iv2(0, 1),
                iv2(-1, 1),
                iv2(-1, 0),
                iv2(0, -1),
                iv2(1, -1),
            ] {
                st.add_chunk_with_heights(c, MergeMode::Polyhedron, |_, p| {
                    (((16.0 - p.magnitude()).max(0.0) * 0.6 + (p.x * 0.4).sin()) / 0.5).round()
                        * 0.5
                });
            }
            st
        };
        let heights = |st: &TerrainStore| -> std::collections::HashMap<IVec2, f32> {
            st.chunk_coords()
                .iter()
                .flat_map(|c| st.config.chunk_cells(*c))
                .filter_map(|s| Some((s, st.stature(s)?)))
                .collect()
        };
        let mut st = mk();
        let before = heights(&st);
        let e = StoreErosion {
            iterations: 20_000,
            ..Default::default()
        };
        assert!(e.terraced(&st));
        erode_store(&mut st, &e);
        let after = heights(&st);
        let mut changed = 0;
        for (s, h) in &after {
            let d = h - before[s];
            assert!(
                ((d / 0.5).round() * 0.5 - d).abs() < 1e-4,
                "{s:?}: change {d} is not a whole step"
            );
            if d.abs() > 1e-4 {
                changed += 1;
                let ns: Vec<f32> = st
                    .config
                    .sample_neighbors(*s)
                    .iter()
                    .filter_map(|n| after.get(n).copied())
                    .collect();
                if ns.len() == 6 {
                    assert!(
                        ns.iter().any(|n| *n >= *h - 1e-4) && ns.iter().any(|n| *n <= *h + 1e-4),
                        "{s:?} isolated"
                    );
                }
            }
        }
        assert!(changed > 0, "terraced erosion changed nothing");
        let mut raw = mk();
        erode_store(
            &mut raw,
            &StoreErosion {
                iterations: 20_000,
                mode: ErosionMode::Raw,
                ..Default::default()
            },
        );
        let r = heights(&raw);
        assert!(r.iter().any(|(s, h)| {
            let d = h - before[s];
            ((d / 0.5).round() * 0.5 - d).abs() > 1e-3
        }));
        let mut c = mk();
        let t = erode_store_with(&mut c, &e, &mut |_| false);
        assert!(t.is_empty() && heights(&c) == before);
    }
    #[test]
    fn small_isolated_columns_are_removed_but_real_terraces_kept() {
        use std::collections::HashMap;
        let mut st = TerrainStore::new(GridConfig {
            grid: GridType::HexRings,
            ring_radius: 8,
            cell_size: v2(2.0, 2.0),
            ..Default::default()
        });
        st.add_chunk_with_heights(iv2(0, 0), MergeMode::Polyhedron, |_, _| 2.0);
        let cells = st.config.chunk_cells(iv2(0, 0));
        let center = cells
            .iter()
            .copied()
            .min_by(|a, b| {
                let (pa, pb) = (st.config.sample_pos(*a), st.config.sample_pos(*b));
                pa.magnitude().total_cmp(&pb.magnitude())
            })
            .unwrap();
        let inner = |s: IVec2| {
            st.config
                .sample_neighbors(s)
                .into_iter()
                .all(|n| st.stature(n).is_some())
        };
        let hill = *cells
            .iter()
            .filter(|s| {
                inner(**s) && st.config.sample_pos(**s).span(st.config.sample_pos(center)) > 9.0
            })
            .next()
            .unwrap();
        st.set_height(hill, 3.0);
        let pair = [center, st.config.sample_neighbors(center)[0]];
        let mut ring: Vec<IVec2> = pair
            .iter()
            .flat_map(|s| st.config.sample_neighbors(*s))
            .filter(|n| !pair.contains(n))
            .collect();
        ring.sort();
        ring.dedup();
        let mut deltas: HashMap<IVec2, f32> = ring.iter().map(|s| (*s, -0.5)).collect();
        let mut kept = deltas.clone();
        remove_small_plateaus(&st, &mut kept, 0);
        assert_eq!(kept, deltas, "off when min_cells < 2");
        remove_small_plateaus(&st, &mut deltas, 4);
        for s in pair {
            assert!(
                (deltas.get(&s).copied().unwrap_or(0.0) + 0.5).abs() < 1e-5,
                "column cell {s:?} not flattened: {:?}",
                deltas.get(&s)
            );
        }
        for s in &ring {
            assert!((deltas[s] + 0.5).abs() < 1e-5);
        }
        assert!(
            !deltas.contains_key(&hill),
            "the real hill top was flattened"
        );
        let mut d2: HashMap<IVec2, f32> = ring.iter().map(|s| (*s, -0.5)).collect();
        remove_small_plateaus(&st, &mut d2, 2);
        assert!(
            !d2.contains_key(&pair[0]),
            "a 2-cell column is not smaller than 2 cells"
        );
    }
}
