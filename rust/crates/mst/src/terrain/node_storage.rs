//! `MarchingSquaresTerrain` storage half: directory storage modes (Baked / Runtime, with
//! change tracking), the original-data importer entry points and the static-mesh baker.
//! Kept apart from `node.rs` (meshing / editing) so storage work does not touch it.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use godot::classes::{
    ArrayMesh, CollisionShape3D, ConcavePolygonShape3D, DirAccess, EditorInterface, FileAccess, MeshInstance3D, MultiMeshInstance3D,
    Node, Node3D, ProjectSettings, ResourceSaver, StaticBody3D,
};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::prelude::*;

use pt_core::data::ChunkData;
use pt_core::grid::GridType;
use pt_core::math::IVec2;
use pt_core::mst_import;

use super::MarchingSquaresTerrain;
use crate::globals;
use crate::terrain::data::MarchingSquaresTerrainData;
use crate::terrain::storage::{self, BakedChunk, MarchingSquaresChunkData};
use crate::util::*;

/// Bumped whenever the mesher's output for the same samples changes, so old baked meshes
/// are rebuilt instead of reused.
const BAKE_FORMAT: u64 = 1;

pub const STORAGE_BAKED: i32 = 1;
pub const STORAGE_RUNTIME: i32 = 2;

fn hash_chunk_set(chunks: &HashMap<IVec2, ChunkData>, neighbors: &[IVec2], c: IVec2, salt: i64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    salt.hash(&mut h);
    let mut ns: Vec<IVec2> = neighbors.iter().copied().filter(|n| chunks.contains_key(n)).collect();
    ns.sort();
    for k in std::iter::once(c).chain(ns) {
        (k.x, k.y).hash(&mut h);
        if let Some(d) = chunks.get(&k) {
            d.to_bytes().hash(&mut h);
        }
    }
    h.finish()
}

/// Internal child that forwards the editor's pre-save notification to the terrain. (The
/// terrain itself cannot override notifications: it receives some while it is bound, e.g.
/// child-order changes when chunk nodes are added.)
#[derive(GodotClass)]
#[class(tool, init, base = Node, internal)]
pub struct MarchingSquaresTerrainSaveHook {
    base: Base<Node>,
}

#[godot_api]
impl godot::classes::INode for MarchingSquaresTerrainSaveHook {
    fn on_notification(&mut self, what: godot::classes::notify::NodeNotification) {
        if what != godot::classes::notify::NodeNotification::EDITOR_PRE_SAVE {
            return;
        }
        let Some(parent) = self.base().get_parent() else { return };
        if let Ok(mut t) = parent.try_cast::<MarchingSquaresTerrain>() {
            let dir = t.bind().directory_mode();
            if dir {
                let _ = t.bind_mut().save_terrain_data();
            }
        }
    }
}

impl MarchingSquaresTerrain {
    pub(super) fn ensure_save_hook(&mut self) {
        if !is_editor() || self.base().has_node("MarchingSquaresTerrainSaveHook") {
            return;
        }
        let mut hook = MarchingSquaresTerrainSaveHook::new_alloc();
        hook.set_name("MarchingSquaresTerrainSaveHook");
        let mut this = self.to_gd();
        add_internal(&mut this, &hook);
    }

    pub fn directory_mode(&self) -> bool {
        self.storage_mode == STORAGE_BAKED || self.storage_mode == STORAGE_RUNTIME
    }

    /// Hash of every setting a baked mesh / grass multimesh depends on.
    pub(super) fn bake_signature(&self) -> i64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        BAKE_FORMAT.hash(&mut h);
        let cfg = &self.store.config;
        (cfg.grid as u8, cfg.chunk_size, cfg.ring_radius, cfg.wrap.x, cfg.wrap.y, cfg.wrap_period).hash(&mut h);
        [cfg.cell_size.x, cfg.cell_size.y, self.grass_size.x, self.grass_size.y, self.grass_ledge_threshold, self.grass_min_height].map(f32::to_bits).hash(&mut h);
        (self.blend_mode, self.grass_enabled, self.grass_subdivisions, self.noise_seed).hash(&mut h);
        self.grass_params().slot_has_grass.hash(&mut h);
        (h.finish() & 0x7fff_ffff_ffff_ffff) as i64
    }

    /// Content hash of a chunk file: its own samples + the bake signature (a Baked mesh that
    /// changed because a neighbour was edited is caught by `save_pending`).
    fn chunk_hash(&self, chunks: &HashMap<IVec2, ChunkData>, c: IVec2, salt: i64) -> u64 {
        hash_chunk_set(chunks, &[], c, salt)
    }

    /// Edited scene root (editor) or current scene / owner (game).
    fn scene_root(&self) -> Option<Gd<Node>> {
        if !self.base().is_inside_tree() {
            return None;
        }
        let tree = self.base().get_tree();
        let root = if is_editor() { tree.get_edited_scene_root() } else { tree.get_current_scene() };
        root.or_else(|| self.base().get_owner())
    }

    /// `[SceneDir]/[SceneName]_TerrainData/[NodeName]_[id]` (the original's layout).
    fn generate_data_directory(&self) -> Option<String> {
        let root = self.scene_root()?;
        let scene = root.get_scene_file_path().to_string();
        if scene.is_empty() {
            return None;
        }
        let (dir, file) = scene.rsplit_once('/')?;
        let name = file.rsplit_once('.').map(|(n, _)| n).unwrap_or(file);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.base().instance_id().to_i64(), godot::classes::Time::singleton().get_ticks_usec()).hash(&mut h);
        Some(format!("{dir}/{name}_TerrainData/{}_{:08x}", self.base().get_name(), h.finish() as u32))
    }

    /// Other terrains of the edited scene (never `self`).
    fn other_terrains(&self) -> Vec<Gd<Node>> {
        let Some(root) = self.scene_root() else { return Vec::new() };
        let me = self.base().instance_id();
        let mut v: Vec<Gd<Node>> = root.find_children_ex("*").type_("MarchingSquaresTerrain").owned(false).done().iter_shared().collect();
        if root.clone().try_cast::<MarchingSquaresTerrain>().is_ok() {
            v.push(root);
        }
        v.retain(|n| n.instance_id() != me);
        v
    }

    /// Directory modes: reads the chunks of `grid` from disk once per node lifetime (later
    /// reloads, e.g. switching grid types back and forth, keep the in-memory edits).
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
        if self.data.as_ref().is_some_and(|d| d.bind().chunk_count(grid) > 0) {
            return; // in-memory data (migrated / imported) wins; it is written on the next save
        }
        let (chunks, errors) = storage::load_mode(&dir, grid);
        for e in errors {
            godot_warn!("MarchingSquaresTerrain: {e}");
        }
        if chunks.is_empty() {
            return;
        }
        let layout = chunks[0].layout;
        let size = chunks[0].data.size;
        let chunks: Vec<_> = chunks.into_iter().filter(|c| c.data.size == size).collect();
        if self.data.is_none() {
            self.data = Some(MarchingSquaresTerrainData::new_gd());
        }
        let map: HashMap<IVec2, ChunkData> = chunks.iter().map(|c| (c.coords, c.data.clone())).collect();
        {
            let data = self.data.as_mut().expect("data");
            let mut d = data.bind_mut();
            let list: Vec<_> = chunks.iter().map(|c| (c.coords, &c.data)).collect();
            d.replace_mode(grid, layout, &list);
        }
        // Remember what is on disk so unchanged chunks are not rewritten.
        let mut hashes = HashMap::new();
        let mut legacy = 0;
        for c in chunks {
            if c.from_mst {
                legacy += 1;
                continue; // re-saved in this addon's format on the next save
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
            grid.name(),
            if legacy > 0 { format!(" ({legacy} original MSTChunkData file(s), converted on the next save)") } else { String::new() }
        );
        if is_editor() {
            self.ensure_unique_directory();
        }
    }

    /// Duplicated terrains share `data_directory`: the copy gets a fresh folder (written on
    /// the next save) so the two never overwrite each other.
    fn ensure_unique_directory(&mut self) {
        let mine = self.data_directory.to_string().trim_end_matches('/').to_string();
        let clash = self.other_terrains().iter().any(|t| t.get("data_directory").to::<GString>().to_string().trim_end_matches('/') == mine);
        if !clash {
            return;
        }
        // The first terrain in tree order keeps the folder.
        let me = self.to_gd().upcast::<Node>();
        let first = self
            .other_terrains()
            .into_iter()
            .filter(|t| t.get("data_directory").to::<GString>().to_string().trim_end_matches('/') == mine)
            .all(|t| me.is_greater_than(&t));
        if first {
            return;
        }
        godot_print!("MarchingSquaresTerrain: {} shares data_directory {mine} with another terrain; it gets its own folder on the next save.", self.base().get_name());
        self.data_directory = GString::new();
        for h in &mut self.saved_hash {
            h.clear();
        }
    }

    pub(super) fn apply_baked(&mut self, c: IVec2, b: BakedChunk, rebuilt: &mut Array<Vector2i>) {
        self.baked_loads += 1;
        self.ensure_nodes(c);
        let n = self.nodes.get_mut(&c).expect("chunk nodes");
        let faces = b.collision_faces.clone().unwrap_or_else(|| b.mesh.get_faces());
        n.mesh.set_mesh(&b.mesh);
        if faces.is_empty() {
            n.shape.set_shape(Gd::<godot::classes::Shape3D>::null_arg());
        } else {
            let mut shape = ConcavePolygonShape3D::new_gd();
            shape.set_backface_collision_enabled(true);
            shape.set_faces(&faces);
            n.shape.set_shape(&shape);
        }
        match &b.grass {
            Some(mm) => n.grass.set_multimesh(mm),
            None => n.grass.set_multimesh(Gd::<godot::classes::MultiMesh>::null_arg()),
        }
        rebuilt.push(from_iv(c));
    }

    fn chunk_file(&self, c: IVec2, data: &ChunkData, baked: bool, sig: i64) -> Gd<MarchingSquaresChunkData> {
        let cfg = &self.store.config;
        let layout = [cfg.chunk_size, cfg.ring_radius, cfg.wrap.x, cfg.wrap.y];
        let mut res = MarchingSquaresChunkData::from_chunk(c, cfg.grid, layout, data);
        if baked && let Some(n) = self.nodes.get(&c) {
            let mesh = n.mesh.get_mesh().and_then(|m| m.try_cast::<ArrayMesh>().ok());
            if let Some(mesh) = mesh {
                let mut r = res.bind_mut();
                r.mesh = Some(mesh);
                r.bake_signature = sig;
                if self.bake_collision
                    && let Some(shape) = n.shape.get_shape().and_then(|s| s.try_cast::<ConcavePolygonShape3D>().ok())
                {
                    r.collision_faces = shape.get_faces();
                }
                if self.bake_grass && self.grass_enabled {
                    r.grass_multimesh = n.grass.get_multimesh();
                } else if self.grass_enabled {
                    // Without baked grass the mesh cannot be reused (grass needs the soup).
                    r.bake_signature = 0;
                }
            }
        }
        res
    }

    /// Writes changed chunks of one grid; returns the number of files written.
    fn save_grid(&mut self, dir: &str, grid: GridType, chunks: &HashMap<IVec2, ChunkData>, live: bool) -> usize {
        let baked = live && self.storage_mode == STORAGE_BAKED;
        let sig = if baked { self.bake_signature() } else { 0 };
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
            if !pending && self.saved_hash[gi].get(&c) == Some(&h) && FileAccess::file_exists(&path) {
                continue;
            }
            res.bind_mut().base_mut().set_name(&format!("Chunk ({}, {})", c.x, c.y));
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

    /// Removes `<scene>_TerrainData/<Node>_<id>` folders no terrain of the scene uses.
    fn cleanup_orphaned_terrain_dirs(&self) {
        let Some(root) = self.scene_root() else { return };
        let scene = root.get_scene_file_path().to_string();
        let Some((dir, file)) = scene.rsplit_once('/') else { return };
        let name = file.rsplit_once('.').map(|(n, _)| n).unwrap_or(file);
        let td = format!("{dir}/{name}_TerrainData");
        if !DirAccess::dir_exists_absolute(&td) {
            return;
        }
        let mut active: Vec<String> = self.other_terrains().iter().map(|t| t.get("data_directory").to::<GString>().to_string()).collect();
        active.push(self.data_directory.to_string());
        let active: Vec<String> = active.into_iter().map(|s| s.trim_end_matches('/').to_string()).collect();
        for d in DirAccess::get_directories_at(&td).as_slice() {
            let d = d.to_string();
            // Only folders this addon (or the original) generated: `<name>_<8 hex digits>`.
            let generated = d.rsplit_once('_').is_some_and(|(_, id)| id.len() == 8 && id.chars().all(|c| c.is_ascii_hexdigit()));
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
            // Switching from the single resource (or between Baked / Runtime): the in-memory
            // chunks are authoritative and are written to the directory on the next save.
            if !was_dir
                && let Some(d) = &self.data
            {
                for g in d.bind().grids_with_chunks() {
                    self.dir_loaded[g as usize] = true;
                }
            }
            let g = self.grid() as usize;
            self.dir_loaded[g] = true;
            self.baked.clear();
        }
        if self.loaded && !self.directory_mode() && was_dir {
            if let Some(d) = &mut self.data {
                d.bind_mut().mark_dirty(); // it now has to be saved as a resource / with the scene
            }
        }
        self.base_mut().call_deferred("notify_property_list_changed", &[]);
        self.base_mut().call_deferred("update_configuration_warnings", &[]);
    }

    pub(super) fn storage_set_bake_grass(&mut self, v: bool) {
        self.bake_grass = v;
    }

    pub(super) fn storage_set_bake_collision(&mut self, v: bool) {
        self.bake_collision = v;
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
        // Pointing at a folder that already holds chunks loads it; otherwise the current
        // chunks are written there on the next save.
        let dir = self.data_directory.to_string();
        let has_data = (0..4).any(|g| !storage::list_chunks(&storage::mode_dir(&dir, GridType::from_i64(g))).is_empty())
            || !storage::list_chunks(&dir).is_empty();
        if has_data {
            self.dir_loaded = [false; 4];
            self.baked.clear();
            self.data = Some(MarchingSquaresTerrainData::new_gd());
            self.need_reload = true;
        }
    }

}

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Saves the terrain data now. Directory modes write changed chunk files (the editor does
    /// this automatically when the scene is saved); Resource mode saves the `data` resource
    /// if it is a file and has changes. Usable at runtime (in-game editors).
    #[func]
    pub fn save_terrain_data(&mut self) -> godot::global::Error {
        if !self.directory_mode() {
            let Some(mut data) = self.data.clone() else { return godot::global::Error::OK };
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
        // Meshes must be current for Baked files.
        self.update();
        if self.data_directory.is_empty() {
            match self.generate_data_directory() {
                Some(d) => {
                    self.data_directory = GString::from(d.as_str());
                    self.base_mut().call_deferred("notify_property_list_changed", &[]);
                }
                None => {
                    godot_warn!("MarchingSquaresTerrain: save the scene once so a data_directory can be generated (or set one).");
                    return godot::global::Error::ERR_UNCONFIGURED;
                }
            }
        }
        let dir = self.data_directory.to_string();
        let grid = self.grid();
        let before = storage::dir_size(&storage::mode_dir(&dir, grid));
        let live = self.store.chunks.clone();
        let mut written = self.save_grid(&dir, grid, &live, true);
        // Other grid types touched in this session are kept in the data resource.
        if let Some(data) = self.data.clone() {
            let others: Vec<GridType> = (0..4).map(GridType::from_i64).filter(|g| *g != grid && self.dir_loaded[*g as usize]).collect();
            for g in others {
                let Some(l) = data.bind().layout_of(g) else { continue };
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

    /// Chunk files written (or removed) by the last `save_terrain_data()` (0 = nothing
    /// changed). Diagnostics / tests.
    #[func]
    pub fn get_last_save_count(&self) -> i64 {
        self.last_save_count
    }

    /// Chunks whose mesh came from a Baked file instead of being meshed (diagnostics / tests).
    #[func]
    pub fn get_baked_load_count(&self) -> i64 {
        self.baked_loads
    }

    pub(super) fn tool_save_terrain_data(&mut self) {
        let _ = self.save_terrain_data();
    }

    /// Imports the original (GDScript) MarchingSquaresTerrain's terrain into this node:
    /// `path` is the original scene (`.tscn`: grid type, dimensions, cell size, hex radius,
    /// blend mode and the data directory are read from its terrain node), a terrain data
    /// directory, a mode folder or one `metadata.res`. `res://` paths inside an original
    /// scene resolve against that scene's project. Returns true on success. The previous
    /// data resource is replaced (the editor's import action is undoable).
    #[func]
    pub fn import_mst(&mut self, path: GString) -> bool {
        let p = path.to_string();
        let mut src = p.clone();
        let mut settings = None;
        if p.ends_with(".tscn") {
            let text = FileAccess::get_file_as_string(&p).to_string();
            let list = mst_import::parse_scene_settings(&text);
            let Some(s) = list.into_iter().next() else {
                godot_error!("MarchingSquaresTerrain.import_mst: no MarchingSquaresTerrain node in {p}");
                return false;
            };
            src = resolve_project_path(&p, &s.data_directory);
            if s.data_directory.is_empty() {
                godot_error!("MarchingSquaresTerrain.import_mst: {} in {p} has no data_directory (embedded data is not supported)", s.node_name);
                return false;
            }
            settings = Some(s);
        }
        let Some(data) = MarchingSquaresTerrainData::import_mst(GString::from(src.as_str())) else { return false };
        let grids = data.bind().grids_with_chunks();
        if let Some(s) = &settings {
            self.grid_type = s.grid_type as i32;
            self.cell_size = Vector2::new(s.cell_size.x.max(0.05), s.cell_size.y.max(0.05));
            self.blend_mode = s.blend_mode.clamp(0, 2);
            self.default_wall_slot = s.default_wall_texture.clamp(0, 15);
            self.grass_subdivisions = s.grass_subdivisions.clamp(1, 8);
            self.grass_size = Vector2::new(s.grass_size.x, s.grass_size.y);
            // Texture look: ground textures, albedos, grass sprites / colors / flags.
            let ts = crate::terrain::textures::MarchingSquaresTexturePreset::from_mst(&s.slots, |tex| import_texture(&p, tex));
            self.texture_set = Some(ts);
        }
        if !grids.contains(&self.grid()) && let Some(g) = grids.first() {
            self.grid_type = *g as i32;
        }
        self.data = Some(data);
        self.baked.clear();
        self.dir_loaded = [true; 4];
        for h in &mut self.saved_hash {
            h.clear();
        }
        self.need_reload = true;
        self.need_look = true;
        self.base_mut().call_deferred("notify_property_list_changed", &[]);
        self.update();
        true
    }

    pub(super) fn tool_import_mst(&mut self) {
        let path = self.import_path.to_string();
        if path.is_empty() {
            godot_warn!("MarchingSquaresTerrain: set import_path to the original scene (.tscn), its terrain data folder or a metadata.res first.");
            return;
        }
        if !is_editor() {
            self.import_mst(GString::from(path.as_str()));
            return;
        }
        let this = self.to_gd();
        let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else { return };
        let obj = this.clone().upcast::<Object>();
        ur.create_action("Import MST terrain data");
        ur.add_do_method(&obj, "import_mst", &[path.to_variant()]);
        // Field values directly: reading properties through `get` would re-enter this object.
        let old: [(&str, Variant); 8] = [
            ("data", self.data.to_variant()),
            ("texture_set", self.texture_set.to_variant()),
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
        ur.add_undo_method(&obj, "rebuild", &[]);
        // Deferred: the do-method needs `self` unbound.
        ur.call_deferred("commit_action", &[]);
    }

    /// Bakes the current chunks into plain nodes under a new `<name> (Baked)` sibling:
    /// `MeshInstance3D` per chunk with a self-contained copy of the terrain material
    /// (textures embedded), `StaticBody3D` collision and the grass multimeshes. The bake no
    /// longer depends on this node (delete or hide the terrain afterwards). Undoable in the
    /// editor. Returns the new root.
    #[func]
    pub fn bake_static_meshes(&mut self) -> Option<Gd<Node3D>> {
        self.update();
        let mut parent = self.base().get_parent()?;
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("{} (Baked)", self.base().get_name()));
        root.set_transform(self.base().get_transform());
        let ts = self.texture_set.clone()?;
        let ts = ts.bind();
        let tex = self.texture_array.clone();
        let mat = self.terrain_material.as_ref().and_then(|m| Some(m.duplicate_resource()));
        if let Some(mut m) = mat.clone() {
            if let Some(t) = &tex {
                m.set_shader_parameter("textures", &t.to_variant());
            }
            m.set_shader_parameter("tints", &ts.tints().to_variant());
            m.set_shader_parameter("scales", &ts.scales().to_variant());
            m.set_shader_parameter("blend_mode", &self.blend_mode.to_variant());
            m.set_shader_parameter("pt_flags", &object_flags(self.pixel_size, self.outline, self.outline_id, false).to_variant());
        }
        let gmat = self.grass_material.as_ref().and_then(|m| Some(m.duplicate_resource()));
        if let Some(mut m) = gmat.clone() {
            if let Some(t) = &tex {
                m.set_shader_parameter("ground_textures", &t.to_variant());
            }
            m.set_shader_parameter("ground_tints", &ts.tints().to_variant());
            m.set_shader_parameter("ground_scales", &ts.scales().to_variant());
            m.set_shader_parameter("grass_tints", &ts.grass_tints().to_variant());
            if let Some(a) = &self.grass_sprites {
                m.set_shader_parameter("grass_sprites", &a.to_variant());
            }
            if m.get_shader_parameter("wind_texture").is_nil()
                && let Some(w) = &self.wind_texture
            {
                m.set_shader_parameter("wind_texture", &w.to_variant());
                m.set_shader_parameter("use_wind_texture", &true.to_variant());
            }
            m.set_shader_parameter("pt_flags", &object_flags(self.grass_pixel_size, self.grass_outline, self.outline_id, false).to_variant());
        }
        drop(ts);
        let layers = self.render_layers | globals::meta_mask();
        let mut coords: Vec<IVec2> = self.nodes.keys().copied().collect();
        coords.sort();
        let mut all: Vec<Gd<Node>> = Vec::new();
        for c in coords {
            let n = &self.nodes[&c];
            // Instanced prism columns have no chunk mesh: bake the meshed HexPrism surface.
            let mesh = match n.mesh.get_mesh() {
                Some(m) => Some(m),
                None if self.columns_active() => pt_core::mesh_chunk(&self.store, c, pt_core::square::BlendMode(self.blend_mode.clamp(0, 2) as u8))
                    .and_then(|soup| build_array_mesh(&soup.build()))
                    .map(|m| m.upcast()),
                None => None,
            };
            let Some(mesh) = mesh else { continue };
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&format!("Chunk ({}, {})", c.x, c.y));
            mi.set_mesh(&mesh);
            mi.set_layer_mask(layers);
            mi.set_cast_shadows_setting(if self.cast_shadows { ShadowCastingSetting::ON } else { ShadowCastingSetting::OFF });
            if let Some(m) = &mat {
                mi.set_material_override(m);
            }
            root.add_child(&mi);
            all.push(mi.clone().upcast());
            if self.collision_enabled
                && let Some(shape) = n.shape.get_shape()
            {
                let mut body = StaticBody3D::new_alloc();
                body.set_name("Body");
                body.set_collision_layer(self.collision_layer);
                body.set_collision_mask(self.collision_mask);
                let mut cs = CollisionShape3D::new_alloc();
                cs.set_name("Shape");
                cs.set_shape(&shape);
                body.add_child(&cs);
                mi.add_child(&body);
                all.push(body.upcast());
                all.push(cs.upcast());
            }
            if self.grass_enabled
                && let Some(mm) = n.grass.get_multimesh()
                && mm.get_instance_count() > 0
            {
                let mut g = MultiMeshInstance3D::new_alloc();
                g.set_name("Grass");
                g.set_multimesh(&mm);
                g.set_layer_mask(layers);
                g.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                if let Some(m) = &gmat {
                    g.set_material_override(m);
                }
                mi.add_child(&g);
                all.push(g.upcast());
            }
        }
        let idx = self.base().get_index() + 1;
        let owner = self.scene_root();
        if is_editor() && let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() {
            let p = parent.clone().upcast::<Object>();
            let r = root.clone().upcast::<Object>();
            ur.create_action("Bake terrain static meshes");
            ur.add_do_method(&p, "add_child", &[root.to_variant(), true.to_variant()]);
            ur.add_do_method(&p, "move_child", &[root.to_variant(), idx.to_variant()]);
            if let Some(o) = &owner {
                ur.add_do_method(&r, "set_owner", &[o.to_variant()]);
                for n in &all {
                    ur.add_do_method(&n.clone().upcast::<Object>(), "set_owner", &[o.to_variant()]);
                }
            }
            ur.add_do_reference(&r);
            ur.add_undo_method(&p, "remove_child", &[root.to_variant()]);
            ur.call_deferred("commit_action", &[]);
        } else {
            parent.add_child(&root);
            parent.move_child(&root, idx);
            if let Some(o) = &owner {
                root.set_owner(o);
                for mut n in all {
                    n.set_owner(o);
                }
            }
        }
        Some(root)
    }

    pub(super) fn tool_bake_static(&mut self) {
        let _ = self.bake_static_meshes();
    }

    /// Geometry baker (the original's `MarchingSquaresGeometryBaker`): exports every chunk to
    /// a GLB whose meshes carry a per-chunk texture atlas with the terrain's slot textures,
    /// tints, blending, wall shading and ridge/ledge swaps baked in (`polygon_resolution`
    /// pixels per triangle pair, the original's `polygon_texture_resolution`), so the result
    /// renders correctly in any engine / DCC without the terrain shader. Void cells are cut
    /// out with alpha scissor. Returns a Godot error code.
    #[func]
    pub fn export_glb_baked(&mut self, path: GString, polygon_resolution: i32) -> godot::global::Error {
        use godot::classes::base_material_3d::{TextureFilter, Transparency};
        use godot::classes::image::Format;
        use godot::classes::mesh::{ArrayType, PrimitiveType};
        use godot::classes::{GltfDocument, GltfState, Image, ImageTexture, StandardMaterial3D};
        let Some(ts) = self.texture_set.clone() else { return godot::global::Error::ERR_UNCONFIGURED };
        let ts = ts.bind();
        let tex_size = ts.texture_size.clamp(8, 256);
        // CPU copies of the slot layers (RGBA8) + tints / scales.
        let layers: Vec<Vec<u8>> = (0..16).map(|i| ts.layer_image(i, tex_size).map(|img| img.get_data().to_vec()).unwrap_or_default()).collect();
        let tints: Vec<Color> = ts.tints().as_slice().to_vec();
        let scales: Vec<f32> = ts.scales().as_slice().to_vec();
        drop(ts);
        let world_size = self
            .terrain_material
            .as_ref()
            .map(|m| m.get_shader_parameter("texture_world_size"))
            .and_then(|v| v.try_to::<f32>().ok())
            .unwrap_or(8.0)
            .max(0.01);
        let sharp = self.terrain_material.as_ref().map(|m| m.get_shader_parameter("blend_sharpness")).and_then(|v| v.try_to::<f32>().ok()).unwrap_or(3.0);
        let wall_shade = self.terrain_material.as_ref().map(|m| m.get_shader_parameter("wall_shade")).and_then(|v| v.try_to::<f32>().ok()).unwrap_or(0.88);
        let ts_i = tex_size as usize;
        let sample = |slot: usize, u: f32, v: f32| -> [f32; 4] {
            if slot >= 15 {
                return [0.0; 4];
            }
            let sc = scales.get(slot).copied().unwrap_or(1.0);
            let Some(px) = layers.get(slot).filter(|l| l.len() >= ts_i * ts_i * 4) else { return [1.0, 0.0, 1.0, 1.0] };
            let x = ((u * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
            let y = ((v * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
            let i = (y * ts_i + x) * 4;
            let t = tints.get(slot).copied().unwrap_or(Color::WHITE);
            [px[i] as f32 / 255.0 * t.r, px[i + 1] as f32 / 255.0 * t.g, px[i + 2] as f32 / 255.0 * t.b, 1.0]
        };
        let triplanar = |slot: usize, p: [f32; 3], n: [f32; 3]| -> [f32; 4] {
            let (wx, wz) = (n[0].abs() + 0.001, n[2].abs() + 0.001);
            let s = wx + wz;
            let a = sample(slot, p[2] / world_size, p[1] / world_size);
            let b = sample(slot, p[0] / world_size, p[1] / world_size);
            std::array::from_fn(|k| a[k] * wx / s + b[k] * wz / s)
        };
        let blend = pt_core::square::BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let hard = self.blend_mode != 0;
        let res = polygon_resolution.clamp(4, 128) as u32;
        let mut root = Node3D::new_alloc();
        root.set_name("MarchingSquaresTerrainExport");
        for c in self.store.chunk_coords() {
            let Some(soup) = pt_core::mesh_chunk(&self.store, c, blend) else { continue };
            let a = soup.build();
            let tris = a.indices.len() / 3;
            if tris == 0 {
                continue;
            }
            let vid = |t: usize, k: usize| a.indices[t * 3 + k] as usize;
            let lerp = |t: usize, w: [f32; 3], f: &dyn Fn(usize) -> [f32; 4]| -> [f32; 4] {
                let (p0, p1, p2) = (f(vid(t, 0)), f(vid(t, 1)), f(vid(t, 2)));
                std::array::from_fn(|k| p0[k] * w[0] + p1[k] * w[1] + p2[k] * w[2])
            };
            let atlas = pt_core::atlas::bake(tris, res, 8192, |t, w| {
                let p = lerp(t, w, &|i| [a.positions[i][0], a.positions[i][1], a.positions[i][2], 0.0]);
                let n = lerp(t, w, &|i| [a.normals[i][0], a.normals[i][1], a.normals[i][2], 0.0]);
                let wt = lerp(t, w, &|i| a.custom1[i]);
                let uv = lerp(t, w, &|i| [a.uv[i][0], a.uv[i][1], 0.0, 0.0]);
                let m = a.custom0[vid(t, 0)];
                let mats = [m[0].round() as usize, m[1].round() as usize, m[2].round() as usize];
                let mut bw = [wt[0].max(0.0), wt[1].max(0.0), (1.0 - wt[0] - wt[1]).max(0.0)];
                if hard {
                    let k = if bw[0] >= bw[1] && bw[0] >= bw[2] { 0 } else if bw[1] >= bw[2] { 1 } else { 2 };
                    bw = [0.0; 3];
                    bw[k] = 1.0;
                } else {
                    bw = bw.map(|x| x.powf(1.0 + sharp));
                }
                let sum = (bw[0] + bw[1] + bw[2]).max(1e-4);
                let bw = bw.map(|x| x / sum);
                let void_w: f32 = (0..3).filter(|k| mats[*k] == 15).map(|k| bw[k]).sum();
                if void_w > 0.5 {
                    return [0.0; 4];
                }
                let pos = [p[0], p[1], p[2]];
                let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
                let mut col = [0.0f32; 4];
                let wall = wt[2] > 0.5;
                for k in 0..3 {
                    let s = if wall { triplanar(mats[k], pos, nn) } else { sample(mats[k], pos[0] / world_size, pos[2] / world_size) };
                    for j in 0..4 {
                        col[j] += s[j] * bw[k];
                    }
                }
                let rl = m[3].round() as usize;
                if wall || (!wall && (uv[0] >= 0.6 || uv[1] >= 0.6)) {
                    if !wall {
                        col = triplanar(rl, pos, [nn[0], 0.0, nn[2]]);
                    }
                    for j in 0..3 {
                        col[j] *= wall_shade;
                    }
                }
                col[3] = 1.0;
                col
            });
            let Some(atlas) = atlas else {
                godot_error!("MarchingSquaresTerrain.export_glb_baked: chunk {c:?} does not fit an 8192 px atlas at {res} px; lower polygon_resolution");
                continue;
            };
            let Some(img) = Image::create_from_data(atlas.size as i32, atlas.size as i32, false, Format::RGBA8, &PackedByteArray::from(atlas.rgba.as_slice())) else { continue };
            // De-indexed triangles with atlas UVs.
            let mut verts = PackedVector3Array::new();
            let mut norms = PackedVector3Array::new();
            let mut uvs = PackedVector2Array::new();
            for t in 0..tris {
                for k in 0..3 {
                    let i = vid(t, k);
                    verts.push(Vector3::new(a.positions[i][0], a.positions[i][1], a.positions[i][2]));
                    norms.push(Vector3::new(a.normals[i][0], a.normals[i][1], a.normals[i][2]));
                    let u = atlas.uvs[t * 3 + k];
                    uvs.push(Vector2::new(u.x, u.y));
                }
            }
            let mut arrays = VarArray::new();
            arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
            arrays.set(ArrayType::VERTEX.ord() as usize, &verts.to_variant());
            arrays.set(ArrayType::NORMAL.ord() as usize, &norms.to_variant());
            arrays.set(ArrayType::TEX_UV.ord() as usize, &uvs.to_variant());
            let mut mesh = ArrayMesh::new_gd();
            mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            let mut mat = StandardMaterial3D::new_gd();
            if let Some(t) = ImageTexture::create_from_image(&img) {
                mat.set_texture(godot::classes::base_material_3d::TextureParam::ALBEDO, &t);
            }
            mat.set_texture_filter(TextureFilter::NEAREST);
            mat.set_transparency(Transparency::ALPHA_SCISSOR);
            mesh.surface_set_material(0, &mat);
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&format!("Chunk ({}, {})", c.x, c.y));
            mi.set_mesh(&mesh);
            root.add_child(&mi);
            mi.set_owner(&root);
        }
        let mut doc = GltfDocument::new_gd();
        let state = GltfState::new_gd();
        let mut err = doc.append_from_scene(&root, &state);
        if err == godot::global::Error::OK {
            err = doc.write_to_filesystem(&state, &path);
        }
        root.free();
        err
    }
}

/// Resolves a `res://` path written in an original scene against that scene's project
/// (the first ancestor folder of the scene holding `project.godot`).
/// Loads a texture referenced by an original MST scene: the original's bundled assets map to
/// this addon's copies, other `res://` paths resolve against the original project (images are
/// read directly, other resources loaded by path).
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
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "tga" | "bmp" | "svg") {
        let img = Image::load_from_file(&abs)?;
        return ImageTexture::create_from_image(&img).map(|t| t.upcast());
    }
    godot::classes::ResourceLoader::singleton().load(&abs).and_then(|r| r.try_cast::<Texture2D>().ok())
}

fn resolve_project_path(scene: &str, path: &str) -> String {
    let Some(rest) = path.strip_prefix("res://") else { return path.to_string() };
    let abs = if scene.starts_with("res://") || scene.starts_with("user://") {
        ProjectSettings::singleton().globalize_path(scene).to_string()
    } else {
        scene.to_string()
    };
    let mut dir = abs.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
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
