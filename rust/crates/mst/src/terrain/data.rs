//! `MarchingSquaresTerrainData` — the terrain's samples, stored outside the scene's node tree.
//!
//! Each grid mode keeps its own set of chunks (switching a terrain between square and hex
//! does not destroy the other mode's work). Chunks are compact versioned binary blobs
//! (`pt_core::data::ChunkData::to_bytes`), so saving the resource as `.res` keeps scenes small.

use godot::prelude::*;
use pt_core::data::{ChunkData, TerrainStore};
use pt_core::grid::GridType;

use crate::util::{from_iv, to_iv};

#[derive(GodotClass)]
#[class(tool, init, base = Resource)]
pub struct MarchingSquaresTerrainData {
    /// `{ "square" | "triangle" | "hex" | "hex_rings": { Vector2i: PackedByteArray } }`
    #[export(storage)]
    chunks: VarDictionary,
    /// Structural layout each mode was authored with: `"<mode>": Vector4i(chunk_size, ring_radius, wrap_x, wrap_y)`.
    #[export(storage)]
    layout: VarDictionary,
    /// Set by every write; cleared by `clear_dirty()` once the data was saved.
    dirty: bool,
    /// Scatter instances per grid mode: `"<mode>": PackedByteArray` (`pt_core::scatter::ScatterSet`).
    #[export(storage)]
    scatter: VarDictionary,
    /// Painted cell tints per grid mode: `"<mode>": PackedByteArray` (`pt_core::celldata::PaintedTints`).
    #[export(storage)]
    cell_tints: VarDictionary,
    base: Base<Resource>,
}

impl MarchingSquaresTerrainData {
    fn mode_dict(&self, grid: GridType) -> VarDictionary {
        self.chunks.get(grid.name()).and_then(|v| v.try_to::<VarDictionary>().ok()).unwrap_or_default()
    }

    pub fn layout_of(&self, grid: GridType) -> Option<Vector4i> {
        self.layout.get(grid.name()).and_then(|v| v.try_to::<Vector4i>().ok())
    }

    /// True when written since the last `take_unsaved_edits` (see `dirty`).
    pub fn take_unsaved_edits(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn mark_unsaved_edits(&mut self) {
        self.dirty = true;
    }

    pub fn write_layout(&mut self, grid: GridType, l: Vector4i) {
        if self.layout_of(grid) != Some(l) {
            self.layout.set(grid.name(), l);
            self.dirty = true;
        }
    }

    /// Grid types that have chunks, in GridType order.
    pub fn grids_with_chunks(&self) -> Vec<GridType> {
        (0..4).map(GridType::from_i64).filter(|g| self.chunk_count(*g) > 0).collect()
    }

    /// Replaces the chunks of `grid` with `chunks` (used by directory loading and imports).
    pub fn replace_mode(&mut self, grid: GridType, layout: [i32; 4], chunks: &[(pt_core::math::IVec2, &ChunkData)]) {
        let mut d = VarDictionary::new();
        for (c, ch) in chunks {
            d.set(from_iv(*c), &PackedByteArray::from(ch.to_bytes().as_slice()));
        }
        self.chunks.set(grid.name(), &d);
        self.layout.set(grid.name(), layout_vec(grid, layout));
        self.dirty = true;
    }

    pub fn chunk_count(&self, grid: GridType) -> usize {
        self.mode_dict(grid).len()
    }

    /// Loads every stored chunk of `grid` into `store` (replacing its chunks).
    pub fn load_into(&self, store: &mut TerrainStore) -> usize {
        store.chunks.clear();
        let expected = store.config.samples_per_chunk();
        let mut bad = 0;
        for (k, v) in self.mode_dict(store.config.grid).iter_shared() {
            let (Ok(coord), Ok(bytes)) = (k.try_to::<Vector2i>(), v.try_to::<PackedByteArray>()) else {
                bad += 1;
                continue;
            };
            match ChunkData::from_bytes(bytes.as_slice()) {
                Some(c) if c.size == expected => {
                    store.chunks.insert(to_iv(coord), c);
                }
                _ => bad += 1,
            }
        }
        if bad > 0 {
            godot_warn!("MarchingSquaresTerrainData: skipped {bad} unreadable/mismatched chunk(s) for '{}'", store.config.grid.name());
        }
        store.chunks.len()
    }

    /// Writes (or removes, when `chunk` is `None`) one chunk blob.
    pub fn write_chunk(&mut self, grid: GridType, coord: pt_core::math::IVec2, chunk: Option<&ChunkData>) {
        self.dirty = true;
        let mut d = self.mode_dict(grid);
        match chunk {
            Some(c) => {
                d.set(from_iv(coord), &PackedByteArray::from(c.to_bytes().as_slice()));
            }
            None => {
                d.remove(from_iv(coord));
            }
        }
        self.chunks.set(grid.name(), &d);
        self.dirty = true;
    }

    fn clear_mode(&mut self, grid: GridType) {
        self.dirty = true;
        self.chunks.remove(grid.name());
        self.layout.remove(grid.name());
        self.scatter.remove(grid.name());
        self.cell_tints.remove(grid.name());
    }

    /// Painted cell tint blob of a grid mode.
    pub fn tint_bytes(&self, grid: GridType) -> Option<Vec<u8>> {
        self.cell_tints.get(grid.name()).and_then(|v| v.try_to::<PackedByteArray>().ok()).map(|b| b.to_vec())
    }

    /// Writes (or removes, for `None`) the painted cell tints of a grid mode.
    pub fn write_tints(&mut self, grid: GridType, bytes: Option<&[u8]>) {
        let old = self.tint_bytes(grid);
        if old.as_deref() == bytes {
            return;
        }
        self.dirty = true;
        match bytes {
            Some(b) => {
                self.cell_tints.set(grid.name(), &PackedByteArray::from(b));
            }
            None => {
                self.cell_tints.remove(grid.name());
            }
        }
    }

    /// Scatter blob of a grid mode.
    pub fn scatter_bytes(&self, grid: GridType) -> Option<Vec<u8>> {
        self.scatter.get(grid.name()).and_then(|v| v.try_to::<PackedByteArray>().ok()).map(|b| b.to_vec())
    }

    /// Writes (or removes, for `None`) the scatter blob of a grid mode.
    pub fn write_scatter(&mut self, grid: GridType, bytes: Option<&[u8]>) {
        match bytes {
            Some(b) => {
                self.dirty = true;
                self.scatter.set(grid.name(), &PackedByteArray::from(b));
            }
            None => {
                self.dirty = true;
                self.scatter.remove(grid.name());
            }
        }
    }
}

#[godot_api]
impl MarchingSquaresTerrainData {
    /// Number of chunks stored for a grid type (0 square, 1 triangle, 2 hex, 3 hex rings).
    #[func]
    fn get_chunk_count(&self, grid_type: i64) -> i64 {
        self.chunk_count(GridType::from_i64(grid_type)) as i64
    }

    /// Deletes every chunk stored for a grid type (lets you change its chunk size / radius).
    #[func]
    fn clear_grid(&mut self, grid_type: i64) {
        self.clear_mode(GridType::from_i64(grid_type));
        self.base_mut().emit_changed();
    }

    /// True when chunks/layout changed since the last `clear_dirty()` (i.e. since it was
    /// saved or loaded). Savers can skip clean data.
    #[func]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marks the data as saved.
    #[func]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Forces the next save.
    #[func]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Structural layout of a grid type: `Vector4i(chunk_size, ring_radius, wrap_x, wrap_y)`
    /// (zero when the grid has no chunks).
    #[func]
    fn get_grid_layout(&self, grid_type: i64) -> Vector4i {
        self.layout_of(GridType::from_i64(grid_type)).unwrap_or(Vector4i::ZERO)
    }

    /// Imports the original (GDScript) MarchingSquaresTerrain's data: a terrain data
    /// directory (`..._TerrainData/<Node>_<uid>/` with `square/`, `hex/`, ... folders), one mode
    /// folder, or a single `metadata.res`. Absolute paths work, so the original project can
    /// be read in place. Returns null when nothing could be read.
    #[func]
    pub fn import_mst(path: GString) -> Option<Gd<MarchingSquaresTerrainData>> {
        let grids = match super::storage::import_mst_path(&path.to_string(), GridType::Square) {
            Ok(g) => g,
            Err(e) => {
                godot_error!("MarchingSquaresTerrainData.import_mst: {e}");
                return None;
            }
        };
        let mut data = MarchingSquaresTerrainData::new_gd();
        {
            let mut d = data.bind_mut();
            for (grid, layout, chunks) in &grids {
                let list: Vec<_> = chunks.iter().map(|c| (c.coords, &c.data)).collect();
                d.replace_mode(*grid, *layout, &list);
                godot_print!("MarchingSquaresTerrainData: imported {} {} chunk(s) (layout {:?}) from {path}", list.len(), grid.name(), layout);
            }
        }
        Some(data)
    }
}

/// Layout vector with sane values for fields the grid does not use.
pub fn layout_vec(grid: GridType, l: [i32; 4]) -> Vector4i {
    let cs = if grid == GridType::HexRings && l[0] < 2 { 32 } else { l[0].clamp(2, 64) };
    let rr = if l[1] < 1 { 4 } else { l[1].clamp(1, 16) };
    Vector4i::new(cs, rr, l[2].max(0), l[3].max(0))
}
