use super::MarchingSquaresTerrain;
use crate::mesh_arrays::build_array_mesh;
use godot::classes::image::Format;
use godot::classes::{ArrayMesh, Image, ImageTexture, RenderingServer, Shader, ShaderMaterial};
use godot::prelude::*;
use pt_core::grid::GridType;
use pt_core::hexmap::{self, MapWindow};
use pt_core::math::IVec2;
use pt_core::prism;
use std::hash::{Hash, Hasher};

const TEMPLATE_LOW: u32 = u32::MAX;
const TEMPLATE_TOP: u32 = u32::MAX - 1;
const TEMPLATE_LOW_TOP: u32 = u32::MAX - 2;
pub fn template_name(key: u32) -> String {
    match key {
        TEMPLATE_LOW => "Columns low".into(),
        TEMPLATE_TOP => "Tops".into(),
        TEMPLATE_LOW_TOP => "Tops low".into(),
        g => format!("Columns {g}"),
    }
}
pub const INCLUDE: &str =
    "res://addons/MarchingSquaresTerrain/shaders/mst_prism_instanced.gdshaderinc";
#[derive(Default)]
pub struct ColumnState {
    shader: Option<Gd<Shader>>,
    made_for: Option<(i64, u64)>,
    switched: Option<Gd<ShaderMaterial>>,
    templates: std::collections::HashMap<u32, Gd<ArrayMesh>>,
    chart: Option<Gd<ImageTexture>>,
    image: Option<Gd<Image>>,
    window: MapWindow,
    pub need_map: bool,
    pub dirty_chunks: std::collections::HashSet<IVec2>,
    texels: Vec<[f32; 4]>,
    pub ready: std::collections::HashMap<IVec2, Vec<(IVec2, [[f32; 4]; 6])>>,
}
pub fn chunk_columns(
    store: &pt_core::terrain_store::TerrainStore,
    c: IVec2,
    low: bool,
    groove_cap: u32,
) -> super::batches::ChunkColumns {
    let (mut sides, mut tops) = (Vec::new(), Vec::new());
    for cell in prism::chunk_instances(store, c) {
        match prism::Column::from_store(store, cell, low) {
            Some(col) if prism::has_exposed_side(&col) => sides.push(cell),
            Some(_) => tops.push(cell),
            None => {}
        }
    }
    let grooves = if low {
        0
    } else {
        prism::groove_bucket(prism::grooves_needed(store, c), groove_cap)
    };
    super::batches::ChunkColumns {
        sides,
        tops,
        side_key: if low { TEMPLATE_LOW } else { grooves },
        top_key: if low { TEMPLATE_LOW_TOP } else { TEMPLATE_TOP },
        rings: grooves,
        low,
        extent: prism::chunk_bounds(store, c),
    }
}
fn code_hash(code: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    h.finish()
}
fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
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
pub fn patch_shader_code(code: &str) -> String {
    let body = replace_word(
        &replace_word(code, "CUSTOM0", "mst_c0"),
        "CUSTOM1",
        "mst_c1",
    );
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
            return format!(
                "{}{}{}\n{}{}",
                &body[..at],
                include,
                &body[at..open],
                PRELUDE,
                &body[open..]
            );
        }
        search = at + 1;
    }
    format!("{body}\n{include}void vertex() {{\n{PRELUDE}}}\n")
}
impl MarchingSquaresTerrain {
    pub(super) fn columns_active(&self) -> bool {
        self.hex_edge_instanced
            && self.hex_edge_style == crate::terrain::grid_choice::HexEdgeStyle::HexPrism
            && matches!(self.grid(), GridType::Hex | GridType::HexRings)
    }
    pub(super) fn columns_material_stale(&self) -> bool {
        if !self.columns_active() {
            return self.columns.switched.is_some();
        }
        let Some(m) = &self.land_stuff else {
            return false;
        };
        let Some(sh) = m.get_shader() else {
            return false;
        };
        self.columns.made_for
            != Some((
                m.instance_id().to_i64(),
                code_hash(&sh.get_code().to_string()),
            ))
    }
    pub(super) fn columns_sync_material(&mut self) {
        let mut rs = RenderingServer::singleton();
        let active = self.columns_active();
        let current = self.land_stuff.clone();
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
        let Some(m) = &self.land_stuff else { return };
        let rid = m.get_rid();
        let mut rs = RenderingServer::singleton();
        let p = prism::PrismParams::from_store(&self.store);
        rs.material_set_param(rid, "mst_prism_width", &p.breadth.to_variant());
        rs.material_set_param(rid, "mst_prism_level", &p.level.to_variant());
        rs.material_set_param(rid, "mst_prism_bevel", &p.chamfer.to_variant());
        rs.material_set_param(
            rid,
            "mst_prism_offset",
            &(self.grid() == GridType::Hex).to_variant(),
        );
        let w = self.columns.window;
        rs.material_set_param(
            rid,
            "mst_prism_origin",
            &Vector2i::new(w.origin.x, w.origin.y).to_variant(),
        );
        if let Some(t) = &self.columns.chart {
            rs.material_set_param(rid, "mst_prism_map", &t.get_rid().to_variant());
        }
    }
    pub(super) fn columns_flush_map(&mut self) {
        let mut ready = std::mem::take(&mut self.columns.ready);
        if !self.columns_active() {
            self.columns.dirty_chunks.clear();
            return;
        }
        if !self.columns.need_map && self.columns.dirty_chunks.is_empty() {
            return;
        }
        let w = hexmap::chunks_window(&self.store, 0);
        if w.check_vacant() {
            return;
        }
        let partial = !self.columns.need_map
            && w == self.columns.window
            && self.columns.texels.len() == w.len() * prism::COLUMN_TEXELS as usize;
        self.columns.need_map = false;
        let dirty: Vec<IVec2> = self.columns.dirty_chunks.drain().collect();
        if partial {
            let cfg = self.store.config.clone();
            let mut cells: Vec<IVec2> = Vec::new();
            for c in &dirty {
                match ready.remove(c) {
                    Some(texels) => {
                        for (cell, t) in texels {
                            if let Some(i) = w.ordinal(cell) {
                                self.columns.texels[i * 6..i * 6 + 6].copy_from_slice(&t);
                            }
                        }
                    }
                    None => cells.extend(cfg.chunk_cells(*c)),
                }
            }
            if !cells.is_empty() {
                prism::update_column_map(&self.store, w, &mut self.columns.texels, cells, 2);
            }
        } else {
            self.columns.texels = prism::build_column_map(&self.store, w);
        }
        let wide = MapWindow::new(
            w.origin,
            pt_core::math::iv2(w.size.x * prism::COLUMN_TEXELS, w.size.y),
        );
        let Some(img) = rgbaf_image(wide, &self.columns.texels) else {
            return;
        };
        match &mut self.columns.chart {
            Some(t) if self.columns.window.size == w.size => t.update(&img),
            _ => self.columns.chart = ImageTexture::create_from_image(&img),
        }
        self.columns.window = w;
        self.columns.image = Some(img);
        self.columns_push_params();
    }
    pub(super) fn columns_template(&mut self, grooves: u32) -> Option<Gd<ArrayMesh>> {
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
    pub(super) fn columns_apply_built(
        &mut self,
        built: Vec<(
            IVec2,
            super::batches::ChunkColumns,
            Vec<(IVec2, [[f32; 4]; 6])>,
        )>,
    ) {
        if !self.columns_active() {
            self.columns.ready.clear();
            let had: Vec<IVec2> = self
                .nodes
                .iter_mut()
                .filter_map(|(c, n)| n.columns.take().map(|_| *c))
                .collect();
            for c in had {
                self.batches.tag(c);
            }
            return;
        }
        let mut cols = Vec::with_capacity(built.len());
        for (c, col, texels) in built {
            self.columns.ready.insert(c, texels);
            cols.push((c, col));
        }
        {
            let _p = crate::prof::breadth("columns.map");
            self.columns_flush_map();
        }
        for (c, col) in cols {
            if let Some(n) = self.nodes.get_mut(&c) {
                n.columns = Some(col);
                self.batches.tag(c);
            }
        }
    }
    pub(super) fn columns_instance_count(&self) -> i64 {
        self.nodes
            .values()
            .filter_map(|n| n.columns.as_ref())
            .map(|c| (c.sides.len() + c.tops.len()) as i64)
            .sum()
    }
    pub(super) fn columns_map_image(&self) -> Option<Gd<Image>> {
        self.columns.image.clone()
    }
    pub(super) fn columns_map_texture(&self) -> Option<Gd<ImageTexture>> {
        self.columns.chart.clone()
    }
    pub(super) fn columns_map_window(&self) -> MapWindow {
        self.columns.window
    }
}
pub fn rgbaf_image(w: MapWindow, texels: &[[f32; 4]]) -> Option<Gd<Image>> {
    let mut bytes = Vec::with_capacity(texels.len() * 16);
    for t in texels {
        for v in t {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    Image::create_from_data(
        w.size.x,
        w.size.y,
        false,
        Format::RGBAF,
        &PackedByteArray::from(bytes.as_slice()),
    )
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
        assert!(
            p.contains("vec4 mst_c0 = CUSTOM0;") && p.contains("mst_prism_vertex(INSTANCE_CUSTOM")
        );
        let v = p.find("void vertex").unwrap();
        assert!(
            p.find("mst_prism_vertex(").unwrap() > v
                && p.find("mats = mst_c0").unwrap() > p.find("mst_prism_vertex(").unwrap()
        );
        assert!(
            patch_shader_code("void vertex() { float MY_CUSTOM0X = 1.0; }").contains("MY_CUSTOM0X")
        );
        let q = patch_shader_code("shader_type spatial;\nvoid fragment() {}\n");
        assert!(q.contains("void vertex() {") && q.contains("mst_prism_vertex("));
    }
}
