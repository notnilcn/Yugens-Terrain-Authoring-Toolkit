//! More element operations ported from Unibuilder (MIT, Leafshade Consulting Ltd):
//! `core/ops/collapse_vertices.gd`, `inset_faces.gd` (rim width, *Equal* mitred corners,
//! concave faces), `subdivide_faces.gd` (any even segment count, kernel centre for concave
//! faces), the insert-vertex modal (`plugin.gd::commit_insert_vertex`), and the helpers they
//! need from `op_utils.gd` (`remap_loop`, `remap_faces`, `remap_overshares`, `kernel_point`,
//! `layout_ok`, `loop_weights`).

use super::edit_ops::{LoopPlane, cross_2d, loop_plane, segments_cross, signed_area_2d};
use super::loops::{insert_edge_points, lerp_color};
use super::model::{BMesh, Edge, Face, edge};
use super::tri::triangulate;
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------- remap helpers

/// `UBOpUtils.remap_loop`: one loop through a vertex remap, split at revisited vertices into
/// pieces (loop, source corner indices); pieces without area are dropped.
fn remap_loop(m: &BMesh, verts: &[u32], remap: &HashMap<u32, u32>) -> Vec<(Vec<u32>, Vec<usize>)> {
    let map = |v: u32| *remap.get(&v).unwrap_or(&v);
    let (mut out, mut src) = (Vec::new(), Vec::new());
    for (i, v) in verts.iter().enumerate() {
        let nv = map(*v);
        if out.last() != Some(&nv) {
            out.push(nv);
            src.push(i);
        }
    }
    while out.len() > 1 && out.first() == out.last() {
        out.pop();
        src.pop();
    }
    if out.len() < 3 {
        return Vec::new();
    }
    let mut pieces = Vec::new();
    let (mut sv, mut ss): (Vec<u32>, Vec<usize>) = (Vec::new(), Vec::new());
    let mut pos: HashMap<u32, usize> = HashMap::new();
    for k in 0..out.len() {
        let v = out[k];
        if let Some(&p) = pos.get(&v) {
            pieces.push((sv[p..].to_vec(), ss[p..].to_vec()));
            for r in p + 1..sv.len() {
                pos.remove(&sv[r]);
            }
            sv.truncate(p + 1);
            ss.truncate(p + 1);
        } else {
            pos.insert(v, sv.len());
            sv.push(v);
            ss.push(src[k]);
        }
    }
    if pieces.is_empty() {
        return vec![(out, src)];
    }
    pieces.push((sv, ss));
    pieces
        .into_iter()
        .filter(|(l, _)| {
            if l.len() < 3 {
                return false;
            }
            let pts: Vec<Vec3> = l.iter().map(|v| m.positions[*v as usize]).collect();
            let scale = (0..pts.len()).map(|i| (pts[i] - pts[(i + 1) % pts.len()]).dot(pts[i] - pts[(i + 1) % pts.len()])).fold(0.0f32, f32::max);
            super::model::newell(&pts).length() > 1e-9 * scale
        })
        .collect()
}

/// `UBOpUtils.remap_overshares`: the remap would newly leave an edge used by more than two faces.
fn remap_overshares(m: &BMesh, remap: &HashMap<u32, u32>) -> bool {
    let mut before: HashMap<Edge, usize> = HashMap::new();
    for f in &m.faces {
        for (a, b) in f.loop_edges() {
            *before.entry(edge(a, b)).or_default() += 1;
        }
    }
    let mut after: HashMap<Edge, usize> = HashMap::new();
    for f in &m.faces {
        for (l, _) in remap_loop(m, &f.verts, remap) {
            for i in 0..l.len() {
                *after.entry(edge(l[i], l[(i + 1) % l.len()])).or_default() += 1;
            }
        }
    }
    after.iter().any(|(e, n)| *n > 2 && before.get(e).copied().unwrap_or(0) <= 2)
}

/// `UBOpUtils.remap_faces`: rewrites every face through the remap (manual UVs and corner
/// colours follow their corners), then drops unused vertices.
fn remap_faces(m: &mut BMesh, remap: &HashMap<u32, u32>) {
    let mut kept = Vec::new();
    let mut extra = Vec::new();
    let faces = std::mem::take(&mut m.faces);
    for f in faces {
        let old_n = f.verts.len();
        let pieces = remap_loop(m, &f.verts, remap);
        let count = pieces.len();
        for (k, (out, src)) in pieces.into_iter().enumerate() {
            let mut face = f.clone();
            if out.len() != old_n || count > 1 {
                if face.uv_coords.len() == old_n {
                    face.uv_coords = src.iter().map(|i| f.uv_coords[*i]).collect();
                }
                if face.color_corners.len() == old_n {
                    face.color_corners = src.iter().map(|i| f.color_corners[*i]).collect();
                }
            }
            face.verts = out;
            if k == 0 { kept.push(face) } else { extra.push(face) }
        }
    }
    kept.extend(extra);
    m.faces = kept;
    m.compact();
}

/// `UBOpCollapseVertices.apply`: collapses the selected vertices into one at their centre.
/// Refused (returns `None`) when that would leave an edge shared by more than two faces.
/// Returns the index of the collapsed vertex after compaction.
pub fn collapse_vertices(m: &mut BMesh, verts: &BTreeSet<u32>) -> Option<u32> {
    let list: Vec<u32> = verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect();
    if list.len() < 2 {
        return None;
    }
    let rep = list[0];
    let target = list.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / list.len() as f32;
    let remap: HashMap<u32, u32> = list.iter().map(|v| (*v, rep)).collect();
    if remap_overshares(m, &remap) {
        return None;
    }
    m.positions[rep as usize] = target;
    // Find the survivor again after compaction by its position.
    remap_faces(m, &remap);
    m.positions.iter().position(|p| (*p - target).length() < 1e-6).map(|i| i as u32)
}

/// Insert-vertex modal (`plugin.gd::commit_insert_vertex`): splits `e` at `pos` in every face
/// using it. Returns the new vertex, `None` when `e` is not an edge of the mesh.
pub fn insert_vertex(m: &mut BMesh, e: Edge, pos: Vec3) -> Option<u32> {
    let e = edge(e.0, e.1);
    if !m.faces.iter().any(|f| f.loop_edges().any(|(a, b)| edge(a, b) == e)) {
        return None;
    }
    let v = m.add_vert(pos);
    insert_edge_points(m, &BTreeMap::from([(e, vec![v])]), &BTreeSet::new());
    Some(v)
}

// ---------------------------------------------------------------- polygon helpers

/// `UBOpUtils.kernel_point`: a point strictly inside the polygon's kernel (seen by every
/// edge), or `None` when it is empty or a sliver.
fn kernel_point(lp: &LoopPlane) -> Option<Vec2> {
    let r = lp.scale * 2.0 + 1.0;
    let mut poly = vec![v2(-r, -r), v2(r, -r), v2(r, r), v2(-r, r)];
    let pts = &lp.pts;
    let n = pts.len();
    for i in 0..n {
        let a = pts[i];
        let dir = pts[(i + 1) % n] - a;
        if dir.dot(dir) <= 0.0 {
            continue;
        }
        let mut clipped = Vec::new();
        let k = poly.len();
        for j in 0..k {
            let (p, q) = (poly[j], poly[(j + 1) % k]);
            let sp = lp.orient * cross_2d(dir, p - a);
            let sq = lp.orient * cross_2d(dir, q - a);
            if sp >= 0.0 {
                clipped.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                clipped.push(p + (q - p) * (sp / (sp - sq)));
            }
        }
        poly = clipped;
        if poly.len() < 3 {
            return None;
        }
    }
    let area = signed_area_2d(&poly);
    if area.abs() <= 1e-6 * lp.scale * lp.scale {
        return None;
    }
    let k = poly.len();
    let c = (0..k).fold(Vec2::ZERO, |c, j| {
        let (p, q) = (poly[j], poly[(j + 1) % k]);
        c + (p + q) * cross_2d(p, q)
    });
    Some(c / (6.0 * area))
}

/// `UBOpUtils.layout_ok`: the 2D loops form a valid planar layout (all wound like the face,
/// no duplicate corners, no crossing edges).
fn layout_ok(loops: &[Vec<Vec2>], orient: f32, scale: f32) -> bool {
    let area_eps = 1e-9 * scale * scale;
    let len_eps = 1e-6 * scale;
    let mut edges = Vec::new();
    for l in loops {
        let n = l.len();
        if n < 3 || orient * signed_area_2d(l) <= area_eps {
            return false;
        }
        for i in 0..n {
            for j in i + 1..n {
                if l[i].distance(l[j]) <= len_eps {
                    return false;
                }
            }
            edges.push((l[i], l[(i + 1) % n]));
        }
    }
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            if segments_cross(edges[i].0, edges[i].1, edges[j].0, edges[j].1, area_eps) {
                return false;
            }
        }
    }
    true
}

/// `UBOpUtils.loop_weights`: per-corner barycentric weights of `p` over a loop's corners.
fn loop_weights(pts: &[Vec3], p: Vec3) -> Vec<f32> {
    let mut w = vec![0.0; pts.len()];
    let tris = triangulate(pts, super::model::newell(pts));
    let mut best = -1.0e30f32;
    for t in &tris {
        let a = pts[t[0]];
        let (ab, ac, ap) = (pts[t[1]] - a, pts[t[2]] - a, p - a);
        let (d00, d01, d11, d20, d21) = (ab.dot(ab), ab.dot(ac), ac.dot(ac), ap.dot(ab), ap.dot(ac));
        let denom = d00 * d11 - d01 * d01;
        if denom.abs() < 1e-12 {
            continue;
        }
        let bv = (d11 * d20 - d01 * d21) / denom;
        let bw = (d00 * d21 - d01 * d20) / denom;
        let bu = 1.0 - bv - bw;
        let fit = bu.min(bv.min(bw));
        if fit <= best {
            continue;
        }
        let (bu, bv, bw) = (bu.max(0.0), bv.max(0.0), bw.max(0.0));
        let s = bu + bv + bw;
        if s < 1e-12 {
            continue;
        }
        best = fit;
        w.iter_mut().for_each(|x| *x = 0.0);
        w[t[0]] += bu / s;
        w[t[1]] += bv / s;
        w[t[2]] += bw / s;
    }
    if best <= -1.0e30 {
        w.iter_mut().for_each(|x| *x = 1.0 / pts.len().max(1) as f32);
    }
    w
}

// ---------------------------------------------------------------- inset

fn inward_edge_normal(face_normal: Vec3, a: Vec3, b: Vec3, center: Vec3) -> Vec3 {
    let perp = face_normal.cross(b - a).normalized();
    if perp.dot(center - (a + b) * 0.5) < 0.0 { -perp } else { perp }
}

fn center_offsets(pts: &[Vec3], normal: Vec3, center: Vec3, amount: f32) -> Vec<Vec3> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let p = pts[i];
            let to_c = center - p;
            let dist = to_c.length();
            if dist <= 1e-6 {
                return Vec3::ZERO;
            }
            let dir = to_c / dist;
            let (prev, next) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let (mut proj, mut count) = (0.0, 0);
            for nrm in [inward_edge_normal(normal, prev, p, center), inward_edge_normal(normal, p, next, center)] {
                if nrm != Vec3::ZERO {
                    proj += dir.dot(nrm);
                    count += 1;
                }
            }
            let mut travel = amount;
            if count > 0 && proj / count as f32 > 1e-3 {
                travel = amount * count as f32 / proj;
            }
            dir * travel.min(dist * 0.9)
        })
        .collect()
}

fn equal_offsets(pts: &[Vec3], normal: Vec3, center: Vec3, amount: f32) -> Vec<Vec3> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let p = pts[i];
            let (prev, next) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let n1 = inward_edge_normal(normal, prev, p, center);
            let n2 = inward_edge_normal(normal, p, next, center);
            let denom = 1.0 + n1.dot(n2);
            let v = if denom > 1e-6 { (n1 + n2) * (amount / denom) } else { n1 * amount };
            let to_c = center - p;
            let mut s = 1.0f32;
            for nrm in [n1, n2] {
                let travel = v.dot(nrm);
                if travel > 1e-9 {
                    s = s.min(0.9 * to_c.dot(nrm).max(0.0) / travel);
                }
            }
            v * s
        })
        .collect()
}

fn miter_offsets(lp: &LoopPlane, amount: f32) -> Vec<Vec3> {
    let pts = &lp.pts;
    let n = pts.len();
    let normals: Vec<Vec2> = (0..n)
        .map(|i| {
            let d = pts[(i + 1) % n] - pts[i];
            v2(-d.y, d.x).normalized() * lp.orient
        })
        .collect();
    (0..n)
        .map(|i| {
            let (n1, n2) = (normals[(i + n - 1) % n], normals[i]);
            let denom = 1.0 + n1.dot(n2);
            let v = if denom > 1e-6 { (n1 + n2) * (amount / denom) } else { n1 * amount };
            lp.u * v.x + lp.v * v.y
        })
        .collect()
}

fn plan_ok(lp: &LoopPlane, offsets: &[Vec3]) -> bool {
    let n = lp.pts.len();
    let inner: Vec<Vec2> = (0..n).map(|i| lp.pts[i] + lp.dir_2d(offsets[i])).collect();
    let mut loops = vec![inner.clone()];
    for i in 0..n {
        let j = (i + 1) % n;
        loops.push(vec![lp.pts[i], lp.pts[j], inner[j], inner[i]]);
    }
    layout_ok(&loops, lp.orient, lp.scale)
}

/// `UBOpInsetFaces.apply`: insets each selected face by a rim `amount` wide (world units);
/// `equal` mitres the corners so the rim has the same width everywhere. Concave faces use
/// their kernel point / mitred offsets and are skipped when no layout is valid. Returns the
/// inner faces (the original indices).
pub fn inset_faces(m: &mut BMesh, faces: &BTreeSet<usize>, amount: f32, equal: bool) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for &fi in faces {
        if fi >= m.faces.len() {
            continue;
        }
        let pts = m.face_points(fi);
        let normal = m.face_normal(fi);
        let offsets = match loop_plane(m, &m.faces[fi].verts) {
            Some(lp) if lp.is_concave() => {
                let center_plan = kernel_point(&lp).map(|k| center_offsets(&pts, normal, lp.to_3d(k), amount)).unwrap_or_default();
                let miter_plan = miter_offsets(&lp, amount);
                let plans = if equal { [miter_plan, center_plan] } else { [center_plan, miter_plan] };
                match plans.into_iter().find(|p| !p.is_empty() && plan_ok(&lp, p)) {
                    Some(p) => p,
                    None => continue,
                }
            }
            _ => {
                let c = m.face_center(fi);
                if equal { equal_offsets(&pts, normal, c, amount) } else { center_offsets(&pts, normal, c, amount) }
            }
        };
        // Extrude by 0 (`UBOpExtrudeFaces.apply(…, 0.0)`): a ring of quads, inner loop on top.
        let old = m.faces[fi].verts.clone();
        let inner: Vec<u32> = old.iter().zip(&offsets).map(|(v, o)| m.add_vert(m.positions[*v as usize] + *o)).collect();
        let src = m.faces[fi].clone();
        let n = old.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let mut side = src.with_verts(vec![old[i], old[j], inner[j], inner[i]]);
            side.smooth = 0;
            if let Some(c) = src.corner_colors() {
                side.color_corners = vec![c[i], c[j], c[j], c[i]];
            }
            m.faces.push(side);
        }
        m.faces[fi].verts = inner;
        out.insert(fi);
    }
    out
}

// ---------------------------------------------------------------- subdivide

fn cells_2d(pts: &[Vec2], c: Vec2, segments: usize) -> Vec<Vec<Vec2>> {
    let n = pts.len();
    let h = segments / 2;
    let mut cells = Vec::new();
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let (corner, nxt, before) = (pts[i], pts[(i + 1) % n], pts[prev]);
        let mut g = vec![vec![Vec2::ZERO; h + 1]; h + 1];
        let mid_i = corner.lerp(nxt, 0.5);
        let mid_prev = before.lerp(corner, 0.5);
        for a in 0..=h {
            g[a][0] = corner.lerp(nxt, a as f32 / segments as f32);
            g[a][h] = mid_prev.lerp(c, a as f32 / h as f32);
        }
        for b in 1..=h {
            g[0][b] = corner.lerp(before, b as f32 / segments as f32);
            g[h][b] = mid_i.lerp(c, b as f32 / h as f32);
        }
        let (p00, p10, p01) = (g[0][0], g[h][0], g[0][h]);
        for a in 1..h {
            for b in 1..h {
                let (s, t) = (a as f32 / h as f32, b as f32 / h as f32);
                g[a][b] = g[a][0] * (1.0 - t) + g[a][h] * t + g[0][b] * (1.0 - s) + g[h][b] * s
                    - (p00 * ((1.0 - s) * (1.0 - t)) + p10 * (s * (1.0 - t)) + p01 * ((1.0 - s) * t) + c * (s * t));
            }
        }
        for a in 0..h {
            for b in 0..h {
                cells.push(vec![g[a][b], g[a + 1][b], g[a + 1][b + 1], g[a][b + 1]]);
            }
        }
    }
    cells
}

/// Corner attribute that can be blended (corner colours as floats, UVs).
trait Blend: Copy {
    fn mix(self, o: Self, t: f32) -> Self;
    fn add(self, o: Self) -> Self;
    fn sub(self, o: Self) -> Self;
    fn scale(self, s: f32) -> Self;
}
impl Blend for Vec2 {
    fn mix(self, o: Self, t: f32) -> Self {
        self.lerp(o, t)
    }
    fn add(self, o: Self) -> Self {
        self + o
    }
    fn sub(self, o: Self) -> Self {
        self - o
    }
    fn scale(self, s: f32) -> Self {
        self * s
    }
}
impl Blend for [f32; 4] {
    fn mix(self, o: Self, t: f32) -> Self {
        std::array::from_fn(|k| self[k] + (o[k] - self[k]) * t)
    }
    fn add(self, o: Self) -> Self {
        std::array::from_fn(|k| self[k] + o[k])
    }
    fn sub(self, o: Self) -> Self {
        std::array::from_fn(|k| self[k] - o[k])
    }
    fn scale(self, s: f32) -> Self {
        std::array::from_fn(|k| self[k] * s)
    }
}

fn to_f(c: [u8; 4]) -> [f32; 4] {
    c.map(|x| x as f32)
}
fn to_b(c: [f32; 4]) -> [u8; 4] {
    c.map(|x| x.round().clamp(0.0, 255.0) as u8)
}

/// Coons-patch fill of the grid interior (`subdivide_faces.gd` inner loop), shared by
/// positions, colours and UVs.
fn coons<T: Blend>(g: &[Vec<u32>], h: usize, val: &HashMap<u32, T>, center: T, a: usize, b: usize) -> T {
    let (s, t) = (a as f32 / h as f32, b as f32 / h as f32);
    let at = |i: usize, j: usize| val[&g[i][j]];
    at(a, 0)
        .scale(1.0 - t)
        .add(at(a, h).scale(t))
        .add(at(0, b).scale(1.0 - s))
        .add(at(h, b).scale(s))
        .sub(at(0, 0).scale((1.0 - s) * (1.0 - t)).add(at(h, 0).scale(s * (1.0 - t))).add(at(0, h).scale((1.0 - s) * t)).add(center.scale(s * t)))
}

/// `UBOpSubdivideFaces.apply`: subdivides each selected face into quads, `segments` (even,
/// ≥ 2) pieces along every edge: a corner patch per corner meeting at the centre (the
/// kernel point for concave faces; faces whose patches would overlap are skipped). Edges are
/// split in the neighbours too; corner colours and manual UVs are interpolated. Returns the
/// new faces (appended after the untouched ones).
pub fn subdivide_faces(m: &mut BMesh, faces: &BTreeSet<usize>, segments: u32) -> BTreeSet<usize> {
    let segments = (segments.max(2) - segments % 2).max(2) as usize;
    let h = segments / 2;
    let mut face_set: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    let mut centers: HashMap<usize, Vec3> = HashMap::new();
    for fi in face_set.clone() {
        let Some(lp) = loop_plane(m, &m.faces[fi].verts) else { continue };
        if !lp.is_concave() {
            continue;
        }
        match kernel_point(&lp) {
            Some(kp) if layout_ok(&cells_2d(&lp.pts, kp, segments), lp.orient, lp.scale) => {
                centers.insert(fi, lp.to_3d(kp));
            }
            _ => {
                face_set.remove(&fi);
            }
        }
    }
    if face_set.is_empty() {
        return BTreeSet::new();
    }
    let mut edge_pts: BTreeMap<Edge, Vec<u32>> = BTreeMap::new();
    for &fi in &face_set {
        let verts = m.faces[fi].verts.clone();
        for i in 0..verts.len() {
            let e = edge(verts[i], verts[(i + 1) % verts.len()]);
            if !edge_pts.contains_key(&e) {
                let (pa, pb) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
                let list = (1..segments).map(|s| m.add_vert(pa.lerp(pb, s as f32 / segments as f32))).collect();
                edge_pts.insert(e, list);
            }
        }
    }
    insert_edge_points(m, &edge_pts, &face_set);
    let mut new_faces: Vec<Face> = Vec::new();
    for &fi in &face_set {
        let face = m.faces[fi].clone();
        let verts = face.verts.clone();
        let n = verts.len();
        let center_pos = centers.get(&fi).copied().unwrap_or_else(|| m.face_center(fi));
        let center = m.add_vert(center_pos);
        let cw = centers.contains_key(&fi).then(|| loop_weights(&verts.iter().map(|v| m.positions[*v as usize]).collect::<Vec<_>>(), center_pos));
        let has_col = face.corner_colors().is_some();
        let has_uv = face.manual_uvs().is_some();
        let mut vcol: HashMap<u32, [f32; 4]> = HashMap::new();
        let mut vuv: HashMap<u32, Vec2> = HashMap::new();
        let mut center_col = [0.0; 4];
        let mut center_uv = Vec2::ZERO;
        if has_col {
            for i in 0..n {
                vcol.insert(verts[i], to_f(face.color_corners[i]));
            }
            center_col = match &cw {
                Some(w) => (0..n).fold([0.0; 4], |a, i| a.add(to_f(face.color_corners[i]).scale(w[i]))),
                None => (0..n).fold([0.0; 4], |a, i| a.add(to_f(face.color_corners[i]))).scale(1.0 / n as f32),
            };
            vcol.insert(center, center_col);
        }
        if has_uv {
            for i in 0..n {
                vuv.insert(verts[i], face.uv_coords[i]);
            }
            center_uv = match &cw {
                Some(w) => (0..n).fold(Vec2::ZERO, |a, i| a + face.uv_coords[i] * w[i]),
                None => (0..n).fold(Vec2::ZERO, |a, i| a + face.uv_coords[i]) / n as f32,
            };
            vuv.insert(center, center_uv);
        }
        // Edge points in this face's winding.
        let pts: Vec<Vec<u32>> = (0..n)
            .map(|i| {
                let a = verts[i];
                let key = edge(a, verts[(i + 1) % n]);
                let mut l = edge_pts[&key].clone();
                if a != key.0 {
                    l.reverse();
                }
                l
            })
            .collect();
        for i in 0..n {
            let j = (i + 1) % n;
            for s in 1..segments {
                let t = s as f32 / segments as f32;
                if has_col {
                    vcol.insert(pts[i][s - 1], to_f(face.color_corners[i]).mix(to_f(face.color_corners[j]), t));
                }
                if has_uv {
                    vuv.insert(pts[i][s - 1], face.uv_coords[i].lerp(face.uv_coords[j], t));
                }
            }
        }
        let spokes: Vec<Vec<u32>> = (0..n)
            .map(|i| {
                let mid = pts[i][h - 1];
                let mut s = vec![mid];
                for k in 1..h {
                    let p = m.positions[mid as usize].lerp(center_pos, k as f32 / h as f32);
                    s.push(m.add_vert(p));
                }
                s.push(center);
                s
            })
            .collect();
        for i in 0..n {
            for k in 1..h {
                let t = k as f32 / h as f32;
                if has_col {
                    let mc = vcol[&pts[i][h - 1]];
                    vcol.insert(spokes[i][k], mc.mix(center_col, t));
                }
                if has_uv {
                    let mu = vuv[&pts[i][h - 1]];
                    vuv.insert(spokes[i][k], mu.lerp(center_uv, t));
                }
            }
        }
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let mut g = vec![vec![0u32; h + 1]; h + 1];
            for a in 0..=h {
                g[a][0] = if a == 0 { verts[i] } else { pts[i][a - 1] };
                g[a][h] = spokes[prev][a];
            }
            for b in 1..=h {
                g[0][b] = pts[prev][segments - 1 - b];
                g[h][b] = spokes[i][b];
            }
            let posmap: HashMap<u32, Vec3> = g.iter().flatten().map(|v| (*v, m.positions[*v as usize])).collect();
            for a in 1..h {
                for b in 1..h {
                    // Positions through the same Coons blend (Vec3 via two Vec2 halves is not
                    // needed: do it inline).
                    let (s, t) = (a as f32 / h as f32, b as f32 / h as f32);
                    let at = |i: usize, j: usize| posmap.get(&g[i][j]).copied().unwrap_or_else(|| m.positions[g[i][j] as usize]);
                    let p = at(a, 0) * (1.0 - t) + at(a, h) * t + at(0, b) * (1.0 - s) + at(h, b) * s
                        - (at(0, 0) * ((1.0 - s) * (1.0 - t)) + at(h, 0) * (s * (1.0 - t)) + at(0, h) * ((1.0 - s) * t) + center_pos * (s * t));
                    let v = m.add_vert(p);
                    g[a][b] = v;
                    if has_col {
                        let c = coons(&g, h, &vcol, center_col, a, b);
                        vcol.insert(v, c);
                    }
                    if has_uv {
                        let u = coons(&g, h, &vuv, center_uv, a, b);
                        vuv.insert(v, u);
                    }
                }
            }
            for a in 0..h {
                for b in 0..h {
                    let lp = vec![g[a][b], g[a + 1][b], g[a + 1][b + 1], g[a][b + 1]];
                    let mut cell = face.with_verts(lp.clone());
                    if has_col {
                        cell.color_corners = lp.iter().map(|v| to_b(vcol[v])).collect();
                    }
                    if has_uv {
                        cell.uv_coords = lp.iter().map(|v| vuv[v]).collect();
                        cell.uv_manual = true;
                    }
                    new_faces.push(cell);
                }
            }
        }
    }
    let old = std::mem::take(&mut m.faces);
    m.faces = old.into_iter().enumerate().filter(|(i, _)| !face_set.contains(i)).map(|(_, f)| f).collect();
    let first = m.faces.len();
    m.faces.extend(new_faces);
    let _ = lerp_color;
    (first..m.faces.len()).collect()
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    fn top(m: &BMesh) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap()
    }

    #[test]
    fn subdivide_four_segments_keeps_closed_and_lerps_paint() {
        let mut m = cuboid(v3(2.0, 2.0, 2.0));
        let t = top(&m);
        m.faces[t].color_corners = vec![[0, 0, 0, 255], [255, 255, 255, 255], [255, 255, 255, 255], [0, 0, 0, 255]];
        let new = subdivide_faces(&mut m, &BTreeSet::from([t]), 4);
        assert_eq!(new.len(), 16);
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 8.0).abs() < 1e-4);
        let area: f32 = new.iter().map(|f| m.face_area(*f)).sum();
        assert!((area - 4.0).abs() < 1e-4);
        assert!(new.iter().all(|f| m.faces[*f].color_corners.len() == 4));
        // Odd counts round down to even.
        let mut m2 = cuboid(v3(1.0, 1.0, 1.0));
        let t2 = top(&m2);
        assert_eq!(subdivide_faces(&mut m2, &BTreeSet::from([t2]), 3).len(), 4);
    }

    #[test]
    fn subdivide_concave_l_uses_the_kernel() {
        let mut m = BMesh::default();
        let pts = [v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(2.0, 0.0, -1.0), v3(1.0, 0.0, -1.0), v3(1.0, 0.0, -2.0), v3(0.0, 0.0, -2.0)];
        let vs: Vec<u32> = pts.iter().map(|p| m.add_vert(*p)).collect();
        m.add_face(vs);
        let new = subdivide_faces(&mut m, &BTreeSet::from([0]), 2);
        assert_eq!(new.len(), 6);
        let area: f32 = new.iter().map(|f| m.face_area(*f)).sum();
        assert!((area - 3.0).abs() < 1e-4);
        assert!(new.iter().all(|f| m.face_normal(*f).y > 0.99));
    }

    #[test]
    fn inset_width_and_equal_rims() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let t = top(&m);
        let inner = inset_faces(&mut m, &BTreeSet::from([t]), 0.2, true);
        assert!(m.is_closed());
        let f = *inner.first().unwrap();
        // Equal: a 2 x 1 face loses 0.2 on every side -> 1.6 x 0.6.
        assert!((m.face_area(f) - 0.96).abs() < 1e-4, "{}", m.face_area(f));
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let t = top(&m);
        let f = *inset_faces(&mut m, &BTreeSet::from([t]), 0.2, false).first().unwrap();
        assert!(m.is_closed());
        assert!(m.face_area(f) < 2.0 && m.face_area(f) > 0.5);
    }

    #[test]
    fn collapse_and_insert_vertex() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = top(&m);
        let v = m.faces[t].verts.clone();
        let mid = (m.positions[v[0] as usize] + m.positions[v[1] as usize]) * 0.5;
        let p = insert_vertex(&mut m, (v[0], v[1]), mid).unwrap();
        assert!(m.is_closed());
        assert_eq!(m.faces[t].verts.len(), 5);
        assert!(insert_vertex(&mut m, (v[0], v[2]), Vec3::ZERO).is_none());
        // Collapse the new vertex into its edge neighbour: back to a closed box.
        assert!(collapse_vertices(&mut m, &BTreeSet::from([p, v[0]])).is_some());
        m.validate().unwrap();
        assert!(m.is_closed());
        assert_eq!(m.positions.len(), 8);
        // Collapsing two opposite top corners would fold the top: refused or still valid.
        let mut b = cuboid(v3(1.0, 1.0, 1.0));
        let t = top(&b);
        let tv = b.faces[t].verts.clone();
        if collapse_vertices(&mut b, &BTreeSet::from([tv[0], tv[2]])).is_some() {
            b.validate().unwrap();
        }
    }
}
