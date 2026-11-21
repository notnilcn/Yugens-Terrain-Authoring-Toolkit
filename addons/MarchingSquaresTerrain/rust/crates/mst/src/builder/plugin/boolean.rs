//! Combines two builder meshes into one by union, subtraction or intersection.
//!
//! The engine's constructive solid geometry does the cutting, and it only works on nodes,
//! so the two meshes are handed to it as a short-lived pair of solids hung under the mesh
//! being edited. The pair exists only until the engine has produced the combined surface
//! for that one operation and is then freed; it never becomes part of the scene the
//! person is building.
use super::super::data::{PixelBuilderMeshData, to_p};
use super::super::node::PixelBuilderMesh;
use super::PixelBuilderPlugin;
use crate::node_children::add_internal;
use godot::classes::csg_shape_3d::Operation;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::object::ConnectFlags;
use godot::classes::{ArrayMesh, CsgMesh3D, EditorInterface, Material, Mesh, StandardMaterial3D};
use godot::prelude::*;
use pt_core::build::compile::{CompileOptions, bake};
use pt_core::build::{BMesh, convert};

pub struct PendingBool {
    base: Gd<CsgMesh3D>,
    target: Gd<PixelBuilderMesh>,
    src: Gd<PixelBuilderMesh>,
    moniker: String,
    tries: u8,
    tags: Vec<(Gd<StandardMaterial3D>, u16, [u8; 4])>,
    materials: Array<Option<Gd<Material>>>,
}
impl PixelBuilderPlugin {
    pub(super) fn act_boolean(&mut self, op: Operation, name: &str) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(sel) = EditorInterface::singleton().get_selection() else {
            return;
        };
        let others: Vec<Gd<PixelBuilderMesh>> = sel
            .get_selected_nodes()
            .iter_shared()
            .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
            .filter(|n| n.instance_id() != target.instance_id() && n.bind().contents().is_some())
            .collect();
        if others.len() != 1 {
            self.message = format!(
                "{name}: select exactly one other builder mesh too (Ctrl+click it in the Scene dock; found {})",
                others.len()
            );
            return;
        }
        let src = others[0].clone();
        let (Some(a), Some(b)) = (target.bind().contents(), src.bind().contents()) else {
            return;
        };
        if !a.bind().cage().is_closed() {
            self.message = format!(
                "{name}: {} is not watertight (Select holes shows the open edges)",
                target.get_name()
            );
            return;
        }
        if !b.bind().cage().is_closed() {
            self.message = format!("{name}: {} is not watertight", src.get_name());
            return;
        }
        let (materials, b_map) = merged_slots(&a, &b);
        let mut tags = Vec::new();
        let mesh_a = tagged_mesh(a.bind().cage(), &|s| s, &mut tags);
        let mesh_b = tagged_mesh(
            b.bind().cage(),
            &|s| b_map.get(s as usize).copied().unwrap_or(s),
            &mut tags,
        );
        let (Some(mesh_a), Some(mesh_b)) = (mesh_a, mesh_b) else {
            return;
        };
        let mut root = CsgMesh3D::new_alloc();
        root.set_mesh(&mesh_a);
        let mut child = CsgMesh3D::new_alloc();
        child.set_mesh(&mesh_b);
        child.set_operation(op);
        child.set_transform(
            target.get_global_transform().affine_inverse() * src.get_global_transform(),
        );
        root.add_child(&child);
        let mut t = target.clone();
        add_internal(&mut t, &root);
        self.pending_bool = Some(PendingBool {
            base: root,
            target,
            src,
            moniker: name.to_string(),
            tries: 0,
            tags,
            materials,
        });
        self.wait_frame();
        self.message = format!("{name}: running…");
    }
    fn wait_frame(&mut self) {
        let cb = self.to_gd().callable("on_boolean_frame");
        if self.base().is_inside_tree() {
            let mut tree = self.base().get_tree();
            tree.connect_flags("process_frame", &cb, ConnectFlags::ONE_SHOT);
        }
    }
    pub(super) fn finish_boolean(&mut self) {
        let Some(mut p) = self.pending_bool.take() else {
            return;
        };
        if !p.base.is_instance_valid()
            || !p.target.is_instance_valid()
            || !p.src.is_instance_valid()
        {
            return;
        }
        let baked = p.base.bake_static_mesh();
        let baked = match baked.filter(|m| m.get_surface_count() > 0) {
            Some(m) => m,
            None if p.tries < 3 => {
                p.tries += 1;
                self.pending_bool = Some(p);
                self.wait_frame();
                return;
            }
            None => {
                p.base.queue_free();
                self.message = format!(
                    "{} produced no geometry: the meshes probably do not overlap. Nothing changed.",
                    p.moniker
                );
                self.base().update_overlays();
                return;
            }
        };
        p.base.queue_free();
        let mesh: Gd<Mesh> = baked.upcast();
        let mut positions = Vec::new();
        let mut tris = Vec::new();
        let mut surf = Vec::new();
        let mut surf_tag: Vec<Option<usize>> = Vec::new();
        for s in 0..mesh.get_surface_count() {
            let arrays = mesh.surface_get_arrays(s);
            let Ok(verts) = arrays
                .at(ArrayType::VERTEX.ord() as usize)
                .try_to::<PackedVector3Array>()
            else {
                continue;
            };
            let base = positions.len() as u32;
            positions.extend(verts.as_slice().iter().map(|q| to_p(*q)));
            let idx: Vec<u32> = match arrays
                .at(ArrayType::INDEX.ord() as usize)
                .try_to::<PackedInt32Array>()
            {
                Ok(i) if !i.is_empty() => i.as_slice().iter().map(|x| *x as u32).collect(),
                _ => (0..verts.len() as u32).collect(),
            };
            let mat = mesh.surface_get_material(s);
            let tag = mat.and_then(|m| {
                p.tags
                    .iter()
                    .position(|(t, _, _)| t.instance_id() == m.instance_id())
            });
            surf_tag.push(tag);
            let si = surf_tag.len() as u16 - 1;
            for t in idx.chunks_exact(3) {
                tris.push([base + t[0], base + t[1], base + t[2]]);
                surf.push(si);
            }
        }
        let mut m = convert::from_triangles(&positions, &tris, &surf, 1e-4);
        for f in &mut m.facets {
            let (slot, col) = surf_tag
                .get(f.stuff as usize)
                .copied()
                .flatten()
                .map(|t| (p.tags[t].1, p.tags[t].2))
                .unwrap_or((0, [255; 4]));
            f.stuff = slot;
            f.tint = col;
        }
        if m.facets.is_empty() || m.sanity_check().is_err() {
            self.message = format!(
                "{} failed: the CSG result is not a valid polygon mesh",
                p.moniker
            );
            self.base().update_overlays();
            return;
        }
        let Some(mut data) = p.target.bind().contents() else {
            return;
        };
        let (before, old_mats) = (data.bind().get_blob(), data.bind().stuff_pockets());
        data.bind_mut().substitute(m);
        data.set("materials", &p.materials.to_variant());
        data.emit_changed();
        let after = data.bind().get_blob();
        let Some(parent) = p.src.get_parent() else {
            return;
        };
        let owner = p.src.get_owner();
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action(&format!("Pixel Builder: {}", p.moniker));
        ur.add_do_method(&data, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&data, "materials", &p.materials.to_variant());
        ur.add_do_method(&parent, "remove_child", &[p.src.to_variant()]);
        ur.add_undo_method(&data, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&data, "materials", &old_mats.to_variant());
        ur.add_undo_method(
            &parent,
            "add_child",
            &[p.src.to_variant(), true.to_variant()],
        );
        if let Some(o) = owner {
            ur.add_undo_method(&p.src, "set_owner", &[o.to_variant()]);
        }
        ur.add_undo_reference(&p.src);
        ur.commit_action();
        self.purge_picked();
        self.palette_slots = None;
        self.message = format!("{} done", p.moniker);
        self.sync_ui();
        self.redraw();
    }
    pub(super) fn boolean_with(&mut self, other: Gd<PixelBuilderMesh>, op: i64) {
        let Some(mut sel) = EditorInterface::singleton().get_selection() else {
            return;
        };
        if let Some(t) = &self.target {
            sel.add_node(t);
        }
        sel.add_node(&other);
        let (o, n) = match op {
            1 => (Operation::SUBTRACTION, "Boolean subtract"),
            2 => (Operation::INTERSECTION, "Boolean intersect"),
            _ => (Operation::UNION, "Boolean union"),
        };
        self.act_boolean(o, n);
    }
}
fn tagged_mesh(
    m: &BMesh,
    slot_map: &dyn Fn(u16) -> u16,
    tags: &mut Vec<(Gd<StandardMaterial3D>, u16, [u8; 4])>,
) -> Option<Gd<ArrayMesh>> {
    let mut tagged = m.clone();
    let first = tags.len();
    let mut keys: Vec<(u16, [u8; 4])> = Vec::new();
    for f in &mut tagged.facets {
        let key = (slot_map(f.stuff), f.tint);
        let i = keys.iter().position(|k| *k == key).unwrap_or_else(|| {
            keys.push(key);
            keys.len() - 1
        });
        f.stuff = (first + i) as u16;
    }
    for (slot, col) in keys {
        tags.push((StandardMaterial3D::new_gd(), slot, col));
    }
    let c = bake(&tagged, &CompileOptions::default());
    let mut mesh = ArrayMesh::new_gd();
    for s in &c.surfaces {
        if s.ordinals.is_empty() {
            continue;
        }
        let mut arrays = VarArray::new();
        arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
        let pos: PackedVector3Array = s
            .positions
            .iter()
            .map(|p| Vector3::new(p[0], p[1], p[2]))
            .collect();
        let nrm: PackedVector3Array = s
            .normals
            .iter()
            .map(|p| Vector3::new(p[0], p[1], p[2]))
            .collect();
        let uv: PackedVector2Array = s.uvs.iter().map(|p| Vector2::new(p[0], p[1])).collect();
        let idx: PackedInt32Array = s.ordinals.iter().copied().collect();
        arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
        arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
        arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
        arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
        mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
        let si = mesh.get_surface_count() - 1;
        if let Some((mat, _, _)) = tags.get(s.stuff as usize) {
            mesh.surface_set_material(si, mat);
        }
    }
    (mesh.get_surface_count() > 0).then_some(mesh)
}
pub(super) fn merged_slots(
    a: &Gd<PixelBuilderMeshData>,
    b: &Gd<PixelBuilderMeshData>,
) -> (Array<Option<Gd<Material>>>, Vec<u16>) {
    let mut materials = a.bind().stuff_pockets().duplicate_shallow();
    let b_mats = b.bind().stuff_pockets();
    let n_b_slots = b
        .bind()
        .cage()
        .facets
        .iter()
        .map(|f| f.stuff as usize + 1)
        .max()
        .unwrap_or(1)
        .max(b_mats.len());
    let mut b_map = Vec::with_capacity(n_b_slots);
    for j in 0..n_b_slots {
        let mat = b_mats.get(j).flatten();
        let found = match &mat {
            Some(m) => materials.iter_shared().position(|x| {
                x.as_ref()
                    .is_some_and(|x| x.instance_id() == m.instance_id())
            }),
            None => Some(j),
        };
        let slot = match found {
            Some(i) => i,
            None => {
                while materials.len()
                    < a.bind()
                        .cage()
                        .facets
                        .iter()
                        .map(|f| f.stuff as usize + 1)
                        .max()
                        .unwrap_or(0)
                {
                    materials.push(None::<&Gd<Material>>);
                }
                materials.push(mat.as_ref());
                materials.len() - 1
            }
        };
        b_map.push(slot as u16);
    }
    (materials, b_map)
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_boolean_frame(&mut self) {
        self.finish_boolean();
    }
}
