use super::model::BMesh;
use super::prims;
use super::xg_shapes;
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
    Poly = 16,
    Prism = 17,
    Sprite = 18,
    Icosphere = 19,
}
impl ShapeKind {
    pub const ALL: [ShapeKind; 20] = [
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
        ShapeKind::Prism,
        ShapeKind::Sprite,
        ShapeKind::Icosphere,
    ];
    pub fn from_i64(v: i64) -> ShapeKind {
        Self::ALL
            .get(v.max(0) as usize)
            .copied()
            .unwrap_or_default()
    }
    pub fn moniker(self) -> &'static str {
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
            ShapeKind::Prism => "Prism",
            ShapeKind::Sprite => "Sprite",
            ShapeKind::Icosphere => "Icosphere",
        }
    }
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
            Poly | Prism | Sprite => (false, false, false, false, false),
            Icosphere => (false, false, false, true, false),
        }
    }
    pub fn uses_modeler(self) -> XgUses {
        use ShapeKind::*;
        XgUses {
            segments_z: self == Plane,
            height_cuts: matches!(self, Cylinder | Pipe),
            torus_arcs: self == Torus,
            arch_band: self == Arch,
            step_height: matches!(self, Stairs | CurvedStairs),
            equal_steps: self == Stairs,
            subdivisions: self == Icosphere,
        }
    }
    pub fn uses_extra(self) -> (bool, bool, bool, bool) {
        match self {
            ShapeKind::CurvedStairs => (true, false, false, false),
            ShapeKind::Door => (false, true, false, false),
            ShapeKind::Trim => (false, false, true, false),
            ShapeKind::Wall => (false, false, false, true),
            _ => (false, false, false, false),
        }
    }
    pub fn is_polyline(self) -> bool {
        matches!(self, ShapeKind::Wall | ShapeKind::Poly)
    }
    pub fn fixed_height(self) -> bool {
        self == ShapeKind::Torus
    }
    pub fn is_grid_shape(self) -> bool {
        matches!(self, ShapeKind::HexPrism | ShapeKind::HexCluster)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XgUses {
    pub segments_z: bool,
    pub height_cuts: bool,
    pub torus_arcs: bool,
    pub arch_band: bool,
    pub step_height: bool,
    pub equal_steps: bool,
    pub subdivisions: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSpec {
    pub sort: ShapeKind,
    pub size: Vec3,
    pub stretches: u32,
    pub steps: u32,
    pub thickness: f32,
    pub glide: bool,
    pub rings: u32,
    pub arc: f32,
    pub inner: f32,
    pub opening: f32,
    pub door_height: f32,
    pub arched: bool,
    pub silhouette: u8,
    pub flank: u8,
    pub segments_z: u32,
    pub height_cuts: u32,
    pub torus_arc: f32,
    pub tube_arc: f32,
    pub arch_band: bool,
    pub arch_degrees: f32,
    pub end_caps: bool,
    pub step_height: f32,
    pub equal_steps: bool,
    pub subdivisions: u32,
}
impl Default for ShapeSpec {
    fn default() -> Self {
        ShapeSpec {
            sort: ShapeKind::Box,
            size: v3(1.0, 1.0, 1.0),
            stretches: 13,
            steps: 7,
            thickness: 0.2605,
            glide: true,
            rings: 1,
            arc: 88.0,
            inner: 0.4205,
            opening: 0.9205,
            door_height: 2.1505,
            arched: false,
            silhouette: 1,
            flank: 0,
            segments_z: 0,
            height_cuts: 0,
            torus_arc: 360.0,
            tube_arc: 360.0,
            arch_band: false,
            arch_degrees: 180.0,
            end_caps: true,
            step_height: 0.0,
            equal_steps: true,
            subdivisions: 1,
        }
    }
}
pub fn produce(spec: &ShapeSpec, grid: Option<&GridConfig>) -> BMesh {
    let s = v3(
        spec.size.x.max(0.0105),
        spec.size.y.max(0.0105),
        spec.size.z.max(0.0105),
    );
    let seg = spec.stretches.clamp(3, 128);
    match spec.sort {
        ShapeKind::Box => cuboid(s),
        ShapeKind::Plane => {
            let dx = spec.stretches.clamp(1, 64);
            plane_grid(
                v2(s.x, s.z),
                dx,
                if spec.segments_z == 0 {
                    dx
                } else {
                    spec.segments_z.clamp(1, 64)
                },
            )
        }
        ShapeKind::Cylinder if spec.height_cuts > 0 => {
            xg_shapes::tube_slashes(s, seg, spec.height_cuts.min(255), spec.glide)
        }
        ShapeKind::Cylinder => tube(s, seg, spec.glide),
        ShapeKind::Cone => taper(s, seg, spec.glide),
        ShapeKind::Sphere => globe(s, seg, spec.steps.clamp(2, 64), spec.glide),
        ShapeKind::Stairs if spec.step_height > 0.0 => {
            let n = xg_shapes::stair_count(s.y, spec.step_height);
            let sh = spec.step_height.max(0.0105);
            stairs_levels(
                s,
                n,
                if spec.equal_steps || n as f32 * sh < s.y {
                    s.y / n as f32
                } else {
                    sh
                },
            )
        }
        ShapeKind::Stairs => steps(s, spec.steps.clamp(1, 128)),
        ShapeKind::Arch if spec.arch_band => xg_shapes::arch_band(
            s,
            spec.thickness,
            spec.stretches.clamp(2, 200),
            spec.arch_degrees,
            spec.end_caps,
            spec.glide,
        ),
        ShapeKind::Arch => bow(s, spec.thickness, spec.stretches.clamp(2, 64)),
        ShapeKind::Wedge => wedge(s),
        ShapeKind::Pipe if spec.height_cuts > 0 => {
            xg_shapes::conduit_slashes(s, spec.thickness, seg, spec.height_cuts.min(31), spec.glide)
        }
        ShapeKind::Pipe => conduit(s, spec.thickness, seg, spec.glide),
        ShapeKind::Prism => xg_shapes::wedge(s),
        ShapeKind::Sprite => plane_grid(v2(s.x, s.z), 1, 1),
        ShapeKind::Icosphere => xg_shapes::icosphere(s, spec.subdivisions.clamp(1, 5), spec.glide),
        ShapeKind::Poly => {
            let (x, z) = (s.x * 0.5, s.z * 0.5);
            super::trim_walls::polygon_solid(&[v2(-x, -z), v2(x, -z), v2(x, z), v2(-x, z)], s.y)
        }
        ShapeKind::Torus
        | ShapeKind::Door
        | ShapeKind::CurvedStairs
        | ShapeKind::Trim
        | ShapeKind::Wall => shape_tool_shape(spec, s),
        ShapeKind::HexPrism | ShapeKind::HexCluster => {
            let fallback = GridConfig {
                grid: GridType::HexRings,
                cell_size: v2(s.x, s.x),
                ..Default::default()
            };
            let cfg = grid
                .filter(|g| g.grid.is_cell_mode())
                .cloned()
                .unwrap_or(fallback);
            if spec.sort == ShapeKind::HexPrism && cfg.grid != GridType::Triangle {
                hex_prism(cfg.hex_radius(), s.y)
            } else {
                let r = if spec.sort == ShapeKind::HexPrism {
                    0
                } else {
                    spec.rings.min(16) as i32
                };
                let cells = cluster_cells(&cfg, iv2(0, 0), r);
                cells_prism(&cfg, &cells, iv2(0, 0), s.y)
            }
        }
    }
}
fn wall_edge(v: u8) -> prims::WallEdge {
    match v {
        1 => prims::WallEdge::Left,
        2 => prims::WallEdge::Right,
        _ => prims::WallEdge::Center,
    }
}
pub fn wall_polyline(spec: &ShapeSpec, points: &[Vec3], closed: bool) -> BMesh {
    prims::wall_solid(
        points,
        closed,
        spec.thickness,
        spec.size.y,
        wall_edge(spec.flank),
    )
}
fn shape_tool_shape(spec: &ShapeSpec, s: Vec3) -> BMesh {
    let (su, sv, ah) = (s.x, s.z, s.y);
    let mut m = match spec.sort {
        ShapeKind::Torus => {
            let extent = su.max(sv) * 0.5;
            let tube = spec.thickness.clamp(0.0015, extent * 0.4605);
            xg_shapes::torus_partial(
                extent - tube,
                tube,
                spec.stretches.clamp(3, 64),
                spec.steps.clamp(3, 64),
                spec.torus_arc,
                spec.tube_arc,
            )
        }
        ShapeKind::Door => {
            let ped = ah - spec.door_height;
            let side = (su - spec.opening) * 0.5;
            let mut d = prims::door_frame(
                su,
                ah,
                sv,
                ped.clamp(0.0125, ah * 0.9605),
                side.clamp(0.0125, su * 0.5 * 0.9605),
                spec.arched,
                spec.stretches.clamp(2, 64),
                spec.glide,
            );
            for p in &mut d.positions {
                *p = v3(p.z, p.y, -p.x);
            }
            d
        }
        ShapeKind::CurvedStairs => {
            let outer = su.max(sv);
            let steps = if spec.step_height > 0.0 {
                xg_shapes::stair_count(ah, spec.step_height).min(64)
            } else {
                spec.steps.clamp(1, 64)
            };
            let mut c = prims::curved_stairs(
                (spec.inner.clamp(0.0125, 0.9605)) * outer,
                outer,
                ah,
                steps,
                spec.arc.clamp(6.0, 350.0),
            );
            prims::center_span_xz(&mut c, outer);
            c
        }
        ShapeKind::Trim => prims::trim_strip(
            sv,
            su,
            spec.thickness.max(0.0015),
            ah,
            prims::TrimSection::from_u8(spec.silhouette),
            spec.stretches.clamp(1, 32),
            false,
            false,
            false,
            spec.glide,
        ),
        _ => wall_polyline(
            spec,
            &[v3(-su * 0.5, 0.0, 0.0), v3(su * 0.5, 0.0, 0.0)],
            false,
        ),
    };
    if let Some((lo, _)) = m.extent() {
        m.displace(v3(0.0, -lo.y, 0.0));
    }
    m
}
pub fn extrude_profile(
    profile: &[Vec2],
    ax: Vec3,
    bx: Vec3,
    c: Vec3,
    origin: Vec3,
    len: f32,
    smooth: u16,
) -> BMesh {
    let mut pts = profile.to_vec();
    let area: f32 = (0..pts.len())
        .map(|i| pts[i].x * pts[(i + 1) % pts.len()].y - pts[(i + 1) % pts.len()].x * pts[i].y)
        .sum();
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
    let cn = c.unitized();
    m.add_face_facing((0..n as u32).collect(), -cn);
    m.add_face_facing((n as u32..2 * n as u32).collect(), cn);
    for i in 0..n {
        let j = (i + 1) % n;
        let d = pts[j] - pts[i];
        let out = ax * d.y - bx * d.x;
        let f = m.add_face_facing(
            vec![i as u32, j as u32, (j + n) as u32, (i + n) as u32],
            out,
        );
        m.facets[f].glide = smooth;
    }
    m
}
pub fn prism_y(outline: &[Vec2], y0: f32, y1: f32, smooth: u16) -> BMesh {
    extrude_profile(
        outline,
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.0, 1.0),
        Vec3::UP,
        v3(0.0, y0, 0.0),
        y1 - y0,
        smooth,
    )
}
pub fn cuboid(s: Vec3) -> BMesh {
    let (hx, hz) = (s.x * 0.5, s.z * 0.5);
    prism_y(
        &[v2(-hx, -hz), v2(hx, -hz), v2(hx, hz), v2(-hx, hz)],
        0.0,
        s.y,
        0,
    )
}
pub fn sheet(size: Vec2, div: u32) -> BMesh {
    plane_grid(size, div, div)
}
pub fn plane_grid(size: Vec2, dx: u32, dz: u32) -> BMesh {
    let mut m = BMesh::default();
    let (dx, dz) = (dx.max(1), dz.max(1));
    for j in 0..=dz {
        for i in 0..=dx {
            m.add_vert(v3(
                (i as f32 / dx as f32 - 0.5) * size.x,
                0.0,
                (j as f32 / dz as f32 - 0.5) * size.y,
            ));
        }
    }
    let w = dx + 1;
    for j in 0..dz {
        for i in 0..dx {
            let a = j * w + i;
            m.add_face_facing(vec![a, a + 1, a + w + 1, a + w], Vec3::UP);
        }
    }
    m
}
pub(super) fn ring(n: u32, rx: f32, rz: f32) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = TAU * (i as f32 + 0.5) / n as f32;
            v2(rx * a.cos(), rz * a.sin())
        })
        .collect()
}
pub fn tube(s: Vec3, segments: u32, smooth: bool) -> BMesh {
    prism_y(
        &ring(segments.max(3), s.x * 0.5, s.z * 0.5),
        0.0,
        s.y,
        smooth as u16,
    )
}
pub fn taper(s: Vec3, segments: u32, smooth: bool) -> BMesh {
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
        m.facets[f].glide = smooth as u16;
    }
    m
}
pub fn globe(s: Vec3, segments: u32, rings: u32, smooth: bool) -> BMesh {
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
        let mut faces = vec![
            vec![top, at(1, i), at(1, i + 1)],
            vec![bottom, at(r - 1, i), at(r - 1, i + 1)],
        ];
        for k in 1..r - 1 {
            faces.push(vec![at(k, i), at(k, i + 1), at(k + 1, i + 1), at(k + 1, i)]);
        }
        for f in faces {
            let ctr = f
                .iter()
                .fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize])
                / f.len() as f32;
            let fi = m.add_face_facing(f, ctr - c);
            m.facets[fi].glide = sg;
        }
    }
    m
}
pub fn steps(s: Vec3, steps: u32) -> BMesh {
    let n = steps.max(1);
    stairs_levels(s, n, s.y / n as f32)
}
pub fn stairs_levels(s: Vec3, steps: u32, step_height: f32) -> BMesh {
    let n = steps.max(1);
    let d = s.z / n as f32;
    let level = |i: u32| {
        if i >= n {
            s.y
        } else {
            (i as f32 * step_height).min(s.y)
        }
    };
    let mut prof = vec![v2(0.0, 0.0), v2(s.z, 0.0), v2(s.z, s.y)];
    for i in (0..n).rev() {
        prof.push(v2(i as f32 * d, level(i + 1)));
        if i > 0 {
            prof.push(v2(i as f32 * d, level(i)));
        }
    }
    extrude_profile(
        &prof,
        v3(0.0, 0.0, 1.0),
        Vec3::UP,
        v3(1.0, 0.0, 0.0),
        v3(-s.x * 0.5, 0.0, -s.z * 0.5),
        s.x,
        0,
    )
}
pub fn wedge(s: Vec3) -> BMesh {
    let prof = [v2(0.0, 0.0), v2(s.z, 0.0), v2(s.z, s.y)];
    extrude_profile(
        &prof,
        v3(0.0, 0.0, 1.0),
        Vec3::UP,
        v3(1.0, 0.0, 0.0),
        v3(-s.x * 0.5, 0.0, -s.z * 0.5),
        s.x,
        0,
    )
}
pub fn bow(s: Vec3, thickness: f32, segments: u32) -> BMesh {
    let hw = s.x * 0.5;
    let t = thickness.clamp(0.0205, hw * 0.9005).min(s.y * 0.9005);
    let r = (hw - t).min(s.y - t).max(0.0105);
    let spring = (s.y - t - r).max(0.0);
    let mut prof = vec![v2(-hw, 0.0), v2(-r, 0.0)];
    if spring > 0.0005 {
        prof.push(v2(-r, spring));
    }
    for k in 1..segments.max(2) {
        let a = PI - PI * k as f32 / segments.max(2) as f32;
        prof.push(v2(r * a.cos(), spring + r * a.sin()));
    }
    if spring > 0.0005 {
        prof.push(v2(r, spring));
    }
    prof.extend([v2(r, 0.0), v2(hw, 0.0), v2(hw, s.y), v2(-hw, s.y)]);
    extrude_profile(
        &prof,
        v3(1.0, 0.0, 0.0),
        Vec3::UP,
        v3(0.0, 0.0, 1.0),
        v3(0.0, 0.0, -s.z * 0.5),
        s.z,
        0,
    )
}
pub fn conduit(s: Vec3, thickness: f32, segments: u32, smooth: bool) -> BMesh {
    let n = segments.max(3);
    let (rx, rz) = (s.x * 0.5, s.z * 0.5);
    let t = thickness.clamp(0.0105, rx.min(rz) * 0.9505);
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
        m.facets[a].glide = smooth as u16;
        let b = m.add_face_facing(vec![ib + i, ib + j, it + j, it + i], -dir);
        m.facets[b].glide = 2 * smooth as u16;
        m.add_face_facing(vec![ot + i, ot + j, it + j, it + i], Vec3::UP);
        m.add_face_facing(vec![ob + i, ob + j, ib + j, ib + i], -Vec3::UP);
    }
    m
}
pub fn hex_prism(radius: f32, height: f32) -> BMesh {
    let pts: Vec<Vec2> = (0..6)
        .map(|i| {
            let a = (-30.0f32 + 60.0 * i as f32).to_radians();
            v2(radius * a.cos(), radius * a.sin())
        })
        .collect();
    prism_y(&pts, 0.0, height, 0)
}
pub fn hex_width(r: f32) -> f32 {
    r * SQRT3
}
pub fn cluster_cells(cfg: &GridConfig, center: IVec2, radius: i32) -> Vec<IVec2> {
    match cfg.grid {
        GridType::HexRings => grid::disk(center, radius),
        GridType::Hex => {
            let a = grid::offset_to_axial(center);
            grid::disk(a, radius)
                .into_iter()
                .map(grid::axial_to_offset)
                .collect()
        }
        _ => {
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
pub fn cells_prism(cfg: &GridConfig, cells: &[IVec2], origin: IVec2, height: f32) -> BMesh {
    let set: BTreeSet<IVec2> = cells.iter().copied().collect();
    let o = cfg.sample_pos(origin);
    let mut m = BMesh::default();
    let mut index: HashMap<(i64, i64, bool), u32> = HashMap::new();
    let mut vid = |m: &mut BMesh, p: Vec2, top: bool| -> u32 {
        let k = (
            ((p.x - o.x) * 2048.0).round() as i64,
            ((p.y - o.y) * 2048.0).round() as i64,
            top,
        );
        *index
            .entry(k)
            .or_insert_with(|| m.add_vert(v3(p.x - o.x, if top { height } else { 0.0 }, p.y - o.y)))
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
            m.add_face_facing(
                vec![bottom[i], bottom[j], top[j], top[i]],
                v3(mid.x, 0.0, mid.y),
            );
        }
    }
    m
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check_closed(m: &BMesh, name: &str) {
        m.sanity_check().unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(m.is_closed(), "{name} not closed");
        assert!(m.volume() > 0.0, "{name} inside out: {}", m.volume());
    }
    #[test]
    fn every_shape_is_valid_and_closed() {
        for k in ShapeKind::ALL {
            let spec = ShapeSpec {
                sort: k,
                size: v3(2.0, 3.0, 2.5),
                ..Default::default()
            };
            let m = produce(&spec, None);
            if matches!(k, ShapeKind::Plane | ShapeKind::Sprite) {
                m.sanity_check().unwrap();
                continue;
            }
            check_closed(&m, k.moniker());
            if k.fixed_height() {
                continue;
            }
            let (lo, hi) = m.extent().unwrap();
            assert!(
                lo.y.abs() < 1e-5 && (hi.y - 3.0).abs() < 1e-4,
                "{}: y {lo:?} {hi:?}",
                k.moniker()
            );
        }
    }
    #[test]
    fn kind_discriminants_match_all() {
        for (i, k) in ShapeKind::ALL.iter().enumerate() {
            assert_eq!(*k as usize, i, "{}", k.moniker());
            assert_eq!(ShapeKind::from_i64(i as i64), *k);
        }
    }
    #[test]
    fn box_volume_and_stairs_volume() {
        assert!((cuboid(v3(2.0, 3.0, 4.0)).volume() - 24.0).abs() < 1e-3);
        let s = steps(v3(1.0, 4.0, 4.0), 4);
        assert!(
            (s.volume() - 16.0 * 5.0 / 8.0).abs() < 1e-3,
            "{}",
            s.volume()
        );
    }
    #[test]
    fn hex_prism_matches_terrain_cells() {
        for g in [GridType::Hex, GridType::HexRings] {
            let cfg = GridConfig {
                grid: g,
                cell_size: v2(2.0, 2.0),
                ..Default::default()
            };
            let m = hex_prism(cfg.hex_radius(), 1.0);
            let c = iv2(3, 5);
            let ctr = cfg.sample_pos(c);
            for corner in cfg.cell_corners(c) {
                let rel = corner - ctr;
                assert!(
                    m.positions
                        .iter()
                        .any(|p| (p.x - rel.x).abs() < 1e-4 && (p.z - rel.y).abs() < 1e-4)
                );
            }
            assert!((hex_width(cfg.hex_radius()) - 2.0).abs() < 1e-4);
        }
    }
    #[test]
    fn hex_cluster_is_watertight_on_every_grid() {
        for g in [GridType::Hex, GridType::HexRings, GridType::Triangle] {
            let cfg = GridConfig {
                grid: g,
                cell_size: v2(2.0, 2.0),
                ..Default::default()
            };
            let cells = cluster_cells(&cfg, iv2(1, 1), 2);
            let m = cells_prism(&cfg, &cells, iv2(1, 1), 1.5);
            check_closed(&m, g.moniker());
            let area_top: f32 = (0..m.facets.len())
                .filter(|f| m.face_normal(*f).y > 0.9)
                .map(|f| m.face_area(f))
                .sum();
            assert!((m.volume() - area_top * 1.5).abs() < 1e-2);
            if g != GridType::Triangle {
                assert_eq!(cells.len(), 19);
            }
        }
    }
}
