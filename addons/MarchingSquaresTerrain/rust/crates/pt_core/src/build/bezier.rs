//! Bezier Plane / Bezier Block (Xogot Modeler, docs/port/06-xogot.md package P2).
//!
//! Port of xogot-rust `modeler_kit/src/bezier_mesh.rs` (MIT, Xibbon and the Xogot Modeler
//! Rust port contributors): knots with tangent offsets and roll, the recipe, cubic evaluation,
//! knot appending / de Casteljau insertion, and the generator that sweeps a ribbon along the
//! curve with a rotation-minimising frame (optionally extruded into a block).
//!
//! Differences from the source, for the builder's shared-vertex `BMesh`:
//! - knots have no UUIDs (indices are their identity here) and the recipe no
//!   `generated_version`; it is stored on the Godot side as `encode()` bytes;
//! - the ribbon shares its vertices (a closed loop reuses the first row) and is oriented as a
//!   whole towards the extrusion side (up for a plane), so a plane is visible from above and a
//!   block stays a consistent closed solid even when the roll turns the ribbon over;
//! - UVs are the source's manual per-corner UVs (`uv_manual`), U across 0..1 and V along the
//!   arc length; the vertex budget is a vertex count (65 535) instead of a record count;
//! - `nearest_point` / `nearest_to_ray` (sampled, 32 samples per segment) back click-to-insert.

use super::model::{BMesh, Face};
use crate::math::*;

/// Samples per segment for `nearest_point` / `nearest_to_ray`.
pub const PICK_SAMPLES: u32 = 32;
/// Picked fractions stay this far inside a segment: a knot inserted right next to another
/// (zero tangents make the curve crawl near knots) would give zero-length samples and the
/// mesh would no longer generate.
pub const PICK_MARGIN: f32 = 0.05;
/// Most vertices a generated Bezier mesh may have.
pub const MAX_VERTS: usize = 0xffff;

/// How a knot's two tangents are tied together (`BezierTangentMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TangentMode {
    /// Independent tangents.
    #[default]
    Free = 0,
    /// Opposite directions, independent lengths.
    Aligned = 1,
    /// Opposite directions, equal lengths.
    Mirrored = 2,
}

impl TangentMode {
    pub const ALL: [TangentMode; 3] = [TangentMode::Free, TangentMode::Aligned, TangentMode::Mirrored];
    pub fn from_u8(v: u8) -> Option<TangentMode> {
        Self::ALL.get(v as usize).copied()
    }
    pub fn name(self) -> &'static str {
        match self {
            TangentMode::Free => "Free",
            TangentMode::Aligned => "Aligned",
            TangentMode::Mirrored => "Mirrored",
        }
    }
}

/// One control point. Tangents are offsets from `position` (`BezierKnot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knot {
    pub position: Vec3,
    pub tangent_in: Vec3,
    pub tangent_out: Vec3,
    /// Roll of the ribbon around the curve at this knot, in degrees.
    pub roll: f32,
    pub mode: TangentMode,
}

impl Knot {
    pub fn new(position: Vec3, tangent_in: Vec3, tangent_out: Vec3) -> Knot {
        Knot { position, tangent_in, tangent_out, roll: 0.0, mode: TangentMode::Free }
    }

    /// Port of `BezierKnot::set_tangent`: sets the incoming or outgoing tangent and updates the
    /// other for the knot's mode (aligned keeps the other's length but points it opposite,
    /// mirrored negates it).
    pub fn set_tangent(&mut self, value: Vec3, incoming: bool) {
        let other = if incoming { self.tangent_out } else { self.tangent_in };
        if incoming {
            self.tangent_in = value;
        } else {
            self.tangent_out = value;
        }
        let opposite = match self.mode {
            TangentMode::Free => return,
            TangentMode::Aligned => {
                let l = value.length();
                if l * l > 1e-12 { value * (-other.length() / l) } else { Vec3::ZERO }
            }
            TangentMode::Mirrored => -value,
        };
        if incoming {
            self.tangent_out = opposite;
        } else {
            self.tangent_in = opposite;
        }
    }

    /// Changes the mode and applies it to the incoming tangent from the outgoing one (Xogot
    /// applies a mode on the next tangent edit; this makes the switch visible at once).
    pub fn set_mode(&mut self, mode: TangentMode) {
        self.mode = mode;
        let out = self.tangent_out;
        self.set_tangent(out, false);
    }
}

/// The Bezier Plane / Block source of a mesh (`BezierRecipe`).
#[derive(Clone, Debug, PartialEq)]
pub struct BezierRecipe {
    pub knots: Vec<Knot>,
    pub closed: bool,
    /// Ribbon faces share smoothing group 1.
    pub smooth: bool,
    /// Half the ribbon width (the UI shows Width = 2 × radius).
    pub radius: f32,
    /// Extrusion height (0: a flat plane, otherwise a block).
    pub height: f32,
    /// Samples per segment (1..=128).
    pub rows: u32,
    /// Quads across the ribbon (1..=128).
    pub columns: u32,
}

impl Default for BezierRecipe {
    /// Xogot's tool defaults (`ToolState::new`).
    fn default() -> Self {
        BezierRecipe { knots: Vec::new(), closed: false, smooth: true, radius: 0.1, height: 0.0, rows: 8, columns: 1 }
    }
}

/// Where a pick landed on the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveHit {
    /// Segment index (from knot `segment` to the next, wrapping when closed).
    pub segment: usize,
    /// Fraction along the segment, within `PICK_MARGIN..=1 - PICK_MARGIN` so `insert_knot`
    /// accepts it and the result still generates.
    pub fraction: f32,
    pub point: Vec3,
    /// Distance from the query point (or ray).
    pub distance: f32,
}

/// `p` rotated by `angle` radians about the unit `axis` (Rodrigues; `Quatf::from_angle_axis`).
fn rotate(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

/// `v` turned by the minimal rotation taking direction `from` to `to` (the source's
/// `quat_from_to`, including its half-turn about an axis orthogonal to `from` for opposite
/// directions).
fn rotate_from_to(from: Vec3, to: Vec3, v: Vec3) -> Vec3 {
    let (a, b) = (from.normalized(), to.normalized());
    let axis = a.cross(b);
    let (sin, cos) = (axis.length(), a.dot(b));
    if sin < 1e-7 {
        if cos > 0.0 {
            return v;
        }
        let m = v3(a.x.abs(), a.y.abs(), a.z.abs());
        let other = if m.y <= m.z {
            if m.y < m.x { v3(0.0, 1.0, 0.0) } else { v3(1.0, 0.0, 0.0) }
        } else if m.x <= m.z {
            v3(1.0, 0.0, 0.0)
        } else {
            v3(0.0, 0.0, 1.0)
        };
        return rotate(v, a.cross(other).normalized(), std::f32::consts::PI);
    }
    rotate(v, axis / sin, sin.atan2(cos))
}

/// `v / |v|`, or zero for (near) zero vectors (`normalized` in the source).
fn safe_normalized(v: Vec3) -> Vec3 {
    let l = v.length();
    if l > 1e-10 { v / l } else { Vec3::ZERO }
}

/// Port of `BezierMesh::point`: the cubic from knot `a` to knot `b` at `t`.
pub fn point(a: &Knot, b: &Knot, t: f32) -> Vec3 {
    let u = 1.0 - t;
    let p1 = a.position + a.tangent_out;
    let p2 = b.position + b.tangent_in;
    a.position * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + b.position * (t * t * t)
}

/// Port of `BezierMesh::tangent`: the unit direction at `t`, or the chord direction where the
/// derivative vanishes (zero for coincident knots).
pub fn tangent(a: &Knot, b: &Knot, t: f32) -> Vec3 {
    let u = 1.0 - t;
    let middle = b.position + b.tangent_in - a.position - a.tangent_out;
    let d = middle * (6.0 * u * t) + a.tangent_out * (3.0 * u * u) - b.tangent_in * (3.0 * t * t);
    if d.dot(d) <= 1e-12 { safe_normalized(b.position - a.position) } else { d.normalized() }
}

impl BezierRecipe {
    /// Width of the ribbon (`2 × radius`), as the UI shows it.
    pub fn width(&self) -> f32 {
        self.radius * 2.0
    }
    pub fn set_width(&mut self, w: f32) {
        self.radius = w * 0.5;
    }

    /// Number of curve segments (knots − 1, or knots when closed).
    pub fn segments(&self) -> usize {
        if self.knots.is_empty() { 0 } else { self.knots.len() - (!self.closed) as usize }
    }

    fn ends(&self, segment: usize) -> (&Knot, &Knot) {
        (&self.knots[segment], &self.knots[(segment + 1) % self.knots.len()])
    }

    /// The curve point on `segment` at `t`.
    pub fn eval(&self, segment: usize, t: f32) -> Vec3 {
        let (a, b) = self.ends(segment);
        point(a, b, t)
    }

    /// Port of `BezierMesh::validate`: 2..=256 knots (3..=256 closed), rows / columns 1..=128,
    /// radius ≥ 1e-4, all values finite.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.knots.len();
        let count_ok = if self.closed { (3..=256).contains(&n) } else { (2..=256).contains(&n) };
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        let knots_ok = self.knots.iter().all(|k| finite(k.position) && finite(k.tangent_in) && finite(k.tangent_out) && k.roll.is_finite());
        if count_ok
            && (1..=128).contains(&self.rows)
            && (1..=128).contains(&self.columns)
            && self.radius.is_finite()
            && self.radius >= 1e-4
            && self.height.is_finite()
            && knots_ok
        {
            Ok(())
        } else {
            Err("Bezier settings are outside their valid range.".into())
        }
    }

    /// Port of `BezierRecipe::append_knot`: appends a free knot at `position` whose tangents
    /// point a third of the way back to / on from the previous knot. With `smooth_previous`,
    /// the previous knot's tangents follow the bisector of its two chords (a lone first knot
    /// just aims its outgoing tangent at the new knot).
    pub fn append_knot(&mut self, position: Vec3, smooth_previous: bool) {
        let Some(last) = self.knots.len().checked_sub(1) else {
            self.knots.push(Knot::new(position, v3(0.0, 0.0, -0.5), v3(0.0, 0.0, 0.5)));
            return;
        };
        let chord = position - self.knots[last].position;
        let t = chord / 3.0;
        if smooth_previous {
            if last == 0 {
                self.knots[0].tangent_out = t;
            } else {
                self.smooth_between(last, last - 1, position);
            }
        }
        self.knots.push(Knot::new(position, -t, t));
    }

    /// Smooths knot `index` along the bisector of its chords to its neighbours (wrapping when
    /// closed; an end knot of an open curve is left as it is). Used when closing a loop.
    pub fn smooth_knot(&mut self, index: usize) {
        let n = self.knots.len();
        if index >= n || n < 2 || (!self.closed && (index == 0 || index + 1 == n)) {
            return;
        }
        let (prev, next) = ((index + n - 1) % n, (index + 1) % n);
        let p = self.knots[next].position;
        self.smooth_between(index, prev, p);
    }

    /// Port of the source's `smooth_knot`: knot `index`'s tangents along the bisector of the
    /// chord from knot `prev` and the chord to `next_pos`, each a third of its chord.
    fn smooth_between(&mut self, index: usize, prev: usize, next_pos: Vec3) {
        let chord = next_pos - self.knots[index].position;
        let previous = self.knots[index].position - self.knots[prev].position;
        let (cl, pl) = (chord.length(), previous.length());
        if !(1e-6 < pl && 1e-6 < cl) {
            return;
        }
        let bisector = chord / cl + previous / pl;
        if bisector.dot(bisector) <= 1e-12 {
            return;
        }
        let dir = bisector.normalized();
        let k = &mut self.knots[index];
        k.tangent_in = -dir * (pl / 3.0);
        k.tangent_out = dir * (cl / 3.0);
    }

    /// Port of `BezierMesh::insert_knot`: splits `segment` at `fraction` (strictly inside 0..1)
    /// with de Casteljau's construction; the new free knot goes after the segment's first knot
    /// and the neighbours' facing tangents are shortened (and made free) so the curve keeps its
    /// shape. The roll is interpolated. Returns the new knot's index.
    pub fn insert_knot(&mut self, segment: usize, fraction: f32) -> Result<usize, String> {
        let count = self.knots.len();
        if count < 2 || segment >= self.segments() || !(0.0 < fraction && fraction < 1.0) {
            return Err("Pick a point between two knots.".into());
        }
        let t = fraction;
        let next = (segment + 1) % count;
        let (a, b) = (self.knots[segment], self.knots[next]);
        let p0 = a.position;
        let p1 = p0 + a.tangent_out;
        let p3 = b.position;
        let p2 = p3 + b.tangent_in;
        let (q0, q1, q2) = (p0.lerp(p1, t), p1.lerp(p2, t), p2.lerp(p3, t));
        let (r0, r1) = (q0.lerp(q1, t), q1.lerp(q2, t));
        let position = r0.lerp(r1, t);
        self.knots[segment].tangent_out = q0 - p0;
        self.knots[segment].mode = TangentMode::Free;
        self.knots[next].tangent_in = q2 - p3;
        self.knots[next].mode = TangentMode::Free;
        let mut k = Knot::new(position, r0 - position, r1 - position);
        k.roll = a.roll + (b.roll - a.roll) * t;
        self.knots.insert(segment + 1, k);
        Ok(segment + 1)
    }

    /// Removes knot `index` if the curve keeps enough knots (2 open, 3 closed).
    pub fn remove_knot(&mut self, index: usize) -> bool {
        let min = if self.closed { 3 } else { 2 };
        if index < self.knots.len() && self.knots.len() > min {
            self.knots.remove(index);
            true
        } else {
            false
        }
    }

    /// Nearest sampled curve point to `p` (`PICK_SAMPLES` per segment), for click-to-insert.
    pub fn nearest_point(&self, p: Vec3) -> Option<CurveHit> {
        self.nearest_by(|q| (q - p).length())
    }

    /// Nearest sampled curve point to the ray `origin + s·dir` (s ≥ 0), for click-to-insert
    /// from a camera ray. `distance` is the point's distance to the ray.
    pub fn nearest_to_ray(&self, origin: Vec3, dir: Vec3) -> Option<CurveHit> {
        let d = safe_normalized(dir);
        self.nearest_by(|q| {
            let s = (q - origin).dot(d).max(0.0);
            (q - (origin + d * s)).length()
        })
    }

    fn nearest_by(&self, dist: impl Fn(Vec3) -> f32) -> Option<CurveHit> {
        let mut best: Option<CurveHit> = None;
        for segment in 0..self.segments() {
            for i in 0..=PICK_SAMPLES {
                let fraction = (i as f32 / PICK_SAMPLES as f32).clamp(PICK_MARGIN, 1.0 - PICK_MARGIN);
                let point = self.eval(segment, fraction);
                let distance = dist(point);
                if best.is_none_or(|b| distance < b.distance) {
                    best = Some(CurveHit { segment, fraction, point, distance });
                }
            }
        }
        best
    }

    /// Port of `BezierMesh::generate`: `rows` samples per segment, a ribbon `2 × radius` wide
    /// and `columns` quads across, swept along the curve with a rotation-minimising frame
    /// (twist spread evenly over a closed loop) and rolled by the knots' roll. A non-zero
    /// `height` extrudes the ribbon (straight up, or down when negative) into a block with
    /// walls on its open edges.
    pub fn generate(&self) -> Result<BMesh, String> {
        self.validate()?;
        let segments = self.segments();
        let rows = self.rows as usize;
        let samples = segments * rows;
        let cols = self.columns as usize;
        let ring = if self.closed { samples } else { samples + 1 };
        let layers = if self.height == 0.0 { 1 } else { 2 };
        if ring * (cols + 1) * layers > MAX_VERTS {
            return Err("The Bezier mesh would have too many vertices; lower rows or columns.".into());
        }
        let path = Path::sample(self, segments)?;
        let mut m = BMesh::default();
        // Vertex (i, j): sample i (a closed loop wraps the last row onto the first), column j.
        for i in 0..ring {
            let across = rotate(path.normals[i], path.tangents[i], path.rolls[i]);
            for j in 0..=cols {
                let w = ((2 * j) as f32 / cols as f32 - 1.0) * self.radius;
                m.add_vert(path.centers[i] + across * w);
            }
        }
        let vid = |i: usize, j: usize| ((i % ring) * (cols + 1) + j) as u32;
        for i in 0..samples {
            for j in 0..cols {
                let (u0, u1) = (j as f32 / cols as f32, (j + 1) as f32 / cols as f32);
                let (near, far) = (path.distance[i], path.distance[i + 1]);
                let mut f = Face::new(vec![vid(i, j), vid(i + 1, j), vid(i + 1, j + 1), vid(i, j + 1)]);
                f.uv_manual = true;
                f.uv_coords = vec![v2(u0, near), v2(u0, far), v2(u1, far), v2(u1, near)];
                f.smooth = self.smooth as u16;
                m.faces.push(f);
            }
        }
        // Orient the whole ribbon towards the extrusion side (up for a plane).
        let side = if self.height < 0.0 { -Vec3::UP } else { Vec3::UP };
        let facing: f32 = (0..m.faces.len()).map(|f| super::model::newell(&m.face_points(f)).dot(side)).sum();
        if facing < 0.0 {
            for f in &mut m.faces {
                f.reverse();
            }
        }
        if self.height != 0.0 {
            extrude(&mut m, self.height);
        }
        m.validate()?;
        Ok(m)
    }

    /// Compact byte form for storage on the Godot side (`PackedByteArray`): magic `BZR`,
    /// format version, flags (bit 0 closed, bit 1 smooth), radius, height, rows, columns, knot
    /// count, then per knot position, tangent in, tangent out (3 × f32 each), roll (f32) and
    /// mode (u8). Little-endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(20 + self.knots.len() * 41);
        b.extend_from_slice(b"BZR");
        b.push(FORMAT_VERSION);
        b.push(self.closed as u8 | (self.smooth as u8) << 1);
        b.extend_from_slice(&self.radius.to_le_bytes());
        b.extend_from_slice(&self.height.to_le_bytes());
        b.extend_from_slice(&(self.rows.min(u16::MAX as u32) as u16).to_le_bytes());
        b.extend_from_slice(&(self.columns.min(u16::MAX as u32) as u16).to_le_bytes());
        b.extend_from_slice(&(self.knots.len().min(u16::MAX as usize) as u16).to_le_bytes());
        for k in self.knots.iter().take(u16::MAX as usize) {
            for v in [k.position, k.tangent_in, k.tangent_out] {
                for c in [v.x, v.y, v.z] {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            b.extend_from_slice(&k.roll.to_le_bytes());
            b.push(k.mode as u8);
        }
        b
    }

    /// Reads `encode()` bytes; `None` for other data, a newer format version or a truncated
    /// blob. (The recipe is not validated: `generate` reports bad values.)
    pub fn decode(bytes: &[u8]) -> Option<BezierRecipe> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(3)? != b"BZR" || r.u8()? != FORMAT_VERSION {
            return None;
        }
        let flags = r.u8()?;
        let radius = r.f32()?;
        let height = r.f32()?;
        let rows = r.u16()? as u32;
        let columns = r.u16()? as u32;
        let n = r.u16()? as usize;
        let mut knots = Vec::with_capacity(n);
        for _ in 0..n {
            let mut v = [Vec3::ZERO; 3];
            for slot in &mut v {
                *slot = v3(r.f32()?, r.f32()?, r.f32()?);
            }
            let roll = r.f32()?;
            let mode = TangentMode::from_u8(r.u8()?)?;
            knots.push(Knot { position: v[0], tangent_in: v[1], tangent_out: v[2], roll, mode });
        }
        Some(BezierRecipe { knots, closed: flags & 1 != 0, smooth: flags & 2 != 0, radius, height, rows, columns })
    }
}

const FORMAT_VERSION: u8 = 1;

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

/// The sampled curve (`segments × rows + 1` entries), port of the source's `Path`.
struct Path {
    centers: Vec<Vec3>,
    tangents: Vec<Vec3>,
    /// Knot roll at each sample, in radians.
    rolls: Vec<f32>,
    /// Unit normals of the rotation-minimising frame.
    normals: Vec<Vec3>,
    /// Arc length from the first sample.
    distance: Vec<f32>,
}

impl Path {
    /// `Path::sample` + `transport_normals` + `close_twist` (+ the distances of `strips`).
    fn sample(r: &BezierRecipe, segments: usize) -> Result<Path, String> {
        let rows = r.rows as usize;
        let samples = segments * rows;
        let mut p = Path { centers: Vec::new(), tangents: Vec::new(), rolls: Vec::new(), normals: Vec::new(), distance: Vec::new() };
        for i in 0..=samples {
            let segment = (i / rows).min(segments - 1);
            let (a, b) = r.ends(segment);
            if (a.position - b.position).dot(a.position - b.position) <= 1e-12 {
                return Err("Adjacent Bezier knots must have different positions.".into());
            }
            let t = (i - segment * rows) as f32 / rows as f32;
            p.centers.push(point(a, b, t));
            p.tangents.push(tangent(a, b, t));
            p.rolls.push((a.roll + t * (b.roll - a.roll)).to_radians());
        }
        // Start perpendicular to the first tangent and world up (world X when the tangent is
        // nearly vertical), then carry the normal along by the minimal rotation per step.
        let first = p.tangents[0];
        let up = if first.y.abs() < 0.9 { Vec3::UP } else { v3(1.0, 0.0, 0.0) };
        p.normals.push(up.cross(first).normalized());
        for i in 1..p.tangents.len() {
            let t = p.tangents[i];
            let rotated = rotate_from_to(p.tangents[i - 1], t, p.normals[i - 1]);
            let n = rotated - t * rotated.dot(t);
            let step = p.centers[i - 1] - p.centers[i];
            let finite = n.x.is_finite() && n.y.is_finite() && n.z.is_finite();
            if !finite || n.dot(n) <= 1e-12 || step.dot(step) <= 1e-14 {
                return Err("The Bezier path reverses or has a zero-length sample.".into());
            }
            p.normals.push(n.normalized());
        }
        if r.closed {
            // Spread the angle between the transported last normal and the first one evenly
            // over the loop so the ribbon meets itself without a seam twist.
            let last = p.normals.len() - 1;
            let (t0, n0) = (p.tangents[0], p.normals[0]);
            let moved = rotate_from_to(p.tangents[last], t0, p.normals[last]);
            let correction = moved.cross(n0).dot(t0).atan2(moved.dot(n0));
            for i in 0..=last {
                p.normals[i] = rotate(p.normals[i], p.tangents[i], i as f32 * correction / last as f32);
            }
        }
        p.distance.push(0.0);
        for i in 1..p.centers.len() {
            let d = p.distance[i - 1] + (p.centers[i] - p.centers[i - 1]).length();
            p.distance.push(d);
        }
        Ok(p)
    }
}

/// Port of the source's `extrude`: the oriented ribbon becomes the far cap (moved `height`
/// along Y), a reversed copy stays as the near cap, and every open ribbon edge gets a wall
/// (manual UVs 0..1 along, 0..|height| up).
fn extrude(m: &mut BMesh, height: f32) {
    let offset = v3(0.0, height, 0.0);
    let k = m.positions.len() as u32;
    let base = m.faces.clone();
    let positions = m.positions.clone();
    m.positions.extend(positions.iter().map(|p| *p + offset));
    let boundary: Vec<(u32, u32)> = {
        let all = (0..base.len()).collect();
        super::topo::region_boundary(m, &all)
    };
    m.faces.clear();
    for f in &base {
        let mut top = f.clone();
        top.verts = f.verts.iter().map(|v| v + k).collect();
        m.faces.push(top);
        let mut bottom = f.clone();
        bottom.reverse();
        m.faces.push(bottom);
    }
    let h = height.abs();
    for (a, b) in boundary {
        // The ribbon's edge a -> b is on the far cap as a+k -> b+k, so the wall runs b+k -> a+k
        // there and a -> b on the reversed near cap.
        let mut f = Face::new(vec![a, b, b + k, a + k]);
        f.uv_manual = true;
        f.uv_coords = vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, h), v2(0.0, h)];
        m.faces.push(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(points: &[Vec3]) -> BezierRecipe {
        let mut r = BezierRecipe { rows: 4, ..Default::default() };
        for p in points {
            r.append_knot(*p, true);
        }
        r
    }

    #[test]
    fn limits_are_checked() {
        let two = [v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0)];
        assert!(line(&two[..1]).generate().is_err());
        assert!(line(&two).generate().is_ok());
        let mut tri = line(&[v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(0.0, 0.0, 2.0)]);
        tri.closed = true;
        assert!(tri.generate().is_ok());
        tri.knots.truncate(2);
        assert!(tri.generate().is_err());
        let mut r = line(&two);
        for (rows, cols, ok) in [(0, 1, false), (129, 1, false), (1, 0, false), (1, 129, false), (128, 128, true)] {
            r.rows = rows;
            r.columns = cols;
            assert_eq!(r.generate().is_ok(), ok, "{rows}x{cols}");
        }
        r.rows = 2;
        r.columns = 1;
        r.radius = 1e-5;
        assert!(r.generate().is_err());
        r.radius = 0.1;
        r.height = f32::NAN;
        assert!(r.generate().is_err());
        // Coincident neighbours are rejected, not divided by zero.
        let mut same = line(&two);
        same.knots[1].position = same.knots[0].position;
        assert!(same.generate().is_err());
    }

    #[test]
    fn straight_plane_faces_up_with_arc_length_uvs() {
        let mut r = line(&[v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0)]);
        r.columns = 2;
        r.radius = 0.5;
        let m = r.generate().unwrap();
        m.validate().unwrap();
        assert_eq!(m.faces.len(), 4 * 2);
        assert_eq!(m.positions.len(), 5 * 3, "shared vertices");
        for f in 0..m.faces.len() {
            assert!(m.face_normal(f).y > 0.99, "face {f} not up");
            let uv = m.faces[f].manual_uvs().unwrap();
            assert!(uv.iter().all(|c| (0.0..=1.0).contains(&c.x) && (-1e-5..=4.0 + 1e-4).contains(&c.y)));
        }
        let total: f32 = (0..m.faces.len()).map(|f| m.face_area(f)).sum();
        assert!((total - 4.0).abs() < 1e-3, "{total}");
    }

    #[test]
    fn block_is_closed_even_when_the_roll_turns_over() {
        let mut r = line(&[v3(0.0, 0.0, 0.0), v3(3.0, 0.0, 1.0), v3(6.0, 0.0, 0.0)]);
        r.height = 0.5;
        r.radius = 0.4;
        r.columns = 2;
        let m = r.generate().unwrap();
        assert!(m.is_closed());
        assert!(m.volume() > 0.0);
        r.knots[2].roll = 180.0;
        let m = r.generate().unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        r.knots[2].roll = 0.0;
        r.height = -0.5;
        let m = r.generate().unwrap();
        assert!(m.is_closed() && m.volume() > 0.0, "{}", m.volume());
    }

    #[test]
    fn closed_loop_shares_its_seam() {
        let mut r = line(&[v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0), v3(4.0, 0.0, 4.0), v3(0.0, 0.0, 4.0)]);
        r.closed = true;
        for i in 0..4 {
            r.smooth_knot(i);
        }
        r.radius = 0.3;
        let flat = r.generate().unwrap();
        flat.validate().unwrap();
        assert_eq!(flat.positions.len(), 4 * 4 * 2, "the last row reuses the first");
        assert!(flat.edge_faces().values().all(|f| f.len() <= 2));
        r.height = 1.0;
        let block = r.generate().unwrap();
        assert!(block.is_closed() && block.volume() > 0.0);
        // The last row's V is the full loop length, not 0.
        let vmax = flat.faces.iter().flat_map(|f| f.uv_coords.iter().map(|c| c.y)).fold(0.0f32, f32::max);
        assert!(vmax > 12.0, "{vmax}");
    }

    #[test]
    fn insert_knot_keeps_the_curve() {
        let r = BezierRecipe {
            knots: vec![Knot::new(v3(0.0, 0.0, 0.0), Vec3::ZERO, v3(0.5, 0.0, 0.2)), Knot::new(v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 0.4), Vec3::ZERO)],
            ..Default::default()
        };
        let t = 0.3;
        let mut s = r.clone();
        assert_eq!(s.insert_knot(0, t), Ok(1));
        for f in [0.1f32, 0.25, 0.5, 0.75, 1.0] {
            let a = r.eval(0, f);
            let b = if f <= t { s.eval(0, f / t) } else { s.eval(1, (f - t) / (1.0 - t)) };
            assert!((a - b).length() < 1e-5, "{f}: {a:?} {b:?}");
        }
        assert!(s.insert_knot(2, 0.5).is_err());
        assert!(s.insert_knot(0, 0.0).is_err());
        assert!(s.insert_knot(0, 1.0).is_err());
    }

    #[test]
    fn nearest_point_feeds_insert_knot() {
        let mut r = line(&[v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0), v3(8.0, 0.0, 0.0)]);
        let hit = r.nearest_point(v3(6.0, 1.0, 0.0)).unwrap();
        assert_eq!(hit.segment, 1);
        assert!((hit.point.x - 6.0).abs() < 0.2 && (hit.distance - 1.0).abs() < 0.05);
        // The end of the curve still gives a fraction insert_knot accepts.
        let end = r.nearest_point(v3(20.0, 0.0, 0.0)).unwrap();
        assert!(end.fraction < 1.0);
        assert!(r.insert_knot(end.segment, end.fraction).is_ok());
        let ray = r.nearest_to_ray(v3(2.0, 5.0, 0.0), v3(0.0, -1.0, 0.0)).unwrap();
        assert_eq!(ray.segment, 0);
        assert!(ray.distance < 0.1);
    }

    #[test]
    fn click_next_to_a_knot_still_generates() {
        let mut r = BezierRecipe {
            knots: vec![Knot::new(v3(0.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO), Knot::new(v3(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO)],
            ..Default::default()
        };
        for target in [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0)] {
            let hit = r.nearest_point(target).unwrap();
            r.insert_knot(hit.segment, hit.fraction).unwrap();
            r.generate().unwrap();
        }
        r.height = 0.2;
        r.generate().unwrap();
    }

    #[test]
    fn tangent_modes() {
        let mut k = Knot::new(Vec3::ZERO, v3(-1.0, 0.0, 0.0), v3(2.0, 0.0, 0.0));
        k.set_tangent(v3(0.0, 0.0, 3.0), false);
        assert_eq!(k.tangent_in, v3(-1.0, 0.0, 0.0), "free leaves the other");
        k.mode = TangentMode::Aligned;
        k.set_tangent(v3(0.0, 0.0, 3.0), false);
        assert!((k.tangent_in - v3(0.0, 0.0, -1.0)).length() < 1e-6, "aligned keeps the length");
        k.mode = TangentMode::Mirrored;
        k.set_tangent(v3(0.0, 2.0, 0.0), true);
        assert_eq!(k.tangent_out, v3(0.0, -2.0, 0.0));
    }

    #[test]
    fn append_smooths_the_previous_knot() {
        let r = line(&[v3(0.0, 0.0, 0.0), v3(3.0, 0.0, 0.0), v3(3.0, 0.0, 3.0)]);
        assert_eq!(r.knots[0].tangent_out, v3(1.0, 0.0, 0.0));
        let k = r.knots[1];
        // Bisector of +X and +Z.
        assert!((k.tangent_out.normalized() - v3(1.0, 0.0, 1.0).normalized()).length() < 1e-5);
        assert!((k.tangent_in + k.tangent_out).length() < 1e-5);
    }

    #[test]
    fn encode_round_trips() {
        let mut r = line(&[v3(0.0, 0.0, 0.0), v3(3.0, 1.0, 0.0), v3(3.0, 0.0, 3.0)]);
        r.knots[1].roll = 30.0;
        r.knots[1].mode = TangentMode::Mirrored;
        r.closed = true;
        r.height = 0.25;
        r.columns = 3;
        let b = r.encode();
        assert_eq!(BezierRecipe::decode(&b), Some(r));
        assert_eq!(BezierRecipe::decode(&b[..b.len() - 1]), None);
        assert_eq!(BezierRecipe::decode(b"nope"), None);
        let mut newer = b.clone();
        newer[3] = 99;
        assert_eq!(BezierRecipe::decode(&newer), None);
    }
}
