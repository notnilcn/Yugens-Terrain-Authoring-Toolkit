//! Builder plugin, part actions: dissolve, offset elements, invert / by-colour / by-smoothing
//! selection, multi-axis mirror, Detach / Duplicate to a new object, Create Reusable Part and
//! the individual-origins pivot. Geometry is `pt_core::build::xg_ops`.

use std::collections::BTreeSet;

use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::resource_loader::CacheMode;
use godot::classes::{
    EditorFileDialog, EditorInterface, PackedScene, ResourceLoader, ResourceSaver,
};
use godot::prelude::*;

use pt_core::build::xg_ops::{self, ElementSel, OffsetSpace};
use pt_core::build::{BMesh, ops};
use pt_core::math::v3;

use super::super::data::{PixelBuilderMeshData, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;

impl PixelBuilderPlugin {
    // ================================================================== dissolve / offset

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
                Mode::Face => xg_ops::dissolve_faces(m, &s.sel_faces),
                Mode::Edge => xg_ops::dissolve_edges(m, &s.sel_edges),
                _ => xg_ops::melt_corners(m, &s.sel_verts),
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
            if let Err(e) = xg_ops::shift_items(m, &verts, &faces, offset, space, basis, per_face) {
                s.message = format!("Offset: {e}");
            }
        });
    }

    // ================================================================== selection

    pub(super) fn perform_negate_picked(&mut self) {
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        match self.mode {
            Mode::Face => self.sel_faces = xg_ops::invert_faces(m, &self.sel_faces),
            Mode::Edge => self.sel_edges = xg_ops::invert_edges(m, &self.sel_edges),
            Mode::Vertex => self.sel_verts = xg_ops::invert_vertices(m, &self.sel_verts),
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
        match xg_ops::grab_via_tint(m, &sel) {
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
        self.sel_faces = xg_ops::grab_via_gliding_cluster(db.cage(), &self.sel_faces);
    }

    // ================================================================== mirror

    /// Mirror across the ticked local planes. The default (X, Duplicate) is the classic
    /// single-axis mirrored copy, so its result is unchanged.
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
                    pt_core::build::loops::mirror_copy(m, axis, true, 0.0014);
                } else if let Err(e) = xg_ops::mirror_axes(m, axes, dup, 1e-3) {
                    s.message = format!("Mirror: {e}");
                }
                s.purge_picked();
            },
        );
    }

    // ================================================================== individual origins

    /// Connected pieces of the drag selection when Individual origins is on (`None` otherwise or
    /// for a single piece, which is the classic pivot anyway).
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
        let g = xg_ops::connected_groups(verts, &links);
        (g.len() > 1).then_some(g)
    }

    // ================================================================== to new object

    /// Detach / Duplicate the selected faces into a new builder mesh next to this one: one undo
    /// step adds the node (and, detaching, removes the faces).
    pub(super) fn act_extract_faces(&mut self, duplicate: bool) {
        let (Some(t), Some(data)) = (self.target.clone(), self.contents()) else {
            return;
        };
        if self.mode != Mode::Face || self.sel_faces.is_empty() {
            self.message = "Select one or more faces (Face mode)".into();
            return;
        }
        let (Some(parent), Some(root)) = (
            t.get_parent(),
            EditorInterface::singleton().get_edited_scene_root(),
        ) else {
            self.message = "The scene root cannot be split".into();
            return;
        };
        let (before, src, mats, units) = {
            let d = data.bind();
            (
                d.get_blob(),
                d.cage().clone(),
                d.stuff_pockets().duplicate_shallow(),
                d.units(),
            )
        };
        let sel = self.sel_faces.clone();
        let mut extracted = src.clone();
        let others: BTreeSet<usize> = (0..src.facets.len()).filter(|f| !sel.contains(f)).collect();
        ops::discard_facets(&mut extracted, &others);
        extracted.compact();
        let mut remaining = src.clone();
        if !duplicate {
            ops::discard_facets(&mut remaining, &sel);
            remaining.compact();
        }
        if extracted.facets.is_empty()
            || extracted.sanity_check().is_err()
            || remaining.sanity_check().is_err()
        {
            self.message = "Could not split the selected faces off".into();
            return;
        }
        let name = if duplicate {
            "Duplicated faces"
        } else {
            "Detached faces"
        };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut nd = PixelBuilderMeshData::new_gd();
        nd.bind_mut().substitute(extracted);
        nd.set("materials", &mats.to_variant());
        nd.set("units_per_repeat", &units.to_variant());
        node.bind_mut().set_data(Some(nd));
        node.set_name(name.replace(' ', "").as_str());
        node.set_transform(t.get_transform());
        // Same look: material override and collider / trigger role.
        for p in ["material_override", "entity_type"] {
            node.set(p, &t.get(p));
        }
        let after = PackedByteArray::from(pt_core::build::blob::pack(&remaining).as_slice());
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(&format!(
            "Pixel Builder: {}",
            if duplicate {
                "Duplicate faces to new object"
            } else {
                "Detach faces to new object"
            }
        ));
        if !duplicate {
            ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
            ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        }
        ur.add_do_method(
            &parent,
            "add_child",
            &[node.to_variant(), true.to_variant()],
        );
        ur.add_do_method(
            &parent,
            "move_child",
            &[node.to_variant(), (t.get_index() + 1).to_variant()],
        );
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.commit_action();
        self.purge_picked();
        self.message = format!("{name}: new builder mesh {}", node.get_name());
        self.redraw();
    }

    // ================================================================== reusable part

    /// Create Reusable Part: asks for a .tscn path.
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
            let mut d = EditorFileDialog::new_alloc();
            d.set_file_mode(FileMode::SAVE_FILE);
            d.set_access(Access::RESOURCES);
            d.set_title("Create reusable part");
            d.add_filter_ex("*.tscn").description("Scene").done();
            d.connect("file_selected", &self.to_gd().callable("on_reusable_file"));
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&d);
            }
            self.xg_dialogs.reusable = Some(d);
        }
        if let Some(d) = &mut self.xg_dialogs.reusable {
            d.set_current_file(&format!("{}.tscn", t.get_name()));
            d.popup_file_dialog();
        }
    }

    /// Saves the target's branch as the scene at `path` and swaps it for an instance, one undo
    /// step.
    pub(super) fn reusable_part_to(&mut self, path: &str) -> Result<String, String> {
        let t = self.target.clone().ok_or("no builder mesh selected")?;
        let node: Gd<Node> = t.clone().upcast();
        let root = EditorInterface::singleton()
            .get_edited_scene_root()
            .ok_or("no edited scene")?;
        let parent = node.get_parent().ok_or("the node is not in the scene")?;
        if root.instance_id() == node.instance_id() {
            return Err("the scene root cannot become a reusable part".into());
        }
        // Descendants owned by the edited scene are owned by the node while packing.
        let children: Vec<Gd<Node>> = node
            .find_children_ex("*")
            .owned(false)
            .done()
            .iter_shared()
            .collect();
        let owners: Vec<Option<Gd<Node>>> = children.iter().map(|c| c.get_owner()).collect();
        for c in &children {
            if c.get_owner()
                .is_some_and(|o| o.instance_id() == root.instance_id())
            {
                c.clone().set_owner(Some(&node));
            }
        }
        // The builder's own gizmo lock must not reach the file.
        self.set_gizmo_lock(None);
        let mut scene = PackedScene::new_gd();
        let packed = scene.pack(&node);
        for (c, o) in children.iter().zip(&owners) {
            c.clone().set_owner(o.as_ref());
        }
        if packed != godot::global::Error::OK {
            return Err(format!(
                "the node could not be packed as a scene ({packed:?})"
            ));
        }
        let saved = ResourceSaver::singleton().save_ex(&scene).path(path).done();
        if saved != godot::global::Error::OK {
            return Err(format!(
                "the scene could not be saved to {path} ({saved:?})"
            ));
        }
        let loaded = ResourceLoader::singleton()
            .load_ex(path)
            .cache_mode(CacheMode::REPLACE)
            .done()
            .and_then(|r| r.try_cast::<PackedScene>().ok());
        let mut instance = loaded
            .and_then(|s| s.instantiate())
            .ok_or("the saved scene could not be instantiated")?;
        instance.set_name(&node.get_name());
        if let Ok(mut n3) = instance.clone().try_cast::<Node3D>() {
            n3.set_transform(t.get_transform());
        }
        let idx = node.get_index();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return Err("no undo manager".into());
        };
        ur.create_action("Pixel Builder: Create reusable part");
        ur.add_do_method(&parent, "remove_child", &[node.to_variant()]);
        ur.add_do_method(
            &parent,
            "add_child",
            &[instance.to_variant(), true.to_variant()],
        );
        ur.add_do_method(
            &parent,
            "move_child",
            &[instance.to_variant(), idx.to_variant()],
        );
        ur.add_do_method(&instance, "set_owner", &[root.to_variant()]);
        ur.add_do_reference(&instance);
        ur.add_undo_method(&parent, "remove_child", &[instance.to_variant()]);
        ur.add_undo_method(
            &parent,
            "add_child",
            &[node.to_variant(), true.to_variant()],
        );
        ur.add_undo_method(
            &parent,
            "move_child",
            &[node.to_variant(), idx.to_variant()],
        );
        ur.add_undo_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_undo_reference(&node);
        // Removing the edited node re-enters this plugin (`edit`): commit from the message queue
        // (the `act_bake` pattern), then select the instance.
        ur.call_deferred("commit_action", &[]);
        EditorInterface::singleton().call_deferred("edit_node", &[instance.to_variant()]);
        if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
            fs.call_deferred("scan", &[]);
        }
        Ok(format!(
            "Saved {path} and replaced {} by an instance of it",
            node.get_name()
        ))
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

    /// Test / scripting hook: Create Reusable Part at `path` without the dialog ("" = ok).
    #[func]
    fn debug_reusable_part(&mut self, path: GString) -> GString {
        match self.reusable_part_to(&path.to_string()) {
            Ok(_) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// Test / scripting hook: rotate the selection about Y by `degrees` the way a rotate-handle
    /// drag does (individual origins honoured), one undo step.
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
            Some(g) => xg_ops::rotate_individual(&mut m, &g, axis, degrees.to_radians()),
            None => {
                let c = xg_ops::group_center(&start, &verts);
                let w = pt_core::build::xform::hard_weights(&verts);
                pt_core::build::xform::rotate_weighted(&mut m, &w, c, axis, degrees.to_radians());
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
