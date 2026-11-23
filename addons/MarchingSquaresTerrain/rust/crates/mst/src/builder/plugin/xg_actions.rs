use super::super::data::to_p;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use godot::classes::EditorInterface;
use godot::classes::file_dialog::{Access, FileMode};
use godot::prelude::*;
use pt_modeler::BMesh;
use pt_modeler::xg_tools::{self, ElementSel, OffsetSpace};
use pt_core::math::v3;
use std::collections::BTreeSet;
impl PixelBuilderPlugin {
    pub(super) fn act_dissolve(&mut self) {
        let mode = self.mode;
        let empty = match mode {
            Mode::Face => self.sel_faces.is_empty(),
            Mode::Edge => self.sel_edges.is_empty(),
            Mode::Vertex => self.sel_verts.is_empty(),
            Mode::Object => true,
        };
        if empty {
            self.message = "Dissolve: select faces, edges or vertices first".into();
            return;
        }
        let name = match mode {
            Mode::Face => "Pixel Builder: Dissolve faces",
            Mode::Edge => "Pixel Builder: Dissolve edges",
            _ => "Pixel Builder: Dissolve vertices",
        };
        self.apply_edit(name, |m, s| {
            let r = match mode {
                Mode::Face => xg_tools::dissolve_faces(m, &s.sel_faces),
                Mode::Edge => xg_tools::dissolve_edges(m, &s.sel_edges),
                _ => xg_tools::melt_corners(m, &s.sel_verts),
            };
            match r {
                Ok(faces) => {
                    s.purge_picked();
                    if mode == Mode::Face {
                        s.sel_faces = faces;
                    }
                }
                Err(e) => s.message = format!("Dissolve: {e}"),
            }
        });
    }
    pub(super) fn act_offset(&mut self) {
        let Some(t) = self.target.clone() else { return };
        let b = t.get_global_transform().basis;
        let basis = [to_p(b.col_a()), to_p(b.col_b()), to_p(b.col_c())];
        let o = self.xg;
        let offset = v3(o.offset[0], o.offset[1], o.offset[2]);
        let space = OffsetSpace::from_i64(o.offset_space);
        let per_face = o.offset_per_face && self.mode == Mode::Face;
        self.apply_edit("Pixel Builder: Offset elements", |m, s| {
            let verts = s.selected_verts(m);
            let faces = if s.mode == Mode::Face {
                s.sel_faces.clone()
            } else {
                BTreeSet::new()
            };
            if let Err(e) = xg_tools::shift_items(m, &verts, &faces, offset, space, basis, per_face) {
                s.message = format!("Offset: {e}");
            }
        });
    }
    pub(super) fn perform_negate_picked(&mut self) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        match self.mode {
            Mode::Face => self.sel_faces = xg_tools::invert_faces(m, &self.sel_faces),
            Mode::Edge => self.sel_edges = xg_tools::invert_edges(m, &self.sel_edges),
            Mode::Vertex => self.sel_verts = xg_tools::invert_vertices(m, &self.sel_verts),
            Mode::Object => {}
        }
    }
    pub(super) fn perform_grab_via_tint(&mut self) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        let sel = match self.mode {
            Mode::Face => ElementSel::Faces(self.sel_faces.clone()),
            Mode::Edge => ElementSel::Edges(self.sel_edges.clone()),
            Mode::Vertex => ElementSel::Vertices(self.sel_verts.clone()),
            Mode::Object => return,
        };
        match xg_tools::grab_via_tint(m, &sel) {
            ElementSel::Faces(f) => self.sel_faces = f,
            ElementSel::Edges(e) => self.sel_edges = e,
            ElementSel::Vertices(v) => self.sel_verts = v,
        }
    }
    pub(super) fn act_select_by_smoothing(&mut self) {
        let Some(d) = self.contents() else { return };
        if self.mode != Mode::Face || self.sel_faces.is_empty() {
            self.message = "Select by smoothing group: select faces (Face mode)".into();
            return;
        }
        let db = d.bind();
        self.sel_faces = xg_tools::grab_via_gliding_cluster(db.cage(), &self.sel_faces);
    }
    pub(super) fn act_mirror_axes(&mut self) {
        let (axes, dup) = (self.xg.mirror, self.xg.mirror_dup);
        let names: Vec<&str> = ["X", "Y", "Z"]
            .into_iter()
            .zip(axes)
            .filter(|(_, on)| *on)
            .map(|(n, _)| n)
            .collect();
        if names.is_empty() {
            self.message = "Mirror: tick at least one axis (X / Y / Z) in the strip".into();
            return;
        }
        let single = axes.iter().filter(|a| **a).count() == 1;
        self.apply_edit(
            &format!("Pixel Builder: Mirror {}", names.join("")),
            |m, s| {
                if single && dup {
                    let axis = axes.iter().position(|a| *a).unwrap_or(0);
                    pt_modeler::loops::mirror_copy(m, axis, true, 0.0014);
                } else if let Err(e) = xg_tools::mirror_axes(m, axes, dup, 1e-3) {
                    s.message = format!("Mirror: {e}");
                }
                s.purge_picked();
            },
        );
    }
    pub(super) fn individual_groups(
        &self,
        m: &BMesh,
        verts: &BTreeSet<u32>,
    ) -> Option<Vec<BTreeSet<u32>>> {
        if !self.xg.individual_origins || self.soft {
            return None;
        }
        let links = match self.mode {
            Mode::Face => m.edges_of_faces(&self.sel_faces),
            Mode::Edge => self.sel_edges.clone(),
            _ => m.edges_within(verts),
        };
        let g = xg_tools::connected_groups(verts, &links);
        (g.len() > 1).then_some(g)
    }
    pub(super) fn act_reusable_part(&mut self) {
        let Some(t) = self.target.clone() else { return };
        let root = EditorInterface::singleton().get_edited_scene_root();
        if root
            .as_ref()
            .is_none_or(|r| r.instance_id() == t.instance_id())
            || t.get_parent().is_none()
        {
            self.message =
                "Create reusable part: select a builder mesh below the scene root".into();
            return;
        }
        if self
            .xg_dialogs
            .reusable
            .as_ref()
            .is_none_or(|d| !d.is_instance_valid())
        {
            let mut d = super::editor_dialogs::file_dialog(
                None,
                Some(FileMode::SAVE_FILE),
                Access::RESOURCES,
                &self.to_gd().callable("on_reusable_file"),
            );
            d.set_title("Create reusable part");
            d.add_filter_ex("*.tscn").description("Scene").done();
            self.xg_dialogs.reusable = Some(d);
        }
        if let Some(d) = &mut self.xg_dialogs.reusable {
            d.set_current_file(&format!("{}.tscn", t.get_name()));
            d.popup_file_dialog();
        }
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_reusable_file(&mut self, path: GString) {
        self.message = match self.reusable_part_to(&path.to_string()) {
            Ok(m) => m,
            Err(e) => format!("Create reusable part: {e}"),
        };
        self.base().update_overlays();
    }
    #[func]
    fn debug_reusable_part(&mut self, path: GString) -> GString {
        match self.reusable_part_to(&path.to_string()) {
            Ok(_) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }
    #[func]
    fn debug_rotate_selection(&mut self, degrees: f32) {
        let Some(mut data) = self.contents() else {
            return;
        };
        let (before, start) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        let verts = self.selected_verts(&start);
        if verts.is_empty() {
            return;
        }
        let mut m = start.clone();
        let axis = to_p((self.xform().basis.inverse() * Vector3::UP).normalized());
        match self.individual_groups(&start, &verts) {
            Some(g) => xg_tools::rotate_individual(&mut m, &g, axis, degrees.to_radians()),
            None => {
                let c = xg_tools::group_center(&start, &verts);
                let w = pt_modeler::xform::hard_weights(&verts);
                pt_modeler::xform::rotate_weighted(&mut m, &w, c, axis, degrees.to_radians());
            }
        }
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            self.commit_blob("Pixel Builder: Rotate", &data, before, after);
        }
        self.redraw();
    }
}
