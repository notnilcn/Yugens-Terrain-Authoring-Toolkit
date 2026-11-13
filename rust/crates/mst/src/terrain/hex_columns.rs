//! GPU-instanced HexPrism columns (`hex_edge_instanced`, child module of `node.rs`).
//!
//! Each chunk gets one `MultiMeshInstance3D` whose instances are its cells (identity transforms,
//! `INSTANCE_CUSTOM = (q, r, low, rings)`), all drawing a column template of `pt_core::prism`
//! with as many groove rings as the chunk's tallest exposed side needs (bucketed).
//! The terrain material is switched (on the RenderingServer only, never in the saved
//! resource) to a runtime variant of its own shader with `mst_prism_instanced.gdshaderinc`
//! called at the top of `vertex()`: template vertices are placed from the column map texture
//! (`pt_core::prism::build_column_map`, 6 texels per cell), every other mesh using
//! the material is untouched. Because the same material RID is used, every parameter the node
//! and the overlays push (`textures`, `pt_flags`, `mst_*`) reaches the columns too.
//!
//! The chunk's collision and grass come from the same bevelled columns placed on the CPU
//! (`pt_core::prism::chunk_soup`, no grooves) in the normal flush. Bake / Export GLB mesh the
//! HexPrism surface on the fly.

use std::hash::{Hash, Hasher};

use godot::classes::image::Format;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{ArrayMesh, Image, ImageTexture, MultiMesh, MultiMeshInstance3D, RenderingServer, Shader, ShaderMaterial};
use godot::prelude::*;

use pt_core::grid::GridType;
use pt_core::hexmap::{self, MapWindow};
use pt_core::math::IVec2;
use pt_core::prism;

use super::MarchingSquaresTerrain;
use crate::util::*;

/// Template cache keys besides the groove counts.
const TEMPLATE_LOW: u32 = u32::MAX;
const TEMPLATE_TOP: u32 = u32::MAX - 1;
const TEMPLATE_LOW_TOP: u32 = u32::MAX - 2;

pub const INCLUDE: &str = "res://addons/MarchingSquaresTerrain/shaders/mst_prism_instanced.gdshaderinc";

#[derive(Default)]
pub struct ColumnState {
    /// Runtime variant of the terrain material's shader (prism include in `vertex()`).
    shader: Option<Gd<Shader>>,
    /// (material instance id, code hash) the variant was made for.
    made_for: Option<(i64, u64)>,
    /// Material currently switched to the variant (restored when instancing stops).
    switched: Option<Gd<ShaderMaterial>>,
    /// Column templates by groove ring count (0 = also the low-detail template).
    templates: std::collections::HashMap<u32, Gd<ArrayMesh>>,
    map: Option<Gd<ImageTexture>>,
    /// CPU copy of the uploaded map (the texture may not read back on every renderer).
    image: Option<Gd<Image>>,
    window: MapWindow,
    /// Full rebuild of the column map pending (layout, material or window changes).
    pub need_map: bool,
    /// Chunks edited since the last upload: only their area is recomputed (`update_column_map`).
    pub dirty_chunks: std::collections::HashSet<IVec2>,
    /// CPU texels of the uploaded map (`build_column_map` layout).
    texels: Vec<[f32; 4]>,
}

fn code_hash(code: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    h.finish()
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Replaces whole-word `from` by `to`.
fn replace_word(code: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(i) = rest.find(from) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + from.len()..].chars().next();
        out.push_str(&rest[..i]);
        if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
            out.push_str(from);
        } else {
            out.push_str(to);
        }
        rest = &rest[i + from.len()..];
    }
    out.push_str(rest);
    out
}

const PRELUDE: &str = "\tvec4 mst_c0 = CUSTOM0;\n\tvec4 mst_c1 = CUSTOM1;\n\tif (CUSTOM1.z < -0.5) {\n\t\tMstPrismV mst_pv = mst_prism_vertex(INSTANCE_CUSTOM, CUSTOM0, CUSTOM1, UV2);\n\t\tVERTEX = mst_pv.pos;\n\t\tNORMAL = mst_pv.nrm;\n\t\tUV = mst_pv.uv;\n\t\tmst_c0 = mst_pv.c0;\n\t\tmst_c1 = mst_pv.c1;\n\t}\n";

/// The terrain shader with instanced prism support: the include goes in front of `vertex()`,
/// the column placement at its top, and the body reads `CUSTOM0/1` through locals that the
/// placement overrides for template vertices. Shaders without `vertex()` get one.
pub fn patch_shader_code(code: &str) -> String {
    let body = replace_word(&replace_word(code, "CUSTOM0", "mst_c0"), "CUSTOM1", "mst_c1");
    let include = format!("#include \"{INCLUDE}\"\n");
    let mut search = 0;
    while let Some(off) = body[search..].find("void vertex") {
        let at = search + off;
        let tail = &body[at + "void vertex".len()..];
        let trimmed = tail.trim_start();
        if trimmed.starts_with('(')
            && let Some(brace) = tail.find('{')
        {
            let open = at + "void vertex".len() + brace + 1;
            return format!("{}{}{}\n{}{}", &body[..at], include, &body[at..open], PRELUDE, &body[open..]);
        }
        search = at + 1;
    }
    format!("{body}\n{include}void vertex() {{\n{PRELUDE}}}\n")
}

impl MarchingSquaresTerrain {
    /// Instanced columns are drawn: HexPrism style, `hex_edge_instanced`, a hex grid.
    pub(super) fn columns_active(&self) -> bool {
        self.hex_edge_instanced && self.hex_edge_style == 3 && matches!(self.grid(), GridType::Hex | GridType::HexRings)
    }

    /// Editor poll: the terrain material or its shader code changed since the variant was made.
    pub(super) fn columns_material_stale(&self) -> bool {
        if !self.columns_active() {
            return self.columns.switched.is_some();
        }
        let Some(m) = &self.terrain_material else { return false };
        let Some(sh) = m.get_shader() else { return false };
        self.columns.made_for != Some((m.instance_id().to_i64(), code_hash(&sh.get_code().to_string())))
    }

    /// Switches the terrain material to the instanced variant (or back) and pushes its uniforms.
    pub(super) fn columns_sync_material(&mut self) {
        let mut rs = RenderingServer::singleton();
        let active = self.columns_active();
        let current = self.terrain_material.clone();
        // Restore a material we no longer drive (instancing off, or the material was replaced).
        if let Some(old) = self.columns.switched.clone()
            && (!active || current.as_ref() != Some(&old))
        {
            if old.is_instance_valid()
                && let Some(sh) = old.get_shader()
            {
                rs.material_set_shader(old.get_rid(), sh.get_rid());
            }
            self.columns.switched = None;
        }
        if !active {
            return;
        }
        let Some(m) = current else { return };
        let Some(sh) = m.get_shader() else { return };
        let code = sh.get_code().to_string();
        let key = (m.instance_id().to_i64(), code_hash(&code));
        if self.columns.made_for != Some(key) || self.columns.shader.is_none() {
            let mut variant = Shader::new_gd();
            variant.set_code(&patch_shader_code(&code));
            self.columns.shader = Some(variant);
            self.columns.made_for = Some(key);
        }
        if let Some(v) = &self.columns.shader {
            rs.material_set_shader(m.get_rid(), v.get_rid());
        }
        self.columns.switched = Some(m);
        self.columns.templates.clear();
        self.columns.need_map = true;
        self.columns_push_params();
    }

    fn columns_push_params(&mut self) {
        let Some(m) = &self.terrain_material else { return };
        let rid = m.get_rid();
        let mut rs = RenderingServer::singleton();
        let p = prism::PrismParams::from_store(&self.store);
        rs.material_set_param(rid, "mst_prism_width", &p.width.to_variant());
        rs.material_set_param(rid, "mst_prism_level", &p.level.to_variant());
        rs.material_set_param(rid, "mst_prism_bevel", &p.bevel.to_variant());
        rs.material_set_param(rid, "mst_prism_offset", &(self.grid() == GridType::Hex).to_variant());
        let w = self.columns.window;
        rs.material_set_param(rid, "mst_prism_origin", &Vector2i::new(w.origin.x, w.origin.y).to_variant());
        if let Some(t) = &self.columns.map {
            rs.material_set_param(rid, "mst_prism_map", &t.get_rid().to_variant());
        }
    }

    /// Re-uploads the column map the columns read (after edits), if instancing is on:
    /// `pt_core::prism::build_column_map`, 6 texels per cell (shape precomputed per column).
    pub(super) fn columns_flush_map(&mut self) {
        if !self.columns_active() {
            self.columns.dirty_chunks.clear();
            return;
        }
        if !self.columns.need_map && self.columns.dirty_chunks.is_empty() {
            return;
        }
        let w = hexmap::chunks_window(&self.store, 0);
        if w.is_empty() {
            return;
        }
        let partial = !self.columns.need_map && w == self.columns.window && self.columns.texels.len() == w.len() * prism::COLUMN_TEXELS as usize;
        self.columns.need_map = false;
        let dirty: Vec<IVec2> = self.columns.dirty_chunks.drain().collect();
        if partial {
            // Edits: recompute only the touched chunks (+ 2 cells for the neighbour-dependent shapes).
            let cfg = self.store.config.clone();
            let cells: Vec<IVec2> = dirty.iter().flat_map(|c| cfg.chunk_cells(*c)).collect();
            prism::update_column_map(&self.store, w, &mut self.columns.texels, cells, 2);
        } else {
            self.columns.texels = prism::build_column_map(&self.store, w);
        }
        let wide = MapWindow::new(w.origin, pt_core::math::iv2(w.size.x * prism::COLUMN_TEXELS, w.size.y));
        let Some(img) = rgbaf_image(wide, &self.columns.texels) else { return };
        match &mut self.columns.map {
            Some(t) if self.columns.window.size == w.size => t.update(&img),
            _ => self.columns.map = ImageTexture::create_from_image(&img),
        }
        self.columns.window = w;
        self.columns.image = Some(img);
        self.columns_push_params();
    }

    /// The column template with `grooves` rings per side (cached).
    fn columns_template(&mut self, grooves: u32) -> Option<Gd<ArrayMesh>> {
        if let Some(m) = self.columns.templates.get(&grooves) {
            return Some(m.clone());
        }
        let t = match grooves {
            TEMPLATE_LOW => prism::template_low(),
            TEMPLATE_TOP => prism::template_top(false),
            TEMPLATE_LOW_TOP => prism::template_top(true),
            g => prism::template(g),
        };
        let m = build_array_mesh(&t.arrays())?;
        self.columns.templates.insert(grooves, m.clone());
        Some(m)
    }

    /// (Re)builds the column instances of rebuilt chunks, or removes them when instancing is off.
    pub(super) fn columns_apply_chunks(&mut self, chunks: &[IVec2]) {
        let active = self.columns_active();
        if !active {
            for n in self.nodes.values_mut() {
                if let Some(c) = n.columns.take() {
                    free_node(Some(c));
                }
            }
            return;
        }
        self.columns_flush_map();
        let layers = self.render_layers | crate::globals::meta_mask();
        let shadows = if self.cast_shadows {
            godot::classes::geometry_instance_3d::ShadowCastingSetting::ON
        } else {
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF
        };
        let mat = self.terrain_material.clone();
        for c in chunks {
            if !self.nodes.contains_key(c) {
                continue;
            }
            let low = self.lod_low.contains(c);
            // Columns with an exposed side draw the full template; interior columns only their
            // top (their walls, bevels and grooves would all collapse).
            let (mut sides, mut tops) = (Vec::new(), Vec::new());
            for cell in prism::chunk_instances(&self.store, *c) {
                match prism::Column::from_store(&self.store, cell, low) {
                    Some(col) if prism::has_exposed_side(&col) => sides.push(cell),
                    Some(_) => tops.push(cell),
                    None => {}
                }
            }
            // Rings for the tallest exposed side of the chunk (bucketed, capped by
            // `hex_edge_grooves`), so tall cliffs keep every groove.
            let grooves = if low { 0 } else { prism::groove_bucket(prism::grooves_needed(&self.store, *c), self.hex_edge_grooves.max(0) as u32) };
            let Some(template) = self.columns_template(if low { TEMPLATE_LOW } else { grooves }) else { continue };
            let Some(top_template) = self.columns_template(if low { TEMPLATE_LOW_TOP } else { TEMPLATE_TOP }) else { continue };
            let bounds = prism::chunk_bounds(&self.store, *c);
            let Some(n) = self.nodes.get_mut(c) else { continue };
            let mut mmi = match n.columns.clone() {
                Some(m) if m.is_instance_valid() => m,
                _ => {
                    let mut m = MultiMeshInstance3D::new_alloc();
                    m.set_name("Columns");
                    n.root.add_child(&m);
                    n.columns = Some(m.clone());
                    m
                }
            };
            let mut top_mmi = match mmi.get_node_or_null("Tops").and_then(|t| t.try_cast::<MultiMeshInstance3D>().ok()) {
                Some(t) => t,
                None => {
                    let mut t = MultiMeshInstance3D::new_alloc();
                    t.set_name("Tops");
                    mmi.add_child(&t);
                    t
                }
            };
            let lowf = if low { 1.0 } else { 0.0 };
            for (node, mesh, cells, rings) in [(&mut mmi, &template, &sides, grooves), (&mut top_mmi, &top_template, &tops, 0)] {
                let mut mm = MultiMesh::new_gd();
                mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                mm.set_use_custom_data(true);
                mm.set_mesh(mesh);
                mm.set_instance_count(cells.len() as i32);
                for (i, cell) in cells.iter().enumerate() {
                    mm.set_instance_transform(i as i32, Transform3D::IDENTITY);
                    mm.set_instance_custom_data(i as i32, Color::from_rgba(cell.x as f32, cell.y as f32, lowf, rings as f32));
                }
                node.set_multimesh(&mm);
                // Identity instance transforms: the shader places the columns, so cull by the
                // chunk's real column bounds.
                if let Some((lo, hi)) = bounds {
                    node.set_custom_aabb(Aabb::new(from_v3(lo), from_v3(hi - lo)));
                }
                node.set_layer_mask(layers);
                node.set_cast_shadows_setting(shadows);
                if let Some(m) = &mat {
                    node.set_material_override(m);
                }
            }
        }
    }

    /// Number of column instances (all chunks), for tests and diagnostics.
    pub(super) fn columns_instance_count(&self) -> i64 {
        self.nodes
            .values()
            .filter_map(|n| n.columns.as_ref())
            .flat_map(|m| {
                let tops = m.get_node_or_null("Tops").and_then(|t| t.try_cast::<MultiMeshInstance3D>().ok());
                [Some(m.clone()), tops]
            })
            .flatten()
            .filter_map(|m| m.get_multimesh())
            .map(|mm| mm.get_instance_count() as i64)
            .sum()
    }

    pub(super) fn columns_map_image(&self) -> Option<Gd<Image>> {
        self.columns.image.clone()
    }

    pub(super) fn columns_map_texture(&self) -> Option<Gd<ImageTexture>> {
        self.columns.map.clone()
    }

    pub(super) fn columns_map_window(&self) -> MapWindow {
        self.columns.window
    }
}

/// RGBA float image of a window's texels.
pub fn rgbaf_image(w: MapWindow, texels: &[[f32; 4]]) -> Option<Gd<Image>> {
    let mut bytes = Vec::with_capacity(texels.len() * 16);
    for t in texels {
        for v in t {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    Image::create_from_data(w.size.x, w.size.y, false, Format::RGBAF, &PackedByteArray::from(bytes.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_inserts_include_and_prelude_once() {
        let code = "shader_type spatial;\nvarying vec4 mats;\nvoid vertex() {\n\tmats = CUSTOM0;\n\tvec4 w = CUSTOM1;\n}\nvoid fragment() {}\n";
        let p = patch_shader_code(code);
        assert_eq!(p.matches(INCLUDE).count(), 1);
        assert!(p.find(INCLUDE).unwrap() < p.find("void vertex").unwrap());
        assert!(p.contains("mats = mst_c0;") && p.contains("vec4 w = mst_c1;"));
        assert!(p.contains("vec4 mst_c0 = CUSTOM0;") && p.contains("mst_prism_vertex(INSTANCE_CUSTOM"));
        // The prelude sits inside vertex(), before the original body.
        let v = p.find("void vertex").unwrap();
        assert!(p.find("mst_prism_vertex(").unwrap() > v && p.find("mats = mst_c0").unwrap() > p.find("mst_prism_vertex(").unwrap());
        // Identifiers that merely contain the word are left alone.
        assert!(patch_shader_code("void vertex() { float MY_CUSTOM0X = 1.0; }").contains("MY_CUSTOM0X"));
        // No vertex(): one is appended.
        let q = patch_shader_code("shader_type spatial;\nvoid fragment() {}\n");
        assert!(q.contains("void vertex() {") && q.contains("mst_prism_vertex("));
    }
}
