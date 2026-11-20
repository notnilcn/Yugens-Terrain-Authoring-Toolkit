//! UV editor window of the Pixel Builder over `pt_core::build::uv_edit`.
//!
//! A canvas shows the UVs of the edited builder mesh (the 3D face selection in Face mode, else
//! every face) over the first slot texture: faces with manual UVs are drawn brighter. Face /
//! vertex selection by click, Shift-click, double-click (UV island) or box; drag moves,
//! Ctrl+drag scales (Shift: per axis), Alt+drag rotates, middle-drag pans, wheel zooms, Esc
//! cancels. The bar bakes / clears manual UVs, projects (planar / box), flips, rotates, fits,
//! sews, copies / pastes layouts and matches texel density. Every edit is one undo step
//! (`PixelBuilderMeshData.set_blob`).
//!
//! Modeler-layout UV extras (`pt_core::build::xg_uv`): in Face mode, Ctrl+click stitches the clicked
//! face to the lowest selected face and Ctrl+Shift+click copies that face's layout onto it; the
//! *Manual UVs* Spherical projection; the *Auto UVs* settings form (Fill, Anchor, Flip U / V,
//! Swap UV, group) + *Apply Auto Settings*; *Select Group* (the UV island of the selection) and
//! *Export UV Template.* (size, grid, transparent background, EditorFileDialog *.png). A Ctrl
//! press with a selection still starts the scale drag; a release without movement is the click.

use std::collections::{BTreeMap, BTreeSet};

use godot::classes::control::{FocusMode, LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    BaseMaterial3D, Button, ButtonGroup, CheckBox, Control, EditorFileDialog, EditorInterface,
    Geometry2D, HFlowContainer, IControl, Image, InputEvent, InputEventKey, InputEventMouseButton,
    InputEventMouseMotion, Label, OptionButton, PanelContainer, ShaderMaterial, SpinBox, Texture2D,
    VBoxContainer, VSeparator, Window,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::BMesh;
use pt_core::build::uv_edit::{self as uve, Affine2, AutoUv, Corner, FitAxis};
use pt_core::build::xg_uv::{self, Anchor, AutoSettings, Fill, TemplateOptions};
use pt_core::math::{Vec2 as PVec2, v2};

use super::data::{PixelBuilderMeshData, to_p};
use super::node::PixelBuilderMesh;

const MESH_COLOR: Color = Color::from_rgb(0.3714, 0.9184, 0.8304);
const SELECTED_COLOR: Color = Color::from_rgb(0.9834, 0.7484, 0.1414);
const CANVAS_COLOR: Color = Color::from_rgb(0.1114, 0.1214, 0.1394);
const PICK_PX: f32 = 8.0;
/// A Ctrl press that travels less than this (px) before its release is a click (stitch / copy
/// layout), not a scale drag.
const CLICK_PX: f32 = 4.0;
const ZOOM_MIN: f32 = 8.0;
const ZOOM_MAX: f32 = 20005.0;
const FRAME_MARGIN: f32 = 0.1205;
const MIN_FACTOR: f32 = 0.0015;
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
    sort: DragKind,
    commence: Vector2,
    orig: BTreeMap<Corner, PVec2>,
    before: PackedByteArray,
    contents: Gd<PixelBuilderMeshData>,
    moved: bool,
    extent: uve::Rect,
    fulcrum: PVec2,
    factors: PVec2,
    inclination: f32,
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
    // Modeler layout UV extras.
    Spherical,
    ApplyAuto,
    Island,
}

const ACTS: [Act; 20] = [
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
    Act::Spherical,
    Act::ApplyAuto,
    Act::Island,
];

/// Auto-settings controls (`set_auto`).
const AUTO_FILL: i64 = 0;
const AUTO_ANCHOR: i64 = 1;
const AUTO_FLIP_U: i64 = 2;
const AUTO_FLIP_V: i64 = 3;
const AUTO_SWAP: i64 = 4;
const AUTO_GROUP: i64 = 5;
/// Template controls (`set_template`).
const TPL_SIZE: i64 = 0;
const TPL_GRID: i64 = 1;
const TPL_TRANSPARENT: i64 = 2;

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
    patch: bool,
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
    /// Auto UV settings applied by *Apply Auto*, and the UV template options.
    auto_settings: AutoSettings,
    template: TemplateOptions,
    /// A Ctrl (+Shift) press in Face mode: its position, whether Shift was held (copy layout
    /// instead of stitch) — run on a release that did not drag.
    ctrl_click: Option<(Vector2, bool)>,
    /// Last status message (shown in the info line).
    status: String,
    facts: Option<Gd<Label>>,
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
            patch: false,
            step: 0.0624,
            scale_uv: [1.0, 1.0],
            zoom: 220.0,
            pan: Vector2::new(60.0, 40.0),
            framed: false,
            pending_frame: None,
            uvs: BTreeMap::new(),
            drag: None,
            marquee: Rect2::default(),
            copied: Vec::new(),
            auto_settings: AutoSettings::default(),
            template: TemplateOptions::default(),
            ctrl_click: None,
            status: String::new(),
            facts: None,
            connected: None,
            base,
        }
    }

    fn draw(&mut self) {
        self.draw_canvas();
    }

    fn gui_input(&mut self, happening: Gd<InputEvent>) {
        self.on_input(happening);
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
    fn contents(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.target
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .and_then(|t| t.bind().contents())
    }

    fn self_auto(&self) -> AutoUv {
        let units = self.contents().map(|d| d.bind().units()).unwrap_or(1.0);
        let origin = self
            .target
            .as_ref()
            .filter(|t| t.is_instance_valid() && t.is_inside_tree())
            .map(|t| t.get_global_position())
            .unwrap_or_default();
        AutoUv {
            units_per_repeat: units,
            world_offset: to_p(origin),
        }
    }

    fn cage(&self) -> Option<BMesh> {
        self.contents().map(|d| d.bind().cage().clone())
    }

    fn breadth(&self, m: &BMesh) -> BTreeSet<usize> {
        let s: BTreeSet<usize> = self
            .scope_sel
            .iter()
            .copied()
            .filter(|f| *f < m.facets.len())
            .collect();
        if s.is_empty() {
            (0..m.facets.len()).collect()
        } else {
            s
        }
    }

    fn target_faces(&self, m: &BMesh) -> BTreeSet<usize> {
        if !self.sel_faces.is_empty() {
            return self.sel_faces.clone();
        }
        if !self.sel_corners.is_empty() {
            return uve::faces_of(&self.sel_corners);
        }
        self.breadth(m)
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
        let text = match self.cage() {
            None => "Select a Pixel Builder mesh".to_string(),
            Some(m) => {
                let manual = self
                    .uvs
                    .keys()
                    .filter(|f| m.facets[**f].by_hand_tex_coords().is_some())
                    .count();
                let (n, what) = if !self.sel_faces.is_empty() {
                    (self.sel_faces.len(), "faces")
                } else {
                    (self.sel_corners.len(), "corners")
                };
                let base = format!(
                    "{} faces ({manual} manual), {n} {what} selected",
                    self.uvs.len()
                );
                if self.status.is_empty() {
                    base
                } else {
                    format!("{base}   ·   {}", self.status)
                }
            }
        };
        if let Some(l) = &mut self.facts
            && l.is_instance_valid()
        {
            l.set_text(&text);
        }
    }

    /// Rebuilds the display cache and prunes the UV selection against the mesh.
    fn refresh_cache(&mut self) {
        self.uvs.clear();
        match self.cage() {
            Some(m) => {
                let a = self.self_auto();
                for f in self.breadth(&m) {
                    self.uvs.insert(f, uve::face_corner_uvs(&m, f, &a));
                }
                let uvs = &self.uvs;
                self.sel_faces.retain(|f| uvs.contains_key(f));
                self.sel_corners
                    .retain(|(f, c)| uvs.get(f).is_some_and(|u| *c < u.len()));
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
        self.zoom = size.x.min(size.y) * 0.8005;
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
        self.pan = size * 0.5 - gv(r.midpoint()) * self.zoom;
        self.framed = true;
        self.base_mut().queue_redraw();
    }

    fn selection_bounds(&self) -> Option<uve::Rect> {
        if !self.sel_corners.is_empty() {
            return uve::Rect::of(
                self.sel_corners
                    .iter()
                    .filter_map(|(f, c)| self.uvs.get(f).and_then(|u| u.get(*c)).copied()),
            );
        }
        let faces: Vec<usize> = if self.sel_faces.is_empty() {
            self.uvs.keys().copied().collect()
        } else {
            self.sel_faces.iter().copied().collect()
        };
        uve::Rect::of(
            faces
                .iter()
                .filter_map(|f| self.uvs.get(f))
                .flatten()
                .copied(),
        )
    }

    // ------------------------------------------------------------------ drawing

    fn slot_texture(&self) -> Option<Gd<Texture2D>> {
        let d = self.contents()?;
        let db = d.bind();
        let slots = db.stuff_pockets();
        for f in self.uvs.keys() {
            let slot = db.cage().facets[*f].stuff as usize;
            let Some(mat) = slots.get(slot).flatten() else {
                continue;
            };
            if let Ok(b) = mat.clone().try_cast::<BaseMaterial3D>()
                && let Some(t) =
                    b.get_texture(godot::classes::base_material_3d::TextureParam::ALBEDO)
            {
                return Some(t);
            }
            if let Ok(s) = mat.try_cast::<ShaderMaterial>() {
                for p in [
                    "albedo_texture",
                    "texture_albedo",
                    "albedo_tex",
                    "main_texture",
                ] {
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
        self.base_mut()
            .draw_rect(Rect2::new(Vector2::ZERO, size), CANVAS_COLOR);
        let Some(m) = self.cage() else {
            if let Some(font) = self.base().get_theme_default_font() {
                self.base_mut()
                    .draw_string_ex(
                        &font,
                        size * 0.5 + Vector2::new(-90.0, 0.0),
                        "Select a Pixel Builder mesh",
                    )
                    .modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.3505))
                    .done();
            }
            return;
        };
        // Texture underlay (first slot texture of the shown faces), tiled on request.
        if let Some(tex) = self.slot_texture() {
            if self.patch {
                let lo = self.px_to_uv(Vector2::ZERO);
                let hi = self.px_to_uv(size);
                let (lo, hi) = (
                    Vector2::new(lo.x.floor(), lo.y.floor()),
                    Vector2::new(hi.x.ceil(), hi.y.ceil()),
                );
                let cells = hi - lo;
                let ts = tex.get_size();
                let dst = Rect2::new(self.uv_to_px(pv(lo)), cells * self.zoom);
                let src = Rect2::new(lo * ts, cells * ts);
                self.base_mut().draw_texture_rect_region(&tex, dst, src);
            } else {
                let r = Rect2::new(
                    self.uv_to_px(PVec2::ZERO),
                    Vector2::new(self.zoom, self.zoom),
                );
                self.base_mut()
                    .draw_texture_rect_ex(&tex, r, false)
                    .modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.4005))
                    .done();
            }
        }
        // Grid at the snap step, and the 0-1 square.
        let step = self.step.max(0.0015);
        let (uv_min, uv_max) = (self.px_to_uv(Vector2::ZERO), self.px_to_uv(size));
        if (uv_max.x - uv_min.x) / step < 200.0 {
            let minor = Color::from_rgba(1.0, 1.0, 1.0, 0.0505);
            let mut x = (uv_min.x / step).floor() * step;
            while x <= uv_max.x {
                let (a, b) = (
                    self.uv_to_px(v2(x, uv_min.y)),
                    self.uv_to_px(v2(x, uv_max.y)),
                );
                self.base_mut().draw_line(a, b, minor);
                x += step;
            }
            let mut y = (uv_min.y / step).floor() * step;
            while y <= uv_max.y {
                let (a, b) = (
                    self.uv_to_px(v2(uv_min.x, y)),
                    self.uv_to_px(v2(uv_max.x, y)),
                );
                self.base_mut().draw_line(a, b, minor);
                y += step;
            }
        }
        let unit = Rect2::new(
            self.uv_to_px(PVec2::ZERO),
            Vector2::new(self.zoom, self.zoom),
        );
        let (uc, uw) = if self.lock {
            (
                Color::from_rgba(SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, 0.8005),
                2.0,
            )
        } else {
            (
                Color::from_rgba(MESH_COLOR.r, MESH_COLOR.g, MESH_COLOR.b, 0.4505),
                1.0,
            )
        };
        self.base_mut()
            .draw_rect_ex(unit, uc)
            .filled(false)
            .width(uw)
            .done();

        let entries: Vec<(usize, Vec<Vector2>)> = self
            .uvs
            .iter()
            .map(|(f, u)| (*f, u.iter().map(|p| self.uv_to_px(*p)).collect()))
            .collect();
        for (f, pts) in entries {
            let selected = self.sel_faces.contains(&f);
            let packed: PackedVector2Array = pts.iter().copied().collect();
            if selected
                && !Geometry2D::singleton()
                    .triangulate_polygon(&packed)
                    .is_empty()
            {
                self.base_mut().draw_colored_polygon(
                    &packed,
                    Color::from_rgba(SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, 0.1805),
                );
            }
            let manual = m.facets[f].by_hand_tex_coords().is_some();
            let wire = if selected {
                SELECTED_COLOR
            } else if manual {
                Color::from_rgba(MESH_COLOR.r, MESH_COLOR.g, MESH_COLOR.b, 0.8005)
            } else {
                Color::from_rgba(MESH_COLOR.r, MESH_COLOR.g, MESH_COLOR.b, 0.3005)
            };
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                self.base_mut()
                    .draw_line_ex(a, b, wire)
                    .width(1.5005)
                    .antialiased(true)
                    .done();
            }
            if self.vert_mode {
                for (ci, p) in pts.iter().enumerate() {
                    let col = if self.sel_corners.contains(&(f, ci)) {
                        SELECTED_COLOR
                    } else {
                        MESH_COLOR
                    };
                    self.base_mut().draw_rect(
                        Rect2::new(*p - Vector2::new(2.5005, 2.5005), Vector2::new(5.0, 5.0)),
                        col,
                    );
                }
            }
        }
        let Some(d) = &self.drag else { return };
        match d.sort {
            DragKind::Marquee => {
                let r = self.marquee.abs();
                self.base_mut().draw_rect(
                    r,
                    Color::from_rgba(MESH_COLOR.r, MESH_COLOR.g, MESH_COLOR.b, 0.1005),
                );
                self.base_mut()
                    .draw_rect_ex(
                        r,
                        Color::from_rgba(MESH_COLOR.r, MESH_COLOR.g, MESH_COLOR.b, 0.7005),
                    )
                    .filled(false)
                    .width(1.0)
                    .done();
            }
            DragKind::Scale | DragKind::Rotate => {
                let pp = self.uv_to_px(d.fulcrum);
                let text = if d.sort == DragKind::Rotate {
                    format!("{:.1}°", d.inclination.to_degrees())
                } else if (d.factors.x - d.factors.y).abs() < 1e-6 {
                    format!("× {:.3}", d.factors.x)
                } else {
                    format!("× {:.3}, {:.3}", d.factors.x, d.factors.y)
                };
                let cursor = d.cursor;
                self.base_mut()
                    .draw_line_ex(
                        pp - Vector2::new(6.0, 0.0),
                        pp + Vector2::new(6.0, 0.0),
                        SELECTED_COLOR,
                    )
                    .width(1.5005)
                    .done();
                self.base_mut()
                    .draw_line_ex(
                        pp - Vector2::new(0.0, 6.0),
                        pp + Vector2::new(0.0, 6.0),
                        SELECTED_COLOR,
                    )
                    .width(1.5005)
                    .done();
                if let Some(font) = self.base().get_theme_default_font() {
                    let pos = cursor + Vector2::new(16.0, -12.0);
                    self.base_mut()
                        .draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), &text)
                        .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8005))
                        .done();
                    self.base_mut()
                        .draw_string_ex(&font, pos, &text)
                        .modulate(Color::WHITE)
                        .done();
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ input

    fn on_input(&mut self, happening: Gd<InputEvent>) {
        if let Ok(mb) = happening.clone().try_cast::<InputEventMouseButton>() {
            let pos = mb.get_position();
            match mb.get_button_index() {
                MouseButton::WHEEL_UP if mb.is_pressed() => self.zoom_at(pos, 1.1505),
                MouseButton::WHEEL_DOWN if mb.is_pressed() => self.zoom_at(pos, 1.0 / 1.1505),
                MouseButton::MIDDLE => {
                    if mb.is_pressed() && self.drag.is_none() {
                        if let Some(data) = self.contents() {
                            self.drag = Some(self.new_drag(DragKind::Pan, pos, data));
                        }
                    } else if !mb.is_pressed()
                        && self.drag.as_ref().is_some_and(|d| d.sort == DragKind::Pan)
                    {
                        self.drag = None;
                    }
                }
                MouseButton::LEFT => {
                    if mb.is_pressed() {
                        self.left_press(&mb);
                    } else {
                        self.left_release(pos);
                    }
                }
                _ => {}
            }
            self.base_mut().accept_event();
        } else if let Ok(mm) = happening.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            let kind = self.drag.as_ref().map(|d| d.sort);
            match kind {
                Some(DragKind::Pan) => {
                    self.pending_frame = None;
                    self.pan += mm.get_relative();
                    self.base_mut().queue_redraw();
                }
                Some(DragKind::Move) => self.revise_relocate(pos),
                Some(DragKind::Scale) => self.update_scale(pos, mm.is_shift_pressed()),
                Some(DragKind::Rotate) => self.update_rotate(pos),
                Some(DragKind::Marquee) => {
                    self.marquee.size = pos - self.marquee.position;
                    self.base_mut().queue_redraw();
                }
                None => {}
            }
            self.base_mut().accept_event();
        } else if let Ok(k) = happening.try_cast::<InputEventKey>()
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
            sort: kind,
            commence: start,
            orig: BTreeMap::new(),
            before: PackedByteArray::new(),
            contents: data,
            moved: false,
            extent: uve::Rect {
                min: PVec2::ZERO,
                max: PVec2::ZERO,
            },
            fulcrum: PVec2::ZERO,
            factors: v2(1.0, 1.0),
            inclination: 0.0,
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
        if self.vert_mode {
            self.sel_corners.clone()
        } else {
            uve::corners_of(m, &self.sel_faces)
        }
    }

    fn left_press(&mut self, mb: &Gd<InputEventMouseButton>) {
        self.base_mut().grab_focus();
        let Some(m) = self.cage() else { return };
        let pos = mb.get_position();
        let shift = mb.is_shift_pressed();
        if mb.is_double_click() {
            self.grab_islet(pos, shift, &m);
            return;
        }
        self.ctrl_click = None;
        if mb.is_ctrl_pressed() && !mb.is_alt_pressed() && !self.vert_mode {
            // Ctrl+click stitches / Ctrl+Shift+click copies the layout onto the clicked face;
            // decided on release (a drag scales instead).
            self.ctrl_click = Some((pos, shift));
        }
        if mb.is_alt_pressed() || mb.is_ctrl_pressed() {
            let corners = self.sel_or_face_corners(&m);
            if !corners.is_empty() {
                let kind = if mb.is_alt_pressed() {
                    DragKind::Rotate
                } else {
                    DragKind::Scale
                };
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

    fn left_release(&mut self, at: Vector2) {
        let click = self.ctrl_click.take();
        let Some(d) = self.drag.take() else {
            if let Some((pos, copy)) = click {
                self.ctrl_click_face(pos, copy);
            }
            return;
        };
        // A Ctrl press released within `CLICK_PX` is a click, whatever the (snapped / per-axis)
        // scale preview did meanwhile: restore the UVs, then stitch / copy.
        if let Some((pos, copy)) = click
            && d.sort == DragKind::Scale
            && at.distance_to(d.commence) < CLICK_PX
        {
            let mut data = d.contents.clone();
            data.bind_mut().set_blob(d.before);
            self.ctrl_click_face(pos, copy);
            self.base_mut().queue_redraw();
            return;
        }
        match d.sort {
            DragKind::Move | DragKind::Scale | DragKind::Rotate => {
                let name = match d.sort {
                    DragKind::Scale => "Scale UVs",
                    DragKind::Rotate => "Rotate UVs",
                    _ => "Move UVs",
                };
                let mut data = d.contents.clone();
                if d.moved && self.contents().as_ref() == Some(&data) {
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

    /// Ctrl(+Shift)+click on face `pos`: the lowest selected face
    /// is the source; another clicked face is stitched to it (Ctrl) or gets its layout
    /// (Ctrl+Shift). Without a source / on the source it selects like a plain click (adding).
    fn ctrl_click_face(&mut self, pos: Vector2, copy: bool) {
        let Some(target) = self.pick_face(pos) else {
            return;
        };
        let source = self.sel_faces.iter().next().copied();
        match source {
            Some(src) if src != target => {
                let a = self.self_auto();
                let mut err = None;
                let name = if copy { "Copy UV layout" } else { "Stitch UVs" };
                self.edit(name, |m, _| {
                    let r = if copy {
                        xg_uv::duplicate_arrangement(m, src, target, &a)
                    } else {
                        xg_uv::seam(m, src, target, &a)
                    };
                    err = r.err();
                });
                self.status = match err {
                    Some(e) => e,
                    None if copy => format!("Layout of face {src} copied onto face {target}"),
                    None => format!("Face {target} stitched to face {src}"),
                };
                self.refresh_cache();
            }
            _ => {
                self.sel_faces.insert(target);
                self.update_info();
                self.base_mut().queue_redraw();
            }
        }
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
        self.uvs
            .iter()
            .find(|(_, poly)| uve::point_in_polygon(uv, poly))
            .map(|(f, _)| *f)
    }

    fn grab_islet(&mut self, px: Vector2, additive: bool, m: &BMesh) {
        let Some(fi) = self.pick_face(px) else { return };
        // `MeshUV::island`: also follows automatic faces (world-continuous UVs).
        let island: BTreeSet<usize> = xg_uv::uv_island(m, fi, &self.self_auto(), 1e-5)
            .into_iter()
            .filter(|f| self.uvs.contains_key(f))
            .collect();
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
        let Some(data) = self.contents() else { return };
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
        let hits: Vec<(usize, usize)> = self
            .uvs
            .iter()
            .flat_map(|(f, uvs)| {
                uvs.iter()
                    .enumerate()
                    .filter(|(_, uv)| r.contains_point(self.uv_to_px(**uv)))
                    .map(|(ci, _)| (*f, ci))
                    .collect::<Vec<_>>()
            })
            .collect();
        if self.vert_mode {
            self.sel_corners.extend(hits);
        } else {
            self.sel_faces.extend(hits.into_iter().map(|h| h.0));
        }
        self.update_info();
    }

    /// Bakes the corners' faces to manual UVs and starts a move / scale / rotate drag.
    fn begin_transform(&mut self, px: Vector2, corners: BTreeSet<Corner>, kind: DragKind) {
        let Some(mut data) = self.contents() else {
            return;
        };
        if corners.is_empty() {
            return;
        }
        let a = self.self_auto();
        let before = data.bind().get_blob();
        let mut m = data.bind().cage().clone();
        uve::freeze_manual_uvs(&mut m, &uve::faces_of(&corners), &a);
        let orig: BTreeMap<Corner, PVec2> = corners
            .iter()
            .filter_map(|(f, c)| {
                m.facets
                    .get(*f)
                    .and_then(|x| x.uv_coords.get(*c))
                    .map(|uv| ((*f, *c), *uv))
            })
            .collect();
        let bounds = uve::Rect::of(orig.values().copied()).unwrap_or(uve::Rect {
            min: PVec2::ZERO,
            max: PVec2::ZERO,
        });
        data.bind_mut().substitute(m);
        data.emit_changed();
        let mut d = self.new_drag(kind, px, data);
        d.before = before;
        d.orig = orig;
        d.extent = bounds;
        d.fulcrum = bounds.midpoint();
        self.drag = Some(d);
    }

    fn write_uvs(&mut self, moved: BTreeMap<Corner, PVec2>) {
        let Some(d) = &self.drag else { return };
        let mut data = d.contents.clone();
        let mut m = data.bind().cage().clone();
        uve::set_uvs(&mut m, &moved);
        data.bind_mut().substitute(m);
        data.emit_changed();
    }

    fn lock_moved(&self, moved: &mut BTreeMap<Corner, PVec2>) {
        if !self.lock || moved.is_empty() {
            return;
        }
        let Some(r) = uve::Rect::of(moved.values().copied()) else {
            return;
        };
        let shift = uve::unit_lock_offset(r);
        if shift != PVec2::ZERO {
            for v in moved.values_mut() {
                *v = *v + shift;
            }
        }
    }

    fn revise_relocate(&mut self, px: Vector2) {
        let (step, snap, lock, zoom) = (self.step.max(0.0015), self.snap, self.lock, self.zoom);
        let Some(d) = &mut self.drag else { return };
        let mut delta = pv((px - d.commence) / zoom);
        if snap {
            delta = v2(
                (delta.x / step).round() * step,
                (delta.y / step).round() * step,
            );
        }
        if lock {
            delta = delta
                + uve::unit_lock_offset(uve::Rect {
                    min: d.extent.min + delta,
                    max: d.extent.max + delta,
                });
        }
        if delta != PVec2::ZERO {
            d.moved = true;
        }
        let moved: BTreeMap<Corner, PVec2> = d.orig.iter().map(|(k, v)| (*k, *v + delta)).collect();
        self.write_uvs(moved);
    }

    fn update_scale(&mut self, px: Vector2, per_axis: bool) {
        let (step, snap, lock) = (self.step.max(0.0015), self.snap, self.lock);
        let Some(d) = &self.drag else { return };
        let pivot_px = self.uv_to_px(d.fulcrum);
        let safe = |v: f32| {
            if v.abs() < 1e-6 {
                4.0
            } else {
                v.signum() * v.abs().max(4.0)
            }
        };
        let mut f = if per_axis {
            v2(
                (px.x - pivot_px.x) / safe(d.commence.x - pivot_px.x),
                (px.y - pivot_px.y) / safe(d.commence.y - pivot_px.y),
            )
        } else {
            let k = pivot_px.distance_to(px) / pivot_px.distance_to(d.commence).max(4.0);
            v2(k, k)
        };
        if snap {
            f = v2((f.x / step).round() * step, (f.y / step).round() * step);
        }
        let floor = |x: f32| {
            if x.abs() >= MIN_FACTOR {
                x
            } else if x >= 0.0 {
                MIN_FACTOR
            } else {
                -MIN_FACTOR
            }
        };
        f = v2(floor(f.x), floor(f.y));
        if lock {
            f = uve::unit_lock_factors(d.extent.size(), f, !per_axis);
        }
        let pivot = d.fulcrum;
        let mut moved: BTreeMap<Corner, PVec2> = d
            .orig
            .iter()
            .map(|(k, v)| (*k, pivot + v2((v.x - pivot.x) * f.x, (v.y - pivot.y) * f.y)))
            .collect();
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
        let pivot_px = self.uv_to_px(d.fulcrum);
        let mut ang = (px - pivot_px).angle() - (d.commence - pivot_px).angle();
        if snap {
            let s = 15f32.to_radians();
            ang = (ang / s).round() * s;
        }
        let xf = Affine2::about(d.fulcrum, ang, v2(1.0, 1.0));
        let mut moved: BTreeMap<Corner, PVec2> =
            d.orig.iter().map(|(k, v)| (*k, xf.enact(*v))).collect();
        self.lock_moved(&mut moved);
        if let Some(d) = &mut self.drag {
            if ang.abs() > 1e-6 {
                d.moved = true;
            }
            d.inclination = ang;
            d.cursor = px;
        }
        self.write_uvs(moved);
        self.base_mut().queue_redraw();
    }

    /// Ends a live gesture like Esc (restoring the UVs). False when nothing was live.
    fn cancel_drag(&mut self) -> bool {
        let Some(d) = self.drag.take() else {
            return false;
        };
        if matches!(d.sort, DragKind::Move | DragKind::Scale | DragKind::Rotate) {
            let mut data = d.contents;
            data.bind_mut().set_blob(d.before);
        }
        self.base_mut().queue_redraw();
        true
    }

    /// Applies `f` to a copy of the mesh and records one undo step.
    fn edit(&mut self, name: &str, f: impl FnOnce(&mut BMesh, &Self)) {
        let Some(mut data) = self.contents() else {
            return;
        };
        let (before, mut m) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        f(&mut m, self);
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            commit(&format!("Pixel Builder: {name}"), &data, before, after);
        }
    }

    fn about_center(&mut self, scale: PVec2, rot: f32, name: &str) {
        let a = self.self_auto();
        let lock = self.lock;
        self.edit(name, |m, s| {
            let c = s.target_corners(m);
            uve::transform_about_bounds(m, &c, scale, rot, lock, &a);
        });
    }

    fn execute(&mut self, act: Act) {
        let a = self.self_auto();
        match act {
            Act::Manual => self.edit("UVs to manual", |m, s| {
                let f = s.target_faces(m);
                uve::freeze_manual_uvs(m, &f, &a);
            }),
            Act::Auto => self.edit("UVs to auto", |m, s| {
                let f = s.target_faces(m);
                uve::drop_manual(m, &f);
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
            Act::Rot90 => {
                self.about_center(v2(1.0, 1.0), std::f32::consts::FRAC_PI_2, "Rotate UVs 90")
            }
            Act::Fit | Act::FitU | Act::FitV => {
                let (axis, name) = match act {
                    Act::FitU => (FitAxis::U, "Fit UVs U"),
                    Act::FitV => (FitAxis::V, "Fit UVs V"),
                    _ => (FitAxis::Both, "Fit UVs"),
                };
                self.edit(name, |m, s| {
                    let f = s.target_faces(m);
                    uve::freeze_manual_uvs(m, &f, &a);
                    uve::fit_to_unit(m, &f, axis);
                });
            }
            Act::Sew => self.edit("Sew UVs", |m, s| {
                let c = s.target_corners(m);
                uve::weld_uvs(m, &c);
            }),
            Act::Copy => {
                if let Some(m) = self.cage()
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
                    uve::freeze_manual_uvs(m, &f, &a);
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
            Act::Spherical => {
                let mut err = None;
                self.edit("Spherical project UVs", |m, s| {
                    let f = s.target_faces(m);
                    err = xg_uv::spherical_project(m, &f).err();
                });
                self.status = err.unwrap_or_else(|| "Spherical projection".into());
            }
            Act::ApplyAuto => {
                let st = self.auto_settings;
                let mut baked = 0;
                self.edit("Apply auto UV settings", |m, s| {
                    let f = s.target_faces(m);
                    baked = xg_uv::enact_self_auto_prefs(m, &f, &st, &a);
                });
                self.status = if baked > 0 {
                    format!("Auto settings applied ({baked} faces baked to manual UVs)")
                } else {
                    "Auto settings applied (automatic UVs)".into()
                };
            }
            Act::Island => {
                if let Some(m) = self.cage() {
                    let seeds: Vec<usize> = if !self.sel_faces.is_empty() {
                        self.sel_faces.iter().copied().collect()
                    } else {
                        uve::faces_of(&self.sel_corners).into_iter().collect()
                    };
                    let mut all = BTreeSet::new();
                    for f in seeds {
                        all.extend(xg_uv::uv_island(&m, f, &a, 1e-5));
                    }
                    all.retain(|f| self.uvs.contains_key(f));
                    if all.is_empty() {
                        self.status = "Select UV island: select a face first".into();
                    } else if self.vert_mode {
                        self.sel_corners = uve::corners_of(&m, &all);
                    } else {
                        self.sel_faces = all;
                    }
                }
            }
        }
        self.refresh_cache();
    }

    /// Writes the UV template PNG of the shown faces (the selection, else every shown face).
    fn export_template_png(&mut self, path: &str) -> Result<String, String> {
        let m = self.cage().ok_or("Select a Pixel Builder mesh")?;
        let faces = self.target_faces(&m);
        let px = xg_uv::template_rgba8(&m, &faces, &self.self_auto(), &self.template)?;
        let n = self.template.size as i32;
        let img = Image::create_from_data(
            n,
            n,
            false,
            godot::classes::image::Format::RGBA8,
            &PackedByteArray::from(px.as_slice()),
        )
        .ok_or("could not create the image")?;
        let err = img.save_png(path);
        if err != godot::global::Error::OK {
            return Err(format!("could not write {path} ({err:?})"));
        }
        // The dialog has FILESYSTEM access (OS paths): a file inside the project is imported
        // under its res:// path — re-imported when the editor knows it, else found by a scan.
        let local = godot::classes::ProjectSettings::singleton()
            .localize_path(path)
            .to_string();
        if local.starts_with("res://")
            && let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem()
        {
            if fs.get_file_type(&local).is_empty() {
                fs.call_deferred("scan", &[]);
            } else {
                // Deferred: this runs inside the bound canvas (an import may emit back into it).
                fs.update_file(&local);
                fs.call_deferred(
                    "reimport_files",
                    &[PackedStringArray::from([GString::from(&local)].as_slice()).to_variant()],
                );
            }
        }
        let shown = if local.starts_with("res://") {
            local
        } else {
            path.to_string()
        };
        Ok(format!(
            "UV template ({n} px, {} faces) saved to {shown}",
            faces.len()
        ))
    }
}

/// Largest UV template (px, square): 4096 (RGBA8, 64 MiB); 8192 (256 MiB) is allowed.
const TEMPLATE_MAX: u32 = 4096;

fn commit(
    name: &str,
    data: &Gd<PixelBuilderMeshData>,
    before: PackedByteArray,
    after: PackedByteArray,
) {
    let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else {
        return;
    };
    ur.create_action(name);
    ur.add_do_method(data, "set_blob", &[after.to_variant()]);
    ur.add_undo_method(data, "set_blob", &[before.to_variant()]);
    ur.commit_action_ex().execute(false).done();
}

#[godot_api]
impl PixelBuilderUvCanvas {
    /// Shows `target`'s UVs; `faces` = the 3D face selection (empty = every face).
    #[func]
    pub fn set_target(&mut self, addressee: Option<Gd<PixelBuilderMesh>>, faces: PackedInt32Array) {
        // Validity first: the old target may have been freed (its scene closed) while the editor was open.
        let same = self
            .target
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .map(|t| t.instance_id())
            == addressee
                .as_ref()
                .filter(|t| t.is_instance_valid())
                .map(|t| t.instance_id());
        if !same {
            self.cancel_drag();
            self.sel_faces.clear();
            self.sel_corners.clear();
        }
        self.target = addressee;
        self.scope_sel = super::data::facet_batch(&faces);
        // Follow the data resource: undo / redo and 3D edits refresh the canvas.
        let data = self.contents();
        if self.connected != data {
            let cb = self.to_gd().callable("refresh");
            if let Some(mut old) = self.connected.take()
                && old.is_instance_valid()
                && old.is_connected("changed", &cb)
            {
                old.disconnect("changed", &cb);
            }
            if let Some(mut d) = data.clone() {
                d.connect_flags(
                    "changed",
                    &cb,
                    godot::classes::object::ConnectFlags::DEFERRED,
                );
            }
            self.connected = data;
        }
        if self.drag.is_none() {
            self.refresh_cache();
        }
    }

    #[func]
    pub fn renew(&mut self) {
        if self
            .drag
            .as_ref()
            .is_none_or(|d| d.sort == DragKind::Pan || d.sort == DragKind::Marquee)
        {
            self.refresh_cache();
        } else {
            self.base_mut().queue_redraw();
        }
    }

    #[func]
    fn perform(&mut self, token_id: i64) {
        if let Some(a) = ACTS.iter().copied().find(|a| *a as i64 == token_id) {
            self.execute(a);
        }
    }

    #[func]
    fn set_flag(&mut self, on: bool, token_id: i64) {
        match token_id {
            FLAG_VERTS => {
                self.vert_mode = on;
                self.sel_faces.clear();
                self.sel_corners.clear();
                self.update_info();
            }
            FLAG_LOCK => self.lock = on,
            FLAG_SPLIT => self.split = on,
            FLAG_SNAP => self.snap = on,
            FLAG_TILE => self.patch = on,
            _ => {}
        }
        self.base_mut().queue_redraw();
    }

    /// An Auto UV settings control changed (`AUTO_*` ids).
    #[func]
    fn assign_self_auto(&mut self, value: f64, token_id: i64) {
        let s = &mut self.auto_settings;
        match token_id {
            AUTO_FILL => s.flood = [Fill::Tile, Fill::Fit, Fill::Stretch][(value as usize).min(2)],
            AUTO_ANCHOR => s.anchor = Anchor::ALL[(value as usize).min(9)],
            AUTO_FLIP_U => s.flip_u = value >= 0.5,
            AUTO_FLIP_V => s.flip_v = value >= 0.5,
            AUTO_SWAP => s.swap_uv = value >= 0.5,
            AUTO_GROUP => s.cluster = value >= 0.5,
            _ => {}
        }
    }

    #[func]
    fn set_auto_check(&mut self, on: bool, token_id: i64) {
        self.assign_self_auto(if on { 1.0 } else { 0.0 }, token_id);
    }

    #[func]
    fn set_auto_option(&mut self, index: i64, token_id: i64) {
        self.assign_self_auto(index as f64, token_id);
    }

    /// A template option changed (`TPL_*` ids).
    #[func]
    fn set_template(&mut self, value: f64, token_id: i64) {
        match token_id {
            TPL_SIZE => self.template.size = (value as u32).clamp(64, TEMPLATE_MAX),
            TPL_GRID => self.template.grid = value >= 0.5,
            TPL_TRANSPARENT => {
                self.template.background = if value >= 0.5 { None } else { Some([1.0; 4]) }
            }
            _ => {}
        }
    }

    #[func]
    fn set_template_check(&mut self, on: bool, token_id: i64) {
        self.set_template(if on { 1.0 } else { 0.0 }, token_id);
    }

    /// The template file dialog's path.
    #[func]
    fn export_template(&mut self, path: GString) {
        self.status = match self.export_template_png(&path.to_string()) {
            Ok(m) => m,
            Err(e) => format!("UV template: {e}"),
        };
        self.update_info();
    }

    /// Test hook: the same dispatch as `gui_input` (mouse / key events in canvas pixels).
    #[func]
    pub fn debug_input(&mut self, happening: Gd<InputEvent>) {
        self.on_input(happening);
    }

    /// Test hook: canvas pixel of the centre of face `f`'s shown UVs (`(-1, -1)` if not shown).
    #[func]
    pub fn debug_face_px(&self, f: i64) -> Vector2 {
        let Some(u) = self.uvs.get(&(f.max(0) as usize)) else {
            return Vector2::new(-1.0, -1.0);
        };
        let n = u.len().max(1) as f32;
        let c = u.iter().fold(PVec2::ZERO, |a, b| a + *b);
        self.uv_to_px(v2(c.x / n, c.y / n))
    }

    /// Test hook: `{faces (selected), corners, status, shown}`.
    #[func]
    pub fn debug_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let faces: PackedInt32Array = self.sel_faces.iter().map(|f| *f as i32).collect();
        d.set("faces", &faces);
        d.set("corners", self.sel_corners.len() as i64);
        d.set("status", self.status.as_str());
        d.set("shown", self.uvs.len() as i64);
        d
    }

    /// Test hook: Auto UV settings `{fill, anchor, flip_u, flip_v, swap, group}` (indices / bools).
    #[func]
    pub fn inspect_self_auto_prefs(&mut self, s: VarDictionary) {
        for (k, id) in [
            ("fill", AUTO_FILL),
            ("anchor", AUTO_ANCHOR),
            ("flip_u", AUTO_FLIP_U),
            ("flip_v", AUTO_FLIP_V),
            ("swap", AUTO_SWAP),
            ("group", AUTO_GROUP),
        ] {
            if let Some(v) = s.get(k) {
                let x = v
                    .try_to::<f64>()
                    .or_else(|_| v.try_to::<i64>().map(|i| i as f64))
                    .or_else(|_| v.try_to::<bool>().map(|b| if b { 1.0 } else { 0.0 }))
                    .unwrap_or(0.0);
                self.assign_self_auto(x, id);
            }
        }
    }

    /// Test hook: writes the UV template (`size`, `grid`, `transparent`); returns the status.
    #[func]
    pub fn debug_export_template(
        &mut self,
        path: GString,
        size: i64,
        grid: bool,
        transparent: bool,
    ) -> GString {
        self.set_template(size as f64, TPL_SIZE);
        self.set_template(if grid { 1.0 } else { 0.0 }, TPL_GRID);
        self.set_template(if transparent { 1.0 } else { 0.0 }, TPL_TRANSPARENT);
        self.export_template(path);
        self.status.as_str().into()
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
        self.step = (v as f32).max(0.0015);
        self.base_mut().queue_redraw();
    }

    #[func]
    fn set_scale_factor(&mut self, v: f64, axis: i64) {
        self.scale_uv[axis.clamp(0, 1) as usize] = v as f32;
    }

    /// Test hook: select UV faces (face mode).
    #[func]
    pub fn debug_select(&mut self, faces: PackedInt32Array) {
        self.sel_faces = super::data::facet_batch(&faces);
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
            "spherical" => Act::Spherical,
            "apply_auto" => Act::ApplyAuto,
            "island" => Act::Island,
            "frame_sel" => Act::FrameSel,
            other => {
                godot_warn!("PixelBuilderUvCanvas.debug_act: unknown action {other:?}");
                return;
            }
        };
        self.execute(a);
    }
}

fn glyph(name: &str) -> Option<Gd<Texture2D>> {
    super::ui::mono_icon(&format!("{ICONS}{name}.svg"), 1.0)
}

/// Builds the "UV Editor" window around a new [`build_panel`]. The window hides on close.
pub fn build_window() -> (Gd<Window>, Gd<PanelContainer>, Gd<PixelBuilderUvCanvas>) {
    let (panel, canvas) = build_panel();
    (wrap_window(&panel), panel, canvas)
}

/// The floating window host of the UV panel (MST layout). The panel can move to the "Mesh UV"
/// bottom dock (modeler layout) and back; the window then stays, hidden and empty.
pub fn wrap_window(panel: &Gd<PanelContainer>) -> Gd<Window> {
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
    win.add_child(panel);
    win
}

/// The UV editor's content (button bars + canvas + status), host-independent: its only
/// connections are to its own controls and the canvas, never to the plugin, so moving it
/// between hosts fires nothing into a bound object.
pub fn build_panel() -> (Gd<PanelContainer>, Gd<PixelBuilderUvCanvas>) {
    let s = crate::editor::ui_kit::editor_scale();
    let mut panel = PanelContainer::new_alloc();
    panel.set_name("PixelBuilderUvPanel");
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
        if let Some(t) = glyph(ic) {
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
    let mut faces = button(
        &mut bar,
        "Faces",
        "mode_face",
        "UV face mode: click / box-select faces, double-click selects a UV island",
        true,
    );
    faces.set_button_group(&modes);
    faces.set_pressed_no_signal(true);
    faces.connect(
        "pressed",
        &Callable::from_object_method(&canvas, "set_face_mode"),
    );
    let mut verts = button(
        &mut bar,
        "Verts",
        "mode_vertex",
        "UV vertex mode: click / box-select corners",
        true,
    );
    verts.set_button_group(&modes);
    verts.connect(
        "pressed",
        &Callable::from_object_method(&canvas, "set_vertex_mode"),
    );
    bar.add_child(&VSeparator::new_alloc());
    let acts: [(&str, &str, &str, Act); 17] = [
        (
            "Manual",
            "uv_manual",
            "Bake manual UVs: freeze the automatic projection into per-corner UVs",
            Act::Manual,
        ),
        (
            "Auto",
            "uv_auto",
            "Back to the automatic (world / box) projection",
            Act::Auto,
        ),
        (
            "Planar",
            "uv_planar",
            "Planar project the faces as one island along their average normal (fit to 0-1)",
            Act::Planar,
        ),
        (
            "Box",
            "uv_box",
            "Box project: every face along its dominant axis (fit to 0-1)",
            Act::Box,
        ),
        (
            "Spherical",
            "shape_sphere",
            "Spherical project: longitude / latitude around the faces' centre (one repeat around, pole to pole)",
            Act::Spherical,
        ),
        (
            "Flip U",
            "uv_flip_u",
            "Flip U about the selection centre",
            Act::FlipU,
        ),
        (
            "Flip V",
            "uv_flip_v",
            "Flip V about the selection centre",
            Act::FlipV,
        ),
        (
            "Rot 90",
            "uv_rot90",
            "Rotate the UVs 90° about the selection centre",
            Act::Rot90,
        ),
        ("Fit", "uv_fit", "Fit to 0-1 (uniform)", Act::Fit),
        ("Fit U", "uv_fit_u", "Fit the width to 0-1", Act::FitU),
        ("Fit V", "uv_fit_v", "Fit the height to 0-1", Act::FitV),
        (
            "Sew",
            "weld",
            "Sew: corners that share a vertex move to their average UV",
            Act::Sew,
        ),
        (
            "Copy",
            "duplicate",
            "Copy the UV layout of the first selected face",
            Act::Copy,
        ),
        (
            "Paste",
            "uv_paste",
            "Paste the copied layout onto every selected face with the same corner count",
            Act::Paste,
        ),
        (
            "Texel",
            "uv_texel",
            "Match the texel density of every selected island to the first selected face",
            Act::Texel,
        ),
        (
            "Island",
            "grow",
            "Select UV island: every face connected to the selection at the same UVs (also automatic faces)",
            Act::Island,
        ),
        ("Frame", "uv_frame", "Frame the 0-1 square", Act::Frame),
    ];
    for (i, (text, ic, tip, a)) in acts.iter().enumerate() {
        if i == 5 || i == 11 || i == 16 {
            bar.add_child(&VSeparator::new_alloc());
        }
        let mut b = button(&mut bar, text, ic, tip, false);
        b.connect(
            "pressed",
            &Callable::from_object_method(&canvas, "act").bind(&[(*a as i64).to_variant()]),
        );
    }
    let mut fs = button(
        &mut bar,
        "Frame Sel",
        "uv_frame_sel",
        "Frame the selection",
        false,
    );
    fs.connect(
        "pressed",
        &Callable::from_object_method(&canvas, "act").bind(&[(Act::FrameSel as i64).to_variant()]),
    );
    bar.add_child(&VSeparator::new_alloc());
    for (text, ic, tip, flag, on) in [
        (
            "Lock 0-1",
            "uv_lock",
            "Keep moved / scaled islands inside the 0-1 square",
            FLAG_LOCK,
            false,
        ),
        (
            "Split",
            "split",
            "Split picking: grab one corner, tearing the seam",
            FLAG_SPLIT,
            false,
        ),
        (
            "Snap",
            "snap",
            "Snap moves and scales to the Step grid (rotation to 15°)",
            FLAG_SNAP,
            true,
        ),
        (
            "Tile",
            "uv_tile",
            "Tile the texture preview",
            FLAG_TILE,
            false,
        ),
    ] {
        let mut b = button(&mut bar, text, ic, tip, true);
        b.set_pressed_no_signal(on);
        b.connect(
            "toggled",
            &Callable::from_object_method(&canvas, "set_flag").bind(&[flag.to_variant()]),
        );
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
    let mut step = spin(
        0.0015,
        1.0,
        0.0015,
        0.0624,
        "UV grid / snap step (0.0625 = one texel of a 16 px texture)",
    );
    step.connect(
        "value_changed",
        &Callable::from_object_method(&canvas, "set_step"),
    );
    bar.add_child(&step);
    bar.add_child(&VSeparator::new_alloc());
    for (axis, tip) in [(0, "Scale factor U"), (1, "Scale factor V")] {
        let mut sb = spin(-64.0, 64.0, 0.0105, 1.0, tip);
        sb.set_allow_lesser(true);
        sb.connect(
            "value_changed",
            &Callable::from_object_method(&canvas, "set_scale_factor")
                .bind(&[(axis as i64).to_variant()]),
        );
        bar.add_child(&sb);
    }
    let mut sc = button(
        &mut bar,
        "Scale",
        "gizmo_scale",
        "Scale the selection by the U / V factors",
        false,
    );
    sc.connect(
        "pressed",
        &Callable::from_object_method(&canvas, "act").bind(&[(Act::Scale as i64).to_variant()]),
    );

    // ---------------------------------------------------------------- Auto UVs (modeler)
    let mut auto = HFlowContainer::new_alloc();
    auto.set_name("AutoUvs");
    let small_label = |row: &mut Gd<HFlowContainer>, text: &str| {
        let mut l = Label::new_alloc();
        l.set_text(text);
        row.add_child(&l);
    };
    small_label(&mut auto, "Auto UVs:");
    let option = |row: &mut Gd<HFlowContainer>, items: &[&str], tip: &str, id: i64| {
        let mut ob = OptionButton::new_alloc();
        for it in items {
            ob.add_item(*it);
        }
        ob.set_tooltip_text(tip);
        ob.set_focus_mode(FocusMode::NONE);
        ob.connect(
            "item_selected",
            &Callable::from_object_method(&canvas, "set_auto_option").bind(&[id.to_variant()]),
        );
        row.add_child(&ob);
    };
    option(
        &mut auto,
        &["Tile", "Fit", "Stretch"],
        "Fill: Tile = world-density repeats; Fit = the longer side spans 0-1; Stretch = both sides span 0-1",
        AUTO_FILL,
    );
    option(
        &mut auto,
        &[
            "No anchor",
            "Upper left",
            "Upper centre",
            "Upper right",
            "Middle left",
            "Centre",
            "Middle right",
            "Lower left",
            "Lower centre",
            "Lower right",
        ],
        "Anchor: which point of the projected UVs sits on the same point of the 0-1 square",
        AUTO_ANCHOR,
    );
    let check = |row: &mut Gd<HFlowContainer>, text: &str, tip: &str, method: &str, id: i64| {
        let mut cb = CheckBox::new_alloc();
        cb.set_text(text);
        cb.set_tooltip_text(tip);
        cb.set_focus_mode(FocusMode::NONE);
        cb.connect(
            "toggled",
            &Callable::from_object_method(&canvas, method).bind(&[id.to_variant()]),
        );
        row.add_child(&cb);
        cb
    };
    check(
        &mut auto,
        "Flip U",
        "Mirror the projection horizontally",
        "set_auto_check",
        AUTO_FLIP_U,
    );
    check(
        &mut auto,
        "Flip V",
        "Mirror the projection vertically",
        "set_auto_check",
        AUTO_FLIP_V,
    );
    check(
        &mut auto,
        "Swap UV",
        "Swap U and V (a quarter turn with a mirror)",
        "set_auto_check",
        AUTO_SWAP,
    );
    check(
        &mut auto,
        "Group",
        "Treat the faces as one batch: one projection and one bounds, so the texture runs across them",
        "set_auto_check",
        AUTO_GROUP,
    );
    let mut apply = button(
        &mut auto,
        "Apply Auto",
        "uv_auto",
        "Apply auto settings: re-project the faces (the selection, else all shown) with these settings",
        false,
    );
    apply.connect(
        "pressed",
        &Callable::from_object_method(&canvas, "act").bind(&[(Act::ApplyAuto as i64).to_variant()]),
    );
    auto.add_child(&VSeparator::new_alloc());
    // ---------------------------------------------------------------- UV template (modeler)
    small_label(&mut auto, "Template:");
    let mut tsize = spin(
        64.0,
        TEMPLATE_MAX as f64,
        64.0,
        1024.0,
        "UV template size in pixels (square, 64-4096)",
    );
    tsize.set_allow_greater(false);
    tsize.connect(
        "value_changed",
        &Callable::from_object_method(&canvas, "set_template").bind(&[TPL_SIZE.to_variant()]),
    );
    auto.add_child(&tsize);
    let mut grid = check(
        &mut auto,
        "Grid",
        "Draw the 1/8 grid under the outlines",
        "set_template_check",
        TPL_GRID,
    );
    grid.set_pressed_no_signal(true);
    check(
        &mut auto,
        "Transparent",
        "Transparent background (else white)",
        "set_template_check",
        TPL_TRANSPARENT,
    );
    let mut dlg = EditorFileDialog::new_alloc();
    dlg.set_name("TemplateDialog");
    dlg.set_file_mode(godot::classes::file_dialog::FileMode::SAVE_FILE);
    dlg.set_access(godot::classes::file_dialog::Access::FILESYSTEM);
    dlg.set_title("Export UV Template");
    dlg.add_filter_ex("*.png").description("PNG images").done();
    dlg.set_current_file("uv_template.png");
    dlg.connect(
        "file_selected",
        &Callable::from_object_method(&canvas, "export_template"),
    );
    let mut export = button(
        &mut auto,
        "Export UV Template…",
        "export_options",
        "Save the UV layout of the shown faces as a PNG to paint over",
        false,
    );
    export.connect(
        "pressed",
        &Callable::from_object_method(&dlg, "popup_file_dialog"),
    );

    let mut info = Label::new_alloc();
    info.set_name("Info");
    info.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7005));
    let mut help = Label::new_alloc();
    help.set_text(
        "Drag: move   Ctrl+drag: scale (Shift: per axis)   Alt+drag: rotate   Middle: pan   Wheel: zoom   Double-click: island   Esc: cancel\nCtrl+click a face: stitch it to the first selected face   Ctrl+Shift+click: copy that face's layout onto it",
    );
    help.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.5));
    help.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
    canvas.bind_mut().facts = Some(info.clone());

    col.add_child(&bar);
    col.add_child(&auto);
    col.add_child(&canvas);
    col.add_child(&info);
    col.add_child(&help);
    panel.add_child(&col);
    // The template dialog travels with the panel (a Window child is not laid out by the container).
    panel.add_child(&dlg);
    (panel, canvas)
}
