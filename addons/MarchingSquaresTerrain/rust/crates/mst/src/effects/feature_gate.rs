use crate::features::{self, Feature};
use godot::classes::Node;
use godot::prelude::*;
pub fn gate(node: &Gd<impl Inherits<Node>>, f: Feature) -> bool {
    let on = features::active(f);
    crate::writes::set_internals_visible(node, on);
    let mut n = node.clone().upcast::<Node>();
    n.set_process(on);
    n.call_deferred("update_configuration_warnings", &[]);
    on
}
pub fn gate_warnings(f: Feature) -> PackedStringArray {
    let mut w = PackedStringArray::new();
    if let Some(msg) = features::warning(f) {
        w.push(&msg);
    }
    w
}
