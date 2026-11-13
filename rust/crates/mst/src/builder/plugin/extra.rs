//! Builder plugin, part 2: the drag-to-draw shape tool, material painting, the material palette
//! and face UV panel, the new topology actions, OBJ / glTF export and mesh conversion.

use std::collections::BTreeSet;

use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::file_access::ModeFlags;
use godot::classes::{
    BaseMaterial3D, EditorFileDialog, FileAccess, GltfDocument, GltfState, Material, Mesh, MeshInstance3D, StandardMaterial3D,
};
use godot::classes::base_material_3d::{Flags, TextureParam};
use godot::classes::mesh::ArrayType;
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::compile::CompileOptions;
use pt_core::build::model::UvParams;
use pt_core::build::{BMesh, ShapeKind, convert, ops, pick, shapes, snap, topo};
use pt_core::math::{Vec3 as PVec3, v2, v3};

use super::super::data::{PixelBuilderMeshData, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::{Mode, UvField};
use super::{PixelBuilderPlugin, Placement, plane_hit, ui_color_bytes};
use crate::util::*;

/// Drag-to-draw state of the shape tool.
pub struct ShapeDrag {
    /// 0 = dragging the footprint, 1 = moving the mouse for the height.
    pub stage: u8,
    pub start: Placement,
    /// Footprint corner under the mouse (same height as `start`).
    pub corner: Vector3,
    pub height: f32,
    pub press: Vector2,
    pub append: bool,
    /// Screen-space anchor of the height stage.
    pub height_from: Vector2,
}

/// One paint stroke (one undo step).
pub struct PaintStroke {
    pub before: PackedByteArray,
    pub painted: BTreeSet<usize>,
}

impl PixelBuilderPlugin {
    // ================================================================== shape tool

    /// Placement point on the horizontal plane through `y` under the mouse, snapped like
    /// `placement` (terrain cells or grid).
    pub(super) fn plane_point(&self, cam: &Gd<Camera3D>, pos: Vector2, y: f32) -> Option<Vector3> {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let mut p = plane_hit(Vector3::new(0.0, y, 0.0), Vector3::UP, o, d)?;
        if self.snap_cell
            && let Some(t) = &self.terrain
        {
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * p;
            let tb = t.bind();
            let cfg = &tb.store().config;
            let q = if self.spec.kind.is_grid_shape() { snap::cell_center(cfg, v2(l.x, l.z)).1 } else { snap::cell_point(cfg, v2(l.x, l.z), true) };
            let w = xf * Vector3::new(q.x, l.y, q.y);
            p.x = w.x;
            p.z = w.z;
        } else if self.snap_grid {
            p.x = snap::snap_value(p.x, self.grid_step);
            p.z = snap::snap_value(p.z, self.grid_step);
        }
        p.y = y;
        Some(p)
    }

    /// Point under the mouse on the drawing plane through `origin` (horizontal, or the face
    /// plane of a Draw-on-surface frame). Grid snap works in the face plane too: the offset from
    /// `origin` (itself snapped by `pick_place`) is snapped along the frame's two in-plane axes.
    pub(super) fn draw_point(&self, cam: &Gd<Camera3D>, pos: Vector2, origin: Vector3, basis: Basis) -> Option<Vector3> {
        if basis == Basis::IDENTITY {
            return self.plane_point(cam, pos, origin.y);
        }
        let hit = plane_hit(origin, basis.col_b(), cam.project_ray_origin(pos), cam.project_ray_normal(pos))?;
        if !self.snap_grid {
            return Some(hit);
        }
        let mut d = basis.inverse() * (hit - origin);
        d.x = snap::snap_value(d.x, self.grid_step);
        d.z = snap::snap_value(d.z, self.grid_step);
        d.y = 0.0;
        Some(origin + basis * d)
    }

    /// Frame of the shape being drawn / previewed.
    pub(super) fn draw_basis(&self) -> Basis {
        if !self.wall_pts.is_empty() {
            return self.wall_basis;
        }
        if let Some(sd) = &self.shape_drag {
            return sd.start.basis;
        }
        self.hover_place.map(|p| p.basis).unwrap_or(Basis::IDENTITY)
    }

    /// Handles input while the shape tool is active. `Some(consumed)` when handled.
    pub(super) fn shape_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let Some(sd) = &self.shape_drag else {
                self.update_hover(cam, pos);
                return Some(false);
            };
            if sd.stage == 0 {
                let (start, basis) = (sd.start.point, sd.start.basis);
                if let Some(c) = self.draw_point(cam, pos, start, basis) {
                    if let Some(sd) = &mut self.shape_drag {
                        sd.corner = c;
                    }
                }
            } else {
                // Height: ray against the vertical plane through the footprint centre that faces
                // the camera.
                let centre = (sd.start.point + sd.corner) * 0.5;
                let up = sd.start.basis.col_b();
                let mut n = cam.get_global_transform().basis.col_c();
                n -= up * n.dot(up);
                let n = if n.length() < 1e-4 { sd.start.basis.col_c() } else { n.normalized() };
                let o = cam.project_ray_origin(pos);
                let d = cam.project_ray_normal(pos);
                let from = sd.height_from;
                let mut h = match plane_hit(centre, n, o, d) {
                    Some(hit) => (hit - sd.start.point).dot(up),
                    None => (from.y - pos.y) * 0.02,
                };
                if self.snap_grid {
                    h = snap::snap_value(h, self.grid_step);
                }
                let min = if self.snap_grid { self.grid_step } else { 0.05 };
                if let Some(sd) = &mut self.shape_drag {
                    sd.height = h.max(min);
                }
            }
            self.base().update_overlays();
            return Some(true);
        }
        let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() else { return None };
        let pos = mb.get_position();
        match mb.get_button_index() {
            MouseButton::RIGHT if mb.is_pressed() && self.shape_drag.is_some() => {
                self.cancel_shape();
                Some(true)
            }
            MouseButton::LEFT => {
                if mb.is_pressed() {
                    if self.shape_drag.as_ref().is_some_and(|s| s.stage == 1) {
                        self.commit_shape();
                        return Some(true);
                    }
                    self.update_hover(cam, pos);
                    let Some(p) = self.hover_place else {
                        self.message = "Nothing under the cursor".into();
                        return Some(true);
                    };
                    let append = Input::singleton().is_key_pressed(Key::SHIFT);
                    self.shape_drag = Some(ShapeDrag { stage: 0, start: p, corner: p.point, height: self.spec.size.y, press: pos, append, height_from: pos });
                } else if let Some(sd) = &self.shape_drag
                    && sd.stage == 0
                {
                    if (pos - sd.press).length() < 6.0 || (sd.corner - sd.start.point).length() < 1e-3 {
                        // A click: stamp at the current size.
                        let append = sd.append;
                        self.shape_drag = None;
                        self.stamp_shape(append);
                    } else if let Some(sd) = &mut self.shape_drag {
                        sd.stage = 1;
                        sd.height_from = pos;
                    }
                }
                self.base().update_overlays();
                Some(true)
            }
            _ => None,
        }
    }

    pub(super) fn cancel_shape(&mut self) {
        if self.shape_drag.take().is_some() {
            self.base().update_overlays();
        }
    }

    /// Spec, local mesh and world origin of the shape being dragged.
    pub(super) fn dragged_shape(&self) -> Option<(Vector3, BMesh)> {
        let sd = self.shape_drag.as_ref()?;
        let a = sd.start.point;
        let b = sd.corner;
        let mut spec = self.spec;
        let h = if sd.stage == 0 { spec.size.y } else { sd.height };
        spec.size.y = h;
        if spec.kind.is_grid_shape() {
            // Footprint radius in cells.
            let cw = self.terrain.as_ref().map(|t| t.bind().store().config.cell_size_safe().x).unwrap_or(1.0).max(0.01);
            let dist = Vector2::new(b.x - a.x, b.z - a.z).length();
            spec.rings = (dist / cw).round() as u32;
            if spec.kind == ShapeKind::HexPrism && spec.rings > 0 {
                spec.kind = ShapeKind::HexCluster;
            }
            let saved = (self.spec, spec);
            let m = {
                // shape_mesh reads self.spec; build with the dragged one.
                let tcfg = self.terrain.as_ref().map(|t| t.bind().store().config.clone()).filter(|c| c.grid.is_cell_mode());
                match (&tcfg, sd.start.cell) {
                    (Some(cfg), Some(c)) => {
                        let r = if spec.kind == ShapeKind::HexPrism { 0 } else { spec.rings.min(16) as i32 };
                        let cells = shapes::cluster_cells(cfg, to_iv(c), r);
                        shapes::cells_prism(cfg, &cells, to_iv(c), h)
                    }
                    _ => shapes::generate(&saved.1, tcfg.as_ref()),
                }
            };
            return Some((a, m));
        }
        // In the drawing frame (horizontal, or the face plane with Draw on surface).
        let dl = sd.start.basis.inverse() * (b - a);
        let (dx, dz) = (dl.x, dl.z);
        let min = if self.snap_grid { self.grid_step } else { 0.05 };
        let along_x = dx.abs() > dz.abs();
        let rises = matches!(spec.kind, ShapeKind::Stairs | ShapeKind::Wedge);
        // Stairs / ramps climb towards the end of the drag along its longer axis.
        let (w, d) = (dx.abs().max(min), dz.abs().max(min));
        spec.size.x = if rises && along_x { d } else { w };
        spec.size.z = if rises && along_x { w } else { d };
        let mut m = shapes::generate(&spec, None);
        if rises {
            let angle = if along_x {
                if dx > 0.0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 }
            } else if dz > 0.0 {
                0.0
            } else {
                std::f32::consts::PI
            };
            if angle != 0.0 {
                let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
                topo::rotate_verts(&mut m, &all, PVec3::ZERO, PVec3::UP, angle);
            }
        }
        let centre = a + sd.start.basis * Vector3::new(dx * 0.5, 0.0, dz * 0.5);
        Some((centre, m))
    }

    pub(super) fn commit_shape_pub(&mut self) {
        self.commit_shape();
    }

    fn commit_shape(&mut self) {
        let Some((origin, m)) = self.dragged_shape() else { return };
        let Some(sd) = self.shape_drag.take() else { return };
        // Remember the drawn size for the next click-stamp.
        let b = m.bounds();
        if let Some((lo, hi)) = b
            && !self.spec.kind.is_grid_shape()
        {
            let rises = matches!(self.spec.kind, ShapeKind::Stairs | ShapeKind::Wedge);
            let (sx, sz) = (hi.x - lo.x, hi.z - lo.z);
            let along_x = rises && (sd.corner.x - sd.start.point.x).abs() > (sd.corner.z - sd.start.point.z).abs();
            self.spec.size = if along_x { v3(sz, hi.y - lo.y, sx) } else { v3(sx, hi.y - lo.y, sz) };
        } else if let Some((lo, hi)) = b {
            self.spec.size.y = hi.y - lo.y;
        }
        let place = Placement { point: origin, cell: sd.start.cell, basis: sd.start.basis };
        self.create_from_mesh(place, m, sd.append);
        self.sync_ui();
    }

    // ================================================================== painting

    pub(super) fn paint_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        self.target.as_ref()?;
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let face = self.face_under(cam, pos);
            if face != self.hover_face {
                self.hover_face = face;
                self.redraw();
            }
            if self.paint.is_some()
                && let Some(f) = face
            {
                self.paint_faces(BTreeSet::from([f]));
            }
            return Some(self.paint.is_some());
        }
        let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() else { return None };
        if mb.get_button_index() != MouseButton::LEFT {
            return None;
        }
        let pos = mb.get_position();
        if mb.is_pressed() {
            let Some(f) = self.face_under(cam, pos) else { return Some(false) };
            let data = self.data()?;
            let input = Input::singleton();
            if input.is_key_pressed(Key::CTRL) {
                // Pick the slot and color of the face.
                let (slot, col, uv) = {
                    let d = data.bind();
                    let face = &d.mesh().faces[f];
                    (face.material, face.color, face.uv)
                };
                self.slot = slot as i64;
                self.color = Color::from_rgba8(col[0], col[1], col[2], col[3]);
                self.load_uv(uv);
                self.message = format!("Picked slot {slot}");
                self.sync_ui();
                return Some(true);
            }
            self.paint = Some(PaintStroke { before: data.bind().get_blob(), painted: BTreeSet::new() });
            let faces = if input.is_key_pressed(Key::SHIFT) { topo::coplanar_faces(data.bind().mesh(), f, 1.0) } else { BTreeSet::from([f]) };
            self.paint_faces(faces);
        } else if let Some(stroke) = self.paint.take()
            && let Some(data) = self.data()
        {
            let after = data.bind().get_blob();
            if after != stroke.before {
                self.commit_blob("Pixel Builder: Paint", &data, stroke.before, after);
            }
        }
        Some(true)
    }

    fn face_under(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<usize> {
        let (o, d) = self.local_ray(cam, pos);
        self.data().and_then(|data| pick::ray_mesh(data.bind().mesh(), o, d, true).map(|h| h.face))
    }

    /// Paints faces live (the stroke is committed as one undo step on release).
    fn paint_faces(&mut self, faces: BTreeSet<usize>) {
        let Some(mut data) = self.data() else { return };
        let Some(stroke) = &mut self.paint else { return };
        let new: BTreeSet<usize> = faces.difference(&stroke.painted).copied().collect();
        if new.is_empty() {
            return;
        }
        stroke.painted.extend(new.iter().copied());
        let (slot, col) = (self.slot.clamp(0, 255) as u16, ui_color_bytes(self.color));
        let mut m = data.bind().mesh().clone();
        ops::set_material(&mut m, &new, slot);
        ops::set_color(&mut m, &new, col);
        data.bind_mut().replace(m);
        data.emit_changed();
        self.redraw();
    }

    // ================================================================== palette / UV

    pub(super) fn uv_params(&self) -> UvParams {
        UvParams { offset: v2(self.uv[0], self.uv[1]), scale: v2(self.uv[2], self.uv[3]), rotation: self.uv[4], world: self.uv_world }
    }

    pub(super) fn load_uv(&mut self, uv: UvParams) {
        self.uv = [uv.offset.x, uv.offset.y, uv.scale.x, uv.scale.y, uv.rotation];
        self.uv_world = uv.world;
    }

    /// After a pick: one selected face loads its slot, color and UV into the panels.
    pub(super) fn load_face_props(&mut self) {
        if self.mode != Mode::Face || self.sel_faces.len() != 1 {
            return;
        }
        let Some(data) = self.data() else { return };
        let f = *self.sel_faces.first().unwrap_or(&0);
        let Some((slot, col, uv)) = data.bind().mesh().faces.get(f).map(|x| (x.material, x.color, x.uv)) else { return };
        self.slot = slot as i64;
        self.color = Color::from_rgba8(col[0], col[1], col[2], col[3]);
        self.load_uv(uv);
        self.sync_ui();
    }

    pub(super) fn apply_uv(&mut self, uv: UvParams) {
        self.apply_edit("Pixel Builder: Face UV", |m, s| {
            let f = s.all_or_selected_faces(m);
            ops::set_uv(m, &f, uv);
        });
    }

    pub(super) fn rotate_uv(&mut self) {
        self.apply_edit("Pixel Builder: Rotate UV", |m, s| {
            for f in s.all_or_selected_faces(m) {
                if let Some(face) = m.faces.get_mut(f) {
                    face.uv.rotation = (face.uv.rotation + 90.0) % 360.0;
                }
            }
        });
        self.load_face_props();
    }

    pub(super) fn uv_field(&mut self, f: UvField, v: f32) {
        self.uv[f as usize] = v;
    }

    /// Slot button: make current, assign to the selected faces in Face mode.
    pub(super) fn pick_slot(&mut self, slot: i64) {
        self.slot = slot;
        if self.mode == Mode::Face && !self.sel_faces.is_empty() {
            let s = slot.clamp(0, 255) as u16;
            self.apply_edit("Pixel Builder: Assign material", |m, st| ops::set_material(m, &st.sel_faces, s));
        }
        self.sync_ui();
    }

    pub(super) fn set_slot_material(&mut self, res: Option<Gd<Resource>>, slot: i64) {
        let Some(data) = self.data() else { return };
        let old = data.bind().material_slots();
        let mut new = old.duplicate_shallow();
        let mat = res.and_then(|r| r.try_cast::<Material>().ok());
        while (new.len() as i64) <= slot {
            new.push(None::<&Gd<Material>>);
        }
        new.set(slot as usize, mat.as_ref());
        self.commit_materials("Pixel Builder: Slot material", &data, old, new);
    }

    pub(super) fn add_slot(&mut self) {
        let Some(data) = self.data() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let old = data.bind().material_slots();
        let mut new = old.duplicate_shallow();
        let mut m = StandardMaterial3D::new_gd();
        m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        m.set_roughness(1.0);
        // A distinct, muted default tint per slot.
        let hues = [0.08, 0.3, 0.58, 0.0, 0.75, 0.15, 0.45];
        let k = new.len();
        m.set_albedo(Color::from_hsv(hues[k % hues.len()] as f64, 0.35, 0.85 - 0.1 * ((k / hues.len()) % 3) as f64));
        let mat: Gd<Material> = m.upcast();
        new.push(Some(&mat));
        self.slot = k as i64;
        self.commit_materials("Pixel Builder: Add slot", &data, old, new);
    }

    fn commit_materials(&mut self, name: &str, data: &Gd<PixelBuilderMeshData>, old: Array<Option<Gd<Material>>>, new: Array<Option<Gd<Material>>>) {
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action(name);
        ur.add_do_property(data, "materials", &new.to_variant());
        ur.add_undo_property(data, "materials", &old.to_variant());
        ur.commit_action();
        self.palette_slots = None;
        self.sync_ui();
    }

    /// Rebuilds the palette rows when the slot list changed.
    pub(super) fn refresh_palette(&mut self) {
        let mats: Vec<Option<Gd<Material>>> = self.data().map(|d| d.bind().material_slots().iter_shared().collect()).unwrap_or_default();
        let ids: Vec<i64> = mats.iter().map(|m| m.as_ref().map(|m| m.instance_id().to_i64()).unwrap_or(0)).collect();
        if self.palette_slots.as_ref() == Some(&ids) {
            return;
        }
        self.palette_slots = Some(ids);
        let slot = self.slot;
        if let Some(u) = &mut self.ui {
            u.rebuild_slots(&mats, slot);
        }
    }

    // ================================================================== new topology actions

    pub(super) fn act_extrude_edges(&mut self) {
        let dist = self.distance;
        self.apply_edit("Pixel Builder: Extrude edges", |m, s| {
            let out = topo::extrude_edges(m, &s.sel_edges, dist, PVec3::ZERO);
            if out.is_empty() {
                s.message = "Extrude edges: only open (boundary) edges extrude".into();
            }
            s.sel_edges = out;
        });
    }

    pub(super) fn act_merge(&mut self) {
        match self.mode {
            Mode::Face => self.apply_edit("Pixel Builder: Merge faces", |m, s| match topo::merge_faces(m, &s.sel_faces) {
                Some(f) => s.sel_faces = BTreeSet::from([f]),
                None => s.message = "Merge: select two or more connected faces with one outline".into(),
            }),
            Mode::Vertex if self.sel_verts.len() > 1 => self.apply_edit("Pixel Builder: Merge vertices", |m, s| {
                let v = s.sel_verts.clone();
                let kept = ops::merge_verts(m, &v);
                s.clear_selection();
                s.sel_verts.extend(kept);
            }),
            _ => self.message = "Merge: select faces or two or more vertices".into(),
        }
    }

    pub(super) fn act_connect(&mut self) {
        self.apply_edit("Pixel Builder: Connect", |m, s| {
            let n = topo::connect_selected(m, &s.sel_verts);
            if n == 0 {
                s.message = "Connect: select two vertices of the same face".into();
            }
        });
    }

    pub(super) fn act_fill(&mut self) {
        self.apply_edit("Pixel Builder: Fill hole", |m, s| {
            let verts = s.selected_verts(m);
            let only = if verts.is_empty() { None } else { Some(&verts) };
            let faces = topo::fill_holes(m, only);
            if faces.is_empty() {
                s.message = "Fill: no open boundary through the selection".into();
            }
        });
    }

    pub(super) fn act_subdivide(&mut self) {
        if self.mode == Mode::Edge {
            self.act_subdivide_edges();
            return;
        }
        if self.mode == Mode::Face && self.sel_faces.is_empty() {
            self.message = "Subdivide: select faces (Face mode), or use it in Object mode for the whole mesh".into();
            return;
        }
        // Object mode = Unibuilder's `subdivide_object` (every face).
        let name = if self.mode == Mode::Object { "Pixel Builder: Subdivide object" } else { "Pixel Builder: Subdivide" };
        let segs = self.opts.subdiv_segs.clamp(2, 16) as u32;
        self.apply_edit(name, |m, s| {
            let f = s.all_or_selected_faces(m);
            // Unibuilder subdivide: Subdiv pieces per edge, kernel centre for concave faces.
            let out = pt_core::build::ub_ops::subdivide_faces(m, &f, segs);
            if s.mode == Mode::Face {
                s.sel_faces = out;
            } else {
                s.clear_selection();
            }
        });
    }

    pub(super) fn act_rotate(&mut self, angle_deg: f32) {
        let Some(pivot) = self.pivot() else {
            self.message = "Rotate: select something first".into();
            return;
        };
        let local = to_p(self.xform().affine_inverse() * pivot);
        self.apply_edit("Pixel Builder: Rotate", |m, s| {
            let v = s.selected_verts(m);
            topo::rotate_verts(m, &v, local, PVec3::UP, angle_deg.to_radians());
        });
    }

    pub(super) fn act_select(&mut self, which: u8) {
        let Some(d) = self.data() else { return };
        let db = d.bind();
        let m = db.mesh();
        match (which, self.mode) {
            (0, Mode::Face) => self.sel_faces = m.grow_faces(&self.sel_faces),
            (0, Mode::Edge) => {
                let v = m.verts_of_edges(&self.sel_edges);
                let faces: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].verts.iter().any(|x| v.contains(x))).collect();
                self.sel_edges = m.edges_of_faces(&faces);
            }
            (0, Mode::Vertex) => {
                let faces: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].verts.iter().any(|x| self.sel_verts.contains(x))).collect();
                self.sel_verts = m.verts_of_faces(&faces);
            }
            (1, Mode::Face) => {
                let mut out = BTreeSet::new();
                for f in &self.sel_faces {
                    out.extend(topo::coplanar_faces(m, *f, 1.0));
                }
                self.sel_faces = out;
            }
            (2, Mode::Face) => {
                let mut out = BTreeSet::new();
                for f in &self.sel_faces {
                    out.extend(topo::similar_faces(m, *f));
                }
                self.sel_faces = out;
            }
            _ => {}
        }
    }

    // ================================================================== export / convert

    pub(super) fn open_export(&mut self, gltf: bool) {
        let Some(t) = self.target.clone() else { return };
        let mut dlg = match self.export_dialog.clone().filter(|d| d.is_instance_valid()) {
            Some(d) => d,
            None => {
                let mut d = EditorFileDialog::new_alloc();
                d.set_file_mode(FileMode::SAVE_FILE);
                d.set_access(Access::FILESYSTEM);
                d.connect("file_selected", &self.to_gd().callable("on_export_file"));
                if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                    base.add_child(&d);
                }
                self.export_dialog = Some(d.clone());
                d
            }
        };
        dlg.clear_filters();
        if gltf {
            dlg.add_filter_ex("*.glb").description("glTF binary").done();
            dlg.add_filter_ex("*.gltf").description("glTF text").done();
        } else {
            dlg.add_filter_ex("*.obj").description("Wavefront OBJ").done();
        }
        dlg.set_title(if gltf { "Export builder mesh (glTF)" } else { "Export builder mesh (OBJ)" });
        dlg.set_current_file(&format!("{}.{}", t.get_name(), if gltf { "glb" } else { "obj" }));
        dlg.popup_file_dialog();
    }

    /// Writes the target to `path` (format from the extension). Returns an error message.
    pub(super) fn export_to(&mut self, path: &str) -> Result<String, String> {
        let t = self.target.clone().ok_or("no builder mesh selected")?;
        let data = t.bind().data().ok_or("mesh has no data")?;
        let lower = path.to_lowercase();
        if lower.ends_with(".obj") {
            let stem = path.rsplit(['/', '\\']).next().unwrap_or("mesh").trim_end_matches(".obj").to_string();
            let mtl_name = format!("{stem}.mtl");
            let mats: Vec<Option<Gd<Material>>> = data.bind().material_slots().iter_shared().collect();
            let names: Vec<String> = (0..mats.len().max(1)).map(|i| format!("{stem}_slot{i}")).collect();
            let opt = CompileOptions { units_per_repeat: data.bind().units(), world_offset: to_p(t.get_global_position()) };
            let obj = convert::to_obj(data.bind().mesh(), &opt, &t.get_name().to_string(), Some(&mtl_name), &names);
            let used: BTreeSet<u16> = data.bind().mesh().faces.iter().map(|f| f.material).collect();
            let mtl_entries: Vec<(String, [f32; 3], Option<String>)> = used
                .iter()
                .map(|s| {
                    let m = mats.get(*s as usize).cloned().flatten();
                    let c = m.as_ref().and_then(super::super::ui::material_color).unwrap_or(Color::WHITE);
                    let tex = m
                        .and_then(|m| m.try_cast::<BaseMaterial3D>().ok())
                        .and_then(|b| b.get_texture(TextureParam::ALBEDO))
                        .map(|t| t.get_path().to_string())
                        .filter(|p| !p.is_empty())
                        .map(|p| godot::classes::ProjectSettings::singleton().globalize_path(&p).to_string());
                    let name = names.get(*s as usize).cloned().unwrap_or_else(|| format!("slot_{s}"));
                    (name, [c.r, c.g, c.b], tex)
                })
                .collect();
            write_text(path, &obj)?;
            let dir = &path[..path.len() - path.rsplit(['/', '\\']).next().map(|s| s.len()).unwrap_or(0)];
            write_text(&format!("{dir}{mtl_name}"), &convert::to_mtl(&mtl_entries))?;
            return Ok(format!("Exported {path}"));
        }
        if lower.ends_with(".glb") || lower.ends_with(".gltf") {
            // Lightmap UV2 at the preferences' texel (Unibuilder unwraps on export / bake too).
            let texel = self.prefs.lightmap_texel;
            let unwrapped = if texel > 0.0 { t.clone().bind_mut().unwrapped_mesh(texel) } else { None };
            let mesh: Gd<Mesh> = match unwrapped {
                Some(u) => u.upcast(),
                None => t.get_mesh().ok_or("mesh is empty")?,
            };
            let mut root = Node3D::new_alloc();
            root.set_name(&t.get_name());
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&t.get_name());
            mi.set_mesh(&mesh);
            root.add_child(&mi);
            // GLTFDocument only walks nodes owned by the exported root.
            mi.set_owner(&root);
            let mut doc = GltfDocument::new_gd();
            let state = GltfState::new_gd();
            let err = doc.append_from_scene(&root, &state);
            let res = if err == godot::global::Error::OK { doc.write_to_filesystem(&state, path) } else { err };
            root.free();
            return if res == godot::global::Error::OK { Ok(format!("Exported {path}")) } else { Err(format!("glTF export failed ({res:?})")) };
        }
        Err("unknown extension (use .obj, .glb or .gltf)".into())
    }

    /// Replaces the selected MeshInstance3D by an editable builder mesh (the original is
    /// hidden, one undo step).
    pub(super) fn convert_selected(&mut self) {
        let Some(src) = self.convert_src.clone() else {
            self.message = "Convert: select a MeshInstance3D".into();
            return;
        };
        let Some(mesh) = src.get_mesh() else {
            self.message = "Convert: the MeshInstance3D has no mesh".into();
            return;
        };
        let (m, mats) = mesh_to_bmesh(&mesh, &src);
        if m.faces.is_empty() {
            self.message = "Convert: no triangles found".into();
            return;
        }
        let (Some(parent), Some(root)) = (src.get_parent(), EditorInterface::singleton().get_edited_scene_root()) else { return };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut data = PixelBuilderMeshData::new_gd();
        data.bind_mut().replace(m);
        data.set("materials", &mats.to_variant());
        node.bind_mut().set_data(Some(data));
        node.set_name(&format!("{}Builder", src.get_name()));
        node.set_transform(src.get_transform());
        let mut ur = match self.base().get_undo_redo() {
            Some(u) => u,
            None => return,
        };
        ur.create_action("Pixel Builder: Convert to builder mesh");
        ur.add_do_method(&parent, "add_child", &[node.to_variant(), true.to_variant()]);
        ur.add_do_method(&parent, "move_child", &[node.to_variant(), (src.get_index() + 1).to_variant()]);
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_property(&src, "visible", &false.to_variant());
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.add_undo_property(&src, "visible", &src.is_visible().to_variant());
        ur.commit_action();
        self.message = format!("Converted {} (original hidden)", src.get_name());
        EditorInterface::singleton().edit_node(&node);
    }
}

/// Triangles of every surface of `mesh` -> editable mesh (+ surface materials as slots).
pub fn mesh_to_bmesh(mesh: &Gd<Mesh>, owner: &Gd<MeshInstance3D>) -> (BMesh, Array<Option<Gd<Material>>>) {
    let mut positions = Vec::new();
    let mut tris = Vec::new();
    let mut slots = Vec::new();
    let mut mats: Array<Option<Gd<Material>>> = Array::new();
    for s in 0..mesh.get_surface_count() {
        let arrays = mesh.surface_get_arrays(s);
        let Ok(verts) = arrays.at(ArrayType::VERTEX.ord() as usize).try_to::<PackedVector3Array>() else { continue };
        let base = positions.len() as u32;
        positions.extend(verts.as_slice().iter().map(|p| v3(p.x, p.y, p.z)));
        let idx: Vec<u32> = match arrays.at(ArrayType::INDEX.ord() as usize).try_to::<PackedInt32Array>() {
            Ok(i) if !i.is_empty() => i.as_slice().iter().map(|x| *x as u32).collect(),
            _ => (0..verts.len() as u32).collect(),
        };
        for t in idx.chunks_exact(3) {
            tris.push([base + t[0], base + t[1], base + t[2]]);
            slots.push(s as u16);
        }
        let over = if s < owner.get_surface_override_material_count() { owner.get_surface_override_material(s) } else { None };
        let mat = over.or_else(|| mesh.surface_get_material(s));
        mats.push(mat.as_ref());
    }
    (convert::from_triangles(&positions, &tris, &slots, 1e-4), mats)
}

fn write_text(path: &str, text: &str) -> Result<(), String> {
    let mut f = FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| format!("cannot write {path}"))?;
    f.store_string(text);
    f.close();
    Ok(())
}

use godot::classes::{Camera3D, EditorInterface, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion, Resource};

