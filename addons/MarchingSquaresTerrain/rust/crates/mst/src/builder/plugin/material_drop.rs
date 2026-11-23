//! Lets someone drag a material from the editor and drop it onto faces of the builder mesh
//! they are editing, right in the 3D viewport.
//!
//! Intent: assigning a material to part of a mesh should be as direct as dropping it where
//! it should go. The person drags a material, the faces under the cursor take it, and the
//! rest of the mesh stays as it was.
//!
//! Responsibility: it only notices that a material is being dragged over a viewport,
//! answers whether the spot under the cursor can take it, and hands the drop to the
//! builder. Which faces receive the material, and recording that as an undo step, are the
//! builder's decisions. It stays invisible to the mouse except while a material is being
//! dragged, so the viewport keeps its normal input the rest of the time.
//!
//! Why a node: the engine only offers drag and drop to controls that sit in the interface
//! tree over the area being dropped on.
//!
//! Why a new type: a built-in control cannot decide whether to accept a drop or receive
//! one without a script, and this project has no scripts.
//!
//! Not exported: the builder it reports to and the viewport it covers are handed to it
//! when it is created. It is never placed in a scene, so there is nothing to author.
//!
//! One of these is created in code for each editor viewport the builder draws into,
//! because how many viewports are showing is the person's own layout choice, made while
//! the editor runs. They are removed when the builder leaves the editor.
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use godot::classes::control::MouseFilter;
use godot::classes::notify::ControlNotification;
use godot::classes::{
    Control, EditorInterface, GeometryInstance3D, IControl, Material, MeshInstance3D, Node,
    ResourceLoader, SubViewport, SubViewportContainer,
};
use godot::prelude::*;
use pt_modeler::{selection_edits, pick};
use std::collections::BTreeSet;
const DROP_NAME: &str = "_PixelBuilderMaterialDrop";
#[derive(Default)]
pub struct MaterialDrag {
    stuff: Option<Gd<Material>>,
    consumed: bool,
    overrides: Vec<(
        Gd<GeometryInstance3D>,
        Option<Gd<Material>>,
        Vec<Option<Gd<Material>>>,
    )>,
    pub addressees: Vec<(InstanceId, Gd<Control>)>,
}
impl PixelBuilderPlugin {
    pub(super) fn ensure_drop_target(&mut self, surface: &Gd<Control>) {
        let sid = surface.instance_id();
        if self
            .drop
            .addressees
            .iter()
            .any(|(s, t)| *s == sid && t.is_instance_valid())
        {
            return;
        }
        let mut target = PixelBuilderMaterialDrop::new_alloc();
        target.set_name(DROP_NAME);
        {
            let mut t = target.bind_mut();
            t.plugin = Some(self.to_gd());
            t.viewport = surface
                .get_parent()
                .into_iter()
                .flat_map(|p| p.get_children().iter_shared().collect::<Vec<_>>())
                .filter_map(|c| c.try_cast::<SubViewportContainer>().ok())
                .find_map(|c| {
                    (c.get_child_count() > 0)
                        .then(|| c.get_child(0))
                        .flatten()
                        .and_then(|v| v.try_cast::<SubViewport>().ok())
                });
        }
        let mut s = surface.clone();
        s.call_deferred("add_child", &[target.to_variant()]);
        self.drop.addressees.push((sid, target.upcast()));
    }
    pub(super) fn remove_drop_targets(&mut self) {
        for (_, t) in self.drop.addressees.drain(..) {
            if t.is_instance_valid() {
                t.free();
            }
        }
    }
    fn drop_face(&self, viewport: &Gd<SubViewport>, pos: Vector2) -> Option<usize> {
        let cam = viewport.get_camera_3d()?;
        let d = self.contents()?;
        let (o, dir) = self.local_ray(&cam, pos);
        pick::intersect_ray_closest(d.bind().cage(), o, dir, true).map(|h| h.face)
    }
    pub(super) fn can_drop_material(
        &self,
        viewport: &Gd<SubViewport>,
        pos: Vector2,
        data: &Variant,
    ) -> bool {
        self.target.is_some()
            && extract_material(data).is_some()
            && self.drop_face(viewport, pos).is_some()
    }
    pub(super) fn drop_material(
        &mut self,
        viewport: &Gd<SubViewport>,
        pos: Vector2,
        data: &Variant,
    ) {
        let Some(mat) = extract_material(data) else {
            return;
        };
        let Some(face) = self.drop_face(viewport, pos) else {
            return;
        };
        self.unshare_data();
        let Some(mut d) = self.contents() else { return };
        let faces: BTreeSet<usize> = if self.mode == Mode::Face {
            if self.sel_faces.contains(&face) {
                self.sel_faces.clone()
            } else {
                BTreeSet::from([face])
            }
        } else {
            (0..d.bind().cage().facets.len()).collect()
        };
        let name = if self.mode == Mode::Face {
            "Pixel Builder: Drop material on face"
        } else {
            "Pixel Builder: Drop material on object"
        };
        let (before, old_mats) = (d.bind().get_blob(), d.bind().stuff_pockets());
        let mut mats = old_mats.duplicate_shallow();
        let mut m = d.bind().cage().clone();
        let slot = match mats.iter_shared().position(|x| {
            x.as_ref()
                .is_some_and(|x| x.instance_id() == mat.instance_id())
        }) {
            Some(i) => i,
            None => {
                let used = m
                    .facets
                    .iter()
                    .map(|f| f.stuff as usize + 1)
                    .max()
                    .unwrap_or(0);
                while mats.len() < used {
                    mats.push(None::<&Gd<Material>>);
                }
                mats.push(Some(&mat));
                mats.len() - 1
            }
        };
        selection_edits::assign_stuff(&mut m, &faces, slot as u16);
        d.bind_mut().substitute(m);
        d.set("materials", &mats.to_variant());
        d.emit_changed();
        let after = d.bind().get_blob();
        let Some(mut ur) = self.open_action(name) else {
            return;
        };
        ur.add_do_method(&d, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&d, "materials", &mats.to_variant());
        ur.add_undo_method(&d, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&d, "materials", &old_mats.to_variant());
        ur.commit_action_ex().execute(false).done();
        self.drop.consumed = true;
        self.pocket = slot as i64;
        self.palette_slots = None;
        self.message = format!(
            "Material dropped on {} face{} (slot {slot})",
            faces.len(),
            if faces.len() == 1 { "" } else { "s" }
        );
        self.sync_ui();
        self.redraw();
    }
    pub(super) fn material_drag_began(&mut self, mat: Gd<Material>) {
        if self.drop.stuff.is_some() {
            return;
        }
        self.drop.stuff = Some(mat);
        self.drop.consumed = false;
        self.drop.overrides.clear();
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            return;
        };
        let mut stack: Vec<Gd<Node>> = vec![root];
        while let Some(n) = stack.pop() {
            stack.extend(n.get_children().iter_shared());
            let Ok(gi) = n.try_cast::<GeometryInstance3D>() else {
                continue;
            };
            let surfaces = gi
                .clone()
                .try_cast::<MeshInstance3D>()
                .map(|mi| {
                    (0..mi.get_surface_override_material_count())
                        .map(|i| mi.get_surface_override_material(i))
                        .collect()
                })
                .unwrap_or_default();
            let ov = gi.get_material_override();
            self.drop.overrides.push((gi, ov, surfaces));
        }
    }
    pub(super) fn material_drag_ended(&mut self) {
        let Some(mat) = self.drop.stuff.take() else {
            return;
        };
        if self.drop.consumed {
            let id = mat.instance_id();
            let is_drag =
                |m: &Option<Gd<Material>>| m.as_ref().is_some_and(|m| m.instance_id() == id);
            for (gi, ov, surfaces) in self.drop.overrides.drain(..) {
                if !gi.is_instance_valid() {
                    continue;
                }
                let mut gi = gi;
                if is_drag(&gi.get_material_override()) && !is_drag(&ov) {
                    gi.set_material_override(ov.as_ref());
                }
                if let Ok(mut mi) = gi.try_cast::<MeshInstance3D>() {
                    for (i, s) in surfaces
                        .iter()
                        .enumerate()
                        .take(mi.get_surface_override_material_count() as usize)
                    {
                        if is_drag(&mi.get_surface_override_material(i as i32)) && !is_drag(s) {
                            mi.set_surface_override_material(i as i32, s.as_ref());
                        }
                    }
                }
            }
        }
        self.drop.overrides.clear();
    }
}
pub fn extract_material(data: &Variant) -> Option<Gd<Material>> {
    let d = data.try_to::<VarDictionary>().ok()?;
    match d.get("type").map(|t| t.to_string()).as_deref() {
        Some("resource") => d.get("resource")?.try_to::<Gd<Material>>().ok(),
        Some("files") => {
            let files = d.get("files")?.try_to::<PackedStringArray>().ok()?;
            if files.len() != 1 {
                return None;
            }
            let path = files.get(0)?;
            let mut rl = ResourceLoader::singleton();
            if !rl.exists_ex(&path).type_hint("Material").done() {
                return None;
            }
            rl.load(&path).and_then(|r| r.try_cast::<Material>().ok())
        }
        _ => None,
    }
}
#[derive(GodotClass)]
#[class(tool, init, base = Control, internal)]
pub struct PixelBuilderMaterialDrop {
    plugin: Option<Gd<PixelBuilderPlugin>>,
    viewport: Option<Gd<SubViewport>>,
    base: Base<Control>,
}
#[godot_api]
impl IControl for PixelBuilderMaterialDrop {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().set_process(true);
    }
    fn process(&mut self, _delta: f64) {
        if let Some(vp) = &self.viewport
            && vp.is_instance_valid()
        {
            let want = Vector2::new(vp.get_size().x as f32, vp.get_size().y as f32);
            if self.base().get_size() != want {
                self.base_mut().set_size(want);
            }
            if self.base().get_position() != Vector2::ZERO {
                self.base_mut().set_position(Vector2::ZERO);
            }
        }
        let Some(vp) = self.base().get_viewport() else {
            return;
        };
        let dragging = vp.gui_is_dragging() && extract_material(&vp.gui_get_drag_data()).is_some();
        let want = if dragging {
            MouseFilter::PASS
        } else {
            MouseFilter::IGNORE
        };
        if self.base().get_mouse_filter() != want {
            self.base_mut().set_mouse_filter(want);
        }
    }
    fn on_notification(&mut self, what: ControlNotification) {
        let Some(mut plugin) = self.plugin.clone().filter(|p| p.is_instance_valid()) else {
            return;
        };
        match what {
            ControlNotification::DRAG_BEGIN => {
                let Some(vp) = self.base().get_viewport() else {
                    return;
                };
                if let Some(m) = extract_material(&vp.gui_get_drag_data()) {
                    plugin.bind_mut().material_drag_began(m);
                }
            }
            ControlNotification::DRAG_END => plugin.bind_mut().material_drag_ended(),
            _ => {}
        }
    }
    fn can_drop_data(&self, at_position: Vector2, data: Variant) -> bool {
        let (Some(plugin), Some(vp)) = (
            self.plugin.as_ref().filter(|p| p.is_instance_valid()),
            self.viewport.as_ref(),
        ) else {
            return false;
        };
        plugin.bind().can_drop_material(vp, at_position, &data)
    }
    fn drop_data(&mut self, at_position: Vector2, data: Variant) {
        let (Some(mut plugin), Some(vp)) = (
            self.plugin.clone().filter(|p| p.is_instance_valid()),
            self.viewport.clone(),
        ) else {
            return;
        };
        plugin.bind_mut().drop_material(&vp, at_position, &data);
    }
}
