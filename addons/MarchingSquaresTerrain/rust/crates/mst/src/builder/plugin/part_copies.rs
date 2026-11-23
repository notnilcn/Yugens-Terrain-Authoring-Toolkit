//! Copies, removes and frames whole parts in the scene being edited, the way a level author
//! duplicates or deletes parts from the viewport's right-click menu.
//!
//! The copies are created in code because which parts are copied, and how many, is the
//! author's choice at that moment, and the author keeps them. Each copy owns its own shape
//! data, so editing one part never changes its twin. Each change goes through the
//! editor's undo history as one step and is owned by the edited scene so it is saved with it.
use super::grouping::{owned_by, owned_subtree, select_only, top_selection};
use super::*;
fn scene_parts(root: &Gd<Node>) -> Vec<Gd<Node>> {
    top_selection()
        .into_iter()
        .filter(|n| n.instance_id() != root.instance_id() && owned_by(n, root))
        .collect()
}
fn own_mesh_data(mut part: Gd<PixelBuilderMesh>) {
    let Some(data) = part.bind().contents() else {
        return;
    };
    let own = data.duplicate_resource();
    part.bind_mut().set_data(Some(own));
}
impl PixelBuilderPlugin {
    pub(super) fn act_duplicate_object(&mut self) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let parts = scene_parts(&root);
        if parts.is_empty() {
            self.message = "Duplicate: select the parts to copy".into();
            return;
        }
        let mut copies: Vec<(Gd<Node>, Gd<Node>, Gd<Node>, i32, Vec<Gd<Node>>)> = Vec::new();
        for n in &parts {
            let Some(parent) = n.get_parent() else {
                continue;
            };
            let copy = n.duplicate_node();
            let owned: Vec<Gd<Node>> = owned_subtree(n, &root)
                .iter()
                .filter_map(|o| {
                    if o.instance_id() == n.instance_id() {
                        Some(copy.clone())
                    } else {
                        copy.get_node_or_null(&n.get_path_to(o))
                    }
                })
                .collect();
            for part in owned.iter().filter_map(|o| o.clone().try_cast::<PixelBuilderMesh>().ok()) {
                own_mesh_data(part);
            }
            copies.push((parent, copy, n.clone(), n.get_index() + 1, owned));
        }
        let Some(mut ur) = self.open_action("Pixel Builder: Duplicate") else {
            for (_, c, ..) in copies {
                c.free();
            }
            return;
        };
        for (parent, copy, _, at, owned) in &copies {
            ur.add_do_method(parent, "add_child", &[copy.to_variant(), true.to_variant()]);
            ur.add_do_method(parent, "move_child", &[copy.to_variant(), at.to_variant()]);
            for o in owned {
                ur.add_do_method(o, "set_owner", &[root.to_variant()]);
            }
            ur.add_do_reference(copy);
            ur.add_undo_method(parent, "remove_child", &[copy.to_variant()]);
        }
        ur.commit_action();
        let made: Vec<Gd<Node>> = copies.iter().map(|c| c.1.clone()).collect();
        select_only(&made);
        self.message = format!(
            "Duplicated {} part{}",
            made.len(),
            if made.len() == 1 { "" } else { "s" }
        );
    }
    pub(super) fn act_delete_object(&mut self) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let parts = scene_parts(&root);
        if parts.is_empty() {
            self.message = "Delete: select the parts to remove".into();
            return;
        }
        let plan: Vec<(Gd<Node>, Gd<Node>, i32, Vec<Gd<Node>>)> = parts
            .iter()
            .filter_map(|n| {
                let parent = n.get_parent()?;
                Some((parent, n.clone(), n.get_index(), owned_subtree(n, &root)))
            })
            .collect();
        select_only(&[]);
        let Some(mut ur) = self.open_action("Pixel Builder: Delete") else {
            return;
        };
        for (parent, n, ..) in &plan {
            ur.add_do_method(parent, "remove_child", &[n.to_variant()]);
        }
        for (parent, n, at, owned) in plan.iter().rev() {
            ur.add_undo_method(parent, "add_child", &[n.to_variant(), false.to_variant()]);
            ur.add_undo_method(parent, "move_child", &[n.to_variant(), at.to_variant()]);
            for o in owned {
                ur.add_undo_method(o, "set_owner", &[root.to_variant()]);
            }
            ur.add_undo_reference(n);
        }
        ur.commit_action();
        self.message = format!(
            "Deleted {} part{}",
            plan.len(),
            if plan.len() == 1 { "" } else { "s" }
        );
    }
    pub(super) fn act_zoom_to_selection(&mut self) {
        let idx = self
            .camera
            .as_ref()
            .filter(|c| c.is_instance_valid())
            .and_then(super::super::modeler_ui::hud::viewport_of)
            .unwrap_or(0);
        let host = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(idx)
            .done()
            .and_then(|vp| vp.get_parent())
            .and_then(|c| c.get_parent());
        if !host
            .as_ref()
            .is_some_and(crate::editor::view_commands::focus_selection)
        {
            self.message = "Zoom to: the 3D view has no Focus Selection shortcut".into();
        }
    }
}
