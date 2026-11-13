//! Poly shape and trim walls, ported from Unibuilder (MIT, Leafshade Consulting Ltd):
//! `core/primitives.gd` (`make_polygon_prism`, `dedupe_loop`) and `core/trim_path.gd`
//! (`wall_segments`, `merge_collinear`, `chain`, `_extend`, `_corner`) feeding the existing
//! `prims::make_trim_path` port. Of the multi-object passes of `paths_from_walls`,
//! `clip_segments` is ported ([`trim_walls_clipped`]: runs buried inside other closed meshes,
//! e.g. a pillar or a crossing wall, are cut out); `settle_on_surfaces` and `keep_rooms` are not.

use super::model::BMesh;
use super::prims::{TrimProfile, make_trim_path};
use crate::math::*;
use std::collections::BTreeSet;

const END_TOL: f32 = 1e-3;
const LINE_TOL: f32 = 1e-3;
const MIN_LEN: f32 = 1e-3;
const FOLD_TOL: f32 = 0.05;
const WALL_MIN_HORIZONTAL: f32 = 0.7;

/// `UBPrimitives.dedupe_loop`: drops repeated consecutive points (and a closing duplicate).
pub fn dedupe_loop(points: &[Vec2], closed: bool, tol: f32) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for p in points {
        if out.last().is_none_or(|l| l.distance(*p) > tol) {
            out.push(*p);
        }
    }
    if closed && out.len() > 1 && out[0].distance(*out.last().unwrap()) <= tol {
        out.pop();
    }
    out
}

/// `UBPrimitives.make_polygon_prism`: the polygon (XZ, local) extruded `height` up: a top,
/// a bottom and one quad per side. Empty when fewer than three distinct points remain.
pub fn polygon_prism(points: &[Vec2], height: f32) -> BMesh {
    let mut m = BMesh::default();
    let pts = dedupe_loop(points, true, 1e-4);
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
    let area: f32 = (0..pts.len()).map(|k| pts[k].x * pts[(k + 1) % pts.len()].y - pts[(k + 1) % pts.len()].x * pts[k].y).sum();
    for i in 0..n {
        let j = (i + 1) % n;
        let d = pts[j as usize] - pts[i as usize];
        let out = if area < 0.0 { v3(-d.y, 0.0, d.x) } else { v3(d.y, 0.0, -d.x) };
        m.add_face_facing(vec![i, j, n + j, n + i], out);
    }
    m
}

#[derive(Clone, Debug)]
struct Seg {
    a: Vec3,
    b: Vec3,
    n: Vec3,
}

/// `UBTrimPath.wall_segments`: the base (or with `top` the top) edges of one wall face.
fn wall_segments(m: &BMesh, fi: usize, top: bool) -> Vec<Seg> {
    let up = Vec3::UP;
    let mut out = Vec::new();
    let normal = m.face_normal(fi);
    let mut n = normal - up * normal.dot(up);
    if n.length() < WALL_MIN_HORIZONTAL {
        return out;
    }
    n = n.normalized();
    let world = m.face_points(fi);
    let level = world.iter().map(|p| p.y).fold(if top { f32::MIN } else { f32::MAX }, |a, h| if top { a.max(h) } else { a.min(h) });
    let count = world.len();
    for i in 0..count {
        let (a, b) = (world[i], world[(i + 1) % count]);
        if (a.y - level).abs() > END_TOL || (b.y - level).abs() > END_TOL || (b - a).length() < MIN_LEN {
            continue;
        }
        let mut en = up.cross((b - a).normalized());
        if en.length() < 0.5 {
            continue;
        }
        en = en.normalized();
        if en.dot(n) < 0.0 {
            en = -en;
        }
        out.push(Seg { a, b, n: en });
    }
    out
}

/// `UBTrimPath.merge_collinear`: overlapping / touching collinear runs with the same facing
/// become one segment.
fn merge_collinear(segments: &[Seg]) -> Vec<Seg> {
    struct Group {
        n: Vec3,
        origin: Vec3,
        dir: Vec3,
        ivs: Vec<(f32, f32)>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for s in segments {
        let len = (s.b - s.a).length();
        if len < MIN_LEN {
            continue;
        }
        let d = (s.b - s.a) / len;
        let mut placed = false;
        for g in &mut groups {
            if g.n.dot(s.n) < 0.999 || d.dot(g.dir).abs() < 0.999 {
                continue;
            }
            let off = s.a - g.origin;
            if (off - g.dir * off.dot(g.dir)).length() > LINE_TOL {
                continue;
            }
            let (ta, tb) = (off.dot(g.dir), (s.b - g.origin).dot(g.dir));
            g.ivs.push((ta.min(tb), ta.max(tb)));
            placed = true;
            break;
        }
        if !placed {
            groups.push(Group { n: s.n, origin: s.a, dir: d, ivs: vec![(0.0, len)] });
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
                out.push(Seg { a: g.origin + g.dir * cur.0, b: g.origin + g.dir * cur.1, n: g.n });
                cur = *iv;
            }
        }
        out.push(Seg { a: g.origin + g.dir * cur.0, b: g.origin + g.dir * cur.1, n: g.n });
    }
    out
}

/// `UBTrimPath._corner`: where the offset lines of two runs meet.
fn corner(p1: Vec3, end: Vec3, n2: Vec3, p2: Vec3) -> Vec3 {
    let up = Vec3::UP;
    let mut d1 = end - p1;
    d1 = d1 - up * d1.dot(up);
    if d1.length() < 1e-9 {
        return end;
    }
    d1 = d1.normalized();
    let denom = n2.dot(d1);
    if denom.abs() < 1e-6 {
        return end;
    }
    end + d1 * (n2.dot(p2 - end) / denom)
}

/// A chained path: corner points, one outward normal per segment, closed flag.
pub struct TrimPath {
    pub points: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub closed: bool,
}

/// `UBTrimPath._extend`: grows the path at one end; true when it closed into a loop.
fn extend(pts: &mut Vec<Vec3>, ns: &mut Vec<Vec3>, segs: &[Seg], used: &mut [bool], forward: bool) -> bool {
    loop {
        let end = if forward { *pts.last().unwrap() } else { pts[0] };
        let mut found = None;
        for (j, s) in segs.iter().enumerate() {
            if used[j] {
                continue;
            }
            if (s.a - end).length() <= END_TOL {
                found = Some((j, s.b));
                break;
            }
            if (s.b - end).length() <= END_TOL {
                found = Some((j, s.a));
                break;
            }
        }
        let Some((j, far)) = found else { return false };
        let n_new = segs[j].n;
        let n_end = if forward { *ns.last().unwrap() } else { ns[0] };
        if 1.0 + n_end.dot(n_new) < FOLD_TOL {
            return false;
        }
        used[j] = true;
        let prev = if forward { pts[pts.len() - 2] } else { pts[1] };
        let c = corner(prev, end, n_new, far);
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
        let other = if forward { pts[0] } else { *pts.last().unwrap() };
        if pts.len() >= 4 && (far - other).length() <= END_TOL {
            let n_first = if forward { ns[0] } else { *ns.last().unwrap() };
            if 1.0 + n_new.dot(n_first) < FOLD_TOL {
                return false;
            }
            let a = if forward { pts[pts.len() - 2] } else { pts[1] };
            let b = if forward { pts[1] } else { pts[pts.len() - 2] };
            let seam = corner(a, far, n_first, b);
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

/// `UBTrimPath.chain`: chains segments end to end into paths.
fn chain(segments: &[Seg]) -> Vec<TrimPath> {
    let segs: Vec<Seg> = segments.iter().filter(|s| (s.b - s.a).length() >= MIN_LEN).cloned().collect();
    let mut used = vec![false; segs.len()];
    let mut paths = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut pts = vec![segs[start].a, segs[start].b];
        let mut ns = vec![segs[start].n];
        let closed = extend(&mut pts, &mut ns, &segs, &mut used, true);
        if !closed {
            extend(&mut pts, &mut ns, &segs, &mut used, false);
        }
        paths.push(TrimPath { points: pts, normals: ns, closed });
    }
    paths
}

/// The trim paths along the base (`top` = along the top) of the selected wall faces.
pub fn wall_paths(m: &BMesh, faces: &BTreeSet<usize>, top: bool) -> Vec<TrimPath> {
    wall_paths_clipped(m, faces, top, &[], 0.0)
}

/// [`wall_paths`] with the parts buried inside `solids` removed (`UBTrimPath.paths_from_walls`
/// minus the settle / keep-rooms passes). `solids` are closed meshes already in `m`'s frame;
/// `lift` is how far above the base (below the top) the clip ray runs (half the trim height).
pub fn wall_paths_clipped(m: &BMesh, faces: &BTreeSet<usize>, top: bool, solids: &[BMesh], lift: f32) -> Vec<TrimPath> {
    let segs: Vec<Seg> = faces.iter().filter(|f| **f < m.faces.len()).flat_map(|f| wall_segments(m, *f, top)).collect();
    let segs = if solids.is_empty() { segs } else { clip_segments(&segs, solids, if top { -lift } else { lift }) };
    chain(&merge_collinear(&segs))
}

/// `UBTrimPath.clip_segments`: drops the parts of each segment inside another closed solid.
/// A ray runs along the segment (lifted by `lift`, pushed `PROBE_OFFSET` off the wall); entering
/// a face (normal against the ray) starts a buried interval, leaving one ends it.
fn clip_segments(segments: &[Seg], solids: &[BMesh], lift: f32) -> Vec<Seg> {
    const PROBE_OFFSET: f32 = 1e-3;
    let mut out = Vec::new();
    for seg in segments {
        let (a, b) = (seg.a, seg.b);
        let length = (b - a).length();
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
                let Some(hit) = super::pick::ray_mesh(solid, origin + dir * t, dir, false) else { break };
                let wt = t + hit.t;
                if hit.t > 1e-7 {
                    if solid.face_normal(hit.face).dot(dir) < 0.0 {
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
                t = wt + 1e-4;
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
                out.push(Seg { a: a + dir * t0, b: a + dir * s, n: seg.n });
            }
            t0 = t0.max(s1);
        }
        if length - t0 > MIN_LEN {
            out.push(Seg { a: a + dir * t0, b: a + dir * length, n: seg.n });
        }
    }
    out
}

/// Trim walls (`Kind.TRIM_WALLS` of `shape_tool.gd`): a moulding swept along the base (or,
/// with `top`, hung under the top) of the selected wall faces, as a separate solid.
#[allow(clippy::too_many_arguments)]
pub fn trim_walls(m: &BMesh, faces: &BTreeSet<usize>, top: bool, depth: f32, height: f32, profile: TrimProfile, segs: u32, offset: f32, smooth: bool) -> BMesh {
    trim_walls_clipped(m, faces, top, depth, height, profile, segs, offset, smooth, &[])
}

/// [`trim_walls`] across meshes: the moulding stops where the walls run into other closed
/// meshes (`solids`, in `m`'s frame), like Unibuilder's trim against the scene's other meshes.
#[allow(clippy::too_many_arguments)]
pub fn trim_walls_clipped(m: &BMesh, faces: &BTreeSet<usize>, top: bool, depth: f32, height: f32, profile: TrimProfile, segs: u32, offset: f32, smooth: bool, solids: &[BMesh]) -> BMesh {
    let mut out = BMesh::default();
    let lift = height.max(0.01) * 0.5;
    for p in wall_paths_clipped(m, faces, top, solids, lift) {
        let piece = make_trim_path(&p.points, &p.normals, p.closed, Vec3::UP, depth.max(1e-3), height.max(1e-3), profile, segs.clamp(1, 32), top, false, offset, smooth, 0.0);
        out.append(&piece);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    #[test]
    fn polygon_prism_l_is_closed_and_outward() {
        let l = [v2(0.0, 0.0), v2(2.0, 0.0), v2(2.0, 1.0), v2(1.0, 1.0), v2(1.0, 2.0), v2(0.0, 2.0), v2(0.0, 2.0)];
        let m = polygon_prism(&l, 1.5);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 4.5).abs() < 1e-4, "{}", m.volume());
        let mut r: Vec<Vec2> = l.to_vec();
        r.reverse();
        assert!((polygon_prism(&r, 1.5).volume() - 4.5).abs() < 1e-4);
        assert!(polygon_prism(&l[..2], 1.0).faces.is_empty());
    }

    #[test]
    fn trim_around_a_box_base_closes_one_loop() {
        let m = cuboid(v3(2.0, 1.0, 2.0));
        let walls: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.face_normal(*f).y.abs() < 0.1).collect();
        let paths = wall_paths(&m, &walls, false);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points.len(), 4);
        let t = trim_walls(&m, &walls, false, 0.1, 0.15, TrimProfile::Chamfer, 4, 0.0, false);
        assert!(!t.faces.is_empty());
        t.validate().unwrap();
        // Two adjacent walls only: one open path of two runs.
        let two: BTreeSet<usize> = walls.iter().copied().take(2).collect();
        let p = wall_paths(&m, &two, true);
        assert!(p.iter().all(|p| !p.closed));
    }

    #[test]
    fn trim_is_clipped_where_a_pillar_stands() {
        let m = cuboid(v3(4.0, 1.0, 4.0));
        let walls: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.face_normal(*f).y.abs() < 0.1).collect();
        // A pillar straddling the middle of one wall (the +X wall at x = 2).
        let mut pillar = cuboid(v3(1.0, 2.0, 1.0));
        for p in &mut pillar.positions {
            *p = *p + v3(2.0, 0.0, 0.0);
        }
        assert!(pillar.is_closed());
        let plain = wall_paths(&m, &walls, false);
        let cut = wall_paths_clipped(&m, &walls, false, &[pillar], 0.05);
        assert_eq!(plain.len(), 1);
        assert!(plain[0].closed);
        assert_eq!(cut.len(), 1, "one open run around the pillar");
        assert!(!cut[0].closed);
        let len = |p: &TrimPath| p.points.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f32>();
        // Perimeter 16 minus the 1 m the pillar buries.
        assert!((len(&cut[0]) - 15.0).abs() < 0.05, "{}", len(&cut[0]));
    }
}
