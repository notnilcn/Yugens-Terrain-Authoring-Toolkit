//! `PixelOutlinePalette` — per-outline-id colors (256 ids x exterior/interior), plus the
//! runtime override registry `PixelObject.override_outline_color` writes into.
//!
//! The renderer bakes the palette into a 256 x 2 RGBA8 texture (row 0 exterior/silhouette,
//! row 1 interior = outline-id edges between touching surfaces) that the resolve pass samples
//! with the outline id from the id pass. Alpha is the row opacity: `mix(scene, rgb, a)`;
//! alpha 0 keeps the renderer's automatic darkening (`outline_darken`).
//! Ids 254 (hover) and 255 (selection) are the highlight rows.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use godot::classes::image::Format;
use godot::classes::{IResource, Image};
use godot::prelude::*;
use pt_core::meta::{HOVER_ID, OUTLINE_IDS, SELECT_ID};

pub const ROWS: usize = OUTLINE_IDS as usize;
pub const HOVER_COLOR: Color = Color::from_rgba(1.0, 0.93, 0.35, 1.0);
pub const SELECT_COLOR: Color = Color::from_rgba(1.0, 0.55, 0.12, 1.0);

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelOutlinePalette {
    /// Silhouette color per outline id (alpha = opacity, 0 = darken the scene color).
    #[export]
    #[var(set = set_exterior_colors)]
    exterior_colors: PackedColorArray,
    /// Interior color per outline id: drawn where this id touches a different id.
    #[export]
    #[var(set = set_interior_colors)]
    interior_colors: PackedColorArray,
    /// Modulate instead of replace (the ProPixelizer `_EdgeHighlightColor` convention): the
    /// scene color becomes `mix(scene, scene * rgb * 2, alpha)`, so rows darker than 0.5
    /// darken and brighter rows brighten; 0.5 is neutral. Alpha is still the opacity.
    #[export]
    #[var(set = set_multiply_colors)]
    multiply_colors: bool,
    base: Base<Resource>,
}

fn default_rows() -> PackedColorArray {
    let mut a = PackedColorArray::new();
    for i in 0..ROWS as u32 {
        a.push(match i {
            HOVER_ID => HOVER_COLOR,
            SELECT_ID => SELECT_COLOR,
            _ => Color::from_rgba(0.0, 0.0, 0.0, 0.0),
        });
    }
    a
}

/// Default (exterior, interior) rows: darken everywhere, highlight colors on 254/255.
pub fn default_rows_vec() -> (Vec<Color>, Vec<Color>) {
    let r: Vec<Color> = default_rows().as_slice().to_vec();
    (r.clone(), r)
}

#[godot_api]
impl IResource for PixelOutlinePalette {
    fn init(base: Base<Resource>) -> Self {
        PixelOutlinePalette { exterior_colors: default_rows(), interior_colors: default_rows(), multiply_colors: false, base }
    }
}

impl PixelOutlinePalette {
    pub fn is_multiply(&self) -> bool {
        self.multiply_colors
    }

    pub fn rows(&self) -> (Vec<Color>, Vec<Color>) {
        let get = |a: &PackedColorArray, i: usize| a.get(i).unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
        ((0..ROWS).map(|i| get(&self.exterior_colors, i)).collect(), (0..ROWS).map(|i| get(&self.interior_colors, i)).collect())
    }
}

#[godot_api]
impl PixelOutlinePalette {
    #[func]
    fn set_exterior_colors(&mut self, v: PackedColorArray) {
        self.exterior_colors = v;
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_interior_colors(&mut self, v: PackedColorArray) {
        self.interior_colors = v;
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_multiply_colors(&mut self, v: bool) {
        self.multiply_colors = v;
        bump();
        self.base_mut().emit_changed();
    }

    /// Sets both rows of one outline id.
    #[func]
    pub fn set_entry(&mut self, id: i32, exterior: Color, interior: Color) {
        if !(0..ROWS as i32).contains(&id) {
            return;
        }
        for (arr, c) in [(&mut self.exterior_colors, exterior), (&mut self.interior_colors, interior)] {
            if arr.len() < ROWS {
                arr.resize(ROWS);
            }
            arr.as_mut_slice()[id as usize] = c;
        }
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn get_exterior(&self, id: i32) -> Color {
        self.exterior_colors.get(id.max(0) as usize).unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0))
    }

    #[func]
    fn get_interior(&self, id: i32) -> Color {
        self.interior_colors.get(id.max(0) as usize).unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0))
    }

    /// The 256 x 2 image the renderer uploads (row 0 exterior, row 1 interior).
    #[func]
    fn bake_image(&self) -> Option<Gd<Image>> {
        let (e, i) = self.rows();
        bake(&e, &i, &BTreeMap::new())
    }
}

// ---- runtime overrides ---------------------------------------------------------------------

/// id -> (exterior, interior) registered by `PixelObject.override_outline_color`.
static OVERRIDES: Mutex<BTreeMap<u32, (Color, Color)>> = Mutex::new(BTreeMap::new());
/// Bumped on every palette/override change; the renderer re-bakes when it differs.
static VERSION: AtomicU64 = AtomicU64::new(1);

pub fn bump() {
    VERSION.fetch_add(1, Ordering::Relaxed);
}

pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

pub fn set_override(id: u32, exterior: Color, interior: Color) {
    let mut m = OVERRIDES.lock().unwrap();
    if m.get(&id) != Some(&(exterior, interior)) {
        m.insert(id, (exterior, interior));
        bump();
    }
}

pub fn clear_override(id: u32) {
    if OVERRIDES.lock().unwrap().remove(&id).is_some() {
        bump();
    }
}

pub fn overrides() -> BTreeMap<u32, (Color, Color)> {
    OVERRIDES.lock().unwrap().clone()
}

/// Bakes rows + overrides into the 256 x 2 RGBA8 palette image.
pub fn bake(exterior: &[Color], interior: &[Color], over: &BTreeMap<u32, (Color, Color)>) -> Option<Gd<Image>> {
    let mut bytes = vec![0u8; ROWS * 2 * 4];
    let to8 = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    for row in 0..2 {
        for id in 0..ROWS {
            let mut c = if row == 0 { exterior.get(id) } else { interior.get(id) }.copied().unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
            if let Some((e, i)) = over.get(&(id as u32)) {
                c = if row == 0 { *e } else { *i };
            }
            let o = (row * ROWS + id) * 4;
            bytes[o..o + 4].copy_from_slice(&[to8(c.r), to8(c.g), to8(c.b), to8(c.a)]);
        }
    }
    Image::create_from_data(ROWS as i32, 2, false, Format::RGBA8, &PackedByteArray::from(bytes.as_slice()))
}
