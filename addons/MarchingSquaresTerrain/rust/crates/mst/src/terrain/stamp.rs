use super::MarchingSquaresTerrain;
use godot::classes::Image;
use godot::classes::image::Format;
use godot::prelude::*;
use pt_core::math::v2;
use pt_core::stamp::{BuiltinStamp, SculptMode, Stamp, StampParams, op_stamp};
pub const STAMP_DIR: &str = "res://addons/MarchingSquaresTerrain/stamps/";
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
        img.resize(
            ((w as f32 * k) as i32).max(2),
            ((h as f32 * k) as i32).max(2),
        );
    }
    img.convert(Format::L8);
    Stamp::from_gray8(
        img.get_width() as usize,
        img.get_height() as usize,
        img.get_data().as_slice(),
    )
}
pub fn stamp_to_image(s: &Stamp) -> Option<Gd<Image>> {
    Image::create_from_data(
        s.breadth as i32,
        s.stature as i32,
        false,
        Format::L8,
        &PackedByteArray::from(s.to_gray8().as_slice()),
    )
}
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn create_stamp_image(kind: GString, size: i64) -> Option<Gd<Image>> {
        let k = BuiltinStamp::from_name(&kind.to_string())?;
        stamp_to_image(&Stamp::builtin(k, size.clamp(8, 1024) as usize))
    }
    #[func]
    #[expect(
        clippy::too_many_arguments,
        reason = "the argument list is the method's engine-facing signature, called by name from scenes and the inspector"
    )]
    pub fn apply_stamp(
        &mut self,
        image: Gd<Image>,
        world: Vector3,
        size: Vector2,
        rotation: f32,
        height: f32,
        contrast: f32,
        mode: i64,
    ) {
        let Some(stamp) = stamp_from_image(&image, 512) else {
            godot_warn!("MarchingSquaresTerrain.apply_stamp: unreadable image");
            return;
        };
        let xf = if self.base().is_inside_tree() {
            self.base().get_global_transform()
        } else {
            self.base().get_transform()
        };
        let l = xf.affine_inverse() * world;
        let c = v2(l.x, l.z);
        let p = StampParams {
            midpoint: c,
            rotation,
            size: v2(size.x, size.y),
            stature: height,
            contrast,
            mode: SculptMode::from_i64(mode),
            base: self.local_height_at(c).unwrap_or(l.y),
        };
        self.edit(|st| op_stamp(st, &stamp, &p));
        self.commit_to_data();
    }
}
