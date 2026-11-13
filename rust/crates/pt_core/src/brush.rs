//! Brush footprints and terrain edit operations. Every operation returns the set of chunks
//! whose meshes must be rebuilt.

use crate::data::TerrainStore;
use crate::grid::GridConfig;
use crate::math::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BrushShape {
    #[default]
    Round = 0,
    Square = 1,
    Hexagon = 2,
    Hexagon30 = 3,
    Triangle = 4,
    Triangle180 = 5,
}

impl BrushShape {
    pub fn from_i64(v: i64) -> BrushShape {
        match v {
            1 => BrushShape::Square,
            2 => BrushShape::Hexagon,
            3 => BrushShape::Hexagon30,
            4 => BrushShape::Triangle,
            5 => BrushShape::Triangle180,
            _ => BrushShape::Round,
        }
    }

    /// Normalized distance of offset `d` (in radii) from the brush centre: `<= 1` is inside.
    pub fn distance(self, d: Vec2) -> f32 {
        let h = 0.866_025_4;
        let dir = |deg: f32| {
            let a = deg.to_radians();
            v2(a.cos(), a.sin())
        };
        match self {
            BrushShape::Round => d.length(),
            BrushShape::Square => d.x.abs().max(d.y.abs()),
            BrushShape::Hexagon => [30.0, 90.0, 150.0].iter().map(|a| d.dot(dir(*a)).abs()).fold(0.0, f32::max) / h,
            BrushShape::Hexagon30 => [0.0, 60.0, 120.0].iter().map(|a| d.dot(dir(*a)).abs()).fold(0.0, f32::max) / h,
            BrushShape::Triangle => [90.0, 210.0, 330.0].iter().map(|a| d.dot(dir(*a))).fold(f32::MIN, f32::max) / 0.5,
            BrushShape::Triangle180 => [270.0, 30.0, 150.0].iter().map(|a| d.dot(dir(*a))).fold(f32::MIN, f32::max) / 0.5,
        }
    }

    /// Outline polygon (unit radius) for gizmo drawing.
    pub fn outline(self, segments: usize) -> Vec<Vec2> {
        let poly = |n: usize, start: f32| -> Vec<Vec2> {
            (0..n).map(|i| v2(0.0, 0.0) + Vec2 { x: 1.0, y: 0.0 }.rotated((start + 360.0 / n as f32 * i as f32).to_radians())).collect()
        };
        match self {
            BrushShape::Round => poly(segments.max(8), 0.0),
            BrushShape::Square => vec![v2(-1.0, -1.0), v2(1.0, -1.0), v2(1.0, 1.0), v2(-1.0, 1.0)],
            BrushShape::Hexagon => poly(6, 0.0),
            BrushShape::Hexagon30 => poly(6, 30.0),
            BrushShape::Triangle => poly(3, 270.0),
            BrushShape::Triangle180 => poly(3, 90.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub shape: BrushShape,
    pub radius: f32,
    pub falloff: bool,
    /// Godot `ease()` curve applied to the falloff (1 = linear, <0 = in-out).
    pub falloff_ease: f32,
    pub grid_aligned: bool,
    pub grid_size: i32,
    /// Anisotropy (Minor Deity radius X / Z): the Z radius is `radius * aspect` (1 = isotropic).
    pub aspect: f32,
    /// Brush rotation around Y in degrees (counter-clockwise seen from above, +X towards +Z).
    pub rotation: f32,
    /// Peak curve applied after the falloff: `> 0` pointy (mountains), `< 0` flat-topped
    /// (mesas), 0 = unchanged. See [`peak_curve`].
    pub peak: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Brush { shape: BrushShape::Round, radius: 6.0, falloff: true, falloff_ease: -1.6, grid_aligned: false, grid_size: 1, aspect: 1.0, rotation: 0.0, peak: 0.0 }
    }
}

/// Minor Deity-style "peak" contrast of a brush weight `w` in [0, 1]: `p > 0` sharpens the
/// centre (`w^(1+4p)`), `p < 0` widens the plateau (`1-(1-w)^(1+4|p|)`).
pub fn peak_curve(w: f32, p: f32) -> f32 {
    let w = w.clamp(0.0, 1.0);
    let p = p.clamp(-1.0, 1.0);
    if p > 0.0 {
        w.powf(1.0 + 4.0 * p)
    } else if p < 0.0 {
        1.0 - (1.0 - w).powf(1.0 + 4.0 * -p)
    } else {
        w
    }
}

impl Brush {
    /// Radii along the brush's own X and Z axes.
    pub fn radii(&self) -> Vec2 {
        let r = self.radius.max(0.05);
        v2(r, (r * self.aspect.clamp(0.05, 20.0)).max(0.05))
    }

    /// Offset from the centre (terrain-local XZ) → unit brush space (`shape.distance() <= 1` inside).
    pub fn to_unit(&self, d: Vec2) -> Vec2 {
        let r = self.radii();
        let l = d.rotated(-self.rotation.to_radians());
        v2(l.x / r.x, l.y / r.y)
    }

    /// Unit brush space → offset from the centre (inverse of [`Brush::to_unit`]).
    pub fn from_unit(&self, u: Vec2) -> Vec2 {
        let r = self.radii();
        v2(u.x * r.x, u.y * r.y).rotated(self.rotation.to_radians())
    }

    /// Half extent of the axis-aligned box enclosing the (rotated, stretched) brush.
    pub fn half_extent(&self) -> Vec2 {
        let r = self.radii();
        let (s, c) = self.rotation.to_radians().sin_cos();
        // Shapes fit in the unit square, so the rotated rectangle bounds them.
        v2((r.x * c).abs() + (r.y * s).abs(), (r.x * s).abs() + (r.y * c).abs()) * 1.0001
    }

    /// Weight of the brush at offset `d` from its centre (0 outside).
    pub fn weight(&self, d: Vec2) -> f32 {
        let dist = self.shape.distance(self.to_unit(d));
        if dist > 1.0 {
            return 0.0;
        }
        let w = if self.falloff { 1.0 - ease(dist, self.falloff_ease) } else { 1.0 };
        peak_curve(w.clamp(0.0, 1.0), self.peak)
    }

    /// Outline of the brush (offsets from the centre, terrain-local XZ).
    pub fn outline(&self, segments: usize) -> Vec<Vec2> {
        self.shape.outline(segments).into_iter().map(|u| self.from_unit(u)).collect()
    }
}

/// Weighted samples under a brush centred at terrain-local `center`.
pub fn footprint(cfg: &GridConfig, brush: &Brush, center: Vec2) -> Vec<(IVec2, f32)> {
    if brush.grid_aligned && cfg.grid.is_cell_mode() {
        return cfg.grid_aligned(center, brush.grid_size).into_iter().map(|c| (c, 1.0)).collect();
    }
    let e = brush.half_extent();
    let mut out = Vec::new();
    for s in cfg.samples_in_rect(center - e, center + e) {
        let w = brush.weight(cfg.sample_pos(s) - center);
        if w > 0.0 {
            out.push((s, w.clamp(0.0, 1.0)));
        }
    }
    if out.is_empty() {
        // Small brushes always hit the nearest sample.
        out.push((cfg.world_to_sample(center), 1.0));
    }
    out
}

pub type Touched = BTreeSet<IVec2>;

fn touch(store: &TerrainStore, s: IVec2, t: &mut Touched) {
    for c in store.chunks_affected_by(s) {
        t.insert(c);
    }
}

/// Height clamp used by the raise/lower tool ("ceiling" and "floor").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clamp {
    pub floor: f32,
    pub ceiling: f32,
}

impl Default for Clamp {
    fn default() -> Self {
        Clamp { floor: f32::MIN, ceiling: f32::MAX }
    }
}

/// MST "set" brush: moves every sample by `offset * weight`, or (flatten) to
/// `base + offset` blended by weight.
pub fn op_offset(store: &mut TerrainStore, fp: &[(IVec2, f32)], base: &[(IVec2, f32)], offset: f32, flatten: Option<f32>) -> Touched {
    let mut t = Touched::new();
    for (i, (s, w)) in fp.iter().enumerate() {
        let original = base.get(i).map(|b| b.1).unwrap_or_else(|| store.height(*s).unwrap_or(0.0));
        let h = match flatten {
            Some(level) => original + ((level + offset) - original) * w,
            None => original + offset * w,
        };
        if store.set_height(*s, h) {
            touch(store, *s, &mut t);
        }
    }
    t
}

/// Minor Deity-style continuous raise (`amount > 0`) / lower (`amount < 0`), clamped.
pub fn op_raise(store: &mut TerrainStore, fp: &[(IVec2, f32)], amount: f32, clamp: Clamp) -> Touched {
    let mut t = Touched::new();
    for (s, w) in fp {
        let Some(h) = store.height(*s) else { continue };
        let mut nh = h + amount * w;
        if amount > 0.0 {
            nh = nh.min(clamp.ceiling.max(h));
        } else {
            nh = nh.max(clamp.floor.min(h));
        }
        if nh != h && store.set_height(*s, nh) {
            touch(store, *s, &mut t);
        }
    }
    t
}

/// Moves samples towards `target` by `strength * weight` (1 = snap).
pub fn op_level(store: &mut TerrainStore, fp: &[(IVec2, f32)], target: f32, strength: f32) -> Touched {
    let mut t = Touched::new();
    for (s, w) in fp {
        let Some(h) = store.height(*s) else { continue };
        let k = (strength * w).clamp(0.0, 1.0);
        let nh = h + (target - h) * k;
        if (nh - h).abs() > 1e-6 && store.set_height(*s, nh) {
            touch(store, *s, &mut t);
        }
    }
    t
}

/// Relaxes samples towards the average of their neighbours.
pub fn op_smooth(store: &mut TerrainStore, fp: &[(IVec2, f32)], strength: f32) -> Touched {
    let mut targets = Vec::with_capacity(fp.len());
    for (s, w) in fp {
        let Some(h) = store.height(*s) else { continue };
        let (mut sum, mut n) = (h, 1.0);
        for nb in store.config.sample_neighbors(*s) {
            if let Some(nh) = store.height(nb) {
                sum += nh;
                n += 1.0;
            }
        }
        targets.push((*s, h + (sum / n - h) * (strength * w).clamp(0.0, 1.0)));
    }
    let mut t = Touched::new();
    for (s, h) in targets {
        if store.set_height(s, h) {
            touch(store, s, &mut t);
        }
    }
    t
}

/// Builds a ramp ("bridge") from `a` to `b` (terrain-local, y = heights) of width
/// `brush.radius * 2`. Heights follow `ease(t, ease_curve)`. In cell modes the cells are
/// flagged smooth so the bridge renders as a continuous slope.
pub fn op_bridge(store: &mut TerrainStore, brush: &Brush, a: Vec3, b: Vec3, ease_curve: f32) -> Touched {
    let cfg = store.config.clone();
    let (pa, pb) = (a.xz(), b.xz());
    let len = (pb - pa).length();
    let r = brush.radius.max(0.05);
    let min = v2(pa.x.min(pb.x), pa.y.min(pb.y)) - v2(r, r);
    let max = v2(pa.x.max(pb.x), pa.y.max(pb.y)) + v2(r, r);
    let mut t = Touched::new();
    for s in cfg.samples_in_rect(min, max) {
        let p = cfg.sample_pos(s);
        let tt = if len > 1e-5 { ((p - pa).dot(pb - pa) / (len * len)).clamp(0.0, 1.0) } else { 0.0 };
        let closest = pa.lerp(pb, tt);
        let d = (p - closest).length() / r;
        if d > 1.0 {
            continue;
        }
        let h = a.y + (b.y - a.y) * ease(tt, if ease_curve == 0.0 { 1.0 } else { ease_curve });
        let smooth = cfg.grid.is_cell_mode();
        let touched = store.edit_sample(s, |c, i| {
            c.heights[i] = h;
            if smooth {
                c.smooth[i] = 1;
            }
        });
        if !touched.is_empty() {
            touch(store, s, &mut t);
        }
    }
    t
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintTarget {
    Floor,
    Wall,
    Grass,
    Smooth,
}

/// Paints a texture slot / grass density / smooth flag where the brush weight exceeds 0.5
/// (or everywhere under the brush when falloff is off).
pub fn op_paint(store: &mut TerrainStore, fp: &[(IVec2, f32)], target: PaintTarget, value: u8) -> Touched {
    let mut t = Touched::new();
    for (s, w) in fp {
        if *w < 0.5 {
            continue;
        }
        let touched = store.edit_sample(*s, |c, i| match target {
            PaintTarget::Floor => c.floor_tex[i] = value,
            PaintTarget::Wall => c.wall_tex[i] = value,
            PaintTarget::Grass => c.grass[i] = value,
            PaintTarget::Smooth => c.smooth[i] = value.min(1),
        });
        if !touched.is_empty() {
            touch(store, *s, &mut t);
        }
    }
    t
}

/// Slope of a sample in degrees: the height gradient for square grids (central differences),
/// the steepest step to an edge neighbour for cell grids (flat cells next to a cliff count as
/// steep, which is what "rock on slopes" style filters want).
pub fn sample_slope_deg(store: &TerrainStore, s: IVec2) -> Option<f32> {
    let h = store.height(s)?;
    let cfg = &store.config;
    let grad = if cfg.grid.is_cell_mode() {
        let p = cfg.sample_pos(s);
        cfg.edge_neighbors(s)
            .into_iter()
            .filter_map(|n| store.height(n).map(|hn| (hn - h).abs() / (cfg.sample_pos(n) - p).length().max(1e-4)))
            .fold(0.0, f32::max)
    } else {
        let cs = cfg.cell_size_safe();
        let d = |a: IVec2, b: IVec2, step: f32| match (store.height(a), store.height(b)) {
            (Some(ha), Some(hb)) => (ha - hb) / (2.0 * step),
            (Some(ha), None) => (ha - h) / step,
            (None, Some(hb)) => (h - hb) / step,
            _ => 0.0,
        };
        let gx = d(s + iv2(1, 0), s - iv2(1, 0), cs.x);
        let gz = d(s + iv2(0, 1), s - iv2(0, 1), cs.y);
        (gx * gx + gz * gz).sqrt()
    };
    Some(grad.atan().to_degrees())
}

/// Terrain3D-style slope range filter: keeps the samples whose slope lies in
/// `[min_deg, max_deg]`. `(0, 90)` keeps everything.
pub fn filter_slope(store: &TerrainStore, fp: &[(IVec2, f32)], min_deg: f32, max_deg: f32) -> Vec<(IVec2, f32)> {
    if min_deg <= 0.0 && max_deg >= 90.0 {
        return fp.to_vec();
    }
    fp.iter()
        .copied()
        .filter(|(s, _)| sample_slope_deg(store, *s).is_some_and(|a| a >= min_deg && a <= max_deg))
        .collect()
}

/// Terrain3D-style gradient: pulls samples towards the straight ramp from `a` to `b`
/// (terrain-local, y = heights; beyond the ends the end height holds) by `strength * weight`.
pub fn op_gradient(store: &mut TerrainStore, fp: &[(IVec2, f32)], a: Vec3, b: Vec3, strength: f32) -> Touched {
    let (pa, pb) = (a.xz(), b.xz());
    let len2 = (pb - pa).length_squared();
    let mut t = Touched::new();
    for (s, w) in fp {
        let Some(h) = store.height(*s) else { continue };
        let p = store.config.sample_pos(*s);
        let k = if len2 > 1e-8 { ((p - pa).dot(pb - pa) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let target = a.y + (b.y - a.y) * k;
        let nh = h + (target - h) * (strength * w).clamp(0.0, 1.0);
        if (nh - h).abs() > 1e-6 && store.set_height(*s, nh) {
            touch(store, *s, &mut t);
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::{GridConfig, GridType};

    fn store(grid: GridType) -> TerrainStore {
        let mut s = TerrainStore::new(GridConfig { grid, chunk_size: 8, ring_radius: 3, ..Default::default() });
        s.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        s
    }

    #[test]
    fn shapes_contain_centre_and_exclude_far_points() {
        for sh in 0..6 {
            let s = BrushShape::from_i64(sh);
            assert!(s.distance(v2(0.0, 0.0)) <= 1e-5);
            assert!(s.distance(v2(3.0, 0.0)) > 1.0);
            for p in s.outline(16) {
                assert!((s.distance(p) - 1.0).abs() < 0.02, "{s:?} outline on boundary: {}", s.distance(p));
            }
        }
    }

    #[test]
    fn raise_respects_ceiling() {
        let mut s = store(GridType::Square);
        let fp = footprint(&s.config, &Brush { falloff: false, ..Default::default() }, v2(8.0, 8.0));
        for _ in 0..10 {
            op_raise(&mut s, &fp, 1.0, Clamp { floor: -10.0, ceiling: 3.0 });
        }
        assert!(fp.iter().all(|(c, _)| s.height(*c) == Some(3.0)));
        op_raise(&mut s, &fp, -5.0, Clamp { floor: 1.0, ceiling: 3.0 });
        assert!(fp.iter().all(|(c, _)| s.height(*c) == Some(1.0)));
    }

    #[test]
    fn smooth_reduces_spike() {
        let mut s = store(GridType::Hex);
        let c = s.config.global_of(iv2(0, 0), iv2(4, 4));
        s.set_height(c, 10.0);
        let fp = vec![(c, 1.0)];
        op_smooth(&mut s, &fp, 1.0);
        assert!(s.height(c).unwrap() < 10.0);
    }

    #[test]
    fn bridge_marks_cells_smooth_and_slopes() {
        let mut s = store(GridType::HexRings);
        let a = s.config.sample_pos(s.config.global_of(iv2(0, 0), iv2(1, 3)));
        let b = s.config.sample_pos(s.config.global_of(iv2(0, 0), iv2(5, 3)));
        let t = op_bridge(&mut s, &Brush { radius: 1.0, ..Default::default() }, v3(a.x, 0.0, a.y), v3(b.x, 4.0, b.y), 1.0);
        assert!(!t.is_empty());
        let mid = s.config.world_to_sample(a.lerp(b, 0.5));
        let sm = s.sample(mid).unwrap();
        assert!(sm.smooth && sm.height > 0.5 && sm.height < 3.5);
    }

    #[test]
    fn grid_aligned_hex_footprint_is_a_disk() {
        let s = store(GridType::Hex);
        let fp = footprint(&s.config, &Brush { grid_aligned: true, grid_size: 2, ..Default::default() }, v2(6.0, 6.0));
        assert_eq!(fp.len(), 7);
    }

    #[test]
    fn ellipse_brush_is_stretched_and_rotated() {
        let b = Brush { radius: 2.0, aspect: 3.0, falloff: false, ..Default::default() };
        assert!(b.weight(v2(0.0, 5.0)) > 0.0, "inside along Z (rz = 6)");
        assert_eq!(b.weight(v2(3.0, 0.0)), 0.0, "outside along X (rx = 2)");
        let r = Brush { rotation: 90.0, ..b };
        assert!(r.weight(v2(5.0, 0.0)) > 0.0, "rotated: long axis along X");
        assert_eq!(r.weight(v2(0.0, 3.0)), 0.0);
        let e = r.half_extent();
        assert!((e.x - 6.0).abs() < 0.01 && (e.y - 2.0).abs() < 0.01);
        for p in r.outline(32) {
            assert!((r.shape.distance(r.to_unit(p)) - 1.0).abs() < 0.02);
        }
    }

    #[test]
    fn peak_sharpens_or_flattens() {
        assert!(peak_curve(0.5, 1.0) < 0.5);
        assert!(peak_curve(0.5, -1.0) > 0.5);
        assert_eq!(peak_curve(0.5, 0.0), 0.5);
        for p in [-1.0, -0.3, 0.0, 0.4, 1.0] {
            assert!((peak_curve(1.0, p) - 1.0).abs() < 1e-6 && peak_curve(0.0, p).abs() < 1e-6);
        }
    }

    #[test]
    fn slope_filter_and_gradient() {
        let mut s = store(GridType::Square);
        // A ramp along x: slope atan(1/2) with 2 m cells and 1 m per sample.
        for z in 0..=8 {
            for x in 0..=8 {
                s.set_height(iv2(x, z), x as f32);
            }
        }
        let a = sample_slope_deg(&s, iv2(4, 4)).unwrap();
        assert!((a - 0.5f32.atan().to_degrees()).abs() < 0.1, "{a}");
        let fp = vec![(iv2(4, 4), 1.0)];
        assert_eq!(filter_slope(&s, &fp, 0.0, 20.0).len(), 0);
        assert_eq!(filter_slope(&s, &fp, 20.0, 40.0).len(), 1);
        // Gradient from (0,0,h=10) to (16,0,h=0): x = 8 m -> 5.
        let fp = vec![(iv2(4, 2), 1.0)];
        op_gradient(&mut s, &fp, v3(0.0, 10.0, 0.0), v3(16.0, 0.0, 0.0), 1.0);
        assert!((s.height(iv2(4, 2)).unwrap() - 5.0).abs() < 1e-4);
    }
}
