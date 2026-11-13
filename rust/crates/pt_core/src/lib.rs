//! `pt_core` — the engine-free half of the Marching Squares Terrain addon.
//!
//! Everything here is plain Rust so it can be unit-tested with `cargo test` and reused by
//! tools: grid topologies, terrain sample storage, the marching-squares and cell meshers,
//! brush operations, grass scatter, palette LUT baking and the metadata packing contract
//! shared with the pixel renderer's shaders.

pub mod atlas;
pub mod autotex;
pub mod brush;
pub mod build;
pub mod cells;
pub mod celldata;
pub mod clipboard;
pub mod data;
pub mod erosion;
pub mod flipbook;
pub mod fire;
pub mod godot_res;
pub mod grass;
pub mod grid;
pub mod hexmap;
pub mod heightmap;
pub mod math;
pub mod mesh;
pub mod meta;
pub mod mst_import;
pub mod overlay;
pub mod palette;
pub mod pick;
pub mod prism;
pub mod scatter;
pub mod square;
pub mod stamp;
pub mod waterfall;
pub mod wildflowers;

use data::TerrainStore;
use grid::GridType;
use math::IVec2;
use mesh::Soup;

/// Level of detail of a meshed chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Detail {
    /// The full surface: ramps, blending, bands, prism bevels and grooves, grass.
    #[default]
    Full,
    /// Distant ring: flat cell tops and plain walls ([`cells::mesh_cell_chunk_low`]). Square
    /// chunks keep their marching-squares surface (the caller drops their grass).
    Low,
}

/// Meshes one chunk of any grid type. Returns `None` if the chunk does not exist.
pub fn mesh_chunk(store: &TerrainStore, chunk: IVec2, blend: square::BlendMode) -> Option<Soup> {
    mesh_chunk_detail(store, chunk, blend, Detail::Full)
}

/// [`mesh_chunk`] at a level of detail.
pub fn mesh_chunk_detail(store: &TerrainStore, chunk: IVec2, blend: square::BlendMode, detail: Detail) -> Option<Soup> {
    let data = store.chunks.get(&chunk)?;
    let mut soup = Soup::default();
    match (store.config.grid, detail) {
        (GridType::Square, _) => square::mesh_square_chunk_in(store, chunk, data, blend, &mut soup),
        (_, Detail::Low) => cells::mesh_cell_chunk_low(store, chunk, &mut soup),
        _ => cells::mesh_cell_chunk(store, chunk, &mut soup),
    }
    Some(soup)
}
