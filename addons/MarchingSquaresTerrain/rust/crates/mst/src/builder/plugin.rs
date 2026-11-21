//! The editor-side half of the builder: the tool someone uses to model low-poly level
//! geometry directly in the 3D viewport.
//!
//! Intent: a level author should be able to block out and refine buildings, props and
//! terrain pieces where they will be seen, without leaving the editor for another
//! modeling program. Picking a builder mesh turns the viewport into a modeler: stamp new
//! shapes onto the terrain grid, select faces, edges or vertices, move, extrude, bevel,
//! cut and combine them, paint and unwrap them, and export the result. Picking an
//! ordinary mesh or a constructive solid offers to turn it into a builder mesh.
//!
//! Responsibility: it owns the editing session, meaning what is selected, which mode and
//! tool are active, any drag in progress, the feedback drawn over the viewport, and
//! turning each finished edit into a single step in the editor's undo history. It
//! deliberately leaves the geometry rules to the engine-free mesh library, the stored
//! mesh to the mesh's data resource, rebuilding the visible mesh to the builder mesh node
//! itself, and the layout of its panels to the builder's interface modules.
//!
//! Why a node: the editor only lets a tool see viewport input, draw over the viewport,
//! follow the selection, record undo steps and add docks and menus through a plugin that
//! lives in the editor's own tree.
//!
//! Why a new type: no built-in editor plugin edits meshes. Everything this node does is
//! the reason it exists, and there is no smaller existing piece to configure instead.
//!
//! Not exported: no scene ever contains this node and no Inspector shows it, so there is
//! nothing for a scene author to set. The choices that outlast a session are the
//! person's preferences, which live in the editor's settings. Everything else is the
//! state of the edit in progress, the state of the plugin's own menus and panels, or a
//! handle the editor gives it while it runs, such as the mesh being edited, the active
//! camera and the terrain under the cursor.
use super::data::{PixelBuilderMeshData, from_p, to_p};
use super::modeler_ui::LayoutUi;
use super::node::PixelBuilderMesh;
use super::ui::{self, Action, Field, Mode, Tool, UvField, Values};
use crate::math_conversions::{from_iv, to_iv, to_v2};
use godot::classes::editor_plugin::AfterGuiInput;
use godot::classes::mesh::PrimitiveType;
use godot::classes::{
    Camera3D, Control, EditorInterface, EditorPlugin, IEditorPlugin, ImmediateMesh, Input,
    InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion, MeshInstance3D,
    Node3D, Object, ResourceSaver,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use pt_core::build::{BMesh, Edge, ShapeKind, ShapeSpec, ops, pick, shapes, snap};
use pt_core::math::{Vec3 as PVec3, v2};
use std::collections::BTreeSet;
mod actions;
mod adjust;
mod attr_overlay;
mod bezier;
mod boolean;
mod builder_menu;
mod context_menu;
mod debug_api;
mod drag;
mod editor_dialogs;
pub(super) mod editor_scenes;
mod extra;
mod gizmo;
mod gltf_export;
mod keys;
mod material_drop;
mod modeler;
mod new_scene_nodes;
mod objects;
mod overlay_mesh;
mod paint_cmds;
mod picking;
mod placement;
mod preferences;
mod ray_hits;
mod tools;
mod uv_studio;
mod viewport_input;
mod viewport_overlay;
mod vpaint;
mod xg_actions;
mod xg_io;
use super::prefs::{self, Prefs};
use super::ui::{SyncState, XformMode};
use super::uv_editor::{self, PixelBuilderUvCanvas};
use crate::terrain::MarchingSquaresTerrain;

use boolean::PendingBool;
use drag::Drag;
pub use extra::mesh_to_bmesh;
use extra::{PaintStroke, ShapeDrag};
use keys::is_undo_redo_key;
use picking::{Elem, Marquee};
use placement::{Placement, find_terrain};
use pt_core::build::xform::Weights;
use ray_hits::{closest_on_axis, plane_hit};
use tools::{Handle, Knife, RotateDrag, ScaleDrag};
use viewport_overlay::Feedback;
const PICK_PX: f32 = 10.0;
const ORANGE: Color = Color::from_rgba(1.0, 0.6005, 0.1505, 1.0);
#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelBuilderPlugin {
    target: Option<Gd<PixelBuilderMesh>>,
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
    xform_mode: XformMode,
    hover_handle: Option<Handle>,
    soft: bool,
    #[init(val = 1.0)]
    soft_radius: f32,
    falloff: i64,
    #[init(val = 15.0)]
    rot_snap: f32,
    snap_vertex: bool,
    #[init(val = true)]
    uv_lock: bool,
    #[init(val = vpaint::VPaint::default())]
    vp: vpaint::VPaint,
    mirror_axis: i64,
    knife: Knife,
    reshape: bool,
    pending_bool: Option<PendingBool>,
    #[init(val = [0.0, 0.0, 1.0, 1.0, 0.0])]
    tex_coord: [f32; 5],
    #[init(val = true)]
    uv_world: bool,
    shape_drag: Option<ShapeDrag>,
    wall_pts: Vec<Vector3>,
    wall_cursor: Option<Vector3>,
    coat: Option<PaintStroke>,
    convert_src: Option<Gd<Node3D>>,
    export_dialog: Option<Gd<godot::classes::EditorFileDialog>>,
    palette_slots: Option<Vec<i64>>,
    message: String,
    ui: Option<LayoutUi>,
    hud_key: String,
    inspector: Option<Gd<super::inspector::PixelBuilderInspector>>,
    gizmo: Option<Gd<MeshInstance3D>>,
    camera: Option<Gd<Camera3D>>,
    visible: bool,
    #[init(val = Prefs::default())]
    prefs: Prefs,
    prefs_panel: Option<prefs::Panel>,
    uv_window: Option<Gd<godot::classes::Window>>,
    uv_canvas: Option<Gd<PixelBuilderUvCanvas>>,
    uv_panel: Option<Gd<godot::classes::PanelContainer>>,
    uv_dock: Option<Gd<godot::classes::EditorDock>>,
    lock_target: Option<Gd<PixelBuilderMesh>>,
    drop: material_drop::MaterialDrag,
    #[init(val = ui::ExtraOpts::default())]
    opts: ui::ExtraOpts,
    #[init(val = ui::SWATCHES)]
    swatches: [Color; 8],
    insert_vert: bool,
    insert_hover: Option<(Edge, Vector3)>,
    surface_draw: bool,
    xray: bool,
    #[init(val = Basis::IDENTITY)]
    wall_basis: Basis,
    builder_menu: Option<Gd<godot::classes::MenuButton>>,
    builder_menu_failed: bool,
    hidden_mesh_menu: Option<Gd<Control>>,
    #[init(val = ui::XgOpts::default())]
    xg: ui::XgOpts,
    active_vert: Option<u32>,
    bz: bezier::BezierTool,
    poly_outline: Option<Vec<Vector3>>,
    poly_holes: Vec<Vec<Vector3>>,
    #[init(val = xg_io::ExportSettings::default())]
    export_opts: xg_io::ExportSettings,
    xg_dialogs: xg_io::Dialogs,
    #[init(val = xg_io::classic_convert())]
    convert_opts: pt_core::build::convert::ImportOptions,
    adjust: Option<adjust::Adjust>,
    adjust_ui: Option<adjust::AdjustUi>,
    adjust_running: bool,
    adjust_coarse: bool,
    no_adjust: bool,
    no_confirm: bool,
    auto_confirm: bool,
    last_confirmation: String,
    bevel_status: String,
    ctx_menu: Option<Gd<godot::classes::PopupMenu>>,
    ctx_state: Option<(Mode, Option<InstanceId>, usize)>,
    ctx_press: Option<Vector2>,
    ctx_travel: f32,
    ctx_pending: Option<Vector2>,
    shortcut_table: Vec<Option<Gd<godot::classes::Shortcut>>>,
    attr_node: Option<Gd<MeshInstance3D>>,
    attr_key: Option<attr_overlay::AttrKey>,
    attr_rebuilds: i64,
    attr_counts: (i64, i64),
    attr_data: Option<Gd<PixelBuilderMeshData>>,
    inspector_key: Option<(i64, i32, bool)>,
    chip_key: Option<(Vec<(u16, usize)>, u16)>,
    palette_dialog: Option<Gd<godot::classes::EditorFileDialog>>,
    palette_dialog_save: bool,
    hover_edge: Option<Edge>,
    hover_vert: Option<u32>,
    feedback: Feedback,
    base: Base<EditorPlugin>,
}
pub(super) const PLUGIN_LOCK_META: &str = "_pt_builder_lock";
const EDIT_LOCK_META: &str = "_edit_lock_";
#[godot_api]
impl IEditorPlugin for PixelBuilderPlugin {
    fn enter_tree(&mut self) {
        self.prefs = Prefs::ingest();
        self.apply_prefs(true);
        self.shortcut_table = super::shortcuts::enroll();
        if let Some(mut es) = EditorInterface::singleton().get_editor_settings() {
            let cb = self.to_gd().callable("on_editor_settings_changed");
            if !es.is_connected("settings_changed", &cb) {
                es.connect_flags(
                    "settings_changed",
                    &cb,
                    godot::classes::object::ConnectFlags::DEFERRED,
                );
            }
        }
        self.add_inspector();
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
        if crate::editor::pixel_handles(&entity) {
            return false;
        }
        if entity.is_class("MultiNodeEdit") {
            return multi_target(None).is_some();
        }
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
        self.drop_freed();
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
        self.terrain = self
            .target
            .as_ref()
            .and_then(|t| t.bind().terrain_or_found())
            .and_then(|t| t.try_cast::<MarchingSquaresTerrain>().ok())
            .or_else(find_terrain);
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
        self.sync_ui();
    }
    fn forward_3d_gui_input(
        &mut self,
        lens: Option<Gd<Camera3D>>,
        happening: Option<Gd<InputEvent>>,
    ) -> i32 {
        self.route_viewport_input(lens, happening)
    }
    fn forward_3d_draw_over_viewport(&mut self, overlay: Option<Gd<Control>>) {
        self.draw_viewport_overlay(overlay)
    }
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
impl PixelBuilderPlugin {
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
    fn apply_edit(&mut self, name: &str, f: impl FnOnce(&mut BMesh, &mut Self)) {
        self.adjust_commit();
        let Some(mut data) = self.contents() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let (before, mut m) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
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
        self.adjust_commit();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(name);
        ur.add_do_method(data, "set_blob", &[after.to_variant()]);
        ur.add_undo_method(data, "set_blob", &[before.to_variant()]);
        ur.commit_action_ex().execute(false).done();
    }
    fn assign_setting(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        self.adjust_commit();
        self.hover_face = None;
        self.hover_edge = None;
        self.hover_vert = None;
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
}
fn in_main_screen(pos: Vector2) -> bool {
    EditorInterface::singleton()
        .get_editor_main_screen()
        .is_some_and(|m| m.get_global_rect().contains_point(pos))
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
    #[func]
    fn on_button_hover(&mut self, token_id: i64) {
        let text = Action::from_i64(token_id)
            .map(ui::hover_text)
            .unwrap_or_default();
        if let Some(LayoutUi::Mst(u)) = &mut self.ui {
            u.hover.set_text(&text);
        }
    }
    #[func]
    fn on_swatch(&mut self, idx: i64) {
        let i = idx.clamp(0, 7) as usize;
        let input = Input::singleton();
        if input.is_key_pressed(Key::CTRL) || input.is_key_pressed(Key::META) {
            self.swatch_store(i);
        } else if input.is_key_pressed(Key::ALT) {
            let c = self.swatches[i];
            self.select_faces_with_colour(c);
        } else {
            self.tint = self.swatches[i];
            self.sync_ui();
        }
    }
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
}
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
