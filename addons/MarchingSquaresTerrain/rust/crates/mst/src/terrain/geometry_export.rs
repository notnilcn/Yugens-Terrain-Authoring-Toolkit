//! Turns the live terrain into standalone geometry that no longer needs the terrain to exist.
//!
//! Authors use this to hand the terrain to a renderer, an engine or a tool that knows
//! nothing about it: either as plain meshes kept in their own scene, or as a model file
//! written to disk. The terrain's own rendering is untouched by either.
//!
//! The nodes are created in code because how many there are, and what they hold, depends
//! on what was painted. Baking into the scene is editor tooling: the generated
//! nodes are handed to the author's scene to keep, owned by it, and reversible through the
//! editor's undo history. Writing a model file builds a temporary tree only so the model
//! writer has something to read, and frees it as soon as the file is written or the author
//! abandons the export. From the editor the export is spread over frames and can be
//! cancelled part way; the final write cannot be spread out, so the author is warned that
//! the editor will stop responding before it starts.
use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::mesh_arrays::build_array_mesh;
use crate::object_flags::object_flags_for;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{
    ArrayMesh, CollisionShape3D, EditorInterface, MeshInstance3D, MultiMeshInstance3D, Node3D,
    StaticBody3D,
};
use godot::prelude::*;
use pt_core::atlas::AtlasPage;
use pt_core::math::IVec2;
use pt_core::mesh::MeshArrays;
use pt_core::square::BlendMode;
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn export_glb(&mut self, path: GString) -> godot::global::Error {
        let work = ExportWork::VertexColour(VertexColourJob::new(
            self.vertex_colour_look(),
            self.store.chunk_coords(),
        ));
        write_glb(self.run_export(work), &path)
    }
    #[func]
    pub fn bake_static_meshes(&mut self) -> Option<Gd<Node3D>> {
        self.update();
        self.batches_flush_collision(true);
        let mut parent = self.bake_destination()?;
        let ts = self.tex_assign.clone()?;
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("{}Baked", self.base().get_name()));
        root.set_transform(self.base().get_transform());
        let ts = ts.bind();
        let tex = self.texture_array.clone();
        let mat = self
            .land_stuff
            .as_ref()
            .and_then(|m| Some(m.duplicate_resource()));
        if let Some(mut m) = mat.clone() {
            if let Some(t) = &tex {
                m.set_shader_parameter("textures", &t.to_variant());
            }
            m.set_shader_parameter("tints", &ts.tints().to_variant());
            m.set_shader_parameter("scales", &ts.scales().to_variant());
            m.set_shader_parameter("blend_mode", &self.blend_mode.to_variant());
            m.set_shader_parameter(
                "pt_flags",
                &object_flags_for(
                    self.base().instance_id(),
                    0,
                    self.pixel_size,
                    self.outline,
                    self.outline_id,
                    false,
                )
                .to_variant(),
            );
        }
        let gmat = self
            .turf_stuff
            .as_ref()
            .and_then(|m| Some(m.duplicate_resource()));
        if let Some(mut m) = gmat.clone() {
            if let Some(t) = &tex {
                m.set_shader_parameter("ground_textures", &t.to_variant());
            }
            m.set_shader_parameter("ground_tints", &ts.tints().to_variant());
            m.set_shader_parameter("ground_scales", &ts.scales().to_variant());
            m.set_shader_parameter("grass_tints", &ts.grass_tints().to_variant());
            if let Some(a) = &self.grass_sprites {
                m.set_shader_parameter("grass_sprites", &a.to_variant());
            }
            if m.get_shader_parameter("wind_texture").is_nil()
                && let Some(w) = &self.wind_texture
            {
                m.set_shader_parameter("wind_texture", &w.to_variant());
                m.set_shader_parameter("use_wind_texture", &true.to_variant());
            }
            m.set_shader_parameter(
                "pt_flags",
                &object_flags_for(
                    self.base().instance_id(),
                    1,
                    self.grass_pixel_size,
                    self.grass_outline,
                    self.outline_id,
                    false,
                )
                .to_variant(),
            );
        }
        drop(ts);
        let layers = self.render_layers | globals::meta_mask();
        let mut coords: Vec<IVec2> = self.nodes.keys().copied().collect();
        coords.sort();
        let mut all: Vec<Gd<Node>> = Vec::new();
        for c in coords {
            let n = &self.nodes[&c];
            let mesh = if self.columns_active() {
                pt_core::mesh_chunk(
                    &self.store,
                    c,
                    pt_core::square::BlendMode(self.blend_mode.clamp(0, 2) as u8),
                )
                .and_then(|soup| build_array_mesh(&soup.assemble()))
                .map(|m| m.upcast())
            } else {
                self.chunk_mesh(c)
            };
            let Some(mesh) = mesh else { continue };
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&format!("Chunk_{}_{}", c.x, c.y));
            mi.set_mesh(&mesh);
            mi.set_layer_mask(layers);
            mi.set_cast_shadows_setting(if self.cast_shadows {
                ShadowCastingSetting::ON
            } else {
                ShadowCastingSetting::OFF
            });
            if let Some(m) = &mat {
                mi.set_material_override(m);
            }
            root.add_child(&mi);
            all.push(mi.clone().upcast());
            if self.impact_active
                && let Some(shape) = n.form.clone()
            {
                let mut body = StaticBody3D::new_alloc();
                body.set_name("Body");
                body.set_collision_layer(self.impact_stratum);
                body.set_collision_mask(self.impact_sieve);
                let mut cs = CollisionShape3D::new_alloc();
                cs.set_name("Shape");
                cs.set_shape(&shape);
                body.add_child(&cs);
                mi.add_child(&body);
                all.push(body.upcast());
                all.push(cs.upcast());
            }
            if self.grass_enabled
                && let Some(mm) = self.chunk_grass_multimesh(c)
            {
                let mut g = MultiMeshInstance3D::new_alloc();
                g.set_name("Grass");
                g.set_multimesh(&mm);
                g.set_layer_mask(layers);
                g.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                if let Some(m) = &gmat {
                    g.set_material_override(m);
                }
                mi.add_child(&g);
                all.push(g.upcast());
            }
        }
        let beside_terrain = parent.is_ancestor_of(&self.to_gd()) && !self.bake_parent_assigned();
        let idx = if beside_terrain {
            self.base().get_index() + 1
        } else {
            parent.get_child_count()
        };
        let owner = self.scene_root();
        if is_editor()
            && let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo()
        {
            let p = parent.clone().upcast::<Object>();
            let r = root.clone().upcast::<Object>();
            let set_owner = "set_owner";
            ur.create_action("Bake terrain static meshes");
            ur.add_do_method(
                &p,
                "add_child",
                &[root.to_variant(), true.to_variant()],
            );
            ur.add_do_method(
                &p,
                "move_child",
                &[root.to_variant(), idx.to_variant()],
            );
            if let Some(o) = &owner {
                ur.add_do_method(&r, set_owner, &[o.to_variant()]);
                for n in &all {
                    ur.add_do_method(&n.clone().upcast::<Object>(), set_owner, &[o.to_variant()]);
                }
            }
            ur.add_do_reference(&r);
            ur.add_undo_method(&p, "remove_child", &[root.to_variant()]);
            ur.run_deferred_gd(|mut ur| ur.commit_action());
        } else {
            parent.add_child(&root);
            parent.move_child(&root, idx);
            if let Some(o) = &owner {
                root.set_owner(o);
                for mut n in all {
                    n.set_owner(o);
                }
            }
        }
        Some(root)
    }
    pub(super) fn tool_bake_static(&mut self) {
        let _ = self.bake_static_meshes();
    }
    fn bake_parent_assigned(&self) -> bool {
        self.bake_parent.as_ref().is_some_and(|p| p.is_instance_valid())
    }
    fn bake_destination(&self) -> Option<Gd<Node>> {
        if let Some(p) = self.bake_parent.clone().filter(|p| p.is_instance_valid()) {
            return Some(p);
        }
        let parent = self.base().get_parent()?;
        godot_warn!(
            "MarchingSquaresTerrain: no Bake Parent assigned; the baked meshes are placed beside the terrain."
        );
        Some(parent)
    }
    #[func]
    pub fn export_glb_baked(
        &mut self,
        path: GString,
        polygon_resolution: i32,
    ) -> godot::global::Error {
        let work = match self.export_look(polygon_resolution) {
            Ok(l) => ExportWork::Atlas(AtlasJob::new(l, self.store.chunk_coords())),
            Err(e) => return e,
        };
        write_glb(self.run_export(work), &path)
    }
    #[func]
    pub fn begin_export_glb_baked(
        &mut self,
        path: GString,
        polygon_resolution: i32,
    ) -> godot::global::Error {
        if self.export_job.is_some() {
            return godot::global::Error::ERR_BUSY;
        }
        match self.export_look(polygon_resolution) {
            Ok(l) => {
                let job = AtlasJob::new(l, self.store.chunk_coords());
                self.begin_export(path, ExportWork::Atlas(job))
            }
            Err(e) => e,
        }
    }
    #[func]
    pub fn get_export_atlas_plan(&self, polygon_resolution: i32) -> VarDictionary {
        let res = atlas_polygon_resolution(polygon_resolution);
        let max = self.atlas_max_size();
        let blend = BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let triangles: usize = self
            .store
            .chunk_coords()
            .into_iter()
            .filter_map(|c| pt_core::mesh_chunk(&self.store, c, blend))
            .map(|s| s.verts.len() / 3)
            .sum();
        let pages = pt_core::atlas::plan(triangles, res, max);
        let mut d = VarDictionary::new();
        d.set("triangles", triangles as i64);
        d.set("textures", pages.len() as i64);
        let sizes: Array<Vector2i> = pages
            .iter()
            .map(|p| Vector2i::new(p.width as i32, p.height as i32))
            .collect();
        d.set("sizes", &sizes);
        d.set("bytes", pages.iter().map(|p| p.bytes() as i64).sum::<i64>());
        d
    }
    fn atlas_max_size(&self) -> u32 {
        self.export_atlas_max_size.clamp(256, 16384) as u32
    }
    fn run_export(&self, mut work: ExportWork) -> Gd<Node3D> {
        while !work.step(self) {}
        let mut root = export_root();
        work.finish(&mut root);
        root
    }
    fn begin_export(&mut self, path: GString, work: ExportWork) -> godot::global::Error {
        if self.export_job.is_some() {
            return godot::global::Error::ERR_BUSY;
        }
        let (done, total) = work.progress();
        self.export_job = Some(ExportJob {
            path,
            work,
            shown_complete: false,
        });
        self.announce_export_progress(done, total);
        godot::global::Error::OK
    }
    #[func]
    pub fn get_export_progress(&self) -> f64 {
        self.export_job.as_ref().map_or(-1.0, |j| {
            let (done, total) = j.work.progress();
            if total == 0 {
                1.0
            } else {
                done as f64 / total as f64
            }
        })
    }
    #[func]
    pub fn cancel_export(&mut self) {
        if let Some(job) = self.export_job.take() {
            drop(job.work);
            self.announce_export_finished(job.path, godot::global::Error::ERR_SKIP);
        }
    }
    fn vertex_colour_look(&self) -> VertexColourLook {
        let mut material = godot::classes::StandardMaterial3D::new_gd();
        material.set_flag(
            godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR,
            true,
        );
        VertexColourLook {
            tints: self
                .tex_assign
                .as_ref()
                .map(|t| t.bind().tints().as_slice().to_vec())
                .unwrap_or_else(|| vec![Color::WHITE; 16]),
            blend: BlendMode(self.blend_mode.clamp(0, 2) as u8),
            material,
        }
    }
    fn vertex_colour_chunk(&self, look: &VertexColourLook, c: IVec2) -> Option<Gd<MeshInstance3D>> {
        use godot::classes::mesh::{ArrayType, PrimitiveType};
        let soup = pt_core::mesh_chunk(&self.store, c, look.blend)?;
        let arrays = soup.assemble();
        let mut mesh = build_array_mesh(&arrays)?;
        let colors: PackedColorArray = arrays
            .custom0
            .iter()
            .zip(arrays.custom1.iter())
            .map(|(m, w)| {
                let wc = (1.0 - w[0] - w[1]).max(0.0);
                let pick = |i: f32| look.tints.get(i as usize).copied().unwrap_or(Color::WHITE);
                let mut col = pick(m[0]) * w[0] + pick(m[1]) * w[1] + pick(m[2]) * wc;
                if w[2] > 0.5 {
                    col *= 0.8505;
                }
                col.a = 1.0;
                col
            })
            .collect();
        let mut surf = mesh.surface_get_arrays(0);
        surf.set(ArrayType::COLOR.ord() as usize, &colors.to_variant());
        surf.set(ArrayType::CUSTOM0.ord() as usize, &Variant::nil());
        surf.set(ArrayType::CUSTOM1.ord() as usize, &Variant::nil());
        mesh.clear_surfaces();
        mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &surf);
        mesh.surface_set_material(0, &look.material);
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name(&format!("Chunk_{}_{}", c.x, c.y));
        mi.set_mesh(&mesh);
        Some(mi)
    }
    fn export_look(&self, polygon_resolution: i32) -> Result<ExportLook, godot::global::Error> {
        let Some(ts) = self.tex_assign.clone() else {
            return Err(godot::global::Error::ERR_UNCONFIGURED);
        };
        let ts = ts.bind();
        let tex_size = ts.texture_size.clamp(8, 256);
        let layers: Vec<Vec<u8>> = (0..16)
            .map(|i| {
                ts.layer_image(i, tex_size)
                    .map(|img| img.get_data().to_vec())
                    .unwrap_or_default()
            })
            .collect();
        let tints: Vec<Color> = ts.tints().as_slice().to_vec();
        let scales: Vec<f32> = ts.scales().as_slice().to_vec();
        drop(ts);
        let shader = |name: &str, fallback: f32| {
            self.land_stuff
                .as_ref()
                .map(|m| m.get_shader_parameter(name))
                .and_then(|v| v.try_to::<f32>().ok())
                .unwrap_or(fallback)
        };
        Ok(ExportLook {
            layers,
            tints,
            scales,
            tex_size: tex_size as usize,
            world_size: shader("texture_world_size", 8.0).max(0.0105),
            sharp: shader("blend_sharpness", 3.0),
            wall_shade: shader("wall_shade", 0.8805),
            blend: pt_core::square::BlendMode(self.blend_mode.clamp(0, 2) as u8),
            hard: self.blend_mode != 0,
            res: atlas_polygon_resolution(polygon_resolution),
            atlas_max: self.atlas_max_size(),
        })
    }
    pub(super) fn step_export(&mut self, budget: std::time::Duration) {
        let Some(mut job) = self.export_job.take() else {
            return;
        };
        let began = std::time::Instant::now();
        let mut finished = false;
        while !finished && began.elapsed() < budget {
            finished = job.work.step(self);
        }
        if !finished || !job.shown_complete {
            job.shown_complete = finished;
            let (done, total) = job.work.progress();
            self.announce_export_progress(done, total);
            self.export_job = Some(job);
            return;
        }
        let mut root = export_root();
        job.work.finish(&mut root);
        drop(job.work);
        let err = write_glb(root, &job.path);
        self.announce_export_finished(job.path, err);
    }
    fn announce_export_progress(&mut self, done: usize, total: usize) {
        self.base_mut().call_deferred(
            "emit_signal",
            &[
                "export_progressed".to_variant(),
                (done as i64).to_variant(),
                (total as i64).to_variant(),
            ],
        );
    }
    fn announce_export_finished(&mut self, path: GString, err: godot::global::Error) {
        self.base_mut().call_deferred(
            "emit_signal",
            &[
                "export_finished".to_variant(),
                path.to_variant(),
                (err.ord() as i64).to_variant(),
            ],
        );
    }
}
pub(super) struct ExportJob {
    path: GString,
    work: ExportWork,
    shown_complete: bool,
}
enum ExportWork {
    VertexColour(VertexColourJob),
    Atlas(AtlasJob),
}
impl ExportWork {
    fn step(&mut self, terrain: &MarchingSquaresTerrain) -> bool {
        match self {
            ExportWork::VertexColour(j) => j.step(terrain),
            ExportWork::Atlas(j) => j.step(terrain),
        }
    }
    fn progress(&self) -> (usize, usize) {
        match self {
            ExportWork::VertexColour(j) => (j.next, j.coords.len()),
            ExportWork::Atlas(j) => j.progress(),
        }
    }
    fn finish(&mut self, root: &mut Gd<Node3D>) {
        match self {
            ExportWork::VertexColour(j) => {
                for mi in std::mem::take(&mut j.built) {
                    adopt(root, mi);
                }
            }
            ExportWork::Atlas(j) => j.finish(root),
        }
    }
}
struct VertexColourJob {
    look: VertexColourLook,
    coords: Vec<IVec2>,
    next: usize,
    built: Vec<Gd<MeshInstance3D>>,
}
impl VertexColourJob {
    fn new(look: VertexColourLook, coords: Vec<IVec2>) -> Self {
        Self {
            look,
            coords,
            next: 0,
            built: Vec::new(),
        }
    }
    fn step(&mut self, terrain: &MarchingSquaresTerrain) -> bool {
        let Some(&c) = self.coords.get(self.next) else {
            return true;
        };
        if let Some(mi) = terrain.vertex_colour_chunk(&self.look, c) {
            self.built.push(mi);
        }
        self.next += 1;
        false
    }
}
impl Drop for VertexColourJob {
    fn drop(&mut self) {
        for mi in self.built.drain(..) {
            mi.free();
        }
    }
}
struct AtlasJob {
    look: ExportLook,
    coords: Vec<IVec2>,
    next_mesh: usize,
    meshed: Vec<MeshArrays>,
    bake: Option<AtlasBake>,
}
struct AtlasBake {
    pages: Vec<AtlasPage>,
    surfaces: Vec<PageSurface>,
    page_capacity: usize,
    next_triangle: usize,
    next_chunk: usize,
}
#[derive(Default)]
struct PageSurface {
    verts: PackedVector3Array,
    norms: PackedVector3Array,
    uvs: PackedVector2Array,
}
impl AtlasJob {
    fn new(look: ExportLook, coords: Vec<IVec2>) -> Self {
        Self {
            look,
            coords,
            next_mesh: 0,
            meshed: Vec::new(),
            bake: None,
        }
    }
    fn step(&mut self, terrain: &MarchingSquaresTerrain) -> bool {
        if let Some(&c) = self.coords.get(self.next_mesh) {
            if let Some(soup) = pt_core::mesh_chunk(&terrain.store, c, self.look.blend) {
                let a = soup.assemble();
                if a.ordinals.len() >= 3 {
                    self.meshed.push(a);
                }
            }
            self.next_mesh += 1;
            return false;
        }
        let Some(bake) = self.bake.as_mut() else {
            self.bake = Some(self.plan_bake());
            return false;
        };
        let Some(a) = self.meshed.get_mut(bake.next_chunk) else {
            return true;
        };
        bake.shade_chunk(&self.look, a);
        *a = MeshArrays::default();
        bake.next_chunk += 1;
        false
    }
    fn plan_bake(&self) -> AtlasBake {
        let triangles: usize = self.meshed.iter().map(|a| a.ordinals.len() / 3).sum();
        let layouts = pt_core::atlas::plan(triangles, self.look.res, self.look.atlas_max);
        AtlasBake {
            surfaces: layouts.iter().map(|_| PageSurface::default()).collect(),
            pages: layouts.into_iter().map(AtlasPage::new).collect(),
            page_capacity: pt_core::atlas::page_capacity(self.look.res, self.look.atlas_max),
            next_triangle: 0,
            next_chunk: 0,
        }
    }
    fn progress(&self) -> (usize, usize) {
        let n = self.coords.len();
        let shaded = match &self.bake {
            None => 0,
            Some(b) if b.next_chunk >= self.meshed.len() => n,
            Some(b) => b.next_chunk * n / self.meshed.len(),
        };
        (
            (self.next_mesh * MESHING_SHARE + shaded * (100 - MESHING_SHARE)) / 100,
            n,
        )
    }
    fn finish(&mut self, root: &mut Gd<Node3D>) {
        use godot::classes::base_material_3d::{TextureFilter, TextureParam, Transparency};
        use godot::classes::image::Format;
        use godot::classes::mesh::{ArrayType, PrimitiveType};
        use godot::classes::{Image, ImageTexture, StandardMaterial3D};
        let Some(bake) = self.bake.take() else {
            return;
        };
        let single = bake.pages.len() == 1;
        for (i, (page, surface)) in bake.pages.into_iter().zip(bake.surfaces).enumerate() {
            if surface.verts.is_empty() {
                continue;
            }
            let l = page.layout;
            let data = PackedByteArray::from(page.rgba);
            let Some(img) =
                Image::create_from_data(l.width as i32, l.height as i32, false, Format::RGBA8, &data)
            else {
                continue;
            };
            drop(data);
            let mut arrays = VarArray::new();
            arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
            arrays.set(ArrayType::VERTEX.ord() as usize, &surface.verts.to_variant());
            arrays.set(ArrayType::NORMAL.ord() as usize, &surface.norms.to_variant());
            arrays.set(ArrayType::TEX_UV.ord() as usize, &surface.uvs.to_variant());
            let mut mesh = ArrayMesh::new_gd();
            mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            let mut mat = StandardMaterial3D::new_gd();
            if let Some(t) = ImageTexture::create_from_image(&img) {
                mat.set_texture(TextureParam::ALBEDO, &t);
            }
            mat.set_texture_filter(TextureFilter::NEAREST);
            mat.set_transparency(Transparency::ALPHA_SCISSOR);
            mesh.surface_set_material(0, &mat);
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&if single {
                "Terrain".to_string()
            } else {
                format!("Terrain_{i}")
            });
            mi.set_mesh(&mesh);
            adopt(root, mi);
        }
    }
}
const MESHING_SHARE: usize = 10;
impl AtlasBake {
    fn shade_chunk(&mut self, look: &ExportLook, a: &MeshArrays) {
        let vid = |t: usize, k: usize| a.ordinals[t * 3 + k] as usize;
        for t in 0..a.ordinals.len() / 3 {
            let page = self.next_triangle / self.page_capacity;
            let slot = self.next_triangle % self.page_capacity;
            self.next_triangle += 1;
            let uvs = self.pages[page].bake_triangle(slot, |w| look.shade(a, t, w));
            let s = &mut self.surfaces[page];
            for (k, uv) in uvs.iter().enumerate() {
                let i = vid(t, k);
                let [px, py, pz] = a.positions[i];
                let [nx, ny, nz] = a.normals[i];
                s.verts.push(Vector3::new(px, py, pz));
                s.norms.push(Vector3::new(nx, ny, nz));
                s.uvs.push(Vector2::new(uv.x, uv.y));
            }
        }
    }
}
struct VertexColourLook {
    tints: Vec<Color>,
    blend: BlendMode,
    material: Gd<godot::classes::StandardMaterial3D>,
}
struct ExportLook {
    layers: Vec<Vec<u8>>,
    tints: Vec<Color>,
    scales: Vec<f32>,
    tex_size: usize,
    world_size: f32,
    sharp: f32,
    wall_shade: f32,
    blend: pt_core::square::BlendMode,
    hard: bool,
    res: u32,
    atlas_max: u32,
}
impl ExportLook {
    fn shade(&self, a: &MeshArrays, t: usize, w: [f32; 3]) -> [f32; 4] {
        let vid = |k: usize| a.ordinals[t * 3 + k] as usize;
        let lerp = |f: &dyn Fn(usize) -> [f32; 4]| -> [f32; 4] {
            let (p0, p1, p2) = (f(vid(0)), f(vid(1)), f(vid(2)));
            std::array::from_fn(|k| p0[k] * w[0] + p1[k] * w[1] + p2[k] * w[2])
        };
        let p = lerp(&|i| [a.positions[i][0], a.positions[i][1], a.positions[i][2], 0.0]);
        let n = lerp(&|i| [a.normals[i][0], a.normals[i][1], a.normals[i][2], 0.0]);
        let wt = lerp(&|i| a.custom1[i]);
        let uv = lerp(&|i| [a.tex_coord[i][0], a.tex_coord[i][1], 0.0, 0.0]);
        let m = a.custom0[vid(0)];
        let mats = [
            m[0].round() as usize,
            m[1].round() as usize,
            m[2].round() as usize,
        ];
        let mut bw = [
            wt[0].max(0.0),
            wt[1].max(0.0),
            (1.0 - wt[0] - wt[1]).max(0.0),
        ];
        if self.hard {
            let k = if bw[0] >= bw[1] && bw[0] >= bw[2] {
                0
            } else if bw[1] >= bw[2] {
                1
            } else {
                2
            };
            bw = [0.0; 3];
            bw[k] = 1.0;
        } else {
            bw = bw.map(|x| x.powf(1.0 + self.sharp));
        }
        let sum = (bw[0] + bw[1] + bw[2]).max(1e-4);
        let bw = bw.map(|x| x / sum);
        let void_w: f32 = (0..3).filter(|k| mats[*k] == 15).map(|k| bw[k]).sum();
        if void_w > 0.5 {
            return [0.0; 4];
        }
        let pos = [p[0], p[1], p[2]];
        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        let nn = [n[0] / nl, n[1] / nl, n[2] / nl];
        let mut col = [0.0f32; 4];
        let wall = wt[2] > 0.5;
        for k in 0..3 {
            let s = if wall {
                self.triplanar(mats[k], pos, nn)
            } else {
                self.sample(mats[k], pos[0] / self.world_size, pos[2] / self.world_size)
            };
            for j in 0..4 {
                col[j] += s[j] * bw[k];
            }
        }
        let rl = m[3].round() as usize;
        if wall || uv[0] >= 0.6005 || uv[1] >= 0.6005 {
            if !wall {
                col = self.triplanar(rl, pos, [nn[0], 0.0, nn[2]]);
            }
            for channel in &mut col[..3] {
                *channel *= self.wall_shade;
            }
        }
        col[3] = 1.0;
        col
    }
    fn sample(&self, slot: usize, u: f32, v: f32) -> [f32; 4] {
        if slot >= 15 {
            return [0.0; 4];
        }
        let ts_i = self.tex_size;
        let sc = self.scales.get(slot).copied().unwrap_or(1.0);
        let Some(px) = self.layers.get(slot).filter(|l| l.len() >= ts_i * ts_i * 4) else {
            return [1.0, 0.0, 1.0, 1.0];
        };
        let x = ((u * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
        let y = ((v * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
        let i = (y * ts_i + x) * 4;
        let t = self.tints.get(slot).copied().unwrap_or(Color::WHITE);
        [
            px[i] as f32 / 255.0 * t.r,
            px[i + 1] as f32 / 255.0 * t.g,
            px[i + 2] as f32 / 255.0 * t.b,
            1.0,
        ]
    }
    fn triplanar(&self, slot: usize, p: [f32; 3], n: [f32; 3]) -> [f32; 4] {
        let (wx, wz) = (n[0].abs() + 0.0015, n[2].abs() + 0.0015);
        let s = wx + wz;
        let a = self.sample(slot, p[2] / self.world_size, p[1] / self.world_size);
        let b = self.sample(slot, p[0] / self.world_size, p[1] / self.world_size);
        std::array::from_fn(|k| a[k] * wx / s + b[k] * wz / s)
    }
}
fn atlas_polygon_resolution(requested: i32) -> u32 {
    requested.clamp(4, 128) as u32
}
pub(super) fn export_budget(frame_seconds: f64) -> std::time::Duration {
    std::time::Duration::from_secs_f64((frame_seconds * 0.5).clamp(0.04, 0.25))
}
fn export_root() -> Gd<Node3D> {
    let mut root = Node3D::new_alloc();
    root.set_name("MarchingSquaresTerrainExport");
    root
}
fn adopt(root: &mut Gd<Node3D>, mut mi: Gd<MeshInstance3D>) {
    root.add_child(&mi);
    mi.set_owner(&*root);
}
fn write_glb(root: Gd<Node3D>, path: &GString) -> godot::global::Error {
    use godot::classes::{GltfDocument, GltfState};
    let mut doc = GltfDocument::new_gd();
    let state = GltfState::new_gd();
    let mut err = doc.append_from_scene(&root, &state);
    if err == godot::global::Error::OK {
        err = doc.write_to_filesystem(&state, path);
    }
    root.free();
    err
}
impl MarchingSquaresTerrain {
    pub(super) fn tool_export_glb(&mut self) {
        let path = self.export_path.clone();
        let work = ExportWork::VertexColour(VertexColourJob::new(
            self.vertex_colour_look(),
            self.store.chunk_coords(),
        ));
        let err = self.begin_export(path.clone(), work);
        if err == godot::global::Error::ERR_BUSY {
            godot_warn!("MarchingSquaresTerrain: an export is already running; wait for it or cancel it");
        } else if err != godot::global::Error::OK {
            godot_error!("MarchingSquaresTerrain: export to {path} failed: {err:?}");
        }
    }
}
