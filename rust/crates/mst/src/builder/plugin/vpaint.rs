//! Builder plugin, part 4 — ports of Unibuilder's (MIT, Leafshade Consulting Ltd):
//!
//! * vertex paint: `editor/paint_tool.gd` (press-drag Brush / Fill strokes as one undo step,
//!   Shift+click eyedropper, brush reticle, fill hover) and `editor/paint_panel.gd` (Brush / Fill,
//!   RGBA / RGB / Alpha channel, radius / strength / softness, alpha value) over
//!   `pt_core::build::paint` (`core/ops/paint_ops.gd`);
//! * blend materials: `plugin.gd::_on_blend_material_requested` / `_on_invert_alpha_requested` /
//!   `_paint_panel_faces` with `core/ops/set_face_props.gd::assign_material`;
//! * Bake to MeshInstance3D with lightmap UV2: `plugin.gd::_bake_edited` and
//!   `nodes/ub_mesh_instance.gd::make_baked`.

use std::collections::{BTreeMap, BTreeSet};

use godot::classes::{BaseMaterial3D, Camera3D, Control, EditorInterface, InputEvent, InputEventMouseButton, InputEventMouseMotion, Material};
use godot::global::MouseButton;
use godot::prelude::*;

use pt_core::build::paint::{self, Mask};
use pt_core::build::{BMesh, pick};
use pt_core::math::v2;

use super::super::blend::PixelBuilderBlendMaterial;
use super::super::node::PixelBuilderMesh;
use super::super::data::{from_p, to_p};
use super::super::ui::Mode;
use super::PixelBuilderPlugin;

const TEAL: Color = Color::from_rgba(0.369, 0.918, 0.831, 0.85);
const AMBER: Color = Color::from_rgb(0.984, 0.749, 0.141);

/// One vertex-paint stroke (one undo step).
pub struct VStroke {
    before: PackedByteArray,
    /// The mesh when the stroke started (dabs blend from it, so they never stack).
    base: BMesh,
    weights: BTreeMap<u32, f32>,
    /// Fill: faces already painted by this stroke.
    painted: BTreeSet<usize>,
}

impl VStroke {
    pub fn new(before: PackedByteArray, base: BMesh) -> VStroke {
        VStroke { before, base, weights: BTreeMap::new(), painted: BTreeSet::new() }
    }
    pub fn before(self) -> PackedByteArray {
        self.before
    }
}

/// Vertex paint settings (Unibuilder paint panel) and live state.
pub struct VPaint {
    pub fill: bool,
    /// 0 RGBA, 1 RGB, 2 Alpha (`paint::Mask`).
    pub channel: i64,
    pub radius: f32,
    pub strength: f32,
    pub softness: f32,
    /// Alpha painted in the Alpha channel.
    pub alpha: f32,
    pub stroke: Option<VStroke>,
    /// Face and world point under the cursor.
    pub hover: Option<(usize, Vector3)>,
}

impl Default for VPaint {
    fn default() -> Self {
        // Unibuilder `set_brush` defaults.
        VPaint { fill: false, channel: 0, radius: 0.5, strength: 1.0, softness: 0.5, alpha: 1.0, stroke: None, hover: None }
    }
}

impl PixelBuilderPlugin {
    fn vp_color(&self) -> paint::Rgba {
        if self.vp.channel == 2 {
            [0.0, 0.0, 0.0, self.vp.alpha]
        } else {
            [self.color.r, self.color.g, self.color.b, self.color.a]
        }
    }

    pub(super) fn vpaint_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        self.target.as_ref()?;
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            self.vp_update_hover(cam, mm.get_position());
            if self.vp.stroke.is_some() {
                self.vp_paint_at();
            }
            self.base().update_overlays();
            return Some(self.vp.stroke.is_some());
        }
        let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() else { return None };
        if mb.get_button_index() != MouseButton::LEFT {
            return None;
        }
        let pos = mb.get_position();
        if mb.is_pressed() {
            if mb.is_shift_pressed() {
                self.vp_eyedrop(cam, pos);
                return Some(true);
            }
            let data = self.data()?;
            let (before, base) = {
                let d = data.bind();
                (d.get_blob(), d.mesh().clone())
            };
            self.vp.stroke = Some(VStroke::new(before, base));
            self.vp_update_hover(cam, pos);
            self.vp_paint_at();
            return Some(true);
        }
        if let Some(stroke) = self.vp.stroke.take()
            && let Some(data) = self.data()
        {
            let after = data.bind().get_blob();
            if after != stroke.before {
                self.commit_blob("Pixel Builder: Paint vertex colors", &data, stroke.before, after);
            }
            return Some(true);
        }
        Some(false)
    }

    /// Reverts a live stroke (Esc / tool change). True if there was one.
    pub(super) fn cancel_vpaint_stroke(&mut self) -> bool {
        let Some(stroke) = self.vp.stroke.take() else { return false };
        if let Some(mut data) = self.data() {
            data.bind_mut().set_blob(stroke.before);
        }
        self.redraw();
        true
    }

    fn vp_update_hover(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let (o, d) = self.local_ray(cam, pos);
        let xf = self.xform();
        self.vp.hover = self.data().and_then(|data| pick::ray_mesh(data.bind().mesh(), o, d, true).map(|h| (h.face, xf * from_p(h.point))));
    }

    /// One dab (Brush) or one face (Fill) at the hover point.
    pub(super) fn vp_paint_at(&mut self) {
        let Some(mut data) = self.data() else { return };
        let Some((face, hit)) = self.vp.hover else { return };
        let (color, mask) = (self.vp_color(), Mask::from_i64(self.vp.channel));
        let xf = self.xform();
        let (fill, radius, strength, softness) = (self.vp.fill, self.vp.radius, self.vp.strength, self.vp.softness);
        let Some(stroke) = self.vp.stroke.as_mut() else { return };
        let mut m = data.bind().mesh().clone();
        if fill {
            if !stroke.painted.insert(face) {
                return;
            }
            paint::paint_faces(&mut m, &BTreeSet::from([face]), color, mask);
        } else {
            let dab = paint::brush_weights(&stroke.base, |p| to_p(xf * from_p(p)), to_p(hit), radius, strength, softness);
            if !paint::accumulate(&mut stroke.weights, &dab) {
                return;
            }
            paint::paint_weighted(&mut m, &stroke.base, &stroke.weights, color, mask);
        }
        data.bind_mut().replace(m);
        data.emit_changed();
    }

    /// Shift+click: the color of the corner nearest the cursor becomes the paint color.
    fn vp_eyedrop(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        let Some(data) = self.data() else { return };
        let (o, d) = self.local_ray(cam, pos);
        let xf = self.xform();
        let picked = {
            let db = data.bind();
            let m = db.mesh();
            pick::ray_mesh(m, o, d, true).and_then(|h| {
                let pts: Vec<_> = m.faces[h.face]
                    .verts
                    .iter()
                    .map(|v| {
                        let w = xf * from_p(m.positions[*v as usize]);
                        if cam.is_position_behind(w) { v2(f32::INFINITY, f32::INFINITY) } else { crate::util::to_v2(cam.unproject_position(w)) }
                    })
                    .collect();
                paint::nearest_corner(&pts, crate::util::to_v2(pos)).map(|ci| paint::sample(m, h.face, ci))
            })
        };
        if let Some(c) = picked {
            let f = paint::to_f(c);
            self.color = Color::from_rgba(f[0], f[1], f[2], f[3]);
            if self.vp.channel == 2 {
                self.vp.alpha = f[3];
            }
            self.message = format!("Picked color {:.2} {:.2} {:.2} / alpha {:.2}", f[0], f[1], f[2], f[3]);
            self.sync_ui();
        }
    }

    /// Brush reticle (Brush) or the face a click would fill (Fill).
    pub(super) fn draw_vpaint(&self, overlay: &mut Gd<Control>, cam: &Gd<Camera3D>) {
        if self.tool != super::Tool::VertexPaint {
            return;
        }
        let Some((face, hit)) = self.vp.hover else { return };
        if cam.is_position_behind(hit) {
            return;
        }
        if !self.vp.fill {
            let s = cam.unproject_position(hit);
            let r = cam.unproject_position(hit + cam.get_global_transform().basis.col_a() * self.vp.radius.max(0.001)).distance_to(s);
            overlay.draw_arc_ex(s, r, 0.0, std::f32::consts::TAU, 64, TEAL).width(1.5).done();
            if self.vp.softness > 0.0 {
                let inner = r * (1.0 - self.vp.softness.clamp(0.0, 1.0));
                overlay.draw_arc_ex(s, inner, 0.0, std::f32::consts::TAU, 48, Color::from_rgba(0.369, 0.918, 0.831, 0.35)).width(1.0).done();
            }
            let c = self.vp_color();
            overlay.draw_circle(s, 3.0, Color::from_rgba(c[0], c[1], c[2], 1.0));
            return;
        }
        let Some(data) = self.data() else { return };
        let xf = self.xform();
        let db = data.bind();
        let m = db.mesh();
        let Some(f) = m.faces.get(face) else { return };
        let mut pts = PackedVector2Array::new();
        for v in &f.verts {
            let w = xf * from_p(m.positions[*v as usize]);
            if cam.is_position_behind(w) {
                return;
            }
            pts.push(cam.unproject_position(w));
        }
        if pts.len() < 3 {
            return;
        }
        if !godot::classes::Geometry2D::singleton().triangulate_polygon(&pts).is_empty() {
            overlay.draw_colored_polygon(&pts, Color::from_rgba(AMBER.r, AMBER.g, AMBER.b, 0.22));
        }
        for i in 0..pts.len() {
            overlay.draw_line_ex(pts[i], pts[(i + 1) % pts.len()], Color::from_rgba(AMBER.r, AMBER.g, AMBER.b, 0.9)).width(2.5).antialiased(true).done();
        }
    }

    /// Faces the paint-panel buttons act on: the face selection, else every face
    /// (`_paint_panel_faces`).
    fn paint_panel_faces(&self, m: &BMesh) -> BTreeSet<usize> {
        if self.mode == Mode::Face && !self.sel_faces.is_empty() { self.sel_faces.clone() } else { (0..m.faces.len()).collect() }
    }

    /// New blend material on the faces (one undo step: slot list + face slots).
    pub(super) fn act_blend_material(&mut self) {
        let Some(mut data) = self.data() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let (before, mut m, old) = {
            let d = data.bind();
            (d.get_blob(), d.mesh().clone(), d.material_slots())
        };
        let faces = self.paint_panel_faces(&m);
        if faces.is_empty() {
            return;
        }
        let mut mat = PixelBuilderBlendMaterial::create();
        // Layer A starts from the faces' current material (both layers otherwise start white).
        let cur = faces.first().map(|f| m.faces[*f].material as usize);
        if let Some(src) = cur.and_then(|s| old.get(s)).flatten().and_then(|x| x.try_cast::<BaseMaterial3D>().ok()) {
            mat.bind_mut().import_layer(0, src);
        }
        let slot = paint::assign_slot(&m, old.len(), None);
        let mut new = old.duplicate_shallow();
        while new.len() < slot {
            new.push(None::<&Gd<Material>>);
        }
        let mat: Gd<Material> = mat.upcast();
        new.push(Some(&mat));
        pt_core::build::ops::set_material(&mut m, &faces, slot.min(u16::MAX as usize) as u16);
        data.bind_mut().replace(m);
        let after = data.bind().get_blob();
        data.bind_mut().set_blob(before.clone());
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action("Pixel Builder: New blend material");
        ur.add_do_property(&data, "materials", &new.to_variant());
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "materials", &old.to_variant());
        ur.commit_action();
        self.slot = slot as i64;
        self.palette_slots = None;
        self.message = format!("Blend material in slot {slot} on {} face{} (paint the mask: Vertex paint, Channel Alpha)", faces.len(), if faces.len() == 1 { "" } else { "s" });
        self.sync_ui();
        self.redraw();
    }

    /// Replaces the builder mesh by a plain MeshInstance3D with lightmap UV2 (undoable swap).
    pub(super) fn act_bake(&mut self) {
        let Some(t) = self.target.clone() else {
            self.message = "Select a builder mesh to bake".into();
            return;
        };
        let (Some(root), Some(parent)) = (EditorInterface::singleton().get_edited_scene_root(), t.get_parent()) else { return };
        if root.instance_id() == t.instance_id() {
            self.message = "Cannot bake the scene root".into();
            return;
        }
        self.set_gizmo_lock(None);
        let texel = self.prefs.lightmap_texel;
        let baked = PixelBuilderMesh::make_baked(t.clone(), texel);
        let idx = t.get_index();
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action("Pixel Builder: Bake to MeshInstance3D");
        // Remove first, so the baked node gets the same name.
        ur.add_do_method(&parent, "remove_child", &[t.to_variant()]);
        ur.add_do_method(&parent, "add_child", &[baked.to_variant(), true.to_variant()]);
        ur.add_do_method(&parent, "move_child", &[baked.to_variant(), idx.to_variant()]);
        ur.add_do_method(&baked, "set_owner", &[root.to_variant()]);
        for c in baked.find_children_ex("*").owned(false).done().iter_shared() {
            ur.add_do_method(&c, "set_owner", &[root.to_variant()]);
        }
        ur.add_do_reference(&baked);
        ur.add_undo_method(&parent, "remove_child", &[baked.to_variant()]);
        ur.add_undo_method(&parent, "add_child", &[t.to_variant(), true.to_variant()]);
        ur.add_undo_method(&parent, "move_child", &[t.to_variant(), idx.to_variant()]);
        ur.add_undo_method(&t, "set_owner", &[root.to_variant()]);
        ur.add_undo_reference(&t);
        // Committed (and the baked node selected) from the message queue: removing the edited
        // node re-enters this plugin (`edit`, `make_visible`), which must not be bound then.
        ur.call_deferred("commit_action", &[]);
        let uv2 = baked.get_mesh().is_some_and(|m| m.has_meta("pt_uv2_texel"));
        self.message = format!("Baked {} to MeshInstance3D{}", baked.get_name(), if uv2 { format!(" (lightmap UV2 at texel {texel})") } else { String::new() });
        EditorInterface::singleton().call_deferred("edit_node", &[baked.to_variant()]);
    }
}
