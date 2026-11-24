//! Gathers parts under a new container node in the scene being edited, and dissolves such
//! containers again, the way a level author groups parts into models and folders.
//!
//! The containers are created in code because making them is the job of the editor tool
//! and the scene author keeps them. Each change goes through the editor's undo history as
//! one step, is owned by the edited scene so it is saved with it, and leaves every moved
//! part exactly where it was in the world.
use super::*;
use godot::classes::{EditorUndoRedoManager, VisualInstance3D};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroupKind {
    Model,
    Folder,
}
impl GroupKind {
    fn node_name(self) -> &'static str {
        match self {
            GroupKind::Model => "Model",
            GroupKind::Folder => "Folder",
        }
    }
}
struct Moved {
    node: Gd<Node>,
    from: Gd<Node>,
    index: i32,
    name: StringName,
    local: Option<Transform3D>,
    global: Option<Transform3D>,
    owned: Vec<Gd<Node>>,
}
pub(super) fn owned_subtree(n: &Gd<Node>, root: &Gd<Node>) -> Vec<Gd<Node>> {
    let mut out = Vec::new();
    let mut stack = vec![n.clone()];
    while let Some(x) = stack.pop() {
        if x.get_owner().is_some_and(|o| o.instance_id() == root.instance_id()) {
            out.push(x.clone());
        }
        stack.extend(x.get_children().iter_shared());
    }
    out
}
pub(super) fn owned_by(n: &Gd<Node>, root: &Gd<Node>) -> bool {
    n.get_owner()
        .is_some_and(|o| o.instance_id() == root.instance_id())
}
pub(super) fn top_selection() -> Vec<Gd<Node>> {
    EditorInterface::singleton()
        .get_selection()
        .map(|s| s.get_top_selected_nodes().iter_shared().collect())
        .unwrap_or_default()
}
fn is_container(n: &Gd<Node>) -> bool {
    n.get_class() == "Node3D" && n.get_scene_file_path().is_empty() && n.get_child_count() > 0
}
pub(super) fn holds_parts(n: &Gd<Node>) -> bool {
    is_container(n)
        && !n
            .find_children_ex("*")
            .type_("PixelBuilderMesh")
            .owned(false)
            .done()
            .is_empty()
}
fn global_of(n: &Gd<Node>) -> Option<Transform3D> {
    n.clone()
        .try_cast::<Node3D>()
        .ok()
        .map(|x| x.get_global_transform())
}
fn moved(n: &Gd<Node>, root: &Gd<Node>) -> Option<Moved> {
    let from = n.get_parent()?;
    let n3 = n.clone().try_cast::<Node3D>().ok();
    Some(Moved {
        node: n.clone(),
        from,
        index: n.get_index(),
        name: n.get_name(),
        local: n3.as_ref().map(|x| x.get_transform()),
        global: n3.as_ref().map(|x| x.get_global_transform()),
        owned: owned_subtree(n, root),
    })
}
fn bounds_points(nodes: &[Gd<Node>]) -> Vec<Vector3> {
    let mut pts = Vec::new();
    for n in nodes {
        let Ok(n3) = n.clone().try_cast::<Node3D>() else {
            continue;
        };
        let xf = n3.get_global_transform();
        pts.push(xf.origin);
        if let Ok(v) = n.clone().try_cast::<VisualInstance3D>() {
            let a = v.get_aabb();
            pts.extend(box_corners(a.position, a.position + a.size).map(|c| xf * c));
        }
    }
    pts
}
pub(super) fn box_corners(lo: Vector3, hi: Vector3) -> [Vector3; 8] {
    std::array::from_fn(|i| {
        Vector3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    })
}
pub(super) fn group_pivot(points: &[Vector3]) -> Option<Vector3> {
    let first = *points.first()?;
    let (lo, hi) = points
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.coord_min(*p), hi.coord_max(*p)));
    Some((lo + hi) * 0.5)
}
pub(super) fn local_under(new_parent_global: Transform3D, global: Transform3D) -> Transform3D {
    new_parent_global.affine_inverse() * global
}
pub(super) fn group_slot(first_index: i32, moved_sibling_indices: &[i32]) -> i32 {
    first_index - moved_sibling_indices.iter().filter(|i| **i < first_index).count() as i32
}
pub(super) fn released_slot(group_index: i32, child_order: usize) -> i32 {
    group_index + 1 + child_order as i32
}
pub(super) fn select_only(nodes: &[Gd<Node>]) {
    if let Some(mut sel) = EditorInterface::singleton().get_selection() {
        sel.clear();
        for n in nodes {
            sel.add_node(n);
        }
    }
}
fn restore_owners(ur: &mut Gd<EditorUndoRedoManager>, m: &Moved, root: &Gd<Node>, undo: bool) {
    for o in &m.owned {
        if undo {
            ur.add_undo_method(o, "set_owner", &[root.to_variant()]);
        } else {
            ur.add_do_method(o, "set_owner", &[root.to_variant()]);
        }
    }
}
impl PixelBuilderPlugin {
    pub(super) fn group_selection_as(&mut self, kind: GroupKind) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let picked: Vec<Gd<Node>> = top_selection()
            .into_iter()
            .filter(|n| n.instance_id() != root.instance_id() && owned_by(n, &root))
            .collect();
        let Some(first) = picked.first().cloned() else {
            self.message = format!(
                "Group as {}: select the parts to group",
                kind.node_name()
            );
            return;
        };
        let Some(parent) = first.get_parent() else {
            return;
        };
        let mut plan: Vec<Moved> = picked.iter().filter_map(|n| moved(n, &root)).collect();
        plan.sort_by_key(|m| (m.from.instance_id().to_i64(), m.index));
        let centre = group_pivot(&bounds_points(&picked)).unwrap_or(Vector3::ZERO);
        let group_global = Transform3D::new(Basis::IDENTITY, centre);
        let parent_global = global_of(&parent).unwrap_or(Transform3D::IDENTITY);
        let siblings: Vec<i32> = plan
            .iter()
            .filter(|m| m.from.instance_id() == parent.instance_id())
            .map(|m| m.index)
            .collect();
        let slot = group_slot(first.get_index(), &siblings);
        let mut group = Node3D::new_alloc();
        group.set_name(kind.node_name());
        group.set_transform(local_under(parent_global, group_global));
        let Some(mut ur) = self.open_action(&format!("Pixel Builder: Group as {}", kind.node_name()))
        else {
            group.free();
            return;
        };
        for m in &plan {
            ur.add_do_method(&m.from, "remove_child", &[m.node.to_variant()]);
        }
        ur.add_do_method(&parent, "add_child", &[group.to_variant(), true.to_variant()]);
        ur.add_do_method(&parent, "move_child", &[group.to_variant(), slot.to_variant()]);
        ur.add_do_method(&group, "set_owner", &[root.to_variant()]);
        for m in &plan {
            ur.add_do_method(&group, "add_child", &[m.node.to_variant(), true.to_variant()]);
            if let Some(g) = m.global {
                let local = local_under(group_global, g);
                ur.add_do_method(&m.node, "set_transform", &[local.to_variant()]);
            }
            restore_owners(&mut ur, m, &root, false);
        }
        ur.add_do_reference(&group);
        ur.add_undo_method(&parent, "remove_child", &[group.to_variant()]);
        for m in &plan {
            ur.add_undo_method(&group, "remove_child", &[m.node.to_variant()]);
            ur.add_undo_method(&m.node, "set_name", &[m.name.to_variant()]);
            ur.add_undo_method(&m.from, "add_child", &[m.node.to_variant(), false.to_variant()]);
            ur.add_undo_method(&m.from, "move_child", &[m.node.to_variant(), m.index.to_variant()]);
            if let Some(l) = m.local {
                ur.add_undo_method(&m.node, "set_transform", &[l.to_variant()]);
            }
            restore_owners(&mut ur, m, &root, true);
        }
        ur.commit_action();
        select_only(&[group.clone().upcast()]);
        self.message = format!(
            "Grouped {} part{} as {}",
            plan.len(),
            if plan.len() == 1 { "" } else { "s" },
            group.get_name()
        );
    }
    pub(super) fn ungroup_selection(&mut self) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let mut groups: Vec<Gd<Node>> = top_selection()
            .into_iter()
            .filter(|n| {
                n.instance_id() != root.instance_id() && owned_by(n, &root) && is_container(n)
            })
            .collect();
        if groups.is_empty() {
            self.message = "Ungroup: select a Model or Folder".into();
            return;
        }
        groups.sort_by_key(|g| std::cmp::Reverse(g.get_index()));
        let Some(mut ur) = self.open_action("Pixel Builder: Ungroup") else {
            return;
        };
        let mut released: Vec<Gd<Node>> = Vec::new();
        let mut undo_plans: Vec<(Gd<Node>, Gd<Node>, i32, Vec<Moved>)> = Vec::new();
        for g in &groups {
            let Some(parent) = g.get_parent() else {
                continue;
            };
            let parent_global = global_of(&parent).unwrap_or(Transform3D::IDENTITY);
            let at = g.get_index();
            let kids: Vec<Moved> = g
                .get_children()
                .iter_shared()
                .filter(|c| owned_by(c, &root))
                .filter_map(|c| moved(&c, &root))
                .collect();
            for (k, m) in kids.iter().enumerate() {
                ur.add_do_method(g, "remove_child", &[m.node.to_variant()]);
                ur.add_do_method(&parent, "add_child", &[m.node.to_variant(), true.to_variant()]);
                ur.add_do_method(
                    &parent,
                    "move_child",
                    &[m.node.to_variant(), released_slot(at, k).to_variant()],
                );
                if let Some(gl) = m.global {
                    let local = local_under(parent_global, gl);
                    ur.add_do_method(&m.node, "set_transform", &[local.to_variant()]);
                }
                restore_owners(&mut ur, m, &root, false);
                released.push(m.node.clone());
            }
            ur.add_do_method(&parent, "remove_child", &[g.to_variant()]);
            ur.add_undo_reference(g);
            undo_plans.push((g.clone(), parent, at, kids));
        }
        for (g, parent, at, kids) in undo_plans.iter().rev() {
            for m in kids {
                ur.add_undo_method(parent, "remove_child", &[m.node.to_variant()]);
            }
            ur.add_undo_method(parent, "add_child", &[g.to_variant(), false.to_variant()]);
            ur.add_undo_method(parent, "move_child", &[g.to_variant(), at.to_variant()]);
            ur.add_undo_method(g, "set_owner", &[root.to_variant()]);
            for m in kids {
                ur.add_undo_method(&m.node, "set_name", &[m.name.to_variant()]);
                ur.add_undo_method(g, "add_child", &[m.node.to_variant(), false.to_variant()]);
                ur.add_undo_method(g, "move_child", &[m.node.to_variant(), m.index.to_variant()]);
                if let Some(l) = m.local {
                    ur.add_undo_method(&m.node, "set_transform", &[l.to_variant()]);
                }
                restore_owners(&mut ur, m, &root, true);
            }
        }
        ur.commit_action();
        select_only(&released);
        self.message = format!(
            "Ungrouped {} group{}",
            groups.len(),
            if groups.len() == 1 { "" } else { "s" }
        );
    }
    pub(super) fn select_children_of_selection(&mut self) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let kids: Vec<Gd<Node>> = top_selection()
            .iter()
            .flat_map(|n| n.get_children().iter_shared().collect::<Vec<_>>())
            .filter(|c| owned_by(c, &root))
            .collect();
        if kids.is_empty() {
            self.message = "Select children: the selection has no children".into();
            return;
        }
        select_only(&kids);
        self.message = format!("Selected {} child node{}", kids.len(), if kids.len() == 1 { "" } else { "s" });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: Transform3D, b: Transform3D) -> bool {
        (a.origin - b.origin).length() < 1e-4
            && (a.basis.col_a() - b.basis.col_a()).length() < 1e-4
            && (a.basis.col_b() - b.basis.col_b()).length() < 1e-4
            && (a.basis.col_c() - b.basis.col_c()).length() < 1e-4
    }
    #[test]
    fn the_pivot_is_the_centre_of_the_bounds() {
        let pts = [
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(4.0, 1.0, 2.0),
            Vector3::new(1.0, 3.0, -2.0),
        ];
        assert_eq!(group_pivot(&pts), Some(Vector3::new(2.0, 1.5, 0.0)));
        assert_eq!(group_pivot(&[]), None);
    }
    #[test]
    fn moving_under_a_new_parent_keeps_the_world_transform() {
        let parent = Transform3D::new(
            Basis::from_axis_angle(Vector3::UP, 0.7).scaled(Vector3::new(2.0, 2.0, 2.0)),
            Vector3::new(3.0, -1.0, 5.0),
        );
        let part = Transform3D::new(
            Basis::from_axis_angle(Vector3::RIGHT, -0.4),
            Vector3::new(-2.0, 4.0, 1.5),
        );
        let group = Transform3D::new(Basis::IDENTITY, Vector3::new(1.0, 2.0, 3.0));
        let group_local = local_under(parent, group);
        assert!(close(parent * group_local, group));
        let part_local = local_under(parent * group_local, part);
        assert!(close(parent * group_local * part_local, part));
    }
    #[test]
    fn the_group_takes_the_first_part_slot() {
        assert_eq!(group_slot(3, &[3, 5]), 3);
        assert_eq!(group_slot(4, &[1, 4, 6]), 3);
        assert_eq!(group_slot(0, &[0]), 0);
    }
    #[test]
    fn released_children_follow_the_group_slot_in_order() {
        let slots: Vec<i32> = (0..3).map(|k| released_slot(2, k)).collect();
        assert_eq!(slots, vec![3, 4, 5]);
    }
}
