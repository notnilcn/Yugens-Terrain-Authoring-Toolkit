//! Builder plugin, part 3 — ports of Unibuilder's editor tools (MIT, Leafshade Consulting Ltd):
//!
//! * rotate / scale handles: `editor/rotate_gizmo.gd`, `editor/scale_gizmo.gd` and
//!   `editor/gizmo_math.gd` (screen arms, edge-on rings, accumulated wrapped angles);
//! * soft selection and vertex snap while dragging: `editor/viewport_input.gd`
//!   (`_snap_setup`, `_snap_world_offset`) with `core/soft_select.gd` weights;
//! * the knife: `editor/cut_tool.gd` (pick a face, click points, close / Enter commits);
//! * loop / ring selection on double-click: `viewport_input.gd::_handle_double_click`;
//! * edge-loop insert, bridge, object mirror, shape re-edit and CSG booleans:
//!   `plugin.gd` (`insert_loop`, `bridge`, `mirror_x/y/z`, `shape_edit_requested`,
//!   `_run_boolean`) with `core/ops/boolean_objects.gd`.

use std::collections::BTreeSet;

use godot::classes::csg_shape_3d::Operation;
use godot::classes::object::ConnectFlags;
use godot::classes::{
    ArrayMesh, Camera3D, Control, CsgMesh3D, EditorInterface, Input, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion, Material,
    Mesh, StandardMaterial3D,
};
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::compile::{CompileOptions, compile};
use pt_core::build::xform::{self, Falloff, Frame, Weights};
use pt_core::build::{BMesh, Edge, ShapeSpec, convert, cut, edge, loops, pick};
use pt_core::math::Vec3 as PVec3;

use super::super::data::{PixelBuilderMeshData, from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::{Mode, Tool, XformMode};
use super::PixelBuilderPlugin;
use crate::util::*;

const AXIS_COLORS: [Color; 3] = [Color::from_rgb(0.96, 0.3, 0.33), Color::from_rgb(0.53, 0.84, 0.28), Color::from_rgb(0.3, 0.55, 0.96)];
fn radius_px() -> f32 {
    64.0 * crate::builder::prefs::handle_scale()
}
fn hit_px() -> f32 {
    10.0 * crate::builder::prefs::handle_scale()
}
const SEGMENTS: usize = 48;
/// Below this |axis . ray| a rotation ring counts as edge-on (screen-angle mode).
const EDGE_ON: f32 = 0.08;
fn axis_len_px() -> f32 {
    64.0 * crate::builder::prefs::handle_scale()
}
fn tip_px() -> f32 {
    7.0 * crate::builder::prefs::handle_scale()
}
fn center_px() -> f32 {
    10.0 * crate::builder::prefs::handle_scale()
}
const UNIFORM_DIR_PX: f32 = 3.0;
const MIN_FACTOR: f32 = 0.001;
const ARM_HIDE_FRAC: f32 = 0.12;
const ARM_SOLID_FRAC: f32 = 0.35;
const CLOSE_PX: f32 = 14.0;
const ELEMENT_SNAP_PX: f32 = 12.0;
const KNIFE: Color = Color::from_rgb(0.369, 0.918, 0.831);
const AMBER: Color = Color::from_rgb(0.984, 0.749, 0.141);

/// A grabbed transform handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handle {
    /// Move arrow (`None` = centre square, camera plane).
    Move(Option<usize>),
    /// Rotation ring around a world axis.
    Rotate(usize),
    /// Scale arm (`None` = centre square, uniform).
    Scale(Option<usize>),
}

/// Live rotate drag (UBRotateGizmo state).
pub struct RotDrag {
    axis: Vector3,
    tangent: Vector3,
    bitangent: Vector3,
    screen_mode: bool,
    prev_raw: f32,
    unsnapped: f32,
    pub last: f32,
}

/// Live scale drag (UBScaleGizmo state).
pub struct ScaleDrag {
    axis: Option<usize>,
    t0: f32,
    press: Vector2,
    dir: Vector2,
    pub factors: Vector3,
}

/// Knife state (UBCutTool).
#[derive(Default)]
pub struct Knife {
    pub face: Option<usize>,
    normal: PVec3,
    point: PVec3,
    pub points: Vec<PVec3>,
    cursor: Option<PVec3>,
    snapped: bool,
}

/// A CSG boolean waiting for the CSG node to update (one or two frames).
pub struct PendingBool {
    root: Gd<CsgMesh3D>,
    target: Gd<PixelBuilderMesh>,
    src: Gd<PixelBuilderMesh>,
    name: String,
    tries: u8,
    /// Placeholder material per tag -> (slot, color).
    tags: Vec<(Gd<StandardMaterial3D>, u16, [u8; 4])>,
    materials: Array<Option<Gd<Material>>>,
}


fn closest_on_segment_2d(p: Vector2, a: Vector2, b: Vector2) -> Vector2 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 < 1e-12 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}

/// `UBGizmoMath.ray_line_t`: parameter t of the point on `pivot + axis t` closest to the ray.
fn ray_line_t(cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3, axis: Vector3) -> Option<f32> {
    let o = cam.project_ray_origin(pos);
    let d = cam.project_ray_normal(pos);
    let w = pivot - o;
    let b = axis.dot(d);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-6 {
        return None;
    }
    Some((b * d.dot(w) - axis.dot(w)) / denom)
}

/// `UBGizmoMath.screen_arm`: screen vector of a gizmo arm, foreshortened by perspective.
fn screen_arm(cam: &Gd<Camera3D>, pivot: Vector3, axis: Vector3, len_px: f32) -> Vector2 {
    let k = (cam.get_global_position().distance_to(pivot) * 0.01).max(0.001);
    let probe = pivot + axis.normalized() * k;
    if cam.is_position_behind(probe) {
        return Vector2::ZERO;
    }
    let at = cam.unproject_position(pivot);
    let across = cam.unproject_position(pivot + cam.get_global_transform().basis.col_a() * k).distance_to(at);
    if across < 1e-6 {
        return Vector2::ZERO;
    }
    let v = (cam.unproject_position(probe) - at) * (len_px / across);
    if v.length() >= len_px * ARM_HIDE_FRAC { v } else { Vector2::ZERO }
}

fn arm_alpha(arm: Vector2, len_px: f32) -> f32 {
    ((arm.length() / len_px - ARM_HIDE_FRAC) / (ARM_SOLID_FRAC - ARM_HIDE_FRAC)).clamp(0.0, 1.0)
}

fn world_radius(cam: &Gd<Camera3D>, pivot: Vector3) -> f32 {
    let right = cam.get_global_transform().basis.col_a();
    let px = cam.unproject_position(pivot + right).distance_to(cam.unproject_position(pivot));
    radius_px() / px.max(1e-3)
}

fn ring_frame(axis: Vector3) -> (Vector3, Vector3) {
    let axis = axis.normalized();
    let r = if axis.x.abs() < 0.9 { Vector3::RIGHT } else { Vector3::UP };
    let t = (r - axis * r.dot(axis)).normalized();
    (t, axis.cross(t))
}

fn circle_runs(cam: &Gd<Camera3D>, pivot: Vector3, axis: Vector3, r: f32) -> Vec<Vec<Vector2>> {
    let (t, b) = ring_frame(axis);
    let mut runs = Vec::new();
    let mut cur: Vec<Vector2> = Vec::new();
    for k in 0..=SEGMENTS {
        let a = std::f32::consts::TAU * k as f32 / SEGMENTS as f32;
        let w = pivot + (t * a.cos() + b * a.sin()) * r;
        if cam.is_position_behind(w) {
            if cur.len() > 1 {
                runs.push(std::mem::take(&mut cur));
            }
            cur.clear();
            continue;
        }
        cur.push(cam.unproject_position(w));
    }
    if cur.len() > 1 {
        runs.push(cur);
    }
    runs
}

fn readout(overlay: &mut Gd<Control>, at: Vector2, text: &str) {
    let Some(font) = overlay.get_theme_default_font() else { return };
    let p = at + Vector2::new(16.0, -12.0);
    overlay.draw_string_ex(&font, p + Vector2::new(1.0, 1.0), text).modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8)).done();
    overlay.draw_string_ex(&font, p, text).modulate(Color::WHITE).done();
}

impl PixelBuilderPlugin {
    // ================================================================== handles

    /// Handle under the mouse for the current transform mode.
    pub(super) fn gizmo_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Handle> {
        let axes_w = self.gizmo_axes();
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return None;
        }
        match self.xform_mode {
            XformMode::Move => self.handle_at(cam, pos).map(Handle::Move),
            XformMode::Rotate => {
                let pivot = self.pivot()?;
                if cam.is_position_behind(pivot) {
                    return None;
                }
                let r = world_radius(cam, pivot);
                let mut best = None;
                let mut best_d = hit_px();
                for (i, a) in axes_w.iter().enumerate() {
                    for run in circle_runs(cam, pivot, *a, r) {
                        for k in 0..run.len() - 1 {
                            let d = closest_on_segment_2d(pos, run[k], run[k + 1]).distance_to(pos);
                            if d < best_d {
                                best_d = d;
                                best = Some(Handle::Rotate(i));
                            }
                        }
                    }
                }
                best
            }
            XformMode::Scale => {
                let pivot = self.pivot()?;
                if cam.is_position_behind(pivot) {
                    return None;
                }
                let origin = cam.unproject_position(pivot);
                if pos.distance_to(origin) <= center_px() {
                    return Some(Handle::Scale(None));
                }
                let mut best = None;
                let mut best_d = hit_px();
                for (i, a) in axes_w.iter().enumerate() {
                    let arm = screen_arm(cam, pivot, *a, axis_len_px());
                    if arm == Vector2::ZERO {
                        continue;
                    }
                    let d = closest_on_segment_2d(pos, origin, origin + arm).distance_to(pos);
                    if d < best_d {
                        best_d = d;
                        best = Some(Handle::Scale(Some(i)));
                    }
                }
                best
            }
        }
    }

    /// Draws the handles of the current transform mode.
    pub(super) fn draw_gizmo(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        let axes_w = self.gizmo_axes();
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return;
        }
        let active = self.drag.as_ref().and_then(|d| d.handle);
        let hot = |h: Handle| active == Some(h) || (active.is_none() && self.hover_handle == Some(h));
        match self.xform_mode {
            XformMode::Move => {
                let Some((s0, ends)) = self.handle_screen(cam) else { return };
                for (a, e) in ends.iter().enumerate() {
                    let h = hot(Handle::Move(Some(a)));
                    let c = if h { AXIS_COLORS[a].lightened(0.45) } else { AXIS_COLORS[a] };
                    overlay.draw_line_ex(s0, *e, c).width(if h { 3.0 } else { 2.0 }).done();
                    let dir = (*e - s0).normalized_or_zero();
                    let side = Vector2::new(-dir.y, dir.x) * 4.0;
                    let tip = PackedVector2Array::from([*e + dir * 8.0, *e + side, *e - side]);
                    overlay.draw_colored_polygon(&tip, c);
                }
                let h = hot(Handle::Move(None));
                overlay
                    .draw_rect_ex(Rect2::new(s0 - Vector2::new(5.0, 5.0), Vector2::new(10.0, 10.0)), Color::from_rgba(1.0, 1.0, 1.0, 0.9))
                    .filled(h)
                    .width(1.5)
                    .done();
                if let Some(d) = &self.drag {
                    let v = d.last_delta;
                    readout(overlay, d.cursor, &format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z));
                    if let Some(s) = d.snap_hit.filter(|s| !cam.is_position_behind(*s)) {
                        let p = cam.unproject_position(s);
                        overlay.draw_rect_ex(Rect2::new(p - Vector2::new(6.0, 6.0), Vector2::new(12.0, 12.0)), AMBER).filled(false).width(2.0).done();
                    }
                }
            }
            XformMode::Rotate => {
                let Some(pivot) = self.drag.as_ref().map(|d| d.pivot).or_else(|| self.pivot()) else { return };
                if cam.is_position_behind(pivot) {
                    return;
                }
                let r = world_radius(cam, pivot);
                for (i, a) in axes_w.iter().enumerate() {
                    let h = hot(Handle::Rotate(i));
                    if active.is_some() && !h {
                        continue;
                    }
                    let c = if h { AXIS_COLORS[i].lightened(0.45) } else { AXIS_COLORS[i] };
                    for run in circle_runs(cam, pivot, *a, r) {
                        for k in 0..run.len() - 1 {
                            overlay.draw_line_ex(run[k], run[k + 1], c).width(if h { 3.0 } else { 2.0 }).antialiased(true).done();
                        }
                    }
                }
                if let Some(d) = &self.drag
                    && let Some(rd) = &d.rot
                {
                    readout(overlay, d.cursor, &format!("{:.1}°", rd.last.to_degrees()));
                }
            }
            XformMode::Scale => {
                let Some(pivot) = self.drag.as_ref().map(|d| d.pivot).or_else(|| self.pivot()) else { return };
                if cam.is_position_behind(pivot) {
                    return;
                }
                let origin = cam.unproject_position(pivot);
                for (i, a) in axes_w.iter().enumerate() {
                    let arm = screen_arm(cam, pivot, *a, axis_len_px());
                    if arm == Vector2::ZERO {
                        continue;
                    }
                    let h = hot(Handle::Scale(Some(i)));
                    let mut c = if h { AXIS_COLORS[i].lightened(0.45) } else { AXIS_COLORS[i] };
                    if !h {
                        c.a = arm_alpha(arm, axis_len_px());
                    }
                    let tip = origin + arm;
                    overlay.draw_line_ex(origin, tip, c).width(if h { 3.5 } else { 2.0 }).antialiased(true).done();
                    let half = (tip_px() + if h { 2.0 } else { 0.0 }) * 0.5;
                    overlay.draw_rect(Rect2::new(tip - Vector2::new(half, half), Vector2::new(half, half) * 2.0), c);
                }
                let ch = hot(Handle::Scale(None));
                let rect = Rect2::new(origin - Vector2::new(center_px(), center_px()) * 0.5, Vector2::new(center_px(), center_px()));
                if ch {
                    overlay.draw_rect(rect, Color::from_rgba(1.0, 1.0, 1.0, 0.35));
                }
                overlay.draw_rect_ex(rect, Color::from_rgba(1.0, 1.0, 1.0, 0.9)).filled(false).width(if ch { 3.0 } else { 2.0 }).done();
                if let Some(d) = &self.drag
                    && let Some(sd) = &d.scale
                {
                    let f = match sd.axis {
                        Some(0) => sd.factors.x,
                        Some(1) => sd.factors.y,
                        Some(_) => sd.factors.z,
                        None => sd.factors.x,
                    };
                    readout(overlay, d.cursor, &format!("× {f:.3}"));
                }
            }
        }
    }

    /// Soft-selection weights for a drag of `verts` (hard weights when soft selection is off).
    pub(super) fn drag_weights(&self, m: &BMesh, verts: &BTreeSet<u32>) -> Weights {
        if !self.soft {
            return xform::hard_weights(verts);
        }
        let xf = self.xform();
        xform::soft_weights(&m.positions, verts, self.soft_radius, &|p| to_p(xf * from_p(p)), Falloff::from_i64(self.falloff))
    }

    /// Vertex-snap setup (`_snap_setup`): anchor = moved vertex nearest the cursor; targets =
    /// every other vertex of this mesh plus all vertices of the other builder meshes (world).
    pub(super) fn snap_setup(&self, cam: &Gd<Camera3D>, cursor: Vector2, m: &BMesh, moved: &Weights, pivot: Vector3) -> (Vector3, Vec<Vector3>) {
        let xf = self.xform();
        let mut anchor = pivot;
        let mut best = f32::INFINITY;
        for v in moved.keys() {
            let w = xf * from_p(m.positions[*v as usize]);
            if cam.is_position_behind(w) {
                continue;
            }
            let d = cam.unproject_position(w).distance_to(cursor);
            if d < best {
                best = d;
                anchor = w;
            }
        }
        let mut targets: Vec<Vector3> = (0..m.positions.len() as u32).filter(|v| !moved.contains_key(v)).map(|v| xf * from_p(m.positions[v as usize])).collect();
        if let Some(root) = EditorInterface::singleton().get_edited_scene_root() {
            for n in root.find_children_ex("*").type_("PixelBuilderMesh").owned(false).done().iter_shared() {
                let Ok(b) = n.try_cast::<PixelBuilderMesh>() else { continue };
                if self.target.as_ref().is_some_and(|t| t.instance_id() == b.instance_id()) {
                    continue;
                }
                let Some(d) = b.bind().data() else { continue };
                let bx = b.get_global_transform();
                targets.extend(d.bind().mesh().positions.iter().map(|p| bx * from_p(*p)));
            }
        }
        (anchor, targets)
    }

    /// `_snap_world_offset`: world offset that puts the anchor on the target under the cursor.
    pub(super) fn snap_offset(&mut self, cam: &Gd<Camera3D>, cursor: Vector2) -> Option<Vector3> {
        let drag = self.drag.as_mut()?;
        let on = self.snap_vertex || Input::singleton().is_key_pressed(Key::V);
        if !on || drag.snap_targets.is_empty() {
            drag.snap_hit = None;
            return None;
        }
        let pts: Vec<PVec3> = drag.snap_targets.iter().map(|w| to_p(*w)).collect();
        let proj = |p: PVec3| {
            let w = from_p(p);
            (!cam.is_position_behind(w)).then(|| to_v2(cam.unproject_position(w)))
        };
        let idx = xform::nearest_target(&pts, &proj, to_v2(cursor), xform::SNAP_PX);
        drag.snap_hit = idx.map(|i| drag.snap_targets[i]);
        drag.snap_hit.map(|t| t - drag.snap_anchor)
    }

    pub(super) fn begin_rotate(&self, cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3, axis_i: usize) -> RotDrag {
        let axes_w = self.gizmo_axes();
        let axis = axes_w[axis_i];
        let (tangent, bitangent) = ring_frame(axis);
        let screen_mode = axis.dot(cam.project_ray_normal(pos)).abs() < EDGE_ON;
        let mut rd = RotDrag { axis, tangent, bitangent, screen_mode, prev_raw: 0.0, unsnapped: 0.0, last: 0.0 };
        rd.prev_raw = raw_angle(&rd, cam, pos, pivot);
        rd
    }

    pub(super) fn begin_scale(&self, cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3, axis: Option<usize>) -> ScaleDrag {
        let axes_w = self.gizmo_axes();
        let t0 = axis.and_then(|a| ray_line_t(cam, pos, pivot, axes_w[a])).filter(|t| t.abs() >= 1e-4).unwrap_or(1e-4);
        ScaleDrag { axis, t0, press: pos, dir: Vector2::ZERO, factors: Vector3::ONE }
    }

    /// Rotate / scale drag motion: recomputes the mesh from the drag's start snapshot.
    pub(super) fn drag_rotate_scale(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let axes_w = self.gizmo_axes();
        let snap = self.snap_grid;
        let rot_step = if snap { self.rot_snap.to_radians() } else { 0.0 };
        let scale_step = if snap { 0.1 } else { 0.0 };
        let xf = self.xform();
        let inv_basis = xf.basis.inverse();
        let Some(drag) = self.drag.as_mut() else { return };
        drag.cursor = pos;
        let pivot = drag.pivot;
        let local_pivot = to_p(xf.affine_inverse() * pivot);
        let mut m = drag.start.clone();
        if let Some(rd) = drag.rot.as_mut() {
            let raw = raw_angle(rd, cam, pos, pivot);
            rd.unsnapped += xform::wrap_pi(raw - rd.prev_raw);
            rd.prev_raw = raw;
            let angle = xform::snap_step(rd.unsnapped, rot_step);
            if angle == rd.last && rd.last != 0.0 {
                return;
            }
            rd.last = angle;
            let axis = to_p((inv_basis * rd.axis).normalized());
            xform::rotate_weighted(&mut m, &drag.weights, local_pivot, axis, angle);
        } else if let Some(sd) = drag.scale.as_mut() {
            let mut factors = Vector3::ONE;
            match sd.axis {
                None => {
                    let delta = pos - sd.press;
                    if sd.dir == Vector2::ZERO {
                        if delta.length() < UNIFORM_DIR_PX {
                            return;
                        }
                        sd.dir = delta.normalized();
                    }
                    let f = xform::snap_step(1.0 + delta.dot(sd.dir) / axis_len_px(), scale_step).max(MIN_FACTOR);
                    factors = Vector3::new(f, f, f);
                }
                Some(a) => {
                    let Some(t) = ray_line_t(cam, pos, pivot, axes_w[a]) else { return };
                    let mut f = xform::snap_step(t / sd.t0, scale_step);
                    if f.abs() < MIN_FACTOR {
                        f = if f >= 0.0 { MIN_FACTOR } else { -MIN_FACTOR };
                    }
                    match a {
                        0 => factors.x = f,
                        1 => factors.y = f,
                        _ => factors.z = f,
                    }
                }
            }
            if factors == sd.factors {
                return;
            }
            sd.factors = factors;
            let frame = Frame { x: to_p((inv_basis * axes_w[0]).normalized()), y: to_p((inv_basis * axes_w[1]).normalized()), z: to_p((inv_basis * axes_w[2]).normalized()) };
            xform::scale_weighted(&mut m, &drag.weights, local_pivot, &frame, to_p(factors));
        } else {
            return;
        }
        drag.lock_uvs(&mut m);
        if let Some(mut data) = self.data() {
            data.bind_mut().replace(m);
            data.emit_changed();
        }
        self.redraw();
    }

    // ================================================================== loops / ring / misc selection

    /// `_handle_double_click`: Edge mode selects the edge loop (Alt: ring) under the cursor,
    /// Face mode the quad loop across the face edge nearest the cursor.
    pub(super) fn double_click(&mut self, cam: &Gd<Camera3D>, pos: Vector2, additive: bool, ring: bool) -> bool {
        let Some(d) = self.data() else { return false };
        let xf = self.xform();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| (to_v2(cam.unproject_position(w)), cam.get_global_position().distance_to(w)))
        };
        let db = d.bind();
        let m = db.mesh();
        let t = loops::Topo::build(m);
        match self.mode {
            Mode::Edge => {
                let Some(e) = pick::nearest_edge(m, &project, to_v2(pos), super::PICK_PX) else { return false };
                let keys = if ring { loops::edge_ring(m, &t, e) } else { loops::edge_loop(&t, e) };
                if !additive {
                    self.sel_edges.clear();
                }
                self.sel_edges.extend(keys);
                true
            }
            Mode::Face => {
                let (o, dir) = self.local_ray(cam, pos);
                let Some(h) = pick::ray_mesh(m, o, dir, true) else { return false };
                // Face edge nearest the cursor on screen.
                let mut best: Option<(f32, Edge)> = None;
                for (a, b) in m.faces[h.face].loop_edges() {
                    let (Some((sa, _)), Some((sb, _))) = (project(m.positions[a as usize]), project(m.positions[b as usize])) else { continue };
                    let dd = closest_on_segment_2d(pos, Vector2::new(sa.x, sa.y), Vector2::new(sb.x, sb.y)).distance_to(pos);
                    if best.is_none_or(|bb| dd < bb.0) {
                        best = Some((dd, edge(a, b)));
                    }
                }
                let Some((_, e)) = best else { return false };
                let fl = loops::face_loop(m, &t, h.face, e);
                if !additive {
                    self.sel_faces.clear();
                }
                self.sel_faces.extend(fl);
                true
            }
            _ => false,
        }
    }

    /// Select loop [L] / ring [Alt+L] through the selected edges; in Face mode the quad loop
    /// through two selected neighbouring faces.
    pub(super) fn act_select_loop(&mut self, ring: bool) {
        let Some(d) = self.data() else { return };
        let db = d.bind();
        let m = db.mesh();
        let t = loops::Topo::build(m);
        match self.mode {
            Mode::Edge if !self.sel_edges.is_empty() => {
                let mut out = BTreeSet::new();
                for e in &self.sel_edges {
                    out.extend(if ring { loops::edge_ring(m, &t, *e) } else { loops::edge_loop(&t, *e) });
                }
                self.sel_edges = out;
            }
            Mode::Face if self.sel_faces.len() >= 2 => {
                let fs: Vec<usize> = self.sel_faces.iter().copied().collect();
                let shared = m.faces[fs[0]].loop_edges().map(|(a, b)| edge(a, b)).find(|e| t.edge_faces.get(e).is_some_and(|l| l.contains(&fs[1])));
                match shared {
                    Some(e) => self.sel_faces = loops::face_loop(m, &t, fs[0], e),
                    None => self.message = "Loop: select two neighbouring faces (or double-click a face near an edge)".into(),
                }
            }
            _ => self.message = "Loop: select an edge (Edge mode) or two neighbouring faces; double-click works too".into(),
        }
    }

    pub(super) fn act_shrink(&mut self) {
        let Some(d) = self.data() else { return };
        let db = d.bind();
        let m = db.mesh();
        let t = loops::Topo::build(m);
        match self.mode {
            Mode::Face => self.sel_faces = loops::shrink_faces(m, &t, &self.sel_faces),
            Mode::Edge => self.sel_edges = loops::shrink_edges(&t, &self.sel_edges),
            Mode::Vertex => self.sel_verts = loops::shrink_vertices(&t, &self.sel_verts),
            Mode::Object => {}
        }
    }

    pub(super) fn act_holes(&mut self) {
        let Some(d) = self.data() else { return };
        let t = loops::Topo::build(d.bind().mesh());
        self.set_mode(Mode::Edge);
        self.sel_edges = loops::boundary_edges(&t);
        if self.sel_edges.is_empty() {
            self.message = "Holes: the mesh is closed".into();
        }
    }

    // ================================================================== loop insert / bridge / mirror / bevel

    pub(super) fn act_insert_loop(&mut self) {
        let Some(&e) = self.sel_edges.iter().next().filter(|_| self.mode == Mode::Edge) else {
            self.message = "Insert loop: select an edge across the quads to cut (Edge mode)".into();
            return;
        };
        let cuts = self.cuts.clamp(1, 16) as u32;
        self.apply_edit("Pixel Builder: Insert edge loop", |m, s| {
            let new = loops::insert_loop(m, e, cuts);
            if new.is_empty() {
                s.message = "Insert loop: the edge has no quad ring".into();
            }
            s.sel_edges = new;
        });
    }

    pub(super) fn act_bridge(&mut self) {
        if self.mode != Mode::Edge || self.sel_edges.len() < 2 {
            self.message = "Bridge: select two open edges (or two equal open edge runs)".into();
            return;
        }
        self.apply_edit("Pixel Builder: Bridge edges", |m, s| {
            let faces = loops::bridge_selected(m, &s.sel_edges);
            if faces.is_empty() {
                s.message = "Bridge: needs two separate open edges that a new face can join cleanly".into();
            }
            s.sel_edges = m.edges_of_faces(&faces);
        });
    }

    pub(super) fn act_mirror(&mut self) {
        let axis = self.mirror_axis.clamp(0, 2) as usize;
        self.apply_edit(&format!("Pixel Builder: Mirror {}", ["X", "Y", "Z"][axis]), |m, s| {
            loops::mirror_object(m, axis, true, 1e-3);
            s.clear_selection();
        });
    }

    pub(super) fn act_bevel_segments(&mut self) {
        if self.mode != Mode::Edge || self.sel_edges.is_empty() {
            self.message = "Bevel: select edges (Edge mode)".into();
            return;
        }
        let (amt, segs) = (self.bevel, self.bevel_segs.clamp(1, 8) as u32);
        self.apply_edit("Pixel Builder: Bevel", |m, s| {
            let faces = pt_core::build::bevel::bevel_edges(m, &s.sel_edges, amt, segs, segs > 1);
            s.sel_edges = m.edges_of_faces(&faces);
        });
    }

    // ================================================================== knife

    /// Knife tool input. `Some(consumed)` when handled.
    pub(super) fn knife_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        if let Ok(k) = event.clone().try_cast::<InputEventKey>() {
            if !k.is_pressed() || k.is_echo() {
                return None;
            }
            return match k.get_keycode() {
                Key::ESCAPE if self.knife.face.is_some() => {
                    self.knife = Knife::default();
                    self.base().update_overlays();
                    Some(true)
                }
                Key::ENTER | Key::KP_ENTER => {
                    self.knife_commit();
                    Some(true)
                }
                Key::BACKSPACE if !self.knife.points.is_empty() => {
                    self.knife.points.pop();
                    self.base().update_overlays();
                    Some(true)
                }
                _ => None,
            };
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            if self.knife.face.is_some() {
                self.knife_cursor(cam, mm.get_position(), mm.is_ctrl_pressed());
                self.base().update_overlays();
            }
            return Some(false);
        }
        let mb = event.clone().try_cast::<InputEventMouseButton>().ok()?;
        if !mb.is_pressed() {
            return Some(mb.get_button_index() == MouseButton::LEFT);
        }
        match mb.get_button_index() {
            MouseButton::RIGHT if self.knife.face.is_some() => {
                self.knife = Knife::default();
                self.base().update_overlays();
                Some(true)
            }
            MouseButton::LEFT => {
                let pos = mb.get_position();
                match self.knife.face {
                    None => {
                        let (o, d) = self.local_ray(cam, pos);
                        let data = self.data()?;
                        let hit = pick::ray_mesh(data.bind().mesh(), o, d, true);
                        if let Some(h) = hit {
                            let db = data.bind();
                            let m = db.mesh();
                            self.knife = Knife { face: Some(h.face), normal: m.face_normal(h.face), point: m.face_center(h.face), ..Default::default() };
                            self.message = "Knife: click points on the face, close on the first point / Enter".into();
                        }
                    }
                    Some(_) => {
                        self.knife_cursor(cam, pos, mb.is_ctrl_pressed());
                        if mb.is_double_click() && self.knife.points.len() >= 3 {
                            self.knife_commit();
                        } else if let Some(c) = self.knife.cursor {
                            if self.knife.points.len() >= 3 && self.knife_near_first(cam, pos) {
                                self.knife_commit();
                            } else {
                                self.knife.points.push(c);
                            }
                        }
                    }
                }
                self.base().update_overlays();
                Some(true)
            }
            _ => None,
        }
    }

    fn knife_near_first(&self, cam: &Gd<Camera3D>, pos: Vector2) -> bool {
        let Some(p) = self.knife.points.first() else { return false };
        let w = self.xform() * from_p(*p);
        !cam.is_position_behind(w) && cam.unproject_position(w).distance_to(pos) <= CLOSE_PX
    }

    /// `_update_cursor` + `_apply_snap`: ray onto the face plane, then vertex / edge / lattice snap.
    fn knife_cursor(&mut self, cam: &Gd<Camera3D>, pos: Vector2, invert_snap: bool) {
        let (o, d) = self.local_ray(cam, pos);
        let (n, q) = (self.knife.normal, self.knife.point);
        let den = n.dot(d);
        self.knife.snapped = false;
        if den.abs() < 1e-6 {
            self.knife.cursor = None;
            return;
        }
        let t = n.dot(q - o) / den;
        if t <= 0.0 {
            self.knife.cursor = None;
            return;
        }
        let mut c = o + d * t;
        let snap_on = self.snap_grid != invert_snap;
        if snap_on
            && let (Some(face), Some(data)) = (self.knife.face, self.data())
        {
            let db = data.bind();
            let m = db.mesh();
            if let Some(f) = m.faces.get(face) {
                let step = self.grid_step.max(0.001);
                let xf = self.xform();
                let frame = cut::lattice_frame(n, m.positions[f.verts[0] as usize]);
                let screen = |p: PVec3| {
                    let w = xf * from_p(p);
                    (!cam.is_position_behind(w)).then(|| cam.unproject_position(w))
                };
                let mut best = ELEMENT_SNAP_PX;
                let mut hit = None;
                for v in &f.verts {
                    let p = m.positions[*v as usize];
                    if let Some(s) = screen(p) {
                        let dd = s.distance_to(pos);
                        if dd < best {
                            best = dd;
                            hit = Some(p);
                        }
                    }
                }
                if hit.is_none() {
                    for (a, b) in f.loop_edges() {
                        let (pa, pb) = (m.positions[a as usize], m.positions[b as usize]);
                        let cp = cut::closest_on_segment(c, pa, pb);
                        if let Some(s) = screen(cp) {
                            let dd = s.distance_to(pos);
                            if dd < best {
                                best = dd;
                                hit = Some(cut::snap_on_edge(c, pa, pb, frame, step));
                            }
                        }
                    }
                }
                match hit {
                    Some(h) => {
                        c = h;
                        self.knife.snapped = true;
                    }
                    None => c = cut::snap_to_lattice(c, frame, step),
                }
            }
        }
        self.knife.cursor = Some(c);
    }

    pub(super) fn knife_commit(&mut self) {
        let Some(face) = self.knife.face else { return };
        if self.knife.points.len() < 3 {
            self.message = "Knife: place at least three points".into();
            return;
        }
        let pts = std::mem::take(&mut self.knife.points);
        self.knife = Knife::default();
        self.apply_edit("Pixel Builder: Knife", |m, s| match cut::cut_face(m, face, &pts) {
            Some(f) => {
                s.sel_faces = BTreeSet::from([f]);
            }
            None => s.message = "Knife: the polygon must lie on the face (inside it or touching its edges)".into(),
        });
        if !self.sel_faces.is_empty() {
            self.mode = Mode::Face;
            self.sync_ui();
        }
    }

    pub(super) fn draw_knife(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        if self.tool != Tool::Knife {
            return;
        }
        let xf = self.xform();
        let proj = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| cam.unproject_position(w))
        };
        let marker = |o: &mut Gd<Control>, p: PVec3, c: Color| {
            if let Some(s) = proj(p) {
                o.draw_rect_ex(Rect2::new(s - Vector2::new(4.0, 4.0), Vector2::new(8.0, 8.0)), c).filled(false).width(2.0).done();
            }
        };
        let seg = |o: &mut Gd<Control>, a: PVec3, b: PVec3, c: Color| {
            if let (Some(sa), Some(sb)) = (proj(a), proj(b)) {
                o.draw_line_ex(sa, sb, c).width(2.0).antialiased(true).done();
            }
        };
        let (Some(face), Some(data)) = (self.knife.face, self.data()) else { return };
        let db = data.bind();
        let m = db.mesh();
        let Some(f) = m.faces.get(face) else { return };
        let dim = Color::from_rgba(KNIFE.r, KNIFE.g, KNIFE.b, 0.5);
        for (a, b) in f.loop_edges() {
            seg(overlay, m.positions[a as usize], m.positions[b as usize], dim);
        }
        let pts = &self.knife.points;
        for (i, p) in pts.iter().enumerate() {
            marker(overlay, *p, if i == 0 && pts.len() >= 3 { AMBER } else { KNIFE });
            if i > 0 {
                seg(overlay, pts[i - 1], *p, KNIFE);
            }
        }
        if let Some(c) = self.knife.cursor {
            if let Some(l) = pts.last() {
                seg(overlay, *l, c, dim);
            }
            marker(overlay, c, if self.knife.snapped { AMBER } else { KNIFE });
        }
    }

    // ================================================================== shape re-edit

    /// Shape parameters toggle: loads the mesh's recipe into the strip; field edits rebuild it.
    pub(super) fn toggle_reshape(&mut self) {
        if self.reshape {
            self.reshape = false;
            return;
        }
        let Some(spec) = self.data().and_then(|d| d.bind().shape_spec()) else {
            self.message = "Shape parameters: this mesh was not drawn from a shape".into();
            return;
        };
        self.spec = spec;
        self.reshape = true;
        self.message = format!("Editing the {} recipe: change the shape fields to rebuild it", spec.kind.name());
    }

    /// Regenerates the target from the current spec (one merged undo step per drag of a field).
    pub(super) fn rebuild_shape(&mut self) {
        let (Some(mut data), Some(t)) = (self.data(), self.target.clone()) else { return };
        let anchored = t.get("anchored").try_to::<bool>().unwrap_or(false);
        let cell = anchored.then(|| t.get("anchor_cell").try_to::<Vector2i>().ok()).flatten();
        let mut m = self.shape_mesh(cell);
        let (before, old_spec, keep) = {
            let d = data.bind();
            let f0 = d.mesh().faces.first().map(|f| (f.material, f.color));
            (d.get_blob(), data.get("shape"), f0)
        };
        if let Some((slot, col)) = keep {
            for f in &mut m.faces {
                f.material = slot;
                f.color = col;
            }
        }
        {
            let mut d = data.bind_mut();
            d.replace(m);
            d.set_shape_spec(&self.spec);
        }
        data.emit_changed();
        let (after, new_spec) = (data.bind().get_blob(), data.get("shape"));
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action_ex("Pixel Builder: Shape parameters").merge_mode(godot::classes::undo_redo::MergeMode::ENDS).done();
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "shape", &new_spec);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "shape", &old_spec);
        ur.commit_action_ex().execute(false).done();
        self.redraw();
    }

    // ================================================================== booleans

    /// `_run_boolean`: CSG of the edited mesh with the one other selected builder mesh.
    pub(super) fn act_boolean(&mut self, op: Operation, name: &str) {
        let Some(target) = self.target.clone() else { return };
        let Some(sel) = EditorInterface::singleton().get_selection() else { return };
        let others: Vec<Gd<PixelBuilderMesh>> = sel
            .get_selected_nodes()
            .iter_shared()
            .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
            .filter(|n| n.instance_id() != target.instance_id() && n.bind().data().is_some())
            .collect();
        if others.len() != 1 {
            self.message = format!("{name}: select exactly one other builder mesh too (Ctrl+click it in the Scene dock; found {})", others.len());
            return;
        }
        let src = others[0].clone();
        let (Some(a), Some(b)) = (target.bind().data(), src.bind().data()) else { return };
        if !a.bind().mesh().is_closed() {
            self.message = format!("{name}: {} is not watertight (Select holes shows the open edges)", target.get_name());
            return;
        }
        if !b.bind().mesh().is_closed() {
            self.message = format!("{name}: {} is not watertight", src.get_name());
            return;
        }
        let (materials, b_map) = merged_slots(&a, &b);
        let mut tags = Vec::new();
        let mesh_a = tagged_mesh(a.bind().mesh(), &|s| s, &mut tags);
        let mesh_b = tagged_mesh(b.bind().mesh(), &|s| b_map.get(s as usize).copied().unwrap_or(s), &mut tags);
        let (Some(mesh_a), Some(mesh_b)) = (mesh_a, mesh_b) else { return };
        let mut root = CsgMesh3D::new_alloc();
        root.set_mesh(&mesh_a);
        let mut child = CsgMesh3D::new_alloc();
        child.set_mesh(&mesh_b);
        child.set_operation(op);
        child.set_transform(target.get_global_transform().affine_inverse() * src.get_global_transform());
        root.add_child(&child);
        let mut t = target.clone();
        add_internal(&mut t, &root);
        self.pending_bool = Some(PendingBool { root, target, src, name: name.to_string(), tries: 0, tags, materials });
        self.wait_frame();
        self.message = format!("{name}: running…");
    }

    fn wait_frame(&mut self) {
        let cb = self.to_gd().callable("on_boolean_frame");
        if self.base().is_inside_tree() {
            let mut tree = self.base().get_tree();
            tree.connect_flags("process_frame", &cb, ConnectFlags::ONE_SHOT);
        }
    }

    /// Second half of a boolean, one frame later: bake, convert, commit (one undo step that also
    /// removes the second operand).
    pub(super) fn finish_boolean(&mut self) {
        let Some(mut p) = self.pending_bool.take() else { return };
        if !p.root.is_instance_valid() || !p.target.is_instance_valid() || !p.src.is_instance_valid() {
            return;
        }
        let baked = p.root.bake_static_mesh();
        let baked = match baked.filter(|m| m.get_surface_count() > 0) {
            Some(m) => m,
            None if p.tries < 3 => {
                p.tries += 1;
                self.pending_bool = Some(p);
                self.wait_frame();
                return;
            }
            None => {
                p.root.queue_free();
                self.message = format!("{} produced no geometry: the meshes probably do not overlap. Nothing changed.", p.name);
                self.base().update_overlays();
                return;
            }
        };
        p.root.queue_free();
        let mesh: Gd<Mesh> = baked.upcast();
        let mut positions = Vec::new();
        let mut tris = Vec::new();
        let mut surf = Vec::new();
        let mut surf_tag: Vec<Option<usize>> = Vec::new();
        for s in 0..mesh.get_surface_count() {
            let arrays = mesh.surface_get_arrays(s);
            let Ok(verts) = arrays.at(ArrayType::VERTEX.ord() as usize).try_to::<PackedVector3Array>() else { continue };
            let base = positions.len() as u32;
            positions.extend(verts.as_slice().iter().map(|q| to_p(*q)));
            let idx: Vec<u32> = match arrays.at(ArrayType::INDEX.ord() as usize).try_to::<PackedInt32Array>() {
                Ok(i) if !i.is_empty() => i.as_slice().iter().map(|x| *x as u32).collect(),
                _ => (0..verts.len() as u32).collect(),
            };
            let mat = mesh.surface_get_material(s);
            let tag = mat.and_then(|m| p.tags.iter().position(|(t, _, _)| t.instance_id() == m.instance_id()));
            surf_tag.push(tag);
            let si = surf_tag.len() as u16 - 1;
            for t in idx.chunks_exact(3) {
                tris.push([base + t[0], base + t[1], base + t[2]]);
                surf.push(si);
            }
        }
        let mut m = convert::from_triangles(&positions, &tris, &surf, 1e-4);
        for f in &mut m.faces {
            let (slot, col) = surf_tag.get(f.material as usize).copied().flatten().map(|t| (p.tags[t].1, p.tags[t].2)).unwrap_or((0, [255; 4]));
            f.material = slot;
            f.color = col;
        }
        if m.faces.is_empty() || m.validate().is_err() {
            self.message = format!("{} failed: the CSG result is not a valid polygon mesh", p.name);
            self.base().update_overlays();
            return;
        }
        let Some(mut data) = p.target.bind().data() else { return };
        let (before, old_mats) = (data.bind().get_blob(), data.bind().material_slots());
        data.bind_mut().replace(m);
        data.set("materials", &p.materials.to_variant());
        data.emit_changed();
        let after = data.bind().get_blob();
        let Some(parent) = p.src.get_parent() else { return };
        let owner = p.src.get_owner();
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action(&format!("Pixel Builder: {}", p.name));
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "materials", &p.materials.to_variant());
        ur.add_do_method(&parent, "remove_child", &[p.src.to_variant()]);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "materials", &old_mats.to_variant());
        ur.add_undo_method(&parent, "add_child", &[p.src.to_variant(), true.to_variant()]);
        if let Some(o) = owner {
            ur.add_undo_method(&p.src, "set_owner", &[o.to_variant()]);
        }
        ur.add_undo_reference(&p.src);
        ur.commit_action();
        self.clear_selection();
        self.palette_slots = None;
        self.message = format!("{} done", p.name);
        self.sync_ui();
        self.redraw();
    }

    /// Debug / test hook used by `debug_boolean`: run a boolean against a given node.
    pub(super) fn boolean_with(&mut self, other: Gd<PixelBuilderMesh>, op: i64) {
        let Some(mut sel) = EditorInterface::singleton().get_selection() else { return };
        if let Some(t) = &self.target {
            sel.add_node(t);
        }
        sel.add_node(&other);
        let (o, n) = match op {
            1 => (Operation::SUBTRACTION, "Boolean subtract"),
            2 => (Operation::INTERSECTION, "Boolean intersect"),
            _ => (Operation::UNION, "Boolean union"),
        };
        self.act_boolean(o, n);
    }

    /// Spec helper so `on_field` can tell whether a field change rebuilds the reshaped mesh.
    pub(super) fn spec_changed(&mut self, old: ShapeSpec) {
        if self.reshape && self.spec != old {
            self.rebuild_shape();
        }
    }

    // ================================================================== wall polyline

    /// Wall shape (`Kind.WALL` / `Stage.PLACE_POINTS` of UBShapeTool): click points, click the
    /// first one to close the room, Enter / double-click finishes an open wall, Backspace
    /// removes the last point, Esc / right-click cancels.
    pub(super) fn wall_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        if let Ok(k) = event.clone().try_cast::<InputEventKey>() {
            if !k.is_pressed() || k.is_echo() || self.wall_pts.is_empty() {
                return None;
            }
            return match k.get_keycode() {
                Key::ESCAPE => {
                    self.wall_pts.clear();
                    self.base().update_overlays();
                    Some(true)
                }
                Key::ENTER | Key::KP_ENTER => {
                    self.commit_wall(false);
                    Some(true)
                }
                Key::BACKSPACE => {
                    self.wall_pts.pop();
                    self.base().update_overlays();
                    Some(true)
                }
                _ => None,
            };
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            match self.wall_pts.first() {
                Some(p0) => self.wall_cursor = self.draw_point(cam, pos, *p0, self.wall_basis),
                None => {
                    self.update_hover(cam, pos);
                    self.wall_cursor = self.hover_place.map(|p| p.point);
                }
            }
            self.base().update_overlays();
            return Some(false);
        }
        let mb = event.clone().try_cast::<InputEventMouseButton>().ok()?;
        if !mb.is_pressed() {
            return Some(mb.get_button_index() == MouseButton::LEFT);
        }
        let pos = mb.get_position();
        match mb.get_button_index() {
            MouseButton::RIGHT if !self.wall_pts.is_empty() => {
                self.wall_pts.clear();
                self.base().update_overlays();
                Some(true)
            }
            MouseButton::LEFT => {
                let p = match self.wall_pts.first() {
                    Some(p0) => self.draw_point(cam, pos, *p0, self.wall_basis),
                    None => {
                        self.update_hover(cam, pos);
                        self.wall_basis = self.hover_place.map(|p| p.basis).unwrap_or(Basis::IDENTITY);
                        self.hover_place.map(|p| p.point)
                    }
                };
                let Some(p) = p else { return Some(true) };
                let near_first = self.wall_pts.len() >= 3
                    && !cam.is_position_behind(self.wall_pts[0])
                    && cam.unproject_position(self.wall_pts[0]).distance_to(pos) <= CLOSE_PX;
                if near_first {
                    self.commit_wall(true);
                } else if mb.is_double_click() && self.wall_pts.len() >= 2 {
                    self.commit_wall(false);
                } else if self.wall_pts.last().is_none_or(|l| l.distance_to(p) > 1e-3) {
                    self.wall_pts.push(p);
                }
                self.base().update_overlays();
                Some(true)
            }
            _ => None,
        }
    }

    /// Wall through the clicked points (+ the cursor) for the preview: origin = first point.
    pub(super) fn wall_preview(&self) -> Option<(Vector3, BMesh)> {
        let o = *self.wall_pts.first()?;
        let inv = self.wall_basis.inverse();
        let mut pts: Vec<PVec3> = self.wall_pts.iter().map(|p| to_p(inv * (*p - o))).collect();
        if let Some(c) = self.wall_cursor {
            pts.push(to_p(inv * (c - o)));
        }
        for p in &mut pts {
            p.y = 0.0;
        }
        if pts.len() < 2 {
            let mut m = BMesh::default();
            m.add_vert(PVec3::ZERO);
            return Some((o, m));
        }
        if self.spec.kind == pt_core::build::ShapeKind::Poly {
            let flat: Vec<pt_core::math::Vec2> = pts.iter().map(|p| pt_core::math::v2(p.x, p.z)).collect();
            let prism = pt_core::build::trim_walls::polygon_prism(&flat, self.spec.size.y.max(0.01));
            if !prism.faces.is_empty() {
                return Some((o, prism));
            }
        }
        Some((o, pt_core::build::shapes::wall_polyline(&self.spec, &pts, false)))
    }

    pub(super) fn commit_wall(&mut self, closed: bool) {
        let pts = std::mem::take(&mut self.wall_pts);
        let Some(&o) = pts.first() else { return };
        let inv = self.wall_basis.inverse();
        let local: Vec<PVec3> = pts.iter().map(|p| {
            let mut q = to_p(inv * (*p - o));
            q.y = 0.0;
            q
        }).collect();
        let m = if self.spec.kind == pt_core::build::ShapeKind::Poly {
            // Unibuilder poly shape: the clicked outline (always closed) extruded `H` up.
            let pts: Vec<pt_core::math::Vec2> = local.iter().map(|p| pt_core::math::v2(p.x, p.z)).collect();
            pt_core::build::trim_walls::polygon_prism(&pts, self.spec.size.y.max(0.01))
        } else {
            pt_core::build::shapes::wall_polyline(&self.spec, &local, closed && local.len() >= 3)
        };
        if m.faces.is_empty() {
            self.message = if self.spec.kind == pt_core::build::ShapeKind::Poly { "Poly shape: click at least three points".into() } else { "Wall: click at least two points".into() };
            return;
        }
        let append = Input::singleton().is_key_pressed(Key::SHIFT);
        let basis = self.wall_basis;
        self.create_from_mesh(super::Placement { point: o, cell: None, basis }, m, append);
        self.wall_cursor = None;
    }

    /// Test / scripting hook body: a wall through world `points`.
    pub(super) fn wall_through(&mut self, points: &[Vector3], closed: bool) {
        self.wall_pts = points.to_vec();
        self.commit_wall(closed);
    }
}

/// Raw ring angle of the cursor (`_raw_angle`).
fn raw_angle(rd: &RotDrag, cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3) -> f32 {
    if !rd.screen_mode {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let den = rd.axis.dot(d);
        if den.abs() > 1e-6 {
            let t = rd.axis.dot(pivot - o) / den;
            if t > 0.0 {
                let dir = o + d * t - pivot;
                return dir.dot(rd.bitangent).atan2(dir.dot(rd.tangent));
            }
        }
        return rd.prev_raw;
    }
    let to_cam = cam.get_global_position() - pivot;
    let sign = if rd.axis.dot(to_cam) >= 0.0 { 1.0 } else { -1.0 };
    let v = pos - cam.unproject_position(pivot);
    -sign * v.y.atan2(v.x)
}

/// Compiles `m` with one surface per (slot, color), each with a fresh placeholder material,
/// so the CSG result's surfaces can be mapped back to slots and colors.
fn tagged_mesh(m: &BMesh, slot_map: &dyn Fn(u16) -> u16, tags: &mut Vec<(Gd<StandardMaterial3D>, u16, [u8; 4])>) -> Option<Gd<ArrayMesh>> {
    let mut tagged = m.clone();
    let first = tags.len();
    let mut keys: Vec<(u16, [u8; 4])> = Vec::new();
    for f in &mut tagged.faces {
        let key = (slot_map(f.material), f.color);
        let i = keys.iter().position(|k| *k == key).unwrap_or_else(|| {
            keys.push(key);
            keys.len() - 1
        });
        f.material = (first + i) as u16;
    }
    for (slot, col) in keys {
        tags.push((StandardMaterial3D::new_gd(), slot, col));
    }
    let c = compile(&tagged, &CompileOptions::default());
    let mut mesh = ArrayMesh::new_gd();
    for s in &c.surfaces {
        if s.indices.is_empty() {
            continue;
        }
        let mut arrays = VarArray::new();
        arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
        let pos: PackedVector3Array = s.positions.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
        let nrm: PackedVector3Array = s.normals.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
        let uv: PackedVector2Array = s.uvs.iter().map(|p| Vector2::new(p[0], p[1])).collect();
        let idx: PackedInt32Array = s.indices.iter().copied().collect();
        arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
        arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
        arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
        arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
        mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
        let si = mesh.get_surface_count() - 1;
        if let Some((mat, _, _)) = tags.get(s.material as usize) {
            mesh.surface_set_material(si, mat);
        }
    }
    (mesh.get_surface_count() > 0).then_some(mesh)
}

/// Material slots of `a` with those of `b` joined in (the same Material keeps one slot; an
/// empty B slot keeps its index), plus the B slot -> merged slot map. Shared by the booleans
/// and Merge objects (Unibuilder `merge_objects.gd::_map_slot`).
pub(super) fn merged_slots(a: &Gd<PixelBuilderMeshData>, b: &Gd<PixelBuilderMeshData>) -> (Array<Option<Gd<Material>>>, Vec<u16>) {
    let mut materials = a.bind().material_slots().duplicate_shallow();
    let b_mats = b.bind().material_slots();
    let n_b_slots = b.bind().mesh().faces.iter().map(|f| f.material as usize + 1).max().unwrap_or(1).max(b_mats.len());
    let mut b_map = Vec::with_capacity(n_b_slots);
    for j in 0..n_b_slots {
        let mat = b_mats.get(j).flatten();
        let found = match &mat {
            Some(m) => materials.iter_shared().position(|x| x.as_ref().is_some_and(|x| x.instance_id() == m.instance_id())),
            // An empty B slot keeps its index (empty slots all use the default material).
            None => Some(j),
        };
        let slot = match found {
            Some(i) => i,
            None => {
                while materials.len() < a.bind().mesh().faces.iter().map(|f| f.material as usize + 1).max().unwrap_or(0) {
                    materials.push(None::<&Gd<Material>>);
                }
                materials.push(mat.as_ref());
                materials.len() - 1
            }
        };
        b_map.push(slot as u16);
    }
    (materials, b_map)
}
