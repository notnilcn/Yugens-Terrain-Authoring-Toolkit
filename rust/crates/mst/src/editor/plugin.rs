//! `MarchingSquaresTerrainPlugin` — the terrain authoring plugin (auto-registered by godot-rust; no
//! plugin.cfg needed). Selecting a `MarchingSquaresTerrain` shows the original MST layout: an icon
//! toolbar on the left of the 3D viewport, the tool attribute strip at the bottom and (Vertex
//! Painting only) the texture panel on the right. UI construction lives in `terrain_ui.rs`, the
//! viewport visuals in `cursor.rs`.
//!
//! Every stroke snapshots the chunks it touches before the first change and records one
//! undo action (`set_chunks_data` with before/after blobs) when the mouse is released.

use std::collections::HashMap;

use godot::classes::editor_plugin::{AfterGuiInput, CustomControlContainer};
use godot::classes::{
    Button, Camera3D, Control, EditorPlugin, IEditorPlugin, Input, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion,
    Object, OptionButton, ResourceSaver, ScrollContainer, VBoxContainer, VFlowContainer,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::brush::{self, Clamp, PaintTarget, Touched};
use pt_core::math::{IVec2, v2};

use super::cursor::{self, Cursor};
use super::tools::{EXTRA_TOOLS, QUICK_PAINTS, Setting, State, TOOLS, Tool};
use crate::terrain::MarchingSquaresTerrain;
use crate::util::*;

pub(super) struct Stroke {
    pub tool: Tool,
    pub before: HashMap<IVec2, Option<Vec<u8>>>,
    /// Brush/Level: footprint + original heights + drag origin.
    pub pattern: Vec<(IVec2, f32)>,
    pub base_heights: Vec<(IVec2, f32)>,
    pub start_mouse_y: f32,
    pub start_local: Vector3,
    pub world_per_pixel: f32,
    pub lmb: bool,
    pub rmb: bool,
    /// Scatter strokes: the scatter blob before the stroke (undo).
    pub scatter_before: Option<Vec<u8>>,
    /// Tint strokes: the painted tint blob before the stroke (undo).
    pub tint_before: Option<Vec<u8>>,
}

#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct MarchingSquaresTerrainPlugin {
    pub(super) terrain: Option<Gd<MarchingSquaresTerrain>>,
    pub(super) state: State,
    pub(super) toolbar: Option<Gd<VFlowContainer>>,
    pub(super) tool_buttons: Vec<Gd<Button>>,
    pub(super) strip: Option<Gd<ScrollContainer>>,
    pub(super) texture_panel: Option<Gd<ScrollContainer>>,
    pub(super) texture_vbox: Option<Gd<VBoxContainer>>,
    pub(super) setting_controls: Vec<(Setting, Gd<Control>)>,
    pub(super) chunk_list: Option<Gd<OptionButton>>,
    /// Terrain Settings page shown in the strip (index into `terrain_ui::SETTINGS_PAGES`).
    pub(super) settings_page: usize,
    pub(super) slot_buttons: Vec<Gd<Button>>,
    pub(super) slot_previews: Vec<Gd<godot::classes::TextureRect>>,
    /// Ground texture picker per slot (index = slot), updated when the pattern changes.
    pub(super) slot_tex_pickers: Vec<(usize, Gd<godot::classes::EditorResourcePicker>)>,
    pub(super) cursor: Option<Cursor>,
    pub(super) hover: Option<Vector3>,
    pub(super) hover_chunk: Option<IVec2>,
    pub(super) selected_chunk: Option<IVec2>,
    pub(super) selection: Vec<(IVec2, f32)>,
    pub(super) stroke: Option<Stroke>,
    pub(super) visible: bool,
    /// Terrains whose editor view was already checked by `frame_if_buried` this session.
    framed: Vec<InstanceId>,
    /// State of the Rust tools appended after the original nine (`extra_tools.rs`).
    pub(super) extra: super::extra_tools::Extra,
    /// Hides Godot's transform gizmo on the terrain while a painting tool is active.
    gizmo_lock: super::gizmo_lock::GizmoLock,
    /// Short inspector labels for long property names (`inspector_labels.rs`).
    labels: Option<Gd<super::inspector_labels::MstInspectorLabels>>,
    /// Vertex handle being dragged (sample, chunk blobs before the drag).
    handle_drag: Option<(Vector2i, VarDictionary)>,
    base: Base<EditorPlugin>,
}

const LEFT: CustomControlContainer = CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT;
const BOTTOM: CustomControlContainer = CustomControlContainer::SPATIAL_EDITOR_BOTTOM;
const RIGHT: CustomControlContainer = CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT;

#[godot_api]
impl IEditorPlugin for MarchingSquaresTerrainPlugin {
    fn enter_tree(&mut self) {
        self.state = State::default();
        let toolbar = self.build_toolbar();
        let strip = super::ui_kit::strip_root("MarchingSquaresToolAttributes");
        let (mut panel, vbox) = super::ui_kit::side_panel("MarchingSquaresTextureSettings");
        // Wider than the original 195 px: rows also hold pattern, color and grass sprite pickers.
        panel.set_custom_minimum_size(super::ui_kit::sz(250.0, 0.0));
        for (c, ctl) in [(LEFT, toolbar.clone().upcast::<Control>()), (BOTTOM, strip.clone().upcast()), (RIGHT, panel.clone().upcast())] {
            self.base_mut().add_control_to_container(c, &ctl);
            let mut ctl = ctl;
            ctl.set_visible(false);
        }
        self.toolbar = Some(toolbar);
        self.strip = Some(strip);
        self.texture_panel = Some(panel);
        self.texture_vbox = Some(vbox);
        self.select_tool(Tool::Brush);
        self.base_mut().set_process(true);
        let cb = self.base().callable("_mst_on_scene_changed");
        self.base_mut().connect("scene_changed", &cb);
        super::chunk_gizmo::register(&mut self.to_gd().upcast());
        let labels = super::inspector_labels::MstInspectorLabels::new_gd();
        self.base_mut().add_inspector_plugin(&labels);
        self.labels = Some(labels);
    }

    fn exit_tree(&mut self) {
        self.gizmo_lock.clear();
        if let Some(l) = self.labels.take() {
            self.base_mut().remove_inspector_plugin(&l);
        }
        super::chunk_gizmo::unregister(&mut self.to_gd().upcast());
        self.drop_cursor();
        self.extra_drop();
        let parts: [(CustomControlContainer, Option<Gd<Control>>); 3] = [
            (LEFT, self.toolbar.take().map(|c| c.upcast())),
            (BOTTOM, self.strip.take().map(|c| c.upcast())),
            (RIGHT, self.texture_panel.take().map(|c| c.upcast())),
        ];
        for (c, ctl) in parts {
            if let Some(ctl) = ctl {
                self.base_mut().remove_control_from_container(c, &ctl);
                ctl.free();
            }
        }
        self.tool_buttons.clear();
        self.setting_controls.clear();
        self.chunk_list = None;
        self.texture_vbox = None;
        self.slot_buttons.clear();
        self.slot_previews.clear();
        self.slot_tex_pickers.clear();
    }

    fn get_plugin_name(&self) -> GString {
        "Marching Squares Terrain".into()
    }

    fn handles(&self, object: Gd<Object>) -> bool {
        object.try_cast::<MarchingSquaresTerrain>().is_ok()
    }

    fn edit(&mut self, object: Option<Gd<Object>>) {
        self.finish_stroke();
        self.drop_cursor();
        self.extra_drop();
        self.selection.clear();
        self.terrain = object.and_then(|o| o.try_cast::<MarchingSquaresTerrain>().ok());
        if let Some(t) = self.terrain.clone() {
            self.ensure_cursor();
            self.select_tool(self.state.tool);
            self.queue_frame(t.upcast());
            self.sync_grid_toggle();
        }
    }

    fn make_visible(&mut self, visible: bool) {
        self.visible = visible;
        if let Some(p) = &mut self.toolbar {
            p.set_visible(visible);
        }
        if let Some(p) = &mut self.strip {
            p.set_visible(visible);
        }
        let paint = visible && self.state.tool == Tool::VertexPaint;
        if let Some(p) = &mut self.texture_panel {
            p.set_visible(paint);
        }
        if let Some(g) = &mut self.cursor {
            g.set_visible(visible);
        }
        self.extra_set_visible(visible);
        if !visible {
            self.finish_stroke();
        }
    }

    fn process(&mut self, delta: f64) {
        self.sync_gizmo_lock();
        let Some(stroke) = &self.stroke else { return };
        let (tool, lmb, rmb) = (stroke.tool, stroke.lmb, stroke.rmb);
        let Some(hover) = self.hover else { return };
        let dt = delta as f32;
        let s = self.state.clone();
        match tool {
            Tool::RaiseLower => {
                if lmb && rmb {
                    self.apply_brush(hover, |st, fp| brush::op_level(st, fp, s.level, (s.raise_strength * dt * 0.25).min(1.0)));
                } else {
                    let amount = s.raise_strength * dt * if lmb { 1.0 } else { -1.0 };
                    let clamp = Clamp { floor: s.floor, ceiling: s.ceiling };
                    self.apply_brush(hover, |st, fp| brush::op_raise(st, fp, amount, clamp));
                }
                if lmb {
                    self.quick_paint_at_hover();
                }
            }
            Tool::Smooth => {
                // Strength is per 1/60 s (the original applies once per motion event).
                let k = 1.0 - (1.0 - s.smooth_strength.clamp(0.0, 1.0)).powf(dt * 60.0);
                self.apply_brush(hover, |st, fp| brush::op_smooth(st, fp, k));
                self.quick_paint_at_hover();
            }
            _ => {}
        }
    }

    fn forward_3d_gui_input(&mut self, camera: Option<Gd<Camera3D>>, event: Option<Gd<InputEvent>>) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let stop = AfterGuiInput::STOP.ord();
        let (Some(camera), Some(event)) = (camera, event) else { return pass };
        if self.terrain.is_none() || !self.visible {
            return pass;
        }

        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            return if self.on_key(&key) { stop } else { pass };
        }

        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            if self.handle_motion(&camera, mm.get_position()) {
                return stop;
            }
            self.update_hover(&camera, mm.get_position());
            if self.stroke.is_some() {
                self.stroke_motion(mm.get_position());
            }
            self.redraw();
            return if self.stroke.is_some() { stop } else { pass };
        }

        if let Ok(mb) = event.try_cast::<InputEventMouseButton>() {
            let input = Input::singleton();
            let shift = input.is_key_pressed(Key::SHIFT);
            let ctrl = input.is_key_pressed(Key::CTRL);
            let button = mb.get_button_index();
            if mb.is_pressed() && (button == MouseButton::WHEEL_UP || button == MouseButton::WHEEL_DOWN) {
                let up = button == MouseButton::WHEEL_UP;
                if self.extra_wheel(up, shift, ctrl) {
                    return stop;
                }
                let factor = mb.get_factor().max(1.0) as f64;
                if shift && self.state.tool.uses_brush() {
                    let size = self.state.get(Setting::Size);
                    let step = (0.5 * self.size_scale()).max(size * 0.08) * factor;
                    let max = 50.0 * self.size_scale();
                    self.set_setting(Setting::Size, (size + if up { step } else { -step }).clamp(1.0 * self.size_scale().min(1.0), max));
                    self.redraw();
                    return stop;
                }
                if ctrl && matches!(self.state.tool, Tool::Smooth | Tool::RaiseLower | Tool::Gradient) {
                    let s = self.state.strength() as f64 * if up { 1.1 } else { 1.0 / 1.1 };
                    self.set_setting(Setting::Strength, s);
                    self.redraw();
                    return stop;
                }
                return pass;
            }
            if self.handle_press(&camera, &mb) {
                return stop;
            }
            self.update_hover(&camera, mb.get_position());
            let consumed = self.on_mouse_button(&camera, &mb, shift, ctrl);
            self.redraw();
            return if consumed { stop } else { pass };
        }
        pass
    }

    fn forward_3d_draw_over_viewport(&mut self, overlay: Option<Gd<Control>>) {
        // Small status line (Rust extra): what is under the cursor.
        let Some(overlay) = overlay else { return };
        let Some(font) = overlay.get_theme_default_font() else { return };
        let (Some(t), Some(h)) = (&self.terrain, self.hover) else { return };
        let line = {
            let t = t.bind();
            let s = t.store().config.world_to_sample(v2(h.x, h.z));
            let c = t.store().config.chunk_of(s);
            let mut l = format!("{}  |  height {:.2}  cell ({}, {})  chunk ({}, {})", self.state.tool.name(), h.y, s.x, s.y, c.x, c.y);
            if !self.selection.is_empty() {
                l.push_str(&format!("  |  {} selected", self.selection.len()));
            }
            if let Some(e) = self.extra_status() {
                l.push_str(&e);
            }
            l
        };
        let size = overlay.get_size();
        let pos = Vector2::new(10.0, size.y - 10.0);
        overlay.draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), line.as_str()).font_size(13).modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8)).done();
        overlay.draw_string_ex(&font, pos, line.as_str()).font_size(13).modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.85)).done();
    }

    fn apply_changes(&mut self) {
        // Runs right before the editor packs a scene: the gizmo lock must not be saved
        // (`process` puts it back on the next frame).
        self.gizmo_lock.strip();
    }

    fn save_external_data(&mut self) {
        // Save the data of every terrain in the edited scene (not just the selected one), so
        // strokes, undo and redo all persist regardless of what is selected at save time.
        let Some(root) = godot::classes::EditorInterface::singleton().get_edited_scene_root() else { return };
        let mut terrains: Vec<Gd<Node>> = root.find_children_ex("*").type_("MarchingSquaresTerrain").owned(false).done().iter_shared().collect();
        if root.clone().try_cast::<MarchingSquaresTerrain>().is_ok() {
            terrains.push(root.clone());
        }
        for t in terrains {
            let Ok(mut data) = t.get("data").try_to::<Gd<crate::terrain::data::MarchingSquaresTerrainData>>() else { continue };
            let res = data.clone().upcast::<godot::classes::Resource>();
            let path = res.get_path().to_string();
            if path.is_empty() || path.contains("::") {
                continue; // embedded: saved with the scene
            }
            // Only data edited since the last save (strokes, undo, redo, clears): saving the
            // scene must not rewrite (and dirty in VCS) terrain data nobody touched.
            if !data.bind_mut().take_unsaved_edits() {
                continue;
            }
            let err = ResourceSaver::singleton().save(&res);
            if err != godot::global::Error::OK {
                godot_error!("Marching Squares Terrain: could not save {path}: {err:?}");
                data.bind_mut().mark_unsaved_edits();
            }
        }
    }
}

impl MarchingSquaresTerrainPlugin {
    /// Godot's move / rotate gizmo is drawn over the terrain and hides the brush cursor: lock it
    /// away in every MST tool mode, so the terrain behaves the same whatever tool is picked.
    /// Chunk Manager's "+" / remove buttons are our own overlay (clicks go through
    /// `forward_3d_gui_input`). Godot ignores gizmo handles on a locked node, so the vertex
    /// handles (`chunk_handles`, drawn by `chunk_gizmo`) are dragged by this plugin instead
    /// (`handle_press` / `handle_motion`, Chunk Manager and Terrain Settings).
    fn sync_gizmo_lock(&mut self) {
        let want = self.terrain.clone().filter(|_| self.visible).map(|t| t.upcast::<godot::classes::Node3D>());
        self.gizmo_lock.sync(want);
    }

    /// LMB press / release on a vertex handle (Chunk Manager, Terrain Settings): starts / ends a
    /// drag of that sample (one undo step). RMB during a drag cancels it. True = consumed.
    fn handle_press(&mut self, camera: &Gd<Camera3D>, mb: &Gd<InputEventMouseButton>) -> bool {
        let Some(t) = self.terrain.clone().map(|t| t.upcast::<godot::classes::Node3D>()) else { return false };
        let mut t = t;
        let (button, pressed) = (mb.get_button_index(), mb.is_pressed());
        if self.handle_drag.is_some() {
            if button == MouseButton::LEFT && !pressed {
                let (_, before) = self.handle_drag.take().unwrap();
                super::chunk_gizmo::commit_drag(&mut t, before, false);
                return true;
            }
            if button == MouseButton::RIGHT && pressed {
                let (_, before) = self.handle_drag.take().unwrap();
                super::chunk_gizmo::commit_drag(&mut t, before, true);
                return true;
            }
            return true;
        }
        if button != MouseButton::LEFT || !pressed || !matches!(self.state.tool, Tool::ChunkManager | Tool::TerrainSettings) {
            return false;
        }
        let r = 12.0 * super::ui_kit::editor_scale();
        let Some(cell) = super::chunk_gizmo::handle_at(&t, camera, mb.get_position(), r) else { return false };
        let chunk = t.get("handle_chunk").try_to::<Vector2i>().unwrap_or_default();
        let before = super::chunk_gizmo::MarchingSquaresChunkGizmoPlugin::snapshot(&mut t, chunk);
        self.handle_drag = Some((cell, before));
        true
    }

    /// Mouse motion during a vertex-handle drag (see `handle_press`).
    fn handle_motion(&mut self, camera: &Gd<Camera3D>, pos: Vector2) -> bool {
        let Some((cell, _)) = &self.handle_drag else { return false };
        let cell = *cell;
        if let Some(t) = self.terrain.clone() {
            super::chunk_gizmo::drag_height(&mut t.upcast(), cell, camera, pos);
        }
        true
    }

    fn queue_frame(&mut self, terrain: Gd<Node>) {
        let id = terrain.instance_id();
        if self.framed.contains(&id) {
            return;
        }
        self.framed.push(id);
        let Ok(t) = terrain.try_cast::<Node3D>() else { return };
        let cb = self.base().callable("_mst_frame_if_buried").bind(&[t.to_variant()]);
        cb.call_deferred(&[]);
    }

    // ------------------------------------------------------------------ input

    fn terrain_transform(&self) -> Transform3D {
        self.terrain.as_ref().map(|t| t.get_global_transform()).unwrap_or(Transform3D::IDENTITY)
    }

    /// Terrain-local ray from a viewport position.
    fn local_ray(&self, camera: &Gd<Camera3D>, pos: Vector2) -> (Vector3, Vector3) {
        let inv = self.terrain_transform().affine_inverse();
        let o = camera.project_ray_origin(pos);
        let d = camera.project_ray_normal(pos);
        (inv * o, (inv.basis * d).normalized())
    }

    fn update_hover(&mut self, camera: &Gd<Camera3D>, pos: Vector2) {
        let (o, d) = self.local_ray(camera, pos);
        let Some(t) = &self.terrain else { return };
        let hit = t.bind().local_raycast(o, d, 20_000.0);
        let plane_h = self.stroke.as_ref().filter(|s| s.tool == Tool::Brush).map(|s| s.start_local.y).unwrap_or(0.0);
        let plane = |h: f32| {
            if d.y.abs() < 1e-5 {
                return None;
            }
            let k = (h - o.y) / d.y;
            (k > 0.0).then(|| o + d * k)
        };
        self.hover = hit.or_else(|| plane(plane_h));
        let chunk_mode = self.state.tool == Tool::ChunkManager;
        let tb = t.bind();
        // Chunk Manager works on the y = 0 plane (like the original) so empty slots are reachable.
        let p = if chunk_mode { plane(0.0).or(self.hover) } else { self.hover.or_else(|| plane(0.0)) };
        self.hover_chunk = p.map(|p| tb.store().config.chunk_at(v2(p.x, p.z)));
        drop(tb);
        self.base().update_overlays();
    }

    fn on_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        if self.extra_key(key) {
            return true;
        }
        let k = key.get_keycode();
        if k == Key::ESCAPE || (k == Key::ALT && self.stroke.is_none()) {
            // Original tooltip: "Alt/RMB/Esc: Reset the current draw selection".
            let had = !self.selection.is_empty();
            self.selection.clear();
            if k == Key::ESCAPE
                && let Some(s) = self.stroke.take()
            {
                self.cancel_stroke(s);
            }
            self.redraw();
            return had || k == Key::ESCAPE;
        }
        if key.is_ctrl_pressed() || key.is_alt_pressed() || key.is_meta_pressed() || key.is_shift_pressed() {
            return false;
        }
        let digits = [Key::KEY_1, Key::KEY_2, Key::KEY_3, Key::KEY_4, Key::KEY_5, Key::KEY_6, Key::KEY_7, Key::KEY_8, Key::KEY_9];
        if let Some(t) = digits.iter().position(|d| *d == k).and_then(|i| TOOLS.get(i)) {
            self.select_tool(*t);
            return true;
        }
        false
    }

    fn on_mouse_button(&mut self, camera: &Gd<Camera3D>, mb: &Gd<InputEventMouseButton>, shift: bool, ctrl: bool) -> bool {
        let button = mb.get_button_index();
        let pressed = mb.is_pressed();
        let tool = self.state.tool;
        let is_l = button == MouseButton::LEFT;
        let is_r = button == MouseButton::RIGHT;
        let is_m = button == MouseButton::MIDDLE;
        if tool == Tool::TerrainSettings {
            return false;
        }
        if tool.is_extra() {
            return self.extra_mouse_button(camera, mb, shift, ctrl);
        }

        if !pressed {
            if let Some(s) = &mut self.stroke {
                if is_l {
                    s.lmb = false;
                }
                if is_r {
                    s.rmb = false;
                }
                let (tool, l, r) = (s.tool, s.lmb, s.rmb);
                if tool == Tool::Bridge && is_l {
                    self.finish_bridge();
                    return true;
                }
                if !l && !r {
                    self.finish_stroke();
                }
                return true;
            }
            return false;
        }

        // Picking shortcuts.
        if ctrl && let Some(h) = self.hover {
            match tool {
                Tool::RaiseLower => {
                    let s = if is_l {
                        Setting::Ceiling
                    } else if is_m {
                        Setting::Height
                    } else {
                        Setting::Floor
                    };
                    self.set_setting(s, h.y as f64);
                    return true;
                }
                Tool::Level if is_l => {
                    self.set_setting(Setting::Height, h.y as f64);
                    return true;
                }
                Tool::VertexPaint if is_l => {
                    if let Some(t) = &self.terrain {
                        let tb = t.bind();
                        let s = tb.store().config.world_to_sample(v2(h.x, h.z));
                        if let Some(sm) = tb.store().sample(s) {
                            let slot = if self.state.paint_walls { sm.wall } else { sm.floor };
                            drop(tb);
                            self.set_setting(Setting::Material, slot as f64);
                            self.refresh_texture_panel_selection();
                        }
                    }
                    return true;
                }
                Tool::ChunkManager if is_l => {
                    let c = self.hover_chunk.filter(|c| self.terrain.as_ref().is_some_and(|t| t.bind().store().has_chunk(*c)));
                    self.select_chunk(c);
                    return true;
                }
                _ => {}
            }
        }
        if ctrl && tool == Tool::ChunkManager && is_l {
            return true;
        }

        let alt = Input::singleton().is_key_pressed(Key::ALT);
        if tool.uses_brush() && (is_r || alt) && self.stroke.is_none() && !matches!(tool, Tool::RaiseLower | Tool::GrassMask) {
            // RMB / Alt reset the draw selection (original behaviour, all brush tools).
            let had = !self.selection.is_empty();
            self.selection.clear();
            return is_r && had;
        }
        if !(is_l || (is_r && matches!(tool, Tool::RaiseLower | Tool::GrassMask))) {
            return false;
        }
        if let Some(s) = &mut self.stroke {
            // Second button during a raise stroke (LMB+RMB = flatten).
            if is_l {
                s.lmb = true;
            }
            if is_r {
                s.rmb = true;
            }
            return true;
        }
        match tool {
            Tool::ChunkManager => {
                self.chunk_click();
                true
            }
            Tool::Debug => {
                self.debug_print();
                true
            }
            _ => {
                let Some(h) = self.hover else { return true };
                if shift && tool != Tool::RaiseLower {
                    // Shift+LMB(+drag): accumulate a draw selection, applied by the next plain drag.
                    let fp = self.footprint(h);
                    merge_selection(&mut self.selection, &fp);
                    self.stroke = Some(self.new_stroke(camera, mb, Tool::Debug, h, is_l, is_r));
                    return true;
                }
                self.stroke = Some(self.new_stroke(camera, mb, tool, h, is_l, is_r));
                self.stroke_begin();
                true
            }
        }
    }

    /// A stroke whose `tool` is `Debug` only accumulates the shift selection.
    pub(super) fn new_stroke(&self, camera: &Gd<Camera3D>, mb: &Gd<InputEventMouseButton>, tool: Tool, h: Vector3, l: bool, r: bool) -> Stroke {
        Stroke {
            tool,
            before: HashMap::new(),
            pattern: Vec::new(),
            base_heights: Vec::new(),
            start_mouse_y: mb.get_position().y,
            start_local: h,
            world_per_pixel: self.world_per_pixel(camera, h),
            lmb: l,
            rmb: r,
            scatter_before: None,
            tint_before: None,
        }
    }

    fn world_per_pixel(&self, camera: &Gd<Camera3D>, hit_local: Vector3) -> f32 {
        let vh = camera.get_viewport().map(|v| v.get_visible_rect().size.y).unwrap_or(720.0).max(1.0);
        if camera.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL {
            camera.get_size() / vh
        } else {
            let world = self.terrain_transform() * hit_local;
            let dist = camera.get_global_position().distance_to(world);
            2.0 * dist * (camera.get_fov().to_radians() * 0.5).tan() / vh
        }
    }

    pub(super) fn footprint(&self, at: Vector3) -> Vec<(IVec2, f32)> {
        let Some(t) = &self.terrain else { return Vec::new() };
        brush::footprint(&t.bind().store().config, &self.state.brush, v2(at.x, at.z))
    }

    /// Snapshots the chunks a footprint may touch, then runs `op` and rebuilds immediately.
    pub(super) fn apply_footprint(&mut self, fp: &[(IVec2, f32)], op: impl FnOnce(&mut pt_core::data::TerrainStore, &[(IVec2, f32)]) -> Touched) {
        let Some(mut t) = self.terrain.clone() else { return };
        if let Some(stroke) = &mut self.stroke {
            let tb = t.bind();
            for (s, _) in fp {
                for c in tb.store().chunks_affected_by(*s) {
                    stroke.before.entry(c).or_insert_with(|| tb.chunk_bytes(c));
                }
            }
        }
        let mut tb = t.bind_mut();
        tb.edit(|st| op(st, fp));
        tb.update();
    }

    fn apply_brush(&mut self, at: Vector3, op: impl FnOnce(&mut pt_core::data::TerrainStore, &[(IVec2, f32)]) -> Touched) {
        let fp = self.footprint(at);
        self.apply_footprint(&fp, op);
    }

    /// Quick paint (original `MarchingSquaresQuickPaint`): ground/wall slot + grass with the height tools.
    fn quick_paint(&mut self, fp: &[(IVec2, f32)]) {
        let qp = &QUICK_PAINTS[self.state.quick_paint.min(QUICK_PAINTS.len() - 1)];
        if self.state.quick_paint == 0 {
            return;
        }
        let slot = self.state.paint_slot;
        let (ground, wall, grass) = (qp.ground.unwrap_or(slot), qp.wall, qp.grass);
        self.apply_footprint(fp, |st, fp| {
            let mut t = brush::op_paint(st, fp, PaintTarget::Floor, ground);
            if let Some(w) = wall {
                t.extend(brush::op_paint(st, fp, PaintTarget::Wall, w));
            }
            if let Some(g) = grass {
                t.extend(brush::op_paint(st, fp, PaintTarget::Grass, if g { 255 } else { 0 }));
            }
            t
        });
    }

    fn quick_paint_at_hover(&mut self) {
        if self.state.quick_paint == 0 {
            return;
        }
        if let Some(h) = self.hover {
            let fp = self.footprint(h);
            self.quick_paint(&fp);
        }
    }

    fn base_heights(&self, pattern: &[(IVec2, f32)]) -> Vec<(IVec2, f32)> {
        let t = self.terrain.as_ref().unwrap().bind();
        pattern.iter().map(|(c, _)| (*c, t.store().height(*c).unwrap_or(0.0))).collect()
    }

    fn stroke_begin(&mut self) {
        let Some(h) = self.hover else { return };
        let s = self.state.clone();
        match s.tool {
            Tool::Brush | Tool::Level => {
                let pattern = if self.selection.is_empty() { self.footprint(h) } else { std::mem::take(&mut self.selection) };
                let heights = self.base_heights(&pattern);
                // Snapshot now: Brush keeps the pattern for the whole drag.
                self.apply_footprint(&pattern, |_, _| Touched::new());
                self.quick_paint(&pattern);
                if let Some(st) = &mut self.stroke {
                    st.pattern = pattern;
                    st.base_heights = heights;
                }
                if s.tool == Tool::Level {
                    self.apply_level();
                }
            }
            Tool::VertexPaint => {
                let target = if s.paint_walls { PaintTarget::Wall } else { PaintTarget::Floor };
                self.paint_filtered(h, target, s.paint_slot);
            }
            Tool::GrassMask => self.grass_at_hover(),
            _ => {}
        }
    }

    fn grass_at_hover(&mut self) {
        let Some(h) = self.hover else { return };
        let lmb = self.stroke.as_ref().map(|s| s.lmb && !s.rmb).unwrap_or(true);
        let on = lmb != self.state.mask_mode;
        self.paint_filtered(h, PaintTarget::Grass, if on { 255 } else { 0 });
    }

    /// Level: every sample the drag has covered moves to `Height`, blended by its brush weight.
    fn apply_level(&mut self) {
        let level = self.state.level;
        let Some(st) = self.stroke.as_ref() else { return };
        let (pattern, base) = (st.pattern.clone(), st.base_heights.clone());
        self.apply_footprint(&pattern, |store, fp| brush::op_offset(store, fp, &base, 0.0, Some(level)));
    }

    fn stroke_motion(&mut self, mouse: Vector2) {
        let Some(stroke) = &self.stroke else { return };
        let tool = stroke.tool;
        let s = self.state.clone();
        match tool {
            Tool::Debug => {
                // Shift selection drag.
                if let Some(h) = self.hover {
                    let fp = self.footprint(h);
                    merge_selection(&mut self.selection, &fp);
                }
            }
            Tool::Brush => {
                let st = self.stroke.as_ref().unwrap();
                let offset = (st.start_mouse_y - mouse.y) * st.world_per_pixel;
                let flatten = s.flatten.then_some(st.start_local.y);
                let pattern = st.pattern.clone();
                let base = st.base_heights.clone();
                self.apply_footprint(&pattern, |store, fp| brush::op_offset(store, fp, &base, offset, flatten));
            }
            Tool::Level => {
                if let Some(h) = self.hover {
                    let fp = self.footprint(h);
                    let new: Vec<(IVec2, f32)> = {
                        let st = self.stroke.as_ref().unwrap();
                        fp.into_iter().filter(|(c, _)| !st.pattern.iter().any(|(p, _)| p == c)).collect()
                    };
                    let mut grown: Vec<(IVec2, f32)> = self.stroke.as_ref().unwrap().pattern.clone();
                    let mut base = self.stroke.as_ref().unwrap().base_heights.clone();
                    base.extend(self.base_heights(&new));
                    grown.extend(new.iter().copied());
                    // Existing samples keep the strongest weight they were brushed with.
                    for (c, w) in self.footprint(h) {
                        if let Some(e) = grown.iter_mut().find(|(p, _)| *p == c) {
                            e.1 = e.1.max(w);
                        }
                    }
                    self.quick_paint(&new);
                    if let Some(st) = &mut self.stroke {
                        st.pattern = grown;
                        st.base_heights = base;
                    }
                    self.apply_level();
                }
            }
            Tool::VertexPaint => {
                if let Some(h) = self.hover {
                    let target = if s.paint_walls { PaintTarget::Wall } else { PaintTarget::Floor };
                    self.paint_filtered(h, target, s.paint_slot);
                }
            }
            Tool::GrassMask => self.grass_at_hover(),
            t if t.is_extra() => self.extra_motion(),
            _ => {}
        }
    }

    fn finish_bridge(&mut self) {
        let (Some(stroke), Some(end)) = (&self.stroke, self.hover) else {
            self.finish_stroke();
            return;
        };
        let start = stroke.start_local;
        let brush = self.state.brush;
        let ease = self.state.bridge_ease;
        // Snapshot every chunk the bridge rectangle can reach.
        let fp: Vec<(IVec2, f32)> = {
            let t = self.terrain.as_ref().unwrap().bind();
            let r = brush.radius;
            let min = v2(start.x.min(end.x) - r, start.z.min(end.z) - r);
            let max = v2(start.x.max(end.x) + r, start.z.max(end.z) + r);
            t.store().config.samples_in_rect(min, max).into_iter().map(|s| (s, 1.0)).collect()
        };
        let (a, b) = (pt_core::math::v3(start.x, start.y, start.z), pt_core::math::v3(end.x, end.y, end.z));
        self.apply_footprint(&fp, |st, _| brush::op_bridge(st, &brush, a, b, ease));
        if self.state.quick_paint != 0 {
            // Paint the ramp footprint (samples within the bridge width).
            let r = brush.radius.max(0.05);
            let (pa, pb) = (v2(a.x, a.z), v2(b.x, b.z));
            let strip: Vec<(IVec2, f32)> = {
                let t = self.terrain.as_ref().unwrap().bind();
                fp.iter()
                    .filter(|(s, _)| {
                        let p = t.store().config.sample_pos(*s);
                        let len2 = (pb - pa).dot(pb - pa).max(1e-6);
                        let tt = ((p - pa).dot(pb - pa) / len2).clamp(0.0, 1.0);
                        (p - pa.lerp(pb, tt)).length() <= r
                    })
                    .copied()
                    .collect()
            };
            self.quick_paint(&strip);
        }
        self.finish_stroke();
    }

    /// Ends the current stroke and records it as one undo action.
    pub(super) fn finish_stroke(&mut self) {
        let Some(stroke) = self.stroke.take() else { return };
        if let Some(before) = stroke.scatter_before {
            self.commit_scatter_undo(stroke.tool.undo_name(), before);
            return;
        }
        if let Some(before) = stroke.tint_before {
            self.commit_tint_undo(stroke.tool.undo_name(), before);
            return;
        }
        let Some(mut terrain) = self.terrain.clone() else { return };
        let mut before = VarDictionary::new();
        let mut after = VarDictionary::new();
        let mut coords = Vec::new();
        {
            let t = terrain.bind();
            for (c, b) in &stroke.before {
                let a = t.chunk_bytes(*c);
                if a != *b {
                    coords.push(*c);
                    before.set(from_iv(*c), &PackedByteArray::from(b.clone().unwrap_or_default().as_slice()));
                    after.set(from_iv(*c), &PackedByteArray::from(a.unwrap_or_default().as_slice()));
                }
            }
        }
        if coords.is_empty() {
            return;
        }
        terrain.bind_mut().write_chunks(&coords);
        let name = if self.state.quick_paint != 0 && matches!(stroke.tool, Tool::Brush | Tool::Level | Tool::Smooth | Tool::Bridge | Tool::RaiseLower) {
            "terrain brush with quick paint"
        } else if stroke.tool == Tool::VertexPaint && self.state.paint_walls {
            "terrain wall paint"
        } else {
            stroke.tool.undo_name()
        };
        self.commit_undo(name, before, after);
    }

    /// Esc during a stroke: restore the snapshot without recording an undo action.
    fn cancel_stroke(&mut self, stroke: Stroke) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let mut tb = terrain.bind_mut();
        for (c, b) in &stroke.before {
            if tb.chunk_bytes(*c) != *b {
                tb.restore_chunk(*c, b.as_deref());
            }
        }
        tb.update();
    }

    fn commit_undo(&mut self, name: &str, before: VarDictionary, after: VarDictionary) {
        let Some(terrain) = self.terrain.clone() else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.upcast::<Object>();
        ur.create_action(name);
        ur.add_do_method(&obj, "set_chunks_data", &[after.to_variant()]);
        ur.add_undo_method(&obj, "set_chunks_data", &[before.to_variant()]);
        ur.commit_action_ex().execute(false).done();
    }

    fn chunk_click(&mut self) {
        if let Some(c) = self.hover_chunk {
            self.toggle_chunk(c);
        }
    }

    /// Adds chunk `c` (it must touch an existing chunk) or removes it if it exists; one undo step.
    pub(super) fn toggle_chunk(&mut self, c: IVec2) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let exists = terrain.bind().store().has_chunk(c);
        let mut before = VarDictionary::new();
        let mut after = VarDictionary::new();
        if exists {
            before.set(from_iv(c), &terrain.bind().get_chunk_data(from_iv(c)));
            terrain.bind_mut().remove_chunk(from_iv(c));
            after.set(from_iv(c), &PackedByteArray::new());
            if self.selected_chunk == Some(c) {
                self.selected_chunk = None;
            }
        } else {
            let can_add = {
                let t = terrain.bind();
                t.store().chunks.is_empty() || t.store().config.chunk_neighbors(c).iter().any(|n| t.store().has_chunk(*n))
            };
            if !can_add {
                godot_warn!("Marching Squares Terrain: new chunks must touch an existing chunk.");
                return;
            }
            before.set(from_iv(c), &PackedByteArray::new());
            terrain.bind_mut().add_chunk(from_iv(c));
            after.set(from_iv(c), &terrain.bind().get_chunk_data(from_iv(c)));
            self.selected_chunk = Some(c);
        }
        terrain.bind_mut().update();
        self.commit_undo(if exists { "remove chunk" } else { "add chunk" }, before, after);
        self.refresh_chunk_list();
        if self.state.tool == Tool::ChunkManager {
            // Chunk count, Remove button state.
            self.rebuild_strip_deferred();
        }
        self.redraw();
    }

    pub(super) fn set_merge_mode(&mut self, all: bool) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let coords: Vec<IVec2> = if all {
            terrain.bind().store().chunk_coords()
        } else {
            match self.selected_chunk {
                Some(c) => vec![c],
                None => {
                    godot_warn!("Marching Squares Terrain: Ctrl+click a chunk (or pick one in the list) first.");
                    return;
                }
            }
        };
        let mut before = VarDictionary::new();
        let mut after = VarDictionary::new();
        for c in &coords {
            before.set(from_iv(*c), &terrain.bind().get_chunk_data(from_iv(*c)));
            terrain.bind_mut().set_merge_mode_of(*c, self.state.merge_mode);
            after.set(from_iv(*c), &terrain.bind().get_chunk_data(from_iv(*c)));
        }
        terrain.bind_mut().write_chunks(&coords);
        terrain.bind_mut().update();
        self.commit_undo("chunk merge mode", before, after);
    }

    fn debug_print(&self) {
        let (Some(t), Some(h)) = (&self.terrain, self.hover) else { return };
        let t = t.bind();
        let cfg = &t.store().config;
        let s = cfg.world_to_sample(v2(h.x, h.z));
        let global = self.terrain_transform() * h;
        match t.store().sample(s) {
            Some(sm) => godot_print!(
                "Marching Squares Terrain: global {global:?}  sample ({}, {}) chunk {:?} local {:?}: height {:.3}, floor {} '{}', wall {} '{}', grass {}, smooth {}",
                s.x,
                s.y,
                cfg.chunk_of(s),
                cfg.local_of(s),
                sm.height,
                sm.floor + 1,
                self.slot_name(sm.floor as usize),
                sm.wall + 1,
                self.slot_name(sm.wall as usize),
                sm.grass,
                sm.smooth,
            ),
            None => godot_print!("Marching Squares Terrain: no sample at ({}, {})", s.x, s.y),
        }
    }

    // ------------------------------------------------------------------ cursor

    fn ensure_cursor(&mut self) {
        if self.cursor.as_ref().is_some_and(|c| c.is_valid()) {
            return;
        }
        let Some(mut t) = self.terrain.clone() else { return };
        self.cursor = Some(Cursor::new(&mut t));
    }

    fn drop_cursor(&mut self) {
        if let Some(c) = self.cursor.take() {
            c.free();
        }
        if let Some(t) = &mut self.terrain
            && t.is_instance_valid()
        {
            t.bind_mut().update_shader_cursor(None);
        }
    }

    /// Footprint color by operation: green raise, red lower, blue level/flatten.
    fn cursor_color(&self) -> Color {
        let (lmb, rmb) = self.stroke.as_ref().map(|s| (s.lmb, s.rmb)).unwrap_or((false, false));
        match self.state.tool {
            Tool::RaiseLower if lmb && rmb => Color::from_rgb(0.35, 0.6, 1.0),
            Tool::RaiseLower if rmb => Color::from_rgb(1.0, 0.3, 0.25),
            Tool::Brush | Tool::RaiseLower => Color::from_rgb(0.35, 1.0, 0.45),
            Tool::Level => Color::from_rgb(0.35, 0.6, 1.0),
            Tool::Smooth => Color::from_rgb(1.0, 0.85, 0.3),
            Tool::Bridge => Color::from_rgb(1.0, 0.6, 0.25),
            Tool::GrassMask if rmb || self.state.mask_mode => Color::from_rgb(1.0, 0.3, 0.25),
            Tool::GrassMask => Color::from_rgb(0.6, 1.0, 0.3),
            Tool::VertexPaint => Color::from_rgb(1.0, 0.5, 0.9),
            _ => Color::from_rgb(0.9, 0.9, 0.9),
        }
    }

    pub(super) fn redraw(&mut self) {
        if !self.cursor.as_ref().is_some_and(|c| c.is_valid()) {
            self.cursor = None;
            return;
        }
        let Some(t) = self.terrain.clone() else { return };
        let tb = t.bind();
        let tool = self.state.tool;
        let b = &self.state.brush;
        let cell_aligned = b.grid_aligned && tb.store().config.grid.is_cell_mode();
        let stroke_pattern = self.stroke.as_ref().filter(|s| matches!(s.tool, Tool::Brush | Tool::Level)).map(|s| s.pattern.clone());
        let marks: Vec<(IVec2, f32)> = if let Some(p) = self.extra_marks(tb.store()) {
            p
        } else if let Some(p) = stroke_pattern {
            p
        } else if !self.selection.is_empty() {
            let mut sel = self.selection.clone();
            if let Some(h) = self.hover.filter(|_| tool.uses_brush()) {
                merge_selection(&mut sel, &brush::footprint(&tb.store().config, b, v2(h.x, h.z)));
            }
            sel
        } else if let Some(h) = self.hover.filter(|_| tool.uses_brush() && tool != Tool::TerrainSettings) {
            brush::footprint(&tb.store().config, b, v2(h.x, h.z))
        } else {
            Vec::new()
        };
        let show_brush = tool.uses_brush() && !cell_aligned && !self.stroke.as_ref().is_some_and(|s| s.tool == Tool::Brush);
        // The terrain shader draws the footprint itself (pixel-crisp, colored by operation).
        let in_shader = tb.shader_cursor;
        let shader_cursor = (show_brush && in_shader && tool != Tool::TerrainSettings)
            .then_some(self.hover)
            .flatten()
            .map(|h| (h, b.radius, b.shape as i32, self.cursor_color(), b.falloff));
        let scene = cursor::Scene {
            hover: self.hover,
            brush: (show_brush && !in_shader).then_some(b),
            marks: &marks,
            chunk_mode: tool == Tool::ChunkManager,
            hover_chunk: self.hover_chunk,
            selected_chunk: self.selected_chunk,
            show_chunk_lines: true,
            bridge_from: self.stroke.as_ref().filter(|s| s.tool == Tool::Bridge).map(|s| s.start_local),
        };
        if let Some(c) = &mut self.cursor {
            c.draw(&tb, &scene);
        }
        drop(tb);
        let mut t = t;
        t.bind_mut().update_shader_cursor(shader_cursor);
        self.extra_redraw();
    }
}

pub(super) fn merge_selection(sel: &mut Vec<(IVec2, f32)>, fp: &[(IVec2, f32)]) {
    for (s, w) in fp {
        match sel.iter_mut().find(|(c, _)| c == s) {
            Some(e) => e.1 = e.1.max(*w),
            None => sel.push((*s, *w)),
        }
    }
}

#[godot_api]
impl MarchingSquaresTerrainPlugin {
    /// A newly opened scene: frame its terrain if the editor camera starts under it.
    #[func]
    fn _mst_on_scene_changed(&mut self, root: Option<Gd<Node>>) {
        let Some(root) = root else { return };
        let mut found: Option<Gd<Node>> = root.clone().try_cast::<MarchingSquaresTerrain>().ok().map(|t| t.upcast());
        if found.is_none() {
            found = root.find_children_ex("*").type_("MarchingSquaresTerrain").owned(false).done().iter_shared().next();
        }
        if let Some(t) = found {
            self.queue_frame(t);
        }
    }

    /// Deferred: if the 3D editor camera is below the top of `terrain` (the default editor view
    /// orbits the origin at 4 m, i.e. inside raised islands such as hex_island), point the view
    /// at the terrain's top centre and pull back so the island is seen from above. The editor
    /// exposes no API for its view cursor, so this drives "Focus Selection" on a temporary
    /// marker and zooms with wheel events; it runs once per terrain and session and never when
    /// the camera already looks from above (e.g. restored editor state).
    #[func]
    fn _mst_frame_if_buried(&self, terrain: Gd<Node3D>) {
        frame_if_buried(terrain);
    }

    #[func]
    fn on_tool(&mut self, index: i64) {
        if let Some(t) = TOOLS.get(index as usize).or_else(|| EXTRA_TOOLS.get((index as usize).wrapping_sub(TOOLS.len()))) {
            self.select_tool(*t);
        }
    }

    /// Strip buttons of the extra tools (Copy, Paste, Fill cells, Clear item).
    #[func]
    fn on_action(&mut self, id: i64) {
        if let Some(s) = Setting::from_i64(id) {
            self.extra_action(s, 1);
            self.redraw();
        }
    }

    /// Terrain Settings page tab pressed.
    #[func]
    fn on_settings_page(&mut self, page: i64) {
        self.settings_page = page.max(0) as usize;
        self.rebuild_strip_deferred();
    }

    #[func]
    fn rebuild_strip_now(&mut self) {
        self.rebuild_strip();
    }

    #[func]
    fn rebuild_texture_panel_now(&mut self) {
        if self.state.tool == Tool::VertexPaint {
            self.rebuild_texture_panel();
        }
    }

    #[func]
    fn on_float(&mut self, value: f64, id: i64) {
        if let Some(s) = Setting::from_i64(id) {
            self.state.set(s, value);
            self.refresh_dependent_states();
            self.redraw();
            self.base().update_overlays();
        }
    }

    #[func]
    fn on_bool(&mut self, pressed: bool, id: i64) {
        if let Some(s) = Setting::from_i64(id) {
            self.state.set(s, if pressed { 1.0 } else { 0.0 });
            if s == Setting::Falloff && pressed {
                // Grid Aligned needs falloff off (original dependency rule).
                self.state.brush.grid_aligned = false;
                self.set_setting(Setting::GridAligned, 0.0);
            }
            self.refresh_dependent_states();
            self.redraw();
        }
    }

    #[func]
    fn on_option(&mut self, index: i64, id: i64) {
        match Setting::from_i64(id) {
            Some(Setting::TexturePreset) => self.apply_preset(index.max(0) as usize),
            Some(Setting::BrushMode) => {
                // Switches Brush <-> RaiseLower; the strip shows other attributes, so rebuild it
                // once this signal has returned (the dropdown is freed by the rebuild).
                self.finish_stroke();
                self.state.set(Setting::BrushMode, index as f64);
                self.rebuild_strip_deferred();
                self.redraw();
                self.base().update_overlays();
            }
            Some(Setting::ScatterAdd) => {
                // One-shot dropdown: creating the item rebuilds the strip (deferred).
                self.extra_action(Setting::ScatterAdd, index);
            }
            Some(s) => {
                self.state.set(s, index as f64);
                if s == Setting::Material {
                    self.refresh_texture_panel_selection();
                }
                self.redraw();
            }
            None => {}
        }
    }

    #[func]
    fn on_text(&mut self, text: GString, id: i64) {
        if Setting::from_i64(id) == Some(Setting::TextureName) {
            let i = self.state.paint_slot as usize;
            self.set_slot_prop(i, "display_name", text.to_variant());
            self.refresh_material_names();
            // Clear the field after submitting (original behaviour).
            for (s, c) in &self.setting_controls {
                if *s == Setting::TextureName
                    && let Ok(mut le) = c.clone().try_cast::<godot::classes::LineEdit>()
                {
                    le.set_text("");
                }
            }
            self.to_gd().call_deferred("rebuild_texture_panel_now", &[]);
        }
    }

    #[func]
    fn on_remove_selected_chunk(&mut self) {
        if let Some(c) = self.selected_chunk.filter(|c| self.terrain.as_ref().is_some_and(|t| t.bind().store().has_chunk(*c))) {
            self.toggle_chunk(c);
        }
    }

    #[func]
    fn on_chunk_handles(&mut self, on: bool) {
        if let Some(c) = self.selected_chunk
            && let Some(t) = self.terrain.as_mut()
        {
            t.set("handle_chunk", &from_iv(c).to_variant());
        }
        if let Some(t) = self.terrain.as_mut() {
            t.set("chunk_handles", &on.to_variant());
        }
    }

    #[func]
    fn on_chunk_selected(&mut self, index: i64) {
        let c = self.chunk_list.as_ref().map(|ob| ob.get_item_metadata(index as i32)).and_then(|v| v.try_to::<Vector2i>().ok());
        self.select_chunk(c.map(|c| pt_core::math::iv2(c.x, c.y)));
    }

    #[func]
    fn on_merge_mode(&mut self, index: i64) {
        self.state.merge_mode = (index as i32).clamp(0, 4);
        self.set_merge_mode(false);
    }

    #[func]
    fn on_apply_merge_all(&mut self) {
        self.set_merge_mode(true);
    }

    #[func]
    fn on_terrain_setting(&mut self, value: Variant, index: i64, comp: i64) {
        self.terrain_setting_changed(value, index as usize, comp);
    }

    #[func]
    fn on_slot_select(&mut self, index: i64) {
        self.set_setting(Setting::Material, index as f64);
    }

    #[func]
    fn on_slot_name(&mut self, text: GString, index: i64) {
        self.set_slot_prop(index as usize, "display_name", text.to_variant());
        self.refresh_material_names();
    }

    #[func]
    fn on_slot_texture(&mut self, res: Variant, index: i64) {
        self.set_slot_prop(index as usize, "texture", res);
    }

    #[func]
    fn on_slot_pattern(&mut self, pattern: i64, index: i64) {
        self.set_slot_pattern(index as usize, pattern as i32);
    }

    #[func]
    fn on_slot_scale(&mut self, value: f64, index: i64, mut label: Gd<godot::classes::Label>) {
        label.set_text(&format!("{value:.1}"));
        self.set_slot_prop(index as usize, "scale", (value as f32).to_variant());
    }

    #[func]
    fn on_slot_tint(&mut self, c: Color, index: i64) {
        self.set_slot_prop(index as usize, "tint", c.to_variant());
    }

    #[func]
    fn on_slot_grass_tint(&mut self, c: Color, index: i64) {
        self.set_slot_prop(index as usize, "grass_tint", c.to_variant());
    }

    #[func]
    fn on_slot_grass_sprite(&mut self, res: Variant, index: i64) {
        self.set_slot_prop(index as usize, "grass_sprite", res);
    }

    #[func]
    fn on_slot_has_grass(&mut self, on: bool, index: i64) {
        self.set_slot_prop(index as usize, "has_grass", on.to_variant());
    }

    #[func]
    fn on_export_preset(&mut self) {
        self.open_export_dialog();
    }

    #[func]
    fn on_export_confirmed(&mut self, edit: Gd<godot::classes::LineEdit>, mut dialog: Gd<godot::classes::AcceptDialog>) {
        let name = edit.get_text().to_string();
        dialog.queue_free();
        self.export_preset(name, false);
    }

    #[func]
    fn on_export_overwrite(&mut self, name: GString, mut dialog: Gd<godot::classes::ConfirmationDialog>) {
        dialog.queue_free();
        self.export_preset(name.to_string(), true);
    }
}

fn frame_if_buried(terrain: Gd<Node3D>) {
    use godot::classes::{EditorInterface, MenuButton, PopupMenu};
    if !terrain.is_inside_tree() {
        return;
    }
    let Ok(t) = terrain.clone().try_cast::<MarchingSquaresTerrain>() else { return };
    let Some(bounds) = t.bind().world_bounds() else { return };
    let ei = EditorInterface::singleton();
    let Some(vp) = ei.get_editor_viewport_3d_ex().idx(0).done() else { return };
    let Some(cam) = vp.get_camera_3d() else { return };
    let top = bounds.position.y + bounds.size.y;
    if cam.get_global_position().y > top + 1.0 {
        return;
    }
    // The viewport's "Focus Selection" entry lives in its view menu (a MenuButton's popup).
    let mut n: Option<Gd<Node>> = vp.get_parent();
    while let Some(p) = n.clone() {
        if p.is_class("Node3DEditorViewport") {
            break;
        }
        n = p.get_parent();
    }
    let Some(view) = n else { return };
    let mut focus: Option<(Gd<PopupMenu>, i32)> = None;
    for mb in view.find_children_ex("*").type_("MenuButton").owned(false).done().iter_shared() {
        let Ok(mb) = mb.try_cast::<MenuButton>() else { continue };
        let Some(pm) = mb.get_popup() else { continue };
        for i in 0..pm.get_item_count() {
            if pm.get_item_text(i).to_string() == "Focus Selection" {
                focus = Some((pm.clone(), pm.get_item_id(i)));
            }
        }
    }
    let Some((mut pm, id)) = focus else {
        godot_warn!("Marching Squares Terrain: could not frame the terrain (no Focus Selection menu entry)");
        return;
    };
    let Some(mut sel) = ei.get_selection() else { return };
    let previous: Vec<Gd<Node>> = sel.get_selected_nodes().iter_shared().collect();
    let mut marker = Node3D::new_alloc();
    marker.set_name("__mst_focus");
    let mut parent = terrain.clone();
    parent.add_child(&marker);
    let c = bounds.position + bounds.size * 0.5;
    marker.set_global_position(Vector3::new(c.x, top, c.z));
    sel.clear();
    sel.add_node(&marker);
    pm.emit_signal("id_pressed", &[id.to_variant()]);
    // Zoom out to about the island's size: wheel-down on the viewport's input surface.
    let span = bounds.size.x.max(bounds.size.z).max(8.0);
    let steps = ((span * 0.6 / 4.0).ln() / 1.08f32.ln()).clamp(0.0, 60.0) as i32;
    for ctl in view.find_children_ex("*").type_("Control").owned(false).done().iter_shared() {
        if ctl.get_class().to_string() != "Control" || ctl.get_signal_connection_list("gui_input").is_empty() {
            continue;
        }
        let mut ctl = ctl;
        for _ in 0..steps {
            let mut ev = InputEventMouseButton::new_gd();
            ev.set_button_index(MouseButton::WHEEL_DOWN);
            ev.set_pressed(true);
            ev.set_factor(1.0);
            ctl.emit_signal("gui_input", &[ev.to_variant()]);
        }
        break;
    }
    sel.clear();
    for p in previous {
        sel.add_node(&p);
    }
    marker.queue_free();
}
