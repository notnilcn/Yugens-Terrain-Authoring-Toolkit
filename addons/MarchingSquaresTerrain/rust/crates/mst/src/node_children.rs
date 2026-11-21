//! Ways of attaching and detaching nodes that a class owns privately, kept out of the scene
//! the author saves. The repeating timer made here is for owners that have no scene of
//! their own to declare it in, such as editor plugins.
use godot::classes::Node;
use godot::prelude::*;
pub fn add_internal(parent: &mut Gd<impl Inherits<Node>>, child: &Gd<impl Inherits<Node>>) {
    use godot::classes::node::InternalMode;
    parent
        .upcast_mut::<Node>()
        .add_child_ex(child)
        .internal(InternalMode::FRONT)
        .done();
}
pub fn add_repeating_timer(
    owner: &mut Gd<impl Inherits<Node>>,
    seconds: f64,
    method: &str,
    autostart: bool,
) -> Gd<godot::classes::Timer> {
    let mut t = godot::classes::Timer::new_alloc();
    t.set_name(method);
    t.set_wait_time(seconds);
    t.set_one_shot(false);
    t.set_autostart(autostart);
    let cb = Callable::from_object_method(&owner.clone().upcast::<Node>(), method);
    t.connect("timeout", &cb);
    owner.upcast_mut::<Node>().add_child(&t);
    t
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
