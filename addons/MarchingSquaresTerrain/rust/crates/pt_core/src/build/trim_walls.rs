//! Poly shape and trim walls, modelled on cyclopsLevelBuilder (MIT, Mark McKay): the outline
//! extrusion of `math/convex_volume.gd` (`init_prism`, `loop_2d`) and the wall-run chaining of
//! `math/pen_stroke.gd` (resampled stroke points, mitre joins) feeding the `prims::trim_path`
//! sweep. Of the multi-object passes only the clipping is ported ([`trim_walls_clipped`]: runs
//! buried inside other closed meshes, e.g. a pillar or a crossing wall, are cut out).

use super::model::BMesh;
use super::prims::{TrimSection, trim_path};
use crate::math::*;
use std::collections::BTreeSet;

const END_TOL: f32 = 0.0014;
const LINE_TOL: f32 = 0.0015;
const MIN_LEN: f32 = 0.0015;
const FOLD_TOL: f32 = 0.0555;
const WALL_MIN_HORIZONTAL: f32 = 0.7205;

/// Drops repeated consecutive points (and a closing duplicate)
/// (`math_util.snap_point_to_point_list_or_append`'s merge radius).
pub fn collapse_points(points: &[Vec2], closed: bool, tol: f32) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for p in points {
        if out.last().is_none_or(|l| l.span(*p) > tol) {
            out.push(*p);
        }
    }
    if closed && out.len() > 1 && out[0].span(*out.last().unwrap()) <= tol {
        out.pop();
    }
    out
}

/// The polygon (XZ, local) extruded `height` up: a top, a bottom and one quad per side. Empty
/// when fewer than three distinct points remain (`convex_volume.gd::init_prism`).
pub fn polygon_solid(points: &[Vec2], height: f32) -> BMesh {
    let mut m = BMesh::default();
    let pts = collapse_points(points, true, 0.0005);
    if pts.len() < 3 {
        return m;
    }
    let n = pts.len() as u32;
    for p in &pts {
        m.add_vert(v3(p.x, 0.0, p.y));
    }
    for p in &pts {
        m.add_vert(v3(p.x, height, p.y));
    }
    let up = if height >= 0.0 { Vec3::UP } else { -Vec3::UP };
    m.add_face_facing((n..2 * n).collect(), up);
    m.add_face_facing((0..n).rev().collect(), -up);
    // Side quads face out of the polygon: the edge normal on the outer side of its winding.
    let area: f32 = (0..pts.len())
        .map(|k| pts[k].x * pts[(k + 1) % pts.len()].y - pts[(k + 1) % pts.len()].x * pts[k].y)
        .sum();
    for i in 0..n {
        let j = (i + 1) % n;
        let d = pts[j as usize] - pts[i as usize];
        let out = if area < 0.0 {
            v3(-d.y, 0.0, d.x)
        } else {
            v3(d.y, 0.0, -d.x)
        };
        m.add_face_facing(vec![i, j, n + j, n + i], out);
    }
    m
}

#[derive(Clone, Debug)]
struct Seg {
    a: Vec3,
    bx: Vec3,
    n: Vec3,
}

/// The base (or with `top` the top) edges of one wall face.
fn face_wall_segments(m: &BMesh, fi: usize, top: bool) -> Vec<Seg> {
    let up = Vec3::UP;
    let mut out = Vec::new();
    let normal = m.face_normal(fi);
    let mut n = normal - up * normal.innerprod(up);
    if n.magnitude() < WALL_MIN_HORIZONTAL {
        return out;
    }
    n = n.unitized();
    let world = m.face_points(fi);
    let level = world
        .iter()
        .map(|p| p.y)
        .fold(if top { f32::MIN } else { f32::MAX }, |a, h| {
            if top { a.max(h) } else { a.min(h) }
        });
    let count = world.len();
    for i in 0..count {
        let (a, b) = (world[i], world[(i + 1) % count]);
        if (a.y - level).abs() > END_TOL
            || (b.y - level).abs() > END_TOL
            || (b - a).magnitude() < MIN_LEN
        {
            continue;
        }
        let mut en = up.cross((b - a).unitized());
        if en.magnitude() < 0.5 {
            continue;
        }
        en = en.unitized();
        if en.innerprod(n) < 0.0 {
            en = -en;
        }
        out.push(Seg { a, bx: b, n: en });
    }
    out
}

/// Overlapping / touching collinear runs with the same facing become one segment
/// (`pen_stroke.gd`'s `resample_points` merging a straight stroke).
fn fuse_sequences(segments: &[Seg]) -> Vec<Seg> {
    struct Group {
        n: Vec3,
        origin: Vec3,
        dir: Vec3,
        ivs: Vec<(f32, f32)>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for s in segments {
        let len = (s.bx - s.a).magnitude();
        if len < MIN_LEN {
            continue;
        }
        let d = (s.bx - s.a) / len;
        let mut placed = false;
        for g in &mut groups {
            if g.n.innerprod(s.n) < 0.9995 || d.innerprod(g.dir).abs() < 0.9995 {
                continue;
            }
            let off = s.a - g.origin;
            if (off - g.dir * off.innerprod(g.dir)).magnitude() > LINE_TOL {
                continue;
            }
            let (ta, tb) = (off.innerprod(g.dir), (s.bx - g.origin).innerprod(g.dir));
            g.ivs.push((ta.min(tb), ta.max(tb)));
            placed = true;
            break;
        }
        if !placed {
            groups.push(Group {
                n: s.n,
                origin: s.a,
                dir: d,
                ivs: vec![(0.0, len)],
            });
        }
    }
    let mut out = Vec::new();
    for mut g in groups {
        g.ivs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut cur = g.ivs[0];
        for iv in &g.ivs[1..] {
            if iv.0 <= cur.1 + END_TOL {
                cur.1 = cur.1.max(iv.1);
            } else {
                out.push(Seg {
                    a: g.origin + g.dir * cur.0,
                    bx: g.origin + g.dir * cur.1,
                    n: g.n,
                });
                cur = *iv;
            }
        }
        out.push(Seg {
            a: g.origin + g.dir * cur.0,
            bx: g.origin + g.dir * cur.1,
            n: g.n,
        });
    }
    out
}

/// Where the offset lines of two runs meet (a mitre join).
fn miter_corner(p1: Vec3, end: Vec3, n2: Vec3, p2: Vec3) -> Vec3 {
    let up = Vec3::UP;
    let mut d1 = end - p1;
    d1 = d1 - up * d1.innerprod(up);
    if d1.magnitude() < 1e-9 {
        return end;
    }
    d1 = d1.unitized();
    let denom = n2.innerprod(d1);
    if denom.abs() < 1e-6 {
        return end;
    }
    end + d1 * (n2.innerprod(p2 - end) / denom)
}

/// A chained path: corner points, one outward normal per segment, closed flag.
pub struct TrimRun {
    pub points: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub is_shut: bool,
}

/// Grows the path at one end; true when it closed into a loop.
fn grow_path(
    pts: &mut Vec<Vec3>,
    ns: &mut Vec<Vec3>,
    segs: &[Seg],
    used: &mut [bool],
    forward: bool,
) -> bool {
    loop {
        let end = if forward {
            *pts.last().unwrap()
        } else {
            pts[0]
        };
        let mut found = None;
        for (j, s) in segs.iter().enumerate() {
            if used[j] {
                continue;
            }
            if (s.a - end).magnitude() <= END_TOL {
                found = Some((j, s.bx));
                break;
            }
            if (s.bx - end).magnitude() <= END_TOL {
                found = Some((j, s.a));
                break;
            }
        }
        let Some((j, far)) = found else { return false };
        let n_new = segs[j].n;
        let n_end = if forward { *ns.last().unwrap() } else { ns[0] };
        if 1.0 + n_end.innerprod(n_new) < FOLD_TOL {
            return false;
        }
        used[j] = true;
        let prev = if forward { pts[pts.len() - 2] } else { pts[1] };
        let c = miter_corner(prev, end, n_new, far);
        if forward {
            let k = pts.len() - 1;
            pts[k] = c;
            pts.push(far);
            ns.push(n_new);
        } else {
            pts[0] = c;
            pts.insert(0, far);
            ns.insert(0, n_new);
        }
        let other = if forward {
            pts[0]
        } else {
            *pts.last().unwrap()
        };
        if pts.len() >= 4 && (far - other).magnitude() <= END_TOL {
            let n_first = if forward { ns[0] } else { *ns.last().unwrap() };
            if 1.0 + n_new.innerprod(n_first) < FOLD_TOL {
                return false;
            }
            let a = if forward { pts[pts.len() - 2] } else { pts[1] };
            let b = if forward { pts[1] } else { pts[pts.len() - 2] };
            let seam = miter_corner(a, far, n_first, b);
            if forward {
                pts.pop();
                pts[0] = seam;
            } else {
                pts.remove(0);
                let k = pts.len() - 1;
                pts[k] = seam;
            }
            return true;
        }
    }
}

/// Chains segments end to end into paths.
fn chain_runs(segments: &[Seg]) -> Vec<TrimRun> {
    let segs: Vec<Seg> = segments
        .iter()
        .filter(|s| (s.bx - s.a).magnitude() >= MIN_LEN)
        .cloned()
        .collect();
    let mut used = vec![false; segs.len()];
    let mut paths = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut pts = vec![segs[start].a, segs[start].bx];
        let mut ns = vec![segs[start].n];
        let closed = grow_path(&mut pts, &mut ns, &segs, &mut used, true);
        if !closed {
            grow_path(&mut pts, &mut ns, &segs, &mut used, false);
        }
        paths.push(TrimRun {
            points: pts,
            normals: ns,
            is_shut: closed,
        });
    }
    paths
}

/// The trim paths along the base (`top` = along the top) of the selected wall faces.
pub fn wall_path_runs(m: &BMesh, faces: &BTreeSet<usize>, top: bool) -> Vec<TrimRun> {
    wall_path_runs_clipped(m, faces, top, &[], 0.0)
}

/// [`wall_path_runs`] with the parts buried inside `solids` removed. `solids` are closed meshes
/// already in `m`'s frame; `lift` is how far above the base (below the top) the clip ray runs
/// (half the trim height).
pub fn wall_path_runs_clipped(
    m: &BMesh,
    faces: &BTreeSet<usize>,
    top: bool,
    solids: &[BMesh],
    lift: f32,
) -> Vec<TrimRun> {
    let segs: Vec<Seg> = faces
        .iter()
        .filter(|f| **f < m.facets.len())
        .flat_map(|f| face_wall_segments(m, *f, top))
        .collect();
    let segs = if solids.is_empty() {
        segs
    } else {
        clip_runs(&segs, solids, if top { -lift } else { lift })
    };
    chain_runs(&fuse_sequences(&segs))
}

/// Drops the parts of each segment inside another closed solid. A ray runs along the segment
/// (lifted by `lift`, pushed `PROBE_OFFSET` off the wall); entering a face (normal against the
/// ray) starts a buried interval, leaving one ends it.
fn clip_runs(segments: &[Seg], solids: &[BMesh], lift: f32) -> Vec<Seg> {
    const PROBE_OFFSET: f32 = 0.0014;
    let mut out = Vec::new();
    for seg in segments {
        let (a, b) = (seg.a, seg.bx);
        let length = (b - a).magnitude();
        if length < MIN_LEN {
            continue;
        }
        let dir = (b - a) * (1.0 / length);
        let mut buried: Vec<(f32, f32)> = Vec::new();
        for solid in solids {
            if !solid.is_closed() {
                continue;
            }
            let origin = a + Vec3::UP * lift + seg.n * PROBE_OFFSET;
            let mut t = 0.0f32;
            let mut inside = false;
            let mut enter_t = 0.0f32;
            for _guard in 0..256 {
                let Some(hit) =
                    super::pick::intersect_ray_closest(solid, origin + dir * t, dir, false)
                else {
                    break;
                };
                let wt = t + hit.t;
                if hit.t > 1e-7 {
                    if solid.face_normal(hit.face).innerprod(dir) < 0.0 {
                        if wt > length {
                            break;
                        }
                        inside = true;
                        enter_t = wt;
                    } else {
                        buried.push((if inside { enter_t } else { 0.0 }, wt.min(length)));
                        inside = false;
                        if wt > length {
                            break;
                        }
                    }
                }
                t = wt + 0.0005;
                if t > length {
                    break;
                }
            }
            if inside {
                buried.push((enter_t, length));
            }
        }
        buried.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut t0 = 0.0f32;
        for (s0, s1) in buried {
            let s = s0.max(t0);
            if s - t0 > MIN_LEN {
                out.push(Seg {
                    a: a + dir * t0,
                    bx: a + dir * s,
                    n: seg.n,
                });
            }
            t0 = t0.max(s1);
        }
        if length - t0 > MIN_LEN {
            out.push(Seg {
                a: a + dir * t0,
                bx: a + dir * length,
                n: seg.n,
            });
        }
    }
    out
}

/// Trim walls: a moulding swept along the base (or, with `top`, hung under the top) of the
/// selected wall faces, as a separate solid.
#[allow(clippy::too_many_arguments)]
pub fn trim_walls(
    m: &BMesh,
    faces: &BTreeSet<usize>,
    top: bool,
    depth: f32,
    height: f32,
    section: TrimSection,
    steps: u32,
    offset: f32,
    smooth: bool,
) -> BMesh {
    trim_walls_clipped(
        m,
        faces,
        top,
        depth,
        height,
        section,
        steps,
        offset,
        smooth,
        &[],
    )
}

/// [`trim_walls`] across meshes: the moulding stops where the walls run into other closed meshes
/// (`solids`, in `m`'s frame), the way a mitred stroke is trimmed against its neighbours.
#[allow(clippy::too_many_arguments)]
pub fn trim_walls_clipped(
    m: &BMesh,
    faces: &BTreeSet<usize>,
    top: bool,
    depth: f32,
    height: f32,
    section: TrimSection,
    steps: u32,
    offset: f32,
    smooth: bool,
    solids: &[BMesh],
) -> BMesh {
    let mut out = BMesh::default();
    let lift = height.max(0.0125) * 0.5;
    for p in wall_path_runs_clipped(m, faces, top, solids, lift) {
        let piece = trim_path(
            &p.points,
            &p.normals,
            p.is_shut,
            Vec3::UP,
            depth.max(0.0015),
            height.max(0.0015),
            section,
            steps.clamp(1, 32),
            top,
            false,
            offset,
            smooth,
            0.0,
        );
        out.append_tail(&piece);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    #[test]
    fn polygon_solid_l_is_closed_and_outward() {
        let l = [
            v2(0.0, 0.0),
            v2(2.0, 0.0),
            v2(2.0, 1.0),
            v2(1.0, 1.0),
            v2(1.0, 2.0),
            v2(0.0, 2.0),
            v2(0.0, 2.0),
        ];
        let m = polygon_solid(&l, 1.52);
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 4.56).abs() < 1e-4, "{}", m.volume());
        let mut r: Vec<Vec2> = l.to_vec();
        r.reverse();
        assert!((polygon_solid(&r, 1.52).volume() - 4.56).abs() < 1e-4);
        assert!(polygon_solid(&l[..2], 1.0).facets.is_empty());
    }

    #[test]
    fn trim_around_a_box_base_closes_one_loop() {
        let m = cuboid(v3(2.0, 1.0, 2.0));
        let walls: BTreeSet<usize> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y.abs() < 0.1)
            .collect();
        let paths = wall_path_runs(&m, &walls, false);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].is_shut);
        assert_eq!(paths[0].points.len(), 4);
        let t = trim_walls(
            &m,
            &walls,
            false,
            0.1,
            0.15,
            TrimSection::Chamfer,
            4,
            0.0,
            false,
        );
        assert!(!t.facets.is_empty());
        t.sanity_check().unwrap();
        // Two adjacent walls only: one open path of two runs.
        let two: BTreeSet<usize> = walls.iter().copied().take(2).collect();
        let p = wall_path_runs(&m, &two, true);
        assert!(p.iter().all(|p| !p.is_shut));
    }

    #[test]
    fn trim_is_clipped_where_a_pillar_stands() {
        let m = cuboid(v3(4.0, 1.0, 4.0));
        let walls: BTreeSet<usize> = (0..m.facets.len())
            .filter(|f| m.face_normal(*f).y.abs() < 0.1)
            .collect();
        // A pillar straddling the middle of one wall (the +X wall at x = 2).
        let mut pillar = cuboid(v3(1.0, 2.0, 1.0));
        for p in &mut pillar.positions {
            *p = *p + v3(2.0, 0.0, 0.0);
        }
        assert!(pillar.is_closed());
        let plain = wall_path_runs(&m, &walls, false);
        let cut = wall_path_runs_clipped(&m, &walls, false, &[pillar], 0.05);
        assert_eq!(plain.len(), 1);
        assert!(plain[0].is_shut);
        assert_eq!(cut.len(), 1, "one open run around the pillar");
        assert!(!cut[0].is_shut);
        let len = |p: &TrimRun| {
            p.points
                .windows(2)
                .map(|w| (w[1] - w[0]).magnitude())
                .sum::<f32>()
        };
        // Perimeter 16 minus the 1 m the pillar buries.
        assert!((len(&cut[0]) - 15.0).abs() < 0.05, "{}", len(&cut[0]));
    }
}
