//! Directory storage — the original MarchingSquaresTerrain's `storage_mode` (Baked / Runtime)
//! with one `metadata.res` per chunk under `<data_directory>/<grid>/chunk_X_Y/`, plus the
//! importer for the original's `MSTChunkData` files.
//!
//! * **Runtime**: each file holds only the source samples; meshes, grass and collision are
//!   rebuilt when the scene loads (smallest files).
//! * **Baked**: the files also hold the chunk's `ArrayMesh` and, optionally, its collision
//!   faces and grass `MultiMesh`, which are used directly on load (no meshing) as long as the
//!   `bake_signature` still matches the terrain's mesh-affecting settings.
//!
//! The layout (folder names, `metadata.res`, field names) follows the original
//! (`resources/mst_data_handler.gd`), so the original's data directories can be pointed at
//! directly: their `MSTChunkData` files are read with the engine-free binary reader in
//! `pt_core::godot_res` (the GDScript class is not needed) and re-saved in this format.

use godot::classes::file_access::CompressionMode;
use godot::classes::resource_loader::CacheMode;
use godot::classes::resource_saver::SaverFlags;
use godot::classes::{ArrayMesh, DirAccess, FileAccess, MultiMesh, ResourceLoader, ResourceSaver};
use godot::prelude::*;

use pt_core::data::{ChunkData, MergeMode};
use pt_core::godot_res::{self, ResourceFile};
use pt_core::grid::GridType;
use pt_core::math::{IVec2, iv2};
use pt_core::mst_import::{self, MstChunk};

use crate::util::{from_iv, to_iv};

pub const METADATA_FILE: &str = "metadata.res";
pub const MODE_NAMES: [&str; 4] = ["square", "triangle", "hex", "hex_rings"];

/// One chunk on disk (`metadata.res`). Field names follow the original's `MSTChunkData`.
#[derive(GodotClass)]
#[class(tool, init, base = Resource)]
pub struct MarchingSquaresChunkData {
    #[export]
    pub chunk_coords: Vector2i,
    /// 0 square, 1 triangle, 2 hex, 3 hex rings.
    #[export]
    pub grid_type: i32,
    /// Bounding-box sample count of the chunk (columns, rows).
    #[export]
    pub cell_count: Vector2i,
    /// Structural layout the samples were authored with.
    #[export]
    pub chunk_size: i32,
    #[export]
    pub ring_radius: i32,
    /// Hex rings torus wrap in chunks.
    #[export]
    pub wrap_chunks: Vector2i,
    #[export]
    pub merge_mode: i32,
    #[export]
    pub height_map: PackedFloat32Array,
    #[export]
    pub ground_texture_idx: PackedByteArray,
    #[export]
    pub wall_texture_idx: PackedByteArray,
    /// Grass density 0..255.
    #[export]
    pub grass_mask: PackedByteArray,
    #[export]
    pub smooth_map: PackedByteArray,
    /// Water level per sample (empty = no water; older files have none).
    #[export]
    pub water_map: PackedFloat32Array,
    /// Baked mode: the visual mesh (no material; the terrain material is an override).
    #[export]
    pub mesh: Option<Gd<ArrayMesh>>,
    /// Baked mode + `bake_collision`.
    #[export]
    pub collision_faces: PackedVector3Array,
    /// Baked mode + `bake_grass`.
    #[export]
    pub grass_multimesh: Option<Gd<MultiMesh>>,
    /// Hash of the settings the baked mesh/grass was built with (0 = nothing baked).
    #[export]
    pub bake_signature: i64,
    base: Base<Resource>,
}

impl MarchingSquaresChunkData {
    pub fn from_chunk(coords: IVec2, grid: GridType, layout: [i32; 4], c: &ChunkData) -> Gd<Self> {
        let mut g = Self::new_gd();
        {
            let mut d = g.bind_mut();
            d.chunk_coords = from_iv(coords);
            d.grid_type = grid as i32;
            d.cell_count = Vector2i::new(c.size.x, c.size.y);
            d.chunk_size = layout[0];
            d.ring_radius = layout[1];
            d.wrap_chunks = Vector2i::new(layout[2], layout[3]);
            d.merge_mode = c.merge_mode as i32;
            d.height_map = c.heights.iter().copied().collect();
            d.ground_texture_idx = PackedByteArray::from(c.floor_tex.as_slice());
            d.wall_texture_idx = PackedByteArray::from(c.wall_tex.as_slice());
            d.grass_mask = PackedByteArray::from(c.grass.as_slice());
            d.smooth_map = PackedByteArray::from(c.smooth.as_slice());
            d.water_map = if c.has_any_water() { c.water.iter().copied().collect() } else { PackedFloat32Array::new() };
        }
        g
    }

    pub fn to_chunk(&self) -> Option<ChunkData> {
        let size = iv2(self.cell_count.x, self.cell_count.y);
        let n = (size.x.max(0) * size.y.max(0)) as usize;
        if n == 0 || self.height_map.len() != n {
            return None;
        }
        let bytes = |b: &PackedByteArray, d: u8| if b.len() == n { b.to_vec() } else { vec![d; n] };
        Some(ChunkData {
            size,
            heights: self.height_map.to_vec(),
            floor_tex: bytes(&self.ground_texture_idx, 0).into_iter().map(|x| x.min(15)).collect(),
            wall_tex: bytes(&self.wall_texture_idx, 5).into_iter().map(|x| x.min(15)).collect(),
            grass: bytes(&self.grass_mask, 255),
            smooth: bytes(&self.smooth_map, 0),
            water: if self.water_map.len() == n { self.water_map.to_vec() } else { vec![pt_core::data::NO_WATER; n] },
            merge_mode: MergeMode::from_u8(self.merge_mode.clamp(0, 4) as u8),
        })
    }
}

#[godot_api]
impl MarchingSquaresChunkData {}

/// Baked artefacts read from a chunk file.
pub struct BakedChunk {
    pub mesh: Gd<ArrayMesh>,
    pub collision_faces: Option<PackedVector3Array>,
    pub grass: Option<Gd<MultiMesh>>,
    pub signature: i64,
}

/// A chunk read from disk.
pub struct LoadedChunk {
    pub coords: IVec2,
    pub data: ChunkData,
    /// `[chunk_size, ring_radius, wrap_x, wrap_y]` the samples were authored with.
    pub layout: [i32; 4],
    pub baked: Option<BakedChunk>,
    /// Read from an original `MSTChunkData` file.
    pub from_mst: bool,
}

pub fn mode_dir(dir: &str, grid: GridType) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), grid.name())
}

pub fn chunk_path(dir: &str, grid: GridType, c: IVec2) -> String {
    format!("{}/{}/{}", mode_dir(dir, grid), mst_import::chunk_dir_name(c), METADATA_FILE)
}

/// Chunk folders (`chunk_X_Y`) present in a mode folder.
pub fn list_chunks(mode_dir: &str) -> Vec<IVec2> {
    if !DirAccess::dir_exists_absolute(mode_dir) {
        return Vec::new();
    }
    let mut v: Vec<IVec2> = DirAccess::get_directories_at(mode_dir).as_slice().iter().filter_map(|n| mst_import::parse_chunk_dir(&n.to_string())).collect();
    v.sort();
    v
}

/// Reads a `.res` file into the engine-free reader (decompressing `RSCC` with the engine).
pub fn read_res_file(path: &str) -> Result<ResourceFile, String> {
    let bytes = FileAccess::get_file_as_bytes(path);
    if bytes.is_empty() {
        return Err(format!("{path}: cannot read file"));
    }
    let b = bytes.as_slice();
    if b.starts_with(b"RSCC") {
        let (mode, total, block_size, blocks) = godot_res::split_compressed(b).map_err(|e| format!("{path}: {e}"))?;
        let mut out = Vec::with_capacity(blocks.len());
        for (i, blk) in blocks.into_iter().enumerate() {
            let want = godot_res::block_len(total, block_size, i);
            if want == 0 {
                out.push(Vec::new());
                continue;
            }
            let packed = PackedByteArray::from(blk);
            let dec = packed.decompress(want, CompressionMode::from_ord(mode as i32)).map_err(|_| format!("{path}: decompression failed"))?;
            out.push(dec.to_vec());
        }
        let raw = godot_res::join_blocks(out, total).map_err(|e| format!("{path}: {e}"))?;
        godot_res::parse_decompressed(&raw).map_err(|e| format!("{path}: {e}"))
    } else {
        godot_res::parse(b).map_err(|e| format!("{path}: {e}"))
    }
}

fn is_mst_file(f: &ResourceFile) -> bool {
    f.script_class == "MSTChunkData" || f.ext_resources.iter().any(|(_, p)| p.ends_with("mst_chunk_data.gd"))
}

/// Reads one chunk file: ours via the ResourceLoader (keeps baked meshes), an original
/// `MSTChunkData` via the binary reader (source data only; its baked meshes use the
/// original's vertex layout and are ignored).
pub fn load_chunk(path: &str, grid: GridType, coords: IVec2) -> Result<LoadedChunk, String> {
    let parsed = read_res_file(path)?;
    if is_mst_file(&parsed) {
        let c: MstChunk = mst_import::chunk_from_resource(&parsed, grid, coords)?;
        let (cs, rr) = c.layout(grid);
        let layout = [cs, rr, 0, 0];
        return Ok(LoadedChunk { coords: c.coords, data: c.data, layout, baked: None, from_mst: true });
    }
    let res = ResourceLoader::singleton()
        .load_ex(path)
        .type_hint("MarchingSquaresChunkData")
        .cache_mode(CacheMode::IGNORE)
        .done()
        .ok_or_else(|| format!("{path}: cannot load"))?;
    let res = res.try_cast::<MarchingSquaresChunkData>().map_err(|_| format!("{path}: not a MarchingSquaresChunkData"))?;
    let d = res.bind();
    let data = d.to_chunk().ok_or_else(|| format!("{path}: inconsistent sample arrays"))?;
    let baked = d.mesh.clone().map(|mesh| BakedChunk {
        mesh,
        collision_faces: (!d.collision_faces.is_empty()).then(|| d.collision_faces.clone()),
        grass: d.grass_multimesh.clone(),
        signature: d.bake_signature,
    });
    Ok(LoadedChunk { coords: to_iv(d.chunk_coords), data, layout: [d.chunk_size, d.ring_radius, d.wrap_chunks.x, d.wrap_chunks.y], baked, from_mst: false })
}

/// Loads every chunk of `grid` under `dir`. Legacy original layouts with square chunk
/// folders directly in `dir` are read as square chunks.
pub fn load_mode(dir: &str, grid: GridType) -> (Vec<LoadedChunk>, Vec<String>) {
    let mut md = mode_dir(dir, grid);
    let mut coords = list_chunks(&md);
    if coords.is_empty() && grid == GridType::Square {
        md = dir.trim_end_matches('/').to_string();
        coords = list_chunks(&md);
    }
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for c in coords {
        let p = format!("{md}/{}/{METADATA_FILE}", mst_import::chunk_dir_name(c));
        if !FileAccess::file_exists(&p) {
            continue;
        }
        match load_chunk(&p, grid, c) {
            Ok(l) => out.push(l),
            Err(e) => errors.push(e),
        }
    }
    (out, errors)
}

pub fn save_chunk(path: &str, res: &Gd<MarchingSquaresChunkData>) -> godot::global::Error {
    let dir = path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    if !DirAccess::dir_exists_absolute(&dir) {
        let e = DirAccess::make_dir_recursive_absolute(&dir);
        if e != godot::global::Error::OK {
            return e;
        }
    }
    ResourceSaver::singleton().save_ex(res).path(path).flags(SaverFlags::COMPRESS).done()
}

/// Deletes a directory tree (files and sub folders).
pub fn remove_tree(path: &str) {
    let p = path.trim_end_matches('/');
    if !DirAccess::dir_exists_absolute(p) {
        return;
    }
    for d in DirAccess::get_directories_at(p).as_slice() {
        remove_tree(&format!("{p}/{d}"));
    }
    for f in DirAccess::get_files_at(p).as_slice() {
        let _ = DirAccess::remove_absolute(&format!("{p}/{f}"));
    }
    let _ = DirAccess::remove_absolute(p);
}

/// Removes chunk folders of `grid` whose chunk no longer exists.
pub fn remove_orphans(dir: &str, grid: GridType, keep: &dyn Fn(IVec2) -> bool) -> usize {
    let md = mode_dir(dir, grid);
    let mut n = 0;
    for c in list_chunks(&md) {
        if !keep(c) {
            remove_tree(&format!("{md}/{}", mst_import::chunk_dir_name(c)));
            n += 1;
        }
    }
    n
}

/// Recursive size of a directory in bytes.
pub fn dir_size(path: &str) -> u64 {
    let p = path.trim_end_matches('/');
    if !DirAccess::dir_exists_absolute(p) {
        return 0;
    }
    let mut s = 0;
    for f in DirAccess::get_files_at(p).as_slice() {
        if let Some(fa) = FileAccess::open(&format!("{p}/{f}"), godot::classes::file_access::ModeFlags::READ) {
            s += fa.get_length();
        }
    }
    for d in DirAccess::get_directories_at(p).as_slice() {
        s += dir_size(&format!("{p}/{d}"));
    }
    s
}

/// Imports an original MarchingSquaresTerrain data source: a terrain data directory (with
/// `square/`, `triangle/`, `hex/`, `hex_rings/` folders or legacy root chunk folders), a
/// single mode folder, or one `metadata.res`. Returns `(grid, layout, chunks)` per grid.
pub fn import_mst_path(path: &str, grid_hint: GridType) -> Result<Vec<(GridType, [i32; 4], Vec<LoadedChunk>)>, String> {
    let p = path.trim_end_matches('/');
    let mut out = Vec::new();
    let mut errors = Vec::new();
    if p.ends_with(".res") || p.ends_with(".tres") {
        let folder = p.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let name = folder.rsplit('/').next().unwrap_or("");
        let coords = mst_import::parse_chunk_dir(name).unwrap_or(iv2(0, 0));
        let mode = folder.rsplit_once('/').map(|(d, _)| d.rsplit('/').next().unwrap_or("")).unwrap_or("");
        let grid = MODE_NAMES.iter().position(|m| *m == mode).map(|i| GridType::from_i64(i as i64)).unwrap_or(grid_hint);
        let c = load_chunk(p, grid, coords)?;
        let layout = c.layout;
        out.push((grid, layout, vec![c]));
        return Ok(out);
    }
    if !DirAccess::dir_exists_absolute(p) {
        return Err(format!("{p}: no such directory"));
    }
    let leaf = p.rsplit('/').next().unwrap_or("");
    let grids: Vec<(GridType, String)> = if let Some(i) = MODE_NAMES.iter().position(|m| *m == leaf) {
        // A mode folder itself.
        let parent = p.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        vec![(GridType::from_i64(i as i64), parent)]
    } else {
        (0..4).map(|i| (GridType::from_i64(i), p.to_string())).collect()
    };
    for (grid, dir) in grids {
        let (chunks, errs) = load_mode(&dir, grid);
        errors.extend(errs);
        if chunks.is_empty() {
            continue;
        }
        // The first chunk decides the layout; chunks of another size are dropped.
        let layout = chunks[0].layout;
        let size = chunks[0].data.size;
        let (ok, bad): (Vec<_>, Vec<_>) = chunks.into_iter().partition(|c| c.data.size == size);
        if !bad.is_empty() {
            errors.push(format!("{}: skipped {} chunk(s) with a different size", grid.name(), bad.len()));
        }
        out.push((grid, layout, ok));
    }
    for e in &errors {
        godot_warn!("MST import: {e}");
    }
    if out.is_empty() {
        return Err(format!("{p}: no MarchingSquaresTerrain chunk data found"));
    }
    Ok(out)
}
