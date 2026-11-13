//! Rust tools appended after the original nine MST tools: Scatter, Stamp, Gradient and Select
//! (copy / paste). Input, strip actions and viewport previews live here; `plugin.rs` only
//! forwards to these hooks. The previews are one internal `ImmediateMesh` line gizmo child of
//! the terrain (stamp result wireframe, gradient end points, paste height ticks).

use godot::classes::base_material_3d::{Flags, ShadingMode, Transparency};
use godot::classes::mesh::PrimitiveType;
use godot::classes::{Camera3D, DirAccess, ImageTexture, ImmediateMesh, Input, InputEventKey, InputEventMouseButton, Material, MeshInstance3D, Object, ResourceLoader, StandardMaterial3D, Texture2D};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use pt_core::brush::{self, PaintTarget};
use pt_core::clipboard::{self, Patch};
use pt_core::data::TerrainStore;
use pt_core::math::{IVec2, Vec2 as PVec2, v2, v3};
use pt_core::stamp::{BUILTIN_STAMPS, Stamp, StampMode, StampParams};

use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::{SCATTER_ADD, Setting, Tool};
use crate::terrain::MarchingSquaresTerrain;
use crate::terrain::scatter::PixelScatterItem;
use crate::terrain::stamp::{STAMP_DIR, stamp_from_image, stamp_to_image};
use crate::util::add_internal;

/// Lines drawn over the terrain (always on top).
pub struct LineGizmo {
    node: Gd<MeshInstance3D>,
    mesh: Gd<ImmediateMesh>,
    mat: Gd<StandardMaterial3D>,
}

impl LineGizmo {
    fn new(terrain: &mut Gd<MarchingSquaresTerrain>) -> LineGizmo {
        let mut node = MeshInstance3D::new_alloc();
        node.set_name("MarchingSquaresTerrainToolGizmo");
        let mesh = ImmediateMesh::new_gd();
        node.set_mesh(&mesh);
        node.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
        node.set_layer_mask(1);
        add_internal(terrain, &node);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_flag(Flags::DISABLE_DEPTH_TEST, true);
        mat.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_render_priority(14);
        LineGizmo { node, mesh, mat }
    }

    fn valid(&self) -> bool {
        self.node.is_instance_valid()
    }

    fn draw(&mut self, lines: &[(Vector3, Vector3, Color)]) {
        self.mesh.clear_surfaces();
        if lines.is_empty() {
            return;
        }
        self.mesh.surface_begin_ex(PrimitiveType::LINES).material(&self.mat.clone().upcast::<Material>()).done();
        for (a, b, c) in lines {
            self.mesh.surface_set_color(*c);
            self.mesh.surface_add_vertex(*a);
            self.mesh.surface_set_color(*c);
            self.mesh.surface_add_vertex(*b);
        }
        self.mesh.surface_end();
    }
}

/// Session state of the extra tools.
#[derive(Default)]
pub struct Extra {
    gizmo: Option<LineGizmo>,
    pub grad_a: Option<Vector3>,
    pub grad_b: Option<Vector3>,
    pub clipboard: Option<Patch>,
    pub pasting: bool,
    /// Stamp library: built-ins, then images from [`STAMP_DIR`] (loaded on first use).
    stamps: Vec<(String, Stamp)>,
    /// Stamp drag handle: centre pinned at the press point while LMB is held; dragging sets
    /// size (twice the drag length) and rotation (drag direction); release stamps.
    pub stamp_anchor: Option<Vector3>,
}

const LIFT: f32 = 0.06;

impl MarchingSquaresTerrainPlugin {
    // ------------------------------------------------------------------ gizmo lifetime

    pub(super) fn extra_drop(&mut self) {
        if let Some(g) = self.extra.gizmo.take()
            && g.valid()
        {
            let mut n = g.node;
            n.queue_free();
        }
        self.extra.pasting = false;
        self.extra.stamp_anchor = None;
    }

    pub(super) fn extra_set_visible(&mut self, v: bool) {
        if let Some(g) = &mut self.extra.gizmo
            && g.valid()
        {
            g.node.set_visible(v);
        }
    }

    // ------------------------------------------------------------------ stamp library

    pub(super) fn stamp_library(&mut self) -> &[(String, Stamp)] {
        if self.extra.stamps.is_empty() {
            let mut v: Vec<(String, Stamp)> = BUILTIN_STAMPS.iter().map(|b| (b.name().to_string(), Stamp::builtin(*b, 128))).collect();
            if let Some(dir) = DirAccess::open(STAMP_DIR) {
                let mut files: Vec<String> = dir.get_files().as_slice().iter().map(|f| f.to_string().trim_end_matches(".import").to_string()).collect();
                files.sort();
                files.dedup();
                for f in files {
                    let lower = f.to_lowercase();
                    if !(lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".webp") || lower.ends_with(".exr")) {
                        continue;
                    }
                    let stem = f.rsplit_once('.').map(|x| x.0).unwrap_or(&f).to_string();
                    // The shipped PNGs of the built-ins are for users; the generated ones are exact.
                    if BUILTIN_STAMPS.iter().any(|b| b.name().eq_ignore_ascii_case(&stem)) {
                        continue;
                    }
                    let path = format!("{STAMP_DIR}{f}");
                    let Some(tex) = ResourceLoader::singleton().load(&path).and_then(|r| r.try_cast::<Texture2D>().ok()) else { continue };
                    let Some(img) = tex.get_image() else { continue };
                    if let Some(s) = stamp_from_image(&img, 256) {
                        v.push((stem.replace('_', " "), s));
                    }
                }
            }
            self.extra.stamps = v;
        }
        &self.extra.stamps
    }

    /// Names of the loaded stamp library (call [`Self::stamp_library`] first).
    pub(super) fn stamp_names(&self) -> Vec<String> {
        self.extra.stamps.iter().map(|s| s.0.clone()).collect()
    }

    pub(super) fn stamp_icon(&self, i: usize) -> Option<Gd<ImageTexture>> {
        let s = self.extra.stamps.get(i)?.1.clone();
        let mut img = stamp_to_image(&s)?;
        img.resize(24, 24);
        ImageTexture::create_from_image(&img)
    }

    fn stamp_params(&self, at: Vector3) -> StampParams {
        let s = &self.state;
        StampParams {
            center: v2(at.x, at.z),
            rotation: s.stamp_rotation,
            size: v2(s.stamp_size, s.stamp_size * s.stamp_aspect),
            height: s.stamp_height,
            contrast: s.stamp_contrast,
            mode: StampMode::from_i64(s.stamp_mode),
            base: at.y,
        }
    }

    fn rotation_step(&self) -> f32 {
        let cell = self.terrain.as_ref().is_some_and(|t| t.bind().store().config.grid.is_cell_mode());
        if cell { 60.0 } else { 15.0 }
    }

    fn rotate_stamp(&mut self, dir: f32) {
        let step = if self.state.stamp_snap { self.rotation_step() } else { 5.0 };
        let mut r = self.state.stamp_rotation + dir * step;
        if self.state.stamp_snap {
            r = (r / step).round() * step;
        }
        self.set_setting(Setting::StampRotation, r as f64);
    }

    // ------------------------------------------------------------------ input

    /// Mouse buttons for the extra tools (press and release). Returns "consumed".
    pub(super) fn extra_mouse_button(&mut self, camera: &Gd<Camera3D>, mb: &Gd<InputEventMouseButton>, shift: bool, ctrl: bool) -> bool {
        let button = mb.get_button_index();
        let (is_l, is_r) = (button == MouseButton::LEFT, button == MouseButton::RIGHT);
        let tool = self.state.tool;
        if !mb.is_pressed() {
            if is_l && let Some(a) = self.extra.stamp_anchor.take() {
                self.commit_stamp(a);
                self.finish_stroke();
                return true;
            }
            if let Some(s) = &mut self.stroke {
                if is_l {
                    s.lmb = false;
                }
                if is_r {
                    s.rmb = false;
                }
                if !s.lmb && !s.rmb {
                    self.finish_stroke();
                }
                return true;
            }
            return false;
        }
        if !(is_l || is_r) || self.stroke.is_some() {
            return self.stroke.is_some();
        }
        let Some(h) = self.hover else { return false };
        match tool {
            Tool::Scatter => {
                let before = self.terrain.as_ref().map(|t| t.bind().scatter_bytes());
                let mut st = self.new_stroke(camera, mb, Tool::Scatter, h, is_l, is_r);
                st.scatter_before = before;
                self.stroke = Some(st);
                self.scatter_at_hover();
                true
            }
            Tool::Tint => {
                let before = self.terrain.as_ref().map(|t| t.bind().get_painted_tints_data().to_vec());
                let mut st = self.new_stroke(camera, mb, Tool::Tint, h, is_l, is_r);
                st.tint_before = before;
                self.stroke = Some(st);
                self.tint_at_hover();
                true
            }
            Tool::Stamp if is_l => {
                // Press pins the stamp; a drag sizes / turns it (`extra_motion`), release stamps.
                self.stroke = Some(self.new_stroke(camera, mb, Tool::Stamp, h, true, false));
                self.extra.stamp_anchor = Some(h);
                true
            }
            Tool::Gradient if ctrl => {
                if is_l {
                    self.extra.grad_a = Some(h);
                } else {
                    self.extra.grad_b = Some(h);
                }
                true
            }
            Tool::Gradient if is_l => {
                if self.extra.grad_a.is_none() || self.extra.grad_b.is_none() {
                    godot_warn!("Marching Squares Terrain: Gradient needs a start (Ctrl+LMB) and an end point (Ctrl+RMB).");
                    return true;
                }
                self.stroke = Some(self.new_stroke(camera, mb, Tool::Gradient, h, true, false));
                self.gradient_at_hover();
                true
            }
            Tool::Select => {
                if self.extra.pasting {
                    if is_l {
                        self.stroke = Some(self.new_stroke(camera, mb, Tool::Select, h, true, false));
                        self.paste_at(h);
                        self.finish_stroke();
                    } else {
                        self.extra.pasting = false;
                    }
                    return true;
                }
                if is_r {
                    let had = !self.selection.is_empty();
                    self.selection.clear();
                    return had;
                }
                if !shift {
                    self.selection.clear();
                }
                // tool = Debug: the stroke only grows the selection (see `stroke_motion`).
                self.stroke = Some(self.new_stroke(camera, mb, Tool::Debug, h, true, false));
                let fp = self.footprint(h);
                super::plugin::merge_selection(&mut self.selection, &fp);
                true
            }
            _ => false,
        }
    }

    /// Mouse motion during an extra-tool stroke.
    pub(super) fn extra_motion(&mut self) {
        match self.stroke.as_ref().map(|s| s.tool) {
            Some(Tool::Scatter) => self.scatter_at_hover(),
            Some(Tool::Tint) => self.tint_at_hover(),
            Some(Tool::Gradient) => self.gradient_at_hover(),
            Some(Tool::Stamp) => self.stamp_drag(),
            _ => {}
        }
    }

    /// Stamp drag handle: size = twice the drag length, rotation = drag direction (snapped to
    /// the grid's rotation step with Snap Rotation). Short drags keep the current size / angle.
    fn stamp_drag(&mut self) {
        let (Some(a), Some(h)) = (self.extra.stamp_anchor, self.hover) else { return };
        let d = Vector2::new(h.x - a.x, h.z - a.z);
        if d.length() < 0.75 {
            return;
        }
        self.set_setting(Setting::StampSize, (2.0 * d.length()).clamp(2.0, 400.0) as f64);
        let mut r = (-d.y).atan2(d.x).to_degrees();
        if self.state.stamp_snap {
            let step = self.rotation_step();
            r = (r / step).round() * step;
        }
        self.set_setting(Setting::StampRotation, r as f64);
    }

    /// Keys of the extra tools. Returns "consumed". Plain keys only: Ctrl+C / Ctrl+V and
    /// Q/W/E/R are editor shortcuts (node copy/paste, transform modes) handled before the
    /// viewport sees them, so the tools use C / V (copy / paste), Z / X (rotate) and M (mirror).
    pub(super) fn extra_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        let k = key.get_keycode();
        if key.is_ctrl_pressed() || key.is_meta_pressed() || key.is_alt_pressed() {
            return false;
        }
        let rot = match k {
            Key::Z => Some(-1.0),
            Key::X => Some(1.0),
            _ => None,
        };
        match self.state.tool {
            Tool::Select => {
                if k == Key::C {
                    self.copy_selection();
                    return true;
                }
                if k == Key::V {
                    self.start_paste();
                    return true;
                }
                if self.extra.pasting {
                    let step = self.terrain.as_ref().map(|t| clipboard::step_degrees(t.bind().store().config.grid)).unwrap_or(60.0);
                    match k {
                        Key::Z | Key::X => {
                            let r = self.state.paste_rotation + step * rot.unwrap_or(1.0);
                            self.set_setting(Setting::PasteRotation, r as f64);
                        }
                        Key::M => {
                            let m = !self.state.paste_mirror;
                            self.set_setting(Setting::PasteMirror, m as i32 as f64);
                        }
                        Key::ESCAPE => self.extra.pasting = false,
                        _ => return false,
                    }
                    self.redraw();
                    return true;
                }
                false
            }
            Tool::Stamp if rot.is_some() => {
                self.rotate_stamp(rot.unwrap_or(1.0));
                self.redraw();
                true
            }
            _ => false,
        }
    }

    /// Mouse wheel: Stamp size / height / rotation, Alt+wheel brush rotation.
    pub(super) fn extra_wheel(&mut self, up: bool, shift: bool, ctrl: bool) -> bool {
        let alt = Input::singleton().is_key_pressed(Key::ALT);
        let dir = if up { 1.0 } else { -1.0 };
        if self.state.tool == Tool::Stamp {
            if shift {
                let s = self.state.stamp_size as f64 * if up { 1.1 } else { 1.0 / 1.1 };
                self.set_setting(Setting::StampSize, s);
            } else if ctrl {
                let h = self.state.stamp_height as f64;
                self.set_setting(Setting::StampHeight, h + dir as f64 * (h.abs() * 0.1).max(0.25));
            } else if alt {
                self.rotate_stamp(dir);
            } else {
                return false;
            }
            self.redraw();
            return true;
        }
        if alt && self.state.tool.uses_brush() {
            let r = self.state.brush.rotation as f64 + dir as f64 * 15.0;
            self.set_setting(Setting::Rotation, r);
            self.redraw();
            return true;
        }
        false
    }

    // ------------------------------------------------------------------ operations

    fn scatter_at_hover(&mut self) {
        let (Some(h), Some(mut t)) = (self.hover, self.terrain.clone()) else { return };
        let rmb = self.stroke.as_ref().is_some_and(|s| s.rmb && !s.lmb);
        let s = &self.state;
        let item = if rmb && s.erase_all { None } else { Some(s.scatter_item) };
        if !rmb && t.bind().scatter_item_labels().is_empty() {
            godot_warn!("Marching Squares Terrain: add a scatter item first (Add Item dropdown or the terrain's Scatter Items).");
            return;
        }
        let (brush, per_cell, slope) = (s.brush, s.scatter_per_cell, (s.slope_min, s.slope_max));
        let mut tb = t.bind_mut();
        if tb.scatter_brush(item, &brush, v2(h.x, h.z), rmb, per_cell, slope) {
            tb.update();
        }
    }

    /// Tint tool: paints (LMB) or erases (RMB) the saved tint of the cells under the brush.
    fn tint_at_hover(&mut self) {
        let (Some(h), Some(mut t)) = (self.hover, self.terrain.clone()) else { return };
        let erase = self.stroke.as_ref().is_some_and(|s| s.rmb && !s.lmb);
        let rgba = if erase { [0; 4] } else { self.state.tint_rgba() };
        let cells: Vec<IVec2> = self.footprint(h).into_iter().map(|(c, _)| c).collect();
        t.bind_mut().paint_tints(&cells, rgba);
    }

    pub(super) fn commit_tint_undo(&mut self, name: &str, before: Vec<u8>) {
        let Some(terrain) = self.terrain.clone() else { return };
        let after = terrain.bind().get_painted_tints_data().to_vec();
        if after == before {
            return;
        }
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.upcast::<Object>();
        ur.create_action(name);
        ur.add_do_method(&obj, "set_painted_tints_data", &[PackedByteArray::from(after.as_slice()).to_variant()]);
        ur.add_undo_method(&obj, "set_painted_tints_data", &[PackedByteArray::from(before.as_slice()).to_variant()]);
        ur.commit_action_ex().execute(false).done();
    }

    fn gradient_at_hover(&mut self) {
        let (Some(h), Some(a), Some(b)) = (self.hover, self.extra.grad_a, self.extra.grad_b) else { return };
        let k = self.state.gradient_strength;
        let (a, b) = (v3(a.x, a.y, a.z), v3(b.x, b.y, b.z));
        let fp = self.footprint(h);
        self.apply_footprint(&fp, |st, fp| brush::op_gradient(st, fp, a, b, k));
    }

    fn commit_stamp(&mut self, at: Vector3) {
        let p = self.stamp_params(at);
        let i = self.state.stamp;
        let lib = self.stamp_library();
        let Some(stamp) = lib.get(i).or(lib.first()).map(|s| s.1.clone()) else { return };
        let fp: Vec<(IVec2, f32)> = {
            let Some(t) = &self.terrain else { return };
            let (min, max) = p.bounds();
            t.bind().store().config.samples_in_rect(min, max).into_iter().map(|s| (s, 1.0)).collect()
        };
        if !self.state.stamp_limits {
            self.apply_footprint(&fp, |st, _| pt_core::stamp::op_stamp(st, &stamp, &p));
            return;
        }
        // Limits (`apply_stamp_limited`): Level for Flatten / Macro, Floor / Ceiling for the Minor
        // Deity modes; samples the original modes changed are clamped to the same range.
        let lim = pt_core::stamp::StampLimits { level: self.state.level, floor: self.state.floor.min(self.state.ceiling), ceiling: self.state.ceiling.max(self.state.floor) };
        self.apply_footprint(&fp, |st, fp| {
            let before: Vec<(IVec2, Option<f32>)> = fp.iter().map(|(s, _)| (*s, st.height(*s))).collect();
            let mut touched = pt_core::stamp::op_stamp_limited(st, &stamp, &p, &lim);
            for (s, h0) in before {
                if let (Some(h0), Some(h)) = (h0, st.height(s))
                    && (h - h0).abs() > 1e-6
                {
                    let c = h.clamp(lim.floor, lim.ceiling);
                    if (c - h).abs() > 1e-6 && st.set_height(s, c) {
                        for ch in st.chunks_affected_by(s) {
                            touched.insert(ch);
                        }
                    }
                }
            }
            touched
        });
    }

    fn copy_selection(&mut self) {
        let Some(t) = &self.terrain else { return };
        let tb = t.bind();
        let cells: Vec<IVec2> = self.selection.iter().map(|s| s.0).collect();
        if cells.is_empty() {
            godot_warn!("Marching Squares Terrain: select cells first (LMB-drag with the Select tool).");
            return;
        }
        let anchor = self.hover.map(|h| tb.store().config.world_to_sample(v2(h.x, h.z))).filter(|a| cells.contains(a)).unwrap_or(cells[0]);
        let patch = Patch::copy(tb.store(), &cells, anchor);
        godot_print!("Marching Squares Terrain: copied {} cells", patch.cells.len());
        drop(tb);
        self.extra.clipboard = Some(patch);
    }

    fn start_paste(&mut self) {
        if self.extra.clipboard.as_ref().is_none_or(|c| c.is_empty()) {
            godot_warn!("Marching Squares Terrain: nothing copied (select cells, then C / Copy).");
            return;
        }
        self.extra.pasting = true;
        self.redraw();
    }

    fn paste_rot_steps(&self, store: &TerrainStore) -> i32 {
        let step = clipboard::step_degrees(store.config.grid);
        (self.state.paste_rotation / step).round() as i32
    }

    fn paste_at(&mut self, at: Vector3) {
        let Some(patch) = self.extra.clipboard.clone() else { return };
        let (target, rot, base) = {
            let Some(t) = &self.terrain else { return };
            let tb = t.bind();
            let target = tb.store().config.world_to_sample(v2(at.x, at.z));
            let base = if self.state.relative_height { Some(tb.store().height(target).unwrap_or(at.y)) } else { None };
            (target, self.paste_rot_steps(tb.store()), base)
        };
        let mirror = self.state.paste_mirror;
        let fp: Vec<(IVec2, f32)> = patch.targets(target, rot, mirror).into_iter().map(|(s, _)| (s, 1.0)).collect();
        self.apply_footprint(&fp, |st, _| patch.paste(st, target, rot, mirror, base));
    }

    /// Strip buttons / one-shot dropdowns of the extra tools.
    pub(super) fn extra_action(&mut self, s: Setting, index: i64) {
        match s {
            Setting::Copy => self.copy_selection(),
            Setting::Paste => self.start_paste(),
            Setting::ScatterAdd if index > 0 => self.add_scatter_item(SCATTER_ADD[index as usize % SCATTER_ADD.len()]),
            Setting::ScatterFill => self.scatter_undoable("terrain scatter fill cells", |t, item| t.scatter_fill_cells(item)),
            Setting::ScatterClear => self.scatter_undoable("terrain scatter clear item", |t, item| t.scatter_clear(item)),
            Setting::ScatterRemove => self.remove_scatter_item(),
            _ => {}
        }
    }

    fn scatter_undoable(&mut self, name: &str, f: impl FnOnce(&mut MarchingSquaresTerrain, i64)) {
        let Some(mut t) = self.terrain.clone() else { return };
        let item = self.state.scatter_item as i64;
        if t.bind().scatter_item_labels().get(item as usize).is_none() {
            godot_warn!("Marching Squares Terrain: no scatter item selected.");
            return;
        }
        let before = t.bind().scatter_bytes();
        {
            let mut tb = t.bind_mut();
            f(&mut tb, item);
            tb.update();
        }
        self.commit_scatter_undo(name, before);
    }

    /// Records a scatter edit (blob before → now) as one undo action.
    pub(super) fn commit_scatter_undo(&mut self, name: &str, before: Vec<u8>) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let after = terrain.bind().scatter_bytes();
        if after == before {
            return;
        }
        terrain.bind_mut().scatter_write();
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.upcast::<Object>();
        ur.create_action(name);
        ur.add_do_method(&obj, "set_scatter_data", &[PackedByteArray::from(after.as_slice()).to_variant()]);
        ur.add_undo_method(&obj, "set_scatter_data", &[PackedByteArray::from(before.as_slice()).to_variant()]);
        ur.commit_action_ex().execute(false).done();
    }

    /// "Remove Item": drops the selected item and its instances (one undo step restoring both).
    fn remove_scatter_item(&mut self) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let index = self.state.scatter_item as i64;
        if terrain.bind().scatter_item_labels().get(index as usize).is_none() {
            godot_warn!("Marching Squares Terrain: no scatter item selected.");
            return;
        }
        let items_before = terrain.get("scatter_items");
        let before = terrain.bind().scatter_bytes();
        terrain.bind_mut().remove_scatter_item(index);
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.clone().upcast::<Object>();
            ur.create_action("terrain remove scatter item");
            ur.add_do_method(&obj, "remove_scatter_item", &[index.to_variant()]);
            ur.add_undo_property(&obj, "scatter_items", &items_before);
            ur.add_undo_method(&obj, "set_scatter_data", &[PackedByteArray::from(before.as_slice()).to_variant()]);
            ur.commit_action_ex().execute(false).done();
        }
        self.state.scatter_item = index.saturating_sub(1).max(0) as _;
        self.rebuild_strip_deferred();
    }

    /// "Add Item" dropdown: appends a built-in `PixelScatterItem` to the terrain's list (undoable).
    fn add_scatter_item(&mut self, kind: &str) {
        let Some(terrain) = self.terrain.clone() else { return };
        let old = terrain.get("scatter_items");
        let mut arr: Array<Option<Gd<PixelScatterItem>>> = old.try_to().unwrap_or_default();
        arr = arr.duplicate_shallow();
        let item = PixelScatterItem::create_builtin(kind.into());
        arr.push(Some(&item));
        let index = arr.len() - 1;
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.upcast::<Object>();
            ur.create_action(&format!("terrain add scatter item {kind}"));
            ur.add_do_property(&obj, "scatter_items", &arr.to_variant());
            ur.add_undo_property(&obj, "scatter_items", &old);
            ur.commit_action();
        }
        self.state.scatter_item = index;
        self.rebuild_strip_deferred();
    }

    // ------------------------------------------------------------------ drawing

    /// Marks shown instead of the brush footprint (paste preview).
    pub(super) fn extra_marks(&self, store: &TerrainStore) -> Option<Vec<(IVec2, f32)>> {
        if self.state.tool != Tool::Select || !self.extra.pasting {
            return None;
        }
        let (patch, h) = (self.extra.clipboard.as_ref()?, self.hover?);
        let target = store.config.world_to_sample(v2(h.x, h.z));
        Some(patch.targets(target, self.paste_rot_steps(store), self.state.paste_mirror).into_iter().map(|(s, _)| (s, 1.0)).collect())
    }

    /// Redraws the line gizmo (stamp preview, gradient points, paste heights).
    pub(super) fn extra_redraw(&mut self) {
        let Some(mut t) = self.terrain.clone() else { return };
        if !self.extra.gizmo.as_ref().is_some_and(|g| g.valid()) {
            self.extra.gizmo = Some(LineGizmo::new(&mut t));
            self.extra_set_visible(self.visible);
        }
        let mut lines: Vec<(Vector3, Vector3, Color)> = Vec::new();
        match self.state.tool {
            Tool::Stamp => {
                if let (Some(a), Some(h)) = (self.extra.stamp_anchor, self.hover) {
                    // Drag handle: centre to cursor.
                    let c = Color::from_rgb(1.0, 0.85, 0.3);
                    lines.push((a + Vector3::UP * LIFT, h + Vector3::UP * LIFT, c));
                    lines.push((a, a + Vector3::UP * 2.0, c));
                }
                if let Some(h) = self.extra.stamp_anchor.or(self.hover) {
                    let p = self.stamp_params(h);
                    let i = self.state.stamp;
                    let stamp = self.stamp_library().get(i).map(|s| s.1.clone());
                    if let Some(stamp) = stamp {
                        let tb = t.bind();
                        stamp_preview(&tb, &stamp, &p, &mut lines);
                    }
                }
            }
            Tool::Gradient => {
                let (a, b) = (self.extra.grad_a, self.extra.grad_b);
                let mark = |lines: &mut Vec<(Vector3, Vector3, Color)>, p: Vector3, c: Color| {
                    lines.push((p, p + Vector3::UP * 2.0, c));
                    for d in [Vector3::new(0.5, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.5)] {
                        lines.push((p - d, p + d, c));
                    }
                };
                if let Some(a) = a {
                    mark(&mut lines, a, Color::from_rgb(0.3, 1.0, 0.4));
                }
                if let Some(b) = b {
                    mark(&mut lines, b, Color::from_rgb(1.0, 0.45, 0.3));
                }
                if let (Some(a), Some(b)) = (a, b) {
                    lines.push((a + Vector3::UP * LIFT, b + Vector3::UP * LIFT, Color::from_rgb(1.0, 1.0, 1.0)));
                }
            }
            Tool::Select if self.extra.pasting => {
                let tb = t.bind();
                if let (Some(patch), Some(h)) = (&self.extra.clipboard, self.hover) {
                    let store = tb.store();
                    let target = store.config.world_to_sample(v2(h.x, h.z));
                    let base = if self.state.relative_height { store.height(target).unwrap_or(h.y) } else { patch.anchor_height };
                    for (s, c) in patch.targets(target, self.paste_rot_steps(store), self.state.paste_mirror) {
                        let p = store.config.sample_pos(s);
                        let now = store.height(s).unwrap_or(base);
                        let new = base + c.height;
                        let col = if new >= now { Color::from_rgba(0.4, 1.0, 0.5, 0.9) } else { Color::from_rgba(1.0, 0.5, 0.4, 0.9) };
                        lines.push((Vector3::new(p.x, now + LIFT, p.y), Vector3::new(p.x, new + LIFT, p.y), col));
                    }
                }
            }
            _ => {}
        }
        if let Some(g) = &mut self.extra.gizmo {
            g.draw(&lines);
        }
    }

    /// Extra text for the viewport status line.
    pub(super) fn extra_status(&self) -> Option<String> {
        match self.state.tool {
            Tool::Stamp => Some(format!(
                "  |  stamp {:.0}° size {:.1} height {:.1}",
                self.state.stamp_rotation, self.state.stamp_size, self.state.stamp_height
            )),
            Tool::Select if self.extra.pasting => Some(format!("  |  paste {:.0}°{}", self.state.paste_rotation, if self.state.paste_mirror { " mirrored" } else { "" })),
            Tool::Scatter => self.terrain.as_ref().map(|t| format!("  |  {} props", t.bind().get_scatter_count(-1))),
            _ => None,
        }
    }

    /// Slope range filter of the paint / grass tools (Terrain3D "slope range").
    pub(super) fn paint_filtered(&mut self, at: Vector3, target: PaintTarget, value: u8) {
        let (lo, hi) = (self.state.slope_min, self.state.slope_max);
        let fp = self.footprint(at);
        self.apply_footprint(&fp, |st, fp| {
            let fp = brush::filter_slope(st, fp, lo, hi);
            brush::op_paint(st, &fp, target, value)
        });
    }
}

/// Wireframe of the stamped surface (orange) over the stamp square outline (white).
fn stamp_preview(t: &MarchingSquaresTerrain, stamp: &Stamp, p: &StampParams, lines: &mut Vec<(Vector3, Vector3, Color)>) {
    const N: usize = 24;
    let surf = |uv: PVec2| -> Option<(Vector3, Vector3)> {
        let q = p.point(uv);
        let h = t.local_height_at(q)?;
        let nh = stamp.apply_height(p, q, h);
        Some((Vector3::new(q.x, h + LIFT, q.y), Vector3::new(q.x, nh + LIFT, q.y)))
    };
    let grid: Vec<Vec<Option<(Vector3, Vector3)>>> = (0..=N).map(|j| (0..=N).map(|i| surf(v2(i as f32 / N as f32, j as f32 / N as f32))).collect()).collect();
    let wire = Color::from_rgba(1.0, 0.62, 0.2, 0.85);
    let edge = Color::from_rgba(1.0, 1.0, 1.0, 0.9);
    for j in 0..=N {
        for i in 0..=N {
            let Some((g0, s0)) = grid[j][i] else { continue };
            for (di, dj) in [(1usize, 0usize), (0, 1)] {
                let (ii, jj) = (i + di, j + dj);
                if ii > N || jj > N {
                    continue;
                }
                let Some((g1, s1)) = grid[jj][ii] else { continue };
                lines.push((s0, s1, wire));
                let border = (dj == 0 && (j == 0 || j == N)) || (di == 0 && (i == 0 || i == N));
                if border {
                    lines.push((g0, g1, edge));
                }
            }
        }
    }
    // Centre cross + "up" arrow showing the stamp's +Z side (rotation).
    if let Some((_, c)) = surf(v2(0.5, 0.5))
        && let Some((_, f)) = surf(v2(0.5, 0.92))
    {
        lines.push((c, f, edge));
        lines.push((c, c + Vector3::UP * 1.5, edge));
    }
}
