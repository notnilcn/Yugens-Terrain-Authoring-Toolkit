//! Scripting API for the sculpting stamp modes (`pt_core::stamp` Lift / Carve / Level / Broad,
//! modelled on Terrain3D's Sculpt tool), with an explicit level, floor and ceiling.

use godot::classes::Image;
use godot::prelude::*;

use pt_core::math::v2;
use pt_core::stamp::{SculptLimits, SculptMode, StampParams, op_stamp_limited};

use super::MarchingSquaresTerrain;
use super::stamp::stamp_from_image;

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Like `apply_stamp`, with the limits the sculpting modes use. `mode`: 0 add, 1 max,
    /// 2 min, 3 replace, 4 lift (stops at `ceiling`), 5 carve (stops at `floor`), 6 level
    /// towards `level`, 7 broad (absolute: `level` at value 0 rising by `height` at value 1,
    /// never lowers). Modes 4–7 use the linear sculpting contrast (1 = image as is, 0 = flat).
    /// Call lift / carve / level repeatedly for a hold-to-grow behaviour.
    #[func]
    #[allow(clippy::too_many_arguments)]
    pub fn apply_stamp_limited(&mut self, image: Gd<Image>, world: Vector3, size: Vector2, rotation: f32, height: f32, contrast: f32, mode: i64, level: f32, floor: f32, ceiling: f32) {
        let Some(stamp) = stamp_from_image(&image, 512) else {
            godot_warn!("MarchingSquaresTerrain.apply_stamp_limited: unreadable image");
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
            mode: SculptMode::from_i64(mode),
            base: self.local_height_at(c).unwrap_or(l.y),
        };
        let lim = SculptLimits { level, floor: floor.min(ceiling), ceiling: ceiling.max(floor) };
        self.edit(|st| op_stamp_limited(st, &stamp, &p, &lim));
        self.commit_to_data();
    }
}
