//! Texture slots. A terrain has 16 slots (slot 15 is "void": painted cells are cut out).
//! Each slot has an optional texture; without one a small procedural pixel texture is
//! generated from the slot's `pattern`. The default texture set
//! ([`MarchingSquaresTexturePreset::new_default`]) uses the bundled ground textures in
//! `textures/ground/` (one PNG per pattern, baked from the procedural patterns, so both look the
//! same) and the original MST's leaf sprite on every grass slot.

use godot::classes::image::{Format, Interpolation};
use godot::classes::{IResource, Image, Texture2D, Texture2DArray};
use godot::prelude::*;
use pt_core::math::{hash01, iv2, value_noise, v2};

pub const SLOT_COUNT: usize = 16;

/// Bundled ground textures, one per `pattern` (index = pattern).
pub const GROUND_DIR: &str = "res://addons/MarchingSquaresTerrain/textures/ground/";
pub const PATTERN_FILES: [&str; 14] =
    ["grass", "meadow", "forest_floor", "sand", "dirt", "cliff", "dark_rock", "snow", "gravel", "clay", "moss", "path", "plain", "void"];

fn load_texture(path: &str) -> Option<Gd<Texture2D>> {
    if !godot::classes::ResourceLoader::singleton().exists(path) {
        return None;
    }
    try_load::<Texture2D>(path).ok()
}

/// The bundled ground texture of a procedural pattern (`None` if the file is missing).
pub fn pattern_texture(pattern: i32) -> Option<Gd<Texture2D>> {
    let f = PATTERN_FILES.get(pattern.clamp(0, 13) as usize)?;
    load_texture(&format!("{GROUND_DIR}{f}.png"))
}

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct MarchingSquaresTextureSlot {
    #[export]
    pub display_name: GString,
    /// Optional texture. When empty, `pattern` generates one procedurally.
    #[export]
    pub texture: Option<Gd<Texture2D>>,
    #[export(enum = (Grass, Meadow, Forest, Sand, Dirt, Cliff, DarkRock, Snow, Gravel, Clay, Moss, Path, Plain, Void))]
    pub pattern: i32,
    #[export]
    pub tint: Color,
    /// Texture repeat multiplier (higher = smaller texels).
    #[export(range = (0.1, 20.0, or_greater))]
    pub scale: f32,
    /// Whether grass grows on this slot.
    #[export]
    pub has_grass: bool,
    /// Color multiplier for grass growing on this slot.
    #[export]
    pub grass_tint: Color,
    /// Leaf sprite for grass on this slot (white leaves on alpha, like MST's
    /// `grass_sprite_tex_N`). Empty = the terrain's `grass_texture`.
    #[export]
    pub grass_sprite: Option<Gd<Texture2D>>,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for MarchingSquaresTextureSlot {
    fn init(base: Base<Resource>) -> Self {
        MarchingSquaresTextureSlot {
            display_name: "Slot".into(),
            texture: None,
            pattern: 12,
            tint: Color::from_rgb(0.8, 0.8, 0.8),
            scale: 1.0,
            has_grass: false,
            grass_tint: Color::from_rgb(1.05, 1.1, 0.95),
            grass_sprite: None,
            base,
        }
    }
}

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct MarchingSquaresTexturePreset {
    /// Name shown in the editor's "Texture Preset" dropdown.
    #[export]
    pub preset_name: GString,
    #[export]
    pub slots: Array<Gd<MarchingSquaresTextureSlot>>,
    /// Edge length of procedural textures (user textures are resized to the largest size).
    #[export(range = (8.0, 256.0, 1.0))]
    pub texture_size: i32,
    /// Biome auto-texturing rules (rock on slopes, sand shores, snow, cliff lips, fillers).
    /// Empty = off; the Vertex Painting panel creates one on first edit.
    #[export]
    pub auto_rules: Option<Gd<MarchingSquaresAutoRules>>,
    base: Base<Resource>,
}

/// Biome auto-texturing rules of a texture preset (`pt_core::autotex::AutoRules`): replace the
/// *unpainted* slots in `base_slots` with a filler variant by noise, sand near the water line,
/// rock on steep slopes, a lip on cliff tops and snow above a noisy line that thins out on steep
/// ground. Painting any other slot wins. Heights are terrain-local. Slot -1 turns a rule off.
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct MarchingSquaresAutoRules {
    #[export]
    pub enabled: bool,
    /// Slots treated as "unpainted" (the rules replace them). Default: slot 0.
    #[export(flags = (Slot0, Slot1, Slot2, Slot3, Slot4, Slot5, Slot6, Slot7, Slot8, Slot9, Slot10, Slot11, Slot12, Slot13, Slot14, Slot15))]
    pub base_slots: i32,
    /// World frequency of the jitter noise roughening every threshold.
    #[export(range = (0.01, 4.0, 0.01))]
    pub noise_scale: f32,
    #[export_group(name = "Filler")]
    #[export(range = (-1.0, 14.0, 1.0))]
    pub filler_slot: i32,
    #[export(range = (0.0, 1.0, 0.01))]
    pub filler_amount: f32,
    #[export(range = (0.005, 1.0, 0.005))]
    pub filler_scale: f32,
    #[export_group(name = "Rock")]
    #[export(range = (-1.0, 14.0, 1.0))]
    pub rock_slot: i32,
    /// Slopes steeper than this (degrees) become rock.
    #[export(range = (0.0, 90.0, 0.5))]
    pub rock_slope: f32,
    #[export(range = (0.0, 30.0, 0.5))]
    pub rock_noise: f32,
    #[export(range = (-1.0, 14.0, 1.0))]
    pub rock_filler_slot: i32,
    #[export_group(name = "Cliff Lip")]
    #[export(range = (-1.0, 14.0, 1.0))]
    pub lip_slot: i32,
    /// Ridge band factor (0 = far from a cliff top, 1 = on the edge) above which the lip shows.
    #[export(range = (0.0, 1.0, 0.01))]
    pub lip_threshold: f32,
    #[export_group(name = "Shore")]
    #[export(range = (-1.0, 14.0, 1.0))]
    pub shore_slot: i32,
    #[export]
    pub water_level: f32,
    /// Sand band height above `water_level`.
    #[export(range = (0.0, 20.0, 0.05))]
    pub shore_height: f32,
    #[export(range = (0.0, 5.0, 0.05))]
    pub shore_noise: f32,
    #[export(range = (-1.0, 14.0, 1.0))]
    pub shore_filler_slot: i32,
    #[export_group(name = "Snow")]
    #[export(range = (-1.0, 14.0, 1.0))]
    pub snow_slot: i32,
    #[export]
    pub snow_height: f32,
    /// Amplitude of the fbm noise moving the snow line up and down.
    #[export(range = (0.0, 50.0, 0.1))]
    pub snow_noise: f32,
    #[export(range = (0.001, 1.0, 0.001))]
    pub snow_noise_scale: f32,
    /// No snow on slopes steeper than this (degrees)…
    #[export(range = (0.0, 90.0, 0.5))]
    pub snow_max_slope: f32,
    /// …fading out over this many degrees below it.
    #[export(range = (0.0, 90.0, 0.5))]
    pub snow_slope_fade: f32,
    /// Height over which snow thickens above the line.
    #[export(range = (0.0, 20.0, 0.05))]
    pub snow_blend: f32,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for MarchingSquaresAutoRules {
    fn init(base: Base<Resource>) -> Self {
        let d = pt_core::autotex::AutoRules::default();
        MarchingSquaresAutoRules {
            enabled: d.enabled,
            base_slots: d.base_mask as i32,
            noise_scale: d.noise_scale,
            filler_slot: d.filler_slot,
            filler_amount: d.filler_amount,
            filler_scale: d.filler_scale,
            rock_slot: d.rock_slot,
            rock_slope: d.rock_slope,
            rock_noise: d.rock_noise,
            rock_filler_slot: d.rock_filler_slot,
            lip_slot: d.lip_slot,
            lip_threshold: d.lip_threshold,
            shore_slot: d.shore_slot,
            water_level: d.water_level,
            shore_height: d.shore_height,
            shore_noise: d.shore_noise,
            shore_filler_slot: d.shore_filler_slot,
            snow_slot: d.snow_slot,
            snow_height: d.snow_height,
            snow_noise: d.snow_noise,
            snow_noise_scale: d.snow_noise_scale,
            snow_max_slope: d.snow_max_slope,
            snow_slope_fade: d.snow_slope_fade,
            snow_blend: d.snow_blend,
            base,
        }
    }
}

impl MarchingSquaresAutoRules {
    pub fn to_core(&self) -> pt_core::autotex::AutoRules {
        pt_core::autotex::AutoRules {
            enabled: self.enabled,
            base_mask: (self.base_slots & 0xffff) as u16,
            noise_scale: self.noise_scale,
            filler_slot: self.filler_slot,
            filler_amount: self.filler_amount,
            filler_scale: self.filler_scale,
            rock_slot: self.rock_slot,
            rock_slope: self.rock_slope,
            rock_noise: self.rock_noise,
            rock_filler_slot: self.rock_filler_slot,
            lip_slot: self.lip_slot,
            lip_threshold: self.lip_threshold,
            shore_slot: self.shore_slot,
            water_level: self.water_level,
            shore_height: self.shore_height,
            shore_noise: self.shore_noise,
            shore_filler_slot: self.shore_filler_slot,
            snow_slot: self.snow_slot,
            snow_height: self.snow_height,
            snow_noise: self.snow_noise,
            snow_noise_scale: self.snow_noise_scale,
            snow_max_slope: self.snow_max_slope,
            snow_slope_fade: self.snow_slope_fade,
            snow_blend: self.snow_blend,
        }
    }
}

/// Grass color multipliers of the default grass slots (Grass, Meadow, Forest Floor, Moss).
const GRASS_TINTS: [(usize, [f32; 3]); 4] = [(0, [1.05, 1.1, 0.95]), (1, [1.12, 1.1, 0.88]), (2, [0.92, 1.05, 0.92]), (10, [1.0, 1.08, 0.86])];

/// (name, pattern, tint, has_grass) for the 16 default slots, Minor Deity-inspired: lush
/// grass, sandy shores, layered cliff rock.
const DEFAULTS: [(&str, i32, u32, bool); SLOT_COUNT] = [
    ("Grass", 0, 0x6f9a4a, true),
    ("Meadow", 1, 0x93b257, true),
    ("Forest Floor", 2, 0x4c6b3a, true),
    ("Sand", 3, 0xd9c48f, false),
    ("Dirt", 4, 0x8c6a48, false),
    ("Cliff", 5, 0xa08c78, false),
    ("Dark Rock", 6, 0x6a6460, false),
    ("Snow", 7, 0xeef2f4, false),
    ("Gravel", 8, 0x9a948a, false),
    ("Clay", 9, 0xb06e4c, false),
    ("Moss", 10, 0x5f7f3c, true),
    ("Path", 11, 0xc2a57c, false),
    ("Stone Tiles", 12, 0xa8a49a, false),
    ("Ash", 12, 0x55524f, false),
    ("Ice", 12, 0xb9d9e6, false),
    ("Void", 13, 0xffffff, false),
];

fn hex_color(c: u32) -> Color {
    Color::from_rgb(((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0)
}

#[godot_api]
impl IResource for MarchingSquaresTexturePreset {
    fn init(base: Base<Resource>) -> Self {
        let mut slots = Array::new();
        for (name, pattern, tint, grass) in DEFAULTS {
            let mut s = MarchingSquaresTextureSlot::new_gd();
            {
                let mut b = s.bind_mut();
                b.display_name = name.into();
                b.pattern = pattern;
                b.tint = hex_color(tint);
                b.has_grass = grass;
            }
            slots.push(&s);
        }
        MarchingSquaresTexturePreset { preset_name: GString::new(), slots, texture_size: 32, auto_rules: None, base }
    }
}

/// Procedural near-white pixel pattern (the slot tint colors it in the shader).
fn pattern_pixels(pattern: i32, size: usize, seed: u32) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let c = iv2(x as i32, y as i32);
            let h = hash01(c, seed);
            // Tileable noise: sample on a torus by wrapping the lattice period to `size`.
            let n = |f: f32| {
                let p = v2(x as f32 / s * f, y as f32 / s * f);
                let a = value_noise(p, seed);
                let b = value_noise(v2(p.x - f, p.y), seed);
                let c2 = value_noise(v2(p.x, p.y - f), seed);
                let d = value_noise(v2(p.x - f, p.y - f), seed);
                let tx = x as f32 / s;
                let ty = y as f32 / s;
                (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c2 * (1.0 - tx) + d * tx) * ty
            };
            let v: f32 = match pattern {
                0 => 0.86 + 0.1 * n(4.0) + if h > 0.9 { 0.12 } else if h < 0.08 { -0.12 } else { 0.0 },
                1 => 0.9 + 0.06 * n(3.0) + if h > 0.93 { 0.15 } else { 0.0 },
                2 => 0.8 + 0.14 * n(5.0) + if h < 0.12 { -0.15 } else { 0.0 },
                3 => 0.93 + 0.04 * n(6.0) + if h > 0.92 { 0.06 } else if h < 0.06 { -0.08 } else { 0.0 },
                4 => 0.82 + 0.12 * n(6.0) + if h > 0.9 { -0.12 } else { 0.0 },
                5 | 6 => {
                    // Horizontal strata with noisy offsets (walls sample uv.y = height).
                    let band = ((y as f32 + 3.0 * n(2.0)) / (s / 6.0)).floor();
                    let stripe = hash01(iv2(0, band as i32), seed ^ 77);
                    0.74 + 0.18 * stripe + 0.08 * n(8.0) + if h > 0.94 { -0.15 } else { 0.0 }
                }
                7 => 0.86 + 0.1 * n(3.0) + if h > 0.93 { 0.08 } else if h < 0.05 { -0.06 } else { 0.0 },
                8 => 0.78 + 0.22 * h,
                9 => 0.85 + 0.1 * n(5.0) + if (y % 8) == 0 && h > 0.4 { -0.08 } else { 0.0 },
                10 => 0.8 + 0.18 * n(7.0),
                11 => 0.88 + 0.08 * n(4.0) + if h > 0.95 { -0.1 } else { 0.0 },
                12 => {
                    let tile = (x % (size / 2).max(1) == 0) || (y % (size / 2).max(1) == 0);
                    if tile { 0.7 } else { 0.92 + 0.05 * n(4.0) }
                }
                _ => 1.0,
            };
            let v = (v.clamp(0.0, 1.0) * 255.0) as u8;
            let o = (y * size + x) * 4;
            out[o] = v;
            out[o + 1] = v;
            out[o + 2] = v;
            out[o + 3] = if pattern == 13 { 0 } else { 255 };
        }
    }
    out
}

impl MarchingSquaresTexturePreset {
    /// The default texture set with its bundled assets: every slot gets the ground texture of
    /// its pattern, grass slots the leaf sprite and their own grass color.
    pub fn new_default() -> Gd<Self> {
        let mut p = Self::new_gd();
        p.bind_mut().assign_default_assets();
        p
    }

    /// Texture set reproducing an original MST terrain's look (`pt_core::mst_import::MstSlot`,
    /// slots 1-15; void stays). Slot textures, scales, albedos (ground tint; the grass takes the
    /// ground color, like the original's `grass_color_N`), grass sprites and grass flags are
    /// copied; `load` resolves a texture path of the original project.
    pub fn from_mst(slots: &[pt_core::mst_import::MstSlot], load: impl Fn(&str) -> Option<Gd<Texture2D>>) -> Gd<Self> {
        let mut p = Self::new_default();
        p.bind_mut().preset_name = "Imported MST".into();
        for (i, src) in slots.iter().enumerate().take(SLOT_COUNT - 1) {
            let Some(mut slot) = p.bind().slot(i) else { continue };
            let mut b = slot.bind_mut();
            let original = i < pt_core::mst_import::ORIG_GRASS_SLOTS;
            if let Some(t) = src.texture.path().and_then(&load) {
                b.texture = Some(t);
            } else if original {
                // The original drew an empty slot as its flat albedo.
                b.texture = pattern_texture(12);
                b.pattern = 12;
            }
            if original {
                b.display_name = GString::from(format!("Texture {}", i + 1).as_str());
            }
            b.scale = src.scale.max(0.01);
            if let Some(c) = src.albedo {
                b.tint = Color::from_rgba(c[0], c[1], c[2], 1.0);
                b.grass_tint = Color::WHITE;
            }
            b.has_grass = src.has_grass;
            if let Some(r) = &src.grass_sprite {
                b.grass_sprite = r.path().and_then(&load).or_else(super::grass_look::default_sprite);
            }
        }
        p
    }

    pub fn slot(&self, i: usize) -> Option<Gd<MarchingSquaresTextureSlot>> {
        self.slots.get(i)
    }

    /// Builds the 16-layer texture array used by the terrain and grass shaders.
    pub fn build_array(&self) -> Option<Gd<Texture2DArray>> {
        let mut size = self.texture_size.clamp(8, 256);
        for i in 0..SLOT_COUNT {
            if let Some(slot) = self.slot(i)
                && let Some(t) = &slot.bind().texture
            {
                size = size.max(t.get_width().max(t.get_height()));
            }
        }
        let size = size.clamp(8, 512);
        let mut images: Array<Gd<Image>> = Array::new();
        for i in 0..SLOT_COUNT {
            let mut img = self.layer_image(i, size)?;
            img.generate_mipmaps();
            images.push(&img);
        }
        let mut arr = Texture2DArray::new_gd();
        let err = arr.create_from_images(&images);
        if err != godot::global::Error::OK {
            godot_error!("MarchingSquaresTexturePreset: could not build texture array ({err:?})");
            return None;
        }
        Some(arr)
    }

    /// RGBA8 `size`² image of slot `i` (its texture, or the procedural pattern), untinted.
    pub fn layer_image(&self, i: usize, size: i32) -> Option<Gd<Image>> {
        let (texture, pattern) = match self.slot(i) {
            Some(s) => {
                let b = s.bind();
                (b.texture.clone(), b.pattern)
            }
            None => (None, 12),
        };
        texture
            .and_then(|t| t.get_image())
            .map(|mut img| {
                if img.is_compressed() {
                    img.decompress();
                }
                img.clear_mipmaps();
                img.convert(Format::RGBA8);
                if img.get_width() != size || img.get_height() != size {
                    img.resize_ex(size, size).interpolation(Interpolation::NEAREST).done();
                }
                img
            })
            .or_else(|| {
                let px = pattern_pixels(pattern, size as usize, 1013 + i as u32 * 7919);
                Image::create_from_data(size, size, false, Format::RGBA8, &PackedByteArray::from(px.as_slice()))
            })
    }

    /// Tinted preview of slot `i` for the editor texture panel.
    pub fn preview_image(&self, i: usize, size: i32) -> Option<Gd<Image>> {
        let mut img = self.layer_image(i, size)?;
        let tint = self.slot(i).map(|s| s.bind().tint).unwrap_or(Color::WHITE);
        for y in 0..size {
            for x in 0..size {
                let c = img.get_pixel(x, y);
                img.set_pixel(x, y, Color::from_rgba(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a));
            }
        }
        Some(img)
    }

    pub fn tints(&self) -> PackedColorArray {
        (0..SLOT_COUNT).map(|i| self.slot(i).map(|s| s.bind().tint).unwrap_or(Color::WHITE)).collect()
    }

    pub fn scales(&self) -> PackedFloat32Array {
        (0..SLOT_COUNT).map(|i| self.slot(i).map(|s| s.bind().scale.max(0.01)).unwrap_or(1.0)).collect()
    }

    pub fn grass_tints(&self) -> PackedColorArray {
        (0..SLOT_COUNT).map(|i| self.slot(i).map(|s| s.bind().grass_tint).unwrap_or(Color::WHITE)).collect()
    }

    pub fn has_grass(&self) -> [bool; 16] {
        let mut out = [false; 16];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.slot(i).map(|s| s.bind().has_grass).unwrap_or(false);
        }
        out
    }

    pub fn slot_name(&self, i: usize) -> String {
        self.slot(i).map(|s| s.bind().display_name.to_string()).unwrap_or_else(|| format!("Slot {i}"))
    }
}

#[godot_api]
impl MarchingSquaresTexturePreset {
    /// Fills every slot without a texture with the bundled ground texture of its pattern, and
    /// gives grass slots without a sprite the bundled leaf sprite (and their default grass
    /// color while it is unchanged). Used for the default texture set.
    #[func]
    pub fn assign_default_assets(&mut self) {
        let sprite = super::grass_look::default_sprite();
        let plain = MarchingSquaresTextureSlot::new_gd().bind().grass_tint;
        for i in 0..SLOT_COUNT {
            let Some(mut slot) = self.slot(i) else { continue };
            let mut b = slot.bind_mut();
            if b.texture.is_none() {
                b.texture = pattern_texture(b.pattern);
            }
            if b.has_grass && b.grass_sprite.is_none() {
                b.grass_sprite = sprite.clone();
            }
            if b.grass_tint == plain
                && let Some((_, t)) = GRASS_TINTS.iter().find(|(k, _)| *k == i)
            {
                b.grass_tint = Color::from_rgb(t[0], t[1], t[2]);
            }
        }
    }

    /// RGBA8 `size`² image of slot `slot` (its texture, or the procedural pattern), untinted.
    #[func]
    pub fn get_slot_image(&self, slot: i64, size: i64) -> Option<Gd<Image>> {
        self.layer_image(slot.clamp(0, 15) as usize, size.clamp(1, 1024) as i32)
    }

    /// Display name of a slot (used by the editor palette).
    #[func]
    fn get_slot_name(&self, slot: i64) -> GString {
        GString::from(self.slot_name(slot.clamp(0, 15) as usize).as_str())
    }
}
