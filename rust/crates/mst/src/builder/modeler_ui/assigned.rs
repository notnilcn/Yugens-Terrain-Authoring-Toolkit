use godot::prelude::*;
pub fn assigned<T: GodotClass>(slot: &Option<Gd<T>>, scene: &str, role: &str) -> Option<Gd<T>> {
    let part = slot.clone();
    if part.is_none() {
        godot_warn!("Pixel Builder: {scene} has no {role} assigned; assign it in the scene.");
    }
    part
}
pub fn unassigned_warnings(slots: &[(bool, &str)]) -> PackedStringArray {
    slots
        .iter()
        .filter(|(missing, _)| *missing)
        .map(|(_, warning)| GString::from(*warning))
        .collect()
}
