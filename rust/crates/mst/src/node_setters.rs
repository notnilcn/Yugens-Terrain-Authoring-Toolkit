use godot::classes::Node3D;
use godot::prelude::*;
pub fn set_global_transform(node: &mut Gd<impl Inherits<Node3D>>, t: Transform3D) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_transform() != t {
        n.set_global_transform(t);
    }
}
pub fn set_internals_visible(node: &Gd<impl Inherits<Node>>, shown: bool) {
    let node = node.upcast_ref::<Node>();
    let internal = node.get_child_count_ex().include_internal(true).done() - node.get_child_count();
    for i in 0..internal {
        if let Some(c) = node.get_child_ex(i).include_internal(true).done()
            && let Ok(mut n) = c.try_cast::<Node3D>()
            && n.is_visible() != shown
        {
            n.set_visible(shown);
        }
    }
}
pub fn assign_world_location(node: &mut Gd<impl Inherits<Node3D>>, p: Vector3) {
    let n = node.upcast_mut::<Node3D>();
    if !n.is_inside_tree() || n.get_global_position() != p {
        n.set_global_position(p);
    }
}
#[macro_export]
macro_rules! set_changed {
    ($obj:expr, $get:ident($($a:expr),*), $set:ident($($b:expr),*), $v:expr) => {{
        let v = $v;
        if $obj.$get($($a),*) != v {
            $obj.$set($($b,)* v);
        }
    }};
    ($obj:expr, $get:ident, $set:ident, $v:expr) => {{
        let v = $v;
        if $obj.$get() != v {
            $obj.$set(v);
        }
    }};
}
