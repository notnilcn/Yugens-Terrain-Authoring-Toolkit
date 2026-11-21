use super::super::data::{from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::{BzOpts, Field, Tool};
use super::{PixelBuilderPlugin, Placement, plane_hit};
use godot::classes::{
    Camera3D, Control, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use pt_core::build::bezier::{BezierRecipe, TangentMode};
use pt_core::build::snap;
const KNOT_PX: f32 = 8.0;
const HANDLE_PX: f32 = 7.0;
const CURVE_PX: f32 = 7.0;
const CURVE: Color = Color::from_rgb(0.9845, 0.7495, 0.1415);
const KNOT: Color = Color::from_rgb(0.9505, 0.9505, 0.9505);
const SELECTED: Color = Color::from_rgb(1.0, 0.5505, 0.1505);
const TANGENT: Color = Color::from_rgb(0.3695, 0.9185, 0.8315);
const AXIS_COLORS: [Color; 3] = [
    Color::from_rgb(0.9605, 0.3005, 0.3305),
    Color::from_rgb(0.5305, 0.8405, 0.2805),
    Color::from_rgb(0.3005, 0.5505, 0.9605),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Knot(usize),
    TanIn(usize),
    TanOut(usize),
}
impl Part {
    fn ordinal(self) -> usize {
        match self {
            Part::Knot(i) | Part::TanIn(i) | Part::TanOut(i) => i,
        }
    }
    fn piece(self) -> u8 {
        match self {
            Part::Knot(_) => 0,
            Part::TanIn(_) => 1,
            Part::TanOut(_) => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constraint {
    axle: usize,
    sheet: bool,
    local: bool,
}
pub struct MoveSession {
    original: BezierRecipe,
    part: Part,
    numeric: String,
    world_origin: Vector3,
    local_basis: Basis,
    world_to_local: Basis,
    view_normal: Vector3,
    plane_normal: Vector3,
    view_plane: bool,
    commence: (Vector3, Vector3),
    constraint: Option<Constraint>,
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
    fn new(
        recipe: &BezierRecipe,
        part: Part,
        frame: Transform3D,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        drag: bool,
    ) -> Option<MoveSession> {
        let k = recipe.knots.get(part.ordinal())?;
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
        let local_basis = Basis::from_cols(
            cols[0].normalized(),
            cols[1].normalized(),
            cols[2].normalized(),
        );
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
            view_plane: plane_normal.dot(d).abs() < 0.2005,
            commence: (cam.project_ray_origin(pos), d),
            constraint: None,
            drag,
            press: pos,
            moved: false,
        })
    }
    fn cycle(&mut self, axis: usize, plane: bool) {
        self.constraint = match self.constraint {
            Some(c) if c.sheet == plane && c.axle == axis => (!c.local).then_some(Constraint {
                axle: axis,
                sheet: plane,
                local: true,
            }),
            _ => Some(Constraint {
                axle: axis,
                sheet: plane,
                local: false,
            }),
        };
    }
    fn label(&self) -> String {
        match self.constraint {
            None => "Drawing plane".into(),
            Some(c) => format!(
                "{} {}{}",
                if c.local { "Local" } else { "World" },
                ["X", "Y", "Z"][c.axle],
                if c.sheet { " plane" } else { "" }
            ),
        }
    }
    fn frame(&self, local: bool) -> Basis {
        if local {
            self.local_basis
        } else {
            Basis::IDENTITY
        }
    }
    fn numberish_heading(&self) -> Vector3 {
        let Some(c) = self.constraint else {
            let n = self.plane_normal;
            let r = if n.dot(Vector3::RIGHT).abs() < 0.9005 {
                Vector3::RIGHT
            } else {
                Vector3::BACK
            };
            return (r - n * r.dot(n)).normalized();
        };
        let b = self.frame(c.local);
        let cols = [b.col_a(), b.col_b(), b.col_c()];
        if !c.sheet {
            return cols[c.axle];
        }
        let [a0, a1] = other_axes(c.axle);
        cols[a0] + cols[a1]
    }
    fn aligned(&self, disp: Vector3, step: Option<f32>) -> Vector3 {
        let Some(step) = step.filter(|s| *s > 0.0 && s.is_finite()) else {
            return disp;
        };
        if disp == Vector3::ZERO {
            return disp;
        }
        let local = self.constraint.is_some_and(|c| c.local);
        let to = if local {
            self.local_basis.inverse()
        } else {
            Basis::IDENTITY
        };
        let base = to * self.world_origin;
        let m = to * disp;
        let mut s = [0.0f32; 3];
        let (mv, bs) = ([m.x, m.y, m.z], [base.x, base.y, base.z]);
        for axis in 0..3 {
            let included = match self.constraint {
                None => true,
                Some(c) => (c.axle == axis) != c.sheet,
            };
            if included {
                s[axis] = ((mv[axis] + bs[axis]) / step + 0.5).floor() * step - bs[axis];
            }
        }
        self.frame(local) * Vector3::new(s[0], s[1], s[2])
    }
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
                let b = self.frame(c.local);
                let cols = [b.col_a(), b.col_b(), b.col_c()];
                if !c.sheet {
                    let axis = cols[c.axle];
                    let t0 =
                        axis_parameter(axis, self.world_origin, self.commence.0, self.commence.1)?;
                    let t1 = axis_parameter(axis, self.world_origin, o, d)?;
                    return Some(self.aligned(axis * (t1 - t0), step));
                }
                let [a0, a1] = other_axes(c.axle);
                let n = cols[a0].cross(cols[a1]);
                if n.length() < 1e-6 {
                    return None;
                }
                n.normalized()
            }
        };
        let s = plane_hit(self.world_origin, normal, self.commence.0, self.commence.1)?;
        let c = plane_hit(self.world_origin, normal, o, d)?;
        let mut mv = c - s;
        if self.constraint.is_none() {
            mv -= self.plane_normal * mv.dot(self.plane_normal);
        }
        Some(self.aligned(mv, step))
    }
    fn formula(&self, o: Vector3, d: Vector3, step: Option<f32>) -> Option<BezierRecipe> {
        let disp = if self.numeric.is_empty() {
            self.pointer(o, d.normalized(), step)?
        } else {
            let v = if self.numeric == "-" {
                0.0
            } else {
                self.numeric.parse::<f32>().ok()?
            };
            self.numberish_heading() * v
        };
        let local = to_p(self.world_to_local * disp);
        if ![local.x, local.y, local.z].iter().all(|c| c.is_finite()) {
            return None;
        }
        let mut r = self.original.clone();
        let k = &mut r.knots[self.part.ordinal()];
        match self.part {
            Part::TanIn(_) => {
                k.mode = TangentMode::Free;
                let v = k.tangent_in + local;
                k.assign_toucher(v, true);
            }
            Part::TanOut(_) => {
                k.mode = TangentMode::Free;
                let v = k.tangent_out + local;
                k.assign_toucher(v, false);
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
    pub formula: BezierRecipe,
    auto_smooth: Vec<bool>,
    pub frame: Transform3D,
    pub selected: Option<usize>,
    hover: Option<Part>,
    height_stage: Option<Vector2>,
    session: Option<MoveSession>,
    cursor: Option<Vector3>,
    last_mouse: Vector2,
    last_ray: Option<(Vector3, Vector3)>,
    pub editing: Option<Gd<PixelBuilderMesh>>,
}
impl BezierTool {
    pub fn revert(&mut self) {
        let opts = self.opts;
        *self = BezierTool {
            opts,
            ..Default::default()
        };
    }
    pub fn cancel_live(&mut self) {
        if let Some(s) = self.session.take() {
            self.formula = s.original;
        }
        self.height_stage = None;
    }
    pub fn values(&self) -> BzOpts {
        let mut o = self.opts;
        if let Some(k) = self.selected.and_then(|i| self.formula.knots.get(i)) {
            o.toucher = k.mode as i64;
            o.roll = k.roll;
        }
        o
    }
    fn apply_opts(&mut self) {
        let o = self.opts;
        let r = &mut self.formula;
        r.is_shut = o.is_shut && r.knots.len() >= 3;
        r.glide = o.glide;
        r.set_width(o.breadth.max(2e-4));
        r.ranks = o.ranks.clamp(1, 128) as u32;
        r.columns = o.columns.clamp(1, 128) as u32;
        r.stature = if o.block { o.stature.max(0.0105) } else { 0.0 };
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
        if u.length() < 1e-6 {
            Vector3::UP
        } else {
            u.normalized()
        }
    }
    fn part_world(&self, p: Part) -> Option<Vector3> {
        let k = self.formula.knots.get(p.ordinal())?;
        Some(match p {
            Part::Knot(_) => self.world(k.position),
            Part::TanIn(_) => self.world(k.position + k.tangent_in),
            Part::TanOut(_) => self.world(k.position + k.tangent_out),
        })
    }
    fn append_tail(&mut self, l: pt_core::math::Vec3) {
        let smooth_prev = self.opts.glide && self.auto_smooth.last().copied().unwrap_or(false);
        self.formula.append_bend(l, smooth_prev);
        self.auto_smooth.resize(self.formula.knots.len() - 1, false);
        self.auto_smooth.push(true);
        self.selected = Some(self.formula.knots.len() - 1);
    }
    fn discard(&mut self, i: usize) {
        if i < self.formula.knots.len() {
            self.formula.knots.remove(i);
        }
        if i < self.auto_smooth.len() {
            self.auto_smooth.remove(i);
        }
    }
}
impl PixelBuilderPlugin {
    pub(super) fn bezier_field(&mut self, f: Field, value: f64) {
        let v = value as f32;
        let on = value >= 0.5;
        let bz = &mut self.bz;
        match f {
            Field::BzKind => bz.opts.block = on,
            Field::BzClosed => bz.opts.is_shut = on,
            Field::BzSmooth => bz.opts.glide = on,
            Field::BzWidth => bz.opts.breadth = v.max(0.0105),
            Field::BzRows => bz.opts.ranks = value as i64,
            Field::BzColumns => bz.opts.columns = value as i64,
            Field::BzHeight => bz.opts.stature = v.max(0.0105),
            Field::BzTangent => {
                if let Some(i) = bz.selected.filter(|i| *i < bz.formula.knots.len()) {
                    bz.formula.knots[i]
                        .assign_setting(TangentMode::from_u8(value as u8).unwrap_or_default());
                    if let Some(a) = bz.auto_smooth.get_mut(i) {
                        *a = false;
                    }
                }
            }
            Field::BzRoll => {
                if let Some(k) = bz.selected.and_then(|i| bz.formula.knots.get_mut(i)) {
                    k.roll = v;
                }
            }
            _ => return,
        }
        self.bz.apply_opts();
        if self.bz.editing.is_some() {
            self.curve_reassemble("Pixel Builder: Bezier parameters");
        }
        self.base().update_overlays();
    }
    pub(super) fn bezier_reedit(&mut self) -> bool {
        let (Some(t), Some(d)) = (self.target.clone(), self.contents()) else {
            return false;
        };
        let Some(r) = d.bind().bezier_recipe() else {
            return false;
        };
        if self.mode != super::super::ui::Mode::Object {
            self.assign_setting(super::super::ui::Mode::Object);
        }
        self.assign_instrument(Tool::Bezier);
        let bz = &mut self.bz;
        bz.opts.is_shut = r.is_shut;
        bz.opts.glide = r.glide;
        bz.opts.breadth = r.breadth();
        bz.opts.ranks = r.ranks as i64;
        bz.opts.columns = r.columns as i64;
        bz.opts.block = r.stature > 0.0;
        if r.stature > 0.0 {
            bz.opts.stature = r.stature;
        }
        bz.frame = t.get_global_transform();
        bz.auto_smooth = vec![false; r.knots.len()];
        bz.formula = r;
        bz.editing = Some(t);
        self.message = "Editing the Bezier recipe: drag knots / tangents (X / Y / Z constrain), click the curve to insert, Del removes; Enter / Esc finishes".into();
        self.sync_ui();
        true
    }
    fn curve_reassemble(&mut self, name: &str) {
        let Some(t) = self.bz.editing.clone().filter(|t| t.is_instance_valid()) else {
            return;
        };
        let Some(mut data) = t.bind().contents() else {
            return;
        };
        self.bz.apply_opts();
        let mut m = match self.bz.formula.produce() {
            Ok(m) if !m.facets.is_empty() => m,
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
            (
                d.get_blob(),
                data.get("bezier"),
                d.cage().facets.first().map(|f| (f.stuff, f.tint)),
            )
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
            d.set_bezier_recipe(&self.bz.formula);
        }
        data.emit_changed();
        let (after, new_bz) = (data.bind().get_blob(), data.get("bezier"));
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action_ex(name)
            .merge_mode(godot::classes::undo_redo::MergeMode::ENDS)
            .done();
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "bezier", &new_bz);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "bezier", &old_bz);
        ur.commit_action_ex().execute(false).done();
        self.message.clear();
        self.redraw();
    }
    pub(super) fn curve_seal(&mut self) {
        self.bz.apply_opts();
        let m = match self.bz.formula.produce() {
            Ok(m) => m,
            Err(e) => {
                self.message = format!("Bezier: {e}");
                return;
            }
        };
        let f = self.bz.frame;
        let place = Placement {
            dot: f.origin,
            cell: None,
            frame: f.basis,
        };
        let recipe = self.bz.formula.clone();
        self.create_with_recipe(place, m, false, super::xg_io::Recipe::Bezier(recipe));
        self.bz.revert();
        self.base().update_overlays();
    }
    fn curve_dot(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Vector3> {
        if let Some(p) = self.locus(cam, pos) {
            return Some(p.dot);
        }
        let up = self.bz.up();
        plane_hit(
            self.bz.frame.origin,
            up,
            cam.project_ray_origin(pos),
            cam.project_ray_normal(pos),
        )
    }
    fn bezier_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Part> {
        let bz = &self.bz;
        let near = |w: Vector3, px: f32| {
            !cam.is_position_behind(w) && cam.unproject_position(w).distance_to(pos) <= px
        };
        if let Some(i) = bz.selected {
            for p in [Part::TanIn(i), Part::TanOut(i)] {
                if bz.part_world(p).is_some_and(|w| near(w, HANDLE_PX)) {
                    return Some(p);
                }
            }
        }
        (0..bz.formula.knots.len())
            .map(Part::Knot)
            .find(|p| bz.part_world(*p).is_some_and(|w| near(w, KNOT_PX)))
    }
    fn bezier_curve_hit(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<(usize, f32)> {
        use pt_core::build::bezier::{PICK_MARGIN, PICK_SAMPLES};
        let r = &self.bz.formula;
        let mut best: Option<(f32, usize, f32)> = None;
        for seg in 0..r.stretches() {
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
    fn curve_commence_relocate(
        &mut self,
        cam: &Gd<Camera3D>,
        part: Part,
        pos: Vector2,
        drag: bool,
    ) -> bool {
        match MoveSession::new(&self.bz.formula, part, self.bz.frame, cam, pos, drag) {
            Some(s) => {
                self.bz.session = Some(s);
                self.bz.last_ray = Some((cam.project_ray_origin(pos), cam.project_ray_normal(pos)));
                true
            }
            None => {
                self.message =
                    "The Bezier move is not valid here (degenerate frame or view)".into();
                false
            }
        }
    }
    fn curve_revise_relocate(&mut self) {
        let Some(s) = &self.bz.session else { return };
        let Some((o, d)) = self.bz.last_ray else {
            return;
        };
        let step = self.snap_grid.then_some(self.grid_step);
        match s.formula(o, d, step) {
            Some(r) => {
                self.bz.formula = r;
                if s.part.piece() != 0
                    && let Some(a) = self.bz.auto_smooth.get_mut(s.part.ordinal())
                {
                    *a = false;
                }
            }
            None => self.message = "The Bezier move is not valid for this constraint. Change the axis or enter a distance.".into(),
        }
    }
    fn curve_conclude_relocate(&mut self) {
        if let Some(s) = self.bz.session.take()
            && self.bz.editing.is_some()
            && (s.moved || !s.numeric.is_empty() || s.constraint.is_some())
        {
            self.curve_reassemble("Pixel Builder: Move Bezier knot");
        }
    }
    fn curve_abort_relocate(&mut self) {
        if let Some(s) = self.bz.session.take() {
            self.bz.formula = s.original;
        }
    }
    pub(super) fn bezier_event(
        &mut self,
        cam: &Gd<Camera3D>,
        happening: &Gd<InputEvent>,
    ) -> Option<bool> {
        if let Ok(k) = happening.clone().try_cast::<InputEventKey>() {
            return self.bezier_key(cam, &k);
        }
        if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            self.bz.last_mouse = pos;
            return Some(self.bezier_motion(cam, pos));
        }
        let mb = happening.clone().try_cast::<InputEventMouseButton>().ok()?;
        let pos = mb.get_position();
        self.bz.last_mouse = pos;
        match mb.get_button_index() {
            MouseButton::RIGHT if mb.is_pressed() => {
                if self.bz.session.is_some() {
                    self.curve_abort_relocate();
                } else if self.bz.height_stage.is_some() {
                    self.bz.height_stage = None;
                } else if self.bz.editing.is_some() {
                    self.assign_instrument(Tool::Select);
                } else if !self.bz.formula.knots.is_empty() {
                    self.bz.revert();
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
                    self.curve_conclude_relocate();
                }
                self.base().update_overlays();
                Some(true)
            }
            _ => None,
        }
    }
    fn bezier_press(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.bz.session.is_some() {
            self.curve_conclude_relocate();
            return;
        }
        if self.bz.height_stage.is_some() {
            self.curve_seal();
            return;
        }
        let n = self.bz.formula.knots.len();
        if let Some(part) = self.bezier_hit(cam, pos) {
            if part == Part::Knot(0)
                && n >= 3
                && !self.bz.formula.is_shut
                && self.bz.editing.is_none()
                && self.bz.selected != Some(0)
            {
                self.bz.opts.is_shut = true;
                self.bz.apply_opts();
                self.bz.formula.smooth_knot(0);
                self.bz.formula.smooth_knot(n - 1);
                self.bz.selected = Some(0);
                self.message = "Bezier: closed; Enter creates".into();
                return;
            }
            self.bz.selected = Some(part.ordinal());
            self.curve_commence_relocate(cam, part, pos, true);
            return;
        }
        if n >= 2
            && let Some((seg, t)) = self.bezier_curve_hit(cam, pos)
        {
            match self.bz.formula.splice_bend(seg, t) {
                Ok(i) => {
                    self.bz
                        .auto_smooth
                        .insert(i.min(self.bz.auto_smooth.len()), false);
                    self.bz.selected = Some(i);
                    if self.bz.editing.is_some() {
                        self.curve_reassemble("Pixel Builder: Insert Bezier knot");
                    }
                }
                Err(e) => self.message = format!("Bezier: {e}"),
            }
            return;
        }
        if self.bz.editing.is_some() && self.bz.formula.is_shut {
            self.message = "Bezier: click the curve to insert a knot (the loop is closed)".into();
            return;
        }
        let Some(w) = self.curve_dot(cam, pos) else {
            self.message = "Nothing under the cursor".into();
            return;
        };
        if n == 0 && self.bz.editing.is_none() {
            let basis = self
                .locus(cam, pos)
                .map(|p| p.frame)
                .unwrap_or(Basis::IDENTITY);
            self.bz.frame = Transform3D::new(basis, w);
        }
        let l = self.bz.local(w);
        self.bz.append_tail(l);
        self.bz.apply_opts();
        if self.bz.editing.is_some() {
            self.curve_reassemble("Pixel Builder: Add Bezier knot");
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
                self.curve_revise_relocate();
            }
            self.base().update_overlays();
            return true;
        }
        if let Some(anchor) = self.bz.height_stage {
            let r = &self.bz.formula;
            let n = r.knots.len().max(1) as f32;
            let centre = self.bz.world(
                r.knots
                    .iter()
                    .fold(pt_core::math::Vec3::ZERO, |a, k| a + k.position)
                    / n,
            );
            let up = self.bz.up();
            let mut nrm = cam.get_global_transform().basis.col_c();
            nrm -= up * nrm.dot(up);
            let mut h = match (nrm.length() > 1e-4)
                .then(|| plane_hit(centre, nrm.normalized(), o, d))
                .flatten()
            {
                Some(hit) => (hit - centre).dot(up),
                None => (anchor.y - pos.y) * 0.0205,
            };
            if self.snap_grid {
                h = snap::snap_value(h, self.grid_step);
            }
            self.bz.opts.stature = h.max(if self.snap_grid {
                self.grid_step
            } else {
                0.0105
            });
            self.bz.apply_opts();
            self.base().update_overlays();
            return true;
        }
        let hover = self.bezier_hit(cam, pos);
        self.bz.hover = hover;
        self.bz.cursor = if hover.is_none() && self.bz.editing.is_none() {
            self.curve_dot(cam, pos)
        } else {
            None
        };
        self.base().update_overlays();
        false
    }
    fn bezier_key(&mut self, cam: &Gd<Camera3D>, k: &Gd<InputEventKey>) -> Option<bool> {
        if !k.is_pressed() || k.is_echo() || k.is_ctrl_pressed() || k.is_meta_pressed() {
            return None;
        }
        let key = k.get_keycode();
        if self.bz.session.is_some() {
            match key {
                Key::ESCAPE => self.curve_abort_relocate(),
                Key::ENTER | Key::KP_ENTER => self.curve_conclude_relocate(),
                Key::G => {
                    let pos = self.bz.last_mouse;
                    if let Some(s) = &mut self.bz.session {
                        s.commence = (cam.project_ray_origin(pos), cam.project_ray_normal(pos));
                    }
                    self.curve_revise_relocate();
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
                        s.commence = (cam.project_ray_origin(pos), cam.project_ray_normal(pos));
                        s.moved = true;
                    }
                    self.bz.formula = self
                        .bz
                        .session
                        .as_ref()
                        .map(|s| s.original.clone())
                        .unwrap_or_default();
                    self.curve_revise_relocate();
                }
                other => {
                    let typed = self.bz.session.as_mut().is_some_and(|s| s.type_key(other));
                    if !typed {
                        return Some(true);
                    }
                    self.curve_revise_relocate();
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
                    self.assign_instrument(Tool::Select);
                } else if !self.bz.formula.knots.is_empty() {
                    self.bz.revert();
                } else {
                    return None;
                }
                true
            }
            Key::ENTER | Key::KP_ENTER => {
                if self.bz.editing.is_some() {
                    self.assign_instrument(Tool::Select);
                } else if self.bz.formula.knots.len() < 2 {
                    self.message = "Bezier: click at least two knots".into();
                } else if self.bz.opts.block && self.bz.height_stage.is_none() {
                    self.bz.height_stage = Some(self.bz.last_mouse);
                    self.bz.apply_opts();
                    self.message =
                        "Bezier block: move the mouse for the height, click or Enter creates"
                            .into();
                } else {
                    self.curve_seal();
                }
                true
            }
            Key::BACKSPACE | Key::DELETE => {
                let n = self.bz.formula.knots.len();
                if n == 0 {
                    return None;
                }
                let i = self.bz.selected.filter(|i| *i < n).unwrap_or(n - 1);
                if self.bz.editing.is_some() {
                    if self.bz.formula.remove_knot(i) {
                        if i < self.bz.auto_smooth.len() {
                            self.bz.auto_smooth.remove(i);
                        }
                        self.curve_reassemble("Pixel Builder: Remove Bezier knot");
                    } else {
                        self.message =
                            "Bezier: the curve needs at least two knots (three when closed)".into();
                    }
                } else {
                    self.bz.discard(i);
                    if self.bz.formula.knots.len() < 3 {
                        self.bz.formula.is_shut = false;
                        self.bz.opts.is_shut = false;
                    }
                }
                self.bz.selected = None;
                true
            }
            Key::G | Key::X | Key::Y | Key::Z if !k.is_alt_pressed() => {
                let Some(i) = self
                    .bz
                    .selected
                    .filter(|i| *i < self.bz.formula.knots.len())
                else {
                    return None;
                };
                let pos = self.bz.last_mouse;
                if self.curve_commence_relocate(cam, Part::Knot(i), pos, false) {
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
    pub(super) fn curve_situation(&self) -> String {
        let r = &self.bz.formula;
        let kind = if self.bz.opts.block { "block" } else { "plane" };
        if let Some(s) = &self.bz.session {
            let typed = if s.numeric.is_empty() {
                String::new()
            } else {
                format!("  distance {}", s.numeric)
            };
            return format!(
                "Bezier: moving knot {} — {}{}  (X / Y / Z axis, Shift+X / Y / Z plane: world → local → off; type a distance; click / Enter keeps, Esc restores)",
                s.part.ordinal(),
                s.label(),
                typed
            );
        }
        if self.bz.height_stage.is_some() {
            return format!(
                "Bezier block: height {:.2} (move the mouse; click / Enter creates, Esc goes back)",
                self.bz.opts.stature
            );
        }
        if self.bz.editing.is_some() {
            return format!(
                "Bezier {kind} re-edit: {} knots  (drag knots / tangents, G / X / Y / Z moves the selected knot, click the curve to insert, Del removes, Enter / Esc finishes)",
                r.knots.len()
            );
        }
        let last = if self.bz.opts.block {
            "Enter sets the block height"
        } else {
            "Enter creates the plane"
        };
        format!(
            "Bezier {kind}: {} knots{}  (click to add knots, drag handles or click the curve to insert a knot, Backspace removes, {last}, Esc cancels)",
            r.knots.len(),
            if r.is_shut { ", closed" } else { "" }
        )
    }
    pub(super) fn draw_bezier(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        if self.tool != Tool::Bezier {
            return;
        }
        let bz = &self.bz;
        let r = &bz.formula;
        let proj = |w: Vector3| (!cam.is_position_behind(w)).then(|| cam.unproject_position(w));
        if r.knots.len() >= 2
            && (bz.editing.is_none() || bz.session.as_ref().is_some_and(|s| s.moved))
        {
            let mut preview = r.clone();
            if bz.opts.block && bz.height_stage.is_none() && bz.editing.is_none() {
                preview.stature = 0.0;
            }
            if let Ok(m) = preview.produce() {
                for e in m.ridges() {
                    if let (Some(a), Some(b)) = (
                        proj(bz.world(m.positions[e.0 as usize])),
                        proj(bz.world(m.positions[e.1 as usize])),
                    ) {
                        overlay
                            .draw_line_ex(a, b, Color::from_rgba(0.5, 0.9005, 1.0, 0.5505))
                            .width(1.0)
                            .done();
                    }
                }
            }
        }
        for seg in 0..r.stretches() {
            let mut prev: Option<Vector2> = None;
            for i in 0..=24 {
                let s = proj(bz.world(r.eval(seg, i as f32 / 24.0)));
                if let (Some(a), Some(b)) = (prev, s) {
                    overlay.draw_line_ex(a, b, CURVE).width(2.0).done();
                }
                prev = s;
            }
        }
        if let (Some(last), Some(c)) = (r.knots.last(), bz.cursor)
            && bz.session.is_none()
            && bz.height_stage.is_none()
            && let (Some(a), Some(b)) = (proj(bz.world(last.position)), proj(c))
        {
            overlay
                .draw_dashed_line_ex(a, b, Color::from_rgba(1.0, 0.8505, 0.4005, 0.7005))
                .width(1.0)
                .dash(4.0)
                .done();
        }
        if let Some(s) = &bz.session
            && let Some(c) = s.constraint
        {
            let b = s.frame(c.local);
            let cols = [b.col_a(), b.col_b(), b.col_c()];
            let p = bz.part_world(s.part).unwrap_or(s.world_origin);
            let axes: Vec<usize> = if c.sheet {
                other_axes(c.axle).to_vec()
            } else {
                vec![c.axle]
            };
            for a in axes {
                let dir = cols[a].normalized();
                if let (Some(s0), Some(s1)) = (proj(p - dir * 1000.0), proj(p + dir * 1000.0)) {
                    overlay
                        .draw_line_ex(s0, s1, AXIS_COLORS[a])
                        .width(1.0)
                        .done();
                }
            }
        }
        for (i, k) in r.knots.iter().enumerate() {
            let Some(s) = proj(bz.world(k.position)) else {
                continue;
            };
            let sel = bz.selected == Some(i);
            if sel {
                for (p, part) in [
                    (k.position + k.tangent_in, Part::TanIn(i)),
                    (k.position + k.tangent_out, Part::TanOut(i)),
                ] {
                    if let Some(h) = proj(bz.world(p)) {
                        overlay.draw_line_ex(s, h, TANGENT).width(1.0).done();
                        let rr = if bz.hover == Some(part) { 5.0 } else { 4.0 };
                        overlay.draw_circle(h, rr, TANGENT);
                    }
                }
            }
            let rr = if bz.hover == Some(Part::Knot(i)) || sel {
                5.0
            } else {
                4.0
            };
            overlay.draw_rect(
                Rect2::new(
                    s - Vector2::new(rr + 1.0, rr + 1.0),
                    Vector2::new(rr * 2.0 + 2.0, rr * 2.0 + 2.0),
                ),
                Color::from_rgba(0.1005, 0.1005, 0.1005, 0.9005),
            );
            overlay.draw_rect(
                Rect2::new(s - Vector2::new(rr, rr), Vector2::new(rr * 2.0, rr * 2.0)),
                if sel { SELECTED } else { KNOT },
            );
        }
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn debug_bezier(
        &mut self,
        points: PackedVector3Array,
        closed: bool,
        block: bool,
        height: f32,
    ) -> GString {
        self.assign_instrument(Tool::Bezier);
        let pts = points.as_slice();
        let Some(first) = pts.first() else {
            return "no points".into();
        };
        self.bz.frame = Transform3D::new(Basis::IDENTITY, *first);
        for p in pts {
            let l = self.bz.local(*p);
            self.bz.append_tail(l);
        }
        self.bz.opts.is_shut = closed;
        self.bz.opts.block = block;
        if block {
            self.bz.opts.stature = height.max(0.0105);
        }
        self.message.clear();
        self.curve_seal();
        GString::from(self.message.as_str())
    }
    #[func]
    fn inspect_curve_relocate_bend(&mut self, index: i64, delta: Vector3) -> bool {
        if self.bz.editing.is_none() && !self.bezier_reedit() {
            return false;
        }
        let Some(k) = self.bz.formula.knots.get_mut(index.max(0) as usize) else {
            return false;
        };
        k.position = k.position + to_p(delta);
        self.curve_reassemble("Pixel Builder: Move Bezier knot");
        true
    }
    #[func]
    fn debug_bezier_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("knots", self.bz.formula.knots.len() as i64);
        d.set("editing", self.bz.editing.is_some());
        d.set("closed", self.bz.formula.is_shut);
        d.set("selected", self.bz.selected.map(|s| s as i64).unwrap_or(-1));
        d.set("moving", self.bz.session.is_some());
        d.set("height_stage", self.bz.height_stage.is_some());
        let pts: PackedVector3Array = self
            .bz
            .formula
            .knots
            .iter()
            .map(|k| self.bz.world(k.position))
            .collect();
        d.set("points", &pts);
        d
    }
}
