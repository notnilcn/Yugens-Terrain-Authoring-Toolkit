use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::math_conversions::{from_iv, to_iv};
use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, RenderingServer};
use godot::prelude::*;
use pt_core::autotex::AutoRules;
use pt_core::cell_shader_data::{self, CellData, PaintedTints};
use pt_core::math::{IVec2, v2};
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub struct OverlayState {
    pub tiles: CellData,
    texture: Option<Gd<ImageTexture>>,
    chunk_sig: u64,
    param_sig: u64,
    auto_sig: u64,
    auto_grass: bool,
    cursor: Option<(Vector4, Color, bool)>,
    published: bool,
    used: bool,
    pub painted: PaintedTints,
}
fn hash_of(v: &impl std::fmt::Debug) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{v:?}").hash(&mut h);
    h.finish()
}
fn color_bytes(c: Color) -> [u8; 4] {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [b(c.r), b(c.g), b(c.b), b(c.a)]
}
impl MarchingSquaresTerrain {
    pub(super) fn auto_rules_core(&self) -> Option<AutoRules> {
        let ts = self.tex_assign.as_ref()?;
        let r = ts.bind().auto_rules.clone()?;
        Some(r.bind().to_core())
    }
    fn fog_mode(&self) -> i32 {
        let fog = if is_editor() {
            self.fog_in_editor
        } else {
            self.fog_enabled
        };
        if fog {
            2
        } else if self.overlay.used {
            1
        } else {
            0
        }
    }
    fn material_rids(&self) -> (Option<Rid>, Option<Rid>) {
        (
            self.land_stuff.as_ref().map(|m| m.get_rid()),
            self.turf_stuff.as_ref().map(|m| m.get_rid()),
        )
    }
    pub(super) fn update_overlay(&mut self, delta: f64) {
        if !self.base().is_inside_tree() || !self.loaded {
            return;
        }
        self.sync_cell_rect();
        let cfg = self.store.config.clone();
        let auto = self.auto_rules_core();
        let auto_sig = hash_of(&auto);
        let (mat, gmat) = self.material_rids();
        if auto_sig != self.overlay.auto_sig {
            self.overlay.auto_sig = auto_sig;
            self.push_auto_rules(auto.as_ref());
            let grass = auto.as_ref().is_some_and(|a| a.active);
            if grass || self.overlay.auto_grass {
                self.mark_all_dirty();
            }
            self.overlay.auto_grass = grass;
        }
        let mode = self.fog_mode();
        let xf = self.base().get_global_transform();
        let sig = hash_of(&(
            mat,
            gmat,
            &cfg,
            is_editor(),
            mode,
            [
                self.show_grid,
                self.show_grid_in_editor,
                self.grid_chunk_lines,
                self.shader_cursor,
            ],
            [self.grid_color, self.fog_color, self.fog_color_alt],
            [
                self.grid_width,
                self.grid_fade_distance,
                self.fog_hidden_dim,
                self.fog_hidden_saturation,
                self.fog_hatch,
                self.fog_softness,
                self.highlight_width,
                self.contour_step,
            ],
            self.debug_view,
            xf,
        ));
        if sig != self.overlay.param_sig {
            self.overlay.param_sig = sig;
            self.push_overlay_params(mode);
            self.push_auto_rules(auto.as_ref());
            self.push_cursor();
        }
        if mode > 0 && !self.overlay.tiles.check_vacant() {
            let changed = self.overlay.tiles.step(delta as f32, self.fog_transition);
            if changed || self.overlay.texture.is_none() || !self.overlay.published {
                self.upload_cells();
            }
        } else if self.overlay.published {
            globals::assign("pt_cell_rect", &Vector4::ZERO.to_variant());
            self.overlay.published = false;
        }
    }
    fn sync_cell_rect(&mut self) {
        let cfg = &self.store.config;
        let mut acc = (self.store.chunks.len() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for k in self.store.chunks.keys() {
            let v = ((k.x as u32 as u64) << 32) | k.y as u32 as u64;
            acc = acc.wrapping_add(v.wrapping_mul(0xff51_afd7_ed55_8ccd).rotate_left(31) ^ v);
        }
        let sig = acc ^ hash_of(cfg);
        if sig == self.overlay.chunk_sig {
            return;
        }
        self.overlay.chunk_sig = sig;
        let (o, s) = cell_shader_data::rect_for(cfg, self.store.chunks.keys().copied());
        self.overlay.tiles.reshape(o, s);
        self.overlay.painted.enact(&mut self.overlay.tiles);
        self.overlay.tiles.dirty = true;
        self.overlay.param_sig = 0;
    }
    pub(super) fn veneer_go_out_hierarchy(&mut self) {
        if self.fog_mode() >= 2 {
            globals::assign("pt_cell_fog", &Vector4::ZERO.to_variant());
            self.overlay.param_sig = 0;
        }
        if self.overlay.published {
            globals::assign("pt_cell_rect", &Vector4::ZERO.to_variant());
            self.overlay.published = false;
        }
        self.overlay.param_sig = 0;
    }
    fn upload_cells(&mut self) {
        let cells = &self.overlay.tiles;
        let (w, h) = (cells.size.x * 2, cells.size.y);
        if w <= 0 || h <= 0 {
            return;
        }
        let bytes = cells.texture_bytes();
        let Some(img) = Image::create_from_data(
            w,
            h,
            false,
            Format::RGBA8,
            &PackedByteArray::from(bytes.as_slice()),
        ) else {
            return;
        };
        match &mut self.overlay.texture {
            Some(t) if t.get_width() == w && t.get_height() == h => t.update(&img),
            _ => self.overlay.texture = ImageTexture::create_from_image(&img),
        }
        let Some(t) = &self.overlay.texture else {
            return;
        };
        globals::assign("pt_cell_data", &t.to_variant());
        let o = cells.origin;
        globals::assign(
            "pt_cell_rect",
            &Vector4::new(
                o.x as f32,
                o.y as f32,
                cells.size.x as f32,
                cells.size.y as f32,
            )
            .to_variant(),
        );
        let cfg = &self.store.config;
        let cs = cfg.cell_size_safe();
        globals::assign(
            "pt_cell_grid",
            &Vector4::new(cfg.grid as i32 as f32, cs.x, cs.y, self.fog_softness * cs.x)
                .to_variant(),
        );
        let xf = self.base().get_global_transform();
        let sc = xf.basis.get_scale();
        let inv = |s: f32| if s.abs() < 1e-5 { 1.0 } else { 1.0 / s };
        globals::assign(
            "pt_cell_xform",
            &Vector4::new(xf.origin.x, xf.origin.z, inv(sc.x), inv(sc.z)).to_variant(),
        );
        self.overlay.published = true;
    }
    fn push_overlay_params(&mut self, mode: i32) {
        let (mat, gmat) = self.material_rids();
        let mut rs = RenderingServer::singleton();
        let cfg = &self.store.config;
        let cs = cfg.cell_size_safe();
        let stride = cfg.shader_stride();
        let grid_on = if is_editor() {
            self.show_grid_in_editor
        } else {
            self.show_grid
        };
        let hidden = Vector4::new(
            self.fog_hidden_dim,
            self.fog_hidden_saturation,
            self.fog_hatch,
            0.0,
        );
        globals::assign(
            "pt_cell_fog",
            &Vector4::new(
                if mode >= 2 { 2.0 } else { 0.0 },
                hidden.x,
                hidden.y,
                hidden.z,
            )
            .to_variant(),
        );
        globals::assign("pt_cell_fog_color", &self.fog_color.to_variant());
        if let Some(rid) = mat {
            let mut set = |k: &str, v: Variant| rs.material_set_param(rid, k, &v);
            set(
                "mst_grid",
                Vector4::new(
                    cfg.grid as i32 as f32,
                    stride.x as f32,
                    stride.y as f32,
                    cfg.ring_radius.max(1) as f32,
                )
                .to_variant(),
            );
            set("mst_cell_size", Vector2::new(cs.x, cs.y).to_variant());
            set(
                "mst_grid_line",
                Vector4::new(
                    grid_on as i32 as f32,
                    self.grid_width.max(0.0015),
                    self.grid_fade_distance.max(0.0),
                    self.grid_chunk_lines as i32 as f32,
                )
                .to_variant(),
            );
            set("mst_grid_color", self.grid_color.to_variant());
            set("mst_fog", mode.to_variant());
            set("mst_fog_a", self.fog_color.to_variant());
            set("mst_fog_b", self.fog_color_alt.to_variant());
            set("mst_fog_hidden", hidden.to_variant());
            set(
                "mst_highlight",
                Vector4::new(self.highlight_width, 4.0, 0.0, 0.0).to_variant(),
            );
            set("mst_debug_view", self.debug_view.clamp(0, 6).to_variant());
            set(
                "mst_contour_step",
                self.contour_step.max(0.0105).to_variant(),
            );
        }
        if let Some(rid) = gmat {
            rs.material_set_param(rid, "mst_fog", &mode.to_variant());
            rs.material_set_param(rid, "mst_fog_hidden", &hidden.to_variant());
            rs.material_set_param(
                rid,
                "mst_debug_view",
                &self.debug_view.clamp(0, 6).to_variant(),
            );
        }
    }
    fn push_auto_rules(&mut self, a: Option<&AutoRules>) {
        let Some(rid) = self.material_rids().0 else {
            return;
        };
        let mut rs = RenderingServer::singleton();
        let d = AutoRules::default();
        let a = a.unwrap_or(&d);
        let f = |x: i32| x as f32;
        let mut set = |k: &str, v: Variant| rs.material_set_param(rid, k, &v);
        set("mst_auto_enabled", (a.active as i32).to_variant());
        set("mst_auto_mask", (a.base_mask as i32).to_variant());
        set(
            "mst_auto_noise",
            Vector4::new(
                a.noise_scale,
                f(a.filler_slot),
                a.filler_amount,
                a.filler_scale,
            )
            .to_variant(),
        );
        set(
            "mst_auto_rock",
            Vector4::new(
                f(a.rock_slot),
                a.rock_slope,
                a.rock_noise,
                f(a.rock_filler_slot),
            )
            .to_variant(),
        );
        set(
            "mst_auto_lip",
            Vector4::new(f(a.lip_slot), a.lip_threshold, 0.0, 0.0).to_variant(),
        );
        set(
            "mst_auto_shore",
            Vector4::new(
                f(a.shore_slot),
                a.water_level,
                a.shore_height,
                a.shore_noise,
            )
            .to_variant(),
        );
        set(
            "mst_auto_shore2",
            Vector4::new(f(a.shore_filler_slot), 0.0, 0.0, 0.0).to_variant(),
        );
        set(
            "mst_auto_snow",
            Vector4::new(
                f(a.snow_slot),
                a.snow_height,
                a.snow_noise,
                a.snow_noise_scale,
            )
            .to_variant(),
        );
        set(
            "mst_auto_snow2",
            Vector4::new(a.snow_max_slope, a.snow_slope_fade, a.snow_blend, 0.0).to_variant(),
        );
    }
    fn push_cursor(&mut self) {
        let Some(rid) = self.material_rids().0 else {
            return;
        };
        let mut rs = RenderingServer::singleton();
        match self.overlay.cursor.filter(|_| self.shader_cursor) {
            Some((c, col, falloff)) => {
                rs.material_set_param(rid, "mst_cursor", &c.to_variant());
                rs.material_set_param(rid, "mst_cursor_color", &col.to_variant());
                let rim = (self.store.config.cell_size_safe().x * 0.0605).clamp(0.0405, 0.6005);
                rs.material_set_param(
                    rid,
                    "mst_cursor_params",
                    &Vector4::new(rim, 0.3005, falloff as i32 as f32, 0.0).to_variant(),
                );
            }
            None => rs.material_set_param(
                rid,
                "mst_cursor",
                &Vector4::new(0.0, 0.0, 1.0, -1.0).to_variant(),
            ),
        }
    }
    pub fn update_shader_cursor(&mut self, cursor: Option<(Vector3, f32, i32, Color, bool)>) {
        let next = cursor.map(|(p, r, shape, col, falloff)| {
            (
                Vector4::new(p.x, p.z, r.max(0.0105), shape as f32),
                col,
                falloff,
            )
        });
        if next == self.overlay.cursor {
            return;
        }
        self.overlay.cursor = next;
        self.push_cursor();
    }
    pub(super) fn painted_tints_reload(&mut self) {
        let grid = self.store.config.grid;
        let p = self
            .contents
            .as_ref()
            .and_then(|d| d.bind().tint_bytes(grid))
            .and_then(|b| PaintedTints::from_bytes(&b))
            .unwrap_or_default();
        self.apply_painted(p);
    }
    fn apply_painted(&mut self, p: PaintedTints) {
        for k in self.overlay.painted.tiles.keys() {
            self.overlay
                .tiles
                .set_tint(pt_core::math::iv2(k.0, k.1), [0; 4]);
        }
        self.overlay.painted = p;
        if !self.overlay.painted.check_vacant() {
            self.touch_cells();
        }
        self.overlay.painted.enact(&mut self.overlay.tiles);
    }
    fn painted_write(&mut self) {
        let grid = self.store.config.grid;
        let bytes = self.overlay.painted.to_bytes();
        let empty = self.overlay.painted.check_vacant();
        if let Some(d) = &mut self.contents {
            d.bind_mut()
                .write_tints(grid, if empty { None } else { Some(&bytes) });
        }
    }
    pub fn paint_tints(&mut self, cells: &[IVec2], rgba: [u8; 4]) -> bool {
        self.touch_cells();
        let mut changed = false;
        for c in cells {
            let k = self.store.config.wrap_sample(*c);
            if self.overlay.painted.assign(k, rgba) {
                changed = true;
                self.overlay.tiles.set_tint(k, rgba);
            }
        }
        if changed {
            self.painted_write();
        }
        changed
    }
    fn cell_key(&self, cell: Vector2i) -> IVec2 {
        self.store.config.wrap_sample(to_iv(cell))
    }
    fn touch_cells(&mut self) {
        self.sync_cell_rect();
        if !self.overlay.used {
            self.overlay.used = true;
            self.overlay.param_sig = 0;
        }
    }
}
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn get_chunk_tints_data(&self, chunk: Vector2i) -> PackedByteArray {
        let c = to_iv(chunk);
        let config = &self.store.config;
        let center = config.chunk_center_cell(c);
        let mut out = PaintedTints::default();
        for (k, rgba) in &self.overlay.painted.tiles {
            let cell = pt_core::math::iv2(k.0, k.1);
            if config.chunk_of(cell) == c {
                out.tiles.insert((cell.x - center.x, cell.y - center.y), *rgba);
            }
        }
        PackedByteArray::from(out.to_bytes().as_slice())
    }
    #[func]
    pub fn set_chunk_tints_data(&mut self, chunk: Vector2i, bytes: PackedByteArray) {
        let Some(given) = PaintedTints::from_bytes(bytes.as_slice()) else {
            godot_error!("MarchingSquaresTerrain.set_chunk_tints_data: not a PTT1 blob");
            return;
        };
        let c = to_iv(chunk);
        let config = self.store.config.clone();
        let center = config.chunk_center_cell(c);
        let mut painted = self.overlay.painted.clone();
        painted
            .tiles
            .retain(|k, _| config.chunk_of(pt_core::math::iv2(k.0, k.1)) != c);
        for ((x, y), rgba) in given.tiles {
            let cell = config.wrap_sample(pt_core::math::iv2(x + center.x, y + center.y));
            if config.chunk_of(cell) == c {
                painted.tiles.insert((cell.x, cell.y), rgba);
            }
        }
        if painted != self.overlay.painted {
            self.apply_painted(painted);
            self.painted_write();
        }
    }
    #[func]
    pub fn assign_pointer(&mut self, world: Vector3, radius: f32, shape: i32, tint: Color) {
        let local = self.global_to_local(world);
        self.update_shader_cursor(Some((local, radius, shape.clamp(0, 5), tint, false)));
    }
    #[func]
    pub fn clear_cursor(&mut self) {
        self.update_shader_cursor(None);
    }
    #[func]
    pub fn world_to_fog_cell(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(
            self.store
                .config
                .wrap_sample(cell_shader_data::data_cell(&self.store.config, v2(l.x, l.z))),
        )
    }
    #[func]
    pub fn get_cell_rect(&mut self) -> Rect2i {
        self.sync_cell_rect();
        let c = &self.overlay.tiles;
        Rect2i::new(from_iv(c.origin), from_iv(c.size))
    }
    #[func]
    pub fn set_cell_visibility(&mut self, cell: Vector2i, visible: bool) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.tiles.assign_shown(k, visible)
    }
    #[func]
    pub fn get_cell_visibility(&self, cell: Vector2i) -> bool {
        self.overlay.tiles.is_visible(self.cell_key(cell))
    }
    #[func]
    pub fn set_cell_explored(&mut self, cell: Vector2i, explored: bool) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.tiles.set_explored(k, explored)
    }
    #[func]
    pub fn is_cell_explored(&self, cell: Vector2i) -> bool {
        self.overlay.tiles.is_explored(self.cell_key(cell))
    }
    #[func]
    pub fn set_cells_visibility(&mut self, cells: Array<Vector2i>, shown: bool) {
        self.touch_cells();
        for c in cells.iter_shared() {
            let k = self.cell_key(c);
            self.overlay.tiles.assign_shown(k, shown);
        }
    }
    #[func]
    pub fn hide_all_cells(&mut self) {
        self.touch_cells();
        self.overlay.tiles.hide_all();
    }
    #[func]
    pub fn reveal_all_cells(&mut self) {
        self.touch_cells();
        self.overlay.tiles.flood(true, true);
    }
    #[func]
    pub fn reset_fog(&mut self) {
        self.touch_cells();
        self.overlay.tiles.flood(false, false);
    }
    #[func]
    pub fn finish_fog_transition(&mut self) {
        self.overlay.tiles.conclude();
    }
    #[func]
    pub fn set_cell_tint(&mut self, cell: Vector2i, color: Color) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.tiles.set_tint(k, color_bytes(color))
    }
    #[func]
    pub fn get_cell_tint(&self, cell: Vector2i) -> Color {
        let t = self
            .overlay
            .tiles
            .tint(self.cell_key(cell))
            .unwrap_or([0; 4]);
        Color::from_rgba8(t[0], t[1], t[2], t[3])
    }
    #[func]
    pub fn set_cells_tint(&mut self, cells: Array<Vector2i>, tint: Color) {
        self.touch_cells();
        let b = color_bytes(tint);
        for c in cells.iter_shared() {
            let k = self.cell_key(c);
            self.overlay.tiles.set_tint(k, b);
        }
    }
    #[func]
    pub fn set_cell_highlight(&mut self, cell: Vector2i, strength: f32) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.tiles.set_highlight(k, strength)
    }
    #[func]
    pub fn clear_cell_tints(&mut self) {
        self.overlay.tiles.clear_tints();
        self.overlay.painted.enact(&mut self.overlay.tiles);
    }
    #[func]
    pub fn paint_cell_tint(&mut self, cell: Vector2i, color: Color) -> bool {
        self.paint_tints(&[to_iv(cell)], color_bytes(color))
    }
    #[func]
    pub fn get_painted_tint(&self, cell: Vector2i) -> Color {
        let t = self
            .overlay
            .painted
            .fetch(self.cell_key(cell))
            .unwrap_or([0; 4]);
        Color::from_rgba8(t[0], t[1], t[2], t[3])
    }
    #[func]
    pub fn get_painted_tints_data(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.painted.to_bytes().as_slice())
    }
    #[func]
    pub fn set_painted_tints_data(&mut self, bytes: PackedByteArray) {
        let Some(p) = PaintedTints::from_bytes(bytes.as_slice()) else {
            godot_error!("MarchingSquaresTerrain.set_painted_tints_data: not a PTT1 blob");
            return;
        };
        self.apply_painted(p);
        self.painted_write();
    }
    #[func]
    pub fn get_cells_bytes(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.tiles.state_bytes().as_slice())
    }
    #[func]
    pub fn set_cells_from_bytes(&mut self, bytes: PackedByteArray) {
        self.touch_cells();
        self.overlay.tiles.set_state_bytes(bytes.as_slice());
    }
    #[func]
    pub fn get_cell_tints_bytes(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.tiles.tint_bytes().as_slice())
    }
    #[func]
    pub fn set_cell_tints_from_bytes(&mut self, bytes: PackedByteArray) {
        self.touch_cells();
        self.overlay.tiles.set_tint_bytes(bytes.as_slice());
    }
    #[func]
    pub fn get_auto_slot_at(&self, world: Vector3) -> i32 {
        let Some(a) = self.auto_rules_core().filter(|a| a.active) else {
            return -1;
        };
        let l = self.global_to_local(world);
        let p = v2(l.x, l.z);
        let Some(h) = self.local_height_at(p) else {
            return -1;
        };
        let e = self.store.config.cell_size_safe().x * 0.2505;
        let hx = self.local_height_at(p + v2(e, 0.0)).unwrap_or(h)
            - self.local_height_at(p - v2(e, 0.0)).unwrap_or(h);
        let hz = self.local_height_at(p + v2(0.0, e)).unwrap_or(h)
            - self.local_height_at(p - v2(0.0, e)).unwrap_or(h);
        let n = Vector3::new(-hx, 2.0 * e, -hz).normalized();
        a.eval(pt_core::math::v3(l.x, h, l.z), n.y, 0.0)
            .map(|s| s as i32)
            .unwrap_or(-1)
    }
}
