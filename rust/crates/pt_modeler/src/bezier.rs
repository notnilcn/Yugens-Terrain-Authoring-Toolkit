use super::model::{BMesh, Face};
use pt_geom::math::*;
pub const PICK_SAMPLES: u32 = 32;
pub const PICK_MARGIN: f32 = 0.0505;
pub const MAX_VERTS: usize = 0xffff;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TangentMode {
    #[default]
    Free = 0,
    Aligned = 1,
    Mirrored = 2,
}
impl TangentMode {
    pub const ALL: [TangentMode; 3] = [
        TangentMode::Free,
        TangentMode::Aligned,
        TangentMode::Mirrored,
    ];
    pub fn from_u8(v: u8) -> Option<TangentMode> {
        Self::ALL.get(v as usize).copied()
    }
    pub fn moniker(self) -> &'static str {
        match self {
            TangentMode::Free => "Free",
            TangentMode::Aligned => "Aligned",
            TangentMode::Mirrored => "Mirrored",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knot {
    pub position: Vec3,
    pub tangent_in: Vec3,
    pub tangent_out: Vec3,
    pub roll: f32,
    pub mode: TangentMode,
}
impl Knot {
    pub fn new(position: Vec3, tangent_in: Vec3, tangent_out: Vec3) -> Knot {
        Knot {
            position,
            tangent_in,
            tangent_out,
            roll: 0.0,
            mode: TangentMode::Free,
        }
    }
    pub fn assign_toucher(&mut self, value: Vec3, incoming: bool) {
        let other = if incoming {
            self.tangent_out
        } else {
            self.tangent_in
        };
        if incoming {
            self.tangent_in = value;
        } else {
            self.tangent_out = value;
        }
        let opposite = match self.mode {
            TangentMode::Free => return,
            TangentMode::Aligned => {
                let l = value.magnitude();
                if l * l > 1e-12 {
                    value * (-other.magnitude() / l)
                } else {
                    Vec3::ZERO
                }
            }
            TangentMode::Mirrored => -value,
        };
        if incoming {
            self.tangent_out = opposite;
        } else {
            self.tangent_in = opposite;
        }
    }
    pub fn assign_setting(&mut self, mode: TangentMode) {
        self.mode = mode;
        let out = self.tangent_out;
        self.assign_toucher(out, false);
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BezierRecipe {
    pub knots: Vec<Knot>,
    pub is_shut: bool,
    pub glide: bool,
    pub radius: f32,
    pub stature: f32,
    pub ranks: u32,
    pub columns: u32,
}
impl Default for BezierRecipe {
    fn default() -> Self {
        BezierRecipe {
            knots: Vec::new(),
            is_shut: false,
            glide: true,
            radius: 0.1005,
            stature: 0.0,
            ranks: 8,
            columns: 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveHit {
    pub segment: usize,
    pub fraction: f32,
    pub dot: Vec3,
    pub span: f32,
}
fn spin(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.innerprod(p) * (1.0 - c))
}
fn rotate_from_to(start_pt: Vec3, to: Vec3, v: Vec3) -> Vec3 {
    let (a, b) = (start_pt.unitized(), to.unitized());
    let axis = a.cross(b);
    let (sin, cos) = (axis.magnitude(), a.innerprod(b));
    if sin < 1e-7 {
        if cos > 0.0 {
            return v;
        }
        let m = v3(a.x.abs(), a.y.abs(), a.z.abs());
        let other = if m.y <= m.z {
            if m.y < m.x {
                v3(0.0, 1.0, 0.0)
            } else {
                v3(1.0, 0.0, 0.0)
            }
        } else if m.x <= m.z {
            v3(1.0, 0.0, 0.0)
        } else {
            v3(0.0, 0.0, 1.0)
        };
        return spin(v, a.cross(other).unitized(), std::f32::consts::PI);
    }
    spin(v, axis / sin, sin.atan2(cos))
}
fn safe_normalized(v: Vec3) -> Vec3 {
    let l = v.magnitude();
    if l > 1e-10 { v / l } else { Vec3::ZERO }
}
pub fn dot(a: &Knot, b: &Knot, t: f32) -> Vec3 {
    let u = 1.0 - t;
    let p1 = a.position + a.tangent_out;
    let p2 = b.position + b.tangent_in;
    a.position * (u * u * u)
        + p1 * (3.0 * u * u * t)
        + p2 * (3.0 * u * t * t)
        + b.position * (t * t * t)
}
pub fn toucher(a: &Knot, b: &Knot, t: f32) -> Vec3 {
    let u = 1.0 - t;
    let middle = b.position + b.tangent_in - a.position - a.tangent_out;
    let d = middle * (6.0 * u * t) + a.tangent_out * (3.0 * u * u) - b.tangent_in * (3.0 * t * t);
    if d.innerprod(d) <= 1e-12 {
        safe_normalized(b.position - a.position)
    } else {
        d.unitized()
    }
}
impl BezierRecipe {
    pub fn breadth(&self) -> f32 {
        self.radius * 2.0
    }
    pub fn set_width(&mut self, w: f32) {
        self.radius = w * 0.5;
    }
    pub fn stretches(&self) -> usize {
        if self.knots.is_empty() {
            0
        } else {
            self.knots.len() - (!self.is_shut) as usize
        }
    }
    fn ends(&self, segment: usize) -> (&Knot, &Knot) {
        (
            &self.knots[segment],
            &self.knots[(segment + 1) % self.knots.len()],
        )
    }
    pub fn eval(&self, segment: usize, t: f32) -> Vec3 {
        let (a, b) = self.ends(segment);
        dot(a, b, t)
    }
    pub fn sanity_check(&self) -> Result<(), String> {
        let n = self.knots.len();
        let count_ok = if self.is_shut {
            (3..=256).contains(&n)
        } else {
            (2..=256).contains(&n)
        };
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        let knots_ok = self.knots.iter().all(|k| {
            finite(k.position)
                && finite(k.tangent_in)
                && finite(k.tangent_out)
                && k.roll.is_finite()
        });
        if count_ok
            && (1..=128).contains(&self.ranks)
            && (1..=128).contains(&self.columns)
            && self.radius.is_finite()
            && self.radius >= 1e-4
            && self.stature.is_finite()
            && knots_ok
        {
            Ok(())
        } else {
            Err("Bezier settings are outside their valid range.".into())
        }
    }
    pub fn append_bend(&mut self, position: Vec3, even_prior: bool) {
        let Some(last) = self.knots.len().checked_sub(1) else {
            self.knots
                .push(Knot::new(position, v3(0.0, 0.0, -0.5), v3(0.0, 0.0, 0.5)));
            return;
        };
        let chord = position - self.knots[last].position;
        let t = chord / 3.0;
        if even_prior {
            if last == 0 {
                self.knots[0].tangent_out = t;
            } else {
                self.smooth_between(last, last - 1, position);
            }
        }
        self.knots.push(Knot::new(position, -t, t));
    }
    pub fn smooth_knot(&mut self, index: usize) {
        let n = self.knots.len();
        if index >= n || n < 2 || (!self.is_shut && (index == 0 || index + 1 == n)) {
            return;
        }
        let (prev, next) = ((index + n - 1) % n, (index + 1) % n);
        let p = self.knots[next].position;
        self.smooth_between(index, prev, p);
    }
    fn smooth_between(&mut self, index: usize, prev: usize, next_pos: Vec3) {
        let chord = next_pos - self.knots[index].position;
        let previous = self.knots[index].position - self.knots[prev].position;
        let (cl, pl) = (chord.magnitude(), previous.magnitude());
        if !(1e-6 < pl && 1e-6 < cl) {
            return;
        }
        let bisector = chord / cl + previous / pl;
        if bisector.innerprod(bisector) <= 1e-12 {
            return;
        }
        let dir = bisector.unitized();
        let k = &mut self.knots[index];
        k.tangent_in = -dir * (pl / 3.0);
        k.tangent_out = dir * (cl / 3.0);
    }
    pub fn splice_bend(&mut self, segment: usize, fraction: f32) -> Result<usize, String> {
        let count = self.knots.len();
        if count < 2 || segment >= self.stretches() || !(0.0 < fraction && fraction < 1.0) {
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
    pub fn remove_knot(&mut self, index: usize) -> bool {
        let min = if self.is_shut { 3 } else { 2 };
        if index < self.knots.len() && self.knots.len() > min {
            self.knots.remove(index);
            true
        } else {
            false
        }
    }
    pub fn nearest_point(&self, p: Vec3) -> Option<CurveHit> {
        self.nearest_by(|q| (q - p).magnitude())
    }
    pub fn nearest_to_ray(&self, origin: Vec3, dir: Vec3) -> Option<CurveHit> {
        let d = safe_normalized(dir);
        self.nearest_by(|q| {
            let s = (q - origin).innerprod(d).max(0.0);
            (q - (origin + d * s)).magnitude()
        })
    }
    fn nearest_by(&self, dist: impl Fn(Vec3) -> f32) -> Option<CurveHit> {
        let mut best: Option<CurveHit> = None;
        for segment in 0..self.stretches() {
            for i in 0..=PICK_SAMPLES {
                let fraction =
                    (i as f32 / PICK_SAMPLES as f32).clamp(PICK_MARGIN, 1.0 - PICK_MARGIN);
                let point = self.eval(segment, fraction);
                let distance = dist(point);
                if best.is_none_or(|b| distance < b.span) {
                    best = Some(CurveHit {
                        segment,
                        fraction,
                        dot: point,
                        span: distance,
                    });
                }
            }
        }
        best
    }
    pub fn produce(&self) -> Result<BMesh, String> {
        self.sanity_check()?;
        let segments = self.stretches();
        let rows = self.ranks as usize;
        let samples = segments * rows;
        let cols = self.columns as usize;
        let ring = if self.is_shut { samples } else { samples + 1 };
        let layers = if self.stature == 0.0 { 1 } else { 2 };
        if ring * (cols + 1) * layers > MAX_VERTS {
            return Err(
                "The Bezier mesh would have too many vertices; lower rows or columns.".into(),
            );
        }
        let path = Path::probe(self, segments)?;
        let mut m = BMesh::default();
        for i in 0..ring {
            let across = spin(path.normals[i], path.tangents[i], path.rolls[i]);
            for j in 0..=cols {
                let w = ((2 * j) as f32 / cols as f32 - 1.0) * self.radius;
                m.add_vert(path.centers[i] + across * w);
            }
        }
        let vid = |i: usize, j: usize| ((i % ring) * (cols + 1) + j) as u32;
        for i in 0..samples {
            for j in 0..cols {
                let (u0, u1) = (j as f32 / cols as f32, (j + 1) as f32 / cols as f32);
                let (near, far) = (path.span[i], path.span[i + 1]);
                let mut f = Face::new(vec![
                    vid(i, j),
                    vid(i + 1, j),
                    vid(i + 1, j + 1),
                    vid(i, j + 1),
                ]);
                f.uv_manual = true;
                f.uv_coords = vec![v2(u0, near), v2(u0, far), v2(u1, far), v2(u1, near)];
                f.glide = self.glide as u16;
                m.facets.push(f);
            }
        }
        let side = if self.stature < 0.0 {
            -Vec3::UP
        } else {
            Vec3::UP
        };
        let facing: f32 = (0..m.facets.len())
            .map(|f| super::model::newell(&m.face_points(f)).innerprod(side))
            .sum();
        if facing < 0.0 {
            for f in &mut m.facets {
                f.flip_order();
            }
        }
        if self.stature != 0.0 {
            extrude(&mut m, self.stature);
        }
        m.sanity_check()?;
        Ok(m)
    }
    pub fn pack(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(20 + self.knots.len() * 41);
        b.extend_from_slice(b"BZR");
        b.push(FORMAT_VERSION);
        b.push(self.is_shut as u8 | (self.glide as u8) << 1);
        b.extend_from_slice(&self.radius.to_le_bytes());
        b.extend_from_slice(&self.stature.to_le_bytes());
        b.extend_from_slice(&(self.ranks.min(u16::MAX as u32) as u16).to_le_bytes());
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
    pub fn unpack(bytes: &[u8]) -> Option<BezierRecipe> {
        let mut r = Reader { bx: bytes, at: 0 };
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
            knots.push(Knot {
                position: v[0],
                tangent_in: v[1],
                tangent_out: v[2],
                roll,
                mode,
            });
        }
        Some(BezierRecipe {
            knots,
            is_shut: flags & 1 != 0,
            glide: flags & 2 != 0,
            radius,
            stature: height,
            ranks: rows,
            columns,
        })
    }
}
const FORMAT_VERSION: u8 = 1;
struct Reader<'a> {
    bx: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bx.get(self.at..self.at + n)?;
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
struct Path {
    centers: Vec<Vec3>,
    tangents: Vec<Vec3>,
    rolls: Vec<f32>,
    normals: Vec<Vec3>,
    span: Vec<f32>,
}
impl Path {
    fn probe(r: &BezierRecipe, segments: usize) -> Result<Path, String> {
        let rows = r.ranks as usize;
        let samples = segments * rows;
        let mut p = Path {
            centers: Vec::new(),
            tangents: Vec::new(),
            rolls: Vec::new(),
            normals: Vec::new(),
            span: Vec::new(),
        };
        for i in 0..=samples {
            let segment = (i / rows).min(segments - 1);
            let (a, b) = r.ends(segment);
            if (a.position - b.position).innerprod(a.position - b.position) <= 1e-12 {
                return Err("Adjacent Bezier knots must have different positions.".into());
            }
            let t = (i - segment * rows) as f32 / rows as f32;
            p.centers.push(dot(a, b, t));
            p.tangents.push(toucher(a, b, t));
            p.rolls.push((a.roll + t * (b.roll - a.roll)).to_radians());
        }
        let first = p.tangents[0];
        let up = if first.y.abs() < 0.9005 {
            Vec3::UP
        } else {
            v3(1.0, 0.0, 0.0)
        };
        p.normals.push(up.cross(first).unitized());
        for i in 1..p.tangents.len() {
            let t = p.tangents[i];
            let rotated = rotate_from_to(p.tangents[i - 1], t, p.normals[i - 1]);
            let n = rotated - t * rotated.innerprod(t);
            let step = p.centers[i - 1] - p.centers[i];
            let finite = n.x.is_finite() && n.y.is_finite() && n.z.is_finite();
            if !finite || n.innerprod(n) <= 1e-12 || step.innerprod(step) <= 1e-14 {
                return Err("The Bezier path reverses or has a zero-length sample.".into());
            }
            p.normals.push(n.unitized());
        }
        if r.is_shut {
            let last = p.normals.len() - 1;
            let (t0, n0) = (p.tangents[0], p.normals[0]);
            let moved = rotate_from_to(p.tangents[last], t0, p.normals[last]);
            let correction = moved.cross(n0).innerprod(t0).atan2(moved.innerprod(n0));
            for i in 0..=last {
                p.normals[i] = spin(
                    p.normals[i],
                    p.tangents[i],
                    i as f32 * correction / last as f32,
                );
            }
        }
        p.span.push(0.0);
        for i in 1..p.centers.len() {
            let d = p.span[i - 1] + (p.centers[i] - p.centers[i - 1]).magnitude();
            p.span.push(d);
        }
        Ok(p)
    }
}
fn extrude(m: &mut BMesh, height: f32) {
    let offset = v3(0.0, height, 0.0);
    let k = m.positions.len() as u32;
    let base = m.facets.clone();
    let positions = m.positions.clone();
    m.positions.extend(positions.iter().map(|p| *p + offset));
    let boundary: Vec<(u32, u32)> = {
        let all = (0..base.len()).collect();
        super::topo::region_boundary(m, &all)
    };
    m.facets.clear();
    for f in &base {
        let mut top = f.clone();
        top.verts = f.verts.iter().map(|v| v + k).collect();
        m.facets.push(top);
        let mut bottom = f.clone();
        bottom.flip_order();
        m.facets.push(bottom);
    }
    let h = height.abs();
    for (a, b) in boundary {
        let mut f = Face::new(vec![a, b, b + k, a + k]);
        f.uv_manual = true;
        f.uv_coords = vec![v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, h), v2(0.0, h)];
        m.facets.push(f);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn streak(points: &[Vec3]) -> BezierRecipe {
        let mut r = BezierRecipe {
            ranks: 4,
            ..Default::default()
        };
        for p in points {
            r.append_bend(*p, true);
        }
        r
    }
    #[test]
    fn limits_are_checked() {
        let two = [v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0)];
        assert!(streak(&two[..1]).produce().is_err());
        assert!(streak(&two).produce().is_ok());
        let mut tri = streak(&[v3(0.0, 0.0, 0.0), v3(2.0, 0.0, 0.0), v3(0.0, 0.0, 2.0)]);
        tri.is_shut = true;
        assert!(tri.produce().is_ok());
        tri.knots.truncate(2);
        assert!(tri.produce().is_err());
        let mut r = streak(&two);
        for (rows, cols, ok) in [
            (0, 1, false),
            (129, 1, false),
            (1, 0, false),
            (1, 129, false),
            (128, 128, true),
        ] {
            r.ranks = rows;
            r.columns = cols;
            assert_eq!(r.produce().is_ok(), ok, "{rows}x{cols}");
        }
        r.ranks = 2;
        r.columns = 1;
        r.radius = 1e-5;
        assert!(r.produce().is_err());
        r.radius = 0.1;
        r.stature = f32::NAN;
        assert!(r.produce().is_err());
        let mut same = streak(&two);
        same.knots[1].position = same.knots[0].position;
        assert!(same.produce().is_err());
    }
    #[test]
    fn straight_plane_faces_up_with_arc_length_uvs() {
        let mut r = streak(&[v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0)]);
        r.columns = 2;
        r.radius = 0.5;
        let m = r.produce().unwrap();
        m.sanity_check().unwrap();
        assert_eq!(m.facets.len(), 4 * 2);
        assert_eq!(m.positions.len(), 5 * 3, "shared vertices");
        for f in 0..m.facets.len() {
            assert!(m.face_normal(f).y > 0.99, "face {f} not up");
            let uv = m.facets[f].by_hand_tex_coords().unwrap();
            assert!(
                uv.iter()
                    .all(|c| (0.0..=1.0).contains(&c.x) && (-1e-5..=4.0 + 1e-4).contains(&c.y))
            );
        }
        let total: f32 = (0..m.facets.len()).map(|f| m.face_area(f)).sum();
        assert!((total - 4.0).abs() < 1e-3, "{total}");
    }
    #[test]
    fn block_is_closed_even_when_the_roll_turns_over() {
        let mut r = streak(&[v3(0.0, 0.0, 0.0), v3(3.0, 0.0, 1.0), v3(6.0, 0.0, 0.0)]);
        r.stature = 0.5;
        r.radius = 0.4;
        r.columns = 2;
        let m = r.produce().unwrap();
        assert!(m.is_closed());
        assert!(m.volume() > 0.0);
        r.knots[2].roll = 180.0;
        let m = r.produce().unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        r.knots[2].roll = 0.0;
        r.stature = -0.5;
        let m = r.produce().unwrap();
        assert!(m.is_closed() && m.volume() > 0.0, "{}", m.volume());
    }
    #[test]
    fn closed_loop_shares_its_seam() {
        let mut r = streak(&[
            v3(0.0, 0.0, 0.0),
            v3(4.0, 0.0, 0.0),
            v3(4.0, 0.0, 4.0),
            v3(0.0, 0.0, 4.0),
        ]);
        r.is_shut = true;
        for i in 0..4 {
            r.smooth_knot(i);
        }
        r.radius = 0.3;
        let flat = r.produce().unwrap();
        flat.sanity_check().unwrap();
        assert_eq!(
            flat.positions.len(),
            4 * 4 * 2,
            "the last row reuses the first"
        );
        assert!(flat.edge_faces().values().all(|f| f.len() <= 2));
        r.stature = 1.0;
        let block = r.produce().unwrap();
        assert!(block.is_closed() && block.volume() > 0.0);
        let vmax = flat
            .facets
            .iter()
            .flat_map(|f| f.uv_coords.iter().map(|c| c.y))
            .fold(0.0f32, f32::max);
        assert!(vmax > 12.0, "{vmax}");
    }
    #[test]
    fn splice_bend_retains_this_arc() {
        let r = BezierRecipe {
            knots: vec![
                Knot::new(v3(0.0, 0.0, 0.0), Vec3::ZERO, v3(0.5, 0.0, 0.2)),
                Knot::new(v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 0.4), Vec3::ZERO),
            ],
            ..Default::default()
        };
        let t = 0.3;
        let mut s = r.clone();
        assert_eq!(s.splice_bend(0, t), Ok(1));
        for f in [0.1f32, 0.25, 0.5, 0.75, 1.0] {
            let a = r.eval(0, f);
            let b = if f <= t {
                s.eval(0, f / t)
            } else {
                s.eval(1, (f - t) / (1.0 - t))
            };
            assert!((a - b).magnitude() < 1e-5, "{f}: {a:?} {b:?}");
        }
        assert!(s.splice_bend(2, 0.5).is_err());
        assert!(s.splice_bend(0, 0.0).is_err());
        assert!(s.splice_bend(0, 1.0).is_err());
    }
    #[test]
    fn closest_dot_supplies_splice_bend() {
        let mut r = streak(&[v3(0.0, 0.0, 0.0), v3(4.0, 0.0, 0.0), v3(8.0, 0.0, 0.0)]);
        let hit = r.nearest_point(v3(6.0, 1.0, 0.0)).unwrap();
        assert_eq!(hit.segment, 1);
        assert!((hit.dot.x - 6.0).abs() < 0.2 && (hit.span - 1.0).abs() < 0.05);
        let end = r.nearest_point(v3(20.0, 0.0, 0.0)).unwrap();
        assert!(end.fraction < 1.0);
        assert!(r.splice_bend(end.segment, end.fraction).is_ok());
        let ray = r
            .nearest_to_ray(v3(2.0, 5.0, 0.0), v3(0.0, -1.0, 0.0))
            .unwrap();
        assert_eq!(ray.segment, 0);
        assert!(ray.span < 0.1);
    }
    #[test]
    fn click_next_to_a_knot_still_generates() {
        let mut r = BezierRecipe {
            knots: vec![
                Knot::new(v3(0.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO),
                Knot::new(v3(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO),
            ],
            ..Default::default()
        };
        for target in [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0)] {
            let hit = r.nearest_point(target).unwrap();
            r.splice_bend(hit.segment, hit.fraction).unwrap();
            r.produce().unwrap();
        }
        r.stature = 0.2;
        r.produce().unwrap();
    }
    #[test]
    fn tangent_modes() {
        let mut k = Knot::new(Vec3::ZERO, v3(-1.0, 0.0, 0.0), v3(2.0, 0.0, 0.0));
        k.assign_toucher(v3(0.0, 0.0, 3.0), false);
        assert_eq!(k.tangent_in, v3(-1.0, 0.0, 0.0), "free leaves the other");
        k.mode = TangentMode::Aligned;
        k.assign_toucher(v3(0.0, 0.0, 3.0), false);
        assert!(
            (k.tangent_in - v3(0.0, 0.0, -1.0)).magnitude() < 1e-6,
            "aligned keeps the length"
        );
        k.mode = TangentMode::Mirrored;
        k.assign_toucher(v3(0.0, 2.0, 0.0), true);
        assert_eq!(k.tangent_out, v3(0.0, -2.0, 0.0));
    }
    #[test]
    fn append_evens_this_prior_bend() {
        let r = streak(&[v3(0.0, 0.0, 0.0), v3(3.0, 0.0, 0.0), v3(3.0, 0.0, 3.0)]);
        assert_eq!(r.knots[0].tangent_out, v3(1.0, 0.0, 0.0));
        let k = r.knots[1];
        assert!((k.tangent_out.unitized() - v3(1.0, 0.0, 1.0).unitized()).magnitude() < 1e-5);
        assert!((k.tangent_in + k.tangent_out).magnitude() < 1e-5);
    }
    #[test]
    fn encode_round_trips() {
        let mut r = streak(&[v3(0.0, 0.0, 0.0), v3(3.0, 1.0, 0.0), v3(3.0, 0.0, 3.0)]);
        r.knots[1].roll = 30.0;
        r.knots[1].mode = TangentMode::Mirrored;
        r.is_shut = true;
        r.stature = 0.25;
        r.columns = 3;
        let b = r.pack();
        assert_eq!(BezierRecipe::unpack(&b), Some(r));
        assert_eq!(BezierRecipe::unpack(&b[..b.len() - 1]), None);
        assert_eq!(BezierRecipe::unpack(b"nope"), None);
        let mut newer = b.clone();
        newer[3] = 99;
        assert_eq!(BezierRecipe::unpack(&newer), None);
    }
}
