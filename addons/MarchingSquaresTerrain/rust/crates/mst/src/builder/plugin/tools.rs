use super::super::data::{from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::{Mode, Tool, XformMode};
use super::PixelBuilderPlugin;
use crate::math_conversions::to_v2;
use godot::classes::{
    Camera3D, Control, EditorInterface, Input, InputEvent, InputEventKey, InputEventMouseButton,
    InputEventMouseMotion,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use pt_core::build::xform::{self, Falloff, Frame, Weights};
use pt_core::build::{BMesh, Edge, ShapeSpec, cut, edge, loops, pick};
use pt_core::math::Vec3 as PVec3;
use std::collections::BTreeSet;

const AXIS_COLORS: [Color; 3] = [
    Color::from_rgb(0.9605, 0.3005, 0.3305),
    Color::from_rgb(0.5305, 0.8405, 0.2805),
    Color::from_rgb(0.3005, 0.5505, 0.9605),
];
fn radius_px() -> f32 {
    64.5005 * crate::builder::prefs::handle_scale()
}
fn hit_px() -> f32 {
    10.5005 * crate::builder::prefs::handle_scale()
}
const SEGMENTS: usize = 50;
const EDGE_ON: f32 = 0.0804;
fn axis_len_px() -> f32 {
    64.5005 * crate::builder::prefs::handle_scale()
}
fn tip_px() -> f32 {
    7.5005 * crate::builder::prefs::handle_scale()
}
fn center_px() -> f32 {
    10.5005 * crate::builder::prefs::handle_scale()
}
const UNIFORM_DIR_PX: f32 = 3.0505;
const MIN_FACTOR: f32 = 0.0014;
const ARM_HIDE_FRAC: f32 = 0.1204;
const ARM_SOLID_FRAC: f32 = 0.3504;
const CLOSE_PX: f32 = 14.5005;
const ELEMENT_SNAP_PX: f32 = 12.5005;
const CLIP_COLOR: Color = Color::from_rgb(0.3714, 0.9184, 0.8304);
const HOVER_COLOR: Color = Color::from_rgb(0.9834, 0.7484, 0.1414);
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handle {
    Move(Option<usize>),
    Rotate(usize),
    Scale(Option<usize>),
}
pub struct RotateDrag {
    axle: Vector3,
    toucher: Vector3,
    bitangent: Vector3,
    screen_mode: bool,
    prev_raw: f32,
    unsnapped: f32,
    pub last: f32,
}
pub struct ScaleDrag {
    axle: Option<usize>,
    t0: f32,
    press: Vector2,
    dir: Vector2,
    pub factors: Vector3,
}
#[derive(Default)]
pub struct Knife {
    pub face: Option<usize>,
    upright: PVec3,
    dot: PVec3,
    pub points: Vec<PVec3>,
    cursor: Option<PVec3>,
    aligned: bool,
}
fn closest_on_segment_2d(p: Vector2, a: Vector2, b: Vector2) -> Vector2 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 < 1e-12 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}
fn axis_ray_param(cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3, axis: Vector3) -> Option<f32> {
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
fn arm_screen_vec(cam: &Gd<Camera3D>, pivot: Vector3, axis: Vector3, len_px: f32) -> Vector2 {
    let k = (cam.get_global_position().distance_to(pivot) * 0.0105).max(0.0015);
    let probe = pivot + axis.normalized() * k;
    if cam.is_position_behind(probe) {
        return Vector2::ZERO;
    }
    let at = cam.unproject_position(pivot);
    let across = cam
        .unproject_position(pivot + cam.get_global_transform().basis.col_a() * k)
        .distance_to(at);
    if across < 1e-6 {
        return Vector2::ZERO;
    }
    let v = (cam.unproject_position(probe) - at) * (len_px / across);
    if v.length() >= len_px * ARM_HIDE_FRAC {
        v
    } else {
        Vector2::ZERO
    }
}
fn arm_fade(arm: Vector2, len_px: f32) -> f32 {
    ((arm.length() / len_px - ARM_HIDE_FRAC) / (ARM_SOLID_FRAC - ARM_HIDE_FRAC)).clamp(0.0, 1.0)
}
fn world_radius(cam: &Gd<Camera3D>, pivot: Vector3) -> f32 {
    let right = cam.get_global_transform().basis.col_a();
    let px = cam
        .unproject_position(pivot + right)
        .distance_to(cam.unproject_position(pivot));
    radius_px() / px.max(1e-3)
}
fn ring_frame(axis: Vector3) -> (Vector3, Vector3) {
    let axis = axis.normalized();
    let r = if axis.x.abs() < 0.9005 {
        Vector3::RIGHT
    } else {
        Vector3::UP
    };
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
fn readout(overlay: &mut Gd<Control>, at: Vector2, caption: &str) {
    let Some(font) = overlay.get_theme_default_font() else {
        return;
    };
    let p = at + Vector2::new(16.0, -12.0);
    overlay
        .draw_string_ex(&font, p + Vector2::new(1.0, 1.0), caption)
        .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8005))
        .done();
    overlay
        .draw_string_ex(&font, p, caption)
        .modulate(Color::WHITE)
        .done();
}
impl PixelBuilderPlugin {
    pub(super) fn gizmo_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Handle> {
        let axes_w = self.gizmo_axes();
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return None;
        }
        match self.xform_mode {
            XformMode::Move => self.handle_at(cam, pos).map(Handle::Move),
            XformMode::Rotate => {
                let pivot = self.fulcrum()?;
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
                let pivot = self.fulcrum()?;
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
                    let arm = arm_screen_vec(cam, pivot, *a, axis_len_px());
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
    pub(super) fn draw_gizmo(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        let axes_w = self.gizmo_axes();
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return;
        }
        let active = self.drag.as_ref().and_then(|d| d.handle);
        let hot =
            |h: Handle| active == Some(h) || (active.is_none() && self.hover_handle == Some(h));
        match self.xform_mode {
            XformMode::Move => {
                let Some((s0, ends)) = self.handle_screen(cam) else {
                    return;
                };
                for (a, e) in ends.iter().enumerate() {
                    let h = hot(Handle::Move(Some(a)));
                    let c = if h {
                        AXIS_COLORS[a].lightened(0.4505)
                    } else {
                        AXIS_COLORS[a]
                    };
                    overlay
                        .draw_line_ex(s0, *e, c)
                        .width(if h { 3.0 } else { 2.0 })
                        .done();
                    let dir = (*e - s0).normalized_or_zero();
                    let side = Vector2::new(-dir.y, dir.x) * 4.0;
                    let tip = PackedVector2Array::from([*e + dir * 8.0, *e + side, *e - side]);
                    overlay.draw_colored_polygon(&tip, c);
                }
                let h = hot(Handle::Move(None));
                overlay
                    .draw_rect_ex(
                        Rect2::new(s0 - Vector2::new(5.0, 5.0), Vector2::new(10.0, 10.0)),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.9005),
                    )
                    .filled(h)
                    .width(1.5005)
                    .done();
                if let Some(d) = &self.drag {
                    let v = d.last_delta;
                    readout(
                        overlay,
                        d.cursor,
                        &format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z),
                    );
                    if let Some(s) = d.snap_hit.filter(|s| !cam.is_position_behind(*s)) {
                        let p = cam.unproject_position(s);
                        overlay
                            .draw_rect_ex(
                                Rect2::new(p - Vector2::new(6.0, 6.0), Vector2::new(12.0, 12.0)),
                                HOVER_COLOR,
                            )
                            .filled(false)
                            .width(2.0)
                            .done();
                    }
                }
            }
            XformMode::Rotate => {
                let Some(pivot) = self
                    .drag
                    .as_ref()
                    .map(|d| d.fulcrum)
                    .or_else(|| self.fulcrum())
                else {
                    return;
                };
                if cam.is_position_behind(pivot) {
                    return;
                }
                let r = world_radius(cam, pivot);
                for (i, a) in axes_w.iter().enumerate() {
                    let h = hot(Handle::Rotate(i));
                    if active.is_some() && !h {
                        continue;
                    }
                    let c = if h {
                        AXIS_COLORS[i].lightened(0.4505)
                    } else {
                        AXIS_COLORS[i]
                    };
                    for run in circle_runs(cam, pivot, *a, r) {
                        for k in 0..run.len() - 1 {
                            overlay
                                .draw_line_ex(run[k], run[k + 1], c)
                                .width(if h { 3.0 } else { 2.0 })
                                .antialiased(true)
                                .done();
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
                let Some(pivot) = self
                    .drag
                    .as_ref()
                    .map(|d| d.fulcrum)
                    .or_else(|| self.fulcrum())
                else {
                    return;
                };
                if cam.is_position_behind(pivot) {
                    return;
                }
                let origin = cam.unproject_position(pivot);
                for (i, a) in axes_w.iter().enumerate() {
                    let arm = arm_screen_vec(cam, pivot, *a, axis_len_px());
                    if arm == Vector2::ZERO {
                        continue;
                    }
                    let h = hot(Handle::Scale(Some(i)));
                    let mut c = if h {
                        AXIS_COLORS[i].lightened(0.4505)
                    } else {
                        AXIS_COLORS[i]
                    };
                    if !h {
                        c.a = arm_fade(arm, axis_len_px());
                    }
                    let tip = origin + arm;
                    overlay
                        .draw_line_ex(origin, tip, c)
                        .width(if h { 3.5005 } else { 2.0 })
                        .antialiased(true)
                        .done();
                    let half = (tip_px() + if h { 2.0 } else { 0.0 }) * 0.5;
                    overlay.draw_rect(
                        Rect2::new(
                            tip - Vector2::new(half, half),
                            Vector2::new(half, half) * 2.0,
                        ),
                        c,
                    );
                }
                let ch = hot(Handle::Scale(None));
                let rect = Rect2::new(
                    origin - Vector2::new(center_px(), center_px()) * 0.5,
                    Vector2::new(center_px(), center_px()),
                );
                if ch {
                    overlay.draw_rect(rect, Color::from_rgba(1.0, 1.0, 1.0, 0.3505));
                }
                overlay
                    .draw_rect_ex(rect, Color::from_rgba(1.0, 1.0, 1.0, 0.9005))
                    .filled(false)
                    .width(if ch { 3.0 } else { 2.0 })
                    .done();
                if let Some(d) = &self.drag
                    && let Some(sd) = &d.magnify
                {
                    let f = match sd.axle {
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
    pub(super) fn drag_weights(&self, m: &BMesh, verts: &BTreeSet<u32>) -> Weights {
        if !self.soft {
            return xform::hard_weights(verts);
        }
        let xf = self.xform();
        xform::proportional_weights(
            &m.positions,
            verts,
            self.soft_radius,
            &|p| to_p(xf * from_p(p)),
            Falloff::from_i64(self.falloff),
        )
    }
    pub(super) fn snap_setup(
        &self,
        cam: &Gd<Camera3D>,
        cursor: Vector2,
        m: &BMesh,
        moved: &Weights,
        pivot: Vector3,
    ) -> (Vector3, Vec<Vector3>) {
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
        let mut targets: Vec<Vector3> = (0..m.positions.len() as u32)
            .filter(|v| !moved.contains_key(v))
            .map(|v| xf * from_p(m.positions[v as usize]))
            .collect();
        if let Some(root) = EditorInterface::singleton().get_edited_scene_root() {
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
                if self
                    .target
                    .as_ref()
                    .is_some_and(|t| t.instance_id() == b.instance_id())
                {
                    continue;
                }
                let Some(d) = b.bind().contents() else {
                    continue;
                };
                let bx = b.get_global_transform();
                targets.extend(d.bind().cage().positions.iter().map(|p| bx * from_p(*p)));
            }
        }
        (anchor, targets)
    }
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
        let idx = xform::closest_target(&pts, &proj, to_v2(cursor), xform::SNAP_PX);
        drag.snap_hit = idx.map(|i| drag.snap_targets[i]);
        drag.snap_hit.map(|t| t - drag.snap_anchor)
    }
    pub(super) fn begin_rotate(
        &self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        pivot: Vector3,
        axis_i: usize,
    ) -> RotateDrag {
        let axes_w = self.gizmo_axes();
        let axis = axes_w[axis_i];
        let (tangent, bitangent) = ring_frame(axis);
        let screen_mode = axis.dot(cam.project_ray_normal(pos)).abs() < EDGE_ON;
        let mut rd = RotateDrag {
            axle: axis,
            toucher: tangent,
            bitangent,
            screen_mode,
            prev_raw: 0.0,
            unsnapped: 0.0,
            last: 0.0,
        };
        rd.prev_raw = raw_angle(&rd, cam, pos, pivot);
        rd
    }
    pub(super) fn begin_scale(
        &self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        pivot: Vector3,
        axis: Option<usize>,
    ) -> ScaleDrag {
        let axes_w = self.gizmo_axes();
        let t0 = axis
            .and_then(|a| axis_ray_param(cam, pos, pivot, axes_w[a]))
            .filter(|t| t.abs() >= 1e-4)
            .unwrap_or(1e-4);
        ScaleDrag {
            axle: axis,
            t0,
            press: pos,
            dir: Vector2::ZERO,
            factors: Vector3::ONE,
        }
    }
    pub(super) fn drag_rotate_scale(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let axes_w = self.gizmo_axes();
        let snap = self.snap_grid;
        let rot_step = if snap {
            self.rot_snap.to_radians()
        } else {
            0.0
        };
        let scale_step = if snap { 0.1005 } else { 0.0 };
        let xf = self.xform();
        let inv_basis = xf.basis.inverse();
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        drag.cursor = pos;
        let pivot = drag.fulcrum;
        let local_pivot = to_p(xf.affine_inverse() * pivot);
        let mut m = drag.commence.clone();
        if let Some(rd) = drag.rot.as_mut() {
            let raw = raw_angle(rd, cam, pos, pivot);
            rd.unsnapped += xform::wrap_pi(raw - rd.prev_raw);
            rd.prev_raw = raw;
            let angle = xform::snap_step(rd.unsnapped, rot_step);
            if angle == rd.last && rd.last != 0.0 {
                return;
            }
            rd.last = angle;
            let axis = to_p((inv_basis * rd.axle).normalized());
            match &drag.groups {
                Some(g) => pt_core::build::xg_ops::rotate_individual(&mut m, g, axis, angle),
                None => xform::rotate_weighted(&mut m, &drag.weights, local_pivot, axis, angle),
            }
        } else if let Some(sd) = drag.magnify.as_mut() {
            let mut factors = Vector3::ONE;
            match sd.axle {
                None => {
                    let delta = pos - sd.press;
                    if sd.dir == Vector2::ZERO {
                        if delta.length() < UNIFORM_DIR_PX {
                            return;
                        }
                        sd.dir = delta.normalized();
                    }
                    let f = xform::snap_step(1.0 + delta.dot(sd.dir) / axis_len_px(), scale_step)
                        .max(MIN_FACTOR);
                    factors = Vector3::new(f, f, f);
                }
                Some(a) => {
                    let Some(t) = axis_ray_param(cam, pos, pivot, axes_w[a]) else {
                        return;
                    };
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
            let frame = Frame {
                x: to_p((inv_basis * axes_w[0]).normalized()),
                y: to_p((inv_basis * axes_w[1]).normalized()),
                z: to_p((inv_basis * axes_w[2]).normalized()),
            };
            match &drag.groups {
                Some(g) => {
                    pt_core::build::xg_ops::scale_individual(&mut m, g, &frame, to_p(factors))
                }
                None => {
                    xform::scale_weighted(&mut m, &drag.weights, local_pivot, &frame, to_p(factors))
                }
            }
        } else {
            return;
        }
        drag.lock_uvs(&mut m);
        if let Some(mut data) = self.contents() {
            data.bind_mut().substitute(m);
            data.emit_changed();
        }
        self.redraw();
    }
    pub(super) fn double_click(
        &mut self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        additive: bool,
        ring: bool,
    ) -> bool {
        let Some(d) = self.contents() else {
            return false;
        };
        let xf = self.xform();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| {
                (
                    to_v2(cam.unproject_position(w)),
                    cam.get_global_position().distance_to(w),
                )
            })
        };
        let db = d.bind();
        let m = db.cage();
        let t = loops::Topo::assemble(m);
        match self.mode {
            Mode::Edge => {
                let Some(e) = pick::pick_closest_edge(m, &project, to_v2(pos), super::PICK_PX)
                else {
                    return false;
                };
                let keys = if ring {
                    loops::quad_ring(m, &t, e)
                } else {
                    loops::edge_chain(&t, e)
                };
                if !additive {
                    self.sel_edges.clear();
                }
                self.sel_edges.extend(keys);
                true
            }
            Mode::Face => {
                let (o, dir) = self.local_ray(cam, pos);
                let Some(h) = pick::intersect_ray_closest(m, o, dir, true) else {
                    return false;
                };
                let mut best: Option<(f32, Edge)> = None;
                for (a, b) in m.facets[h.face].loop_edges() {
                    let (Some((sa, _)), Some((sb, _))) = (
                        project(m.positions[a as usize]),
                        project(m.positions[b as usize]),
                    ) else {
                        continue;
                    };
                    let dd = closest_on_segment_2d(
                        pos,
                        Vector2::new(sa.x, sa.y),
                        Vector2::new(sb.x, sb.y),
                    )
                    .distance_to(pos);
                    if best.is_none_or(|bb| dd < bb.0) {
                        best = Some((dd, edge(a, b)));
                    }
                }
                let Some((_, e)) = best else { return false };
                let fl = loops::face_strip(m, &t, h.face, e);
                if !additive {
                    self.sel_faces.clear();
                }
                self.sel_faces.extend(fl);
                true
            }
            _ => false,
        }
    }
    pub(super) fn perform_grab_ring(&mut self, ring: bool) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        let t = loops::Topo::assemble(m);
        match self.mode {
            Mode::Edge if !self.sel_edges.is_empty() => {
                let mut out = BTreeSet::new();
                for e in &self.sel_edges {
                    out.extend(if ring { loops::quad_ring(m, &t, *e) } else { loops::edge_chain(&t, *e) });
                }
                self.sel_edges = out;
            }
            Mode::Face if self.sel_faces.len() >= 2 => {
                let fs: Vec<usize> = self.sel_faces.iter().copied().collect();
                let shared = m.facets[fs[0]].loop_edges().map(|(a, b)| edge(a, b)).find(|e| t.edge_faces.get(e).is_some_and(|l| l.contains(&fs[1])));
                match shared {
                    Some(e) => self.sel_faces = loops::face_strip(m, &t, fs[0], e),
                    None => self.message = "Loop: select two neighbouring faces (or double-click a face near an edge)".into(),
                }
            }
            _ => self.message = "Loop: select an edge (Edge mode) or two neighbouring faces; double-click works too".into(),
        }
    }
    pub(super) fn act_shrink(&mut self) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        let t = loops::Topo::assemble(m);
        match self.mode {
            Mode::Face => self.sel_faces = loops::contract_faces(m, &t, &self.sel_faces),
            Mode::Edge => self.sel_edges = loops::contract_edges(&t, &self.sel_edges),
            Mode::Vertex => self.sel_verts = loops::contract_vertices(&t, &self.sel_verts),
            Mode::Object => {}
        }
    }
    pub(super) fn act_holes(&mut self) {
        let Some(d) = self.contents() else { return };
        let t = loops::Topo::assemble(d.bind().cage());
        self.assign_setting(Mode::Edge);
        self.sel_edges = loops::exposed_ridges(&t);
        if self.sel_edges.is_empty() {
            self.message = "Holes: the mesh is closed".into();
        }
    }
    pub(super) fn act_insert_loop(&mut self) {
        let Some(&e) = self
            .sel_edges
            .iter()
            .next()
            .filter(|_| self.mode == Mode::Edge)
        else {
            self.message = "Insert loop: select an edge across the quads to cut (Edge mode)".into();
            return;
        };
        let cuts = self.cuts.clamp(1, 16) as u32;
        self.apply_edit("Pixel Builder: Insert edge loop", |m, s| {
            let new = loops::loop_cut(m, e, cuts);
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
                s.message =
                    "Bridge: needs two separate open edges that a new face can join cleanly".into();
            }
            s.sel_edges = m.edges_of_faces(&faces);
        });
    }
    pub(super) fn chamfer_choices(&self) -> pt_core::build::xg_bevel::BevelOptions {
        use pt_core::build::xg_bevel::{AmountType, BevelAffect, BevelOptions};
        BevelOptions {
            affect: if self.mode == Mode::Vertex {
                BevelAffect::Vertices
            } else {
                BevelAffect::Edges
            },
            amount: self.chamfer,
            amount_type: AmountType::from_u8(self.xg.bevel_type.clamp(0, 3) as u8),
            stretches: self.bevel_segs.clamp(1, BevelOptions::MAX_SEGMENTS as i64) as u32,
            silhouette: self.xg.bevel_profile,
            clamp_overlap: self.xg.bevel_clamp,
            harden_normals: self.xg.bevel_harden,
        }
        .sanitized()
    }
    pub(super) fn perform_chamfer_stretches(&mut self) {
        use pt_core::build::xg_bevel;
        let targets = match self.mode {
            Mode::Edge if !self.sel_edges.is_empty() => self.sel_edges.len(),
            Mode::Vertex if !self.sel_verts.is_empty() => self.sel_verts.len(),
            _ => {
                self.message = "Bevel: select edges (Edge mode) or vertices (Vertex mode)".into();
                return;
            }
        };
        let full = self.chamfer_choices();
        let mut opts = full;
        if self.adjust_coarse {
            opts.stretches = xg_bevel::prelook_stretches(&full, targets);
        }
        self.bevel_status.clear();
        let mode = self.mode;
        self.apply_edit("Pixel Builder: Bevel", |m, s| {
            match xg_bevel::chamfer(m, &s.sel_edges, &s.sel_verts, &opts) {
                Ok(r) => {
                    if mode == Mode::Vertex {
                        s.sel_verts = m.verts_of_faces(&r.facets);
                    } else {
                        s.sel_edges = m.edges_of_faces(&r.facets);
                    }
                    s.bevel_status = xg_bevel::status_line(&full, &r, targets).unwrap_or_default();
                    s.message = match s.bevel_status.as_str() {
                        "" => xg_bevel::overview(&full),
                        st => format!("{}  ·  {st}", xg_bevel::overview(&full)),
                    };
                }
                Err(e) => s.message = format!("Bevel: {e}"),
            }
        });
    }
    pub(super) fn knife_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        if let Ok(k) = happening.clone().try_cast::<InputEventKey>() {
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
        if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            if self.knife.face.is_some() {
                self.knife_cursor(cam, mm.get_position(), mm.is_ctrl_pressed());
                self.base().update_overlays();
            }
            return Some(false);
        }
        let mb = happening.clone().try_cast::<InputEventMouseButton>().ok()?;
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
                        let data = self.contents()?;
                        let hit = pick::intersect_ray_closest(data.bind().cage(), o, d, true);
                        if let Some(h) = hit {
                            let db = data.bind();
                            let m = db.cage();
                            self.knife = Knife {
                                face: Some(h.face),
                                upright: m.face_normal(h.face),
                                dot: m.face_center(h.face),
                                ..Default::default()
                            };
                            self.message =
                                "Knife: click points on the face, close on the first point / Enter"
                                    .into();
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
        let Some(p) = self.knife.points.first() else {
            return false;
        };
        let w = self.xform() * from_p(*p);
        !cam.is_position_behind(w) && cam.unproject_position(w).distance_to(pos) <= CLOSE_PX
    }
    fn knife_cursor(&mut self, cam: &Gd<Camera3D>, pos: Vector2, invert_snap: bool) {
        let (o, d) = self.local_ray(cam, pos);
        let (n, q) = (self.knife.upright, self.knife.dot);
        let den = n.innerprod(d);
        self.knife.aligned = false;
        if den.abs() < 1e-6 {
            self.knife.cursor = None;
            return;
        }
        let t = n.innerprod(q - o) / den;
        if t <= 0.0 {
            self.knife.cursor = None;
            return;
        }
        let mut c = o + d * t;
        let snap_on = self.snap_grid != invert_snap;
        if snap_on && let (Some(face), Some(data)) = (self.knife.face, self.contents()) {
            let db = data.bind();
            let m = db.cage();
            if let Some(f) = m.facets.get(face) {
                let step = self.grid_step.max(0.0015);
                let xf = self.xform();
                let frame = cut::snap_frame(n, m.positions[f.verts[0] as usize]);
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
                        let cp = cut::closest_point_on_segment(c, pa, pb);
                        if let Some(s) = screen(cp) {
                            let dd = s.distance_to(pos);
                            if dd < best {
                                best = dd;
                                hit = Some(cut::snap_to_edge(c, pa, pb, frame, step));
                            }
                        }
                    }
                }
                match hit {
                    Some(h) => {
                        c = h;
                        self.knife.aligned = true;
                    }
                    None => c = cut::align_into_lattice(c, frame, step),
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
        self.apply_edit("Pixel Builder: Knife", |m, s| {
            match cut::knife_face(m, face, &pts) {
                Some(f) => {
                    s.sel_faces = BTreeSet::from([f]);
                }
                None => {
                    s.message =
                        "Knife: the polygon must lie on the face (inside it or touching its edges)"
                            .into()
                }
            }
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
                o.draw_rect_ex(
                    Rect2::new(s - Vector2::new(4.0, 4.0), Vector2::new(8.0, 8.0)),
                    c,
                )
                .filled(false)
                .width(2.0)
                .done();
            }
        };
        let seg = |o: &mut Gd<Control>, a: PVec3, b: PVec3, c: Color| {
            if let (Some(sa), Some(sb)) = (proj(a), proj(b)) {
                o.draw_line_ex(sa, sb, c)
                    .width(2.0)
                    .antialiased(true)
                    .done();
            }
        };
        let (Some(face), Some(data)) = (self.knife.face, self.contents()) else {
            return;
        };
        let db = data.bind();
        let m = db.cage();
        let Some(f) = m.facets.get(face) else { return };
        let dim = Color::from_rgba(CLIP_COLOR.r, CLIP_COLOR.g, CLIP_COLOR.b, 0.5);
        for (a, b) in f.loop_edges() {
            seg(
                overlay,
                m.positions[a as usize],
                m.positions[b as usize],
                dim,
            );
        }
        let pts = &self.knife.points;
        for (i, p) in pts.iter().enumerate() {
            marker(
                overlay,
                *p,
                if i == 0 && pts.len() >= 3 {
                    HOVER_COLOR
                } else {
                    CLIP_COLOR
                },
            );
            if i > 0 {
                seg(overlay, pts[i - 1], *p, CLIP_COLOR);
            }
        }
        if let Some(c) = self.knife.cursor {
            if let Some(l) = pts.last() {
                seg(overlay, *l, c, dim);
            }
            marker(
                overlay,
                c,
                if self.knife.aligned {
                    HOVER_COLOR
                } else {
                    CLIP_COLOR
                },
            );
        }
    }
    pub(super) fn toggle_reshape(&mut self) {
        if self.reshape {
            self.reshape = false;
            return;
        }
        if self.bz.editing.is_some() {
            self.assign_instrument(Tool::Select);
            return;
        }
        if self.bezier_reedit() {
            return;
        }
        let Some(spec) = self.contents().and_then(|d| d.bind().shape_spec()) else {
            self.message = "Shape parameters: this mesh was not drawn from a shape".into();
            return;
        };
        self.spec = spec;
        self.reshape = true;
        self.message = format!(
            "Editing the {} recipe: change the shape fields to rebuild it",
            spec.sort.moniker()
        );
    }
    pub(super) fn reassemble_form(&mut self) {
        let (Some(mut data), Some(t)) = (self.contents(), self.target.clone()) else {
            return;
        };
        let anchored = t.get("anchored").try_to::<bool>().unwrap_or(false);
        let cell = anchored
            .then(|| t.get("anchor_cell").try_to::<Vector2i>().ok())
            .flatten();
        let mut m = self.form_cage(cell);
        if m.facets.is_empty() {
            self.message = format!(
                "{}: these settings make an empty mesh (the mesh is kept)",
                self.spec.sort.moniker()
            );
            self.base().update_overlays();
            return;
        }
        let (before, old_spec, keep) = {
            let d = data.bind();
            let f0 = d.cage().facets.first().map(|f| (f.stuff, f.tint));
            (d.get_blob(), data.get("shape"), f0)
        };
        if let Some((slot, col)) = keep {
            for f in &mut m.facets {
                f.stuff = slot;
                f.tint = col;
            }
        }
        {
            let mut d = data.bind_mut();
            d.substitute(m);
            d.set_shape_spec(&self.spec);
        }
        data.emit_changed();
        let (after, new_spec) = (data.bind().get_blob(), data.get("shape"));
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action_ex("Pixel Builder: Shape parameters")
            .merge_mode(godot::classes::undo_redo::MergeMode::ENDS)
            .done();
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "shape", &new_spec);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "shape", &old_spec);
        ur.commit_action_ex().execute(false).done();
        self.redraw();
    }
    pub(super) fn spec_changed(&mut self, old: ShapeSpec) {
        if self.reshape && self.spec != old {
            self.reassemble_form();
        }
    }
    pub(super) fn wall_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        if let Ok(k) = happening.clone().try_cast::<InputEventKey>() {
            if !k.is_pressed()
                || k.is_echo()
                || (self.wall_pts.is_empty() && self.poly_outline.is_none())
            {
                return None;
            }
            return match k.get_keycode() {
                Key::ESCAPE => {
                    self.wall_pts.clear();
                    self.poly_outline = None;
                    self.poly_holes.clear();
                    self.base().update_overlays();
                    Some(true)
                }
                Key::ENTER | Key::KP_ENTER => {
                    self.commit_wall(false);
                    Some(true)
                }
                Key::BACKSPACE => {
                    if self.wall_pts.pop().is_none() {
                        self.poly_holes.pop();
                    }
                    self.base().update_overlays();
                    Some(true)
                }
                _ => None,
            };
        }
        let anchor = self
            .wall_pts
            .first()
            .copied()
            .or_else(|| self.poly_outline.as_ref().and_then(|o| o.first().copied()));
        if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            match anchor.as_ref() {
                Some(p0) => self.wall_cursor = self.draw_point(cam, pos, *p0, self.wall_basis),
                None => {
                    self.update_hover(cam, pos);
                    self.wall_cursor = self.hover_place.map(|p| p.dot);
                }
            }
            self.base().update_overlays();
            return Some(false);
        }
        let mb = happening.clone().try_cast::<InputEventMouseButton>().ok()?;
        if !mb.is_pressed() {
            return Some(mb.get_button_index() == MouseButton::LEFT);
        }
        let pos = mb.get_position();
        match mb.get_button_index() {
            MouseButton::RIGHT if !self.wall_pts.is_empty() || self.poly_outline.is_some() => {
                self.wall_pts.clear();
                self.poly_outline = None;
                self.poly_holes.clear();
                self.base().update_overlays();
                Some(true)
            }
            MouseButton::LEFT => {
                let p = match anchor.as_ref() {
                    Some(p0) => self.draw_point(cam, pos, *p0, self.wall_basis),
                    None => {
                        self.update_hover(cam, pos);
                        self.wall_basis =
                            self.hover_place.map(|p| p.frame).unwrap_or(Basis::IDENTITY);
                        self.hover_place.map(|p| p.dot)
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
    pub(super) fn wall_preview(&self) -> Option<(Vector3, BMesh)> {
        if let Some(outline) = &self.poly_outline {
            let o = *outline.first()?;
            let flat = |pts: &[Vector3], extra: Option<Vector3>| -> Vec<pt_core::math::Vec2> {
                let inv = self.wall_basis.inverse();
                pts.iter()
                    .copied()
                    .chain(extra)
                    .map(|p| inv * (p - o))
                    .map(|q| pt_core::math::v2(q.x, q.z))
                    .collect()
            };
            let mut holes: Vec<Vec<pt_core::math::Vec2>> =
                self.poly_holes.iter().map(|h| flat(h, None)).collect();
            if self.wall_pts.len() >= 2 {
                holes.push(flat(&self.wall_pts, self.wall_cursor));
            }
            let m = pt_core::build::xg_shapes::polygon_form(
                &flat(outline, None),
                &holes,
                self.spec.size.y.max(0.0105),
            )
            .or_else(|_| {
                pt_core::build::xg_shapes::polygon_form(
                    &flat(outline, None),
                    &holes[..self.poly_holes.len()],
                    self.spec.size.y.max(0.0105),
                )
            })
            .unwrap_or_default();
            return Some((o, m));
        }
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
        if self.spec.sort == pt_core::build::ShapeKind::Poly {
            let flat: Vec<pt_core::math::Vec2> =
                pts.iter().map(|p| pt_core::math::v2(p.x, p.z)).collect();
            let prism =
                pt_core::build::trim_walls::polygon_solid(&flat, self.spec.size.y.max(0.0105));
            if !prism.facets.is_empty() {
                return Some((o, prism));
            }
        }
        Some((
            o,
            pt_core::build::shapes::wall_polyline(&self.spec, &pts, false),
        ))
    }
    pub(super) fn commit_wall(&mut self, closed: bool) {
        if self.spec.sort == pt_core::build::ShapeKind::Poly
            && (self.xg.poly_holes || self.poly_outline.is_some())
        {
            self.commit_poly_holes(closed);
            return;
        }
        let pts = std::mem::take(&mut self.wall_pts);
        let Some(&o) = pts.first() else { return };
        let inv = self.wall_basis.inverse();
        let local: Vec<PVec3> = pts
            .iter()
            .map(|p| {
                let mut q = to_p(inv * (*p - o));
                q.y = 0.0;
                q
            })
            .collect();
        let m = if self.spec.sort == pt_core::build::ShapeKind::Poly {
            let pts: Vec<pt_core::math::Vec2> =
                local.iter().map(|p| pt_core::math::v2(p.x, p.z)).collect();
            pt_core::build::trim_walls::polygon_solid(&pts, self.spec.size.y.max(0.0105))
        } else {
            pt_core::build::shapes::wall_polyline(&self.spec, &local, closed && local.len() >= 3)
        };
        if m.facets.is_empty() {
            self.message = if self.spec.sort == pt_core::build::ShapeKind::Poly {
                "Poly shape: click at least three points".into()
            } else {
                "Wall: click at least two points".into()
            };
            return;
        }
        let append = Input::singleton().is_key_pressed(Key::SHIFT);
        let basis = self.wall_basis;
        self.create_from_mesh(
            super::Placement {
                dot: o,
                cell: None,
                frame: basis,
            },
            m,
            append,
        );
        self.wall_cursor = None;
    }
    pub(super) fn commit_poly_holes(&mut self, closed: bool) {
        let pts = std::mem::take(&mut self.wall_pts);
        if self.poly_outline.is_none() {
            if pts.len() < 3 {
                self.message = "Poly shape: click at least three points".into();
                self.wall_pts = pts;
                return;
            }
            self.poly_outline = Some(pts);
            if closed {
                self.message = "Outline closed: click the points of a hole and close it on its first point; Enter creates".into();
                return;
            }
        } else if pts.len() >= 3 {
            self.poly_holes.push(pts);
            if closed {
                self.message = format!(
                    "{} hole{}: draw another or press Enter",
                    self.poly_holes.len(),
                    if self.poly_holes.len() == 1 { "" } else { "s" }
                );
                return;
            }
        } else if closed {
            self.message = "A hole needs at least three points".into();
            self.wall_pts = pts;
            return;
        }
        let outline = self.poly_outline.take().unwrap_or_default();
        let holes = std::mem::take(&mut self.poly_holes);
        let Some(&o) = outline.first() else { return };
        let inv = self.wall_basis.inverse();
        let flat = |pts: &[Vector3]| -> Vec<pt_core::math::Vec2> {
            pts.iter()
                .map(|p| inv * (*p - o))
                .map(|q| pt_core::math::v2(q.x, q.z))
                .collect()
        };
        let hole_loops: Vec<Vec<pt_core::math::Vec2>> = holes.iter().map(|h| flat(h)).collect();
        match pt_core::build::xg_shapes::polygon_form(
            &flat(&outline),
            &hole_loops,
            self.spec.size.y.max(0.0105),
        ) {
            Ok(m) => {
                let append = Input::singleton().is_key_pressed(Key::SHIFT);
                let basis = self.wall_basis;
                self.create_from_mesh(
                    super::Placement {
                        dot: o,
                        cell: None,
                        frame: basis,
                    },
                    m,
                    append,
                );
                if !hole_loops.is_empty() {
                    self.message = format!(
                        "Created Poly shape with {} hole{}",
                        hole_loops.len(),
                        if hole_loops.len() == 1 { "" } else { "s" }
                    );
                }
            }
            Err(e) => {
                self.message = format!("Poly shape: {e}");
                self.poly_outline = Some(outline);
                self.poly_holes = holes;
            }
        }
        self.wall_cursor = None;
    }
    pub(super) fn draw_poly_holes(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        let Some(outline) = &self.poly_outline else {
            return;
        };
        let proj = |w: Vector3| (!cam.is_position_behind(w)).then(|| cam.unproject_position(w));
        for (lp, col) in std::iter::once((outline, HOVER_COLOR))
            .chain(self.poly_holes.iter().map(|h| (h, CLIP_COLOR)))
        {
            for i in 0..lp.len() {
                if let (Some(a), Some(b)) = (proj(lp[i]), proj(lp[(i + 1) % lp.len()])) {
                    overlay.draw_line_ex(a, b, col).width(2.0).done();
                }
            }
        }
        let cur: Vec<Vector3> = self
            .wall_pts
            .iter()
            .copied()
            .chain(self.wall_cursor)
            .collect();
        for w in cur.windows(2) {
            if let (Some(a), Some(b)) = (proj(w[0]), proj(w[1])) {
                overlay.draw_line_ex(a, b, CLIP_COLOR).width(1.5005).done();
            }
        }
    }
    pub(super) fn poly_through(&mut self, outline: &[Vector3], holes: &[Vec<Vector3>]) {
        self.wall_basis = Basis::IDENTITY;
        self.poly_outline = Some(outline.to_vec());
        self.poly_holes = holes.to_vec();
        self.wall_pts.clear();
        self.commit_poly_holes(false);
    }
    pub(super) fn wall_through(&mut self, points: &[Vector3], closed: bool) {
        self.wall_pts = points.to_vec();
        self.commit_wall(closed);
    }
}
fn raw_angle(rd: &RotateDrag, cam: &Gd<Camera3D>, pos: Vector2, pivot: Vector3) -> f32 {
    if !rd.screen_mode {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let den = rd.axle.dot(d);
        if den.abs() > 1e-6 {
            let t = rd.axle.dot(pivot - o) / den;
            if t > 0.0 {
                let dir = o + d * t - pivot;
                return dir.dot(rd.bitangent).atan2(dir.dot(rd.toucher));
            }
        }
        return rd.prev_raw;
    }
    let to_cam = cam.get_global_position() - pivot;
    let sign = if rd.axle.dot(to_cam) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let v = pos - cam.unproject_position(pivot);
    -sign * v.y.atan2(v.x)
}
