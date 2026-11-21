//! The builder actions whose result is new nodes in the scene being edited: new builder
//! meshes the person stamps, converts from other geometry or splits off, and the scene
//! instance that replaces a mesh saved as a reusable part.
//!
//! They are created in code because generating them is the job of the editor tool, and
//! the scene author keeps them. Each is added through the editor's undo history and owned
//! by the edited scene, so it is saved with that scene and disappears again on undo.
use super::super::data::{PixelBuilderMeshData, from_p, to_p};
use super::super::node::PixelBuilderMesh;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use super::placement::Placement;
use super::xg_io;
use godot::classes::resource_loader::CacheMode;
use godot::classes::{EditorInterface, PackedScene, ResourceLoader, ResourceSaver};
use godot::prelude::*;
use pt_core::build::convert::ImportOptions;
use pt_core::build::{BMesh, ops};
use std::collections::BTreeSet;
impl PixelBuilderPlugin {
    pub(super) fn create_with_recipe(
        &mut self,
        p: Placement,
        m: BMesh,
        append: bool,
        recipe: xg_io::Recipe,
    ) {
        if m.facets.is_empty() {
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
            node.free();
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
    }
    pub(super) fn convert_with(&mut self, opts: ImportOptions) {
        let Some(src) = self.convert_src.clone() else {
            self.message = "Convert: select a MeshInstance3D or CSG shape".into();
            return;
        };
        let (m, mats, note) = match self.convert_source(&src, &opts) {
            Ok(r) => r,
            Err(e) => {
                self.message = format!("Convert: {e}");
                self.base().update_overlays();
                return;
            }
        };
        if m.facets.is_empty() {
            self.message = "Convert: no triangles found".into();
            return;
        }
        if let Err(e) = m.sanity_check() {
            self.message = format!("Convert failed: {e}");
            return;
        }
        let (Some(parent), Some(root)) = (
            src.get_parent(),
            EditorInterface::singleton().get_edited_scene_root(),
        ) else {
            return;
        };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut data = PixelBuilderMeshData::new_gd();
        data.bind_mut().substitute(m);
        data.set("materials", &mats.to_variant());
        node.bind_mut().set_data(Some(data));
        node.set_name(&format!("{}Builder", src.get_name()));
        node.set_transform(src.get_transform());
        let Some(mut ur) = self.base().get_undo_redo() else {
            node.free();
            return;
        };
        ur.create_action("Pixel Builder: Convert to builder mesh");
        ur.add_do_method(
            &parent,
            "add_child",
            &[node.to_variant(), true.to_variant()],
        );
        ur.add_do_method(
            &parent,
            "move_child",
            &[node.to_variant(), (src.get_index() + 1).to_variant()],
        );
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_property(&src, "visible", &false.to_variant());
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.add_undo_property(&src, "visible", &src.is_visible().to_variant());
        ur.commit_action();
        self.message = format!("Converted {} (original hidden; {note})", src.get_name());
        EditorInterface::singleton().call_deferred("edit_node", &[node.to_variant()]);
    }
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
        for p in ["material_override", "entity_type"] {
            node.set(p, &t.get(p));
        }
        let after = PackedByteArray::from(pt_core::build::blob::pack(&remaining).as_slice());
        let Some(mut ur) = self.base().get_undo_redo() else {
            node.free();
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
            instance.free();
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
