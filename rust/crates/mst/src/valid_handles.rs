use godot::prelude::*;
pub fn live<T: GodotClass>(n: &Option<Gd<T>>) -> Option<Gd<T>> {
    n.as_ref().filter(|n| n.is_instance_valid()).cloned()
}
