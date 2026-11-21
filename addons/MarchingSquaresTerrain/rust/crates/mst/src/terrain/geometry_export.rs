//! Turns the live terrain into standalone geometry that no longer needs the terrain to exist.
//!
//! Authors use this to hand the terrain to a renderer, an engine or a tool that knows
//! nothing about it: either as plain meshes kept in their own scene, or as a model file
//! written to disk. The terrain's own rendering is untouched by either.
//!
//! The nodes are created in code because there is one per chunk, and which chunks exist
//! depends on what was painted. Baking into the scene is editor tooling: the generated
//! nodes are handed to the author's scene to keep, owned by it, and reversible through the
//! editor's undo history. Writing a model file builds a temporary tree only so the model
//! writer has something to read, and frees it as soon as the file is written.
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
use pt_core::math::IVec2;
use pt_core::square::BlendMode;
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn export_glb(&mut self, path: GString) -> godot::global::Error {
        use godot::classes::mesh::ArrayType;
        use godot::classes::{GltfDocument, GltfState, StandardMaterial3D};
        let tints: Vec<Color> = self
            .tex_assign
            .as_ref()
            .map(|t| t.bind().tints().as_slice().to_vec())
            .unwrap_or_else(|| vec![Color::WHITE; 16]);
        let blend = BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let mut root = Node3D::new_alloc();
        root.set_name("MarchingSquaresTerrainExport");
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_flag(
            godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR,
            true,
        );
        for c in self.store.chunk_coords() {
            let Some(soup) = pt_core::mesh_chunk(&self.store, c, blend) else {
                continue;
            };
            let arrays = soup.assemble();
            let Some(mut mesh) = build_array_mesh(&arrays) else {
                continue;
            };
            let colors: PackedColorArray = arrays
                .custom0
                .iter()
                .zip(arrays.custom1.iter())
                .map(|(m, w)| {
                    let wc = (1.0 - w[0] - w[1]).max(0.0);
                    let pick = |i: f32| tints.get(i as usize).copied().unwrap_or(Color::WHITE);
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
            mesh.add_surface_from_arrays(godot::classes::mesh::PrimitiveType::TRIANGLES, &surf);
            mesh.surface_set_material(0, &mat);
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&format!("Chunk ({}, {})", c.x, c.y));
            mi.set_mesh(&mesh);
            root.add_child(&mi);
            mi.set_owner(&root);
        }
        let mut doc = GltfDocument::new_gd();
        let state = GltfState::new_gd();
        let mut err = doc.append_from_scene(&root, &state);
        if err == godot::global::Error::OK {
            err = doc.write_to_filesystem(&state, &path);
        }
        root.free();
        err
    }
    #[func]
    pub fn bake_static_meshes(&mut self) -> Option<Gd<Node3D>> {
        self.update();
        self.batches_flush_collision(true);
        let mut parent = self.base().get_parent()?;
        let ts = self.tex_assign.clone()?;
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("{} (Baked)", self.base().get_name()));
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
            mi.set_name(&format!("Chunk ({}, {})", c.x, c.y));
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
        let idx = self.base().get_index() + 1;
        let owner = self.scene_root();
        if is_editor()
            && let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo()
        {
            let p = parent.clone().upcast::<Object>();
            let r = root.clone().upcast::<Object>();
            ur.create_action("Bake terrain static meshes");
            ur.add_do_method(&p, "add_child", &[root.to_variant(), true.to_variant()]);
            ur.add_do_method(&p, "move_child", &[root.to_variant(), idx.to_variant()]);
            if let Some(o) = &owner {
                ur.add_do_method(&r, "set_owner", &[o.to_variant()]);
                for n in &all {
                    ur.add_do_method(
                        &n.clone().upcast::<Object>(),
                        "set_owner",
                        &[o.to_variant()],
                    );
                }
            }
            ur.add_do_reference(&r);
            ur.add_undo_method(&p, "remove_child", &[root.to_variant()]);
            ur.call_deferred("commit_action", &[]);
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
    #[func]
    pub fn export_glb_baked(
        &mut self,
        path: GString,
        polygon_resolution: i32,
    ) -> godot::global::Error {
        use godot::classes::base_material_3d::{TextureFilter, Transparency};
        use godot::classes::image::Format;
        use godot::classes::mesh::{ArrayType, PrimitiveType};
        use godot::classes::{GltfDocument, GltfState, Image, ImageTexture, StandardMaterial3D};
        let Some(ts) = self.tex_assign.clone() else {
            return godot::global::Error::ERR_UNCONFIGURED;
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
        let world_size = self
            .land_stuff
            .as_ref()
            .map(|m| m.get_shader_parameter("texture_world_size"))
            .and_then(|v| v.try_to::<f32>().ok())
            .unwrap_or(8.0)
            .max(0.0105);
        let sharp = self
            .land_stuff
            .as_ref()
            .map(|m| m.get_shader_parameter("blend_sharpness"))
            .and_then(|v| v.try_to::<f32>().ok())
            .unwrap_or(3.0);
        let wall_shade = self
            .land_stuff
            .as_ref()
            .map(|m| m.get_shader_parameter("wall_shade"))
            .and_then(|v| v.try_to::<f32>().ok())
            .unwrap_or(0.8805);
        let ts_i = tex_size as usize;
        let sample = |slot: usize, u: f32, v: f32| -> [f32; 4] {
            if slot >= 15 {
                return [0.0; 4];
            }
            let sc = scales.get(slot).copied().unwrap_or(1.0);
            let Some(px) = layers.get(slot).filter(|l| l.len() >= ts_i * ts_i * 4) else {
                return [1.0, 0.0, 1.0, 1.0];
            };
            let x = ((u * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
            let y = ((v * sc).rem_euclid(1.0) * ts_i as f32) as usize % ts_i;
            let i = (y * ts_i + x) * 4;
            let t = tints.get(slot).copied().unwrap_or(Color::WHITE);
            [
                px[i] as f32 / 255.0 * t.r,
                px[i + 1] as f32 / 255.0 * t.g,
                px[i + 2] as f32 / 255.0 * t.b,
                1.0,
            ]
        };
        let triplanar = |slot: usize, p: [f32; 3], n: [f32; 3]| -> [f32; 4] {
            let (wx, wz) = (n[0].abs() + 0.0015, n[2].abs() + 0.0015);
            let s = wx + wz;
            let a = sample(slot, p[2] / world_size, p[1] / world_size);
            let b = sample(slot, p[0] / world_size, p[1] / world_size);
            std::array::from_fn(|k| a[k] * wx / s + b[k] * wz / s)
        };
        let blend = pt_core::square::BlendMode(self.blend_mode.clamp(0, 2) as u8);
        let hard = self.blend_mode != 0;
        let res = polygon_resolution.clamp(4, 128) as u32;
        let mut root = Node3D::new_alloc();
        root.set_name("MarchingSquaresTerrainExport");
        for c in self.store.chunk_coords() {
            let Some(soup) = pt_core::mesh_chunk(&self.store, c, blend) else {
                continue;
            };
            let a = soup.assemble();
            let tris = a.ordinals.len() / 3;
            if tris == 0 {
                continue;
            }
            let vid = |t: usize, k: usize| a.ordinals[t * 3 + k] as usize;
            let lerp = |t: usize, w: [f32; 3], f: &dyn Fn(usize) -> [f32; 4]| -> [f32; 4] {
                let (p0, p1, p2) = (f(vid(t, 0)), f(vid(t, 1)), f(vid(t, 2)));
                std::array::from_fn(|k| p0[k] * w[0] + p1[k] * w[1] + p2[k] * w[2])
            };
            let atlas = pt_core::atlas::bake(tris, res, 8192, |t, w| {
                let p = lerp(t, w, &|i| {
                    [a.positions[i][0], a.positions[i][1], a.positions[i][2], 0.0]
                });
                let n = lerp(t, w, &|i| {
                    [a.normals[i][0], a.normals[i][1], a.normals[i][2], 0.0]
                });
                let wt = lerp(t, w, &|i| a.custom1[i]);
                let uv = lerp(t, w, &|i| [a.tex_coord[i][0], a.tex_coord[i][1], 0.0, 0.0]);
                let m = a.custom0[vid(t, 0)];
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
                if hard {
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
                    bw = bw.map(|x| x.powf(1.0 + sharp));
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
                        triplanar(mats[k], pos, nn)
                    } else {
                        sample(mats[k], pos[0] / world_size, pos[2] / world_size)
                    };
                    for j in 0..4 {
                        col[j] += s[j] * bw[k];
                    }
                }
                let rl = m[3].round() as usize;
                if wall || (!wall && (uv[0] >= 0.6005 || uv[1] >= 0.6005)) {
                    if !wall {
                        col = triplanar(rl, pos, [nn[0], 0.0, nn[2]]);
                    }
                    for j in 0..3 {
                        col[j] *= wall_shade;
                    }
                }
                col[3] = 1.0;
                col
            });
            let Some(atlas) = atlas else {
                godot_error!(
                    "MarchingSquaresTerrain.export_glb_baked: chunk {c:?} does not fit an 8192 px atlas at {res} px; lower polygon_resolution"
                );
                continue;
            };
            let Some(img) = Image::create_from_data(
                atlas.size as i32,
                atlas.size as i32,
                false,
                Format::RGBA8,
                &PackedByteArray::from(atlas.rgba.as_slice()),
            ) else {
                continue;
            };
            let mut verts = PackedVector3Array::new();
            let mut norms = PackedVector3Array::new();
            let mut uvs = PackedVector2Array::new();
            for t in 0..tris {
                for k in 0..3 {
                    let i = vid(t, k);
                    verts.push(Vector3::new(
                        a.positions[i][0],
                        a.positions[i][1],
                        a.positions[i][2],
                    ));
                    norms.push(Vector3::new(
                        a.normals[i][0],
                        a.normals[i][1],
                        a.normals[i][2],
                    ));
                    let u = atlas.uvs[t * 3 + k];
                    uvs.push(Vector2::new(u.x, u.y));
                }
            }
            let mut arrays = VarArray::new();
            arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
            arrays.set(ArrayType::VERTEX.ord() as usize, &verts.to_variant());
            arrays.set(ArrayType::NORMAL.ord() as usize, &norms.to_variant());
            arrays.set(ArrayType::TEX_UV.ord() as usize, &uvs.to_variant());
            let mut mesh = ArrayMesh::new_gd();
            mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            let mut mat = StandardMaterial3D::new_gd();
            if let Some(t) = ImageTexture::create_from_image(&img) {
                mat.set_texture(godot::classes::base_material_3d::TextureParam::ALBEDO, &t);
            }
            mat.set_texture_filter(TextureFilter::NEAREST);
            mat.set_transparency(Transparency::ALPHA_SCISSOR);
            mesh.surface_set_material(0, &mat);
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&format!("Chunk ({}, {})", c.x, c.y));
            mi.set_mesh(&mesh);
            root.add_child(&mi);
            mi.set_owner(&root);
        }
        let mut doc = GltfDocument::new_gd();
        let state = GltfState::new_gd();
        let mut err = doc.append_from_scene(&root, &state);
        if err == godot::global::Error::OK {
            err = doc.write_to_filesystem(&state, &path);
        }
        root.free();
        err
    }
}
impl MarchingSquaresTerrain {
    pub(super) fn tool_export_glb(&mut self) {
        let path = self.export_path.clone();
        let err = self.export_glb(path.clone());
        if err == godot::global::Error::OK {
            godot_print!("MarchingSquaresTerrain: exported {path}");
        } else {
            godot_error!("MarchingSquaresTerrain: export to {path} failed: {err:?}");
        }
    }
}
