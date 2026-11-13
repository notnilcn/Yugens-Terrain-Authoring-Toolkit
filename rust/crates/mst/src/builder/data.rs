//! `PixelBuilderMeshData` — the editable polygon mesh of a `PixelBuilderMesh`, stored as one
//! versioned binary blob (`pt_core::build::blob`) plus material slots.
//!
//! The GDScript API (`add_shape`, `extrude`, `inset`, `faces_facing`, …) lets tools and tests
//! build geometry procedurally; the editor plugin uses the same operations.

use std::collections::BTreeSet;

use godot::classes::{Material, Resource};
use godot::prelude::*;
use pt_core::build::{self, BMesh, ShapeKind, ShapeSpec, blob, ops};
use pt_core::math::{Vec3 as PVec3, v3};

use super::spec;
use crate::terrain::MarchingSquaresTerrain;
use crate::util::to_iv;

pub fn to_p(v: Vector3) -> PVec3 {
    v3(v.x, v.y, v.z)
}
pub fn from_p(v: PVec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}
pub fn face_set(faces: &PackedInt32Array) -> BTreeSet<usize> {
    faces.as_slice().iter().filter(|f| **f >= 0).map(|f| *f as usize).collect()
}
pub fn face_array(faces: &BTreeSet<usize>) -> PackedInt32Array {
    faces.iter().map(|f| *f as i32).collect()
}

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelBuilderMeshData {
    /// Encoded polygon mesh (see `pt_core::build::blob`).
    #[export(storage)]
    #[var(get = get_blob, set = set_blob)]
    blob: PackedByteArray,
    /// One material per slot; faces pick a slot. Empty slots use a vertex-color material.
    #[export]
    #[var(set = set_materials)]
    materials: Array<Option<Gd<Material>>>,
    /// World units per texture repeat for the automatic UVs (1 = a 16 px texture is 16 texels/m).
    #[export(range = (0.0625, 16.0, 0.0625))]
    #[var(set = set_units_per_repeat)]
    units_per_repeat: f32,
    /// Recipe of the last generated shape (`kind`, `size`, …), used by "Rebuild shape".
    #[export(storage)]
    shape: VarDictionary,

    mesh: BMesh,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for PixelBuilderMeshData {
    fn init(base: Base<Resource>) -> Self {
        PixelBuilderMeshData {
            blob: PackedByteArray::new(),
            materials: Array::new(),
            units_per_repeat: 1.0,
            shape: VarDictionary::new(),
            mesh: BMesh::default(),
            base,
        }
    }
}

impl PixelBuilderMeshData {
    pub fn mesh(&self) -> &BMesh {
        &self.mesh
    }
    pub fn units(&self) -> f32 {
        self.units_per_repeat
    }
    pub fn material_slots(&self) -> Array<Option<Gd<Material>>> {
        self.materials.clone()
    }
    /// Replaces the mesh (re-encodes the blob). The caller must `emit_changed` afterwards,
    /// outside any bind of this resource.
    pub fn replace(&mut self, m: BMesh) {
        self.blob = PackedByteArray::from(blob::encode(&m).as_slice());
        self.mesh = m;
    }
    pub fn set_shape_spec(&mut self, s: &ShapeSpec) {
        self.shape = spec::to_dict(s);
    }
    /// The stored shape recipe (`None` for meshes not drawn from a shape).
    pub fn shape_spec(&self) -> Option<ShapeSpec> {
        (!self.shape.is_empty()).then(|| spec::from_dict(&self.shape, ShapeSpec::default()))
    }
    fn edit<R>(&mut self, f: impl FnOnce(&mut BMesh) -> R) -> R {
        let mut m = std::mem::take(&mut self.mesh);
        let r = f(&mut m);
        self.replace(m);
        self.base_mut().emit_changed();
        r
    }
}

#[godot_api]
impl PixelBuilderMeshData {
    #[func]
    pub fn get_blob(&self) -> PackedByteArray {
        self.blob.clone()
    }

    /// Replaces the mesh from an encoded blob (used by undo/redo).
    #[func]
    pub fn set_blob(&mut self, b: PackedByteArray) {
        match blob::decode(b.as_slice()) {
            Ok(m) => {
                self.mesh = m;
                self.blob = b;
            }
            Err(e) => godot_error!("PixelBuilderMeshData: {e}"),
        }
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_materials(&mut self, v: Array<Option<Gd<Material>>>) {
        self.materials = v;
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_units_per_repeat(&mut self, v: f32) {
        self.units_per_repeat = v.max(0.01);
        self.base_mut().emit_changed();
    }

    #[func]
    fn get_vertex_count(&self) -> i64 {
        self.mesh.positions.len() as i64
    }
    #[func]
    fn get_face_count(&self) -> i64 {
        self.mesh.faces.len() as i64
    }
    #[func]
    fn get_face_center(&self, face: i64) -> Vector3 {
        if (face as usize) < self.mesh.faces.len() { from_p(self.mesh.face_center(face as usize)) } else { Vector3::ZERO }
    }
    #[func]
    fn get_face_normal(&self, face: i64) -> Vector3 {
        if (face as usize) < self.mesh.faces.len() { from_p(self.mesh.face_normal(face as usize)) } else { Vector3::UP }
    }
    #[func]
    fn get_face_vertices(&self, face: i64) -> PackedInt32Array {
        self.mesh.faces.get(face as usize).map(|f| f.verts.iter().map(|v| *v as i32).collect()).unwrap_or_default()
    }
    #[func]
    fn get_vertex(&self, v: i64) -> Vector3 {
        self.mesh.positions.get(v as usize).map(|p| from_p(*p)).unwrap_or_default()
    }
    #[func]
    fn set_vertex(&mut self, v: i64, p: Vector3) {
        self.edit(|m| {
            if let Some(q) = m.positions.get_mut(v as usize) {
                *q = to_p(p);
            }
        });
    }
    /// True if every edge is shared by exactly two faces.
    #[func]
    fn is_closed(&self) -> bool {
        self.mesh.is_closed()
    }

    #[func]
    fn clear(&mut self) {
        self.edit(|m| *m = BMesh::default());
    }

    /// Appends a generated shape (bottom centre at `offset`). `kind`: 0 Box, 1 Plane,
    /// 2 Cylinder, 3 Cone, 4 Sphere, 5 Stairs, 6 Arch, 7 Ramp, 8 Pipe, 9 Hex prism,
    /// 10 Hex cluster, 11 Torus, 12 Door, 13 Curved stairs, 14 Trim, 15 Wall. `params` may hold
    /// `segments`, `steps`, `thickness`, `smooth`, `rings`, `arc`, `inner`, `opening`, `door_height`,
    /// `arched`, `profile`, `side`,
    /// `material`, `color`, `rotation_y` (degrees). Returns the new face indices.
    #[func]
    fn add_shape(&mut self, kind: i64, size: Vector3, offset: Vector3, params: VarDictionary) -> PackedInt32Array {
        let mut s = spec::from_dict(&params, ShapeSpec::default());
        s.kind = ShapeKind::from_i64(kind);
        s.size = to_p(size);
        let g = build::shapes::generate(&s, None);
        self.set_shape_spec(&s);
        self.append_generated(g, offset, &params)
    }

    /// Appends whole terrain cells as a solid (`cells` in the terrain's sample coordinates;
    /// positions are relative to the centre of `origin`). Returns the new face indices.
    #[func]
    fn add_terrain_cells(&mut self, terrain: Gd<MarchingSquaresTerrain>, cells: VarArray, origin: Vector2i, height: f32, offset: Vector3, params: VarDictionary) -> PackedInt32Array {
        let cfg = terrain.bind().store().config.clone();
        let list: Vec<_> = cells.iter_shared().filter_map(|v| v.try_to::<Vector2i>().ok()).map(to_iv).collect();
        let g = build::shapes::cells_prism(&cfg, &list, to_iv(origin), height);
        self.append_generated(g, offset, &params)
    }

    /// Faces whose normal is within `max_angle_deg` of `dir` (optionally only among `among`).
    #[func]
    fn faces_facing(&self, dir: Vector3, max_angle_deg: f32, among: PackedInt32Array) -> PackedInt32Array {
        let d = to_p(dir).normalized();
        let c = max_angle_deg.to_radians().cos();
        let pool: Vec<usize> = if among.is_empty() { (0..self.mesh.faces.len()).collect() } else { face_set(&among).into_iter().collect() };
        pool.into_iter().filter(|f| *f < self.mesh.faces.len() && self.mesh.face_normal(*f).dot(d) >= c - 1e-5).map(|f| f as i32).collect()
    }

    #[func]
    fn extrude(&mut self, faces: PackedInt32Array, distance: f32) -> PackedInt32Array {
        let s = face_set(&faces);
        face_array(&self.edit(|m| ops::extrude_faces(m, &s, distance)))
    }
    #[func]
    fn inset(&mut self, faces: PackedInt32Array, amount: f32) -> PackedInt32Array {
        let s = face_set(&faces);
        face_array(&self.edit(|m| ops::inset_faces(m, &s, amount)))
    }
    #[func]
    fn subdivide(&mut self, faces: PackedInt32Array) -> PackedInt32Array {
        let s = face_set(&faces);
        face_array(&self.edit(|m| ops::subdivide_faces(m, &s)))
    }
    #[func]
    fn delete_faces(&mut self, faces: PackedInt32Array) {
        let s = face_set(&faces);
        self.edit(|m| ops::delete_faces(m, &s));
    }
    #[func]
    fn flip_faces(&mut self, faces: PackedInt32Array) {
        let s = face_set(&faces);
        self.edit(|m| ops::flip_faces(m, &s));
    }
    /// Moves every vertex of `faces` by `delta`.
    #[func]
    fn translate_faces(&mut self, faces: PackedInt32Array, delta: Vector3) {
        let s = face_set(&faces);
        self.edit(|m| {
            let v = m.verts_of_faces(&s);
            ops::translate_verts(m, &v, to_p(delta));
        });
    }
    /// Scales the vertices of `faces` about their centroid.
    #[func]
    fn scale_faces(&mut self, faces: PackedInt32Array, scale: Vector3) {
        let s = face_set(&faces);
        self.edit(|m| {
            let v = m.verts_of_faces(&s);
            let c = v.iter().fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize]) / v.len().max(1) as f32;
            ops::scale_verts(m, &v, c, to_p(scale));
        });
    }
    #[func]
    fn weld(&mut self, threshold: f32) -> i64 {
        self.edit(|m| ops::weld_by_distance(m, threshold, None)) as i64
    }
    #[func]
    fn set_faces_material(&mut self, faces: PackedInt32Array, slot: i64) {
        let s = face_set(&faces);
        self.edit(|m| ops::set_material(m, &s, slot.clamp(0, 255) as u16));
    }
    #[func]
    fn set_faces_color(&mut self, faces: PackedInt32Array, color: Color) {
        let s = face_set(&faces);
        let c = color_bytes(color);
        self.edit(|m| ops::set_color(m, &s, c));
    }
    /// Chamfers edges given as vertex pairs `[a0, b0, a1, b1, …]`. Returns the strip faces.
    #[func]
    fn bevel(&mut self, edge_pairs: PackedInt32Array, amount: f32) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs.as_slice().chunks_exact(2).map(|p| build::edge(p[0] as u32, p[1] as u32)).collect();
        face_array(&self.edit(|m| build::topo::bevel_edges(m, &e, amount)))
    }
    /// Bevels edges (vertex pairs) by `amount` over `segments` strips (rounded and smooth
    /// shaded for segments > 1). Returns the new faces.
    #[func]
    fn bevel_rounded(&mut self, edge_pairs: PackedInt32Array, amount: f32, segments: i64) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs.as_slice().chunks_exact(2).map(|p| build::edge(p[0] as u32, p[1] as u32)).collect();
        let s = segments.clamp(1, 16) as u32;
        face_array(&self.edit(|m| build::bevel::bevel_edges(m, &e, amount, s, s > 1)))
    }
    /// Splits edges (vertex pairs) into `segments` equal parts (Unibuilder Subdivide edges).
    /// Returns the new vertices.
    #[func]
    fn subdivide_edges(&mut self, edge_pairs: PackedInt32Array, segments: i64) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs.as_slice().chunks_exact(2).map(|p| build::edge(p[0] as u32, p[1] as u32)).collect();
        let s = segments.clamp(2, 64) as u32;
        self.edit(|m| build::edit_ops::subdivide_edges(m, &e, s)).into_iter().map(|v| v as i32).collect()
    }
    /// Connects edges (vertex pairs) through their midpoints (Unibuilder Connect edges).
    /// Returns the new chords as vertex pairs.
    #[func]
    fn connect_edges(&mut self, edge_pairs: PackedInt32Array) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs.as_slice().chunks_exact(2).map(|p| build::edge(p[0] as u32, p[1] as u32)).collect();
        self.edit(|m| build::edit_ops::connect_edges(m, &e)).into_iter().flat_map(|(x, y)| [x as i32, y as i32]).collect()
    }
    /// Unibuilder subdivide: `segments` (even) pieces along every edge of the faces. Returns
    /// the new faces.
    #[func]
    fn subdivide_segments(&mut self, faces: PackedInt32Array, segments: i64) -> PackedInt32Array {
        let s = face_set(&faces);
        face_array(&self.edit(|m| build::ub_ops::subdivide_faces(m, &s, segments.clamp(2, 64) as u32)))
    }
    /// Unibuilder inset: a rim `amount` wide (metres); `equal` mitres the corners.
    #[func]
    fn inset_width(&mut self, faces: PackedInt32Array, amount: f32, equal: bool) -> PackedInt32Array {
        let s = face_set(&faces);
        face_array(&self.edit(|m| build::ub_ops::inset_faces(m, &s, amount, equal)))
    }
    /// Collapses vertices into one at their centre; returns it (-1 when refused).
    #[func]
    fn collapse_vertices(&mut self, verts: PackedInt32Array) -> i64 {
        let s: BTreeSet<u32> = verts.as_slice().iter().filter(|v| **v >= 0).map(|v| *v as u32).collect();
        self.edit(|m| build::ub_ops::collapse_vertices(m, &s)).map(|v| v as i64).unwrap_or(-1)
    }
    /// Splits edge (a, b) at local `pos`; returns the new vertex (-1 if (a, b) is no edge).
    #[func]
    fn insert_vertex(&mut self, a: i64, b: i64, pos: Vector3) -> i64 {
        self.edit(|m| build::ub_ops::insert_vertex(m, (a as u32, b as u32), to_p(pos))).map(|v| v as i64).unwrap_or(-1)
    }
    /// Trim walls: a moulding (`profile` 0–5, `depth` x `height`) along the base (`top`: under
    /// the top) of the given wall faces, appended. Returns the new faces.
    #[func]
    fn trim_walls(&mut self, faces: PackedInt32Array, top: bool, depth: f32, height: f32, profile: i64) -> PackedInt32Array {
        let s = face_set(&faces);
        let t = build::trim_walls::trim_walls(&self.mesh, &s, top, depth, height, build::prims::TrimProfile::from_u8(profile.clamp(0, 5) as u8), 6, 0.0, true);
        face_array(&self.edit(|m| {
            let start = m.faces.len();
            m.append(&t);
            (start..m.faces.len()).collect::<BTreeSet<usize>>()
        }))
    }
    /// Poly shape: the local XZ outline (`points`, y ignored) extruded `height` up at `offset`.
    #[func]
    fn add_poly(&mut self, points: PackedVector3Array, height: f32, offset: Vector3, params: VarDictionary) -> PackedInt32Array {
        let pts: Vec<pt_core::math::Vec2> = points.as_slice().iter().map(|p| pt_core::math::v2(p.x, p.z)).collect();
        let g = build::trim_walls::polygon_prism(&pts, height);
        self.append_generated(g, offset, &params)
    }
    /// Turns the triangulation diagonal of the given quads. Returns how many faces changed.
    #[func]
    fn flip_face_edge(&mut self, faces: PackedInt32Array) -> i64 {
        let s = face_set(&faces);
        self.edit(|m| build::edit_ops::flip_face_edge(m, &s)) as i64
    }
    /// Gives every face touching the given vertices its own copy. Returns the new vertices.
    #[func]
    fn split_vertices(&mut self, verts: PackedInt32Array) -> PackedInt32Array {
        let s: BTreeSet<u32> = verts.as_slice().iter().filter(|v| **v >= 0).map(|v| *v as u32).collect();
        self.edit(|m| build::edit_ops::split_vertices(m, &s)).into_iter().map(|v| v as i32).collect()
    }
    /// Splits faces into triangles (all faces when `faces` is empty). Returns the triangles.
    #[func]
    fn triangulate(&mut self, faces: PackedInt32Array) -> PackedInt32Array {
        let mut s = face_set(&faces);
        if s.is_empty() {
            s = (0..self.mesh.faces.len()).collect();
        }
        face_array(&self.edit(|m| ops::triangulate_faces(m, &s)))
    }
    /// Appends `other`'s geometry transformed by `xform` (other's local → this mesh's local);
    /// its faces keep their slot numbers. Returns the new faces.
    #[func]
    fn merge_from(&mut self, other: Gd<PixelBuilderMeshData>, xform: Transform3D) -> PackedInt32Array {
        let src = if other.instance_id() == self.to_gd().instance_id() { self.mesh.clone() } else { other.bind().mesh().clone() };
        let xf = build::edit_ops::Affine { x: to_p(xform.basis.col_a()), y: to_p(xform.basis.col_b()), z: to_p(xform.basis.col_c()), origin: to_p(xform.origin) };
        face_array(&self.edit(|m| build::edit_ops::merge_objects(m, &src, &xf, &[])))
    }
    /// Inserts `cuts` edge loops across the quad ring through edge (a, b). Returns the new
    /// loop edges as vertex pairs.
    #[func]
    fn insert_loop(&mut self, a: i64, b: i64, cuts: i64) -> PackedInt32Array {
        let es = self.edit(|m| build::loops::insert_loop(m, build::edge(a as u32, b as u32), cuts.clamp(1, 16) as u32));
        es.into_iter().flat_map(|(x, y)| [x as i32, y as i32]).collect()
    }
    /// Bridges two open edges (vertex pairs `[a0, b0, a1, b1]`) with a face; returns it (-1 if not possible).
    #[func]
    fn bridge(&mut self, edge_pairs: PackedInt32Array) -> i64 {
        let p = edge_pairs.as_slice();
        if p.len() < 4 {
            return -1;
        }
        let (e1, e2) = (build::edge(p[0] as u32, p[1] as u32), build::edge(p[2] as u32, p[3] as u32));
        self.edit(|m| build::loops::bridge_edges(m, e1, e2)).map(|f| f as i64).unwrap_or(-1)
    }
    /// Adds a copy mirrored across the local `axis` (0 x, 1 y, 2 z) = 0 plane, welding the seam.
    #[func]
    fn mirror(&mut self, axis: i64) {
        self.edit(|m| build::loops::mirror_object(m, axis.clamp(0, 2) as usize, true, 1e-3));
    }
    /// Knife: cuts the polygon `points` (local, on the face plane) into `face`. Returns the new
    /// inner face (-1 if the polygon does not fit the face).
    #[func]
    fn cut_face(&mut self, face: i64, points: PackedVector3Array) -> i64 {
        let pts: Vec<PVec3> = points.as_slice().iter().map(|p| to_p(*p)).collect();
        self.edit(|m| build::cut::cut_face(m, face.max(0) as usize, &pts)).map(|f| f as i64).unwrap_or(-1)
    }
    /// Appends a wall swept along `points` (local, y ignored -> 0; `closed` joins the ends).
    /// `params`: `thickness`, `side` (0 centre, 1 left, 2 right), `material`, `color`.
    #[func]
    fn add_wall(&mut self, points: PackedVector3Array, closed: bool, height: f32, offset: Vector3, params: VarDictionary) -> PackedInt32Array {
        let mut s = spec::from_dict(&params, ShapeSpec::default());
        s.kind = ShapeKind::Wall;
        s.size.y = height;
        let pts: Vec<PVec3> = points.as_slice().iter().map(|p| v3(p.x, 0.0, p.z)).collect();
        let g = build::shapes::wall_polyline(&s, &pts, closed);
        self.append_generated(g, offset, &params)
    }
    /// Edges (vertex pairs, flat) of `faces`.
    #[func]
    fn get_face_edges(&self, faces: PackedInt32Array) -> PackedInt32Array {
        let s = face_set(&faces);
        self.mesh.edges_of_faces(&s).into_iter().flat_map(|(a, b)| [a as i32, b as i32]).collect()
    }
    /// Merges connected faces into one n-gon; returns its index (-1 if not possible).
    #[func]
    fn merge_faces(&mut self, faces: PackedInt32Array) -> i64 {
        let s = face_set(&faces);
        self.edit(|m| build::topo::merge_faces(m, &s)).map(|f| f as i64).unwrap_or(-1)
    }
    /// Sets the UV projection of `faces` (offset / scale in repeats, rotation in degrees).
    #[func]
    fn set_faces_uv(&mut self, faces: PackedInt32Array, offset: Vector2, scale: Vector2, rotation_deg: f32, world: bool) {
        let s = face_set(&faces);
        let uv = build::UvParams { offset: pt_core::math::v2(offset.x, offset.y), scale: pt_core::math::v2(scale.x, scale.y), rotation: rotation_deg, world };
        self.edit(|m| ops::set_uv(m, &s, uv));
    }
    /// Wavefront OBJ text of the mesh (object-space UVs).
    #[func]
    fn to_obj(&self, name: GString) -> GString {
        let opt = build::compile::CompileOptions { units_per_repeat: self.units_per_repeat, ..Default::default() };
        build::convert::to_obj(&self.mesh, &opt, &name.to_string(), None, &[]).as_str().into()
    }
    /// Replaces the mesh with the triangles of `mesh` (coplanar triangles become n-gons,
    /// one slot per surface).
    #[func]
    fn set_from_mesh(&mut self, mesh: Gd<godot::classes::Mesh>) {
        let mi = godot::classes::MeshInstance3D::new_alloc();
        let (m, mats) = super::plugin::mesh_to_bmesh(&mesh, &mi);
        mi.free();
        self.materials = mats;
        self.edit(|x| *x = m);
    }

    /// Makes face windings consistent per connected part (Unibuilder "Conform Normals").
    /// Returns the number of faces flipped.
    #[func]
    fn conform_normals(&mut self) -> i64 {
        self.edit(pt_core::build::ops::conform_normals) as i64
    }

    /// UVs a face shows (manual per-corner UVs, else the automatic projection with the node
    /// origin `world_offset`).
    #[func]
    fn get_face_uvs(&self, face: i64, world_offset: Vector3) -> PackedVector2Array {
        if face < 0 || face as usize >= self.mesh.faces.len() {
            return PackedVector2Array::new();
        }
        let a = pt_core::build::uv_edit::AutoUv { units_per_repeat: self.units_per_repeat, world_offset: to_p(world_offset) };
        pt_core::build::uv_edit::face_uvs(&self.mesh, face as usize, &a).iter().map(|p| Vector2::new(p.x, p.y)).collect()
    }

    /// True if the face uses manual (UV editor) coordinates.
    #[func]
    fn is_face_uv_manual(&self, face: i64) -> bool {
        self.mesh.faces.get(face.max(0) as usize).is_some_and(|f| f.manual_uvs().is_some())
    }

    /// Sets manual per-corner UVs (one per corner, else ignored).
    #[func]
    fn set_face_uvs(&mut self, face: i64, uvs: PackedVector2Array) {
        let n = self.mesh.faces.get(face.max(0) as usize).map(|f| f.verts.len());
        if n != Some(uvs.len()) {
            return;
        }
        let coords: Vec<pt_core::math::Vec2> = uvs.as_slice().iter().map(|p| pt_core::math::v2(p.x, p.y)).collect();
        self.edit(|m| {
            let f = &mut m.faces[face as usize];
            f.uv_coords = coords;
            f.uv_manual = true;
        });
    }

    /// Vertex paint, Fill: paints every corner of the faces (`mask` 0 RGBA, 1 RGB, 2 alpha only).
    #[func]
    fn paint_faces(&mut self, faces: PackedInt32Array, color: Color, mask: i64) {
        let s = face_set(&faces);
        self.edit(|m| build::paint::paint_faces(m, &s, [color.r, color.g, color.b, color.a], build::paint::Mask::from_i64(mask)));
    }

    /// Vertex paint, one brush dab at a local-space point (falloff by `softness`).
    #[func]
    fn paint_brush(&mut self, center: Vector3, radius: f32, strength: f32, softness: f32, color: Color, mask: i64) {
        let base = self.mesh.clone();
        let w = build::paint::brush_weights(&base, |p| p, to_p(center), radius, strength, softness);
        self.edit(|m| build::paint::paint_weighted(m, &base, &w, [color.r, color.g, color.b, color.a], build::paint::Mask::from_i64(mask)));
    }

    /// Inverts the painted alpha (swaps the two layers of a blend material).
    #[func]
    fn invert_alpha(&mut self, faces: PackedInt32Array) {
        let s = face_set(&faces);
        self.edit(|m| build::paint::invert_alpha(m, &s));
    }

    /// Color of one corner (its vertex-paint color, else the face color).
    #[func]
    fn get_corner_color(&self, face: i64, corner: i64) -> Color {
        let c = build::paint::to_f(build::paint::sample(&self.mesh, face.max(0) as usize, corner.max(0) as usize));
        Color::from_rgba(c[0], c[1], c[2], c[3])
    }

    /// Removes the vertex-paint colors of the faces (back to the face color).
    #[func]
    fn clear_corner_colors(&mut self, faces: PackedInt32Array) {
        let s = face_set(&faces);
        self.edit(|m| build::paint::clear_corner_colors(m, &s));
    }

    /// Material slot of a face (-1 if out of range).
    #[func]
    fn get_face_slot(&self, face: i64) -> i64 {
        self.mesh.faces.get(face.max(0) as usize).map(|f| f.material as i64).unwrap_or(-1)
    }

    /// Frozen automatic-UV projection axis of a face (UV lock; zero = the face normal).
    #[func]
    fn get_face_uv_axis(&self, face: i64) -> Vector3 {
        self.mesh.faces.get(face.max(0) as usize).map(|f| from_p(f.uv_axis)).unwrap_or(Vector3::ZERO)
    }

    #[func]
    fn set_faces_smooth(&mut self, faces: PackedInt32Array, group: i64) {
        let s = face_set(&faces);
        self.edit(|m| ops::set_smooth(m, &s, group.clamp(0, 65535) as u16));
    }
}

pub fn color_bytes(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl PixelBuilderMeshData {
    fn append_generated(&mut self, mut g: BMesh, offset: Vector3, params: &VarDictionary) -> PackedInt32Array {
        if let Some(r) = params.get("rotation_y").and_then(|v| v.try_to::<f64>().ok()) {
            let all = (0..g.positions.len() as u32).collect();
            ops::rotate_verts_y(&mut g, &all, PVec3::ZERO, (r as f32).to_radians());
        }
        g.translate(to_p(offset));
        let mat = params.get("material").and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0).clamp(0, 255) as u16;
        let col = params.get("color").and_then(|v| v.try_to::<Color>().ok()).map(color_bytes);
        for f in &mut g.faces {
            f.material = mat;
            if let Some(c) = col {
                f.color = c;
            }
        }
        let added = self.edit(|m| {
            let start = m.faces.len();
            m.append(&g);
            start..m.faces.len()
        });
        added.map(|f| f as i32).collect()
    }
}
