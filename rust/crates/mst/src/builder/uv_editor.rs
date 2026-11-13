//! UV editor window of the Pixel Builder: port of Unibuilder's `UBUVEditor`
//! (`editor/uv_editor.gd` + `uv_editor.tscn`, MIT) onto `pt_core::build::uv_edit`.
//!
//! A canvas shows the UVs of the edited builder mesh (the 3D face selection in Face mode, else
//! every face) over the first slot texture: faces with manual UVs are drawn brighter. Face /
//! vertex selection by click, Shift-click, double-click (UV island) or box; drag moves,
//! Ctrl+drag scales (Shift: per axis), Alt+drag rotates, middle-drag pans, wheel zooms, Esc
//! cancels. The bar bakes / clears manual UVs, projects (planar / box), flips, rotates, fits,
//! sews, copies / pastes layouts and matches texel density. Every edit is one undo step
//! (`PixelBuilderMeshData.set_blob`).

use std::collections::{BTreeMap, BTreeSet};

use godot::classes::control::{FocusMode, LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    BaseMaterial3D, Button, ButtonGroup, Control, EditorInterface, Geometry2D, HFlowContainer, IControl, InputEvent, InputEventKey,
    InputEventMouseButton, InputEventMouseMotion, Label, PanelContainer, ShaderMaterial, SpinBox, Texture2D, VBoxContainer, VSeparator, Window,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::BMesh;
use pt_core::build::uv_edit::{self as uve, Affine2, AutoUv, Corner, FitAxis};
use pt_core::math::{Vec2 as PVec2, v2};

use super::data::{PixelBuilderMeshData, to_p};
use super::node::PixelBuilderMesh;

const TEAL: Color = Color::from_rgb(0.369, 0.918, 0.831);
const AMBER: Color = Color::from_rgb(0.984, 0.749, 0.141);
const BG: Color = Color::from_rgb(0.11, 0.12, 0.14);
const PICK_PX: f32 = 8.0;
const ZOOM_MIN: f32 = 8.0;
const ZOOM_MAX: f32 = 20000.0;
const FRAME_MARGIN: f32 = 0.12;
const MIN_FACTOR: f32 = 0.001;
const ICONS: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DragKind {
    Pan,
    Move,
    Scale,
    Rotate,
    Marquee,
}

struct Drag {
    kind: DragKind,
    start: Vector2,
    orig: BTreeMap<Corner, PVec2>,
    before: PackedByteArray,
    data: Gd<PixelBuilderMeshData>,
    moved: bool,
    bounds: uve::Rect,
    pivot: PVec2,
    factors: PVec2,
    angle: f32,
    cursor: Vector2,
}

/// Bar actions (`act`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i64)]
enum Act {
    Manual = 0,
    Auto,
    Planar,
    Box,
    FlipU,
    FlipV,
    Rot90,
    Fit,
    FitU,
    FitV,
    Sew,
    Copy,
    Paste,
    Texel,
    Scale,
    Frame,
    FrameSel,
}

const ACTS: [Act; 17] = [
    Act::Manual,
    Act::Auto,
    Act::Planar,
    Act::Box,
    Act::FlipU,
    Act::FlipV,
    Act::Rot90,
    Act::Fit,
    Act::FitU,
    Act::FitV,
    Act::Sew,
    Act::Copy,
    Act::Paste,
    Act::Texel,
    Act::Scale,
    Act::Frame,
    Act::FrameSel,
];

/// Toggles (`set_flag`).
const FLAG_VERTS: i64 = 0;
const FLAG_LOCK: i64 = 1;
const FLAG_SPLIT: i64 = 2;
const FLAG_SNAP: i64 = 3;
const FLAG_TILE: i64 = 4;

#[derive(GodotClass)]
#[class(tool, base = Control, internal)]
pub struct PixelBuilderUvCanvas {
    target: Option<Gd<PixelBuilderMesh>>,
    /// Faces of the 3D face selection (empty = every face).
    scope_sel: BTreeSet<usize>,
    sel_faces: BTreeSet<usize>,
    sel_corners: BTreeSet<Corner>,
    vert_mode: bool,
    lock: bool,
    split: bool,
    snap: bool,
    tile: bool,
    step: f32,
    scale_uv: [f32; 2],
    zoom: f32,
    pan: Vector2,
    framed: bool,
    /// Frame request made before the canvas had a size (applied on the first resize).
    pending_frame: Option<uve::Rect>,
    uvs: BTreeMap<usize, Vec<PVec2>>,
    drag: Option<Drag>,
    marquee: Rect2,
    copied: Vec<PVec2>,
    info: Option<Gd<Label>>,
    connected: Option<Gd<PixelBuilderMeshData>>,
    base: Base<Control>,
}

#[godot_api]
impl IControl for PixelBuilderUvCanvas {
    fn init(base: Base<Control>) -> Self {
        PixelBuilderUvCanvas {
            target: None,
            scope_sel: BTreeSet::new(),
            sel_faces: BTreeSet::new(),
            sel_corners: BTreeSet::new(),
            vert_mode: false,
            lock: false,
            split: false,
            snap: true,
            tile: false,
            step: 0.0625,
            scale_uv: [1.0, 1.0],
            zoom: 220.0,
            pan: Vector2::new(60.0, 40.0),
            framed: false,
            pending_frame: None,
            uvs: BTreeMap::new(),
            drag: None,
            marquee: Rect2::default(),
            copied: Vec::new(),
            info: None,
            connected: None,
            base,
        }
    }

    fn draw(&mut self) {
        self.draw_canvas();
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        self.on_input(event);
    }

    fn on_notification(&mut self, what: godot::classes::notify::ControlNotification) {
        if what == godot::classes::notify::ControlNotification::RESIZED {
            if let Some(r) = self.pending_frame {
                self.frame_rect(r);
            } else if !self.framed {
                self.frame_unit();
            }
        }
    }
}

fn gv(p: PVec2) -> Vector2 {
    Vector2::new(p.x, p.y)
}
fn pv(p: Vector2) -> PVec2 {
    v2(p.x, p.y)
}

impl PixelBuilderUvCanvas {
    fn data(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.target.as_ref().filter(|t| t.is_instance_valid()).and_then(|t| t.bind().data())
    }

    fn auto(&self) -> AutoUv {
        let units = self.data().map(|d| d.bind().units()).unwrap_or(1.0);
        let origin = self.target.as_ref().filter(|t| t.is_instance_valid() && t.is_inside_tree()).map(|t| t.get_global_position()).unwrap_or_default();
        AutoUv { units_per_repeat: units, world_offset: to_p(origin) }
    }

    fn mesh(&self) -> Option<BMesh> {
        self.data().map(|d| d.bind().mesh().clone())
    }

    fn scope(&self, m: &BMesh) -> BTreeSet<usize> {
        let s: BTreeSet<usize> = self.scope_sel.iter().copied().filter(|f| *f < m.faces.len()).collect();
        if s.is_empty() { (0..m.faces.len()).collect() } else { s }
    }

    fn target_faces(&self, m: &BMesh) -> BTreeSet<usize> {
        if !self.sel_faces.is_empty() {
            return self.sel_faces.clone();
        }
        if !self.sel_corners.is_empty() {
            return uve::faces_of(&self.sel_corners);
        }
        self.scope(m)
    }

    fn target_corners(&self, m: &BMesh) -> BTreeSet<Corner> {
        if !self.sel_corners.is_empty() {
            return self.sel_corners.clone();
        }
        uve::corners_of(m, &self.target_faces(m))
    }

    fn uv_to_px(&self, uv: PVec2) -> Vector2 {
        gv(uv) * self.zoom + self.pan
    }

    fn px_to_uv(&self, px: Vector2) -> PVec2 {
        pv((px - self.pan) / self.zoom)
    }

    fn update_info(&mut self) {
        let text = match self.mesh() {
            None => "Select a Pixel Builder mesh".to_string(),
            Some(m) => {
                let manual = self.uvs.keys().filter(|f| m.faces[**f].manual_uvs().is_some()).count();
                let (n, what) = if !self.sel_faces.is_empty() { (self.sel_faces.len(), "faces") } else { (self.sel_corners.len(), "corners") };
                format!("{} faces ({manual} manual), {n} {what} selected", self.uvs.len())
            }
        };
        if let Some(l) = &mut self.info
            && l.is_instance_valid()
        {
            l.set_text(&text);
        }
    }

    /// Rebuilds the display cache and prunes the UV selection against the mesh.
    fn refresh_cache(&mut self) {
        self.uvs.clear();
        match self.mesh() {
            Some(m) => {
                let a = self.auto();
                for f in self.scope(&m) {
                    self.uvs.insert(f, uve::face_uvs(&m, f, &a));
                }
                let uvs = &self.uvs;
                self.sel_faces.retain(|f| uvs.contains_key(f));
                self.sel_corners.retain(|(f, c)| uvs.get(f).is_some_and(|u| *c < u.len()));
            }
            None => {
                self.sel_faces.clear();
                self.sel_corners.clear();
            }
        }
        self.update_info();
        self.base_mut().queue_redraw();
    }

    fn frame_unit(&mut self) {
        self.pending_frame = None;
        let size = self.base().get_size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        self.zoom = size.x.min(size.y) * 0.8;
        self.pan = (size - Vector2::new(self.zoom, self.zoom)) * 0.5;
        self.framed = true;
        self.base_mut().queue_redraw();
    }

    fn frame_rect(&mut self, r: uve::Rect) {
        // Kept until the view is panned / zoomed by hand, so a resize (the window's first
        // layout included) frames the same rectangle again.
        self.pending_frame = Some(r);
        let size = self.base().get_size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let avail = size * (1.0 - 2.0 * FRAME_MARGIN);
        let s = r.size();
        let mut fit = f32::INFINITY;
        if s.x > 1e-6 {
            fit = fit.min(avail.x / s.x);
        }
        if s.y > 1e-6 {
            fit = fit.min(avail.y / s.y);
        }
        if fit.is_finite() {
            self.zoom = fit.clamp(ZOOM_MIN, ZOOM_MAX);
        }
        self.pan = size * 0.5 - gv(r.center()) * self.zoom;
        self.framed = true;
        self.base_mut().queue_redraw();
    }

    fn selection_bounds(&self) -> Option<uve::Rect> {
        if !self.sel_corners.is_empty() {
            return uve::Rect::of(self.sel_corners.iter().filter_map(|(f, c)| self.uvs.get(f).and_then(|u| u.get(*c)).copied()));
        }
        let faces: Vec<usize> = if self.sel_faces.is_empty() { self.uvs.keys().copied().collect() } else { self.sel_faces.iter().copied().collect() };
        uve::Rect::of(faces.iter().filter_map(|f| self.uvs.get(f)).flatten().copied())
    }

    // ------------------------------------------------------------------ drawing

    fn slot_texture(&self) -> Option<Gd<Texture2D>> {
        let d = self.data()?;
        let db = d.bind();
        let slots = db.material_slots();
        for f in self.uvs.keys() {
            let slot = db.mesh().faces[*f].material as usize;
            let Some(mat) = slots.get(slot).flatten() else { continue };
            if let Ok(b) = mat.clone().try_cast::<BaseMaterial3D>()
                && let Some(t) = b.get_texture(godot::classes::base_material_3d::TextureParam::ALBEDO)
            {
                return Some(t);
            }
            if let Ok(s) = mat.try_cast::<ShaderMaterial>() {
                for p in ["albedo_texture", "texture_albedo", "albedo_tex", "main_texture"] {
                    if let Ok(t) = s.get_shader_parameter(p).try_to::<Gd<Texture2D>>() {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    fn draw_canvas(&mut self) {
        let size = self.base().get_size();
        self.base_mut().draw_rect(Rect2::new(Vector2::ZERO, size), BG);
        let Some(m) = self.mesh() else {
            if let Some(font) = self.base().get_theme_default_font() {
                self.base_mut()
                    .draw_string_ex(&font, size * 0.5 + Vector2::new(-90.0, 0.0), "Select a Pixel Builder mesh")
                    .modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.35))
                    .done();
            }
            return;
        };
        // Texture underlay (first slot texture of the shown faces), tiled on request.
        if let Some(tex) = self.slot_texture() {
            if self.tile {
                let lo = self.px_to_uv(Vector2::ZERO);
                let hi = self.px_to_uv(size);
                let (lo, hi) = (Vector2::new(lo.x.floor(), lo.y.floor()), Vector2::new(hi.x.ceil(), hi.y.ceil()));
                let cells = hi - lo;
                let ts = tex.get_size();
                let dst = Rect2::new(self.uv_to_px(pv(lo)), cells * self.zoom);
                let src = Rect2::new(lo * ts, cells * ts);
                self.base_mut().draw_texture_rect_region(&tex, dst, src);
            } else {
                let r = Rect2::new(self.uv_to_px(PVec2::ZERO), Vector2::new(self.zoom, self.zoom));
                self.base_mut().draw_texture_rect_ex(&tex, r, false).modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.4)).done();
            }
        }
        // Grid at the snap step, and the 0-1 square.
        let step = self.step.max(0.001);
        let (uv_min, uv_max) = (self.px_to_uv(Vector2::ZERO), self.px_to_uv(size));
        if (uv_max.x - uv_min.x) / step < 200.0 {
            let minor = Color::from_rgba(1.0, 1.0, 1.0, 0.05);
            let mut x = (uv_min.x / step).floor() * step;
            while x <= uv_max.x {
                let (a, b) = (self.uv_to_px(v2(x, uv_min.y)), self.uv_to_px(v2(x, uv_max.y)));
                self.base_mut().draw_line(a, b, minor);
                x += step;
            }
            let mut y = (uv_min.y / step).floor() * step;
            while y <= uv_max.y {
                let (a, b) = (self.uv_to_px(v2(uv_min.x, y)), self.uv_to_px(v2(uv_max.x, y)));
                self.base_mut().draw_line(a, b, minor);
                y += step;
            }
        }
        let unit = Rect2::new(self.uv_to_px(PVec2::ZERO), Vector2::new(self.zoom, self.zoom));
        let (uc, uw) = if self.lock { (Color::from_rgba(AMBER.r, AMBER.g, AMBER.b, 0.8), 2.0) } else { (Color::from_rgba(TEAL.r, TEAL.g, TEAL.b, 0.45), 1.0) };
        self.base_mut().draw_rect_ex(unit, uc).filled(false).width(uw).done();

        let entries: Vec<(usize, Vec<Vector2>)> = self.uvs.iter().map(|(f, u)| (*f, u.iter().map(|p| self.uv_to_px(*p)).collect())).collect();
        for (f, pts) in entries {
            let selected = self.sel_faces.contains(&f);
            let packed: PackedVector2Array = pts.iter().copied().collect();
            if selected && !Geometry2D::singleton().triangulate_polygon(&packed).is_empty() {
                self.base_mut().draw_colored_polygon(&packed, Color::from_rgba(AMBER.r, AMBER.g, AMBER.b, 0.18));
            }
            let manual = m.faces[f].manual_uvs().is_some();
            let wire = if selected {
                AMBER
            } else if manual {
                Color::from_rgba(TEAL.r, TEAL.g, TEAL.b, 0.8)
            } else {
                Color::from_rgba(TEAL.r, TEAL.g, TEAL.b, 0.3)
            };
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                self.base_mut().draw_line_ex(a, b, wire).width(1.5).antialiased(true).done();
            }
            if self.vert_mode {
                for (ci, p) in pts.iter().enumerate() {
                    let col = if self.sel_corners.contains(&(f, ci)) { AMBER } else { TEAL };
                    self.base_mut().draw_rect(Rect2::new(*p - Vector2::new(2.5, 2.5), Vector2::new(5.0, 5.0)), col);
                }
            }
        }
        let Some(d) = &self.drag else { return };
        match d.kind {
            DragKind::Marquee => {
                let r = self.marquee.abs();
                self.base_mut().draw_rect(r, Color::from_rgba(TEAL.r, TEAL.g, TEAL.b, 0.1));
                self.base_mut().draw_rect_ex(r, Color::from_rgba(TEAL.r, TEAL.g, TEAL.b, 0.7)).filled(false).width(1.0).done();
            }
            DragKind::Scale | DragKind::Rotate => {
                let pp = self.uv_to_px(d.pivot);
                let text = if d.kind == DragKind::Rotate {
                    format!("{:.1}°", d.angle.to_degrees())
                } else if (d.factors.x - d.factors.y).abs() < 1e-6 {
                    format!("× {:.3}", d.factors.x)
                } else {
                    format!("× {:.3}, {:.3}", d.factors.x, d.factors.y)
                };
                let cursor = d.cursor;
                self.base_mut().draw_line_ex(pp - Vector2::new(6.0, 0.0), pp + Vector2::new(6.0, 0.0), AMBER).width(1.5).done();
                self.base_mut().draw_line_ex(pp - Vector2::new(0.0, 6.0), pp + Vector2::new(0.0, 6.0), AMBER).width(1.5).done();
                if let Some(font) = self.base().get_theme_default_font() {
                    let pos = cursor + Vector2::new(16.0, -12.0);
                    self.base_mut().draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), &text).modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8)).done();
                    self.base_mut().draw_string_ex(&font, pos, &text).modulate(Color::WHITE).done();
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ input

    fn on_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            let pos = mb.get_position();
            match mb.get_button_index() {
                MouseButton::WHEEL_UP if mb.is_pressed() => self.zoom_at(pos, 1.15),
                MouseButton::WHEEL_DOWN if mb.is_pressed() => self.zoom_at(pos, 1.0 / 1.15),
                MouseButton::MIDDLE => {
                    if mb.is_pressed() && self.drag.is_none() {
                        if let Some(data) = self.data() {
                            self.drag = Some(self.new_drag(DragKind::Pan, pos, data));
                        }
                    } else if !mb.is_pressed() && self.drag.as_ref().is_some_and(|d| d.kind == DragKind::Pan) {
                        self.drag = None;
                    }
                }
                MouseButton::LEFT => {
                    if mb.is_pressed() {
                        self.left_press(&mb);
                    } else {
                        self.left_release();
                    }
                }
                _ => {}
            }
            self.base_mut().accept_event();
        } else if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let kind = self.drag.as_ref().map(|d| d.kind);
            match kind {
                Some(DragKind::Pan) => {
                    self.pending_frame = None;
                    self.pan += mm.get_relative();
                    self.base_mut().queue_redraw();
                }
                Some(DragKind::Move) => self.update_move(pos),
                Some(DragKind::Scale) => self.update_scale(pos, mm.is_shift_pressed()),
                Some(DragKind::Rotate) => self.update_rotate(pos),
                Some(DragKind::Marquee) => {
                    self.marquee.size = pos - self.marquee.position;
                    self.base_mut().queue_redraw();
                }
                None => {}
            }
            self.base_mut().accept_event();
        } else if let Ok(k) = event.try_cast::<InputEventKey>()
            && k.is_pressed()
            && k.get_keycode() == Key::ESCAPE
        {
            if !self.cancel_drag() {
                self.sel_faces.clear();
                self.sel_corners.clear();
                self.update_info();
                self.base_mut().queue_redraw();
            }
            self.base_mut().accept_event();
        }
    }

    fn new_drag(&self, kind: DragKind, start: Vector2, data: Gd<PixelBuilderMeshData>) -> Drag {
        Drag {
            kind,
            start,
            orig: BTreeMap::new(),
            before: PackedByteArray::new(),
            data,
            moved: false,
            bounds: uve::Rect { min: PVec2::ZERO, max: PVec2::ZERO },
            pivot: PVec2::ZERO,
            factors: v2(1.0, 1.0),
            angle: 0.0,
            cursor: start,
        }
    }

    fn zoom_at(&mut self, px: Vector2, factor: f32) {
        self.pending_frame = None;
        let old = self.zoom;
        self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.pan = px - (px - self.pan) * (self.zoom / old);
        self.base_mut().queue_redraw();
    }

    fn sel_or_face_corners(&self, m: &BMesh) -> BTreeSet<Corner> {
        if self.vert_mode { self.sel_corners.clone() } else { uve::corners_of(m, &self.sel_faces) }
    }

    fn left_press(&mut self, mb: &Gd<InputEventMouseButton>) {
        self.base_mut().grab_focus();
        let Some(m) = self.mesh() else { return };
        let pos = mb.get_position();
        let shift = mb.is_shift_pressed();
        if mb.is_double_click() {
            self.select_island(pos, shift, &m);
            return;
        }
        if mb.is_alt_pressed() || mb.is_ctrl_pressed() {
            let corners = self.sel_or_face_corners(&m);
            if !corners.is_empty() {
                let kind = if mb.is_alt_pressed() { DragKind::Rotate } else { DragKind::Scale };
                self.begin_transform(pos, corners, kind);
                return;
            }
        }
        if self.vert_mode {
            let hits = self.pick_corners(pos);
            if hits.is_empty() {
                self.begin_marquee(pos, shift);
                return;
            }
            let any_selected = hits.iter().any(|k| self.sel_corners.contains(k));
            if !any_selected {
                if !shift {
                    self.sel_corners.clear();
                }
                self.sel_corners.extend(hits);
            } else if shift {
                for k in hits {
                    self.sel_corners.remove(&k);
                }
                self.update_info();
                self.base_mut().queue_redraw();
                return;
            }
            let c = self.sel_corners.clone();
            self.begin_transform(pos, c, DragKind::Move);
        } else {
            let Some(fi) = self.pick_face(pos) else {
                self.begin_marquee(pos, shift);
                return;
            };
            if !self.sel_faces.contains(&fi) {
                if !shift {
                    self.sel_faces.clear();
                }
                self.sel_faces.insert(fi);
            } else if shift {
                self.sel_faces.remove(&fi);
                self.update_info();
                self.base_mut().queue_redraw();
                return;
            }
            let c = uve::corners_of(&m, &self.sel_faces);
            self.begin_transform(pos, c, DragKind::Move);
        }
        self.update_info();
        self.base_mut().queue_redraw();
    }

    fn left_release(&mut self) {
        let Some(d) = self.drag.take() else { return };
        match d.kind {
            DragKind::Move | DragKind::Scale | DragKind::Rotate => {
                let name = match d.kind {
                    DragKind::Scale => "Scale UVs",
                    DragKind::Rotate => "Rotate UVs",
                    _ => "Move UVs",
                };
                let mut data = d.data.clone();
                if d.moved && self.data().as_ref() == Some(&data) {
                    let after = data.bind().get_blob();
                    commit(&format!("Pixel Builder: {name}"), &data, d.before, after);
                } else {
                    // Nothing moved: drop the baked manual UVs again.
                    data.bind_mut().set_blob(d.before);
                }
            }
            DragKind::Marquee => self.apply_marquee(),
            DragKind::Pan => {}
        }
        self.base_mut().queue_redraw();
    }

    fn pick_corners(&self, px: Vector2) -> Vec<Corner> {
        let mut hits = Vec::new();
        let mut best = None;
        let mut best_d = PICK_PX;
        for (f, uvs) in &self.uvs {
            for (ci, uv) in uvs.iter().enumerate() {
                let d = self.uv_to_px(*uv).distance_to(px);
                if d <= PICK_PX {
                    hits.push((*f, ci));
                    if d < best_d {
                        best_d = d;
                        best = Some((*f, ci));
                    }
                }
            }
        }
        if self.split {
            return best.into_iter().collect();
        }
        hits
    }

    fn pick_face(&self, px: Vector2) -> Option<usize> {
        let uv = self.px_to_uv(px);
        self.uvs.iter().find(|(_, poly)| uve::point_in_polygon(uv, poly)).map(|(f, _)| *f)
    }

    fn select_island(&mut self, px: Vector2, additive: bool, m: &BMesh) {
        let Some(fi) = self.pick_face(px) else { return };
        let island = uve::island_faces(m, fi, 1e-5);
        if self.vert_mode {
            if !additive {
                self.sel_corners.clear();
            }
            self.sel_corners.extend(uve::corners_of(m, &island));
        } else {
            if !additive {
                self.sel_faces.clear();
            }
            self.sel_faces.extend(island);
        }
        self.update_info();
        self.base_mut().queue_redraw();
    }

    fn begin_marquee(&mut self, pos: Vector2, additive: bool) {
        let Some(data) = self.data() else { return };
        self.drag = Some(self.new_drag(DragKind::Marquee, pos, data));
        self.marquee = Rect2::new(pos, Vector2::ZERO);
        if !additive {
            self.sel_faces.clear();
            self.sel_corners.clear();
        }
        self.update_info();
        self.base_mut().queue_redraw();
    }

    fn apply_marquee(&mut self) {
        let r = self.marquee.abs();
        let hits: Vec<(usize, usize)> =
            self.uvs.iter().flat_map(|(f, uvs)| uvs.iter().enumerate().filter(|(_, uv)| r.contains_point(self.uv_to_px(**uv))).map(|(ci, _)| (*f, ci)).collect::<Vec<_>>()).collect();
        if self.vert_mode {
            self.sel_corners.extend(hits);
        } else {
            self.sel_faces.extend(hits.into_iter().map(|h| h.0));
        }
        self.update_info();
    }

    /// Bakes the corners' faces to manual UVs and starts a move / scale / rotate drag.
    fn begin_transform(&mut self, px: Vector2, corners: BTreeSet<Corner>, kind: DragKind) {
        let Some(mut data) = self.data() else { return };
        if corners.is_empty() {
            return;
        }
        let a = self.auto();
        let before = data.bind().get_blob();
        let mut m = data.bind().mesh().clone();
        uve::bake_manual(&mut m, &uve::faces_of(&corners), &a);
        let orig: BTreeMap<Corner, PVec2> = corners.iter().filter_map(|(f, c)| m.faces.get(*f).and_then(|x| x.uv_coords.get(*c)).map(|uv| ((*f, *c), *uv))).collect();
        let bounds = uve::Rect::of(orig.values().copied()).unwrap_or(uve::Rect { min: PVec2::ZERO, max: PVec2::ZERO });
        data.bind_mut().replace(m);
        data.emit_changed();
        let mut d = self.new_drag(kind, px, data);
        d.before = before;
        d.orig = orig;
        d.bounds = bounds;
        d.pivot = bounds.center();
        self.drag = Some(d);
    }

    fn write_uvs(&mut self, moved: BTreeMap<Corner, PVec2>) {
        let Some(d) = &self.drag else { return };
        let mut data = d.data.clone();
        let mut m = data.bind().mesh().clone();
        uve::set_uvs(&mut m, &moved);
        data.bind_mut().replace(m);
        data.emit_changed();
    }

    fn lock_moved(&self, moved: &mut BTreeMap<Corner, PVec2>) {
        if !self.lock || moved.is_empty() {
            return;
        }
        let Some(r) = uve::Rect::of(moved.values().copied()) else { return };
        let shift = uve::unit_lock_offset(r);
        if shift != PVec2::ZERO {
            for v in moved.values_mut() {
                *v = *v + shift;
            }
        }
    }

    fn update_move(&mut self, px: Vector2) {
        let (step, snap, lock, zoom) = (self.step.max(0.001), self.snap, self.lock, self.zoom);
        let Some(d) = &mut self.drag else { return };
        let mut delta = pv((px - d.start) / zoom);
        if snap {
            delta = v2((delta.x / step).round() * step, (delta.y / step).round() * step);
        }
        if lock {
            delta = delta + uve::unit_lock_offset(uve::Rect { min: d.bounds.min + delta, max: d.bounds.max + delta });
        }
        if delta != PVec2::ZERO {
            d.moved = true;
        }
        let moved: BTreeMap<Corner, PVec2> = d.orig.iter().map(|(k, v)| (*k, *v + delta)).collect();
        self.write_uvs(moved);
    }

    fn update_scale(&mut self, px: Vector2, per_axis: bool) {
        let (step, snap, lock) = (self.step.max(0.001), self.snap, self.lock);
        let Some(d) = &self.drag else { return };
        let pivot_px = self.uv_to_px(d.pivot);
        let safe = |v: f32| if v.abs() < 1e-6 { 4.0 } else { v.signum() * v.abs().max(4.0) };
        let mut f = if per_axis {
            v2((px.x - pivot_px.x) / safe(d.start.x - pivot_px.x), (px.y - pivot_px.y) / safe(d.start.y - pivot_px.y))
        } else {
            let k = pivot_px.distance_to(px) / pivot_px.distance_to(d.start).max(4.0);
            v2(k, k)
        };
        if snap {
            f = v2((f.x / step).round() * step, (f.y / step).round() * step);
        }
        let floor = |x: f32| if x.abs() >= MIN_FACTOR { x } else if x >= 0.0 { MIN_FACTOR } else { -MIN_FACTOR };
        f = v2(floor(f.x), floor(f.y));
        if lock {
            f = uve::unit_lock_factors(d.bounds.size(), f, !per_axis);
        }
        let pivot = d.pivot;
        let mut moved: BTreeMap<Corner, PVec2> = d.orig.iter().map(|(k, v)| (*k, pivot + v2((v.x - pivot.x) * f.x, (v.y - pivot.y) * f.y))).collect();
        self.lock_moved(&mut moved);
        if let Some(d) = &mut self.drag {
            if (f.x - 1.0).abs() > 1e-6 || (f.y - 1.0).abs() > 1e-6 {
                d.moved = true;
            }
            d.factors = f;
            d.cursor = px;
        }
        self.write_uvs(moved);
        self.base_mut().queue_redraw();
    }

    fn update_rotate(&mut self, px: Vector2) {
        let snap = self.snap;
        let Some(d) = &self.drag else { return };
        let pivot_px = self.uv_to_px(d.pivot);
        let mut ang = (px - pivot_px).angle() - (d.start - pivot_px).angle();
        if snap {
            let s = 15f32.to_radians();
            ang = (ang / s).round() * s;
        }
        let xf = Affine2::about(d.pivot, ang, v2(1.0, 1.0));
        let mut moved: BTreeMap<Corner, PVec2> = d.orig.iter().map(|(k, v)| (*k, xf.apply(*v))).collect();
        self.lock_moved(&mut moved);
        if let Some(d) = &mut self.drag {
            if ang.abs() > 1e-6 {
                d.moved = true;
            }
            d.angle = ang;
            d.cursor = px;
        }
        self.write_uvs(moved);
        self.base_mut().queue_redraw();
    }

    /// Ends a live gesture like Esc (restoring the UVs). False when nothing was live.
    fn cancel_drag(&mut self) -> bool {
        let Some(d) = self.drag.take() else { return false };
        if matches!(d.kind, DragKind::Move | DragKind::Scale | DragKind::Rotate) {
            let mut data = d.data;
            data.bind_mut().set_blob(d.before);
        }
        self.base_mut().queue_redraw();
        true
    }

    /// Applies `f` to a copy of the mesh and records one undo step.
    fn edit(&mut self, name: &str, f: impl FnOnce(&mut BMesh, &Self)) {
        let Some(mut data) = self.data() else { return };
        let (before, mut m) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone())
        };
        f(&mut m, self);
        data.bind_mut().replace(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            commit(&format!("Pixel Builder: {name}"), &data, before, after);
        }
    }

    fn about_center(&mut self, scale: PVec2, rot: f32, name: &str) {
        let a = self.auto();
        let lock = self.lock;
        self.edit(name, |m, s| {
            let c = s.target_corners(m);
            uve::transform_about_center(m, &c, scale, rot, lock, &a);
        });
    }

    fn run(&mut self, act: Act) {
        let a = self.auto();
        match act {
            Act::Manual => self.edit("UVs to manual", |m, s| {
                let f = s.target_faces(m);
                uve::bake_manual(m, &f, &a);
            }),
            Act::Auto => self.edit("UVs to auto", |m, s| {
                let f = s.target_faces(m);
                uve::clear_manual(m, &f);
            }),
            Act::Planar => self.edit("Planar project UVs", |m, s| {
                let f = s.target_faces(m);
                uve::planar_project(m, &f, true);
            }),
            Act::Box => self.edit("Box project UVs", |m, s| {
                let f = s.target_faces(m);
                uve::box_project(m, &f, true);
            }),
            Act::FlipU => self.about_center(v2(-1.0, 1.0), 0.0, "Flip UVs U"),
            Act::FlipV => self.about_center(v2(1.0, -1.0), 0.0, "Flip UVs V"),
            Act::Rot90 => self.about_center(v2(1.0, 1.0), std::f32::consts::FRAC_PI_2, "Rotate UVs 90"),
            Act::Fit | Act::FitU | Act::FitV => {
                let (axis, name) = match act {
                    Act::FitU => (FitAxis::U, "Fit UVs U"),
                    Act::FitV => (FitAxis::V, "Fit UVs V"),
                    _ => (FitAxis::Both, "Fit UVs"),
                };
                self.edit(name, |m, s| {
                    let f = s.target_faces(m);
                    uve::bake_manual(m, &f, &a);
                    uve::fit_to_unit(m, &f, axis);
                });
            }
            Act::Sew => self.edit("Sew UVs", |m, s| {
                let c = s.target_corners(m);
                uve::weld_uvs(m, &c);
            }),
            Act::Copy => {
                if let Some(m) = self.mesh()
                    && let Some(f) = self.target_faces(&m).first()
                    && let Some(u) = self.uvs.get(f)
                {
                    self.copied = u.clone();
                }
            }
            Act::Paste => {
                let coords = self.copied.clone();
                if !coords.is_empty() {
                    self.edit("Paste UVs", |m, s| {
                        let f = s.target_faces(m);
                        uve::copy_uvs(m, &coords, &f);
                    });
                }
            }
            Act::Texel => self.edit("Normalize texel density", |m, s| {
                let f = s.target_faces(m);
                if let Some(r) = f.first().copied()
                    && f.len() > 1
                {
                    uve::bake_manual(m, &f, &a);
                    uve::texel_normalize(m, &f, r);
                }
            }),
            Act::Scale => {
                let s = v2(self.scale_uv[0], self.scale_uv[1]);
                if (s.x - 1.0).abs() > 1e-6 || (s.y - 1.0).abs() > 1e-6 {
                    self.about_center(s, 0.0, "Scale UVs");
                }
            }
            Act::Frame => self.frame_unit(),
            Act::FrameSel => {
                if let Some(r) = self.selection_bounds() {
                    self.frame_rect(r);
                }
            }
        }
        self.refresh_cache();
    }
}

fn commit(name: &str, data: &Gd<PixelBuilderMeshData>, before: PackedByteArray, after: PackedByteArray) {
    let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else { return };
    ur.create_action(name);
    ur.add_do_method(data, "set_blob", &[after.to_variant()]);
    ur.add_undo_method(data, "set_blob", &[before.to_variant()]);
    ur.commit_action_ex().execute(false).done();
}

#[godot_api]
impl PixelBuilderUvCanvas {
    /// Shows `target`'s UVs; `faces` = the 3D face selection (empty = every face).
    #[func]
    pub fn set_target(&mut self, target: Option<Gd<PixelBuilderMesh>>, faces: PackedInt32Array) {
        let same = self.target.as_ref().map(|t| t.instance_id()) == target.as_ref().map(|t| t.instance_id());
        if !same {
            self.cancel_drag();
            self.sel_faces.clear();
            self.sel_corners.clear();
        }
        self.target = target;
        self.scope_sel = super::data::face_set(&faces);
        // Follow the data resource: undo / redo and 3D edits refresh the canvas.
        let data = self.data();
        if self.connected != data {
            let cb = self.to_gd().callable("refresh");
            if let Some(mut old) = self.connected.take()
                && old.is_instance_valid()
                && old.is_connected("changed", &cb)
            {
                old.disconnect("changed", &cb);
            }
            if let Some(mut d) = data.clone() {
                d.connect_flags("changed", &cb, godot::classes::object::ConnectFlags::DEFERRED);
            }
            self.connected = data;
        }
        if self.drag.is_none() {
            self.refresh_cache();
        }
    }

    #[func]
    pub fn refresh(&mut self) {
        if self.drag.as_ref().is_none_or(|d| d.kind == DragKind::Pan || d.kind == DragKind::Marquee) {
            self.refresh_cache();
        } else {
            self.base_mut().queue_redraw();
        }
    }

    #[func]
    fn act(&mut self, id: i64) {
        if let Some(a) = ACTS.iter().copied().find(|a| *a as i64 == id) {
            self.run(a);
        }
    }

    #[func]
    fn set_flag(&mut self, on: bool, id: i64) {
        match id {
            FLAG_VERTS => {
                self.vert_mode = on;
                self.sel_faces.clear();
                self.sel_corners.clear();
                self.update_info();
            }
            FLAG_LOCK => self.lock = on,
            FLAG_SPLIT => self.split = on,
            FLAG_SNAP => self.snap = on,
            FLAG_TILE => self.tile = on,
            _ => {}
        }
        self.base_mut().queue_redraw();
    }

    #[func]
    fn set_vert_mode(&mut self, on: bool) {
        self.set_flag(on, FLAG_VERTS);
    }

    #[func]
    fn set_face_mode(&mut self) {
        self.set_flag(false, FLAG_VERTS);
    }

    #[func]
    fn set_vertex_mode(&mut self) {
        self.set_flag(true, FLAG_VERTS);
    }

    #[func]
    pub fn set_step(&mut self, v: f64) {
        self.step = (v as f32).max(0.001);
        self.base_mut().queue_redraw();
    }

    #[func]
    fn set_scale_factor(&mut self, v: f64, axis: i64) {
        self.scale_uv[axis.clamp(0, 1) as usize] = v as f32;
    }

    /// Test hook: select UV faces (face mode).
    #[func]
    pub fn debug_select(&mut self, faces: PackedInt32Array) {
        self.sel_faces = super::data::face_set(&faces);
        self.sel_corners.clear();
        self.update_info();
        self.base_mut().queue_redraw();
    }

    /// Test hook: run a bar action by name ("planar", "box", "flip_u", "rot90", "fit", "auto", …).
    #[func]
    pub fn debug_act(&mut self, name: GString) {
        let a = match name.to_string().as_str() {
            "manual" => Act::Manual,
            "auto" => Act::Auto,
            "planar" => Act::Planar,
            "box" => Act::Box,
            "flip_u" => Act::FlipU,
            "flip_v" => Act::FlipV,
            "rot90" => Act::Rot90,
            "fit" => Act::Fit,
            "sew" => Act::Sew,
            "texel" => Act::Texel,
            "frame" => Act::Frame,
            _ => Act::FrameSel,
        };
        self.run(a);
    }
}

fn icon(name: &str) -> Option<Gd<Texture2D>> {
    super::ui::mono_icon(&format!("{ICONS}{name}.svg"), 1.0)
}

/// Builds the "UV Editor" window (bar + canvas). The window hides on close.
pub fn build_window() -> (Gd<Window>, Gd<PixelBuilderUvCanvas>) {
    let mut win = Window::new_alloc();
    win.set_name("PixelBuilderUvEditor");
    win.set_title("Pixel Builder - UV Editor");
    let s = crate::editor::ui_kit::editor_scale();
    win.set_size(Vector2i::new((820.0 * s) as i32, (620.0 * s) as i32));
    win.set_min_size(Vector2i::new((420.0 * s) as i32, (320.0 * s) as i32));
    win.set_transient(true);
    win.set_exclusive(false);
    win.set_wrap_controls(false);
    win.set_visible(false);
    let hide = Callable::from_object_method(&win, "hide");
    win.connect("close_requested", &hide);

    let mut panel = PanelContainer::new_alloc();
    panel.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
    let mut col = VBoxContainer::new_alloc();
    let mut bar = HFlowContainer::new_alloc();
    bar.set_name("Bar");
    let mut canvas = PixelBuilderUvCanvas::new_alloc();
    canvas.set_name("Canvas");
    canvas.set_v_size_flags(SizeFlags::EXPAND_FILL);
    canvas.set_h_size_flags(SizeFlags::EXPAND_FILL);
    canvas.set_clip_contents(true);
    canvas.set_focus_mode(FocusMode::CLICK);
    canvas.set_mouse_filter(MouseFilter::STOP);
    canvas.set_custom_minimum_size(Vector2::new(200.0, 160.0) * s);

    let button = |bar: &mut Gd<HFlowContainer>, text: &str, ic: &str, tip: &str, toggle: bool| {
        let mut b = Button::new_alloc();
        b.set_name(text);
        b.set_text(text);
        if let Some(t) = icon(ic) {
            b.set_button_icon(&t);
        }
        b.set_tooltip_text(tip);
        b.set_flat(true);
        b.set_toggle_mode(toggle);
        b.set_focus_mode(FocusMode::NONE);
        bar.add_child(&b);
        b
    };
    let modes = ButtonGroup::new_gd();
    let mut faces = button(&mut bar, "Faces", "mode_face", "UV face mode: click / box-select faces, double-click selects a UV island", true);
    faces.set_button_group(&modes);
    faces.set_pressed_no_signal(true);
    faces.connect("pressed", &Callable::from_object_method(&canvas, "set_face_mode"));
    let mut verts = button(&mut bar, "Verts", "mode_vertex", "UV vertex mode: click / box-select corners", true);
    verts.set_button_group(&modes);
    verts.connect("pressed", &Callable::from_object_method(&canvas, "set_vertex_mode"));
    bar.add_child(&VSeparator::new_alloc());
    let acts: [(&str, &str, &str, Act); 15] = [
        ("Manual", "uv_manual", "Bake manual UVs: freeze the automatic projection into per-corner UVs", Act::Manual),
        ("Auto", "uv_auto", "Back to the automatic (world / box) projection", Act::Auto),
        ("Planar", "uv_planar", "Planar project the faces as one island along their average normal (fit to 0-1)", Act::Planar),
        ("Box", "uv_box", "Box project: every face along its dominant axis (fit to 0-1)", Act::Box),
        ("Flip U", "uv_flip_u", "Flip U about the selection centre", Act::FlipU),
        ("Flip V", "uv_flip_v", "Flip V about the selection centre", Act::FlipV),
        ("Rot 90", "uv_rot90", "Rotate the UVs 90° about the selection centre", Act::Rot90),
        ("Fit", "uv_fit", "Fit to 0-1 (uniform)", Act::Fit),
        ("Fit U", "uv_fit_u", "Fit the width to 0-1", Act::FitU),
        ("Fit V", "uv_fit_v", "Fit the height to 0-1", Act::FitV),
        ("Sew", "weld", "Sew: corners that share a vertex move to their average UV", Act::Sew),
        ("Copy", "duplicate", "Copy the UV layout of the first selected face", Act::Copy),
        ("Paste", "uv_paste", "Paste the copied layout onto every selected face with the same corner count", Act::Paste),
        ("Texel", "uv_texel", "Match the texel density of every selected island to the first selected face", Act::Texel),
        ("Frame", "uv_frame", "Frame the 0-1 square", Act::Frame),
    ];
    for (i, (text, ic, tip, a)) in acts.iter().enumerate() {
        if i == 4 || i == 10 || i == 14 {
            bar.add_child(&VSeparator::new_alloc());
        }
        let mut b = button(&mut bar, text, ic, tip, false);
        b.connect("pressed", &Callable::from_object_method(&canvas, "act").bind(&[(*a as i64).to_variant()]));
    }
    let mut fs = button(&mut bar, "Frame Sel", "uv_frame_sel", "Frame the selection", false);
    fs.connect("pressed", &Callable::from_object_method(&canvas, "act").bind(&[(Act::FrameSel as i64).to_variant()]));
    bar.add_child(&VSeparator::new_alloc());
    for (text, ic, tip, flag, on) in [
        ("Lock 0-1", "uv_lock", "Keep moved / scaled islands inside the 0-1 square", FLAG_LOCK, false),
        ("Split", "split", "Split picking: grab one corner, tearing the seam", FLAG_SPLIT, false),
        ("Snap", "snap", "Snap moves and scales to the Step grid (rotation to 15°)", FLAG_SNAP, true),
        ("Tile", "uv_tile", "Tile the texture preview", FLAG_TILE, false),
    ] {
        let mut b = button(&mut bar, text, ic, tip, true);
        b.set_pressed_no_signal(on);
        b.connect("toggled", &Callable::from_object_method(&canvas, "set_flag").bind(&[flag.to_variant()]));
    }
    let spin = |min: f64, max: f64, step: f64, v: f64, tip: &str| {
        let mut sb = SpinBox::new_alloc();
        sb.set_min(min);
        sb.set_max(max);
        sb.set_step(step);
        sb.set_value_no_signal(v);
        sb.set_tooltip_text(tip);
        sb.set_allow_greater(true);
        sb
    };
    let mut lab = Label::new_alloc();
    lab.set_text("Step");
    bar.add_child(&lab);
    let mut step = spin(0.001, 1.0, 0.001, 0.0625, "UV grid / snap step (0.0625 = one texel of a 16 px texture)");
    step.connect("value_changed", &Callable::from_object_method(&canvas, "set_step"));
    bar.add_child(&step);
    bar.add_child(&VSeparator::new_alloc());
    for (axis, tip) in [(0, "Scale factor U"), (1, "Scale factor V")] {
        let mut sb = spin(-64.0, 64.0, 0.01, 1.0, tip);
        sb.set_allow_lesser(true);
        sb.connect("value_changed", &Callable::from_object_method(&canvas, "set_scale_factor").bind(&[(axis as i64).to_variant()]));
        bar.add_child(&sb);
    }
    let mut sc = button(&mut bar, "Scale", "gizmo_scale", "Scale the selection by the U / V factors", false);
    sc.connect("pressed", &Callable::from_object_method(&canvas, "act").bind(&[(Act::Scale as i64).to_variant()]));

    let mut info = Label::new_alloc();
    info.set_name("Info");
    info.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7));
    let mut help = Label::new_alloc();
    help.set_text("Drag: move   Ctrl+drag: scale (Shift: per axis)   Alt+drag: rotate   Middle: pan   Wheel: zoom   Double-click: island   Esc: cancel");
    help.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.5));
    help.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
    canvas.bind_mut().info = Some(info.clone());

    col.add_child(&bar);
    col.add_child(&canvas);
    col.add_child(&info);
    col.add_child(&help);
    panel.add_child(&col);
    win.add_child(&panel);
    (win, canvas)
}
