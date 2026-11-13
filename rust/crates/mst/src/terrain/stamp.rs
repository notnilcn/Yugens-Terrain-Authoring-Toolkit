//! Heightmap stamps on the terrain node: built-in stamp images, image → `pt_core::stamp::Stamp`
//! conversion and the scripting API (`apply_stamp`). The Stamp editor tool uses the same code.

use godot::classes::Image;
use godot::classes::image::Format;
use godot::prelude::*;

use pt_core::math::v2;
use pt_core::stamp::{BuiltinStamp, Stamp, StampMode, StampParams, op_stamp};

use super::MarchingSquaresTerrain;

/// Folder scanned by the Stamp tool for extra stamp images (greyscale; white = high).
pub const STAMP_DIR: &str = "res://addons/MarchingSquaresTerrain/stamps/";

/// Converts any image to a stamp (luminance, `max_size` caps the resolution).
pub fn stamp_from_image(img: &Gd<Image>, max_size: i32) -> Option<Stamp> {
    let mut img = img.duplicate_resource();
    if img.is_compressed() && img.decompress() != godot::global::Error::OK {
        return None;
    }
    let (w, h) = (img.get_width(), img.get_height());
    if w < 2 || h < 2 {
        return None;
    }
    if w.max(h) > max_size {
        let k = max_size as f32 / w.max(h) as f32;
        img.resize(((w as f32 * k) as i32).max(2), ((h as f32 * k) as i32).max(2));
    }
    img.convert(Format::L8);
    Stamp::from_gray8(img.get_width() as usize, img.get_height() as usize, img.get_data().as_slice())
}

pub fn stamp_to_image(s: &Stamp) -> Option<Gd<Image>> {
    Image::create_from_data(s.width as i32, s.height as i32, false, Format::L8, &PackedByteArray::from(s.to_gray8().as_slice()))
}

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Greyscale image of a built-in stamp (`Mountain`, `Crater`, `Mesa`, `Valley`, `Hill`).
    #[func]
    pub fn create_stamp_image(kind: GString, size: i64) -> Option<Gd<Image>> {
        let k = BuiltinStamp::from_name(&kind.to_string())?;
        stamp_to_image(&Stamp::builtin(k, size.clamp(8, 1024) as usize))
    }

    /// Stamps a greyscale image (white = `height`) centred at a world position.
    /// `mode`: 0 add, 1 max, 2 min, 3 replace (Max/Min/Replace are relative to the terrain
    /// height at the centre). `rotation` in degrees, `size` = world extent (x, z).
    #[func]
    #[allow(clippy::too_many_arguments)]
    pub fn apply_stamp(&mut self, image: Gd<Image>, world: Vector3, size: Vector2, rotation: f32, height: f32, contrast: f32, mode: i64) {
        let Some(stamp) = stamp_from_image(&image, 512) else {
            godot_warn!("MarchingSquaresTerrain.apply_stamp: unreadable image");
            return;
        };
        let xf = if self.base().is_inside_tree() { self.base().get_global_transform() } else { self.base().get_transform() };
        let l = xf.affine_inverse() * world;
        let c = v2(l.x, l.z);
        let p = StampParams {
            center: c,
            rotation,
            size: v2(size.x, size.y),
            height,
            contrast,
            mode: StampMode::from_i64(mode),
            base: self.local_height_at(c).unwrap_or(l.y),
        };
        self.edit(|st| op_stamp(st, &stamp, &p));
        self.commit_to_data();
    }
}
