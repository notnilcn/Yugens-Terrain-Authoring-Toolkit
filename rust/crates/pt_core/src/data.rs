//! Terrain sample storage: one [`ChunkData`] per chunk, keyed by chunk coordinate.
//! Square terrains own their vertex samples exactly once (a chunk reads its +X/+Z
//! neighbour for the shared border), so there is no duplicated border data to keep in sync.

use crate::grid::{GridConfig, GridType};
use crate::math::*;
use std::collections::HashMap;

pub const TEXTURE_SLOTS: usize = 16;
/// Texture slot 15 is the "void" slot: cells painted with it are cut out of the mesh.
pub const VOID_SLOT: u8 = 15;

/// Merge modes: the max height difference that becomes a slope/ramp instead of a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MergeMode {
    Cubic = 0,
    #[default]
    Polyhedron = 1,
    RoundedPolyhedron = 2,
    SemiRound = 3,
    Spherical = 4,
}

impl MergeMode {
    pub fn from_u8(v: u8) -> MergeMode {
        match v {
            0 => MergeMode::Cubic,
            2 => MergeMode::RoundedPolyhedron,
            3 => MergeMode::SemiRound,
            4 => MergeMode::Spherical,
            _ => MergeMode::Polyhedron,
        }
    }
    pub fn threshold(self) -> f32 {
        match self {
            MergeMode::Cubic => 0.6,
            MergeMode::Polyhedron => 1.3,
            MergeMode::RoundedPolyhedron => 2.1,
            MergeMode::SemiRound => 5.0,
            MergeMode::Spherical => 20.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChunkData {
    pub size: IVec2,
    pub heights: Vec<f32>,
    /// Floor texture slot per sample (0..15).
    pub floor_tex: Vec<u8>,
    /// Wall texture slot per sample (0..15).
    pub wall_tex: Vec<u8>,
    /// Grass density per sample (0 = masked off, 255 = full).
    pub grass: Vec<u8>,
    /// Cell modes: 1 = top surface is a smooth plane through neighbouring smooth cells (bridges).
    pub smooth: Vec<u8>,
    /// Water surface per sample (kg_world `water_level`, terrain-local), [`NO_WATER`] = none.
    /// A level above the ground is raised water (a pool / river cell with waterfalls).
    pub water: Vec<f32>,
    pub merge_mode: MergeMode,
}

/// Water level of a sample without water (below any terrain).
pub const NO_WATER: f32 = -10000.0;

/// Whether a stored water level is set.
pub fn has_water(w: f32) -> bool {
    w.is_finite() && w > NO_WATER + 1.0
}

impl ChunkData {
    pub fn new(size: IVec2, default_floor: u8, default_wall: u8, merge_mode: MergeMode) -> ChunkData {
        let n = (size.x * size.y).max(0) as usize;
        ChunkData {
            size,
            heights: vec![0.0; n],
            floor_tex: vec![default_floor; n],
            wall_tex: vec![default_wall; n],
            grass: vec![255; n],
            smooth: vec![0; n],
            water: vec![NO_WATER; n],
            merge_mode,
        }
    }

    /// Whether any sample has a water level.
    pub fn has_any_water(&self) -> bool {
        self.water.iter().any(|w| has_water(*w))
    }

    pub fn index(&self, local: IVec2) -> usize {
        (local.y * self.size.x + local.x) as usize
    }

    pub fn len(&self) -> usize {
        self.heights.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    /// Versioned little-endian binary encoding (stored in the Godot data resource). `PTC1`:
    /// heights, floor, wall, grass, smooth. `PTC2` appends the water levels (`n` f32); chunks
    /// without water keep writing `PTC1`, so old data stays byte-identical.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.len();
        let water = self.has_any_water() && self.water.len() == n;
        let mut out = Vec::with_capacity(16 + n * if water { 12 } else { 8 });
        out.extend_from_slice(if water { b"PTC2" } else { b"PTC1" });
        out.extend_from_slice(&self.size.x.to_le_bytes());
        out.extend_from_slice(&self.size.y.to_le_bytes());
        out.push(self.merge_mode as u8);
        out.extend_from_slice(&[0, 0, 0]);
        for h in &self.heights {
            out.extend_from_slice(&h.to_le_bytes());
        }
        out.extend_from_slice(&self.floor_tex);
        out.extend_from_slice(&self.wall_tex);
        out.extend_from_slice(&self.grass);
        out.extend_from_slice(&self.smooth);
        if water {
            for w in &self.water {
                out.extend_from_slice(&w.to_le_bytes());
            }
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<ChunkData> {
        if b.len() < 16 || (&b[0..4] != b"PTC1" && &b[0..4] != b"PTC2") {
            return None;
        }
        let v2 = &b[0..4] == b"PTC2";
        let rd = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let size = iv2(rd(4), rd(8));
        if size.x <= 0 || size.y <= 0 || size.x > 4096 || size.y > 4096 {
            return None;
        }
        let merge_mode = MergeMode::from_u8(b[12]);
        let n = (size.x * size.y) as usize;
        if b.len() != 16 + n * if v2 { 12 } else { 8 } {
            return None;
        }
        let mut o = 16;
        let heights = (0..n)
            .map(|i| f32::from_le_bytes([b[o + i * 4], b[o + i * 4 + 1], b[o + i * 4 + 2], b[o + i * 4 + 3]]))
            .collect();
        o += n * 4;
        let take = |o: &mut usize| {
            let v = b[*o..*o + n].to_vec();
            *o += n;
            v
        };
        let floor_tex = take(&mut o);
        let wall_tex = take(&mut o);
        let grass = take(&mut o);
        let smooth = take(&mut o);
        // PTC1 (older data): no water anywhere.
        let water = if v2 {
            (0..n).map(|i| f32::from_le_bytes([b[o + i * 4], b[o + i * 4 + 1], b[o + i * 4 + 2], b[o + i * 4 + 3]])).collect()
        } else {
            vec![NO_WATER; n]
        };
        Some(ChunkData { size, heights, floor_tex, wall_tex, grass, smooth, water, merge_mode })
    }
}

/// A complete sample read from the store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub height: f32,
    pub floor: u8,
    pub wall: u8,
    pub grass: u8,
    pub smooth: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TerrainStore {
    pub config: GridConfig,
    pub chunks: HashMap<IVec2, ChunkData>,
    pub default_floor: u8,
    pub default_wall: u8,
    /// How cell grids (triangle/hex) close height steps between cells (`cells::EdgeParams`).
    pub edges: crate::cells::EdgeParams,
}

impl TerrainStore {
    pub fn new(config: GridConfig) -> TerrainStore {
        TerrainStore { config, chunks: HashMap::new(), default_floor: 0, default_wall: 5, edges: Default::default() }
    }

    pub fn grid(&self) -> GridType {
        self.config.grid
    }

    pub fn add_chunk(&mut self, chunk: IVec2, merge: MergeMode) -> &mut ChunkData {
        let size = self.config.samples_per_chunk();
        let (f, w) = (self.default_floor, self.default_wall);
        self.chunks.entry(chunk).or_insert_with(|| ChunkData::new(size, f, w, merge))
    }

    /// Adds a chunk whose heights are generated from fractal noise (`amplitude` in height units).
    pub fn add_chunk_with_noise(&mut self, chunk: IVec2, merge: MergeMode, amplitude: f32, frequency: f32, seed: u32) {
        let cfg = self.config.clone();
        let data = self.add_chunk(chunk, merge);
        let s = data.size;
        for y in 0..s.y {
            for x in 0..s.x {
                let l = iv2(x, y);
                if !cfg.has_local(l) {
                    continue;
                }
                let p = cfg.sample_pos(cfg.global_of(chunk, l));
                let i = data.index(l);
                data.heights[i] = fbm(p * frequency, seed, 4) * amplitude;
            }
        }
    }

    /// Adds a chunk whose heights come from `height(global_sample, local_position)` (e.g. a
    /// Godot `Noise` resource, like the original's `noise_hmap`). Square border samples
    /// shared with existing neighbours are synced afterwards by the caller.
    pub fn add_chunk_with_heights(&mut self, chunk: IVec2, merge: MergeMode, mut height: impl FnMut(IVec2, Vec2) -> f32) {
        let cfg = self.config.clone();
        let data = self.add_chunk(chunk, merge);
        let s = data.size;
        for y in 0..s.y {
            for x in 0..s.x {
                let l = iv2(x, y);
                if !cfg.has_local(l) {
                    continue;
                }
                let g = cfg.global_of(chunk, l);
                let i = data.index(l);
                data.heights[i] = height(g, cfg.sample_pos(g));
            }
        }
    }

    pub fn has_chunk(&self, chunk: IVec2) -> bool {
        self.chunks.contains_key(&chunk)
    }

    pub fn sample(&self, s: IVec2) -> Option<Sample> {
        for (chunk, l) in self.config.sample_slots(s) {
            if !self.config.has_local(l) {
                continue;
            }
            if let Some(ch) = self.chunks.get(&chunk) {
                let i = ch.index(l);
                return Some(Sample {
                    height: ch.heights[i],
                    floor: ch.floor_tex[i],
                    wall: ch.wall_tex[i],
                    grass: ch.grass[i],
                    smooth: ch.smooth[i] != 0,
                });
            }
        }
        None
    }

    pub fn height(&self, s: IVec2) -> Option<f32> {
        self.sample(s).map(|x| x.height)
    }

    /// Stored water level of a sample (`None` = no chunk or no water).
    pub fn water(&self, s: IVec2) -> Option<f32> {
        for (chunk, l) in self.config.sample_slots(s) {
            if !self.config.has_local(l) {
                continue;
            }
            if let Some(ch) = self.chunks.get(&chunk) {
                return ch.water.get(ch.index(l)).copied().filter(|w| has_water(*w));
            }
        }
        None
    }

    /// Sets (`Some`) or clears a sample's water level; returns whether a chunk stores it.
    pub fn set_water(&mut self, s: IVec2, w: Option<f32>) -> bool {
        let v = w.filter(|w| has_water(*w)).unwrap_or(NO_WATER);
        !self
            .edit_sample(s, |c, i| {
                if c.water.len() != c.heights.len() {
                    c.water = vec![NO_WATER; c.heights.len()];
                }
                c.water[i] = v;
            })
            .is_empty()
    }

    /// Every sample with a water level: `(sample, level)`, sorted.
    pub fn water_cells(&self) -> Vec<(IVec2, f32)> {
        let mut out = Vec::new();
        for (c, d) in &self.chunks {
            if !d.has_any_water() {
                continue;
            }
            let size = d.size;
            for y in 0..size.y {
                for x in 0..size.x {
                    let l = iv2(x, y);
                    let w = d.water[d.index(l)];
                    if self.config.has_local(l) && has_water(w) {
                        out.push((self.config.global_of(*c, l), w));
                    }
                }
            }
        }
        out.sort_by_key(|(c, _)| (c.y, c.x));
        out
    }

    /// Applies `f` to every stored copy of sample `s`; returns the chunks touched.
    pub fn edit_sample(&mut self, s: IVec2, mut f: impl FnMut(&mut ChunkData, usize)) -> Vec<IVec2> {
        let mut touched = Vec::new();
        for (chunk, l) in self.config.sample_slots(s) {
            if !self.config.has_local(l) {
                continue;
            }
            if let Some(ch) = self.chunks.get_mut(&chunk) {
                let i = ch.index(l);
                f(ch, i);
                touched.push(chunk);
            }
        }
        touched
    }

    pub fn set_height(&mut self, s: IVec2, h: f32) -> bool {
        !self.edit_sample(s, |c, i| c.heights[i] = h).is_empty()
    }

    /// Chunks whose mesh depends on sample `s`: every chunk storing a copy plus, for cell
    /// modes, the chunks of edge neighbours (their walls/ramps read this cell's height).
    pub fn chunks_affected_by(&self, s: IVec2) -> Vec<IVec2> {
        let cfg = &self.config;
        let mut out: Vec<IVec2> = cfg.sample_slots(s).into_iter().map(|(c, _)| c).collect();
        if cfg.grid.is_cell_mode() {
            // Ramps inset a cell's top depending on its neighbours, and the edge strips of a
            // cell read its neighbour's insets: meshes depend on cells up to two steps away.
            for n in cfg.edge_neighbors(s) {
                for m in std::iter::once(n).chain(cfg.edge_neighbors(n)) {
                    let c = cfg.chunk_of(m);
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out.retain(|c| self.chunks.contains_key(c));
        out
    }

    /// Copies the shared border rows of square chunks from their owners so freshly added
    /// chunks line up with existing neighbours.
    pub fn sync_square_borders(&mut self, chunk: IVec2) {
        if self.config.grid != GridType::Square {
            return;
        }
        let size = self.config.samples_per_chunk();
        let mut writes = Vec::new();
        for y in 0..size.y {
            for x in 0..size.x {
                let l = iv2(x, y);
                let g = self.config.global_of(chunk, l);
                for (other, ol) in self.config.sample_slots(g) {
                    if other == chunk {
                        continue;
                    }
                    if let Some(o) = self.chunks.get(&other) {
                        let i = o.index(ol);
                        writes.push((l, o.heights[i], o.floor_tex[i], o.wall_tex[i], o.grass[i]));
                        break;
                    }
                }
            }
        }
        if let Some(c) = self.chunks.get_mut(&chunk) {
            for (l, h, f, w, gr) in writes {
                let i = c.index(l);
                c.heights[i] = h;
                c.floor_tex[i] = f;
                c.wall_tex[i] = w;
                c.grass[i] = gr;
            }
        }
    }

    pub fn chunk_coords(&self) -> Vec<IVec2> {
        let mut v: Vec<IVec2> = self.chunks.keys().copied().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_bytes_roundtrip() {
        let mut c = ChunkData::new(iv2(5, 4), 2, 7, MergeMode::Spherical);
        c.heights[3] = 4.5;
        c.grass[2] = 0;
        c.smooth[1] = 1;
        assert_eq!(ChunkData::from_bytes(&c.to_bytes()), Some(c));
        assert_eq!(ChunkData::from_bytes(b"nope"), None);
    }

    #[test]
    fn water_levels_round_trip_and_old_blobs_migrate() {
        let mut c = ChunkData::new(iv2(4, 4), 0, 5, MergeMode::Polyhedron);
        let dry = c.to_bytes();
        assert_eq!(&dry[0..4], b"PTC1", "no water: the old format, byte for byte");
        c.water[5] = 2.5;
        let wet = c.to_bytes();
        assert_eq!(&wet[0..4], b"PTC2");
        assert_eq!(wet.len(), dry.len() + 16 * 4);
        assert_eq!(ChunkData::from_bytes(&wet), Some(c.clone()));
        // A PTC1 blob loads with no water.
        let old = ChunkData::from_bytes(&dry).unwrap();
        assert!(!old.has_any_water() && old.water.len() == 16);
        // Store API (hex rings, wrapped lookups through sample_slots).
        let mut s = TerrainStore::new(GridConfig { grid: GridType::HexRings, ring_radius: 2, ..Default::default() });
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        assert!(s.set_water(iv2(1, -1), Some(3.0)));
        assert_eq!(s.water(iv2(1, -1)), Some(3.0));
        assert_eq!(s.water_cells(), vec![(iv2(1, -1), 3.0)]);
        assert!(s.set_water(iv2(1, -1), None));
        assert_eq!(s.water(iv2(1, -1)), None);
        assert!(!s.set_water(iv2(9, 9), Some(1.0)), "no chunk");
    }

    #[test]
    fn chunk_bytes_reject_bad_sizes() {
        let c = ChunkData::new(iv2(3, 3), 0, 5, MergeMode::Polyhedron);
        let mut b = c.to_bytes();
        b.pop();
        assert_eq!(ChunkData::from_bytes(&b), None);
        let mut b = c.to_bytes();
        b[4..8].copy_from_slice(&0i32.to_le_bytes());
        assert_eq!(ChunkData::from_bytes(&b), None);
        // Unknown merge modes fall back to the default instead of failing.
        let mut b = c.to_bytes();
        b[12] = 99;
        assert_eq!(ChunkData::from_bytes(&b).unwrap().merge_mode, MergeMode::Polyhedron);
    }

    #[test]
    fn heights_from_function_cover_owned_samples() {
        for grid in [GridType::Square, GridType::Hex, GridType::HexRings] {
            let mut s = TerrainStore::new(GridConfig { grid, chunk_size: 4, ring_radius: 2, ..Default::default() });
            s.add_chunk_with_heights(iv2(1, 0), MergeMode::Polyhedron, |g, p| g.x as f32 * 10.0 + g.y as f32 + p.x * 0.0);
            let cfg = s.config.clone();
            for g in cfg.chunk_cells(iv2(1, 0)) {
                assert_eq!(s.height(g), Some(g.x as f32 * 10.0 + g.y as f32), "{grid:?} {g:?}");
            }
        }
    }

    #[test]
    fn square_border_samples_affect_neighbours() {
        let mut s = TerrainStore::new(GridConfig { chunk_size: 4, ..Default::default() });
        for c in [iv2(0, 0), iv2(1, 0), iv2(0, 1), iv2(1, 1)] {
            s.add_chunk(c, MergeMode::Polyhedron);
        }
        assert!(s.set_height(iv2(4, 4), 3.0));
        assert_eq!(s.chunks[&iv2(0, 0)].heights[s.chunks[&iv2(0, 0)].index(iv2(4, 4))], 3.0);
        assert_eq!(s.chunks[&iv2(1, 1)].heights[0], 3.0);
        let mut a = s.chunks_affected_by(iv2(4, 4));
        a.sort();
        assert_eq!(a, vec![iv2(0, 0), iv2(0, 1), iv2(1, 0), iv2(1, 1)]);
    }
}
