//! Ways of attaching and detaching nodes that a class owns privately, kept out of the scene
//! the author saves.
//!
//! A private part built from a packed scene keeps that scene's ownership, and the 3D editor
//! gives every owned node under the edited scene a gizmo that it draws and picks against.
//! Attaching a part therefore clears that ownership first, so private parts such as the
//! pixel preview's cameras or a fire's spawned lamps never show or catch editor gizmos.
use godot::classes::Node;
use godot::prelude::*;
pub fn add_internal(parent: &mut Gd<impl Inherits<Node>>, child: &Gd<impl Inherits<Node>>) {
    use godot::classes::node::InternalMode;
    disown_descendants(child);
    parent
        .upcast_mut::<Node>()
        .add_child_ex(child)
        .internal(InternalMode::FRONT)
        .done();
}
fn disown_descendants(node: &Gd<impl Inherits<Node>>) {
    let mut stack: Vec<Gd<Node>> = node
        .upcast_ref::<Node>()
        .get_children_ex()
        .include_internal(true)
        .done()
        .iter_shared()
        .collect();
    while let Some(mut n) = stack.pop() {
        stack.extend(n.get_children_ex().include_internal(true).done().iter_shared());
        if n.get_owner().is_some() {
            n.set_owner(Gd::<Node>::null_arg());
        }
    }
}
pub fn free_node(n: Option<Gd<impl Inherits<Node>>>) {
    if let Some(n) = n {
        let mut n = n.upcast::<Node>();
        if n.is_instance_valid() {
            if let Some(mut p) = n.get_parent() {
                p.remove_child(&n);
            }
            n.queue_free();
        }
    }
}
