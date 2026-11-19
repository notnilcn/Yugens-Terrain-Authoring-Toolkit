//! Xogot Modeler UV extras on the builder's `BMesh` (docs/port/06-xogot.md, package P4), on top
//! on top of the builder's own UV editor ops in `uv_edit.rs` (planar / box projection, fit, weld, island of
//! manual faces, copy UVs). Ports of xogot-rust (MIT, Xibbon Inc. / Xogot Modeler):
//! `modeler_kit/src/mesh_uv.rs` (`MeshUV::project` spherical + `unwrap_seam`, `MeshUV::stitch`,
//! `MeshUV::copy_face` / `paste`, `MeshUV::island`), `modeler_kit/src/mesh_compiler.rs`
//! (`refresh_uvs` fill / `anchor_point` / `transformed_uv`, the `AutoUnwrap` settings of
//! `editable_mesh.rs`), `editor_core/src/uv_editor_state.rs` (`pick` Ctrl / Ctrl+Shift click,
//! `template_pixels` + `UVTemplateRaster`, `capsule_span`, `linear_range`, `composite`).
//!
//! The builder uses Godot's UV convention (V grows downwards, as in `uv.rs` and the UV editor
//! canvas), so everything Xogot does with V-up is mirrored here: spherical latitude runs from the
//! north pole at v = 0, the *Upper* anchors pin the low-V edge, and the template's pixel row is
//! `v * size` (no `1 - v`), i.e. the image lines up with the texture the UVs sample.
//!
//! Gap fixes: Auto UV settings keep plain automatic UVs (`UvParams`) whenever the settings can be
//! expressed by them (tile, no anchor, no swap: flips become negative scales) and only bake
//! manual UVs otherwise; the template can wrap each face into the 0–1 square (world-space
//! automatic UVs rarely lie inside it); the UV island also follows automatic faces.

use std::collections::{BTreeMap, BTreeSet};

use super::model::BMesh;
use super::uv::projection_frame;
use super::uv_edit::{AutoUv, Rect, freeze_manual_uvs, copy_uvs, face_corner_uvs, planar_project};
use crate::math::*;

/// Error text for a refused operation (shown to the user).
pub type OpResult<T> = Result<T, String>;

fn valid(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<usize> {
    faces.iter().copied().filter(|f| *f < m.faces.len()).collect()
}

// ================================================================== spherical projection

/// Spherical seam fix (port of `unwrap_seam`): a face whose U values span more than one half
/// wraps its low side.
fn unwrap_seam(uvs: &mut [Vec2]) {
    let Some(first) = uvs.first().map(|u| u.x) else { return };
    let high = uvs.iter().fold(first, |m, u| m.max(u.x));
    let low = uvs.iter().fold(first, |m, u| m.min(u.x));
    if high - low <= 0.5 {
        return;
    }
    for u in uvs {
        if u.x < 0.5 {
            u.x += 1.0;
        }
    }
}

/// Spherical projection (port of `MeshUV::project(.spherical)`): longitude / latitude of every
/// corner around the faces' vertex centre, one repeat around (u) and pole to pole (v, north at
/// v = 0). The faces become manual. Refused when a vertex sits on the centre.
pub fn spherical_project(m: &mut BMesh, faces: &BTreeSet<usize>) -> OpResult<()> {
    let fs = valid(m, faces);
    if fs.is_empty() {
        return Err("Select faces to project.".into());
    }
    let verts = m.verts_of_faces(&fs.iter().copied().collect());
    let center = verts.iter().fold(Vec3::ZERO, |a, v| a + m.positions[*v as usize]) / verts.len() as f32;
    let sphere = |p: Vec3| -> Option<Vec2> {
        let d = p - center;
        let len = d.length();
        if len <= 1e-10 {
            return None;
        }
        // Longitude negated from Xogot's `atan2(z, x)` so U runs to the viewer's right (with V
        // mirrored, Xogot's formula reads backwards in Godot's UV convention).
        let lon = (-d.z).atan2(d.x);
        let lat = (d.y / len).clamp(-1.0, 1.0).asin();
        Some(v2(lon / std::f32::consts::TAU + 0.5, 0.5 - lat / std::f32::consts::PI))
    };
    let mut out = Vec::with_capacity(fs.len());
    for &f in &fs {
        let mut uvs = Vec::with_capacity(m.faces[f].verts.len());
        for v in &m.faces[f].verts {
            uvs.push(sphere(m.positions[*v as usize]).ok_or("A spherical projection cannot include its center.")?);
        }
        unwrap_seam(&mut uvs);
        out.push((f, uvs));
    }
    for (f, uvs) in out {
        m.faces[f].uv_coords = uvs;
        m.faces[f].uv_manual = true;
    }
    Ok(())
}

// ================================================================== auto UV settings

/// How the automatic projection fills the face (Xogot `Fill`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Fill {
    /// World-density tiling (the builder's normal automatic UVs).
    #[default]
    Tile,
    /// Uniformly scaled so the longer side spans 0–1.
    Fit,
    /// Each side scaled to span 0–1.
    Stretch,
}

/// Which point of the projected bounds sits on the matching point of the 0–1 square (Xogot
/// `Anchor`). *Upper* = low V (the texture's top).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    None,
    UpperLeft,
    UpperCenter,
    UpperRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    LowerLeft,
    LowerCenter,
    LowerRight,
}

impl Anchor {
    /// Declaration order (None first, then row by row) — handy for an OptionButton index.
    pub const ALL: [Anchor; 10] = [
        Anchor::None,
        Anchor::UpperLeft,
        Anchor::UpperCenter,
        Anchor::UpperRight,
        Anchor::MiddleLeft,
        Anchor::MiddleCenter,
        Anchor::MiddleRight,
        Anchor::LowerLeft,
        Anchor::LowerCenter,
        Anchor::LowerRight,
    ];
}

/// Xogot's per-face `AutoUnwrap` options that the builder's `UvParams` lacks (offset, rotation,
/// scale and world space stay in each face's `UvParams`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AutoSettings {
    pub fill: Fill,
    pub anchor: Anchor,
    pub flip_u: bool,
    pub flip_v: bool,
    pub swap_uv: bool,
    /// Treat the faces as one batch (Xogot texture group): one projection plane (summed
    /// normal) and one bounds for fill / anchor, so the texture runs across them.
    pub group: bool,
}

/// The UV-space point that maps to the anchor's point of the unit square (port of
/// `anchor_point`, V mirrored: *upper* is the low-V edge). `None` anchors at the origin.
fn anchor_point(anchor: Anchor, r: Rect) -> Vec2 {
    let c = r.center();
    let left = r.min.x;
    let mid_x = c.x - 0.5;
    let right = r.max.x - 1.0;
    let upper = r.min.y;
    let mid_y = c.y - 0.5;
    let lower = r.max.y - 1.0;
    match anchor {
        Anchor::None => Vec2::ZERO,
        Anchor::UpperLeft => v2(left, upper),
        Anchor::UpperCenter => v2(mid_x, upper),
        Anchor::UpperRight => v2(right, upper),
        Anchor::MiddleLeft => v2(left, mid_y),
        Anchor::MiddleCenter => v2(mid_x, mid_y),
        Anchor::MiddleRight => v2(right, mid_y),
        Anchor::LowerLeft => v2(left, lower),
        Anchor::LowerCenter => v2(mid_x, lower),
        Anchor::LowerRight => v2(right, lower),
    }
}

/// The face's `UvParams` (rotation, |scale|, offset) as in `uv::auto_uv`; the scale's sign is
/// the flip state, applied separately.
fn params_xf(q: Vec2, p: &super::model::UvParams) -> Vec2 {
    let r = q.rotated(p.rotation.to_radians());
    let fix = |x: f32| if x.abs() < 1e-6 { 1.0 } else { x.abs() };
    let s = v2(fix(p.scale.x), fix(p.scale.y));
    v2(r.x / s.x, r.y / s.y) + p.offset
}

/// Applies Xogot's Auto UV settings to the faces once (port of `MeshUV::set_auto_settings` +
/// `MeshCompiler::refresh_uvs`). The faces are projected from their positions (manual UVs are
/// replaced), filled, anchored, then their own `UvParams` (rotation, scale, offset), swap and
/// flips are applied.
///
/// With `group`, the whole batch uses the first face's `UvParams` and world flag (like Xogot's
/// texture groups), so the texture runs on across the faces.
///
/// The flip state is always stored as the sign of `UvParams::scale` (so the UI can read it
/// back). Tile with no anchor, no swap and no grouping stays automatic: the faces drop manual
/// UVs and the sign alone flips them. Anything else bakes manual UVs.
/// Returns the number of faces baked to manual UVs.
pub fn apply_auto_settings(m: &mut BMesh, faces: &BTreeSet<usize>, s: &AutoSettings, a: &AutoUv) -> usize {
    let fs = valid(m, faces);
    for f in &fs {
        let sc = &mut m.faces[*f].uv.scale;
        sc.x = if s.flip_u { -sc.x.abs() } else { sc.x.abs() };
        sc.y = if s.flip_v { -sc.y.abs() } else { sc.y.abs() };
    }
    if s.fill == Fill::Tile && s.anchor == Anchor::None && !s.swap_uv && !s.group {
        for f in fs {
            m.faces[f].uv_manual = false;
            m.faces[f].uv_coords.clear();
        }
        return 0;
    }
    let batches: Vec<Vec<usize>> = if s.group { vec![fs.clone()] } else { fs.iter().map(|f| vec![*f]).collect() };
    let k = 1.0 / a.units_per_repeat.max(1e-6);
    for batch in batches {
        if batch.is_empty() {
            continue;
        }
        let normal = if s.group {
            let n = batch.iter().fold(Vec3::ZERO, |acc, f| acc + m.face_normal(*f));
            if n.dot(n) < 1e-10 { Vec3::UP } else { n.normalized() }
        } else {
            let f = batch[0];
            m.faces[f].uv_normal(m.face_normal(f))
        };
        let (u, v) = projection_frame(normal);
        let lead = m.faces[batch[0]].uv;
        let mut coords: Vec<Vec<Vec2>> = batch
            .iter()
            .map(|f| {
                m.faces[*f]
                    .verts
                    .iter()
                    .map(|vi| {
                        let p = m.positions[*vi as usize];
                        let p = if lead.world { p + a.world_offset } else { p };
                        v2(p.dot(u) * k, p.dot(v) * k)
                    })
                    .collect()
            })
            .collect();
        let Some(r) = Rect::of(coords.iter().flatten().copied()) else { continue };
        let ext = r.size();
        match s.fill {
            Fill::Tile => {}
            Fill::Fit => {
                let d = ext.x.max(ext.y).max(1e-10);
                coords.iter_mut().flatten().for_each(|c| *c = (*c - r.min) / d);
            }
            Fill::Stretch => {
                let d = v2(ext.x.max(1e-10), ext.y.max(1e-10));
                coords.iter_mut().flatten().for_each(|c| *c = v2((c.x - r.min.x) / d.x, (c.y - r.min.y) / d.y));
            }
        }
        let anchor = anchor_point(s.anchor, Rect::of(coords.iter().flatten().copied()).unwrap_or(r));
        for (f, cs) in batch.iter().zip(coords) {
            let params = if s.group { lead } else { m.faces[*f].uv };
            let uvs = cs
                .into_iter()
                .map(|c| {
                    let mut uv = params_xf(c - anchor, &params);
                    if s.swap_uv {
                        uv = v2(uv.y, uv.x);
                    }
                    if s.flip_u {
                        uv.x = -uv.x;
                    }
                    if s.flip_v {
                        uv.y = -uv.y;
                    }
                    uv
                })
                .collect();
            m.faces[*f].uv_coords = uvs;
            m.faces[*f].uv_manual = true;
        }
    }
    fs.len()
}

// ================================================================== island / stitch / copy

/// Faces in the same UV island as `seed`: connected through a shared vertex at the same shown
/// UV (port of `MeshUV::island`). Unlike `uv_edit::island_faces` this also follows automatic
/// faces, whose world-space UVs are continuous across flat neighbours. Automatic faces are only
/// joined when they also share the projection axis (perpendicular faces can meet at the same
/// UV by coincidence, e.g. at the world origin).
pub fn uv_island(m: &BMesh, seed: usize, a: &AutoUv, eps: f32) -> BTreeSet<usize> {
    if seed >= m.faces.len() {
        return BTreeSet::new();
    }
    let shown: Vec<Vec<Vec2>> = (0..m.faces.len()).map(|f| face_corner_uvs(m, f, a)).collect();
    let mut by_vertex: BTreeMap<u32, Vec<(usize, Vec2)>> = BTreeMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        for (ci, v) in f.verts.iter().enumerate() {
            by_vertex.entry(*v).or_default().push((fi, shown[fi][ci]));
        }
    }
    // Automatic faces project along their dominant axis; two of them only form one chart when
    // the axes match.
    let axis = |f: usize| {
        let (u, v) = projection_frame(m.faces[f].uv_normal(m.face_normal(f)));
        u.cross(v)
    };
    let same_chart = |a: usize, b: usize| {
        m.faces[a].manual_uvs().is_some() || m.faces[b].manual_uvs().is_some() || axis(a).dot(axis(b)) > 0.999
    };
    let mut out = BTreeSet::from([seed]);
    let mut queue = vec![seed];
    while let Some(fi) = queue.pop() {
        for (ci, v) in m.faces[fi].verts.iter().enumerate() {
            let uv = shown[fi][ci];
            for (nf, nuv) in by_vertex.get(v).map(|x| x.as_slice()).unwrap_or(&[]) {
                if !out.contains(nf) && nuv.distance(uv) <= eps && same_chart(fi, *nf) {
                    out.insert(*nf);
                    queue.push(*nf);
                }
            }
        }
    }
    out
}

/// Ctrl+Shift+click in the UV editor (port of `copy_layout_to` = `MeshUV::copy_face` +
/// `paste`): the target face gets the source face's shown UVs, corner for corner in loop order.
pub fn copy_layout(m: &mut BMesh, source: usize, target: usize, a: &AutoUv) -> OpResult<()> {
    if source >= m.faces.len() || target >= m.faces.len() {
        return Err("Select a face first.".into());
    }
    let coords = face_corner_uvs(m, source, a);
    if m.faces[target].verts.len() != coords.len() {
        return Err("UV layouts require faces with the same vertex count.".into());
    }
    copy_uvs(m, &coords, &BTreeSet::from([target]));
    Ok(())
}

/// Ctrl+click in the UV editor (port of `MeshUV::stitch`): moves face `target`'s UVs so its
/// edge shared with `source` lands on the source edge's UVs (rotated and scaled to match,
/// mirrored if it would fold over the source). Both faces become manual.
pub fn stitch(m: &mut BMesh, source: usize, target: usize, a: &AutoUv) -> OpResult<()> {
    let n = m.faces.len();
    if source >= n || target >= n || source == target {
        return Err("Select a face first.".into());
    }
    // A directed source edge (a, b) whose reverse (b, a) is a target edge.
    let sf = &m.faces[source];
    let tf = &m.faces[target];
    let shared = (0..sf.verts.len()).find_map(|si| {
        let (va, vb) = (sf.verts[si], sf.verts[(si + 1) % sf.verts.len()]);
        let ti = (0..tf.verts.len()).find(|ti| tf.verts[*ti] == vb && tf.verts[(ti + 1) % tf.verts.len()] == va)?;
        Some((si, (si + 1) % sf.verts.len(), (ti + 1) % tf.verts.len(), ti))
    });
    let Some((s_start, s_end, t_start, t_end)) = shared else {
        return Err("Stitch requires two edge-adjacent faces with consistent winding.".into());
    };
    let mut work = m.clone();
    freeze_manual_uvs(&mut work, &BTreeSet::from([source, target]), a);
    let zero = |w: &BMesh, f: usize, i: usize, j: usize| w.faces[f].uv_coords[i].distance(w.faces[f].uv_coords[j]).powi(2) < 1e-8;
    if zero(&work, target, t_start, t_end) {
        planar_project(&mut work, &BTreeSet::from([target]), false);
    }
    let zero_len = "Cannot stitch an edge with zero UV length.";
    if zero(&work, target, t_start, t_end) || zero(&work, source, s_start, s_end) {
        return Err(zero_len.into());
    }
    let (su, tu) = (work.faces[source].uv_coords.clone(), work.faces[target].uv_coords.clone());
    let (ta, sa) = (tu[t_start], su[s_start]);
    let te = tu[t_end] - ta;
    let se = su[s_end] - sa;
    let angle = se.y.atan2(se.x) - te.y.atan2(te.x);
    let (sin, cos) = angle.sin_cos();
    let ratio = se.length() / te.length();
    let mut moved: Vec<Vec2> = tu
        .iter()
        .map(|uv| {
            let d = (*uv - ta) * ratio;
            v2(cos * d.x - d.y * sin, d.x * sin + cos * d.y) + sa
        })
        .collect();
    let avg = |c: &[Vec2]| c.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / c.len().max(1) as f32;
    let side = |p: Vec2| se.x * (p.y - sa.y) - se.y * (p.x - sa.x);
    if side(avg(&moved)) * side(avg(&su)) > 0.0 {
        let axis = se.normalized();
        for p in &mut moved {
            let rel = *p - sa;
            let along = axis.dot(rel);
            *p = axis * (2.0 * along) + sa - rel;
        }
    }
    moved[t_start] = su[s_start];
    moved[t_end] = su[s_end];
    work.faces[target].uv_coords = moved;
    work.faces[target].uv_manual = true;
    work.validate()?;
    *m = work;
    Ok(())
}

// ================================================================== UV template

/// Options of the UV template image (Xogot UV editor *Save Template*).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemplateOptions {
    /// Square image side, 64..=8192 px.
    pub size: u32,
    /// Wireframe colour (RGBA 0..1).
    pub line: [f32; 4],
    /// Background colour, `None` for transparent.
    pub background: Option<[f32; 4]>,
    /// Draw the 1/8 grid (grey, 30 % alpha).
    pub grid: bool,
    /// Shift each face by whole tiles so its UV bounds start inside the 0–1 square.
    pub wrap: bool,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        TemplateOptions { size: 1024, line: [0.0, 0.0, 0.0, 1.0], background: Some([1.0; 4]), grid: true, wrap: true }
    }
}

/// `color` composited over an RGBA8 pixel ("over"), back to RGBA8 (port of `composite`).
fn composite(color: [f32; 4], below: [u8; 4]) -> [u8; 4] {
    let below_alpha = f32::from(below[3]) / 255.0;
    let remaining = 1.0 - color[3];
    let alpha = remaining * below_alpha + color[3];
    if alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    let to_byte = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    let ch = |i: usize| to_byte((color[i] * color[3] + f32::from(below[i]) / 255.0 * below_alpha * remaining) / alpha * 255.0);
    [ch(0), ch(1), ch(2), to_byte(alpha * 255.0)]
}

/// The x values with `low <= k * x + c <= high` (port of `linear_range`).
fn linear_range(k: f32, c: f32, low: f32, high: f32) -> Option<(f32, f32)> {
    if k == 0.0 {
        return (low <= c && c <= high).then_some((f32::NEG_INFINITY, f32::INFINITY));
    }
    let (a, b) = ((low - c) / k, (high - c) / k);
    Some((a.min(b), a.max(b)))
}

/// The x range covered on row `y` by segment `p0`–`p1` thickened to `radius` (port of
/// `capsule_span`).
fn capsule_span(p0: Vec2, p1: Vec2, radius: f32, y: f32) -> Option<(f32, f32)> {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    for p in [p0, p1] {
        let dy = y - p.y;
        let h = radius * radius - dy * dy;
        if h >= 0.0 {
            let s = h.sqrt();
            left = left.min(p.x - s);
            right = right.max(p.x + s);
        }
    }
    let d = p1 - p0;
    let l2 = d.length_squared();
    if l2 > 0.0 {
        let l = l2.sqrt();
        let dy = y - p0.y;
        let band = linear_range(-d.y, d.x * dy + d.y * p0.x, -radius * l, radius * l);
        let along = linear_range(d.x, d.y * dy - d.x * p0.x, 0.0, l2);
        if let (Some(b), Some(al)) = (band, along) {
            let (lo, hi) = (b.0.max(al.0), b.1.min(al.1));
            if lo <= hi {
                left = left.min(lo);
                right = right.max(hi);
            }
        }
    }
    (left <= right).then_some((left, right))
}

/// Port of `UVTemplateRaster`.
struct Raster {
    size: usize,
    rgba: Vec<u8>,
}

impl Raster {
    fn blend(&mut self, index: usize, color: [f32; 4]) {
        let o = index * 4;
        let below = [self.rgba[o], self.rgba[o + 1], self.rgba[o + 2], self.rgba[o + 3]];
        self.rgba[o..o + 4].copy_from_slice(&composite(color, below));
    }

    /// Draws UV segments as round-capped lines `width` px wide, each pixel once per call
    /// (port of `UVTemplateRaster::stroke`; row = `v * size`).
    fn stroke(&mut self, segments: &[(Vec2, Vec2)], width: f32, color: [f32; 4]) {
        let size = self.size;
        let n = size as f32;
        let radius = (width * 0.5).max(0.5);
        let mut rows: Vec<Vec<(usize, usize)>> = vec![Vec::new(); size];
        for &(a, b) in segments {
            let (p0, p1) = (a * n, b * n);
            let top = p0.y.min(p1.y) - radius;
            let bottom = p0.y.max(p1.y) + radius;
            let first = (top - 0.5).ceil().max(0.0);
            let last = (bottom - 0.5).floor().min(n - 1.0);
            if !(first <= last) || !first.is_finite() || !last.is_finite() {
                continue;
            }
            for row in first as usize..=last as usize {
                let y = row as f32 + 0.5;
                if let Some((l, r)) = capsule_span(p0, p1, radius, y) {
                    let start = (l - 0.5).ceil().max(0.0);
                    let end = (r - 0.5).floor().min(n - 1.0);
                    if start <= end && start.is_finite() && end.is_finite() {
                        rows[row].push((start as usize, end as usize));
                    }
                }
            }
        }
        for (row, spans) in rows.iter_mut().enumerate() {
            spans.sort();
            let mut next = 0usize;
            for &(start, end) in spans.iter() {
                if end < next {
                    continue;
                }
                for x in start.max(next)..=end {
                    self.blend(row * size + x, color);
                }
                next = end + 1;
            }
        }
    }
}

/// Rasterises the faces' shown UVs as a square RGBA8 image, rows top (v = 0) to bottom, for a
/// PNG UV template (port of `template_pixels`): background, optional 1/8 grid, then the face
/// outlines `max(1, size / 1024)` px wide.
pub fn template_rgba8(m: &BMesh, faces: &BTreeSet<usize>, a: &AutoUv, o: &TemplateOptions) -> OpResult<Vec<u8>> {
    let fs = valid(m, faces);
    if fs.is_empty() {
        return Err("There is no UV layout to save.".into());
    }
    if !(64..=8192).contains(&o.size) {
        return Err("The template size must be from 64 through 8192 pixels.".into());
    }
    let unit = |c: &f32| c.is_finite() && (0.0..=1.0).contains(c);
    if !o.line.iter().chain(o.background.iter().flatten()).all(unit) {
        return Err("Template colors must be between 0 and 1.".into());
    }
    let size = o.size as usize;
    let mut r = Raster { size, rgba: vec![0; size * size * 4] };
    if let Some(bg) = o.background {
        let px = composite(bg, [0; 4]);
        r.rgba.chunks_exact_mut(4).for_each(|p| p.copy_from_slice(&px));
    }
    if o.grid {
        let mut grid = Vec::new();
        for step in 0..=8 {
            let t = step as f32 * 0.125;
            grid.push((v2(t, 0.0), v2(t, 1.0)));
            grid.push((v2(0.0, t), v2(1.0, t)));
        }
        r.stroke(&grid, 1.0, [0.5, 0.5, 0.5, 0.3]);
    }
    let mut edges = Vec::new();
    for f in fs {
        let mut uvs = face_corner_uvs(m, f, a);
        if o.wrap {
            if let Some(b) = Rect::of(uvs.iter().copied()) {
                let shift = v2(b.min.x.floor(), b.min.y.floor());
                uvs.iter_mut().for_each(|u| *u = *u - shift);
            }
        }
        for i in 0..uvs.len() {
            edges.push((uvs[i], uvs[(i + 1) % uvs.len()]));
        }
    }
    r.stroke(&edges, (size as f32 / 1024.0).max(1.0), o.line);
    Ok(r.rgba)
}

#[cfg(test)]
mod tests {
    use super::super::model::Face;
    use super::super::shapes::cuboid;
    use super::super::uv_edit::island_faces;
    use super::*;

    fn all(m: &BMesh) -> BTreeSet<usize> {
        (0..m.faces.len()).collect()
    }

    /// Two quads side by side on the floor (y = 0), sharing the edge x = 1, CCW from +Y.
    fn strip() -> BMesh {
        let mut m = BMesh::default();
        for p in [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(0.0, 0.0, -1.0), v3(1.0, 0.0, -1.0), v3(2.0, 0.0, -1.0)] {
            m.add_vert(p);
        }
        m.add_face(vec![0, 1, 4, 3]);
        m.add_face(vec![1, 2, 5, 4]);
        m.validate().unwrap();
        m
    }

    #[test]
    fn spherical_wraps_and_orients() {
        let mut m = cuboid(v3(2.0, 2.0, 2.0));
        m.translate(v3(5.0, 5.0, 5.0));
        let fs = all(&m);
        spherical_project(&mut m, &fs).unwrap();
        for (fi, f) in m.faces.iter().enumerate() {
            let c = f.manual_uvs().unwrap();
            assert!(c.iter().all(|u| (0.0..=1.0).contains(&u.y)));
            if m.face_normal(fi).y.abs() > 0.5 {
                continue; // pole faces span every longitude (as in Xogot)
            }
            let lo = c.iter().fold(9.0f32, |a, u| a.min(u.x));
            let hi = c.iter().fold(-9.0f32, |a, u| a.max(u.x));
            assert!(hi - lo <= 0.5 + 1e-5, "seam unwrapped: {c:?}");
        }
        // The top face (+Y) sits at the north pole: small V (texture top).
        let top = (0..m.faces.len()).find(|f| m.face_normal(*f).y > 0.9).unwrap();
        assert!(m.faces[top].uv_coords.iter().all(|u| u.y < 0.5));
        // U runs to the viewer's right: on the +Z side, larger X has larger U.
        let front = (0..m.faces.len()).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        let f = &m.faces[front];
        let (i, j) = (0..4).flat_map(|i| (0..4).map(move |j| (i, j))).find(|(i, j)| m.positions[f.verts[*i] as usize].x > m.positions[f.verts[*j] as usize].x + 0.5).unwrap();
        assert!(f.uv_coords[i].x > f.uv_coords[j].x);
        // A vertex on the centre is refused and leaves the mesh untouched.
        let mut t = BMesh::default();
        for p in [v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0)] {
            t.add_vert(p);
        }
        t.add_face(vec![0, 1, 2]);
        let before = t.clone();
        assert!(spherical_project(&mut t, &BTreeSet::from([0])).is_err());
        assert_eq!(t, before);
    }

    #[test]
    fn auto_tile_flips_stay_automatic() {
        let mut m = strip();
        m.faces[0].uv_manual = true;
        m.faces[0].uv_coords = vec![Vec2::ZERO; 4];
        m.faces[0].uv.scale = v2(2.0, 1.0);
        let s = AutoSettings { flip_u: true, ..Default::default() };
        assert_eq!(apply_auto_settings(&mut m, &BTreeSet::from([0]), &s, &AutoUv::default()), 0);
        assert!(!m.faces[0].uv_manual && m.faces[0].uv_coords.is_empty());
        assert_eq!(m.faces[0].uv.scale, v2(-2.0, 1.0));
        let s = AutoSettings::default();
        apply_auto_settings(&mut m, &BTreeSet::from([0]), &s, &AutoUv::default());
        assert_eq!(m.faces[0].uv.scale, v2(2.0, 1.0));
    }

    #[test]
    fn auto_flips_are_not_applied_twice() {
        let a = AutoUv::default();
        let one = BTreeSet::from([0]);
        let fit = |flip_u| AutoSettings { fill: Fill::Fit, flip_u, ..Default::default() };
        let mut plain = strip();
        apply_auto_settings(&mut plain, &one, &fit(false), &a);
        let mut flipped = strip();
        apply_auto_settings(&mut flipped, &one, &fit(true), &a);
        // Tile + flip first, then Fit + flip: still flipped once.
        let mut chained = strip();
        apply_auto_settings(&mut chained, &one, &AutoSettings { flip_u: true, ..Default::default() }, &a);
        apply_auto_settings(&mut chained, &one, &fit(true), &a);
        assert_eq!(chained.faces[0].uv_coords, flipped.faces[0].uv_coords);
        assert_ne!(flipped.faces[0].uv_coords, plain.faces[0].uv_coords);
        assert!(chained.faces[0].uv.scale.x < 0.0, "flip state readable from the sign");
        // Then Fit without flip: unflipped.
        apply_auto_settings(&mut chained, &one, &fit(false), &a);
        assert_eq!(chained.faces[0].uv_coords, plain.faces[0].uv_coords);
    }

    #[test]
    fn grouped_faces_share_the_first_faces_params() {
        let a = AutoUv::default();
        let mut m = strip();
        m.faces[0].uv.offset = v2(0.25, 0.0);
        m.faces[1].uv.offset = v2(0.7, 0.3);
        let both = all(&m);
        apply_auto_settings(&mut m, &both, &AutoSettings { anchor: Anchor::UpperLeft, group: true, ..Default::default() }, &a);
        // Shared verts 1 and 4: face 0 corners 1, 2 = face 1 corners 0, 3.
        let (f0, f1) = (&m.faces[0].uv_coords, &m.faces[1].uv_coords);
        assert!(f0[1].distance(f1[0]) < 1e-5 && f0[2].distance(f1[3]) < 1e-5);
    }

    #[test]
    fn auto_fit_stretch_and_anchor() {
        let a = AutoUv::default();
        let mut m = strip();
        m.positions[2].x = 4.0; // face 1 is 3 × 1
        m.positions[5].x = 4.0;
        let one = BTreeSet::from([1]);
        let bounds = |m: &BMesh, f: usize| Rect::of(m.faces[f].uv_coords.iter().copied()).unwrap();
        apply_auto_settings(&mut m, &one, &AutoSettings { fill: Fill::Stretch, ..Default::default() }, &a);
        let r = bounds(&m, 1);
        assert!(r.min.distance(Vec2::ZERO) < 1e-5 && r.max.distance(v2(1.0, 1.0)) < 1e-5, "{r:?}");
        apply_auto_settings(&mut m, &one, &AutoSettings { fill: Fill::Fit, ..Default::default() }, &a);
        let r = bounds(&m, 1);
        assert!((r.size().x - 1.0).abs() < 1e-5 && (r.size().y - 1.0 / 3.0).abs() < 1e-5, "{r:?}");
        // Lower right: the bounds' bottom-right corner sits on (1, 1).
        apply_auto_settings(&mut m, &one, &AutoSettings { fill: Fill::Fit, anchor: Anchor::LowerRight, ..Default::default() }, &a);
        let r = bounds(&m, 1);
        assert!(r.max.distance(v2(1.0, 1.0)) < 1e-5, "{r:?}");
        // Upper centre: top edge on v = 0, centred in u.
        apply_auto_settings(&mut m, &one, &AutoSettings { fill: Fill::Stretch, anchor: Anchor::UpperCenter, ..Default::default() }, &a);
        let r = bounds(&m, 1);
        assert!(r.min.y.abs() < 1e-5 && (r.center().x - 0.5).abs() < 1e-5, "{r:?}");
        // Swap: a fitted 3 × 1 face becomes 1/3 × 1.
        apply_auto_settings(&mut m, &one, &AutoSettings { fill: Fill::Fit, swap_uv: true, ..Default::default() }, &a);
        let r = bounds(&m, 1);
        assert!((r.size().y - 1.0).abs() < 1e-5 && (r.size().x - 1.0 / 3.0).abs() < 1e-5, "{r:?}");
        // Grouped stretch spans both faces as one batch.
        let both = all(&m);
        assert_eq!(apply_auto_settings(&mut m, &both, &AutoSettings { fill: Fill::Stretch, group: true, ..Default::default() }, &a), 2);
        let r = Rect::of(m.faces.iter().flat_map(|f| f.uv_coords.iter().copied())).unwrap();
        assert!(r.min.distance(Vec2::ZERO) < 1e-5 && r.max.distance(v2(1.0, 1.0)) < 1e-5);
        assert!((bounds(&m, 0).size().x - 0.25).abs() < 1e-5);
        m.validate().unwrap();
    }

    #[test]
    fn island_follows_automatic_faces() {
        let a = AutoUv::default();
        let mut m = strip();
        assert_eq!(island_faces(&m, 0, 1e-4), BTreeSet::from([0]), "uv_edit only follows manual faces");
        assert_eq!(uv_island(&m, 0, &a, 1e-4), BTreeSet::from([0, 1]));
        m.faces[1].uv.offset = v2(0.5, 0.0);
        assert_eq!(uv_island(&m, 0, &a, 1e-4), BTreeSet::from([0]));
        assert!(uv_island(&m, 7, &a, 1e-4).is_empty());
        // A box corner at the origin: perpendicular automatic faces meet at UV (0, 0) there but
        // are separate charts.
        let c = cuboid(v3(1.0, 1.0, 1.0));
        let mut c2 = c.clone();
        let lo = c.bounds().unwrap().0;
        c2.translate(lo * -1.0);
        assert_eq!(uv_island(&c2, 0, &a, 1e-4), BTreeSet::from([0]));
    }

    #[test]
    fn stitch_joins_the_shared_edge() {
        let a = AutoUv::default();
        let mut m = strip();
        // Scramble face 1: shifted, rotated, scaled.
        m.faces[1].uv_manual = true;
        m.faces[1].uv_coords = vec![v2(10.0, 10.0), v2(10.0, 12.0), v2(8.0, 12.0), v2(8.0, 10.0)];
        stitch(&mut m, 0, 1, &a).unwrap();
        let s = face_corner_uvs(&m, 0, &a);
        let t = &m.faces[1].uv_coords;
        // Shared verts 1 and 4: source corners 1, 2; target corners 0, 3.
        assert!(t[0].distance(s[1]) < 1e-5 && t[3].distance(s[2]) < 1e-5);
        // Same size as the source edge and on the other side of it (no fold-over).
        let side = |p: Vec2| (s[2] - s[1]).x * (p.y - s[1].y) - (s[2] - s[1]).y * (p.x - s[1].x);
        let ct = t.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / 4.0;
        let cs = s.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / 4.0;
        assert!(side(ct) * side(cs) < 0.0);
        assert!(m.faces[0].uv_manual && m.faces[1].uv_manual);
        // Not adjacent / same face.
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let opposite = (1..6).find(|f| c.face_normal(*f).dot(c.face_normal(0)) < -0.9).unwrap();
        assert!(stitch(&mut c, 0, opposite, &a).is_err());
        assert!(stitch(&mut c, 0, 0, &a).is_err());
    }

    #[test]
    fn copy_layout_needs_matching_corners() {
        let a = AutoUv::default();
        let mut m = strip();
        copy_layout(&mut m, 0, 1, &a).unwrap();
        assert_eq!(m.faces[1].uv_coords, face_corner_uvs(&m, 0, &a));
        let v = m.add_vert(v3(0.0, 0.0, 1.0));
        m.faces.push(Face::new(vec![1, 0, v]));
        assert!(copy_layout(&mut m, 0, 2, &a).is_err());
    }

    #[test]
    fn template_draws_outlines_top_down() {
        let a = AutoUv::default();
        let mut m = strip();
        m.faces[0].uv_manual = true;
        // A small triangle-ish quad near the top-left (v small = image top).
        m.faces[0].uv_coords = vec![v2(0.1, 0.1), v2(0.4, 0.1), v2(0.4, 0.2), v2(0.1, 0.2)];
        let o = TemplateOptions { size: 64, grid: false, wrap: false, ..Default::default() };
        let px = template_rgba8(&m, &BTreeSet::from([0]), &a, &o).unwrap();
        assert_eq!(px.len(), 64 * 64 * 4);
        let at = |x: usize, y: usize| &px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(16, 6), &[0, 0, 0, 255], "top edge v = 0.1 → row 6");
        assert_eq!(at(16, 57), &[255, 255, 255, 255], "nothing mirrored at the bottom");
        assert_eq!(at(60, 60), &[255, 255, 255, 255]);
        // Transparent background with grid.
        let o = TemplateOptions { size: 64, background: None, grid: true, wrap: false, ..Default::default() };
        let px = template_rgba8(&m, &BTreeSet::from([0]), &a, &o).unwrap();
        assert_eq!(px[(36 * 64 + 36) * 4 + 3], 0);
        assert!(px[(36 * 64 + 32) * 4 + 3] > 0, "grid line at u = 0.5");
        // Wrap brings far-away automatic UVs into the image.
        let mut far = strip();
        far.translate(v3(100.25, 0.0, -50.0));
        let o = TemplateOptions { size: 64, grid: false, wrap: true, ..Default::default() };
        let px = template_rgba8(&far, &BTreeSet::from([0]), &a, &o).unwrap();
        assert!(px.chunks(4).any(|p| p[0] == 0));
        // Validation.
        assert!(template_rgba8(&m, &BTreeSet::new(), &a, &o).is_err());
        assert!(template_rgba8(&m, &BTreeSet::from([0]), &a, &TemplateOptions { size: 32, ..o }).is_err());
        assert!(template_rgba8(&m, &BTreeSet::from([0]), &a, &TemplateOptions { line: [2.0, 0.0, 0.0, 1.0], ..o }).is_err());
    }
}
