//! Shape generators. Every shape sits on the XZ plane: its footprint is centred on the origin
//! and its bottom is at `y = 0`, so placing a shape means moving its node to a ground point
//! (e.g. a terrain cell centre). Generated meshes share vertices and are closed (except `plane`).

use super::model::BMesh;
use super::prims;
use crate::grid::{self, GridConfig, GridType, SQRT3};
use crate::math::*;
use std::collections::{BTreeSet, HashMap};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ShapeKind {
    #[default]
    Box = 0,
    Plane = 1,
    Cylinder = 2,
    Cone = 3,
    Sphere = 4,
    Stairs = 5,
    Arch = 6,
    Wedge = 7,
    Pipe = 8,
    HexPrism = 9,
    HexCluster = 10,
    Torus = 11,
    Door = 12,
    CurvedStairs = 13,
    Trim = 14,
    Wall = 15,
    /// Unibuilder `Kind.POLY`: a clicked polygon extruded `size.y` up.
    Poly = 16,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 17] = [
        ShapeKind::Box,
        ShapeKind::Plane,
        ShapeKind::Cylinder,
        ShapeKind::Cone,
        ShapeKind::Sphere,
        ShapeKind::Stairs,
        ShapeKind::Arch,
        ShapeKind::Wedge,
        ShapeKind::Pipe,
        ShapeKind::HexPrism,
        ShapeKind::HexCluster,
        ShapeKind::Torus,
        ShapeKind::Door,
        ShapeKind::CurvedStairs,
        ShapeKind::Trim,
        ShapeKind::Wall,
        ShapeKind::Poly,
    ];
    pub fn from_i64(v: i64) -> ShapeKind {
        Self::ALL.get(v.max(0) as usize).copied().unwrap_or_default()
    }
    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Box => "Box",
            ShapeKind::Plane => "Plane",
            ShapeKind::Cylinder => "Cylinder",
            ShapeKind::Cone => "Cone",
            ShapeKind::Sphere => "Sphere",
            ShapeKind::Stairs => "Stairs",
            ShapeKind::Arch => "Arch",
            ShapeKind::Wedge => "Ramp",
            ShapeKind::Pipe => "Pipe",
            ShapeKind::HexPrism => "Hex prism",
            ShapeKind::HexCluster => "Hex cluster",
            ShapeKind::Torus => "Torus",
            ShapeKind::Door => "Door",
            ShapeKind::CurvedStairs => "Curved stairs",
            ShapeKind::Trim => "Trim",
            ShapeKind::Wall => "Wall",
            ShapeKind::Poly => "Poly shape",
        }
    }
    /// Which `ShapeSpec` fields the shape reads (for UI): (segments, steps, thickness, smooth, rings).
    pub fn uses(self) -> (bool, bool, bool, bool, bool) {
        use ShapeKind::*;
        match self {
            Box | Wedge => (false, false, false, false, false),
            Plane => (true, false, false, false, false),
            Cylinder | Cone => (true, false, false, true, false),
            Sphere => (true, true, false, true, false),
            Stairs => (false, true, false, false, false),
            Arch => (true, false, true, false, false),
            Pipe => (true, false, true, true, false),
            HexPrism => (false, false, false, false, false),
            HexCluster => (false, false, false, false, true),
            Torus => (true, true, true, false, false),
            Door => (true, false, false, true, false),
            CurvedStairs => (false, true, false, false, false),
            Trim => (true, false, true, true, false),
            Wall => (false, false, true, false, false),
            Poly => (false, false, false, false, false),
        }
    }
    /// Extra fields read by the Unibuilder-port shapes: (arc + inner, opening + door height +
    /// arched, trim profile, wall side).
    pub fn uses_extra(self) -> (bool, bool, bool, bool) {
        match self {
            ShapeKind::CurvedStairs => (true, false, false, false),
            ShapeKind::Door => (false, true, false, false),
            ShapeKind::Trim => (false, false, true, false),
            ShapeKind::Wall => (false, false, false, true),
            _ => (false, false, false, false),
        }
    }
    /// Shapes drawn by clicking a polyline instead of dragging a footprint.
    pub fn is_polyline(self) -> bool {
        matches!(self, ShapeKind::Wall | ShapeKind::Poly)
    }
    /// Shapes whose height does not follow `size.y` (the torus is as tall as its tube).
    pub fn fixed_height(self) -> bool {
        self == ShapeKind::Torus
    }
    /// True for shapes whose footprint comes from the terrain grid.
    pub fn is_grid_shape(self) -> bool {
        matches!(self, ShapeKind::HexPrism | ShapeKind::HexCluster)
    }
}

/// Flat parameter set shared by all shapes (each kind reads the fields it needs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSpec {
    pub kind: ShapeKind,
    /// Width (x), height (y), depth (z). Hex prisms use `x` as the across-flats width.
    pub size: Vec3,
    /// Radial segments / plane subdivisions / arch segments.
    pub segments: u32,
    /// Stair steps / sphere rings.
    pub steps: u32,
    /// Wall thickness for arch and pipe.
    pub thickness: f32,
    pub smooth: bool,
    /// Hex cluster disk radius in cells.
    pub rings: u32,
    /// Curved stairs sweep in degrees.
    pub arc: f32,
    /// Curved stairs inner radius as a fraction of the outer one.
    pub inner: f32,
    /// Door: doorway width and height.
    pub opening: f32,
    pub door_height: f32,
    /// Door: round-topped doorway.
    pub arched: bool,
    /// Trim cross-section (`prims::TrimProfile`).
    pub profile: u8,
    /// Wall side of the drawn line: 0 centre, 1 left, 2 right.
    pub side: u8,
}

impl Default for ShapeSpec {
    fn default() -> Self {
        ShapeSpec {
            kind: ShapeKind::Box,
            size: v3(1.0, 1.0, 1.0),
            segments: 12,
            steps: 6,
            thickness: 0.25,
            smooth: true,
            rings: 1,
            arc: 90.0,
            inner: 0.4,
            opening: 0.9,
            door_height: 2.1,
            arched: false,
            profile: 1,
            side: 0,
        }
    }
}

/// Builds a shape. Grid shapes use `grid` (cell size, topology) when given, else a hex grid with
/// `size.x` across flats.
pub fn generate(spec: &ShapeSpec, grid: Option<&GridConfig>) -> BMesh {
    let s = v3(spec.size.x.max(0.01), spec.size.y.max(0.01), spec.size.z.max(0.01));
    let seg = spec.segments.clamp(3, 128);
    match spec.kind {
        ShapeKind::Box => cuboid(s),
        ShapeKind::Plane => plane(v2(s.x, s.z), spec.segments.clamp(1, 64)),
        ShapeKind::Cylinder => cylinder(s, seg, spec.smooth),
        ShapeKind::Cone => cone(s, seg, spec.smooth),
        ShapeKind::Sphere => sphere(s, seg, spec.steps.clamp(2, 64), spec.smooth),
        ShapeKind::Stairs => stairs(s, spec.steps.clamp(1, 128)),
        ShapeKind::Arch => arch(s, spec.thickness, spec.segments.clamp(2, 64)),
        ShapeKind::Wedge => wedge(s),
        ShapeKind::Pipe => pipe(s, spec.thickness, seg, spec.smooth),
        // The clicked outline is not part of the recipe: re-edits get its bounding square.
        ShapeKind::Poly => {
            let (x, z) = (s.x * 0.5, s.z * 0.5);
            super::trim_walls::polygon_prism(&[v2(-x, -z), v2(x, -z), v2(x, z), v2(-x, z)], s.y)
        }
        ShapeKind::Torus | ShapeKind::Door | ShapeKind::CurvedStairs | ShapeKind::Trim | ShapeKind::Wall => unibuilder_shape(spec, s),
        ShapeKind::HexPrism | ShapeKind::HexCluster => {
            let fallback = GridConfig { grid: GridType::HexRings, cell_size: v2(s.x, s.x), ..Default::default() };
            let cfg = grid.filter(|g| g.grid.is_cell_mode()).cloned().unwrap_or(fallback);
            if spec.kind == ShapeKind::HexPrism && cfg.grid != GridType::Triangle {
                hex_prism(cfg.hex_radius(), s.y)
            } else {
                let r = if spec.kind == ShapeKind::HexPrism { 0 } else { spec.rings.min(16) as i32 };
                let cells = cluster_cells(&cfg, iv2(0, 0), r);
                cells_prism(&cfg, &cells, iv2(0, 0), s.y)
            }
        }
    }
}

fn wall_side(v: u8) -> prims::WallSide {
    match v {
        1 => prims::WallSide::Left,
        2 => prims::WallSide::Right,
        _ => prims::WallSide::Centre,
    }
}

/// A wall along a polyline (`points` on the ground, y = 0 local), see `prims::make_wall`.
pub fn wall_polyline(spec: &ShapeSpec, points: &[Vec3], closed: bool) -> BMesh {
    prims::make_wall(points, closed, spec.thickness, spec.size.y, wall_side(spec.side))
}

/// Torus, door, curved stairs, trim and wall: Unibuilder's `build_shape` parameter mapping
/// (`editor/shapes/shape_tool.gd`), then moved so the bottom sits on y = 0.
fn unibuilder_shape(spec: &ShapeSpec, s: Vec3) -> BMesh {
    let (su, sv, ah) = (s.x, s.z, s.y);
    let mut m = match spec.kind {
        ShapeKind::Torus => {
            let extent = su.max(sv) * 0.5;
            let tube = spec.thickness.clamp(0.001, extent * 0.45);
            prims::make_torus(extent - tube, tube, spec.segments.clamp(3, 64), spec.steps.clamp(3, 64))
        }
        ShapeKind::Door => {
            // Unibuilder spans the door along Z; turn it so the span follows the drawn width.
            let ped = ah - spec.door_height;
            let side = (su - spec.opening) * 0.5;
            let mut d = prims::make_door(su, ah, sv, ped.clamp(0.01, ah * 0.95), side.clamp(0.01, su * 0.5 * 0.95), spec.arched, spec.segments.clamp(2, 64), spec.smooth);
            for p in &mut d.positions {
                *p = v3(p.z, p.y, -p.x);
            }
            d
        }
        ShapeKind::CurvedStairs => {
            let outer = su.max(sv);
            let mut c = prims::make_curved_stairs((spec.inner.clamp(0.01, 0.95)) * outer, outer, ah, spec.steps.clamp(1, 64), spec.arc.clamp(5.0, 355.0));
            prims::fit_xz(&mut c, outer);
            c
        }
        ShapeKind::Trim => prims::make_trim(sv, su, spec.thickness.max(0.001), ah, prims::TrimProfile::from_u8(spec.profile), spec.segments.clamp(1, 32), false, false, false, spec.smooth),
        _ => wall_polyline(spec, &[v3(-su * 0.5, 0.0, 0.0), v3(su * 0.5, 0.0, 0.0)], false),
    };
    if let Some((lo, _)) = m.bounds() {
        m.translate(v3(0.0, -lo.y, 0.0));
    }
    m
}

/// Extrudes a 2D profile (any winding) along `c` by `len`; profile point `(a, b)` maps to
/// `origin + ax * a + bx * b`. Side faces get smoothing group `smooth`.
pub fn extrude_profile(profile: &[Vec2], ax: Vec3, bx: Vec3, c: Vec3, origin: Vec3, len: f32, smooth: u16) -> BMesh {
    let mut pts = profile.to_vec();
    let area: f32 = (0..pts.len()).map(|i| pts[i].x * pts[(i + 1) % pts.len()].y - pts[(i + 1) % pts.len()].x * pts[i].y).sum();
    if area < 0.0 {
        pts.reverse();
    }
    let n = pts.len();
    let mut m = BMesh::default();
    let map = |p: Vec2| origin + ax * p.x + bx * p.y;
    for p in &pts {
        m.add_vert(map(*p));
    }
    for p in &pts {
        m.add_vert(map(*p) + c * len);
    }
    let cn = c.normalized();
    m.add_face_facing((0..n as u32).collect(), -cn);
    m.add_face_facing((n as u32..2 * n as u32).collect(), cn);
    for i in 0..n {
        let j = (i + 1) % n;
        let d = pts[j] - pts[i];
        let out = ax * d.y - bx * d.x;
        let f = m.add_face_facing(vec![i as u32, j as u32, (j + n) as u32, (i + n) as u32], out);
        m.faces[f].smooth = smooth;
    }
    m
}

/// Vertical prism of an XZ outline from `y0` to `y1`.
pub fn prism_y(outline: &[Vec2], y0: f32, y1: f32, smooth: u16) -> BMesh {
    extrude_profile(outline, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), Vec3::UP, v3(0.0, y0, 0.0), y1 - y0, smooth)
}

pub fn cuboid(s: Vec3) -> BMesh {
    let (hx, hz) = (s.x * 0.5, s.z * 0.5);
    prism_y(&[v2(-hx, -hz), v2(hx, -hz), v2(hx, hz), v2(-hx, hz)], 0.0, s.y, 0)
}

/// Upward-facing plane subdivided `div` × `div`.
pub fn plane(size: Vec2, div: u32) -> BMesh {
    let mut m = BMesh::default();
    let d = div.max(1);
    for j in 0..=d {
        for i in 0..=d {
            m.add_vert(v3((i as f32 / d as f32 - 0.5) * size.x, 0.0, (j as f32 / d as f32 - 0.5) * size.y));
        }
    }
    let w = d + 1;
    for j in 0..d {
        for i in 0..d {
            let a = j * w + i;
            m.add_face_facing(vec![a, a + 1, a + w + 1, a + w], Vec3::UP);
        }
    }
    m
}

fn ring(n: u32, rx: f32, rz: f32) -> Vec<Vec2> {
    // Half-step rotation so a flat side faces +Z (reads better in a fixed iso camera).
    (0..n).map(|i| {
        let a = TAU * (i as f32 + 0.5) / n as f32;
        v2(rx * a.cos(), rz * a.sin())
    })
    .collect()
}

pub fn cylinder(s: Vec3, segments: u32, smooth: bool) -> BMesh {
    prism_y(&ring(segments.max(3), s.x * 0.5, s.z * 0.5), 0.0, s.y, smooth as u16)
}

pub fn cone(s: Vec3, segments: u32, smooth: bool) -> BMesh {
    let n = segments.max(3);
    let mut m = BMesh::default();
    for p in ring(n, s.x * 0.5, s.z * 0.5) {
        m.add_vert(v3(p.x, 0.0, p.y));
    }
    let apex = m.add_vert(v3(0.0, s.y, 0.0));
    m.add_face_facing((0..n).collect(), -Vec3::UP);
    for i in 0..n {
        let j = (i + 1) % n;
        let mid = (m.positions[i as usize] + m.positions[j as usize]) * 0.5;
        let f = m.add_face_facing(vec![i, j, apex], v3(mid.x, 0.0, mid.z));
        m.faces[f].smooth = smooth as u16;
    }
    m
}

/// Ellipsoid resting on y = 0.
pub fn sphere(s: Vec3, segments: u32, rings: u32, smooth: bool) -> BMesh {
    let (n, r) = (segments.max(3), rings.max(2));
    let c = v3(0.0, s.y * 0.5, 0.0);
    let mut m = BMesh::default();
    let top = m.add_vert(c + v3(0.0, s.y * 0.5, 0.0));
    let bottom = m.add_vert(c - v3(0.0, s.y * 0.5, 0.0));
    for k in 1..r {
        let phi = PI * k as f32 / r as f32;
        for p in ring(n, s.x * 0.5 * phi.sin(), s.z * 0.5 * phi.sin()) {
            m.add_vert(c + v3(p.x, s.y * 0.5 * phi.cos(), p.y));
        }
    }
    let at = |k: u32, i: u32| 2 + (k - 1) * n + (i % n);
    let sg = smooth as u16;
    for i in 0..n {
        let mut faces = vec![vec![top, at(1, i), at(1, i + 1)], vec![bottom, at(r - 1, i), at(r - 1, i + 1)]];
        for k in 1..r - 1 {
            faces.push(vec![at(k, i), at(k, i + 1), at(k + 1, i + 1), at(k + 1, i)]);
        }
        for f in faces {
            let ctr = f.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / f.len() as f32;
            let fi = m.add_face_facing(f, ctr - c);
            m.faces[fi].smooth = sg;
        }
    }
    m
}

/// Straight stairs rising towards +Z.
pub fn stairs(s: Vec3, steps: u32) -> BMesh {
    let n = steps.max(1);
    let (d, h) = (s.z / n as f32, s.y / n as f32);
    let mut prof = vec![v2(0.0, 0.0), v2(s.z, 0.0), v2(s.z, s.y)];
    for i in (0..n).rev() {
        prof.push(v2(i as f32 * d, (i + 1) as f32 * h));
        if i > 0 {
            prof.push(v2(i as f32 * d, i as f32 * h));
        }
    }
    // (z, y) profile extruded along x.
    extrude_profile(&prof, v3(0.0, 0.0, 1.0), Vec3::UP, v3(1.0, 0.0, 0.0), v3(-s.x * 0.5, 0.0, -s.z * 0.5), s.x, 0)
}

/// Ramp rising towards +Z.
pub fn wedge(s: Vec3) -> BMesh {
    let prof = [v2(0.0, 0.0), v2(s.z, 0.0), v2(s.z, s.y)];
    extrude_profile(&prof, v3(0.0, 0.0, 1.0), Vec3::UP, v3(1.0, 0.0, 0.0), v3(-s.x * 0.5, 0.0, -s.z * 0.5), s.x, 0)
}

/// Wall with a round-topped opening; the opening runs along Z (depth = `size.z`).
pub fn arch(s: Vec3, thickness: f32, segments: u32) -> BMesh {
    let hw = s.x * 0.5;
    let t = thickness.clamp(0.02, hw * 0.9).min(s.y * 0.9);
    let r = (hw - t).min(s.y - t).max(0.01);
    let spring = (s.y - t - r).max(0.0);
    let mut prof = vec![v2(-hw, 0.0), v2(-r, 0.0)];
    if spring > 1e-4 {
        prof.push(v2(-r, spring));
    }
    for k in 1..segments.max(2) {
        let a = PI - PI * k as f32 / segments.max(2) as f32;
        prof.push(v2(r * a.cos(), spring + r * a.sin()));
    }
    if spring > 1e-4 {
        prof.push(v2(r, spring));
    }
    prof.extend([v2(r, 0.0), v2(hw, 0.0), v2(hw, s.y), v2(-hw, s.y)]);
    extrude_profile(&prof, v3(1.0, 0.0, 0.0), Vec3::UP, v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -s.z * 0.5), s.z, 0)
}

/// Hollow cylinder.
pub fn pipe(s: Vec3, thickness: f32, segments: u32, smooth: bool) -> BMesh {
    let n = segments.max(3);
    let (rx, rz) = (s.x * 0.5, s.z * 0.5);
    let t = thickness.clamp(0.01, rx.min(rz) * 0.95);
    let outer = ring(n, rx, rz);
    let inner = ring(n, rx - t, rz - t);
    let mut m = BMesh::default();
    for (pts, y) in [(&outer, 0.0), (&outer, s.y), (&inner, 0.0), (&inner, s.y)] {
        for p in pts.iter() {
            m.add_vert(v3(p.x, y, p.y));
        }
    }
    let (ob, ot, ib, it) = (0, n, 2 * n, 3 * n);
    for i in 0..n {
        let j = (i + 1) % n;
        let dir = {
            let mid = (outer[i as usize] + outer[j as usize]) * 0.5;
            v3(mid.x, 0.0, mid.y)
        };
        let a = m.add_face_facing(vec![ob + i, ob + j, ot + j, ot + i], dir);
        m.faces[a].smooth = smooth as u16;
        let b = m.add_face_facing(vec![ib + i, ib + j, it + j, it + i], -dir);
        m.faces[b].smooth = 2 * smooth as u16;
        m.add_face_facing(vec![ot + i, ot + j, it + j, it + i], Vec3::UP);
        m.add_face_facing(vec![ob + i, ob + j, ib + j, ib + i], -Vec3::UP);
    }
    m
}

/// Pointy-top hexagon prism with circumradius `radius`, matching the corners of the terrain's
/// hex cells (`GridConfig::cell_corners`).
pub fn hex_prism(radius: f32, height: f32) -> BMesh {
    let pts: Vec<Vec2> = (0..6)
        .map(|i| {
            let a = (-30.0f32 + 60.0 * i as f32).to_radians();
            v2(radius * a.cos(), radius * a.sin())
        })
        .collect();
    prism_y(&pts, 0.0, height, 0)
}

/// Across-flats width of a hex with circumradius `r`.
pub fn hex_width(r: f32) -> f32 {
    r * SQRT3
}

/// Cells within `radius` steps of `center` on any cell-mode grid (hex disks for hex grids).
pub fn cluster_cells(cfg: &GridConfig, center: IVec2, radius: i32) -> Vec<IVec2> {
    match cfg.grid {
        GridType::HexRings => grid::disk(center, radius),
        GridType::Hex => {
            let a = grid::offset_to_axial(center);
            grid::disk(a, radius).into_iter().map(grid::axial_to_offset).collect()
        }
        _ => {
            // Breadth-first over edge neighbours.
            let mut set = BTreeSet::from([center]);
            let mut frontier = vec![center];
            for _ in 0..radius {
                let mut next = Vec::new();
                for c in frontier {
                    for nb in cfg.edge_neighbors(c) {
                        if set.insert(nb) {
                            next.push(nb);
                        }
                    }
                }
                frontier = next;
            }
            set.into_iter().collect()
        }
    }
}

/// Solid made of whole grid cells: one top and one bottom face per cell (so single cells can be
/// extruded into towers later) and side walls only along the outline. Positions are relative to
/// the centre of `origin` (terrain-local XZ of the cell centre maps to x/z = 0).
pub fn cells_prism(cfg: &GridConfig, cells: &[IVec2], origin: IVec2, height: f32) -> BMesh {
    let set: BTreeSet<IVec2> = cells.iter().copied().collect();
    let o = cfg.sample_pos(origin);
    let mut m = BMesh::default();
    let mut index: HashMap<(i64, i64, bool), u32> = HashMap::new();
    let mut vid = |m: &mut BMesh, p: Vec2, top: bool| -> u32 {
        let k = (((p.x - o.x) * 2048.0).round() as i64, ((p.y - o.y) * 2048.0).round() as i64, top);
        *index.entry(k).or_insert_with(|| m.add_vert(v3(p.x - o.x, if top { height } else { 0.0 }, p.y - o.y)))
    };
    for c in &set {
        let corners = cfg.cell_corners(*c);
        let bottom: Vec<u32> = corners.iter().map(|p| vid(&mut m, *p, false)).collect();
        let top: Vec<u32> = corners.iter().map(|p| vid(&mut m, *p, true)).collect();
        m.add_face_facing(top.clone(), Vec3::UP);
        m.add_face_facing(bottom.clone(), -Vec3::UP);
        let nbs = cfg.edge_neighbors(*c);
        let centre = cfg.sample_pos(*c);
        for (k, nb) in nbs.iter().enumerate() {
            if set.contains(nb) {
                continue;
            }
            let (i, j) = cfg.edge_corners(*c, k);
            let mid = (corners[i] + corners[j]) * 0.5 - centre;
            m.add_face_facing(vec![bottom[i], bottom[j], top[j], top[i]], v3(mid.x, 0.0, mid.y));
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_closed(m: &BMesh, name: &str) {
        m.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(m.is_closed(), "{name} not closed");
        assert!(m.volume() > 0.0, "{name} inside out: {}", m.volume());
    }

    #[test]
    fn every_shape_is_valid_and_closed() {
        for k in ShapeKind::ALL {
            let spec = ShapeSpec { kind: k, size: v3(2.0, 3.0, 2.5), ..Default::default() };
            let m = generate(&spec, None);
            if k == ShapeKind::Plane {
                m.validate().unwrap();
                continue;
            }
            check_closed(&m, k.name());
            if k.fixed_height() {
                continue;
            }
            let (lo, hi) = m.bounds().unwrap();
            assert!(lo.y.abs() < 1e-5 && (hi.y - 3.0).abs() < 1e-4, "{}: y {lo:?} {hi:?}", k.name());
        }
    }

    #[test]
    fn box_volume_and_stairs_volume() {
        assert!((cuboid(v3(2.0, 3.0, 4.0)).volume() - 24.0).abs() < 1e-3);
        // n steps fill (n+1)/(2n) of the bounding box.
        let s = stairs(v3(1.0, 4.0, 4.0), 4);
        assert!((s.volume() - 16.0 * 5.0 / 8.0).abs() < 1e-3, "{}", s.volume());
    }

    #[test]
    fn hex_prism_matches_terrain_cells() {
        for g in [GridType::Hex, GridType::HexRings] {
            let cfg = GridConfig { grid: g, cell_size: v2(2.0, 2.0), ..Default::default() };
            let m = hex_prism(cfg.hex_radius(), 1.0);
            let c = iv2(3, 5);
            let ctr = cfg.sample_pos(c);
            for corner in cfg.cell_corners(c) {
                let rel = corner - ctr;
                assert!(m.positions.iter().any(|p| (p.x - rel.x).abs() < 1e-4 && (p.z - rel.y).abs() < 1e-4));
            }
            assert!((hex_width(cfg.hex_radius()) - 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn hex_cluster_is_watertight_on_every_grid() {
        for g in [GridType::Hex, GridType::HexRings, GridType::Triangle] {
            let cfg = GridConfig { grid: g, cell_size: v2(2.0, 2.0), ..Default::default() };
            let cells = cluster_cells(&cfg, iv2(1, 1), 2);
            let m = cells_prism(&cfg, &cells, iv2(1, 1), 1.5);
            check_closed(&m, g.name());
            let area_top: f32 = (0..m.faces.len()).filter(|f| m.face_normal(*f).y > 0.9).map(|f| m.face_area(f)).sum();
            assert!((m.volume() - area_top * 1.5).abs() < 1e-2);
            if g != GridType::Triangle {
                assert_eq!(cells.len(), 19);
            }
        }
    }
}
