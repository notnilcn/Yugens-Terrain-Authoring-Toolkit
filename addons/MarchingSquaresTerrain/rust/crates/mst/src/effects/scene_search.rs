use crate::editor_detection::is_editor;
use godot::classes::Node;
use godot::prelude::*;
pub fn find_terrain(node: &Gd<Node>) -> Option<Gd<Node>> {
    find_in_scene(node, "MarchingSquaresTerrain")
}
thread_local! {
    static SCENE_LOOKUPS: std::cell::RefCell<std::collections::HashMap<&'static str, (InstanceId, Option<Gd<Node>>, u64)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}
pub fn find_in_scene(node: &Gd<Node>, class: &'static str) -> Option<Gd<Node>> {
    if !node.is_inside_tree() {
        return None;
    }
    let tree = node.get_tree();
    let root: Gd<Node> = if is_editor() {
        godot::classes::EditorInterface::singleton().get_edited_scene_root()?
    } else {
        tree.get_current_scene()
            .unwrap_or_else(|| tree.get_root().upcast())
    };
    let now = godot::classes::Time::singleton().get_ticks_msec();
    let root_id = root.instance_id();
    enum Cached {
        Hit(Gd<Node>),
        Miss,
        Stale,
    }
    let cached = SCENE_LOOKUPS.with_borrow(|m| match m.get(class) {
        Some((rid, Some(n), _))
            if *rid == root_id && n.is_instance_valid() && n.is_inside_tree() =>
        {
            Cached::Hit(n.clone())
        }
        Some((rid, None, retry_at)) if *rid == root_id && now < *retry_at => Cached::Miss,
        _ => Cached::Stale,
    });
    match cached {
        Cached::Hit(n) => return Some(n),
        Cached::Miss => return None,
        Cached::Stale => {}
    }
    let found = if root.is_class(class) {
        Some(root.clone())
    } else {
        root.find_children_ex("*")
            .type_(class)
            .recursive(true)
            .owned(false)
            .done()
            .iter_shared()
            .next()
    };
    SCENE_LOOKUPS.with_borrow_mut(|m| m.insert(class, (root_id, found.clone(), now + 1000)));
    found
}
pub fn ground_height(terrain: &mut Option<Gd<Node>>, p: Vector3, fallback: f32) -> f32 {
    match terrain {
        Some(t) if t.is_instance_valid() => t
            .call("get_height_at", &[p.to_variant(), fallback.to_variant()])
            .try_to::<f32>()
            .unwrap_or(fallback),
        _ => fallback,
    }
}
pub fn settle_terrain(chosen: &Option<Gd<Node>>, found: &mut Option<Gd<Node>>, node: &Gd<Node>) {
    if let Some(t) = crate::node_references::live(chosen) {
        *found = Some(t);
    } else if found.as_ref().is_none_or(|t| !t.is_instance_valid()) {
        *found = find_terrain(node);
    }
}
