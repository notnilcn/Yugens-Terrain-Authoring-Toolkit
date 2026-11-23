use godot::prelude::*;
pub fn object_flags(pixel_size: i32, outline: bool, outline_id: i32, snap: bool) -> i32 {
    pt_core::meta::object_flags(pixel_size, outline, outline_id, snap, true, true)
}
pub fn object_flags_for(
    owner: InstanceId,
    slot: u8,
    pixel_size: i32,
    outline: bool,
    outline_id: i32,
    snap: bool,
) -> i32 {
    crate::render::object::note_pixel_size(owner, slot, pixel_size);
    object_flags(pixel_size, outline, outline_id, snap)
}
