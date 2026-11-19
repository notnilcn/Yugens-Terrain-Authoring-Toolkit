//! Xogot Modeler paint features on the builder's `BMesh` (docs/port/06-xogot.md, package P4):
//! built-in material presets, smoothing-group helpers, the material / colour palette with its
//! JSON form, and sample / match helpers. Ports of xogot-rust (MIT, Xibbon Inc. / Xogot Modeler):
//! `editor_core/src/material_kind.rs` (kinds, references, default slots, palette validation),
//! `xogot_modeler/src/shared/material_presets.rs` (generated materials and pattern textures,
//! palette JSON / `.tres` keys), `editor_core/src/editor_state.rs` (`set_shading_intent`,
//! `create_smoothing_group_from_selection`, `next_free_smoothing_group_index`),
//! `editor_core/src/attributes_state.rs` (`ModelerPalette`, golden-angle group tints) and
//! `xogot_modeler/src/shared/attributes.rs` (merge / clear groups, match material / colour /
//! smoothing).
//!
//! Engine-free: the Godot side builds `StandardMaterial3D`s from [`PresetMaterial`] and
//! `ImageTexture`s from [`pattern_rgba8`].
//!
//! Differences from xogot-rust (builder model, gap fixes):
//! * Palettes hold any number of materials and colours (the builder has its own swatch count);
//!   [`Palette::to_xogot_json`] pads / truncates to Xogot's 10 colours / ≥ 10 materials so the
//!   file still loads in Xogot, and [`Palette::from_json`] reads either form.
//! * Merge / clear accept any group (builder bevels number groups past 23); only *new* groups
//!   are limited to 1..=23 like Xogot.
//! * Match helpers return the most common value of the selection, not the first record's.

use std::collections::{BTreeMap, BTreeSet};

use super::model::BMesh;
use crate::math::hash_u32;

// ================================================================== built-in materials

/// A built-in material (port of `ModelerMaterialKind`, declaration order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Preset {
    Grid,
    Checker,
    NoDraw,
    Collider,
    Trigger,
}

/// Prefix of a built-in material reference (`modeler-builtin:grid`, ...).
pub const BUILTIN_PREFIX: &str = "modeler-builtin:";
/// Prefix of a material stored inside a `.tres` palette (`modeler-palette:<path>:<index>`).
pub const PALETTE_PREFIX: &str = "modeler-palette:";
/// Side of the generated pattern textures, in pixels.
pub const PATTERN_SIZE: usize = 64;
/// Roughness of every generated material.
pub const GENERATED_ROUGHNESS: f32 = 0.85;

impl Preset {
    pub const ALL: [Preset; 5] = [Preset::Grid, Preset::Checker, Preset::NoDraw, Preset::Collider, Preset::Trigger];

    /// The Swift raw value (`grid`, `checker`, `noDraw`, `collider`, `trigger`).
    pub fn raw_value(self) -> &'static str {
        match self {
            Preset::Grid => "grid",
            Preset::Checker => "checker",
            Preset::NoDraw => "noDraw",
            Preset::Collider => "collider",
            Preset::Trigger => "trigger",
        }
    }

    pub fn from_raw_value(raw: &str) -> Option<Preset> {
        Self::ALL.into_iter().find(|k| k.raw_value() == raw)
    }

    /// The palette reference standing for this material: `modeler-builtin:<rawValue>`.
    pub fn reference(self) -> String {
        format!("{BUILTIN_PREFIX}{}", self.raw_value())
    }

    /// The built-in a reference names, if it is one.
    pub fn from_reference(reference: &str) -> Option<Preset> {
        reference.strip_prefix(BUILTIN_PREFIX).and_then(Preset::from_raw_value)
    }

    /// Display title.
    pub fn title(self) -> &'static str {
        match self {
            Preset::Grid => "Grid",
            Preset::Checker => "Checker",
            Preset::NoDraw => "No Draw",
            Preset::Collider => "Collider",
            Preset::Trigger => "Trigger",
        }
    }

    /// The generated material's settings (port of `generatedMaterial(_:)`).
    pub fn material(self) -> PresetMaterial {
        let mut m = PresetMaterial {
            name: format!("Modeler {}", self.title()),
            albedo: [1.0; 4],
            pattern: None,
            uv1_scale: [1.0, 1.0, 1.0],
            transparent: false,
            unshaded: false,
            cull_disabled: false,
            vertex_color: true,
            roughness: GENERATED_ROUGHNESS,
            texture_repeat: true,
            anisotropic_filter: true,
        };
        match self {
            Preset::NoDraw => {
                m.albedo = [1.0, 1.0, 1.0, 0.0];
                m.transparent = true;
                m.unshaded = true;
            }
            _ => {
                m.uv1_scale = [2.0, 2.0, 1.0];
                m.pattern = Some(if self == Preset::Checker { Pattern::Checker } else { Pattern::Grid });
                let tint = match self {
                    Preset::Collider => Some([0.2, 0.8, 0.4, 0.35]),
                    Preset::Trigger => Some([1.0, 0.65, 0.12, 0.35]),
                    _ => None,
                };
                if let Some(t) = tint {
                    m.albedo = t;
                    m.transparent = true;
                    m.unshaded = true;
                    m.cull_disabled = true;
                }
            }
        }
        m
    }
}

/// Generated pattern texture of a preset material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Grid,
    Checker,
}

impl Pattern {
    /// Texture resource name.
    pub fn name(self) -> &'static str {
        match self {
            Pattern::Grid => "Modeler Grid Pattern",
            Pattern::Checker => "Modeler Checker Pattern",
        }
    }
}

/// What a built-in material looks like; the Godot side maps each field onto a
/// `StandardMaterial3D` (albedo colour, albedo texture from [`pattern_rgba8`] with mipmaps,
/// `uv1_scale`, alpha transparency, unshaded, cull disabled, albedo from vertex colour,
/// roughness, texture repeat, linear-mipmap-anisotropic filter).
#[derive(Clone, Debug, PartialEq)]
pub struct PresetMaterial {
    pub name: String,
    /// Linear-ish albedo RGBA (Godot `Color`).
    pub albedo: [f32; 4],
    pub pattern: Option<Pattern>,
    pub uv1_scale: [f32; 3],
    pub transparent: bool,
    pub unshaded: bool,
    pub cull_disabled: bool,
    pub vertex_color: bool,
    pub roughness: f32,
    pub texture_repeat: bool,
    pub anisotropic_filter: bool,
}

/// Grey level of one pattern pixel (port of `pattern_pixel`): the grid is a 0xd7 field with
/// 0xaa lines every 16 px and a 0x69 border two pixels wide; the checker alternates 0xdc / 0x73
/// in 32 px squares.
pub fn pattern_pixel(pattern: Pattern, x: usize, y: usize) -> u8 {
    match pattern {
        Pattern::Grid => {
            if x > 1 && y > 1 {
                if x % 16 == 0 || y % 16 == 0 { 0xaa } else { 0xd7 }
            } else {
                0x69
            }
        }
        Pattern::Checker => {
            if ((x >> 5) + (y >> 5)) & 1 == 0 {
                0xdc
            } else {
                0x73
            }
        }
    }
}

/// The 64×64 opaque RGBA8 pixels of a pattern texture (port of `texture(checker:)`, which
/// builds RGB8; alpha is 255 here). Rows top to bottom.
pub fn pattern_rgba8(pattern: Pattern) -> Vec<u8> {
    let mut px = Vec::with_capacity(PATTERN_SIZE * PATTERN_SIZE * 4);
    for y in 0..PATTERN_SIZE {
        for x in 0..PATTERN_SIZE {
            let v = pattern_pixel(pattern, x, y);
            px.extend_from_slice(&[v, v, v, 255]);
        }
    }
    px
}

// ================================================================== smoothing groups

/// Highest smoothing group a new group may get (Xogot `MAX_SMOOTHING_GROUP`).
pub const MAX_NEW_SMOOTHING_GROUP: u16 = 23;

/// One smoothing group in use (port of `ModelerSmoothingGroupInfo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupInfo {
    pub index: u16,
    pub face_count: usize,
}

fn valid(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<usize> {
    faces.iter().copied().filter(|f| *f < m.faces.len()).collect()
}

/// Smoothing groups above 0 used by the mesh, ascending, with their face counts.
pub fn smoothing_groups(m: &BMesh) -> Vec<GroupInfo> {
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for f in &m.faces {
        if f.smooth > 0 {
            *counts.entry(f.smooth).or_default() += 1;
        }
    }
    counts.into_iter().map(|(index, face_count)| GroupInfo { index, face_count }).collect()
}

/// Lowest group in 1..=23 no face uses (port of `next_free_smoothing_group_index`).
pub fn next_free_group(m: &BMesh) -> Option<u16> {
    let used: BTreeSet<u16> = m.faces.iter().map(|f| f.smooth).collect();
    (1..=MAX_NEW_SMOOTHING_GROUP).find(|i| !used.contains(i))
}

/// Puts the faces in group `group` (0 = faceted).
pub fn set_group(m: &mut BMesh, faces: &BTreeSet<usize>, group: u16) {
    for f in valid(m, faces) {
        m.faces[f].smooth = group;
    }
}

/// *Smooth* (port of `set_shading_intent(.smooth)` + `preferred_smoothing_group`): the faces
/// join the first group above 0 in `chips` (the groups selected in the panel), else `panel`
/// when it is above 0, else the first free group. Returns the group used, `None` when no faces
/// are given or no group is free.
pub fn smooth_faces(m: &mut BMesh, faces: &BTreeSet<usize>, chips: &[u16], panel: u16) -> Option<u16> {
    if valid(m, faces).is_empty() {
        return None;
    }
    let group = chips.iter().copied().find(|g| *g > 0).or((panel > 0).then_some(panel)).or_else(|| next_free_group(m))?;
    set_group(m, faces, group);
    Some(group)
}

/// *Faceted* (port of `set_shading_intent(.faceted)`): the faces go to group 0.
pub fn facet_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    set_group(m, faces, 0);
}

/// *New group from selection* (port of `create_smoothing_group_from_selection`): the faces get
/// the next free group in 1..=23, which is returned; `None` without faces or a free group.
pub fn new_group_from_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    if valid(m, faces).is_empty() {
        return None;
    }
    let g = next_free_group(m)?;
    set_group(m, faces, g);
    Some(g)
}

/// Moves every face of group `source` into `destination` (port of
/// `merge_smoothing_group_core`): no-op when equal; source 0 or a source no face uses is an
/// error. Returns the number of faces moved.
pub fn merge_group(m: &mut BMesh, source: u16, destination: u16) -> Result<usize, String> {
    if source == destination {
        return Ok(0);
    }
    if source == 0 {
        return Err("Faceted faces (group 0) cannot be merged into a group.".into());
    }
    let mut n = 0;
    for f in &mut m.faces {
        if f.smooth == source {
            f.smooth = destination;
            n += 1;
        }
    }
    if n == 0 {
        return Err(format!("Smoothing group {source} does not exist."));
    }
    Ok(n)
}

/// Clears group `index`: its faces become faceted (port of `clear_smoothing_group`). Returns
/// the number of faces changed.
pub fn clear_group(m: &mut BMesh, index: u16) -> usize {
    if index == 0 {
        return 0;
    }
    merge_group(m, index, 0).unwrap_or(0)
}

/// Faces of group `index` (the chip's *select*).
pub fn faces_in_group(m: &BMesh, index: u16) -> BTreeSet<usize> {
    (0..m.faces.len()).filter(|f| m.faces[*f].smooth == index).collect()
}

/// Display tint of a smoothing group (port of `ModelerPalette::smoothing_group_color`): hues
/// spaced by the golden angle (137°), saturation 0.62, value 0.9; groups below 1 use group 1.
pub fn smoothing_group_color(index: u16) -> [f32; 4] {
    const SATURATION: f32 = 0.62;
    const BRIGHTNESS: f32 = 0.9;
    let index = (index as i64).max(1);
    let hue = ((index * 137) % 360) as f32 / 360.0;
    let scaled = hue * 6.0;
    let sector = (scaled as i64) % 6;
    let fraction = scaled - (scaled as i64) as f32;
    let p = BRIGHTNESS * (1.0 - SATURATION);
    let q = (1.0 - fraction * SATURATION) * BRIGHTNESS;
    let t = (1.0 - (1.0 - fraction) * SATURATION) * BRIGHTNESS;
    let [r, g, b] = match sector {
        0 => [BRIGHTNESS, t, p],
        1 => [q, BRIGHTNESS, p],
        2 => [p, BRIGHTNESS, t],
        3 => [p, q, BRIGHTNESS],
        4 => [t, p, BRIGHTNESS],
        _ => [BRIGHTNESS, p, q],
    };
    [r, g, b, 1.0]
}

// ================================================================== sample / match

/// Most common value (ties: the first seen).
fn most_common<T: Ord + Copy>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut counts: BTreeMap<T, (usize, usize)> = BTreeMap::new();
    for (i, v) in values.into_iter().enumerate() {
        let e = counts.entry(v).or_insert((0, i));
        e.0 += 1;
    }
    counts.into_iter().max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.1.1.cmp(&a.1.1))).map(|(v, _)| v)
}

/// Material slot used by most of the faces (port of `match_material`'s read).
pub fn sample_material(m: &BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    most_common(valid(m, faces).into_iter().map(|f| m.faces[f].material))
}

/// Colour shown by most corners of the faces (corner colours, else face colours; port of
/// `selected_color_core`).
pub fn sample_color(m: &BMesh, faces: &BTreeSet<usize>) -> Option<[u8; 4]> {
    most_common(valid(m, faces).into_iter().flat_map(|f| {
        let face = &m.faces[f];
        (0..face.verts.len()).map(move |c| face.corner_color(c))
    }))
}

/// Smoothing group of most of the faces (port of `match_smoothing`).
pub fn sample_smoothing(m: &BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    most_common(valid(m, faces).into_iter().map(|f| m.faces[f].smooth))
}

// ================================================================== palette

/// Version of the `.tres` palette dictionary.
pub const PALETTE_VERSION: i64 = 1;
/// Resource metadata key of a `.tres` palette (a bare `Resource` with this meta dictionary).
pub const PALETTE_MARKER: &str = "_xogot_modeler_palette";
/// Keys of the `.tres` palette dictionary (`version`, `materials` = array of Material or null,
/// `colors` = PackedColorArray, `labels` = PackedStringArray, `emptyColorPresetIndices` =
/// PackedInt32Array, `colorPresetLabels` = PackedStringArray). Xogot quirk: its *save* writes
/// the colour labels under `labels` but its *load* reads `labels` as material labels (and colour
/// labels from `colorPresetLabels`); write material labels to `labels` and colour labels to
/// `colorPresetLabels` so both directions work.
pub const TRES_KEY_VERSION: &str = "version";
pub const TRES_KEY_MATERIALS: &str = "materials";
pub const TRES_KEY_COLORS: &str = "colors";
pub const TRES_KEY_LABELS: &str = "labels";
pub const TRES_KEY_EMPTY: &str = "emptyColorPresetIndices";
pub const TRES_KEY_COLOR_LABELS: &str = "colorPresetLabels";

/// Colour count of an Xogot palette.
pub const XOGOT_COLORS: usize = 10;
/// Minimum material count of an Xogot palette.
pub const XOGOT_MIN_MATERIALS: usize = 10;

/// One material slot (port of `ModelerMaterialPreset`): a resource path, a
/// `modeler-builtin:` / `modeler-palette:` reference, or empty.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRef {
    pub id: String,
    pub path: String,
    pub label: Option<String>,
}

impl MaterialRef {
    /// A slot with an id derived from its position and path (stable, unique per index).
    pub fn new(index: usize, path: impl Into<String>, label: Option<String>) -> MaterialRef {
        let path = path.into();
        MaterialRef { id: make_id(index, &path), path, label }
    }
}

fn make_id(index: usize, path: &str) -> String {
    let h = path.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    // UUID-shaped (8-4-4-4-12 hex) like Xogot's ids, but deterministic.
    let x = |salt: u64| hash_u32(h ^ (index as u64).wrapping_mul(0x9e37_79b9) ^ salt);
    let (a, b, c, d) = (x(1), x(2), x(3), x(4));
    format!("{a:08x}-{:04x}-{:04x}-{:04x}-{:04x}{d:08x}", b >> 16, b & 0xffff, c >> 16, c & 0xffff)
}

/// Material slots and colour presets (port of `ModelerPalette`). Builder palettes may hold any
/// number of either; Xogot ones hold exactly ten colours and at least ten materials.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub materials: Vec<MaterialRef>,
    /// RGBA colours in 0..1.
    pub colors: Vec<[f32; 4]>,
    /// Colour labels by colour index.
    pub labels: BTreeMap<usize, String>,
    /// Colour slots the user cleared.
    pub empty_color_indices: BTreeSet<usize>,
    pub version: i64,
}

impl Default for Palette {
    /// Xogot's default palette: ten slots, the first five the built-ins, ten default colours.
    fn default() -> Self {
        let mut p = Palette {
            materials: (0..XOGOT_MIN_MATERIALS).map(|i| MaterialRef::new(i, "", None)).collect(),
            colors: DEFAULT_COLORS.to_vec(),
            labels: BTreeMap::new(),
            empty_color_indices: BTreeSet::new(),
            version: PALETTE_VERSION,
        };
        p.install_default_presets();
        p
    }
}

/// Xogot's ten default colours: white, near black, red, orange, yellow, green, blue, indigo,
/// violet, pink.
pub const DEFAULT_COLORS: [[f32; 4]; 10] = [
    [1.0, 1.0, 1.0, 1.0],
    [0.12, 0.12, 0.12, 1.0],
    [0.9, 0.15, 0.15, 1.0],
    [1.0, 0.5, 0.1, 1.0],
    [1.0, 0.85, 0.15, 1.0],
    [0.25, 0.75, 0.3, 1.0],
    [0.15, 0.65, 0.9, 1.0],
    [0.25, 0.3, 0.8, 1.0],
    [0.6, 0.25, 0.8, 1.0],
    [0.9, 0.4, 0.65, 1.0],
];

/// Message of a palette that fails validation (Xogot's text).
pub const NOT_A_VALID_PALETTE: &str = "This is not a valid mesh palette.";

impl Palette {
    /// A builder palette from material paths (empty = unused slot) and colours.
    pub fn from_slots(materials: &[String], colors: &[[f32; 4]]) -> Palette {
        Palette {
            materials: materials.iter().enumerate().map(|(i, p)| MaterialRef::new(i, p.clone(), None)).collect(),
            colors: colors.to_vec(),
            labels: BTreeMap::new(),
            empty_color_indices: BTreeSet::new(),
            version: PALETTE_VERSION,
        }
    }

    /// Port of `install_default_material_presets`: slot *i* of the first five gets built-in
    /// *i* (Grid, Checker, No Draw, Collider, Trigger) when its path is empty.
    pub fn install_default_presets(&mut self) {
        for (slot, kind) in self.materials.iter_mut().zip(Preset::ALL) {
            if slot.path.is_empty() {
                slot.path = kind.reference();
                slot.label = Some(kind.title().to_owned());
            }
        }
    }

    /// Builder validation: distinct material ids, finite colours, labels / cleared indices
    /// naming existing colours.
    pub fn validated(&self) -> Result<&Palette, String> {
        let ids: BTreeSet<&str> = self.materials.iter().map(|m| m.id.as_str()).collect();
        if ids.len() != self.materials.len()
            || self.colors.iter().flatten().any(|c| !c.is_finite())
            || self.labels.keys().any(|i| *i >= self.colors.len())
            || self.empty_color_indices.iter().any(|i| *i >= self.colors.len())
        {
            return Err(NOT_A_VALID_PALETTE.into());
        }
        Ok(self)
    }

    /// Xogot's validation (port of `ModelerPalette::validated`): the builder rules plus at
    /// least ten materials and exactly ten colours.
    pub fn validated_xogot(&self) -> Result<&Palette, String> {
        self.validated()?;
        if self.materials.len() < XOGOT_MIN_MATERIALS || self.colors.len() != XOGOT_COLORS {
            return Err(NOT_A_VALID_PALETTE.into());
        }
        Ok(self)
    }

    /// This palette fitted to Xogot's shape: materials padded with empty slots to ten (more are
    /// kept), colours padded with cleared white slots or truncated to ten (labels / cleared
    /// indices past ten are dropped).
    pub fn to_xogot(&self) -> Palette {
        let mut p = self.clone();
        while p.materials.len() < XOGOT_MIN_MATERIALS {
            let i = p.materials.len();
            let mut r = MaterialRef::new(i, "", None);
            while p.materials.iter().any(|m| m.id == r.id) {
                r.id.push('x');
            }
            p.materials.push(r);
        }
        if p.colors.len() > XOGOT_COLORS {
            p.colors.truncate(XOGOT_COLORS);
        }
        while p.colors.len() < XOGOT_COLORS {
            p.empty_color_indices.insert(p.colors.len());
            p.colors.push([1.0; 4]);
        }
        p.labels.retain(|i, _| *i < XOGOT_COLORS);
        p.empty_color_indices.retain(|i| *i < XOGOT_COLORS);
        p
    }

    /// Compact JSON in Xogot's Codable form (`materials` [{id, path, label?}], `colors`
    /// [[r,g,b,a]], `colorPresetLabels` {"index": label}, `emptyColorPresetIndices`), plus a
    /// `version` key Xogot ignores. Sizes are written as they are; see [`Palette::to_xogot_json`].
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\"materials\":[");
        for (i, m) in self.materials.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str("{\"id\":");
            json_str(&m.id, &mut s);
            s.push_str(",\"path\":");
            json_str(&m.path, &mut s);
            if let Some(l) = &m.label {
                s.push_str(",\"label\":");
                json_str(l, &mut s);
            }
            s.push('}');
        }
        s.push_str("],\"colors\":[");
        for (i, c) in self.colors.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push('[');
            for (k, v) in c.iter().enumerate() {
                if k > 0 {
                    s.push(',');
                }
                s.push_str(&json_num(*v as f64));
            }
            s.push(']');
        }
        s.push(']');
        if !self.labels.is_empty() {
            s.push_str(",\"colorPresetLabels\":{");
            for (i, (k, v)) in self.labels.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                json_str(&k.to_string(), &mut s);
                s.push(':');
                json_str(v, &mut s);
            }
            s.push('}');
        }
        if !self.empty_color_indices.is_empty() {
            s.push_str(",\"emptyColorPresetIndices\":[");
            let v: Vec<String> = self.empty_color_indices.iter().map(|i| i.to_string()).collect();
            s.push_str(&v.join(","));
            s.push(']');
        }
        s.push_str(&format!(",\"version\":{}}}", self.version));
        s
    }

    /// JSON that Xogot Modeler loads (`to_xogot().to_json()`).
    pub fn to_xogot_json(&self) -> String {
        self.to_xogot().to_json()
    }

    /// Parses an Xogot or builder JSON palette (any key order, unknown keys ignored, `version`
    /// optional) and checks it with [`Palette::validated`].
    pub fn from_json(text: &str) -> Result<Palette, String> {
        let bad = || NOT_A_VALID_PALETTE.to_string();
        let v = Json::parse(text).ok_or_else(bad)?;
        let obj = v.obj().ok_or_else(bad)?;
        let get = |k: &str| obj.iter().find(|(n, _)| n == k).map(|(_, v)| v);
        let mut materials = Vec::new();
        for m in get("materials").and_then(Json::arr).ok_or_else(bad)? {
            let o = m.obj().ok_or_else(bad)?;
            let f = |k: &str| o.iter().find(|(n, _)| n == k).map(|(_, v)| v);
            let id = f("id").and_then(Json::str).ok_or_else(bad)?.to_owned();
            let path = f("path").and_then(Json::str).ok_or_else(bad)?.to_owned();
            let label = match f("label") {
                None | Some(Json::Null) => None,
                Some(l) => Some(l.str().ok_or_else(bad)?.to_owned()),
            };
            materials.push(MaterialRef { id, path, label });
        }
        let mut colors = Vec::new();
        for c in get("colors").and_then(Json::arr).ok_or_else(bad)? {
            let a = c.arr().ok_or_else(bad)?;
            if a.len() != 4 {
                return Err(bad());
            }
            let mut out = [0.0f32; 4];
            for (k, x) in a.iter().enumerate() {
                out[k] = x.num().ok_or_else(bad)? as f32;
            }
            colors.push(out);
        }
        let mut labels = BTreeMap::new();
        match get("colorPresetLabels") {
            None | Some(Json::Null) => {}
            Some(l) => {
                for (k, v) in l.obj().ok_or_else(bad)? {
                    labels.insert(k.parse::<usize>().map_err(|_| bad())?, v.str().ok_or_else(bad)?.to_owned());
                }
            }
        }
        let mut empty_color_indices = BTreeSet::new();
        match get("emptyColorPresetIndices") {
            None | Some(Json::Null) => {}
            Some(l) => {
                for x in l.arr().ok_or_else(bad)? {
                    let n = x.num().ok_or_else(bad)?;
                    if n < 0.0 || n.fract() != 0.0 {
                        return Err(bad());
                    }
                    empty_color_indices.insert(n as usize);
                }
            }
        }
        let version = get("version").and_then(Json::num).map_or(PALETTE_VERSION, |n| n as i64);
        let p = Palette { materials, colors, labels, empty_color_indices, version };
        p.validated()?;
        Ok(p)
    }
}

// ------------------------------------------------------------------ minimal JSON

fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Shortest text that reads back to the same `f32` (palette numbers are `Float`s in Xogot).
fn json_num(v: f64) -> String {
    let f = v as f32;
    if !f.is_finite() {
        return "0".into();
    }
    if f.fract() == 0.0 && f.abs() < 1e9 {
        return format!("{}", f as i64);
    }
    format!("{f}")
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn obj(&self) -> Option<&Vec<(String, Json)>> {
        if let Json::Obj(o) = self { Some(o) } else { None }
    }
    fn arr(&self) -> Option<&Vec<Json>> {
        if let Json::Arr(a) = self { Some(a) } else { None }
    }
    fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }
    fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self { Some(*n) } else { None }
    }

    fn parse(text: &str) -> Option<Json> {
        let mut p = Parser { s: text.as_bytes(), i: 0 };
        let v = p.value(0)?;
        p.ws();
        (p.i == p.s.len()).then_some(v)
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn lit(&mut self, w: &str, v: Json) -> Option<Json> {
        self.s[self.i..].starts_with(w.as_bytes()).then(|| {
            self.i += w.len();
            v
        })
    }
    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > 64 {
            return None;
        }
        self.ws();
        match *self.s.get(self.i)? {
            b'n' => self.lit("null", Json::Null),
            b't' => self.lit("true", Json::Bool(true)),
            b'f' => self.lit("false", Json::Bool(false)),
            b'"' => self.string().map(Json::Str),
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                if self.eat(b']') {
                    return Some(Json::Arr(a));
                }
                loop {
                    a.push(self.value(depth + 1)?);
                    if self.eat(b']') {
                        return Some(Json::Arr(a));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut o = Vec::new();
                if self.eat(b'}') {
                    return Some(Json::Obj(o));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    if !self.eat(b':') {
                        return None;
                    }
                    o.push((k, self.value(depth + 1)?));
                    if self.eat(b'}') {
                        return Some(Json::Obj(o));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i]).ok()?.parse::<f64>().ok().map(Json::Num)
            }
        }
    }
    fn hex4(&mut self) -> Option<u32> {
        let h = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
        self.i += 4;
        u32::from_str_radix(h, 16).ok()
    }
    fn string(&mut self) -> Option<String> {
        if self.s.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'"' && self.s[self.i] != b'\\' {
                self.i += 1;
            }
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).ok()?);
            match *self.s.get(self.i)? {
                b'"' => {
                    self.i += 1;
                    return Some(out);
                }
                _ => {
                    self.i += 1;
                    let c = *self.s.get(self.i)?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut u = self.hex4()?;
                            if (0xd800..0xdc00).contains(&u) && self.s.get(self.i..self.i + 2) == Some(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                u = 0x10000 + ((u - 0xd800) << 10) + (lo.wrapping_sub(0xdc00) & 0x3ff);
                            }
                            out.push(char::from_u32(u).unwrap_or('\u{fffd}'));
                        }
                        _ => return None,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;
    use crate::math::v3;

    #[test]
    fn presets_follow_xogot() {
        assert_eq!(Preset::from_reference("modeler-builtin:noDraw"), Some(Preset::NoDraw));
        assert_eq!(Preset::Grid.reference(), "modeler-builtin:grid");
        assert_eq!(Preset::from_reference("res://x.tres"), None);
        let g = Preset::Grid.material();
        assert_eq!((g.pattern, g.uv1_scale, g.transparent), (Some(Pattern::Grid), [2.0, 2.0, 1.0], false));
        let n = Preset::NoDraw.material();
        assert!(n.transparent && n.unshaded && n.pattern.is_none() && n.albedo[3] == 0.0 && !n.cull_disabled);
        let c = Preset::Collider.material();
        assert_eq!(c.albedo, [0.2, 0.8, 0.4, 0.35]);
        assert!(c.unshaded && c.cull_disabled && c.pattern == Some(Pattern::Grid) && c.uv1_scale[0] == 2.0);
        let t = Preset::Trigger.material();
        assert_eq!(t.albedo, [1.0, 0.65, 0.12, 0.35]);
        assert!(t.cull_disabled && t.unshaded);
        assert_eq!(Preset::Checker.material().name, "Modeler Checker");
    }

    #[test]
    fn pattern_pixels() {
        let g = pattern_rgba8(Pattern::Grid);
        assert_eq!(g.len(), 64 * 64 * 4);
        let at = |px: &[u8], x: usize, y: usize| px[(y * 64 + x) * 4];
        assert_eq!(at(&g, 0, 30), 0x69);
        assert_eq!(at(&g, 1, 1), 0x69);
        assert_eq!(at(&g, 16, 5), 0xaa);
        assert_eq!(at(&g, 5, 32), 0xaa);
        assert_eq!(at(&g, 5, 5), 0xd7);
        assert!(g.chunks(4).all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]));
        let c = pattern_rgba8(Pattern::Checker);
        assert_eq!((at(&c, 0, 0), at(&c, 32, 0), at(&c, 32, 32), at(&c, 63, 31)), (0xdc, 0x73, 0xdc, 0x73));
    }

    #[test]
    fn smoothing_groups_ops() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<usize> = (0..6).collect();
        assert_eq!(new_group_from_faces(&mut m, &BTreeSet::from([0, 1])), Some(1));
        assert_eq!(new_group_from_faces(&mut m, &BTreeSet::from([2])), Some(2));
        assert_eq!(new_group_from_faces(&mut m, &BTreeSet::new()), None);
        assert_eq!(smoothing_groups(&m), vec![GroupInfo { index: 1, face_count: 2 }, GroupInfo { index: 2, face_count: 1 }]);
        // Smooth: chip first, then panel, then a free group.
        assert_eq!(smooth_faces(&mut m, &BTreeSet::from([3]), &[0, 2], 7), Some(2));
        assert_eq!(smooth_faces(&mut m, &BTreeSet::from([4]), &[], 7), Some(7));
        assert_eq!(smooth_faces(&mut m, &BTreeSet::from([5]), &[], 0), Some(3));
        assert_eq!(faces_in_group(&m, 2), BTreeSet::from([2, 3]));
        facet_faces(&mut m, &BTreeSet::from([5]));
        assert_eq!(m.faces[5].smooth, 0);
        assert_eq!(merge_group(&mut m, 2, 1), Ok(2));
        assert_eq!(faces_in_group(&m, 1), BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(merge_group(&mut m, 1, 1), Ok(0));
        assert!(merge_group(&mut m, 0, 1).is_err());
        assert!(merge_group(&mut m, 9, 1).is_err());
        assert_eq!(clear_group(&mut m, 1), 4);
        assert_eq!(clear_group(&mut m, 0), 0);
        // All 23 groups in use: none free.
        let mut big = BMesh::default();
        for i in 0..24u16 {
            let b = big.positions.len() as u32;
            for p in [v3(0.0, 0.0, i as f32), v3(1.0, 0.0, i as f32), v3(0.0, 1.0, i as f32)] {
                big.add_vert(p);
            }
            let f = big.add_face(vec![b, b + 1, b + 2]);
            big.faces[f].smooth = i;
        }
        assert_eq!(next_free_group(&big), None);
        assert_eq!(new_group_from_faces(&mut big, &BTreeSet::from([0])), None);
        set_group(&mut m, &all, 0);
        m.validate().unwrap();
    }

    #[test]
    fn group_tints_use_the_golden_angle() {
        let one = smoothing_group_color(1);
        assert_eq!(one, smoothing_group_color(0));
        let fraction = 137.0f32 / 360.0 * 6.0 - 2.0;
        assert_eq!(one, [0.9 * (1.0 - 0.62), 0.9, (1.0 - (1.0 - fraction) * 0.62) * 0.9, 1.0]);
        let three = smoothing_group_color(3);
        assert_eq!(three[0], 0.9);
        assert!((three[2] - 0.342).abs() < 1e-6);
        assert_ne!(smoothing_group_color(2), one);
    }

    #[test]
    fn sample_picks_the_most_common_value() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.faces[0].material = 2;
        m.faces[1].material = 2;
        m.faces[2].material = 1;
        m.faces[2].smooth = 4;
        m.faces[1].color = [9, 9, 9, 255];
        m.faces[2].color_corners = vec![[1, 2, 3, 4]; 4];
        let sel = BTreeSet::from([0, 1, 2, 99]);
        assert_eq!(sample_material(&m, &sel), Some(2));
        assert_eq!(sample_smoothing(&m, &BTreeSet::from([2])), Some(4));
        assert_eq!(sample_color(&m, &sel), Some([255; 4]), "tie of 4 corners each: first seen");
        assert_eq!(sample_color(&m, &BTreeSet::from([1, 2])), Some([9, 9, 9, 255]));
        assert_eq!(sample_material(&m, &BTreeSet::new()), None);
    }

    #[test]
    fn default_palette_is_a_valid_xogot_palette() {
        let p = Palette::default();
        p.validated_xogot().unwrap();
        assert_eq!(p.materials[0].path, "modeler-builtin:grid");
        assert_eq!(p.materials[2].label.as_deref(), Some("No Draw"));
        assert_eq!(p.materials[5].path, "");
        let back = Palette::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn builder_palette_round_trips_and_fits_xogot() {
        let mut p = Palette::from_slots(&["res://mats/wood.tres".into(), String::new(), "res://a\"b\\c.tres".into()], &[[0.5, 0.25, 0.125, 1.0]; 8]);
        p.labels.insert(3, "Ink \u{e9}".into());
        p.empty_color_indices.insert(7);
        p.validated().unwrap();
        assert!(p.validated_xogot().is_err());
        let json = p.to_json();
        assert!(json.contains("res:\\/\\/mats"), "slashes escaped like JSONEncoder: {json}");
        assert_eq!(Palette::from_json(&json).unwrap(), p);
        let x = p.to_xogot();
        x.validated_xogot().unwrap();
        assert_eq!(x.materials.len(), 10);
        assert_eq!(x.colors.len(), 10);
        assert_eq!(x.empty_color_indices, BTreeSet::from([7, 8, 9]));
        assert_eq!(Palette::from_json(&p.to_xogot_json()).unwrap(), x);
        // More than ten colours: truncated, labels past ten dropped.
        let mut many = Palette::from_slots(&[], &[[1.0; 4]; 12]);
        many.labels.insert(11, "gone".into());
        let x = many.to_xogot();
        assert_eq!(x.colors.len(), 10);
        assert!(x.labels.is_empty());
        x.validated_xogot().unwrap();
    }

    #[test]
    fn reads_xogot_json() {
        // Hand-written in Xogot's Codable form: other key order, ints for indices, null label.
        let text = r#" { "emptyColorPresetIndices": [9, 1.0], "colorPresetLabels": {"3": "Wood"},
            "colors": [[1,1,1,1],[0.12,0.12,0.12,1],[0.9,0.15,0.15,1],[1,0.5,0.1,1],[1,0.85,0.15,1],
                       [0.25,0.75,0.3,1],[0.15,0.65,0.9,1],[0.25,0.3,0.8,1],[0.6,0.25,0.8,1],[0.9,0.4,0.65,1]],
            "materials": [{"path": "modeler-builtin:grid", "id": "A-1", "label": "Grid"},
                          {"id": "B-2", "path": "res:\/\/m.tres", "label": null, "extra": true}] } "#;
        let p = Palette::from_json(text).unwrap();
        assert_eq!(p.materials[1].id, "B-2");
        assert_eq!(p.materials[1].path, "res://m.tres");
        assert_eq!(p.materials[1].label, None);
        assert_eq!(p.labels.get(&3).map(String::as_str), Some("Wood"));
        assert_eq!(p.empty_color_indices, BTreeSet::from([1, 9]));
        assert_eq!(p.version, PALETTE_VERSION);
        assert_eq!(p.colors[1], [0.12, 0.12, 0.12, 1.0]);
        assert!(p.validated_xogot().is_err(), "only two materials");
        assert!(p.to_xogot().validated_xogot().is_ok());
    }

    #[test]
    fn rejects_bad_palettes() {
        for bad in [
            "",
            "[]",
            "{\"materials\":[]}",
            "{\"materials\":[],\"colors\":[[1,1,1]]}",
            "{\"materials\":[{\"id\":\"a\",\"path\":\"\"},{\"id\":\"a\",\"path\":\"\"}],\"colors\":[]}",
            "{\"materials\":[],\"colors\":[[1,1,1,1]],\"colorPresetLabels\":{\"4\":\"x\"}}",
            "{\"materials\":[],\"colors\":[[1,1,1,1]],\"emptyColorPresetIndices\":[1]}",
            "{\"materials\":[],\"colors\":[]} trailing",
        ] {
            assert!(Palette::from_json(bad).is_err(), "{bad}");
        }
    }
}
