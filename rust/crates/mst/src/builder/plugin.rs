//! `PixelBuilderPlugin` — level-builder editor plugin (auto-registered). Active when a
//! `PixelBuilderMesh` (edit it) or a `PixelObject` (stamp new meshes into it) is selected.
//!
//! Every geometry edit snapshots the mesh blob before and after and records one undo action
//! (`set_blob`), so undo/redo never depends on the plugin's selection state.

use std::collections::BTreeSet;

use godot::classes::base_material_3d::{CullMode, Flags, ShadingMode, Transparency};
use godot::classes::editor_plugin::{AfterGuiInput, CustomControlContainer};
use godot::classes::mesh::PrimitiveType;
use godot::classes::{
    Camera3D, Control, EditorInterface, EditorPlugin, IEditorPlugin, ImmediateMesh, Input, InputEvent, InputEventKey, InputEventMouseButton,
    InputEventMouseMotion, MeshInstance3D, Node3D, Object, ResourceSaver, StandardMaterial3D,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::{BMesh, Edge, ShapeKind, ShapeSpec, ops, pick, shapes, snap};
use pt_core::math::{Vec3 as PVec3, v2};

use super::data::{PixelBuilderMeshData, from_p, to_p};
use super::node::PixelBuilderMesh;
use super::ui::{self, Action, Field, Mode, Tool, Ui, UvField, Values};

mod extra;
mod objects;
mod tools;
mod vpaint;
pub use extra::mesh_to_bmesh;
use extra::{PaintStroke, ShapeDrag};
use pt_core::build::xform::Weights;
use tools::{Handle, Knife, PendingBool, RotDrag, ScaleDrag};
use super::prefs::{self, Prefs};
use super::ui::{SyncState, XformMode};
use super::uv_editor::{self, PixelBuilderUvCanvas};
use crate::terrain::MarchingSquaresTerrain;
use crate::util::*;

const PICK_PX: f32 = 10.0;
const ORANGE: Color = Color::from_rgba(1.0, 0.6, 0.15, 1.0);

struct Drag {
    /// 0..2 world axis, None = camera plane.
    axis: Option<usize>,
    before: PackedByteArray,
    start: BMesh,
    pivot: Vector3,
    start_t: f32,
    start_hit: Vector3,
    name: &'static str,
    last_delta: Vector3,
    /// Grabbed handle (move / rotate / scale).
    handle: Option<Handle>,
    /// Per-vertex weights (soft selection; 1 for the selection).
    weights: Weights,
    rot: Option<RotDrag>,
    scale: Option<ScaleDrag>,
    /// Vertex snap: moved vertex nearest the cursor at the start, candidate targets (world).
    snap_anchor: Vector3,
    snap_targets: Vec<Vector3>,
    snap_hit: Option<Vector3>,
    cursor: Vector2,
    /// UV lock: start state of the dragged faces' UVs (refitted on every motion).
    uv_frames: Option<pt_core::build::uv_lock::Frames>,
    /// Handle axes (world) fixed for the whole drag (Global / Local / Normal orientation).
    axes: [Vector3; 3],
}

impl Drag {
    /// UV lock: refits the dragged faces' UVs of `m` (a moved copy of `start`).
    pub(super) fn lock_uvs(&self, m: &mut BMesh) {
        if let Some(f) = &self.uv_frames {
            pt_core::build::uv_lock::refit_drag_uvs(m, f);
        }
    }
}

struct Marquee {
    from: Vector2,
    to: Vector2,
    additive: bool,
}

/// Where a click lands: world point, terrain cell (if on terrain) and the hit surface normal.
#[derive(Clone, Copy)]
struct Placement {
    point: Vector3,
    cell: Option<Vector2i>,
    /// Drawing frame (y = up of the new shape): identity, or the frame of the builder face under
    /// the cursor with *Draw on surface* (Unibuilder `_apply_surface_plane`).
    basis: Basis,
}

/// Frame whose y axis is `n` (shapes drawn on a face stand along its normal).
pub(super) fn frame_from_normal(n: Vector3) -> Basis {
    let n = n.normalized();
    if n.dot(Vector3::UP) > 0.9999 {
        return Basis::IDENTITY;
    }
    let r = if n.x.abs() < 0.9 { Vector3::RIGHT } else { Vector3::BACK };
    let t = (r - n * r.dot(n)).normalized();
    Basis::from_cols(t, n, t.cross(n))
}

#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelBuilderPlugin {
    target: Option<Gd<PixelBuilderMesh>>,
    /// Non-builder node selected (a PixelObject): new shapes become its children.
    host: Option<Gd<Node3D>>,
    terrain: Option<Gd<MarchingSquaresTerrain>>,
    mode: Mode,
    tool: Tool,
    sel_faces: BTreeSet<usize>,
    sel_edges: BTreeSet<Edge>,
    sel_verts: BTreeSet<u32>,
    hover_face: Option<usize>,
    hover_place: Option<Placement>,
    drag: Option<Drag>,
    marquee: Option<Marquee>,
    #[init(val = ShapeSpec::default())]
    spec: ShapeSpec,
    slot: i64,
    #[init(val = Color::WHITE)]
    color: Color,
    #[init(val = 0.25)]
    grid_step: f32,
    #[init(val = true)]
    snap_grid: bool,
    #[init(val = true)]
    snap_cell: bool,
    #[init(val = 1.0)]
    distance: f32,
    #[init(val = 0.2)]
    inset: f32,
    #[init(val = 0.1)]
    bevel: f32,
    #[init(val = 1)]
    cuts: i64,
    #[init(val = 1)]
    bevel_segs: i64,
    /// Select-tool handle mode.
    xform_mode: XformMode,
    hover_handle: Option<Handle>,
    /// Soft (proportional) selection.
    soft: bool,
    #[init(val = 1.0)]
    soft_radius: f32,
    falloff: i64,
    #[init(val = 15.0)]
    rot_snap: f32,
    /// Moves always snap onto vertices (V holds it temporarily).
    snap_vertex: bool,
    /// UV lock while dragging elements (Unibuilder's drag UV refit).
    #[init(val = true)]
    uv_lock: bool,
    /// Vertex paint settings and the live stroke.
    #[init(val = vpaint::VPaint::default())]
    vp: vpaint::VPaint,
    mirror_axis: i64,
    knife: Knife,
    /// Shape parameters re-edit: strip shape fields rebuild the selected mesh.
    reshape: bool,
    pending_bool: Option<PendingBool>,
    /// Face UV panel: offset u, v, scale u, v, rotation (degrees).
    #[init(val = [0.0, 0.0, 1.0, 1.0, 0.0])]
    uv: [f32; 5],
    #[init(val = true)]
    uv_world: bool,
    shape_drag: Option<ShapeDrag>,
    /// Wall tool: clicked polyline points (world) and the snapped cursor.
    wall_pts: Vec<Vector3>,
    wall_cursor: Option<Vector3>,
    paint: Option<PaintStroke>,
    /// Plain MeshInstance3D selected: offered for conversion.
    convert_src: Option<Gd<MeshInstance3D>>,
    export_dialog: Option<Gd<godot::classes::EditorFileDialog>>,
    /// Material slot count shown by the palette (rebuilt when it changes).
    palette_slots: Option<Vec<i64>>,
    message: String,
    ui: Option<Ui>,
    gizmo: Option<Gd<MeshInstance3D>>,
    camera: Option<Gd<Camera3D>>,
    visible: bool,
    #[init(val = Prefs::default())]
    prefs: Prefs,
    prefs_panel: Option<Gd<godot::classes::PopupPanel>>,
    prefs_controls: Vec<(prefs::Key, Gd<Control>)>,
    uv_window: Option<Gd<godot::classes::Window>>,
    uv_canvas: Option<Gd<PixelBuilderUvCanvas>>,
    /// Node carrying our `_edit_lock_` (hides Godot's transform gizmo in element modes).
    lock_target: Option<Gd<PixelBuilderMesh>>,
    /// Material drag-and-drop onto faces (drop catchers over the viewports, drag state).
    drop: objects::MaterialDrag,
    /// W10 settings (inset mode, subdivide segments, handle axes, trim walls).
    #[init(val = ui::ExtraOpts::default())]
    opts: ui::ExtraOpts,
    /// Paint color swatches (Unibuilder paint panel).
    #[init(val = ui::SWATCHES)]
    swatches: [Color; 8],
    /// Insert-vertex modal (Edge mode): on, and the edge / point under the cursor.
    insert_vert: bool,
    insert_hover: Option<(Edge, Vector3)>,
    /// Draw shapes on the builder face under the cursor.
    surface_draw: bool,
    /// Unibuilder X-ray: picks and box selections include elements hidden behind faces.
    xray: bool,
    /// Frame of the wall / poly shape being clicked.
    #[init(val = Basis::IDENTITY)]
    wall_basis: Basis,
    /// "Builder" menu in the 3D toolbar, and Godot's MeshInstance3D "Mesh" menu hidden meanwhile.
    builder_menu: Option<Gd<godot::classes::MenuButton>>,
    hidden_mesh_menu: Option<Gd<Control>>,
    base: Base<EditorPlugin>,
}

/// Meta set next to `_edit_lock_` so only our own lock is ever removed (Unibuilder's
/// `PLUGIN_LOCK_META`).
pub(super) const PLUGIN_LOCK_META: &str = "_pt_builder_lock";
const EDIT_LOCK_META: &str = "_edit_lock_";

#[godot_api]
impl IEditorPlugin for PixelBuilderPlugin {
    fn enter_tree(&mut self) {
        self.prefs = Prefs::load();
        self.apply_prefs(true);
        let gd = self.to_gd().upcast::<Object>();
        let mut u = ui::build(&gd, &self.values());
        let (tb, strip, pal) = u.controls();
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, &tb);
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &strip);
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT, &pal);
        u.set_visible(false, false);
        self.ui = Some(u);
        self.sync_ui();
    }

    fn exit_tree(&mut self) {
        self.remove_drop_targets();
        self.remove_menus();
        self.drop_gizmo();
        self.set_gizmo_lock(None);
        for w in [self.uv_window.take().map(|w| w.upcast::<godot::classes::Node>()), self.prefs_panel.take().map(|w| w.upcast::<godot::classes::Node>())]
            .into_iter()
            .flatten()
        {
            if w.is_instance_valid() {
                w.free();
            }
        }
        self.uv_canvas = None;
        if let Some(u) = self.ui.take() {
            let (tb, strip, pal) = u.controls();
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, &tb);
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &strip);
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT, &pal);
            tb.free();
            strip.free();
            pal.free();
        }
        if let Some(d) = self.export_dialog.take()
            && d.is_instance_valid()
        {
            d.free();
        }
    }

    fn get_plugin_name(&self) -> GString {
        "Pixel Builder".into()
    }

    fn handles(&self, object: Gd<Object>) -> bool {
        // Nodes with their own Pixel World panel (PixelObject, PixelWater, effects; some are
        // or contain MeshInstance3Ds) belong to that plugin: two plugins handling the
        // same node stacked two toolbars + bottom strips and the "Pixel Builder" overlay.
        if crate::editor::pixel_handles(&object) {
            return false;
        }
        object.clone().try_cast::<PixelBuilderMesh>().is_ok() || object.is_class("MeshInstance3D")
    }

    fn edit(&mut self, object: Option<Gd<Object>>) {
        // Re-edit of the mesh already being edited (e.g. the selection nudge after the gizmo
        // lock changed): keep the element selection.
        if let (Some(o), Some(t)) = (&object, &self.target)
            && o.instance_id() == t.instance_id()
        {
            self.sync_ui();
            return;
        }
        self.cancel_drag();
        self.drop_gizmo();
        self.clear_selection();
        self.cancel_shape();
        self.paint = None;
        self.vp.stroke = None;
        self.knife = Knife::default();
        self.reshape = false;
        self.target = None;
        self.host = None;
        self.convert_src = None;
        let Some(o) = object else { return };
        match o.clone().try_cast::<PixelBuilderMesh>() {
            Ok(t) => {
                self.target = Some(t);
                self.unshare_data();
                self.ensure_gizmo();
            }
            Err(o) => {
                if let Ok(mi) = o.clone().try_cast::<MeshInstance3D>() {
                    self.convert_src = Some(mi);
                }
                self.host = o.try_cast::<Node3D>().ok();
                if self.mode != Mode::Object {
                    self.mode = Mode::Object;
                }
                if matches!(self.tool, Tool::Paint | Tool::Place | Tool::Knife | Tool::VertexPaint) {
                    self.tool = Tool::Select;
                }
            }
        }
        self.palette_slots = None;
        self.terrain = find_terrain();
        self.sync_ui();
        self.redraw();
    }

    fn make_visible(&mut self, visible: bool) {
        self.visible = visible;
        let has_target = self.target.is_some();
        if let Some(u) = &mut self.ui {
            u.set_visible(visible, has_target);
        }
        if let Some(g) = &mut self.gizmo {
            g.set_visible(visible);
        }
        self.sync_menus(visible);
        if !visible {
            self.cancel_drag();
            self.cancel_shape();
        }
        self.sync_gizmo_lock();
    }

    fn forward_3d_gui_input(&mut self, camera: Option<Gd<Camera3D>>, event: Option<Gd<InputEvent>>) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let stop = AfterGuiInput::STOP.ord();
        let (Some(camera), Some(event)) = (camera, event) else { return pass };
        if !self.visible || (self.target.is_none() && self.host.is_none()) {
            return pass;
        }
        self.camera = Some(camera.clone());
        let editing = self.target.is_some() && self.mode != Mode::Object && self.tool == Tool::Select;
        let placing = self.tool != Tool::Select;

        if self.tool == Tool::Knife
            && self.target.is_some()
            && let Some(r) = self.knife_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Shape
            && self.spec.kind.is_polyline()
            && let Some(r) = self.wall_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.insert_vert
            && self.mode == Mode::Edge
            && self.target.is_some()
            && self.tool == Tool::Select
            && let Some(r) = self.insert_vert_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            return if self.on_key(&key) { stop } else { pass };
        }
        if self.tool == Tool::Shape
            && let Some(r) = self.shape_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Paint
            && let Some(r) = self.paint_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::VertexPaint
            && let Some(r) = self.vpaint_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            if self.drag.is_some() {
                self.drag_motion(&camera, pos);
                return stop;
            }
            if let Some(m) = &mut self.marquee {
                m.to = pos;
                self.base().update_overlays();
                return stop;
            }
            if editing {
                let h = self.gizmo_hit(&camera, pos);
                if h != self.hover_handle {
                    self.hover_handle = h;
                    self.base().update_overlays();
                }
            }
            self.update_hover(&camera, pos);
            return pass;
        }
        if let Ok(mb) = event.try_cast::<InputEventMouseButton>() {
            let button = mb.get_button_index();
            let pos = mb.get_position();
            let shift = Input::singleton().is_key_pressed(Key::SHIFT);
            if button == MouseButton::RIGHT && mb.is_pressed() && (self.drag.is_some() || self.marquee.is_some()) {
                self.cancel_drag();
                self.marquee = None;
                self.redraw();
                return stop;
            }
            if button != MouseButton::LEFT {
                return pass;
            }
            if placing {
                if mb.is_pressed() {
                    self.update_hover(&camera, pos);
                    match self.tool {
                        Tool::Shape => self.stamp_shape(shift),
                        Tool::Place => self.place_target(),
                        Tool::Select | Tool::Paint | Tool::Knife | Tool::VertexPaint => {}
                    }
                }
                return stop;
            }
            if !editing {
                return pass;
            }
            if mb.is_pressed() {
                if mb.is_double_click() && self.drag.is_none() && self.double_click(&camera, pos, shift, mb.is_alt_pressed()) {
                    self.marquee = None;
                } else if let Some(h) = self.gizmo_hit(&camera, pos) {
                    self.begin_drag(&camera, pos, h, shift);
                } else if !self.pick(&camera, pos, shift) {
                    self.marquee = Some(Marquee { from: pos, to: pos, additive: shift });
                } else {
                    self.load_face_props();
                }
            } else if self.drag.is_some() {
                self.finish_drag();
            } else if let Some(m) = self.marquee.take() {
                self.marquee_select(&camera, m);
            }
            self.redraw();
            return stop;
        }
        pass
    }

    fn forward_3d_draw_over_viewport(&mut self, overlay: Option<Gd<Control>>) {
        let Some(mut overlay) = overlay else { return };
        self.ensure_drop_target(&overlay);
        // The overlay call has no camera: use the one from the last input event, else viewport 0's.
        let cam = self.camera.clone().filter(|c| c.is_instance_valid()).or_else(|| {
            EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done().and_then(|v| v.get_camera_3d())
        });
        let Some(cam) = cam else { return };
        let proj = |w: Vector3| (!cam.is_position_behind(w)).then(|| cam.unproject_position(w));
        // Vertices (vertex mode), selected ones highlighted.
        if let (Some(t), Mode::Vertex) = (&self.target, self.mode)
            && let Some(d) = t.bind().data()
        {
            let xf = t.get_global_transform();
            let d = d.bind();
            for (i, p) in d.mesh().positions.iter().enumerate() {
                if let Some(s) = proj(xf * from_p(*p)) {
                    let sel = self.sel_verts.contains(&(i as u32));
                    let r = self.prefs.vertex_size * 0.5 + if sel { 1.0 } else { 0.0 };
                    overlay.draw_rect(Rect2::new(s - Vector2::new(r, r), Vector2::new(r * 2.0, r * 2.0)), if sel { self.prefs.color_vertex_selected } else { Color::from_rgba(0.1, 0.1, 0.1, 0.9) });
                    if !sel {
                        let k = (r - 1.0).max(1.0);
                        overlay.draw_rect(Rect2::new(s - Vector2::new(k, k), Vector2::new(k * 2.0, k * 2.0)), self.prefs.color_vertex);
                    }
                }
            }
        }
        // Move / rotate / scale handles, the knife and the vertex paint brush.
        self.draw_gizmo(&mut overlay, &cam);
        self.draw_insert_hover(&mut overlay, &cam);
        self.draw_knife(&mut overlay, &cam);
        self.draw_vpaint(&mut overlay, &cam);
        if self.soft
            && self.drag.is_some()
            && let Some(p) = self.drag.as_ref().map(|d| d.pivot)
            && !cam.is_position_behind(p)
        {
            // Soft selection radius around the pivot.
            let s = cam.unproject_position(p);
            let r = cam.unproject_position(p + cam.get_global_transform().basis.col_a() * self.soft_radius).distance_to(s);
            overlay.draw_arc_ex(s, r, 0.0, std::f32::consts::TAU, 64, Color::from_rgba(1.0, 0.75, 0.3, 0.6)).width(1.0).done();
        }
        // Placement preview: the shape's edges at the hover point.
        let preview = if !self.wall_pts.is_empty() { self.wall_preview() } else if self.shape_drag.is_some() { self.dragged_shape() } else { self.hover_place.filter(|_| self.tool == Tool::Shape).map(|p| (p.point, self.shape_mesh(p.cell))) };
        let frame = self.draw_basis();
        if self.tool == Tool::Shape
            && let Some((origin, m)) = preview
        {
            let col = if self.shape_drag.is_some() || !self.wall_pts.is_empty() { Color::from_rgba(1.0, 0.75, 0.3, 0.95) } else { Color::from_rgba(0.5, 0.9, 1.0, 0.85) };
            for e in m.edges() {
                let (a, b) = (origin + frame * from_p(m.positions[e.0 as usize]), origin + frame * from_p(m.positions[e.1 as usize]));
                if let (Some(sa), Some(sb)) = (proj(a), proj(b)) {
                    overlay.draw_line_ex(sa, sb, col).width(1.5).done();
                }
            }
        }
        if self.tool == Tool::Place
            && let (Some(p), Some(t)) = (self.hover_place, &self.terrain)
            && let Some(c) = p.cell
        {
            let tb = t.bind();
            let cfg = &tb.store().config;
            let xf = t.get_global_transform();
            let ly = (xf.affine_inverse() * p.point).y;
            let pts: Vec<Vector2> = cfg.cell_corners(to_iv(c)).iter().filter_map(|k| proj(xf * Vector3::new(k.x, ly, k.y))).collect();
            for i in 0..pts.len() {
                overlay.draw_line_ex(pts[i], pts[(i + 1) % pts.len()], ORANGE).width(2.0).done();
            }
        }
        if let Some(m) = &self.marquee {
            let r = Rect2::new(m.from, Vector2::ZERO).expand(m.to);
            overlay.draw_rect(r, Color::from_rgba(0.4, 0.7, 1.0, 0.15));
            overlay.draw_rect_ex(r, Color::from_rgba(0.4, 0.7, 1.0, 0.9)).filled(false).done();
        }
        // Status text.
        let Some(font) = overlay.get_theme_default_font() else { return };
        let mut lines = vec![format!("Pixel Builder - {} mode, {}", self.mode.name(), self.tool.name())];
        if let Some(t) = &self.target
            && let Some(d) = t.bind().data()
        {
            let d = d.bind();
            lines.push(format!(
                "{} verts  {} faces  |  selected: {} faces {} edges {} verts",
                d.mesh().positions.len(),
                d.mesh().faces.len(),
                self.sel_faces.len(),
                self.sel_edges.len(),
                self.sel_verts.len()
            ));
        }
        if self.tool == Tool::Shape && self.spec.kind.is_polyline() {
            lines.push(format!(
                "Wall  {} points  height {:.2}  thick {:.2}  (click points, click the first to close, Enter / double-click finishes)",
                self.wall_pts.len(),
                self.spec.size.y,
                self.spec.thickness
            ));
        } else if self.tool == Tool::Shape {
            match self.dragged_shape().and_then(|(_, m)| m.bounds()) {
                Some((lo, hi)) => lines.push(format!(
                    "{}  {:.2} x {:.2} x {:.2}  {}",
                    self.spec.kind.name(),
                    hi.x - lo.x,
                    hi.y - lo.y,
                    hi.z - lo.z,
                    if self.shape_drag.as_ref().is_some_and(|d| d.stage == 1) { "move for height, click to finish" } else { "drag the footprint" }
                )),
                None => {
                    let s = &self.spec;
                    lines.push(format!("{}  {:.2} x {:.2} x {:.2}  slot {}  (drag to draw, click to stamp)", s.kind.name(), s.size.x, s.size.y, s.size.z, self.slot));
                }
            }
        }
        if let Some(d) = &self.drag
            && d.rot.is_none()
            && d.scale.is_none()
        {
            let v = d.last_delta;
            lines.push(format!("move ({:.2}, {:.2}, {:.2})", v.x, v.y, v.z));
        }
        if !self.message.is_empty() {
            lines.push(self.message.clone());
        }
        for (i, l) in lines.iter().enumerate() {
            let pos = Vector2::new(14.0, 64.0 + 18.0 * i as f32);
            overlay.draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), l.as_str()).modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8)).done();
            overlay.draw_string_ex(&font, pos, l.as_str()).modulate(Color::from_rgb(0.85, 0.95, 1.0)).done();
        }
    }

    fn save_external_data(&mut self) {
        // The gizmo lock never reaches the scene file: `PixelBuilderMesh` strips it on
        // EDITOR_PRE_SAVE and restores it on EDITOR_POST_SAVE (Unibuilder's way).
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else { return };
        for n in root.find_children_ex("*").type_("PixelBuilderMesh").owned(false).done().iter_shared() {
            let Ok(res) = n.get("data").try_to::<Gd<godot::classes::Resource>>() else { continue };
            let path = res.get_path().to_string();
            if path.is_empty() || path.contains("::") {
                continue;
            }
            if ResourceSaver::singleton().save(&res) != godot::global::Error::OK {
                godot_error!("Pixel Builder: could not save {path}");
            }
        }
    }
}

/// First `MarchingSquaresTerrain` in the edited scene.
fn find_terrain() -> Option<Gd<MarchingSquaresTerrain>> {
    let root = EditorInterface::singleton().get_edited_scene_root()?;
    if let Ok(t) = root.clone().try_cast::<MarchingSquaresTerrain>() {
        return Some(t);
    }
    root.find_children_ex("*").type_("MarchingSquaresTerrain").owned(false).done().iter_shared().find_map(|n| n.try_cast().ok())
}

fn closest_on_axis(p: Vector3, a: Vector3, o: Vector3, d: Vector3) -> Option<f32> {
    // Parameter t on line p + a t closest to the ray o + d s.
    let b = a.dot(d);
    let w = p - o;
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-4 {
        return None;
    }
    Some((b * d.dot(w) - a.dot(w)) / denom)
}

impl PixelBuilderPlugin {
    fn values(&self) -> Values<'_> {
        self.values_with(&self.spec)
    }

    fn values_with<'a>(&self, spec: &'a ShapeSpec) -> Values<'a> {
        Values {
            spec,
            slot: self.slot,
            grid_step: self.grid_step,
            distance: self.distance,
            inset: self.inset,
            bevel: self.bevel,
            cuts: self.cuts,
            color: self.color,
            uv: self.uv,
            uv_world: self.uv_world,
            bevel_segs: self.bevel_segs,
            soft_radius: self.soft_radius,
            falloff: self.falloff,
            rot_snap: self.rot_snap,
            mirror_axis: self.mirror_axis,
            vp_fill: self.vp.fill,
            vp_channel: self.vp.channel,
            vp_radius: self.vp.radius,
            vp_strength: self.vp.strength,
            vp_softness: self.vp.softness,
            vp_alpha: self.vp.alpha,
            opts: self.opts,
            swatches: self.swatches,
        }
    }

    fn sync_ui(&mut self) {
        self.refresh_palette();
        let has_target = self.target.is_some();
        let st = SyncState {
            mode: self.mode,
            tool: self.tool,
            xform: self.xform_mode,
            snap_grid: self.snap_grid,
            snap_cell: self.snap_cell,
            snap_vertex: self.snap_vertex,
            soft: self.soft,
            uv_lock: self.uv_lock,
            reshape: self.reshape && has_target,
            has_target,
            has_spec: self.data().is_some_and(|d| d.bind().shape_spec().is_some()),
            can_convert: self.convert_src.is_some(),
            insert_vert: self.insert_vert,
            surface_draw: self.surface_draw,
            xray: self.xray,
            entity: self.target.as_ref().map(|t| t.get("entity_type").try_to::<i32>().unwrap_or(0)).unwrap_or(0),
        };
        let spec = self.spec;
        let v = self.values_with(&spec);
        let visible = self.visible;
        if let Some(u) = &mut self.ui {
            u.sync(&v, &st);
            u.palette.set_visible(visible && has_target);
            let what = if self.target.is_some() {
                "Editing builder mesh"
            } else if self.convert_src.is_some() {
                "Mesh selected: Convert makes it an editable builder mesh; the shape tool draws new ones"
            } else if self.host.is_some() {
                "Draw shape tool: drag on the terrain to add builder meshes here"
            } else {
                ""
            };
            u.info.set_text(what);
        }
        self.sync_gizmo_lock();
        self.sync_uv_editor();
        self.base().update_overlays();
    }

    fn data(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.target.as_ref().and_then(|t| t.bind().data())
    }

    fn clear_selection(&mut self) {
        self.sel_faces.clear();
        self.sel_edges.clear();
        self.sel_verts.clear();
        self.hover_face = None;
    }

    /// Godot's "Duplicate" shares the data resource; give this node its own copy before editing.
    fn unshare_data(&mut self) {
        let (Some(t), Some(d)) = (self.target.clone(), self.data()) else { return };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else { return };
        let id = d.instance_id();
        let shared = root
            .find_children_ex("*")
            .type_("PixelBuilderMesh")
            .owned(false)
            .done()
            .iter_shared()
            .filter(|n| n.instance_id() != t.instance_id())
            .any(|n| n.get("data").try_to::<Gd<Object>>().is_ok_and(|o| o.instance_id() == id));
        if shared && let Some(copy) = d.duplicate_resource().try_cast::<PixelBuilderMeshData>().ok() {
            let mut t = t;
            t.bind_mut().set_data(Some(copy));
        }
    }

    fn xform(&self) -> Transform3D {
        self.target.as_ref().map(|t| t.get_global_transform()).unwrap_or(Transform3D::IDENTITY)
    }

    /// Selected vertices of the current mode (local indices).
    fn selected_verts(&self, m: &BMesh) -> BTreeSet<u32> {
        match self.mode {
            Mode::Face => m.verts_of_faces(&self.sel_faces),
            Mode::Edge => m.verts_of_edges(&self.sel_edges),
            Mode::Vertex => self.sel_verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect(),
            Mode::Object => BTreeSet::new(),
        }
    }

    fn pivot(&self) -> Option<Vector3> {
        let d = self.data()?;
        let d = d.bind();
        let m = d.mesh();
        let v = self.selected_verts(m);
        if v.is_empty() {
            return None;
        }
        let c = v.iter().fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize]) / v.len() as f32;
        Some(self.xform() * from_p(c))
    }

    /// Handle centre and the three axis tips on screen.
    fn handle_screen(&self, cam: &Gd<Camera3D>) -> Option<(Vector2, [Vector2; 3])> {
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return None;
        }
        let p = self.drag.as_ref().map(|d| d.pivot + d.last_delta).or_else(|| self.pivot())?;
        if cam.is_position_behind(p) {
            return None;
        }
        let k = if cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL {
            cam.get_size() * 0.1
        } else {
            cam.get_global_position().distance_to(p) * 0.12
        };
        let s0 = cam.unproject_position(p);
        let axes = self.gizmo_axes();
        let ends = axes.map(|a| cam.unproject_position(p + a * k));
        Some((s0, ends))
    }

    /// `Some(None)` = centre (camera plane), `Some(Some(axis))`.
    fn handle_at(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Option<usize>> {
        let (s0, ends) = self.handle_screen(cam)?;
        if (pos - s0).length() < 8.0 {
            return Some(None);
        }
        let mut best: Option<(f32, usize)> = None;
        for (a, e) in ends.iter().enumerate() {
            let ab = *e - s0;
            let t = if ab.length_squared() < 1.0 { 0.0 } else { ((pos - s0).dot(ab) / ab.length_squared()).clamp(0.0, 1.15) };
            let dist = (s0 + ab * t - pos).length();
            if dist < 7.0 && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, a));
            }
        }
        best.map(|b| Some(b.1))
    }

    // ------------------------------------------------------------------ picking

    fn local_ray(&self, cam: &Gd<Camera3D>, pos: Vector2) -> (PVec3, PVec3) {
        let inv = self.xform().affine_inverse();
        let o = inv * cam.project_ray_origin(pos);
        let d = (inv.basis * cam.project_ray_normal(pos)).normalized();
        (to_p(o), to_p(d))
    }

    fn pick(&mut self, cam: &Gd<Camera3D>, pos: Vector2, additive: bool) -> bool {
        let Some(d) = self.data() else { return false };
        let xf = self.xform();
        let (o, dir) = self.local_ray(cam, pos);
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| (to_v2(cam.unproject_position(w)), cam.get_global_position().distance_to(w)))
        };
        let mouse = to_v2(pos);
        let db = d.bind();
        let m = db.mesh();
        let hit = pick::ray_mesh(m, o, dir, true);
        fn toggle<T: Ord + Copy>(set: &mut BTreeSet<T>, v: T, additive: bool) {
            if !additive {
                set.clear();
                set.insert(v);
            } else if !set.remove(&v) {
                set.insert(v);
            }
        }
        // Only elements that are not hidden behind the nearest face hit (X-ray: any).
        let xray = self.xray;
        let front = |p: PVec3| xray || hit.is_none_or(|h| (p - o).length() <= h.t + 0.02);
        let found = match self.mode {
            Mode::Face => hit.map(|h| toggle(&mut self.sel_faces, h.face, additive)).is_some(),
            Mode::Edge => pick::nearest_edge(m, &project, mouse, PICK_PX)
                .filter(|e| front((m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5))
                .map(|e| toggle(&mut self.sel_edges, e, additive))
                .is_some(),
            Mode::Vertex => pick::nearest_vertex(m, &project, mouse, PICK_PX)
                .filter(|v| front(m.positions[*v as usize]))
                .map(|v| toggle(&mut self.sel_verts, v, additive))
                .is_some(),
            Mode::Object => false,
        };
        drop(db);
        if !found && !additive {
            self.clear_selection();
        }
        found
    }

    fn marquee_select(&mut self, cam: &Gd<Camera3D>, mq: Marquee) {
        let Some(d) = self.data() else { return };
        let xf = self.xform();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| (to_v2(cam.unproject_position(w)), 0.0))
        };
        let lo = v2(mq.from.x.min(mq.to.x), mq.from.y.min(mq.to.y));
        let hi = v2(mq.from.x.max(mq.to.x), mq.from.y.max(mq.to.y));
        if (hi - lo).length() < 3.0 {
            return;
        }
        let db = d.bind();
        let m = db.mesh();
        let verts = pick::verts_in_rect(m, &project, lo, hi);
        if !mq.additive {
            self.clear_selection();
        }
        // Unibuilder marquee_*: without X-ray, elements behind other faces are skipped (the
        // vertex, the edge midpoint, the face centre; faces turned away too).
        let eye = to_p(xf.affine_inverse() * cam.get_global_position());
        let ortho = cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL;
        let view_dir = to_p((xf.affine_inverse().basis * -cam.get_global_transform().basis.col_c()).normalized());
        let seen = |p: PVec3| {
            if self.xray {
                return true;
            }
            // Orthographic: look from far back along the view direction.
            let from = if ortho { p - view_dir * 1.0e4 } else { eye };
            !pick::point_occluded(m, from, p, true)
        };
        match self.mode {
            Mode::Face => {
                let faces: Vec<usize> = m.faces_within(&verts).into_iter().collect();
                for f in faces {
                    let c = m.face_center(f);
                    let to_eye = if ortho { -view_dir } else { eye - c };
                    if self.xray || (m.face_normal(f).dot(to_eye) > 0.0 && seen(c)) {
                        self.sel_faces.insert(f);
                    }
                }
            }
            Mode::Edge => {
                let edges: Vec<_> = m.edges_within(&verts).into_iter().collect();
                for e in edges {
                    if seen((m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5) {
                        self.sel_edges.insert(e);
                    }
                }
            }
            Mode::Vertex => {
                for v in verts {
                    if seen(m.positions[v as usize]) {
                        self.sel_verts.insert(v);
                    }
                }
            }
            Mode::Object => {}
        }
    }

    fn update_hover(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.tool == Tool::Select {
            if self.target.is_some() && self.mode == Mode::Face {
                let (o, d) = self.local_ray(cam, pos);
                let hf = self.data().and_then(|data| pick::ray_mesh(data.bind().mesh(), o, d, true).map(|h| h.face));
                if hf != self.hover_face {
                    self.hover_face = hf;
                    self.redraw();
                }
            }
            return;
        }
        self.hover_place = self.placement(cam, pos);
        self.base().update_overlays();
    }

    /// Placement point under the mouse: nearest of builder meshes and terrain, else the y=0 plane.
    fn placement(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Placement> {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let mut best: Option<(f32, Vector3, bool)> = None;
        let mut face_normal: Option<Vector3> = None;
        if let Some(t) = &self.terrain {
            let xf = t.get_global_transform();
            let inv = xf.affine_inverse();
            if let Some(h) = t.bind().local_raycast(inv * o, (inv.basis * d).normalized(), 20_000.0) {
                let w = xf * h;
                best = Some((o.distance_to(w), w, true));
            }
        }
        // Builder meshes (stacking), excluding the one being placed by the Place tool.
        if let Some(root) = EditorInterface::singleton().get_edited_scene_root() {
            for n in root.find_children_ex("*").type_("PixelBuilderMesh").owned(false).done().iter_shared() {
                let Ok(b) = n.try_cast::<PixelBuilderMesh>() else { continue };
                if self.tool == Tool::Place && self.target.as_ref().is_some_and(|t| t.instance_id() == b.instance_id()) {
                    continue;
                }
                let Some(data) = b.bind().data() else { continue };
                let xf = b.get_global_transform();
                let inv = xf.affine_inverse();
                if let Some(h) = pick::ray_mesh(data.bind().mesh(), to_p(inv * o), to_p((inv.basis * d).normalized()), true) {
                    let w = xf * from_p(h.point);
                    let dist = o.distance_to(w);
                    if best.is_none_or(|bb| dist < bb.0) {
                        best = Some((dist, w, false));
                        face_normal = Some((xf.basis.inverse().transposed() * from_p(data.bind().mesh().face_normal(h.face))).normalized());
                    }
                }
            }
        }
        if self.surface_draw
            && let (Some((_, w, false)), Some(n)) = (best, face_normal)
        {
            // Draw on surface: start exactly on the face, standing along its normal. Grid snap
            // snaps the start along the face plane's own axes (world-anchored lattice).
            let basis = frame_from_normal(n);
            let mut point = w;
            if self.snap_grid {
                let mut l = basis.inverse() * w;
                l.x = snap::snap_value(l.x, self.grid_step);
                l.z = snap::snap_value(l.z, self.grid_step);
                point = basis * l;
            }
            return Some(Placement { point, cell: None, basis });
        }
        let (mut point, on_terrain) = match best {
            Some((_, w, t)) => (w, t),
            None => {
                if d.y.abs() < 1e-5 {
                    return None;
                }
                let k = -o.y / d.y;
                if k <= 0.0 {
                    return None;
                }
                (o + d * k, false)
            }
        };
        let mut cell = None;
        if self.snap_cell
            && let Some(t) = &self.terrain
        {
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * point;
            let tb = t.bind();
            let (c, ctr) = snap::cell_center(&tb.store().config, v2(l.x, l.z));
            drop(tb);
            cell = Some(from_iv(c));
            if on_terrain && let Some(p) = super::node::cell_world(t, from_iv(c)) {
                point = p;
            } else {
                point = xf * Vector3::new(ctr.x, l.y, ctr.y);
            }
        } else if self.snap_grid {
            point.x = snap::snap_value(point.x, self.grid_step);
            point.z = snap::snap_value(point.z, self.grid_step);
            if !on_terrain && best.is_none() {
                point.y = snap::snap_value(point.y, self.grid_step);
            }
        }
        Some(Placement { point, cell, basis: Basis::IDENTITY })
    }

    // ------------------------------------------------------------------ shapes

    /// The current shape as a mesh around its placement origin. Grid shapes follow the terrain
    /// grid (and the actual cell, which matters for triangle / offset layouts).
    fn shape_mesh(&self, cell: Option<Vector2i>) -> BMesh {
        let tcfg = self.terrain.as_ref().map(|t| t.bind().store().config.clone()).filter(|c| c.grid.is_cell_mode());
        match (self.spec.kind.is_grid_shape(), &tcfg, cell) {
            (true, Some(cfg), Some(c)) => {
                let r = if self.spec.kind == ShapeKind::HexPrism { 0 } else { self.spec.rings.min(16) as i32 };
                let cells = shapes::cluster_cells(cfg, to_iv(c), r);
                shapes::cells_prism(cfg, &cells, to_iv(c), self.spec.size.y)
            }
            _ => shapes::generate(&self.spec, tcfg.as_ref()),
        }
    }

    fn finish_shape(&self, mut m: BMesh) -> BMesh {
        let c = ui_color_bytes(self.color);
        for f in &mut m.faces {
            f.material = self.slot.clamp(0, 255) as u16;
            f.color = c;
        }
        m
    }

    fn stamp_shape(&mut self, append: bool) {
        let Some(p) = self.hover_place else {
            self.message = "Nothing under the cursor".into();
            return;
        };
        let m = self.shape_mesh(p.cell);
        self.create_from_mesh(p, m, append);
    }

    /// Adds a generated mesh at `p`: appended to the selected mesh, or as a new builder node.
    fn create_from_mesh(&mut self, p: Placement, m: BMesh, append: bool) {
        let m = self.finish_shape(m);
        if append && self.target.is_some() {
            let xf = self.xform();
            let inv = xf.affine_inverse();
            let mut local = m.clone();
            for q in &mut local.positions {
                *q = to_p(inv * (p.point + p.basis * from_p(*q)));
            }
            self.apply_edit("Pixel Builder: Add shape", |mm, _| mm.append(&local));
            return;
        }
        let parent: Option<Gd<Node>> = match (&self.host, &self.target) {
            (Some(h), _) => Some(h.clone().upcast()),
            (None, Some(t)) => t.get_parent(),
            _ => EditorInterface::singleton().get_edited_scene_root(),
        };
        let Some(parent) = parent else { return };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else { return };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut data = PixelBuilderMeshData::new_gd();
        {
            let mut db = data.bind_mut();
            db.replace(m);
            db.set_shape_spec(&self.spec);
        }
        // New meshes drawn while another is selected share its material slots.
        if let Some(src) = self.data() {
            let mats = src.bind().material_slots().duplicate_shallow();
            data.set("materials", &mats.to_variant());
        }
        node.bind_mut().set_data(Some(data));
        node.set_name(self.spec.kind.name().replace(' ', "").as_str());
        if let Some(c) = p.cell {
            node.set("anchor_cell", &c.to_variant());
            node.set("anchored", &true.to_variant());
        }
        let parent_xf = parent.clone().try_cast::<Node3D>().map(|n| n.get_global_transform()).unwrap_or(Transform3D::IDENTITY);
        node.set_transform(parent_xf.affine_inverse() * Transform3D::new(p.basis, p.point));
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action(&format!("Pixel Builder: Create {}", self.spec.kind.name()));
        ur.add_do_method(&parent, "add_child", &[node.to_variant(), true.to_variant()]);
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.commit_action();
        self.message = format!("Created {}", self.spec.kind.name());
    }

    fn place_target(&mut self) {
        let (Some(t), Some(terrain), Some(p)) = (self.target.clone(), self.terrain.clone(), self.hover_place) else {
            self.message = "Place needs a selected mesh and a terrain under the cursor".into();
            return;
        };
        let Some(cell) = p.cell.or_else(|| Some(super::node::world_cell(&terrain, p.point))) else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action("Pixel Builder: Place on cell");
        ur.add_do_method(&t, "place_on_cell", &[terrain.to_variant(), cell.to_variant()]);
        ur.add_undo_property(&t, "position", &t.get_position().to_variant());
        ur.add_undo_property(&t, "anchor_cell", &t.get("anchor_cell"));
        ur.add_undo_property(&t, "anchored", &t.get("anchored"));
        ur.commit_action();
        self.redraw();
    }

    // ------------------------------------------------------------------ edits

    /// Applies `f` to a copy of the mesh, stores it and records an undo action.
    fn apply_edit(&mut self, name: &str, f: impl FnOnce(&mut BMesh, &mut Self)) {
        let Some(mut data) = self.data() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let (before, mut m) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone())
        };
        f(&mut m, self);
        if let Err(e) = m.validate() {
            self.message = format!("Operation failed: {e}");
            return;
        }
        data.bind_mut().replace(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            self.commit_blob(name, &data, before, after);
        }
        self.message.clear();
        self.redraw();
    }

    fn commit_blob(&mut self, name: &str, data: &Gd<PixelBuilderMeshData>, before: PackedByteArray, after: PackedByteArray) {
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action(name);
        ur.add_do_method(data, "set_blob", &[after.to_variant()]);
        ur.add_undo_method(data, "set_blob", &[before.to_variant()]);
        ur.commit_action_ex().execute(false).done();
    }

    fn all_or_selected_faces(&self, m: &BMesh) -> BTreeSet<usize> {
        match self.mode {
            Mode::Object => (0..m.faces.len()).collect(),
            Mode::Face => self.sel_faces.clone(),
            _ => m.faces_within(&self.selected_verts(m)),
        }
    }

    fn run_action(&mut self, a: Action) {
        match a {
            Action::ModeObject => self.set_mode(Mode::Object),
            Action::ModeFace => self.set_mode(Mode::Face),
            Action::ModeEdge => self.set_mode(Mode::Edge),
            Action::ModeVertex => self.set_mode(Mode::Vertex),
            Action::ToolSelect => self.set_tool(Tool::Select),
            Action::ToolShape => self.set_tool(Tool::Shape),
            Action::ToolPlace => self.set_tool(Tool::Place),
            Action::ToolPaint => self.set_tool(Tool::Paint),
            Action::ToolVertexPaint => self.set_tool(if self.tool == Tool::VertexPaint { Tool::Select } else { Tool::VertexPaint }),
            Action::UvLock => self.uv_lock = !self.uv_lock,
            Action::BlendMaterial => self.act_blend_material(),
            Action::InvertAlpha => self.apply_edit("Pixel Builder: Invert alpha", |m, s| {
                let f = s.all_or_selected_faces(m);
                pt_core::build::paint::invert_alpha(m, &f);
            }),
            Action::BakeMesh => self.act_bake(),
            Action::Bevel => self.act_bevel_segments(),
            Action::GizmoMove => self.set_xform(XformMode::Move),
            Action::GizmoRotate => self.set_xform(XformMode::Rotate),
            Action::GizmoScale => self.set_xform(XformMode::Scale),
            Action::SoftSelect => self.soft = !self.soft,
            Action::SnapVertex => self.snap_vertex = !self.snap_vertex,
            Action::InsertLoop => self.act_insert_loop(),
            Action::Bridge => self.act_bridge(),
            Action::SelectLoop => self.act_select_loop(Input::singleton().is_key_pressed(Key::ALT)),
            Action::Shrink => self.act_shrink(),
            Action::Holes => self.act_holes(),
            Action::Mirror => self.act_mirror(),
            Action::ToolKnife => {
                if self.mode == Mode::Object {
                    self.set_mode(Mode::Face);
                }
                self.set_tool(Tool::Knife);
            }
            Action::ShapeParams => self.toggle_reshape(),
            Action::BoolUnion => self.act_boolean(godot::classes::csg_shape_3d::Operation::UNION, "Boolean union"),
            Action::BoolSubtract => self.act_boolean(godot::classes::csg_shape_3d::Operation::SUBTRACTION, "Boolean subtract"),
            Action::BoolIntersect => self.act_boolean(godot::classes::csg_shape_3d::Operation::INTERSECTION, "Boolean intersect"),
            Action::Merge => self.act_merge(),
            Action::UvEditor => self.open_uv_editor(),
            Action::Preferences => self.open_preferences(),
            Action::ConformNormals => self.apply_edit("Pixel Builder: Conform normals", |m, s| {
                let n = ops::conform_normals(m);
                s.message = format!("Conform normals: {n} face{} flipped", if n == 1 { "" } else { "s" });
            }),
            Action::Connect => self.act_connect(),
            Action::Collapse => self.act_collapse(),
            Action::InsertVertex => {
                self.insert_vert = !self.insert_vert;
                self.insert_hover = None;
                if self.insert_vert && self.mode != Mode::Edge {
                    self.set_mode(Mode::Edge);
                }
            }
            Action::TrimWalls => self.act_trim_walls(),
            Action::SurfaceDraw => self.surface_draw = !self.surface_draw,
            Action::XRay => self.xray = !self.xray,
            Action::SetCollider => self.act_entity(1),
            Action::SetTrigger => self.act_entity(2),
            Action::Fill => self.act_fill(),
            Action::Detach => self.apply_edit("Pixel Builder: Detach", |m, s| pt_core::build::topo::detach_faces(m, &s.sel_faces)),
            Action::Duplicate => self.apply_edit("Pixel Builder: Duplicate", |m, s| {
                let f = s.sel_faces.clone();
                s.sel_faces = ops::duplicate_faces(m, &f, pt_core::math::Vec3::ZERO);
            }),
            Action::Rotate => {
                let neg = Input::singleton().is_key_pressed(Key::SHIFT);
                self.act_rotate(if neg { -90.0 } else { 90.0 });
            }
            Action::Grow => self.act_select(0),
            Action::Coplanar => self.act_select(1),
            Action::Similar => self.act_select(2),
            Action::ExportObj => self.open_export(false),
            Action::ExportGltf => self.open_export(true),
            Action::Convert => self.convert_selected(),
            Action::SnapGrid => self.snap_grid = !self.snap_grid,
            Action::SnapCell => self.snap_cell = !self.snap_cell,
            Action::Extrude => {
                let dist = self.distance;
                if self.mode == Mode::Edge && !self.sel_edges.is_empty() {
                    self.act_extrude_edges();
                } else if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Extrude: select faces (Face mode)".into();
                } else {
                    self.apply_edit("Pixel Builder: Extrude", |m, s| s.sel_faces = ops::extrude_faces(m, &s.sel_faces, dist));
                }
            }
            Action::Inset => {
                let (amt, how) = (self.inset, self.opts.inset_mode);
                if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Inset: select faces (Face mode)".into();
                } else if how == 0 {
                    self.apply_edit("Pixel Builder: Inset", |m, s| s.sel_faces = ops::inset_faces(m, &s.sel_faces, amt.min(0.99)));
                } else {
                    // Unibuilder inset: a rim `amt` wide; Equal mitres the corners.
                    self.apply_edit("Pixel Builder: Inset", |m, s| s.sel_faces = pt_core::build::ub_ops::inset_faces(m, &s.sel_faces, amt, how == 2));
                }
            }
            Action::Subdivide => self.act_subdivide(),
            Action::SubdivideEdges => self.act_subdivide_edges(),
            Action::ConnectEdges => self.act_connect_edges(),
            Action::FlipEdge => self.act_flip_edge(),
            Action::SplitVerts => self.act_split_verts(),
            Action::Triangulate => self.act_triangulate(),
            Action::MergeObjects => self.act_merge_objects(),
            Action::CenterPivot => self.act_center_pivot(),
            Action::PivotToSelection => self.act_pivot_to_selection(),
            Action::FreezeTransform => self.act_freeze_transform(),
            Action::Flip => self.apply_edit("Pixel Builder: Flip normals", |m, s| {
                let f = s.all_or_selected_faces(m);
                ops::flip_faces(m, &f);
            }),
            Action::Delete => self.delete_selected(),
            Action::Weld => self.apply_edit("Pixel Builder: Weld", |m, s| {
                if s.mode == Mode::Vertex && s.sel_verts.len() > 1 {
                    let v = s.sel_verts.clone();
                    let kept = ops::merge_verts(m, &v);
                    s.clear_selection();
                    s.sel_verts.extend(kept);
                } else {
                    ops::weld_by_distance(m, 1e-3, None);
                    s.clear_selection();
                }
            }),
            Action::Material => {
                let (slot, col) = (self.slot.clamp(0, 255) as u16, ui_color_bytes(self.color));
                self.apply_edit("Pixel Builder: Assign material", |m, s| {
                    let f = s.all_or_selected_faces(m);
                    ops::set_material(m, &f, slot);
                    ops::set_color(m, &f, col);
                });
            }
        }
        self.sync_ui();
        self.redraw();
    }

    fn delete_selected(&mut self) {
        let mode = self.mode;
        self.apply_edit("Pixel Builder: Delete", |m, s| {
            match mode {
                Mode::Face => ops::delete_faces(m, &s.sel_faces),
                Mode::Edge => ops::delete_edges(m, &s.sel_edges),
                Mode::Vertex => ops::delete_verts(m, &s.sel_verts),
                Mode::Object => {}
            }
            s.clear_selection();
        });
    }

    fn set_mode(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        // Convert the selection (face -> its verts / edges, verts -> faces fully inside).
        if let Some(d) = self.data() {
            let db = d.bind();
            let m = db.mesh();
            let verts = self.selected_verts(m);
            self.sel_faces = if mode == Mode::Face { m.faces_within(&verts) } else { BTreeSet::new() };
            self.sel_edges = if mode == Mode::Edge { m.edges_within(&verts) } else { BTreeSet::new() };
            self.sel_verts = if mode == Mode::Vertex { verts } else { BTreeSet::new() };
        }
        self.mode = mode;
        if mode != Mode::Object {
            self.tool = Tool::Select;
        }
        self.cancel_drag();
        self.sync_ui();
        self.redraw();
    }

    fn set_xform(&mut self, x: XformMode) {
        self.xform_mode = x;
        self.hover_handle = None;
        if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
        }
    }

    fn set_tool(&mut self, t: Tool) {
        self.cancel_vpaint_stroke();
        self.tool = t;
        self.knife = Knife::default();
        self.wall_pts.clear();
        self.wall_cursor = None;
        self.hover_place = None;
        if t != Tool::Select {
            self.cancel_drag();
        }
        self.sync_ui();
        self.redraw();
    }

    // ------------------------------------------------------------------ drag

    fn begin_drag(&mut self, cam: &Gd<Camera3D>, pos: Vector2, handle: Handle, extrude: bool) {
        let Some(data) = self.data() else { return };
        let (before, mut start) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone())
        };
        let axis = match handle {
            Handle::Move(a) => a,
            _ => None,
        };
        let mut name = match handle {
            Handle::Move(_) => "Pixel Builder: Move",
            Handle::Rotate(_) => "Pixel Builder: Rotate",
            Handle::Scale(_) => "Pixel Builder: Scale",
        };
        if extrude && self.mode == Mode::Face && !self.sel_faces.is_empty() {
            self.sel_faces = ops::extrude_faces(&mut start, &self.sel_faces, 0.0);
            name = "Pixel Builder: Extrude";
        }
        let verts = match self.mode {
            Mode::Face => start.verts_of_faces(&self.sel_faces),
            Mode::Edge => start.verts_of_edges(&self.sel_edges),
            _ => self.sel_verts.clone(),
        };
        let Some(pivot) = self.pivot() else { return };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let axes = self.gizmo_axes();
        let start_t = axis.and_then(|a| closest_on_axis(pivot, axes[a], o, d)).unwrap_or(0.0);
        let n = cam.get_global_transform().basis.col_c();
        let start_hit = plane_hit(pivot, n, o, d).unwrap_or(pivot);
        let weights = self.drag_weights(&start, &verts);
        let (rot, scale) = match handle {
            Handle::Rotate(a) => (Some(self.begin_rotate(cam, pos, pivot, a)), None),
            Handle::Scale(a) => (None, Some(self.begin_scale(cam, pos, pivot, a))),
            Handle::Move(_) => (None, None),
        };
        let (snap_anchor, snap_targets) = if matches!(handle, Handle::Move(_)) { self.snap_setup(cam, pos, &start, &weights, pivot) } else { (pivot, Vec::new()) };
        let uv_frames = self.uv_lock.then(|| pt_core::build::uv_lock::drag_uv_frames(&start, &weights.keys().copied().collect()));
        self.drag = Some(Drag {
            axis,
            before,
            start,
            pivot,
            start_t,
            start_hit,
            name,
            last_delta: Vector3::ZERO,
            handle: Some(handle),
            weights,
            rot,
            scale,
            snap_anchor,
            snap_targets,
            snap_hit: None,
            cursor: pos,
            uv_frames,
            axes,
        });
    }

    fn drag_motion(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.drag.as_ref().is_some_and(|d| d.rot.is_some() || d.scale.is_some()) {
            self.drag_rotate_scale(cam, pos);
            return;
        }
        let vsnap = self.snap_offset(cam, pos);
        if let Some(d) = &mut self.drag {
            d.cursor = pos;
        }
        let Some(drag) = &self.drag else { return };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let axes = drag.axes;
        let mut delta = match drag.axis {
            Some(a) => match closest_on_axis(drag.pivot, axes[a], o, d) {
                Some(t) => axes[a] * (t - drag.start_t),
                None => return,
            },
            None => {
                let n = cam.get_global_transform().basis.col_c();
                match plane_hit(drag.pivot, n, o, d) {
                    Some(h) => h - drag.start_hit,
                    None => return,
                }
            }
        };
        if self.snap_cell
            && drag.axis != Some(1)
            && let Some(t) = &self.terrain
        {
            // Snap the pivot onto the nearest terrain cell corner / centre (hex-aligned walls).
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * (drag.pivot + delta);
            let q = snap::cell_point(&t.bind().store().config, v2(l.x, l.z), true);
            let w = xf * Vector3::new(q.x, l.y, q.y);
            delta.x = w.x - drag.pivot.x;
            delta.z = w.z - drag.pivot.z;
            if drag.axis.is_some() {
                let a = axes[drag.axis.unwrap_or(0)];
                delta = a * delta.dot(a);
            }
            if self.snap_grid {
                delta.y = snap::snap_value(delta.y, self.grid_step);
            }
        } else if self.snap_grid {
            delta = from_p(snap::snap_vec(to_p(delta), self.grid_step));
        }
        if let Some(v) = vsnap {
            delta = v;
        }
        if delta == drag.last_delta {
            self.base().update_overlays();
            return;
        }
        let local = self.xform().basis.inverse() * delta;
        let mut m = drag.start.clone();
        pt_core::build::xform::translate_weighted(&mut m, &drag.weights, to_p(local));
        drag.lock_uvs(&mut m);
        if let Some(dr) = &mut self.drag {
            dr.last_delta = delta;
        }
        if let Some(mut data) = self.data() {
            data.bind_mut().replace(m);
            data.emit_changed();
        }
        self.redraw();
    }

    fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        let Some(data) = self.data() else { return };
        let after = data.bind().get_blob();
        if after != drag.before {
            self.commit_blob(drag.name, &data, drag.before, after);
        }
    }

    fn cancel_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        if let Some(mut data) = self.data() {
            data.bind_mut().set_blob(drag.before);
        }
        self.redraw();
    }

    // ------------------------------------------------------------------ keys

    fn on_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        let k = key.get_keycode();
        let ctrl = key.is_ctrl_pressed() || key.is_meta_pressed();
        if ctrl {
            if k == Key::A && self.mode != Mode::Object {
                if let Some(d) = self.data() {
                    let db = d.bind();
                    let m = db.mesh();
                    match self.mode {
                        Mode::Face => self.sel_faces = (0..m.faces.len()).collect(),
                        Mode::Edge => self.sel_edges = m.edges().into_iter().collect(),
                        Mode::Vertex => self.sel_verts = (0..m.positions.len() as u32).collect(),
                        Mode::Object => {}
                    }
                }
                self.redraw();
                return true;
            }
            if k == Key::B && self.mode == Mode::Edge {
                self.run_action(Action::Bevel);
                return true;
            }
            if k == Key::D && self.mode == Mode::Face && !self.sel_faces.is_empty() {
                self.run_action(Action::Duplicate);
                return true;
            }
            if k == Key::R && self.mode == Mode::Edge && !self.sel_edges.is_empty() {
                self.run_action(Action::InsertLoop);
                return true;
            }
            if k == Key::T && self.target.is_some() && matches!(self.mode, Mode::Object | Mode::Face) {
                self.run_action(Action::Triangulate);
                return true;
            }
            return false;
        }
        let editing_target = self.mode != Mode::Object && self.target.is_some();
        if key.is_alt_pressed() && editing_target {
            match k {
                Key::R => {
                    self.act_rotate(-90.0);
                    self.redraw();
                    return true;
                }
                Key::L => {
                    self.act_select_loop(true);
                    self.redraw();
                    return true;
                }
                _ => {}
            }
        }
        if key.is_shift_pressed() && editing_target {
            // Unibuilder's Shift+W / E / R handle modes, Shift+G shrink.
            let a = match k {
                Key::W => Some(Action::GizmoMove),
                Key::E => Some(Action::GizmoRotate),
                Key::R => Some(Action::GizmoScale),
                Key::G => Some(Action::Shrink),
                Key::V if self.mode == Mode::Edge => Some(Action::InsertVertex),
                _ => None,
            };
            if let Some(a) = a {
                self.run_action(a);
                return true;
            }
        }
        if key.is_shift_pressed() || key.is_alt_pressed() {
            return false;
        }
        let editing = self.mode != Mode::Object;
        let action = match k {
            Key::KEY_1 => Some(Action::ModeObject),
            Key::KEY_2 => Some(Action::ModeFace),
            Key::KEY_3 => Some(Action::ModeEdge),
            Key::KEY_4 => Some(Action::ModeVertex),
            Key::C => Some(Action::ToolShape),
            Key::P => Some(Action::ToolPlace),
            Key::W if editing || self.tool != Tool::Select => Some(Action::ToolSelect),
            Key::E if editing => Some(Action::Extrude),
            Key::I if editing => Some(Action::Inset),
            Key::B => Some(Action::ToolPaint),
            Key::M if editing => Some(Action::Merge),
            Key::J if self.mode == Mode::Edge => Some(Action::ConnectEdges),
            Key::J if editing => Some(Action::Connect),
            Key::F if editing => Some(Action::Fill),
            Key::R if editing => Some(Action::Rotate),
            Key::G if editing => Some(Action::Grow),
            Key::K if editing => Some(Action::ToolKnife),
            Key::L if editing => Some(Action::SelectLoop),
            Key::O if editing => Some(Action::SoftSelect),
            Key::DELETE if editing => Some(Action::Delete),
            Key::U => Some(Action::UvEditor),
            Key::KEY_5 => Some(Action::ToolVertexPaint),
            _ => None,
        };
        if let Some(a) = action {
            if self.target.is_none() && !matches!(a, Action::ToolShape | Action::ToolSelect) {
                return false;
            }
            self.run_action(a);
            return true;
        }
        match k {
            Key::ESCAPE => {
                if self.shape_drag.is_some() {
                    self.cancel_shape();
                } else if self.cancel_vpaint_stroke() {
                } else if self.drag.is_some() {
                    self.cancel_drag();
                } else if self.tool != Tool::Select {
                    self.set_tool(Tool::Select);
                } else if editing && !(self.sel_faces.is_empty() && self.sel_edges.is_empty() && self.sel_verts.is_empty()) {
                    self.clear_selection();
                } else if editing {
                    self.set_mode(Mode::Object);
                } else {
                    return false;
                }
                self.redraw();
                true
            }
            Key::BRACKETLEFT | Key::BRACKETRIGHT => {
                let f = if k == Key::BRACKETLEFT { 0.5 } else { 2.0 };
                self.grid_step = (self.grid_step * f).clamp(0.0625, 64.0);
                self.sync_ui();
                true
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------------ wireframe gizmo

    fn ensure_gizmo(&mut self) {
        if self.gizmo.is_some() {
            return;
        }
        let Some(mut t) = self.target.clone() else { return };
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name("PixelBuilderGizmo");
        mi.set_mesh(&ImmediateMesh::new_gd());
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_cull_mode(CullMode::DISABLED);
        mat.set_render_priority(100);
        mi.set_material_override(&mat);
        mi.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
        mi.set_layer_mask(1);
        mi.add_to_group("pixel_ignore");
        add_internal(&mut t, &mi);
        self.gizmo = Some(mi);
    }

    fn drop_gizmo(&mut self) {
        if let Some(g) = self.gizmo.take()
            && g.is_instance_valid()
        {
            let mut g = g;
            g.queue_free();
        }
    }

    fn redraw(&mut self) {
        self.base().update_overlays();
        self.sync_uv_editor();
        let Some(g) = &self.gizmo else { return };
        if !g.is_instance_valid() {
            self.gizmo = None;
            return;
        }
        let Some(mesh) = g.get_mesh() else { return };
        let Ok(mut im) = mesh.try_cast::<ImmediateMesh>() else { return };
        im.clear_surfaces();
        let Some(data) = self.data() else { return };
        let db = data.bind();
        let m = db.mesh();
        // Drop selections that an undo / external edit made invalid.
        let nf = m.faces.len();
        let nv = m.positions.len() as u32;
        self.sel_faces.retain(|f| *f < nf);
        self.sel_verts.retain(|v| *v < nv);
        self.sel_edges.retain(|e| e.0 < nv && e.1 < nv);
        if self.hover_face.is_some_and(|f| f >= nf) {
            self.hover_face = None;
        }
        if self.mode == Mode::Object {
            return;
        }
        let p = |i: u32| from_p(m.positions[i as usize]);
        // Face tints.
        let tris = pt_core::build::compile::face_triangles(m);
        let mut tri_pts: Vec<(Vector3, Color)> = Vec::new();
        let faces: Vec<(usize, Color)> = self
            .sel_faces
            .iter()
            .map(|f| (*f, self.prefs.color_face_selected))
            .chain(self.hover_face.filter(|f| !self.sel_faces.contains(f)).map(|f| (f, self.prefs.color_face_hover)))
            .collect();
        for (f, c) in faces {
            let n = from_p(m.face_normal(f)) * 0.004;
            for t in &tris[f] {
                for k in t {
                    tri_pts.push((p(m.faces[f].verts[*k]) + n, c));
                }
            }
        }
        if !tri_pts.is_empty() {
            im.surface_begin(PrimitiveType::TRIANGLES);
            for (v, c) in tri_pts {
                im.surface_set_color(c);
                im.surface_add_vertex(v);
            }
            im.surface_end();
        }
        // Edges.
        let sel_edges: BTreeSet<Edge> = match self.mode {
            Mode::Face => m.edges_of_faces(&self.sel_faces),
            Mode::Edge => self.sel_edges.clone(),
            _ => m.edges_within(&self.sel_verts),
        };
        im.surface_begin(PrimitiveType::LINES);
        for e in m.edges() {
            let c = if sel_edges.contains(&e) { self.prefs.color_edge_selected } else { self.prefs.color_edge };
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.0));
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.1));
        }
        im.surface_end();
    }
}

impl PixelBuilderPlugin {
    // ------------------------------------------------------------------ Godot gizmo lock

    /// Godot draws its own move / rotate / scale gizmo on the selected node, on top of the
    /// builder's element handles. Unibuilder (`_sync_native_gizmo_lock`) hides it by setting the
    /// editor's `_edit_lock_` meta while the plugin owns the transform; we do the same in the
    /// Face / Edge / Vertex modes (and for the element tools). A node the user locked himself
    /// is left alone; the lock is stripped while the scene is being saved.
    pub(super) fn sync_gizmo_lock(&mut self) {
        let want = self.target.clone().filter(|t| t.is_instance_valid() && self.visible && (self.mode != Mode::Object || matches!(self.tool, Tool::Paint | Tool::Knife | Tool::VertexPaint)));
        if want.as_ref().map(|t| t.instance_id()) == self.lock_target.as_ref().map(|t| t.instance_id()) {
            return;
        }
        self.set_gizmo_lock(want);
    }

    fn set_gizmo_lock(&mut self, want: Option<Gd<PixelBuilderMesh>>) {
        let mut changed = false;
        if let Some(mut old) = self.lock_target.take()
            && old.is_instance_valid()
            && old.has_meta(PLUGIN_LOCK_META)
        {
            old.remove_meta(PLUGIN_LOCK_META);
            old.remove_meta(EDIT_LOCK_META);
            changed = true;
        }
        if let Some(mut t) = want
            && !t.has_meta(EDIT_LOCK_META)
        {
            t.set_meta(EDIT_LOCK_META, &true.to_variant());
            t.set_meta(PLUGIN_LOCK_META, &true.to_variant());
            self.lock_target = Some(t);
            changed = true;
        }
        if changed && let Some(sel) = EditorInterface::singleton().get_selection() {
            // The 3D editor rebuilds its gizmo on a selection change (Unibuilder's "selection nudge").
            sel.upcast::<Object>().call_deferred("emit_signal", &["selection_changed".to_variant()]);
        }
    }

    // ------------------------------------------------------------------ UV editor

    fn open_uv_editor(&mut self) {
        if self.uv_window.as_ref().is_none_or(|w| !w.is_instance_valid()) {
            let (win, canvas) = uv_editor::build_window();
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&win);
            }
            self.uv_window = Some(win);
            self.uv_canvas = Some(canvas);
        }
        if let Some(w) = &mut self.uv_window {
            if w.is_visible() {
                w.grab_focus();
            } else {
                w.popup_centered();
            }
        }
        self.sync_uv_editor();
    }

    /// Hands the edited mesh and the 3D face selection to the UV window (if open).
    fn sync_uv_editor(&mut self) {
        let open = self.uv_window.as_ref().is_some_and(|w| w.is_instance_valid() && w.is_visible());
        if !open {
            return;
        }
        let faces = if self.mode == Mode::Face { super::data::face_array(&self.sel_faces) } else { PackedInt32Array::new() };
        let target = self.target.clone();
        if let Some(c) = &mut self.uv_canvas
            && c.is_instance_valid()
        {
            c.bind_mut().set_target(target, faces);
        }
    }

    // ------------------------------------------------------------------ preferences

    fn open_preferences(&mut self) {
        if self.prefs_panel.as_ref().is_none_or(|p| !p.is_instance_valid()) {
            let gd = self.to_gd().upcast::<Object>();
            let (panel, controls) = prefs::build_panel(&gd, &self.prefs);
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&panel);
            }
            self.prefs_panel = Some(panel);
            self.prefs_controls = controls;
        }
        prefs::sync_panel(&mut self.prefs_controls, &self.prefs);
        if let Some(p) = &mut self.prefs_panel {
            p.popup_centered();
        }
    }

    /// Applies the preferences (`startup`: also the default snap steps).
    fn apply_prefs(&mut self, startup: bool) {
        prefs::set_handle_scale(self.prefs.handle_scale);
        if startup {
            self.grid_step = self.prefs.grid_step;
            self.rot_snap = self.prefs.rot_snap;
            self.snap_grid = self.prefs.snap_grid;
        }
    }
}

fn plane_hit(p: Vector3, n: Vector3, o: Vector3, d: Vector3) -> Option<Vector3> {
    let den = n.dot(d);
    if den.abs() < 1e-6 {
        return None;
    }
    let t = n.dot(p - o) / den;
    (t > 0.0).then(|| o + d * t)
}

fn ui_color_bytes(c: Color) -> [u8; 4] {
    super::data::color_bytes(c)
}

#[godot_api]
impl PixelBuilderPlugin {
    #[func]
    fn on_action(&mut self, id: i64) {
        if let Some(a) = Action::from_i64(id) {
            self.run_action(a);
        }
    }

    #[func]
    fn on_field(&mut self, value: f64, id: i64) {
        let Some(f) = Field::from_i64(id) else { return };
        let v = value as f32;
        let old = self.spec;
        match f {
            Field::BevelSegs => self.bevel_segs = value as i64,
            Field::SoftRadius => self.soft_radius = v,
            Field::Falloff => self.falloff = value as i64,
            Field::RotSnap => self.rot_snap = v,
            Field::MirrorAxis => self.mirror_axis = value as i64,
            Field::Arc => self.spec.arc = v,
            Field::Inner => self.spec.inner = v,
            Field::Opening => self.spec.opening = v,
            Field::DoorHeight => self.spec.door_height = v,
            Field::Profile => self.spec.profile = value as u8,
            Field::Side => self.spec.side = value as u8,
            Field::SizeX => self.spec.size.x = v,
            Field::SizeY => self.spec.size.y = v,
            Field::SizeZ => self.spec.size.z = v,
            Field::Segments => self.spec.segments = value as u32,
            Field::Steps => self.spec.steps = value as u32,
            Field::Thickness => self.spec.thickness = v,
            Field::Rings => self.spec.rings = value as u32,
            Field::Slot => self.slot = value as i64,
            Field::GridStep => self.grid_step = v,
            Field::Distance => self.distance = v,
            Field::Inset => self.inset = v,
            Field::Bevel => self.bevel = v,
            Field::Cuts => self.cuts = value as i64,
            Field::PaintApply => self.vp.fill = value >= 0.5,
            Field::PaintChannel => self.vp.channel = value as i64,
            Field::PaintRadius => self.vp.radius = v,
            Field::PaintStrength => self.vp.strength = v,
            Field::PaintSoftness => self.vp.softness = v,
            Field::PaintAlpha => self.vp.alpha = v,
            Field::InsetMode => self.opts.inset_mode = value as i64,
            Field::SubdivSegs => self.opts.subdiv_segs = (value as i64).max(2),
            Field::GizmoOrient => self.opts.gizmo_orient = value as i64,
            Field::TrimPlace => self.opts.trim_place = value as i64,
            Field::TrimDepth => self.opts.trim_depth = v,
            Field::TrimHeight => self.opts.trim_height = v,
        }
        if matches!(f, Field::PaintApply | Field::PaintChannel) {
            self.sync_ui();
        }
        self.spec_changed(old);
        self.base().update_overlays();
    }

    #[func]
    fn on_field_option(&mut self, index: i64, id: i64) {
        self.on_field(index as f64, id);
    }

    #[func]
    fn on_kind(&mut self, index: i64) {
        let old = self.spec;
        self.spec.kind = ShapeKind::from_i64(index);
        if self.reshape && self.target.is_some() {
            self.spec_changed(old);
        } else if self.tool != Tool::Shape {
            self.tool = Tool::Shape;
        }
        self.sync_ui();
    }

    #[func]
    fn on_smooth(&mut self, on: bool) {
        let old = self.spec;
        self.spec.smooth = on;
        self.spec_changed(old);
    }

    #[func]
    fn on_arched(&mut self, on: bool) {
        let old = self.spec;
        self.spec.arched = on;
        self.spec_changed(old);
        self.base().update_overlays();
    }

    /// One frame after a boolean started: bake the CSG result.
    #[func]
    fn on_boolean_frame(&mut self) {
        self.finish_boolean();
    }

    /// Test / scripting hook: boolean of the edited mesh with `other` (0 union, 1 subtract,
    /// 2 intersect); finishes a frame later.
    #[func]
    fn debug_boolean(&mut self, other: Gd<PixelBuilderMesh>, op: i64) {
        self.boolean_with(other, op);
    }

    /// Test / scripting hook: knife-cut `points` (local) into `face`.
    #[func]
    fn debug_knife(&mut self, face: i64, points: PackedVector3Array) {
        self.set_mode(Mode::Face);
        self.set_tool(Tool::Knife);
        if let Some(d) = self.data() {
            let db = d.bind();
            let m = db.mesh();
            if (face as usize) < m.faces.len() {
                self.knife.face = Some(face as usize);
            }
        }
        self.knife.points = points.as_slice().iter().map(|p| to_p(*p)).collect();
        self.knife_commit();
    }

    /// Test / scripting hook: draw a wall (current wall settings) through world `points`.
    #[func]
    fn debug_wall(&mut self, points: PackedVector3Array, closed: bool) {
        if !self.spec.kind.is_polyline() {
            self.spec.kind = ShapeKind::Wall;
        }
        self.tool = Tool::Shape;
        self.wall_through(points.as_slice(), closed);
        self.sync_ui();
    }

    /// Test / scripting hook: set a shape field / option by id (same ids as the strip).
    #[func]
    fn debug_field(&mut self, id: i64, value: f64) {
        self.on_field(value, id);
        self.sync_ui();
    }

    /// Test / scripting hook: switch the handle mode (0 move, 1 rotate, 2 scale).
    #[func]
    fn debug_set_xform(&mut self, mode: i64) {
        self.set_xform(match mode {
            1 => XformMode::Rotate,
            2 => XformMode::Scale,
            _ => XformMode::Move,
        });
        self.redraw();
    }

    #[func]
    fn on_pref(&mut self, value: Variant, key: i64) {
        let Some(k) = prefs::Key::from_i64(key) else { return };
        self.prefs.set(k, &value);
        self.prefs.save();
        // Snap defaults apply right away too (the strip shows them).
        match k {
            prefs::Key::GridStep => self.grid_step = self.prefs.grid_step,
            prefs::Key::RotSnap => self.rot_snap = self.prefs.rot_snap,
            prefs::Key::SnapGrid => self.snap_grid = self.prefs.snap_grid,
            _ => {}
        }
        self.apply_prefs(false);
        self.sync_ui();
        self.redraw();
    }

    #[func]
    fn on_prefs_reset(&mut self) {
        self.prefs = Prefs::default();
        self.prefs.save();
        self.apply_prefs(true);
        prefs::sync_panel(&mut self.prefs_controls, &self.prefs);
        self.sync_ui();
        self.redraw();
    }

    #[func]
    fn sync_gizmo_lock_deferred(&mut self) {
        self.sync_gizmo_lock();
    }

    /// Test / scripting hook: the UV editor canvas (opens the window).
    #[func]
    fn debug_uv_editor(&mut self) -> Option<Gd<PixelBuilderUvCanvas>> {
        self.open_uv_editor();
        self.uv_canvas.clone()
    }

    /// Test / scripting hook: the preferences popup (opens it).
    #[func]
    fn debug_preferences(&mut self) -> Option<Gd<godot::classes::PopupPanel>> {
        self.open_preferences();
        self.prefs_panel.clone()
    }

    #[func]
    fn on_color(&mut self, c: Color) {
        self.color = c;
    }

    #[func]
    fn on_uv_field(&mut self, value: f64, id: i64) {
        if let Some(f) = UvField::from_i64(id) {
            self.uv_field(f, value as f32);
        }
    }

    #[func]
    fn on_uv_world(&mut self, on: bool) {
        self.uv_world = on;
    }

    #[func]
    fn on_uv_apply(&mut self) {
        let uv = self.uv_params();
        self.apply_uv(uv);
    }

    #[func]
    fn on_uv_reset(&mut self) {
        self.load_uv(pt_core::build::UvParams::default());
        let uv = self.uv_params();
        self.apply_uv(uv);
        self.sync_ui();
    }

    #[func]
    fn on_uv_rotate(&mut self) {
        self.rotate_uv();
    }

    #[func]
    fn on_slot_pick(&mut self, slot: i64) {
        self.pick_slot(slot);
    }

    #[func]
    fn on_slot_material(&mut self, res: Option<Gd<godot::classes::Resource>>, slot: i64) {
        self.set_slot_material(res, slot);
    }

    #[func]
    fn on_add_slot(&mut self) {
        self.add_slot();
    }

    #[func]
    fn on_export_file(&mut self, path: GString) {
        self.message = match self.export_to(&path.to_string()) {
            Ok(m) => m,
            Err(e) => format!("Export failed: {e}"),
        };
        if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
            fs.scan();
        }
        self.base().update_overlays();
    }

    /// Test / scripting hook: run a toolbar action by id.
    #[func]
    fn debug_action(&mut self, id: i64) {
        self.on_action(id);
    }

    /// Builder toolbar button under the mouse (-1 = none): its name and shortcut in the strip.
    #[func]
    fn on_button_hover(&mut self, id: i64) {
        let text = Action::from_i64(id).map(ui::hover_text).unwrap_or_default();
        if let Some(u) = &mut self.ui {
            u.hover.set_text(&text);
        }
    }

    /// Paint swatch: click uses its color, Ctrl+click stores the current color in it
    /// (`paint_panel.gd::_on_swatch`).
    #[func]
    fn on_swatch(&mut self, idx: i64) {
        let i = idx.clamp(0, 7) as usize;
        let input = Input::singleton();
        if input.is_key_pressed(Key::CTRL) || input.is_key_pressed(Key::META) {
            self.swatches[i] = self.color;
            let c = self.color;
            if let Some(u) = &mut self.ui {
                u.set_swatch(i, c);
            }
        } else {
            self.color = self.swatches[i];
            self.sync_ui();
        }
    }

    /// Builder menu (3D toolbar).
    #[func]
    fn on_builder_menu(&mut self, id: i64) {
        let a = match id {
            0 => Some(Action::UvEditor),
            1 => Some(Action::Preferences),
            2 => Some(Action::BakeMesh),
            3 => Some(Action::ExportObj),
            4 => Some(Action::ExportGltf),
            5 => Some(Action::FreezeTransform),
            6 => Some(Action::CenterPivot),
            _ => None,
        };
        if let Some(a) = a {
            self.run_action(a);
        }
    }

    /// Test hook: the Builder menu button and whether Godot's "Mesh" menu is hidden.
    #[func]
    fn debug_menus(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("builder_menu", self.builder_menu.as_ref().is_some_and(|m| m.is_instance_valid() && m.is_visible()));
        d.set("builder_menu_text", self.builder_menu.as_ref().map(|m| m.get_text().to_string()).unwrap_or_default());
        d.set("mesh_menu_hidden", self.hidden_mesh_menu.as_ref().is_some_and(|m| m.is_instance_valid() && !m.is_visible()));
        d
    }

    /// Test hook: insert a vertex on edge (a, b) at world `pos` (the insert-vertex click).
    #[func]
    fn debug_insert_vertex(&mut self, a: i64, b: i64, pos: Vector3) {
        let local = to_p(self.xform().affine_inverse() * pos);
        self.apply_edit("Pixel Builder: Insert vertex", |m, _| {
            pt_core::build::ub_ops::insert_vertex(m, (a as u32, b as u32), local);
        });
    }

    /// Test hook: shape placement under a screen point (frame y axis included).
    #[func]
    fn debug_placement(&mut self, pos: Vector2) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(cam) = EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done().and_then(|v| v.get_camera_3d()) else { return d };
        if let Some(p) = self.placement(&cam, pos) {
            d.set("point", p.point);
            d.set("up", p.basis.col_b());
        }
        d
    }

    /// Test hook: drop `material` (as a dragged resource) at `pos` of 3D viewport 0, through
    /// the same path as a drag-and-drop from the FileSystem / inspector. True when it landed.
    #[func]
    fn debug_drop_material(&mut self, material: Gd<godot::classes::Material>, pos: Vector2) -> bool {
        let Some(vp) = EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done() else { return false };
        let mut d = VarDictionary::new();
        d.set("type", "resource");
        d.set("resource", &material);
        let data = d.to_variant();
        if !self.can_drop_material(&vp, pos, &data) {
            return false;
        }
        self.material_drag_began(material);
        self.drop_material(&vp, pos, &data);
        self.material_drag_ended();
        true
    }

    /// Test / scripting hook: export the selected mesh to `path` ("" on success, else the error).
    #[func]
    fn debug_export(&mut self, path: GString) -> GString {
        match self.export_to(&path.to_string()) {
            Ok(_) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// Test / scripting hook: select edges by vertex pairs (Edge mode).
    #[func]
    fn debug_select_edges(&mut self, pairs: PackedInt32Array) {
        self.set_mode(Mode::Edge);
        self.sel_edges = pairs.as_slice().chunks_exact(2).map(|p| pt_core::build::edge(p[0] as u32, p[1] as u32)).collect();
        self.redraw();
    }

    /// Test / scripting hook: simulate a shape drag in world space (start, corner, height).
    #[func]
    fn debug_draw_shape(&mut self, start: Vector3, corner: Vector3, height: f32) {
        let cell = self.terrain.as_ref().map(|t| super::node::world_cell(t, start));
        self.shape_drag = Some(ShapeDrag {
            stage: 1,
            start: Placement { point: start, cell, basis: Basis::IDENTITY },
            corner,
            height,
            press: Vector2::ZERO,
            append: false,
            height_from: Vector2::ZERO,
        });
        self.commit_shape_pub();
    }

    /// Test / scripting hook: one vertex-paint stroke through world `points` (the face under
    /// each point is `faces[i]`, used by Fill), committed as one undo step.
    #[func]
    fn debug_vpaint(&mut self, points: PackedVector3Array, faces: PackedInt32Array) {
        if self.tool != Tool::VertexPaint {
            self.set_tool(Tool::VertexPaint);
        }
        let Some(data) = self.data() else { return };
        let (before, base) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone())
        };
        self.vp.stroke = Some(vpaint::VStroke::new(before, base));
        for (i, p) in points.as_slice().iter().enumerate() {
            self.vp.hover = Some((faces.as_slice().get(i).copied().unwrap_or(0).max(0) as usize, *p));
            self.vp_paint_at();
        }
        if let Some(stroke) = self.vp.stroke.take() {
            let after = data.bind().get_blob();
            let before = stroke.before();
            if after != before {
                self.commit_blob("Pixel Builder: Paint vertex colors", &data, before, after);
            }
        }
        self.redraw();
    }

    /// Test / scripting hook: puts the vertex-paint cursor on a world point over `face`.
    #[func]
    fn vp_hover_debug(&mut self, point: Vector3, face: i64) {
        self.vp.hover = Some((face.max(0) as usize, point));
        self.base().update_overlays();
    }

    /// Test / scripting hook: moves the current element selection by a world `delta` the way a
    /// handle drag does (soft selection, UV lock), one undo step.
    #[func]
    fn debug_move_selection(&mut self, delta: Vector3) {
        let Some(data) = self.data() else { return };
        let (before, start) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone())
        };
        let verts = self.selected_verts(&start);
        let weights = self.drag_weights(&start, &verts);
        let frames = self.uv_lock.then(|| pt_core::build::uv_lock::drag_uv_frames(&start, &weights.keys().copied().collect()));
        let mut m = start.clone();
        let local = self.xform().basis.inverse() * delta;
        pt_core::build::xform::translate_weighted(&mut m, &weights, to_p(local));
        if let Some(f) = &frames {
            pt_core::build::uv_lock::refit_drag_uvs(&mut m, f);
        }
        let mut data = data;
        data.bind_mut().replace(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            self.commit_blob("Pixel Builder: Move", &data, before, after);
        }
        self.redraw();
    }

    /// Test / scripting hook: current mode, tool and selection sizes.
    #[func]
    fn get_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("mode", self.mode.name());
        d.set("tool", self.tool.name());
        d.set("faces", self.sel_faces.len() as i64);
        d.set("edges", self.sel_edges.len() as i64);
        d.set("verts", self.sel_verts.len() as i64);
        d.set("uv_lock", self.uv_lock);
        d.set("lock_meta", self.lock_target.as_ref().is_some_and(|t| t.is_instance_valid() && t.has_meta(EDIT_LOCK_META)));
        d.set("message", self.message.as_str());
        d
    }

    /// Test / scripting hook: select vertices (switches to Vertex mode).
    #[func]
    fn debug_select_verts(&mut self, verts: PackedInt32Array) {
        self.set_mode(Mode::Vertex);
        self.sel_verts = verts.as_slice().iter().filter(|v| **v >= 0).map(|v| *v as u32).collect();
        self.redraw();
    }

    /// Test / scripting hook: run a toolbar action by id and optionally select faces.
    #[func]
    fn debug_select_faces(&mut self, faces: PackedInt32Array) {
        self.set_mode(Mode::Face);
        self.sel_faces = super::data::face_set(&faces);
        self.redraw();
    }
}
