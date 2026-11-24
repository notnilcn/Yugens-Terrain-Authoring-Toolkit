//! The Studio-style handles a part gets in the Roblox layout: they live on the whole object, not on
//! its faces or vertices, so they are drawn, hit and dragged here, apart from the element handles.
use super::objects::affine;
use super::part_handle_rules::{
    self as rules, WorldBox, anchor_point, axis_lengths, box_corners, grown_factors, world_box,
};
use super::tools::{AXIS_COLORS, hit_px, readout, ring_frame};
use super::*;
use crate::camera_projection::project;
use pt_modeler::topology_edits;
use pt_modeler::model::UvParams;
use pt_core::math::v3;
const ARROW_GAP_PX: f32 = 10.0;
const ARROW_LEN_PX: f32 = 46.0;
const BALL_GAP_PX: f32 = 14.0;
const BALL_PX: f32 = 7.0;
const CORNER_PX: f32 = 5.0;
const RING_GAP_PX: f32 = 26.0;
const PLANE_GAP_PX: f32 = 44.0;
const ARC_HALF: f32 = 0.6;
const RING_SEGMENTS: usize = 64;
const SNAP_TO_PARTS_PX: f32 = 12.0;
const SELECTION_BLUE: Color = Color::from_rgb(0.2205, 0.6205, 1.0);
const PLANE_YELLOW: Color = Color::from_rgb(1.0, 0.8505, 0.1505);
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Grip {
    Arrow(usize, i8),
    Ring(usize),
    Ball(usize, i8),
    Corner([i8; 3]),
    Plane,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    Move,
    Rotate,
    Resize,
    Pivot,
}
struct Recipe {
    spec: ShapeSpec,
    offset: PVec3,
    looks: Vec<(u16, [u8; 4], UvParams)>,
    old_form: Variant,
}
pub(super) struct Hold {
    grip: Grip,
    job: Job,
    pivot: Vector3,
    axis: Vector3,
    start_t: f32,
    start_hit: Vector3,
    start_angle: f32,
    turned: f32,
    nodes: Vec<(Gd<Node3D>, Transform3D, Transform3D)>,
    start_xf: Transform3D,
    lo: Vector3,
    hi: Vector3,
    recipe: Option<Recipe>,
    before: Option<PackedByteArray>,
    cage: Option<BMesh>,
    kids: Vec<(Gd<Node3D>, Transform3D)>,
    obstacles: Vec<WorldBox>,
    neighbours: Vec<WorldBox>,
    snap_axes: [bool; 3],
    snap_reach: f32,
    clear_delta: Vector3,
    caption: String,
    cursor: Vector2,
    turn: Basis,
    wanted: Vector3,
    unstepped: Vector3,
}
#[derive(Default)]
pub(super) struct RobloxHandles {
    pub(super) hover: Option<Grip>,
    pub(super) hold: Option<Hold>,
    pub(super) swallow_drag: bool,
    pub(super) click_lock: bool,
    pub(super) pivot_snap: bool,
    pub(super) snap_boxes: bool,
    pub(super) grid_plane_hidden: bool,
    pub(super) move_step: Option<f32>,
    pub(super) other_step: Option<f32>,
}
pub(super) fn user_locked(n: &Gd<Node>) -> bool {
    rules::is_user_lock(n.has_meta(EDIT_LOCK_META), n.has_meta(PLUGIN_LOCK_META))
}
fn units_per_px(cam: &Gd<Camera3D>, p: Vector3) -> f32 {
    let right = cam.get_global_transform().basis.col_a();
    let px = project(cam, p + right)
        .zip(project(cam, p))
        .map_or(0.0, |(a, b)| a.distance_to(b));
    1.0 / px.max(1e-3)
}
fn screen(cam: &Gd<Camera3D>, p: Vector3) -> Option<Vector2> {
    project(cam, p)
}
fn ring_runs(
    cam: &Gd<Camera3D>,
    centre: Vector3,
    axis: Vector3,
    r: f32,
    window: Option<f32>,
) -> Vec<Vec<Vector2>> {
    let (t, b) = ring_frame(axis);
    let (from, span) = match window {
        Some(mid) => (mid - ARC_HALF, ARC_HALF * 2.0),
        None => (0.0, std::f32::consts::TAU),
    };
    let mut runs = Vec::new();
    let mut cur: Vec<Vector2> = Vec::new();
    for k in 0..=RING_SEGMENTS {
        let a = from + span * k as f32 / RING_SEGMENTS as f32;
        let w = centre + (t * a.cos() + b * a.sin()) * r;
        match screen(cam, w) {
            Some(s) => cur.push(s),
            None => {
                if cur.len() > 1 {
                    runs.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() > 1 {
        runs.push(cur);
    }
    runs
}
fn arc_middle(cam: &Gd<Camera3D>, axis: Vector3, i: usize) -> f32 {
    let cb = cam.get_global_transform().basis;
    let want = if i == 1 { cb.col_c() } else { cb.col_b() };
    let (t, b) = ring_frame(axis);
    want.dot(b).atan2(want.dot(t))
}
fn ring_angle(pivot: Vector3, axis: Vector3, o: Vector3, d: Vector3) -> Option<f32> {
    let h = plane_hit(pivot, axis, o, d).or_else(|| plane_hit(pivot, -axis, o, d))?;
    let (t, b) = ring_frame(axis);
    let v = h - pivot;
    Some(v.dot(b).atan2(v.dot(t)))
}
impl PixelBuilderPlugin {
    fn roblox_handles_shown(&self) -> bool {
        self.is_roblox()
            && self.mode == Mode::Object
            && matches!(self.tool, Tool::Select | Tool::EditPivot)
            && self
                .target
                .as_ref()
                .is_some_and(|t| t.is_instance_valid() && !user_locked(&t.clone().upcast::<Node>()))
    }
    pub(super) fn roblox_wants_gizmo_lock(&self) -> bool {
        self.is_roblox()
            && self.mode == Mode::Object
            && matches!(self.tool, Tool::Select | Tool::EditPivot)
    }
    fn target_box(&self) -> Option<(Vector3, Vector3)> {
        let d = self.contents()?;
        let (lo, hi) = d.bind().cage().extent()?;
        Some((from_p(lo), from_p(hi)))
    }
    fn move_axes(&self) -> [Vector3; 3] {
        if self.opts.gizmo_orient == 1 {
            let b = self.xform().basis.orthonormalized();
            [b.col_a(), b.col_b(), b.col_c()]
        } else {
            [Vector3::RIGHT, Vector3::UP, Vector3::BACK]
        }
    }
    fn part_axes(&self) -> [Vector3; 3] {
        let b = self.xform().basis;
        [
            b.col_a().normalized_or_zero(),
            b.col_b().normalized_or_zero(),
            b.col_c().normalized_or_zero(),
        ]
    }
    fn grips(&self, cam: &Gd<Camera3D>) -> Vec<(Grip, Vec<Vector2>)> {
        let mut out = Vec::new();
        let Some((lo, hi)) = self.target_box() else {
            return out;
        };
        let xf = self.xform();
        let centre = xf * ((lo + hi) * 0.5);
        let origin = xf.origin;
        let pivot_tool = self.tool == Tool::EditPivot;
        let job_mode = if pivot_tool {
            None
        } else {
            Some(self.xform_mode)
        };
        let upp = units_per_px(cam, centre);
        let arrows = matches!(job_mode, None | Some(XformMode::Move));
        let rings = matches!(
            job_mode,
            None | Some(XformMode::Rotate) | Some(XformMode::Combined)
        );
        if arrows {
            let at = if pivot_tool { origin } else { centre };
            for (a, dir) in self.move_axes().iter().enumerate() {
                for s in [1i8, -1] {
                    if pivot_tool && s < 0 {
                        continue;
                    }
                    let d = *dir * s as f32;
                    let half = if pivot_tool {
                        0.0
                    } else {
                        rules::half_extent_along(xf, lo, hi, d)
                    };
                    let base = at + d * (half + ARROW_GAP_PX * upp);
                    let tip = base + d * ARROW_LEN_PX * upp;
                    if let (Some(p), Some(q)) = (screen(cam, base), screen(cam, tip)) {
                        out.push((Grip::Arrow(a, s), vec![p, q]));
                    }
                }
            }
        }
        if rings {
            let r =
                (hi - lo).length() * 0.5 * axis_lengths(xf.basis).x.max(1e-3) + RING_GAP_PX * upp;
            let r = if pivot_tool {
                RING_GAP_PX * 2.0 * upp
            } else {
                r
            };
            for (i, a) in self.move_axes().iter().enumerate() {
                let window =
                    (job_mode == Some(XformMode::Combined)).then(|| arc_middle(cam, *a, i));
                for run in ring_runs(cam, origin, *a, r, window) {
                    out.push((Grip::Ring(i), run));
                }
            }
        }
        if job_mode == Some(XformMode::Scale) {
            let axes = self.part_axes();
            for (a, n) in axes.iter().enumerate() {
                for s in [1i8, -1] {
                    let w =
                        xf * rules::face_point(lo, hi, a, s) + *n * s as f32 * BALL_GAP_PX * upp;
                    if let Some(p) = screen(cam, w) {
                        out.push((Grip::Ball(a, s), vec![p]));
                    }
                }
            }
        }
        if job_mode == Some(XformMode::Combined) {
            for (i, c) in box_corners(xf, lo, hi).iter().enumerate() {
                let sides = rules::corner_sides(i);
                if let Some(p) = screen(cam, *c) {
                    out.push((Grip::Corner(sides), vec![p]));
                }
            }
            let top = centre
                + Vector3::UP
                    * (rules::half_extent_along(xf, lo, hi, Vector3::UP) + PLANE_GAP_PX * upp);
            if let Some(p) = screen(cam, top) {
                out.push((Grip::Plane, vec![p]));
            }
        }
        out
    }
    fn roblox_grip_at(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Grip> {
        if !self.roblox_handles_shown()
            || self.xform_mode == XformMode::Pick && self.tool == Tool::Select
        {
            return None;
        }
        let cands: Vec<(Grip, u8, f32)> = self
            .grips(cam)
            .into_iter()
            .map(|(g, pts)| {
                let rank = match g {
                    Grip::Ball(..) | Grip::Corner(_) | Grip::Plane => 0,
                    Grip::Arrow(..) => 1,
                    Grip::Ring(_) => 2,
                };
                (g, rank, rules::polyline_distance(pos, &pts))
            })
            .collect();
        rules::nearest_within(&cands, hit_px())
    }
    pub(super) fn draw_roblox_handles(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        let Some(t) = self.target.as_ref().filter(|t| t.is_instance_valid()) else {
            return;
        };
        if user_locked(&t.clone().upcast::<Node>()) {
            return;
        }
        let Some((lo, hi)) = self.target_box() else {
            return;
        };
        let xf = self.xform();
        let c = box_corners(xf, lo, hi);
        for (a, b) in [
            (0, 1),
            (2, 3),
            (4, 5),
            (6, 7),
            (0, 2),
            (1, 3),
            (4, 6),
            (5, 7),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            if let (Some(p), Some(q)) = (screen(cam, c[a]), screen(cam, c[b])) {
                overlay
                    .draw_line_ex(p, q, SELECTION_BLUE)
                    .width(1.5005)
                    .antialiased(true)
                    .done();
            }
        }
        if !self.roblox_handles_shown() {
            return;
        }
        if self.tool == Tool::EditPivot
            && let Some(p) = screen(cam, xf.origin)
        {
            overlay.draw_circle(p, 6.0, Color::from_rgba(0.0, 0.0, 0.0, 0.6005));
            overlay.draw_circle(p, 4.5005, Color::WHITE);
        }
        if self.tool == Tool::Select
            && self.xform_mode == XformMode::Combined
            && !self.rbx.grid_plane_hidden
        {
            self.draw_ground_plane(overlay, cam, xf, lo, hi);
        }
        let active = self.rbx.hold.as_ref().map(|h| h.grip);
        let hot = |g: Grip| active == Some(g) || (active.is_none() && self.rbx.hover == Some(g));
        let shade = |i: usize, g: Grip| {
            if hot(g) {
                AXIS_COLORS[i].lightened(0.4505)
            } else {
                AXIS_COLORS[i]
            }
        };
        for (g, pts) in self.grips(cam) {
            if active.is_some() && !hot(g) && matches!(g, Grip::Ring(_)) {
                continue;
            }
            match g {
                Grip::Arrow(a, _) => {
                    let col = shade(a, g);
                    let (p, q) = (pts[0], pts[1]);
                    let dir = (q - p).normalized_or_zero();
                    let side = Vector2::new(-dir.y, dir.x);
                    let head = (q - p).length().min(18.0);
                    let neck = q - dir * head;
                    overlay
                        .draw_line_ex(p, neck, col)
                        .width(if hot(g) { 4.0 } else { 3.0 })
                        .antialiased(true)
                        .done();
                    let cone = PackedVector2Array::from([q, neck + side * 7.0, neck - side * 7.0]);
                    overlay.draw_colored_polygon(&cone, col);
                }
                Grip::Ring(i) => {
                    let col = shade(i, g);
                    for w in pts.windows(2) {
                        overlay
                            .draw_line_ex(w[0], w[1], col)
                            .width(if hot(g) { 4.0 } else { 3.0 })
                            .antialiased(true)
                            .done();
                    }
                }
                Grip::Ball(a, _) => {
                    let col = shade(a, g);
                    let r = BALL_PX + if hot(g) { 2.0 } else { 0.0 };
                    overlay.draw_circle(pts[0], r + 1.5005, Color::from_rgba(0.0, 0.0, 0.0, 0.5));
                    overlay.draw_circle(pts[0], r, col);
                    overlay.draw_circle(
                        pts[0] - Vector2::new(r, r) * 0.3,
                        r * 0.35,
                        Color::from_rgba(1.0, 1.0, 1.0, 0.5505),
                    );
                }
                Grip::Corner(_) => {
                    let h = CORNER_PX + if hot(g) { 1.5005 } else { 0.0 };
                    let rect = Rect2::new(pts[0] - Vector2::new(h, h), Vector2::new(h, h) * 2.0);
                    overlay.draw_rect(rect, Color::WHITE);
                    overlay
                        .draw_rect_ex(rect, Color::from_rgba(0.0, 0.0, 0.0, 0.7005))
                        .filled(false)
                        .width(1.0)
                        .done();
                }
                Grip::Plane => {
                    let p = pts[0];
                    let col = if hot(g) {
                        PLANE_YELLOW.lightened(0.3505)
                    } else {
                        PLANE_YELLOW
                    };
                    let cone = PackedVector2Array::from([
                        p + Vector2::new(0.0, -11.0),
                        p + Vector2::new(7.0, 6.0),
                        p + Vector2::new(-7.0, 6.0),
                    ]);
                    overlay.draw_colored_polygon(&cone, col);
                }
            }
        }
        if let Some(h) = &self.rbx.hold
            && !h.caption.is_empty()
        {
            readout(overlay, h.cursor, &h.caption);
        }
    }
    fn draw_ground_plane(
        &self,
        overlay: &mut Gd<Control>,
        cam: &Gd<Camera3D>,
        xf: Transform3D,
        lo: Vector3,
        hi: Vector3,
    ) {
        let (wlo, whi) = world_box(xf, lo, hi);
        let step = self.grid_step.max(0.0625);
        let pad = step.max(1.0);
        let (x0, x1) = (wlo.x - pad, whi.x + pad);
        let (z0, z1) = (wlo.z - pad, whi.z + pad);
        let y = wlo.y;
        let quad: Vec<Vector2> = [
            Vector3::new(x0, y, z0),
            Vector3::new(x1, y, z0),
            Vector3::new(x1, y, z1),
            Vector3::new(x0, y, z1),
        ]
        .iter()
        .filter_map(|p| screen(cam, *p))
        .collect();
        if quad.len() == 4 {
            overlay.draw_colored_polygon(
                &PackedVector2Array::from(quad.as_slice()),
                Color::from_rgba(0.1005, 0.1105, 0.1305, 0.5505),
            );
        }
        let lines = ((x1 - x0) / step).ceil() as usize + ((z1 - z0) / step).ceil() as usize;
        if lines > 160 {
            return;
        }
        let grid = Color::from_rgba(1.0, 1.0, 1.0, 0.1205);
        let mut x = (x0 / step).ceil() * step;
        while x <= x1 {
            if let (Some(p), Some(q)) = (
                screen(cam, Vector3::new(x, y, z0)),
                screen(cam, Vector3::new(x, y, z1)),
            ) {
                overlay.draw_line(p, q, grid);
            }
            x += step;
        }
        let mut z = (z0 / step).ceil() * step;
        while z <= z1 {
            if let (Some(p), Some(q)) = (
                screen(cam, Vector3::new(x0, y, z)),
                screen(cam, Vector3::new(x1, y, z)),
            ) {
                overlay.draw_line(p, q, grid);
            }
            z += step;
        }
    }
    pub(super) fn roblox_gizmo_event(
        &mut self,
        cam: &Gd<Camera3D>,
        event: &Gd<InputEvent>,
    ) -> Option<bool> {
        if !self.is_roblox() || self.mode != Mode::Object {
            return None;
        }
        if self.rbx.swallow_drag {
            let kind = if event.is_class("InputEventMouseMotion") {
                rules::PointerAfterCancel::Motion
            } else if event
                .clone()
                .try_cast::<InputEventMouseButton>()
                .is_ok_and(|b| b.get_button_index() == MouseButton::LEFT && !b.is_pressed())
            {
                rules::PointerAfterCancel::LeftRelease
            } else {
                rules::PointerAfterCancel::Other
            };
            let (consume, keep) = rules::swallow_after_cancel(true, kind);
            self.rbx.swallow_drag = keep;
            if consume {
                return Some(true);
            }
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            if self.rbx.hold.is_some() {
                self.roblox_hold_motion(cam, pos);
                return Some(true);
            }
            let g = if self.rbx.click_lock {
                None
            } else {
                self.roblox_grip_at(cam, pos)
            };
            if g != self.rbx.hover {
                self.rbx.hover = g;
                self.base().update_overlays();
            }
            return None;
        }
        let mb = event.clone().try_cast::<InputEventMouseButton>().ok()?;
        let pos = mb.get_position();
        if mb.get_button_index() == MouseButton::RIGHT && mb.is_pressed() && self.rbx.hold.is_some()
        {
            self.cancel_roblox_hold();
            return Some(true);
        }
        if mb.get_button_index() != MouseButton::LEFT {
            return None;
        }
        if self.rbx.hold.is_some() {
            if !mb.is_pressed() {
                self.finish_roblox_hold();
            }
            return Some(true);
        }
        if !mb.is_pressed() {
            return (self.tool == Tool::EditPivot).then_some(false);
        }
        if self.rbx.click_lock {
            self.click_lock_at(cam, pos);
            return Some(true);
        }
        match self.roblox_grip_at(cam, pos) {
            Some(g) => {
                self.begin_roblox_hold(cam, pos, g);
                Some(true)
            }
            None => (self.tool == Tool::EditPivot).then_some(false),
        }
    }
    fn selected_movers(&self) -> Vec<Gd<Node3D>> {
        let mut out: Vec<Gd<Node3D>> = EditorInterface::singleton()
            .get_selection()
            .map(|s| {
                s.get_top_selected_nodes()
                    .iter_shared()
                    .filter_map(|n| n.try_cast::<Node3D>().ok())
                    .filter(|n| !user_locked(&n.clone().upcast::<Node>()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(t) = &self.target {
            let t3 = t.clone().upcast::<Node3D>();
            if !out.iter().any(|n| n.instance_id() == t3.instance_id()) {
                out.push(t3);
            }
        }
        out
    }
    fn recipe_now(&self) -> Option<Recipe> {
        let data = self.contents()?;
        let old_form = data.get("form");
        let db = data.bind();
        let spec = db.shape_spec()?;
        if spec.sort.is_grid_shape() || spec.sort.is_polyline() {
            return None;
        }
        let cage = db.cage();
        let g = shapes::produce(&spec, None);
        rules::recipe_offset(cage, &g).map(|offset| Recipe {
            spec,
            offset,
            looks: cage
                .facets
                .iter()
                .map(|f| (f.stuff, f.tint, f.tex_coord))
                .collect(),
            old_form,
        })
    }
    fn obstacles_for(&self, movers: &[Gd<Node3D>]) -> Vec<WorldBox> {
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return Vec::new();
        };
        root.find_children_ex("*")
            .type_("MeshInstance3D")
            .owned(false)
            .done()
            .iter_shared()
            .filter_map(|n| n.try_cast::<MeshInstance3D>().ok())
            .filter(|n| n.is_visible_in_tree())
            .filter(|n| {
                let as_node = n.clone().upcast::<Node>();
                !movers
                    .iter()
                    .any(|m| m.instance_id() == n.instance_id() || m.is_ancestor_of(&as_node))
            })
            .filter(|n| {
                let mut p = n.get_parent();
                while let Some(x) = p {
                    if x.is_class("MarchingSquaresTerrain") {
                        return false;
                    }
                    p = x.get_parent();
                }
                true
            })
            .map(|n| {
                let a = n.get_aabb();
                world_box(n.get_global_transform(), a.position, a.position + a.size)
            })
            .collect()
    }
    fn begin_roblox_hold(&mut self, cam: &Gd<Camera3D>, pos: Vector2, grip: Grip) {
        let Some((lo, hi)) = self.target_box() else {
            return;
        };
        let Some(target) = self.target.clone() else {
            return;
        };
        let job = match (self.tool, grip) {
            (Tool::EditPivot, _) => Job::Pivot,
            (_, Grip::Arrow(..) | Grip::Plane) => Job::Move,
            (_, Grip::Ring(_)) => Job::Rotate,
            (_, Grip::Ball(..) | Grip::Corner(_)) => Job::Resize,
        };
        if matches!(job, Job::Resize | Job::Pivot) {
            self.unshare_data();
        }
        let xf = self.xform();
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let centre = xf * ((lo + hi) * 0.5);
        let (pivot, axis) = match grip {
            Grip::Arrow(a, _) => (
                if job == Job::Pivot { xf.origin } else { centre },
                self.move_axes()[a],
            ),
            Grip::Ring(i) => (xf.origin, self.move_axes()[i]),
            Grip::Ball(a, s) => (
                xf * rules::face_point(lo, hi, a, s),
                self.part_axes()[a] * s as f32,
            ),
            Grip::Corner(s) => (
                xf * rules::corner_point(lo, hi, s),
                cam.get_global_transform().basis.col_c(),
            ),
            Grip::Plane => (
                centre,
                rules::ground_drag_normal(d, cam.get_global_transform().basis.col_c()),
            ),
        };
        let start_t = match grip {
            Grip::Arrow(..) | Grip::Ball(..) => closest_on_axis(pivot, axis, o, d).unwrap_or(0.0),
            _ => 0.0,
        };
        let start_hit = plane_hit(pivot, axis, o, d)
            .or_else(|| plane_hit(pivot, -axis, o, d))
            .unwrap_or(pivot);
        let start_angle = match grip {
            Grip::Ring(_) => ring_angle(pivot, axis, o, d).unwrap_or(0.0),
            _ => 0.0,
        };
        let movers = if job == Job::Move || job == Job::Rotate {
            self.selected_movers()
        } else {
            vec![target.clone().upcast::<Node3D>()]
        };
        let nodes = movers
            .iter()
            .map(|n| (n.clone(), n.get_global_transform(), n.get_transform()))
            .collect();
        let obstacles = if job == Job::Move && self.prefs.collisions {
            self.obstacles_for(&movers)
        } else {
            Vec::new()
        };
        let neighbours = if job == Job::Move && self.snap_vertex {
            self.obstacles_for(&movers)
        } else {
            Vec::new()
        };
        let snap_axes = rules::snap_axes_for(match grip {
            Grip::Plane => None,
            _ => Some(axis),
        });
        let snap_reach = units_per_px(cam, pivot) * SNAP_TO_PARTS_PX;
        let data = self.contents();
        let before = data.as_ref().map(|d| d.bind().get_blob());
        let recipe = (job == Job::Resize).then(|| self.recipe_now()).flatten();
        let cage = (job == Job::Pivot)
            .then(|| data.as_ref().map(|d| d.bind().cage().clone()))
            .flatten();
        let kids = if job == Job::Pivot {
            target
                .get_children()
                .iter_shared()
                .filter_map(|c| c.try_cast::<Node3D>().ok())
                .map(|c| {
                    let t = c.get_transform();
                    (c, t)
                })
                .collect()
        } else {
            Vec::new()
        };
        self.rbx.hover = Some(grip);
        self.rbx.hold = Some(Hold {
            grip,
            job,
            pivot,
            axis,
            start_t,
            start_hit,
            start_angle,
            turned: 0.0,
            nodes,
            start_xf: xf,
            lo,
            hi,
            recipe,
            before,
            cage,
            kids,
            obstacles,
            neighbours,
            snap_axes,
            snap_reach,
            clear_delta: Vector3::ZERO,
            caption: String::new(),
            cursor: pos,
            turn: Basis::IDENTITY,
            wanted: Vector3::ZERO,
            unstepped: Vector3::ZERO,
        });
        self.base().update_overlays();
    }
    fn step_or_free(&self, v: f32) -> f32 {
        rules::snapped(v, self.snap_grid.then_some(self.grid_step))
    }
    fn move_step_along(&self, raw: f32, dir: Vector3) -> f32 {
        let low = self.rbx.hold.as_ref().map_or(0.0, |h| {
            let d = dir.normalized_or_zero();
            let turned = rules::dragged(h.start_xf, h.pivot, h.turn, Vector3::ZERO);
            box_corners(turned, h.lo, h.hi)
                .iter()
                .map(|c| c.dot(d))
                .fold(f32::INFINITY, f32::min)
        });
        rules::move_step(
            raw,
            low,
            self.snap_grid.then_some(self.grid_step),
            self.rbx.snap_boxes,
        )
    }
    fn roblox_hold_motion(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let Some(h) = self.rbx.hold.as_mut() else {
            return;
        };
        h.cursor = pos;
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let (grip, job, pivot, axis) = (h.grip, h.job, h.pivot, h.axis);
        match (job, grip) {
            (Job::Move, Grip::Arrow(..)) => {
                let Some(t) = closest_on_axis(pivot, axis, o, d) else {
                    return;
                };
                let raw = t - self.rbx.hold.as_ref().map_or(0.0, |h| h.start_t);
                let delta = axis * self.move_step_along(raw, axis);
                if let Some(h) = self.rbx.hold.as_mut() {
                    h.unstepped = axis * raw;
                }
                self.apply_move(delta);
            }
            (Job::Move, _) => {
                let Some(hit) =
                    plane_hit(pivot, axis, o, d).or_else(|| plane_hit(pivot, -axis, o, d))
                else {
                    return;
                };
                let start = self.rbx.hold.as_ref().map_or(hit, |h| h.start_hit);
                let raw = hit - start;
                let delta = Vector3::new(
                    self.move_step_along(raw.x, Vector3::RIGHT),
                    0.0,
                    self.move_step_along(raw.z, Vector3::BACK),
                );
                if let Some(h) = self.rbx.hold.as_mut() {
                    h.unstepped = Vector3::new(raw.x, 0.0, raw.z);
                }
                self.apply_move(delta);
            }
            (Job::Rotate, _) => {
                let Some(a) = ring_angle(pivot, axis, o, d) else {
                    return;
                };
                let Some(h) = self.rbx.hold.as_mut() else {
                    return;
                };
                h.turned += rules::wrap_angle(a - h.start_angle);
                h.start_angle = a;
                let ang = rules::snapped_angle(h.turned, self.rot_snap);
                self.apply_rotate(ang);
            }
            (Job::Resize, Grip::Ball(a, s)) => {
                let Some(t) = closest_on_axis(pivot, axis, o, d) else {
                    return;
                };
                let raw = t - self.rbx.hold.as_ref().map_or(0.0, |h| h.start_t);
                let mut deltas = Vector3::ZERO;
                let mut sides = [0i8; 3];
                sides[a] = s;
                match a {
                    0 => deltas.x = self.step_or_free(raw),
                    1 => deltas.y = self.step_or_free(raw),
                    _ => deltas.z = self.step_or_free(raw),
                }
                let sym = Input::singleton().is_key_pressed(Key::CTRL);
                self.apply_resize(deltas, sides, sym);
            }
            (Job::Resize, Grip::Corner(sides)) => {
                let Some(hit) = plane_hit(pivot, axis, o, d) else {
                    return;
                };
                let start = self.rbx.hold.as_ref().map_or(hit, |h| h.start_hit);
                let w = hit - start;
                let ax = self.rbx_start_part_axes();
                let deltas = Vector3::new(
                    self.step_or_free(w.dot(ax[0]) * sides[0] as f32),
                    self.step_or_free(w.dot(ax[1]) * sides[1] as f32),
                    self.step_or_free(w.dot(ax[2]) * sides[2] as f32),
                );
                let sym = Input::singleton().is_key_pressed(Key::CTRL);
                self.apply_resize(deltas, sides, sym);
            }
            (Job::Pivot, Grip::Arrow(..)) => {
                let Some(t) = closest_on_axis(pivot, axis, o, d) else {
                    return;
                };
                let raw = t - self.rbx.hold.as_ref().map_or(0.0, |h| h.start_t);
                let along = if self.rbx.pivot_snap {
                    rules::snapped_to_grid_along(pivot.dot(axis), raw, self.grid_step)
                } else {
                    raw
                };
                let Some(h) = self.rbx.hold.as_ref() else {
                    return;
                };
                let mut xf = h.start_xf;
                xf.origin += axis * along;
                self.apply_pivot(xf, format!("{along:+.2} m"));
            }
            (Job::Pivot, Grip::Ring(_)) => {
                let Some(a) = ring_angle(pivot, axis, o, d) else {
                    return;
                };
                let Some(h) = self.rbx.hold.as_mut() else {
                    return;
                };
                h.turned += rules::wrap_angle(a - h.start_angle);
                h.start_angle = a;
                let ang = rules::snapped_angle(h.turned, self.rot_snap);
                let s = h.start_xf;
                let r = Basis::from_axis_angle(axis, ang);
                let xf = Transform3D::new(r * s.basis, s.origin);
                self.apply_pivot(xf, format!("{:.1}°", ang.to_degrees()));
            }
            _ => {}
        }
        self.base().update_overlays();
    }
    fn rbx_start_part_axes(&self) -> [Vector3; 3] {
        let b = self
            .rbx
            .hold
            .as_ref()
            .map_or(self.xform().basis, |h| h.start_xf.basis);
        [
            b.col_a().normalized_or_zero(),
            b.col_b().normalized_or_zero(),
            b.col_c().normalized_or_zero(),
        ]
    }
    fn apply_move(&mut self, wanted: Vector3) {
        let Some(h) = self.rbx.hold.as_mut() else {
            return;
        };
        h.wanted = wanted;
        let mut delta = wanted;
        if !h.neighbours.is_empty()
            && let Some((first, start, _)) = h.nodes.first()
            && self
                .target
                .as_ref()
                .is_some_and(|t| t.instance_id() == first.instance_id())
        {
            let from = world_box(
                rules::dragged(*start, h.pivot, h.turn, Vector3::ZERO),
                h.lo,
                h.hi,
            );
            delta = rules::snap_to_neighbours(
                from,
                &h.neighbours,
                delta,
                h.unstepped,
                h.snap_axes,
                h.snap_reach,
            );
        }
        if !h.obstacles.is_empty()
            && let Some((first, start, _)) = h.nodes.first()
            && self
                .target
                .as_ref()
                .is_some_and(|t| t.instance_id() == first.instance_id())
        {
            let from = world_box(
                rules::dragged(*start, h.pivot, h.turn, Vector3::ZERO),
                h.lo,
                h.hi,
            );
            delta = rules::collision_stop(from, &h.obstacles, h.clear_delta, delta);
            h.clear_delta = delta;
        }
        let (pivot, turn) = (h.pivot, h.turn);
        for (n, g, _) in &mut h.nodes {
            if n.is_instance_valid() {
                n.set_global_transform(rules::dragged(*g, pivot, turn, delta));
            }
        }
        h.caption = format!("({:.2}, {:.2}, {:.2})", delta.x, delta.y, delta.z);
    }
    fn apply_rotate(&mut self, ang: f32) {
        let Some(h) = self.rbx.hold.as_mut() else {
            return;
        };
        let (axis, p) = (h.axis, h.pivot);
        for (n, g, _) in &mut h.nodes {
            if n.is_instance_valid() {
                n.set_global_transform(rules::turned_about(*g, axis, ang, p));
            }
        }
        h.caption = format!("{:.1}°", ang.to_degrees());
    }
    fn apply_resize(&mut self, deltas: Vector3, sides: [i8; 3], sym: bool) {
        let Some(mut node) = self.target.clone() else {
            return;
        };
        let data = self.contents();
        let Some(h) = self.rbx.hold.as_mut() else {
            return;
        };
        let xf0 = h.start_xf;
        let world_ext = (h.hi - h.lo) * axis_lengths(xf0.basis);
        let f = grown_factors(world_ext, deltas, sym);
        let anchor = anchor_point(h.lo, h.hi, sides, sym);
        let new_ext = world_ext * f;
        h.caption = format!("{:.2} x {:.2} x {:.2}", new_ext.x, new_ext.y, new_ext.z);
        match (&h.recipe, data) {
            (Some(r), Some(mut data)) => {
                let mut spec = r.spec;
                spec.size = v3(spec.size.x * f.x, spec.size.y * f.y, spec.size.z * f.z);
                let mut g = shapes::produce(&spec, None);
                g.displace(r.offset);
                if g.facets.len() == r.looks.len() {
                    for (face, (stuff, tint, tc)) in g.facets.iter_mut().zip(&r.looks) {
                        face.stuff = *stuff;
                        face.tint = *tint;
                        face.tex_coord = *tc;
                    }
                } else if let Some((stuff, tint, _)) = r.looks.first() {
                    for face in &mut g.facets {
                        face.stuff = *stuff;
                        face.tint = *tint;
                    }
                }
                let Some((nlo, nhi)) = g.extent() else {
                    return;
                };
                let after = anchor_point(from_p(nlo), from_p(nhi), sides, sym);
                let xf = rules::regenerated_origin(xf0, anchor, after);
                {
                    let mut d = data.bind_mut();
                    d.substitute(g);
                    d.set_shape_spec(&spec);
                }
                data.emit_changed();
                node.set_global_transform(xf);
            }
            _ => node.set_global_transform(rules::node_scaled(xf0, f, anchor)),
        }
    }
    fn apply_pivot(&mut self, xf: Transform3D, caption: String) {
        let Some(mut node) = self.target.clone() else {
            return;
        };
        let Some(mut data) = self.contents() else {
            return;
        };
        let Some(h) = self.rbx.hold.as_mut() else {
            return;
        };
        let Some(cage) = &h.cage else {
            return;
        };
        let m = rules::pivot_compensation(h.start_xf, xf);
        let mut c = cage.clone();
        topology_edits::bake_transform(&mut c, &affine(m));
        data.bind_mut().substitute(c);
        data.emit_changed();
        node.set_global_transform(xf);
        for (k, t) in &mut h.kids {
            if k.is_instance_valid() {
                k.set_transform(m * *t);
            }
        }
        h.caption = caption;
    }
    pub(super) fn finish_roblox_hold(&mut self) -> bool {
        let Some(h) = self.rbx.hold.take() else {
            return false;
        };
        self.base().update_overlays();
        let moniker = match h.job {
            Job::Move => "Pixel Builder: Move part",
            Job::Rotate => "Pixel Builder: Rotate part",
            Job::Resize => "Pixel Builder: Resize part",
            Job::Pivot => "Pixel Builder: Edit pivot",
        };
        let moved: Vec<(Gd<Node3D>, Transform3D, Transform3D)> = h
            .nodes
            .iter()
            .filter(|(n, _, _)| n.is_instance_valid())
            .map(|(n, _, local)| (n.clone(), *local, n.get_transform()))
            .filter(|(_, a, b)| a != b)
            .collect();
        let data = self.contents();
        let after = data.as_ref().map(|d| d.bind().get_blob());
        let mesh_changed = matches!(h.job, Job::Resize | Job::Pivot) && after != h.before;
        if moved.is_empty() && !mesh_changed {
            return true;
        }
        let Some(mut ur) = self.open_action(moniker) else {
            return true;
        };
        if mesh_changed && let (Some(d), Some(before), Some(after)) = (&data, &h.before, &after) {
            ur.add_do_method(d, "set_blob", &[after.to_variant()]);
            ur.add_undo_method(d, "set_blob", &[before.to_variant()]);
            if let Some(r) = &h.recipe {
                ur.add_do_property(d, "form", &d.get("form"));
                ur.add_undo_property(d, "form", &r.old_form);
            }
        }
        for (n, old, new) in &moved {
            ur.add_do_property(n, "transform", &new.to_variant());
            ur.add_undo_property(n, "transform", &old.to_variant());
        }
        for (k, old) in &h.kids {
            if k.is_instance_valid() {
                ur.add_do_property(k, "transform", &k.get_transform().to_variant());
                ur.add_undo_property(k, "transform", &old.to_variant());
            }
        }
        ur.commit_action_ex().execute(false).done();
        self.redraw();
        true
    }
    pub(super) fn roblox_drag_key(&mut self, cam: &Gd<Camera3D>, key: &Gd<InputEventKey>) -> bool {
        if !self.is_roblox() {
            return false;
        }
        let moving = self.rbx.hold.as_ref().is_some_and(|h| h.job == Job::Move);
        let code = match key.get_keycode() {
            Key::NONE => key.get_physical_keycode(),
            k => k,
        };
        let letter = match code {
            Key::R => Some('r'),
            Key::T => Some('t'),
            _ => None,
        };
        let modified = key.is_ctrl_pressed()
            || key.is_alt_pressed()
            || key.is_shift_pressed()
            || key.is_meta_pressed();
        let Some(turn) = rules::drag_turn_key(moving, letter, modified) else {
            return false;
        };
        if key.is_pressed() && !key.is_echo() {
            let axis = rules::quarter_turn_axis(turn, cam.get_global_transform().basis.col_a());
            let wanted = match self.rbx.hold.as_mut() {
                Some(h) => {
                    h.turn = rules::quarter_turned(h.turn, axis);
                    h.wanted
                }
                None => return true,
            };
            self.apply_move(wanted);
            self.base().update_overlays();
        }
        true
    }
    pub(super) fn cancel_roblox_hold(&mut self) -> bool {
        let Some(h) = self.rbx.hold.take() else {
            return false;
        };
        self.rbx.swallow_drag = Input::singleton().is_mouse_button_pressed(MouseButton::LEFT);
        for (n, g, _) in &h.nodes {
            if n.is_instance_valid() {
                let mut n = n.clone();
                n.set_global_transform(*g);
            }
        }
        for (k, t) in &h.kids {
            if k.is_instance_valid() {
                let mut k = k.clone();
                k.set_transform(*t);
            }
        }
        if let (Some(mut d), Some(before)) = (self.contents(), h.before) {
            if d.bind().get_blob() != before {
                d.bind_mut().set_blob(before);
            }
            if let Some(r) = &h.recipe {
                d.set("form", &r.old_form);
            }
        }
        self.redraw();
        true
    }
    pub(super) fn extrude_normal(&self) -> Option<Vector3> {
        let d = self.contents()?;
        let db = d.bind();
        let m = db.cage();
        let sum = self
            .sel_faces
            .iter()
            .filter(|f| **f < m.facets.len())
            .fold(PVec3::ZERO, |a, f| a + m.face_normal(*f));
        let n = (self.xform().basis.inverse().transposed() * from_p(sum)).normalized_or_zero();
        (n.length_squared() > 1e-8).then_some(n)
    }
    pub(super) fn extrude_ball(&self, cam: &Gd<Camera3D>) -> Option<(Vector2, Vector3)> {
        if !self.is_roblox()
            || self.mode != Mode::Face
            || self.xform_mode != XformMode::Scale
            || self.sel_faces.is_empty()
        {
            return None;
        }
        let n = self
            .drag
            .as_ref()
            .filter(|d| d.handle == Some(Handle::Extrude))
            .map_or_else(|| self.extrude_normal(), |d| Some(d.axes[0]))?;
        let base = self
            .drag
            .as_ref()
            .map(|d| d.fulcrum + d.last_delta)
            .or_else(|| self.fulcrum())?;
        let tip = base + n * (BALL_GAP_PX * 3.0) * units_per_px(cam, base);
        Some((screen(cam, tip)?, n))
    }
    pub(super) fn draw_extrude_ball(
        &self,
        overlay: &mut Gd<Control>,
        cam: &Gd<Camera3D>,
        hot: bool,
    ) {
        let Some((p, n)) = self.extrude_ball(cam) else {
            return;
        };
        let i = [n.x.abs(), n.y.abs(), n.z.abs()]
            .iter()
            .enumerate()
            .fold((0, -1.0f32), |b, (i, v)| if *v > b.1 { (i, *v) } else { b })
            .0;
        let col = if hot {
            AXIS_COLORS[i].lightened(0.4505)
        } else {
            AXIS_COLORS[i]
        };
        let r = BALL_PX + if hot { 2.0 } else { 0.0 };
        overlay.draw_circle(p, r + 1.5005, Color::from_rgba(0.0, 0.0, 0.0, 0.5));
        overlay.draw_circle(p, r, col);
    }
    pub(super) fn roblox_handles_debug(&self) -> VarDictionary {
        let mut out = VarDictionary::new();
        if let (Some((lo, hi)), Some(t)) = (self.target_box(), self.target.as_ref()) {
            let (wlo, whi) = world_box(t.get_global_transform(), lo, hi);
            out.set("aabb_min", wlo);
            out.set("aabb_max", whi);
        }
        out.set("shown", self.roblox_handles_shown());
        out.set("move_step", self.grid_step);
        let cam = self.camera.clone().filter(|c| c.is_instance_valid());
        let mut grips = VarArray::new();
        if let Some(cam) = cam {
            for (g, pts) in self.grips(&cam) {
                let mut e = VarDictionary::new();
                e.set("grip", format!("{g:?}").as_str());
                let at = match g {
                    Grip::Arrow(..) => pts[0].lerp(pts[1], 0.7),
                    Grip::Ring(_) => pts[pts.len() / 2],
                    _ => pts[0],
                };
                e.set("at", at);
                grips.push(&e.to_variant());
            }
            if let Some((p, _)) = self.extrude_ball(&cam) {
                let mut e = VarDictionary::new();
                e.set("grip", "Extrude");
                e.set("at", p);
                grips.push(&e.to_variant());
            }
        }
        out.set("grips", &grips);
        out
    }
}
