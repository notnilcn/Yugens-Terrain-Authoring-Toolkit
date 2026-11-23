use godot::prelude::*;
use pt_core::math::{IVec2, Vec2 as PVec2, Vec3 as PVec3, iv2, v2};
pub fn to_iv(v: Vector2i) -> IVec2 {
    iv2(v.x, v.y)
}
pub fn from_iv(v: IVec2) -> Vector2i {
    Vector2i::new(v.x, v.y)
}
pub fn to_v2(v: Vector2) -> PVec2 {
    v2(v.x, v.y)
}
pub fn from_v3(v: PVec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}
