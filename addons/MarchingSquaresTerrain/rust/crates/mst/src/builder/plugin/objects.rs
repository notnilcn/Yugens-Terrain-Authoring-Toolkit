use super::super::data::{from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use godot::builtin::math::ApproxEq;
use godot::classes::{Camera3D, Control, EditorInterface, InputEvent, Node3D};
use godot::prelude::*;
use pt_core::build::edit_ops::{self, Affine};
use pt_core::build::{ops, pick};
use pt_core::math::Vec3 as PVec3;
use std::collections::BTreeSet;
const AXES_WORLD: [Vector3; 3] = [Vector3::RIGHT, Vector3::UP, Vector3::BACK];
pub(super) fn affine(xf: Transform3D) -> Affine {
    Affine {
        x: to_p(xf.basis.col_a()),
        y: to_p(xf.basis.col_b()),
        z: to_p(xf.basis.col_c()),
        origin: to_p(xf.origin),
    }
}
impl PixelBuilderPlugin {
    pub(super) fn perform_refine_ridges(&mut self) {
        if self.mode != Mode::Edge || self.sel_edges.is_empty() {
            self.message = "Subdivide edges: select edges (Edge mode)".into();
            return;
        }
        let segments = self.cuts.clamp(1, 16) as u32 + 1;
        self.apply_edit("Pixel Builder: Subdivide edges", |m, s| {
            edit_ops::divide_edges(m, &s.sel_edges, segments);
            s.sel_edges.clear();
        });
    }
    pub(super) fn perform_join_ridges(&mut self) {
        if self.mode != Mode::Edge || self.sel_edges.len() < 2 {
            self.message =
                "Connect edges: select two or more edges of the same faces (Edge mode)".into();
            return;
        }
        self.apply_edit("Pixel Builder: Connect edges", |m, s| {
            let chords = edit_ops::join_ridges(m, &s.sel_edges);
            if chords.is_empty() {
                s.message = "Connect edges: no face has exactly two of the selected edges".into();
            } else {
                s.sel_edges = chords;
            }
        });
    }
    pub(super) fn perform_turnover_ridge(&mut self) {
        if self.mode != Mode::Face || self.sel_faces.is_empty() {
            self.message = "Flip face edge: select quads (Face mode)".into();
            return;
        }
        self.apply_edit("Pixel Builder: Flip face edge", |m, s| {
            if edit_ops::rotate_face_loop(m, &s.sel_faces) == 0 {
                s.message =
                    "Flip face edge: the selection has no faces with four or more corners".into();
            }
        });
    }
    pub(super) fn act_split_verts(&mut self) {
        if self.mode != Mode::Vertex || self.sel_verts.is_empty() {
            self.message = "Split vertices: select vertices (Vertex mode)".into();
            return;
        }
        self.apply_edit("Pixel Builder: Split vertices", |m, s| {
            let new = edit_ops::unshare_vertices(m, &s.sel_verts);
            s.sel_verts.extend(new);
        });
    }
    pub(super) fn act_triangulate(&mut self) {
        let object = self.mode == Mode::Object;
        if !object && (self.mode != Mode::Face || self.sel_faces.is_empty()) {
            self.message = "Triangulate: select faces (Face mode), or use it in Object mode for the whole mesh".into();
            return;
        }
        let name = if object {
            "Pixel Builder: Triangulate object"
        } else {
            "Pixel Builder: Triangulate faces"
        };
        self.apply_edit(name, |m, s| {
            let f = s.every_or_picked_facets(m);
            let out = ops::mesh_fill_facets(m, &f);
            if object {
                s.purge_picked();
            } else {
                s.sel_faces = out;
            }
        });
    }
    pub(super) fn perform_fuse_entities(&mut self) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(sel) = EditorInterface::singleton().get_selection() else {
            return;
        };
        let sources: Vec<Gd<PixelBuilderMesh>> = sel
            .get_selected_nodes()
            .iter_shared()
            .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
            .filter(|n| n.instance_id() != target.instance_id() && n.bind().contents().is_some())
            .collect();
        if sources.is_empty() {
            self.message = format!(
                "Merge objects: select at least one other builder mesh along with {} (Ctrl+click it in the Scene dock)",
                target.get_name()
            );
            return;
        }
        self.unshare_data();
        let Some(mut data) = self.contents() else {
            return;
        };
        let (before, old_mats) = (data.bind().get_blob(), data.bind().stuff_pockets());
        let mut m = data.bind().cage().clone();
        let mut materials = old_mats.duplicate_shallow();
        let inv = target.get_global_transform().affine_inverse();
        for src in &sources {
            let Some(sd) = src.bind().contents() else {
                continue;
            };
            let mut acc = super::super::data::PixelBuilderMeshData::new_gd();
            acc.bind_mut().substitute(m.clone());
            acc.set("materials", &materials.to_variant());
            let (mats, map) = super::boolean::merged_slots(&acc, &sd);
            materials = mats;
            let xf = affine(inv * src.get_global_transform());
            let sm = sd.bind().cage().clone();
            edit_ops::merge_blocks(&mut m, &sm, &xf, &map);
        }
        if let Err(e) = m.sanity_check() {
            self.message = format!("Merge objects failed: {e}");
            return;
        }
        data.bind_mut().substitute(m);
        data.set("materials", &materials.to_variant());
        data.emit_changed();
        let after = data.bind().get_blob();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action("Pixel Builder: Merge objects");
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "materials", &materials.to_variant());
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "materials", &old_mats.to_variant());
        for src in &sources {
            let Some(parent) = src.get_parent() else {
                continue;
            };
            ur.add_do_method(&parent, "remove_child", &[src.to_variant()]);
            ur.add_undo_method(&parent, "add_child", &[src.to_variant(), true.to_variant()]);
            ur.add_undo_method(
                &parent,
                "move_child",
                &[src.to_variant(), src.get_index().to_variant()],
            );
            if let Some(o) = src.get_owner() {
                ur.add_undo_method(src, "set_owner", &[o.to_variant()]);
            }
            ur.add_undo_reference(src);
        }
        ur.commit_action_ex().execute(false).done();
        for src in &sources {
            if let Some(mut parent) = src.get_parent() {
                parent.remove_child(src);
            }
        }
        if let Some(mut sel) = EditorInterface::singleton().get_selection() {
            sel.clear();
            sel.add_node(&target);
        }
        self.purge_picked();
        self.palette_slots = None;
        self.message = format!(
            "Merged {} mesh{} into {}",
            sources.len(),
            if sources.len() == 1 { "" } else { "es" },
            target.get_name()
        );
    }
    fn run_pivot_op(&mut self, name: &str, center: PVec3) {
        let Some(mut node) = self.target.clone() else {
            return;
        };
        if center.magnitude() < 1e-6 {
            self.message = format!("{name}: the pivot is already there");
            return;
        }
        self.unshare_data();
        let Some(mut data) = self.contents() else {
            return;
        };
        let before = data.bind().get_blob();
        let mut m = data.bind().cage().clone();
        m.displace(-center);
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        let old_xf = node.get_transform();
        let mut new_xf = old_xf;
        new_xf.origin += old_xf.basis * from_p(center);
        self.commit_node_xform(name, &node, &data, before, after, old_xf, new_xf);
        node.set_transform(new_xf);
        self.purge_picked();
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "recording one transform undo step needs the node, its data, both state snapshots and both transforms at once"
    )]
    fn commit_node_xform(
        &mut self,
        moniker: &str,
        node: &Gd<PixelBuilderMesh>,
        contents: &Gd<super::super::data::PixelBuilderMeshData>,
        before: PackedByteArray,
        after: PackedByteArray,
        old_xf: Transform3D,
        new_xf: Transform3D,
    ) {
        let kids: Vec<(Gd<Node3D>, Transform3D, Transform3D)> = node
            .get_children()
            .iter_shared()
            .filter_map(|c| c.try_cast::<Node3D>().ok())
            .map(|c| {
                let t = c.get_transform();
                (c, t, new_xf.affine_inverse() * old_xf * t)
            })
            .collect();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(moniker);
        ur.add_do_method(contents, "set_blob", &[after.to_variant()]);
        ur.add_do_property(node, "transform", &new_xf.to_variant());
        ur.add_undo_method(contents, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(node, "transform", &old_xf.to_variant());
        for (c, old, new) in &kids {
            ur.add_do_property(c, "transform", &new.to_variant());
            ur.add_undo_property(c, "transform", &old.to_variant());
        }
        ur.commit_action_ex().execute(false).done();
        for (mut c, _, new) in kids {
            c.set_transform(new);
        }
    }
    pub(super) fn act_center_pivot(&mut self) {
        let Some(d) = self.contents() else { return };
        let c = edit_ops::bounds_midpoint(d.bind().cage());
        self.run_pivot_op("Pixel Builder: Center pivot", c);
    }
    pub(super) fn act_pivot_to_selection(&mut self) {
        let Some(d) = self.contents() else { return };
        let c = {
            let db = d.bind();
            let m = db.cage();
            let v = self.selected_verts(m);
            if v.is_empty() {
                self.message = "Pivot to selection: select faces, edges or vertices first".into();
                return;
            }
            v.iter()
                .fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize])
                / v.len() as f32
        };
        self.run_pivot_op("Pixel Builder: Pivot to selection", c);
    }
    pub(super) fn perform_ice_warp(&mut self) {
        let Some(mut node) = self.target.clone() else {
            return;
        };
        let old_xf = node.get_transform();
        if old_xf.approx_eq(&Transform3D::IDENTITY) {
            self.message = "Freeze transform: the transform is already the identity".into();
            return;
        }
        self.unshare_data();
        let Some(mut data) = self.contents() else {
            return;
        };
        let before = data.bind().get_blob();
        let mut m = data.bind().cage().clone();
        edit_ops::bake_transform(&mut m, &affine(old_xf));
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        self.commit_node_xform(
            "Pixel Builder: Freeze transform",
            &node,
            &data,
            before,
            after,
            old_xf,
            Transform3D::IDENTITY,
        );
        node.set_transform(Transform3D::IDENTITY);
        self.purge_picked();
    }
    pub(super) fn act_collapse(&mut self) {
        if self.mode != Mode::Vertex || self.sel_verts.len() < 2 {
            self.message = "Collapse: select two or more vertices (Vertex mode)".into();
            return;
        }
        if self.xg.collapse_first {
            let target = self.active_vert;
            self.apply_edit("Pixel Builder: Collapse vertices", |m, s| {
                let v = s.sel_verts.clone();
                match pt_core::build::xg_ops::condense_corners_into(m, &v, target) {
                    Some(k) => {
                        s.sel_verts = BTreeSet::from([k]);
                        s.active_vert = Some(k);
                    }
                    None => {
                        s.message =
                            "Collapse refused: it would fold an edge onto a third face".into()
                    }
                }
            });
            return;
        }
        self.apply_edit("Pixel Builder: Collapse vertices", |m, s| {
            let v = s.sel_verts.clone();
            match pt_core::build::mesh_ops::collapse_selection(m, &v) {
                Some(k) => s.sel_verts = BTreeSet::from([k]),
                None => {
                    s.message = "Collapse refused: it would fold an edge onto a third face".into()
                }
            }
        });
    }
    fn trim_solids(&self) -> Vec<pt_core::build::BMesh> {
        let Some(target) = self.target.clone() else {
            return Vec::new();
        };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return Vec::new();
        };
        let to_local = target.get_global_transform().affine_inverse();
        let mut out = Vec::new();
        for n in root
            .find_children_ex("*")
            .type_("PixelBuilderMesh")
            .owned(false)
            .done()
            .iter_shared()
        {
            let Ok(b) = n.try_cast::<PixelBuilderMesh>() else {
                continue;
            };
            if b.instance_id() == target.instance_id() {
                continue;
            }
            let Some(data) = b.bind().contents() else {
                continue;
            };
            let mut m = data.bind().cage().clone();
            if !m.is_closed() {
                continue;
            }
            let xf = to_local * b.get_global_transform();
            for p in &mut m.positions {
                *p = to_p(xf * from_p(*p));
            }
            out.push(m);
        }
        out
    }
    pub(super) fn act_trim_walls(&mut self) {
        if self.mode != Mode::Face || self.sel_faces.is_empty() {
            self.message = "Trim walls: select wall faces (Face mode)".into();
            return;
        }
        let o = self.opts;
        let profile = pt_core::build::prims::TrimSection::from_u8(self.spec.silhouette);
        let (slot, col) = (
            self.pocket.clamp(0, 255) as u16,
            super::ui_color_bytes(self.tint),
        );
        let solids = self.trim_solids();
        self.apply_edit("Pixel Builder: Trim walls", |m, s| {
            let mut t = pt_core::build::trim_walls::trim_walls_clipped(
                m,
                &s.sel_faces,
                o.trim_place == 1,
                o.trim_depth,
                o.trim_height,
                profile,
                6,
                0.0,
                true,
                &solids,
            );
            if t.facets.is_empty() {
                s.message = "Trim walls: the selection has no (near-vertical) wall faces".into();
                return;
            }
            for f in &mut t.facets {
                f.stuff = slot;
                f.tint = col;
            }
            m.append_tail(&t);
        });
    }
    pub(super) fn act_entity(&mut self, kind: i32) {
        let Some(t) = self.target.clone() else { return };
        let old = t.get("entity_type").try_to::<i32>().unwrap_or(0);
        let new = if old == kind { 0 } else { kind };
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(
            [
                "Pixel Builder: Normal mesh",
                "Pixel Builder: Set collider",
                "Pixel Builder: Set trigger",
            ][new as usize],
        );
        ur.add_do_property(&t, "entity_type", &new.to_variant());
        ur.add_undo_property(&t, "entity_type", &old.to_variant());
        ur.commit_action();
        self.message = [
            "Normal mesh",
            "Collider: invisible static collider in the game",
            "Trigger: invisible Area3D in the game",
        ][new as usize]
            .into();
    }
    pub(super) fn gizmo_axes(&self) -> [Vector3; 3] {
        if let Some(d) = &self.drag {
            return d.axes;
        }
        match self.opts.gizmo_orient {
            1 => {
                let b = self.xform().basis.orthonormalized();
                [b.col_a(), b.col_b(), b.col_c()]
            }
            2 => {
                let Some(d) = self.contents() else {
                    return AXES_WORLD;
                };
                let db = d.bind();
                let m = db.cage();
                let faces: BTreeSet<usize> = match self.mode {
                    Mode::Face => self.sel_faces.clone(),
                    _ => {
                        let v = self.selected_verts(m);
                        (0..m.facets.len())
                            .filter(|f| m.facets[*f].verts.iter().any(|x| v.contains(x)))
                            .collect()
                    }
                };
                let sum = faces
                    .iter()
                    .filter(|f| **f < m.facets.len())
                    .fold(PVec3::ZERO, |a, f| a + m.face_normal(*f));
                let n = (self.xform().basis.inverse().transposed() * from_p(sum)).normalized();
                if n.length_squared() < 1e-8 {
                    return AXES_WORLD;
                }
                let r = if n.x.abs() < 0.9005 {
                    Vector3::RIGHT
                } else {
                    Vector3::UP
                };
                let t = (r - n * r.dot(n)).normalized();
                [t, n.cross(t), n]
            }
            _ => AXES_WORLD,
        }
    }
    pub(super) fn insert_vert_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        if let Ok(k) = happening
            .clone()
            .try_cast::<godot::classes::InputEventKey>()
        {
            if k.is_pressed() && k.get_keycode() == godot::global::Key::ESCAPE {
                self.insert_vert = false;
                self.insert_hover = None;
                self.sync_ui();
                self.base().update_overlays();
                return Some(true);
            }
            return None;
        }
        if let Ok(mm) = happening
            .clone()
            .try_cast::<godot::classes::InputEventMouseMotion>()
        {
            self.insert_hover = self.insert_target(cam, mm.get_position());
            self.base().update_overlays();
            return Some(false);
        }
        let mb = happening
            .clone()
            .try_cast::<godot::classes::InputEventMouseButton>()
            .ok()?;
        if mb.get_button_index() != godot::global::MouseButton::LEFT {
            return None;
        }
        if !mb.is_pressed() {
            return Some(true);
        }
        let Some((e, w)) = self.insert_target(cam, mb.get_position()) else {
            return Some(true);
        };
        let local = to_p(self.xform().affine_inverse() * w);
        self.apply_edit("Pixel Builder: Insert vertex", |m, s| {
            if let Some(v) = pt_core::build::mesh_ops::insert_vertex(m, e, local) {
                s.sel_edges.clear();
                s.sel_verts = BTreeSet::from([v]);
            }
        });
        self.insert_hover = None;
        Some(true)
    }
    fn insert_target(
        &self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
    ) -> Option<(pt_core::build::Edge, Vector3)> {
        let d = self.contents()?;
        let xf = self.xform();
        let db = d.bind();
        let m = db.cage();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| {
                (
                    pt_core::math::v2(cam.unproject_position(w).x, cam.unproject_position(w).y),
                    cam.get_global_position().distance_to(w),
                )
            })
        };
        let e = pick::pick_closest_edge(m, &project, pt_core::math::v2(pos.x, pos.y), 12.0)?;
        let (a, b) = (
            xf * from_p(m.positions[e.0 as usize]),
            xf * from_p(m.positions[e.1 as usize]),
        );
        let (sa, sb) = (cam.unproject_position(a), cam.unproject_position(b));
        let ab = sb - sa;
        let t = if ab.length_squared() < 1e-6 {
            0.5
        } else {
            ((pos - sa).dot(ab) / ab.length_squared()).clamp(0.0505, 0.9505)
        };
        let mut w = a.lerp(b, t);
        if self.snap_grid {
            let len = (b - a).length();
            if len > 1e-6 {
                let along = pt_core::build::snap::snap_value(t * len, self.grid_step)
                    .clamp(0.0505 * len, 0.9505 * len);
                w = a + (b - a) * (along / len);
            }
        }
        Some((e, w))
    }
    pub(super) fn draw_insert_hover(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        if !self.insert_vert || self.mode != Mode::Edge {
            return;
        }
        let Some((e, w)) = self.insert_hover else {
            return;
        };
        let Some(d) = self.contents() else { return };
        let xf = self.xform();
        let db = d.bind();
        let m = db.cage();
        if (e.0 as usize) < m.positions.len() && (e.1 as usize) < m.positions.len() {
            let (a, b) = (
                xf * from_p(m.positions[e.0 as usize]),
                xf * from_p(m.positions[e.1 as usize]),
            );
            if !cam.is_position_behind(a) && !cam.is_position_behind(b) {
                overlay
                    .draw_line_ex(
                        cam.unproject_position(a),
                        cam.unproject_position(b),
                        Color::from_rgba(1.0, 0.7505, 0.3005, 0.9005),
                    )
                    .width(2.0)
                    .done();
            }
        }
        if !cam.is_position_behind(w) {
            overlay.draw_circle(
                cam.unproject_position(w),
                5.0,
                Color::from_rgba(1.0, 0.8505, 0.2005, 1.0),
            );
        }
    }
}
