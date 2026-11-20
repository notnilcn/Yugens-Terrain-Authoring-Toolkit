//! `PixelOutlineColors` — per-outline-id colours (256 ids x silhouette/edge rows), plus the
//! runtime override registry `PixelObject.override_outline_color` writes into.
//!
//! The silhouette / edge split takes its words from the `outline_color` / `edge_color` uniforms of
//! `3d-pixel-art-in-godot`'s `outline_and_edge.gdshader`.
//!
//! The renderer bakes the rows into a 256 x 2 RGBA8 image (row 0 silhouette, row 1 edge =
//! outline-id boundaries between touching surfaces) that the resolve pass samples with the
//! outline id from the id pass. Alpha is the row opacity: `mix(scene, rgb, a)`; alpha 0 keeps the
//! renderer's automatic darkening (`outline_darken`).
//! Ids 254 (hover) and 255 (selection) are the highlight rows.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use godot::classes::image::Format;
use godot::classes::{IResource, Image};
use godot::prelude::*;
use pt_core::meta::{HOVER_ID, OUTLINE_IDS, SELECT_ID};

pub const PALETTE_ROWS: usize = OUTLINE_IDS as usize;
/// Neutral row value of the modulate mode: there the pass scales the scene colour by twice the
/// row's RGB, so a row this close to mid grey leaves the scene alone (darker rows shade, brighter
/// rows lift).
pub const MODULATE_NEUTRAL: f32 = 0.5004;
pub const HOVER_COLOR: Color = Color::from_rgba(1.0, 0.9294, 0.3504, 1.0);
pub const SELECT_COLOR: Color = Color::from_rgba(1.0, 0.5544, 0.1204, 1.0);

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelOutlineColors {
    /// Silhouette colour per outline id (alpha = opacity, 0 = darken the scene colour).
    #[export]
    #[var(set = set_silhouette_colors)]
    silhouette_colors: PackedColorArray,
    /// Edge colour per outline id: drawn where this id touches a different id.
    #[export]
    #[var(set = set_edge_colors)]
    edge_colors: PackedColorArray,
    /// Modulate instead of replace: the scene colour becomes `mix(scene, scene * rgb * 2, alpha)`,
    /// so rows around 0.5004 leave it alone, darker rows shade and brighter rows lift. Alpha is
    /// still the opacity.
    #[export]
    #[var(set = set_modulate_rows)]
    modulate_rows: bool,
    base: Base<Resource>,
}

fn fallback_rank_tints_seq() -> PackedColorArray {
    let mut a = PackedColorArray::new();
    for i in 0..PALETTE_ROWS as u32 {
        a.push(match i {
            HOVER_ID => HOVER_COLOR,
            SELECT_ID => SELECT_COLOR,
            _ => Color::from_rgba(MODULATE_NEUTRAL, MODULATE_NEUTRAL, MODULATE_NEUTRAL, 0.0),
        });
    }
    a
}

/// Default (silhouette, edge) rows: darken everywhere, highlight colours on 254/255.
pub fn fallback_rank_tints_vector() -> (Vec<Color>, Vec<Color>) {
    let r: Vec<Color> = fallback_rank_tints_seq().as_slice().to_vec();
    (r.clone(), r)
}

#[godot_api]
impl IResource for PixelOutlineColors {
    fn init(base: Base<Resource>) -> Self {
        PixelOutlineColors {
            silhouette_colors: fallback_rank_tints_seq(),
            edge_colors: fallback_rank_tints_seq(),
            modulate_rows: false,
            base,
        }
    }
}

impl PixelOutlineColors {
    pub fn is_modulate(&self) -> bool {
        self.modulate_rows
    }

    pub fn row_colors(&self) -> (Vec<Color>, Vec<Color>) {
        let get = |a: &PackedColorArray, i: usize| {
            a.get(i).unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0))
        };
        (
            (0..PALETTE_ROWS)
                .map(|i| get(&self.silhouette_colors, i))
                .collect(),
            (0..PALETTE_ROWS)
                .map(|i| get(&self.edge_colors, i))
                .collect(),
        )
    }
}

#[godot_api]
impl PixelOutlineColors {
    #[func]
    fn set_silhouette_colors(&mut self, v: PackedColorArray) {
        self.silhouette_colors = v;
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_edge_colors(&mut self, v: PackedColorArray) {
        self.edge_colors = v;
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn set_modulate_rows(&mut self, v: bool) {
        self.modulate_rows = v;
        bump();
        self.base_mut().emit_changed();
    }

    /// Sets both rows of one outline id.
    #[func]
    pub fn set_id_colors(&mut self, token_id: i32, silhouette: Color, edge: Color) {
        if !(0..PALETTE_ROWS as i32).contains(&token_id) {
            return;
        }
        for (arr, c) in [
            (&mut self.silhouette_colors, silhouette),
            (&mut self.edge_colors, edge),
        ] {
            if arr.len() < PALETTE_ROWS {
                arr.resize(PALETTE_ROWS);
            }
            arr.as_mut_slice()[token_id as usize] = c;
        }
        bump();
        self.base_mut().emit_changed();
    }

    #[func]
    fn get_silhouette(&self, id: i32) -> Color {
        self.silhouette_colors
            .get(id.max(0) as usize)
            .unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0))
    }

    #[func]
    fn fetch_ridge(&self, id: i32) -> Color {
        self.edge_colors
            .get(id.max(0) as usize)
            .unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0))
    }

    /// The 256 x 2 image the renderer uploads (row 0 silhouette, row 1 edge).
    #[func]
    fn bake_image(&self) -> Option<Gd<Image>> {
        let (s, e) = self.row_colors();
        bake_rows_image(&s, &e, &BTreeMap::new())
    }
}

// ---- runtime overrides ---------------------------------------------------------------------

/// id -> (silhouette, edge) registered by `PixelObject.override_outline_color`.
static OVERRIDES: Mutex<BTreeMap<u32, (Color, Color)>> = Mutex::new(BTreeMap::new());
/// Bumped on every colour/override change; the renderer re-bakes when it differs.
static VERSION: AtomicU64 = AtomicU64::new(1);

pub fn bump() {
    VERSION.fetch_add(1, Ordering::Relaxed);
}

pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

pub fn set_override(token_id: u32, silhouette: Color, edge: Color) {
    let mut m = OVERRIDES.lock().unwrap();
    if m.get(&token_id) != Some(&(silhouette, edge)) {
        m.insert(token_id, (silhouette, edge));
        bump();
    }
}

pub fn clear_override(token_id: u32) {
    if OVERRIDES.lock().unwrap().remove(&token_id).is_some() {
        bump();
    }
}

pub fn overrides() -> BTreeMap<u32, (Color, Color)> {
    OVERRIDES.lock().unwrap().clone()
}

/// Bakes rows + overrides into the 256 x 2 RGBA8 image.
pub fn bake_rows_image(
    silhouette: &[Color],
    edge: &[Color],
    over: &BTreeMap<u32, (Color, Color)>,
) -> Option<Gd<Image>> {
    let mut bytes = vec![0u8; PALETTE_ROWS * 2 * 4];
    let to8 = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    for row in 0..2 {
        let colors = if row == 0 { silhouette } else { edge };
        for id in 0..PALETTE_ROWS {
            let mut c = colors
                .get(id)
                .copied()
                .unwrap_or(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
            if let Some((s, e)) = over.get(&(id as u32)) {
                c = if row == 0 { *s } else { *e };
            }
            let o = (row * PALETTE_ROWS + id) * 4;
            bytes[o..o + 4].copy_from_slice(&[to8(c.r), to8(c.g), to8(c.b), to8(c.a)]);
        }
    }
    Image::create_from_data(
        PALETTE_ROWS as i32,
        2,
        false,
        Format::RGBA8,
        &PackedByteArray::from(bytes.as_slice()),
    )
}
