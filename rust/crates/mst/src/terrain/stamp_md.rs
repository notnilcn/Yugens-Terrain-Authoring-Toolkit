//! Scripting API for the Minor Deity stamp modes (`pt_core::stamp` Raise / Lower / Flatten /
//! Macro, ported from `ElevationStampJob`), with an explicit level, floor and ceiling.

use godot::classes::Image;
use godot::prelude::*;

use pt_core::math::v2;
use pt_core::stamp::{StampLimits, StampMode, StampParams, op_stamp_limited};

use super::MarchingSquaresTerrain;
use super::stamp::stamp_from_image;

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Like `apply_stamp`, with the limits Minor Deity's stampers use. `mode`: 0 add, 1 max,
    /// 2 min, 3 replace, 4 raise (stops at `ceiling`), 5 lower (stops at `floor`), 6 flatten
    /// towards `level`, 7 macro (absolute: `level` at value 0 rising by `height` at value 1,
    /// never lowers). Modes 4–7 use Minor Deity's linear contrast (1 = image as is, 0 = flat).
    /// Call raise/lower/flatten repeatedly for Minor Deity's hold-to-grow behaviour.
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
            mode: StampMode::from_i64(mode),
            base: self.local_height_at(c).unwrap_or(l.y),
        };
        let lim = StampLimits { level, floor: floor.min(ceiling), ceiling: ceiling.max(floor) };
        self.edit(|st| op_stamp_limited(st, &stamp, &p, &lim));
        self.commit_to_data();
    }
}
