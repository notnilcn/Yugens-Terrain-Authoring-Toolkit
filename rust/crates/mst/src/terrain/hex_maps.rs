//! Hex game map API (child module of `node.rs`): height / water level / biome maps as images and
//! textures for a rectangle of cells or a window around the player (BitCraft's continuous
//! map window), per-cell water levels (kg_world `water_level`), and blocked (paved /
//! built-on) small hexes for top decorations.
//!
//! Maps are rectangles of grid sample coordinates (`Rect2i`: position = first cell, size in
//! cells; axial `(q, r)` on hex rings): texel `(x, y)` is cell `position + (x, y)`, read
//! through the torus wrap. See `pt_core::hexmap` for the combined texel layout.

use std::collections::{HashMap, HashSet};

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, Texture2D};
use godot::prelude::*;

use pt_core::brush::Touched;
use pt_core::hexmap::{self, MapWindow, NO_WATER};
use pt_core::math::{IVec2, Vec3 as PVec3, iv2, v2};

use super::MarchingSquaresTerrain;
use crate::util::*;

#[derive(Default)]
pub struct HexMapState {
    /// Paved / built-on small hexes (no top decorations, scatter props removed).
    pub blocked: HashSet<IVec2>,
    window: MapWindow,
    texture: Option<Gd<ImageTexture>>,
    timer: f64,
    /// Terrain data / water changed since the window texture was built.
    pub dirty: bool,
}

fn rect_window(r: Rect2i) -> MapWindow {
    MapWindow::new(iv2(r.position.x, r.position.y), iv2(r.size.x, r.size.y))
}

fn window_rect(w: MapWindow) -> Rect2i {
    Rect2i::new(Vector2i::new(w.origin.x, w.origin.y), Vector2i::new(w.size.x, w.size.y))
}

/// Largest map built in one call (texels).
const MAX_TEXELS: usize = 4096 * 4096;

fn float_image(w: MapWindow, values: &[f32]) -> Option<Gd<Image>> {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    Image::create_from_data(w.size.x, w.size.y, false, Format::RF, &PackedByteArray::from(bytes.as_slice()))
}

/// Reads channel R of every texel of `img` (float formats: the value; 8/16-bit unorm formats:
/// 0..1, so pass a scale such as 255 · level height).
fn image_values(img: &Gd<Image>) -> (IVec2, Vec<f32>) {
    let (w, h) = (img.get_width(), img.get_height());
    let mut v = Vec::with_capacity((w * h).max(0) as usize);
    for y in 0..h {
        for x in 0..w {
            v.push(img.get_pixel(x, y).r);
        }
    }
    (iv2(w, h), v)
}

fn is_unorm(f: Format) -> bool {
    !matches!(f, Format::RF | Format::RGF | Format::RGBF | Format::RGBAF | Format::RH | Format::RGH | Format::RGBH | Format::RGBAH)
}

impl MarchingSquaresTerrain {
    /// The PixelWater nodes of this terrain's scene that belong to it (`terrain` unset = the
    /// first terrain), like the grass water cull.
    fn owned_waters(&self) -> Vec<Gd<Node>> {
        if !self.base().is_inside_tree() {
            return Vec::new();
        }
        let tn = self.to_gd().upcast::<Node>();
        let scene = match tn.get_owner() {
            Some(o) => o,
            None if is_editor() => tn.clone(),
            None => tn.get_tree().get_root().upcast::<Node>(),
        };
        let mut waters: Vec<Gd<Node>> = scene.find_children_ex("*").type_("PixelWater").recursive(true).owned(false).done().iter_shared().collect();
        if scene.is_class("PixelWater") {
            waters.push(scene.clone());
        }
        let first = crate::effects::find_terrain(&scene);
        waters.retain(|w| w.get("terrain").try_to::<Gd<Node>>().ok().or_else(|| first.clone()).is_none_or(|o| o == tn));
        waters
    }

    /// Water surface per cell: explicit levels, else the raised cells of this terrain's
    /// PixelWater nodes, else the sea (a visible PixelWater plane) for cells below it.
    pub(super) fn water_source(&self) -> impl Fn(IVec2) -> Option<f32> + '_ {
        let mut raised: HashMap<IVec2, f32> = HashMap::new();
        let mut sea: Option<f32> = None;
        let ty = if self.base().is_inside_tree() { self.base().get_global_position().y } else { 0.0 };
        for w in self.owned_waters() {
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    let e = raised.entry(self.store.config.wrap_sample(iv2(v.x.round() as i32, v.y.round() as i32))).or_insert(v.z);
                    *e = e.max(v.z);
                }
            }
            if let Ok(n) = w.clone().try_cast::<Node3D>()
                && n.is_visible_in_tree()
            {
                let y = n.get_global_position().y - ty;
                sea = Some(sea.map_or(y, |s: f32| s.max(y)));
            }
        }
        move |c: IVec2| {
            let k = self.store.config.wrap_sample(c);
            if let Some(w) = self.store.water(c).or_else(|| raised.get(&k).copied()) {
                return Some(w);
            }
            let h = self.store.height(c)?;
            sea.filter(|s| *s > h)
        }
    }

    /// Decoration spots (small hex, terrain-local point) of a cell with the game filters:
    /// `min_height`, the cell's water level and blocked small hexes.
    pub(crate) fn decoration_spots_local(&self, cell: IVec2, min_height: f32) -> Vec<(IVec2, PVec3)> {
        let water = self.store.water(cell);
        let blocked = &self.hexmaps.blocked;
        pt_core::cells::decoration_spots_ex(&self.store, cell, min_height, &|small, p| {
            blocked.contains(&small) || water.is_some_and(|w| p.y <= w)
        })
    }

    fn map_image(&self, r: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(r);
        if w.is_empty() || w.len() > MAX_TEXELS {
            godot_warn!("MarchingSquaresTerrain: map rect {r:?} is empty or larger than 4096²");
            return None;
        }
        let water = self.water_source();
        super::hex_columns::rgbaf_image(w, &hexmap::build_cell_map(&self.store, w, &|c| water(c)))
    }

    /// Focus cell of the map window: `map_window_focus`, else the LOD focus point.
    fn map_focus_cell(&self) -> Option<IVec2> {
        let p = match self.map_window_focus.as_ref().filter(|f| f.is_instance_valid() && f.is_inside_tree()) {
            Some(f) => f.get_global_position(),
            None => self.lod_focus_point()?,
        };
        let l = self.global_to_local(p);
        Some(self.store.config.world_to_sample(v2(l.x, l.z)))
    }

    /// Per-frame: recentres / rebuilds the map window texture (a few times per second).
    pub(super) fn update_map_window(&mut self, delta: f64) {
        if self.map_window_radius <= 0 || !self.grid().is_cell_mode() {
            return;
        }
        self.hexmaps.timer -= delta;
        if self.hexmaps.timer > 0.0 {
            return;
        }
        self.hexmaps.timer = 0.2;
        if let Some(c) = self.map_focus_cell() {
            self.map_window_at(c, false);
        }
    }

    /// Moves the window to `center` if needed (or `force`), rebuilding its texture.
    fn map_window_at(&mut self, center: IVec2, force: bool) -> bool {
        let r = self.map_window_radius.max(1);
        let margin = self.map_window_margin.clamp(0, r);
        let moved = force || hexmap::needs_recenter(self.hexmaps.window, center, margin) || self.hexmaps.window.size.x != 2 * r + 1;
        if !moved && !self.hexmaps.dirty {
            return false;
        }
        let w = if moved { hexmap::window_around(self.grid(), center, r) } else { self.hexmaps.window };
        let Some(img) = self.map_image(window_rect(w)) else { return false };
        match &mut self.hexmaps.texture {
            Some(t) if self.hexmaps.window.size == w.size => t.update(&img),
            _ => self.hexmaps.texture = ImageTexture::create_from_image(&img),
        }
        self.hexmaps.window = w;
        self.hexmaps.dirty = false;
        self.signals().map_window_changed().emit(window_rect(w));
        true
    }

    /// Writes water levels into the store (and the data resource), then refreshes the maps, the
    /// grass cull and the PixelWater pools / falls (via the rebuilt chunks).
    fn write_water(&mut self, items: Vec<(IVec2, Option<f32>)>) {
        let mut touched = Touched::new();
        let mut owners: Vec<IVec2> = Vec::new();
        for (c, w) in items {
            if self.store.water(c) == w.filter(|w| pt_core::data::has_water(*w)) {
                continue;
            }
            if self.store.set_water(c, w) {
                for (ch, _) in self.store.config.sample_slots(c) {
                    if !owners.contains(&ch) {
                        owners.push(ch);
                    }
                }
                touched.extend(self.store.chunks_affected_by(c));
            }
        }
        if touched.is_empty() {
            return;
        }
        self.write_chunks(&owners);
        self.mark_dirty(&touched);
        self.hexmaps.dirty = true;
        self.columns.need_map = true;
    }

    fn apply_map_touch(&mut self, t: Vec<IVec2>) {
        let touched: Touched = t.into_iter().collect();
        self.mark_dirty(&touched);
    }
}

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    // ---- water levels (kg_world `water_level`) ----

    /// Water surface height of a cell (terrain-local, same units as heights). A level above
    /// the cell's ground makes it a raised pool (saved in the terrain data; PixelWater
    /// draws it with the waterfalls where a neighbour drops `waterfall_min_levels` or more).
    #[func]
    pub fn set_cell_water_level(&mut self, cell: Vector2i, level: f32) {
        self.write_water(vec![(to_iv(cell), Some(level))]);
    }

    /// Bulk water levels (pairs `cells[i]` with `levels[i]`; NaN or ≤ -9999 clears).
    #[func]
    pub fn set_cell_water_levels(&mut self, cells: Array<Vector2i>, levels: PackedFloat32Array) {
        let items = cells.iter_shared().zip(levels.as_slice().iter().copied()).map(|(c, l)| (to_iv(c), Some(l))).collect();
        self.write_water(items);
    }

    /// Integer water levels (`level * level_height`), e.g. a kg_world patch's `water_level`s
    /// with `get_chunk_cells_spiral(patch)`; levels ≤ the cell's elevation still count (dry).
    #[func]
    pub fn set_cell_water_level_steps(&mut self, cells: Array<Vector2i>, levels: PackedInt32Array) {
        let lh = self.level_height_safe();
        let hs: PackedFloat32Array = levels.as_slice().iter().map(|l| *l as f32 * lh).collect();
        self.set_cell_water_levels(cells, hs);
    }

    #[func]
    pub fn clear_cell_water_level(&mut self, cell: Vector2i) {
        self.write_water(vec![(to_iv(cell), None)]);
    }

    #[func]
    pub fn clear_water_levels(&mut self) {
        let items = self.store.water_cells().into_iter().map(|(c, _)| (c, None)).collect();
        self.write_water(items);
    }

    /// Stored water levels whose surface is above the ground (raised pools / rivers) as
    /// `(q, r, level)` — what PixelWater draws as pools and waterfalls (it merges these with its
    /// own `water_cells`).
    #[func]
    pub fn get_raised_water_cells(&self) -> PackedVector3Array {
        self.store
            .water_cells()
            .into_iter()
            .filter(|(c, w)| self.store.height(*c).is_some_and(|h| *w > h + 1e-4))
            .map(|(c, w)| Vector3::new(c.x as f32, c.y as f32, w))
            .collect()
    }

    /// Every stored water level as `(q, r, level)` (dry levels included).
    #[func]
    pub fn get_water_cells(&self) -> PackedVector3Array {
        self.store.water_cells().into_iter().map(|(c, w)| Vector3::new(c.x as f32, c.y as f32, w)).collect()
    }

    /// Water surface of a cell: its explicit level, else a PixelWater raised cell, else the sea
    /// level where the cell is below it; -10000 for dry cells.
    #[func]
    pub fn get_cell_water_level(&self, cell: Vector2i) -> f32 {
        self.water_source()(to_iv(cell)).unwrap_or(NO_WATER)
    }

    /// Edges where water spills into a lower neighbour (from the cell water levels and the
    /// scene's PixelWater cells / sea): `{cell, neighbor, top, bottom, waterfall,
    /// into_water}`, `waterfall` when the drop is at least `min_levels · level_height` (the
    /// TODO's "waterfalls where a neighbour drops 5 or more": `min_levels = 5`).
    #[func]
    pub fn get_waterfalls(&self, min_levels: i32) -> Array<VarDictionary> {
        // Explicit levels first, then the raised cells of this terrain's PixelWater nodes.
        let mut map: pt_core::waterfall::WaterMap = self.store.water_cells().into_iter().collect();
        let mut sea: Option<f32> = None;
        let ty = self.base().get_global_position().y;
        for w in self.owned_waters() {
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    map.entry(iv2(v.x.round() as i32, v.y.round() as i32)).or_insert(v.z);
                }
            }
            if let Ok(n) = w.try_cast::<Node3D>()
                && n.is_visible_in_tree()
            {
                let y = n.get_global_position().y - ty;
                sea = Some(sea.map_or(y, |s: f32| s.max(y)));
            }
        }
        let p = pt_core::waterfall::FallParams { min_drop: self.level_height_safe().abs() * min_levels.max(1) as f32, sea, ..Default::default() };
        let mut out = Array::new();
        for f in pt_core::waterfall::find_falls(&self.store, &map, &p) {
            let mut d = VarDictionary::new();
            d.set("cell", from_iv(f.cell));
            d.set("neighbor", from_iv(f.neighbor));
            d.set("top", f.top);
            d.set("bottom", f.bottom);
            d.set("waterfall", f.waterfall);
            d.set("into_water", f.into_water);
            out.push(&d);
        }
        out
    }

    // ---- map images (game shaders / simulation) ----

    /// Rectangle of cells holding the hex disk of `radius` around `cell` (axial square on hex
    /// rings), for the `get_*_map` calls.
    #[func]
    pub fn get_map_rect_around(&self, cell: Vector2i, radius: i32) -> Rect2i {
        window_rect(hexmap::window_around(self.grid(), to_iv(cell), radius))
    }

    /// Rectangle of every stored cell plus a border of `pad` cells.
    #[func]
    pub fn get_map_rect_of_chunks(&self, pad: i32) -> Rect2i {
        window_rect(hexmap::chunks_window(&self.store, pad.max(0)))
    }

    /// Combined cell map (RGBA float): R height, G water level (-10000 = dry), B floor slot =
    /// biome = `textures` array slice (15 void), A wall slot + 16 · grass (-1 = no cell).
    #[func]
    pub fn get_cell_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        self.map_image(rect)
    }

    /// Heights (R float), `missing` where no chunk stores the cell.
    #[func]
    pub fn get_height_map(&self, rect: Rect2i, missing: f32) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.is_empty() || w.len() > MAX_TEXELS {
            return None;
        }
        float_image(w, &hexmap::height_map(&self.store, w, missing))
    }

    /// Water surface levels (R float, -10000 = dry), see `get_cell_water_level`.
    #[func]
    pub fn get_water_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.is_empty() || w.len() > MAX_TEXELS {
            return None;
        }
        let water = self.water_source();
        float_image(w, &w.cells().map(|c| water(c).unwrap_or(NO_WATER)).collect::<Vec<_>>())
    }

    /// Dominant biome = floor slot per cell (R8, value = slot, 255 = no cell).
    #[func]
    pub fn get_biome_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.is_empty() || w.len() > MAX_TEXELS {
            return None;
        }
        Image::create_from_data(w.size.x, w.size.y, false, Format::R8, &PackedByteArray::from(hexmap::biome_map(&self.store, w).as_slice()))
    }

    /// Uploads heights: texel `(x, y)` of `image` (channel R × `scale`) goes to cell
    /// `origin + (x, y)` where a chunk stores it. Float images carry heights directly; 8/16-bit
    /// images read 0..1 (e.g. `scale = 255 · level_height` for an L8 level map). One remesh.
    #[func]
    pub fn set_height_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>, scale: f32) {
        let Some(img) = image else { return };
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let vals: Vec<f32> = v.into_iter().map(|x| x * scale).collect();
        let t = hexmap::apply_heights(&mut self.store, w, &vals);
        self.apply_map_touch(t);
    }

    /// Uploads water levels (channel R × `scale`; values ≤ -9999 clear), like `set_height_map`.
    #[func]
    pub fn set_water_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>, scale: f32) {
        let Some(img) = image else { return };
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let items = v.into_iter().enumerate().map(|(i, x)| (w.cell(i), Some(x * scale))).collect();
        self.write_water(items);
    }

    /// Uploads biomes = floor slots (float images: the slot; 8-bit images: R × 255). Values
    /// above 15 are skipped. One remesh.
    #[func]
    pub fn set_biome_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>) {
        let Some(img) = image else { return };
        let unorm = is_unorm(img.get_format());
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let vals: Vec<u8> = v.into_iter().map(|x| if unorm { (x * 255.0).round() } else { x.round() }.clamp(0.0, 255.0) as u8).collect();
        let t = hexmap::apply_biomes(&mut self.store, w, &vals);
        self.apply_map_touch(t);
    }

    // ---- map window around the player ----

    /// Cell rectangle of the map window texture (`map_window_radius`).
    #[func]
    pub fn get_map_window(&self) -> Rect2i {
        window_rect(self.hexmaps.window)
    }

    /// The map window texture (RGBA float, `get_cell_map` layout; null while
    /// `map_window_radius` is 0). Sample it with `texelFetch(tex, cell - window.position, 0)`.
    #[func]
    pub fn get_map_texture(&self) -> Option<Gd<Texture2D>> {
        self.hexmaps.texture.clone().map(|t| t.upcast())
    }

    /// Recentres the map window on a world position now (rebuilding its texture); returns
    /// whether it moved or was rebuilt.
    #[func]
    pub fn update_map_window_at(&mut self, world: Vector3) -> bool {
        if self.map_window_radius <= 0 {
            return false;
        }
        let l = self.global_to_local(world);
        let c = self.store.config.world_to_sample(v2(l.x, l.z));
        self.map_window_at(c, true)
    }

    /// The column map the instanced prism columns read (null unless `hex_edge_instanced` is
    /// active): 6 RGBA float texels per cell of `get_column_map_rect()`, texel `(x·6 + k, y)`
    /// (see `pt_core::prism::column_texels`; texel 0 = height, floor, wall + 16·grass, bevel).
    #[func]
    pub fn get_column_map_texture(&self) -> Option<Gd<Texture2D>> {
        self.columns_map_texture().map(|t| t.upcast())
    }

    /// CPU copy of the column map (same texels as `get_column_map_texture`).
    #[func]
    pub fn get_column_map(&self) -> Option<Gd<Image>> {
        self.columns_map_image()
    }

    #[func]
    pub fn get_column_map_rect(&self) -> Rect2i {
        window_rect(self.columns_map_window())
    }

    /// Instanced prism column instances drawn (0 when meshed).
    #[func]
    pub fn get_column_instance_count(&self) -> i64 {
        self.columns_instance_count()
    }

    // ---- small hexes: top decorations ----

    /// Small hexes (kg_hex `SmallHex`) of the decoration spots of `cell`, in the order of
    /// `get_cell_decoration_spots(cell, min_height)`.
    #[func]
    pub fn get_cell_decoration_hexes(&self, cell: Vector2i, min_height: f32) -> Array<Vector2i> {
        self.decoration_spots_local(to_iv(cell), min_height).into_iter().map(|(s, _)| from_iv(s)).collect()
    }

    /// Marks small hexes as paved / built on (`blocked = true`) or free: blocked small hexes
    /// get no top decorations, and scatter props standing on them are removed.
    #[func]
    pub fn set_small_hexes_blocked(&mut self, smalls: Array<Vector2i>, blocked: bool) {
        let list: Vec<IVec2> = smalls.iter_shared().map(to_iv).collect();
        for s in &list {
            if blocked {
                self.hexmaps.blocked.insert(*s);
            } else {
                self.hexmaps.blocked.remove(s);
            }
        }
        if blocked && !list.is_empty() {
            let cfg = self.store.config.clone();
            let (mut lo, mut hi) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
            for s in &list {
                let p = cfg.small_hex_pos(*s);
                lo = v2(lo.x.min(p.x), lo.y.min(p.y));
                hi = v2(hi.x.max(p.x), hi.y.max(p.y));
            }
            let pad = v2(cfg.cell_size_safe().x, cfg.cell_size_safe().x);
            let set: HashSet<IVec2> = list.into_iter().collect();
            self.scatter_remove_on_small_hexes(lo - pad, hi + pad, &|s| set.contains(&s));
        }
    }

    #[func]
    pub fn is_small_hex_blocked(&self, small: Vector2i) -> bool {
        self.hexmaps.blocked.contains(&to_iv(small))
    }

    #[func]
    pub fn clear_blocked_small_hexes(&mut self) {
        self.hexmaps.blocked.clear();
    }
}
