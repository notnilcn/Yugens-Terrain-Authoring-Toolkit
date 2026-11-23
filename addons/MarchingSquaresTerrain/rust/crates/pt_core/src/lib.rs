pub mod atlas;
pub mod autotex;
pub mod brush;
pub mod cell_shader_data;
pub mod cells;
pub mod clipboard;
pub mod erosion;
pub mod fire;
pub mod flipbook;
pub mod godot_res;
pub mod grass;
pub mod heightmap;
pub mod hexmap;
pub mod mesh;
pub mod meta;
pub mod mst_import;
pub mod overlay;
pub mod palette;
pub mod pick;
pub mod prism;
pub mod scatter;
pub mod selection;
pub mod square;
pub mod stamp;
pub mod terrain_store;
pub mod waterfall;
pub mod wildflowers;
pub use pt_geom::{grid, math};
use terrain_store::TerrainStore;
use grid::GridType;
use math::IVec2;
use mesh::Soup;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Detail {
    #[default]
    Full,
    Low,
}
pub fn mesh_chunk(store: &TerrainStore, chunk: IVec2, blend: square::BlendMode) -> Option<Soup> {
    mesh_chunk_detail(store, chunk, blend, Detail::Full)
}
pub fn mesh_chunk_detail(
    store: &TerrainStore,
    chunk: IVec2,
    blend: square::BlendMode,
    detail: Detail,
) -> Option<Soup> {
    let data = store.chunks.get(&chunk)?;
    let mut soup = Soup::default();
    match (store.config.grid, detail) {
        (GridType::Square, _) => square::mesh_square_chunk_in(store, chunk, data, blend, &mut soup),
        (_, Detail::Low) => cells::mesh_cell_chunk_low(store, chunk, &mut soup),
        _ => cells::mesh_cell_chunk(store, chunk, &mut soup),
    }
    Some(soup)
}
