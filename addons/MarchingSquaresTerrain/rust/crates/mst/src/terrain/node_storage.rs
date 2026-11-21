use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::math_conversions::from_iv;
use crate::terrain::data::MarchingSquaresTerrainData;
use crate::terrain::storage::{self, BakedChunk, MarchingSquaresChunkData};
use godot::classes::{
    ArrayMesh, DirAccess, EditorInterface, FileAccess, Node, ProjectSettings, ResourceSaver,
};
use godot::prelude::*;
use pt_core::data::ChunkData;
use pt_core::grid::GridType;
use pt_core::math::IVec2;
use pt_core::mst_import;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

const BAKE_FORMAT: u64 = 1;
pub const STORAGE_BAKED: i32 = 1;
pub const STORAGE_RUNTIME: i32 = 2;
fn hash_chunk_set(
    chunks: &HashMap<IVec2, ChunkData>,
    neighbors: &[IVec2],
    c: IVec2,
    salt: i64,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    salt.hash(&mut h);
    let mut ns: Vec<IVec2> = neighbors
        .iter()
        .copied()
        .filter(|n| chunks.contains_key(n))
        .collect();
    ns.sort();
    for k in std::iter::once(c).chain(ns) {
        (k.x, k.y).hash(&mut h);
        if let Some(d) = chunks.get(&k) {
            d.to_bytes().hash(&mut h);
        }
    }
    h.finish()
}
impl MarchingSquaresTerrain {
    pub fn directory_mode(&self) -> bool {
        self.storage_mode == STORAGE_BAKED || self.storage_mode == STORAGE_RUNTIME
    }
    pub(super) fn bake_signature(&self) -> i64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        BAKE_FORMAT.hash(&mut h);
        let cfg = &self.store.config;
        (
            cfg.grid as u8,
            cfg.chunk_size,
            cfg.ring_radius,
            cfg.wrap.x,
            cfg.wrap.y,
            cfg.wrap_period,
        )
            .hash(&mut h);
        [
            cfg.cell_size.x,
            cfg.cell_size.y,
            self.grass_size.x,
            self.grass_size.y,
            self.grass_ledge_threshold,
            self.grass_min_height,
        ]
        .map(f32::to_bits)
        .hash(&mut h);
        (
            self.blend_mode,
            self.grass_enabled,
            self.grass_subdivisions,
            self.noise_seed,
        )
            .hash(&mut h);
        self.grass_params().slot_has_grass.hash(&mut h);
        (h.finish() & 0x7fff_ffff_ffff_ffff) as i64
    }
    fn chunk_hash(&self, chunks: &HashMap<IVec2, ChunkData>, c: IVec2, salt: i64) -> u64 {
        hash_chunk_set(chunks, &[], c, salt)
    }
    pub(super) fn scene_root(&self) -> Option<Gd<Node>> {
        if !self.base().is_inside_tree() {
            return None;
        }
        let tree = self.base().get_tree();
        let root = if is_editor() {
            tree.get_edited_scene_root()
        } else {
            tree.get_current_scene()
        };
        root.or_else(|| self.base().get_owner())
    }
    fn generate_data_directory(&self) -> Option<String> {
        let root = self.scene_root()?;
        let scene = root.get_scene_file_path().to_string();
        if scene.is_empty() {
            return None;
        }
        let (dir, file) = scene.rsplit_once('/')?;
        let name = file.rsplit_once('.').map(|(n, _)| n).unwrap_or(file);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            self.base().instance_id().to_i64(),
            godot::classes::Time::singleton().get_ticks_usec(),
        )
            .hash(&mut h);
        Some(format!(
            "{dir}/{name}_TerrainData/{}_{:08x}",
            self.base().get_name(),
            h.finish() as u32
        ))
    }
    fn other_terrains(&self) -> Vec<Gd<Node>> {
        let Some(root) = self.scene_root() else {
            return Vec::new();
        };
        let me = self.base().instance_id();
        let mut v: Vec<Gd<Node>> = root
            .find_children_ex("*")
            .type_("MarchingSquaresTerrain")
            .owned(false)
            .done()
            .iter_shared()
            .collect();
        if root.clone().try_cast::<MarchingSquaresTerrain>().is_ok() {
            v.push(root);
        }
        v.retain(|n| n.instance_id() != me);
        v
    }
    pub(super) fn load_directory_grid(&mut self, grid: GridType) {
        let gi = grid as usize;
        if !self.directory_mode() || self.dir_loaded[gi] {
            return;
        }
        let dir = self.data_directory.to_string();
        if dir.is_empty() {
            return;
        }
        self.dir_loaded[gi] = true;
        if self
            .contents
            .as_ref()
            .is_some_and(|d| d.bind().chunk_count(grid) > 0)
        {
            return;
        }
        let (chunks, errors) = storage::load_mode(&dir, grid);
        for e in errors {
            godot_warn!("MarchingSquaresTerrain: {e}");
        }
        if chunks.is_empty() {
            return;
        }
        let layout = chunks[0].layout;
        let size = chunks[0].contents.size;
        let chunks: Vec<_> = chunks
            .into_iter()
            .filter(|c| c.contents.size == size)
            .collect();
        if self.contents.is_none() {
            self.contents = Some(MarchingSquaresTerrainData::new_gd());
        }
        let map: HashMap<IVec2, ChunkData> = chunks
            .iter()
            .map(|c| (c.coords, c.contents.clone()))
            .collect();
        {
            let Some(data) = self.contents.as_mut() else {
                return;
            };
            let mut d = data.bind_mut();
            let list: Vec<_> = chunks.iter().map(|c| (c.coords, &c.contents)).collect();
            d.replace_mode(grid, layout, &list);
        }
        let mut hashes = HashMap::new();
        let mut legacy = 0;
        for c in chunks {
            if c.from_mst {
                legacy += 1;
                continue;
            }
            let salt = c.baked.as_ref().map(|b| b.signature).unwrap_or(0);
            hashes.insert(c.coords, hash_chunk_set(&map, &[], c.coords, salt));
            if let Some(b) = c.baked {
                self.baked.insert(c.coords, b);
            }
        }
        self.saved_hash[gi] = hashes;
        godot_print!(
            "MarchingSquaresTerrain: loaded {} {} chunk(s) from {dir}{}",
            map.len(),
            grid.moniker(),
            if legacy > 0 {
                format!(" ({legacy} original MSTChunkData file(s), converted on the next save)")
            } else {
                String::new()
            }
        );
        if is_editor() {
            self.ensure_unique_directory();
        }
    }
    fn ensure_unique_directory(&mut self) {
        let mine = self
            .data_directory
            .to_string()
            .trim_end_matches('/')
            .to_string();
        let clash = self.other_terrains().iter().any(|t| {
            t.get("data_directory")
                .to::<GString>()
                .to_string()
                .trim_end_matches('/')
                == mine
        });
        if !clash {
            return;
        }
        let me = self.to_gd().upcast::<Node>();
        let first = self
            .other_terrains()
            .into_iter()
            .filter(|t| {
                t.get("data_directory")
                    .to::<GString>()
                    .to_string()
                    .trim_end_matches('/')
                    == mine
            })
            .all(|t| me.is_greater_than(&t));
        if first {
            return;
        }
        godot_print!(
            "MarchingSquaresTerrain: {} shares data_directory {mine} with another terrain; it gets its own folder on the next save.",
            self.base().get_name()
        );
        self.data_directory = GString::new();
        for h in &mut self.saved_hash {
            h.clear();
        }
    }
    pub(super) fn apply_baked(&mut self, c: IVec2, b: BakedChunk, rebuilt: &mut Array<Vector2i>) {
        self.baked_loads += 1;
        self.ensure_nodes(c);
        let faces = b
            .collision_faces
            .clone()
            .unwrap_or_else(|| b.cage.get_faces());
        self.set_chunk_mesh(
            c,
            crate::mesh_arrays::mesh_arrays_of(&b.cage.clone().upcast()),
        );
        self.assign_panel_impact(c, faces);
        let grass = b
            .grass
            .as_ref()
            .map(super::batches::grass_buffer)
            .unwrap_or_default();
        if let Some(n) = self.nodes.get_mut(&c) {
            n.grass = grass;
        }
        self.batches.tag(c);
        rebuilt.push(from_iv(c));
    }
    fn chunk_file(
        &self,
        c: IVec2,
        data: &ChunkData,
        baked: bool,
        sig: i64,
    ) -> Gd<MarchingSquaresChunkData> {
        let cfg = &self.store.config;
        let layout = [cfg.chunk_size, cfg.ring_radius, cfg.wrap.x, cfg.wrap.y];
        let mut res = MarchingSquaresChunkData::from_chunk(c, cfg.grid, layout, data);
        if baked
            && !self.columns_active()
            && let Some(n) = self.nodes.get(&c)
        {
            let mesh = self
                .chunk_mesh(c)
                .and_then(|m| m.try_cast::<ArrayMesh>().ok());
            if let Some(mesh) = mesh {
                let mut r = res.bind_mut();
                r.cage = Some(mesh);
                r.bake_signature = sig;
                if self.bake_impact
                    && let Some(shape) = n.form.as_ref()
                {
                    r.collision_faces = shape.get_faces();
                }
                if self.bake_grass && self.grass_enabled {
                    r.grass_multimesh = self.chunk_grass_multimesh(c);
                } else if self.grass_enabled {
                    r.bake_signature = 0;
                }
            }
        }
        res
    }
    fn save_grid(
        &mut self,
        dir: &str,
        grid: GridType,
        chunks: &HashMap<IVec2, ChunkData>,
        live: bool,
    ) -> usize {
        let baked = live && self.storage_mode == STORAGE_BAKED;
        let sig = if baked { self.bake_signature() } else { 0 };
        if baked {
            self.batches_flush_collision(true);
        }
        let gi = grid as usize;
        let mut coords: Vec<IVec2> = chunks.keys().copied().collect();
        coords.sort();
        let mut saved = 0;
        for c in coords {
            let path = storage::chunk_path(dir, grid, c);
            let mut res = self.chunk_file(c, &chunks[&c], baked, sig);
            let salt = res.bind().bake_signature;
            let h = self.chunk_hash(chunks, c, salt);
            let pending = live && baked && self.save_pending.contains(&c);
            if !pending && self.saved_hash[gi].get(&c) == Some(&h) && FileAccess::file_exists(&path)
            {
                continue;
            }
            res.bind_mut()
                .base_mut()
                .set_name(&format!("Chunk ({}, {})", c.x, c.y));
            let err = storage::save_chunk(&path, &res);
            if err == godot::global::Error::OK {
                self.saved_hash[gi].insert(c, h);
                saved += 1;
            } else {
                godot_error!("MarchingSquaresTerrain: could not save {path}: {err:?}");
            }
        }
        if live {
            self.save_pending.clear();
        }
        let removed = storage::remove_orphans(dir, grid, &|c| chunks.contains_key(&c));
        self.saved_hash[gi].retain(|c, _| chunks.contains_key(c));
        saved + removed
    }
    fn cleanup_orphaned_terrain_dirs(&self) {
        let Some(root) = self.scene_root() else {
            return;
        };
        let scene = root.get_scene_file_path().to_string();
        let Some((dir, file)) = scene.rsplit_once('/') else {
            return;
        };
        let name = file.rsplit_once('.').map(|(n, _)| n).unwrap_or(file);
        let td = format!("{dir}/{name}_TerrainData");
        if !DirAccess::dir_exists_absolute(&td) {
            return;
        }
        let mut active: Vec<String> = self
            .other_terrains()
            .iter()
            .map(|t| t.get("data_directory").to::<GString>().to_string())
            .collect();
        active.push(self.data_directory.to_string());
        let active: Vec<String> = active
            .into_iter()
            .map(|s| s.trim_end_matches('/').to_string())
            .collect();
        for d in DirAccess::get_directories_at(&td).as_slice() {
            let d = d.to_string();
            let generated = d
                .rsplit_once('_')
                .is_some_and(|(_, id)| id.len() == 8 && id.chars().all(|c| c.is_ascii_hexdigit()));
            let full = format!("{td}/{d}");
            if generated && !active.contains(&full) {
                storage::remove_tree(&full);
                godot_print!("MarchingSquaresTerrain: removed orphaned terrain data {full}");
            }
        }
    }
}
impl MarchingSquaresTerrain {
    pub(super) fn storage_set_storage_mode(&mut self, v: i32) {
        let v = v.clamp(0, 2);
        if v == self.storage_mode {
            return;
        }
        let was_dir = self.directory_mode();
        self.storage_mode = v;
        if self.loaded && self.directory_mode() {
            if !was_dir && let Some(d) = &self.contents {
                for g in d.bind().grids_with_chunks() {
                    self.dir_loaded[g as usize] = true;
                }
            }
            let g = self.grid() as usize;
            self.dir_loaded[g] = true;
            self.baked.clear();
        }
        if self.loaded && !self.directory_mode() && was_dir {
            if let Some(d) = &mut self.contents {
                d.bind_mut().mark_dirty();
            }
        }
        self.base_mut()
            .call_deferred("notify_property_list_changed", &[]);
        self.base_mut()
            .call_deferred("update_configuration_warnings", &[]);
    }
    pub(super) fn storage_set_bake_grass(&mut self, v: bool) {
        self.bake_grass = v;
    }
    pub(super) fn store_assign_bake_impact(&mut self, v: bool) {
        self.bake_impact = v;
    }
    pub(super) fn storage_set_data_directory(&mut self, v: GString) {
        let v = GString::from(v.to_string().trim_end_matches('/'));
        if v == self.data_directory {
            return;
        }
        self.data_directory = v;
        for h in &mut self.saved_hash {
            h.clear();
        }
        if !self.loaded || !self.directory_mode() {
            return;
        }
        let dir = self.data_directory.to_string();
        let has_data = (0..4).any(|g| {
            !storage::list_chunks(&storage::mode_dir(&dir, GridType::from_i64(g))).is_empty()
        }) || !storage::list_chunks(&dir).is_empty();
        if has_data {
            self.dir_loaded = [false; 4];
            self.baked.clear();
            self.contents = Some(MarchingSquaresTerrainData::new_gd());
            self.need_reload = true;
        }
    }
}
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn save_terrain_data(&mut self) -> godot::global::Error {
        if !self.directory_mode() {
            let Some(mut data) = self.contents.clone() else {
                return godot::global::Error::OK;
            };
            let path = data.get_path().to_string();
            if path.is_empty() || path.contains("::") || !data.bind().is_dirty() {
                return godot::global::Error::OK;
            }
            let err = ResourceSaver::singleton().save(&data);
            if err == godot::global::Error::OK {
                data.bind_mut().clear_dirty();
            }
            return err;
        }
        self.update();
        if self.data_directory.is_empty() {
            match self.generate_data_directory() {
                Some(d) => {
                    self.data_directory = GString::from(d.as_str());
                    self.base_mut()
                        .call_deferred("notify_property_list_changed", &[]);
                }
                None => {
                    godot_warn!(
                        "MarchingSquaresTerrain: save the scene once so a data_directory can be generated (or set one)."
                    );
                    return godot::global::Error::ERR_UNCONFIGURED;
                }
            }
        }
        let dir = self.data_directory.to_string();
        let grid = self.grid();
        let before = storage::dir_size(&storage::mode_dir(&dir, grid));
        let live = self.store.chunks.clone();
        let mut written = self.save_grid(&dir, grid, &live, true);
        if let Some(data) = self.contents.clone() {
            let others: Vec<GridType> = (0..4)
                .map(GridType::from_i64)
                .filter(|g| *g != grid && self.dir_loaded[*g as usize])
                .collect();
            for g in others {
                let Some(l) = data.bind().layout_of(g) else {
                    continue;
                };
                let mut cfg = self.store.config.clone();
                cfg.grid = g;
                cfg.chunk_size = l.x;
                cfg.ring_radius = l.y;
                cfg.wrap = pt_core::math::iv2(l.z, l.w);
                let mut st = pt_core::data::TerrainStore::new(cfg);
                data.bind().load_into(&mut st);
                written += self.save_grid(&dir, g, &st.chunks, false);
            }
        }
        self.last_save_count = written as i64;
        if written > 0 {
            let after = storage::dir_size(&storage::mode_dir(&dir, grid));
            godot_print!(
                "MarchingSquaresTerrain: saved {written} chunk file(s) to {dir} ({} mode): {} -> {} bytes",
                ["Resource", "Baked", "Runtime"][self.storage_mode as usize],
                before,
                after
            );
        }
        if is_editor() {
            self.cleanup_orphaned_terrain_dirs();
        }
        godot::global::Error::OK
    }
    #[func]
    pub fn get_last_save_count(&self) -> i64 {
        self.last_save_count
    }
    #[func]
    pub fn get_baked_load_count(&self) -> i64 {
        self.baked_loads
    }
    pub(super) fn tool_save_terrain_data(&mut self) {
        let _ = self.save_terrain_data();
    }
    #[func]
    pub fn import_mst(&mut self, path: GString) -> bool {
        let p = path.to_string();
        let mut src = p.clone();
        let mut settings = None;
        if p.ends_with(".tscn") {
            let text = FileAccess::get_file_as_string(&p).to_string();
            let list = mst_import::parse_scene_settings(&text);
            let Some(s) = list.into_iter().next() else {
                godot_error!(
                    "MarchingSquaresTerrain.import_mst: no MarchingSquaresTerrain node in {p}"
                );
                return false;
            };
            src = resolve_project_path(&p, &s.data_directory);
            if s.data_directory.is_empty() {
                godot_error!(
                    "MarchingSquaresTerrain.import_mst: {} in {p} has no data_directory (embedded data is not supported)",
                    s.vertex_node_moniker
                );
                return false;
            }
            settings = Some(s);
        }
        let Some(data) = MarchingSquaresTerrainData::import_mst(GString::from(src.as_str())) else {
            return false;
        };
        let grids = data.bind().grids_with_chunks();
        if let Some(s) = &settings {
            self.grid_type = s.grid_type as i32;
            self.cell_size = Vector2::new(s.cell_size.x.max(0.0505), s.cell_size.y.max(0.0505));
            self.blend_mode = s.blend_mode.clamp(0, 2);
            self.default_wall_slot = s.default_wall_texture.clamp(0, 15);
            self.grass_subdivisions = s.grass_subdivisions.clamp(1, 8);
            self.grass_size = Vector2::new(s.grass_size.x, s.grass_size.y);
            let ts =
                crate::terrain::textures::MarchingSquaresTexturePreset::from_mst(&s.slots, |tex| {
                    import_texture(&p, tex)
                });
            self.tex_assign = Some(ts);
        }
        if !grids.contains(&self.grid())
            && let Some(g) = grids.first()
        {
            self.grid_type = *g as i32;
        }
        self.contents = Some(data);
        self.baked.clear();
        self.dir_loaded = [true; 4];
        for h in &mut self.saved_hash {
            h.clear();
        }
        self.need_reload = true;
        self.need_look = true;
        self.base_mut()
            .call_deferred("notify_property_list_changed", &[]);
        self.update();
        true
    }
    pub(super) fn tool_import_mst(&mut self) {
        let path = self.import_path.to_string();
        if path.is_empty() {
            godot_warn!(
                "MarchingSquaresTerrain: set import_path to the original scene (.tscn), its terrain data folder or a metadata.res first."
            );
            return;
        }
        if !is_editor() {
            self.import_mst(GString::from(path.as_str()));
            return;
        }
        let this = self.to_gd();
        let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else {
            return;
        };
        let obj = this.clone().upcast::<Object>();
        ur.create_action("Import MST terrain data");
        ur.add_do_method(&obj, "import_mst", &[path.to_variant()]);
        let old: [(&str, Variant); 8] = [
            ("contents", self.contents.to_variant()),
            ("tex_assign", self.tex_assign.to_variant()),
            ("grid_type", self.grid_type.to_variant()),
            ("cell_size", self.cell_size.to_variant()),
            ("blend_mode", self.blend_mode.to_variant()),
            ("default_wall_slot", self.default_wall_slot.to_variant()),
            ("grass_subdivisions", self.grass_subdivisions.to_variant()),
            ("grass_size", self.grass_size.to_variant()),
        ];
        for (prop, v) in old {
            ur.add_undo_property(&obj, prop, &v);
        }
        ur.add_undo_method(&obj, "reassemble", &[]);
        ur.call_deferred("commit_action", &[]);
    }
}
fn import_texture(scene: &str, path: &str) -> Option<Gd<godot::classes::Texture2D>> {
    use godot::classes::{Image, ImageTexture, Texture2D};
    if let Some(ours) = mst_import::bundled_asset(path) {
        return try_load::<Texture2D>(ours).ok();
    }
    let abs = resolve_project_path(scene, path);
    if !FileAccess::file_exists(&abs) {
        godot_warn!("MarchingSquaresTerrain.import_mst: texture {path} not found ({abs})");
        return None;
    }
    let ext = abs.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "tga" | "bmp" | "svg"
    ) {
        let img = Image::load_from_file(&abs)?;
        return ImageTexture::create_from_image(&img).map(|t| t.upcast());
    }
    godot::classes::ResourceLoader::singleton()
        .load(&abs)
        .and_then(|r| r.try_cast::<Texture2D>().ok())
}
fn resolve_project_path(scene: &str, path: &str) -> String {
    let Some(rest) = path.strip_prefix("res://") else {
        return path.to_string();
    };
    let abs = if scene.starts_with("res://") || scene.starts_with("user://") {
        ProjectSettings::singleton()
            .globalize_path(scene)
            .to_string()
    } else {
        scene.to_string()
    };
    let mut dir = abs
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    while !dir.is_empty() {
        if FileAccess::file_exists(&format!("{dir}/project.godot")) {
            return format!("{dir}/{rest}");
        }
        match dir.rsplit_once('/') {
            Some((d, _)) => dir = d.to_string(),
            None => break,
        }
    }
    path.to_string()
}
