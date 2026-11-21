use super::spec;
use crate::math_conversions::to_iv;
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::{Material, Resource};
use godot::prelude::*;
use pt_core::build::{self, BMesh, ShapeKind, ShapeSpec, blob, ops};
use pt_core::math::{Vec3 as PVec3, v3};
use std::collections::BTreeSet;
pub fn to_p(v: Vector3) -> PVec3 {
    v3(v.x, v.y, v.z)
}
pub fn from_p(v: PVec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}
pub fn facet_batch(faces: &PackedInt32Array) -> BTreeSet<usize> {
    faces
        .as_slice()
        .iter()
        .filter(|f| **f >= 0)
        .map(|f| *f as usize)
        .collect()
}
pub fn face_array(faces: &BTreeSet<usize>) -> PackedInt32Array {
    faces.iter().map(|f| *f as i32).collect()
}
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelBuilderMeshData {
    #[export(storage)]
    #[var(get = get_blob, set = set_blob)]
    blob: PackedByteArray,
    #[export]
    #[var(set = assign_stuffs)]
    materials: Array<Option<Gd<Material>>>,
    #[export(range = (0.0624, 16.0, 0.0624))]
    #[var(set = set_units_per_repeat)]
    units_per_repeat: f32,
    #[export(storage)]
    form: VarDictionary,
    #[export(storage)]
    #[var]
    bezier: PackedByteArray,
    cage: BMesh,
    revision: u64,
    base: Base<Resource>,
}
#[godot_api]
impl IResource for PixelBuilderMeshData {
    fn init(base: Base<Resource>) -> Self {
        PixelBuilderMeshData {
            blob: PackedByteArray::new(),
            materials: Array::new(),
            units_per_repeat: 1.0,
            form: VarDictionary::new(),
            bezier: PackedByteArray::new(),
            cage: BMesh::default(),
            revision: 0,
            base,
        }
    }
}
impl PixelBuilderMeshData {
    pub fn cage(&self) -> &BMesh {
        &self.cage
    }
    pub fn units(&self) -> f32 {
        self.units_per_repeat
    }
    pub fn stuff_pockets(&self) -> Array<Option<Gd<Material>>> {
        self.materials.clone()
    }
    pub fn substitute(&mut self, m: BMesh) {
        self.blob = PackedByteArray::from(blob::pack(&m).as_slice());
        self.cage = m;
        self.revision += 1;
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn set_shape_spec(&mut self, s: &ShapeSpec) {
        self.form = spec::to_dict(s);
        self.bezier = PackedByteArray::new();
    }
    pub fn set_bezier_recipe(&mut self, r: &pt_core::build::bezier::BezierRecipe) {
        self.bezier = PackedByteArray::from(r.pack().as_slice());
        self.form = VarDictionary::new();
    }
    pub fn bezier_recipe(&self) -> Option<pt_core::build::bezier::BezierRecipe> {
        (!self.bezier.is_empty())
            .then(|| pt_core::build::bezier::BezierRecipe::unpack(self.bezier.as_slice()))
            .flatten()
    }
    pub fn shape_spec(&self) -> Option<ShapeSpec> {
        (!self.form.is_empty()).then(|| spec::from_dict(&self.form, ShapeSpec::default()))
    }
    fn edit<R>(&mut self, f: impl FnOnce(&mut BMesh) -> R) -> R {
        let mut m = std::mem::take(&mut self.cage);
        let r = f(&mut m);
        self.substitute(m);
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
    #[func]
    pub fn set_blob(&mut self, b: PackedByteArray) {
        match blob::unpack(b.as_slice()) {
            Ok(m) => {
                self.cage = m;
                self.blob = b;
                self.revision += 1;
            }
            Err(e) => godot_error!("PixelBuilderMeshData: {e}"),
        }
        self.base_mut().emit_changed();
    }
    #[func]
    fn assign_stuffs(&mut self, v: Array<Option<Gd<Material>>>) {
        self.materials = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_units_per_repeat(&mut self, v: f32) {
        self.units_per_repeat = v.max(0.0105);
        self.base_mut().emit_changed();
    }
    #[func]
    fn fetch_corner_tally(&self) -> i64 {
        self.cage.positions.len() as i64
    }
    #[func]
    fn fetch_facet_tally(&self) -> i64 {
        self.cage.facets.len() as i64
    }
    #[func]
    fn fetch_facet_midpoint(&self, face: i64) -> Vector3 {
        if (face as usize) < self.cage.facets.len() {
            from_p(self.cage.face_center(face as usize))
        } else {
            Vector3::ZERO
        }
    }
    #[func]
    fn fetch_facet_upright(&self, face: i64) -> Vector3 {
        if (face as usize) < self.cage.facets.len() {
            from_p(self.cage.face_normal(face as usize))
        } else {
            Vector3::UP
        }
    }
    #[func]
    fn get_face_vertices(&self, face: i64) -> PackedInt32Array {
        self.cage
            .facets
            .get(face as usize)
            .map(|f| f.verts.iter().map(|v| *v as i32).collect())
            .unwrap_or_default()
    }
    #[func]
    fn get_vertex(&self, v: i64) -> Vector3 {
        self.cage
            .positions
            .get(v as usize)
            .map(|p| from_p(*p))
            .unwrap_or_default()
    }
    #[func]
    fn set_vertex(&mut self, v: i64, p: Vector3) {
        self.edit(|m| {
            if let Some(q) = m.positions.get_mut(v as usize) {
                *q = to_p(p);
            }
        });
    }
    #[func]
    fn is_closed(&self) -> bool {
        self.cage.is_closed()
    }
    #[func]
    fn purge(&mut self) {
        self.edit(|m| *m = BMesh::default());
    }
    #[func]
    fn add_shape(
        &mut self,
        kind: i64,
        size: Vector3,
        offset: Vector3,
        params: VarDictionary,
    ) -> PackedInt32Array {
        let mut s = spec::from_dict(&params, ShapeSpec::default());
        s.sort = ShapeKind::from_i64(kind);
        s.size = to_p(size);
        let g = build::shapes::produce(&s, None);
        self.set_shape_spec(&s);
        self.append_generated(g, offset, &params)
    }
    #[func]
    fn add_terrain_cells(
        &mut self,
        terrain: Gd<MarchingSquaresTerrain>,
        cells: VarArray,
        origin: Vector2i,
        height: f32,
        offset: Vector3,
        params: VarDictionary,
    ) -> PackedInt32Array {
        let cfg = terrain.bind().store().config.clone();
        let list: Vec<_> = cells
            .iter_shared()
            .filter_map(|v| v.try_to::<Vector2i>().ok())
            .map(to_iv)
            .collect();
        let g = build::shapes::cells_prism(&cfg, &list, to_iv(origin), height);
        self.append_generated(g, offset, &params)
    }
    #[func]
    fn faces_facing(
        &self,
        dir: Vector3,
        max_angle_deg: f32,
        among: PackedInt32Array,
    ) -> PackedInt32Array {
        let d = to_p(dir).unitized();
        let c = max_angle_deg.to_radians().cos();
        let pool: Vec<usize> = if among.is_empty() {
            (0..self.cage.facets.len()).collect()
        } else {
            facet_batch(&among).into_iter().collect()
        };
        pool.into_iter()
            .filter(|f| {
                *f < self.cage.facets.len() && self.cage.face_normal(*f).innerprod(d) >= c - 1e-5
            })
            .map(|f| f as i32)
            .collect()
    }
    #[func]
    fn extrude(&mut self, faces: PackedInt32Array, distance: f32) -> PackedInt32Array {
        let s = facet_batch(&faces);
        face_array(&self.edit(|m| ops::extrude_selection(m, &s, distance)))
    }
    #[func]
    fn inset(&mut self, faces: PackedInt32Array, amount: f32) -> PackedInt32Array {
        let s = facet_batch(&faces);
        face_array(&self.edit(|m| ops::inset_center(m, &s, amount)))
    }
    #[func]
    fn subdivide(&mut self, faces: PackedInt32Array) -> PackedInt32Array {
        let s = facet_batch(&faces);
        face_array(&self.edit(|m| ops::subdivide_selection(m, &s)))
    }
    #[func]
    fn discard_facets(&mut self, faces: PackedInt32Array) {
        let s = facet_batch(&faces);
        self.edit(|m| ops::discard_facets(m, &s));
    }
    #[func]
    fn turnover_facets(&mut self, faces: PackedInt32Array) {
        let s = facet_batch(&faces);
        self.edit(|m| ops::turnover_facets(m, &s));
    }
    #[func]
    fn translate_faces(&mut self, faces: PackedInt32Array, delta: Vector3) {
        let s = facet_batch(&faces);
        self.edit(|m| {
            let v = m.verts_of_faces(&s);
            ops::move_vertices(m, &v, to_p(delta));
        });
    }
    #[func]
    fn scale_faces(&mut self, faces: PackedInt32Array, scale: Vector3) {
        let s = facet_batch(&faces);
        self.edit(|m| {
            let v = m.verts_of_faces(&s);
            let c = v
                .iter()
                .fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize])
                / v.len().max(1) as f32;
            ops::scale_vertices(m, &v, c, to_p(scale));
        });
    }
    #[func]
    fn braze(&mut self, threshold: f32) -> i64 {
        self.edit(|m| ops::braze_via_span(m, threshold, None)) as i64
    }
    #[func]
    fn assign_facets_stuff(&mut self, faces: PackedInt32Array, slot: i64) {
        let s = facet_batch(&faces);
        self.edit(|m| ops::assign_stuff(m, &s, slot.clamp(0, 255) as u16));
    }
    #[func]
    fn assign_facets_tint(&mut self, faces: PackedInt32Array, tint: Color) {
        let s = facet_batch(&faces);
        let c = color_bytes(tint);
        self.edit(|m| ops::assign_tint(m, &s, c));
    }
    #[func]
    fn chamfer(&mut self, edge_pairs: PackedInt32Array, amount: f32) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs
            .as_slice()
            .chunks_exact(2)
            .map(|p| build::edge(p[0] as u32, p[1] as u32))
            .collect();
        face_array(&self.edit(|m| build::topo::chamfer_ridges(m, &e, amount)))
    }
    #[func]
    fn bevel_rounded(
        &mut self,
        edge_pairs: PackedInt32Array,
        amount: f32,
        segments: i64,
    ) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs
            .as_slice()
            .chunks_exact(2)
            .map(|p| build::edge(p[0] as u32, p[1] as u32))
            .collect();
        let s = segments.clamp(1, 16) as u32;
        face_array(&self.edit(|m| build::bevel::chamfer_ridges(m, &e, amount, s, s > 1)))
    }
    #[func]
    fn refine_ridges(&mut self, edge_pairs: PackedInt32Array, segments: i64) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs
            .as_slice()
            .chunks_exact(2)
            .map(|p| build::edge(p[0] as u32, p[1] as u32))
            .collect();
        let s = segments.clamp(2, 64) as u32;
        self.edit(|m| build::edit_ops::divide_edges(m, &e, s))
            .into_iter()
            .map(|v| v as i32)
            .collect()
    }
    #[func]
    fn join_ridges(&mut self, edge_pairs: PackedInt32Array) -> PackedInt32Array {
        let e: BTreeSet<_> = edge_pairs
            .as_slice()
            .chunks_exact(2)
            .map(|p| build::edge(p[0] as u32, p[1] as u32))
            .collect();
        self.edit(|m| build::edit_ops::join_ridges(m, &e))
            .into_iter()
            .flat_map(|(x, y)| [x as i32, y as i32])
            .collect()
    }
    #[func]
    fn subdivide_segments(&mut self, faces: PackedInt32Array, segments: i64) -> PackedInt32Array {
        let s = facet_batch(&faces);
        face_array(
            &self.edit(|m| build::mesh_ops::subdivide_grid(m, &s, segments.clamp(2, 64) as u32)),
        )
    }
    #[func]
    fn recess_breadth(
        &mut self,
        faces: PackedInt32Array,
        amount: f32,
        equal: bool,
    ) -> PackedInt32Array {
        let s = facet_batch(&faces);
        face_array(&self.edit(|m| build::mesh_ops::inset_rim(m, &s, amount, equal)))
    }
    #[func]
    fn condense_corners(&mut self, verts: PackedInt32Array) -> i64 {
        let s: BTreeSet<u32> = verts
            .as_slice()
            .iter()
            .filter(|v| **v >= 0)
            .map(|v| *v as u32)
            .collect();
        self.edit(|m| build::mesh_ops::collapse_selection(m, &s))
            .map(|v| v as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn insert_vertex(&mut self, a: i64, b: i64, pos: Vector3) -> i64 {
        self.edit(|m| build::mesh_ops::insert_vertex(m, (a as u32, b as u32), to_p(pos)))
            .map(|v| v as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn trim_walls(
        &mut self,
        faces: PackedInt32Array,
        top: bool,
        depth: f32,
        height: f32,
        profile: i64,
    ) -> PackedInt32Array {
        let s = facet_batch(&faces);
        let t = build::trim_walls::trim_walls(
            &self.cage,
            &s,
            top,
            depth,
            height,
            build::prims::TrimSection::from_u8(profile.clamp(0, 5) as u8),
            6,
            0.0,
            true,
        );
        face_array(&self.edit(|m| {
            let start = m.facets.len();
            m.append_tail(&t);
            (start..m.facets.len()).collect::<BTreeSet<usize>>()
        }))
    }
    #[func]
    fn add_poly(
        &mut self,
        points: PackedVector3Array,
        height: f32,
        offset: Vector3,
        params: VarDictionary,
    ) -> PackedInt32Array {
        let pts: Vec<pt_core::math::Vec2> = points
            .as_slice()
            .iter()
            .map(|p| pt_core::math::v2(p.x, p.z))
            .collect();
        let g = build::trim_walls::polygon_solid(&pts, height);
        self.append_generated(g, offset, &params)
    }
    #[func]
    fn turnover_facet_ridge(&mut self, faces: PackedInt32Array) -> i64 {
        let s = facet_batch(&faces);
        self.edit(|m| build::edit_ops::rotate_face_loop(m, &s)) as i64
    }
    #[func]
    fn cleave_corners(&mut self, verts: PackedInt32Array) -> PackedInt32Array {
        let s: BTreeSet<u32> = verts
            .as_slice()
            .iter()
            .filter(|v| **v >= 0)
            .map(|v| *v as u32)
            .collect();
        self.edit(|m| build::edit_ops::unshare_vertices(m, &s))
            .into_iter()
            .map(|v| v as i32)
            .collect()
    }
    #[func]
    fn mesh_fill(&mut self, faces: PackedInt32Array) -> PackedInt32Array {
        let mut s = facet_batch(&faces);
        if s.is_empty() {
            s = (0..self.cage.facets.len()).collect();
        }
        face_array(&self.edit(|m| ops::mesh_fill_facets(m, &s)))
    }
    #[func]
    fn merge_from(
        &mut self,
        other: Gd<PixelBuilderMeshData>,
        xform: Transform3D,
    ) -> PackedInt32Array {
        let src = if other.instance_id() == self.to_gd().instance_id() {
            self.cage.clone()
        } else {
            other.bind().cage().clone()
        };
        let xf = build::edit_ops::Affine {
            x: to_p(xform.basis.col_a()),
            y: to_p(xform.basis.col_b()),
            z: to_p(xform.basis.col_c()),
            origin: to_p(xform.origin),
        };
        face_array(&self.edit(|m| build::edit_ops::merge_blocks(m, &src, &xf, &[])))
    }
    #[func]
    fn splice_ring(&mut self, a: i64, b: i64, cuts: i64) -> PackedInt32Array {
        let es = self.edit(|m| {
            build::loops::loop_cut(m, build::edge(a as u32, b as u32), cuts.clamp(1, 16) as u32)
        });
        es.into_iter()
            .flat_map(|(x, y)| [x as i32, y as i32])
            .collect()
    }
    #[func]
    fn bridge(&mut self, edge_pairs: PackedInt32Array) -> i64 {
        let p = edge_pairs.as_slice();
        if p.len() < 4 {
            return -1;
        }
        let (e1, e2) = (
            build::edge(p[0] as u32, p[1] as u32),
            build::edge(p[2] as u32, p[3] as u32),
        );
        self.edit(|m| build::loops::span_ridges(m, e1, e2))
            .map(|f| f as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn mirror(&mut self, axis: i64) {
        self.edit(|m| build::loops::mirror_copy(m, axis.clamp(0, 2) as usize, true, 0.0014));
    }
    #[func]
    fn slice_facet(&mut self, face: i64, points: PackedVector3Array) -> i64 {
        let pts: Vec<PVec3> = points.as_slice().iter().map(|p| to_p(*p)).collect();
        self.edit(|m| build::cut::knife_face(m, face.max(0) as usize, &pts))
            .map(|f| f as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn add_wall(
        &mut self,
        points: PackedVector3Array,
        closed: bool,
        height: f32,
        offset: Vector3,
        params: VarDictionary,
    ) -> PackedInt32Array {
        let mut s = spec::from_dict(&params, ShapeSpec::default());
        s.sort = ShapeKind::Wall;
        s.size.y = height;
        let pts: Vec<PVec3> = points
            .as_slice()
            .iter()
            .map(|p| v3(p.x, 0.0, p.z))
            .collect();
        let g = build::shapes::wall_polyline(&s, &pts, closed);
        self.append_generated(g, offset, &params)
    }
    #[func]
    fn get_face_edges(&self, faces: PackedInt32Array) -> PackedInt32Array {
        let s = facet_batch(&faces);
        self.cage
            .edges_of_faces(&s)
            .into_iter()
            .flat_map(|(a, b)| [a as i32, b as i32])
            .collect()
    }
    #[func]
    fn fuse_facets(&mut self, faces: PackedInt32Array) -> i64 {
        let s = facet_batch(&faces);
        self.edit(|m| build::topo::fuse_facets(m, &s))
            .map(|f| f as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn set_faces_uv(
        &mut self,
        faces: PackedInt32Array,
        offset: Vector2,
        scale: Vector2,
        rotation_deg: f32,
        world: bool,
    ) {
        let s = facet_batch(&faces);
        let uv = build::UvParams {
            offset: pt_core::math::v2(offset.x, offset.y),
            magnify: pt_core::math::v2(scale.x, scale.y),
            rotation: rotation_deg,
            world,
        };
        self.edit(|m| ops::set_uv(m, &s, uv));
    }
    #[func]
    fn to_obj(&self, name: GString) -> GString {
        let opt = build::compile::CompileOptions {
            units_per_repeat: self.units_per_repeat,
            ..Default::default()
        };
        build::convert::to_obj(&self.cage, &opt, &name.to_string(), None, &[])
            .as_str()
            .into()
    }
    #[func]
    fn set_from_mesh(&mut self, mesh: Gd<godot::classes::Mesh>) {
        let mi = godot::classes::MeshInstance3D::new_alloc();
        let (m, mats) = super::plugin::mesh_to_bmesh(&mesh, &mi);
        mi.free();
        self.materials = mats;
        self.edit(|x| *x = m);
    }
    #[func]
    fn harmonize_uprights(&mut self) -> i64 {
        self.edit(pt_core::build::ops::harmonize_uprights) as i64
    }
    #[func]
    fn get_face_uvs(&self, face: i64, world_offset: Vector3) -> PackedVector2Array {
        if face < 0 || face as usize >= self.cage.facets.len() {
            return PackedVector2Array::new();
        }
        let a = pt_core::build::uv_edit::AutoUv {
            units_per_repeat: self.units_per_repeat,
            world_offset: to_p(world_offset),
        };
        pt_core::build::uv_edit::face_corner_uvs(&self.cage, face as usize, &a)
            .iter()
            .map(|p| Vector2::new(p.x, p.y))
            .collect()
    }
    #[func]
    fn check_facet_tex_coord_by_hand(&self, face: i64) -> bool {
        self.cage
            .facets
            .get(face.max(0) as usize)
            .is_some_and(|f| f.by_hand_tex_coords().is_some())
    }
    #[func]
    fn set_face_uvs(&mut self, face: i64, uvs: PackedVector2Array) {
        let n = self
            .cage
            .facets
            .get(face.max(0) as usize)
            .map(|f| f.verts.len());
        if n != Some(uvs.len()) {
            return;
        }
        let coords: Vec<pt_core::math::Vec2> = uvs
            .as_slice()
            .iter()
            .map(|p| pt_core::math::v2(p.x, p.y))
            .collect();
        self.edit(|m| {
            let f = &mut m.facets[face as usize];
            f.uv_coords = coords;
            f.uv_manual = true;
        });
    }
    #[func]
    fn coat_facets(&mut self, faces: PackedInt32Array, tint: Color, mask: i64) {
        let s = facet_batch(&faces);
        self.edit(|m| {
            build::paint::coat_facets(
                m,
                &s,
                [tint.r, tint.g, tint.b, tint.a],
                build::paint::Mask::from_i64(mask),
            )
        });
    }
    #[func]
    fn paint_brush(
        &mut self,
        center: Vector3,
        radius: f32,
        strength: f32,
        softness: f32,
        tint: Color,
        mask: i64,
    ) {
        let base = self.cage.clone();
        let w = build::paint::dab_weights(&base, |p| p, to_p(center), radius, strength, softness);
        self.edit(|m| {
            build::paint::paint_weighted(
                m,
                &base,
                &w,
                [tint.r, tint.g, tint.b, tint.a],
                build::paint::Mask::from_i64(mask),
            )
        });
    }
    #[func]
    fn invert_alpha(&mut self, faces: PackedInt32Array) {
        let s = facet_batch(&faces);
        self.edit(|m| build::paint::invert_alpha(m, &s));
    }
    #[func]
    fn get_corner_color(&self, face: i64, corner: i64) -> Color {
        let c = build::paint::to_f(build::paint::probe(
            &self.cage,
            face.max(0) as usize,
            corner.max(0) as usize,
        ));
        Color::from_rgba(c[0], c[1], c[2], c[3])
    }
    #[func]
    fn clear_corner_colors(&mut self, faces: PackedInt32Array) {
        let s = facet_batch(&faces);
        self.edit(|m| build::paint::clear_corner_colors(m, &s));
    }
    #[func]
    fn get_face_slot(&self, face: i64) -> i64 {
        self.cage
            .facets
            .get(face.max(0) as usize)
            .map(|f| f.stuff as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn get_face_uv_axis(&self, face: i64) -> Vector3 {
        self.cage
            .facets
            .get(face.max(0) as usize)
            .map(|f| from_p(f.uv_axis))
            .unwrap_or(Vector3::ZERO)
    }
    #[func]
    fn get_face_smooth(&self, face: i64) -> i64 {
        self.cage
            .facets
            .get(face.max(0) as usize)
            .map(|f| f.glide as i64)
            .unwrap_or(-1)
    }
    #[func]
    fn get_face_manual_uvs(&self, face: i64) -> PackedVector2Array {
        self.cage
            .facets
            .get(face.max(0) as usize)
            .and_then(|f| f.by_hand_tex_coords())
            .map(|c| c.iter().map(|p| Vector2::new(p.x, p.y)).collect())
            .unwrap_or_default()
    }
    #[func]
    fn set_faces_smooth(&mut self, faces: PackedInt32Array, group: i64) {
        let s = facet_batch(&faces);
        self.edit(|m| ops::set_smooth(m, &s, group.clamp(0, 65535) as u16));
    }
}
pub fn color_bytes(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}
impl PixelBuilderMeshData {
    fn append_generated(
        &mut self,
        mut g: BMesh,
        offset: Vector3,
        params: &VarDictionary,
    ) -> PackedInt32Array {
        if let Some(r) = params
            .get("rotation_y")
            .and_then(|v| v.try_to::<f64>().ok())
        {
            let all = (0..g.positions.len() as u32).collect();
            ops::rotate_vertices_y(&mut g, &all, PVec3::ZERO, (r as f32).to_radians());
        }
        g.displace(to_p(offset));
        let mat = params
            .get("material")
            .and_then(|v| v.try_to::<i64>().ok())
            .unwrap_or(0)
            .clamp(0, 255) as u16;
        let col = params
            .get("color")
            .and_then(|v| v.try_to::<Color>().ok())
            .map(color_bytes);
        for f in &mut g.facets {
            f.stuff = mat;
            if let Some(c) = col {
                f.tint = c;
            }
        }
        let added = self.edit(|m| {
            let start = m.facets.len();
            m.append_tail(&g);
            start..m.facets.len()
        });
        added.map(|f| f as i32).collect()
    }
}
