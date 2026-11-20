//! `PixelBuilderPlugin` — level-builder editor plugin (auto-registered). Active when a
//! `PixelBuilderMesh` (edit it) or a `PixelObject` (stamp new meshes into it) is selected.
//!
//! Every geometry edit snapshots the mesh blob before and after and records one undo action
//! (`set_blob`), so undo/redo never depends on the plugin's selection state.

use std::collections::BTreeSet;

use godot::classes::base_material_3d::{CullMode, Flags, ShadingMode, Transparency};
use godot::classes::editor_plugin::AfterGuiInput;
use godot::classes::mesh::PrimitiveType;
use godot::classes::{
    Camera3D, Control, EditorInterface, EditorPlugin, IEditorPlugin, ImmediateMesh, Input,
    InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion, MeshInstance3D,
    Node3D, Object, ResourceSaver, StandardMaterial3D,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::build::{BMesh, Edge, ShapeKind, ShapeSpec, ops, pick, shapes, snap};
use pt_core::math::{Vec3 as PVec3, v2};

use super::data::{PixelBuilderMeshData, from_p, to_p};
use super::modeler_ui::LayoutUi;
use super::node::PixelBuilderMesh;
use super::ui::{self, Action, Field, Mode, Tool, UvField, Values};

mod adjust;
mod attr_overlay;
mod bezier;
mod context_menu;
mod extra;
mod modeler;
mod objects;
mod paint_cmds;
mod tools;
mod vpaint;
mod xg_actions;
mod xg_io;
use super::prefs::{self, Prefs};
use super::ui::{SyncState, XformMode};
use super::uv_editor::{self, PixelBuilderUvCanvas};
use crate::terrain::MarchingSquaresTerrain;
use crate::util::*;
pub use extra::mesh_to_bmesh;
use extra::{PaintStroke, ShapeDrag};
use pt_core::build::xform::Weights;
use tools::{Handle, Knife, PendingBool, RotateDrag, ScaleDrag};

const PICK_PX: f32 = 10.0;
const ORANGE: Color = Color::from_rgba(1.0, 0.6005, 0.1505, 1.0);

struct Drag {
    /// 0..2 world axis, None = camera plane.
    axle: Option<usize>,
    before: PackedByteArray,
    commence: BMesh,
    fulcrum: Vector3,
    start_t: f32,
    start_hit: Vector3,
    moniker: &'static str,
    last_delta: Vector3,
    /// Grabbed handle (move / rotate / scale).
    handle: Option<Handle>,
    /// Individual origins: connected pieces rotated / scaled about their own centres.
    groups: Option<Vec<BTreeSet<u32>>>,
    /// Per-vertex weights (soft selection; 1 for the selection).
    weights: Weights,
    rot: Option<RotateDrag>,
    magnify: Option<ScaleDrag>,
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
            pt_core::build::uv_lock::refit_after_drag(m, f);
        }
    }
}

struct Marquee {
    start_pt: Vector2,
    to: Vector2,
    /// Shift held: the box changes the selection by the Shift selects preference.
    additive: bool,
}

/// What a matched shortcut runs.
#[derive(Clone, Copy, Debug)]
enum KeyCmd {
    Run(Action),
    CycleMode,
    Grid(f32),
}

/// The pickable element under the cursor in an element mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Elem {
    Face(usize),
    Edge(Edge),
    Vert(u32),
}

/// Viewport feedback text cached on change (`redraw`), drawn by the overlay.
#[derive(Default)]
struct Feedback {
    /// Mesh / selection counts (Show scene info).
    facts: Option<String>,
    /// Selection bounding box: world anchor (top centre) and the size label (Show dimensions).
    dims: Option<(Vector3, String)>,
}

/// Shift adds, subtracts or toggles `picked` into `set` (no Shift replaces).
fn apply_shift<T: Ord + Copy>(set: &mut BTreeSet<T>, picked: &BTreeSet<T>, shift: bool, mode: i64) {
    if !shift {
        *set = picked.clone();
        return;
    }
    match mode {
        1 => set.retain(|x| !picked.contains(x)),
        2 => {
            for x in picked {
                if !set.remove(x) {
                    set.insert(*x);
                }
            }
        }
        _ => set.extend(picked.iter().copied()),
    }
}

/// Liang–Barsky: the part of segment a–b inside the rectangle lo–hi, as parameters (t0, t1).
fn trim_stretch_quad(a: Vector2, b: Vector2, lo: Vector2, hi: Vector2) -> Option<(f32, f32)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

/// Point-in-polygon (even-odd) for the rectangle-centre test of the Intersect box mode.
fn point_in_poly(p: Vector2, poly: &[Vector2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Where a click lands: world point, terrain cell (if on terrain) and the hit surface normal.
#[derive(Clone, Copy)]
struct Placement {
    dot: Vector3,
    cell: Option<Vector2i>,
    /// Drawing frame (y = up of the new shape): identity, or the frame of the builder face under
    /// the cursor with *Draw on surface*.
    frame: Basis,
}

/// Frame whose y axis is `n` (shapes drawn on a face stand along its normal).
pub(super) fn frame_from_upright(n: Vector3) -> Basis {
    let n = n.normalized();
    if n.dot(Vector3::UP) > 0.9995 {
        return Basis::IDENTITY;
    }
    let r = if n.x.abs() < 0.9005 {
        Vector3::RIGHT
    } else {
        Vector3::BACK
    };
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
    pocket: i64,
    #[init(val = Color::WHITE)]
    tint: Color,
    #[init(val = 0.2505)]
    grid_step: f32,
    #[init(val = true)]
    snap_grid: bool,
    #[init(val = true)]
    snap_cell: bool,
    #[init(val = 1.0)]
    span: f32,
    #[init(val = 0.2005)]
    inset: f32,
    #[init(val = 0.1005)]
    chamfer: f32,
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
    /// UV lock while dragging elements (the drag keeps the face's UVs in place).
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
    tex_coord: [f32; 5],
    #[init(val = true)]
    uv_world: bool,
    shape_drag: Option<ShapeDrag>,
    /// Wall tool: clicked polyline points (world) and the snapped cursor.
    wall_pts: Vec<Vector3>,
    wall_cursor: Option<Vector3>,
    coat: Option<PaintStroke>,
    /// Plain MeshInstance3D (or root CSG shape) selected: offered for conversion.
    convert_src: Option<Gd<Node3D>>,
    export_dialog: Option<Gd<godot::classes::EditorFileDialog>>,
    /// Material slot count shown by the palette (rebuilt when it changes).
    palette_slots: Option<Vec<i64>>,
    message: String,
    ui: Option<LayoutUi>,
    /// HUD content key last drawn (the overlay schedules a deferred HUD refresh on change).
    hud_key: String,
    /// Mesh data last seen by the builder (instance, revision, face count, vertex count):
    /// recorded at each own commit and on every history change, so an undo / redo that changed
    /// the topology clears the selection whose indices it invalidated (r4 #13).
    seen_mesh: Option<(InstanceId, u64, usize, usize)>,
    /// Inspector "Editable Mesh" section.
    inspector: Option<Gd<super::inspector::PixelBuilderInspector>>,
    gizmo: Option<Gd<MeshInstance3D>>,
    camera: Option<Gd<Camera3D>>,
    visible: bool,
    #[init(val = Prefs::default())]
    prefs: Prefs,
    prefs_panel: Option<prefs::Panel>,
    uv_window: Option<Gd<godot::classes::Window>>,
    uv_canvas: Option<Gd<PixelBuilderUvCanvas>>,
    /// The UV editor's content: a child of `uv_window` (MST layout) or of `uv_dock` (docked layout).
    uv_panel: Option<Gd<godot::classes::PanelContainer>>,
    /// The docked layout's "Mesh UV" bottom dock (added while that layout uses it).
    uv_dock: Option<Gd<godot::classes::EditorDock>>,
    /// Node carrying our `_edit_lock_` (hides Godot's transform gizmo in element modes).
    lock_target: Option<Gd<PixelBuilderMesh>>,
    /// Material drag-and-drop onto faces (drop catchers over the viewports, drag state).
    drop: objects::MaterialDrag,
    /// W10 settings (inset mode, subdivide segments, handle axes, trim walls).
    #[init(val = ui::ExtraOpts::default())]
    opts: ui::ExtraOpts,
    /// Paint color swatches (the vertex colour brush dock's palette).
    #[init(val = ui::SWATCHES)]
    swatches: [Color; 8],
    /// Insert-vertex modal (Edge mode): on, and the edge / point under the cursor.
    insert_vert: bool,
    insert_hover: Option<(Edge, Vector3)>,
    /// Draw shapes on the builder face under the cursor.
    surface_draw: bool,
    /// X-ray: picks and box selections include elements hidden behind faces.
    xray: bool,
    /// Frame of the wall / poly shape being clicked.
    #[init(val = Basis::IDENTITY)]
    wall_basis: Basis,
    /// "Builder" menu in the 3D toolbar, and Godot's MeshInstance3D "Mesh" menu hidden meanwhile.
    builder_menu: Option<Gd<godot::classes::MenuButton>>,
    hidden_mesh_menu: Option<Gd<Control>>,
    /// Options of the extended actions (strip fields).
    #[init(val = ui::XgOpts::default())]
    xg: ui::XgOpts,
    /// First vertex picked into the current vertex selection ("Collapse to first").
    active_vert: Option<u32>,
    /// Bezier Plane / Block tool state.
    bz: bezier::BezierTool,
    /// Poly shape with holes: the closed outline and the closed holes (world points).
    poly_outline: Option<Vec<Vector3>>,
    poly_holes: Vec<Vec<Vector3>>,
    /// Export options (popup) and the dialogs of the extended features.
    #[init(val = xg_io::ExportSettings::default())]
    export_opts: xg_io::ExportSettings,
    xg_dialogs: xg_io::Dialogs,
    /// Convert options (the dialog); the defaults reproduce the classic Convert.
    #[init(val = xg_io::classic_convert())]
    convert_opts: pt_core::build::convert::ImportOptions,
    /// Live adjust session (the Adjustment HUD), its panel and the one-shot debounce timer.
    adjust: Option<adjust::Adjust>,
    adjust_ui: Option<adjust::AdjustUi>,
    adjust_timer: Option<Gd<godot::classes::Timer>>,
    /// The session's own (re-)run is in progress: edits are not committed.
    adjust_running: bool,
    /// The current re-run is the coarse bevel preview.
    adjust_coarse: bool,
    /// `debug_action`: run without opening an adjust session (commits at once).
    no_adjust: bool,
    /// Run without the alternate layout's scene-graph confirmation (`debug_action`, a confirmed run).
    no_confirm: bool,
    /// Test hook `debug_auto_confirm`: confirmations accept at once (headless runs).
    auto_confirm: bool,
    /// Title of the last confirmation asked (test hook `debug_confirmation`).
    last_confirmation: String,
    /// Last bevel's status line (skipped / clamped / may overlap).
    bevel_status: String,
    /// Right-click menu, the state it was built for and a recorded right press.
    ctx_menu: Option<Gd<godot::classes::PopupMenu>>,
    ctx_state: Option<(Mode, Option<InstanceId>, usize)>,
    ctx_press: Option<Vector2>,
    /// Mouse travel since the recorded right press (relative motion: free-look captures it).
    ctx_travel: f32,
    /// A right click whose menu opens on the next idle frame (`open_context_menu_pending`).
    ctx_pending: Option<Vector2>,
    /// Builder shortcuts registered in the editor settings (index-aligned with
    /// `shortcuts::ENTRIES`).
    shortcut_table: Vec<Option<Gd<godot::classes::Shortcut>>>,
    /// Smoothing tint / normals overlay (`attr_overlay.rs`), the state it was built for and a
    /// rebuild counter (tests: rebuilt on change only).
    attr_node: Option<Gd<MeshInstance3D>>,
    attr_key: Option<attr_overlay::AttrKey>,
    attr_rebuilds: i64,
    attr_counts: (i64, i64),
    /// The data whose `changed` (deferred) refreshes the overlay (undo / redo, scripts).
    attr_data: Option<Gd<PixelBuilderMeshData>>,
    /// Inspector stats key (target, entity type, Live UV2): a change refreshes the section.
    inspector_key: Option<(i64, i32, bool)>,
    /// MST palette smoothing chips: the (groups, current group) they were built for.
    chip_key: Option<(Vec<(u16, usize)>, u16)>,
    /// Shared palette Load / Save file dialog (both layouts) and whether it saves.
    palette_dialog: Option<Gd<godot::classes::EditorFileDialog>>,
    palette_dialog_save: bool,
    /// Edge / vertex under the cursor (Edge / Vertex mode hover highlight).
    hover_edge: Option<Edge>,
    hover_vert: Option<u32>,
    feedback: Feedback,
    base: Base<EditorPlugin>,
}

/// Meta set next to `_edit_lock_` so only our own lock is ever removed.
pub(super) const PLUGIN_LOCK_META: &str = "_pt_builder_lock";
const EDIT_LOCK_META: &str = "_edit_lock_";

#[godot_api]
impl IEditorPlugin for PixelBuilderPlugin {
    fn enter_tree(&mut self) {
        self.prefs = Prefs::ingest();
        self.apply_prefs(true);
        // Shortcuts first: the toolbar tooltips show their keys.
        self.shortcut_table = super::shortcuts::enroll();
        super::layout::enroll();
        if let Some(mut es) = EditorInterface::singleton().get_editor_settings() {
            let cb = self.to_gd().callable("on_editor_settings_changed");
            if !es.is_connected("settings_changed", &cb) {
                // Deferred: `set_setting` from one of our own `#[func]`s may emit it at once, and
                // a synchronous call into the bound plugin would panic.
                es.connect_flags(
                    "settings_changed",
                    &cb,
                    godot::classes::object::ConnectFlags::DEFERRED,
                );
            }
        }
        self.add_inspector();
        // An undo / redo / other action recorded while an adjust session is open ends it
        // (deferred: our own commits emit it while the plugin is bound).
        if let Some(mut ur) = self.base().get_undo_redo() {
            let cb = self.to_gd().callable("on_history_changed");
            if !ur.is_connected("version_changed", &cb) {
                ur.connect_flags(
                    "version_changed",
                    &cb,
                    godot::classes::object::ConnectFlags::DEFERRED,
                );
            }
        }
        self.build_layout(super::layout::Layout::ingest());
        self.sync_ui();
    }

    fn exit_tree(&mut self) {
        self.adjust_commit();
        self.free_adjust_panel();
        if let Some(mut ur) = self.base().get_undo_redo() {
            let cb = self.to_gd().callable("on_history_changed");
            if ur.is_connected("version_changed", &cb) {
                ur.disconnect("version_changed", &cb);
            }
        }
        if let Some(mut es) = EditorInterface::singleton().get_editor_settings() {
            let cb = self.to_gd().callable("on_editor_settings_changed");
            if es.is_connected("settings_changed", &cb) {
                es.disconnect("settings_changed", &cb);
            }
        }
        self.remove_drop_targets();
        self.remove_menus();
        self.drop_gizmo();
        self.set_gizmo_lock(None);
        // The dock first (it may hold the UV panel), then the window (holding it otherwise).
        if let Some(mut d) = self.uv_dock.take()
            && d.is_instance_valid()
        {
            self.base_mut().remove_dock(&d);
            d.queue_free();
        }
        self.uv_panel = None;
        for w in [
            self.uv_window
                .take()
                .map(|w| w.upcast::<godot::classes::Node>()),
            self.prefs_panel
                .take()
                .map(|p| p.popup.upcast::<godot::classes::Node>()),
            self.ctx_menu
                .take()
                .map(|m| m.upcast::<godot::classes::Node>()),
        ]
        .into_iter()
        .flatten()
        {
            if w.is_instance_valid() {
                w.free();
            }
        }
        self.uv_canvas = None;
        if let Some(d) = self.palette_dialog.take()
            && d.is_instance_valid()
        {
            d.free();
        }
        self.teardown_layout(true);
        self.remove_inspector();
        if let Some(d) = self.export_dialog.take()
            && d.is_instance_valid()
        {
            d.free();
        }
        self.xg_dialogs.free_all();
    }

    fn get_plugin_name(&self) -> GString {
        "Pixel Builder".into()
    }

    fn handles(&self, entity: Gd<Object>) -> bool {
        // Nodes with their own Pixel World panel (PixelObject, PixelWater, effects; some are
        // or contain MeshInstance3Ds) belong to that plugin: two plugins handling the
        // same node stacked two toolbars + bottom strips and the "Pixel Builder" overlay.
        if crate::editor::pixel_handles(&entity) {
            return false;
        }
        // A Ctrl+click multi-selection is edited as a `MultiNodeEdit`: keep editing a builder mesh
        // in it (Merge objects / Export › Combine need the target and the other selected meshes).
        if entity.is_class("MultiNodeEdit") {
            return multi_target(None).is_some();
        }
        // CSG shapes are offered for Convert (bake to a builder mesh); only the root of a CSG
        // tree has a baked mesh.
        let csg_root = entity.is_class("CSGShape3D")
            && entity
                .clone()
                .try_cast::<godot::classes::CsgShape3D>()
                .is_ok_and(|c| c.is_root_shape());
        entity.clone().try_cast::<PixelBuilderMesh>().is_ok()
            || entity.is_class("MeshInstance3D")
            || csg_root
    }

    fn edit(&mut self, entity: Option<Gd<Object>>) {
        // Re-edit of the mesh already being edited (e.g. the selection nudge after the gizmo
        // lock changed): keep the element selection.
        // A target / gizmo freed behind the plugin's back (a script freeing the edited node).
        self.drop_freed();
        // A multi-selection: the mesh being edited stays the target if it is selected, else the
        // first selected builder mesh.
        let object = match entity {
            Some(o) if o.is_class("MultiNodeEdit") => {
                multi_target(self.target.as_ref()).map(|t| t.upcast::<Object>())
            }
            o => o,
        };
        if let (Some(o), Some(t)) = (&object, &self.target)
            && o.instance_id() == t.instance_id()
        {
            self.sync_ui();
            return;
        }
        // Another node: the open adjustment is kept as one undo step.
        self.adjust_commit();
        self.ctx_press = None;
        self.ctx_pending = None;
        self.cancel_drag();
        self.drop_gizmo();
        self.purge_picked();
        self.cancel_shape();
        self.coat = None;
        self.vp.sweep = None;
        self.knife = Knife::default();
        self.reshape = false;
        self.bz.revert();
        self.poly_outline = None;
        self.poly_holes.clear();
        self.active_vert = None;
        self.target = None;
        self.host = None;
        self.convert_src = None;
        let Some(o) = object else {
            self.sync_gizmo_lock();
            return;
        };
        // A newly selected node starts as an object: Object mode + Select, so Godot's own
        // move / rotate / scale gizmo shows and takes the clicks. An element mode or a placing
        // tool left over from the previous mesh would lock the gizmo (element modes) or swallow
        // every left click (Shape / Place / Paint ...); the user switches again on purpose.
        self.mode = Mode::Object;
        self.tool = Tool::Select;
        match o.clone().try_cast::<PixelBuilderMesh>() {
            Ok(t) => {
                self.target = Some(t);
                self.unshare_data();
                self.secure_handlebar();
            }
            Err(o) => {
                if o.is_class("MeshInstance3D") || o.is_class("CSGShape3D") {
                    self.convert_src = o.clone().try_cast::<Node3D>().ok();
                }
                self.host = o.try_cast::<Node3D>().ok();
            }
        }
        self.palette_slots = None;
        self.terrain = find_terrain();
        self.sync_ui();
        self.redraw();
    }

    fn make_visible(&mut self, shown: bool) {
        self.drop_freed();
        self.visible = shown;
        let has_target = self.target.is_some();
        if let Some(u) = &mut self.ui {
            u.assign_shown(shown, has_target);
        }
        if let Some(g) = &mut self.gizmo {
            g.set_visible(shown);
        }
        self.refresh_attr_overlay();
        self.sync_menus(shown);
        if !shown {
            self.adjust_commit();
            self.cancel_drag();
            self.cancel_shape();
            self.bz.cancel_live();
        }
        self.sync_gizmo_lock();
        // The layouts' visibility rules read `visible` (alternate toolbar buttons, HUD).
        self.sync_ui();
    }

    fn forward_3d_gui_input(
        &mut self,
        lens: Option<Gd<Camera3D>>,
        happening: Option<Gd<InputEvent>>,
    ) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let stop = AfterGuiInput::STOP.ord();
        let (Some(camera), Some(event)) = (lens, happening) else {
            return pass;
        };
        if !self.visible || (self.target.is_none() && self.host.is_none()) {
            return pass;
        }
        if self
            .camera
            .as_ref()
            .is_none_or(|c| c.instance_id() != camera.instance_id())
        {
            // Another viewport has the input now: the HUD follows it.
            if let Some(LayoutUi::Modeler(x)) = &mut self.ui
                && let Some(i) = super::modeler_ui::hud::viewport_of(&camera)
            {
                x.hud.set_viewport(i);
            }
        }
        self.camera = Some(camera.clone());
        // A left click in the viewport ends an open adjustment (kept as one undo step).
        if self.adjust.is_some()
            && let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>()
            && mb.is_pressed()
            && mb.get_button_index() == MouseButton::LEFT
        {
            self.adjust_commit();
        }
        let editing =
            self.target.is_some() && self.mode != Mode::Object && self.tool == Tool::Select;
        let placing = self.tool != Tool::Select;

        if self.tool == Tool::Knife
            && self.target.is_some()
            && let Some(r) = self.knife_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Bezier
            && let Some(r) = self.bezier_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Shape
            && self.spec.sort.is_polyline()
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
            if !self.on_key(&key) {
                return pass;
            }
            // STOP alone does not end a key event's propagation: the editor's own shortcuts
            // (`shortcut_input`) still ran, so `H` (cycle mode) also ran Godot's
            // "Toggle Selected Node(s) Visibility" and hid the mesh being edited every other press.
            if let Some(mut vp) = self.base().get_viewport() {
                vp.set_input_as_handled();
            }
            return stop;
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
            // A right press that moved (free-look captures the mouse, so the position stays put:
            // count the relative motion too) is a look-around, not a menu click.
            if let Some(p) = self.ctx_press {
                self.ctx_travel += mm.get_relative().length();
                if p.distance_to(pos) > context_menu::CLICK_SLOP
                    || self.ctx_travel > context_menu::CLICK_SLOP
                {
                    self.ctx_press = None;
                }
            }
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
            if button == MouseButton::RIGHT
                && mb.is_pressed()
                && (self.drag.is_some() || self.marquee.is_some())
            {
                self.cancel_drag();
                self.marquee = None;
                self.redraw();
                return stop;
            }
            // Context menu (`handle_context_click`): a right press without modifiers on a
            // selected element is recorded, its release (within the slop) opens the menu. Both
            // pass through, so a right-drag still free-looks (Godot ends free-look on the
            // release); the menu opens deferred, after Godot handled the release.
            if button == MouseButton::RIGHT
                && !mb.is_shift_pressed()
                && !mb.is_alt_pressed()
                && !mb.is_ctrl_pressed()
                && !mb.is_meta_pressed()
            {
                if mb.is_pressed() {
                    self.ctx_press = None;
                    self.ctx_travel = 0.0;
                    if self.context_menu_available()
                        && self.menu_ctx_addressees_picked(&camera, pos)
                    {
                        self.ctx_press = Some(pos);
                    }
                } else if let Some(p) = self.ctx_press.take() {
                    self.ctx_pending = Some(p);
                    self.to_gd().call_deferred("open_context_menu_pending", &[]);
                }
                return pass;
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
                        Tool::Select
                        | Tool::Paint
                        | Tool::Knife
                        | Tool::VertexPaint
                        | Tool::Bezier => {}
                    }
                }
                return stop;
            }
            if !editing {
                return pass;
            }
            if mb.is_pressed() {
                if mb.is_double_click()
                    && self.drag.is_none()
                    && self.double_click(&camera, pos, shift, mb.is_alt_pressed())
                {
                    self.marquee = None;
                } else if let Some(h) = self.gizmo_hit(&camera, pos) {
                    self.begin_drag(&camera, pos, h, shift);
                } else if !self.choose(&camera, pos, shift) {
                    self.marquee = Some(Marquee {
                        start_pt: pos,
                        to: pos,
                        additive: shift,
                    });
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
        self.drop_freed();
        self.ensure_drop_target(&overlay);
        // The overlay call has no camera: use the one from the last input event, else viewport 0's.
        let cam = self
            .camera
            .clone()
            .filter(|c| c.is_instance_valid())
            .or_else(|| {
                EditorInterface::singleton()
                    .get_editor_viewport_3d_ex()
                    .idx(0)
                    .done()
                    .and_then(|v| v.get_camera_3d())
            });
        let Some(cam) = cam else { return };
        let proj = |w: Vector3| (!cam.is_position_behind(w)).then(|| cam.unproject_position(w));
        // Vertices (vertex mode), selected ones highlighted.
        if let (Some(t), Mode::Vertex) = (&self.target, self.mode)
            && let Some(d) = t.bind().contents()
        {
            let xf = t.get_global_transform();
            let d = d.bind();
            for (i, p) in d.cage().positions.iter().enumerate() {
                if let Some(s) = proj(xf * from_p(*p)) {
                    let sel = self.sel_verts.contains(&(i as u32));
                    let hov = self.hover_vert == Some(i as u32);
                    let r = self.prefs.vertex_size * 0.5 + if sel || hov { 1.0 } else { 0.0 };
                    overlay.draw_rect(
                        Rect2::new(s - Vector2::new(r, r), Vector2::new(r * 2.0, r * 2.0)),
                        if sel {
                            self.prefs.color_vertex_selected
                        } else {
                            Color::from_rgba(0.1005, 0.1005, 0.1005, 0.9005)
                        },
                    );
                    if !sel {
                        let k = (r - 1.0).max(1.0);
                        overlay.draw_rect(
                            Rect2::new(s - Vector2::new(k, k), Vector2::new(k * 2.0, k * 2.0)),
                            if hov {
                                self.prefs.color_vertex_hover
                            } else {
                                self.prefs.color_vertex
                            },
                        );
                    } else if hov {
                        overlay
                            .draw_rect_ex(
                                Rect2::new(
                                    s - Vector2::new(r + 1.5005, r + 1.5005),
                                    Vector2::new(r * 2.0 + 3.0, r * 2.0 + 3.0),
                                ),
                                self.prefs.color_vertex_hover,
                            )
                            .filled(false)
                            .width(1.5005)
                            .done();
                    }
                }
            }
        }
        // Move / rotate / scale handles, the knife and the vertex paint brush.
        self.draw_gizmo(&mut overlay, &cam);
        self.draw_insert_hover(&mut overlay, &cam);
        self.draw_knife(&mut overlay, &cam);
        self.draw_vpaint(&mut overlay, &cam);
        self.draw_bezier(&mut overlay, &cam);
        self.draw_poly_holes(&mut overlay, &cam);
        if self.soft
            && self.drag.is_some()
            && let Some(p) = self.drag.as_ref().map(|d| d.fulcrum)
            && !cam.is_position_behind(p)
        {
            // Soft selection radius around the pivot.
            let s = cam.unproject_position(p);
            let r = cam
                .unproject_position(p + cam.get_global_transform().basis.col_a() * self.soft_radius)
                .distance_to(s);
            overlay
                .draw_arc_ex(
                    s,
                    r,
                    0.0,
                    std::f32::consts::TAU,
                    64,
                    Color::from_rgba(1.0, 0.7505, 0.3005, 0.6005),
                )
                .width(1.0)
                .done();
        }
        // Placement preview: the shape's edges at the hover point.
        let preview = if !self.wall_pts.is_empty() || self.poly_outline.is_some() {
            self.wall_preview()
        } else if self.shape_drag.is_some() {
            self.dragged_shape()
        } else {
            self.hover_place
                .filter(|_| self.tool == Tool::Shape)
                .map(|p| (p.dot, self.form_cage(p.cell)))
        };
        let frame = self.draw_basis();
        if self.tool == Tool::Shape
            && let Some((origin, m)) = preview
        {
            let col = if self.shape_drag.is_some()
                || !self.wall_pts.is_empty()
                || self.poly_outline.is_some()
            {
                Color::from_rgba(1.0, 0.7505, 0.3005, 0.9505)
            } else {
                Color::from_rgba(0.5, 0.9005, 1.0, 0.8505)
            };
            for e in m.ridges() {
                let (a, b) = (
                    origin + frame * from_p(m.positions[e.0 as usize]),
                    origin + frame * from_p(m.positions[e.1 as usize]),
                );
                if let (Some(sa), Some(sb)) = (proj(a), proj(b)) {
                    overlay.draw_line_ex(sa, sb, col).width(1.5005).done();
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
            let ly = (xf.affine_inverse() * p.dot).y;
            let pts: Vec<Vector2> = cfg
                .cell_corners(to_iv(c))
                .iter()
                .filter_map(|k| proj(xf * Vector3::new(k.x, ly, k.y)))
                .collect();
            for i in 0..pts.len() {
                overlay
                    .draw_line_ex(pts[i], pts[(i + 1) % pts.len()], ORANGE)
                    .width(2.0)
                    .done();
            }
        }
        if let Some(m) = &self.marquee {
            let r = Rect2::new(m.start_pt, Vector2::ZERO).expand(m.to);
            overlay.draw_rect(r, Color::from_rgba(0.4005, 0.7005, 1.0, 0.1505));
            overlay
                .draw_rect_ex(r, Color::from_rgba(0.4005, 0.7005, 1.0, 0.9005))
                .filled(false)
                .done();
        }
        // HUD: refreshed (deferred) when the tool status / message changed.
        self.check_modeler_hud();
        // Status text.
        let Some(font) = overlay.get_theme_default_font() else {
            return;
        };
        let mut lines = vec![format!(
            "Pixel Builder - {} mode, {}",
            self.mode.moniker(),
            self.tool.moniker()
        )];
        // Show scene info (counts cached by `redraw`, on change only).
        if let Some(info) = &self.feedback.facts {
            lines.push(info.clone());
        }
        // Show dimensions: the size label above the selection's box.
        if let Some((anchor, text)) = &self.feedback.dims
            && let Some(s) = proj(*anchor)
        {
            let size = font.get_string_size(text.as_str());
            let p = s + Vector2::new(-size.x * 0.5, -10.0);
            overlay.draw_rect(
                Rect2::new(
                    p - Vector2::new(4.0, size.y - 2.0),
                    size + Vector2::new(8.0, 4.0),
                ),
                Color::from_rgba(0.0, 0.0, 0.0, 0.6005),
            );
            overlay
                .draw_string_ex(&font, p, text.as_str())
                .modulate(Color::from_rgb(1.0, 0.8505, 0.5505))
                .done();
        }
        lines.extend(self.tool_lines());
        if let Some(d) = &self.drag
            && d.rot.is_none()
            && d.magnify.is_none()
        {
            let v = d.last_delta;
            lines.push(format!("move ({:.2}, {:.2}, {:.2})", v.x, v.y, v.z));
        }
        if !self.message.is_empty() {
            lines.push(self.message.clone());
        }
        for (i, l) in lines.iter().enumerate() {
            let pos = Vector2::new(14.0, 64.0 + 18.0 * i as f32);
            overlay
                .draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), l.as_str())
                .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8005))
                .done();
            overlay
                .draw_string_ex(&font, pos, l.as_str())
                .modulate(Color::from_rgb(0.8505, 0.9505, 1.0))
                .done();
        }
    }

    /// Undo / redo with an open adjust session (R1 review): `_input` runs before any control
    /// or shortcut sees the key, wherever the focus is, so the adjusted action is committed as
    /// one undo step first and Godot's undo then takes all of it back (redo: nothing is lost).
    /// The key is never consumed.
    ///
    /// A mouse press outside the 3D main screen commits it the same way (R3 review): Edit › Undo /
    /// Redo or the History dock clicked with the mouse then undo the whole adjusted action instead
    /// of dropping it. The main screen is left alone: its viewports hold the adjust panel and end
    /// the session themselves, and the builder's strip / palette / toolbar there drive it.
    fn input(&mut self, happening: Gd<InputEvent>) {
        if self.adjust.is_none() {
            return;
        }
        if let Ok(mb) = happening.clone().try_cast::<InputEventMouseButton>() {
            if mb.is_pressed()
                && matches!(
                    mb.get_button_index(),
                    MouseButton::LEFT | MouseButton::RIGHT
                )
                && !in_main_screen(mb.get_global_position())
            {
                self.adjust_commit();
                self.sync_ui();
                self.redraw();
            }
            return;
        }
        let Ok(k) = happening.try_cast::<InputEventKey>() else {
            return;
        };
        if is_undo_redo_key(&k) {
            self.adjust_commit();
            self.sync_ui();
            self.redraw();
        }
    }

    /// Enter / Esc for the adjust panel when the viewport does not have the focus (a panel
    /// field took it): keys no control used.
    fn shortcut_input(&mut self, happening: Gd<InputEvent>) {
        if self.adjust.is_none()
            || !self
                .adjust_ui
                .as_ref()
                .is_some_and(|u| u.panel.is_instance_valid() && u.panel.is_visible_in_tree())
        {
            return;
        }
        let Ok(k) = happening.try_cast::<InputEventKey>() else {
            return;
        };
        if !k.is_pressed()
            || k.is_echo()
            || k.is_ctrl_pressed()
            || k.is_alt_pressed()
            || k.is_meta_pressed()
        {
            return;
        }
        match k.get_keycode() {
            Key::ENTER | Key::KP_ENTER => {
                self.adjust_commit();
                self.sync_ui();
                self.redraw();
            }
            Key::ESCAPE => self.adjust_cancel(),
            _ => return,
        }
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
    }

    fn save_external_data(&mut self) {
        // The gizmo lock never reaches the scene file: `PixelBuilderMesh` strips it on
        // EDITOR_PRE_SAVE and restores it on EDITOR_POST_SAVE.
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        for n in root
            .find_children_ex("*")
            .type_("PixelBuilderMesh")
            .owned(false)
            .done()
            .iter_shared()
        {
            let Ok(res) = n.get("contents").try_to::<Gd<godot::classes::Resource>>() else {
                continue;
            };
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
    root.find_children_ex("*")
        .type_("MarchingSquaresTerrain")
        .owned(false)
        .done()
        .iter_shared()
        .find_map(|n| n.try_cast().ok())
}

fn from_v2(v: pt_core::math::Vec2) -> Vector2 {
    Vector2::new(v.x, v.y)
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
    /// Status lines of the active drawing tool (the overlay text; the HUD's detail).
    pub(super) fn tool_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.tool == Tool::Shape && self.spec.sort.is_polyline() {
            lines.push(format!(
                "Wall  {} points  height {:.2}  thick {:.2}  (click points, click the first to close, Enter / double-click finishes)",
                self.wall_pts.len(),
                self.spec.size.y,
                self.spec.thickness
            ));
        } else if self.tool == Tool::Shape {
            match self.dragged_shape().and_then(|(_, m)| m.extent()) {
                Some((lo, hi)) => lines.push(format!(
                    "{}  {:.2} x {:.2} x {:.2}  {}",
                    self.spec.sort.moniker(),
                    hi.x - lo.x,
                    hi.y - lo.y,
                    hi.z - lo.z,
                    if self.shape_drag.as_ref().is_some_and(|d| d.stage == 1) {
                        "move for height, click to finish"
                    } else {
                        "drag the footprint"
                    }
                )),
                None => {
                    let s = &self.spec;
                    lines.push(format!(
                        "{}  {:.2} x {:.2} x {:.2}  slot {}  (drag to draw, click to stamp)",
                        s.sort.moniker(),
                        s.size.x,
                        s.size.y,
                        s.size.z,
                        self.pocket
                    ));
                }
            }
        }
        if self.tool == Tool::Bezier {
            lines.push(self.curve_situation());
        }
        if self.poly_outline.is_some() && self.tool == Tool::Shape {
            lines.push(format!(
                "Poly shape: outline closed, {} hole{}  (click hole points, click the first to close a hole, Enter creates, Esc cancels)",
                self.poly_holes.len(),
                if self.poly_holes.len() == 1 { "" } else { "s" }
            ));
        }
        lines
    }

    fn values_with<'a>(&self, spec: &'a ShapeSpec) -> Values<'a> {
        Values {
            spec,
            pocket: self.pocket,
            grid_step: self.grid_step,
            span: self.span,
            inset: self.inset,
            chamfer: self.chamfer,
            cuts: self.cuts,
            tint: self.tint,
            tex_coord: self.tex_coord,
            uv_world: self.uv_world,
            bevel_segs: self.bevel_segs,
            soft_radius: self.soft_radius,
            falloff: self.falloff,
            rot_snap: self.rot_snap,
            mirror_axis: self.mirror_axis,
            vp_fill: self.vp.flood,
            vp_channel: self.vp.channel,
            vp_radius: self.vp.radius,
            vp_strength: self.vp.strength,
            vp_softness: self.vp.softness,
            vp_alpha: self.vp.alpha,
            opts: self.opts,
            swatches: self.swatches,
            xg: self.xg,
            bz: self.bz.values(),
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
            reshape: (self.reshape || (self.bz.editing.is_some() && self.tool == Tool::Bezier))
                && has_target,
            has_target,
            has_spec: self.contents().is_some_and(|d| {
                d.bind().shape_spec().is_some() || d.bind().bezier_recipe().is_some()
            }),
            can_convert: self.convert_src.is_some(),
            insert_vert: self.insert_vert,
            surface_draw: self.surface_draw,
            xray: self.xray,
            entity: self
                .target
                .as_ref()
                .map(|t| t.get("entity_type").try_to::<i32>().unwrap_or(0))
                .unwrap_or(0),
            bz_knot: self.bz.selected.is_some(),
            ..self.modeler_condition()
        };
        let spec = self.spec;
        let v = self.values_with(&spec);
        let visible = self.visible;
        if let Some(LayoutUi::Modeler(_)) = &self.ui {
            self.sync_modeler(&v, &st);
        }
        if let Some(LayoutUi::Mst(u)) = &mut self.ui {
            u.sync(&v, &st);
            u.palette.set_visible(visible && has_target);
            let what = if self.target.is_some() {
                "Editing builder mesh"
            } else if self.convert_src.is_some() {
                "Mesh / CSG selected: Convert makes it an editable builder mesh; the shape tool draws new ones"
            } else if self.host.is_some() {
                "Draw shape tool: drag on the terrain to add builder meshes here"
            } else {
                ""
            };
            u.facts.set_text(what);
        }
        self.sync_gizmo_lock();
        self.tune_tex_coord_studio();
        self.refresh_inspector_stats();
        self.base().update_overlays();
    }

    /// Node state the inspector section shows but the data's `changed` does not report (entity
    /// type, Live UV2): refresh it (deferred) when that changed.
    fn refresh_inspector_stats(&mut self) {
        let key = self
            .target
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .map(|t| {
                (
                    t.instance_id().to_i64(),
                    t.get("entity_type").try_to::<i32>().unwrap_or(0),
                    t.get("live_uv2").try_to::<bool>().unwrap_or(false),
                )
            });
        if key == self.inspector_key {
            return;
        }
        self.inspector_key = key;
        // Check validity before cloning: the inspector frees the section on every re-parse.
        let stats = self.inspector.as_ref().and_then(|i| {
            i.bind()
                .stats
                .as_ref()
                .filter(|s| s.is_instance_valid())
                .cloned()
        });
        if let Some(mut s) = stats {
            s.call_deferred("renew", &[]);
        }
    }

    /// Forgets nodes freed while the plugin held them (the edited mesh, its gizmo, the convert
    /// source, the Bezier re-edit target).
    fn drop_freed(&mut self) {
        if self.target.as_ref().is_some_and(|t| !t.is_instance_valid()) {
            self.target = None;
            self.purge_picked();
            self.drag = None;
        }
        if self.gizmo.as_ref().is_some_and(|g| !g.is_instance_valid()) {
            self.gizmo = None;
        }
        if self.host.as_ref().is_some_and(|h| !h.is_instance_valid()) {
            self.host = None;
        }
        if self
            .convert_src
            .as_ref()
            .is_some_and(|c| !c.is_instance_valid())
        {
            self.convert_src = None;
        }
        if self
            .lock_target
            .as_ref()
            .is_some_and(|t| !t.is_instance_valid())
        {
            self.lock_target = None;
        }
        if self
            .bz
            .editing
            .as_ref()
            .is_some_and(|t| !t.is_instance_valid())
        {
            self.bz.editing = None;
        }
    }

    fn contents(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.target.as_ref().and_then(|t| t.bind().contents())
    }

    fn purge_picked(&mut self) {
        self.sel_faces.clear();
        self.sel_edges.clear();
        self.sel_verts.clear();
        self.hover_face = None;
        self.hover_edge = None;
        self.hover_vert = None;
    }

    /// Godot's "Duplicate" shares the data resource; give this node its own copy before editing.
    fn unshare_data(&mut self) {
        let (Some(t), Some(d)) = (self.target.clone(), self.contents()) else {
            return;
        };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let id = d.instance_id();
        let shared = root
            .find_children_ex("*")
            .type_("PixelBuilderMesh")
            .owned(false)
            .done()
            .iter_shared()
            .filter(|n| n.instance_id() != t.instance_id())
            .any(|n| {
                n.get("contents")
                    .try_to::<Gd<Object>>()
                    .is_ok_and(|o| o.instance_id() == id)
            });
        if shared
            && let Some(copy) = d
                .duplicate_resource()
                .try_cast::<PixelBuilderMeshData>()
                .ok()
        {
            let mut t = t;
            t.bind_mut().set_data(Some(copy));
        }
    }

    fn xform(&self) -> Transform3D {
        self.target
            .as_ref()
            .map(|t| t.get_global_transform())
            .unwrap_or(Transform3D::IDENTITY)
    }

    /// Selected vertices of the current mode (local indices).
    fn selected_verts(&self, m: &BMesh) -> BTreeSet<u32> {
        match self.mode {
            Mode::Face => m.verts_of_faces(&self.sel_faces),
            Mode::Edge => m.verts_of_edges(&self.sel_edges),
            Mode::Vertex => self
                .sel_verts
                .iter()
                .copied()
                .filter(|v| (*v as usize) < m.positions.len())
                .collect(),
            Mode::Object => BTreeSet::new(),
        }
    }

    fn fulcrum(&self) -> Option<Vector3> {
        let d = self.contents()?;
        let d = d.bind();
        let m = d.cage();
        let v = self.selected_verts(m);
        if v.is_empty() {
            return None;
        }
        let c = v
            .iter()
            .fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize])
            / v.len() as f32;
        Some(self.xform() * from_p(c))
    }

    /// Handle centre and the three axis tips on screen.
    fn handle_screen(&self, cam: &Gd<Camera3D>) -> Option<(Vector2, [Vector2; 3])> {
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return None;
        }
        let p = self
            .drag
            .as_ref()
            .map(|d| d.fulcrum + d.last_delta)
            .or_else(|| self.fulcrum())?;
        if cam.is_position_behind(p) {
            return None;
        }
        let k = if cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL {
            cam.get_size() * 0.1005
        } else {
            cam.get_global_position().distance_to(p) * 0.1205
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
            let t = if ab.length_squared() < 1.0 {
                0.0
            } else {
                ((pos - s0).dot(ab) / ab.length_squared()).clamp(0.0, 1.1505)
            };
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

    /// The face / edge / vertex of the current mode under `pos` (the same test a click uses:
    /// without X-ray only elements in front of the nearest face hit).
    fn element_at(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Elem> {
        let d = self.contents()?;
        let xf = self.xform();
        let (o, dir) = self.local_ray(cam, pos);
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| {
                (
                    to_v2(cam.unproject_position(w)),
                    cam.get_global_position().distance_to(w),
                )
            })
        };
        let mouse = to_v2(pos);
        let db = d.bind();
        let m = db.cage();
        let hit = pick::intersect_ray_closest(m, o, dir, true);
        let xray = self.xray;
        let front = |p: PVec3| xray || hit.is_none_or(|h| (p - o).magnitude() <= h.t + 0.0205);
        match self.mode {
            Mode::Face => hit.map(|h| Elem::Face(h.face)),
            Mode::Edge => pick::pick_closest_edge(m, &project, mouse, PICK_PX)
                .filter(|e| front((m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5))
                .map(Elem::Edge),
            Mode::Vertex => pick::pick_closest_vertex(m, &project, mouse, PICK_PX)
                .filter(|v| front(m.positions[*v as usize]))
                .map(Elem::Vert),
            Mode::Object => None,
        }
    }

    /// Click pick. Without Shift the element replaces the selection; with Shift it is added,
    /// subtracted or toggled (by the Shift selects preference).
    fn choose(&mut self, cam: &Gd<Camera3D>, pos: Vector2, additive: bool) -> bool {
        let shift_mode = self.prefs.shift_for(self.is_modeler(), false);
        let found = match self.element_at(cam, pos) {
            Some(Elem::Face(f)) => {
                apply_shift(
                    &mut self.sel_faces,
                    &BTreeSet::from([f]),
                    additive,
                    shift_mode,
                );
                true
            }
            Some(Elem::Edge(e)) => {
                apply_shift(
                    &mut self.sel_edges,
                    &BTreeSet::from([e]),
                    additive,
                    shift_mode,
                );
                true
            }
            Some(Elem::Vert(v)) => {
                apply_shift(
                    &mut self.sel_verts,
                    &BTreeSet::from([v]),
                    additive,
                    shift_mode,
                );
                // "Collapse to first": the first vertex picked into the selection.
                if !additive
                    || self
                        .active_vert
                        .is_none_or(|a| !self.sel_verts.contains(&a))
                {
                    self.active_vert = self.sel_verts.contains(&v).then_some(v);
                }
                true
            }
            None => false,
        };
        if !found && !additive {
            self.purge_picked();
        }
        found
    }

    fn marquee_select(&mut self, cam: &Gd<Camera3D>, mq: Marquee) {
        let Some(d) = self.contents() else { return };
        let xf = self.xform();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| (to_v2(cam.unproject_position(w)), 0.0))
        };
        let lo = v2(mq.start_pt.x.min(mq.to.x), mq.start_pt.y.min(mq.to.y));
        let hi = v2(mq.start_pt.x.max(mq.to.x), mq.start_pt.y.max(mq.to.y));
        if (hi - lo).magnitude() < 3.0 {
            return;
        }
        let db = d.bind();
        let m = db.cage();
        let verts = pick::vertices_in_rect(m, &project, lo, hi);
        let complete = self.prefs.rect_for(self.is_modeler()) == 1;
        // Marquee: without X-ray, elements behind other faces are skipped (the
        // vertex, the edge midpoint, the face centre; faces turned away too).
        let eye = to_p(xf.affine_inverse() * cam.get_global_position());
        let ortho = cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL;
        let view_dir = to_p(
            (xf.affine_inverse().basis * -cam.get_global_transform().basis.col_c()).normalized(),
        );
        let seen = |p: PVec3| {
            if self.xray {
                return true;
            }
            // Orthographic: look from far back along the view direction.
            let from = if ortho { p - view_dir * 1.0e4 } else { eye };
            !pick::point_hidden(m, from, p, true)
        };
        let scr = |p: PVec3| project(p).map(|s| from_v2(s.0));
        let (glo, ghi) = (from_v2(lo), from_v2(hi));
        let inside = |s: Vector2| s.x >= glo.x && s.x <= ghi.x && s.y >= glo.y && s.y <= ghi.y;
        // Complete takes elements with every point inside; Intersect
        // every element the box touches, judged visible by the samples of its inside part.
        let mut picked_f = BTreeSet::new();
        let mut picked_e = BTreeSet::new();
        let mut picked_v = BTreeSet::new();
        match self.mode {
            Mode::Face => {
                let cand: BTreeSet<usize> = if complete {
                    m.faces_within(&verts)
                } else {
                    (0..m.facets.len()).collect()
                };
                for f in cand {
                    let c = m.face_center(f);
                    let to_eye = if ortho { -view_dir } else { eye - c };
                    if !self.xray && m.face_normal(f).innerprod(to_eye) <= 0.0 {
                        continue;
                    }
                    let fv = &m.facets[f].verts;
                    let Some(poly) = fv
                        .iter()
                        .map(|v| scr(m.positions[*v as usize]))
                        .collect::<Option<Vec<_>>>()
                    else {
                        continue;
                    };
                    let mut samples: Vec<PVec3> = Vec::new();
                    if complete {
                        samples.push(c);
                    } else {
                        let centre = (glo + ghi) * 0.5;
                        if inside(
                            poly.iter().fold(Vector2::ZERO, |a, p| a + *p) / poly.len() as f32,
                        ) || point_in_poly(centre, &poly)
                        {
                            samples.push(c);
                        }
                        for (i, v) in fv.iter().enumerate() {
                            if inside(poly[i]) {
                                samples.push(m.positions[*v as usize]);
                            }
                            let j = (i + 1) % fv.len();
                            if let Some((t0, t1)) = trim_stretch_quad(poly[i], poly[j], glo, ghi) {
                                let (a, b) =
                                    (m.positions[*v as usize], m.positions[fv[j] as usize]);
                                samples.push(a + (b - a) * ((t0 + t1) * 0.5));
                            }
                        }
                        if point_in_poly(centre, &poly) {
                            // The box lies inside the face: sample under the box centre.
                            let (o, dir) = self.local_ray(cam, centre);
                            if let Some(h) =
                                pick::intersect_ray_closest(m, o, dir, true).filter(|h| h.face == f)
                            {
                                samples.push(h.dot);
                            }
                        }
                    }
                    if self.xray || samples.iter().any(|p| seen(*p)) {
                        if !samples.is_empty() {
                            picked_f.insert(f);
                        }
                    }
                }
            }
            Mode::Edge => {
                let cand: Vec<Edge> = if complete {
                    m.edges_within(&verts).into_iter().collect()
                } else {
                    m.ridges()
                };
                for e in cand {
                    let (a, b) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
                    let (Some(sa), Some(sb)) = (scr(a), scr(b)) else {
                        continue;
                    };
                    let t = if complete {
                        Some((0.0, 1.0))
                    } else {
                        trim_stretch_quad(sa, sb, glo, ghi)
                    };
                    let Some((t0, t1)) = t else { continue };
                    let samples = [
                        a + (b - a) * ((t0 + t1) * 0.5),
                        a + (b - a) * t0,
                        a + (b - a) * t1,
                    ];
                    if samples.iter().any(|p| seen(*p)) {
                        picked_e.insert(e);
                    }
                }
            }
            Mode::Vertex => {
                for v in verts {
                    if seen(m.positions[v as usize]) {
                        picked_v.insert(v);
                    }
                }
            }
            Mode::Object => {}
        }
        drop(db);
        let shift_mode = self.prefs.shift_for(self.is_modeler(), true);
        match self.mode {
            Mode::Face => apply_shift(&mut self.sel_faces, &picked_f, mq.additive, shift_mode),
            Mode::Edge => apply_shift(&mut self.sel_edges, &picked_e, mq.additive, shift_mode),
            Mode::Vertex => {
                apply_shift(&mut self.sel_verts, &picked_v, mq.additive, shift_mode);
                if self
                    .active_vert
                    .is_none_or(|a| !self.sel_verts.contains(&a))
                {
                    self.active_vert = self.sel_verts.iter().next().copied();
                }
            }
            Mode::Object => {}
        }
        if !mq.additive {
            self.hover_face = None;
        }
    }

    fn update_hover(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.tool == Tool::Select {
            // Hover highlight (faces tinted, edges / vertices in the hover colours): computed on
            // mouse motion only, redrawn only when the hovered element changes.
            let el = if self.target.is_some() && self.prefs.hover && self.mode != Mode::Object {
                self.element_at(cam, pos)
            } else {
                None
            };
            let (hf, he, hv) = match el {
                Some(Elem::Face(f)) => (Some(f), None, None),
                Some(Elem::Edge(e)) => (None, Some(e), None),
                Some(Elem::Vert(v)) => (None, None, Some(v)),
                None => (None, None, None),
            };
            if (hf, he, hv) != (self.hover_face, self.hover_edge, self.hover_vert) {
                let gizmo = he != self.hover_edge || hf != self.hover_face;
                self.hover_face = hf;
                self.hover_edge = he;
                self.hover_vert = hv;
                if gizmo {
                    self.redraw();
                } else {
                    self.base().update_overlays();
                }
            }
            return;
        }
        self.hover_place = self.locus(cam, pos);
        self.base().update_overlays();
    }

    /// Placement point under the mouse: nearest of builder meshes and terrain, else the y=0 plane.
    fn locus(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Placement> {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let mut best: Option<(f32, Vector3, bool)> = None;
        let mut face_normal: Option<Vector3> = None;
        if let Some(t) = &self.terrain {
            let xf = t.get_global_transform();
            let inv = xf.affine_inverse();
            if let Some(h) = t
                .bind()
                .local_raycast(inv * o, (inv.basis * d).normalized(), 20_000.0)
            {
                let w = xf * h;
                best = Some((o.distance_to(w), w, true));
            }
        }
        // Builder meshes (stacking), excluding the one being placed by the Place tool.
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
                if self.tool == Tool::Place
                    && self
                        .target
                        .as_ref()
                        .is_some_and(|t| t.instance_id() == b.instance_id())
                {
                    continue;
                }
                // The Bezier mesh being re-edited (its knots sit under its own surface).
                if self.tool == Tool::Bezier
                    && self
                        .bz
                        .editing
                        .as_ref()
                        .is_some_and(|t| t.instance_id() == b.instance_id())
                {
                    continue;
                }
                let Some(data) = b.bind().contents() else {
                    continue;
                };
                let xf = b.get_global_transform();
                let inv = xf.affine_inverse();
                if let Some(h) = pick::intersect_ray_closest(
                    data.bind().cage(),
                    to_p(inv * o),
                    to_p((inv.basis * d).normalized()),
                    true,
                ) {
                    let w = xf * from_p(h.dot);
                    let dist = o.distance_to(w);
                    if best.is_none_or(|bb| dist < bb.0) {
                        best = Some((dist, w, false));
                        face_normal = Some(
                            (xf.basis.inverse().transposed()
                                * from_p(data.bind().cage().face_normal(h.face)))
                            .normalized(),
                        );
                    }
                }
            }
        }
        if self.surface_draw
            && let (Some((_, w, false)), Some(n)) = (best, face_normal)
        {
            // Draw on surface: start exactly on the face, standing along its normal. Grid snap
            // snaps the start along the face plane's own axes (world-anchored lattice).
            let basis = frame_from_upright(n);
            let mut point = w;
            if self.snap_grid {
                let mut l = basis.inverse() * w;
                l.x = snap::snap_value(l.x, self.grid_step);
                l.z = snap::snap_value(l.z, self.grid_step);
                point = basis * l;
            }
            return Some(Placement {
                dot: point,
                cell: None,
                frame: basis,
            });
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
        Some(Placement {
            dot: point,
            cell,
            frame: Basis::IDENTITY,
        })
    }

    // ------------------------------------------------------------------ shapes

    /// The current shape as a mesh around its placement origin. Grid shapes follow the terrain
    /// grid (and the actual cell, which matters for triangle / offset layouts).
    fn form_cage(&self, cell: Option<Vector2i>) -> BMesh {
        let tcfg = self
            .terrain
            .as_ref()
            .map(|t| t.bind().store().config.clone())
            .filter(|c| c.grid.is_cell_mode());
        match (self.spec.sort.is_grid_shape(), &tcfg, cell) {
            (true, Some(cfg), Some(c)) => {
                let r = if self.spec.sort == ShapeKind::HexPrism {
                    0
                } else {
                    self.spec.rings.min(16) as i32
                };
                let cells = shapes::cluster_cells(cfg, to_iv(c), r);
                shapes::cells_prism(cfg, &cells, to_iv(c), self.spec.size.y)
            }
            _ => shapes::produce(&self.spec, tcfg.as_ref()),
        }
    }

    fn finish_shape(&self, mut m: BMesh) -> BMesh {
        let c = ui_color_bytes(self.tint);
        for f in &mut m.facets {
            f.stuff = self.pocket.clamp(0, 255) as u16;
            f.tint = c;
        }
        m
    }

    fn stamp_shape(&mut self, append: bool) {
        let Some(p) = self.hover_place else {
            self.message = "Nothing under the cursor".into();
            return;
        };
        let m = self.form_cage(p.cell);
        self.create_from_mesh(p, m, append);
    }

    /// Adds a generated mesh at `p`: appended to the selected mesh, or as a new builder node
    /// carrying the current shape recipe.
    fn create_from_mesh(&mut self, p: Placement, m: BMesh, append: bool) {
        let spec = self.spec;
        self.create_with_recipe(p, m, append, xg_io::Recipe::Shape(spec));
    }

    /// `create_from_mesh` with an explicit recipe (shape spec, Bezier recipe or none).
    fn create_with_recipe(&mut self, p: Placement, m: BMesh, append: bool, recipe: xg_io::Recipe) {
        if m.facets.is_empty() {
            // Empty shapes (a 0° torus, …) never become nodes.
            self.message = "The settings make an empty mesh: nothing created".into();
            return;
        }
        let m = self.finish_shape(m);
        if append && self.target.is_some() {
            let xf = self.xform();
            let inv = xf.affine_inverse();
            let mut local = m.clone();
            for q in &mut local.positions {
                *q = to_p(inv * (p.dot + p.frame * from_p(*q)));
            }
            self.apply_edit("Pixel Builder: Add shape", |mm, _| mm.append_tail(&local));
            return;
        }
        let parent: Option<Gd<Node>> = match (&self.host, &self.target) {
            (Some(h), _) => Some(h.clone().upcast()),
            (None, Some(t)) => t.get_parent(),
            _ => EditorInterface::singleton().get_edited_scene_root(),
        };
        let Some(parent) = parent else { return };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut data = PixelBuilderMeshData::new_gd();
        let name = match &recipe {
            xg_io::Recipe::Shape(s) => s.sort.moniker().to_string(),
            xg_io::Recipe::Bezier(r) => {
                if r.stature > 0.0 {
                    "Bezier block".into()
                } else {
                    "Bezier plane".into()
                }
            }
        };
        {
            let mut db = data.bind_mut();
            db.substitute(m);
            match &recipe {
                xg_io::Recipe::Shape(s) => db.set_shape_spec(s),
                xg_io::Recipe::Bezier(r) => db.set_bezier_recipe(r),
            }
        }
        // New meshes drawn while another is selected share its material slots.
        if let Some(src) = self.contents() {
            let mats = src.bind().stuff_pockets().duplicate_shallow();
            data.set("materials", &mats.to_variant());
        }
        node.bind_mut().set_data(Some(data));
        node.set_name(name.replace(' ', "").as_str());
        if let Some(c) = p.cell {
            node.set("anchor_cell", &c.to_variant());
            node.set("anchored", &true.to_variant());
        }
        let parent_xf = parent
            .clone()
            .try_cast::<Node3D>()
            .map(|n| n.get_global_transform())
            .unwrap_or(Transform3D::IDENTITY);
        node.set_transform(parent_xf.affine_inverse() * Transform3D::new(p.frame, p.dot));
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(&format!("Pixel Builder: Create {name}"));
        ur.add_do_method(
            &parent,
            "add_child",
            &[node.to_variant(), true.to_variant()],
        );
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.commit_action();
        self.message = format!("Created {name}");
        // The new sibling becomes the edited mesh (Scene tree, Inspector and builder target), so
        // the next W / B acts on it. Deferred: the selection change re-enters this plugin
        // (`handles` / `edit`) and this runs inside `forward_3d_gui_input`.
        if let Some(sel) = EditorInterface::singleton().get_selection() {
            let mut sel = sel.upcast::<Object>();
            sel.call_deferred("clear", &[]);
            sel.call_deferred("add_node", &[node.to_variant()]);
        }
    }

    fn place_target(&mut self) {
        let (Some(t), Some(terrain), Some(p)) =
            (self.target.clone(), self.terrain.clone(), self.hover_place)
        else {
            self.message = "Place needs a selected mesh and a terrain under the cursor".into();
            return;
        };
        let Some(cell) = p
            .cell
            .or_else(|| Some(super::node::world_cell(&terrain, p.dot)))
        else {
            return;
        };
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action("Pixel Builder: Place on cell");
        ur.add_do_method(
            &t,
            "place_on_cell",
            &[terrain.to_variant(), cell.to_variant()],
        );
        ur.add_undo_property(&t, "position", &t.get_position().to_variant());
        ur.add_undo_property(&t, "anchor_cell", &t.get("anchor_cell"));
        ur.add_undo_property(&t, "anchored", &t.get("anchored"));
        ur.commit_action();
        self.redraw();
    }

    // ------------------------------------------------------------------ edits

    /// Applies `f` to a copy of the mesh, stores it and records an undo action.
    fn apply_edit(&mut self, name: &str, f: impl FnOnce(&mut BMesh, &mut Self)) {
        // Any other edit ends an open adjustment first (it becomes its own undo step).
        self.adjust_commit();
        let Some(mut data) = self.contents() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let (before, mut m) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        // Cleared before the edit so a message the edit sets (a refusal, a count) stays shown.
        self.message.clear();
        f(&mut m, self);
        if let Err(e) = m.sanity_check() {
            self.message = format!("Operation failed: {e}");
            return;
        }
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            if self.adjust_running {
                // The adjust session's own run: it commits once, at Done.
                if let Some(adj) = &mut self.adjust {
                    adj.moniker = name.to_string();
                }
            } else {
                self.commit_blob(name, &data, before, after);
            }
        }
        self.redraw();
    }

    fn commit_blob(
        &mut self,
        name: &str,
        data: &Gd<PixelBuilderMeshData>,
        before: PackedByteArray,
        after: PackedByteArray,
    ) {
        // A drag / stroke / other path recording its own step: the adjustment goes first.
        self.adjust_commit();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(name);
        ur.add_do_method(data, "set_blob", &[after.to_variant()]);
        ur.add_undo_method(data, "set_blob", &[before.to_variant()]);
        ur.commit_action_ex().execute(false).done();
        self.seen_mesh = Self::mesh_stamp(data);
    }

    /// (instance, revision, face count, vertex count) of a mesh data resource.
    pub(super) fn mesh_stamp(
        data: &Gd<PixelBuilderMeshData>,
    ) -> Option<(InstanceId, u64, usize, usize)> {
        if !data.is_instance_valid() {
            return None;
        }
        let b = data.bind();
        let m = b.cage();
        Some((data.instance_id(), b.revision(), m.facets.len(), m.positions.len()))
    }

    fn every_or_picked_facets(&self, m: &BMesh) -> BTreeSet<usize> {
        match self.mode {
            Mode::Object => (0..m.facets.len()).collect(),
            Mode::Face => self.sel_faces.clone(),
            _ => m.faces_within(&self.selected_verts(m)),
        }
    }

    fn run_action(&mut self, a: Action) {
        // Commit a running adjustment, then (for a parameterised action)
        // run it inside a new adjustment the panel can re-run. The session's own re-runs come
        // back here with `adjust_running` set.
        let rerun = self.adjust_running;
        if !rerun && !self.no_confirm && self.ask_confirmation(a) {
            return;
        }
        if !rerun {
            self.adjust_commit();
        }
        let session = (!rerun
            && !self.no_adjust
            && self.prefs.adjust_panel
            && self.target.is_some()
            && self.tool == Tool::Select)
            .then(|| adjust::fields(a, self.mode, self.xg.to_new_object))
            .flatten();
        let started = session.is_some();
        if let Some(fields) = session {
            self.adjust_begin(a, fields);
        }
        self.run_action_inner(a);
        if started {
            self.adjust_started();
        }
        if !rerun {
            self.sync_ui();
            self.redraw();
        }
    }

    fn run_action_inner(&mut self, a: Action) {
        match a {
            Action::ModeObject => self.assign_setting(Mode::Object),
            Action::ModeFace => self.assign_setting(Mode::Face),
            Action::ModeEdge => self.assign_setting(Mode::Edge),
            Action::ModeVertex => self.assign_setting(Mode::Vertex),
            Action::ToolSelect => self.assign_instrument(Tool::Select),
            Action::ToolShape => self.assign_instrument(Tool::Shape),
            Action::ToolPlace => self.assign_instrument(Tool::Place),
            Action::ToolPaint => self.assign_instrument(Tool::Paint),
            Action::ToolVertexPaint => self.assign_instrument(if self.tool == Tool::VertexPaint {
                Tool::Select
            } else {
                Tool::VertexPaint
            }),
            Action::UvLock => self.uv_lock = !self.uv_lock,
            Action::BlendMaterial => self.act_blend_material(),
            Action::InvertAlpha => self.apply_edit("Pixel Builder: Invert alpha", |m, s| {
                let f = s.every_or_picked_facets(m);
                pt_core::build::paint::invert_alpha(m, &f);
            }),
            Action::BakeMesh => self.act_bake(),
            Action::Bevel => self.perform_chamfer_stretches(),
            Action::GizmoMove => self.set_xform(XformMode::Move),
            Action::GizmoRotate => self.set_xform(XformMode::Rotate),
            Action::GizmoScale => self.set_xform(XformMode::Scale),
            Action::SoftSelect => self.soft = !self.soft,
            Action::SnapVertex => self.snap_vertex = !self.snap_vertex,
            Action::InsertLoop => self.act_insert_loop(),
            Action::Bridge => self.act_bridge(),
            Action::SelectLoop => {
                self.perform_grab_ring(Input::singleton().is_key_pressed(Key::ALT))
            }
            Action::Shrink => self.act_shrink(),
            Action::Holes => self.act_holes(),
            Action::Mirror => self.act_mirror_axes(),
            Action::Dissolve => self.act_dissolve(),
            Action::OffsetElements => self.act_offset(),
            Action::InvertSelection => self.perform_negate_picked(),
            Action::SelectByColor => self.perform_grab_via_tint(),
            Action::SelectBySmoothing => self.act_select_by_smoothing(),
            Action::ToolBezier => {
                if self.mode != Mode::Object {
                    self.assign_setting(Mode::Object);
                }
                self.assign_instrument(Tool::Bezier);
            }
            Action::ReusablePart => self.act_reusable_part(),
            Action::IndividualOrigins => self.xg.individual_origins = !self.xg.individual_origins,
            Action::ExportStl => self.unveil_ship_out_xtra(xg_io::Format::Stl),
            Action::ExportPly => self.unveil_ship_out_xtra(xg_io::Format::Ply),
            Action::ExportRes => self.unveil_ship_out_xtra(xg_io::Format::Res),
            Action::ExportOptions => self.unveil_ship_out_choices(),
            Action::ToolKnife => {
                if self.mode == Mode::Object {
                    self.assign_setting(Mode::Face);
                }
                self.assign_instrument(Tool::Knife);
            }
            Action::ShapeParams => self.toggle_reshape(),
            Action::BoolUnion => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::UNION,
                "Boolean union",
            ),
            Action::BoolSubtract => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::SUBTRACTION,
                "Boolean subtract",
            ),
            Action::BoolIntersect => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::INTERSECTION,
                "Boolean intersect",
            ),
            Action::Merge => self.act_merge(),
            Action::UvEditor => self.unveil_tex_coord_studio(),
            Action::Preferences => self.open_preferences(),
            Action::ConformNormals => self.apply_edit("Pixel Builder: Conform normals", |m, s| {
                let n = ops::harmonize_uprights(m);
                s.message = format!(
                    "Conform normals: {n} face{} flipped",
                    if n == 1 { "" } else { "s" }
                );
            }),
            Action::Connect => self.act_connect(),
            Action::Collapse => self.act_collapse(),
            Action::InsertVertex => {
                self.insert_vert = !self.insert_vert;
                self.insert_hover = None;
                if self.insert_vert && self.mode != Mode::Edge {
                    self.assign_setting(Mode::Edge);
                }
            }
            Action::TrimWalls => self.act_trim_walls(),
            Action::SurfaceDraw => self.surface_draw = !self.surface_draw,
            Action::XRay => self.xray = !self.xray,
            Action::SetCollider => self.act_entity(1),
            Action::SetTrigger => self.act_entity(2),
            Action::Fill => self.act_fill(),
            Action::Detach if self.xg.to_new_object => self.act_extract_faces(false),
            Action::Duplicate if self.xg.to_new_object => self.act_extract_faces(true),
            Action::Detach => self.apply_edit("Pixel Builder: Detach", |m, s| {
                pt_core::build::topo::unlink_facets(m, &s.sel_faces)
            }),
            Action::Duplicate => self.apply_edit("Pixel Builder: Duplicate", |m, s| {
                let f = s.sel_faces.clone();
                s.sel_faces = ops::replicate_facets(m, &f, pt_core::math::Vec3::ZERO);
            }),
            Action::Rotate => {
                let neg = Input::singleton().is_key_pressed(Key::SHIFT);
                self.act_rotate(if neg { -90.0 } else { 90.0 });
            }
            Action::Grow => self.act_select(0),
            Action::Coplanar => self.act_select(1),
            Action::Similar => self.act_select(2),
            Action::ExportObj => self.unveil_ship_out_xtra(xg_io::Format::Obj),
            Action::ExportGltf => self.unveil_ship_out(true),
            Action::Convert => self.convert_selected(),
            Action::SnapGrid => self.snap_grid = !self.snap_grid,
            Action::SnapCell => self.snap_cell = !self.snap_cell,
            Action::Extrude => {
                let (dist, method) = (self.span, self.xg.extrude_method);
                if self.mode == Mode::Edge && !self.sel_edges.is_empty() {
                    self.perform_pull_out_ridges();
                } else if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Extrude: select faces (Face mode)".into();
                } else if method == 0 {
                    self.apply_edit("Pixel Builder: Extrude", |m, s| {
                        s.sel_faces = ops::extrude_selection(m, &s.sel_faces, dist)
                    });
                } else {
                    // Other extrude methods: vertex normals / every face on its own.
                    let how = pt_core::build::xg_ops::ExtrudeMethod::from_i64(method);
                    self.apply_edit("Pixel Builder: Extrude", |m, s| {
                        match pt_core::build::xg_ops::pull_out_picked_approach(
                            m,
                            &s.sel_faces,
                            dist,
                            how,
                            false,
                        ) {
                            Ok(caps) => s.sel_faces = caps,
                            Err(e) => s.message = format!("Extrude: {e}"),
                        }
                    });
                }
            }
            Action::Inset => {
                let (amt, how) = (self.inset, self.opts.inset_mode);
                if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Inset: select faces (Face mode)".into();
                } else if how == 0 {
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        s.sel_faces = ops::inset_center(m, &s.sel_faces, amt.min(0.9905))
                    });
                } else if how == 1 && !self.xg.inset_individual {
                    // Grouped inset: connected flat faces inset as one region.
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        match pt_core::build::xg_ops::inset_center_grouped(
                            m,
                            &s.sel_faces,
                            amt,
                            false,
                        ) {
                            Ok(inner) => s.sel_faces = inner,
                            Err(e) => s.message = format!("Inset: {e}"),
                        }
                    });
                } else {
                    // Width inset: a rim `amt` wide; Equal mitres the corners.
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        s.sel_faces =
                            pt_core::build::mesh_ops::inset_rim(m, &s.sel_faces, amt, how == 2)
                    });
                }
            }
            Action::Subdivide => self.act_subdivide(),
            Action::SubdivideEdges => self.perform_refine_ridges(),
            Action::ConnectEdges => self.perform_join_ridges(),
            Action::FlipEdge => self.perform_turnover_ridge(),
            Action::SplitVerts => self.act_split_verts(),
            Action::Triangulate => self.act_triangulate(),
            Action::MergeObjects => self.perform_fuse_entities(),
            Action::CenterPivot => self.act_center_pivot(),
            Action::PivotToSelection => self.act_pivot_to_selection(),
            Action::FreezeTransform => self.perform_ice_warp(),
            Action::Flip => self.apply_edit("Pixel Builder: Flip normals", |m, s| {
                let f = s.every_or_picked_facets(m);
                ops::turnover_facets(m, &f);
            }),
            Action::Delete => self.delete_selected(),
            Action::Weld => {
                let dist = self.xg.weld_dist.max(1e-6);
                self.apply_edit("Pixel Builder: Weld", |m, s| {
                    if s.mode == Mode::Vertex && s.sel_verts.len() > 1 {
                        // The selected vertices closer than the Weld distance.
                        let v = s.sel_verts.clone();
                        let n = pt_core::build::loops::braze_picked_corners(m, &v, dist);
                        m.compact();
                        s.purge_picked();
                        s.message = if n == 0 {
                            format!("Weld: no selected vertices closer than {dist}")
                        } else {
                            format!("Weld: {n} vertices merged")
                        };
                    } else {
                        let n = ops::braze_via_span(m, dist, None);
                        s.purge_picked();
                        s.message = format!("Weld: {n} vertices merged");
                    }
                })
            }
            Action::SelectAll => self.grab_every(),
            Action::ClearSelection => self.purge_picked(),
            Action::SelectRing => self.perform_grab_ring(true),
            Action::RotateNeg => self.act_rotate(-90.0),
            Action::GenerateUv2 => self.perform_produce_uv_b(),
            Action::PaintColour => self.act_paint_colour(),
            Action::Material => {
                let (slot, col) = (self.pocket.clamp(0, 255) as u16, ui_color_bytes(self.tint));
                self.apply_edit("Pixel Builder: Assign material", |m, s| {
                    let f = s.every_or_picked_facets(m);
                    ops::assign_stuff(m, &f, slot);
                    ops::assign_tint(m, &f, col);
                });
            }
        }
    }

    /// Ctrl+A: every element of the mode.
    fn grab_every(&mut self) {
        self.adjust_commit();
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            match self.mode {
                Mode::Face => self.sel_faces = (0..m.facets.len()).collect(),
                Mode::Edge => self.sel_edges = m.ridges().into_iter().collect(),
                Mode::Vertex => self.sel_verts = (0..m.positions.len() as u32).collect(),
                Mode::Object => {}
            }
        }
    }

    /// H: Object → Face → Edge → Vertex → Object (the documented 1→2→3→4 order).
    fn cycle_mode(&mut self) {
        let next = match self.mode {
            Mode::Object => Mode::Face,
            Mode::Face => Mode::Edge,
            Mode::Edge => Mode::Vertex,
            Mode::Vertex => Mode::Object,
        };
        self.assign_setting(next);
    }

    fn delete_selected(&mut self) {
        let mode = self.mode;
        self.apply_edit("Pixel Builder: Delete", |m, s| {
            match mode {
                Mode::Face => ops::discard_facets(m, &s.sel_faces),
                Mode::Edge => ops::delete_edges(m, &s.sel_edges),
                Mode::Vertex => ops::delete_verts(m, &s.sel_verts),
                Mode::Object => {}
            }
            s.purge_picked();
        });
    }

    fn assign_setting(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        self.adjust_commit();
        self.hover_face = None;
        self.hover_edge = None;
        self.hover_vert = None;
        // Convert the selection (face -> its verts / edges, verts -> faces fully inside).
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            let verts = self.selected_verts(m);
            self.sel_faces = if mode == Mode::Face {
                m.faces_within(&verts)
            } else {
                BTreeSet::new()
            };
            self.sel_edges = if mode == Mode::Edge {
                m.edges_within(&verts)
            } else {
                BTreeSet::new()
            };
            self.sel_verts = if mode == Mode::Vertex {
                verts
            } else {
                BTreeSet::new()
            };
        }
        self.mode = mode;
        if mode != Mode::Object {
            // Element modes always use Select: drop the drawing tools' state too (a Bezier
            // re-edit, a poly outline).
            self.tool = Tool::Select;
            self.bz.revert();
            self.poly_outline = None;
            self.poly_holes.clear();
            self.wall_pts.clear();
        }
        self.cancel_drag();
        self.sync_ui();
        self.redraw();
    }

    fn set_xform(&mut self, x: XformMode) {
        self.xform_mode = x;
        self.hover_handle = None;
        if self.tool != Tool::Select {
            self.assign_instrument(Tool::Select);
        }
    }

    fn assign_instrument(&mut self, t: Tool) {
        if t != self.tool {
            self.adjust_commit();
        }
        self.cancel_vpaint_stroke();
        self.tool = t;
        self.knife = Knife::default();
        self.wall_pts.clear();
        self.wall_cursor = None;
        self.poly_outline = None;
        self.poly_holes.clear();
        self.bz.revert();
        self.hover_place = None;
        if t != Tool::Select {
            self.cancel_drag();
        }
        self.sync_ui();
        self.redraw();
    }

    // ------------------------------------------------------------------ drag

    fn begin_drag(&mut self, cam: &Gd<Camera3D>, pos: Vector2, handle: Handle, extrude: bool) {
        let Some(data) = self.contents() else { return };
        let (before, mut start) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
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
            self.sel_faces = ops::extrude_selection(&mut start, &self.sel_faces, 0.0);
            name = "Pixel Builder: Extrude";
        }
        let verts = match self.mode {
            Mode::Face => start.verts_of_faces(&self.sel_faces),
            Mode::Edge => start.verts_of_edges(&self.sel_edges),
            _ => self.sel_verts.clone(),
        };
        let Some(pivot) = self.fulcrum() else { return };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let axes = self.gizmo_axes();
        let start_t = axis
            .and_then(|a| closest_on_axis(pivot, axes[a], o, d))
            .unwrap_or(0.0);
        let n = cam.get_global_transform().basis.col_c();
        let start_hit = plane_hit(pivot, n, o, d).unwrap_or(pivot);
        let weights = self.drag_weights(&start, &verts);
        let (rot, scale) = match handle {
            Handle::Rotate(a) => (Some(self.begin_rotate(cam, pos, pivot, a)), None),
            Handle::Scale(a) => (None, Some(self.begin_scale(cam, pos, pivot, a))),
            Handle::Move(_) => (None, None),
        };
        let (snap_anchor, snap_targets) = if matches!(handle, Handle::Move(_)) {
            self.snap_setup(cam, pos, &start, &weights, pivot)
        } else {
            (pivot, Vec::new())
        };
        let uv_frames = self.uv_lock.then(|| {
            pt_core::build::uv_lock::capture_drag_frames(&start, &weights.keys().copied().collect())
        });
        let groups = if matches!(handle, Handle::Rotate(_) | Handle::Scale(_)) {
            self.individual_groups(&start, &verts)
        } else {
            None
        };
        self.drag = Some(Drag {
            axle: axis,
            before,
            commence: start,
            fulcrum: pivot,
            start_t,
            start_hit,
            moniker: name,
            last_delta: Vector3::ZERO,
            handle: Some(handle),
            groups,
            weights,
            rot,
            magnify: scale,
            snap_anchor,
            snap_targets,
            snap_hit: None,
            cursor: pos,
            uv_frames,
            axes,
        });
    }

    fn drag_motion(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self
            .drag
            .as_ref()
            .is_some_and(|d| d.rot.is_some() || d.magnify.is_some())
        {
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
        let mut delta = match drag.axle {
            Some(a) => match closest_on_axis(drag.fulcrum, axes[a], o, d) {
                Some(t) => axes[a] * (t - drag.start_t),
                None => return,
            },
            None => {
                let n = cam.get_global_transform().basis.col_c();
                match plane_hit(drag.fulcrum, n, o, d) {
                    Some(h) => h - drag.start_hit,
                    None => return,
                }
            }
        };
        if self.snap_cell
            && drag.axle != Some(1)
            && let Some(t) = &self.terrain
        {
            // Snap the pivot onto the nearest terrain cell corner / centre (hex-aligned walls).
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * (drag.fulcrum + delta);
            let q = snap::nearest_cell_point(&t.bind().store().config, v2(l.x, l.z), true);
            let w = xf * Vector3::new(q.x, l.y, q.y);
            delta.x = w.x - drag.fulcrum.x;
            delta.z = w.z - drag.fulcrum.z;
            if drag.axle.is_some() {
                let a = axes[drag.axle.unwrap_or(0)];
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
        let mut m = drag.commence.clone();
        pt_core::build::xform::translate_weighted(&mut m, &drag.weights, to_p(local));
        drag.lock_uvs(&mut m);
        if let Some(dr) = &mut self.drag {
            dr.last_delta = delta;
        }
        if let Some(mut data) = self.contents() {
            data.bind_mut().substitute(m);
            data.emit_changed();
        }
        self.redraw();
    }

    fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        let Some(data) = self.contents() else { return };
        let after = data.bind().get_blob();
        if after != drag.before {
            self.commit_blob(drag.moniker, &data, drag.before, after);
        }
    }

    fn cancel_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        if let Some(mut data) = self.contents() {
            data.bind_mut().set_blob(drag.before);
        }
        self.redraw();
    }

    // ------------------------------------------------------------------ keys

    fn on_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        // Top-row digits arrive as KP_1.. (the viewport's numpad emulation): match the mode
        // keys 1–4 (and 5) against the key the user pressed.
        let key = &crate::util::undo_numpad_emulation(key);
        let k = key.get_keycode();
        let ctrl = key.is_ctrl_pressed() || key.is_meta_pressed();
        // Adjust panel: Enter keeps the result, Esc restores; Ctrl+Z first commits the adjusted
        // action so Godot's undo takes all of it back.
        if self.adjust.is_some() {
            match k {
                Key::ENTER | Key::KP_ENTER if !ctrl => {
                    self.adjust_commit();
                    self.sync_ui();
                    self.redraw();
                    return true;
                }
                Key::ESCAPE => {
                    self.adjust_cancel();
                    return true;
                }
                Key::Z if ctrl => {
                    self.adjust_commit();
                    return false;
                }
                _ => {}
            }
        }
        // Rebindable shortcuts (Editor Settings › Shortcuts › Mst Builder): the first entry whose
        // key matches and whose guard accepts runs; a refused one lets the key through.
        for i in 0..super::shortcuts::ENTRIES.len() {
            if !super::shortcuts::fits(&self.shortcut_table, i, key) {
                continue;
            }
            if let Some(cmd) = self.shortcut_command(super::shortcuts::ENTRIES[i].id) {
                self.run_key_command(cmd);
                return true;
            }
        }
        let editing = self.mode != Mode::Object;
        match k {
            Key::ESCAPE if !ctrl && !key.is_shift_pressed() && !key.is_alt_pressed() => {
                if self.shape_drag.is_some() {
                    self.cancel_shape();
                } else if self.cancel_vpaint_stroke() {
                } else if self.drag.is_some() {
                    self.cancel_drag();
                } else if self.tool != Tool::Select {
                    self.assign_instrument(Tool::Select);
                } else if editing
                    && !(self.sel_faces.is_empty()
                        && self.sel_edges.is_empty()
                        && self.sel_verts.is_empty())
                {
                    self.purge_picked();
                } else if editing {
                    self.assign_setting(Mode::Object);
                } else {
                    return false;
                }
                self.redraw();
                true
            }
            _ => false,
        }
    }

    /// The command shortcut `id` runs now, or `None` when its context guard refuses (the
    /// builder's former hard-coded conditions).
    fn shortcut_command(&self, id: &str) -> Option<KeyCmd> {
        let has = self.target.is_some();
        let editing = self.mode != Mode::Object;
        let et = editing && has;
        let run = |ok: bool, a: Action| ok.then_some(KeyCmd::Run(a));
        match id {
            "mode_object" => run(has, Action::ModeObject),
            "mode_face" => run(has, Action::ModeFace),
            "mode_edge" => run(has, Action::ModeEdge),
            "mode_vertex" => run(has, Action::ModeVertex),
            "cycle_mode" => has.then_some(KeyCmd::CycleMode),
            "tool_select" => run(editing || self.tool != Tool::Select, Action::ToolSelect),
            "tool_shape" => Some(KeyCmd::Run(Action::ToolShape)),
            "tool_bezier" => Some(KeyCmd::Run(Action::ToolBezier)),
            "tool_paint" => run(has, Action::ToolPaint),
            "tool_vertex_paint" => run(has, Action::ToolVertexPaint),
            "tool_place" => run(has, Action::ToolPlace),
            "tool_knife" => run(et, Action::ToolKnife),
            "gizmo_move" => run(et, Action::GizmoMove),
            "gizmo_rotate" => run(et, Action::GizmoRotate),
            "gizmo_scale" => run(et, Action::GizmoScale),
            "soft_select" => run(et, Action::SoftSelect),
            "extrude" => run(et, Action::Extrude),
            "inset" => run(et, Action::Inset),
            "bevel" => run(
                has && matches!(self.mode, Mode::Edge | Mode::Vertex),
                Action::Bevel,
            ),
            "insert_loop" => run(
                has && self.mode == Mode::Edge && !self.sel_edges.is_empty(),
                Action::InsertLoop,
            ),
            "merge" => run(et, Action::Merge),
            "connect" => run(
                et,
                if self.mode == Mode::Edge {
                    Action::ConnectEdges
                } else {
                    Action::Connect
                },
            ),
            // Only where a fill can run (Edge / Vertex mode with a selection); otherwise `F`
            // goes to Godot and frames the selection (r4 #9: F never framed in an element mode).
            "fill" => run(
                has && matches!(self.mode, Mode::Edge | Mode::Vertex)
                    && !(self.sel_edges.is_empty() && self.sel_verts.is_empty()),
                Action::Fill,
            ),
            "triangulate" => run(
                has && matches!(self.mode, Mode::Object | Mode::Face),
                Action::Triangulate,
            ),
            "rotate" => run(et, Action::Rotate),
            "rotate_back" => run(et, Action::RotateNeg),
            "duplicate" => run(
                has && self.mode == Mode::Face && !self.sel_faces.is_empty(),
                Action::Duplicate,
            ),
            "delete" => run(et, Action::Delete),
            "select_all" => run(et, Action::SelectAll),
            "grow" => run(et, Action::Grow),
            "shrink" => run(et, Action::Shrink),
            "select_loop" => run(et, Action::SelectLoop),
            "select_ring" => run(et, Action::SelectRing),
            "insert_vertex" => run(et && self.mode == Mode::Edge, Action::InsertVertex),
            "uv_editor" => run(has, Action::UvEditor),
            "grid_halve" => Some(KeyCmd::Grid(0.5)),
            "grid_double" => Some(KeyCmd::Grid(2.0)),
            _ => None,
        }
    }

    fn run_key_command(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Run(a) => self.run_action(a),
            KeyCmd::CycleMode => self.cycle_mode(),
            KeyCmd::Grid(f) => {
                self.grid_step = (self.grid_step * f).clamp(0.0624, 64.0);
                self.sync_ui();
            }
        }
    }

    // ------------------------------------------------------------------ wireframe gizmo

    fn secure_handlebar(&mut self) {
        if self.gizmo.is_some() {
            return;
        }
        let Some(mut t) = self.target.clone() else {
            return;
        };
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
        mi.set_cast_shadows_setting(
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF,
        );
        mi.set_layer_mask(1);
        mi.add_to_group("pixel_ignore");
        add_internal(&mut t, &mi);
        self.gizmo = Some(mi);
    }

    fn drop_gizmo(&mut self) {
        self.drop_attr_overlay();
        if let Some(g) = self.gizmo.take()
            && g.is_instance_valid()
        {
            let mut g = g;
            g.queue_free();
        }
    }

    /// Recomputes the viewport feedback text (scene info, dimensions) for the overlay.
    fn refresh_feedback(&mut self) {
        self.feedback = Feedback::default();
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        if self.prefs.show_scene_info {
            self.feedback.facts = Some(format!(
                "{} verts  {} edges  {} faces  |  selected: {} faces {} edges {} verts",
                m.positions.len(),
                m.ridges().len(),
                m.facets.len(),
                self.sel_faces.len(),
                self.sel_edges.len(),
                self.sel_verts.len()
            ));
        }
        if self.prefs.show_dimensions {
            let verts: Vec<u32> = if self.mode == Mode::Object {
                (0..m.positions.len() as u32).collect()
            } else {
                self.selected_verts(m).into_iter().collect()
            };
            if !verts.is_empty() {
                // World-space box of the selection (axis-aligned in world space).
                let xf = self.xform();
                let mut lo = Vector3::splat(f32::INFINITY);
                let mut hi = Vector3::splat(f32::NEG_INFINITY);
                for v in verts {
                    let w = xf * from_p(m.positions[v as usize]);
                    lo = lo.coord_min(w);
                    hi = hi.coord_max(w);
                }
                let s = hi - lo;
                let anchor = Vector3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5);
                self.feedback.dims =
                    Some((anchor, format!("{:.3} x {:.3} x {:.3}", s.x, s.y, s.z)));
            }
        }
    }

    fn redraw(&mut self) {
        // Smoothing tint / normals: rebuilt only when the mesh or its settings changed.
        self.refresh_attr_overlay();
        // Drop selections / hovers that an undo / external edit made invalid.
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            let (nf, nv) = (m.facets.len(), m.positions.len() as u32);
            self.sel_faces.retain(|f| *f < nf);
            self.sel_verts.retain(|v| *v < nv);
            self.sel_edges.retain(|e| e.0 < nv && e.1 < nv);
            if self.hover_face.is_some_and(|f| f >= nf) {
                self.hover_face = None;
            }
            if self.hover_vert.is_some_and(|v| v >= nv) {
                self.hover_vert = None;
            }
            if self.hover_edge.is_some_and(|e| e.0 >= nv || e.1 >= nv) {
                self.hover_edge = None;
            }
        }
        self.refresh_feedback();
        self.base().update_overlays();
        self.tune_tex_coord_studio();
        let Some(g) = &self.gizmo else { return };
        if !g.is_instance_valid() {
            self.gizmo = None;
            return;
        }
        let Some(mesh) = g.get_mesh() else { return };
        let Ok(mut im) = mesh.try_cast::<ImmediateMesh>() else {
            return;
        };
        im.clear_surfaces();
        let Some(data) = self.contents() else { return };
        let db = data.bind();
        let m = db.cage();
        // Drop selections that an undo / external edit made invalid.
        let nf = m.facets.len();
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
            .chain(
                self.hover_face
                    .filter(|f| !self.sel_faces.contains(f))
                    .map(|f| (f, self.prefs.color_face_hover)),
            )
            .collect();
        for (f, c) in faces {
            let n = from_p(m.face_normal(f)) * 0.0045;
            for t in &tris[f] {
                for k in t {
                    tri_pts.push((p(m.facets[f].verts[*k]) + n, c));
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
        for e in m.ridges() {
            let c = if sel_edges.contains(&e) {
                self.prefs.color_edge_selected
            } else {
                self.prefs.color_edge
            };
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.0));
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.1));
        }
        im.surface_end();
        // Hovered edge (Edge mode): a thin quad strip along it so it reads thicker than the wire.
        if let Some(e) = self.hover_edge.filter(|_| self.mode == Mode::Edge)
            && (e.0 as usize) < m.positions.len()
            && (e.1 as usize) < m.positions.len()
        {
            let c = self.prefs.color_edge_hover;
            let (a, b) = (p(e.0), p(e.1));
            let len = (b - a).length().max(1e-4);
            let w = (len * 0.0125).clamp(0.0045, 0.0305);
            let dir = (b - a) / len;
            let side = if dir.cross(Vector3::UP).length() > 0.1005 {
                dir.cross(Vector3::UP).normalized()
            } else {
                dir.cross(Vector3::RIGHT).normalized()
            };
            let up = dir.cross(side).normalized();
            im.surface_begin(PrimitiveType::TRIANGLES);
            for off in [side * w, up * w] {
                for q in [a - off, b - off, b + off, a - off, b + off, a + off] {
                    im.surface_set_color(c);
                    im.surface_add_vertex(q);
                }
            }
            im.surface_end();
        }
    }
}

impl PixelBuilderPlugin {
    // ------------------------------------------------------------------ Godot gizmo lock

    /// Godot draws its own move / rotate / scale gizmo on the selected node, on top of the
    /// builder's element handles. The editor's own gizmo is hidden by setting the
    /// editor's `_edit_lock_` meta while the plugin owns the transform; we do the same in the
    /// Face / Edge / Vertex modes (and for the element tools). A node the user locked himself
    /// is left alone; the lock is stripped while the scene is being saved.
    pub(super) fn sync_gizmo_lock(&mut self) {
        let want = self.target.clone().filter(|t| {
            t.is_instance_valid()
                && self.visible
                && (self.mode != Mode::Object
                    || matches!(
                        self.tool,
                        Tool::Paint | Tool::Knife | Tool::VertexPaint | Tool::Bezier
                    ))
        });
        // Validity first: the locked node may have been freed (its scene closed).
        if want
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .map(|t| t.instance_id())
            == self
                .lock_target
                .as_ref()
                .filter(|t| t.is_instance_valid())
                .map(|t| t.instance_id())
        {
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
            // The 3D editor rebuilds its gizmo on a selection change ("selection nudge").
            sel.upcast::<Object>()
                .call_deferred("emit_signal", &["selection_changed".to_variant()]);
        }
    }

    // ------------------------------------------------------------------ UV editor

    /// Opens the UV editor: the floating window (MST layout) or the "Mesh UV" bottom dock (docked
    /// layout). One panel / canvas instance serves both hosts.
    pub(super) fn unveil_tex_coord_studio(&mut self) {
        if self
            .uv_window
            .as_ref()
            .is_none_or(|w| !w.is_instance_valid())
            || self
                .uv_panel
                .as_ref()
                .is_none_or(|p| !p.is_instance_valid())
        {
            let (win, panel, canvas) = uv_editor::build_window();
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&win);
            }
            self.uv_window = Some(win);
            self.uv_panel = Some(panel);
            self.uv_canvas = Some(canvas);
        }
        let docked = self.is_modeler();
        self.place_uv(docked);
        if docked {
            if let Some(d) = &mut self.uv_dock {
                d.make_visible();
            }
        } else if let Some(w) = &mut self.uv_window {
            if w.is_visible() {
                w.grab_focus();
            } else {
                w.popup_centered();
            }
        }
        // Not `sync_uv_editor`: a dock made visible may only report visible next frame.
        self.push_uv_target();
    }

    /// Moves the UV panel into the bottom dock (`dock`) or back into the window (change-only).
    /// Leaving the dock closes it (empty, tab hidden); `exit_tree` removes and frees it. Nothing
    /// here is connected to the plugin, so the reparenting cannot call back into it.
    pub(super) fn place_uv(&mut self, dock: bool) {
        let (Some(mut panel), Some(mut win)) = (
            self.uv_panel.clone().filter(|p| p.is_instance_valid()),
            self.uv_window.clone().filter(|w| w.is_instance_valid()),
        ) else {
            return;
        };
        let s = crate::editor::ui_kit::editor_scale();
        if dock {
            if self.uv_dock.as_ref().is_none_or(|d| !d.is_instance_valid()) {
                use godot::classes::editor_dock::{DockLayout, DockSlot};
                let mut d = godot::classes::EditorDock::new_alloc();
                d.set_name("PixelBuilderMeshUv");
                d.set_title("Mesh UV");
                d.set_layout_key("PixelBuilderMeshUv");
                d.set_default_slot(DockSlot::BOTTOM);
                d.set_available_layouts(DockLayout::HORIZONTAL | DockLayout::FLOATING);
                // Opened by the builder only (not by a saved editor layout or a shortcut).
                d.set_transient(true);
                d.set_global(false);
                d.set_closable(true);
                d.set_clip_contents(true);
                self.base_mut().add_dock(&d);
                self.uv_dock = Some(d);
            }
            let Some(mut d) = self.uv_dock.clone() else {
                return;
            };
            if panel
                .get_parent()
                .is_none_or(|p| p.instance_id() != d.instance_id())
            {
                win.hide();
                if let Some(mut p) = panel.get_parent() {
                    p.remove_child(&panel);
                }
                panel.set_custom_minimum_size(Vector2::new(0.0, 320.0 * s));
                d.add_child(&panel);
            }
        } else {
            if panel
                .get_parent()
                .is_none_or(|p| p.instance_id() != win.instance_id())
            {
                if let Some(mut p) = panel.get_parent() {
                    p.remove_child(&panel);
                }
                panel.set_custom_minimum_size(Vector2::ZERO);
                win.add_child(&panel);
                panel.set_anchors_and_offsets_preset(
                    godot::classes::control::LayoutPreset::FULL_RECT,
                );
            }
            // The dock stays registered (closed, its tab hidden) until the plugin exits: removing
            // and freeing it here left the editor's dock manager deferred calls on a freed
            // object, flooding the message queue.
            if let Some(d) = &mut self.uv_dock
                && d.is_instance_valid()
                && d.is_visible_in_tree()
            {
                d.close();
            }
        }
    }

    /// Whether the UV editor is showing (window visible, or the dock's panel visible in the tree).
    pub(super) fn tex_coord_unveil(&self) -> bool {
        if self.tex_coord_carrier() == "bottom_panel" {
            return self
                .uv_panel
                .as_ref()
                .is_some_and(|p| p.is_instance_valid() && p.is_visible_in_tree());
        }
        self.uv_window
            .as_ref()
            .is_some_and(|w| w.is_instance_valid() && w.is_visible())
    }

    /// `"window"` / `"bottom_panel"` (the UV panel's host), `""` before the first open.
    pub(super) fn tex_coord_carrier(&self) -> &'static str {
        match &self.uv_panel {
            Some(p) if p.is_instance_valid() => {
                if self.uv_dock.as_ref().is_some_and(|d| {
                    d.is_instance_valid()
                        && p.get_parent()
                            .is_some_and(|x| x.instance_id() == d.instance_id())
                }) {
                    "bottom_panel"
                } else {
                    "window"
                }
            }
            _ => "",
        }
    }

    /// Hands the edited mesh and the 3D face selection to the UV editor (if open).
    fn tune_tex_coord_studio(&mut self) {
        if self.tex_coord_unveil() {
            self.push_uv_target();
        }
    }

    fn push_uv_target(&mut self) {
        let faces = if self.mode == Mode::Face {
            super::data::face_array(&self.sel_faces)
        } else {
            PackedInt32Array::new()
        };
        let target = self.target.clone();
        if let Some(c) = &mut self.uv_canvas
            && c.is_instance_valid()
        {
            c.bind_mut().set_target(target, faces);
        }
    }

    // ------------------------------------------------------------------ preferences

    fn open_preferences(&mut self) {
        if self
            .prefs_panel
            .as_ref()
            .is_none_or(|p| !p.popup.is_instance_valid())
        {
            let gd = self.to_gd().upcast::<Object>();
            let panel = prefs::build_panel(&gd, &self.prefs);
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&panel.popup);
            }
            self.prefs_panel = Some(panel);
        }
        let p = self.prefs;
        if let Some(panel) = &mut self.prefs_panel {
            prefs::sync_panel(panel, &p);
            panel.unveil();
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

/// Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y pressed.
/// Whether `pos` (main editor window coordinates) is over the editor's main screen (the 3D
/// editor with its viewports and the builder's toolbar / strip / palette containers).
fn in_main_screen(pos: Vector2) -> bool {
    EditorInterface::singleton()
        .get_editor_main_screen()
        .is_some_and(|m| m.get_global_rect().contains_point(pos))
}

fn is_undo_redo_key(k: &Gd<InputEventKey>) -> bool {
    k.is_pressed()
        && !k.is_echo()
        && (k.is_ctrl_pressed() || k.is_meta_pressed())
        && !k.is_alt_pressed()
        && matches!(k.get_keycode(), Key::Z | Key::Y)
}

fn ui_color_bytes(c: Color) -> [u8; 4] {
    super::data::color_bytes(c)
}

#[godot_api]
impl PixelBuilderPlugin {
    #[func]
    fn upon_deed(&mut self, token_id: i64) {
        if let Some(a) = Action::from_i64(token_id) {
            self.run_action(a);
        }
    }

    #[func]
    fn on_field(&mut self, value: f64, token_id: i64) {
        let Some(f) = Field::from_i64(token_id) else {
            return;
        };
        let v = value as f32;
        let old = self.spec;
        match f {
            Field::BevelSegs => self.bevel_segs = value as i64,
            Field::SoftRadius => self.soft_radius = v,
            Field::Falloff => self.falloff = value as i64,
            Field::RotSnap => self.rot_snap = v,
            Field::MirrorAxis => {
                // Older single-axis field: that axis only.
                self.mirror_axis = value as i64;
                self.xg.mirror = [0, 1, 2].map(|i| i == self.mirror_axis.clamp(0, 2));
            }
            Field::Arc => self.spec.arc = v,
            Field::Inner => self.spec.inner = v,
            Field::Opening => self.spec.opening = v,
            Field::DoorHeight => self.spec.door_height = v,
            Field::Profile => self.spec.silhouette = value as u8,
            Field::Side => self.spec.flank = value as u8,
            Field::SizeX => self.spec.size.x = v,
            Field::SizeY => self.spec.size.y = v,
            Field::SizeZ => self.spec.size.z = v,
            Field::Segments => self.spec.stretches = value as u32,
            Field::Steps => self.spec.steps = value as u32,
            Field::Thickness => self.spec.thickness = v,
            Field::Rings => self.spec.rings = value as u32,
            Field::Slot => self.pocket = value as i64,
            Field::GridStep => self.grid_step = v,
            Field::Distance => self.span = v,
            Field::Inset => self.inset = v,
            Field::Bevel => self.chamfer = v,
            Field::Cuts => self.cuts = value as i64,
            Field::PaintApply => self.vp.flood = value >= 0.5,
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
            Field::OffsetX => self.xg.offset[0] = v,
            Field::OffsetY => self.xg.offset[1] = v,
            Field::OffsetZ => self.xg.offset[2] = v,
            Field::OffsetSpace => self.xg.offset_space = value as i64,
            Field::OffsetPerFace => self.xg.offset_per_face = value >= 0.5,
            Field::GrowAngle => self.xg.grow_angle = v,
            Field::ExtrudeMethod => self.xg.extrude_method = value as i64,
            Field::EdgesGroup => self.xg.edges_group = value >= 0.5,
            Field::InsetIndividual => self.xg.inset_individual = value >= 0.5,
            Field::CollapseFirst => self.xg.collapse_first = value >= 0.5,
            Field::WeldDist => self.xg.weld_dist = v,
            Field::FillEntire => self.xg.fill_entire = value >= 0.5,
            Field::MirrorX => self.xg.mirror[0] = value >= 0.5,
            Field::MirrorY => self.xg.mirror[1] = value >= 0.5,
            Field::MirrorZ => self.xg.mirror[2] = value >= 0.5,
            Field::MirrorDup => self.xg.mirror_dup = value >= 0.5,
            Field::ToNewObject => self.xg.to_new_object = value >= 0.5,
            Field::SegmentsZ => self.spec.segments_z = value.clamp(0.0, 64.0) as u32,
            Field::HeightCuts => self.spec.height_cuts = value.clamp(0.0, 255.0) as u32,
            Field::TorusArc => self.spec.torus_arc = v.clamp(0.0, 360.0),
            Field::TubeArc => self.spec.tube_arc = v.clamp(0.0, 360.0),
            Field::ArchBand => self.spec.arch_band = value >= 0.5,
            Field::ArchDegrees => self.spec.arch_degrees = v.clamp(1.0, 360.0),
            Field::EndCaps => self.spec.end_caps = value >= 0.5,
            Field::StepHeight => self.spec.step_height = if v > 0.0 { v.max(0.0105) } else { 0.0 },
            Field::EqualSteps => self.spec.equal_steps = value >= 0.5,
            Field::Subdivisions => self.spec.subdivisions = value.clamp(1.0, 5.0) as u32,
            Field::PolyHoles => {
                self.xg.poly_holes = value >= 0.5;
                if !self.xg.poly_holes {
                    self.poly_outline = None;
                    self.poly_holes.clear();
                }
            }
            Field::BevelAmountType => self.xg.bevel_type = (value as i64).clamp(0, 3),
            Field::BevelProfile => self.xg.bevel_profile = v.clamp(0.0, 1.0),
            Field::BevelClamp => self.xg.bevel_clamp = value >= 0.5,
            Field::BevelHarden => self.xg.bevel_harden = value >= 0.5,
            Field::SmoothTint => self.xg.smooth_tint = value >= 0.5,
            Field::ShowNormals => self.xg.show_normals = value >= 0.5,
            Field::NormalLength => self.xg.normal_length = v.clamp(0.0, 10.0),
            Field::BzKind
            | Field::BzClosed
            | Field::BzSmooth
            | Field::BzWidth
            | Field::BzRows
            | Field::BzColumns
            | Field::BzHeight
            | Field::BzTangent
            | Field::BzRoll => self.bezier_field(f, value),
        }
        // Fields that show / hide others.
        if matches!(
            f,
            Field::OffsetSpace
                | Field::ArchBand
                | Field::StepHeight
                | Field::BzKind
                | Field::InsetMode
        ) {
            self.sync_ui();
        }
        if matches!(f, Field::PaintApply | Field::PaintChannel) {
            self.sync_ui();
        }
        if matches!(
            f,
            Field::SmoothTint | Field::ShowNormals | Field::NormalLength
        ) {
            self.refresh_attr_overlay();
            self.sync_ui();
        }
        // An option of the open adjustment: strip and panel show the same value, the action
        // re-runs after the debounce.
        if self.adjust.as_ref().is_some_and(|a| a.fields.contains(&f)) {
            self.sync_ui();
            self.adjust_field_changed(f);
        }
        self.spec_changed(old);
        self.base().update_overlays();
    }

    #[func]
    fn on_field_option(&mut self, index: i64, token_id: i64) {
        self.on_field(index as f64, token_id);
    }

    #[func]
    fn on_field_check(&mut self, on: bool, token_id: i64) {
        self.on_field(if on { 1.0 } else { 0.0 }, token_id);
    }

    #[func]
    fn on_kind(&mut self, index: i64) {
        let old = self.spec;
        self.spec.sort = ShapeKind::from_i64(index);
        // A wall / poly drawing in progress belongs to the old kind.
        self.wall_pts.clear();
        self.poly_outline = None;
        self.poly_holes.clear();
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
        self.spec.glide = on;
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
        self.assign_setting(Mode::Face);
        self.assign_instrument(Tool::Knife);
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            if (face as usize) < m.facets.len() {
                self.knife.face = Some(face as usize);
            }
        }
        self.knife.points = points.as_slice().iter().map(|p| to_p(*p)).collect();
        self.knife_commit();
    }

    /// Test / scripting hook: draw a wall (current wall settings) through world `points`.
    #[func]
    fn debug_wall(&mut self, points: PackedVector3Array, closed: bool) {
        if !self.spec.sort.is_polyline() {
            self.spec.sort = ShapeKind::Wall;
        }
        self.tool = Tool::Shape;
        self.wall_through(points.as_slice(), closed);
        self.sync_ui();
    }

    /// Test / scripting hook: a poly shape (current height) from a world outline with holes
    /// (each an array of world points), through the same path as the drawing tool.
    #[func]
    fn debug_poly(&mut self, outline: PackedVector3Array, holes: VarArray) {
        self.spec.sort = ShapeKind::Poly;
        self.tool = Tool::Shape;
        let hs: Vec<Vec<Vector3>> = holes
            .iter_shared()
            .filter_map(|h| h.try_to::<PackedVector3Array>().ok())
            .map(|h| h.as_slice().to_vec())
            .collect();
        self.poly_through(outline.as_slice(), &hs);
        self.sync_ui();
    }

    /// Test / scripting hook: feeds `event` through the same dispatch as viewport input
    /// (`forward_3d_gui_input`) with 3D viewport 0's camera. True when the plugin consumed it.
    #[func]
    fn debug_input(&mut self, happening: Gd<InputEvent>) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.forward_3d_gui_input(Some(cam), Some(happening)) == AfterGuiInput::STOP.ord()
    }

    /// Test / scripting hook: `event` as if it arrived in editor 3D viewport `idx` (0..3).
    #[func]
    fn debug_viewport_input(&mut self, idx: i64, happening: Gd<InputEvent>) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(idx as i32)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.forward_3d_gui_input(Some(cam), Some(happening)) == AfterGuiInput::STOP.ord()
    }

    /// Test / scripting hook: set a shape field / option by id (same ids as the strip).
    #[func]
    fn debug_field(&mut self, token_id: i64, value: f64) {
        self.on_field(value, token_id);
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
        let Some(k) = prefs::Key::from_i64(key) else {
            return;
        };
        let before = self.prefs.worth(k);
        self.prefs.assign(k, &value);
        if self.prefs.worth(k) != before {
            self.prefs.save_key(k);
        }
        // Snap defaults apply right away too (the strip shows them).
        match k {
            prefs::Key::GridStep => self.grid_step = self.prefs.grid_step,
            prefs::Key::RotSnap => self.rot_snap = self.prefs.rot_snap,
            prefs::Key::SnapGrid => self.snap_grid = self.prefs.snap_grid,
            prefs::Key::Hover if !self.prefs.hover => {
                self.hover_face = None;
                self.hover_edge = None;
                self.hover_vert = None;
            }
            prefs::Key::AdjustPanel if !self.prefs.adjust_panel => self.adjust_commit(),
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
        let p = self.prefs;
        if let Some(panel) = &mut self.prefs_panel {
            prefs::sync_panel(panel, &p);
        }
        self.sync_ui();
        self.redraw();
    }

    #[func]
    fn sync_gizmo_lock_deferred(&mut self) {
        self.sync_gizmo_lock();
    }

    /// Test / scripting hook: the UV editor canvas (opens the window).
    #[func]
    fn inspect_tex_coord_studio(&mut self) -> Option<Gd<PixelBuilderUvCanvas>> {
        self.unveil_tex_coord_studio();
        self.uv_canvas.clone()
    }

    /// Test / scripting hook: the preferences popup (opens it).
    #[func]
    fn debug_preferences(&mut self) -> Option<Gd<godot::classes::PopupPanel>> {
        self.open_preferences();
        self.prefs_panel.as_ref().map(|p| p.popup.clone())
    }

    /// A shortcut may have been rebound: reload them and the tooltips that show them.
    #[func]
    fn on_editor_settings_changed(&mut self) {
        self.shortcut_table = super::shortcuts::reingest();
        if let Some(u) = &mut self.ui {
            u.refresh_tooltips();
        }
        self.refresh_menu_keys();
        // The UI layout setting (Editor Settings dialog, or one of the switch controls).
        let changed = EditorInterface::singleton()
            .get_editor_settings()
            .is_some_and(|es| es.check_changed_settings_in_group(super::layout::GROUP));
        if changed
            || self
                .ui
                .as_ref()
                .is_some_and(|u| u.layout() != super::layout::Layout::ingest())
        {
            self.apply_layout();
        }
    }

    /// Test / scripting hook: the registered shortcuts as `{id: [path, key text, registered]}`.
    #[func]
    fn debug_shortcuts(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let es = EditorInterface::singleton().get_editor_settings();
        for (i, e) in super::shortcuts::ENTRIES.iter().enumerate() {
            let path = super::shortcuts::route(e.id);
            let mut row = VarArray::new();
            row.push(&path.to_variant());
            row.push(&super::shortcuts::key_text(e.id).to_variant());
            row.push(
                &es.as_ref()
                    .is_some_and(|s| s.has_shortcut(&path))
                    .to_variant(),
            );
            row.push(
                &self
                    .shortcut_table
                    .get(i)
                    .is_some_and(|s| s.is_some())
                    .to_variant(),
            );
            d.set(e.id, &row);
        }
        d
    }

    /// Test / scripting hook: the tooltip and hover-label text of toolbar action `id`.
    #[func]
    fn debug_tooltip(&self, id: i64) -> VarArray {
        let mut out = VarArray::new();
        if let (Some(a), Some(LayoutUi::Mst(u))) = (Action::from_i64(id), &self.ui)
            && let Some((_, b, _)) = u.buttons.iter().find(|(x, _, _)| *x == a)
        {
            out.push(&b.get_tooltip_text().to_variant());
            out.push(&ui::hover_text(a).to_variant());
        }
        out
    }

    /// Test / scripting hook: hover state and the cached viewport feedback text.
    #[func]
    fn debug_hover_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("face", self.hover_face.map(|f| f as i64).unwrap_or(-1));
        d.set(
            "edge",
            &self
                .hover_edge
                .map(|e| PackedInt32Array::from(&[e.0 as i32, e.1 as i32][..]))
                .unwrap_or_default(),
        );
        d.set("vert", self.hover_vert.map(|v| v as i64).unwrap_or(-1));
        d.set(
            "info",
            self.feedback.facts.clone().unwrap_or_default().as_str(),
        );
        d.set(
            "dims",
            self.feedback
                .dims
                .as_ref()
                .map(|x| x.1.clone())
                .unwrap_or_default()
                .as_str(),
        );
        d
    }

    /// Test / scripting hook: set a preference by key id (same as the panel).
    #[func]
    fn debug_pref(&mut self, key: i64, value: Variant) {
        self.on_pref(value, key);
    }

    /// Test / scripting hook: a box selection from `from` to `to` (viewport 0 px), Shift held or not.
    #[func]
    fn debug_marquee(&mut self, start_pt: Vector2, to: Vector2, shift: bool) {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return;
        };
        self.adjust_commit();
        self.marquee_select(
            &cam,
            Marquee {
                start_pt,
                to,
                additive: shift,
            },
        );
        self.redraw();
    }

    /// Test / scripting hook: a click pick at `pos` (viewport 0 px), Shift held or not.
    #[func]
    fn debug_pick(&mut self, pos: Vector2, shift: bool) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.adjust_commit();
        let r = self.choose(&cam, pos, shift);
        self.redraw();
        r
    }

    /// Test / scripting hook: mouse hover at `pos` (viewport 0 px).
    #[func]
    fn debug_hover(&mut self, pos: Vector2) {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return;
        };
        self.update_hover(&cam, pos);
    }

    #[func]
    fn on_color(&mut self, c: Color) {
        self.tint = c;
    }

    #[func]
    fn on_uv_field(&mut self, value: f64, token_id: i64) {
        if let Some(f) = UvField::from_i64(token_id) {
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
        self.assign_pocket_stuff(res, slot);
    }

    #[func]
    fn on_add_slot(&mut self) {
        self.insert_pocket();
    }

    #[func]
    fn on_export_file(&mut self, path: GString) {
        self.message = match self.ship_out_into(&path.to_string()) {
            Ok(m) => m,
            Err(e) => format!("Export failed: {e}"),
        };
        if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
            fs.scan();
        }
        self.base().update_overlays();
    }

    /// Test / scripting hook: run a toolbar action by id (applied at once: no adjust session;
    /// `debug_action_adjust` runs it like a button press).
    #[func]
    fn debug_action(&mut self, token_id: i64) {
        let old = (self.no_adjust, self.no_confirm);
        self.no_adjust = true;
        self.no_confirm = true;
        self.upon_deed(token_id);
        (self.no_adjust, self.no_confirm) = old;
    }

    /// Builder toolbar button under the mouse (-1 = none): its name and shortcut in the strip.
    /// The toolbar buttons no longer call this (their hover signals set the label directly,
    /// deferred, r4 #2); kept for scripts. The label is updated through a deferred call so this
    /// never touches a control while a container sort is in progress.
    #[func]
    fn on_button_hover(&self, token_id: i64) {
        let text = Action::from_i64(token_id)
            .map(ui::hover_text)
            .unwrap_or_default();
        if let Some(LayoutUi::Mst(u)) = &self.ui {
            let mut l = u.hover.clone();
            l.call_deferred("set_text", &[GString::from(text.as_str()).to_variant()]);
        }
    }

    /// Paint swatch: click uses its color, Ctrl+click stores the current color in it.
    #[func]
    fn on_swatch(&mut self, idx: i64) {
        let i = idx.clamp(0, 7) as usize;
        let input = Input::singleton();
        if input.is_key_pressed(Key::CTRL) || input.is_key_pressed(Key::META) {
            self.swatch_store(i);
        } else if input.is_key_pressed(Key::ALT) {
            // Alt+click: select the faces painted with this colour.
            let c = self.swatches[i];
            self.select_faces_with_colour(c);
        } else {
            self.tint = self.swatches[i];
            self.sync_ui();
        }
    }

    /// Builder menu (3D toolbar).
    #[func]
    fn on_builder_menu(&mut self, token_id: i64) {
        let a = match token_id {
            0 => Some(Action::UvEditor),
            1 => Some(Action::Preferences),
            2 => Some(Action::BakeMesh),
            3 => Some(Action::ExportObj),
            4 => Some(Action::ExportGltf),
            5 => Some(Action::FreezeTransform),
            6 => Some(Action::CenterPivot),
            7 => Some(Action::ExportStl),
            8 => Some(Action::ExportPly),
            9 => Some(Action::ExportOptions),
            10 => Some(Action::ReusablePart),
            11 => {
                // The alternate-layout check item: write the setting, swap after this signal.
                let next = if self.is_modeler() {
                    super::layout::Layout::Mst
                } else {
                    super::layout::Layout::Modeler
                };
                self.switch_layout(next);
                None
            }
            12 => {
                self.unveil_xtra_choices(None);
                None
            }
            13 => Some(Action::GenerateUv2),
            14 => Some(Action::ExportRes),
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
        d.set(
            "builder_menu",
            self.builder_menu
                .as_ref()
                .is_some_and(|m| m.is_instance_valid() && m.is_visible()),
        );
        d.set(
            "builder_menu_text",
            self.builder_menu
                .as_ref()
                .map(|m| m.get_text().to_string())
                .unwrap_or_default(),
        );
        d.set(
            "mesh_menu_hidden",
            self.hidden_mesh_menu
                .as_ref()
                .is_some_and(|m| m.is_instance_valid() && !m.is_visible()),
        );
        d
    }

    /// Test hook: insert a vertex on edge (a, b) at world `pos` (the insert-vertex click).
    #[func]
    fn debug_insert_vertex(&mut self, a: i64, b: i64, pos: Vector3) {
        let local = to_p(self.xform().affine_inverse() * pos);
        self.apply_edit("Pixel Builder: Insert vertex", |m, _| {
            pt_core::build::mesh_ops::insert_vertex(m, (a as u32, b as u32), local);
        });
    }

    /// Test hook: shape placement under a screen point (frame y axis included).
    #[func]
    fn debug_placement(&mut self, pos: Vector2) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return d;
        };
        if let Some(p) = self.locus(&cam, pos) {
            d.set("point", p.dot);
            d.set("up", p.frame.col_b());
        }
        d
    }

    /// Test hook: drop `material` (as a dragged resource) at `pos` of 3D viewport 0, through
    /// the same path as a drag-and-drop from the FileSystem / inspector. True when it landed.
    #[func]
    fn debug_drop_material(
        &mut self,
        material: Gd<godot::classes::Material>,
        pos: Vector2,
    ) -> bool {
        let Some(vp) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
        else {
            return false;
        };
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
        match self.ship_out_into(&path.to_string()) {
            Ok(_) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// Test / scripting hook: select edges by vertex pairs (Edge mode).
    #[func]
    fn debug_select_edges(&mut self, pairs: PackedInt32Array) {
        self.assign_setting(Mode::Edge);
        self.sel_edges = pairs
            .as_slice()
            .chunks_exact(2)
            .map(|p| pt_core::build::edge(p[0] as u32, p[1] as u32))
            .collect();
        self.redraw();
    }

    /// Test / scripting hook: simulate a shape drag in world space (start, corner, height).
    #[func]
    fn debug_draw_shape(&mut self, start: Vector3, corner: Vector3, height: f32) {
        let cell = self
            .terrain
            .as_ref()
            .map(|t| super::node::world_cell(t, start));
        self.shape_drag = Some(ShapeDrag {
            stage: 1,
            commence: Placement {
                dot: start,
                cell,
                frame: Basis::IDENTITY,
            },
            corner,
            stature: height,
            press: Vector2::ZERO,
            append_tail: false,
            height_from: Vector2::ZERO,
        });
        self.seal_form_exported();
    }

    /// Test / scripting hook: one vertex-paint stroke through world `points` (the face under
    /// each point is `faces[i]`, used by Fill), committed as one undo step.
    #[func]
    fn debug_vpaint(&mut self, points: PackedVector3Array, faces: PackedInt32Array) {
        if self.tool != Tool::VertexPaint {
            self.assign_instrument(Tool::VertexPaint);
        }
        let Some(data) = self.contents() else { return };
        let (before, base) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        self.vp.sweep = Some(vpaint::VStroke::new(before, base));
        for (i, p) in points.as_slice().iter().enumerate() {
            self.vp.hover = Some((
                faces.as_slice().get(i).copied().unwrap_or(0).max(0) as usize,
                *p,
            ));
            self.vp_paint_at();
        }
        if let Some(stroke) = self.vp.sweep.take() {
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
        let Some(data) = self.contents() else { return };
        let (before, start) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        let verts = self.selected_verts(&start);
        let weights = self.drag_weights(&start, &verts);
        let frames = self.uv_lock.then(|| {
            pt_core::build::uv_lock::capture_drag_frames(&start, &weights.keys().copied().collect())
        });
        let mut m = start.clone();
        let local = self.xform().basis.inverse() * delta;
        pt_core::build::xform::translate_weighted(&mut m, &weights, to_p(local));
        if let Some(f) = &frames {
            pt_core::build::uv_lock::refit_after_drag(&mut m, f);
        }
        let mut data = data;
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            self.commit_blob("Pixel Builder: Move", &data, before, after);
        }
        self.redraw();
    }

    /// Test / scripting hook: current mode, tool and selection sizes.
    #[func]
    fn fetch_condition(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("mode", self.mode.moniker());
        d.set("tool", self.tool.moniker());
        d.set("faces", self.sel_faces.len() as i64);
        d.set("edges", self.sel_edges.len() as i64);
        d.set("verts", self.sel_verts.len() as i64);
        d.set("uv_lock", self.uv_lock);
        d.set(
            "lock_meta",
            self.lock_target
                .as_ref()
                .is_some_and(|t| t.is_instance_valid() && t.has_meta(EDIT_LOCK_META)),
        );
        d.set("message", self.message.as_str());
        d.set("distance", self.span);
        d.set("kind", self.spec.sort as i64);
        d.set("slot", self.pocket);
        d.set("color", self.tint);
        d.set(
            "layout",
            self.ui.as_ref().map(|u| u.layout().moniker()).unwrap_or(""),
        );
        d
    }

    /// Test / scripting hook: select vertices (switches to Vertex mode).
    #[func]
    fn debug_select_verts(&mut self, verts: PackedInt32Array) {
        self.assign_setting(Mode::Vertex);
        self.sel_verts = verts
            .as_slice()
            .iter()
            .filter(|v| **v >= 0)
            .map(|v| *v as u32)
            .collect();
        // The first listed vertex counts as the first picked one ("Collapse to first").
        self.active_vert = verts
            .as_slice()
            .first()
            .filter(|v| **v >= 0)
            .map(|v| *v as u32);
        self.redraw();
    }

    /// Test / scripting hook: run a toolbar action by id and optionally select faces.
    #[func]
    fn debug_select_faces(&mut self, faces: PackedInt32Array) {
        self.assign_setting(Mode::Face);
        self.sel_faces = super::data::facet_batch(&faces);
        self.redraw();
    }
}

/// The builder mesh a multi-selection edits: `current` when it is selected, else the first
/// selected `PixelBuilderMesh` (None when the selection has none).
fn multi_target(current: Option<&Gd<PixelBuilderMesh>>) -> Option<Gd<PixelBuilderMesh>> {
    let sel = EditorInterface::singleton().get_selection()?;
    let meshes: Vec<Gd<PixelBuilderMesh>> = sel
        .get_selected_nodes()
        .iter_shared()
        .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
        .collect();
    if let Some(c) = current
        && meshes.iter().any(|m| m.instance_id() == c.instance_id())
    {
        return Some(c.clone());
    }
    meshes.into_iter().next()
}
