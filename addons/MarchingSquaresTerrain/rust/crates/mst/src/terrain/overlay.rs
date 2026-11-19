//! Terrain overlays driven by `MarchingSquaresTerrain` (child module of `node.rs`): grid lines,
//! the shader brush cursor, per-cell data (fog of war, tint, highlight; `pt_core::celldata`),
//! biome auto-texturing rules (`pt_core::autotex`) and debug views. Everything ends up as
//! `mst_*` material parameters set through the RenderingServer (never saved into the material)
//! plus the `pt_cell_*` global shader parameters for the cell data texture.

use std::hash::{Hash, Hasher};

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, RenderingServer};
use godot::prelude::*;

use pt_core::autotex::AutoRules;
use pt_core::celldata::{self, CellData, PaintedTints};
use pt_core::math::{IVec2, v2};

use super::MarchingSquaresTerrain;
use crate::globals;
use crate::util::*;

#[derive(Default)]
pub struct OverlayState {
    pub cells: CellData,
    texture: Option<Gd<ImageTexture>>,
    chunk_sig: u64,
    param_sig: u64,
    auto_sig: u64,
    auto_grass: bool,
    /// (centre x, z, radius, shape), color, falloff.
    cursor: Option<(Vector4, Color, bool)>,
    published: bool,
    /// A script touched the cell data (tint / highlight show even without fog).
    used: bool,
    /// Painted (saved) tints, the base layer under runtime tints (Tint tool).
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
        let ts = self.texture_set.as_ref()?;
        let r = ts.bind().auto_rules.clone()?;
        Some(r.bind().to_core())
    }

    fn fog_mode(&self) -> i32 {
        let fog = if is_editor() { self.fog_in_editor } else { self.fog_enabled };
        if fog {
            2
        } else if self.overlay.used {
            1
        } else {
            0
        }
    }

    fn material_rids(&self) -> (Option<Rid>, Option<Rid>) {
        (self.terrain_material.as_ref().map(|m| m.get_rid()), self.grass_material.as_ref().map(|m| m.get_rid()))
    }

    /// Per-frame overlay work: cell rectangle, parameter pushes, fog transitions, uploads.
    pub(super) fn update_overlay(&mut self, delta: f64) {
        if !self.base().is_inside_tree() || !self.loaded {
            return;
        }
        self.sync_cell_rect();
        let cfg = self.store.config.clone();
        // Auto rules (texture preset sub-resource).
        let auto = self.auto_rules_core();
        let auto_sig = hash_of(&auto);
        let (mat, gmat) = self.material_rids();
        if auto_sig != self.overlay.auto_sig {
            self.overlay.auto_sig = auto_sig;
            self.push_auto_rules(auto.as_ref());
            let grass = auto.as_ref().is_some_and(|a| a.enabled);
            if grass || self.overlay.auto_grass {
                self.mark_all_dirty();
            }
            self.overlay.auto_grass = grass;
        }
        // Grid / fog / debug parameters.
        let mode = self.fog_mode();
        let xf = self.base().get_global_transform();
        let sig = hash_of(&(
            mat,
            gmat,
            &cfg,
            is_editor(),
            mode,
            [self.show_grid, self.show_grid_in_editor, self.grid_chunk_lines, self.shader_cursor],
            [self.grid_color, self.fog_color, self.fog_color_alt],
            [self.grid_width, self.grid_fade_distance, self.fog_hidden_dim, self.fog_hidden_saturation, self.fog_hatch, self.fog_softness, self.highlight_width, self.contour_step],
            self.debug_view,
            xf,
        ));
        if sig != self.overlay.param_sig {
            self.overlay.param_sig = sig;
            self.push_overlay_params(mode);
            // Material swaps lose RS-side parameters: re-send rules and cursor too.
            self.push_auto_rules(auto.as_ref());
            self.push_cursor();
        }
        // Cell data texture.
        if mode > 0 && !self.overlay.cells.is_empty() {
            let changed = self.overlay.cells.step(delta as f32, self.fog_transition);
            if changed || self.overlay.texture.is_none() || !self.overlay.published {
                self.upload_cells();
            }
        } else if self.overlay.published {
            globals::set("pt_cell_rect", &Vector4::ZERO.to_variant());
            self.overlay.published = false;
        }
    }

    /// Keeps the cell data rectangle on the chunk set (cheap order-independent signature).
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
        let (o, s) = celldata::rect_for(cfg, self.store.chunks.keys().copied());
        self.overlay.cells.resize(o, s);
        // New cells (added chunks) start untinted: lay the painted tints down again.
        self.overlay.painted.apply(&mut self.overlay.cells);
        self.overlay.cells.dirty = true;
        self.overlay.param_sig = 0;
    }

    pub(super) fn overlay_exit_tree(&mut self) {
        if self.fog_mode() >= 2 {
            globals::set("pt_cell_fog", &Vector4::ZERO.to_variant());
            self.overlay.param_sig = 0;
        }
        if self.overlay.published {
            globals::set("pt_cell_rect", &Vector4::ZERO.to_variant());
            self.overlay.published = false;
        }
        self.overlay.param_sig = 0;
    }

    fn upload_cells(&mut self) {
        let cells = &self.overlay.cells;
        let (w, h) = (cells.size.x * 2, cells.size.y);
        if w <= 0 || h <= 0 {
            return;
        }
        let bytes = cells.texture_bytes();
        let Some(img) = Image::create_from_data(w, h, false, Format::RGBA8, &PackedByteArray::from(bytes.as_slice())) else { return };
        match &mut self.overlay.texture {
            Some(t) if t.get_width() == w && t.get_height() == h => t.update(&img),
            _ => self.overlay.texture = ImageTexture::create_from_image(&img),
        }
        let Some(t) = &self.overlay.texture else { return };
        globals::set("pt_cell_data", &t.to_variant());
        let o = cells.origin;
        globals::set("pt_cell_rect", &Vector4::new(o.x as f32, o.y as f32, cells.size.x as f32, cells.size.y as f32).to_variant());
        let cfg = &self.store.config;
        let cs = cfg.cell_size_safe();
        globals::set("pt_cell_grid", &Vector4::new(cfg.grid as i32 as f32, cs.x, cs.y, self.fog_softness * cs.x).to_variant());
        let xf = self.base().get_global_transform();
        let sc = xf.basis.get_scale();
        let inv = |s: f32| if s.abs() < 1e-5 { 1.0 } else { 1.0 / s };
        globals::set("pt_cell_xform", &Vector4::new(xf.origin.x, xf.origin.z, inv(sc.x), inv(sc.z)).to_variant());
        self.overlay.published = true;
    }

    fn push_overlay_params(&mut self, mode: i32) {
        let (mat, gmat) = self.material_rids();
        let mut rs = RenderingServer::singleton();
        let cfg = &self.store.config;
        let cs = cfg.cell_size_safe();
        let stride = cfg.chunk_stride();
        let grid_on = if is_editor() { self.show_grid_in_editor } else { self.show_grid };
        let hidden = Vector4::new(self.fog_hidden_dim, self.fog_hidden_saturation, self.fog_hatch, 0.0);
        // Water, props and objects read the fog through globals (`pt_fog_surface`).
        globals::set("pt_cell_fog", &Vector4::new(if mode >= 2 { 2.0 } else { 0.0 }, hidden.x, hidden.y, hidden.z).to_variant());
        globals::set("pt_cell_fog_color", &self.fog_color.to_variant());
        if let Some(rid) = mat {
            let mut set = |k: &str, v: Variant| rs.material_set_param(rid, k, &v);
            set("mst_grid", Vector4::new(cfg.grid as i32 as f32, stride.x as f32, stride.y as f32, cfg.ring_radius.max(1) as f32).to_variant());
            set("mst_cell_size", Vector2::new(cs.x, cs.y).to_variant());
            set(
                "mst_grid_line",
                Vector4::new(grid_on as i32 as f32, self.grid_width.max(0.001), self.grid_fade_distance.max(0.0), self.grid_chunk_lines as i32 as f32).to_variant(),
            );
            set("mst_grid_color", self.grid_color.to_variant());
            set("mst_fog", mode.to_variant());
            set("mst_fog_a", self.fog_color.to_variant());
            set("mst_fog_b", self.fog_color_alt.to_variant());
            set("mst_fog_hidden", hidden.to_variant());
            set("mst_highlight", Vector4::new(self.highlight_width, 4.0, 0.0, 0.0).to_variant());
            set("mst_debug_view", self.debug_view.clamp(0, 6).to_variant());
            set("mst_contour_step", self.contour_step.max(0.01).to_variant());
        }
        if let Some(rid) = gmat {
            rs.material_set_param(rid, "mst_fog", &mode.to_variant());
            rs.material_set_param(rid, "mst_fog_hidden", &hidden.to_variant());
            rs.material_set_param(rid, "mst_debug_view", &self.debug_view.clamp(0, 6).to_variant());
        }
    }

    fn push_auto_rules(&mut self, a: Option<&AutoRules>) {
        let Some(rid) = self.material_rids().0 else { return };
        let mut rs = RenderingServer::singleton();
        let d = AutoRules::default();
        let a = a.unwrap_or(&d);
        let f = |x: i32| x as f32;
        let mut set = |k: &str, v: Variant| rs.material_set_param(rid, k, &v);
        set("mst_auto_enabled", (a.enabled as i32).to_variant());
        set("mst_auto_mask", (a.base_mask as i32).to_variant());
        set("mst_auto_noise", Vector4::new(a.noise_scale, f(a.filler_slot), a.filler_amount, a.filler_scale).to_variant());
        set("mst_auto_rock", Vector4::new(f(a.rock_slot), a.rock_slope, a.rock_noise, f(a.rock_filler_slot)).to_variant());
        set("mst_auto_lip", Vector4::new(f(a.lip_slot), a.lip_threshold, 0.0, 0.0).to_variant());
        set("mst_auto_shore", Vector4::new(f(a.shore_slot), a.water_level, a.shore_height, a.shore_noise).to_variant());
        set("mst_auto_shore2", Vector4::new(f(a.shore_filler_slot), 0.0, 0.0, 0.0).to_variant());
        set("mst_auto_snow", Vector4::new(f(a.snow_slot), a.snow_height, a.snow_noise, a.snow_noise_scale).to_variant());
        set("mst_auto_snow2", Vector4::new(a.snow_max_slope, a.snow_slope_fade, a.snow_blend, 0.0).to_variant());
    }

    fn push_cursor(&mut self) {
        let Some(rid) = self.material_rids().0 else { return };
        let mut rs = RenderingServer::singleton();
        match self.overlay.cursor.filter(|_| self.shader_cursor) {
            Some((c, col, falloff)) => {
                rs.material_set_param(rid, "mst_cursor", &c.to_variant());
                rs.material_set_param(rid, "mst_cursor_color", &col.to_variant());
                let rim = (self.store.config.cell_size_safe().x * 0.06).clamp(0.04, 0.6);
                rs.material_set_param(rid, "mst_cursor_params", &Vector4::new(rim, 0.3, falloff as i32 as f32, 0.0).to_variant());
            }
            None => rs.material_set_param(rid, "mst_cursor", &Vector4::new(0.0, 0.0, 1.0, -1.0).to_variant()),
        }
    }

    /// Rust-side cursor for the editor plugin: terrain-local centre, radius, brush shape
    /// (`BrushShape` index), color, banded falloff. `None` hides it.
    pub fn update_shader_cursor(&mut self, cursor: Option<(Vector3, f32, i32, Color, bool)>) {
        let next = cursor.map(|(p, r, shape, col, falloff)| (Vector4::new(p.x, p.z, r.max(0.01), shape as f32), col, falloff));
        if next == self.overlay.cursor {
            return;
        }
        self.overlay.cursor = next;
        self.push_cursor();
    }

    /// Loads the painted tints of the current grid type from the data resource.
    pub(super) fn painted_tints_reload(&mut self) {
        let grid = self.store.config.grid;
        let p = self.data.as_ref().and_then(|d| d.bind().tint_bytes(grid)).and_then(|b| PaintedTints::from_bytes(&b)).unwrap_or_default();
        self.apply_painted(p);
    }

    /// Replaces the painted layer: cells it no longer tints are cleared, the new ones applied.
    fn apply_painted(&mut self, p: PaintedTints) {
        for k in self.overlay.painted.cells.keys() {
            self.overlay.cells.set_tint(pt_core::math::iv2(k.0, k.1), [0; 4]);
        }
        self.overlay.painted = p;
        if !self.overlay.painted.is_empty() {
            self.touch_cells();
        }
        self.overlay.painted.apply(&mut self.overlay.cells);
    }

    fn painted_write(&mut self) {
        let grid = self.store.config.grid;
        let bytes = self.overlay.painted.to_bytes();
        let empty = self.overlay.painted.is_empty();
        if let Some(d) = &mut self.data {
            d.bind_mut().write_tints(grid, if empty { None } else { Some(&bytes) });
        }
    }

    /// Paints (alpha > 0) or erases (alpha 0) the saved tint of the cells; returns true when
    /// anything changed. Editor Tint tool; the data is written to the resource right away.
    pub fn paint_tints(&mut self, cells: &[IVec2], rgba: [u8; 4]) -> bool {
        self.touch_cells();
        let mut changed = false;
        for c in cells {
            let k = self.store.config.wrap_sample(*c);
            if self.overlay.painted.set(k, rgba) {
                changed = true;
                self.overlay.cells.set_tint(k, rgba);
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
    // ---- cursor ----

    /// Draws a footprint on the ground (world centre, radius, shape 0 round / 1 square /
    /// 2 hexagon / 3 hexagon 30° / 4 triangle / 5 triangle 180°, color) — e.g. a build
    /// placement preview, green when valid, red when not. Needs `shader_cursor`.
    #[func]
    pub fn set_cursor(&mut self, world: Vector3, radius: f32, shape: i32, color: Color) {
        let local = self.global_to_local(world);
        self.update_shader_cursor(Some((local, radius, shape.clamp(0, 5), color, false)));
    }

    #[func]
    pub fn clear_cursor(&mut self) {
        self.update_shader_cursor(None);
    }

    // ---- cell data: fog of war, tint, highlight ----

    /// Cell of the cell data (fog / tint) under a world point: `world_to_cell` on cell grids,
    /// the marching-squares cell `floor(p / cell_size)` on square grids.
    #[func]
    pub fn world_to_fog_cell(&self, world: Vector3) -> Vector2i {
        let l = self.global_to_local(world);
        from_iv(self.store.config.wrap_sample(celldata::data_cell(&self.store.config, v2(l.x, l.z))))
    }

    /// Rectangle of cells covered by the cell data (`position` = first cell, `size` in cells),
    /// the layout of the `*_bytes` calls (row-major, `size.x` cells per row).
    #[func]
    pub fn get_cell_rect(&mut self) -> Rect2i {
        self.sync_cell_rect();
        let c = &self.overlay.cells;
        Rect2i::new(from_iv(c.origin), from_iv(c.size))
    }

    /// Visible cells are explored too. Returns false outside the terrain.
    #[func]
    pub fn set_cell_visibility(&mut self, cell: Vector2i, visible: bool) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.cells.set_visible(k, visible)
    }

    #[func]
    pub fn get_cell_visibility(&self, cell: Vector2i) -> bool {
        self.overlay.cells.is_visible(self.cell_key(cell))
    }

    /// Un-exploring a cell also hides it.
    #[func]
    pub fn set_cell_explored(&mut self, cell: Vector2i, explored: bool) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.cells.set_explored(k, explored)
    }

    #[func]
    pub fn is_cell_explored(&self, cell: Vector2i) -> bool {
        self.overlay.cells.is_explored(self.cell_key(cell))
    }

    /// Bulk visibility for a cell list (e.g. a unit's vision disk).
    #[func]
    pub fn set_cells_visibility(&mut self, cells: Array<Vector2i>, visible: bool) {
        self.touch_cells();
        for c in cells.iter_shared() {
            let k = self.cell_key(c);
            self.overlay.cells.set_visible(k, visible);
        }
    }

    /// Hides every cell but keeps it explored (start of a per-turn vision update).
    #[func]
    pub fn hide_all_cells(&mut self) {
        self.touch_cells();
        self.overlay.cells.hide_all();
    }

    /// Everything visible and explored.
    #[func]
    pub fn reveal_all_cells(&mut self) {
        self.touch_cells();
        self.overlay.cells.fill(true, true);
    }

    /// Everything unexplored again.
    #[func]
    pub fn reset_fog(&mut self) {
        self.touch_cells();
        self.overlay.cells.fill(false, false);
    }

    /// Skips the running fade (e.g. after loading a save).
    #[func]
    pub fn finish_fog_transition(&mut self) {
        self.overlay.cells.finish();
    }

    /// Tint color of a cell; alpha = strength (0 = none). Territory, selection, AoE previews.
    #[func]
    pub fn set_cell_tint(&mut self, cell: Vector2i, color: Color) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.cells.set_tint(k, color_bytes(color))
    }

    #[func]
    pub fn get_cell_tint(&self, cell: Vector2i) -> Color {
        let t = self.overlay.cells.tint(self.cell_key(cell)).unwrap_or([0; 4]);
        Color::from_rgba8(t[0], t[1], t[2], t[3])
    }

    #[func]
    pub fn set_cells_tint(&mut self, cells: Array<Vector2i>, color: Color) {
        self.touch_cells();
        let b = color_bytes(color);
        for c in cells.iter_shared() {
            let k = self.cell_key(c);
            self.overlay.cells.set_tint(k, b);
        }
    }

    /// Pulsing outline around a cell (0..1; 0 = off), drawn in the cell's tint color.
    #[func]
    pub fn set_cell_highlight(&mut self, cell: Vector2i, strength: f32) -> bool {
        self.touch_cells();
        let k = self.cell_key(cell);
        self.overlay.cells.set_highlight(k, strength)
    }

    /// Removes every runtime tint and highlight (painted tints stay).
    #[func]
    pub fn clear_cell_tints(&mut self) {
        self.overlay.cells.clear_tints();
        self.overlay.painted.apply(&mut self.overlay.cells);
    }

    /// Paints a saved tint on a cell (alpha = strength; alpha 0 erases). Painted tints are
    /// stored in the terrain data (per grid type) and sit under the runtime `set_cell_tint`s;
    /// `clear_cell_tints` restores them. The editor's Tint tool uses this.
    #[func]
    pub fn paint_cell_tint(&mut self, cell: Vector2i, color: Color) -> bool {
        self.paint_tints(&[to_iv(cell)], color_bytes(color))
    }

    /// Painted tint of a cell (transparent when none).
    #[func]
    pub fn get_painted_tint(&self, cell: Vector2i) -> Color {
        let t = self.overlay.painted.get(self.cell_key(cell)).unwrap_or([0; 4]);
        Color::from_rgba8(t[0], t[1], t[2], t[3])
    }

    /// Every painted tint as a `PTT1` blob (undo snapshots, copying between terrains).
    #[func]
    pub fn get_painted_tints_data(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.painted.to_bytes().as_slice())
    }

    /// Replaces every painted tint with a `get_painted_tints_data` blob (empty = none).
    #[func]
    pub fn set_painted_tints_data(&mut self, bytes: PackedByteArray) {
        let Some(p) = PaintedTints::from_bytes(bytes.as_slice()) else {
            godot_error!("MarchingSquaresTerrain.set_painted_tints_data: not a PTT1 blob");
            return;
        };
        self.apply_painted(p);
        self.painted_write();
    }

    /// State of every cell in `get_cell_rect` order, 4 bytes per cell:
    /// `(visible ≥ 128, explored ≥ 128, highlight, reserved)`.
    #[func]
    pub fn get_cells_bytes(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.cells.state_bytes().as_slice())
    }

    /// Bulk state update in the `get_cells_bytes` layout (one call per vision update).
    #[func]
    pub fn set_cells_from_bytes(&mut self, bytes: PackedByteArray) {
        self.touch_cells();
        self.overlay.cells.set_state_bytes(bytes.as_slice());
    }

    /// Tints of every cell in `get_cell_rect` order, RGBA8 (alpha = strength).
    #[func]
    pub fn get_cell_tints_bytes(&self) -> PackedByteArray {
        PackedByteArray::from(self.overlay.cells.tint_bytes().as_slice())
    }

    #[func]
    pub fn set_cell_tints_from_bytes(&mut self, bytes: PackedByteArray) {
        self.touch_cells();
        self.overlay.cells.set_tint_bytes(bytes.as_slice());
    }

    /// Slot the texture preset's auto rules would put at a world point on flat ground of the
    /// default floor slot (-1 = rules off or base slot kept). Normal estimated from heights.
    #[func]
    pub fn get_auto_slot_at(&self, world: Vector3) -> i32 {
        let Some(a) = self.auto_rules_core().filter(|a| a.enabled) else { return -1 };
        let l = self.global_to_local(world);
        let p = v2(l.x, l.z);
        let Some(h) = self.local_height_at(p) else { return -1 };
        let e = self.store.config.cell_size_safe().x * 0.25;
        let hx = self.local_height_at(p + v2(e, 0.0)).unwrap_or(h) - self.local_height_at(p - v2(e, 0.0)).unwrap_or(h);
        let hz = self.local_height_at(p + v2(0.0, e)).unwrap_or(h) - self.local_height_at(p - v2(0.0, e)).unwrap_or(h);
        let n = Vector3::new(-hx, 2.0 * e, -hz).normalized();
        a.eval(pt_core::math::v3(l.x, h, l.z), n.y, 0.0).map(|s| s as i32).unwrap_or(-1)
    }
}
