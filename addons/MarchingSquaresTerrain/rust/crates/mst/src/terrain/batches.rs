//! Draws and collides a terrain's chunks through nodes grouped by region, so a large terrain
//! costs a handful of draw calls and bodies instead of one per chunk.
//!
//! The nodes are created in code because which regions exist, and so how many drawn and
//! colliding nodes there are, follows from the chunks the author has painted and changes
//! while they paint. They are internal children of the terrain, rebuilt from its data, never
//! stored in the scene, and freed when their region empties or the terrain resets.
use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::math_conversions::from_v3;
use crate::mesh_arrays::{build_array_mesh, build_array_mesh_concat};
use crate::node_children::{add_internal, free_node};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    ConcavePolygonShape3D, MeshInstance3D, MultiMesh, MultiMeshInstance3D, Node3D, QuadMesh,
    StaticBody3D,
};
use godot::prelude::*;
use pt_core::math::{IVec2, Vec3 as PVec3, iv2};
use pt_core::mesh::MeshArrays;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const REGION_CELLS: f32 = 1200.0;
const MESH_REGION_CELLS: f32 = 300.0;
pub const GRASS_STRIDE: usize = 16;
pub fn region_chunks(chunk_cells: usize) -> i32 {
    (REGION_CELLS / chunk_cells.max(1) as f32)
        .sqrt()
        .round()
        .clamp(1.0, 8.0) as i32
}
pub fn mesh_region_chunks(chunk_cells: usize) -> i32 {
    (MESH_REGION_CELLS / chunk_cells.max(1) as f32)
        .sqrt()
        .round()
        .clamp(1.0, 8.0) as i32
}
pub struct ChunkColumns {
    pub sides: Vec<IVec2>,
    pub tops: Vec<IVec2>,
    pub side_key: u32,
    pub top_key: u32,
    pub rings: u32,
    pub low: bool,
    pub extent: Option<(PVec3, PVec3)>,
}
#[derive(Default)]
pub struct ChunkNodes {
    pub render: Option<MeshArrays>,
    pub form: Option<Gd<ConcavePolygonShape3D>>,
    pub owner: Option<u32>,
    pub grass: Vec<f32>,
    pub columns: Option<ChunkColumns>,
}
#[derive(Default)]
struct RegionNodes {
    body: Option<Gd<StaticBody3D>>,
    grass: Option<Gd<MultiMeshInstance3D>>,
    columns: BTreeMap<u32, Gd<MultiMeshInstance3D>>,
}
impl RegionNodes {
    fn free(self) {
        free_node(self.body);
        free_node(self.grass);
        for (_, n) in self.columns {
            free_node(Some(n));
        }
    }
}
pub struct Batches {
    pub size: i32,
    base: Option<Gd<Node3D>>,
    regions: HashMap<IVec2, RegionNodes>,
    pub dirty: BTreeSet<IVec2>,
    pub mesh_size: i32,
    meshes: HashMap<IVec2, Gd<MeshInstance3D>>,
    mesh_dirty: BTreeSet<IVec2>,
    pending_collision: HashMap<IVec2, PackedVector3Array>,
    last_edit_ms: u64,
}
impl Default for Batches {
    fn default() -> Self {
        Batches {
            size: 1,
            base: None,
            regions: HashMap::new(),
            dirty: BTreeSet::new(),
            mesh_size: 1,
            meshes: HashMap::new(),
            mesh_dirty: BTreeSet::new(),
            pending_collision: HashMap::new(),
            last_edit_ms: 0,
        }
    }
}
fn now_ms() -> u64 {
    godot::classes::Time::singleton().get_ticks_msec()
}
impl Batches {
    pub fn region_of(&self, c: IVec2) -> IVec2 {
        let r = self.size.max(1);
        iv2(c.x.div_euclid(r), c.y.div_euclid(r))
    }
    pub fn mesh_region_of(&self, c: IVec2) -> IVec2 {
        let r = self.mesh_size.max(1);
        iv2(c.x.div_euclid(r), c.y.div_euclid(r))
    }
    pub fn tag(&mut self, chunk: IVec2) {
        self.dirty.insert(self.region_of(chunk));
        self.mesh_dirty.insert(self.mesh_region_of(chunk));
    }
    fn mark_mesh(&mut self, chunk: IVec2) {
        self.mesh_dirty.insert(self.mesh_region_of(chunk));
    }
    pub fn purge(&mut self) {
        for (_, r) in self.regions.drain() {
            r.free();
        }
        for (_, m) in self.meshes.drain() {
            free_node(Some(m));
        }
        self.dirty.clear();
        self.mesh_dirty.clear();
        self.pending_collision.clear();
    }
}
struct Look {
    layers: u32,
    shadows: ShadowCastingSetting,
    grass_on: bool,
    mat: Option<Gd<godot::classes::ShaderMaterial>>,
    gmat: Option<Gd<godot::classes::ShaderMaterial>>,
}
impl MarchingSquaresTerrain {
    fn batch_look(&self) -> Look {
        Look {
            layers: self.render_layers | globals::meta_mask(),
            shadows: if self.cast_shadows {
                ShadowCastingSetting::ON
            } else {
                ShadowCastingSetting::OFF
            },
            grass_on: self.grass_on(),
            mat: self.land_stuff.clone(),
            gmat: self.turf_stuff.clone(),
        }
    }
    fn batch_root(&mut self) -> Gd<Node3D> {
        if let Some(r) = self.batches.base.as_ref().filter(|r| r.is_instance_valid()) {
            return r.clone();
        }
        let mut root = Node3D::new_alloc();
        root.set_name("Regions");
        let mut this = self.to_gd();
        add_internal(&mut this, &root);
        self.batches.base = Some(root.clone());
        self.batches.regions.clear();
        self.batches.meshes.clear();
        root
    }
    pub(super) fn set_chunk_mesh(&mut self, c: IVec2, mesh: Option<MeshArrays>) {
        let Some(n) = self.nodes.get_mut(&c) else {
            return;
        };
        n.render = mesh.filter(|m| !m.ordinals.is_empty()).map(|mut m| {
            m.collision_faces = Vec::new();
            m
        });
        self.batches.mark_mesh(c);
    }
    pub(super) fn assign_panel_impact(&mut self, c: IVec2, faces: PackedVector3Array) {
        if is_editor() && self.nodes.get(&c).is_some_and(|n| n.form.is_some()) {
            self.batches.pending_collision.insert(c, faces);
            self.batches.last_edit_ms = now_ms();
            return;
        }
        self.batches.pending_collision.remove(&c);
        self.assign_panel_impact_instant(c, faces);
    }
    pub(super) fn batches_flush_collision(&mut self, force: bool) {
        if self.batches.pending_collision.is_empty()
            || (!force
                && now_ms().saturating_sub(self.batches.last_edit_ms)
                    < self.collision_rebuild_delay_ms.max(0) as u64)
        {
            return;
        }
        let _p = crate::prof::breadth("flush.collision");
        let mut pending: Vec<(IVec2, PackedVector3Array)> =
            self.batches.pending_collision.drain().collect();
        pending.sort_by_key(|(c, _)| *c);
        for (c, faces) in pending {
            if self.nodes.contains_key(&c) {
                self.assign_panel_impact_instant(c, faces);
            }
        }
    }
    pub(super) fn chunk_collision_faces(&self, c: IVec2) -> PackedVector3Array {
        if let Some(f) = self.batches.pending_collision.get(&c) {
            return f.clone();
        }
        self.nodes
            .get(&c)
            .and_then(|n| n.form.as_ref())
            .map(|s| s.get_faces())
            .unwrap_or_default()
    }
    fn assign_panel_impact_instant(&mut self, c: IVec2, faces: PackedVector3Array) {
        let shape = (!faces.is_empty()).then(|| {
            let mut s = ConcavePolygonShape3D::new_gd();
            s.set_backface_collision_enabled(true);
            s.set_faces(&faces);
            s
        });
        self.set_chunk_shape(c, shape);
    }
    pub(super) fn set_chunk_shape(&mut self, c: IVec2, shape: Option<Gd<ConcavePolygonShape3D>>) {
        let r = self.batches.region_of(c);
        let (layer, mask, enabled) = (self.impact_stratum, self.impact_sieve, self.impact_active);
        let has_body = self
            .batches
            .regions
            .get(&r)
            .and_then(|g| g.body.as_ref())
            .is_some_and(|b| b.is_instance_valid());
        if shape.is_some() && !has_body {
            let mut root = self.batch_root();
            let mut b = StaticBody3D::new_alloc();
            b.set_name(&format!("Region ({}, {}) collision", r.x, r.y));
            b.set_collision_layer(layer);
            b.set_collision_mask(mask);
            root.add_child(&b);
            self.batches.regions.entry(r).or_default().body = Some(b);
        }
        let body = self
            .batches
            .regions
            .get(&r)
            .and_then(|g| g.body.clone())
            .filter(|b| b.is_instance_valid());
        let Some(n) = self.nodes.get_mut(&c) else {
            return;
        };
        let Some(mut body) = body else {
            n.form = None;
            n.owner = None;
            return;
        };
        match (&shape, n.owner) {
            (None, Some(id)) => {
                body.remove_shape_owner(id);
                n.owner = None;
            }
            (Some(s), owner) => {
                let id = match owner {
                    Some(id) => {
                        body.shape_owner_clear_shapes(id);
                        id
                    }
                    None => {
                        let obj = body.clone().upcast::<Object>();
                        body.create_shape_owner(&obj) as u32
                    }
                };
                body.shape_owner_add_shape(id, s);
                body.shape_owner_set_disabled(id, !enabled);
                n.owner = Some(id);
            }
            (None, None) => {}
        }
        n.form = shape;
        if body.get_shape_owners().is_empty() {
            free_node(Some(body));
            if let Some(g) = self.batches.regions.get_mut(&r) {
                g.body = None;
            }
        }
    }
    pub(super) fn remove_chunk_nodes(&mut self, c: IVec2) {
        if !self.nodes.contains_key(&c) {
            return;
        }
        self.set_chunk_shape(c, None);
        self.batches.pending_collision.remove(&c);
        self.nodes.remove(&c);
        self.batches.tag(c);
    }
    pub(super) fn clear_chunk_nodes(&mut self) {
        self.nodes.clear();
        self.batches.purge();
    }
    pub(super) fn batches_flush(&mut self) {
        self.batches_flush_meshes();
        if self.batches.dirty.is_empty() {
            return;
        }
        let _p = crate::prof::breadth("flush.regions");
        let dirty = std::mem::take(&mut self.batches.dirty);
        let mut members: BTreeMap<IVec2, Vec<IVec2>> =
            dirty.iter().map(|r| (*r, Vec::new())).collect();
        for c in self.nodes.keys() {
            if let Some(v) = members.get_mut(&self.batches.region_of(*c)) {
                v.push(*c);
            }
        }
        let look = self.batch_look();
        let size = Vector2::new(self.grass_size.x.max(0.0105), self.grass_size.y.max(0.0105));
        for (r, mut chunks) in members {
            chunks.sort();
            let _pg = crate::prof::breadth("regions.grass");
            let mut grass: Vec<f32> = Vec::new();
            for c in &chunks {
                grass.extend_from_slice(&self.nodes[c].grass);
            }
            let old = self
                .batches
                .regions
                .get_mut(&r)
                .and_then(|g| g.grass.take())
                .filter(|n| n.is_instance_valid());
            if grass.is_empty() {
                free_node(old);
            } else {
                let mmi = old.unwrap_or_else(|| {
                    let mut mmi = MultiMeshInstance3D::new_alloc();
                    mmi.set_name(&format!("Region ({}, {}) grass", r.x, r.y));
                    let mut mm = MultiMesh::new_gd();
                    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                    mm.set_use_custom_data(true);
                    mm.set_mesh(&QuadMesh::new_gd());
                    mmi.set_multimesh(&mm);
                    apply_grass_look(&mut mmi, &look);
                    self.batch_root().add_child(&mmi);
                    mmi
                });
                if let Some(mut mm) = mmi.get_multimesh() {
                    if let Some(mut quad) =
                        mm.get_mesh().and_then(|m| m.try_cast::<QuadMesh>().ok())
                        && quad.get_size() != size
                    {
                        quad.set_size(size);
                        quad.set_center_offset(Vector3::new(0.0, size.y * 0.5, 0.0));
                    }
                    let count = (grass.len() / GRASS_STRIDE) as i32;
                    if mm.get_instance_count() != count {
                        mm.set_instance_count(count);
                    }
                    mm.set_buffer(&PackedFloat32Array::from(grass.as_slice()));
                }
                self.batches.regions.entry(r).or_default().grass = Some(mmi);
            }
            drop(_pg);
            let _pc = crate::prof::breadth("regions.columns");
            let mut keys: BTreeMap<u32, (Vec<f32>, Option<(PVec3, PVec3)>)> = BTreeMap::new();
            for c in &chunks {
                let Some(col) = &self.nodes[c].columns else {
                    continue;
                };
                let lowf = if col.low { 1.0 } else { 0.0 };
                for (key, cells, rings) in [
                    (col.side_key, &col.sides, col.rings),
                    (col.top_key, &col.tops, 0),
                ] {
                    if cells.is_empty() {
                        continue;
                    }
                    let e = keys.entry(key).or_default();
                    for cell in cells {
                        crate::writes::push_instance(
                            &mut e.0,
                            Transform3D::IDENTITY,
                            None,
                            Some(Color::from_rgba(
                                cell.x as f32,
                                cell.y as f32,
                                lowf,
                                rings as f32,
                            )),
                        );
                    }
                    if let Some((lo, hi)) = col.extent {
                        e.1 = Some(match e.1 {
                            Some((a, b)) => (
                                pt_core::math::v3(a.x.min(lo.x), a.y.min(lo.y), a.z.min(lo.z)),
                                pt_core::math::v3(b.x.max(hi.x), b.y.max(hi.y), b.z.max(hi.z)),
                            ),
                            None => (lo, hi),
                        });
                    }
                }
            }
            let mut old = self
                .batches
                .regions
                .get_mut(&r)
                .map(|g| std::mem::take(&mut g.columns))
                .unwrap_or_default();
            let mut now = BTreeMap::new();
            for (key, (buf, bounds)) in keys {
                let Some(template) = self.columns_template(key) else {
                    continue;
                };
                let mut mmi = old
                    .remove(&key)
                    .filter(|n| n.is_instance_valid())
                    .unwrap_or_else(|| {
                        let mut mmi = MultiMeshInstance3D::new_alloc();
                        mmi.set_name(&format!(
                            "Region ({}, {}) {}",
                            r.x,
                            r.y,
                            super::hex_columns::template_name(key)
                        ));
                        let mut mm = MultiMesh::new_gd();
                        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                        mm.set_use_custom_data(true);
                        mmi.set_multimesh(&mm);
                        apply_columns_look(&mut mmi, &look);
                        self.batch_root().add_child(&mmi);
                        mmi
                    });
                if let Some(mut mm) = mmi.get_multimesh() {
                    if mm.get_mesh().as_ref() != Some(&template.clone().upcast()) {
                        mm.set_mesh(&template);
                    }
                    let count = (buf.len() / GRASS_STRIDE) as i32;
                    if mm.get_instance_count() != count {
                        mm.set_instance_count(count);
                    }
                    mm.set_buffer(&PackedFloat32Array::from(buf.as_slice()));
                }
                if let Some((lo, hi)) = bounds {
                    mmi.set_custom_aabb(Aabb::new(from_v3(lo), from_v3(hi - lo)));
                }
                now.insert(key, mmi);
            }
            for (_, n) in old {
                free_node(Some(n));
            }
            let empty = {
                let g = self.batches.regions.entry(r).or_default();
                g.columns = now;
                g.grass.is_none() && g.columns.is_empty() && g.body.is_none()
            };
            if empty {
                self.batches.regions.remove(&r);
            }
        }
    }
    fn batches_flush_meshes(&mut self) {
        if self.batches.mesh_dirty.is_empty() {
            return;
        }
        let _p = crate::prof::breadth("regions.mesh");
        let dirty = std::mem::take(&mut self.batches.mesh_dirty);
        let mut members: BTreeMap<IVec2, Vec<IVec2>> =
            dirty.iter().map(|r| (*r, Vec::new())).collect();
        for c in self.nodes.keys() {
            if let Some(v) = members.get_mut(&self.batches.mesh_region_of(*c)) {
                v.push(*c);
            }
        }
        let look = self.batch_look();
        for (r, mut chunks) in members {
            chunks.sort();
            let mesh = {
                let parts: Vec<&MeshArrays> = chunks
                    .iter()
                    .filter_map(|c| self.nodes[c].render.as_ref())
                    .collect();
                build_array_mesh_concat(&parts)
            };
            let old = self
                .batches
                .meshes
                .remove(&r)
                .filter(|n| n.is_instance_valid());
            let Some(mesh) = mesh else {
                free_node(old);
                continue;
            };
            let mut mi = old.unwrap_or_else(|| {
                let mut mi = MeshInstance3D::new_alloc();
                mi.set_name(&format!("Region mesh ({}, {})", r.x, r.y));
                apply_mesh_look(&mut mi, &look);
                self.batch_root().add_child(&mi);
                mi
            });
            mi.set_mesh(&mesh);
            self.batches.meshes.insert(r, mi);
        }
    }
    pub(super) fn batches_apply_settings(&mut self) {
        let look = self.batch_look();
        let (layer, mask, enabled) = (self.impact_stratum, self.impact_sieve, self.impact_active);
        for mi in self.batches.meshes.values_mut() {
            if mi.is_instance_valid() {
                apply_mesh_look(mi, &look);
            }
        }
        for g in self.batches.regions.values_mut() {
            if let Some(mmi) = g.grass.as_mut().filter(|m| m.is_instance_valid()) {
                apply_grass_look(mmi, &look);
            }
            for mmi in g.columns.values_mut() {
                if mmi.is_instance_valid() {
                    apply_columns_look(mmi, &look);
                }
            }
            if let Some(b) = g.body.as_mut().filter(|b| b.is_instance_valid()) {
                b.set_collision_layer(layer);
                b.set_collision_mask(mask);
                for id in b.get_shape_owners().as_slice() {
                    b.shape_owner_set_disabled(*id as u32, !enabled);
                }
            }
        }
    }
    pub(super) fn chunk_grass_multimesh(&self, c: IVec2) -> Option<Gd<MultiMesh>> {
        let n = self.nodes.get(&c)?;
        if n.grass.is_empty() {
            return None;
        }
        let size = Vector2::new(self.grass_size.x.max(0.0105), self.grass_size.y.max(0.0105));
        let mut quad = QuadMesh::new_gd();
        quad.set_size(size);
        quad.set_center_offset(Vector3::new(0.0, size.y * 0.5, 0.0));
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&quad);
        mm.set_instance_count((n.grass.len() / GRASS_STRIDE) as i32);
        mm.set_buffer(&PackedFloat32Array::from(n.grass.as_slice()));
        Some(mm)
    }
    pub(super) fn chunk_mesh(&self, c: IVec2) -> Option<Gd<godot::classes::Mesh>> {
        self.nodes
            .get(&c)?
            .render
            .as_ref()
            .and_then(build_array_mesh)
            .map(|m| m.upcast())
    }
}
pub fn grass_buffer(mm: &Gd<MultiMesh>) -> Vec<f32> {
    let n = mm.get_instance_count().max(0) as usize;
    let src = mm.get_buffer();
    let src = src.as_slice();
    let colors = mm.is_using_colors();
    let custom = mm.is_using_custom_data();
    let stride = 12 + if colors { 4 } else { 0 } + if custom { 4 } else { 0 };
    if mm.get_transform_format() != TransformFormat::TRANSFORM_3D || src.len() < n * stride {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n * GRASS_STRIDE);
    for i in 0..n {
        let s = &src[i * stride..(i + 1) * stride];
        out.extend_from_slice(&s[..12]);
        if custom {
            out.extend_from_slice(&s[stride - 4..]);
        } else {
            out.extend_from_slice(&[0.0; 4]);
        }
    }
    out
}
fn apply_mesh_look(mi: &mut Gd<MeshInstance3D>, look: &Look) {
    mi.set_layer_mask(look.layers);
    mi.set_cast_shadows_setting(look.shadows);
    if let Some(m) = &look.mat {
        mi.set_material_override(m);
    }
}
fn apply_grass_look(mmi: &mut Gd<MultiMeshInstance3D>, look: &Look) {
    mmi.set_layer_mask(look.layers);
    mmi.set_visible(look.grass_on);
    mmi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    if let Some(m) = &look.gmat {
        mmi.set_material_override(m);
    }
}
fn apply_columns_look(mmi: &mut Gd<MultiMeshInstance3D>, look: &Look) {
    mmi.set_layer_mask(look.layers);
    mmi.set_cast_shadows_setting(look.shadows);
    if let Some(m) = &look.mat {
        mmi.set_material_override(m);
    }
}
