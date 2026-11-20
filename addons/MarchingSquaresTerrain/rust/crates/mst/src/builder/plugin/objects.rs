//! Builder plugin, part 5 - mesh and object commands after cyclopsLevelBuilder (MIT):
//!
//! * element ops: Subdivide edges, Connect edges, Flip face edge, Split vertices, Triangulate
//!   (via `pt_core::build::edit_ops`);
//! * object ops: Triangulate / Subdivide object, Merge objects, Center pivot / Pivot to selection,
//!   Freeze transform, children held in place;
//! * material drag-and-drop onto faces (preview-override snapshot / restore).

use std::collections::BTreeSet;

use godot::builtin::math::ApproxEq;
use godot::classes::control::MouseFilter;
use godot::classes::notify::ControlNotification;
use godot::classes::{
    Camera3D, Control, EditorInterface, GeometryInstance3D, IControl, InputEvent, Material,
    MeshInstance3D, Node, Node3D, ResourceLoader, SubViewport, SubViewportContainer,
};
use godot::prelude::*;

use pt_core::build::edit_ops::{self, Affine};
use pt_core::build::{ops, pick};
use pt_core::math::Vec3 as PVec3;

use super::super::data::{from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;

const DROP_NAME: &str = "_PixelBuilderMaterialDrop";
const AXES_WORLD: [Vector3; 3] = [Vector3::RIGHT, Vector3::UP, Vector3::BACK];

/// Godot's MeshInstance3D editor menu ("Mesh") next to our menu in the 3D toolbar.
fn find_mesh_menu(ours: Option<&Gd<godot::classes::MenuButton>>) -> Option<Gd<Control>> {
    let parent = ours?.get_parent()?;
    parent
        .get_children()
        .iter_shared()
        .filter_map(|c| c.try_cast::<godot::classes::MenuButton>().ok())
        .find(|m| {
            m.get_text().to_string() == "Mesh" && m.get_name().to_string() != "PixelBuilderMenu"
        })
        .map(|m| m.upcast())
}

pub(super) fn affine(xf: Transform3D) -> Affine {
    Affine {
        x: to_p(xf.basis.col_a()),
        y: to_p(xf.basis.col_b()),
        z: to_p(xf.basis.col_c()),
        origin: to_p(xf.origin),
    }
}

/// Material drag state (the dragged material and the preview overrides it puts on faces).
#[derive(Default)]
pub struct MaterialDrag {
    stuff: Option<Gd<Material>>,
    consumed: bool,
    /// (node, material_override, surface overrides) of every geometry in the scene at drag start.
    overrides: Vec<(
        Gd<GeometryInstance3D>,
        Option<Gd<Material>>,
        Vec<Option<Gd<Material>>>,
    )>,
    /// (viewport surface, drop catcher) per 3D viewport.
    pub addressees: Vec<(InstanceId, Gd<Control>)>,
}

impl PixelBuilderPlugin {
    // ================================================================== element ops

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

    // ================================================================== object ops

    /// `_run_merge_objects`: appends every other selected builder mesh (transform baked into this
    /// mesh's space, material slots joined) and removes them, in one undo step.
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
            // Join this source's slots into the running palette (a temporary data resource
            // carries the palette so far).
            let mut acc = super::super::data::PixelBuilderMeshData::new_gd();
            acc.bind_mut().substitute(m.clone());
            acc.set("materials", &materials.to_variant());
            let (mats, map) = super::tools::merged_slots(&acc, &sd);
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

    /// `_run_pivot_op`: shifts the mesh by `-center` and the node by `+center` (children held in
    /// place), one undo step.
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

    /// Records data + node transform (+ children held in place) and applies the children.
    #[allow(clippy::too_many_arguments)]
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

    /// `_run_freeze`: bakes the node transform into the mesh and resets it to identity.
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

    // ================================================================== collapse / trim / insert

    /// Collapse vertices onto one point (Vertex mode).
    pub(super) fn act_collapse(&mut self) {
        if self.mode != Mode::Vertex || self.sel_verts.len() < 2 {
            self.message = "Collapse: select two or more vertices (Vertex mode)".into();
            return;
        }
        if self.xg.collapse_first {
            // "Collapse to first": onto the first picked vertex.
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

    /// Closed builder meshes of the edited scene other than the target, in the target's frame.
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

    /// Trim walls (the Trim Walls shape kind): the moulding is appended to this mesh, cut
    /// where the walls run into other closed builder meshes.
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
        // Trims run across the scene's meshes: the other closed builder meshes, moved into
        // this mesh's frame, cut the moulding where the walls run into them (`clip_runs`).
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

    /// Collider / trigger toggle (the mesh's entity type), undoable.
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

    /// Handle axes in world space: Global, Local (the node's
    /// axes) or Normal (the selection's average face normal as z).
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

    /// Insert-vertex modal input (Edge mode): hover finds the edge under the cursor and the
    /// point on it; a click splits the edge there.
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
            // Snap the split along the edge to the grid step (from its first end).
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

    // ================================================================== menus

    /// Adds the "Builder" menu to the 3D toolbar and hides Godot's own MeshInstance3D "Mesh"
    /// menu while a builder mesh is edited (it acts on the compiled, unsaved mesh).
    pub(super) fn sync_menus(&mut self, shown: bool) {
        use godot::classes::MenuButton;
        use godot::classes::editor_plugin::CustomControlContainer;
        if self
            .builder_menu
            .as_ref()
            .is_none_or(|m| !m.is_instance_valid())
        {
            let mut mb = MenuButton::new_alloc();
            mb.set_name("PixelBuilderMenu");
            mb.set_text("Builder");
            mb.set_flat(false);
            mb.set_tooltip_text("Pixel Builder: UV editor, preferences, bake, export, pivot / freeze of the selected builder mesh");
            if let Some(mut p) = mb.get_popup() {
                // (title, id): ids are `on_builder_menu`'s (stable), the order is the menu's.
                for (t, id) in [
                    ("UV editor…", 0),
                    ("Preferences…", 1),
                    ("Bake to MeshInstance3D", 2),
                    ("Export OBJ…", 3),
                    ("Export glTF…", 4),
                    ("Freeze transform", 5),
                    ("Center pivot", 6),
                    ("Export STL…", 7),
                    ("Export PLY…", 8),
                    ("Export Godot mesh (.res)…", 14),
                    ("Export options…", 9),
                    ("Create reusable part…", 10),
                ] {
                    p.add_item_ex(t).id(id).done();
                }
                p.add_item_ex("Generate lightmap UVs").id(13).done();
                p.add_separator();
                p.add_check_item_ex("Modeler layout").id(11).done();
                p.add_item_ex("Modeler options…").id(12).done();
                p.connect("id_pressed", &self.to_gd().callable("on_builder_menu"));
            }
            self.base_mut()
                .add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &mb);
            self.builder_menu = Some(mb);
            self.refresh_menu_keys();
            self.refresh_menu_layout();
        }
        let show = shown && self.target.is_some();
        if let Some(m) = &mut self.builder_menu {
            m.set_visible(show);
        }
        if show {
            if self
                .hidden_mesh_menu
                .as_ref()
                .is_none_or(|m| !m.is_instance_valid())
            {
                self.hidden_mesh_menu = find_mesh_menu(self.builder_menu.as_ref());
            }
            if let Some(m) = &mut self.hidden_mesh_menu {
                m.set_visible(false);
            }
        } else if let Some(mut m) = self.hidden_mesh_menu.take()
            && m.is_instance_valid()
        {
            m.set_visible(true);
        }
    }

    /// The Builder menu shows the UV editor's current shortcut (it can be rebound).
    pub(super) fn refresh_menu_keys(&mut self) {
        let k = crate::builder::shortcuts::key_text("uv_editor");
        if let Some(m) = &self.builder_menu
            && m.is_instance_valid()
            && let Some(mut p) = m.get_popup()
        {
            p.set_item_text(
                0,
                &if k.is_empty() {
                    "UV editor…".to_string()
                } else {
                    format!("UV editor…  [{k}]")
                },
            );
        }
    }

    pub(super) fn remove_menus(&mut self) {
        use godot::classes::editor_plugin::CustomControlContainer;
        if let Some(mut m) = self.hidden_mesh_menu.take()
            && m.is_instance_valid()
        {
            m.set_visible(true);
        }
        if let Some(mb) = self.builder_menu.take()
            && mb.is_instance_valid()
        {
            self.base_mut()
                .remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &mb);
            mb.free();
        }
    }

    // ================================================================== material drop

    /// `_ensure_drop_target`: a drop catcher over the viewport surface (added once).
    pub(super) fn ensure_drop_target(&mut self, surface: &Gd<Control>) {
        // The catcher is added deferred (we are inside the surface's draw), so remember the
        // surface instead of asking it for the child.
        let sid = surface.instance_id();
        if self
            .drop
            .addressees
            .iter()
            .any(|(s, t)| *s == sid && t.is_instance_valid())
        {
            return;
        }
        let mut target = PixelBuilderMaterialDrop::new_alloc();
        target.set_name(DROP_NAME);
        {
            let mut t = target.bind_mut();
            t.plugin = Some(self.to_gd());
            t.viewport = surface
                .get_parent()
                .into_iter()
                .flat_map(|p| p.get_children().iter_shared().collect::<Vec<_>>())
                .filter_map(|c| c.try_cast::<SubViewportContainer>().ok())
                .find_map(|c| {
                    (c.get_child_count() > 0)
                        .then(|| c.get_child(0))
                        .flatten()
                        .and_then(|v| v.try_cast::<SubViewport>().ok())
                });
        }
        let mut s = surface.clone();
        s.call_deferred("add_child", &[target.to_variant()]);
        self.drop.addressees.push((sid, target.upcast()));
    }

    pub(super) fn remove_drop_targets(&mut self) {
        for (_, t) in self.drop.addressees.drain(..) {
            if t.is_instance_valid() {
                t.free();
            }
        }
    }

    /// Face of the edited mesh under `pos` in `viewport`.
    fn drop_face(&self, viewport: &Gd<SubViewport>, pos: Vector2) -> Option<usize> {
        let cam = viewport.get_camera_3d()?;
        let d = self.contents()?;
        let (o, dir) = self.local_ray(&cam, pos);
        pick::intersect_ray_closest(d.bind().cage(), o, dir, true).map(|h| h.face)
    }

    pub(super) fn can_drop_material(
        &self,
        viewport: &Gd<SubViewport>,
        pos: Vector2,
        data: &Variant,
    ) -> bool {
        self.target.is_some()
            && extract_material(data).is_some()
            && self.drop_face(viewport, pos).is_some()
    }

    /// `drop_material`: Face mode with the hovered face selected → the selected faces, Face mode
    /// otherwise → that face, other modes → the whole object. The material reuses its slot or
    /// gets a new one; one undo step.
    pub(super) fn drop_material(
        &mut self,
        viewport: &Gd<SubViewport>,
        pos: Vector2,
        data: &Variant,
    ) {
        let Some(mat) = extract_material(data) else {
            return;
        };
        let Some(face) = self.drop_face(viewport, pos) else {
            return;
        };
        self.unshare_data();
        let Some(mut d) = self.contents() else { return };
        let faces: BTreeSet<usize> = if self.mode == Mode::Face {
            if self.sel_faces.contains(&face) {
                self.sel_faces.clone()
            } else {
                BTreeSet::from([face])
            }
        } else {
            (0..d.bind().cage().facets.len()).collect()
        };
        let name = if self.mode == Mode::Face {
            "Pixel Builder: Drop material on face"
        } else {
            "Pixel Builder: Drop material on object"
        };
        let (before, old_mats) = (d.bind().get_blob(), d.bind().stuff_pockets());
        let mut mats = old_mats.duplicate_shallow();
        let mut m = d.bind().cage().clone();
        let slot = match mats.iter_shared().position(|x| {
            x.as_ref()
                .is_some_and(|x| x.instance_id() == mat.instance_id())
        }) {
            Some(i) => i,
            None => {
                let used = m
                    .facets
                    .iter()
                    .map(|f| f.stuff as usize + 1)
                    .max()
                    .unwrap_or(0);
                while mats.len() < used {
                    mats.push(None::<&Gd<Material>>);
                }
                mats.push(Some(&mat));
                mats.len() - 1
            }
        };
        ops::assign_stuff(&mut m, &faces, slot as u16);
        d.bind_mut().substitute(m);
        d.set("materials", &mats.to_variant());
        d.emit_changed();
        let after = d.bind().get_blob();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(name);
        ur.add_do_method(&d, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&d, "materials", &mats.to_variant());
        ur.add_undo_method(&d, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&d, "materials", &old_mats.to_variant());
        ur.commit_action_ex().execute(false).done();
        self.drop.consumed = true;
        self.pocket = slot as i64;
        self.palette_slots = None;
        self.message = format!(
            "Material dropped on {} face{} (slot {slot})",
            faces.len(),
            if faces.len() == 1 { "" } else { "s" }
        );
        self.sync_ui();
        self.redraw();
    }

    pub(super) fn material_drag_began(&mut self, mat: Gd<Material>) {
        if self.drop.stuff.is_some() {
            return;
        }
        self.drop.stuff = Some(mat);
        self.drop.consumed = false;
        // `_snapshot_overrides`: Godot's own viewport drop previews the material on meshes it
        // hovers; a drop we consume restores what was there.
        self.drop.overrides.clear();
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let mut stack: Vec<Gd<Node>> = vec![root];
        while let Some(n) = stack.pop() {
            stack.extend(n.get_children().iter_shared());
            let Ok(gi) = n.try_cast::<GeometryInstance3D>() else {
                continue;
            };
            let surfaces = gi
                .clone()
                .try_cast::<MeshInstance3D>()
                .map(|mi| {
                    (0..mi.get_surface_override_material_count())
                        .map(|i| mi.get_surface_override_material(i))
                        .collect()
                })
                .unwrap_or_default();
            let ov = gi.get_material_override();
            self.drop.overrides.push((gi, ov, surfaces));
        }
    }

    pub(super) fn material_drag_ended(&mut self) {
        let Some(mat) = self.drop.stuff.take() else {
            return;
        };
        if self.drop.consumed {
            let id = mat.instance_id();
            let is_drag =
                |m: &Option<Gd<Material>>| m.as_ref().is_some_and(|m| m.instance_id() == id);
            for (gi, ov, surfaces) in self.drop.overrides.drain(..) {
                if !gi.is_instance_valid() {
                    continue;
                }
                let mut gi = gi;
                if is_drag(&gi.get_material_override()) && !is_drag(&ov) {
                    gi.set_material_override(ov.as_ref());
                }
                if let Ok(mut mi) = gi.try_cast::<MeshInstance3D>() {
                    for (i, s) in surfaces
                        .iter()
                        .enumerate()
                        .take(mi.get_surface_override_material_count() as usize)
                    {
                        if is_drag(&mi.get_surface_override_material(i as i32)) && !is_drag(s) {
                            mi.set_surface_override_material(i as i32, s.as_ref());
                        }
                    }
                }
            }
        }
        self.drop.overrides.clear();
    }
}

/// The dragged Material (a resource, or one material file).
pub fn extract_material(data: &Variant) -> Option<Gd<Material>> {
    let d = data.try_to::<VarDictionary>().ok()?;
    match d.get("type").map(|t| t.to_string()).as_deref() {
        Some("resource") => d.get("resource")?.try_to::<Gd<Material>>().ok(),
        Some("files") => {
            let files = d.get("files")?.try_to::<PackedStringArray>().ok()?;
            if files.len() != 1 {
                return None;
            }
            let path = files.get(0)?;
            let mut rl = ResourceLoader::singleton();
            if !rl.exists_ex(&path).type_hint("Material").done() {
                return None;
            }
            rl.load(&path).and_then(|r| r.try_cast::<Material>().ok())
        }
        _ => None,
    }
}

/// Transparent control over the 3D viewport surface that only catches the
/// mouse while a Material is being dragged, so drops land on builder faces.
#[derive(GodotClass)]
#[class(tool, init, base = Control, internal)]
pub struct PixelBuilderMaterialDrop {
    plugin: Option<Gd<PixelBuilderPlugin>>,
    viewport: Option<Gd<SubViewport>>,
    base: Base<Control>,
}

#[godot_api]
impl IControl for PixelBuilderMaterialDrop {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().set_process(true);
    }

    fn process(&mut self, _delta: f64) {
        if let Some(vp) = &self.viewport
            && vp.is_instance_valid()
        {
            let want = Vector2::new(vp.get_size().x as f32, vp.get_size().y as f32);
            if self.base().get_size() != want {
                self.base_mut().set_size(want);
            }
            if self.base().get_position() != Vector2::ZERO {
                self.base_mut().set_position(Vector2::ZERO);
            }
        }
        let Some(vp) = self.base().get_viewport() else {
            return;
        };
        let dragging = vp.gui_is_dragging() && extract_material(&vp.gui_get_drag_data()).is_some();
        let want = if dragging {
            MouseFilter::PASS
        } else {
            MouseFilter::IGNORE
        };
        if self.base().get_mouse_filter() != want {
            self.base_mut().set_mouse_filter(want);
        }
    }

    fn on_notification(&mut self, what: ControlNotification) {
        let Some(mut plugin) = self.plugin.clone().filter(|p| p.is_instance_valid()) else {
            return;
        };
        match what {
            ControlNotification::DRAG_BEGIN => {
                let Some(vp) = self.base().get_viewport() else {
                    return;
                };
                if let Some(m) = extract_material(&vp.gui_get_drag_data()) {
                    plugin.bind_mut().material_drag_began(m);
                }
            }
            ControlNotification::DRAG_END => plugin.bind_mut().material_drag_ended(),
            _ => {}
        }
    }

    fn can_drop_data(&self, at_position: Vector2, data: Variant) -> bool {
        let (Some(plugin), Some(vp)) = (
            self.plugin.as_ref().filter(|p| p.is_instance_valid()),
            self.viewport.as_ref(),
        ) else {
            return false;
        };
        plugin.bind().can_drop_material(vp, at_position, &data)
    }

    fn drop_data(&mut self, at_position: Vector2, data: Variant) {
        let (Some(mut plugin), Some(vp)) = (
            self.plugin.clone().filter(|p| p.is_instance_valid()),
            self.viewport.clone(),
        ) else {
            return;
        };
        plugin.bind_mut().drop_material(&vp, at_position, &data);
    }
}
