use super::MarchingSquaresTerrain;
use crate::math_conversions::{from_iv, to_iv};
use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, Texture2D};
use godot::prelude::*;
use pt_core::brush::Touched;
use pt_core::hexmap::{self, MapWindow, NO_WATER};
use pt_core::math::{IVec2, Vec3 as PVec3, iv2, v2};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct HexMapState {
    pub blocked: HashSet<IVec2>,
    window: MapWindow,
    texture: Option<Gd<ImageTexture>>,
    timer: f64,
    pub dirty: bool,
}
fn rect_window(r: Rect2i) -> MapWindow {
    MapWindow::new(iv2(r.position.x, r.position.y), iv2(r.size.x, r.size.y))
}
fn window_rect(w: MapWindow) -> Rect2i {
    Rect2i::new(
        Vector2i::new(w.origin.x, w.origin.y),
        Vector2i::new(w.size.x, w.size.y),
    )
}
const MAX_TEXELS: usize = 4096 * 4096;
fn float_image(w: MapWindow, values: &[f32]) -> Option<Gd<Image>> {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    Image::create_from_data(
        w.size.x,
        w.size.y,
        false,
        Format::RF,
        &PackedByteArray::from(bytes.as_slice()),
    )
}
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
    !matches!(
        f,
        Format::RF
            | Format::RGF
            | Format::RGBF
            | Format::RGBAF
            | Format::RH
            | Format::RGH
            | Format::RGBH
            | Format::RGBAH
    )
}
impl MarchingSquaresTerrain {
    pub(crate) fn assigned_waters(&self) -> Vec<Gd<crate::effects::water::PixelWater>> {
        self.waters
            .iter_shared()
            .flatten()
            .filter(|w| w.is_instance_valid())
            .collect()
    }
    pub(super) fn owned_waters(&self) -> Vec<Gd<crate::effects::water::PixelWater>> {
        if !self.base().is_inside_tree() {
            return Vec::new();
        }
        self.assigned_waters()
    }
    pub(super) fn water_source(&self) -> impl Fn(IVec2) -> Option<f32> + '_ {
        let mut raised: HashMap<IVec2, f32> = HashMap::new();
        let mut sea: Option<f32> = None;
        let ty = if self.base().is_inside_tree() {
            self.base().get_global_position().y
        } else {
            0.0
        };
        for w in self.owned_waters() {
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    let e = raised
                        .entry(
                            self.store
                                .config
                                .wrap_sample(iv2(v.x.round() as i32, v.y.round() as i32)),
                        )
                        .or_insert(v.z);
                    *e = e.max(v.z);
                }
            }
            let n = w.clone().upcast::<Node3D>();
            if n.is_visible_in_tree() {
                let y = n.get_global_position().y - ty;
                sea = Some(sea.map_or(y, |s: f32| s.max(y)));
            }
        }
        move |c: IVec2| {
            let k = self.store.config.wrap_sample(c);
            if let Some(w) = self.store.water(c).or_else(|| raised.get(&k).copied()) {
                return Some(w);
            }
            let h = self.store.stature(c)?;
            sea.filter(|s| *s > h)
        }
    }
    pub(crate) fn decoration_spots_local(
        &self,
        cell: IVec2,
        min_height: f32,
    ) -> Vec<(IVec2, PVec3)> {
        let water = self.store.water(cell);
        let blocked = &self.hexmaps.blocked;
        pt_core::cells::decoration_spots_ex(&self.store, cell, min_height, &|small, p| {
            blocked.contains(&small) || water.is_some_and(|w| p.y <= w)
        })
    }
    fn map_image(&self, r: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(r);
        if w.check_vacant() || w.len() > MAX_TEXELS {
            godot_warn!("MarchingSquaresTerrain: map rect {r:?} is empty or larger than 4096²");
            return None;
        }
        let water = self.water_source();
        super::hex_columns::rgbaf_image(w, &hexmap::build_cell_map(&self.store, w, &|c| water(c)))
    }
    fn map_focus_cell(&self) -> Option<IVec2> {
        let p = match self
            .map_window_focus
            .as_ref()
            .filter(|f| f.is_instance_valid() && f.is_inside_tree())
        {
            Some(f) => f.get_global_position(),
            None => self.lod_focus_point()?,
        };
        let l = self.global_to_local(p);
        Some(self.store.config.world_to_sample(v2(l.x, l.z)))
    }
    pub(super) fn update_map_window(&mut self, delta: f64) {
        if self.map_window_radius <= 0 || !self.grid().is_cell_mode() {
            return;
        }
        self.hexmaps.timer -= delta;
        if self.hexmaps.timer > 0.0 {
            return;
        }
        self.hexmaps.timer = self.map_window_refresh_interval;
        if let Some(c) = self.map_focus_cell() {
            self.map_window_at(c, false);
        }
    }
    fn map_window_at(&mut self, center: IVec2, force: bool) -> bool {
        let r = self.map_window_radius.max(1);
        let margin = self.map_window_margin.clamp(0, r);
        let moved = force
            || hexmap::needs_recenter(self.hexmaps.window, center, margin)
            || self.hexmaps.window.size.x != 2 * r + 1;
        if !moved && !self.hexmaps.dirty {
            return false;
        }
        let w = if moved {
            hexmap::window_around(self.grid(), center, r)
        } else {
            self.hexmaps.window
        };
        let Some(img) = self.map_image(window_rect(w)) else {
            return false;
        };
        match &mut self.hexmaps.texture {
            Some(t) if self.hexmaps.window.size == w.size => t.update(&img),
            _ => self.hexmaps.texture = ImageTexture::create_from_image(&img),
        }
        self.hexmaps.window = w;
        self.hexmaps.dirty = false;
        self.signals().map_window_changed().emit(window_rect(w));
        true
    }
    fn write_water(&mut self, items: Vec<(IVec2, Option<f32>)>) {
        let mut touched = Touched::new();
        let mut owners: Vec<IVec2> = Vec::new();
        for (c, w) in items {
            if self.store.water(c) == w.filter(|w| pt_core::terrain_store::has_water(*w)) {
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
    #[func]
    pub fn set_cell_water_level(&mut self, cell: Vector2i, level: f32) {
        self.write_water(vec![(to_iv(cell), Some(level))]);
    }
    #[func]
    pub fn set_cell_water_levels(&mut self, cells: Array<Vector2i>, levels: PackedFloat32Array) {
        let items = cells
            .iter_shared()
            .zip(levels.as_slice().iter().copied())
            .map(|(c, l)| (to_iv(c), Some(l)))
            .collect();
        self.write_water(items);
    }
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
        let items = self
            .store
            .water_cells()
            .into_iter()
            .map(|(c, _)| (c, None))
            .collect();
        self.write_water(items);
    }
    #[func]
    pub fn get_raised_water_cells(&self) -> PackedVector3Array {
        self.store
            .water_cells()
            .into_iter()
            .filter(|(c, w)| self.store.stature(*c).is_some_and(|h| *w > h + 1e-4))
            .map(|(c, w)| Vector3::new(c.x as f32, c.y as f32, w))
            .collect()
    }
    #[func]
    pub fn get_water_cells(&self) -> PackedVector3Array {
        self.store
            .water_cells()
            .into_iter()
            .map(|(c, w)| Vector3::new(c.x as f32, c.y as f32, w))
            .collect()
    }
    #[func]
    pub fn get_cell_water_level(&self, cell: Vector2i) -> f32 {
        self.water_source()(to_iv(cell)).unwrap_or(NO_WATER)
    }
    #[func]
    pub fn get_waterfalls(&self, min_levels: i32) -> Array<VarDictionary> {
        let mut map: pt_core::waterfall::WaterMap = self.store.water_cells().into_iter().collect();
        let mut sea: Option<f32> = None;
        let ty = self.base().get_global_position().y;
        for w in self.owned_waters() {
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    map.entry(iv2(v.x.round() as i32, v.y.round() as i32))
                        .or_insert(v.z);
                }
            }
            let n = w.upcast::<Node3D>();
            if n.is_visible_in_tree() {
                let y = n.get_global_position().y - ty;
                sea = Some(sea.map_or(y, |s: f32| s.max(y)));
            }
        }
        let p = pt_core::waterfall::FallParams {
            min_drop: self.level_height_safe().abs() * min_levels.max(1) as f32,
            sea,
            ..Default::default()
        };
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
    #[func]
    pub fn get_map_rect_around(&self, cell: Vector2i, radius: i32) -> Rect2i {
        window_rect(hexmap::window_around(self.grid(), to_iv(cell), radius))
    }
    #[func]
    pub fn get_map_rect_of_chunks(&self, pad: i32) -> Rect2i {
        window_rect(hexmap::chunks_window(&self.store, pad.max(0)))
    }
    #[func]
    pub fn get_cell_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        self.map_image(rect)
    }
    #[func]
    pub fn get_height_map(&self, rect: Rect2i, missing: f32) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.check_vacant() || w.len() > MAX_TEXELS {
            return None;
        }
        float_image(w, &hexmap::height_map(&self.store, w, missing))
    }
    #[func]
    pub fn get_water_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.check_vacant() || w.len() > MAX_TEXELS {
            return None;
        }
        let water = self.water_source();
        float_image(
            w,
            &w.tiles()
                .map(|c| water(c).unwrap_or(NO_WATER))
                .collect::<Vec<_>>(),
        )
    }
    #[func]
    pub fn get_biome_map(&self, rect: Rect2i) -> Option<Gd<Image>> {
        let w = rect_window(rect);
        if w.check_vacant() || w.len() > MAX_TEXELS {
            return None;
        }
        Image::create_from_data(
            w.size.x,
            w.size.y,
            false,
            Format::R8,
            &PackedByteArray::from(hexmap::biome_map(&self.store, w).as_slice()),
        )
    }
    #[func]
    pub fn set_height_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>, scale: f32) {
        let Some(img) = image else { return };
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let vals: Vec<f32> = v.into_iter().map(|x| x * scale).collect();
        let t = hexmap::apply_heights(&mut self.store, w, &vals);
        self.apply_map_touch(t);
    }
    #[func]
    pub fn set_water_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>, scale: f32) {
        let Some(img) = image else { return };
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let items = v
            .into_iter()
            .enumerate()
            .map(|(i, x)| (w.cell(i), Some(x * scale)))
            .collect();
        self.write_water(items);
    }
    #[func]
    pub fn set_biome_map(&mut self, origin: Vector2i, image: Option<Gd<Image>>) {
        let Some(img) = image else { return };
        let unorm = is_unorm(img.get_format());
        let (size, v) = image_values(&img);
        let w = MapWindow::new(to_iv(origin), size);
        let vals: Vec<u8> = v
            .into_iter()
            .map(|x| {
                if unorm {
                    (x * 255.0).round()
                } else {
                    x.round()
                }
                .clamp(0.0, 255.0) as u8
            })
            .collect();
        let t = hexmap::apply_biomes(&mut self.store, w, &vals);
        self.apply_map_touch(t);
    }
    #[func]
    pub fn get_map_window(&self) -> Rect2i {
        window_rect(self.hexmaps.window)
    }
    #[func]
    pub fn get_map_texture(&self) -> Option<Gd<Texture2D>> {
        self.hexmaps.texture.clone().map(|t| t.upcast())
    }
    #[func]
    pub fn update_map_window_at(&mut self, world: Vector3) -> bool {
        if self.map_window_radius <= 0 {
            return false;
        }
        let l = self.global_to_local(world);
        let c = self.store.config.world_to_sample(v2(l.x, l.z));
        self.map_window_at(c, true)
    }
    #[func]
    pub fn get_column_map_texture(&self) -> Option<Gd<Texture2D>> {
        self.columns_map_texture().map(|t| t.upcast())
    }
    #[func]
    pub fn get_column_map(&self) -> Option<Gd<Image>> {
        self.columns_map_image()
    }
    #[func]
    pub fn get_column_map_rect(&self) -> Rect2i {
        window_rect(self.columns_map_window())
    }
    #[func]
    pub fn get_column_instance_count(&self) -> i64 {
        self.columns_instance_count()
    }
    #[func]
    pub fn get_cell_decoration_hexes(&self, cell: Vector2i, min_height: f32) -> Array<Vector2i> {
        self.decoration_spots_local(to_iv(cell), min_height)
            .into_iter()
            .map(|(s, _)| from_iv(s))
            .collect()
    }
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
