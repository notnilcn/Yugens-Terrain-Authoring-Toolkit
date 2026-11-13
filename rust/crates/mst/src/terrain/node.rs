//! `MarchingSquaresTerrain` — the terrain node. Owns the in-memory sample store, rebuilds chunk meshes
//! (in parallel) when samples change, scatters grass, builds collision, and publishes a
//! height texture so water/other shaders can read terrain depth without a depth buffer.
//!
//! Chunk nodes are *internal* children: they are never saved into the scene, so scenes stay
//! tiny and there are no save hooks; all persistent state lives in `MarchingSquaresTerrainData`.

use std::collections::{BTreeSet, HashMap, HashSet};

use godot::classes::image::Format;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    CollisionShape3D, ConcavePolygonShape3D, INode3D, Image, ImageTexture, MeshInstance3D, MultiMesh, MultiMeshInstance3D,
    Node3D, Noise, QuadMesh, ShaderMaterial, StaticBody3D, Texture2D, Texture2DArray,
};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::prelude::*;

use pt_core::brush::{self, Brush, BrushShape, Clamp, PaintTarget, Touched};
use pt_core::data::{ChunkData, MergeMode, TerrainStore};
use pt_core::grass::{GrassInstance, GrassParams, WaterCull};
use pt_core::grid::{GridConfig, GridType};
use pt_core::math::{IVec2, Vec2 as PVec2, iv2, v2};
use pt_core::mesh::MeshArrays;
use pt_core::square::BlendMode;

use super::data::MarchingSquaresTerrainData;
use super::storage::BakedChunk;
use super::textures::MarchingSquaresTexturePreset;
use crate::globals;
use crate::util::*;

// Grid overlay, brush cursor, cell data (fog of war / tint), auto rules, debug views.
#[path = "overlay.rs"]
mod overlay;
// Storage modes, importer and static bake (same type, separate file).
#[path = "node_storage.rs"]
mod node_storage;
// GPU-instanced HexPrism columns (`hex_edge_instanced`).
#[path = "hex_columns.rs"]
mod hex_columns;
// Height / water / biome map textures, water levels, blocked small hexes (hex game API).
#[path = "hex_maps.rs"]
mod hex_maps;


struct ChunkNodes {
    root: Gd<Node3D>,
    mesh: Gd<MeshInstance3D>,
    body: Gd<StaticBody3D>,
    shape: Gd<CollisionShape3D>,
    grass: Gd<MultiMeshInstance3D>,
    /// Instanced prism columns (only while `hex_edge_instanced` is active).
    columns: Option<Gd<MultiMeshInstance3D>>,
}

struct Built {
    coord: IVec2,
    arrays: MeshArrays,
    grass: Vec<GrassInstance>,
    /// Drawn by instanced columns: `arrays` is only the collision / grass surface.
    columns: bool,
}

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct MarchingSquaresTerrain {
    /// Samples of every chunk. Save it as a `.res` file to keep scenes small.
    #[export]
    #[var(set = set_data)]
    pub(super) data: Option<Gd<MarchingSquaresTerrainData>>,

    #[export_group(name = "Storage")]
    /// Resource = all chunks in the `data` resource (save it as `.res`). Baked / Runtime =
    /// the original's directory storage: one `metadata.res` per chunk under
    /// `data_directory/<grid>/chunk_X_Y/`, written when the scene is saved (only changed
    /// chunks). Baked files also hold the chunk mesh (+ grass / collision) and load without
    /// meshing; Runtime files hold only the samples (smallest).
    #[export(enum = (Resource, Baked, Runtime))]
    #[var(set = set_storage_mode)]
    storage_mode: i32,
    /// Baked: also store the grass multimeshes.
    #[export]
    #[var(set = set_bake_grass)]
    bake_grass: bool,
    /// Baked: also store the collision faces.
    #[export]
    #[var(set = set_bake_collision)]
    bake_collision: bool,
    /// Directory storage folder. Empty = generated on the first save as
    /// `<scene dir>/<scene name>_TerrainData/<node name>_<id>/` (like the original).
    #[export(dir)]
    #[var(set = set_data_directory)]
    data_directory: GString,
    #[export_tool_button(fn = Self::tool_save_terrain_data, name = "Save Terrain Data", icon = "Save")]
    save_data_button: PhantomVar<Callable>,

    #[export_group(name = "Grid")]
    /// Square = marching squares over vertex heights. Triangle/Hex/HexRings = cells at their own height.
    #[export(enum = (Square, Triangle, Hex, HexRings))]
    #[var(set = set_grid_type)]
    grid_type: i32,
    /// Cells per chunk edge.
    #[export(range = (2.0, 64.0, 1.0))]
    #[var(set = set_chunk_size)]
    chunk_size: i32,
    /// Cell size in world units (hex/triangle cells use `x` only and stay regular).
    #[export]
    #[var(set = set_cell_size)]
    cell_size: Vector2,
    /// Hex rings: radius of the hex disk each chunk owns.
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = set_ring_radius)]
    ring_radius: i32,
    /// Hex rings: torus wrap in chunks (0 = no wrap).
    #[export]
    #[var(set = set_wrap_chunks)]
    wrap_chunks: Vector2i,
    /// Hex rings: torus wrap as an axial cell period P (cell `(q, r)` == `(q mod P, r mod P)`,
    /// the kg_hex `Torus` rule; the game uses `P = 19k` with `ring_radius = 2`). Must be a
    /// multiple of the chunk area `3R(R+1)+1`. 0 = off; overrides `wrap_chunks`.
    #[export]
    #[var(set = set_wrap_period)]
    wrap_period: i32,
    // Inspector labels: subgroup prefixes shorten the long names ("Level Height" -> Levels ›
    // "Height", "Hex Edge Style" -> Hex Edges › "Style", "Default Floor Slot" -> Defaults ›
    // "Floor Slot") so they are not cut off in a narrow inspector. Saved names are unchanged.
    #[export_subgroup(name = "Levels", prefix = "level_")]
    /// World height of one integer level for the `*_cell_level*` API (game elevation units).
    /// With `hex_edge_style = Terraces`, steps of up to one level become terraces.
    #[export]
    #[var(set = set_level_height)]
    level_height: f32,
    /// Triangle/hex grids: how height steps between cells are closed. FlatWalls = ramps up to
    /// the chunk's merge threshold, vertical cliffs above. Terraces = steps up to the merge
    /// threshold become stepped terraces (rock risers, grassy treads), larger ones cliffs.
    /// Slopes = ramps, and cliffs become steep sloped rock faces. HexPrism = stacked prism
    /// columns (BitCraft / kg_hex look): flat tops, every step a vertical prism side with a
    /// bevelled groove per `level_height`, smooth flags ignored. Height queries, raycasts,
    /// grass and collision follow the meshed surface in every style.
    #[export_subgroup(name = "Hex Edges", prefix = "hex_edge_")]
    #[export(enum = (FlatWalls, Terraces, Slopes, HexPrism))]
    #[var(set = set_hex_edge_style)]
    hex_edge_style: i32,
    /// HexPrism on hex grids: draw the columns as GPU instances (one MultiMesh instance per
    /// cell, BitCraft-style) instead of a meshed chunk. The vertex shader rebuilds each column
    /// (bevelled rim, sides, grooves, blending, ledge bands) from the terrain's cell map
    /// texture, so height edits only re-upload the map; collision and grass follow the same
    /// bevelled column tops. Same look as the meshed prisms (unit-tested).
    /// Bake / Export GLB mesh the prisms on the fly.
    #[export]
    #[var(set = set_hex_edge_instanced)]
    hex_edge_instanced: bool,
    /// Instanced prisms: most groove rings per side. Each chunk draws as many as its tallest
    /// exposed side needs (rounded up to a power of two), up to this; deeper grooves become
    /// plain wall. More rings = more vertices per instance on tall cliffs only.
    #[export(range = (0.0, 64.0, 1.0))]
    #[var(set = set_hex_edge_grooves)]
    hex_edge_grooves: i32,
    /// Texture blending between slots.
    #[export_subgroup(name = "Blending", prefix = "blend_")]
    #[export(enum = (Smooth, HardSquares, HardVertices))]
    #[var(set = set_blend_mode)]
    blend_mode: i32,
    /// Merge mode for new chunks: the height step below which slopes/ramps replace walls.
    #[export_subgroup(name = "Defaults", prefix = "default_")]
    #[export(enum = (Cubic, Polyhedron, RoundedPolyhedron, SemiRound, Spherical))]
    default_merge_mode: i32,
    /// Floor slot for new chunks.
    #[export(range = (0.0, 15.0, 1.0))]
    default_floor_slot: i32,
    /// Wall slot for new chunks.
    #[export(range = (0.0, 15.0, 1.0))]
    default_wall_slot: i32,

    #[export_group(name = "New Chunks")]
    #[export_subgroup(name = "Noise", prefix = "new_chunk_noise_")]
    /// Height amplitude of fractal noise applied to newly added chunks (0 = flat).
    #[export]
    new_chunk_noise_amplitude: f32,
    #[export]
    new_chunk_noise_frequency: f32,
    #[export_subgroup(name = "Noise", prefix = "noise_")]
    #[export]
    noise_seed: i64,
    /// Noise for new chunks (the original's `noise_hmap`): heights = noise x `noise_hmap_height`,
    /// sampled at the global vertex index (square) or the cell centre (cell grids). Overrides
    /// the fractal noise above when set.
    #[export]
    noise_hmap: Option<Gd<Noise>>,
    /// Height range of `noise_hmap` (the original's `dimensions.y`).
    #[export]
    noise_hmap_height: f32,

    #[export_group(name = "Look")]
    #[export]
    #[var(set = set_texture_set)]
    texture_set: Option<Gd<MarchingSquaresTexturePreset>>,
    /// Terrain material (`mst_terrain.gdshader`). Created automatically; edit its parameters freely.
    #[export_subgroup(name = "Terrain", prefix = "terrain_")]
    #[export]
    #[var(set = set_terrain_material)]
    terrain_material: Option<Gd<ShaderMaterial>>,
    /// Pixel size multiplier in the pixel renderer (1-4).
    #[export(range = (1.0, 4.0, 1.0))]
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

    #[export_group(name = "Grass", prefix = "grass_")]
    #[export]
    #[var(set = set_grass_enabled)]
    grass_enabled: bool,
    /// Grass blades per cell = subdivisions^2.
    #[export(range = (1.0, 8.0, 1.0))]
    #[var(set = set_grass_subdivisions)]
    grass_subdivisions: i32,
    #[export]
    #[var(set = set_grass_size)]
    grass_size: Vector2,
    #[export]
    #[var(set = set_grass_texture)]
    grass_texture: Option<Gd<Texture2D>>,
    /// Grass material (`mst_grass.gdshader`). Created automatically.
    #[export]
    #[var(set = set_grass_material)]
    grass_material: Option<Gd<ShaderMaterial>>,
    #[export(range = (1.0, 4.0, 1.0))]
    #[var(set = set_grass_pixel_size)]
    grass_pixel_size: i32,
    #[export]
    #[var(set = set_grass_outline)]
    grass_outline: bool,
    /// Blades are not planted on cliff-edge bands stronger than this.
    #[export(range = (0.0, 1.0))]
    #[var(set = set_grass_ledge_threshold)]
    grass_ledge_threshold: f32,
    /// No grass below this terrain-local height (set it to your water level).
    #[export]
    #[var(set = set_grass_min_height)]
    grass_min_height: f32,

    #[export_group(name = "Scatter")]
    /// Prop kinds painted with the Scatter tool (instances refer to items by index).
    #[export]
    pub(super) scatter_items: Array<Option<Gd<super::scatter::PixelScatterItem>>>,

    #[export_group(name = "Distant LOD")]
    /// Distant low-detail ring: chunks whose centre is farther than this (terrain units, XZ)
    /// from the LOD focus are meshed at low detail: flat cell tops and plain walls (no ramps,
    /// prism bevels, grooves or rock strata) and no grass. Cell grids only; square chunks just
    /// drop their grass. Scatter props on low chunks are hidden on every grid. 0 = off.
    #[export(range = (0.0, 4000.0, 1.0, or_greater))]
    #[var(set = set_lod_distance)]
    lod_distance: f32,
    /// Centre of the full-detail area (e.g. the player). Empty: the point the active camera
    /// looks at on the terrain's base plane (or the camera itself when it looks up).
    #[export]
    #[var(get = get_lod_focus)]
    lod_focus: Option<Gd<Node3D>>,
    /// Apply the ring in the editor too (off: the editor always shows full detail).
    #[export]
    #[var(set = set_lod_in_editor)]
    lod_in_editor: bool,

    #[export_group(name = "Hex Maps", prefix = "map_")]
    /// Hex grids: keep a cell map texture (`get_map_texture`: R height, G water level, B
    /// biome = floor slot, A wall slot + 16·grass, -1 = no cell) for a window of this many
    /// cells around `map_window_focus` (else the LOD focus), recentred when the focus comes
    /// within `map_window_margin` cells of its border. 0 = off.
    #[export(range = (0.0, 1024.0, 1.0, or_greater))]
    map_window_radius: i32,
    /// Centre of the map window (e.g. the player). Empty: the LOD focus / camera target.
    #[export]
    #[var(get = get_map_window_focus)]
    map_window_focus: Option<Gd<Node3D>>,
    /// Recentre the window when the focus is this close to its border (cells).
    #[export(range = (0.0, 512.0, 1.0, or_greater))]
    map_window_margin: i32,

    #[export_group(name = "Collision", prefix = "collision_")]
    #[export]
    #[var(set = set_collision_enabled)]
    collision_enabled: bool,
    #[export(flags_3d_physics)]
    #[var(set = set_collision_layer)]
    collision_layer: u32,
    #[export(flags_3d_physics)]
    #[var(set = set_collision_mask)]
    collision_mask: u32,

    #[export_group(name = "Integration", prefix = "publish_")]
    /// Publish `pt_terrain_height` so water shades depth/shorelines from this terrain.
    #[export]
    #[var(set = set_publish_height_texture)]
    publish_height_texture: bool,

    #[export_group(name = "Overlay")]
    #[export_subgroup(name = "Grid", prefix = "show_")]
    /// In-game grid overlay (toggle it from a key, e.g. F2).
    #[export]
    pub show_grid: bool,
    /// Grid overlay while editing.
    #[export]
    pub show_grid_in_editor: bool,
    /// Line color; alpha = opacity.
    #[export_subgroup(name = "Grid", prefix = "grid_")]
    #[export]
    pub grid_color: Color,
    /// Line width in world units (at least one pixel is drawn by the low-res pass).
    #[export(range = (0.005, 2.0, 0.005))]
    pub grid_width: f32,
    /// Lines fade out by this camera distance (0 = no fade).
    #[export(range = (0.0, 2000.0, 1.0))]
    pub grid_fade_distance: f32,
    /// Also draw chunk borders.
    #[export]
    pub grid_chunk_lines: bool,
    /// Draw the editor brush footprint (and `set_cursor`) in the terrain shader.
    #[export]
    pub shader_cursor: bool,
    /// Terrain debug view (editor and game).
    #[export(enum = (Off, Slope, HeightContours, SlotIds, ChunkBorders, CellIds, CellData))]
    pub debug_view: i32,
    /// Height between contour lines of the HeightContours view.
    #[export(range = (0.05, 100.0, 0.05))]
    pub contour_step: f32,

    #[export_group(name = "Fog Of War")]
    #[export_subgroup(name = "Fog", prefix = "fog_")]
    /// Fog of war in game: unexplored cells are dithered fog, explored-but-hidden cells are
    /// dimmed and hatched (`set_cell_visibility` / `set_cell_explored`).
    #[export]
    pub fog_enabled: bool,
    /// Show the fog while editing (otherwise everything is visible in the editor).
    #[export]
    pub fog_in_editor: bool,
    /// Seconds a cell takes to fade in / out.
    #[export(range = (0.0, 5.0, 0.01))]
    pub fog_transition: f32,
    #[export]
    pub fog_color: Color,
    /// Second dither color of unexplored cells.
    #[export]
    pub fog_color_alt: Color,
    /// Brightness of explored-but-hidden cells.
    #[export_subgroup(name = "Hidden Cells", prefix = "fog_hidden_")]
    #[export(range = (0.0, 1.0, 0.01))]
    pub fog_hidden_dim: f32,
    #[export(range = (0.0, 1.0, 0.01))]
    pub fog_hidden_saturation: f32,
    /// Diagonal hatch strength on explored-but-hidden cells.
    #[export_subgroup(name = "Fog", prefix = "fog_")]
    #[export(range = (0.0, 1.0, 0.01))]
    pub fog_hatch: f32,
    /// Width of the dithered transition across cell borders (fraction of a cell).
    #[export(range = (0.0, 0.5, 0.01))]
    pub fog_softness: f32,
    /// Width of the `set_cell_highlight` outline (world units).
    #[export(range = (0.0, 4.0, 0.01))]
    pub highlight_width: f32,

    #[export_group(name = "Tools")]
    /// Target file for "Export GLB" (vertex-colored bake of every chunk).
    #[export(file = "*.glb")]
    export_path: GString,
    #[export_tool_button(fn = Self::tool_export_glb, name = "Export GLB", icon = "Save")]
    export_glb_button: PhantomVar<Callable>,
    #[export_tool_button(fn = Self::rebuild, name = "Rebuild Meshes", icon = "Reload")]
    rebuild_button: PhantomVar<Callable>,
    /// Original MarchingSquaresTerrain data to import: its scene (`.tscn`, reads the grid
    /// settings too), its terrain data directory, or one `metadata.res`. Absolute paths work.
    #[export(global_file = "*.tscn,*.res")]
    import_path: GString,
    #[export_tool_button(fn = Self::tool_import_mst, name = "Import MST Data", icon = "Load")]
    import_button: PhantomVar<Callable>,
    #[export_tool_button(fn = Self::tool_bake_static, name = "Bake Static Meshes", icon = "MeshInstance3D")]
    bake_static_button: PhantomVar<Callable>,
    /// Editor: drag handles on every sample of `handle_chunk` (the original's chunk gizmo).
    /// Chunk Manager's chunk selection (Ctrl+click) picks the chunk.
    #[export]
    #[var(set = set_chunk_handles)]
    chunk_handles: bool,
    /// Chunk whose samples get handles when `chunk_handles` is on.
    #[var(set = set_handle_chunk)]
    handle_chunk: Vector2i,

    store: TerrainStore,
    pub(super) scatter: super::scatter::ScatterLayer,
    nodes: HashMap<IVec2, ChunkNodes>,
    dirty: BTreeSet<IVec2>,
    need_reload: bool,
    need_look: bool,
    need_height: bool,
    need_instances: bool,
    texture_array: Option<Gd<Texture2DArray>>,
    /// Kept alive: their RIDs are bound to the grass material through the RenderingServer.
    grass_sprites: Option<Gd<Texture2DArray>>,
    wind_texture: Option<Gd<Texture2D>>,
    height_texture: Option<Gd<ImageTexture>>,
    look_signature: u64,
    poll_timer: f64,
    overlay: overlay::OverlayState,
    loaded: bool,
    /// Directory storage: baked meshes waiting to be applied by the next flush.
    baked: HashMap<IVec2, BakedChunk>,
    /// Directory storage: grids already read from disk (later reloads use the in-memory data).
    dir_loaded: [bool; 4],
    /// Directory storage: per grid, the content hash each chunk file was last written/read with.
    saved_hash: [HashMap<IVec2, u64>; 4],
    last_save_count: i64,
    baked_loads: i64,
    /// Chunks whose mesh changed through an edit since the last directory save (Baked files
    /// of unedited neighbours must be rewritten too).
    save_pending: BTreeSet<IVec2>,
    /// Chunks meshed at low detail (distant LOD ring); their scatter props are hidden.
    pub(super) lod_low: HashSet<IVec2>,
    lod_timer: f64,
    /// This terrain published `pt_lod_fade` (cleared when its LOD turns off).
    lod_fade_published: bool,
    /// Cells cleared at runtime (`set_cells_cleared`: built on, paved): no grass or props.
    cleared_cells: HashSet<IVec2>,
    /// Raised water cells of the scene's PixelWater nodes: no grass under them.
    pub(super) grass_water: WaterCull,
    columns: hex_columns::ColumnState,
    hexmaps: hex_maps::HexMapState,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for MarchingSquaresTerrain {
    fn init(base: Base<Node3D>) -> Self {
        MarchingSquaresTerrain {
            data: None,
            grid_type: 0,
            chunk_size: 32,
            cell_size: Vector2::new(2.0, 2.0),
            ring_radius: 4,
            wrap_chunks: Vector2i::ZERO,
            wrap_period: 0,
            level_height: 0.5,
            hex_edge_style: 0,
            hex_edge_instanced: false,
            hex_edge_grooves: 32,
            map_window_radius: 0,
            map_window_focus: None,
            map_window_margin: 8,
            columns: Default::default(),
            hexmaps: Default::default(),
            default_merge_mode: 1,
            blend_mode: 0,
            default_floor_slot: 0,
            default_wall_slot: 5,
            new_chunk_noise_amplitude: 0.0,
            new_chunk_noise_frequency: 0.05,
            noise_seed: 1,
            texture_set: None,
            terrain_material: None,
            pixel_size: 1,
            outline: true,
            outline_id: 0,
            render_layers: 1,
            cast_shadows: true,
            grass_enabled: true,
            grass_subdivisions: 2,
            grass_size: Vector2::new(1.0, 1.0),
            grass_texture: None,
            grass_material: None,
            grass_pixel_size: 1,
            grass_outline: false,
            grass_ledge_threshold: 0.5,
            grass_min_height: -1000.0,
            lod_distance: 0.0,
            lod_focus: None,
            lod_in_editor: false,
            lod_low: HashSet::new(),
            grass_water: WaterCull::default(),
            lod_timer: 0.0,
            lod_fade_published: false,
            cleared_cells: HashSet::new(),
            collision_enabled: true,
            collision_layer: 1,
            collision_mask: 0,
            publish_height_texture: true,
            export_path: "res://terrain_export.glb".into(),
            export_glb_button: PhantomVar::default(),
            rebuild_button: PhantomVar::default(),
            import_path: GString::new(),
            import_button: PhantomVar::default(),
            bake_static_button: PhantomVar::default(),
            chunk_handles: false,
            handle_chunk: Vector2i::ZERO,
            storage_mode: 0,
            bake_grass: true,
            bake_collision: true,
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
            scatter: Default::default(),
            nodes: HashMap::new(),
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
            show_grid: false,
            show_grid_in_editor: false,
            grid_color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
            grid_width: 0.08,
            grid_fade_distance: 0.0,
            grid_chunk_lines: false,
            shader_cursor: true,
            debug_view: 0,
            contour_step: 1.0,
            fog_enabled: false,
            fog_in_editor: false,
            fog_transition: 0.35,
            fog_color: Color::from_rgb(0.05, 0.06, 0.09),
            fog_color_alt: Color::from_rgb(0.10, 0.11, 0.16),
            fog_hidden_dim: 0.55,
            fog_hidden_saturation: 0.25,
            fog_hatch: 0.25,
            fog_softness: 0.12,
            highlight_width: 0.25,
            overlay: overlay::OverlayState::default(),
            loaded: false,
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        let mut gd = self.to_gd();
        gd.add_to_group(globals::REFRESH_GROUP);
        crate::features::watch(&gd);
        self.ensure_resources();
        self.ensure_save_hook();
        self.update();
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        if is_editor() {
            self.poll_timer -= delta;
            if self.poll_timer <= 0.0 {
                self.poll_timer = 0.3;
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
    }

    fn exit_tree(&mut self) {
        if std::mem::take(&mut self.lod_fade_published) {
            crate::globals::set("pt_lod_fade", &Vector4::ZERO.to_variant());
        }
        if self.publish_height_texture && self.height_texture.is_some() {
            globals::set("pt_terrain_rect", &Vector4::ZERO.to_variant());
        }
        self.overlay_exit_tree();
    }

    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        use godot::register::info::PropertyUsageFlags as U;
        let name = property.property_name.to_string();
        let strip = |p: &mut godot::register::info::PropertyInfo, f: U| p.usage = U::from_ord(p.usage.ord() & !f.ord());
        match name.as_str() {
            // Directory modes keep the samples on disk: never embed them in the scene.
            "data" if self.directory_mode() => strip(property, U::STORAGE),
            "bake_grass" | "bake_collision" if self.storage_mode != 1 => strip(property, U::EDITOR),
            "data_directory" | "save_data_button" if !self.directory_mode() => strip(property, U::EDITOR),
            // Grid-specific settings only show for the grids they apply to (they are still
            // saved, like the strip's Terrain Settings, which hides the same rows).
            "chunk_size" if self.grid_type == 3 => strip(property, U::EDITOR),
            "ring_radius" | "wrap_chunks" | "wrap_period" if self.grid_type != 3 => strip(property, U::EDITOR),
            "hex_edge_style" if self.grid_type == 0 => strip(property, U::EDITOR),
            "hex_edge_instanced" | "hex_edge_grooves" if self.grid_type < 2 => strip(property, U::EDITOR),
            "map_window_radius" | "map_window_focus" | "map_window_margin" if self.grid_type < 2 => strip(property, U::EDITOR),
            _ => {}
        }
        crate::util::space_enum_hint(property);
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.data.is_none() && !self.directory_mode() {
            w.push("No MarchingSquaresTerrainData assigned: one is created automatically and embedded in the scene. Save it as a .res file to keep the scene small.");
        }
        if self.store.chunks.is_empty() {
            w.push("The terrain has no chunks. Select the terrain and use the Chunks tool (or call add_chunk) to add one.");
        }
        if self.grass_enabled
            && let Some(msg) = crate::features::warning(crate::features::Feature::Grass)
        {
            w.push(&msg.replace("this node draws nothing", "the terrain draws no grass"));
        }
        w
    }
}

impl MarchingSquaresTerrain {
    /// `grass_enabled` unless the project switched the Grass feature off.
    fn grass_on(&self) -> bool {
        self.grass_enabled && crate::features::enabled(crate::features::Feature::Grass)
    }

    pub fn store(&self) -> &TerrainStore {
        &self.store
    }

    fn edge_params(&self) -> pt_core::cells::EdgeParams {
        pt_core::cells::EdgeParams {
            style: pt_core::cells::EdgeStyle::from_i32(self.hex_edge_style),
            terrace_step: self.level_height_safe().abs(),
            ..Default::default()
        }
    }

    fn level_height_safe(&self) -> f32 {
        if self.level_height.abs() < 1e-4 { 1.0 } else { self.level_height }
    }

    pub fn grid(&self) -> GridType {
        GridType::from_i64(self.grid_type as i64)
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
        if self.data.is_none() {
            self.data = Some(MarchingSquaresTerrainData::new_gd());
        }
        if self.texture_set.is_none() {
            self.texture_set = Some(MarchingSquaresTexturePreset::new_default());
        }
        if self.terrain_material.is_none() {
            self.terrain_material = Some(shader_material("mst_terrain.gdshader"));
        }
        if self.grass_material.is_none() {
            self.grass_material = Some(shader_material("mst_grass.gdshader"));
        }
    }

    fn compute_look_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(ts) = &self.texture_set {
            let b = ts.bind();
            b.texture_size.hash(&mut h);
            for i in 0..b.slots.len() {
                if let Some(s) = b.slots.get(i) {
                    let s = s.bind();
                    s.pattern.hash(&mut h);
                    s.texture.as_ref().map(|t| t.instance_id().to_i64()).hash(&mut h);
                    for c in [s.tint, s.grass_tint] {
                        [c.r, c.g, c.b, c.a].map(f32::to_bits).hash(&mut h);
                    }
                    s.scale.to_bits().hash(&mut h);
                    s.has_grass.hash(&mut h);
                    s.grass_sprite.as_ref().map(|t| t.instance_id().to_i64()).hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Applies any pending reload / look / mesh / height work.
    pub fn update(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        self.ensure_resources();
        if self.need_reload {
            self.reload();
        }
        if self.need_look {
            self.apply_look();
        }
        if self.need_instances {
            self.apply_instance_settings();
        }
        if !self.dirty.is_empty() {
            self.flush();
        }
        if self.need_height {
            self.publish_height();
        }
        self.columns_flush_map();
        self.scatter_flush();
    }

    /// Re-reads the data resource for the current grid type and rebuilds everything.
    fn reload(&mut self) {
        self.need_reload = false;
        let grid = self.grid();
        let mut cfg = self.config();
        self.load_directory_grid(grid);
        if let Some(data) = &self.data {
            // The data's recorded layout wins: chunk blobs are only valid for the layout they
            // were authored with.
            if let Some(l) = data.bind().layout_of(grid)
                && data.bind().chunk_count(grid) > 0 {
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
        store.default_floor = self.default_floor_slot.clamp(0, 15) as u8;
        store.default_wall = self.default_wall_slot.clamp(0, 15) as u8;
        store.edges = self.edge_params();
        if let Some(data) = &self.data {
            data.bind().load_into(&mut store);
        }
        self.store = store;
        for (_, n) in self.nodes.drain() {
            free_node(Some(n.root));
        }
        self.dirty = self.store.chunks.keys().copied().collect();
        self.scatter_reload();
        self.painted_tints_reload();
        self.need_height = true;
        self.columns.need_map = true;
        self.hexmaps.dirty = true;
        // Grid type changes switch the instanced-column material variant on / off.
        self.columns_sync_material();
        self.loaded = true;
        self.base_mut().update_configuration_warnings();
    }

    fn apply_look(&mut self) {
        self.need_look = false;
        self.look_signature = self.compute_look_signature();
        let Some(ts) = self.texture_set.clone() else { return };
        let ts = ts.bind();
        self.texture_array = ts.build_array();
        let tex = self.texture_array.as_ref().map(|t| t.get_rid().to_variant()).unwrap_or_default();
        // Node-driven parameters go through the RenderingServer so they are never serialized
        // into the (user-editable, saved) material resources.
        let mut rs = godot::classes::RenderingServer::singleton();
        if let Some(m) = &self.terrain_material {
            let rid = m.get_rid();
            rs.material_set_param(rid, "textures", &tex);
            rs.material_set_param(rid, "tints", &ts.tints().to_variant());
            rs.material_set_param(rid, "scales", &ts.scales().to_variant());
            rs.material_set_param(rid, "blend_mode", &self.blend_mode.to_variant());
        }
        let world_size = self
            .terrain_material
            .as_ref()
            .map(|m| m.get_shader_parameter("texture_world_size"))
            .filter(|v| !v.is_nil())
            .unwrap_or(8.0f32.to_variant());
        let blade: Option<Gd<Texture2D>> = self.grass_texture.clone().or_else(super::grass_look::default_sprite);
        self.grass_sprites = super::grass_look::build_sprite_array(Some(&*ts), blade.as_ref());
        if self.wind_texture.is_none() {
            self.wind_texture = super::grass_look::wind_texture();
        }
        if let Some(m) = &self.grass_material {
            let rid = m.get_rid();
            rs.material_set_param(rid, "ground_textures", &tex);
            rs.material_set_param(rid, "ground_tints", &ts.tints().to_variant());
            rs.material_set_param(rid, "ground_scales", &ts.scales().to_variant());
            rs.material_set_param(rid, "grass_tints", &ts.grass_tints().to_variant());
            rs.material_set_param(rid, "texture_world_size", &world_size);
            if let Some(a) = &self.grass_sprites {
                rs.material_set_param(rid, "grass_sprites", &a.get_rid().to_variant());
            }
            // A user-assigned wind texture on the material wins.
            let user_wind = !m.get_shader_parameter("wind_texture").is_nil();
            if let (false, Some(w)) = (user_wind, &self.wind_texture) {
                rs.material_set_param(rid, "wind_texture", &w.get_rid().to_variant());
            }
            let has_wind = user_wind || self.wind_texture.is_some();
            rs.material_set_param(rid, "use_wind_texture", &has_wind.to_variant());
        }
        drop(ts);
        self.columns_sync_material();
        // Grass placement depends on which slots grow grass.
        self.mark_all_dirty();
        self.need_instances = true;
    }

    /// Layers, flags, shadows and collision settings on existing chunk nodes.
    fn apply_instance_settings(&mut self) {
        self.need_instances = false;
        let layers = self.render_layers | globals::meta_mask();
        let flags = object_flags(self.pixel_size, self.outline, self.outline_id, false);
        let gflags = object_flags(self.grass_pixel_size, self.grass_outline, self.outline_id, false);
        let shadows = if self.cast_shadows { ShadowCastingSetting::ON } else { ShadowCastingSetting::OFF };
        let mat = self.terrain_material.clone();
        let gmat = self.grass_material.clone();
        // Flags are a material uniform (PT_FLAGS_MATERIAL), set on the RenderingServer side so
        // the material resource itself is not modified.
        let mut rs = godot::classes::RenderingServer::singleton();
        if let Some(m) = &mat {
            rs.material_set_param(m.get_rid(), "pt_flags", &flags.to_variant());
        }
        if let Some(m) = &gmat {
            rs.material_set_param(m.get_rid(), "pt_flags", &gflags.to_variant());
        }
        let grass_on = self.grass_on();
        for n in self.nodes.values_mut() {
            n.mesh.set_layer_mask(layers);
            n.mesh.set_cast_shadows_setting(shadows);
            if let Some(m) = &mat {
                n.mesh.set_material_override(m);
            }
            n.grass.set_layer_mask(layers);
            n.grass.set_visible(grass_on);
            if let Some(m) = &gmat {
                n.grass.set_material_override(m);
            }
            n.body.set_collision_layer(self.collision_layer);
            n.body.set_collision_mask(self.collision_mask);
            n.shape.set_disabled(!self.collision_enabled);
            if let Some(c) = &n.columns {
                // The chunk's full columns and their "Tops" child (interior columns).
                let tops = c.get_node_or_null("Tops").and_then(|t| t.try_cast::<MultiMeshInstance3D>().ok());
                for mut c in [Some(c.clone()), tops].into_iter().flatten() {
                    c.set_layer_mask(layers);
                    c.set_cast_shadows_setting(shadows);
                    if let Some(m) = &mat {
                        c.set_material_override(m);
                    }
                }
            }
        }
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty = self.store.chunks.keys().copied().collect();
        self.need_height = true;
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
            self.need_height = true;
            // Edits refresh only their part of the instanced column map.
            self.columns.dirty_chunks.extend(touched.iter().copied());
            self.hexmaps.dirty = true;
        }
    }

    fn grass_params(&self) -> GrassParams {
        GrassParams {
            subdivisions: self.grass_subdivisions.clamp(1, 8) as u32,
            slot_has_grass: self.texture_set.as_ref().map(|t| t.bind().has_grass()).unwrap_or([false; 16]),
            ledge_threshold: self.grass_ledge_threshold,
            min_height: self.grass_min_height,
            seed: self.noise_seed as u32 ^ 0x5eed,
        }
    }

    /// Rebuilds every dirty chunk: meshing + grass scatter run in parallel threads, then the
    /// Godot resources are created on the main thread.
    pub fn flush(&mut self) {
        let coords: Vec<IVec2> = std::mem::take(&mut self.dirty).into_iter().collect();
        let (existing, removed): (Vec<IVec2>, Vec<IVec2>) = coords.into_iter().partition(|c| self.store.has_chunk(*c));
        for c in removed {
            if let Some(n) = self.nodes.remove(&c) {
                free_node(Some(n.root));
            }
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
            if !rebuilt.is_empty() {
                let coords: Vec<IVec2> = rebuilt.iter_shared().map(crate::util::to_iv).collect();
                self.scatter_mark_chunks(&coords);
                self.need_instances = true;
                self.apply_instance_settings();
                self.signals().chunks_rebuilt().emit(&rebuilt);
            }
            return;
        }
        let blend = BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let gp = self.grass_params();
        let grass_on = self.grass_on();
        if grass_on || !self.scatter.set.is_empty() || !self.cleared_cells.is_empty() {
            self.grass_water = self.collect_grass_water();
        }
        let water = &self.grass_water;
        let auto = self.auto_rules_core().filter(|a| a.enabled);
        let store = &self.store;
        let lod_low = &self.lod_low;
        let columns = self.columns_active();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 16);
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
                                let detail = if low { pt_core::Detail::Low } else { pt_core::Detail::Full };
                                let soup = if columns {
                                    // Instanced columns draw the chunk; this is its collision /
                                    // grass surface (bevelled column tops and sides, no grooves).
                                    store.chunks.get(c)?;
                                    pt_core::prism::chunk_soup(store, *c, low)
                                } else {
                                    pt_core::mesh_chunk_detail(store, *c, blend, detail)?
                                };
                                let mut grass = if grass_on && !low { pt_core::grass::scatter(&soup, gp) } else { Vec::new() };
                                if let Some(a) = auto {
                                    // No blades where the auto rules put rock / sand / snow.
                                    grass.retain(|g| a.keeps_grass(g.slot, g.pos, g.normal.y.abs(), &gp.slot_has_grass));
                                }
                                if !water.is_empty() {
                                    // No blades in raised pools or poking through their falls.
                                    grass.retain(|g| !water.covers(&store.config, g.pos));
                                }
                                Some(Built { coord: *c, arrays: soup.build(), grass, columns })
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
        for b in built {
            self.apply_built(b, &mut rebuilt);
        }
        self.columns_apply_chunks(&existing);
        // Props follow the new surface (baked and rebuilt chunks).
        let coords: Vec<IVec2> = rebuilt.iter_shared().map(crate::util::to_iv).collect();
        self.scatter_mark_chunks(&coords);
        self.need_instances = true;
        self.apply_instance_settings();
        if self.chunk_handles {
            self.base_mut().call_deferred("update_gizmos", &[]);
        }
        self.signals().chunks_rebuilt().emit(&rebuilt);
    }

    /// Raised water cells (terrain-local surface heights) of every PixelWater in this
    /// terrain's scene that belongs to it (`terrain` unset = the first terrain).
    fn collect_grass_water(&self) -> WaterCull {
        let mut cull = WaterCull { blocked: self.cleared_cells.clone(), ..Default::default() };
        if !self.base().is_inside_tree() {
            return cull;
        }
        let tn = self.to_gd().upcast::<Node>();
        let scene = match tn.get_owner() {
            Some(o) => o,
            None if crate::util::is_editor() => tn.clone(),
            None => tn.get_tree().get_root().upcast::<Node>(),
        };
        // Level-builder floors (`PixelBuilderMesh.get_grass_footprints`, world XZ polygons).
        let inv = self.base().get_global_transform().affine_inverse();
        for b in scene.find_children_ex("*").type_("PixelBuilderMesh").recursive(true).owned(false).done().iter_shared() {
            if let Ok(polys) = b.clone().call("get_grass_footprints", &[]).try_to::<Array<PackedVector3Array>>() {
                cull.footprints.extend(polys.iter_shared().map(|poly| poly.as_slice().iter().map(|w| { let l = inv * *w; v2(l.x, l.z) }).collect::<Vec<_>>()));
            }
        }
        if !self.grid().is_cell_mode() {
            return cull;
        }
        let mut waters: Vec<Gd<Node>> = scene.find_children_ex("*").type_("PixelWater").recursive(true).owned(false).done().iter_shared().collect();
        if scene.is_class("PixelWater") {
            waters.push(scene.clone());
        }
        let first = crate::effects::find_terrain(&scene);
        for w in waters {
            let own = w.get("terrain").try_to::<Gd<Node>>().ok().or_else(|| first.clone());
            if own.is_some_and(|o| o != tn) {
                continue;
            }
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    let e = cull.cells.entry(iv2(v.x.round() as i32, v.y.round() as i32)).or_insert(v.z);
                    *e = e.max(v.z);
                }
            }
        }
        // Raised water stored in the terrain data (cell water levels).
        for (c, w) in self.store.water_cells() {
            if self.store.height(c).is_some_and(|h| w > h + 1e-4) {
                let e = cull.cells.entry(c).or_insert(w);
                *e = e.max(w);
            }
        }
        cull
    }

    fn ensure_nodes(&mut self, c: IVec2) {
        if self.nodes.contains_key(&c) {
            return;
        }
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("Chunk ({}, {})", c.x, c.y));
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_name("Mesh");
        let mut body = StaticBody3D::new_alloc();
        body.set_name("Body");
        let mut shape = CollisionShape3D::new_alloc();
        shape.set_name("Shape");
        let mut grass = MultiMeshInstance3D::new_alloc();
        grass.set_name("Grass");
        grass.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        body.add_child(&shape);
        root.add_child(&mesh);
        root.add_child(&body);
        root.add_child(&grass);
        let mut this = self.to_gd();
        add_internal(&mut this, &root);
        self.nodes.insert(c, ChunkNodes { root, mesh, body, shape, grass, columns: None });
    }

    fn apply_built(&mut self, b: Built, rebuilt: &mut Array<Vector2i>) {
        self.ensure_nodes(b.coord);
        let gsize = self.grass_size;
        let n = self.nodes.get_mut(&b.coord).expect("chunk nodes");
        match build_array_mesh(&b.arrays).filter(|_| !b.columns) {
            Some(m) => n.mesh.set_mesh(&m),
            None => n.mesh.set_mesh(Gd::<godot::classes::Mesh>::null_arg()),
        }
        let faces: PackedVector3Array = b.arrays.collision_faces.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
        if faces.is_empty() {
            n.shape.set_shape(Gd::<godot::classes::Shape3D>::null_arg());
        } else {
            let mut shape = ConcavePolygonShape3D::new_gd();
            shape.set_backface_collision_enabled(true);
            shape.set_faces(&faces);
            n.shape.set_shape(&shape);
        }
        // Grass multimesh: blade quads with their bottom at the instance origin.
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(gsize.x.max(0.01), gsize.y.max(0.01)));
        quad.set_center_offset(Vector3::new(0.0, gsize.y.max(0.01) * 0.5, 0.0));
        mm.set_mesh(&quad);
        mm.set_instance_count(b.grass.len() as i32);
        for (i, g) in b.grass.iter().enumerate() {
            let t = Transform3D::new(Basis::IDENTITY, from_v3(g.pos));
            mm.set_instance_transform(i as i32, t);
            mm.set_instance_custom_data(i as i32, Color::from_rgba(g.slot as f32, g.rand, 0.0, 0.0));
        }
        n.grass.set_multimesh(&mm);
        rebuilt.push(from_iv(b.coord));
    }

    // ------------------------------------------------------------------ heights

    /// Terrain-local surface height under a terrain-local XZ point.
    pub fn local_height_at(&self, p: PVec2) -> Option<f32> {
        let cfg = &self.store.config;
        match cfg.grid {
            GridType::Square => {
                let cs = cfg.cell_size_safe();
                let fx = p.x / cs.x;
                let fz = p.y / cs.y;
                let (x0, z0) = (fx.floor() as i32, fz.floor() as i32);
                let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
                let h = |x: i32, z: i32| self.store.height(iv2(x, z));
                let (a, b, c, d) = (h(x0, z0)?, h(x0 + 1, z0)?, h(x0, z0 + 1)?, h(x0 + 1, z0 + 1)?);
                Some((a * (1.0 - tx) + b * tx) * (1.0 - tz) + (c * (1.0 - tx) + d * tx) * tz)
            }
            // Exact meshed surface: flat tops, ramps and smooth planes.
            _ => pt_core::cells::surface_height(&self.store, p),
        }
    }

    /// World-space bounds of the stored terrain (`None` without chunks).
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

    /// CPU ray cast against the height field (terrain-local). Returns the local hit point.
    pub fn local_raycast(&self, origin: Vector3, dir: Vector3, max_dist: f32) -> Option<Vector3> {
        let (min, max, hmin, hmax) = self.local_bounds()?;
        let d = dir.normalized();
        // Slab intersection with the bounds box.
        let (mut t0, mut t1) = (0.0f32, max_dist);
        for (o, dd, lo, hi) in [(origin.x, d.x, min.x, max.x), (origin.y, d.y, hmin - 1.0, hmax + 1.0), (origin.z, d.z, min.y, max.y)] {
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
        let step = (cs.x.min(cs.y) * 0.2).max(0.02);
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
        self.need_height = false;
        if !self.publish_height_texture {
            return;
        }
        let Some((min, max, _, _)) = self.local_bounds() else {
            globals::set("pt_terrain_rect", &Vector4::ZERO.to_variant());
            return;
        };
        // Margin around the terrain over which the published sea floor falls away (see
        // pt_core::heightmap::fill_sea_floor), so water shades from shallow to deep smoothly
        // instead of switching to "deep" at the edge of the chunks.
        const SEA_PAD: f32 = 16.0;
        const SEA_FALLOFF: f32 = 0.6;
        let (min, max) = (min - v2(SEA_PAD, SEA_PAD), max + v2(SEA_PAD, SEA_PAD));
        let cs = self.store.config.cell_size_safe();
        let texel = if self.grid() == GridType::Square { cs.x.min(cs.y) } else { cs.x * 0.5 };
        let w = (((max.x - min.x) / texel).ceil() as i32 + 1).clamp(2, 2048);
        let h = (((max.y - min.y) / texel).ceil() as i32 + 1).clamp(2, 2048);
        let sx = (max.x - min.x) / (w - 1) as f32;
        let sz = (max.y - min.y) / (h - 1) as f32;
        let origin = self.base().get_global_transform().origin;
        let scale = self.base().get_global_transform().basis.get_scale();
        let mut src = Vec::with_capacity((w * h) as usize);
        for z in 0..h {
            for x in 0..w {
                let p = v2(min.x + x as f32 * sx, min.y + z as f32 * sz);
                src.push(self.local_height_at(p).map(|v| v * scale.y + origin.y));
            }
        }
        let filled = pt_core::heightmap::fill_sea_floor(&src, w as usize, h as usize, sx.max(sz) * scale.x.abs().max(1e-3), SEA_FALLOFF, -1.0e4);
        let mut bytes = Vec::with_capacity((w * h * 4) as usize);
        for v in filled {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let Some(img) = Image::create_from_data(w, h, false, Format::RF, &PackedByteArray::from(bytes.as_slice())) else { return };
        match &mut self.height_texture {
            Some(t) if t.get_width() == w && t.get_height() == h => t.update(&img),
            _ => self.height_texture = ImageTexture::create_from_image(&img),
        }
        if let Some(t) = &self.height_texture {
            globals::set("pt_terrain_height", &t.to_variant());
        }
        let rect = Vector4::new(
            origin.x + min.x * scale.x,
            origin.z + min.y * scale.z,
            (max.x - min.x) * scale.x,
            (max.y - min.y) * scale.z,
        );
        globals::set("pt_terrain_rect", &rect.to_variant());
    }

    // ------------------------------------------------------------------ editing API (Rust)

    /// Runs an edit on the store and marks the touched chunks dirty (not yet written to data).
    pub fn edit(&mut self, f: impl FnOnce(&mut TerrainStore) -> Touched) -> Touched {
        let t = f(&mut self.store);
        self.mark_dirty(&t);
        t
    }

    pub fn chunk_bytes(&self, c: IVec2) -> Option<Vec<u8>> {
        self.store.chunks.get(&c).map(|d| d.to_bytes())
    }

    /// Restores (or removes, for `None`) a chunk, writing it to the data resource too.
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
        self.dirty.insert(c);
        self.save_pending.insert(c);
        for n in self.store.config.chunk_neighbors(c) {
            if self.store.has_chunk(n) {
                self.dirty.insert(n);
                self.save_pending.insert(n);
            }
        }
        self.need_height = true;
        self.base_mut().update_configuration_warnings();
    }

    /// Persists the given chunks into the data resource.
    pub fn write_chunks(&mut self, coords: &[IVec2]) {
        let grid = self.grid();
        let cfg = self.store.config.clone();
        if let Some(data) = &mut self.data {
            let mut d = data.bind_mut();
            d.write_layout(grid, Vector4i::new(cfg.chunk_size, cfg.ring_radius, cfg.wrap.x, cfg.wrap.y));
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
            self.dirty.insert(c);
            self.baked.remove(&c);
            self.save_pending.insert(c);
        }
    }

    pub fn texture_set_ref(&self) -> Option<Gd<MarchingSquaresTexturePreset>> {
        self.texture_set.clone()
    }

    fn tool_export_glb(&mut self) {
        let path = self.export_path.clone();
        let err = self.export_glb(path.clone());
        if err == godot::global::Error::OK {
            godot_print!("MarchingSquaresTerrain: exported {path}");
        } else {
            godot_error!("MarchingSquaresTerrain: export to {path} failed: {err:?}");
        }
    }

    // ------------------------------------------------------------------ distant LOD

    /// Re-evaluates the distant ring a few times per second (see `lod_distance`).
    fn update_lod(&mut self, delta: f64) {
        let active = self.lod_distance > 0.0 && (self.lod_in_editor || !is_editor());
        if !active {
            self.set_lod_chunks(HashSet::new());
            if self.lod_fade_published {
                self.lod_fade_published = false;
                crate::globals::set("pt_lod_fade", &Vector4::ZERO.to_variant());
            }
            return;
        }
        self.lod_timer -= delta;
        if self.lod_timer > 0.0 {
            return;
        }
        self.lod_timer = 0.2;
        if let Some(focus) = self.lod_focus_point() {
            self.apply_lod_focus(focus);
        }
    }

    /// Global position of the LOD focus: `lod_focus`, else where the active camera's view ray
    /// meets the terrain's base plane (or the camera position).
    fn lod_focus_point(&self) -> Option<Vector3> {
        if let Some(f) = self.lod_focus.as_ref().filter(|f| f.is_instance_valid() && f.is_inside_tree()) {
            return Some(f.get_global_position());
        }
        let cam = self.base().get_viewport()?.get_camera_3d()?;
        let xf = cam.get_global_transform();
        let (o, fwd) = (xf.origin, -xf.basis.col_c());
        let y0 = self.base().get_global_position().y;
        if fwd.y < -1e-3 {
            let t = (y0 - o.y) / fwd.y;
            if t > 0.0 && t < 1e5 {
                return Some(o + fwd * t);
            }
        }
        Some(o)
    }

    /// Recomputes the low-detail chunks around a global focus point; changed chunks are
    /// remeshed by the next `update`.
    fn apply_lod_focus(&mut self, focus: Vector3) {
        if self.lod_distance <= 0.0 {
            self.set_lod_chunks(HashSet::new());
            return;
        }
        let local = self.global_to_local(focus);
        let cfg = &self.store.config;
        // Half a chunk radius of hysteresis.
        let radius = cfg.chunk_outline(iv2(0, 0)).first().map(|p| (*p - cfg.chunk_center_pos(iv2(0, 0))).length()).unwrap_or(0.0);
        let low = cfg.lod_low_chunks(self.store.chunks.keys().copied(), v2(local.x, local.z), self.lod_distance, radius * 0.5, &self.lod_low);
        self.set_lod_chunks(low);
        // Props / grass fade: a chunk turns low once its centre is past lod_distance (back to
        // full half a radius closer), so anything farther than lod_distance - 1.5 radius may
        // sit in a low chunk. Fade out just before that (world units, assuming an unscaled
        // terrain); never closer than half the ring.
        let scale = self.base().get_global_transform().basis.col_a().length().max(1e-3);
        let end = (self.lod_distance - 1.5 * radius).max(self.lod_distance * 0.5) * scale;
        let band = (end * 0.15).clamp(1.0, 8.0);
        crate::globals::set("pt_lod_fade", &Vector4::new(focus.x, focus.z, end, band).to_variant());
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
    /// `lod_focus`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_lod_focus(&self) -> Option<Gd<Node3D>> {
        crate::util::live(&self.lod_focus)
    }

    /// `map_window_focus`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_map_window_focus(&self) -> Option<Gd<Node3D>> {
        crate::util::live(&self.map_window_focus)
    }

    /// Emitted after chunk meshes were rebuilt (e.g. to refresh navigation).
    #[signal]
    fn chunks_rebuilt(chunks: Array<Vector2i>);
    /// Emitted when the map window (`map_window_radius`) moved or its texture was rebuilt.
    #[signal]
    fn map_window_changed(window: Rect2i);

    // ---- setters (only flag work; it happens in `update`) ----

    #[func]
    fn set_chunk_handles(&mut self, v: bool) {
        self.chunk_handles = v;
        self.base_mut().call_deferred("update_gizmos", &[]);
    }
    #[func]
    fn set_handle_chunk(&mut self, v: Vector2i) {
        self.handle_chunk = v;
        self.base_mut().call_deferred("update_gizmos", &[]);
    }

    // Storage setters (logic in node_storage.rs).
    #[func]
    fn set_storage_mode(&mut self, v: i32) {
        self.storage_set_storage_mode(v);
    }
    #[func]
    fn set_bake_grass(&mut self, v: bool) {
        self.storage_set_bake_grass(v);
    }
    #[func]
    fn set_bake_collision(&mut self, v: bool) {
        self.storage_set_bake_collision(v);
    }
    #[func]
    fn set_data_directory(&mut self, v: GString) {
        self.storage_set_data_directory(v);
    }

    #[func]
    fn set_data(&mut self, v: Option<Gd<MarchingSquaresTerrainData>>) {
        self.data = v;
        self.need_reload = true;
    }
    #[func]
    fn set_grid_type(&mut self, v: i32) {
        let v = v.clamp(0, 3);
        let changed = v != self.grid_type;
        self.grid_type = v;
        self.need_reload = true;
        if changed {
            // Grid-specific properties appear / disappear (`on_validate_property`).
            self.base_mut().call_deferred("notify_property_list_changed", &[]);
        }
    }
    #[func]
    fn set_chunk_size(&mut self, v: i32) {
        if self.loaded && !self.store.chunks.is_empty() && v != self.chunk_size {
            godot_warn!("MarchingSquaresTerrain: chunk_size is fixed once chunks exist for this grid type (clear the data or switch grid type first).");
            return;
        }
        self.chunk_size = v.clamp(2, 64);
        self.need_reload = true;
    }
    #[func]
    fn set_cell_size(&mut self, v: Vector2) {
        self.cell_size = Vector2::new(v.x.max(0.05), v.y.max(0.05));
        self.store.config.cell_size = to_v2(self.cell_size);
        self.mark_all_dirty();
    }
    #[func]
    fn set_ring_radius(&mut self, v: i32) {
        if self.loaded && !self.store.chunks.is_empty() && self.grid() == GridType::HexRings && v != self.ring_radius {
            godot_warn!("MarchingSquaresTerrain: ring_radius is fixed once hex-ring chunks exist.");
            return;
        }
        self.ring_radius = v.clamp(1, 16);
        self.need_reload = true;
    }
    #[func]
    fn set_wrap_chunks(&mut self, v: Vector2i) {
        self.wrap_chunks = Vector2i::new(v.x.max(0), v.y.max(0));
        self.need_reload = true;
    }
    #[func]
    fn set_wrap_period(&mut self, v: i32) {
        // Not structural (chunk blobs are unchanged): apply at once and remesh.
        self.wrap_period = v.max(0);
        self.store.config.wrap_period = self.wrap_period;
        self.mark_all_dirty();
    }
    #[func]
    fn set_hex_edge_style(&mut self, v: i32) {
        // Mesh-only (chunk data is unchanged): apply at once and remesh.
        self.hex_edge_style = v.clamp(0, 3);
        self.store.edges = self.edge_params();
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
        // Switches the material variant and rebuilds every chunk.
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
        self.store.edges = self.edge_params();
        if self.grid() != GridType::Square && matches!(self.hex_edge_style, 1 | 3) {
            self.mark_all_dirty();
        }
    }
    #[func]
    fn set_blend_mode(&mut self, v: i32) {
        self.blend_mode = v.clamp(0, 2);
        self.need_look = true;
    }
    #[func]
    fn set_texture_set(&mut self, v: Option<Gd<MarchingSquaresTexturePreset>>) {
        self.texture_set = v;
        self.need_look = true;
    }
    #[func]
    fn set_terrain_material(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.terrain_material = v;
        self.need_look = true;
    }
    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = v.clamp(1, 4);
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
    /// The project's Grass toggle changed (`features::CALLBACK`): re-mesh the grass.
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        self.need_instances = true;
        self.mark_all_dirty();
        self.base_mut().call_deferred("update_configuration_warnings", &[]);
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
    fn set_grass_material(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.grass_material = v;
        self.need_look = true;
    }
    #[func]
    fn set_grass_pixel_size(&mut self, v: i32) {
        self.grass_pixel_size = v.clamp(1, 4);
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
    fn set_collision_enabled(&mut self, v: bool) {
        self.collision_enabled = v;
        self.need_instances = true;
    }
    #[func]
    fn set_collision_layer(&mut self, v: u32) {
        self.collision_layer = v;
        self.need_instances = true;
    }
    #[func]
    fn set_collision_mask(&mut self, v: u32) {
        self.collision_mask = v;
        self.need_instances = true;
    }
    #[func]
    fn set_publish_height_texture(&mut self, v: bool) {
        self.publish_height_texture = v;
        self.need_height = true;
    }

    /// Re-applies render layers / flags (called when the renderer's meta layer changes).
    #[func]
    fn refresh_pixel_settings(&mut self) {
        self.need_instances = true;
    }

    // ---- gameplay / scripting API ----

    /// Distant LOD: re-evaluates the low-detail ring around a global `focus` point now and
    /// remeshes the chunks that changed (normally done a few times per second from
    /// `lod_focus` or the active camera).
    #[func]
    pub fn update_lod_at(&mut self, focus: Vector3) {
        self.apply_lod_focus(focus);
        self.update();
    }

    /// Distant LOD: the chunks currently meshed at low detail.
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

    /// Re-reads the raised water cells of the scene's PixelWater nodes and rebuilds the
    /// chunks whose grass they change (PixelWater calls it, deferred, when its cells change).
    #[func]
    pub fn refresh_grass_water(&mut self) {
        if !self.grass_on() && self.scatter.set.is_empty() && self.cleared_cells.is_empty() && self.grass_water.is_empty() {
            return;
        }
        let new = self.collect_grass_water();
        let old = std::mem::replace(&mut self.grass_water, new);
        if old.cells != self.grass_water.cells || old.footprints != self.grass_water.footprints || old.blocked != self.grass_water.blocked {
            // Props hide under builder floors, raised water and cleared cells too.
            self.scatter.mark_all();
        }
        let mut touched = Touched::new();
        for c in old.blocked.symmetric_difference(&self.grass_water.blocked) {
            touched.extend(self.store.chunks_affected_by(*c));
        }
        let cur = &self.grass_water.cells;
        for c in old.cells.keys().chain(cur.keys()) {
            let (a, b) = (old.cells.get(c), cur.get(c));
            if a != b {
                touched.extend(self.store.chunks_affected_by(*c));
            }
        }
        if old.footprints != self.grass_water.footprints {
            // Builder floors moved / changed: the chunks under both the old and new polygons.
            let cfg = &self.store.config;
            let step = cfg.cell_size_safe().x.min(cfg.cell_size_safe().y) * 0.5;
            for poly in old.footprints.iter().chain(self.grass_water.footprints.iter()) {
                let lo = poly.iter().fold(v2(f32::MAX, f32::MAX), |a, p| v2(a.x.min(p.x), a.y.min(p.y)));
                let hi = poly.iter().fold(v2(f32::MIN, f32::MIN), |a, p| v2(a.x.max(p.x), a.y.max(p.y)));
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

    /// Clears (or restores) cells for the game: no grass and no scatter props grow on cleared
    /// cells (built on, paved). Runtime state, not saved with the terrain data; cells are
    /// sample coordinates (hex / triangle cells, square vertices).
    #[func]
    pub fn set_cells_cleared(&mut self, cells: Array<Variant>, cleared: bool) {
        let mut changed = false;
        for c in cells.iter_shared().filter_map(|v| v.try_to::<Vector2i>().ok()) {
            let c = to_iv(c);
            changed |= if cleared { self.cleared_cells.insert(c) } else { self.cleared_cells.remove(&c) };
        }
        if changed {
            self.refresh_grass_water();
            if !self.grass_on() {
                self.update();
            }
        }
    }

    /// Cells cleared with `set_cells_cleared`.
    #[func]
    pub fn get_cleared_cells(&self) -> Array<Vector2i> {
        let mut v: Vec<IVec2> = self.cleared_cells.iter().copied().collect();
        v.sort_by_key(|c| (c.y, c.x));
        v.into_iter().map(from_iv).collect()
    }

    /// Rebuilds every chunk now.
    #[func]
    pub fn rebuild(&mut self) {
        self.need_reload = true;
        self.update();
    }

    /// Adds a chunk (noise-initialized if `new_chunk_noise_amplitude > 0`). Returns false if it exists.
    #[func]
    pub fn add_chunk(&mut self, coords: Vector2i) -> bool {
        // Under a torus wrap a chunk is stored at its canonical coordinate.
        let c = self.store.config.wrap_chunk(to_iv(coords));
        if self.store.has_chunk(c) {
            return false;
        }
        let merge = MergeMode::from_u8(self.default_merge_mode.clamp(0, 4) as u8);
        if let Some(noise) = self.noise_hmap.clone() {
            let amp = self.noise_hmap_height;
            let square = self.store.config.grid == GridType::Square;
            self.store.add_chunk_with_heights(c, merge, |g, p| {
                // Like the original: vertex index for square chunks, cell centre otherwise.
                let (x, z) = if square { (g.x as f32, g.y as f32) } else { (p.x, p.y) };
                noise.get_noise_2d(x, z) * amp
            });
        } else if self.new_chunk_noise_amplitude.abs() > 1e-4 {
            self.store.add_chunk_with_noise(c, merge, self.new_chunk_noise_amplitude, self.new_chunk_noise_frequency, self.noise_seed as u32);
        } else {
            self.store.add_chunk(c, merge);
        }
        self.store.sync_square_borders(c);
        self.write_chunks(&[c]);
        self.dirty.insert(c);
        self.save_pending.insert(c);
        for n in self.store.config.chunk_neighbors(c) {
            if self.store.has_chunk(n) {
                self.dirty.insert(n);
                self.save_pending.insert(n);
            }
        }
        self.need_height = true;
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

    /// Raw chunk blob (empty if missing); used by undo/redo and save tools.
    #[func]
    pub fn get_chunk_data(&self, coords: Vector2i) -> PackedByteArray {
        self.chunk_bytes(to_iv(coords)).map(|b| PackedByteArray::from(b.as_slice())).unwrap_or_default()
    }

    /// Restores a chunk blob (empty removes the chunk).
    #[func]
    pub fn set_chunk_data(&mut self, coords: Vector2i, bytes: PackedByteArray) {
        let b = bytes.to_vec();
        self.restore_chunk(to_iv(coords), if b.is_empty() { None } else { Some(&b) });
    }

    /// Batch form of `set_chunk_data` (`{Vector2i: PackedByteArray}`), applied then rebuilt.
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

    /// Chunk containing a world position.
    #[func]
    pub fn world_to_chunk(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.chunk_at(v2(l.x, l.z)))
    }

    /// Grid sample (vertex for square, cell otherwise) nearest to a world position.
    #[func]
    pub fn world_to_sample(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.world_to_sample(v2(l.x, l.z)))
    }

    /// World position of a sample at its current height.
    #[func]
    pub fn sample_to_world(&self, sample: Vector2i) -> Vector3 {
        let s = to_iv(sample);
        let p = self.store.config.sample_pos(s);
        let h = self.store.height(s).unwrap_or(0.0);
        self.local_to_global(Vector3::new(p.x, h, p.y))
    }

    /// Surface height (world Y) under a world position; `fallback` where there is no terrain.
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

    /// Ray cast against the terrain surface without physics. Returns `{hit, position, sample}`.
    #[func]
    pub fn raycast(&self, from: Vector3, direction: Vector3, max_distance: f32) -> VarDictionary {
        let mut d = VarDictionary::new();
        let t = self.base().get_global_transform();
        let inv = t.affine_inverse();
        let o = inv * from;
        let dir = inv.basis * direction;
        match self.local_raycast(o, dir, max_distance) {
            Some(hit) => {
                d.set("hit", true);
                d.set("position", t * hit);
                // Nudge along the ray so a hit on a cliff face picks the cell owning the wall
                // (the higher one), not the one in front of it.
                let probe = hit + dir.normalized() * 0.01;
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
        self.store.height(to_iv(sample)).unwrap_or(0.0)
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

    /// Whether a cell-mode sample is smooth (a plane through neighbouring smooth cells
    /// instead of a flat terrace).
    #[func]
    pub fn get_sample_smooth(&self, sample: Vector2i) -> bool {
        self.store.sample(to_iv(sample)).map(|s| s.smooth).unwrap_or(false)
    }

    /// Sets / clears the smooth flag (the Bridge tool sets it; this is how to undo it).
    #[func]
    pub fn set_sample_smooth(&mut self, sample: Vector2i, smooth: bool) {
        let s = to_iv(sample);
        self.edit(|st| brush::op_paint(st, &[(s, 1.0)], PaintTarget::Smooth, smooth as u8));
    }

    /// Height of a sample in whole levels of `level_height` (rounded).
    #[func]
    pub fn get_cell_level(&self, cell: Vector2i) -> i32 {
        (self.get_sample_height(cell) / self.level_height_safe()).round() as i32
    }

    /// Sets a sample to `level * level_height`.
    #[func]
    pub fn set_cell_level(&mut self, cell: Vector2i, level: i32) {
        let h = level as f32 * self.level_height_safe();
        self.set_sample_height(cell, h);
    }

    /// Global coordinates of every cell (sample) a chunk owns, in local row-major order.
    /// Hex rings: axial `(q, r)` of the chunk's hex disk (= the game's LargeHex coords with
    /// `ring_radius = 2`, `cell_size = (6, 6)`). The order matches the `*_chunk_*` bulk calls.
    #[func]
    pub fn get_chunk_cells(&self, chunk: Vector2i) -> Array<Vector2i> {
        self.store.config.chunk_cells(to_iv(chunk)).into_iter().map(from_iv).collect()
    }

    /// Centre cell of a chunk (hex rings: the disk centre).
    #[func]
    pub fn get_chunk_center_cell(&self, chunk: Vector2i) -> Vector2i {
        from_iv(self.store.config.chunk_center_cell(to_iv(chunk)))
    }

    /// Chunk owning a cell.
    #[func]
    pub fn get_cell_chunk(&self, cell: Vector2i) -> Vector2i {
        from_iv(self.store.config.chunk_of(to_iv(cell)))
    }

    /// The (up to 6) edge neighbours of a cell, in edge order (hex: E, SE, SW, W, NW, NE in
    /// axial `(1,0),(0,1),(-1,1),(-1,0),(0,-1),(1,-1)`, like kg_hex `DIRS`).
    #[func]
    pub fn get_cell_neighbors(&self, cell: Vector2i) -> Array<Vector2i> {
        self.store.config.edge_neighbors(to_iv(cell)).into_iter().map(from_iv).collect()
    }

    /// A chunk's cells in kg_hex spiral order (centre, then ring k from `(0, -k)` walking E, SE,
    /// SW, W, NW, NE) — the order of kg_hex `LARGE_LOCAL` / a kg_world patch. Hex rings only;
    /// other grids return `get_chunk_cells`. Feed it to `set_cell_heights` / `set_cell_levels`.
    #[func]
    pub fn get_chunk_cells_spiral(&self, chunk: Vector2i) -> Array<Vector2i> {
        self.store.config.chunk_cells_spiral(to_iv(chunk)).into_iter().map(from_iv).collect()
    }

    /// World position of a chunk's centre cell at its height.
    #[func]
    pub fn chunk_to_world(&self, chunk: Vector2i) -> Vector3 {
        self.sample_to_world(self.get_chunk_center_cell(chunk))
    }

    /// Hex-friendly alias of `world_to_sample`: the cell under a world position (axial `(q, r)`
    /// on hex rings).
    #[func]
    pub fn world_to_cell(&self, world: Vector3) -> Vector2i {
        self.world_to_sample(world)
    }

    /// World position of a cell centre on its surface (alias of `sample_to_world`).
    #[func]
    pub fn cell_to_world(&self, cell: Vector2i) -> Vector3 {
        self.sample_to_world(cell)
    }

    /// Hex grids: world positions of the top-decoration spots of `cell` read as a kg_hex large
    /// hex (its 7 whole small hexes: centre + ring at a third of the cell width), skipping
    /// uneven ground (ramps, terraces, smooth cells), spots at or below `min_height`
    /// (terrain-local, e.g. the sea level) or under the cell's water level
    /// (`set_cell_water_level`), and paved / built-on small hexes (`set_small_hexes_blocked`).
    /// `get_cell_decoration_hexes` gives the small hexes of the same spots.
    #[func]
    pub fn get_cell_decoration_spots(&self, cell: Vector2i, min_height: f32) -> PackedVector3Array {
        let t = self.base().get_global_transform();
        self.decoration_spots_local(to_iv(cell), min_height).into_iter().map(|(_, p)| t * Vector3::new(p.x, p.y, p.z)).collect()
    }

    /// Stored (terrace) height of a cell; 0 for cells without a chunk.
    #[func]
    pub fn get_cell_height(&self, cell: Vector2i) -> f32 {
        self.get_sample_height(cell)
    }

    #[func]
    pub fn set_cell_height(&mut self, cell: Vector2i, height: f32) {
        self.set_sample_height(cell, height);
    }

    /// Canonical coordinate of a cell under the torus wrap (identity without wrap).
    #[func]
    pub fn wrap_cell(&self, cell: Vector2i) -> Vector2i {
        from_iv(self.store.config.wrap_sample(to_iv(cell)))
    }

    /// Canonical coordinate of a chunk under the torus wrap (identity without wrap).
    #[func]
    pub fn wrap_chunk(&self, chunk: Vector2i) -> Vector2i {
        from_iv(self.store.config.wrap_chunk(to_iv(chunk)))
    }

    /// Heights of arbitrary cells (0 where missing), e.g. a `get_chunk_cells_spiral` list.
    #[func]
    pub fn get_cell_heights(&self, cells: Array<Vector2i>) -> PackedFloat32Array {
        cells.iter_shared().map(|c| self.store.height(to_iv(c)).unwrap_or(0.0)).collect()
    }

    /// Bulk height write for arbitrary cells (pairs `cells[i]` with `heights[i]`); one remesh.
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

    /// Bulk integer-level write (`level * level_height`) for arbitrary cells, e.g. a kg_world
    /// patch's `elevation`s with `get_chunk_cells_spiral(patch)`.
    #[func]
    pub fn set_cell_levels(&mut self, cells: Array<Vector2i>, levels: PackedInt32Array) {
        let lh = self.level_height_safe();
        let hs: PackedFloat32Array = levels.as_slice().iter().map(|l| *l as f32 * lh).collect();
        self.set_cell_heights(cells, hs);
    }

    /// Small hex (kg_hex `SmallHex`, 3x finer axial lattice, same origin) under a world position.
    #[func]
    pub fn world_to_small_hex(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.world_to_small_hex(v2(l.x, l.z)))
    }

    /// World centre of a small hex on the terrain surface (`fallback` height off-terrain).
    #[func]
    pub fn small_hex_to_world(&self, small: Vector2i, fallback: f32) -> Vector3 {
        let p = self.store.config.small_hex_pos(to_iv(small));
        let h = self.local_height_at(p).unwrap_or(fallback);
        self.local_to_global(Vector3::new(p.x, h, p.y))
    }

    /// The cell (large hex) owning a small hex (kg_hex `SmallHex::large`).
    #[func]
    pub fn small_hex_to_cell(&self, small: Vector2i) -> Vector2i {
        from_iv(self.store.config.small_hex_owner(to_iv(small)))
    }

    /// Heights of a chunk's cells in `get_chunk_cells` order (empty if the chunk is missing).
    #[func]
    pub fn get_chunk_heights(&self, chunk: Vector2i) -> PackedFloat32Array {
        let c = to_iv(chunk);
        if !self.store.has_chunk(c) {
            return PackedFloat32Array::new();
        }
        self.store.config.chunk_cells(c).into_iter().map(|g| self.store.height(g).unwrap_or(0.0)).collect()
    }

    /// Bulk height write in `get_chunk_cells` order; one remesh of the affected chunks.
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

    /// Bulk integer-level write (`level * level_height`) in `get_chunk_cells` order, e.g. a
    /// game patch's elevations.
    #[func]
    pub fn set_chunk_levels(&mut self, chunk: Vector2i, levels: PackedInt32Array) {
        let lh = self.level_height_safe();
        let hs: PackedFloat32Array = levels.as_slice().iter().map(|l| *l as f32 * lh).collect();
        self.set_chunk_heights(chunk, hs);
    }

    /// Floor slots of a chunk's cells in `get_chunk_cells` order (`wall`: wall slots).
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
            .map(|g| self.store.sample(g).map(|s| if wall { s.wall } else { s.floor }).unwrap_or(0))
            .collect()
    }

    /// Bulk slot write in `get_chunk_cells` order (values clamped to 0..15).
    #[func]
    pub fn set_chunk_slots(&mut self, chunk: Vector2i, slots: PackedByteArray, wall: bool) {
        let cells = self.store.config.chunk_cells(to_iv(chunk));
        let target = if wall { PaintTarget::Wall } else { PaintTarget::Floor };
        let v = slots.as_slice().to_vec();
        self.edit(|st| {
            let mut t = Touched::new();
            for (g, s) in cells.iter().zip(v) {
                t.extend(brush::op_paint(st, &[(*g, 1.0)], target, s.min(15)));
            }
            t
        });
    }

    /// Floor (or wall, when `wall`) texture slot of a sample.
    #[func]
    pub fn get_sample_slot(&self, sample: Vector2i, wall: bool) -> i32 {
        self.store.sample(to_iv(sample)).map(|s| if wall { s.wall } else { s.floor } as i32).unwrap_or(-1)
    }

    #[func]
    pub fn set_sample_slot(&mut self, sample: Vector2i, slot: i32, wall: bool) {
        let s = to_iv(sample);
        let target = if wall { PaintTarget::Wall } else { PaintTarget::Floor };
        self.edit(|st| brush::op_paint(st, &[(s, 1.0)], target, slot.clamp(0, 15) as u8));
    }

    /// Gameplay terraforming: `op` is one of raise, lower, level, smooth, paint, paint_wall,
    /// grass_on, grass_off, smooth_on, smooth_off (cell modes). `amount` is height delta / target height / strength / slot.
    #[func]
    pub fn apply_brush(&mut self, op: GString, world: Vector3, radius: f32, amount: f32, shape: i64) {
        let l = self.global_to_local(world);
        let b = Brush { shape: BrushShape::from_i64(shape), radius, ..Default::default() };
        let fp = brush::footprint(&self.store.config, &b, v2(l.x, l.z));
        let op = op.to_string();
        self.edit(|st| match op.as_str() {
            "raise" => brush::op_raise(st, &fp, amount.abs(), Clamp::default()),
            "lower" => brush::op_raise(st, &fp, -amount.abs(), Clamp::default()),
            "level" => brush::op_level(st, &fp, amount, 1.0),
            "smooth" => brush::op_smooth(st, &fp, amount.clamp(0.0, 1.0)),
            "paint" => brush::op_paint(st, &fp, PaintTarget::Floor, amount.clamp(0.0, 15.0) as u8),
            "paint_wall" => brush::op_paint(st, &fp, PaintTarget::Wall, amount.clamp(0.0, 15.0) as u8),
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

    /// Writes runtime edits back into the data resource (for in-game editors / saving).
    #[func]
    pub fn commit_to_data(&mut self) {
        let all: Vec<IVec2> = self.store.chunk_coords();
        self.write_chunks(&all);
    }

    /// Exports every chunk as a GLB with the slot tints baked into vertex colors
    /// (for use in other tools / as a static mesh). Returns a Godot error code.
    #[func]
    pub fn export_glb(&mut self, path: GString) -> godot::global::Error {
        use godot::classes::mesh::ArrayType;
        use godot::classes::{GltfDocument, GltfState, StandardMaterial3D};
        let tints: Vec<Color> = self.texture_set.as_ref().map(|t| t.bind().tints().as_slice().to_vec()).unwrap_or_else(|| vec![Color::WHITE; 16]);
        let blend = BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let mut root = Node3D::new_alloc();
        root.set_name("MarchingSquaresTerrainExport");
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_flag(godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        for c in self.store.chunk_coords() {
            let Some(soup) = pt_core::mesh_chunk(&self.store, c, blend) else { continue };
            let arrays = soup.build();
            let Some(mut mesh) = build_array_mesh(&arrays) else { continue };
            // Re-upload with a COLOR stream derived from the slot weights.
            let colors: PackedColorArray = arrays
                .custom0
                .iter()
                .zip(arrays.custom1.iter())
                .map(|(m, w)| {
                    let wc = (1.0 - w[0] - w[1]).max(0.0);
                    let pick = |i: f32| tints.get(i as usize).copied().unwrap_or(Color::WHITE);
                    let mut col = pick(m[0]) * w[0] + pick(m[1]) * w[1] + pick(m[2]) * wc;
                    if w[2] > 0.5 {
                        col *= 0.85;
                    }
                    col.a = 1.0;
                    col
                })
                .collect();
            let mut surf = mesh.surface_get_arrays(0);
            surf.set(ArrayType::COLOR.ord() as usize, &colors.to_variant());
            // The vertex-colored export needs no shader streams (and float CUSTOM arrays would
            // need their format flags to be re-added).
            surf.set(ArrayType::CUSTOM0.ord() as usize, &Variant::nil());
            surf.set(ArrayType::CUSTOM1.ord() as usize, &Variant::nil());
            mesh.clear_surfaces();
            mesh.add_surface_from_arrays(godot::classes::mesh::PrimitiveType::TRIANGLES, &surf);
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

    /// Number of grass blades currently planted (diagnostics).
    #[func]
    pub fn get_grass_count(&self) -> i64 {
        self.nodes
            .values()
            .map(|n| n.grass.get_multimesh().map(|m| m.get_instance_count() as i64).unwrap_or(0))
            .sum()
    }
}
