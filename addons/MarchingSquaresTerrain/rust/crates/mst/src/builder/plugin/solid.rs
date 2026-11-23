//! Roblox-style solid modelling: parts are marked as cutters, any number of selected parts
//! are combined into one solid, and a combined solid can be split back into its parts.
//!
//! The combined solid is a new builder mesh created in code, because it is the result of
//! an editing command on parts chosen at that moment, and the scene author keeps it. The
//! parts it was made from become its hidden children instead of being deleted, so the
//! split can give them back exactly as they were. Every change goes through the editor's
//! undo history as one step and is owned by the edited scene.
use super::super::data::PixelBuilderMeshData;
use super::super::solid_role::{self, NEGATE_META, OPERAND_META};
use super::super::solid_rules::{
    self as rules, Candidate, UndoMove, NEED_COMBINED, NEED_PART, NEED_WATERTIGHT, SolidOp, Step,
    plan_solid,
};
use super::*;
use godot::classes::Material;
use godot::classes::csg_shape_3d::Operation;
const KEPT_FROM_BASE: &[&str] = &["collision", "impact_stratum"];
fn engine_operation(s: Step) -> Operation {
    match s {
        Step::Union => Operation::UNION,
        Step::Intersect => Operation::INTERSECTION,
        Step::Subtract => Operation::SUBTRACTION,
    }
}
pub struct SolidJob {
    name: &'static str,
    base: Gd<PixelBuilderMesh>,
    operands: Vec<Gd<PixelBuilderMesh>>,
}
impl SolidJob {
    pub(super) fn is_valid(&self) -> bool {
        self.base.is_instance_valid() && self.operands.iter().all(|o| o.is_instance_valid())
    }
}
fn negated(m: &Gd<PixelBuilderMesh>) -> bool {
    solid_role::is_negated(&m.clone().upcast())
}
fn operand_children(n: &Gd<Node>) -> Vec<Gd<PixelBuilderMesh>> {
    n.get_children()
        .iter_shared()
        .filter(solid_role::is_operand)
        .filter_map(|c| c.try_cast::<PixelBuilderMesh>().ok())
        .collect()
}
fn local_frame_of(parent: &Gd<Node>) -> Transform3D {
    parent
        .clone()
        .try_cast::<Node3D>()
        .map(|n| n.get_global_transform())
        .unwrap_or(Transform3D::IDENTITY)
}
impl PixelBuilderPlugin {
    fn selected_parts(&self) -> Vec<Gd<PixelBuilderMesh>> {
        let ei = EditorInterface::singleton();
        let (Some(sel), Some(root)) = (ei.get_selection(), ei.get_edited_scene_root()) else {
            return Vec::new();
        };
        sel.get_top_selected_nodes()
            .iter_shared()
            .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
            .filter(|m| {
                let n: Gd<Node> = m.clone().upcast();
                rules::is_selectable_part(Candidate {
                    role: solid_role::role_of(&n),
                    is_scene_root: n.instance_id() == root.instance_id(),
                    owned_by_scene: n
                        .get_owner()
                        .is_some_and(|o| o.instance_id() == root.instance_id()),
                    has_shape: m.bind().contents().is_some(),
                })
            })
            .collect()
    }
    fn selected_combined(&self) -> Vec<Gd<PixelBuilderMesh>> {
        self.selected_parts()
            .into_iter()
            .filter(|p| !operand_children(&p.clone().upcast()).is_empty())
            .collect()
    }
    fn combine_reason(&self, op: SolidOp) -> Option<&'static str> {
        let parts = self.selected_parts();
        let flags: Vec<bool> = parts.iter().map(negated).collect();
        if let Err(e) = plan_solid(op, &flags) {
            return Some(e);
        }
        boolean::first_open_mesh(&parts).map(|_| NEED_WATERTIGHT)
    }
    pub(super) fn solid_requirement(&self, a: Action) -> Option<&'static str> {
        match a {
            Action::Negate => self.selected_parts().is_empty().then_some(NEED_PART),
            Action::RobloxUnion => self.combine_reason(SolidOp::Union),
            Action::RobloxIntersect => self.combine_reason(SolidOp::Intersect),
            Action::Separate => self.selected_combined().is_empty().then_some(NEED_COMBINED),
            _ => None,
        }
    }
    pub(super) fn act_negate(&mut self) {
        let parts = self.selected_parts();
        if parts.is_empty() {
            self.message = format!("Negate: {NEED_PART}");
            return;
        }
        let flags: Vec<bool> = parts.iter().map(negated).collect();
        let negate = rules::negate_turns_on(&flags);
        let title = if negate { "Negate" } else { "Un-negate" };
        let Some(mut ur) = self.open_action(&format!("Pixel Builder: {title}")) else {
            return;
        };
        for p in &parts {
            let was = negated(p);
            for (doing, on) in [(true, negate), (false, was)] {
                let (method, args) = if on {
                    ("set_meta", vec![NEGATE_META.to_variant(), true.to_variant()])
                } else {
                    ("remove_meta", vec![NEGATE_META.to_variant()])
                };
                if doing {
                    ur.add_do_method(p, method, &args);
                    ur.add_do_method(p, "refresh_solid_role", &[]);
                } else {
                    ur.add_undo_method(p, method, &args);
                    ur.add_undo_method(p, "refresh_solid_role", &[]);
                }
            }
        }
        ur.commit_action();
        self.message = format!(
            "{title}: {} part{}",
            parts.len(),
            if parts.len() == 1 { "" } else { "s" }
        );
    }
    pub(super) fn act_roblox_union(&mut self) {
        self.act_solid(SolidOp::Union);
    }
    pub(super) fn act_roblox_intersect(&mut self) {
        self.act_solid(SolidOp::Intersect);
    }
    fn act_solid(&mut self, op: SolidOp) {
        let name = op.result_name();
        let parts = self.selected_parts();
        let flags: Vec<bool> = parts.iter().map(negated).collect();
        let plan = match plan_solid(op, &flags) {
            Ok(p) => p,
            Err(e) => {
                self.message = format!("{name}: {e}");
                return;
            }
        };
        if let Some(open) = boolean::first_open_mesh(&parts) {
            self.message = format!(
                "{name}: {} is not watertight (Select holes shows the open edges)",
                open.get_name()
            );
            return;
        }
        self.set_gizmo_lock(None);
        let base = parts[plan.base].clone();
        let steps = plan
            .steps
            .iter()
            .map(|(i, s)| (parts[*i].clone(), engine_operation(*s)))
            .collect();
        let job = SolidJob {
            name,
            base: base.clone(),
            operands: parts,
        };
        self.start_csg(base, steps, name, Some(job));
    }
    pub(super) fn commit_solid(
        &mut self,
        job: SolidJob,
        m: BMesh,
        materials: Array<Option<Gd<Material>>>,
    ) {
        let ei = EditorInterface::singleton();
        let (Some(parent), Some(root)) = (job.base.get_parent(), ei.get_edited_scene_root())
        else {
            return;
        };
        let index = job.base.get_index();
        let mut data = PixelBuilderMeshData::new_gd();
        data.bind_mut().substitute(m);
        data.set("materials", &materials.to_variant());
        let mut result = PixelBuilderMesh::new_alloc();
        result.bind_mut().set_data(Some(data));
        result.set_name(job.name);
        for kept in KEPT_FROM_BASE {
            result.set(*kept, &job.base.get(*kept));
        }
        let base_global = job.base.get_global_transform();
        result.set_transform(local_frame_of(&parent).affine_inverse() * base_global);
        let Some(mut ur) = self.open_action(&format!("Pixel Builder: {}", job.name)) else {
            result.free();
            return;
        };
        ur.add_do_method(
            &parent,
            "add_child",
            &[result.to_variant(), true.to_variant()],
        );
        ur.add_do_method(&parent, "move_child", &[result.to_variant(), index.to_variant()]);
        ur.add_do_method(&result, "set_owner", &[root.to_variant()]);
        let inv = base_global.affine_inverse();
        for o in &job.operands {
            ur.add_do_method(o, "reparent", &[result.to_variant(), true.to_variant()]);
            ur.add_do_method(
                o,
                "set_transform",
                &[(inv * o.get_global_transform()).to_variant()],
            );
            ur.add_do_method(o, "set_visible", &[false.to_variant()]);
            ur.add_do_method(o, "set_meta", &[OPERAND_META.to_variant(), true.to_variant()]);
            ur.add_do_method(o, "refresh_solid_role", &[]);
        }
        ur.add_do_reference(&result);
        let homes: Vec<(Gd<PixelBuilderMesh>, Gd<Node>, i32)> = job
            .operands
            .iter()
            .filter_map(|o| o.get_parent().map(|p| (o.clone(), p, o.get_index())))
            .collect();
        let keys: Vec<(i64, i32)> = homes
            .iter()
            .map(|(_, p, i)| (p.instance_id().to_i64(), *i))
            .collect();
        for m in rules::combine_undo_plan(&keys) {
            match m {
                UndoMove::ParkResultLast => {
                    ur.add_undo_method(
                        &parent,
                        "move_child",
                        &[result.to_variant(), (-1).to_variant()],
                    );
                }
                UndoMove::Reinsert { part, index } => {
                    let (o, p) = (&homes[part].0, &homes[part].1);
                    ur.add_undo_method(o, "remove_meta", &[OPERAND_META.to_variant()]);
                    ur.add_undo_method(o, "set_visible", &[o.is_visible().to_variant()]);
                    ur.add_undo_method(o, "reparent", &[p.to_variant(), true.to_variant()]);
                    ur.add_undo_method(p, "move_child", &[o.to_variant(), index.to_variant()]);
                    ur.add_undo_method(o, "set_transform", &[o.get_transform().to_variant()]);
                    ur.add_undo_method(o, "refresh_solid_role", &[]);
                }
                UndoMove::RemoveResult => {
                    ur.add_undo_method(&parent, "remove_child", &[result.to_variant()]);
                }
            }
        }
        ur.commit_action();
        self.after_solid_change(Some(result.upcast()));
        self.message = format!("{} done", job.name);
    }
    pub(super) fn act_separate(&mut self) {
        let combined = self.selected_combined();
        if combined.is_empty() {
            self.message = format!("Separate: {NEED_COMBINED}");
            return;
        }
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        self.set_gizmo_lock(None);
        let Some(mut ur) = self.open_action("Pixel Builder: Separate") else {
            return;
        };
        let mut restored = Vec::new();
        for c in &combined {
            let Some(parent) = c.get_parent() else {
                continue;
            };
            let index = c.get_index();
            let parts = operand_children(&c.clone().upcast());
            let frame = local_frame_of(&parent).affine_inverse() * c.get_global_transform();
            let slots = rules::separated_indices(index, parts.len());
            for (o, slot) in parts.iter().zip(slots) {
                ur.add_do_method(o, "remove_meta", &[OPERAND_META.to_variant()]);
                ur.add_do_method(o, "set_visible", &[true.to_variant()]);
                ur.add_do_method(o, "reparent", &[parent.to_variant(), true.to_variant()]);
                ur.add_do_method(
                    &parent,
                    "move_child",
                    &[o.to_variant(), slot.to_variant()],
                );
                ur.add_do_method(
                    o,
                    "set_transform",
                    &[(frame * o.get_transform()).to_variant()],
                );
                ur.add_do_method(o, "refresh_solid_role", &[]);
                restored.push(o.clone().upcast::<Node>());
            }
            ur.add_do_method(&parent, "remove_child", &[c.to_variant()]);
            ur.add_undo_method(&parent, "add_child", &[c.to_variant(), true.to_variant()]);
            ur.add_undo_method(&parent, "move_child", &[c.to_variant(), index.to_variant()]);
            ur.add_undo_method(c, "set_owner", &[root.to_variant()]);
            for o in &parts {
                ur.add_undo_method(o, "reparent", &[c.to_variant(), true.to_variant()]);
                ur.add_undo_method(o, "set_transform", &[o.get_transform().to_variant()]);
                ur.add_undo_method(o, "set_visible", &[false.to_variant()]);
                ur.add_undo_method(o, "set_meta", &[OPERAND_META.to_variant(), true.to_variant()]);
                ur.add_undo_method(o, "refresh_solid_role", &[]);
            }
            ur.add_undo_reference(c);
        }
        ur.commit_action();
        self.after_solid_change(restored.first().cloned());
        self.message = format!(
            "Separate: {} part{} restored",
            restored.len(),
            if restored.len() == 1 { "" } else { "s" }
        );
    }
    fn after_solid_change(&mut self, edit: Option<Gd<Node>>) {
        self.purge_picked();
        self.palette_slots = None;
        if let Some(n) = edit {
            EditorInterface::singleton().call_deferred("edit_node", &[n.to_variant()]);
        }
        self.sync_ui();
        self.redraw();
    }
}
