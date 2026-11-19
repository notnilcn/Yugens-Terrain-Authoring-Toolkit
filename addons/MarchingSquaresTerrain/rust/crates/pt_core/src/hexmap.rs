//! Cell maps: per-cell height / water level / biome (floor slot) data laid out as a texture over
//! a rectangle of cell coordinates, for game shaders and simulations (BitCraft-style: the
//! terrain uploads R16-like height, water and dominant-biome maps for a continuous window
//! around the player, and the shaders sample them instead of a heightfield mesh).
//!
//! A [`MapWindow`] is a rectangle of **grid sample coordinates**: axial `(q, r)` on hex rings
//! (texel `(x, y)` = cell `(origin.x + x, origin.y + y)`), offset `(col, row)` on the odd-r
//! hex grid. Every texel is read through the store, so a window crossing the torus seam reads
//! the wrapped cells (a continuous window, no false cliffs at the edge of the world).
//!
//! Texel layout of the combined map ([`build_cell_map`], an RGBA float texture):
//!
//! | Channel | Value |
//! |---|---|
//! | R | cell height (terrain-local units); 0 for missing cells |
//! | G | water surface level, [`NO_WATER`] when the cell has none |
//! | B | floor slot = biome = `textures` array slice (15 = void) |
//! | A | `wall_slot + 16 · grass`, or [`MISSING`] (-1) where no chunk stores the cell |

use crate::data::TerrainStore;
use crate::grid::{GridType, axial_to_offset, offset_to_axial};
use crate::math::*;

/// Water channel value of a cell without water (below any terrain).
pub const NO_WATER: f32 = -10000.0;
/// Alpha channel value of a cell no chunk stores.
pub const MISSING: f32 = -1.0;

/// A rectangle of grid sample coordinates (`size` texels, `origin` = texel (0, 0)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MapWindow {
    pub origin: IVec2,
    pub size: IVec2,
}

impl MapWindow {
    pub fn new(origin: IVec2, size: IVec2) -> MapWindow {
        MapWindow { origin, size: iv2(size.x.max(0), size.y.max(0)) }
    }
    pub fn len(&self) -> usize {
        (self.size.x.max(0) * self.size.y.max(0)) as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn contains(&self, c: IVec2) -> bool {
        let d = c - self.origin;
        d.x >= 0 && d.y >= 0 && d.x < self.size.x && d.y < self.size.y
    }
    /// Texel index of a cell (row-major), `None` outside.
    pub fn index(&self, c: IVec2) -> Option<usize> {
        self.contains(c).then(|| {
            let d = c - self.origin;
            (d.y * self.size.x + d.x) as usize
        })
    }
    /// Cell of a texel index.
    pub fn cell(&self, i: usize) -> IVec2 {
        let w = self.size.x.max(1) as usize;
        self.origin + iv2((i % w) as i32, (i / w) as i32)
    }
    pub fn cells(&self) -> impl Iterator<Item = IVec2> + '_ {
        (0..self.len()).map(|i| self.cell(i))
    }
    /// Smallest window holding every cell of `cells`, grown by `pad` on every side.
    pub fn bounding(cells: impl IntoIterator<Item = IVec2>, pad: i32) -> MapWindow {
        let (mut lo, mut hi) = (iv2(i32::MAX, i32::MAX), iv2(i32::MIN, i32::MIN));
        for c in cells {
            lo = iv2(lo.x.min(c.x), lo.y.min(c.y));
            hi = iv2(hi.x.max(c.x), hi.y.max(c.y));
        }
        if lo.x > hi.x {
            return MapWindow::default();
        }
        MapWindow::new(lo - iv2(pad, pad), hi - lo + iv2(1 + 2 * pad, 1 + 2 * pad))
    }
}

/// Window covering the hex disk of `radius` cells around `center` (hex rings: the axial
/// square `[q - R, q + R] × [r - R, r + R]`, which holds the disk; odd-r hex: the offset
/// rectangle holding it). Other grids: the plain square around the sample.
pub fn window_around(grid: GridType, center: IVec2, radius: i32) -> MapWindow {
    let r = radius.max(0);
    match grid {
        GridType::Hex => {
            let a = offset_to_axial(center);
            MapWindow::bounding(crate::grid::disk(a, r).into_iter().map(axial_to_offset), 0)
        }
        _ => MapWindow::new(center - iv2(r, r), iv2(2 * r + 1, 2 * r + 1)),
    }
}

/// Window over every cell of every chunk of the store, plus `pad` cells (neighbour lookups of
/// border cells, including across the torus seam).
pub fn chunks_window(store: &TerrainStore, pad: i32) -> MapWindow {
    let cfg = &store.config;
    MapWindow::bounding(store.chunks.keys().flat_map(|c| cfg.chunk_cells(*c)), pad)
}

/// One decoded texel of the combined map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellTexel {
    pub height: f32,
    pub water: f32,
    pub floor: u8,
    pub wall: u8,
    pub grass: bool,
}

/// Packs a texel (`None` = missing cell).
pub fn pack(t: Option<CellTexel>) -> [f32; 4] {
    match t {
        Some(t) => [t.height, t.water, t.floor as f32, t.wall as f32 + if t.grass { 16.0 } else { 0.0 }],
        None => [0.0, NO_WATER, 15.0, MISSING],
    }
}

/// Unpacks a texel written by [`pack`].
pub fn unpack(v: [f32; 4]) -> Option<CellTexel> {
    if v[3] < -0.5 {
        return None;
    }
    let a = v[3].round().max(0.0) as u32;
    Some(CellTexel { height: v[0], water: v[1], floor: v[2].round().clamp(0.0, 15.0) as u8, wall: (a % 16) as u8, grass: a >= 16 })
}

/// The combined RGBA map of a window (row-major texels, see the module docs). `water(cell)`
/// gives the water surface of a cell (`None` = dry).
pub fn build_cell_map(store: &TerrainStore, w: MapWindow, water: &dyn Fn(IVec2) -> Option<f32>) -> Vec<[f32; 4]> {
    w.cells()
        .map(|c| {
            pack(store.sample(c).map(|s| CellTexel {
                height: s.height,
                water: water(c).unwrap_or(NO_WATER),
                floor: s.floor,
                wall: s.wall,
                grass: s.grass != 0,
            }))
        })
        .collect()
}

/// Heights of a window (`missing` where no chunk stores the cell).
pub fn height_map(store: &TerrainStore, w: MapWindow, missing: f32) -> Vec<f32> {
    w.cells().map(|c| store.height(c).unwrap_or(missing)).collect()
}

/// Floor slots (biome / texture-array slice) of a window, 255 where missing.
pub fn biome_map(store: &TerrainStore, w: MapWindow) -> Vec<u8> {
    w.cells().map(|c| store.sample(c).map(|s| s.floor).unwrap_or(255)).collect()
}

/// Writes heights from a window-shaped buffer into the store (cells without a chunk and
/// non-finite values are skipped). Returns the chunks touched (remesh set).
pub fn apply_heights(store: &mut TerrainStore, w: MapWindow, values: &[f32]) -> Vec<IVec2> {
    let mut touched = Vec::new();
    for (i, v) in values.iter().enumerate().take(w.len()) {
        if !v.is_finite() {
            continue;
        }
        let c = w.cell(i);
        if store.height(c).is_some_and(|h| (h - v).abs() > 1e-6) && store.set_height(c, *v) {
            for t in store.chunks_affected_by(c) {
                if !touched.contains(&t) {
                    touched.push(t);
                }
            }
        }
    }
    touched
}

/// Writes floor slots (biomes) from a window-shaped buffer (values > 15 are skipped).
pub fn apply_biomes(store: &mut TerrainStore, w: MapWindow, values: &[u8]) -> Vec<IVec2> {
    let mut touched = Vec::new();
    for (i, v) in values.iter().enumerate().take(w.len()) {
        if *v > 15 {
            continue;
        }
        let c = w.cell(i);
        if store.sample(c).is_some_and(|s| s.floor != *v) {
            for t in store.edit_sample(c, |d, k| d.floor_tex[k] = *v) {
                if !touched.contains(&t) {
                    touched.push(t);
                }
            }
            for t in store.chunks_affected_by(c) {
                if !touched.contains(&t) {
                    touched.push(t);
                }
            }
        }
    }
    touched
}

/// Whether the window must move to keep `center` at least `margin` cells from its border
/// (recentre-on-demand for a window following the player).
pub fn needs_recenter(w: MapWindow, center: IVec2, margin: i32) -> bool {
    if w.is_empty() {
        return true;
    }
    let d = center - w.origin;
    let m = margin.max(0);
    d.x < m || d.y < m || d.x >= w.size.x - m || d.y >= w.size.y - m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::GridConfig;

    fn rings(wrap_period: i32) -> TerrainStore {
        let cfg = GridConfig { grid: GridType::HexRings, ring_radius: 2, cell_size: v2(6.0, 6.0), wrap_period, ..Default::default() };
        let mut s = TerrainStore::new(cfg);
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        s
    }

    #[test]
    fn window_indexing_round_trips() {
        let w = MapWindow::new(iv2(-3, 2), iv2(5, 4));
        assert_eq!(w.len(), 20);
        for i in 0..w.len() {
            assert_eq!(w.index(w.cell(i)), Some(i));
        }
        assert_eq!(w.index(iv2(2, 2)), None);
        let d = window_around(GridType::HexRings, iv2(4, -1), 3);
        assert_eq!((d.origin, d.size), (iv2(1, -4), iv2(7, 7)));
        for c in crate::grid::disk(iv2(4, -1), 3) {
            assert!(d.contains(c));
        }
        let o = window_around(GridType::Hex, iv2(5, 3), 2);
        for a in crate::grid::disk(offset_to_axial(iv2(5, 3)), 2) {
            assert!(o.contains(axial_to_offset(a)));
        }
    }

    #[test]
    fn cell_map_packs_height_water_biome() {
        let mut s = rings(0);
        s.set_height(iv2(0, 0), 3.5);
        s.edit_sample(iv2(1, 0), |d, k| {
            d.floor_tex[k] = 7;
            d.wall_tex[k] = 4;
            d.grass[k] = 1;
        });
        let w = chunks_window(&s, 1);
        assert_eq!((w.origin, w.size), (iv2(-3, -3), iv2(7, 7)));
        let map = build_cell_map(&s, w, &|c| (c == iv2(0, 0)).then_some(4.0));
        let t0 = unpack(map[w.index(iv2(0, 0)).unwrap()]).unwrap();
        assert_eq!((t0.height, t0.water), (3.5, 4.0));
        let t1 = unpack(map[w.index(iv2(1, 0)).unwrap()]).unwrap();
        assert_eq!((t1.floor, t1.wall, t1.grass, t1.water), (7, 4, true, NO_WATER));
        // Outside the chunk disk: missing.
        assert!(unpack(map[w.index(iv2(2, 2)).unwrap()]).is_none());
        assert_eq!(biome_map(&s, w)[w.index(iv2(1, 0)).unwrap()], 7);
    }

    #[test]
    fn upload_writes_heights_and_biomes() {
        let mut s = rings(0);
        let w = window_around(GridType::HexRings, iv2(0, 0), 1);
        let hs: Vec<f32> = (0..w.len()).map(|i| i as f32).collect();
        let t = apply_heights(&mut s, w, &hs);
        assert_eq!(t, vec![iv2(0, 0)]);
        assert_eq!(height_map(&s, w, -1.0), hs.iter().enumerate().map(|(i, h)| if s.height(w.cell(i)).is_some() { *h } else { -1.0 }).collect::<Vec<_>>());
        assert_eq!(s.height(iv2(0, 0)), Some(4.0));
        let bs = vec![9u8; w.len()];
        apply_biomes(&mut s, w, &bs);
        assert_eq!(s.sample(iv2(1, -1)).unwrap().floor, 9);
        // A second identical upload touches nothing.
        assert!(apply_heights(&mut s, w, &hs).is_empty());
    }

    #[test]
    fn window_across_the_torus_seam_reads_wrapped_cells() {
        // P = 19: the radius-2 patch (0, 0) is the whole world; every cell wraps onto it.
        let mut s = rings(19);
        s.set_height(iv2(0, 0), 2.0);
        let w = window_around(GridType::HexRings, iv2(19, 0), 1);
        let map = build_cell_map(&s, w, &|_| None);
        let t = unpack(map[w.index(iv2(19, 0)).unwrap()]).expect("wrapped cell exists");
        assert_eq!(t.height, 2.0);
        assert!(w.cells().all(|c| s.sample(c).is_some()), "no hole anywhere on a one-patch torus");
    }

    #[test]
    fn recenter_margin() {
        let w = window_around(GridType::HexRings, iv2(0, 0), 8);
        assert!(!needs_recenter(w, iv2(3, -3), 4));
        assert!(needs_recenter(w, iv2(6, 0), 4));
        assert!(needs_recenter(MapWindow::default(), iv2(0, 0), 0));
    }
}
