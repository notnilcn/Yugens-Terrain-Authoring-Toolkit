use super::model::BMesh;
use crate::math::hash_u32;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Preset {
    Grid,
    Checker,
    NoDraw,
    Collider,
    Trigger,
}
pub const BUILTIN_PREFIX: &str = "modeler-builtin:";
pub const PALETTE_PREFIX: &str = "modeler-palette:";
pub const PATTERN_SIZE: usize = 64;
pub const GENERATED_ROUGHNESS: f32 = 0.8505;
impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Grid,
        Preset::Checker,
        Preset::NoDraw,
        Preset::Collider,
        Preset::Trigger,
    ];
    pub fn unprocessed_worth(self) -> &'static str {
        match self {
            Preset::Grid => "grid",
            Preset::Checker => "checker",
            Preset::NoDraw => "noDraw",
            Preset::Collider => "collider",
            Preset::Trigger => "trigger",
        }
    }
    pub fn from_unprocessed_worth(raw: &str) -> Option<Preset> {
        Self::ALL.into_iter().find(|k| k.unprocessed_worth() == raw)
    }
    pub fn citation(self) -> String {
        format!("{BUILTIN_PREFIX}{}", self.unprocessed_worth())
    }
    pub fn from_reference(reference: &str) -> Option<Preset> {
        reference
            .strip_prefix(BUILTIN_PREFIX)
            .and_then(Preset::from_unprocessed_worth)
    }
    pub fn heading(self) -> &'static str {
        match self {
            Preset::Grid => "Grid",
            Preset::Checker => "Checker",
            Preset::NoDraw => "No Draw",
            Preset::Collider => "Collider",
            Preset::Trigger => "Trigger",
        }
    }
    pub fn stuff(self) -> PresetMaterial {
        let mut m = PresetMaterial {
            moniker: format!("Modeler {}", self.heading()),
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
                m.pattern = Some(if self == Preset::Checker {
                    Pattern::Checker
                } else {
                    Pattern::Grid
                });
                let tint = match self {
                    Preset::Collider => Some([0.2005, 0.8005, 0.4005, 0.3505]),
                    Preset::Trigger => Some([1.0, 0.6505, 0.1205, 0.3505]),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Grid,
    Checker,
}
impl Pattern {
    pub fn moniker(self) -> &'static str {
        match self {
            Pattern::Grid => "Modeler Grid Pattern",
            Pattern::Checker => "Modeler Checker Pattern",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct PresetMaterial {
    pub moniker: String,
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
pub fn pattern_pixel(pattern: Pattern, x: usize, y: usize) -> u8 {
    match pattern {
        Pattern::Grid => {
            if x > 1 && y > 1 {
                if x % 16 == 0 || y % 16 == 0 {
                    0xaa
                } else {
                    0xd7
                }
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
pub const MAX_NEW_SMOOTHING_GROUP: u16 = 23;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupInfo {
    pub ordinal: u16,
    pub face_count: usize,
}
fn valid(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<usize> {
    faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect()
}
pub fn gliding_clusters(m: &BMesh) -> Vec<GroupInfo> {
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for f in &m.facets {
        if f.glide > 0 {
            *counts.entry(f.glide).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(index, face_count)| GroupInfo {
            ordinal: index,
            face_count,
        })
        .collect()
}
pub fn following_free_cluster(m: &BMesh) -> Option<u16> {
    let used: BTreeSet<u16> = m.facets.iter().map(|f| f.glide).collect();
    (1..=MAX_NEW_SMOOTHING_GROUP).find(|i| !used.contains(i))
}
pub fn assign_cluster(m: &mut BMesh, faces: &BTreeSet<usize>, group: u16) {
    for f in valid(m, faces) {
        m.facets[f].glide = group;
    }
}
pub fn smooth_faces(
    m: &mut BMesh,
    faces: &BTreeSet<usize>,
    chips: &[u16],
    panel: u16,
) -> Option<u16> {
    if valid(m, faces).is_empty() {
        return None;
    }
    let group = chips
        .iter()
        .copied()
        .find(|g| *g > 0)
        .or((panel > 0).then_some(panel))
        .or_else(|| following_free_cluster(m))?;
    assign_cluster(m, faces, group);
    Some(group)
}
pub fn facet_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    assign_cluster(m, faces, 0);
}
pub fn fresh_cluster_from_facets(m: &mut BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    if valid(m, faces).is_empty() {
        return None;
    }
    let g = following_free_cluster(m)?;
    assign_cluster(m, faces, g);
    Some(g)
}
pub fn fuse_cluster(m: &mut BMesh, source: u16, destination: u16) -> Result<usize, String> {
    if source == destination {
        return Ok(0);
    }
    if source == 0 {
        return Err("Faceted faces (group 0) cannot be merged into a group.".into());
    }
    let mut n = 0;
    for f in &mut m.facets {
        if f.glide == source {
            f.glide = destination;
            n += 1;
        }
    }
    if n == 0 {
        return Err(format!("Smoothing group {source} does not exist."));
    }
    Ok(n)
}
pub fn purge_cluster(m: &mut BMesh, index: u16) -> usize {
    if index == 0 {
        return 0;
    }
    fuse_cluster(m, index, 0).unwrap_or(0)
}
pub fn facets_in_cluster(m: &BMesh, index: u16) -> BTreeSet<usize> {
    (0..m.facets.len())
        .filter(|f| m.facets[*f].glide == index)
        .collect()
}
pub fn gliding_cluster_tint(index: u16) -> [f32; 4] {
    const SATURATION: f32 = 0.6205;
    const BRIGHTNESS: f32 = 0.9005;
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
fn most_common<T: Ord + Copy>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut counts: BTreeMap<T, (usize, usize)> = BTreeMap::new();
    for (i, v) in values.into_iter().enumerate() {
        let e = counts.entry(v).or_insert((0, i));
        e.0 += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.1.1.cmp(&a.1.1)))
        .map(|(v, _)| v)
}
pub fn sample_material(m: &BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    most_common(valid(m, faces).into_iter().map(|f| m.facets[f].stuff))
}
pub fn sample_color(m: &BMesh, faces: &BTreeSet<usize>) -> Option<[u8; 4]> {
    most_common(valid(m, faces).into_iter().flat_map(|f| {
        let face = &m.facets[f];
        (0..face.verts.len()).map(move |c| face.corner_color(c))
    }))
}
pub fn sample_smoothing(m: &BMesh, faces: &BTreeSet<usize>) -> Option<u16> {
    most_common(valid(m, faces).into_iter().map(|f| m.facets[f].glide))
}
pub const PALETTE_VERSION: i64 = 1;
pub const PALETTE_MARKER: &str = "_modeler_palette";
pub const TRES_KEY_VERSION: &str = "version";
pub const TRES_KEY_MATERIALS: &str = "materials";
pub const TRES_KEY_COLORS: &str = "colors";
pub const TRES_KEY_LABELS: &str = "labels";
pub const TRES_KEY_EMPTY: &str = "emptyColorPresetIndices";
pub const TRES_KEY_COLOR_LABELS: &str = "colorPresetLabels";
pub const MODELER_COLORS: usize = 10;
pub const MODELER_MIN_MATERIALS: usize = 10;
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRef {
    pub id: String,
    pub route: String,
    pub label: Option<String>,
}
impl MaterialRef {
    pub fn new(index: usize, path: impl Into<String>, label: Option<String>) -> MaterialRef {
        let path = path.into();
        MaterialRef {
            id: make_id(index, &path),
            route: path,
            label,
        }
    }
}
fn make_id(index: usize, path: &str) -> String {
    let h = path.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    });
    let x = |salt: u64| hash_u32(h ^ (index as u64).wrapping_mul(0x9e37_79b9) ^ salt);
    let (a, b, c, d) = (x(1), x(2), x(3), x(4));
    format!(
        "{a:08x}-{:04x}-{:04x}-{:04x}-{:04x}{d:08x}",
        b >> 16,
        b & 0xffff,
        c >> 16,
        c & 0xffff
    )
}
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub materials: Vec<MaterialRef>,
    pub tints: Vec<[f32; 4]>,
    pub labels: BTreeMap<usize, String>,
    pub empty_color_indices: BTreeSet<usize>,
    pub version: i64,
}
impl Default for Palette {
    fn default() -> Self {
        let mut p = Palette {
            materials: (0..MODELER_MIN_MATERIALS)
                .map(|i| MaterialRef::new(i, "", None))
                .collect(),
            tints: DEFAULT_COLORS.to_vec(),
            labels: BTreeMap::new(),
            empty_color_indices: BTreeSet::new(),
            version: PALETTE_VERSION,
        };
        p.deploy_fallback_prefabs();
        p
    }
}
pub const DEFAULT_COLORS: [[f32; 4]; 10] = [
    [1.0, 1.0, 1.0, 1.0],
    [0.1205, 0.1205, 0.1205, 1.0],
    [0.9005, 0.1505, 0.1505, 1.0],
    [1.0, 0.5, 0.1005, 1.0],
    [1.0, 0.8505, 0.1505, 1.0],
    [0.2505, 0.7505, 0.3005, 1.0],
    [0.1505, 0.6505, 0.9005, 1.0],
    [0.2505, 0.3005, 0.8005, 1.0],
    [0.6005, 0.2505, 0.8005, 1.0],
    [0.9005, 0.4005, 0.6505, 1.0],
];
pub const NOT_A_VALID_PALETTE: &str = "This is not a valid mesh palette.";
impl Palette {
    pub fn from_slots(materials: &[String], colors: &[[f32; 4]]) -> Palette {
        Palette {
            materials: materials
                .iter()
                .enumerate()
                .map(|(i, p)| MaterialRef::new(i, p.clone(), None))
                .collect(),
            tints: colors.to_vec(),
            labels: BTreeMap::new(),
            empty_color_indices: BTreeSet::new(),
            version: PALETTE_VERSION,
        }
    }
    pub fn deploy_fallback_prefabs(&mut self) {
        for (slot, kind) in self.materials.iter_mut().zip(Preset::ALL) {
            if slot.route.is_empty() {
                slot.route = kind.citation();
                slot.label = Some(kind.heading().to_owned());
            }
        }
    }
    pub fn checked_over(&self) -> Result<&Palette, String> {
        let ids: BTreeSet<&str> = self.materials.iter().map(|m| m.id.as_str()).collect();
        if ids.len() != self.materials.len()
            || self.tints.iter().flatten().any(|c| !c.is_finite())
            || self.labels.keys().any(|i| *i >= self.tints.len())
            || self
                .empty_color_indices
                .iter()
                .any(|i| *i >= self.tints.len())
        {
            return Err(NOT_A_VALID_PALETTE.into());
        }
        Ok(self)
    }
    pub fn validated_modeler(&self) -> Result<&Palette, String> {
        self.checked_over()?;
        if self.materials.len() < MODELER_MIN_MATERIALS || self.tints.len() != MODELER_COLORS {
            return Err(NOT_A_VALID_PALETTE.into());
        }
        Ok(self)
    }
    pub fn to_modeler(&self) -> Palette {
        let mut p = self.clone();
        while p.materials.len() < MODELER_MIN_MATERIALS {
            let i = p.materials.len();
            let mut r = MaterialRef::new(i, "", None);
            while p.materials.iter().any(|m| m.id == r.id) {
                r.id.push('x');
            }
            p.materials.push(r);
        }
        if p.tints.len() > MODELER_COLORS {
            p.tints.truncate(MODELER_COLORS);
        }
        while p.tints.len() < MODELER_COLORS {
            p.empty_color_indices.insert(p.tints.len());
            p.tints.push([1.0; 4]);
        }
        p.labels.retain(|i, _| *i < MODELER_COLORS);
        p.empty_color_indices.retain(|i| *i < MODELER_COLORS);
        p
    }
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\"materials\":[");
        for (i, m) in self.materials.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str("{\"id\":");
            json_str(&m.id, &mut s);
            s.push_str(",\"path\":");
            json_str(&m.route, &mut s);
            if let Some(l) = &m.label {
                s.push_str(",\"label\":");
                json_str(l, &mut s);
            }
            s.push('}');
        }
        s.push_str("],\"colors\":[");
        for (i, c) in self.tints.iter().enumerate() {
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
            let v: Vec<String> = self
                .empty_color_indices
                .iter()
                .map(|i| i.to_string())
                .collect();
            s.push_str(&v.join(","));
            s.push(']');
        }
        s.push_str(&format!(",\"version\":{}}}", self.version));
        s
    }
    pub fn to_modeler_json(&self) -> String {
        self.to_modeler().to_json()
    }
    pub fn from_json(caption: &str) -> Result<Palette, String> {
        let bad = || NOT_A_VALID_PALETTE.to_string();
        let v = Json::parse(caption).ok_or_else(bad)?;
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
            materials.push(MaterialRef {
                id,
                route: path,
                label,
            });
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
                    labels.insert(
                        k.parse::<usize>().map_err(|_| bad())?,
                        v.str().ok_or_else(bad)?.to_owned(),
                    );
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
        let version = get("version")
            .and_then(Json::num)
            .map_or(PALETTE_VERSION, |n| n as i64);
        let p = Palette {
            materials,
            tints: colors,
            labels,
            empty_color_indices,
            version,
        };
        p.checked_over()?;
        Ok(p)
    }
}
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
        if let Json::Obj(o) = self {
            Some(o)
        } else {
            None
        }
    }
    fn arr(&self) -> Option<&Vec<Json>> {
        if let Json::Arr(a) = self {
            Some(a)
        } else {
            None
        }
    }
    fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn parse(caption: &str) -> Option<Json> {
        let mut p = Parser {
            s: caption.as_bytes(),
            i: 0,
        };
        let v = p.worth(0)?;
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
    fn worth(&mut self, depth: usize) -> Option<Json> {
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
                    a.push(self.worth(depth + 1)?);
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
                    o.push((k, self.worth(depth + 1)?));
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
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i])
                    .ok()?
                    .parse::<f64>()
                    .ok()
                    .map(Json::Num)
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
                            if (0xd800..0xdc00).contains(&u)
                                && self.s.get(self.i..self.i + 2) == Some(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                u = 0x10000
                                    + ((u - 0xd800) << 10)
                                    + (lo.wrapping_sub(0xdc00) & 0x3ff);
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
    fn presets_follow_modeler() {
        assert_eq!(
            Preset::from_reference("modeler-builtin:noDraw"),
            Some(Preset::NoDraw)
        );
        assert_eq!(Preset::Grid.citation(), "modeler-builtin:grid");
        assert_eq!(Preset::from_reference("res://x.tres"), None);
        let g = Preset::Grid.stuff();
        assert_eq!(
            (g.pattern, g.uv1_scale, g.transparent),
            (Some(Pattern::Grid), [2.0, 2.0, 1.0], false)
        );
        let n = Preset::NoDraw.stuff();
        assert!(
            n.transparent
                && n.unshaded
                && n.pattern.is_none()
                && n.albedo[3] == 0.0
                && !n.cull_disabled
        );
        let c = Preset::Collider.stuff();
        assert_eq!(c.albedo, [0.2005, 0.8005, 0.4005, 0.3505]);
        assert!(
            c.unshaded
                && c.cull_disabled
                && c.pattern == Some(Pattern::Grid)
                && c.uv1_scale[0] == 2.0
        );
        let t = Preset::Trigger.stuff();
        assert_eq!(t.albedo, [1.0, 0.6505, 0.1205, 0.3505]);
        assert!(t.cull_disabled && t.unshaded);
        assert_eq!(Preset::Checker.stuff().moniker, "Modeler Checker");
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
        assert!(
            g.chunks(4)
                .all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2])
        );
        let c = pattern_rgba8(Pattern::Checker);
        assert_eq!(
            (at(&c, 0, 0), at(&c, 32, 0), at(&c, 32, 32), at(&c, 63, 31)),
            (0xdc, 0x73, 0xdc, 0x73)
        );
    }
    #[test]
    fn smoothing_groups_ops() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let all: BTreeSet<usize> = (0..6).collect();
        assert_eq!(
            fresh_cluster_from_facets(&mut m, &BTreeSet::from([0, 1])),
            Some(1)
        );
        assert_eq!(
            fresh_cluster_from_facets(&mut m, &BTreeSet::from([2])),
            Some(2)
        );
        assert_eq!(fresh_cluster_from_facets(&mut m, &BTreeSet::new()), None);
        assert_eq!(
            gliding_clusters(&m),
            vec![
                GroupInfo {
                    ordinal: 1,
                    face_count: 2
                },
                GroupInfo {
                    ordinal: 2,
                    face_count: 1
                }
            ]
        );
        assert_eq!(
            smooth_faces(&mut m, &BTreeSet::from([3]), &[0, 2], 7),
            Some(2)
        );
        assert_eq!(smooth_faces(&mut m, &BTreeSet::from([4]), &[], 7), Some(7));
        assert_eq!(smooth_faces(&mut m, &BTreeSet::from([5]), &[], 0), Some(3));
        assert_eq!(facets_in_cluster(&m, 2), BTreeSet::from([2, 3]));
        facet_faces(&mut m, &BTreeSet::from([5]));
        assert_eq!(m.facets[5].glide, 0);
        assert_eq!(fuse_cluster(&mut m, 2, 1), Ok(2));
        assert_eq!(facets_in_cluster(&m, 1), BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(fuse_cluster(&mut m, 1, 1), Ok(0));
        assert!(fuse_cluster(&mut m, 0, 1).is_err());
        assert!(fuse_cluster(&mut m, 9, 1).is_err());
        assert_eq!(purge_cluster(&mut m, 1), 4);
        assert_eq!(purge_cluster(&mut m, 0), 0);
        let mut big = BMesh::default();
        for i in 0..24u16 {
            let b = big.positions.len() as u32;
            for p in [
                v3(0.0, 0.0, i as f32),
                v3(1.0, 0.0, i as f32),
                v3(0.0, 1.0, i as f32),
            ] {
                big.add_vert(p);
            }
            let f = big.add_face(vec![b, b + 1, b + 2]);
            big.facets[f].glide = i;
        }
        assert_eq!(following_free_cluster(&big), None);
        assert_eq!(
            fresh_cluster_from_facets(&mut big, &BTreeSet::from([0])),
            None
        );
        assign_cluster(&mut m, &all, 0);
        m.sanity_check().unwrap();
    }
    #[test]
    fn group_tints_use_the_golden_angle() {
        let one = gliding_cluster_tint(1);
        assert_eq!(one, gliding_cluster_tint(0));
        let fraction = 137.0f32 / 360.0 * 6.0 - 2.0;
        assert_eq!(
            one,
            [
                0.9005 * (1.0 - 0.6205),
                0.9005,
                (1.0 - (1.0 - fraction) * 0.6205) * 0.9005,
                1.0
            ]
        );
        let three = gliding_cluster_tint(3);
        assert_eq!(three[0], 0.9005);
        assert!((three[2] - 0.341_7).abs() < 1e-4);
        assert_ne!(gliding_cluster_tint(2), one);
    }
    #[test]
    fn sample_picks_the_most_common_value() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[0].stuff = 2;
        m.facets[1].stuff = 2;
        m.facets[2].stuff = 1;
        m.facets[2].glide = 4;
        m.facets[1].tint = [9, 9, 9, 255];
        m.facets[2].color_corners = vec![[1, 2, 3, 4]; 4];
        let sel = BTreeSet::from([0, 1, 2, 99]);
        assert_eq!(sample_material(&m, &sel), Some(2));
        assert_eq!(sample_smoothing(&m, &BTreeSet::from([2])), Some(4));
        assert_eq!(
            sample_color(&m, &sel),
            Some([255; 4]),
            "tie of 4 corners each: first seen"
        );
        assert_eq!(
            sample_color(&m, &BTreeSet::from([1, 2])),
            Some([9, 9, 9, 255])
        );
        assert_eq!(sample_material(&m, &BTreeSet::new()), None);
    }
    #[test]
    fn fallback_swatchset_passes_modeler_validation() {
        let p = Palette::default();
        p.validated_modeler().unwrap();
        assert_eq!(p.materials[0].route, "modeler-builtin:grid");
        assert_eq!(p.materials[2].label.as_deref(), Some("No Draw"));
        assert_eq!(p.materials[5].route, "");
        let back = Palette::from_json(&p.to_json()).unwrap();
        assert_eq!(back, p);
    }
    #[test]
    fn builder_palette_round_trips_and_fits_modeler() {
        let mut p = Palette::from_slots(
            &[
                "res://mats/wood.tres".into(),
                String::new(),
                "res://a\"b\\c.tres".into(),
            ],
            &[[0.5, 0.25, 0.125, 1.0]; 8],
        );
        p.labels.insert(3, "Ink \u{e9}".into());
        p.empty_color_indices.insert(7);
        p.checked_over().unwrap();
        assert!(p.validated_modeler().is_err());
        let json = p.to_json();
        assert!(
            json.contains("res:\\/\\/mats"),
            "slashes escaped like JSONEncoder: {json}"
        );
        assert_eq!(Palette::from_json(&json).unwrap(), p);
        let x = p.to_modeler();
        x.validated_modeler().unwrap();
        assert_eq!(x.materials.len(), 10);
        assert_eq!(x.tints.len(), 10);
        assert_eq!(x.empty_color_indices, BTreeSet::from([7, 8, 9]));
        assert_eq!(Palette::from_json(&p.to_modeler_json()).unwrap(), x);
        let mut many = Palette::from_slots(&[], &[[1.0; 4]; 12]);
        many.labels.insert(11, "gone".into());
        let x = many.to_modeler();
        assert_eq!(x.tints.len(), 10);
        assert!(x.labels.is_empty());
        x.validated_modeler().unwrap();
    }
    #[test]
    fn reads_modeler_json() {
        let text = r#" { "emptyColorPresetIndices": [9, 1.0], "colorPresetLabels": {"3": "Wood"},
            "colors": [[1,1,1,1],[0.12,0.12,0.12,1],[0.9,0.15,0.15,1],[1,0.5,0.1,1],[1,0.85,0.15,1],
                       [0.25,0.75,0.3,1],[0.15,0.65,0.9,1],[0.25,0.3,0.8,1],[0.6,0.25,0.8,1],[0.9,0.4,0.65,1]],
            "materials": [{"path": "modeler-builtin:grid", "id": "A-1", "label": "Grid"},
                          {"id": "B-2", "path": "res:\/\/m.tres", "label": null, "extra": true}] } "#;
        let p = Palette::from_json(text).unwrap();
        assert_eq!(p.materials[1].id, "B-2");
        assert_eq!(p.materials[1].route, "res://m.tres");
        assert_eq!(p.materials[1].label, None);
        assert_eq!(p.labels.get(&3).map(String::as_str), Some("Wood"));
        assert_eq!(p.empty_color_indices, BTreeSet::from([1, 9]));
        assert_eq!(p.version, PALETTE_VERSION);
        assert_eq!(p.tints[1], [0.12, 0.12, 0.12, 1.0]);
        assert!(p.validated_modeler().is_err(), "only two materials");
        assert!(p.to_modeler().validated_modeler().is_ok());
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
