//! A piece of sculptable, paintable ground that a level author shapes by hand in the editor.
//!
//! Intent: the person placing this wants terrain they can raise, carve, smooth and paint
//! directly in the viewport, on whichever cell layout suits the level, and have it look
//! like the rest of the pixel-art world without building meshes themselves. Dropping one
//! into a scene and painting on it is meant to be the whole workflow; grass, props,
//! collision and overlays follow from what was painted.
//!
//! Responsibility: it owns the terrain's shape and paint, turning them into what is drawn
//! and what bodies collide with, and keeping that up to date as the author edits. It owns
//! how its data is stored and saved, and how it is exported for use elsewhere. It leaves
//! the editing tools and their interface to the editor side, the rules for meshing,
//! brushes, erosion and placement to plain code that knows nothing of the engine, and the
//! look of shared world effects such as water, sky and lighting to the nodes that provide
//! them; it only reads what it needs from the ones the scene author links it to.
//!
//! Why a node: the terrain has a place and a transform in the world, its drawn and
//! colliding parts must live in the tree beneath it, and it needs the engine's per-frame
//! and editor callbacks to rebuild what changed and to follow the camera.
//!
//! Why a new type: no built-in node edits a heightfield of this kind, and assembling the
//! result from built-in meshes and bodies in a scene would freeze what can only be decided
//! by what the author paints.
//!
//! Not exported: the working copy of the terrain decoded from its data resource, and the
//! painted props and baked results read with it, because the exported data already holds
//! them and a second stored copy could disagree. What it has learned from the nodes it is
//! linked to, such as where water lies, because that belongs to those nodes and is read
//! again when they change. Textures and lookups computed from its exported look settings, which are
//! caches of those settings. The nodes it generates, its records of what is out of date or
//! already saved, and its timers and last-seen camera position, which only mean something
//! while it is running. The chunk the editing tools have selected, because that choice
//! belongs to the editing session and the tools write it, not the scene author.
use super::data::MarchingSquaresTerrainData;
use super::grid_choice::{HexEdgeStyle, TerrainGridType};
use super::storage::BakedChunk;
use super::textures::MarchingSquaresTexturePreset;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::math_conversions::{from_iv, from_v3, to_iv, to_v2};
use crate::object_flags::object_flags_for;
use crate::shader_materials::shader_material;
use godot::classes::image::Format;
use godot::classes::{
    INode3D, Image, ImageTexture, Node3D, Noise, ShaderMaterial, Texture2D, Texture2DArray,
};
use godot::prelude::*;
use pt_core::brush::{self, Brush, BrushShape, Clamp, PaintTarget, Touched};
use pt_core::terrain_store::{ChunkData, MergeMode, TEXTURE_SLOTS, TerrainStore};
use pt_core::grass::{GrassInstance, GrassParams, WaterCull};
use pt_core::grid::{GridConfig, GridType};
use pt_core::math::{IVec2, Vec2 as PVec2, iv2, v2};
use pt_core::mesh::MeshArrays;
use pt_core::square::BlendMode;
use std::collections::{BTreeSet, HashMap, HashSet};

#[path = "batches.rs"]
mod batches;
#[path = "geometry_export.rs"]
mod geometry_export;
#[path = "hex_columns.rs"]
mod hex_columns;
#[path = "hex_maps.rs"]
mod hex_maps;
#[path = "node_storage.rs"]
mod node_storage;
#[path = "overlay.rs"]
mod overlay;
#[path = "save_hook.rs"]
mod save_hook;
use batches::ChunkNodes;
struct Built {
    coord: IVec2,
    arrays: MeshArrays,
    grass: Vec<GrassInstance>,
    columns: bool,
    render: Option<MeshArrays>,
    instances: Option<(batches::ChunkColumns, Vec<(IVec2, [[f32; 4]; 6])>)>,
}
const SEA_PAD: f32 = 16.0;
const SEA_FALLOFF: f32 = 0.6005;
const MAX_SLOT: u8 = TEXTURE_SLOTS as u8 - 1;
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct MarchingSquaresTerrain {
    #[export]
    #[var(set = set_data)]
    pub(super) contents: Option<Gd<MarchingSquaresTerrainData>>,
    #[export]
    #[var(set = set_grid_type)]
    grid_type: TerrainGridType,
    #[export(range = (2.0, 64.0, 1.0))]
    #[var(set = set_chunk_size)]
    chunk_size: i32,
    #[export]
    #[var(set = set_cell_size)]
    cell_size: Vector2,
    #[export]
    #[var(set = set_texture_set)]
    tex_assign: Option<Gd<MarchingSquaresTexturePreset>>,
    #[export_group(name = "Grid")]
    #[export_subgroup(name = "Hex Rings")]
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = set_ring_radius)]
    ring_radius: i32,
    #[export]
    #[var(set = set_wrap_chunks)]
    wrap_chunks: Vector2i,
    #[export]
    #[var(set = set_wrap_period)]
    wrap_period: i32,
    #[export_subgroup(name = "Levels", prefix = "level_")]
    #[export]
    #[var(set = set_level_height)]
    level_height: f32,
    #[export_subgroup(name = "Hex Edges", prefix = "hex_edge_")]
    #[export]
    #[var(set = set_hex_edge_style)]
    hex_edge_style: HexEdgeStyle,
    #[export]
    #[var(set = set_hex_edge_instanced)]
    hex_edge_instanced: bool,
    #[export(range = (0.0, 64.0, 1.0))]
    #[var(set = set_hex_edge_grooves)]
    hex_edge_grooves: i32,
    #[export_subgroup(name = "Blending", prefix = "blend_")]
    #[export(enum = (Smooth, HardSquares, HardVertices))]
    #[var(set = set_blend_mode)]
    blend_mode: i32,
    #[export_subgroup(name = "Defaults", prefix = "default_")]
    #[export(enum = (Cubic, Polyhedron, RoundedPolyhedron, SemiRound, Spherical))]
    default_merge_mode: i32,
    #[export(range = (0.0, 15.0, 1.0))]
    #[var(set = set_default_floor_slot)]
    default_floor_slot: i32,
    #[export(range = (0.0, 15.0, 1.0))]
    #[var(set = set_default_wall_slot)]
    default_wall_slot: i32,
    #[export_subgroup(name = "New Chunk Noise", prefix = "new_chunk_noise_")]
    #[export]
    new_chunk_noise_amplitude: f32,
    #[export]
    new_chunk_noise_frequency: f32,
    #[export_subgroup(name = "New Chunk Noise", prefix = "noise_")]
    #[export]
    noise_seed: i64,
    #[export]
    noise_hmap: Option<Gd<Noise>>,
    #[export]
    noise_hmap_height: f32,
    #[export_group(name = "Look")]
    #[export_subgroup(name = "Terrain")]
    #[export]
    #[var(set = assign_land_stuff)]
    land_stuff: Option<Gd<ShaderMaterial>>,
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    #[var(set = set_pixel_size)]
    pixel_size: i32,
    #[export]
    #[var(set = set_outline)]
    outline: bool,
    #[export(range = (0.0, 253.0, 1.0))]
    #[var(set = set_outline_id)]
    outline_id: i32,
    #[export(flags_3d_render)]
    #[var(set = set_render_layers)]
    pub(super) render_layers: u32,
    #[export]
    cast_shadows: bool,
    #[export_subgroup(name = "Grass", prefix = "grass_")]
    #[export]
    #[var(set = set_grass_enabled)]
    grass_enabled: bool,
    #[export(range = (1.0, 8.0, 1.0))]
    #[var(set = set_grass_subdivisions)]
    grass_subdivisions: i32,
    #[export]
    #[var(set = set_grass_size)]
    grass_size: Vector2,
    #[export]
    #[var(set = set_grass_texture)]
    grass_texture: Option<Gd<Texture2D>>,
    #[export]
    #[var(set = assign_turf_stuff)]
    turf_stuff: Option<Gd<ShaderMaterial>>,
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    #[var(set = set_grass_pixel_size)]
    grass_pixel_size: i32,
    #[export]
    #[var(set = set_grass_outline)]
    grass_outline: bool,
    #[export(range = (0.0, 1.0))]
    #[var(set = set_grass_ledge_threshold)]
    grass_ledge_threshold: f32,
    #[export]
    #[var(set = set_grass_min_height)]
    grass_min_height: f32,
    #[export_subgroup(name = "Foliage", prefix = "scatter_")]
    #[export]
    pub(super) scatter_items: Array<Option<Gd<super::scatter::PixelScatterItem>>>,
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = set_scatter_group_tiles)]
    pub(super) scatter_group_tiles: i32,
    #[export(range = (0.0, 5000.0, 1.0, or_greater))]
    pub(super) scatter_body_quiet_ms: i32,
    #[export(range = (0.0, 5000.0, 1.0, or_greater))]
    pub(super) scatter_follow_interval_ms: i32,
    #[export_group(name = "Overlay")]
    #[export_subgroup(name = "Grid", prefix = "show_")]
    #[export]
    pub show_grid: bool,
    #[export]
    pub show_grid_in_editor: bool,
    #[export_subgroup(name = "Grid", prefix = "grid_")]
    #[export]
    pub grid_color: Color,
    #[export(range = (0.0055, 2.0, 0.0055))]
    pub grid_width: f32,
    #[export(range = (0.0, 2000.0, 1.0))]
    pub grid_fade_distance: f32,
    #[export]
    pub grid_chunk_lines: bool,
    #[export_subgroup(name = "Debug Views")]
    #[export]
    pub shader_cursor: bool,
    #[export(enum = (Off, Slope, HeightContours, SlotIds, ChunkBorders, CellIds, CellData))]
    pub debug_view: i32,
    #[export(range = (0.0505, 100.0, 0.0505))]
    pub contour_step: f32,
    #[export_subgroup(name = "Fog Of War", prefix = "fog_")]
    #[export]
    pub fog_enabled: bool,
    #[export]
    pub fog_in_editor: bool,
    #[export(range = (0.0, 5.0, 0.0105))]
    pub fog_transition: f32,
    #[export]
    pub fog_color: Color,
    #[export]
    pub fog_color_alt: Color,
    #[export_subgroup(name = "Fog: Hidden Cells", prefix = "fog_hidden_")]
    #[export(range = (0.0, 1.0, 0.0105))]
    pub fog_hidden_dim: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    pub fog_hidden_saturation: f32,
    #[export_subgroup(name = "Fog Of War", prefix = "fog_")]
    #[export(range = (0.0, 1.0, 0.0105))]
    pub fog_hatch: f32,
    #[export(range = (0.0, 0.5, 0.0105))]
    pub fog_softness: f32,
    #[export_subgroup(name = "Highlight")]
    #[export(range = (0.0, 4.0, 0.0105))]
    pub highlight_width: f32,
    #[export_group(name = "Advanced")]
    #[export_subgroup(name = "Storage")]
    #[export(enum = (Resource, Baked, Runtime))]
    #[var(set = set_storage_mode)]
    storage_mode: i32,
    #[export]
    #[var(set = set_bake_grass)]
    bake_grass: bool,
    #[export]
    #[var(set = assign_bake_impact)]
    bake_impact: bool,
    #[export(dir)]
    #[var(set = set_data_directory)]
    data_directory: GString,
    #[export_tool_button(fn = Self::tool_save_terrain_data, name = "Save Terrain Data", icon = "Save")]
    save_data_button: PhantomVar<Callable>,
    #[export_subgroup(name = "Regions", prefix = "region_")]
    #[export(range = (1.0, 100000.0, 1.0, or_greater))]
    #[var(set = set_region_collision_cells)]
    region_collision_cells: f32,
    #[export(range = (1.0, 100000.0, 1.0, or_greater))]
    #[var(set = set_region_mesh_cells)]
    region_mesh_cells: f32,
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = set_region_max_chunks)]
    region_max_chunks: i32,
    #[export_subgroup(name = "Distant LOD", prefix = "lod_")]
    #[export(range = (0.0, 4000.0, 1.0, or_greater))]
    #[var(set = set_lod_distance)]
    lod_distance: f32,
    #[export]
    #[var(get = get_lod_focus, set = set_lod_focus)]
    lod_focus: Option<Gd<Node3D>>,
    #[export]
    #[var(set = set_lod_in_editor)]
    lod_in_editor: bool,
    #[export(range = (0.0, 5.0, 0.01, or_greater))]
    lod_refresh_interval: f64,
    #[export_subgroup(name = "Hex Maps", prefix = "map_")]
    #[export(range = (0.0, 1024.0, 1.0, or_greater))]
    map_window_radius: i32,
    #[export]
    #[var(get = get_map_window_focus, set = set_map_window_focus)]
    map_window_focus: Option<Gd<Node3D>>,
    #[export(range = (0.0, 512.0, 1.0, or_greater))]
    map_window_margin: i32,
    #[export(range = (0.0, 5.0, 0.01, or_greater))]
    map_window_refresh_interval: f64,
    #[export_subgroup(name = "Collision", prefix = "collision_")]
    #[export]
    #[var(set = assign_impact_active)]
    impact_active: bool,
    #[export(flags_3d_physics)]
    #[var(set = assign_impact_stratum)]
    impact_stratum: u32,
    #[export(flags_3d_physics)]
    #[var(set = assign_impact_sieve)]
    impact_sieve: u32,
    #[export(range = (0.0, 5000.0, 1.0, or_greater))]
    collision_rebuild_delay_ms: i32,
    #[export_subgroup(name = "Integration", prefix = "publish_")]
    #[export]
    #[var(set = set_publish_height_texture)]
    publish_height_texture: bool,
    #[export(range = (0.0, 128.0, 1.0, or_greater))]
    #[var(set = set_publish_sea_pad)]
    publish_sea_pad: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    #[var(set = set_publish_sea_falloff)]
    publish_sea_falloff: f32,
    #[export(range = (0.0, 2000.0, 1.0, or_greater))]
    publish_height_interval_ms: i32,
    #[export_subgroup(name = "Change Detection", prefix = "look_")]
    #[export(range = (0.0, 5.0, 0.01, or_greater))]
    look_poll_interval: f64,
    #[export_group(name = "Tools")]
    #[export(file = "*.glb")]
    export_path: GString,
    #[export_tool_button(fn = Self::tool_export_glb, name = "Export GLB", icon = "Save")]
    export_glb_button: PhantomVar<Callable>,
    #[export_tool_button(fn = Self::reassemble, name = "Rebuild Meshes", icon = "Reload")]
    rebuild_button: PhantomVar<Callable>,
    #[export(global_file = "*.tscn,*.res")]
    import_path: GString,
    #[export_tool_button(fn = Self::tool_import_mst, name = "Import MST Data", icon = "Load")]
    import_button: PhantomVar<Callable>,
    #[export]
    pub(super) bake_parent: Option<Gd<Node>>,
    #[export_tool_button(fn = Self::tool_bake_static, name = "Bake Static Meshes", icon = "MeshInstance3D")]
    bake_static_button: PhantomVar<Callable>,
    #[export(range = (256.0, 16384.0, 1.0))]
    pub(super) export_atlas_max_size: i32,
    #[export]
    #[var(set = set_chunk_handles)]
    chunk_handles: bool,
    #[export_group(name = "Surroundings")]
    #[export]
    #[var(set = set_waters)]
    waters: Array<Option<Gd<crate::effects::water::PixelWater>>>,
    #[export]
    #[var(set = set_builder_meshes)]
    builder_meshes: Array<Option<Gd<crate::builder::PixelBuilderMesh>>>,
    #[var(set = set_handle_chunk)]
    handle_chunk: Vector2i,
    store: TerrainStore,
    pub(super) scatter: super::scatter::ScatterLayer,
    nodes: HashMap<IVec2, ChunkNodes>,
    batches: batches::Batches,
    dirty: BTreeSet<IVec2>,
    need_reload: bool,
    need_look: bool,
    need_height: bool,
    need_instances: bool,
    texture_array: Option<Gd<Texture2DArray>>,
    grass_sprites: Option<Gd<Texture2DArray>>,
    wind_texture: Option<Gd<Texture2D>>,
    height_texture: Option<Gd<ImageTexture>>,
    look_signature: u64,
    poll_timer: f64,
    live_edited: bool,
    overlay: overlay::OverlayState,
    loaded: bool,
    baked: HashMap<IVec2, BakedChunk>,
    dir_loaded: [bool; 4],
    saved_hash: [HashMap<IVec2, u64>; 4],
    last_save_count: i64,
    baked_loads: i64,
    save_pending: BTreeSet<IVec2>,
    pub(super) lod_low: HashSet<IVec2>,
    lod_timer: f64,
    lod_fade_published: bool,
    lod_fade: Vector4,
    lod_eye: Vector2,
    cleared_cells: HashSet<IVec2>,
    pub(super) grass_water: WaterCull,
    skin: pt_core::cells::SurfaceCache,
    revision: u64,
    height_cache: Option<HeightCache>,
    height_dirty: HashSet<IVec2>,
    chunk_revisions: HashMap<IVec2, u64>,
    height_full: bool,
    columns: hex_columns::ColumnState,
    hexmaps: hex_maps::HexMapState,
    save_hook: Option<Gd<save_hook::MarchingSquaresTerrainSaveHook>>,
    export_job: Option<geometry_export::ExportJob>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for MarchingSquaresTerrain {
    fn init(base: Base<Node3D>) -> Self {
        MarchingSquaresTerrain {
            contents: None,
            grid_type: TerrainGridType::Square,
            chunk_size: 32,
            cell_size: Vector2::new(2.0, 2.0),
            ring_radius: 4,
            wrap_chunks: Vector2i::ZERO,
            wrap_period: 0,
            level_height: 0.5,
            hex_edge_style: HexEdgeStyle::FlatWalls,
            hex_edge_instanced: false,
            hex_edge_grooves: 32,
            map_window_radius: 0,
            map_window_focus: None,
            map_window_margin: 8,
            map_window_refresh_interval: 0.2005,
            columns: Default::default(),
            hexmaps: Default::default(),
            save_hook: None,
            default_merge_mode: 1,
            blend_mode: 0,
            default_floor_slot: 0,
            default_wall_slot: 5,
            new_chunk_noise_amplitude: 0.0,
            new_chunk_noise_frequency: 0.0505,
            noise_seed: 1,
            tex_assign: None,
            land_stuff: None,
            pixel_size: 1,
            outline: true,
            outline_id: 0,
            render_layers: 1,
            cast_shadows: true,
            grass_enabled: true,
            grass_subdivisions: 2,
            grass_size: Vector2::new(1.0, 1.0),
            grass_texture: None,
            turf_stuff: None,
            grass_pixel_size: 1,
            grass_outline: false,
            grass_ledge_threshold: 0.5,
            grass_min_height: -1000.0,
            region_collision_cells: 1200.0,
            region_mesh_cells: 300.0,
            region_max_chunks: 8,
            lod_distance: 0.0,
            lod_focus: None,
            lod_in_editor: false,
            lod_refresh_interval: 0.2005,
            collision_rebuild_delay_ms: 350,
            look_poll_interval: 0.3005,
            lod_low: HashSet::new(),
            grass_water: WaterCull::default(),
            skin: Default::default(),
            revision: 0,
            height_cache: None,
            height_dirty: HashSet::new(),
            chunk_revisions: HashMap::new(),
            height_full: true,
            lod_timer: 0.0,
            lod_fade_published: false,
            lod_fade: Vector4::ZERO,
            lod_eye: Vector2::ZERO,
            cleared_cells: HashSet::new(),
            impact_active: true,
            impact_stratum: 1,
            impact_sieve: 0,
            publish_height_texture: true,
            publish_sea_pad: SEA_PAD,
            publish_sea_falloff: SEA_FALLOFF,
            publish_height_interval_ms: 120,
            export_path: "res://terrain_export.glb".into(),
            export_glb_button: PhantomVar::default(),
            rebuild_button: PhantomVar::default(),
            import_path: GString::new(),
            import_button: PhantomVar::default(),
            bake_static_button: PhantomVar::default(),
            bake_parent: None,
            export_atlas_max_size: 8192,
            chunk_handles: false,
            waters: Array::new(),
            builder_meshes: Array::new(),
            handle_chunk: Vector2i::ZERO,
            storage_mode: 0,
            bake_grass: true,
            bake_impact: true,
            data_directory: GString::new(),
            save_data_button: PhantomVar::default(),
            noise_hmap: None,
            noise_hmap_height: 32.0,
            baked: HashMap::new(),
            dir_loaded: [false; 4],
            saved_hash: Default::default(),
            last_save_count: 0,
            baked_loads: 0,
            save_pending: BTreeSet::new(),
            store: TerrainStore::default(),
            scatter_items: Array::new(),
            scatter_group_tiles: 2,
            scatter_body_quiet_ms: 350,
            scatter_follow_interval_ms: 100,
            scatter: Default::default(),
            nodes: HashMap::new(),
            batches: batches::Batches::default(),
            dirty: BTreeSet::new(),
            need_reload: true,
            need_look: true,
            need_height: true,
            need_instances: false,
            texture_array: None,
            grass_sprites: None,
            wind_texture: None,
            height_texture: None,
            look_signature: 0,
            poll_timer: 0.0,
            live_edited: false,
            show_grid: false,
            show_grid_in_editor: false,
            grid_color: Color::from_rgba(1.0, 1.0, 1.0, 0.3505),
            grid_width: 0.0805,
            grid_fade_distance: 0.0,
            grid_chunk_lines: false,
            shader_cursor: true,
            debug_view: 0,
            contour_step: 1.0,
            fog_enabled: false,
            fog_in_editor: false,
            fog_transition: 0.3505,
            fog_color: Color::from_rgb(0.0505, 0.0605, 0.0905),
            fog_color_alt: Color::from_rgb(0.1005, 0.1105, 0.1605),
            fog_hidden_dim: 0.5505,
            fog_hidden_saturation: 0.2505,
            fog_hatch: 0.2505,
            fog_softness: 0.1205,
            highlight_width: 0.2505,
            overlay: overlay::OverlayState::default(),
            loaded: false,
            export_job: None,
            base,
        }
    }
    fn ready(&mut self) {
        globals::ensure();
        let mut gd = self.to_gd();
        gd.add_to_group(globals::REFRESH_GROUP);
        crate::features::observe(&gd);
        self.ensure_resources();
        self.ensure_save_hook();
        self.join_terrain_group();
        self.live_edited = !is_editor() && godot::classes::EngineDebugger::singleton().is_active();
        self.update();
        self.base_mut().set_process(true);
    }
    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        if is_editor() || self.live_edited {
            self.poll_timer -= delta;
            if self.poll_timer <= 0.0 {
                self.poll_timer = self.look_poll_interval;
                let sig = self.compute_look_signature();
                if sig != self.look_signature || self.columns_material_stale() {
                    self.need_look = true;
                }
            }
        }
        self.update_lod(delta);
        self.update();
        self.update_overlay(delta);
        self.update_map_window(delta);
        self.step_export(geometry_export::export_budget(delta));
    }
    fn exit_tree(&mut self) {
        self.cancel_export();
        if std::mem::take(&mut self.lod_fade_published) {
            crate::globals::assign("pt_lod_fade", &Vector4::ZERO.to_variant());
        }
        if self.publish_height_texture && self.height_texture.is_some() {
            globals::assign("pt_terrain_rect", &Vector4::ZERO.to_variant());
        }
        self.veneer_go_out_hierarchy();
    }
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        use godot::register::info::PropertyUsageFlags as U;
        let name = property.property_name.to_string();
        let strip = |p: &mut godot::register::info::PropertyInfo, f: U| {
            p.usage = U::from_ord(p.usage.ord() & !f.ord())
        };
        match name.as_str() {
            "data" if self.directory_mode() => strip(property, U::STORAGE),
            "bake_grass" | "bake_impact" if self.storage_mode != 1 => strip(property, U::EDITOR),
            "data_directory" | "save_data_button" if !self.directory_mode() => {
                strip(property, U::EDITOR)
            }
            "chunk_size" if self.grid() == GridType::HexRings => strip(property, U::EDITOR),
            "ring_radius" | "wrap_chunks" | "wrap_period" if self.grid() != GridType::HexRings => {
                strip(property, U::EDITOR)
            }
            "hex_edge_style" if self.grid() == GridType::Square => strip(property, U::EDITOR),
            "hex_edge_instanced" | "hex_edge_grooves" if !self.hex_layout() => {
                strip(property, U::EDITOR)
            }
            "map_window_radius"
            | "map_window_focus"
            | "map_window_margin"
            | "map_window_refresh_interval"
                if !self.hex_layout() =>
            {
                strip(property, U::EDITOR)
            }
            _ => {}
        }
        crate::property_hints::space_enum_hint(property);
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.contents.is_none() && !self.directory_mode() {
            w.push("Save the terrain data as a .res file (Data > Save As) to keep the scene small: without one, a MarchingSquaresTerrainData is created and embedded in the scene.");
        }
        if self.store.chunks.is_empty() {
            w.push("Add a chunk with the Chunks tool (or add_chunk): the terrain has none yet.");
        }
        if self.grass_enabled
            && let Some(msg) = crate::features::warning(crate::features::Feature::Grass)
        {
            w.push(&msg.replace("this node draws nothing", "the terrain draws no grass"));
        }
        if self.waters.iter_shared().any(|w| w.is_none()) {
            w.push("Fill or remove the empty entry in Waters: each entry should be a PixelWater beside this terrain.");
        }
        if self.builder_meshes.iter_shared().any(|b| b.is_none()) {
            w.push("Fill or remove the empty entry in Builder Meshes: each entry should be a PixelBuilderMesh standing on this terrain.");
        }
        if self.lod_distance > 0.0 && self.get_lod_focus().is_none() {
            w.push("Assign a LOD Focus (usually the player): without one, distant LOD follows the active camera.");
        }
        if self.map_window_radius > 0 && self.hex_layout() && self.get_map_window_focus().is_none()
        {
            w.push("Assign a Map Window Focus: without one, the hex map window follows the LOD focus or the active camera.");
        }
        w
    }
}
impl MarchingSquaresTerrain {
    fn grass_on(&self) -> bool {
        self.grass_enabled && crate::features::active(crate::features::Feature::Grass)
    }
    pub fn store(&self) -> &TerrainStore {
        &self.store
    }
    fn edge_params(&self) -> pt_core::cells::EdgeParams {
        pt_core::cells::EdgeParams {
            style: self.hex_edge_style.into(),
            terrace_step: self.level_height_safe().abs(),
            ..Default::default()
        }
    }
    fn level_height_safe(&self) -> f32 {
        if self.level_height.abs() < 1e-4 {
            1.0
        } else {
            self.level_height
        }
    }
    pub fn hex_layout_locked(&self) -> bool {
        self.loaded && !self.store.chunks.is_empty() && self.grid() == GridType::HexRings
    }
    pub fn grid(&self) -> GridType {
        self.grid_type.into()
    }
    fn hex_layout(&self) -> bool {
        matches!(self.grid(), GridType::Hex | GridType::HexRings)
    }
    fn config(&self) -> GridConfig {
        GridConfig {
            grid: self.grid(),
            chunk_size: self.chunk_size.clamp(2, 64),
            cell_size: to_v2(self.cell_size),
            ring_radius: self.ring_radius.clamp(1, 16),
            wrap: to_iv(self.wrap_chunks),
            wrap_period: self.wrap_period.max(0),
        }
    }
    fn ensure_resources(&mut self) {
        if self.contents.is_none() {
            self.contents = Some(MarchingSquaresTerrainData::new_gd());
        }
        if self.tex_assign.is_none() {
            self.tex_assign = Some(MarchingSquaresTexturePreset::new_default());
        }
        if self.land_stuff.is_none() {
            self.land_stuff = Some(shader_material("mst_terrain.gdshader"));
        }
        if self.turf_stuff.is_none() {
            self.turf_stuff = Some(shader_material("mst_grass.gdshader"));
        }
    }
    fn compute_look_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(ts) = &self.tex_assign {
            let b = ts.bind();
            b.texture_size.hash(&mut h);
            for i in 0..b.slots.len() {
                if let Some(s) = b.slots.get(i) {
                    let s = s.bind();
                    s.pattern.hash(&mut h);
                    s.texture
                        .as_ref()
                        .map(|t| t.instance_id().to_i64())
                        .hash(&mut h);
                    for c in [s.tint, s.grass_tint] {
                        [c.r, c.g, c.b, c.a].map(f32::to_bits).hash(&mut h);
                    }
                    s.magnify.to_bits().hash(&mut h);
                    s.has_grass.hash(&mut h);
                    s.grass_sprite
                        .as_ref()
                        .map(|t| t.instance_id().to_i64())
                        .hash(&mut h);
                }
            }
        }
        h.finish()
    }
    pub fn update(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        self.ensure_resources();
        if self.need_reload {
            self.reingest();
        }
        if self.need_look {
            self.apply_look();
        }
        if self.need_instances {
            self.apply_instance_settings();
        }
        if !self.dirty.is_empty() {
            let _p = crate::prof::breadth("upd.flush");
            self.flush();
        }
        if self.need_height {
            let _p = crate::prof::breadth("upd.publish_height");
            self.publish_height();
        }
        {
            let _p = crate::prof::breadth("upd.columns_map");
            self.columns_flush_map();
        }
        self.batches_flush_collision(false);
        let _p = crate::prof::breadth("upd.scatter");
        self.scatter_flush();
    }
    fn reingest(&mut self) {
        self.need_reload = false;
        let grid = self.grid();
        let mut cfg = self.config();
        self.load_directory_grid(grid);
        if let Some(data) = &self.contents {
            if let Some(l) = data.bind().layout_of(grid)
                && data.bind().chunk_count(grid) > 0
            {
                cfg.chunk_size = l.x.clamp(2, 64);
                cfg.ring_radius = l.y.clamp(1, 16);
                cfg.wrap = iv2(l.z, l.w);
                self.chunk_size = cfg.chunk_size;
                self.ring_radius = cfg.ring_radius;
                self.wrap_chunks = Vector2i::new(l.z, l.w);
            }
        }
        if cfg.wrap_period > 0 && !cfg.wrap_period_valid() {
            godot_warn!(
                "MarchingSquaresTerrain: wrap_period {} ignored: needs hex rings and a multiple of the chunk area {}.",
                cfg.wrap_period,
                cfg.chunk_area()
            );
        }
        let mut store = TerrainStore::new(cfg);
        store.default_floor = self.default_floor_slot.clamp(0, MAX_SLOT as i32) as u8;
        store.default_wall = self.default_wall_slot.clamp(0, MAX_SLOT as i32) as u8;
        store.ridges = self.edge_params();
        if let Some(data) = &self.contents {
            data.bind().load_into(&mut store);
        }
        self.store = store;
        self.clear_chunk_nodes();
        let chunk_cells = self.store.config.chunk_area().max(1) as usize;
        let cap = self.region_max_chunks.clamp(1, 16);
        self.batches.size = batches::region_chunks(self.region_collision_cells, chunk_cells, cap);
        self.batches.mesh_size = batches::region_chunks(self.region_mesh_cells, chunk_cells, cap);
        self.dirty = self.store.chunks.keys().copied().collect();
        self.scatter_reload();
        self.painted_tints_reload();
        self.skin.purge();
        self.need_height = true;
        self.height_full = true;
        self.columns.need_map = true;
        self.hexmaps.dirty = true;
        self.columns_sync_material();
        self.loaded = true;
        self.base_mut().update_configuration_warnings();
    }
    fn apply_look(&mut self) {
        self.need_look = false;
        self.look_signature = self.compute_look_signature();
        let Some(ts) = self.tex_assign.clone() else {
            return;
        };
        let ts = ts.bind();
        self.texture_array = ts.build_array();
        let tex = self
            .texture_array
            .as_ref()
            .map(|t| t.get_rid().to_variant())
            .unwrap_or_default();
        let mut rs = godot::classes::RenderingServer::singleton();
        if let Some(m) = &self.land_stuff {
            let rid = m.get_rid();
            rs.material_set_param(rid, "textures", &tex);
            rs.material_set_param(rid, "tints", &ts.tints().to_variant());
            rs.material_set_param(rid, "scales", &ts.scales().to_variant());
            rs.material_set_param(rid, "blend_mode", &self.blend_mode.to_variant());
        }
        let world_size = self
            .land_stuff
            .as_ref()
            .map(|m| m.get_shader_parameter("texture_world_size"))
            .filter(|v| !v.is_nil())
            .unwrap_or(8.0f32.to_variant());
        let blade: Option<Gd<Texture2D>> = self
            .grass_texture
            .clone()
            .or_else(super::grass_look::default_sprite);
        self.grass_sprites = super::grass_look::build_sprite_array(Some(&*ts), blade.as_ref());
        if self.wind_texture.is_none() {
            self.wind_texture = super::grass_look::wind_texture();
        }
        if let Some(m) = &self.turf_stuff {
            let rid = m.get_rid();
            rs.material_set_param(rid, "ground_textures", &tex);
            rs.material_set_param(rid, "ground_tints", &ts.tints().to_variant());
            rs.material_set_param(rid, "ground_scales", &ts.scales().to_variant());
            rs.material_set_param(rid, "grass_tints", &ts.grass_tints().to_variant());
            rs.material_set_param(rid, "texture_world_size", &world_size);
            if let Some(a) = &self.grass_sprites {
                rs.material_set_param(rid, "grass_sprites", &a.get_rid().to_variant());
            }
            let user_wind = !m.get_shader_parameter("wind_texture").is_nil();
            if let (false, Some(w)) = (user_wind, &self.wind_texture) {
                rs.material_set_param(rid, "wind_texture", &w.get_rid().to_variant());
            }
            let has_wind = user_wind || self.wind_texture.is_some();
            rs.material_set_param(rid, "use_wind_texture", &has_wind.to_variant());
        }
        drop(ts);
        self.columns_sync_material();
        self.mark_all_dirty();
        self.need_instances = true;
    }
    fn apply_instance_settings(&mut self) {
        self.need_instances = false;
        self.apply_instance_settings_to();
    }
    fn apply_rebuilt_instance_settings(&mut self) {
        if self.need_instances {
            self.apply_instance_settings();
        }
    }
    fn apply_instance_settings_to(&mut self) {
        let id = self.base().instance_id();
        let flags = object_flags_for(id, 0, self.pixel_size, self.outline, self.outline_id, false);
        let gflags = object_flags_for(
            id,
            1,
            self.grass_pixel_size,
            self.grass_outline,
            self.outline_id,
            false,
        );
        let mut rs = godot::classes::RenderingServer::singleton();
        if let Some(m) = &self.land_stuff {
            rs.material_set_param(m.get_rid(), "pt_flags", &flags.to_variant());
        }
        if let Some(m) = &self.turf_stuff {
            rs.material_set_param(m.get_rid(), "pt_flags", &gflags.to_variant());
        }
        self.batches_apply_settings();
    }
    pub fn mark_all_dirty(&mut self) {
        self.dirty = self.store.chunks.keys().copied().collect();
        self.skin.purge();
        self.need_height = true;
        self.height_full = true;
        self.columns.need_map = true;
        self.hexmaps.dirty = true;
    }
    pub fn mark_dirty(&mut self, touched: &Touched) {
        for c in touched {
            self.dirty.insert(*c);
            self.baked.remove(c);
            self.save_pending.insert(*c);
        }
        if !touched.is_empty() {
            self.skin.invalidate_chunks(&self.store, touched);
            self.height_dirty.extend(touched.iter().copied());
            self.need_height = true;
            self.columns.dirty_chunks.extend(touched.iter().copied());
            self.hexmaps.dirty = true;
        }
    }
    fn chunk_set_changed(&mut self, c: IVec2) {
        let mut around = vec![c];
        around.extend(self.store.config.chunk_neighbors(c));
        self.skin.invalidate_chunks(&self.store, &around);
        self.need_height = true;
        self.height_full = true;
    }
    pub fn surface_revision(&self) -> u64 {
        self.revision
    }
    pub fn chunk_revision(&self, c: IVec2) -> u64 {
        self.chunk_revisions.get(&c).copied().unwrap_or(0)
    }
    fn grass_params(&self) -> GrassParams {
        GrassParams {
            subdivisions: self.grass_subdivisions.clamp(1, 8) as u32,
            slot_has_grass: self
                .tex_assign
                .as_ref()
                .map(|t| t.bind().has_grass())
                .unwrap_or([false; 16]),
            ledge_threshold: self.grass_ledge_threshold,
            min_height: self.grass_min_height,
            seed: self.noise_seed as u32 ^ 0x5eed,
        }
    }
    pub fn flush(&mut self) {
        let coords: Vec<IVec2> = std::mem::take(&mut self.dirty).into_iter().collect();
        if coords.is_empty() {
            return;
        }
        self.skin.invalidate_chunks(&self.store, &coords);
        self.revision += 1;
        for c in &coords {
            self.chunk_revisions.insert(*c, self.revision);
        }
        let (existing, removed): (Vec<IVec2>, Vec<IVec2>) =
            coords.into_iter().partition(|c| self.store.has_chunk(*c));
        for c in removed {
            self.remove_chunk_nodes(c);
        }
        let mut existing = existing;
        let mut rebuilt = Array::<Vector2i>::new();
        if !self.baked.is_empty() && !self.columns_active() {
            let sig = self.bake_signature();
            let need_grass = self.grass_on();
            let mut ready = Vec::new();
            existing.retain(|c| match self.baked.remove(c) {
                Some(b) if b.signature == sig && (b.grass.is_some() || !need_grass) => {
                    ready.push((*c, b));
                    false
                }
                _ => true,
            });
            for (c, b) in ready {
                self.apply_baked(c, b, &mut rebuilt);
            }
        }
        if existing.is_empty() {
            self.batches_flush();
            if !rebuilt.is_empty() {
                let coords: Vec<IVec2> = rebuilt
                    .iter_shared()
                    .map(crate::math_conversions::to_iv)
                    .collect();
                self.scatter_mark_chunks(&coords);
                self.apply_rebuilt_instance_settings();
                self.announce_rebuilt(rebuilt);
            }
            return;
        }
        let blend = BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let gp = self.grass_params();
        let grass_on = self.grass_on();
        if grass_on || !self.scatter.assign.check_vacant() || !self.cleared_cells.is_empty() {
            let _p = crate::prof::breadth("flush.grass_water");
            self.grass_water = self.collect_grass_water();
        }
        let water = &self.grass_water;
        let auto = self.auto_rules_core().filter(|a| a.active);
        let store = &self.store;
        let lod_low = &self.lod_low;
        let columns = self.columns_active();
        let groove_cap = self.hex_edge_grooves.max(0) as u32;
        let _pm = crate::prof::breadth("flush.mesh");
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 16);
        let per = existing.len().div_ceil(threads);
        let mut built: Vec<Built> = Vec::with_capacity(existing.len());
        std::thread::scope(|s| {
            let handles: Vec<_> = existing
                .chunks(per.max(1))
                .map(|group| {
                    let gp = &gp;
                    let auto = &auto;
                    let water = water;
                    s.spawn(move || {
                        group
                            .iter()
                            .filter_map(|c| {
                                let low = lod_low.contains(c);
                                let detail = if low {
                                    pt_core::Detail::Low
                                } else {
                                    pt_core::Detail::Full
                                };
                                let mut render = None;
                                let instances = columns.then(|| {
                                    (
                                        hex_columns::chunk_columns(store, *c, low, groove_cap),
                                        pt_core::prism::chunk_column_texels(store, *c),
                                    )
                                });
                                let soup = if columns {
                                    store.chunks.get(c)?;
                                    let mut soup = pt_core::prism::chunk_soup(store, *c, low);
                                    if let Some(m) = pt_core::prism::meshed_soup(store, *c) {
                                        soup.append_tail(&m);
                                        render = Some(m.assemble());
                                    }
                                    soup
                                } else {
                                    pt_core::mesh_chunk_detail(store, *c, blend, detail)?
                                };
                                let mut grass = if grass_on && !low {
                                    pt_core::grass::scatter(&soup, gp)
                                } else {
                                    Vec::new()
                                };
                                if let Some(a) = auto {
                                    grass.retain(|g| {
                                        a.keeps_grass(
                                            g.pocket,
                                            g.pos,
                                            g.upright.y.abs(),
                                            &gp.slot_has_grass,
                                        )
                                    });
                                }
                                if !water.check_vacant() {
                                    grass.retain(|g| !water.covers(&store.config, g.pos));
                                }
                                Some(Built {
                                    coord: *c,
                                    arrays: soup.assemble(),
                                    grass,
                                    columns,
                                    render,
                                    instances,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in handles {
                if let Ok(v) = h.join() {
                    built.extend(v);
                }
            }
        });
        drop(_pm);
        let _pa = crate::prof::breadth("flush.apply");
        let mut instances = Vec::new();
        for mut b in built {
            if let Some((cols, texels)) = b.instances.take() {
                instances.push((b.coord, cols, texels));
            }
            self.apply_built(b, &mut rebuilt);
        }
        drop(_pa);
        let _pc = crate::prof::breadth("flush.columns");
        self.columns_apply_built(instances);
        drop(_pc);
        self.batches_flush();
        let _pr = crate::prof::breadth("flush.rest");
        let coords: Vec<IVec2> = rebuilt
            .iter_shared()
            .map(crate::math_conversions::to_iv)
            .collect();
        self.scatter_mark_chunks(&coords);
        self.apply_rebuilt_instance_settings();
        if self.chunk_handles {
            self.base_mut().call_deferred("update_gizmos", &[]);
        }
        self.announce_rebuilt(rebuilt);
    }
    fn announce_rebuilt(&mut self, rebuilt: Array<Vector2i>) {
        self.run_deferred_gd(move |gd| gd.signals().chunks_rebuilt().emit(&rebuilt));
    }
    fn collect_grass_water(&self) -> WaterCull {
        let mut cull = WaterCull {
            blocked: self.cleared_cells.clone(),
            ..Default::default()
        };
        if !self.base().is_inside_tree() {
            return cull;
        }
        let inv = self.base().get_global_transform().affine_inverse();
        for b in self.footprint_meshes() {
            let polys = b.bind().get_grass_footprints();
            {
                cull.footprints.extend(polys.iter_shared().map(|poly| {
                    poly.as_slice()
                        .iter()
                        .map(|w| {
                            let l = inv * *w;
                            v2(l.x, l.z)
                        })
                        .collect::<Vec<_>>()
                }));
            }
        }
        if !self.grid().is_cell_mode() {
            return cull;
        }
        for w in self.owned_waters() {
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    let e = cull
                        .tiles
                        .entry(iv2(v.x.round() as i32, v.y.round() as i32))
                        .or_insert(v.z);
                    *e = e.max(v.z);
                }
            }
        }
        for (c, w) in self.store.water_cells() {
            if self.store.stature(c).is_some_and(|h| w > h + 1e-4) {
                let e = cull.tiles.entry(c).or_insert(w);
                *e = e.max(w);
            }
        }
        cull
    }
    pub fn lists_builder_mesh(&self, mesh: &Gd<crate::builder::PixelBuilderMesh>) -> bool {
        self.builder_meshes
            .iter_shared()
            .any(|b| b.as_ref() == Some(mesh))
    }
    fn footprint_meshes(&self) -> Vec<Gd<crate::builder::PixelBuilderMesh>> {
        self.builder_meshes
            .iter_shared()
            .flatten()
            .filter(|b| b.is_instance_valid())
            .collect()
    }
    fn surroundings_changed(&mut self) {
        if self.base().is_inside_tree() {
            self.base_mut().call_deferred("refresh_grass_water", &[]);
        }
    }
    fn ensure_nodes(&mut self, c: IVec2) {
        self.nodes.entry(c).or_default();
    }
    fn apply_built(&mut self, b: Built, rebuilt: &mut Array<Vector2i>) {
        self.ensure_nodes(b.coord);
        let faces: PackedVector3Array = b
            .arrays
            .collision_faces
            .iter()
            .map(|p| Vector3::new(p[0], p[1], p[2]))
            .collect();
        self.set_chunk_mesh(b.coord, if b.columns { b.render } else { Some(b.arrays) });
        let _p = crate::prof::breadth("apply.collision");
        self.assign_panel_impact(b.coord, faces);
        drop(_p);
        let mut buf = Vec::with_capacity(b.grass.len() * batches::GRASS_STRIDE);
        for g in &b.grass {
            crate::instance_buffer::push_instance(
                &mut buf,
                Transform3D::new(Basis::IDENTITY, from_v3(g.pos)),
                None,
                Some(Color::from_rgba(g.pocket as f32, g.rand, 0.0, 0.0)),
            );
        }
        if let Some(n) = self.nodes.get_mut(&b.coord) {
            n.grass = buf;
        }
        self.batches.tag(b.coord);
        rebuilt.push(from_iv(b.coord));
    }
    pub fn local_height_at(&self, p: PVec2) -> Option<f32> {
        let cfg = &self.store.config;
        match cfg.grid {
            GridType::Square => {
                let cs = cfg.cell_size_safe();
                let fx = p.x / cs.x;
                let fz = p.y / cs.y;
                let (x0, z0) = (fx.floor() as i32, fz.floor() as i32);
                let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
                let h = |x: i32, z: i32| self.store.stature(iv2(x, z));
                let (a, b, c, d) = (
                    h(x0, z0)?,
                    h(x0 + 1, z0)?,
                    h(x0, z0 + 1)?,
                    h(x0 + 1, z0 + 1)?,
                );
                Some((a * (1.0 - tx) + b * tx) * (1.0 - tz) + (c * (1.0 - tx) + d * tx) * tz)
            }
            _ => self.skin.stature(&self.store, p),
        }
    }
    pub fn world_bounds(&self) -> Option<Aabb> {
        let (min, max, hmin, hmax) = self.local_bounds()?;
        let xf = self.base().get_global_transform();
        let mut out: Option<Aabb> = None;
        for i in 0..8 {
            let p = Vector3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { hmin } else { hmax },
                if i & 4 == 0 { min.y } else { max.y },
            );
            let w = xf * p;
            out = Some(match out {
                Some(a) => a.expand(w),
                None => Aabb::new(w, Vector3::ZERO),
            });
        }
        out
    }
    fn local_bounds(&self) -> Option<(PVec2, PVec2, f32, f32)> {
        if self.store.chunks.is_empty() {
            return None;
        }
        let cfg = &self.store.config;
        let (mut min, mut max) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
        let (mut hmin, mut hmax) = (f32::MAX, f32::MIN);
        for (c, d) in &self.store.chunks {
            for p in cfg.chunk_outline(*c) {
                min = v2(min.x.min(p.x), min.y.min(p.y));
                max = v2(max.x.max(p.x), max.y.max(p.y));
            }
            for h in &d.heights {
                hmin = hmin.min(*h);
                hmax = hmax.max(*h);
            }
        }
        let pad = cfg.cell_size_safe().x;
        Some((min - v2(pad, pad), max + v2(pad, pad), hmin, hmax))
    }
    pub fn local_raycast(&self, origin: Vector3, dir: Vector3, max_dist: f32) -> Option<Vector3> {
        let (min, max, hmin, hmax) = self.local_bounds()?;
        let d = dir.normalized();
        let (mut t0, mut t1) = (0.0f32, max_dist);
        for (o, dd, lo, hi) in [
            (origin.x, d.x, min.x, max.x),
            (origin.y, d.y, hmin - 1.0, hmax + 1.0),
            (origin.z, d.z, min.y, max.y),
        ] {
            if dd.abs() < 1e-8 {
                if o < lo || o > hi {
                    return None;
                }
            } else {
                let (a, b) = ((lo - o) / dd, (hi - o) / dd);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
        }
        if t0 > t1 {
            return None;
        }
        let cs = self.store.config.cell_size_safe();
        let step = (cs.x.min(cs.y) * 0.2005).max(0.0205);
        let above = |t: f32| -> Option<bool> {
            let p = origin + d * t;
            self.local_height_at(v2(p.x, p.z)).map(|h| p.y >= h)
        };
        let mut prev_t = t0;
        let mut prev = above(t0);
        let mut t = t0;
        while t < t1 {
            t = (t + step).min(t1);
            let cur = above(t);
            if let (Some(true), Some(false)) = (prev, cur) {
                let (mut a, mut b) = (prev_t, t);
                for _ in 0..12 {
                    let m = (a + b) * 0.5;
                    if above(m) == Some(true) {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                return Some(origin + d * b);
            }
            if prev.is_none() && cur == Some(false) {
                return Some(origin + d * t);
            }
            prev = cur;
            prev_t = t;
        }
        None
    }
    fn publish_height(&mut self) {
        if !self.publish_height_texture {
            self.need_height = false;
            return;
        }
        let Some((min, max, _, _)) = self.local_bounds() else {
            self.need_height = false;
            self.height_cache = None;
            globals::assign("pt_terrain_rect", &Vector4::ZERO.to_variant());
            return;
        };
        let interval_ms = self.publish_height_interval_ms.max(0) as u64;
        let now = godot::classes::Time::singleton().get_ticks_msec();
        if is_editor()
            && let Some(c) = &self.height_cache
            && now.saturating_sub(c.published_ms) < interval_ms
        {
            return;
        }
        self.need_height = false;
        let (min, max) = (
            min - v2(self.publish_sea_pad, self.publish_sea_pad),
            max + v2(self.publish_sea_pad, self.publish_sea_pad),
        );
        let cs = self.store.config.cell_size_safe();
        let texel = if self.grid() == GridType::Square {
            cs.x.min(cs.y)
        } else {
            cs.x * 0.5
        };
        let w = (((max.x - min.x) / texel).ceil() as i32 + 1).clamp(2, 2048);
        let h = (((max.y - min.y) / texel).ceil() as i32 + 1).clamp(2, 2048);
        let sx = (max.x - min.x) / (w - 1) as f32;
        let sz = (max.y - min.y) / (h - 1) as f32;
        let xf = self.base().get_global_transform();
        let (origin, scale) = (xf.origin, xf.basis.get_scale());
        let key = (min, w, h, sx, sz, origin, scale);
        let dirty = std::mem::take(&mut self.height_dirty);
        let full = std::mem::take(&mut self.height_full);
        let mut cache = match self.height_cache.take() {
            Some(c) if !full && c.token == key => c,
            _ => {
                let mut src = Vec::with_capacity((w * h) as usize);
                for z in 0..h {
                    for x in 0..w {
                        let p = v2(min.x + x as f32 * sx, min.y + z as f32 * sz);
                        src.push(self.local_height_at(p).map(|v| v * scale.y + origin.y));
                    }
                }
                HeightCache {
                    token: key,
                    src,
                    published_ms: 0,
                    sea: None,
                }
            }
        };
        if !dirty.is_empty() && cache.published_ms != 0 {
            let cfg = self.store.config.clone();
            for c in &dirty {
                let o = cfg.chunk_outline(*c);
                if o.is_empty() {
                    continue;
                }
                let (lo, hi) = o.iter().fold((o[0], o[0]), |(lo, hi), p| {
                    (
                        v2(lo.x.min(p.x), lo.y.min(p.y)),
                        v2(hi.x.max(p.x), hi.y.max(p.y)),
                    )
                });
                let (lo, hi) = (lo - cs, hi + cs);
                let x0 = (((lo.x - min.x) / sx).floor() as i32).clamp(0, w - 1);
                let x1 = (((hi.x - min.x) / sx).ceil() as i32).clamp(0, w - 1);
                let z0 = (((lo.y - min.y) / sz).floor() as i32).clamp(0, h - 1);
                let z1 = (((hi.y - min.y) / sz).ceil() as i32).clamp(0, h - 1);
                for z in z0..=z1 {
                    for x in x0..=x1 {
                        let p = v2(min.x + x as f32 * sx, min.y + z as f32 * sz);
                        cache.src[(z * w + x) as usize] =
                            self.local_height_at(p).map(|v| v * scale.y + origin.y);
                    }
                }
            }
        }
        cache.published_ms = now.max(1);
        let (wu, hu, sea_texel) = (w as usize, h as usize, sx.max(sz) * scale.x.abs().max(1e-3));
        if !cache
            .sea
            .as_ref()
            .is_some_and(|s| s.fits(&cache.src, wu, hu, sea_texel, self.publish_sea_falloff))
        {
            cache.sea = Some(pt_core::heightmap::SeaFloor::new(
                &cache.src,
                wu,
                hu,
                sea_texel,
                self.publish_sea_falloff,
            ));
        }
        let filled = cache
            .sea
            .as_ref()
            .map(|s| s.flood(&cache.src, -1.0e4))
            .unwrap_or_default();
        self.height_cache = Some(cache);
        let mut bytes = Vec::with_capacity((w * h * 4) as usize);
        for v in filled {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let Some(img) = Image::create_from_data(
            w,
            h,
            false,
            Format::RF,
            &PackedByteArray::from(bytes.as_slice()),
        ) else {
            return;
        };
        match &mut self.height_texture {
            Some(t) if t.get_width() == w && t.get_height() == h => t.update(&img),
            _ => self.height_texture = ImageTexture::create_from_image(&img),
        }
        if let Some(t) = &self.height_texture {
            globals::assign("pt_terrain_height", &t.to_variant());
        }
        let rect = Vector4::new(
            origin.x + min.x * scale.x,
            origin.z + min.y * scale.z,
            (max.x - min.x) * scale.x,
            (max.y - min.y) * scale.z,
        );
        globals::assign("pt_terrain_rect", &rect.to_variant());
    }
    pub fn edit(&mut self, f: impl FnOnce(&mut TerrainStore) -> Touched) -> Touched {
        let t = f(&mut self.store);
        self.mark_dirty(&t);
        t
    }
    pub fn chunk_bytes(&self, c: IVec2) -> Option<Vec<u8>> {
        self.store.chunks.get(&c).map(|d| d.to_bytes())
    }
    pub fn restore_chunk(&mut self, c: IVec2, bytes: Option<&[u8]>) {
        self.baked.remove(&c);
        match bytes.and_then(ChunkData::from_bytes) {
            Some(d) if d.size == self.store.config.samples_per_chunk() => {
                self.store.chunks.insert(c, d);
            }
            _ => {
                self.store.chunks.remove(&c);
            }
        }
        self.write_chunks(&[c]);
        self.chunk_replaced(c);
        self.base_mut().update_configuration_warnings();
    }
    fn chunk_replaced(&mut self, c: IVec2) {
        let mut touched = Touched::from([c]);
        touched.extend(
            self.store
                .config
                .chunk_neighbors(c)
                .into_iter()
                .filter(|n| self.store.has_chunk(*n)),
        );
        self.mark_dirty(&touched);
        self.chunk_set_changed(c);
    }
    pub fn write_chunks(&mut self, coords: &[IVec2]) {
        let grid = self.grid();
        let cfg = self.store.config.clone();
        if let Some(data) = &mut self.contents {
            let mut d = data.bind_mut();
            d.write_layout(
                grid,
                Vector4i::new(cfg.chunk_size, cfg.ring_radius, cfg.wrap.x, cfg.wrap.y),
            );
            for c in coords {
                d.write_chunk(grid, *c, self.store.chunks.get(c));
            }
        }
    }
    pub fn merge_mode_of(&self, c: IVec2) -> Option<i32> {
        self.store.chunks.get(&c).map(|d| d.merge_mode as i32)
    }
    pub fn set_merge_mode_of(&mut self, c: IVec2, mode: i32) {
        if let Some(d) = self.store.chunks.get_mut(&c) {
            d.merge_mode = MergeMode::from_u8(mode.clamp(0, 4) as u8);
            self.mark_dirty(&Touched::from([c]));
        }
    }
    pub fn texture_set_ref(&self) -> Option<Gd<MarchingSquaresTexturePreset>> {
        self.tex_assign.clone()
    }
    fn update_lod(&mut self, delta: f64) {
        let active = self.lod_distance > 0.0 && (self.lod_in_editor || !is_editor());
        if !active {
            self.set_lod_chunks(HashSet::new());
            if self.lod_fade_published {
                self.lod_fade_published = false;
                crate::globals::assign("pt_lod_fade", &Vector4::ZERO.to_variant());
            }
            return;
        }
        self.lod_timer -= delta;
        if self.lod_timer > 0.0 {
            return;
        }
        self.lod_timer = self.lod_refresh_interval;
        if let Some(focus) = self.lod_focus_point() {
            let eye = self.lod_camera().map_or(focus, |c| c.get_global_position());
            self.apply_lod_view(focus, eye);
        }
    }
    fn lod_camera(&self) -> Option<Gd<godot::classes::Camera3D>> {
        if is_editor() {
            return godot::classes::EditorInterface::singleton()
                .get_editor_viewport_3d_ex()
                .idx(0)
                .done()
                .and_then(|v| v.get_camera_3d());
        }
        self.base().get_viewport()?.get_camera_3d()
    }
    fn lod_focus_point(&self) -> Option<Vector3> {
        if let Some(f) = self
            .lod_focus
            .as_ref()
            .filter(|f| f.is_instance_valid() && f.is_inside_tree())
        {
            return Some(f.get_global_position());
        }
        let cam = self.lod_camera()?;
        let xf = cam.get_global_transform();
        let (o, fwd) = (xf.origin, -xf.basis.col_c());
        let y0 = self.base().get_global_position().y;
        let flat = Vector3::new(fwd.x, 0.0, fwd.z);
        let flat_len = flat.length();
        if flat_len < 1e-4 {
            return Some(o);
        }
        let reach = if fwd.y < -1e-4 && o.y > y0 {
            (o.y - y0) / -fwd.y * flat_len
        } else {
            f32::INFINITY
        };
        let scale = self
            .base()
            .get_global_transform()
            .basis
            .col_a()
            .length()
            .max(1e-3);
        let ahead = reach.min(self.lod_distance * 0.5 * scale);
        Some(o + flat / flat_len * ahead)
    }
    fn apply_lod_view(&mut self, focus: Vector3, eye: Vector3) {
        if self.lod_distance <= 0.0 {
            self.set_lod_chunks(HashSet::new());
            return;
        }
        let local = self.global_to_local(focus);
        let local_eye = self.global_to_local(eye);
        let cfg = &self.store.config;
        let radius = cfg
            .chunk_outline(iv2(0, 0))
            .first()
            .map(|p| (*p - cfg.chunk_center_pos(iv2(0, 0))).magnitude())
            .unwrap_or(0.0);
        let low = cfg.lod_low_chunks(
            self.store.chunks.keys().copied(),
            v2(local.x, local.z),
            v2(local_eye.x, local_eye.z),
            self.lod_distance,
            radius * 0.5,
            &self.lod_low,
        );
        self.set_lod_chunks(low);
        let scale = self
            .base()
            .get_global_transform()
            .basis
            .col_a()
            .length()
            .max(1e-3);
        let end = (self.lod_distance - 1.5005 * radius).max(self.lod_distance * 0.5) * scale;
        let band = (end * 0.1505).clamp(1.0, 8.0);
        self.lod_fade = Vector4::new(focus.x, focus.z, end, band);
        self.lod_eye = Vector2::new(eye.x, eye.z);
        crate::globals::assign("pt_lod_eye", &self.lod_eye.to_variant());
        crate::globals::assign("pt_lod_fade", &self.lod_fade.to_variant());
        self.lod_fade_published = true;
    }
    fn set_lod_chunks(&mut self, low: HashSet<IVec2>) {
        if low == self.lod_low {
            return;
        }
        for c in self.lod_low.symmetric_difference(&low) {
            if self.store.has_chunk(*c) {
                self.dirty.insert(*c);
            }
        }
        self.lod_low = low;
    }
    fn global_to_local(&self, p: Vector3) -> Vector3 {
        self.base().get_global_transform().affine_inverse() * p
    }
    fn local_to_global(&self, p: Vector3) -> Vector3 {
        self.base().get_global_transform() * p
    }
}
#[godot_api]
impl MarchingSquaresTerrain {
    #[func]
    fn get_lod_focus(&self) -> Option<Gd<Node3D>> {
        self.lod_focus.clone().filter(|f| f.is_instance_valid())
    }
    #[func]
    fn get_map_window_focus(&self) -> Option<Gd<Node3D>> {
        self.map_window_focus
            .clone()
            .filter(|f| f.is_instance_valid())
    }
    #[func]
    fn set_scatter_group_tiles(&mut self, v: i32) {
        self.regroup_scatter(v);
    }
    #[func]
    fn set_region_collision_cells(&mut self, v: f32) {
        self.region_collision_cells = v.max(1.0);
        self.need_reload = true;
    }
    #[func]
    fn set_region_mesh_cells(&mut self, v: f32) {
        self.region_mesh_cells = v.max(1.0);
        self.need_reload = true;
    }
    #[func]
    fn set_region_max_chunks(&mut self, v: i32) {
        self.region_max_chunks = v.clamp(1, 16);
        self.need_reload = true;
    }
    #[func]
    fn set_lod_focus(&mut self, v: Option<Gd<Node3D>>) {
        self.lod_focus = v;
        self.lod_timer = 0.0;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_map_window_focus(&mut self, v: Option<Gd<Node3D>>) {
        self.map_window_focus = v;
        self.base_mut().update_configuration_warnings();
    }
    #[signal]
    fn chunks_rebuilt(chunks: Array<Vector2i>);
    #[signal]
    fn map_window_changed(window: Rect2i);
    #[signal]
    fn export_progressed(done: i64, total: i64);
    #[signal]
    fn export_finished(path: GString, error: i64);
    #[func]
    fn set_chunk_handles(&mut self, v: bool) {
        self.chunk_handles = v;
        self.base_mut().call_deferred("update_gizmos", &[]);
    }
    #[func]
    fn set_waters(&mut self, v: Array<Option<Gd<crate::effects::water::PixelWater>>>) {
        self.waters = v;
        self.surroundings_changed();
    }
    #[func]
    fn set_builder_meshes(&mut self, v: Array<Option<Gd<crate::builder::PixelBuilderMesh>>>) {
        self.builder_meshes = v;
        self.surroundings_changed();
    }
    #[func]
    fn set_handle_chunk(&mut self, v: Vector2i) {
        self.handle_chunk = v;
        self.base_mut().call_deferred("update_gizmos", &[]);
    }
    #[func]
    fn set_storage_mode(&mut self, v: i32) {
        self.storage_set_storage_mode(v);
    }
    #[func]
    fn set_bake_grass(&mut self, v: bool) {
        self.storage_set_bake_grass(v);
    }
    #[func]
    fn assign_bake_impact(&mut self, v: bool) {
        self.store_assign_bake_impact(v);
    }
    #[func]
    fn set_data_directory(&mut self, v: GString) {
        self.storage_set_data_directory(v);
    }
    #[func]
    fn set_data(&mut self, v: Option<Gd<MarchingSquaresTerrainData>>) {
        self.contents = v;
        self.need_reload = true;
    }
    #[func]
    fn set_grid_type(&mut self, v: TerrainGridType) {
        let changed = v != self.grid_type;
        self.grid_type = v;
        self.need_reload = true;
        if changed {
            self.base_mut()
                .call_deferred("notify_property_list_changed", &[]);
        }
    }
    #[func]
    fn set_chunk_size(&mut self, v: i32) {
        if self.loaded && !self.store.chunks.is_empty() && v != self.chunk_size {
            godot_warn!(
                "MarchingSquaresTerrain: chunk_size is fixed once chunks exist for this grid type (clear the data or switch grid type first)."
            );
            return;
        }
        self.chunk_size = v.clamp(2, 64);
        self.need_reload = true;
    }
    #[func]
    fn set_cell_size(&mut self, v: Vector2) {
        self.cell_size = Vector2::new(v.x.max(0.0505), v.y.max(0.0505));
        self.store.config.cell_size = to_v2(self.cell_size);
        self.mark_all_dirty();
    }
    #[func]
    fn set_ring_radius(&mut self, v: i32) {
        if self.hex_layout_locked() && v != self.ring_radius {
            godot_warn!("MarchingSquaresTerrain: ring_radius is fixed once hex-ring chunks exist.");
            return;
        }
        self.ring_radius = v.clamp(1, 16);
        self.need_reload = true;
    }
    #[func]
    fn set_wrap_chunks(&mut self, v: Vector2i) {
        let v = Vector2i::new(v.x.max(0), v.y.max(0));
        if self.hex_layout_locked() && v != self.wrap_chunks {
            godot_warn!("MarchingSquaresTerrain: wrap_chunks is fixed once hex-ring chunks exist.");
            return;
        }
        self.wrap_chunks = v;
        self.need_reload = true;
    }
    #[func]
    fn set_wrap_period(&mut self, v: i32) {
        self.wrap_period = v.max(0);
        self.store.config.wrap_period = self.wrap_period;
        self.mark_all_dirty();
    }
    #[func]
    fn set_hex_edge_style(&mut self, v: HexEdgeStyle) {
        self.hex_edge_style = v;
        self.store.ridges = self.edge_params();
        if self.grid() != GridType::Square {
            self.mark_all_dirty();
        }
        if self.hex_edge_instanced {
            self.need_look = true;
        }
    }
    #[func]
    fn set_hex_edge_instanced(&mut self, v: bool) {
        self.hex_edge_instanced = v;
        self.need_look = true;
    }
    #[func]
    fn set_hex_edge_grooves(&mut self, v: i32) {
        self.hex_edge_grooves = v.clamp(0, 64);
        if self.columns_active() {
            self.need_look = true;
        }
    }
    #[func]
    fn set_level_height(&mut self, v: f32) {
        self.level_height = v;
        self.store.ridges = self.edge_params();
        if self.grid() != GridType::Square && matches!(
                self.hex_edge_style,
                HexEdgeStyle::Terraces | HexEdgeStyle::HexPrism
            ) {
            self.mark_all_dirty();
        }
    }
    #[func]
    fn set_default_floor_slot(&mut self, v: i32) {
        self.default_floor_slot = v.clamp(0, MAX_SLOT as i32);
        self.store.default_floor = self.default_floor_slot as u8;
    }
    #[func]
    fn set_default_wall_slot(&mut self, v: i32) {
        self.default_wall_slot = v.clamp(0, MAX_SLOT as i32);
        self.store.default_wall = self.default_wall_slot as u8;
    }
    #[func]
    fn set_blend_mode(&mut self, v: i32) {
        self.blend_mode = v.clamp(0, 2);
        self.need_look = true;
    }
    #[func]
    fn set_texture_set(&mut self, v: Option<Gd<MarchingSquaresTexturePreset>>) {
        self.tex_assign = v;
        self.need_look = true;
    }
    #[func]
    fn assign_land_stuff(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.land_stuff = v;
        self.need_look = true;
    }
    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = v.clamp(1, pt_core::meta::MAX_PIXEL_SIZE as i32);
        self.need_instances = true;
    }
    #[func]
    fn set_outline(&mut self, v: bool) {
        self.outline = v;
        self.need_instances = true;
    }
    #[func]
    fn set_outline_id(&mut self, v: i32) {
        self.outline_id = v.clamp(0, pt_core::meta::MAX_USER_ID as i32);
        self.need_instances = true;
    }
    #[func]
    fn set_render_layers(&mut self, v: u32) {
        self.render_layers = v;
        self.need_instances = true;
    }
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        self.need_instances = true;
        self.mark_all_dirty();
        self.base_mut()
            .call_deferred("update_configuration_warnings", &[]);
    }
    #[func]
    fn set_grass_enabled(&mut self, v: bool) {
        self.grass_enabled = v;
        self.mark_all_dirty();
    }
    #[func]
    fn set_grass_subdivisions(&mut self, v: i32) {
        self.grass_subdivisions = v.clamp(1, 8);
        self.mark_all_dirty();
    }
    #[func]
    fn set_grass_size(&mut self, v: Vector2) {
        self.grass_size = v;
        self.mark_all_dirty();
    }
    #[func]
    fn set_grass_texture(&mut self, v: Option<Gd<Texture2D>>) {
        self.grass_texture = v;
        self.need_look = true;
    }
    #[func]
    fn assign_turf_stuff(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.turf_stuff = v;
        self.need_look = true;
    }
    #[func]
    fn set_grass_pixel_size(&mut self, v: i32) {
        self.grass_pixel_size = v.clamp(1, pt_core::meta::MAX_PIXEL_SIZE as i32);
        self.need_instances = true;
    }
    #[func]
    fn set_grass_outline(&mut self, v: bool) {
        self.grass_outline = v;
        self.need_instances = true;
    }
    #[func]
    fn set_grass_ledge_threshold(&mut self, v: f32) {
        self.grass_ledge_threshold = v.clamp(0.0, 1.0);
        self.mark_all_dirty();
    }
    #[func]
    fn set_grass_min_height(&mut self, v: f32) {
        self.grass_min_height = v;
        self.mark_all_dirty();
    }
    #[func]
    fn assign_impact_active(&mut self, v: bool) {
        self.impact_active = v;
        self.need_instances = true;
    }
    #[func]
    fn assign_impact_stratum(&mut self, v: u32) {
        self.impact_stratum = v;
        self.need_instances = true;
    }
    #[func]
    fn assign_impact_sieve(&mut self, v: u32) {
        self.impact_sieve = v;
        self.need_instances = true;
    }
    #[func]
    fn set_publish_height_texture(&mut self, v: bool) {
        self.publish_height_texture = v;
        self.need_height = true;
        self.height_full = true;
    }
    #[func]
    fn set_publish_sea_pad(&mut self, v: f32) {
        self.publish_sea_pad = v;
        self.need_height = true;
    }
    #[func]
    fn set_publish_sea_falloff(&mut self, v: f32) {
        self.publish_sea_falloff = v;
        self.need_height = true;
    }
    #[func]
    fn refresh_pixel_settings(&mut self) {
        self.need_instances = true;
    }
    #[func]
    pub fn update_lod_at(&mut self, focus: Vector3) {
        self.apply_lod_view(focus, focus);
        self.update();
    }
    #[func]
    pub fn update_lod_view(&mut self, focus: Vector3, eye: Vector3) {
        self.apply_lod_view(focus, eye);
        self.update();
    }
    #[func]
    pub fn get_lod_eye(&self) -> Vector2 {
        self.lod_eye
    }
    #[func]
    pub fn get_lod_fade(&self) -> Vector4 {
        if self.lod_fade_published {
            self.lod_fade
        } else {
            Vector4::ZERO
        }
    }
    #[func]
    pub fn get_low_detail_chunks(&self) -> Array<Vector2i> {
        let mut v: Vec<IVec2> = self.lod_low.iter().copied().collect();
        v.sort_by_key(|c| (c.y, c.x));
        v.into_iter().map(from_iv).collect()
    }
    #[func]
    fn set_lod_distance(&mut self, v: f32) {
        self.lod_distance = v.max(0.0);
        self.lod_timer = 0.0;
    }
    #[func]
    fn set_lod_in_editor(&mut self, v: bool) {
        self.lod_in_editor = v;
        self.lod_timer = 0.0;
    }
    #[func]
    pub fn refresh_grass_water(&mut self) {
        let _p = crate::prof::breadth("terrain.refresh_grass_water");
        if !self.grass_on()
            && self.scatter.assign.check_vacant()
            && self.cleared_cells.is_empty()
            && self.grass_water.check_vacant()
        {
            return;
        }
        let new = self.collect_grass_water();
        let old = std::mem::replace(&mut self.grass_water, new);
        if old.tiles != self.grass_water.tiles
            || old.footprints != self.grass_water.footprints
            || old.blocked != self.grass_water.blocked
        {
            self.scatter.mark_all();
        }
        let mut touched = Touched::new();
        for c in old.blocked.symmetric_difference(&self.grass_water.blocked) {
            touched.extend(self.store.chunks_affected_by(*c));
        }
        let cur = &self.grass_water.tiles;
        for c in old.tiles.keys().chain(cur.keys()) {
            let (a, b) = (old.tiles.get(c), cur.get(c));
            if a != b {
                touched.extend(self.store.chunks_affected_by(*c));
            }
        }
        if old.footprints != self.grass_water.footprints {
            let cfg = &self.store.config;
            let step = cfg.cell_size_safe().x.min(cfg.cell_size_safe().y) * 0.5;
            for poly in old
                .footprints
                .iter()
                .chain(self.grass_water.footprints.iter())
            {
                let lo = poly.iter().fold(v2(f32::MAX, f32::MAX), |a, p| {
                    v2(a.x.min(p.x), a.y.min(p.y))
                });
                let hi = poly.iter().fold(v2(f32::MIN, f32::MIN), |a, p| {
                    v2(a.x.max(p.x), a.y.max(p.y))
                });
                let (mut x, mut cells) = (lo.x - step, HashSet::new());
                while x <= hi.x + step {
                    let mut z = lo.y - step;
                    while z <= hi.y + step {
                        cells.insert(cfg.world_to_sample(v2(x, z)));
                        z += step;
                    }
                    x += step;
                }
                for c in cells {
                    touched.extend(self.store.chunks_affected_by(c));
                }
            }
        }
        if !touched.is_empty() {
            for c in &touched {
                self.dirty.insert(*c);
            }
            self.update();
        }
    }
    #[func]
    pub fn set_cells_cleared(&mut self, cells: Array<Variant>, cleared: bool) {
        let mut changed = false;
        for c in cells
            .iter_shared()
            .filter_map(|v| v.try_to::<Vector2i>().ok())
        {
            let c = to_iv(c);
            changed |= if cleared {
                self.cleared_cells.insert(c)
            } else {
                self.cleared_cells.remove(&c)
            };
        }
        if changed {
            self.refresh_grass_water();
            if !self.grass_on() {
                self.update();
            }
        }
    }
    #[func]
    pub fn get_cleared_cells(&self) -> Array<Vector2i> {
        let mut v: Vec<IVec2> = self.cleared_cells.iter().copied().collect();
        v.sort_by_key(|c| (c.y, c.x));
        v.into_iter().map(from_iv).collect()
    }
    #[func]
    pub fn reassemble(&mut self) {
        self.need_reload = true;
        self.update();
    }
    #[func]
    pub fn add_chunk(&mut self, coords: Vector2i) -> bool {
        let c = self.store.config.wrap_chunk(to_iv(coords));
        if self.store.has_chunk(c) {
            return false;
        }
        let merge = MergeMode::from_u8(self.default_merge_mode.clamp(0, 4) as u8);
        if let Some(noise) = self.noise_hmap.clone() {
            let amp = self.noise_hmap_height;
            let square = self.store.config.grid == GridType::Square;
            self.store.add_chunk_with_heights(c, merge, |g, p| {
                let (x, z) = if square {
                    (g.x as f32, g.y as f32)
                } else {
                    (p.x, p.y)
                };
                noise.get_noise_2d(x, z) * amp
            });
        } else if self.new_chunk_noise_amplitude.abs() > 1e-4 {
            self.store.add_chunk_with_noise(
                c,
                merge,
                self.new_chunk_noise_amplitude,
                self.new_chunk_noise_frequency,
                self.noise_seed as u32,
            );
        } else {
            self.store.add_chunk(c, merge);
        }
        self.store.sync_square_borders(c);
        self.write_chunks(&[c]);
        self.chunk_replaced(c);
        self.base_mut().update_configuration_warnings();
        true
    }
    #[func]
    pub fn remove_chunk(&mut self, coords: Vector2i) -> bool {
        let c = to_iv(coords);
        if !self.store.has_chunk(c) {
            return false;
        }
        self.restore_chunk(c, None);
        true
    }
    #[func]
    pub fn has_chunk(&self, coords: Vector2i) -> bool {
        self.store.has_chunk(to_iv(coords))
    }
    #[func]
    pub fn get_chunks(&self) -> Array<Vector2i> {
        self.store.chunk_coords().into_iter().map(from_iv).collect()
    }
    #[func]
    pub fn get_chunk_data(&self, coords: Vector2i) -> PackedByteArray {
        self.chunk_bytes(to_iv(coords))
            .map(|b| PackedByteArray::from(b.as_slice()))
            .unwrap_or_default()
    }
    #[func]
    pub fn set_chunk_data(&mut self, coords: Vector2i, bytes: PackedByteArray) {
        let b = bytes.to_vec();
        self.restore_chunk(to_iv(coords), if b.is_empty() { None } else { Some(&b) });
    }
    #[func]
    pub fn set_chunks_data(&mut self, blobs: VarDictionary) {
        for (k, v) in blobs.iter_shared() {
            if let (Ok(c), Ok(b)) = (k.try_to::<Vector2i>(), v.try_to::<PackedByteArray>()) {
                let b = b.to_vec();
                self.restore_chunk(to_iv(c), if b.is_empty() { None } else { Some(&b) });
            }
        }
        self.update();
    }
    #[func]
    pub fn world_to_chunk(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.chunk_at(v2(l.x, l.z)))
    }
    #[func]
    pub fn world_to_sample(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.world_to_sample(v2(l.x, l.z)))
    }
    #[func]
    pub fn sample_to_world(&self, sample: Vector2i) -> Vector3 {
        let s = to_iv(sample);
        let p = self.store.config.sample_pos(s);
        let h = self.store.stature(s).unwrap_or(0.0);
        self.local_to_global(Vector3::new(p.x, h, p.y))
    }
    #[func]
    pub fn get_height_at(&self, world: Vector3, fallback: f32) -> f32 {
        let l = self.global_to_local(world);
        match self.local_height_at(v2(l.x, l.z)) {
            Some(h) => self.local_to_global(Vector3::new(l.x, h, l.z)).y,
            None => fallback,
        }
    }
    #[func]
    pub fn has_ground_at(&self, world: Vector3) -> bool {
        let l = self.global_to_local(world);
        self.local_height_at(v2(l.x, l.z)).is_some()
    }
    #[func]
    pub fn ray_hit(&self, from: Vector3, direction: Vector3, max_distance: f32) -> VarDictionary {
        let mut d = VarDictionary::new();
        let t = self.base().get_global_transform();
        let inv = t.affine_inverse();
        let o = inv * from;
        let dir = inv.basis * direction;
        match self.local_raycast(o, dir, max_distance) {
            Some(hit) => {
                d.set("hit", true);
                d.set("position", t * hit);
                let probe = hit + dir.normalized() * 0.0105;
                let cell = self.store.config.world_to_sample(v2(probe.x, probe.z));
                d.set("sample", from_iv(cell));
                d.set("cell", from_iv(self.store.config.wrap_sample(cell)));
                d.set("chunk", from_iv(self.store.config.chunk_of(cell)));
            }
            None => {
                d.set("hit", false);
            }
        }
        d
    }
    #[func]
    pub fn get_sample_height(&self, sample: Vector2i) -> f32 {
        self.store.stature(to_iv(sample)).unwrap_or(0.0)
    }
    #[func]
    pub fn set_sample_height(&mut self, sample: Vector2i, height: f32) {
        let s = to_iv(sample);
        let t = self.edit(|st| {
            let mut t = Touched::new();
            if st.set_height(s, height) {
                t.extend(st.chunks_affected_by(s));
            }
            t
        });
        let _ = t;
    }
    #[func]
    pub fn get_sample_smooth(&self, sample: Vector2i) -> bool {
        self.store
            .probe(to_iv(sample))
            .map(|s| s.glide)
            .unwrap_or(false)
    }
    #[func]
    pub fn set_sample_smooth(&mut self, sample: Vector2i, smooth: bool) {
        let s = to_iv(sample);
        self.edit(|st| brush::op_paint(st, &[(s, 1.0)], PaintTarget::Smooth, smooth as u8));
    }
    #[func]
    pub fn apply_bridge(&mut self, start_pt: Vector3, to: Vector3, breadth: f32, ease_curve: f32) {
        let b = Brush {
            radius: (breadth * 0.5).max(0.0505),
            ..Default::default()
        };
        let (a, z) = (
            pt_core::math::v3(start_pt.x, start_pt.y, start_pt.z),
            pt_core::math::v3(to.x, to.y, to.z),
        );
        self.edit(|st| brush::op_bridge(st, &b, a, z, ease_curve));
    }
    #[func]
    pub fn get_cell_level(&self, cell: Vector2i) -> i32 {
        (self.get_sample_height(cell) / self.level_height_safe()).round() as i32
    }
    #[func]
    pub fn set_cell_level(&mut self, cell: Vector2i, level: i32) {
        let h = level as f32 * self.level_height_safe();
        self.set_sample_height(cell, h);
    }
    #[func]
    pub fn get_chunk_cells(&self, chunk: Vector2i) -> Array<Vector2i> {
        self.store
            .config
            .chunk_cells(to_iv(chunk))
            .into_iter()
            .map(from_iv)
            .collect()
    }
    #[func]
    pub fn get_chunk_center_cell(&self, chunk: Vector2i) -> Vector2i {
        from_iv(self.store.config.chunk_center_cell(to_iv(chunk)))
    }
    #[func]
    pub fn get_cell_chunk(&self, cell: Vector2i) -> Vector2i {
        from_iv(self.store.config.chunk_of(to_iv(cell)))
    }
    #[func]
    pub fn get_cell_neighbors(&self, cell: Vector2i) -> Array<Vector2i> {
        self.store
            .config
            .edge_neighbors(to_iv(cell))
            .into_iter()
            .map(from_iv)
            .collect()
    }
    #[func]
    pub fn get_chunk_cells_spiral(&self, chunk: Vector2i) -> Array<Vector2i> {
        self.store
            .config
            .chunk_cells_spiral(to_iv(chunk))
            .into_iter()
            .map(from_iv)
            .collect()
    }
    #[func]
    pub fn chunk_to_world(&self, chunk: Vector2i) -> Vector3 {
        self.sample_to_world(self.get_chunk_center_cell(chunk))
    }
    #[func]
    pub fn world_to_cell(&self, world: Vector3) -> Vector2i {
        self.world_to_sample(world)
    }
    #[func]
    pub fn cell_to_world(&self, cell: Vector2i) -> Vector3 {
        self.sample_to_world(cell)
    }
    #[func]
    pub fn get_cell_decoration_spots(&self, cell: Vector2i, min_height: f32) -> PackedVector3Array {
        let t = self.base().get_global_transform();
        self.decoration_spots_local(to_iv(cell), min_height)
            .into_iter()
            .map(|(_, p)| t * Vector3::new(p.x, p.y, p.z))
            .collect()
    }
    #[func]
    pub fn get_cell_height(&self, cell: Vector2i) -> f32 {
        self.get_sample_height(cell)
    }
    #[func]
    pub fn set_cell_height(&mut self, cell: Vector2i, height: f32) {
        self.set_sample_height(cell, height);
    }
    #[func]
    pub fn wrap_cell(&self, cell: Vector2i) -> Vector2i {
        from_iv(self.store.config.wrap_sample(to_iv(cell)))
    }
    #[func]
    pub fn wrap_chunk(&self, chunk: Vector2i) -> Vector2i {
        from_iv(self.store.config.wrap_chunk(to_iv(chunk)))
    }
    #[func]
    pub fn get_cell_heights(&self, cells: Array<Vector2i>) -> PackedFloat32Array {
        cells
            .iter_shared()
            .map(|c| self.store.stature(to_iv(c)).unwrap_or(0.0))
            .collect()
    }
    #[func]
    pub fn set_cell_heights(&mut self, cells: Array<Vector2i>, heights: PackedFloat32Array) {
        let cells: Vec<IVec2> = cells.iter_shared().map(to_iv).collect();
        let hs = heights.as_slice().to_vec();
        self.edit(|st| {
            let mut t = Touched::new();
            for (g, h) in cells.iter().zip(hs) {
                if st.set_height(*g, h) {
                    t.extend(st.chunks_affected_by(*g));
                }
            }
            t
        });
    }
    #[func]
    pub fn set_cell_levels(&mut self, cells: Array<Vector2i>, levels: PackedInt32Array) {
        let lh = self.level_height_safe();
        let hs: PackedFloat32Array = levels.as_slice().iter().map(|l| *l as f32 * lh).collect();
        self.set_cell_heights(cells, hs);
    }
    #[func]
    pub fn world_to_small_hex(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.world_to_small_hex(v2(l.x, l.z)))
    }
    #[func]
    pub fn small_hex_to_world(&self, small: Vector2i, fallback: f32) -> Vector3 {
        let p = self.store.config.small_hex_pos(to_iv(small));
        let h = self.local_height_at(p).unwrap_or(fallback);
        self.local_to_global(Vector3::new(p.x, h, p.y))
    }
    #[func]
    pub fn small_hex_to_cell(&self, small: Vector2i) -> Vector2i {
        from_iv(self.store.config.small_hex_owner(to_iv(small)))
    }
    #[func]
    pub fn get_chunk_heights(&self, chunk: Vector2i) -> PackedFloat32Array {
        let c = to_iv(chunk);
        if !self.store.has_chunk(c) {
            return PackedFloat32Array::new();
        }
        self.store
            .config
            .chunk_cells(c)
            .into_iter()
            .map(|g| self.store.stature(g).unwrap_or(0.0))
            .collect()
    }
    #[func]
    pub fn set_chunk_heights(&mut self, chunk: Vector2i, heights: PackedFloat32Array) {
        let cells = self.store.config.chunk_cells(to_iv(chunk));
        let hs = heights.as_slice().to_vec();
        self.edit(|st| {
            let mut t = Touched::new();
            for (g, h) in cells.iter().zip(hs) {
                if st.set_height(*g, h) {
                    t.extend(st.chunks_affected_by(*g));
                }
            }
            t
        });
    }
    #[func]
    pub fn set_chunk_levels(&mut self, chunk: Vector2i, levels: PackedInt32Array) {
        let lh = self.level_height_safe();
        let hs: PackedFloat32Array = levels.as_slice().iter().map(|l| *l as f32 * lh).collect();
        self.set_chunk_heights(chunk, hs);
    }
    #[func]
    pub fn get_chunk_slots(&self, chunk: Vector2i, wall: bool) -> PackedByteArray {
        let c = to_iv(chunk);
        if !self.store.has_chunk(c) {
            return PackedByteArray::new();
        }
        self.store
            .config
            .chunk_cells(c)
            .into_iter()
            .map(|g| {
                self.store
                    .probe(g)
                    .map(|s| if wall { s.wall } else { s.floor })
                    .unwrap_or(0)
            })
            .collect()
    }
    #[func]
    pub fn set_chunk_slots(&mut self, chunk: Vector2i, slots: PackedByteArray, wall: bool) {
        let cells = self.store.config.chunk_cells(to_iv(chunk));
        let target = if wall {
            PaintTarget::Wall
        } else {
            PaintTarget::Floor
        };
        let v = slots.as_slice().to_vec();
        self.edit(|st| {
            let mut t = Touched::new();
            for (g, s) in cells.iter().zip(v) {
                t.extend(brush::op_paint(st, &[(*g, 1.0)], target, s.min(MAX_SLOT)));
            }
            t
        });
    }
    #[func]
    pub fn get_sample_slot(&self, sample: Vector2i, wall: bool) -> i32 {
        self.store
            .probe(to_iv(sample))
            .map(|s| if wall { s.wall } else { s.floor } as i32)
            .unwrap_or(-1)
    }
    #[func]
    pub fn set_sample_slot(&mut self, sample: Vector2i, slot: i32, wall: bool) {
        let s = to_iv(sample);
        let target = if wall {
            PaintTarget::Wall
        } else {
            PaintTarget::Floor
        };
        self.edit(|st| {
            brush::op_paint(
                st,
                &[(s, 1.0)],
                target,
                slot.clamp(0, MAX_SLOT as i32) as u8,
            )
        });
    }
    #[func]
    pub fn apply_brush(
        &mut self,
        op: GString,
        world: Vector3,
        radius: f32,
        amount: f32,
        shape: i64,
    ) {
        let l = self.global_to_local(world);
        let b = Brush {
            form: BrushShape::from_i64(shape),
            radius,
            ..Default::default()
        };
        let fp = brush::footprint(&self.store.config, &b, v2(l.x, l.z));
        let op = op.to_string();
        self.edit(|st| match op.as_str() {
            "raise" => brush::op_raise(st, &fp, amount.abs(), Clamp::default()),
            "lower" => brush::op_raise(st, &fp, -amount.abs(), Clamp::default()),
            "level" => brush::op_level(st, &fp, amount, 1.0),
            "smooth" => brush::op_smooth(st, &fp, amount.clamp(0.0, 1.0)),
            "paint" => brush::op_paint(
                st,
                &fp,
                PaintTarget::Floor,
                amount.clamp(0.0, MAX_SLOT as f32) as u8,
            ),
            "paint_wall" => brush::op_paint(
                st,
                &fp,
                PaintTarget::Wall,
                amount.clamp(0.0, MAX_SLOT as f32) as u8,
            ),
            "grass_on" => brush::op_paint(st, &fp, PaintTarget::Grass, 255),
            "grass_off" => brush::op_paint(st, &fp, PaintTarget::Grass, 0),
            "smooth_on" => brush::op_paint(st, &fp, PaintTarget::Smooth, 1),
            "smooth_off" => brush::op_paint(st, &fp, PaintTarget::Smooth, 0),
            _ => {
                godot_warn!("MarchingSquaresTerrain.apply_brush: unknown op '{op}'");
                Touched::new()
            }
        });
    }
    #[func]
    pub fn commit_to_data(&mut self) {
        let all: Vec<IVec2> = self.store.chunk_coords();
        self.write_chunks(&all);
    }
    #[func]
    pub fn get_grass_count(&self) -> i64 {
        self.nodes
            .values()
            .map(|n| (n.grass.len() / batches::GRASS_STRIDE) as i64)
            .sum()
    }
    #[func]
    pub fn get_chunk_grass_count(&self, chunk: Vector2i) -> i64 {
        self.nodes
            .get(&to_iv(chunk))
            .map(|n| (n.grass.len() / batches::GRASS_STRIDE) as i64)
            .unwrap_or(0)
    }
    #[func]
    pub fn get_chunk_mesh(&self, chunk: Vector2i) -> Option<Gd<godot::classes::Mesh>> {
        self.chunk_mesh(to_iv(chunk))
    }
    #[func]
    pub fn get_chunk_collision_faces(&self, chunk: Vector2i) -> PackedVector3Array {
        self.chunk_collision_faces(to_iv(chunk))
    }
}
struct HeightCache {
    token: (PVec2, i32, i32, f32, f32, Vector3, Vector3),
    src: Vec<Option<f32>>,
    published_ms: u64,
    sea: Option<pt_core::heightmap::SeaFloor>,
}
