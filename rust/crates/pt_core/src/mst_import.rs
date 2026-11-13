//! Import of the original (GDScript) MarchingSquaresTerrain's per-chunk `MSTChunkData`
//! resources (`<data_directory>/<mode>/chunk_X_Y/metadata.res`) into [`ChunkData`].
//!
//! Layout equivalence (verified against the original's grid scripts):
//!
//! * Square: `height_map` is `dimensions.z` rows × `dimensions.x` vertices (`Array[Array]`),
//!   neighbouring chunks duplicate their shared border — exactly our `(n+1)²` square chunks,
//!   so `chunk_size = dimensions.x - 1`.
//! * Triangle / Hex / HexRings: flat `height_map` in `cell_index = row * cells.x + col`
//!   order over the chunk's bounding box (`cell_count`), global cell = `chunk * cells + local`
//!   (hex: odd-r offset, pointy top; hex rings: axial disks around `chunk_center_hex`) — the
//!   same addressing as [`crate::grid`]. `chunk_size = cells.y` (triangle `cells.x = 2n`),
//!   hex rings `ring_radius = (cells.x - 1) / 2`.
//! * Texture indices are the same 0..15 slots (15 = void). V2 files store them as bytes; V1
//!   files store the 4×4 vertex-colour channel pairs, decoded by [`colors_to_texture_idx`].
//! * `grass_mask` is 0/1 (ours 0/255); `smooth_map` is one byte per cell (cell modes).
//! * Texture look (scene, [`MstSlot`]): `texture_N` / `texture_scale_N` (15 slots),
//!   `texture_albedo_N` (ground *and* grass colour), `grass_sprite_tex_N` and `texN_has_grass`
//!   (slots 1-6). Absent properties keep the original's defaults (noise ground textures, the leaf
//!   sprite, its six albedos); the original's bundled assets map to this addon's copies
//!   ([`bundled_asset`]).

use crate::data::{ChunkData, MergeMode};
use crate::godot_res::{ResourceFile, Value};
use crate::grid::GridType;
use crate::math::*;

/// One chunk read from an `MSTChunkData` resource.
#[derive(Clone, Debug, PartialEq)]
pub struct MstChunk {
    pub coords: IVec2,
    /// Grid type recorded in the file (`None` for old square-only files).
    pub grid: Option<GridType>,
    /// Bounding-box sample count (`x` columns, `y` rows).
    pub size: IVec2,
    pub data: ChunkData,
    /// The file was in the legacy V1 colour-map format.
    pub legacy: bool,
}

impl MstChunk {
    /// Structural layout implied by the chunk: `(chunk_size, ring_radius)` for `grid`.
    pub fn layout(&self, grid: GridType) -> (i32, i32) {
        match grid {
            GridType::Square => (self.size.x - 1, 0),
            GridType::Triangle | GridType::Hex => (self.size.y, 0),
            GridType::HexRings => (0, (self.size.x - 1) / 2),
        }
    }
}

/// Dominant-channel decode of the original's 4×4 vertex-colour texture encoding.
pub fn colors_to_texture_idx(c0: [f32; 4], c1: [f32; 4]) -> u8 {
    let dom = |c: [f32; 4]| {
        let mut idx = 0;
        let mut m = c[0];
        for (i, v) in c.iter().enumerate().skip(1) {
            if *v > m {
                m = *v;
                idx = i;
            }
        }
        idx as u8
    };
    dom(c0) * 4 + dom(c1)
}

/// Inverse of [`colors_to_texture_idx`] (one-hot channels).
pub fn texture_idx_to_colors(idx: u8) -> ([f32; 4], [f32; 4]) {
    let mut c0 = [0.0; 4];
    let mut c1 = [0.0; 4];
    c0[(idx as usize / 4) % 4] = 1.0;
    c1[idx as usize % 4] = 1.0;
    (c0, c1)
}

/// Parses `chunk_X_Y` directory names.
pub fn parse_chunk_dir(name: &str) -> Option<IVec2> {
    let rest = name.strip_prefix("chunk_")?;
    let (a, b) = rest.split_once('_')?;
    Some(iv2(a.parse().ok()?, b.parse().ok()?))
}

pub fn chunk_dir_name(c: IVec2) -> String {
    format!("chunk_{}_{}", c.x, c.y)
}

/// Converts a parsed `MSTChunkData` resource. `grid_hint` is used when the file does not
/// record a grid type (square-era files); `fallback_coords` when it has no coordinates.
pub fn chunk_from_resource(f: &ResourceFile, grid_hint: GridType, fallback_coords: IVec2) -> Result<MstChunk, String> {
    let m = f.main().ok_or("empty resource")?;
    let coords = m.get("chunk_coords").and_then(Value::as_ivec2).map(|v| iv2(v[0], v[1])).unwrap_or(fallback_coords);
    // Only present (non-default) properties are stored: grid_type 0 is omitted.
    let grid_in_file = m.get("grid_type").and_then(Value::as_i64);
    let cell_count = m.get("cell_count").and_then(Value::as_ivec2).map(|v| iv2(v[0], v[1])).unwrap_or(iv2(0, 0));
    let grid = match grid_in_file {
        Some(g) => GridType::from_i64(g),
        None if cell_count.x > 0 => grid_hint,
        None => GridType::Square,
    };
    // Properties equal to the class default (0) are not written: a missing merge_mode is Cubic.
    let merge = MergeMode::from_u8(m.get("merge_mode").and_then(Value::as_i64).unwrap_or(0).clamp(0, 4) as u8);
    let hm = m.get("height_map").ok_or("no height_map")?;
    let heights: Vec<f32> = hm.numbers().ok_or("height_map is not numeric")?.into_iter().map(|x| x as f32).collect();
    let size = if grid == GridType::Square {
        let rows = match hm {
            Value::Array(a) => a.len() as i32,
            _ => 0,
        };
        if rows <= 0 || heights.len() as i32 % rows != 0 {
            return Err("square height_map is not rectangular".into());
        }
        iv2(heights.len() as i32 / rows, rows)
    } else {
        if cell_count.x <= 0 || cell_count.y <= 0 {
            return Err("cell chunk without cell_count".into());
        }
        cell_count
    };
    let n = (size.x * size.y) as usize;
    if heights.len() != n {
        return Err(format!("height_map has {} values, expected {n}", heights.len()));
    }
    let bytes = |k: &str| m.get(k).and_then(Value::as_bytes).map(|b| b.to_vec()).unwrap_or_default();
    let colors = |k: &str| match m.get(k) {
        Some(Value::Colors(c)) => c.clone(),
        _ => Vec::new(),
    };
    let mut floor = bytes("ground_texture_idx");
    let mut wall = bytes("wall_texture_idx");
    let mut grass = bytes("grass_mask");
    let legacy = floor.is_empty();
    if legacy {
        let (a, b) = (colors("color_map_0"), colors("color_map_1"));
        let (wa, wb) = (colors("wall_color_map_0"), colors("wall_color_map_1"));
        let gm = colors("grass_mask_map");
        floor = (0..n).map(|i| a.get(i).zip(b.get(i)).map(|(x, y)| colors_to_texture_idx(*x, *y)).unwrap_or(0)).collect();
        wall = (0..n).map(|i| wa.get(i).zip(wb.get(i)).map(|(x, y)| colors_to_texture_idx(*x, *y)).unwrap_or(5)).collect();
        grass = (0..n).map(|i| gm.get(i).map(|c| (c[0] > 0.5) as u8).unwrap_or(1)).collect();
    }
    let fix = |v: Vec<u8>, default: u8| -> Vec<u8> {
        if v.len() == n { v.into_iter().map(|x| x.min(15)).collect() } else { vec![default; n] }
    };
    let floor = fix(floor, 0);
    let wall = fix(wall, 5);
    let grass: Vec<u8> = if grass.len() == n { grass.into_iter().map(|g| if g > 0 { 255 } else { 0 }).collect() } else { vec![255; n] };
    let smooth = bytes("smooth_map");
    let smooth: Vec<u8> = if smooth.len() == n { smooth.into_iter().map(|s| (s != 0) as u8).collect() } else { vec![0; n] };
    let data = ChunkData { size, heights, floor_tex: floor, wall_tex: wall, grass, smooth, water: vec![crate::data::NO_WATER; n], merge_mode: merge };
    Ok(MstChunk { coords, grid: grid_in_file.map(GridType::from_i64), size, data, legacy })
}

/// The original's bundled ground noise (`grass_terrain_noise.res`, slots 1-5) and wall noise
/// (`wall_noise_texture.res`, slot 6), its leaf sprite and empty "void" texture.
pub const ORIG_MATERIALS: &str = "res://addons/MarchingSquaresTerrain/resources/plugin_materials/";
pub const ORIG_GROUND_NOISE: &str = "res://addons/MarchingSquaresTerrain/resources/plugin_materials/grass_terrain_noise.res";
pub const ORIG_WALL_NOISE: &str = "res://addons/MarchingSquaresTerrain/resources/plugin_materials/wall_noise_texture.res";
pub const ORIG_LEAF_SPRITE: &str = "res://addons/MarchingSquaresTerrain/resources/plugin_materials/grass_leaf_sprite.png";
/// `texture_albedo_1..6` defaults of the original terrain node.
pub const ORIG_ALBEDOS: [u32; 6] = [0x647851, 0x527b62, 0x5f6c4b, 0x647941, 0x4a7e5d, 0x71725d];
/// Number of texture slots with grass settings in the original (1-6).
pub const ORIG_GRASS_SLOTS: usize = 6;

/// This addon's copy of one of the original's bundled assets (`res://` path in this project).
pub fn bundled_asset(original: &str) -> Option<&'static str> {
    let name = original.strip_prefix(ORIG_MATERIALS)?;
    Some(match name {
        "grass_terrain_noise.res" | "wall_noise_texture.res" => "res://addons/MarchingSquaresTerrain/textures/ground/mst_noise.png",
        "grass_leaf_sprite.png" => "res://addons/MarchingSquaresTerrain/textures/grass_blade.png",
        "void_texture.tres" => "res://addons/MarchingSquaresTerrain/textures/ground/void.png",
        _ => return None,
    })
}

/// A texture reference in an original scene.
#[derive(Clone, Debug, PartialEq)]
pub enum TexRef {
    /// Explicitly empty (`null`, or an embedded placeholder sub-resource).
    Empty,
    /// `res://` path of an external texture (in the original project).
    Path(String),
}

impl TexRef {
    pub fn path(&self) -> Option<&str> {
        match self {
            TexRef::Path(p) => Some(p),
            TexRef::Empty => None,
        }
    }
}

/// Texture look of one of the original's slots.
#[derive(Clone, Debug, PartialEq)]
pub struct MstSlot {
    pub texture: TexRef,
    pub scale: f32,
    /// Ground and grass colour (`texture_albedo_N`; slots 1-6 only).
    pub albedo: Option<[f32; 4]>,
    pub has_grass: bool,
    /// `grass_sprite_tex_N` (slots 1-6 only).
    pub grass_sprite: Option<TexRef>,
}

/// The original terrain node's default slot look (15 slots; void has none).
pub fn default_slots() -> Vec<MstSlot> {
    (0..15)
        .map(|i| {
            let grass = i < ORIG_GRASS_SLOTS;
            MstSlot {
                texture: match i {
                    0..=4 => TexRef::Path(ORIG_GROUND_NOISE.into()),
                    5 => TexRef::Path(ORIG_WALL_NOISE.into()),
                    _ => TexRef::Empty,
                },
                scale: 1.0,
                albedo: grass.then(|| {
                    let c = ORIG_ALBEDOS[i];
                    [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0, 1.0]
                }),
                has_grass: grass,
                grass_sprite: grass.then(|| TexRef::Path(ORIG_LEAF_SPRITE.into())),
            }
        })
        .collect()
}

/// Terrain-node settings read from an original scene (`.tscn`), so an import reproduces the
/// grid layout and texture look. Missing keys keep the original's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct MstSceneSettings {
    pub node_name: String,
    /// `res://` path as written in the scene (resolve against the original project).
    pub data_directory: String,
    pub grid_type: GridType,
    /// `(x, height range, z)`; square chunks are `x - 1` cells across.
    pub dimensions: [i32; 3],
    pub cell_size: Vec2,
    pub chunk_hex_radius: i32,
    pub wrap_chunks: IVec2,
    pub blend_mode: i32,
    pub default_wall_texture: i32,
    pub grass_subdivisions: i32,
    pub grass_size: Vec2,
    /// Texture look of the 15 non-void slots ([`default_slots`] for missing keys).
    pub slots: Vec<MstSlot>,
}

impl Default for MstSceneSettings {
    fn default() -> Self {
        MstSceneSettings {
            node_name: String::new(),
            data_directory: String::new(),
            grid_type: GridType::Square,
            dimensions: [33, 32, 33],
            cell_size: v2(2.0, 2.0),
            chunk_hex_radius: 2,
            wrap_chunks: iv2(0, 0),
            blend_mode: 0,
            default_wall_texture: 5,
            grass_subdivisions: 3,
            grass_size: v2(1.0, 1.0),
            slots: default_slots(),
        }
    }
}

impl MstSceneSettings {
    /// Our `chunk_size` for the scene's grid.
    pub fn chunk_size(&self) -> i32 {
        match self.grid_type {
            GridType::Square => self.dimensions[0] - 1,
            _ => self.dimensions[2].max(1),
        }
    }
}

fn parse_numbers(v: &str) -> Vec<f32> {
    let inner = v.split_once('(').map(|(_, r)| r.trim_end_matches(')')).unwrap_or(v);
    inner.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect()
}

/// Finds every MarchingSquaresTerrain node in a text scene (nodes whose script path ends
/// in `marching_squares_terrain.gd`) and reads their settings.
pub fn parse_scene_settings(tscn: &str) -> Vec<MstSceneSettings> {
    // ext_resource id -> path (scripts, and every resource for texture references)
    let mut scripts = std::collections::HashMap::new();
    let mut resources = std::collections::HashMap::new();
    for line in tscn.lines() {
        if line.starts_with("[ext_resource") {
            let get = |key: &str| line.split(&format!("{key}=\"")).nth(1).and_then(|r| r.split('"').next()).map(str::to_string);
            if let (Some(p), Some(id)) = (get("path"), get(" id").or_else(|| get("id"))) {
                if line.contains("type=\"Script\"") {
                    scripts.insert(id.clone(), p.clone());
                }
                resources.insert(id, p);
            }
        }
    }
    let tex_ref = |v: &str| -> TexRef {
        match v.strip_prefix("ExtResource(").and_then(|r| r.split('"').nth(1)).and_then(|id| resources.get(id)) {
            Some(p) => TexRef::Path(p.clone()),
            None => TexRef::Empty,
        }
    };
    // `<prefix><N><suffix>` with N in 1..=max -> slot N - 1.
    let slot_key = |k: &str, prefix: &str, suffix: &str, max: usize| -> Option<usize> {
        let n: usize = k.strip_prefix(prefix)?.strip_suffix(suffix)?.parse().ok()?;
        (1..=max).contains(&n).then(|| n - 1)
    };
    let mut out = Vec::new();
    let mut cur: Option<(MstSceneSettings, bool)> = None;
    let flush = |cur: &mut Option<(MstSceneSettings, bool)>, out: &mut Vec<MstSceneSettings>| {
        if let Some((s, true)) = cur.take() {
            out.push(s);
        }
    };
    for line in tscn.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut cur, &mut out);
            if line.starts_with("[node") {
                let name = line.split("name=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                cur = Some((MstSceneSettings { node_name: name, ..Default::default() }, false));
            }
            continue;
        }
        let Some((s, is_terrain)) = cur.as_mut() else { continue };
        let Some((k, v)) = line.split_once(" = ") else { continue };
        let v = v.trim();
        match k.trim() {
            "script" => {
                let id = v.split('"').nth(1).unwrap_or("");
                *is_terrain = scripts.get(id).is_some_and(|p| p.ends_with("marching_squares_terrain.gd"));
            }
            "data_directory" => s.data_directory = v.trim_matches('"').to_string(),
            "grid_type" => s.grid_type = GridType::from_i64(v.parse().unwrap_or(0)),
            "dimensions" => {
                let n = parse_numbers(v);
                if n.len() == 3 {
                    s.dimensions = [n[0] as i32, n[1] as i32, n[2] as i32];
                }
            }
            "cell_size" | "grass_size" => {
                let n = parse_numbers(v);
                if n.len() == 2 {
                    if k.trim() == "cell_size" { s.cell_size = v2(n[0], n[1]) } else { s.grass_size = v2(n[0], n[1]) }
                }
            }
            "chunk_hex_radius" => s.chunk_hex_radius = v.parse().unwrap_or(2),
            "wrap_chunk_cols" => s.wrap_chunks.x = v.parse().unwrap_or(0),
            "wrap_chunk_rows" => s.wrap_chunks.y = v.parse().unwrap_or(0),
            "blend_mode" => s.blend_mode = v.parse().unwrap_or(0),
            "default_wall_texture" => s.default_wall_texture = v.parse().unwrap_or(5),
            "grass_subdivisions" => s.grass_subdivisions = v.parse().unwrap_or(3),
            key => {
                if let Some(i) = slot_key(key, "texture_scale_", "", 15) {
                    s.slots[i].scale = v.parse().unwrap_or(1.0);
                } else if let Some(i) = slot_key(key, "texture_albedo_", "", ORIG_GRASS_SLOTS) {
                    let n = parse_numbers(v);
                    if n.len() >= 3 {
                        s.slots[i].albedo = Some([n[0], n[1], n[2], n.get(3).copied().unwrap_or(1.0)]);
                    }
                } else if let Some(i) = slot_key(key, "grass_sprite_tex_", "", ORIG_GRASS_SLOTS) {
                    s.slots[i].grass_sprite = Some(tex_ref(v));
                } else if let Some(i) = slot_key(key, "tex", "_has_grass", ORIG_GRASS_SLOTS) {
                    s.slots[i].has_grass = v == "true";
                } else if let Some(i) = slot_key(key, "texture_", "", 15) {
                    s.slots[i].texture = tex_ref(v);
                }
            }
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godot_res::{ObjRef, parse, write_simple};

    #[test]
    fn reads_terrain_settings_from_scene() {
        let tscn = r#"[gd_scene format=3]
[ext_resource type="Script" uid="uid://x" path="res://addons/MarchingSquaresTerrain/algorithm/terrain/marching_squares_terrain.gd" id="5_q"]
[ext_resource type="Script" path="res://other.gd" id="6_o"]

[node name="Other" type="Node3D"]
script = ExtResource("6_o")
grid_type = 3

[node name="MarchingSquaresTerrain" type="Node3D" parent="."]
script = ExtResource("5_q")
data_directory = "res://demo/demo_TerrainData/MarchingSquaresTerrain_126b8b50"
_storage_initialized = true
grid_type = 2
cell_size = Vector2(3, 3)
dimensions = Vector3i(17, 32, 17)

[node name="Chunk (0, 0)" type="MeshInstance3D" parent="MarchingSquaresTerrain"]
"#;
        let s = parse_scene_settings(tscn);
        assert_eq!(s.len(), 1);
        let s = &s[0];
        assert_eq!(s.node_name, "MarchingSquaresTerrain");
        assert_eq!(s.grid_type, GridType::Hex);
        assert_eq!(s.data_directory, "res://demo/demo_TerrainData/MarchingSquaresTerrain_126b8b50");
        assert_eq!(s.cell_size, v2(3.0, 3.0));
        assert_eq!(s.chunk_size(), 17);
        assert_eq!(MstSceneSettings { grid_type: GridType::Square, ..s.clone() }.chunk_size(), 16);
    }

    #[test]
    fn reads_slot_textures_and_grass_from_scene() {
        let tscn = r#"[gd_scene format=3]
[ext_resource type="Script" path="res://addons/MarchingSquaresTerrain/algorithm/terrain/marching_squares_terrain.gd" id="5_q"]
[ext_resource type="Texture2D" uid="uid://a" path="res://art/rock.png" id="7_r"]
[ext_resource type="Texture2D" path="res://addons/MarchingSquaresTerrain/resources/plugin_materials/grass_leaf_sprite.png" id="8_l"]

[sub_resource type="Texture2D" id="Texture2D_x"]

[node name="T" type="Node3D"]
script = ExtResource("5_q")
data_directory = "res://d"
texture_2 = ExtResource("7_r")
texture_3 = null
texture_9 = SubResource("Texture2D_x")
texture_scale_2 = 2.5
texture_albedo_1 = Color(0.5, 0.25, 0.125, 1)
grass_sprite_tex_4 = ExtResource("8_l")
tex3_has_grass = false
"#;
        let s = &parse_scene_settings(tscn)[0];
        assert_eq!(s.slots.len(), 15);
        // Untouched slots keep the original's defaults.
        assert_eq!(s.slots[0].texture, TexRef::Path(ORIG_GROUND_NOISE.into()));
        assert_eq!(s.slots[5].texture, TexRef::Path(ORIG_WALL_NOISE.into()));
        assert_eq!(s.slots[1].texture, TexRef::Path("res://art/rock.png".into()));
        assert_eq!(s.slots[2].texture, TexRef::Empty);
        assert_eq!(s.slots[8].texture, TexRef::Empty);
        assert_eq!(s.slots[1].scale, 2.5);
        assert_eq!(s.slots[0].albedo, Some([0.5, 0.25, 0.125, 1.0]));
        assert!((s.slots[1].albedo.unwrap()[0] - 0x52 as f32 / 255.0).abs() < 1e-6);
        assert!(!s.slots[2].has_grass && s.slots[3].has_grass && !s.slots[7].has_grass);
        let sprite = s.slots[3].grass_sprite.as_ref().and_then(|t| t.path()).and_then(bundled_asset);
        assert_eq!(sprite, Some("res://addons/MarchingSquaresTerrain/textures/grass_blade.png"));
        assert_eq!(s.slots[7].grass_sprite, None);
        assert_eq!(bundled_asset(ORIG_WALL_NOISE), bundled_asset(ORIG_GROUND_NOISE));
        assert_eq!(bundled_asset("res://art/rock.png"), None);
    }

    #[test]
    fn texture_idx_colour_roundtrip() {
        for i in 0..16u8 {
            let (a, b) = texture_idx_to_colors(i);
            assert_eq!(colors_to_texture_idx(a, b), i);
        }
        // Blended colours decode to the dominant channel.
        assert_eq!(colors_to_texture_idx([0.1, 0.7, 0.2, 0.0], [0.0, 0.0, 0.3, 0.9]), 4 + 3);
    }

    #[test]
    fn chunk_dir_names() {
        assert_eq!(parse_chunk_dir("chunk_-1_0"), Some(iv2(-1, 0)));
        assert_eq!(parse_chunk_dir("chunk_3_-12"), Some(iv2(3, -12)));
        assert_eq!(parse_chunk_dir("chunk_x"), None);
        assert_eq!(parse_chunk_dir(&chunk_dir_name(iv2(-4, 7))), Some(iv2(-4, 7)));
    }

    #[test]
    fn imports_v2_hex_chunk() {
        let n = 3 * 2;
        let b = write_simple(
            "Resource",
            "MSTChunkData",
            &[
                ("chunk_coords", Value::Ints(vec![0, -1])),
                ("grid_type", Value::Int(2)),
                ("cell_count", Value::Ints(vec![3, 2])),
                ("merge_mode", Value::Int(4)),
                ("height_map", Value::Array((0..n).map(|i| Value::Float(i as f64 * 0.5)).collect())),
                ("ground_texture_idx", Value::Bytes(vec![0, 1, 2, 3, 15, 20])),
                ("wall_texture_idx", Value::Bytes(vec![5; n])),
                ("grass_mask", Value::Bytes(vec![1, 0, 1, 0, 1, 0])),
                ("smooth_map", Value::Bytes(vec![0, 0, 1, 0, 0, 0])),
                ("mesh", Value::Object(ObjRef::Empty)),
            ],
        );
        let c = chunk_from_resource(&parse(&b).unwrap(), GridType::Square, iv2(9, 9)).unwrap();
        assert_eq!(c.coords, iv2(0, -1));
        assert_eq!(c.grid, Some(GridType::Hex));
        assert_eq!(c.size, iv2(3, 2));
        assert_eq!(c.layout(GridType::Hex), (2, 0));
        assert_eq!(c.data.merge_mode, MergeMode::Spherical);
        assert_eq!(c.data.heights[5], 2.5);
        assert_eq!(c.data.floor_tex, vec![0, 1, 2, 3, 15, 15]);
        assert_eq!(c.data.grass, vec![255, 0, 255, 0, 255, 0]);
        assert_eq!(c.data.smooth[2], 1);
        assert!(!c.legacy);
        // Round-trips through our own blob format.
        assert_eq!(ChunkData::from_bytes(&c.data.to_bytes()), Some(c.data.clone()));
    }

    #[test]
    fn imports_legacy_v1_square_chunk() {
        // 3x2 vertices, Array[Array] rows; colour maps instead of index bytes.
        let rows = Value::Array(vec![
            Value::Array(vec![Value::Float(0.0), Value::Float(1.0), Value::Float(2.0)]),
            Value::Array(vec![Value::Float(3.0), Value::Float(4.0), Value::Float(5.0)]),
        ]);
        let (a, b) = texture_idx_to_colors(6);
        let (wa, wb) = texture_idx_to_colors(9);
        let b = write_simple(
            "Resource",
            "MSTChunkData",
            &[
                ("chunk_coords", Value::Ints(vec![1, 1])),
                ("merge_mode", Value::Int(0)),
                ("height_map", rows),
                ("color_map_0", Value::Colors(vec![a; 6])),
                ("color_map_1", Value::Colors(vec![b; 6])),
                ("wall_color_map_0", Value::Colors(vec![wa; 6])),
                ("wall_color_map_1", Value::Colors(vec![wb; 6])),
                ("grass_mask_map", Value::Colors(vec![[1.0, 0.0, 0.0, 0.0], [0.0; 4], [1.0, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]])),
            ],
        );
        let c = chunk_from_resource(&parse(&b).unwrap(), GridType::Hex, iv2(0, 0)).unwrap();
        assert!(c.legacy);
        assert_eq!(c.grid, None);
        assert_eq!(c.size, iv2(3, 2));
        assert_eq!(c.layout(GridType::Square), (2, 0));
        assert_eq!(c.data.heights, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(c.data.floor_tex, vec![6; 6]);
        assert_eq!(c.data.wall_tex, vec![9; 6]);
        assert_eq!(c.data.grass, vec![255, 0, 255, 0, 0, 0]);
        assert_eq!(c.data.merge_mode, MergeMode::Cubic);
    }

    #[test]
    fn rejects_inconsistent_chunks() {
        let b = write_simple(
            "Resource",
            "MSTChunkData",
            &[("grid_type", Value::Int(2)), ("cell_count", Value::Ints(vec![4, 4])), ("height_map", Value::Array(vec![Value::Float(1.0)]))],
        );
        assert!(chunk_from_resource(&parse(&b).unwrap(), GridType::Hex, iv2(0, 0)).is_err());
    }
}
