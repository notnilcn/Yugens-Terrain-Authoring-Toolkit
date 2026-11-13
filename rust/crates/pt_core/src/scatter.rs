//! Prop / vegetation scatter (engine-free half of `PixelScatterItem` + the Scatter tool).
//!
//! Ideas from Terrain3D's instancer (paint MultiMesh instances, scale/rotation/colour jitter,
//! slope filter, re-snap after sculpting; MIT) and hex_map_godot's feature manager
//! (deterministic per-cell hash placement; MIT, after catlikecoding's Hex Map tutorials), plus
//! Minor Deity's clustered wild flowers (idea only: placement gated by a low-frequency noise).
//!
//! Instances store only XZ (terrain-local), yaw, scale and colour jitter; their height and tilt
//! are recomputed from the terrain whenever it is remeshed, so props follow sculpting.

use std::collections::BTreeMap;

use crate::brush::Brush;
use crate::grid::{GridConfig, GridType, SQRT3};
use crate::math::*;

/// Placement rules of one scatter item (mirrors the `PixelScatterItem` resource).
#[derive(Clone, Debug, PartialEq)]
pub struct ScatterRules {
    /// Instances per square world unit at full brush weight.
    pub density: f32,
    pub scale_min: f32,
    pub scale_max: f32,
    /// Random yaw range in degrees (360 = any direction).
    pub yaw_jitter: f32,
    /// Hue shift range (± turns, e.g. 0.05) and value (brightness) range (± fraction).
    pub hue_jitter: f32,
    pub value_jitter: f32,
    /// Allowed terrain slope in degrees.
    pub slope_min: f32,
    pub slope_max: f32,
    /// Allowed terrain height (terrain-local).
    pub height_min: f32,
    pub height_max: f32,
    /// Cluster noise: world size of the patches (0 = off) and the noise level (−1..1) an
    /// instance needs to be placed (higher = smaller, sparser patches).
    pub cluster_scale: f32,
    pub cluster_threshold: f32,
    /// Per-cell mode: at most this many instances per cell.
    pub per_cell_max: u32,
    /// Floor texture slots the item may stand on (bit `i` = slot `i`; 0 = any).
    pub slot_mask: u32,
    pub seed: u32,
    /// Per-cell mode on hex grids: stand on the 7 whole small hexes of each cell (kg_hex top
    /// decorations, [`ScatterRules::small_hex_candidates`]) instead of jittered points.
    pub small_hex: bool,
}

impl Default for ScatterRules {
    fn default() -> Self {
        ScatterRules {
            density: 0.05,
            scale_min: 0.8,
            scale_max: 1.2,
            yaw_jitter: 360.0,
            hue_jitter: 0.03,
            value_jitter: 0.1,
            slope_min: 0.0,
            slope_max: 35.0,
            height_min: -1.0e6,
            height_max: 1.0e6,
            cluster_scale: 0.0,
            cluster_threshold: 0.0,
            per_cell_max: 1,
            slot_mask: 0,
            seed: 1,
            small_hex: false,
        }
    }
}

/// Small-hex spots of a cell: `(small hex, terrain-local XZ)` (`cells::decoration_spots_ex`).
pub type SpotsFn<'a> = &'a dyn Fn(IVec2) -> Vec<(IVec2, Vec2)>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScatterInstance {
    pub item: u16,
    /// Terrain-local XZ.
    pub pos: Vec2,
    /// Radians.
    pub yaw: f32,
    pub scale: f32,
    /// Hue shift in turns and value multiplier offset (shader: `INSTANCE_CUSTOM.rg`).
    pub hue: f32,
    pub value: f32,
}

/// Terrain surface query: height, unit normal and floor texture slot at a terrain-local XZ point.
pub type SurfaceFn<'a> = &'a dyn Fn(Vec2) -> Option<(f32, Vec3, u8)>;

fn h3(p: IVec2, item: u16, seed: u32, k: u32) -> f32 {
    hash01(p, seed.wrapping_mul(0x9E37_79B1) ^ (item as u32).wrapping_mul(0x85EB_CA77) ^ k.wrapping_mul(0xC2B2_AE3D))
}

impl ScatterRules {
    pub fn slope_ok(&self, normal: Vec3) -> bool {
        let a = normal.y.clamp(-1.0, 1.0).acos().to_degrees();
        a >= self.slope_min - 1e-3 && a <= self.slope_max + 1e-3
    }

    pub fn cluster_ok(&self, p: Vec2) -> bool {
        if self.cluster_scale <= 0.0 {
            return true;
        }
        fbm(p / self.cluster_scale, self.seed ^ 0xC1u32, 3) * 1.6 > self.cluster_threshold
    }

    pub fn slot_ok(&self, slot: u8) -> bool {
        self.slot_mask == 0 || (slot < 32 && self.slot_mask & (1 << slot) != 0)
    }

    /// Height, slope, ground slot and cluster filters.
    pub fn accepts(&self, p: Vec2, surface: SurfaceFn) -> bool {
        let Some((h, n, slot)) = surface(p) else { return false };
        h >= self.height_min && h <= self.height_max && self.slope_ok(n) && self.slot_ok(slot) && self.cluster_ok(p)
    }

    /// Deterministic yaw / scale / colour jitter for an instance keyed by `key`.
    pub fn make_instance(&self, item: u16, pos: Vec2, key: IVec2, k: u32) -> ScatterInstance {
        let r = |n: u32| h3(key, item, self.seed, k * 8 + n);
        let (lo, hi) = (self.scale_min.min(self.scale_max), self.scale_min.max(self.scale_max));
        ScatterInstance {
            item,
            pos,
            yaw: (r(3) - 0.5) * self.yaw_jitter.clamp(0.0, 360.0).to_radians(),
            scale: (lo + (hi - lo) * r(4)).max(0.01),
            hue: (r(5) * 2.0 - 1.0) * self.hue_jitter,
            value: (r(6) * 2.0 - 1.0) * self.value_jitter,
        }
    }

    /// Lattice spacing for painting at `density`.
    pub fn spacing(&self) -> f32 {
        1.0 / self.density.max(1e-4).sqrt()
    }

    /// Paint-mode candidates under a brush: one jittered point per lattice cell of
    /// [`ScatterRules::spacing`]; the brush weight is the acceptance probability, so falloff
    /// thins the edges. Deterministic: painting the same spot twice yields the same points
    /// (the set rejects duplicates).
    pub fn paint_candidates(&self, item: u16, brush: &Brush, center: Vec2, surface: SurfaceFn) -> Vec<ScatterInstance> {
        let s = self.spacing();
        let e = brush.half_extent();
        let (i0, i1) = (((center.x - e.x) / s).floor() as i32, ((center.x + e.x) / s).ceil() as i32);
        let (j0, j1) = (((center.y - e.y) / s).floor() as i32, ((center.y + e.y) / s).ceil() as i32);
        let mut out = Vec::new();
        if (i1 - i0) as i64 * (j1 - j0) as i64 > 400_000 {
            return out; // absurd density for this brush; refuse instead of stalling the editor
        }
        for j in j0..=j1 {
            for i in i0..=i1 {
                let key = iv2(i, j);
                let p = v2((i as f32 + h3(key, item, self.seed, 1)) * s, (j as f32 + h3(key, item, self.seed, 2)) * s);
                let w = brush.weight(p - center);
                if w <= 0.0 || h3(key, item, self.seed, 7) > w {
                    continue;
                }
                if self.accepts(p, surface) {
                    out.push(self.make_instance(item, p, key, 0));
                }
            }
        }
        out
    }

    /// Per-cell mode (hex_map_godot feature manager idea): every cell gets
    /// `floor(density * cell_area + hash)` instances (≤ `per_cell_max`), at hash-jittered
    /// positions inside the cell. Stable across reloads and identical on every client.
    pub fn cell_candidates(&self, item: u16, cfg: &GridConfig, cells: &[IVec2], surface: SurfaceFn) -> Vec<ScatterInstance> {
        let cs = cfg.cell_size_safe();
        let (area, inner) = match cfg.grid {
            GridType::Square => (cs.x * cs.y, cs.x.min(cs.y) * 0.5),
            GridType::Triangle => (cs.x * cs.x * SQRT3 / 4.0, cs.x * SQRT3 / 6.0),
            GridType::Hex | GridType::HexRings => (cs.x * cs.x * SQRT3 / 2.0, cs.x * 0.5),
        };
        let mut out = Vec::new();
        for c in cells {
            // Square grids store vertices: use the cell whose lower corner is the sample.
            let centre = if cfg.grid == GridType::Square { cfg.sample_pos(*c) + cs * 0.5 } else { cfg.sample_pos(*c) };
            let n = ((self.density * area + h3(*c, item, self.seed, 11)).floor() as u32).min(self.per_cell_max);
            let phase = h3(*c, item, self.seed, 12) * std::f32::consts::TAU;
            for k in 0..n {
                let off = if n == 1 {
                    let a = h3(*c, item, self.seed, 13 + k) * std::f32::consts::TAU;
                    v2(a.cos(), a.sin()) * inner * 0.35 * h3(*c, item, self.seed, 20 + k)
                } else {
                    let a = phase + std::f32::consts::TAU * k as f32 / n as f32;
                    let d = inner * (0.4 + 0.25 * h3(*c, item, self.seed, 30 + k));
                    v2(a.cos(), a.sin()) * d
                };
                let p = centre + off;
                if self.accepts(p, surface) {
                    out.push(self.make_instance(item, p, *c, k + 1));
                }
            }
        }
        out
    }
}

impl ScatterRules {
    /// Small-hex mode (BitCraft / kg_hex top decorations): instances stand exactly on the
    /// centres of each cell's whole small hexes as given by `spots` (which already skips
    /// uneven, submerged, paved or built-on ground). Each small hex is taken with probability
    /// `density × small-hex area` (≥ 1 = every spot), at most `per_cell_max` per cell (the
    /// lowest hashes win), keyed by the small hex itself so the choice and the yaw / scale /
    /// colour jitter are identical on every client.
    pub fn small_hex_candidates(&self, item: u16, cfg: &GridConfig, cells: &[IVec2], spots: SpotsFn, surface: SurfaceFn) -> Vec<ScatterInstance> {
        let w = cfg.cell_size_safe().x;
        let small_area = w * w * SQRT3 / 2.0 / 9.0;
        let chance = (self.density * small_area).clamp(0.0, 1.0);
        let mut out = Vec::new();
        for c in cells {
            let mut picks: Vec<(f32, IVec2, Vec2)> = spots(*c)
                .into_iter()
                .map(|(small, p)| (h3(small, item, self.seed, 41), small, p))
                .filter(|(r, _, p)| *r < chance && self.accepts(*p, surface))
                .collect();
            picks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            picks.truncate(self.per_cell_max.max(1) as usize);
            out.extend(picks.into_iter().map(|(_, small, p)| self.make_instance(item, p, small, 0)));
        }
        out
    }
}

/// Basis columns (x, y, z, scaled) and origin of an instance standing on the terrain.
/// `align` blends the up axis from world up (0) to the terrain normal (1).
pub fn instance_transform(inst: &ScatterInstance, height: f32, normal: Vec3, align: f32, y_offset: f32) -> ([Vec3; 3], Vec3) {
    let up0 = v3(0.0, 1.0, 0.0);
    let n = if normal.y > 0.0 { normal } else { up0 };
    let u = up0.lerp(n, align.clamp(0.0, 1.0)).normalized();
    let (s, c) = inst.yaw.sin_cos();
    let x0 = v3(c, 0.0, -s);
    let x = (x0 - u * x0.dot(u)).normalized();
    let z = x.cross(u);
    let k = inst.scale;
    ([x * k, u * k, z * k], v3(inst.pos.x, height, inst.pos.y) + u * (y_offset * k))
}

// --------------------------------------------------------------------------- storage

/// All scatter instances of a terrain, bucketed into square tiles of `tile` world units
/// (one MultiMesh per tile and item part).
#[derive(Clone, Debug, PartialEq)]
pub struct ScatterSet {
    pub tile: f32,
    pub tiles: BTreeMap<IVec2, Vec<ScatterInstance>>,
}

impl Default for ScatterSet {
    fn default() -> Self {
        ScatterSet::new(24.0)
    }
}

const MAGIC: &[u8; 4] = b"PTSC";
const VERSION: u8 = 1;

pub type TileSet = std::collections::BTreeSet<IVec2>;

impl ScatterSet {
    pub fn new(tile: f32) -> ScatterSet {
        ScatterSet { tile: tile.max(1.0), tiles: BTreeMap::new() }
    }

    pub fn tile_of(&self, p: Vec2) -> IVec2 {
        iv2((p.x / self.tile).floor() as i32, (p.y / self.tile).floor() as i32)
    }

    /// Tiles overlapping a terrain-local rectangle.
    pub fn tiles_in_rect(&self, min: Vec2, max: Vec2) -> Vec<IVec2> {
        let (a, b) = (self.tile_of(min), self.tile_of(max));
        let mut v = Vec::new();
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                v.push(iv2(x, y));
            }
        }
        v
    }

    pub fn len(&self) -> usize {
        self.tiles.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &ScatterInstance> {
        self.tiles.values().flatten()
    }

    pub fn count_item(&self, item: u16) -> usize {
        self.iter().filter(|i| i.item == item).count()
    }

    /// Is an instance of `item` within `r` of `p`?
    pub fn any_within(&self, item: u16, p: Vec2, r: f32) -> bool {
        let rr = v2(r, r);
        self.tiles_in_rect(p - rr, p + rr)
            .iter()
            .filter_map(|t| self.tiles.get(t))
            .flatten()
            .any(|i| i.item == item && (i.pos - p).length_squared() < r * r)
    }

    pub fn insert(&mut self, inst: ScatterInstance) -> IVec2 {
        let t = self.tile_of(inst.pos);
        self.tiles.entry(t).or_default().push(inst);
        t
    }

    /// Adds candidates that are not within `min_dist` of an instance of the same item.
    pub fn add_unique(&mut self, cands: Vec<ScatterInstance>, min_dist: f32) -> TileSet {
        let mut t = TileSet::new();
        for c in cands {
            if !self.any_within(c.item, c.pos, min_dist) {
                t.insert(self.insert(c));
            }
        }
        t
    }

    /// Removes the instances (of `item`, or all items) for which `inside` is true, searching the
    /// tiles of `[min, max]`.
    pub fn remove_where(&mut self, min: Vec2, max: Vec2, item: Option<u16>, inside: impl Fn(Vec2) -> bool) -> TileSet {
        let mut t = TileSet::new();
        for tile in self.tiles_in_rect(min, max) {
            let Some(v) = self.tiles.get_mut(&tile) else { continue };
            let before = v.len();
            v.retain(|i| !(item.is_none_or(|it| it == i.item) && inside(i.pos)));
            if v.len() != before {
                t.insert(tile);
            }
            if v.is_empty() {
                self.tiles.remove(&tile);
            }
        }
        t
    }

    /// Erases under a brush (weight > 0).
    pub fn erase(&mut self, brush: &Brush, center: Vec2, item: Option<u16>) -> TileSet {
        let e = brush.half_extent();
        self.remove_where(center - e, center + e, item, |p| brush.weight(p - center) > 0.0)
    }

    /// Paints `item` under a brush.
    pub fn paint(&mut self, rules: &ScatterRules, item: u16, brush: &Brush, center: Vec2, surface: SurfaceFn) -> TileSet {
        let c = rules.paint_candidates(item, brush, center, surface);
        self.add_unique(c, rules.spacing() * 0.5)
    }

    /// Per-cell mode: replaces `item`'s instances in `cells` by the deterministic per-cell set.
    pub fn fill_cells(&mut self, rules: &ScatterRules, item: u16, cfg: &GridConfig, cells: &[IVec2], surface: SurfaceFn) -> TileSet {
        let cands = rules.cell_candidates(item, cfg, cells, surface);
        self.replace_cells(item, cfg, cells, cands)
    }

    /// Small-hex mode: replaces `item`'s instances in `cells` by the small-hex set
    /// ([`ScatterRules::small_hex_candidates`]).
    pub fn fill_small_hexes(&mut self, rules: &ScatterRules, item: u16, cfg: &GridConfig, cells: &[IVec2], spots: SpotsFn, surface: SurfaceFn) -> TileSet {
        let cands = rules.small_hex_candidates(item, cfg, cells, spots, surface);
        self.replace_cells(item, cfg, cells, cands)
    }

    /// Removes the instances (any item) standing on small hexes for which `blocked` is true
    /// (paved / built-on ground). Returns the tiles touched.
    pub fn remove_on_small_hexes(&mut self, cfg: &GridConfig, min: Vec2, max: Vec2, blocked: &dyn Fn(IVec2) -> bool) -> TileSet {
        self.remove_where(min, max, None, |p| blocked(cfg.world_to_small_hex(p)))
    }

    fn replace_cells(&mut self, item: u16, cfg: &GridConfig, cells: &[IVec2], cands: Vec<ScatterInstance>) -> TileSet {
        if cells.is_empty() {
            return TileSet::new();
        }
        let set: std::collections::HashSet<IVec2> = cells.iter().copied().collect();
        let pad = cfg.cell_size_safe() * 2.0;
        let (mut min, mut max) = (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN));
        for c in cells {
            let p = cfg.sample_pos(*c);
            min = v2(min.x.min(p.x), min.y.min(p.y));
            max = v2(max.x.max(p.x), max.y.max(p.y));
        }
        let owner = |p: Vec2| {
            if cfg.grid == GridType::Square {
                let cs = cfg.cell_size_safe();
                iv2((p.x / cs.x).floor() as i32, (p.y / cs.y).floor() as i32)
            } else {
                cfg.world_to_sample(p)
            }
        };
        let mut t = self.remove_where(min - pad, max + pad, Some(item), |p| set.contains(&owner(p)));
        for c in cands {
            t.insert(self.insert(c));
        }
        t
    }

    /// Re-indexes instances after the item list changed: `map[old] = Some(new)` keeps the
    /// instances of item `old` as item `new`, `None` (or an index past `map`) drops them.
    pub fn remap_items(&mut self, map: &[Option<u16>]) {
        for v in self.tiles.values_mut() {
            v.retain_mut(|i| match map.get(i.item as usize).copied().flatten() {
                Some(n) => {
                    i.item = n;
                    true
                }
                None => false,
            });
        }
        self.tiles.retain(|_, v| !v.is_empty());
    }

    /// Drops instances of `item` and shifts higher item indices down (item list edits).
    pub fn remove_item(&mut self, item: u16) {
        for v in self.tiles.values_mut() {
            v.retain(|i| i.item != item);
            for i in v.iter_mut() {
                if i.item > item {
                    i.item -= 1;
                }
            }
        }
        self.tiles.retain(|_, v| !v.is_empty());
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(13 + self.len() * 20);
        b.extend_from_slice(MAGIC);
        b.push(VERSION);
        b.extend_from_slice(&self.tile.to_le_bytes());
        b.extend_from_slice(&(self.len() as u32).to_le_bytes());
        for i in self.iter() {
            b.extend_from_slice(&i.item.to_le_bytes());
            for f in [i.pos.x, i.pos.y, i.yaw, i.scale] {
                b.extend_from_slice(&f.to_le_bytes());
            }
            b.push(((i.hue * 254.0).round().clamp(-127.0, 127.0) as i8) as u8);
            b.push(((i.value * 127.0).round().clamp(-127.0, 127.0) as i8) as u8);
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<ScatterSet> {
        if b.len() < 13 || &b[0..4] != MAGIC || b[4] != VERSION {
            return None;
        }
        let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let mut s = ScatterSet::new(f(5));
        let n = u32::from_le_bytes([b[9], b[10], b[11], b[12]]) as usize;
        if b.len() != 13 + n * 20 {
            return None;
        }
        for k in 0..n {
            let o = 13 + k * 20;
            s.insert(ScatterInstance {
                item: u16::from_le_bytes([b[o], b[o + 1]]),
                pos: v2(f(o + 2), f(o + 6)),
                yaw: f(o + 10),
                scale: f(o + 14),
                hue: (b[o + 18] as i8) as f32 / 254.0,
                value: (b[o + 19] as i8) as f32 / 127.0,
            });
        }
        Some(s)
    }
}

// --------------------------------------------------------------------------- built-in meshes

/// Built-in low-poly prop meshes (flat-shaded, vertex-coloured, ~1 unit tall/wide, origin at
/// the base), so the addon ships usable scatter items without any assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropShape {
    Rock,
    Bush,
    Tree,
    Pine,
    Flowers,
}

pub const PROP_SHAPES: [&str; 6] = ["Custom", "Rock", "Bush", "Tree", "Pine", "Flowers"];

impl PropShape {
    /// `0` = custom mesh / scene (None).
    pub fn from_i64(v: i64) -> Option<PropShape> {
        match v {
            1 => Some(PropShape::Rock),
            2 => Some(PropShape::Bush),
            3 => Some(PropShape::Tree),
            4 => Some(PropShape::Pine),
            5 => Some(PropShape::Flowers),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<[f32; 4]>,
}

impl PropMesh {
    /// Adds a triangle facing away from `inside` (Godot front faces are clockwise seen from
    /// outside), with a flat normal.
    fn tri_out(&mut self, a: Vec3, b: Vec3, c: Vec3, inside: Vec3, col: [f32; 3]) {
        let ccw = (b - a).cross(c - a);
        let centroid = (a + b + c) * (1.0 / 3.0);
        let out = if ccw.dot(centroid - inside) >= 0.0 { 1.0 } else { -1.0 };
        let n = ccw.normalized() * out;
        let (b, c) = if out > 0.0 { (c, b) } else { (b, c) };
        for p in [a, b, c] {
            self.positions.push(p);
            self.normals.push(n);
            self.colors.push([col[0], col[1], col[2], 1.0]);
        }
    }

    /// Jittered low-poly blob (subdivided octahedron) centred at `c`.
    fn blob(&mut self, c: Vec3, r: Vec3, col: [f32; 3], seed: u32, rough: f32) {
        let dirs = [v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0)];
        let (top, bot) = (v3(0.0, 1.0, 0.0), v3(0.0, -1.0, 0.0));
        let mut faces = Vec::new();
        for i in 0..4 {
            let (a, b) = (dirs[i], dirs[(i + 1) % 4]);
            faces.push([top, b, a]);
            faces.push([bot, a, b]);
        }
        // One subdivision.
        let mut sub = Vec::new();
        for [a, b, c2] in faces {
            let (ab, bc, ca) = ((a + b).normalized(), (b + c2).normalized(), (c2 + a).normalized());
            sub.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c2], [ab, bc, ca]]);
        }
        let jitter = |p: Vec3| {
            let k = iv2((p.x * 97.0 + p.y * 13.0).round() as i32, (p.z * 89.0 - p.y * 31.0).round() as i32);
            let f = 1.0 + (hash01(k, seed) - 0.5) * 2.0 * rough;
            c + v3(p.x * r.x, p.y * r.y, p.z * r.z) * f
        };
        for (k, [a, b, c2]) in sub.into_iter().enumerate() {
            let shade = 0.9 + 0.2 * hash01(iv2(k as i32, 7), seed);
            let col = [col[0] * shade, col[1] * shade, col[2] * shade];
            self.tri_out(jitter(a), jitter(b), jitter(c2), c, col);
        }
    }

    /// `n`-sided cone/frustum from `y0` (radius `r0`) to `y1` (radius `r1`), with caps.
    fn frustum(&mut self, n: usize, y0: f32, r0: f32, y1: f32, r1: f32, col: [f32; 3], twist: f32) {
        let ring = |y: f32, r: f32, i: usize| {
            let a = std::f32::consts::TAU * i as f32 / n as f32 + twist;
            v3(a.cos() * r, y, a.sin() * r)
        };
        let mid = v3(0.0, (y0 + y1) * 0.5, 0.0);
        let dark = [col[0] * 0.7, col[1] * 0.7, col[2] * 0.7];
        for i in 0..n {
            let j = (i + 1) % n;
            let (a0, b0, a1, b1) = (ring(y0, r0, i), ring(y0, r0, j), ring(y1, r1, i), ring(y1, r1, j));
            if r1 > 1e-4 {
                self.tri_out(a0, a1, b1, mid, col);
                self.tri_out(v3(0.0, y1, 0.0), a1, b1, mid, col);
            }
            // For a cone b1 == a1 is the apex.
            self.tri_out(a0, b1, b0, mid, col);
            self.tri_out(v3(0.0, y0, 0.0), b0, a0, mid, dark);
        }
    }

    pub fn build(shape: PropShape, seed: u32) -> PropMesh {
        let mut m = PropMesh::default();
        match shape {
            PropShape::Rock => {
                m.blob(v3(0.0, 0.25, 0.0), v3(0.55, 0.42, 0.45), [0.56, 0.54, 0.52], seed, 0.28);
                m.blob(v3(0.35, 0.12, 0.2), v3(0.25, 0.2, 0.22), [0.55, 0.53, 0.5], seed ^ 3, 0.3);
            }
            PropShape::Bush => {
                let g = [0.3, 0.55, 0.22];
                m.blob(v3(0.0, 0.35, 0.0), v3(0.5, 0.38, 0.5), g, seed, 0.2);
                m.blob(v3(0.32, 0.28, 0.12), v3(0.32, 0.28, 0.32), [0.34, 0.6, 0.25], seed ^ 5, 0.2);
                m.blob(v3(-0.25, 0.25, -0.2), v3(0.3, 0.26, 0.3), [0.27, 0.5, 0.2], seed ^ 9, 0.2);
            }
            PropShape::Tree => {
                m.frustum(6, 0.0, 0.14, 1.1, 0.09, [0.45, 0.31, 0.2], 0.0);
                m.blob(v3(0.0, 1.55, 0.0), v3(0.75, 0.62, 0.75), [0.24, 0.46, 0.2], seed, 0.18);
                m.blob(v3(0.25, 2.0, -0.1), v3(0.45, 0.4, 0.45), [0.29, 0.52, 0.23], seed ^ 7, 0.18);
            }
            PropShape::Pine => {
                m.frustum(5, 0.0, 0.11, 0.5, 0.08, [0.42, 0.29, 0.19], 0.0);
                let g = [0.2, 0.42, 0.26];
                for (k, (y0, r, h)) in [(0.35, 0.7, 1.0), (0.95, 0.55, 0.9), (1.5, 0.38, 0.8)].into_iter().enumerate() {
                    let s = 0.92 + 0.1 * k as f32;
                    m.frustum(7, y0, r, y0 + h, 0.0, [g[0] * s, g[1] * s, g[2] * s], k as f32 * 0.4);
                }
            }
            PropShape::Flowers => {
                let colors = [[0.95, 0.85, 0.3], [0.9, 0.45, 0.55], [0.85, 0.85, 0.95], [0.65, 0.5, 0.9]];
                for k in 0..5 {
                    let a = std::f32::consts::TAU * hash01(iv2(k, 1), seed);
                    let d = 0.15 + 0.3 * hash01(iv2(k, 2), seed);
                    let (x, z) = (a.cos() * d, a.sin() * d);
                    let h = 0.25 + 0.2 * hash01(iv2(k, 3), seed);
                    let stem = [0.3, 0.55, 0.22];
                    // Two crossed stem blades (both sides).
                    for (a, b) in [(v3(x - 0.03, 0.0, z), v3(x + 0.03, 0.0, z)), (v3(x, 0.0, z - 0.03), v3(x, 0.0, z + 0.03))] {
                        let top = v3(x, h, z);
                        let side = (b - a).cross(v3(0.0, 1.0, 0.0));
                        m.tri_out(a, top, b, top + side, stem);
                        m.tri_out(a, top, b, top - side, stem);
                    }
                    let c = colors[(hash01(iv2(k, 4), seed) * 4.0) as usize % 4];
                    m.blob(v3(x, h, z), v3(0.08, 0.05, 0.08), c, seed ^ k as u32, 0.1);
                }
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_items_drops_and_reindexes() {
        let mut set = ScatterSet::new(8.0);
        for item in 0..3u16 {
            set.insert(ScatterInstance { item, pos: v2(item as f32, 0.0), yaw: 0.0, scale: 1.0, hue: 0.0, value: 0.0 });
        }
        // Item 1 removed: 0 stays, 2 becomes 1.
        set.remap_items(&[Some(0), None, Some(1)]);
        let mut items: Vec<(u16, f32)> = set.iter().map(|i| (i.item, i.pos.x)).collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert_eq!(items, vec![(0, 0.0), (1, 2.0)]);
    }

    fn flat(h: f32) -> impl Fn(Vec2) -> Option<(f32, Vec3, u8)> {
        move |_| Some((h, v3(0.0, 1.0, 0.0), 2))
    }

    #[test]
    fn paint_is_deterministic_and_does_not_stack() {
        let rules = ScatterRules { density: 0.5, ..Default::default() };
        let brush = Brush { radius: 6.0, falloff: false, ..Default::default() };
        let surf = flat(0.0);
        let mut s = ScatterSet::default();
        let t = s.paint(&rules, 0, &brush, v2(10.0, 10.0), &surf);
        let n = s.len();
        assert!(!t.is_empty());
        // Area ≈ 113 m² at 0.5 / m².
        assert!(n > 35 && n < 80, "{n}");
        s.paint(&rules, 0, &brush, v2(10.0, 10.0), &surf);
        assert_eq!(s.len(), n, "repainting the same spot adds nothing");
        let mut s2 = ScatterSet::default();
        s2.paint(&rules, 0, &brush, v2(10.0, 10.0), &surf);
        assert_eq!(s, s2);
        // A second item paints independently.
        s.paint(&rules, 1, &brush, v2(10.0, 10.0), &surf);
        assert!(s.count_item(1) > 30);
        s.erase(&brush, v2(10.0, 10.0), Some(1));
        assert_eq!(s.count_item(1), 0);
        assert_eq!(s.len(), n);
    }

    #[test]
    fn filters_reject_slopes_heights_and_clusters() {
        let steep = |_: Vec2| Some((0.0, v3(0.7071, 0.7071, 0.0), 0u8));
        let rules = ScatterRules { slope_max: 30.0, ..Default::default() };
        assert!(!rules.accepts(v2(0.0, 0.0), &steep));
        let r2 = ScatterRules { slope_min: 40.0, slope_max: 50.0, ..Default::default() };
        assert!(r2.accepts(v2(0.0, 0.0), &steep));
        let low = ScatterRules { height_min: 1.0, ..Default::default() };
        assert!(!low.accepts(v2(0.0, 0.0), &flat(0.5)));
        let sand_only = ScatterRules { slot_mask: 1 << 3, ..Default::default() };
        assert!(!sand_only.accepts(v2(0.0, 0.0), &flat(0.0)));
        assert!(ScatterRules { slot_mask: 1 << 2, ..Default::default() }.accepts(v2(0.0, 0.0), &flat(0.0)));
        let clustered = ScatterRules { density: 1.0, cluster_scale: 8.0, cluster_threshold: 0.2, ..Default::default() };
        let brush = Brush { radius: 30.0, falloff: false, ..Default::default() };
        let all = ScatterRules { density: 1.0, ..Default::default() }.paint_candidates(0, &brush, v2(0.0, 0.0), &flat(0.0)).len();
        let some = clustered.paint_candidates(0, &brush, v2(0.0, 0.0), &flat(0.0)).len();
        assert!(some > 0 && some < all / 2, "{some} of {all}");
    }

    #[test]
    fn per_cell_mode_is_stable_and_bounded() {
        let cfg = GridConfig { grid: GridType::HexRings, cell_size: v2(2.0, 2.0), ring_radius: 3, ..Default::default() };
        let cells = crate::grid::disk(iv2(0, 0), 3);
        let rules = ScatterRules { density: 0.6, per_cell_max: 2, ..Default::default() };
        let mut a = ScatterSet::default();
        a.fill_cells(&rules, 0, &cfg, &cells, &flat(0.0));
        let n = a.len();
        assert!(n >= cells.len() && n <= cells.len() * 2, "{n}");
        // Re-filling replaces instead of adding; instances stay inside their cell.
        a.fill_cells(&rules, 0, &cfg, &cells, &flat(0.0));
        assert_eq!(a.len(), n);
        for i in a.iter() {
            assert!(cells.contains(&cfg.world_to_sample(i.pos)));
        }
        let mut b = ScatterSet::default();
        b.fill_cells(&rules, 0, &cfg, &cells, &flat(0.0));
        assert_eq!(a, b);
    }

    #[test]
    fn small_hex_mode_uses_the_seven_spots() {
        use crate::cells::{EdgeStyle, decoration_spots_ex};
        use crate::data::{MergeMode, TerrainStore};
        let cfg = GridConfig { grid: GridType::HexRings, cell_size: v2(6.0, 6.0), ring_radius: 2, ..Default::default() };
        let mut store = TerrainStore::new(cfg.clone());
        store.edges.style = EdgeStyle::HexPrism;
        store.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        let cells = cfg.chunk_cells(iv2(0, 0));
        // The spots are the small hexes 3c + AXIAL[k] (centre first) at their exact centres.
        let c = iv2(1, -1);
        let sp = decoration_spots_ex(&store, c, -1.0, &|_, _| false);
        assert_eq!(sp.len(), 7);
        assert_eq!(sp[0].0, iv2(3, -3));
        for (small, p) in &sp {
            assert_eq!(cfg.small_hex_owner(*small), c);
            assert!((cfg.small_hex_pos(*small) - p.xz()).length() < 1e-4);
        }
        // A blocked (paved) small hex is skipped.
        assert_eq!(decoration_spots_ex(&store, c, -1.0, &|s, _| s == iv2(3, -3)).len(), 6);
        let spots = |c: IVec2| decoration_spots_ex(&store, c, -1.0, &|_, _| false).into_iter().map(|(s, p)| (s, p.xz())).collect();
        // Density high enough for every spot, capped per cell.
        let rules = ScatterRules { density: 10.0, per_cell_max: 7, small_hex: true, ..Default::default() };
        let mut a = ScatterSet::default();
        a.fill_small_hexes(&rules, 0, &cfg, &cells, &spots, &flat(0.0));
        assert_eq!(a.len(), cells.len() * 7);
        for i in a.iter() {
            let s = cfg.world_to_small_hex(i.pos);
            assert!((cfg.small_hex_pos(s) - i.pos).length() < 1e-4, "on a small-hex centre");
        }
        let capped = ScatterRules { per_cell_max: 2, ..rules.clone() };
        let mut b = ScatterSet::default();
        b.fill_small_hexes(&capped, 0, &cfg, &cells, &spots, &flat(0.0));
        assert_eq!(b.len(), cells.len() * 2);
        let mut b2 = ScatterSet::default();
        b2.fill_small_hexes(&capped, 0, &cfg, &cells, &spots, &flat(0.0));
        assert_eq!(b, b2, "deterministic");
        // Paving removes the props standing there.
        let t = a.remove_on_small_hexes(&cfg, v2(-50.0, -50.0), v2(50.0, 50.0), &|s| s == iv2(3, -3));
        assert!(!t.is_empty());
        assert_eq!(a.len(), cells.len() * 7 - 1);
    }

    #[test]
    fn bytes_round_trip() {
        let mut s = ScatterSet::new(16.0);
        s.insert(ScatterInstance { item: 3, pos: v2(-5.5, 40.25), yaw: 1.0, scale: 1.3, hue: 0.1, value: -0.2 });
        s.insert(ScatterInstance { item: 0, pos: v2(1.0, 2.0), yaw: -2.0, scale: 0.7, hue: 0.0, value: 0.0 });
        let b = ScatterSet::from_bytes(&s.to_bytes()).unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b.tile, 16.0);
        let i = b.iter().find(|i| i.item == 3).unwrap();
        assert_eq!(i.pos, v2(-5.5, 40.25));
        assert!((i.hue - 0.1).abs() < 0.01 && (i.value + 0.2).abs() < 0.01);
        assert!(ScatterSet::from_bytes(b"nope").is_none());
    }

    #[test]
    fn transform_aligns_to_normal_and_keeps_scale() {
        let inst = ScatterInstance { item: 0, pos: v2(1.0, 2.0), yaw: 0.3, scale: 2.0, hue: 0.0, value: 0.0 };
        let n = v3(0.0, 1.0, 1.0).normalized();
        let ([x, y, z], o) = instance_transform(&inst, 5.0, n, 1.0, 0.0);
        assert!((y.normalized().dot(n) - 1.0).abs() < 1e-4);
        assert!((x.length() - 2.0).abs() < 1e-4 && x.dot(y).abs() < 1e-3 && z.dot(y).abs() < 1e-3);
        assert_eq!(o, v3(1.0, 5.0, 2.0));
        let ([_, y0, _], _) = instance_transform(&inst, 0.0, n, 0.0, 0.0);
        assert!((y0.normalized().y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn prop_meshes_are_closed_enough_and_upright() {
        for k in 1..=5 {
            let m = PropMesh::build(PropShape::from_i64(k).unwrap(), 7);
            assert!(m.positions.len() >= 30 && m.positions.len() % 3 == 0);
            assert!(m.positions.iter().all(|p| p.y > -0.3 && p.y < 3.0));
            assert_eq!(m.positions.len(), m.colors.len());
        }
    }
}
