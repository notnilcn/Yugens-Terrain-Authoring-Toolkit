use super::part_handle_rules::{self as rules, ray_box};
use super::roblox_gizmo::user_locked;
use super::roblox_rules::{self as core, ClickMode};
use super::*;
impl PixelBuilderPlugin {
    pub(super) fn handles_requirement(&self, a: Action) -> Option<&'static str> {
        match a {
            Action::ToggleLock if self.lock_candidates().is_empty() => {
                Some("Select a node to lock or unlock")
            }
            _ => None,
        }
    }
    pub(super) fn handles_pressed(&self, a: Action) -> Option<bool> {
        match a {
            Action::ToggleLock => {
                let flags: Vec<bool> = self.lock_candidates().iter().map(user_locked).collect();
                Some(core::every_one_on(&flags))
            }
            Action::ClickToLock => Some(self.rbx.click_lock),
            Action::PivotSnap => Some(self.rbx.pivot_snap),
            Action::Collisions => Some(self.prefs.collisions),
            Action::SnapBoundingBoxes => Some(self.rbx.snap_boxes),
            Action::GridPlane => Some(!self.rbx.grid_plane_hidden),
            Action::GizmoTransform => {
                Some(self.tool == Tool::Select && self.xform_mode == XformMode::Combined)
            }
            Action::FaceResize => {
                Some(self.tool == Tool::Select && self.xform_mode == XformMode::Scale)
            }
            _ => None,
        }
    }
    pub(super) fn act_gizmo_transform(&mut self) {
        self.set_xform(XformMode::Combined);
    }
    pub(super) fn act_face_resize(&mut self) {
        self.set_xform(XformMode::Scale);
        self.message = "Scale: drag a ball to move that face; Ctrl grows both sides".into();
    }
    pub(super) fn act_edit_pivot(&mut self) {
        let (next, mode) = core::edit_pivot_toggle(self.tool, self.mode);
        if let Some(m) = mode {
            self.assign_setting(m);
        }
        self.assign_instrument(next);
        self.sync_gizmo_lock();
    }
    pub(super) fn act_pivot_snap(&mut self) {
        self.rbx.pivot_snap = !self.rbx.pivot_snap;
        self.message = if self.rbx.pivot_snap {
            "Pivot snap: on (the pivot lands on the move grid)".into()
        } else {
            "Pivot snap: off".into()
        };
    }
    pub(super) fn act_collisions(&mut self) {
        let on = !self.prefs.collisions;
        self.prefs.assign(prefs::Key::Collisions, &on.to_variant());
        self.prefs.save_key(prefs::Key::Collisions);
        self.message = if on {
            "Collisions: on (a dragged part stops at the first overlap)".into()
        } else {
            "Collisions: off".into()
        };
    }
    pub(super) fn act_snap_boxes(&mut self) {
        self.rbx.snap_boxes = !self.rbx.snap_boxes;
        self.message = if self.rbx.snap_boxes {
            "Use Bounding Boxes: on (a moved part lands with its box edge on the grid)".into()
        } else {
            "Use Bounding Boxes: off".into()
        };
    }
    pub(super) fn act_grid_plane(&mut self) {
        self.rbx.grid_plane_hidden = !self.rbx.grid_plane_hidden;
        self.base().update_overlays();
    }
    pub(super) fn act_click_to_lock(&mut self) {
        self.toggle_click_mode(ClickMode::Lock);
        self.rbx.hover = None;
        self.message = if self.rbx.click_lock {
            "Click to lock: click a part to lock or unlock it (click the menu item again to stop)"
                .into()
        } else {
            String::new()
        };
    }
    fn lock_candidates(&self) -> Vec<Gd<Node>> {
        let mut out: Vec<Gd<Node>> = EditorInterface::singleton()
            .get_selection()
            .map(|s| {
                s.get_top_selected_nodes()
                    .iter_shared()
                    .filter(|n| n.is_class("Node3D"))
                    .collect()
            })
            .unwrap_or_default();
        if out.is_empty()
            && let Some(t) = self.target.as_ref().filter(|t| t.is_instance_valid())
        {
            out.push(t.clone().upcast::<Node>());
        }
        out
    }
    pub(super) fn act_toggle_lock(&mut self) {
        let nodes = self.lock_candidates();
        if nodes.is_empty() {
            self.message = "Lock: select a node first".into();
            return;
        }
        let flags: Vec<bool> = nodes.iter().map(user_locked).collect();
        let lock = rules::lock_wanted(&flags);
        self.write_locks(&nodes, lock);
    }
    fn write_locks(&mut self, nodes: &[Gd<Node>], lock: bool) {
        let name = if lock {
            "Pixel Builder: Lock"
        } else {
            "Pixel Builder: Unlock"
        };
        let Some(mut ur) = self.open_action(name) else {
            return;
        };
        let yes = true.to_variant();
        for n in nodes {
            let had_edit = n.has_meta(EDIT_LOCK_META);
            let had_plugin = n.has_meta(PLUGIN_LOCK_META);
            if lock {
                ur.add_do_method(n, "set_meta", &[EDIT_LOCK_META.to_variant(), yes.clone()]);
                ur.add_do_method(n, "remove_meta", &[PLUGIN_LOCK_META.to_variant()]);
            } else {
                ur.add_do_method(n, "remove_meta", &[EDIT_LOCK_META.to_variant()]);
                ur.add_do_method(n, "remove_meta", &[PLUGIN_LOCK_META.to_variant()]);
            }
            for (meta, had) in [(EDIT_LOCK_META, had_edit), (PLUGIN_LOCK_META, had_plugin)] {
                if had {
                    ur.add_undo_method(n, "set_meta", &[meta.to_variant(), yes.clone()]);
                } else {
                    ur.add_undo_method(n, "remove_meta", &[meta.to_variant()]);
                }
            }
        }
        let me = self.to_gd();
        ur.add_do_method(&me, "on_roblox_lock_changed", &[]);
        ur.add_undo_method(&me, "on_roblox_lock_changed", &[]);
        ur.commit_action_ex().execute(false).done();
        for n in nodes {
            let mut n = n.clone();
            if lock {
                n.set_meta(EDIT_LOCK_META, &yes);
            } else {
                n.remove_meta(EDIT_LOCK_META);
            }
            n.remove_meta(PLUGIN_LOCK_META);
        }
        self.lock_changed();
        self.message = format!(
            "{} {} node{}",
            if lock { "Locked" } else { "Unlocked" },
            nodes.len(),
            if nodes.len() == 1 { "" } else { "s" }
        );
    }
    pub(super) fn click_lock_at(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let mut best: Option<(f32, Gd<Node>)> = None;
        for n in root
            .find_children_ex("*")
            .type_("MeshInstance3D")
            .owned(false)
            .done()
            .iter_shared()
        {
            let Ok(mi) = n.clone().try_cast::<MeshInstance3D>() else {
                continue;
            };
            if !mi.is_visible_in_tree() || under_terrain(&n) {
                continue;
            }
            let xf = mi.get_global_transform();
            let inv = xf.affine_inverse();
            let (lo_, ld) = (inv * o, inv.basis * d);
            let hit = match mi.clone().try_cast::<PixelBuilderMesh>() {
                Ok(pb) => pb.bind().contents().and_then(|c| {
                    pick::intersect_ray_closest(c.bind().cage(), to_p(lo_), to_p(ld), true)
                        .map(|h| (xf * from_p(h.dot)).distance_to(o))
                }),
                Err(_) => {
                    let a = mi.get_aabb();
                    ray_box(lo_, ld, a.position, a.position + a.size)
                        .map(|t| (xf * (lo_ + ld * t)).distance_to(o))
                }
            };
            if let Some(dist) = hit
                && best.as_ref().is_none_or(|b| dist < b.0)
            {
                best = Some((dist, n));
            }
        }
        match best {
            Some((_, n)) => {
                let lock = !user_locked(&n);
                self.write_locks(&[n], lock);
            }
            None => self.message = "Click to lock: no part under the cursor".into(),
        }
        self.sync_ui();
    }
    pub(super) fn enter_roblox_move_step(&mut self) {
        if let Some(s) = core::entering_move_step(self.rbx.other_step, self.rbx.move_step, self.grid_step) {
            self.rbx.other_step = Some(s.stashed_other);
            self.grid_step = s.grid;
        }
    }
    pub(super) fn leave_roblox_move_step(&mut self) {
        if let Some(r) = core::leaving_move_step(self.rbx.other_step.take(), self.grid_step) {
            self.rbx.move_step = Some(r.last_roblox);
            self.grid_step = r.grid;
        }
    }
}
fn under_terrain(n: &Gd<Node>) -> bool {
    let mut p = n.get_parent();
    while let Some(x) = p {
        if x.is_class("MarchingSquaresTerrain") {
            return true;
        }
        p = x.get_parent();
    }
    false
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_roblox_lock_changed(&mut self) {
        self.lock_changed();
    }
    #[func]
    fn debug_roblox_handles(&self) -> VarDictionary {
        self.roblox_handles_debug()
    }
}
impl PixelBuilderPlugin {
    fn lock_changed(&mut self) {
        if let Some(t) = self.target.clone().filter(|t| t.is_instance_valid()) {
            if t.has_meta(PLUGIN_LOCK_META) {
                self.lock_target = Some(t);
            } else if self
                .lock_target
                .as_ref()
                .is_some_and(|l| l.instance_id() == t.instance_id())
            {
                self.lock_target = None;
            }
        }
        self.sync_gizmo_lock();
        self.sync_ui();
        self.base().update_overlays();
    }
}
