use super::MarchingSquaresTerrain;
use super::stamp::stamp_from_image;
use godot::classes::Image;
use godot::prelude::*;
use pt_core::math::v2;
use pt_core::stamp::{SculptLimits, SculptMode, StampParams, op_stamp_limited};
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    #[expect(
        clippy::too_many_arguments,
        reason = "the argument list is the method's engine-facing signature, called by name from scenes and the inspector"
    )]
    pub fn apply_stamp_limited(
        &mut self,
        image: Gd<Image>,
        world: Vector3,
        size: Vector2,
        rotation: f32,
        height: f32,
        contrast: f32,
        mode: i64,
        level: f32,
        floor: f32,
        ceiling: f32,
    ) {
        let Some(stamp) = stamp_from_image(&image, 512) else {
            godot_warn!("MarchingSquaresTerrain.apply_stamp_limited: unreadable image");
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
        let lim = SculptLimits {
            level,
            floor: floor.min(ceiling),
            ceiling: ceiling.max(floor),
        };
        self.edit(|st| op_stamp_limited(st, &stamp, &p, &lim));
        self.commit_to_data();
    }
}
