//! Builder plugin, Xogot Modeler port: the Bezier Plane / Block tool.
//!
//! Ports of xogot-rust (MIT):
//! * the tool flow of `editor_core/src/tool_state.rs` (`append_knot` with auto-smoothing of the
//!   previous knot until one of its tangents is edited by hand, `begin_bezier_height`,
//!   `bezier_preview`: a block still being drawn previews flat) and the instructions of
//!   `drawing_instruction`;
//! * `editor_core/src/bezier_move_session.rs` (`BezierMoveSession`: a knot / tangent move whose
//!   X / Y / Z keys cycle a world → local → no axis constraint, Shift+X / Y / Z a plane, digits
//!   type an exact distance, grid snap in the constraint space) and its driver in
//!   `xogot_modeler/src/shared/tool_controller.rs` (`begin_bezier_move`,
//!   `forward_bezier_move_input`: G / X / Y / Z start or steer a move, click / Enter commits,
//!   Esc / right-click restores).
//!
//! Over `pt_core::build::bezier`: click the ground / a surface to add knots (grid / cell snapping
//! and Draw on surface like the shape tool), click the curve to insert a knot, drag knots and
//! the selected knot's tangent handles (a drag is a move session too, so X / Y / Z constrain it),
//! Backspace / Del removes the selected (or last) knot, Enter creates (a block first asks for its
//! height), Esc / right-click cancels. The recipe is stored on the mesh data
//! (`PixelBuilderMeshData::bezier`, knots node-local) so *Shape parameters* reopens it.

use godot::classes::{Camera3D, Control, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::bezier::{BezierRecipe, TangentMode};
use pt_core::build::snap;

use super::super::data::{from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::{BzOpts, Field, Tool};
use super::{PixelBuilderPlugin, Placement, plane_hit};

const KNOT_PX: f32 = 8.0;
const HANDLE_PX: f32 = 7.0;
const CURVE_PX: f32 = 7.0;
const CURVE: Color = Color::from_rgb(0.984, 0.749, 0.141);
const KNOT: Color = Color::from_rgb(0.95, 0.95, 0.95);
const SELECTED: Color = Color::from_rgb(1.0, 0.55, 0.15);
const TANGENT: Color = Color::from_rgb(0.369, 0.918, 0.831);
const AXIS_COLORS: [Color; 3] = [Color::from_rgb(0.96, 0.3, 0.33), Color::from_rgb(0.53, 0.84, 0.28), Color::from_rgb(0.3, 0.55, 0.96)];

/// A grabbable part of the curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Knot(usize),
    TanIn(usize),
    TanOut(usize),
}

impl Part {
    fn index(self) -> usize {
        match self {
            Part::Knot(i) | Part::TanIn(i) | Part::TanOut(i) => i,
        }
    }
    /// `BezierMoveSession::component`: 0 position, 1 incoming, 2 outgoing tangent.
    fn component(self) -> u8 {
        match self {
            Part::Knot(_) => 0,
            Part::TanIn(_) => 1,
            Part::TanOut(_) => 2,
        }
    }
}

/// `bezier_move_session.rs` `Constraint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constraint {
    axis: usize,
    /// Move in the plane perpendicular to `axis` (Shift+X / Y / Z).
    plane: bool,
    /// The drawing frame's axes instead of world axes.
    local: bool,
}

/// Port of `BezierMoveSession`: one knot / tangent move from the recipe before it.
pub struct MoveSession {
    original: BezierRecipe,
    part: Part,
    numeric: String,
    world_origin: Vector3,
    /// Orthonormal frame axes (the "local" constraint basis) and world → recipe-local basis.
    local_basis: Basis,
    world_to_local: Basis,
    view_normal: Vector3,
    plane_normal: Vector3,
    /// The drawing plane is edge-on to the start ray: free moves use the view plane.
    view_plane: bool,
    start: (Vector3, Vector3),
    constraint: Option<Constraint>,
    /// Started by dragging (ends on release) rather than by G / X / Y / Z (ends on click / Enter).
    drag: bool,
    press: Vector2,
    moved: bool,
}

fn axis_parameter(axis: Vector3, world_origin: Vector3, o: Vector3, d: Vector3) -> Option<f32> {
    let along = axis.dot(d);
    let den = 1.0 - along * along;
    if den <= 1e-6 {
        return None;
    }
    let off = o - world_origin;
    let t = (off.dot(axis) - along * off.dot(d)) / den;
    t.is_finite().then_some(t)
}

fn other_axes(axis: usize) -> [usize; 2] {
    match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

impl MoveSession {
    fn new(recipe: &BezierRecipe, part: Part, frame: Transform3D, cam: &Gd<Camera3D>, pos: Vector2, drag: bool) -> Option<MoveSession> {
        let k = recipe.knots.get(part.index())?;
        let local_point = match part {
            Part::Knot(_) => k.position,
            Part::TanIn(_) => k.position + k.tangent_in,
            Part::TanOut(_) => k.position + k.tangent_out,
        };
        let world_origin = frame * from_p(local_point);
        let b = frame.basis;
        let cols = [b.col_a(), b.col_b(), b.col_c()];
        if cols.iter().any(|c| c.length() < 1e-6) || b.determinant().abs() <= 1e-9 {
            return None;
        }
        let local_basis = Basis::from_cols(cols[0].normalized(), cols[1].normalized(), cols[2].normalized());
        let d = cam.project_ray_normal(pos);
        let plane_normal = local_basis.col_b().normalized();
        Some(MoveSession {
            original: recipe.clone(),
            part,
            numeric: String::new(),
            world_origin,
            local_basis,
            world_to_local: b.inverse(),
            view_normal: cam.get_global_transform().basis.col_c().normalized(),
            plane_normal,
            view_plane: plane_normal.dot(d).abs() < 0.2,
            start: (cam.project_ray_origin(pos), d),
            constraint: None,
            drag,
            press: pos,
            moved: false,
        })
    }

    /// `cycle_constraint`: world, then local, then none.
    fn cycle(&mut self, axis: usize, plane: bool) {
        self.constraint = match self.constraint {
            Some(c) if c.plane == plane && c.axis == axis => (!c.local).then_some(Constraint { axis, plane, local: true }),
            _ => Some(Constraint { axis, plane, local: false }),
        };
    }

    fn label(&self) -> String {
        match self.constraint {
            None => "Drawing plane".into(),
            Some(c) => format!("{} {}{}", if c.local { "Local" } else { "World" }, ["X", "Y", "Z"][c.axis], if c.plane { " plane" } else { "" }),
        }
    }

    fn basis(&self, local: bool) -> Basis {
        if local { self.local_basis } else { Basis::IDENTITY }
    }

    /// `numeric_direction`: the axis, the sum of a plane's two axes, or a drawing-plane direction.
    fn numeric_direction(&self) -> Vector3 {
        let Some(c) = self.constraint else {
            let n = self.plane_normal;
            let r = if n.dot(Vector3::RIGHT).abs() < 0.9 { Vector3::RIGHT } else { Vector3::BACK };
            return (r - n * r.dot(n)).normalized();
        };
        let b = self.basis(c.local);
        let cols = [b.col_a(), b.col_b(), b.col_c()];
        if !c.plane {
            return cols[c.axis];
        }
        let [a0, a1] = other_axes(c.axis);
        cols[a0] + cols[a1]
    }

    /// `snapped_displacement`: rounds the moved point onto the grid of the constraint space.
    fn snapped(&self, disp: Vector3, step: Option<f32>) -> Vector3 {
        let Some(step) = step.filter(|s| *s > 0.0 && s.is_finite()) else { return disp };
        if disp == Vector3::ZERO {
            return disp;
        }
        let local = self.constraint.is_some_and(|c| c.local);
        let to = if local { self.local_basis.inverse() } else { Basis::IDENTITY };
        let base = to * self.world_origin;
        let m = to * disp;
        let mut s = [0.0f32; 3];
        let (mv, bs) = ([m.x, m.y, m.z], [base.x, base.y, base.z]);
        for axis in 0..3 {
            let included = match self.constraint {
                None => true,
                Some(c) => (c.axis == axis) != c.plane,
            };
            if included {
                s[axis] = ((mv[axis] + bs[axis]) / step + 0.5).floor() * step - bs[axis];
            }
        }
        self.basis(local) * Vector3::new(s[0], s[1], s[2])
    }

    /// `pointer_displacement`.
    fn pointer(&self, o: Vector3, d: Vector3, step: Option<f32>) -> Option<Vector3> {
        let normal = match self.constraint {
            None => {
                if self.view_plane {
                    self.view_normal
                } else {
                    self.plane_normal
                }
            }
            Some(c) => {
                let b = self.basis(c.local);
                let cols = [b.col_a(), b.col_b(), b.col_c()];
                if !c.plane {
                    let axis = cols[c.axis];
                    let t0 = axis_parameter(axis, self.world_origin, self.start.0, self.start.1)?;
                    let t1 = axis_parameter(axis, self.world_origin, o, d)?;
                    return Some(self.snapped(axis * (t1 - t0), step));
                }
                let [a0, a1] = other_axes(c.axis);
                let n = cols[a0].cross(cols[a1]);
                if n.length() < 1e-6 {
                    return None;
                }
                n.normalized()
            }
        };
        let s = plane_hit(self.world_origin, normal, self.start.0, self.start.1)?;
        let c = plane_hit(self.world_origin, normal, o, d)?;
        let mut mv = c - s;
        if self.constraint.is_none() {
            mv -= self.plane_normal * mv.dot(self.plane_normal);
        }
        Some(self.snapped(mv, step))
    }

    /// `recipe`: the knot moved by the typed distance or the pointer, in recipe-local space.
    fn recipe(&self, o: Vector3, d: Vector3, step: Option<f32>) -> Option<BezierRecipe> {
        let disp = if self.numeric.is_empty() {
            self.pointer(o, d.normalized(), step)?
        } else {
            let v = if self.numeric == "-" { 0.0 } else { self.numeric.parse::<f32>().ok()? };
            self.numeric_direction() * v
        };
        let local = to_p(self.world_to_local * disp);
        if ![local.x, local.y, local.z].iter().all(|c| c.is_finite()) {
            return None;
        }
        let mut r = self.original.clone();
        let k = &mut r.knots[self.part.index()];
        match self.part {
            Part::TanIn(_) => {
                k.mode = TangentMode::Free;
                let v = k.tangent_in + local;
                k.set_tangent(v, true);
            }
            Part::TanOut(_) => {
                k.mode = TangentMode::Free;
                let v = k.tangent_out + local;
                k.set_tangent(v, false);
            }
            Part::Knot(_) => k.position = k.position + local,
        }
        Some(r)
    }

    fn type_key(&mut self, key: Key) -> bool {
        let digit = match key {
            Key::KEY_0 | Key::KP_0 => Some(0),
            Key::KEY_1 | Key::KP_1 => Some(1),
            Key::KEY_2 | Key::KP_2 => Some(2),
            Key::KEY_3 | Key::KP_3 => Some(3),
            Key::KEY_4 | Key::KP_4 => Some(4),
            Key::KEY_5 | Key::KP_5 => Some(5),
            Key::KEY_6 | Key::KP_6 => Some(6),
            Key::KEY_7 | Key::KP_7 => Some(7),
            Key::KEY_8 | Key::KP_8 => Some(8),
            Key::KEY_9 | Key::KP_9 => Some(9),
            _ => None,
        };
        if let Some(n) = digit {
            self.numeric.push_str(&n.to_string());
            return true;
        }
        match key {
            Key::MINUS | Key::KP_SUBTRACT => {
                if self.numeric.starts_with('-') {
                    self.numeric.remove(0);
                } else {
                    self.numeric.insert(0, '-');
                }
                true
            }
            Key::PERIOD | Key::KP_PERIOD => {
                if !self.numeric.contains('.') {
                    if self.numeric.is_empty() || self.numeric == "-" {
                        self.numeric.push('0');
                    }
                    self.numeric.push('.');
                }
                true
            }
            Key::BACKSPACE => {
                self.numeric.pop();
                true
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct BezierTool {
    pub opts: BzOpts,
    /// Knots in the drawing frame (node-local once created).
    pub recipe: BezierRecipe,
    /// Per knot: still auto-smoothed when the next knot is appended (`auto_smooth_knot_ids`).
    auto_smooth: Vec<bool>,
    /// Drawing frame: local -> world (the node's global transform while re-editing).
    pub frame: Transform3D,
    pub selected: Option<usize>,
    hover: Option<Part>,
    /// Block height stage (after Enter): mouse anchor.
    height_stage: Option<Vector2>,
    session: Option<MoveSession>,
    /// Next-knot preview point (world).
    cursor: Option<Vector3>,
    last_mouse: Vector2,
    last_ray: Option<(Vector3, Vector3)>,
    /// Re-editing this mesh's stored recipe (changes rebuild it live).
    pub editing: Option<Gd<PixelBuilderMesh>>,
}

impl BezierTool {
    /// Drops the curve being drawn and the re-edit (the settings stay).
    pub fn reset(&mut self) {
        let opts = self.opts;
        *self = BezierTool { opts, ..Default::default() };
    }
    /// Stops live interactions (viewport hidden): a running move is restored.
    pub fn cancel_live(&mut self) {
        if let Some(s) = self.session.take() {
            self.recipe = s.original;
        }
        self.height_stage = None;
    }
    /// Strip values (tangent / roll from the selected knot).
    pub fn values(&self) -> BzOpts {
        let mut o = self.opts;
        if let Some(k) = self.selected.and_then(|i| self.recipe.knots.get(i)) {
            o.tangent = k.mode as i64;
            o.roll = k.roll;
        }
        o
    }
    /// The settings into the recipe (closed, smooth, width, rows, columns, block height).
    fn apply_opts(&mut self) {
        let o = self.opts;
        let r = &mut self.recipe;
        r.closed = o.closed && r.knots.len() >= 3;
        r.smooth = o.smooth;
        r.set_width(o.width.max(2e-4));
        r.rows = o.rows.clamp(1, 128) as u32;
        r.columns = o.columns.clamp(1, 128) as u32;
        r.height = if o.block { o.height.max(0.01) } else { 0.0 };
        self.auto_smooth.resize(r.knots.len(), false);
    }
    fn world(&self, local: pt_core::math::Vec3) -> Vector3 {
        self.frame * from_p(local)
    }
    fn local(&self, world: Vector3) -> pt_core::math::Vec3 {
        to_p(self.frame.affine_inverse() * world)
    }
    fn up(&self) -> Vector3 {
        let u = self.frame.basis.col_b();
        if u.length() < 1e-6 { Vector3::UP } else { u.normalized() }
    }
    fn part_world(&self, p: Part) -> Option<Vector3> {
        let k = self.recipe.knots.get(p.index())?;
        Some(match p {
            Part::Knot(_) => self.world(k.position),
            Part::TanIn(_) => self.world(k.position + k.tangent_in),
            Part::TanOut(_) => self.world(k.position + k.tangent_out),
        })
    }
    /// `ToolState::append_knot`: the previous knot is smoothed while it is still auto-smooth.
    fn append(&mut self, l: pt_core::math::Vec3) {
        let smooth_prev = self.opts.smooth && self.auto_smooth.last().copied().unwrap_or(false);
        self.recipe.append_knot(l, smooth_prev);
        self.auto_smooth.resize(self.recipe.knots.len() - 1, false);
        self.auto_smooth.push(true);
        self.selected = Some(self.recipe.knots.len() - 1);
    }
    fn remove(&mut self, i: usize) {
        if i < self.recipe.knots.len() {
            self.recipe.knots.remove(i);
        }
        if i < self.auto_smooth.len() {
            self.auto_smooth.remove(i);
        }
    }
}

impl PixelBuilderPlugin {
    /// Strip field of the Bezier tool changed.
    pub(super) fn bezier_field(&mut self, f: Field, value: f64) {
        let v = value as f32;
        let on = value >= 0.5;
        let bz = &mut self.bz;
        match f {
            Field::BzKind => bz.opts.block = on,
            Field::BzClosed => bz.opts.closed = on,
            Field::BzSmooth => bz.opts.smooth = on,
            Field::BzWidth => bz.opts.width = v.max(0.01),
            Field::BzRows => bz.opts.rows = value as i64,
            Field::BzColumns => bz.opts.columns = value as i64,
            Field::BzHeight => bz.opts.height = v.max(0.01),
            Field::BzTangent => {
                if let Some(i) = bz.selected.filter(|i| *i < bz.recipe.knots.len()) {
                    bz.recipe.knots[i].set_mode(TangentMode::from_u8(value as u8).unwrap_or_default());
                    if let Some(a) = bz.auto_smooth.get_mut(i) {
                        *a = false;
                    }
                }
            }
            Field::BzRoll => {
                if let Some(k) = bz.selected.and_then(|i| bz.recipe.knots.get_mut(i)) {
                    k.roll = v;
                }
            }
            _ => return,
        }
        self.bz.apply_opts();
        if self.bz.editing.is_some() {
            self.bezier_rebuild("Pixel Builder: Bezier parameters");
        }
        self.base().update_overlays();
    }

    /// Re-edit of a mesh's stored Bezier recipe (from *Shape parameters*).
    pub(super) fn bezier_reedit(&mut self) -> bool {
        let (Some(t), Some(d)) = (self.target.clone(), self.data()) else { return false };
        let Some(r) = d.bind().bezier_recipe() else { return false };
        if self.mode != super::super::ui::Mode::Object {
            self.set_mode(super::super::ui::Mode::Object);
        }
        self.set_tool(Tool::Bezier);
        let bz = &mut self.bz;
        bz.opts.closed = r.closed;
        bz.opts.smooth = r.smooth;
        bz.opts.width = r.width();
        bz.opts.rows = r.rows as i64;
        bz.opts.columns = r.columns as i64;
        bz.opts.block = r.height > 0.0;
        if r.height > 0.0 {
            bz.opts.height = r.height;
        }
        bz.frame = t.get_global_transform();
        bz.auto_smooth = vec![false; r.knots.len()];
        bz.recipe = r;
        bz.editing = Some(t);
        self.message = "Editing the Bezier recipe: drag knots / tangents (X / Y / Z constrain), click the curve to insert, Del removes; Enter / Esc finishes".into();
        self.sync_ui();
        true
    }

    /// Writes the recipe into the re-edited mesh (one merged undo step per gesture).
    fn bezier_rebuild(&mut self, name: &str) {
        let Some(t) = self.bz.editing.clone().filter(|t| t.is_instance_valid()) else { return };
        let Some(mut data) = t.bind().data() else { return };
        self.bz.apply_opts();
        let mut m = match self.bz.recipe.generate() {
            Ok(m) if !m.faces.is_empty() => m,
            Ok(_) => {
                self.message = "Bezier: the settings make an empty mesh (kept the old one)".into();
                return;
            }
            Err(e) => {
                self.message = format!("Bezier: {e}");
                return;
            }
        };
        let (before, old_bz, keep) = {
            let d = data.bind();
            (d.get_blob(), data.get("bezier"), d.mesh().faces.first().map(|f| (f.material, f.color)))
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
            d.set_bezier_recipe(&self.bz.recipe);
        }
        data.emit_changed();
        let (after, new_bz) = (data.bind().get_blob(), data.get("bezier"));
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action_ex(name).merge_mode(godot::classes::undo_redo::MergeMode::ENDS).done();
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "bezier", &new_bz);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "bezier", &old_bz);
        ur.commit_action_ex().execute(false).done();
        self.message.clear();
        self.redraw();
    }

    /// Creates the drawn curve as a new builder mesh.
    pub(super) fn bezier_commit(&mut self) {
        self.bz.apply_opts();
        let m = match self.bz.recipe.generate() {
            Ok(m) => m,
            Err(e) => {
                self.message = format!("Bezier: {e}");
                return;
            }
        };
        let f = self.bz.frame;
        let place = Placement { point: f.origin, cell: None, basis: f.basis };
        let recipe = self.bz.recipe.clone();
        self.create_with_recipe(place, m, false, super::xg_io::Recipe::Bezier(recipe));
        self.bz.reset();
        self.base().update_overlays();
    }

    /// World point for a new knot under the mouse (placement, else the drawing plane).
    fn bezier_point(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Vector3> {
        if let Some(p) = self.placement(cam, pos) {
            return Some(p.point);
        }
        let up = self.bz.up();
        plane_hit(self.bz.frame.origin, up, cam.project_ray_origin(pos), cam.project_ray_normal(pos))
    }

    fn bezier_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Part> {
        let bz = &self.bz;
        let near = |w: Vector3, px: f32| !cam.is_position_behind(w) && cam.unproject_position(w).distance_to(pos) <= px;
        if let Some(i) = bz.selected {
            for p in [Part::TanIn(i), Part::TanOut(i)] {
                if bz.part_world(p).is_some_and(|w| near(w, HANDLE_PX)) {
                    return Some(p);
                }
            }
        }
        (0..bz.recipe.knots.len()).map(Part::Knot).find(|p| bz.part_world(*p).is_some_and(|w| near(w, KNOT_PX)))
    }

    /// Curve hit under the mouse (screen distance), for click-to-insert.
    fn bezier_curve_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<(usize, f32)> {
        use pt_core::build::bezier::{PICK_MARGIN, PICK_SAMPLES};
        let r = &self.bz.recipe;
        let mut best: Option<(f32, usize, f32)> = None;
        for seg in 0..r.segments() {
            for i in 0..=PICK_SAMPLES {
                let t = (i as f32 / PICK_SAMPLES as f32).clamp(PICK_MARGIN, 1.0 - PICK_MARGIN);
                let w = self.bz.world(r.eval(seg, t));
                if cam.is_position_behind(w) {
                    continue;
                }
                let d = cam.unproject_position(w).distance_to(pos);
                if d <= CURVE_PX && best.is_none_or(|b| d < b.0) {
                    best = Some((d, seg, t));
                }
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Starts a move session of `part` (drag or G / X / Y / Z).
    fn bezier_begin_move(&mut self, cam: &Gd<Camera3D>, part: Part, pos: Vector2, drag: bool) -> bool {
        match MoveSession::new(&self.bz.recipe, part, self.bz.frame, cam, pos, drag) {
            Some(s) => {
                self.bz.session = Some(s);
                self.bz.last_ray = Some((cam.project_ray_origin(pos), cam.project_ray_normal(pos)));
                true
            }
            None => {
                self.message = "The Bezier move is not valid here (degenerate frame or view)".into();
                false
            }
        }
    }

    /// `update_bezier_move`: applies the session to the recipe from the last pointer ray.
    fn bezier_update_move(&mut self) {
        let Some(s) = &self.bz.session else { return };
        let Some((o, d)) = self.bz.last_ray else { return };
        let step = self.snap_grid.then_some(self.grid_step);
        match s.recipe(o, d, step) {
            Some(r) => {
                self.bz.recipe = r;
                if s.part.component() != 0
                    && let Some(a) = self.bz.auto_smooth.get_mut(s.part.index())
                {
                    // `mark_bezier_tangent_edited`: hand-edited tangents stop the auto-smoothing.
                    *a = false;
                }
            }
            None => self.message = "The Bezier move is not valid for this constraint. Change the axis or enter a distance.".into(),
        }
    }

    /// `finish_bezier_move`: keeps the moved recipe (rebuilds a re-edited mesh).
    fn bezier_finish_move(&mut self) {
        if let Some(s) = self.bz.session.take()
            && self.bz.editing.is_some()
            && (s.moved || !s.numeric.is_empty() || s.constraint.is_some())
        {
            self.bezier_rebuild("Pixel Builder: Move Bezier knot");
        }
    }

    /// `cancel_bezier_move`: restores the recipe from before the move.
    fn bezier_cancel_move(&mut self) {
        if let Some(s) = self.bz.session.take() {
            self.bz.recipe = s.original;
        }
    }

    /// Bezier tool input. `Some(consumed)` when handled.
    pub(super) fn bezier_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        if let Ok(k) = event.clone().try_cast::<InputEventKey>() {
            return self.bezier_key(cam, &k);
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            self.bz.last_mouse = pos;
            return Some(self.bezier_motion(cam, pos));
        }
        let mb = event.clone().try_cast::<InputEventMouseButton>().ok()?;
        let pos = mb.get_position();
        self.bz.last_mouse = pos;
        match mb.get_button_index() {
            MouseButton::RIGHT if mb.is_pressed() => {
                if self.bz.session.is_some() {
                    self.bezier_cancel_move();
                } else if self.bz.height_stage.is_some() {
                    self.bz.height_stage = None;
                } else if self.bz.editing.is_some() {
                    self.set_tool(Tool::Select);
                } else if !self.bz.recipe.knots.is_empty() {
                    self.bz.reset();
                } else {
                    return None;
                }
                self.base().update_overlays();
                Some(true)
            }
            MouseButton::LEFT if mb.is_pressed() => {
                self.bezier_press(cam, pos);
                self.sync_ui();
                self.base().update_overlays();
                Some(true)
            }
            MouseButton::LEFT => {
                if self.bz.session.as_ref().is_some_and(|s| s.drag) {
                    self.bezier_finish_move();
                }
                self.base().update_overlays();
                Some(true)
            }
            _ => None,
        }
    }

    fn bezier_press(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.bz.session.is_some() {
            // G / X / Y / Z move: a click commits it.
            self.bezier_finish_move();
            return;
        }
        if self.bz.height_stage.is_some() {
            self.bezier_commit();
            return;
        }
        let n = self.bz.recipe.knots.len();
        if let Some(part) = self.bezier_hit(cam, pos) {
            if part == Part::Knot(0) && n >= 3 && !self.bz.recipe.closed && self.bz.editing.is_none() && self.bz.selected != Some(0) {
                // Clicking the first knot closes the loop (like the wall / poly tools).
                self.bz.opts.closed = true;
                self.bz.apply_opts();
                self.bz.recipe.smooth_knot(0);
                self.bz.recipe.smooth_knot(n - 1);
                self.bz.selected = Some(0);
                self.message = "Bezier: closed; Enter creates".into();
                return;
            }
            self.bz.selected = Some(part.index());
            self.bezier_begin_move(cam, part, pos, true);
            return;
        }
        if n >= 2
            && let Some((seg, t)) = self.bezier_curve_hit(cam, pos)
        {
            match self.bz.recipe.insert_knot(seg, t) {
                Ok(i) => {
                    self.bz.auto_smooth.insert(i.min(self.bz.auto_smooth.len()), false);
                    self.bz.selected = Some(i);
                    if self.bz.editing.is_some() {
                        self.bezier_rebuild("Pixel Builder: Insert Bezier knot");
                    }
                }
                Err(e) => self.message = format!("Bezier: {e}"),
            }
            return;
        }
        if self.bz.editing.is_some() && self.bz.recipe.closed {
            self.message = "Bezier: click the curve to insert a knot (the loop is closed)".into();
            return;
        }
        let Some(w) = self.bezier_point(cam, pos) else {
            self.message = "Nothing under the cursor".into();
            return;
        };
        if n == 0 && self.bz.editing.is_none() {
            // The first knot sets the drawing frame (Draw on surface: the face's frame).
            let basis = self.placement(cam, pos).map(|p| p.basis).unwrap_or(Basis::IDENTITY);
            self.bz.frame = Transform3D::new(basis, w);
        }
        let l = self.bz.local(w);
        self.bz.append(l);
        self.bz.apply_opts();
        if self.bz.editing.is_some() {
            self.bezier_rebuild("Pixel Builder: Add Bezier knot");
        }
    }

    fn bezier_motion(&mut self, cam: &Gd<Camera3D>, pos: Vector2) -> bool {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        if let Some(s) = &mut self.bz.session {
            if !s.moved && (!s.drag || s.press.distance_to(pos) >= 3.0) {
                s.moved = true;
            }
            if s.moved {
                self.bz.last_ray = Some((o, d));
                self.bezier_update_move();
            }
            self.base().update_overlays();
            return true;
        }
        if let Some(anchor) = self.bz.height_stage {
            let r = &self.bz.recipe;
            let n = r.knots.len().max(1) as f32;
            let centre = self.bz.world(r.knots.iter().fold(pt_core::math::Vec3::ZERO, |a, k| a + k.position) / n);
            let up = self.bz.up();
            let mut nrm = cam.get_global_transform().basis.col_c();
            nrm -= up * nrm.dot(up);
            let mut h = match (nrm.length() > 1e-4).then(|| plane_hit(centre, nrm.normalized(), o, d)).flatten() {
                Some(hit) => (hit - centre).dot(up),
                None => (anchor.y - pos.y) * 0.02,
            };
            if self.snap_grid {
                h = snap::snap_value(h, self.grid_step);
            }
            self.bz.opts.height = h.max(if self.snap_grid { self.grid_step } else { 0.01 });
            self.bz.apply_opts();
            self.base().update_overlays();
            return true;
        }
        let hover = self.bezier_hit(cam, pos);
        self.bz.hover = hover;
        self.bz.cursor = if hover.is_none() && self.bz.editing.is_none() { self.bezier_point(cam, pos) } else { None };
        self.base().update_overlays();
        false
    }

    fn bezier_key(&mut self, cam: &Gd<Camera3D>, k: &Gd<InputEventKey>) -> Option<bool> {
        if !k.is_pressed() || k.is_echo() || k.is_ctrl_pressed() || k.is_meta_pressed() {
            return None;
        }
        let key = k.get_keycode();
        // A running move (`forward_bezier_move_input`).
        if self.bz.session.is_some() {
            match key {
                Key::ESCAPE => self.bezier_cancel_move(),
                Key::ENTER | Key::KP_ENTER => self.bezier_finish_move(),
                Key::G => {
                    let pos = self.bz.last_mouse;
                    if let Some(s) = &mut self.bz.session {
                        s.start = (cam.project_ray_origin(pos), cam.project_ray_normal(pos));
                    }
                    self.bezier_update_move();
                }
                Key::X | Key::Y | Key::Z => {
                    let axis = match key {
                        Key::X => 0,
                        Key::Y => 1,
                        _ => 2,
                    };
                    let pos = self.bz.last_mouse;
                    if let Some(s) = &mut self.bz.session {
                        s.cycle(axis, k.is_shift_pressed());
                        // `restart_bezier_move_pointer` after a constraint change.
                        s.start = (cam.project_ray_origin(pos), cam.project_ray_normal(pos));
                        s.moved = true;
                    }
                    self.bz.recipe = self.bz.session.as_ref().map(|s| s.original.clone()).unwrap_or_default();
                    self.bezier_update_move();
                }
                other => {
                    let typed = self.bz.session.as_mut().is_some_and(|s| s.type_key(other));
                    if !typed {
                        return Some(true);
                    }
                    self.bezier_update_move();
                }
            }
            self.sync_ui();
            self.base().update_overlays();
            return Some(true);
        }
        let consumed = match key {
            Key::ESCAPE => {
                if self.bz.height_stage.is_some() {
                    self.bz.height_stage = None;
                } else if self.bz.editing.is_some() {
                    self.set_tool(Tool::Select);
                } else if !self.bz.recipe.knots.is_empty() {
                    self.bz.reset();
                } else {
                    return None;
                }
                true
            }
            Key::ENTER | Key::KP_ENTER => {
                if self.bz.editing.is_some() {
                    self.set_tool(Tool::Select);
                } else if self.bz.recipe.knots.len() < 2 {
                    self.message = "Bezier: click at least two knots".into();
                } else if self.bz.opts.block && self.bz.height_stage.is_none() {
                    // `begin_bezier_height`.
                    self.bz.height_stage = Some(self.bz.last_mouse);
                    self.bz.apply_opts();
                    self.message = "Bezier block: move the mouse for the height, click or Enter creates".into();
                } else {
                    self.bezier_commit();
                }
                true
            }
            Key::BACKSPACE | Key::DELETE => {
                let n = self.bz.recipe.knots.len();
                if n == 0 {
                    return None;
                }
                let i = self.bz.selected.filter(|i| *i < n).unwrap_or(n - 1);
                if self.bz.editing.is_some() {
                    if self.bz.recipe.remove_knot(i) {
                        if i < self.bz.auto_smooth.len() {
                            self.bz.auto_smooth.remove(i);
                        }
                        self.bezier_rebuild("Pixel Builder: Remove Bezier knot");
                    } else {
                        self.message = "Bezier: the curve needs at least two knots (three when closed)".into();
                    }
                } else {
                    self.bz.remove(i);
                    if self.bz.recipe.knots.len() < 3 {
                        self.bz.recipe.closed = false;
                        self.bz.opts.closed = false;
                    }
                }
                self.bz.selected = None;
                true
            }
            Key::G | Key::X | Key::Y | Key::Z if !k.is_alt_pressed() => {
                // `begin_bezier_move`: move the selected knot; X / Y / Z start constrained.
                let Some(i) = self.bz.selected.filter(|i| *i < self.bz.recipe.knots.len()) else { return None };
                let pos = self.bz.last_mouse;
                if self.bezier_begin_move(cam, Part::Knot(i), pos, false) {
                    let axis = match key {
                        Key::X => Some(0),
                        Key::Y => Some(1),
                        Key::Z => Some(2),
                        _ => None,
                    };
                    if let (Some(a), Some(s)) = (axis, self.bz.session.as_mut()) {
                        s.cycle(a, k.is_shift_pressed());
                    }
                }
                true
            }
            _ => return None,
        };
        if consumed {
            self.sync_ui();
            self.base().update_overlays();
        }
        Some(consumed)
    }

    /// Status line of the tool (`drawing_instruction`, move HUD label).
    pub(super) fn bezier_status(&self) -> String {
        let r = &self.bz.recipe;
        let kind = if self.bz.opts.block { "block" } else { "plane" };
        if let Some(s) = &self.bz.session {
            let typed = if s.numeric.is_empty() { String::new() } else { format!("  distance {}", s.numeric) };
            return format!(
                "Bezier: moving knot {} — {}{}  (X / Y / Z axis, Shift+X / Y / Z plane: world → local → off; type a distance; click / Enter keeps, Esc restores)",
                s.part.index(),
                s.label(),
                typed
            );
        }
        if self.bz.height_stage.is_some() {
            return format!("Bezier block: height {:.2} (move the mouse; click / Enter creates, Esc goes back)", self.bz.opts.height);
        }
        if self.bz.editing.is_some() {
            return format!("Bezier {kind} re-edit: {} knots  (drag knots / tangents, G / X / Y / Z moves the selected knot, click the curve to insert, Del removes, Enter / Esc finishes)", r.knots.len());
        }
        let last = if self.bz.opts.block { "Enter sets the block height" } else { "Enter creates the plane" };
        format!(
            "Bezier {kind}: {} knots{}  (click to add knots, drag handles or click the curve to insert a knot, Backspace removes, {last}, Esc cancels)",
            r.knots.len(),
            if r.closed { ", closed" } else { "" }
        )
    }

    /// Curve, knots, the selected knot's tangent handles and the mesh preview.
    pub(super) fn draw_bezier(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        if self.tool != Tool::Bezier {
            return;
        }
        let bz = &self.bz;
        let r = &bz.recipe;
        let proj = |w: Vector3| (!cam.is_position_behind(w)).then(|| cam.unproject_position(w));
        // Mesh preview (`bezier_preview`: a block still being drawn previews flat).
        if r.knots.len() >= 2 && (bz.editing.is_none() || bz.session.as_ref().is_some_and(|s| s.moved)) {
            let mut preview = r.clone();
            if bz.opts.block && bz.height_stage.is_none() && bz.editing.is_none() {
                preview.height = 0.0;
            }
            if let Ok(m) = preview.generate() {
                for e in m.edges() {
                    if let (Some(a), Some(b)) = (proj(bz.world(m.positions[e.0 as usize])), proj(bz.world(m.positions[e.1 as usize]))) {
                        overlay.draw_line_ex(a, b, Color::from_rgba(0.5, 0.9, 1.0, 0.55)).width(1.0).done();
                    }
                }
            }
        }
        // Curve.
        for seg in 0..r.segments() {
            let mut prev: Option<Vector2> = None;
            for i in 0..=24 {
                let s = proj(bz.world(r.eval(seg, i as f32 / 24.0)));
                if let (Some(a), Some(b)) = (prev, s) {
                    overlay.draw_line_ex(a, b, CURVE).width(2.0).done();
                }
                prev = s;
            }
        }
        // Rubber band to the next knot.
        if let (Some(last), Some(c)) = (r.knots.last(), bz.cursor)
            && bz.session.is_none()
            && bz.height_stage.is_none()
            && let (Some(a), Some(b)) = (proj(bz.world(last.position)), proj(c))
        {
            overlay.draw_dashed_line_ex(a, b, Color::from_rgba(1.0, 0.85, 0.4, 0.7)).width(1.0).dash(4.0).done();
        }
        // Constraint line / plane axes of a running move.
        if let Some(s) = &bz.session
            && let Some(c) = s.constraint
        {
            let b = s.basis(c.local);
            let cols = [b.col_a(), b.col_b(), b.col_c()];
            let p = bz.part_world(s.part).unwrap_or(s.world_origin);
            let axes: Vec<usize> = if c.plane { other_axes(c.axis).to_vec() } else { vec![c.axis] };
            for a in axes {
                let dir = cols[a].normalized();
                if let (Some(s0), Some(s1)) = (proj(p - dir * 1000.0), proj(p + dir * 1000.0)) {
                    overlay.draw_line_ex(s0, s1, AXIS_COLORS[a]).width(1.0).done();
                }
            }
        }
        // Knots and tangent handles.
        for (i, k) in r.knots.iter().enumerate() {
            let Some(s) = proj(bz.world(k.position)) else { continue };
            let sel = bz.selected == Some(i);
            if sel {
                for (p, part) in [(k.position + k.tangent_in, Part::TanIn(i)), (k.position + k.tangent_out, Part::TanOut(i))] {
                    if let Some(h) = proj(bz.world(p)) {
                        overlay.draw_line_ex(s, h, TANGENT).width(1.0).done();
                        let rr = if bz.hover == Some(part) { 5.0 } else { 4.0 };
                        overlay.draw_circle(h, rr, TANGENT);
                    }
                }
            }
            let rr = if bz.hover == Some(Part::Knot(i)) || sel { 5.0 } else { 4.0 };
            overlay.draw_rect(Rect2::new(s - Vector2::new(rr + 1.0, rr + 1.0), Vector2::new(rr * 2.0 + 2.0, rr * 2.0 + 2.0)), Color::from_rgba(0.1, 0.1, 0.1, 0.9));
            overlay.draw_rect(Rect2::new(s - Vector2::new(rr, rr), Vector2::new(rr * 2.0, rr * 2.0)), if sel { SELECTED } else { KNOT });
        }
    }
}

#[godot_api(secondary)]
impl PixelBuilderPlugin {
    /// Test / scripting hook: draw a Bezier curve through world `points` with the current tool
    /// settings and create it (`block` with `height`). Returns the status message.
    #[func]
    fn debug_bezier(&mut self, points: PackedVector3Array, closed: bool, block: bool, height: f32) -> GString {
        self.set_tool(Tool::Bezier);
        let pts = points.as_slice();
        let Some(first) = pts.first() else { return "no points".into() };
        self.bz.frame = Transform3D::new(Basis::IDENTITY, *first);
        for p in pts {
            let l = self.bz.local(*p);
            self.bz.append(l);
        }
        self.bz.opts.closed = closed;
        self.bz.opts.block = block;
        if block {
            self.bz.opts.height = height.max(0.01);
        }
        self.message.clear();
        self.bezier_commit();
        GString::from(self.message.as_str())
    }

    /// Test / scripting hook: re-edit the selected mesh's Bezier recipe and move knot `index` by a
    /// local `delta` (one undo step). False when the mesh has no recipe.
    #[func]
    fn debug_bezier_move_knot(&mut self, index: i64, delta: Vector3) -> bool {
        if self.bz.editing.is_none() && !self.bezier_reedit() {
            return false;
        }
        let Some(k) = self.bz.recipe.knots.get_mut(index.max(0) as usize) else { return false };
        k.position = k.position + to_p(delta);
        self.bezier_rebuild("Pixel Builder: Move Bezier knot");
        true
    }

    /// Test hook: the Bezier tool state (knots, editing, closed, selected, moving, knot positions
    /// in world space).
    #[func]
    fn debug_bezier_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("knots", self.bz.recipe.knots.len() as i64);
        d.set("editing", self.bz.editing.is_some());
        d.set("closed", self.bz.recipe.closed);
        d.set("selected", self.bz.selected.map(|s| s as i64).unwrap_or(-1));
        d.set("moving", self.bz.session.is_some());
        d.set("height_stage", self.bz.height_stage.is_some());
        let pts: PackedVector3Array = self.bz.recipe.knots.iter().map(|k| self.bz.world(k.position)).collect();
        d.set("points", &pts);
        d
    }
}
