//! Grass look: the per-slot leaf-sprite texture array and the wind texture bound to
//! `mst_grass.gdshader`. The resources are owned by the terrain node so the RIDs bound through
//! the RenderingServer stay valid (a dropped texture leaves a dangling RID and the sampler
//! falls back to its default).

use godot::classes::image::{Format, Interpolation};
use godot::classes::{Image, Texture2D, Texture2DArray};
use godot::prelude::*;

use super::textures::{MarchingSquaresTexturePreset, SLOT_COUNT};

pub const GRASS_TEXTURE: &str = "res://addons/MarchingSquaresTerrain/textures/grass_blade.png";
pub const WIND_TEXTURE: &str = "res://addons/MarchingSquaresTerrain/textures/wind_noise_texture.tres";

fn sprite_image(t: &Gd<Texture2D>) -> Option<Gd<Image>> {
    let mut img = t.get_image()?;
    if img.is_compressed() {
        img.decompress();
    }
    img.clear_mipmaps();
    img.convert(Format::RGBA8);
    Some(img)
}

/// 16 layers, one per slot: the slot's `grass_sprite`, else `default` (the terrain's
/// `grass_texture` or the bundled leaf sprite). All layers are resized (nearest) to the largest.
pub fn build_sprite_array(ts: Option<&MarchingSquaresTexturePreset>, default: Option<&Gd<Texture2D>>) -> Option<Gd<Texture2DArray>> {
    let mut layers: Vec<Option<Gd<Image>>> = (0..SLOT_COUNT)
        .map(|i| {
            ts.and_then(|ts| ts.slot(i))
                .and_then(|s| s.bind().grass_sprite.clone())
                .and_then(|t| sprite_image(&t))
                .or_else(|| default.and_then(sprite_image))
        })
        .collect();
    let size = layers.iter().flatten().map(|i| i.get_width().max(i.get_height())).max().unwrap_or(0).clamp(0, 512);
    if size == 0 {
        return None;
    }
    let mut images: Array<Gd<Image>> = Array::new();
    for l in layers.iter_mut() {
        let img = match l.take() {
            Some(mut img) => {
                if img.get_width() != size || img.get_height() != size {
                    img.resize_ex(size, size).interpolation(Interpolation::NEAREST).done();
                }
                img
            }
            None => Image::create_empty(size, size, false, Format::RGBA8)?,
        };
        images.push(&img);
    }
    let mut arr = Texture2DArray::new_gd();
    if arr.create_from_images(&images) != godot::global::Error::OK {
        godot_error!("MarchingSquaresTerrain: could not build the grass sprite array");
        return None;
    }
    Some(arr)
}

pub fn default_sprite() -> Option<Gd<Texture2D>> {
    try_load::<Texture2D>(GRASS_TEXTURE).ok()
}

pub fn wind_texture() -> Option<Gd<Texture2D>> {
    try_load::<Texture2D>(WIND_TEXTURE).ok()
}
