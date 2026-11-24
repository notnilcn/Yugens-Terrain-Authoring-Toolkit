use crate::math_conversions::{from_iv, to_iv};
use godot::classes::file_access::CompressionMode;
use godot::classes::resource_loader::CacheMode;
use godot::classes::resource_saver::SaverFlags;
use godot::classes::{ArrayMesh, DirAccess, FileAccess, MultiMesh, ResourceLoader, ResourceSaver};
use godot::prelude::*;
use pt_core::terrain_store::{ChunkData, MergeMode};
use pt_core::godot_res::{self, ResourceFile};
use pt_core::grid::GridType;
use pt_core::math::{IVec2, iv2};
use pt_core::mst_import::{self, MstChunk};
pub const METADATA_FILE: &str = "metadata.res";
pub const MODE_NAMES: [&str; 4] = ["square", "triangle", "hex", "hex_rings"];
#[derive(GodotClass)]
#[class(tool, init, base = Resource)]
pub struct MarchingSquaresChunkData {
    #[export]
    pub chunk_coords: Vector2i,
    #[export]
    pub grid_type: i32,
    #[export]
    pub cell_count: Vector2i,
    #[export]
    pub chunk_size: i32,
    #[export]
    pub ring_radius: i32,
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
    #[export]
    pub grass_mask: PackedByteArray,
    #[export]
    pub smooth_map: PackedByteArray,
    #[export]
    pub water_map: PackedFloat32Array,
    #[export]
    pub cage: Option<Gd<ArrayMesh>>,
    #[export]
    pub collision_faces: PackedVector3Array,
    #[export]
    pub grass_multimesh: Option<Gd<MultiMesh>>,
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
            d.smooth_map = PackedByteArray::from(c.glide.as_slice());
            d.water_map = if c.has_any_water() {
                c.water.iter().copied().collect()
            } else {
                PackedFloat32Array::new()
            };
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
            floor_tex: bytes(&self.ground_texture_idx, 0)
                .into_iter()
                .map(|x| x.min(15))
                .collect(),
            wall_tex: bytes(&self.wall_texture_idx, 5)
                .into_iter()
                .map(|x| x.min(15))
                .collect(),
            grass: bytes(&self.grass_mask, 255),
            glide: bytes(&self.smooth_map, 0),
            water: if self.water_map.len() == n {
                self.water_map.to_vec()
            } else {
                vec![pt_core::terrain_store::NO_WATER; n]
            },
            merge_mode: MergeMode::from_u8(self.merge_mode.clamp(0, 4) as u8),
        })
    }
}
#[godot_api]
impl MarchingSquaresChunkData {}
pub struct BakedChunk {
    pub cage: Gd<ArrayMesh>,
    pub collision_faces: Option<PackedVector3Array>,
    pub grass: Option<Gd<MultiMesh>>,
    pub signature: i64,
}
pub struct LoadedChunk {
    pub coords: IVec2,
    pub contents: ChunkData,
    pub layout: [i32; 4],
    pub baked: Option<BakedChunk>,
    pub from_mst: bool,
}
pub fn mode_dir(dir: &str, grid: GridType) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), grid.moniker())
}
pub fn chunk_path(dir: &str, grid: GridType, c: IVec2) -> String {
    format!(
        "{}/{}/{}",
        mode_dir(dir, grid),
        mst_import::chunk_dir_name(c),
        METADATA_FILE
    )
}
pub fn list_chunks(mode_dir: &str) -> Vec<IVec2> {
    if !DirAccess::dir_exists_absolute(mode_dir) {
        return Vec::new();
    }
    let mut v: Vec<IVec2> = DirAccess::get_directories_at(mode_dir)
        .as_slice()
        .iter()
        .filter_map(|n| mst_import::parse_chunk_dir(&n.to_string()))
        .collect();
    v.sort();
    v
}
pub fn read_res_file(path: &str) -> Result<ResourceFile, String> {
    let bytes = FileAccess::get_file_as_bytes(path);
    if bytes.is_empty() {
        return Err(format!("{path}: cannot read file"));
    }
    let b = bytes.as_slice();
    if b.starts_with(b"RSCC") {
        let (mode, total, block_size, blocks) =
            godot_res::split_compressed(b).map_err(|e| format!("{path}: {e}"))?;
        let mut out = Vec::with_capacity(blocks.len());
        for (i, blk) in blocks.into_iter().enumerate() {
            let want = godot_res::block_len(total, block_size, i);
            if want == 0 {
                out.push(Vec::new());
                continue;
            }
            let packed = PackedByteArray::from(blk);
            let dec = packed
                .decompress(want, CompressionMode::from_ord(mode as i32))
                .map_err(|_| format!("{path}: decompression failed"))?;
            out.push(dec.to_vec());
        }
        let raw = godot_res::join_blocks(out, total).map_err(|e| format!("{path}: {e}"))?;
        godot_res::parse_decompressed(&raw).map_err(|e| format!("{path}: {e}"))
    } else {
        godot_res::parse(b).map_err(|e| format!("{path}: {e}"))
    }
}
fn is_mst_file(f: &ResourceFile) -> bool {
    f.script_class == "MSTChunkData"
        || f.ext_resources
            .iter()
            .any(|(_, p)| p.ends_with("mst_chunk_data.gd"))
}
pub fn load_chunk(path: &str, grid: GridType, coords: IVec2) -> Result<LoadedChunk, String> {
    let parsed = read_res_file(path)?;
    if is_mst_file(&parsed) {
        let c: MstChunk = mst_import::chunk_from_resource(&parsed, grid, coords)?;
        let (cs, rr) = c.layout(grid);
        let layout = [cs, rr, 0, 0];
        return Ok(LoadedChunk {
            coords: c.coords,
            contents: c.contents,
            layout,
            baked: None,
            from_mst: true,
        });
    }
    let res = ResourceLoader::singleton()
        .load_ex(path)
        .type_hint("MarchingSquaresChunkData")
        .cache_mode(CacheMode::IGNORE)
        .done()
        .ok_or_else(|| format!("{path}: cannot load"))?;
    let res = res
        .try_cast::<MarchingSquaresChunkData>()
        .map_err(|_| format!("{path}: not a MarchingSquaresChunkData"))?;
    let d = res.bind();
    let data = d
        .to_chunk()
        .ok_or_else(|| format!("{path}: inconsistent sample arrays"))?;
    let baked = d.cage.clone().map(|mesh| BakedChunk {
        cage: mesh,
        collision_faces: (!d.collision_faces.is_empty()).then(|| d.collision_faces.clone()),
        grass: d.grass_multimesh.clone(),
        signature: d.bake_signature,
    });
    Ok(LoadedChunk {
        coords: to_iv(d.chunk_coords),
        contents: data,
        layout: [
            d.chunk_size,
            d.ring_radius,
            d.wrap_chunks.x,
            d.wrap_chunks.y,
        ],
        baked,
        from_mst: false,
    })
}
fn mode_chunks(dir: &str, grid: GridType) -> (String, Vec<IVec2>) {
    let md = mode_dir(dir, grid);
    let coords = list_chunks(&md);
    if coords.is_empty() && grid == GridType::Square {
        let root = dir.trim_end_matches('/').to_string();
        let coords = list_chunks(&root);
        return (root, coords);
    }
    (md, coords)
}
pub fn has_mode_chunks(dir: &str, grid: GridType) -> bool {
    let (md, coords) = mode_chunks(dir, grid);
    coords.into_iter().any(|c| {
        FileAccess::file_exists(&format!(
            "{md}/{}/{METADATA_FILE}",
            mst_import::chunk_dir_name(c)
        ))
    })
}
pub fn load_mode(dir: &str, grid: GridType) -> (Vec<LoadedChunk>, Vec<String>) {
    let (md, coords) = mode_chunks(dir, grid);
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
    let dir = path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    if !DirAccess::dir_exists_absolute(&dir) {
        let e = DirAccess::make_dir_recursive_absolute(&dir);
        if e != godot::global::Error::OK {
            return e;
        }
    }
    ResourceSaver::singleton()
        .save_ex(res)
        .path(path)
        .flags(SaverFlags::COMPRESS)
        .done()
}
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
pub fn dir_size(path: &str) -> u64 {
    let p = path.trim_end_matches('/');
    if !DirAccess::dir_exists_absolute(p) {
        return 0;
    }
    let mut s = 0;
    for f in DirAccess::get_files_at(p).as_slice() {
        if let Some(fa) = FileAccess::open(
            &format!("{p}/{f}"),
            godot::classes::file_access::ModeFlags::READ,
        ) {
            s += fa.get_length();
        }
    }
    for d in DirAccess::get_directories_at(p).as_slice() {
        s += dir_size(&format!("{p}/{d}"));
    }
    s
}
pub fn import_mst_path(
    path: &str,
    grid_hint: GridType,
) -> Result<Vec<(GridType, [i32; 4], Vec<LoadedChunk>)>, String> {
    let p = path.trim_end_matches('/');
    let mut out = Vec::new();
    let mut errors = Vec::new();
    if p.ends_with(".res") || p.ends_with(".tres") {
        let folder = p.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let name = folder.rsplit('/').next().unwrap_or("");
        let coords = mst_import::parse_chunk_dir(name).unwrap_or(iv2(0, 0));
        let mode = folder
            .rsplit_once('/')
            .map(|(d, _)| d.rsplit('/').next().unwrap_or(""))
            .unwrap_or("");
        let grid = MODE_NAMES
            .iter()
            .position(|m| *m == mode)
            .map(|i| GridType::from_i64(i as i64))
            .unwrap_or(grid_hint);
        let c = load_chunk(p, grid, coords)?;
        let layout = c.layout;
        out.push((grid, layout, vec![c]));
        return Ok(out);
    }
    if !DirAccess::dir_exists_absolute(p) {
        return Err(format!("{p}: no such directory"));
    }
    let leaf = p.rsplit('/').next().unwrap_or("");
    let grids: Vec<(GridType, String)> = if let Some(i) = MODE_NAMES.iter().position(|m| *m == leaf)
    {
        let parent = p
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default();
        vec![(GridType::from_i64(i as i64), parent)]
    } else {
        (0..4)
            .map(|i| (GridType::from_i64(i), p.to_string()))
            .collect()
    };
    for (grid, dir) in grids {
        let (chunks, errs) = load_mode(&dir, grid);
        errors.extend(errs);
        if chunks.is_empty() {
            continue;
        }
        let layout = chunks[0].layout;
        let size = chunks[0].contents.size;
        let (ok, bad): (Vec<_>, Vec<_>) = chunks.into_iter().partition(|c| c.contents.size == size);
        if !bad.is_empty() {
            errors.push(format!(
                "{}: skipped {} chunk(s) with a different size",
                grid.moniker(),
                bad.len()
            ));
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
