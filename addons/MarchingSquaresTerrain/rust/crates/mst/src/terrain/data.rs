use crate::math_conversions::{from_iv, to_iv};
use godot::prelude::*;
use pt_core::terrain_store::{ChunkData, TerrainStore};
use pt_core::grid::GridType;
use super::grid_choice::TerrainGridType;
#[derive(GodotClass)]
#[class(tool, init, base = Resource)]
pub struct MarchingSquaresTerrainData {
    #[export(storage)]
    chunks: VarDictionary,
    #[export(storage)]
    layout: VarDictionary,
    dirty: bool,
    #[export(storage)]
    scatter: VarDictionary,
    #[export(storage)]
    cell_tints: VarDictionary,
    base: Base<Resource>,
}
impl MarchingSquaresTerrainData {
    fn mode_dict(&self, grid: GridType) -> VarDictionary {
        self.chunks
            .get(grid.moniker())
            .and_then(|v| v.try_to::<VarDictionary>().ok())
            .unwrap_or_default()
    }
    pub fn layout_of(&self, grid: GridType) -> Option<Vector4i> {
        self.layout
            .get(grid.moniker())
            .and_then(|v| v.try_to::<Vector4i>().ok())
    }
    pub fn take_unsaved_edits(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
    pub fn mark_unsaved_edits(&mut self) {
        self.dirty = true;
    }
    pub fn write_layout(&mut self, grid: GridType, l: Vector4i) {
        if self.layout_of(grid) != Some(l) {
            self.layout.set(grid.moniker(), l);
            self.dirty = true;
        }
    }
    pub fn grids_with_chunks(&self) -> Vec<GridType> {
        (0..4)
            .map(GridType::from_i64)
            .filter(|g| self.chunk_count(*g) > 0)
            .collect()
    }
    pub fn replace_mode(
        &mut self,
        grid: GridType,
        layout: [i32; 4],
        chunks: &[(pt_core::math::IVec2, &ChunkData)],
    ) {
        let mut d = VarDictionary::new();
        for (c, ch) in chunks {
            d.set(
                from_iv(*c),
                &PackedByteArray::from(ch.to_bytes().as_slice()),
            );
        }
        self.chunks.set(grid.moniker(), &d);
        self.layout.set(grid.moniker(), layout_vec(grid, layout));
        self.dirty = true;
    }
    pub fn chunk_count(&self, grid: GridType) -> usize {
        self.mode_dict(grid).len()
    }
    pub fn load_into(&self, store: &mut TerrainStore) -> usize {
        store.chunks.clear();
        let expected = store.config.samples_per_chunk();
        let mut bad = 0;
        for (k, v) in self.mode_dict(store.config.grid).iter_shared() {
            let (Ok(coord), Ok(bytes)) = (k.try_to::<Vector2i>(), v.try_to::<PackedByteArray>())
            else {
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
            godot_warn!(
                "MarchingSquaresTerrainData: skipped {bad} unreadable/mismatched chunk(s) for '{}'",
                store.config.grid.moniker()
            );
        }
        store.chunks.len()
    }
    pub fn write_chunk(
        &mut self,
        grid: GridType,
        coord: pt_core::math::IVec2,
        chunk: Option<&ChunkData>,
    ) {
        self.dirty = true;
        let mut d = self.mode_dict(grid);
        match chunk {
            Some(c) => {
                d.set(
                    from_iv(coord),
                    &PackedByteArray::from(c.to_bytes().as_slice()),
                );
            }
            None => {
                d.remove(from_iv(coord));
            }
        }
        self.chunks.set(grid.moniker(), &d);
        self.dirty = true;
    }
    fn clear_mode(&mut self, grid: GridType) {
        self.dirty = true;
        self.chunks.remove(grid.moniker());
        self.layout.remove(grid.moniker());
        self.scatter.remove(grid.moniker());
        self.cell_tints.remove(grid.moniker());
    }
    pub fn tint_bytes(&self, grid: GridType) -> Option<Vec<u8>> {
        self.cell_tints
            .get(grid.moniker())
            .and_then(|v| v.try_to::<PackedByteArray>().ok())
            .map(|b| b.to_vec())
    }
    pub fn write_tints(&mut self, grid: GridType, bytes: Option<&[u8]>) {
        let old = self.tint_bytes(grid);
        if old.as_deref() == bytes {
            return;
        }
        self.dirty = true;
        match bytes {
            Some(b) => {
                self.cell_tints
                    .set(grid.moniker(), &PackedByteArray::from(b));
            }
            None => {
                self.cell_tints.remove(grid.moniker());
            }
        }
    }
    pub fn scatter_bytes(&self, grid: GridType) -> Option<Vec<u8>> {
        self.scatter
            .get(grid.moniker())
            .and_then(|v| v.try_to::<PackedByteArray>().ok())
            .map(|b| b.to_vec())
    }
    pub fn write_scatter(&mut self, grid: GridType, bytes: Option<&[u8]>) {
        match bytes {
            Some(b) => {
                self.dirty = true;
                self.scatter.set(grid.moniker(), &PackedByteArray::from(b));
            }
            None => {
                self.dirty = true;
                self.scatter.remove(grid.moniker());
            }
        }
    }
}
#[godot_api]
impl MarchingSquaresTerrainData {
    #[func]
    fn get_chunk_count(&self, grid_type: TerrainGridType) -> i64 {
        self.chunk_count(grid_type.into()) as i64
    }
    #[func]
    fn clear_grid(&mut self, grid_type: TerrainGridType) {
        self.clear_mode(grid_type.into());
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    #[func]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    #[func]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    #[func]
    fn get_grid_layout(&self, grid_type: TerrainGridType) -> Vector4i {
        self.layout_of(grid_type.into())
            .unwrap_or(Vector4i::ZERO)
    }
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
                let list: Vec<_> = chunks.iter().map(|c| (c.coords, &c.contents)).collect();
                d.replace_mode(*grid, *layout, &list);
                godot_print!(
                    "MarchingSquaresTerrainData: imported {} {} chunk(s) (layout {:?}) from {path}",
                    list.len(),
                    grid.moniker(),
                    layout
                );
            }
        }
        Some(data)
    }
}
pub fn layout_vec(grid: GridType, l: [i32; 4]) -> Vector4i {
    let cs = if grid == GridType::HexRings && l[0] < 2 {
        32
    } else {
        l[0].clamp(2, 64)
    };
    let rr = if l[1] < 1 { 4 } else { l[1].clamp(1, 16) };
    Vector4i::new(cs, rr, l[2].max(0), l[3].max(0))
}
