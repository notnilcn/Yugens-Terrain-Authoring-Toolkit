use super::super::data::{PixelBuilderMeshData, to_p};
use super::super::ui::{Mode, UvField};
use super::{PixelBuilderPlugin, Placement, plane_hit, ui_color_bytes};
use crate::math_conversions::to_iv;
use godot::classes::base_material_3d::Flags;
use godot::classes::file_access::ModeFlags;
use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::mesh::ArrayType;
use godot::classes::{FileAccess, Material, Mesh, MeshInstance3D, StandardMaterial3D};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use pt_modeler::compile::CompileOptions;
use pt_modeler::model::UvParams;
use pt_modeler::{BMesh, ShapeKind, convert, selection_edits, pick, shapes, snap, topo};
use pt_core::math::{Vec3 as PVec3, v2, v3};
use std::collections::BTreeSet;

pub struct ShapeDrag {
    pub stage: u8,
    pub commence: Placement,
    pub corner: Vector3,
    pub stature: f32,
    pub press: Vector2,
    pub append_tail: bool,
    pub height_from: Vector2,
}
pub struct PaintStroke {
    pub before: PackedByteArray,
    pub painted: BTreeSet<usize>,
}
impl PixelBuilderPlugin {
    pub(super) fn sheet_dot(&self, cam: &Gd<Camera3D>, pos: Vector2, y: f32) -> Option<Vector3> {
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
            let q = if self.spec.sort.is_grid_shape() {
                snap::cell_center(cfg, v2(l.x, l.z)).1
            } else {
                snap::nearest_cell_point(cfg, v2(l.x, l.z), true)
            };
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
    pub(super) fn draw_point(
        &self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        origin: Vector3,
        basis: Basis,
    ) -> Option<Vector3> {
        if basis == Basis::IDENTITY {
            return self.sheet_dot(cam, pos, origin.y);
        }
        let hit = plane_hit(
            origin,
            basis.col_b(),
            cam.project_ray_origin(pos),
            cam.project_ray_normal(pos),
        )?;
        if !self.snap_grid {
            return Some(hit);
        }
        let mut d = basis.inverse() * (hit - origin);
        d.x = snap::snap_value(d.x, self.grid_step);
        d.z = snap::snap_value(d.z, self.grid_step);
        d.y = 0.0;
        Some(origin + basis * d)
    }
    pub(super) fn draw_basis(&self) -> Basis {
        if !self.wall_pts.is_empty() || self.poly_outline.is_some() {
            return self.wall_basis;
        }
        if let Some(sd) = &self.shape_drag {
            return sd.commence.frame;
        }
        self.hover_place.map(|p| p.frame).unwrap_or(Basis::IDENTITY)
    }
    pub(super) fn shape_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let Some(sd) = &self.shape_drag else {
                self.update_hover(cam, pos);
                return Some(false);
            };
            if sd.stage == 0 {
                let (start, basis) = (sd.commence.dot, sd.commence.frame);
                if let Some(c) = self.draw_point(cam, pos, start, basis) {
                    if let Some(sd) = &mut self.shape_drag {
                        sd.corner = c;
                    }
                }
            } else {
                let centre = (sd.commence.dot + sd.corner) * 0.5;
                let up = sd.commence.frame.col_b();
                let mut n = cam.get_global_transform().basis.col_c();
                n -= up * n.dot(up);
                let n = if n.length() < 1e-4 {
                    sd.commence.frame.col_c()
                } else {
                    n.normalized()
                };
                let o = cam.project_ray_origin(pos);
                let d = cam.project_ray_normal(pos);
                let from = sd.height_from;
                let mut h = match plane_hit(centre, n, o, d) {
                    Some(hit) => (hit - sd.commence.dot).dot(up),
                    None => (from.y - pos.y) * 0.0205,
                };
                if self.snap_grid {
                    h = snap::snap_value(h, self.grid_step);
                }
                let min = if self.snap_grid {
                    self.grid_step
                } else {
                    0.0505
                };
                if let Some(sd) = &mut self.shape_drag {
                    sd.stature = h.max(min);
                }
            }
            self.base().update_overlays();
            return Some(true);
        }
        let Ok(mb) = happening.clone().try_cast::<InputEventMouseButton>() else {
            return None;
        };
        let pos = mb.get_position();
        match mb.get_button_index() {
            MouseButton::RIGHT if mb.is_pressed() && self.shape_drag.is_some() => {
                self.cancel_shape();
                Some(true)
            }
            MouseButton::LEFT => {
                if mb.is_pressed() {
                    if self.shape_drag.as_ref().is_some_and(|s| s.stage == 1) {
                        self.seal_form();
                        return Some(true);
                    }
                    self.update_hover(cam, pos);
                    let Some(p) = self.hover_place else {
                        self.message = "Nothing under the cursor".into();
                        return Some(true);
                    };
                    let append = Input::singleton().is_key_pressed(Key::SHIFT);
                    self.shape_drag = Some(ShapeDrag {
                        stage: 0,
                        commence: p,
                        corner: p.dot,
                        stature: self.spec.size.y,
                        press: pos,
                        append_tail: append,
                        height_from: pos,
                    });
                } else if let Some(sd) = &self.shape_drag
                    && sd.stage == 0
                {
                    if (pos - sd.press).length() < 6.0
                        || (sd.corner - sd.commence.dot).length() < 1e-3
                    {
                        let append = sd.append_tail;
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
    pub(super) fn dragged_shape(&self) -> Option<(Vector3, BMesh)> {
        let sd = self.shape_drag.as_ref()?;
        let a = sd.commence.dot;
        let b = sd.corner;
        let mut spec = self.spec;
        let h = if sd.stage == 0 {
            spec.size.y
        } else {
            sd.stature
        };
        spec.size.y = h;
        if spec.sort.is_grid_shape() {
            let cw = self
                .terrain
                .as_ref()
                .map(|t| t.bind().store().config.cell_size_safe().x)
                .unwrap_or(1.0)
                .max(0.0105);
            let dist = Vector2::new(b.x - a.x, b.z - a.z).length();
            spec.rings = (dist / cw).round() as u32;
            if spec.sort == ShapeKind::HexPrism && spec.rings > 0 {
                spec.sort = ShapeKind::HexCluster;
            }
            let saved = (self.spec, spec);
            let m = {
                let tcfg = self
                    .terrain
                    .as_ref()
                    .map(|t| t.bind().store().config.clone())
                    .filter(|c| c.grid.is_cell_mode());
                match (&tcfg, sd.commence.cell) {
                    (Some(cfg), Some(c)) => {
                        let r = if spec.sort == ShapeKind::HexPrism {
                            0
                        } else {
                            spec.rings.min(16) as i32
                        };
                        let cells = shapes::cluster_cells(cfg, to_iv(c), r);
                        shapes::cells_prism(cfg, &cells, to_iv(c), h)
                    }
                    _ => shapes::produce(&saved.1, tcfg.as_ref()),
                }
            };
            return Some((a, m));
        }
        let dl = sd.commence.frame.inverse() * (b - a);
        let (dx, dz) = (dl.x, dl.z);
        let min = if self.snap_grid {
            self.grid_step
        } else {
            0.0505
        };
        let along_x = dx.abs() > dz.abs();
        let rises = matches!(spec.sort, ShapeKind::Stairs | ShapeKind::Wedge);
        let (w, d) = (dx.abs().max(min), dz.abs().max(min));
        spec.size.x = if rises && along_x { d } else { w };
        spec.size.z = if rises && along_x { w } else { d };
        let mut m = shapes::produce(&spec, None);
        if rises {
            let angle = if along_x {
                if dx > 0.0 {
                    std::f32::consts::FRAC_PI_2
                } else {
                    -std::f32::consts::FRAC_PI_2
                }
            } else if dz > 0.0 {
                0.0
            } else {
                std::f32::consts::PI
            };
            if angle != 0.0 {
                let all: BTreeSet<u32> = (0..m.positions.len() as u32).collect();
                topo::rotate_vertices(&mut m, &all, PVec3::ZERO, PVec3::UP, angle);
            }
        }
        let centre = a + sd.commence.frame * Vector3::new(dx * 0.5, 0.0, dz * 0.5);
        Some((centre, m))
    }
    pub(super) fn seal_form_exported(&mut self) {
        self.seal_form();
    }
    fn seal_form(&mut self) {
        let Some((origin, m)) = self.dragged_shape() else {
            return;
        };
        let Some(sd) = self.shape_drag.take() else {
            return;
        };
        let b = m.extent();
        if let Some((lo, hi)) = b
            && !self.spec.sort.is_grid_shape()
        {
            let rises = matches!(self.spec.sort, ShapeKind::Stairs | ShapeKind::Wedge);
            let (sx, sz) = (hi.x - lo.x, hi.z - lo.z);
            let along_x = rises
                && (sd.corner.x - sd.commence.dot.x).abs()
                    > (sd.corner.z - sd.commence.dot.z).abs();
            self.spec.size = if along_x {
                v3(sz, hi.y - lo.y, sx)
            } else {
                v3(sx, hi.y - lo.y, sz)
            };
        } else if let Some((lo, hi)) = b {
            self.spec.size.y = hi.y - lo.y;
        }
        let place = Placement {
            dot: origin,
            cell: sd.commence.cell,
            frame: sd.commence.frame,
        };
        self.create_from_mesh(place, m, sd.append_tail);
        self.sync_ui();
    }
    pub(super) fn paint_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        self.target.as_ref()?;
        if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let face = self.face_under(cam, pos);
            if face != self.hover_face {
                self.hover_face = face;
                self.redraw();
            }
            if self.coat.is_some()
                && let Some(f) = face
            {
                self.coat_facets(BTreeSet::from([f]));
            }
            return Some(self.coat.is_some());
        }
        let Ok(mb) = happening.clone().try_cast::<InputEventMouseButton>() else {
            return None;
        };
        if mb.get_button_index() != MouseButton::LEFT {
            return None;
        }
        let pos = mb.get_position();
        if mb.is_pressed() {
            let Some(f) = self.face_under(cam, pos) else {
                return Some(false);
            };
            let data = self.contents()?;
            let input = Input::singleton();
            if input.is_key_pressed(Key::CTRL) {
                let (slot, col, uv) = {
                    let d = data.bind();
                    let face = &d.cage().facets[f];
                    (face.stuff, face.tint, face.tex_coord)
                };
                self.pocket = slot as i64;
                self.tint = Color::from_rgba8(col[0], col[1], col[2], col[3]);
                self.load_uv(uv);
                self.message = format!("Picked slot {slot}");
                self.sync_ui();
                return Some(true);
            }
            self.coat = Some(PaintStroke {
                before: data.bind().get_blob(),
                painted: BTreeSet::new(),
            });
            let faces = if input.is_key_pressed(Key::SHIFT) {
                topo::coplanar_faces(data.bind().cage(), f, 1.0)
            } else {
                BTreeSet::from([f])
            };
            self.coat_facets(faces);
        } else if let Some(stroke) = self.coat.take()
            && let Some(data) = self.contents()
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
        self.contents().and_then(|data| {
            pick::intersect_ray_closest(data.bind().cage(), o, d, true).map(|h| h.face)
        })
    }
    fn coat_facets(&mut self, faces: BTreeSet<usize>) {
        let Some(mut data) = self.contents() else {
            return;
        };
        let Some(stroke) = &mut self.coat else { return };
        let new: BTreeSet<usize> = faces.difference(&stroke.painted).copied().collect();
        if new.is_empty() {
            return;
        }
        stroke.painted.extend(new.iter().copied());
        let (slot, col) = (self.pocket.clamp(0, 255) as u16, ui_color_bytes(self.tint));
        let mut m = data.bind().cage().clone();
        selection_edits::assign_stuff(&mut m, &new, slot);
        selection_edits::assign_tint(&mut m, &new, col);
        data.bind_mut().substitute(m);
        data.emit_changed();
        self.redraw();
    }
    pub(super) fn uv_params(&self) -> UvParams {
        UvParams {
            offset: v2(self.tex_coord[0], self.tex_coord[1]),
            magnify: v2(self.tex_coord[2], self.tex_coord[3]),
            rotation: self.tex_coord[4],
            world: self.uv_world,
        }
    }
    pub(super) fn load_uv(&mut self, tex_coord: UvParams) {
        self.tex_coord = [
            tex_coord.offset.x,
            tex_coord.offset.y,
            tex_coord.magnify.x,
            tex_coord.magnify.y,
            tex_coord.rotation,
        ];
        self.uv_world = tex_coord.world;
    }
    pub(super) fn load_face_props(&mut self) {
        if self.mode != Mode::Face || self.sel_faces.len() != 1 {
            return;
        }
        let Some(data) = self.contents() else { return };
        let f = *self.sel_faces.first().unwrap_or(&0);
        let Some((slot, col, uv)) = data
            .bind()
            .cage()
            .facets
            .get(f)
            .map(|x| (x.stuff, x.tint, x.tex_coord))
        else {
            return;
        };
        self.pocket = slot as i64;
        self.tint = Color::from_rgba8(col[0], col[1], col[2], col[3]);
        self.load_uv(uv);
        self.sync_ui();
    }
    pub(super) fn apply_uv(&mut self, tex_coord: UvParams) {
        self.apply_edit("Pixel Builder: Face UV", |m, s| {
            let f = s.every_or_picked_facets(m);
            selection_edits::set_uv(m, &f, tex_coord);
        });
    }
    pub(super) fn rotate_uv(&mut self) {
        self.apply_edit("Pixel Builder: Rotate UV", |m, s| {
            for f in s.every_or_picked_facets(m) {
                if let Some(face) = m.facets.get_mut(f) {
                    face.tex_coord.rotation = (face.tex_coord.rotation + 90.0) % 360.0;
                }
            }
        });
        self.load_face_props();
    }
    pub(super) fn uv_field(&mut self, f: UvField, v: f32) {
        self.tex_coord[f as usize] = v;
    }
    pub(super) fn pick_slot(&mut self, slot: i64) {
        self.pocket = slot;
        if self.mode == Mode::Face && !self.sel_faces.is_empty() {
            let s = slot.clamp(0, 255) as u16;
            self.apply_edit("Pixel Builder: Assign material", |m, st| {
                selection_edits::assign_stuff(m, &st.sel_faces, s)
            });
        }
        self.sync_ui();
    }
    pub(super) fn assign_pocket_stuff(&mut self, res: Option<Gd<Resource>>, slot: i64) {
        let Some(data) = self.contents() else { return };
        let old = data.bind().stuff_pockets();
        let mut new = old.duplicate_shallow();
        let mat = res.and_then(|r| r.try_cast::<Material>().ok());
        while (new.len() as i64) <= slot {
            new.push(None::<&Gd<Material>>);
        }
        new.set(slot as usize, mat.as_ref());
        self.commit_materials("Pixel Builder: Slot material", &data, old, new);
    }
    pub(super) fn insert_pocket(&mut self) {
        let Some(data) = self.contents() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let old = data.bind().stuff_pockets();
        let mut new = old.duplicate_shallow();
        let mut m = StandardMaterial3D::new_gd();
        m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        m.set_roughness(1.0);
        let hues = [0.0805, 0.3005, 0.5805, 0.0, 0.7505, 0.1505, 0.4505];
        let k = new.len();
        m.set_albedo(Color::from_hsv(
            hues[k % hues.len()] as f64,
            0.3505,
            0.8505 - 0.1005 * ((k / hues.len()) % 3) as f64,
        ));
        let mat: Gd<Material> = m.upcast();
        new.push(Some(&mat));
        self.pocket = k as i64;
        self.commit_materials("Pixel Builder: Add slot", &data, old, new);
    }
    pub(super) fn commit_materials(
        &mut self,
        name: &str,
        data: &Gd<PixelBuilderMeshData>,
        old: Array<Option<Gd<Material>>>,
        new: Array<Option<Gd<Material>>>,
    ) {
        let Some(mut ur) = self.open_action(name) else {
            return;
        };
        ur.add_do_property(data, "materials", &new.to_variant());
        ur.add_undo_property(data, "materials", &old.to_variant());
        ur.commit_action();
        self.palette_slots = None;
        self.sync_ui();
    }
    pub(super) fn refresh_palette(&mut self) {
        let groups = self.gliding_clusters();
        let chip_key = Some((groups.clone(), self.xg.smooth_group));
        if self.palette_slots.is_none() || self.chip_key != chip_key {
            self.chip_key = chip_key;
            let cur = self.xg.smooth_group;
            if let Some(super::LayoutUi::Mst(u)) = &mut self.ui {
                u.rebuild_chips(&groups, cur);
            }
        }
        let cur = self.xg.smooth_group;
        if let Some(super::LayoutUi::Mst(u)) = &mut self.ui {
            u.sync_chips(cur);
        }
        let mats: Vec<Option<Gd<Material>>> = self
            .contents()
            .map(|d| d.bind().stuff_pockets().iter_shared().collect())
            .unwrap_or_default();
        let ids: Vec<i64> = mats
            .iter()
            .map(|m| m.as_ref().map(|m| m.instance_id().to_i64()).unwrap_or(0))
            .collect();
        if self.palette_slots.as_ref() == Some(&ids) {
            return;
        }
        self.palette_slots = Some(ids);
        let slot = self.pocket;
        if let Some(u) = &mut self.ui {
            u.rebuild_slots(&mats, slot);
        }
    }
    pub(super) fn perform_pull_out_ridges(&mut self) {
        let (dist, group) = (self.span, self.xg.edges_group);
        self.apply_edit("Pixel Builder: Extrude edges", |m, s| {
            let out = if group {
                topo::pull_out_exposed_ridges(m, &s.sel_edges, dist, PVec3::ZERO)
            } else {
                pt_modeler::xg_tools::pull_out_ridges_grouped(
                    m,
                    &s.sel_edges,
                    dist,
                    PVec3::ZERO,
                    false,
                )
            };
            if out.is_empty() {
                s.message = "Extrude edges: only open (boundary) edges extrude".into();
            }
            s.sel_edges = out;
        });
    }
    pub(super) fn act_merge(&mut self) {
        match self.mode {
            Mode::Face => {
                self.apply_edit(
                    "Pixel Builder: Merge faces",
                    |m, s| match topo::fuse_facets(m, &s.sel_faces) {
                        Some(f) => s.sel_faces = BTreeSet::from([f]),
                        None => {
                            s.message =
                                "Merge: select two or more connected faces with one outline".into()
                        }
                    },
                )
            }
            Mode::Vertex if self.sel_verts.len() > 1 => {
                self.apply_edit("Pixel Builder: Merge vertices", |m, s| {
                    let v = s.sel_verts.clone();
                    let kept = selection_edits::merge_vertices(m, &v);
                    s.purge_picked();
                    s.sel_verts.extend(kept);
                })
            }
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
        if !self.xg.fill_entire {
            let sel = match self.mode {
                Mode::Edge => pt_modeler::xg_tools::HoleSel::Edges(self.sel_edges.clone()),
                _ => pt_modeler::xg_tools::HoleSel::Vertices(self.sel_verts.clone()),
            };
            self.apply_edit("Pixel Builder: Fill hole", |m, s| {
                match pt_modeler::xg_tools::flood_picked_cavities(m, &sel, false) {
                    Ok(f) if f.is_empty() => {
                        s.message = "Fill: select three or more border vertices of one hole".into()
                    }
                    Ok(_) => {}
                    Err(e) => s.message = format!("Fill: {e}"),
                }
            });
            return;
        }
        self.apply_edit("Pixel Builder: Fill hole", |m, s| {
            let verts = s.selected_verts(m);
            let only = if verts.is_empty() { None } else { Some(&verts) };
            let faces = topo::flood_cavities(m, only);
            if faces.is_empty() {
                s.message = "Fill: no open boundary through the selection".into();
            }
        });
    }
    pub(super) fn act_subdivide(&mut self) {
        if self.mode == Mode::Edge {
            self.perform_refine_ridges();
            return;
        }
        if self.mode == Mode::Face && self.sel_faces.is_empty() {
            self.message =
                "Subdivide: select faces (Face mode), or use it in Object mode for the whole mesh"
                    .into();
            return;
        }
        let name = if self.mode == Mode::Object {
            "Pixel Builder: Subdivide object"
        } else {
            "Pixel Builder: Subdivide"
        };
        let segs = self.opts.subdiv_segs.clamp(2, 16) as u32;
        self.apply_edit(name, |m, s| {
            let f = s.every_or_picked_facets(m);
            let out = pt_modeler::mesh_ops::subdivide_grid(m, &f, segs);
            if s.mode == Mode::Face {
                s.sel_faces = out;
            } else {
                s.purge_picked();
            }
        });
    }
    pub(super) fn act_rotate(&mut self, angle_deg: f32) {
        let Some(pivot) = self.fulcrum() else {
            self.message = "Rotate: select something first".into();
            return;
        };
        let local = to_p(self.xform().affine_inverse() * pivot);
        self.apply_edit("Pixel Builder: Rotate", |m, s| {
            let v = s.selected_verts(m);
            topo::rotate_vertices(m, &v, local, PVec3::UP, angle_deg.to_radians());
        });
    }
    pub(super) fn act_select(&mut self, which: u8) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        let angle = self.xg.grow_angle;
        match (which, self.mode) {
            (0, Mode::Face) if angle > 0.0 => {
                self.sel_faces =
                    pt_modeler::xg_tools::expand_facets_via_inclination(m, &self.sel_faces, angle)
            }
            (0, Mode::Edge) if angle > 0.0 => {
                self.sel_edges =
                    pt_modeler::xg_tools::expand_ridges_via_inclination(m, &self.sel_edges, angle)
            }
            (0, Mode::Face) => self.sel_faces = m.expand_facets(&self.sel_faces),
            (0, Mode::Edge) => {
                let v = m.verts_of_edges(&self.sel_edges);
                let faces: BTreeSet<usize> = (0..m.facets.len())
                    .filter(|f| m.facets[*f].verts.iter().any(|x| v.contains(x)))
                    .collect();
                self.sel_edges = m.edges_of_faces(&faces);
            }
            (0, Mode::Vertex) => {
                let faces: BTreeSet<usize> = (0..m.facets.len())
                    .filter(|f| {
                        m.facets[*f]
                            .verts
                            .iter()
                            .any(|x| self.sel_verts.contains(x))
                    })
                    .collect();
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
    pub(super) fn unveil_ship_out(&mut self, gltf: bool) {
        let Some(t) = self.target.clone() else { return };
        let mut dlg = match self.export_dialog.clone().filter(|d| d.is_instance_valid()) {
            Some(d) => d,
            None => {
                let d = super::editor_dialogs::file_dialog(
                    None,
                    Some(FileMode::SAVE_FILE),
                    Access::FILESYSTEM,
                    &self.to_gd().callable("on_export_file"),
                );
                self.export_dialog = Some(d.clone());
                d
            }
        };
        dlg.clear_filters();
        if gltf {
            dlg.add_filter_ex("*.glb").description("glTF binary").done();
            dlg.add_filter_ex("*.gltf").description("glTF text").done();
        } else {
            dlg.add_filter_ex("*.obj")
                .description("Wavefront OBJ")
                .done();
        }
        dlg.set_title(if gltf {
            "Export builder mesh (glTF)"
        } else {
            "Export builder mesh (OBJ)"
        });
        dlg.set_current_file(&format!(
            "{}.{}",
            t.get_name(),
            if gltf { "glb" } else { "obj" }
        ));
        dlg.popup_file_dialog();
    }
    pub(super) fn ship_out_into(&mut self, path: &str) -> Result<String, String> {
        let t = self.target.clone().ok_or("no builder mesh selected")?;
        let data = t.bind().contents().ok_or("mesh has no data")?;
        let lower = path.to_lowercase();
        if lower.ends_with(".res") || lower.ends_with(".tres") {
            return self.export_res(path);
        }
        if lower.ends_with(".stl")
            || lower.ends_with(".ply")
            || (lower.ends_with(".obj") && !self.export_opts.classic_obj())
        {
            return self.export_xg(path);
        }
        if lower.ends_with(".obj") {
            let stem = path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("mesh")
                .trim_end_matches(".obj")
                .to_string();
            let mtl_name = format!("{stem}.mtl");
            let mats: Vec<Option<Gd<Material>>> =
                data.bind().stuff_pockets().iter_shared().collect();
            let names: Vec<String> = (0..mats.len().max(1))
                .map(|i| format!("{stem}_slot{i}"))
                .collect();
            let opt = CompileOptions {
                units_per_repeat: data.bind().units(),
                world_offset: to_p(t.get_global_position()),
            };
            let obj = convert::to_obj(
                data.bind().cage(),
                &opt,
                &t.get_name().to_string(),
                Some(&mtl_name),
                &names,
            );
            let used: BTreeSet<u16> = data.bind().cage().facets.iter().map(|f| f.stuff).collect();
            let mtl_entries = super::xg_io::mtl_entries(&mats, &used, &names);
            write_text(path, &obj)?;
            let dir = &path[..path.len()
                - path
                    .rsplit(['/', '\\'])
                    .next()
                    .map(|s| s.len())
                    .unwrap_or(0)];
            write_text(&format!("{dir}{mtl_name}"), &convert::to_mtl(&mtl_entries))?;
            return Ok(format!("Exported {path}"));
        }
        if lower.ends_with(".glb") || lower.ends_with(".gltf") {
            let texel = self.prefs.lightmap_texel;
            pt_modeler::lightmap::check_atlas(
                data.bind().cage(),
                to_p(t.get_global_transform().basis.get_scale()),
                texel,
            )?;
            let (mesh, count) = self.export_mesh()?;
            let res = super::gltf_export::write_gltf(&t.get_name(), &mesh.upcast(), path);
            return if res == godot::global::Error::OK {
                Ok(if count > 1 {
                    format!("Exported {path} ({count} meshes)")
                } else {
                    format!("Exported {path}")
                })
            } else {
                Err(format!("glTF export failed ({res:?})"))
            };
        }
        Err("unknown extension (use .obj, .glb, .gltf, .stl, .ply, .res or .tres)".into())
    }
}
pub fn mesh_to_bmesh(
    mesh: &Gd<Mesh>,
    owner: Option<&Gd<MeshInstance3D>>,
) -> (BMesh, Array<Option<Gd<Material>>>) {
    let mut positions = Vec::new();
    let mut tris = Vec::new();
    let mut slots = Vec::new();
    let mut mats: Array<Option<Gd<Material>>> = Array::new();
    for s in 0..mesh.get_surface_count() {
        let arrays = mesh.surface_get_arrays(s);
        let Ok(verts) = arrays
            .at(ArrayType::VERTEX.ord() as usize)
            .try_to::<PackedVector3Array>()
        else {
            continue;
        };
        let base = positions.len() as u32;
        positions.extend(verts.as_slice().iter().map(|p| v3(p.x, p.y, p.z)));
        let idx: Vec<u32> = match arrays
            .at(ArrayType::INDEX.ord() as usize)
            .try_to::<PackedInt32Array>()
        {
            Ok(i) if !i.is_empty() => i.as_slice().iter().map(|x| *x as u32).collect(),
            _ => (0..verts.len() as u32).collect(),
        };
        for t in idx.chunks_exact(3) {
            tris.push([base + t[0], base + t[1], base + t[2]]);
            slots.push(s as u16);
        }
        let over = owner
            .filter(|o| s < o.get_surface_override_material_count())
            .and_then(|o| o.get_surface_override_material(s));
        let mat = over.or_else(|| mesh.surface_get_material(s));
        mats.push(mat.as_ref());
    }
    (
        convert::from_triangles(&positions, &tris, &slots, 1e-4),
        mats,
    )
}
fn write_text(path: &str, text: &str) -> Result<(), String> {
    let mut f =
        FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| format!("cannot write {path}"))?;
    f.store_string(text);
    f.close();
    Ok(())
}
use godot::classes::{
    Camera3D, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion, Resource,
};
