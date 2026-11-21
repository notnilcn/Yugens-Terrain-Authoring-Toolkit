use crate::math::*;
use std::collections::{BTreeSet, HashMap};
pub type Edge = (u32, u32);
pub fn edge(a: u32, b: u32) -> Edge {
    if a < b { (a, b) } else { (b, a) }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvParams {
    pub offset: Vec2,
    pub rotation: f32,
    pub magnify: Vec2,
    pub world: bool,
}
impl Default for UvParams {
    fn default() -> Self {
        UvParams {
            offset: Vec2::ZERO,
            rotation: 0.0,
            magnify: v2(1.0, 1.0),
            world: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub verts: Vec<u32>,
    pub stuff: u16,
    pub glide: u16,
    pub tint: [u8; 4],
    pub tex_coord: UvParams,
    pub uv_manual: bool,
    pub uv_coords: Vec<Vec2>,
    pub uv_axis: Vec3,
    pub color_corners: Vec<[u8; 4]>,
}
impl Face {
    pub fn new(verts: Vec<u32>) -> Face {
        Face {
            verts,
            stuff: 0,
            glide: 0,
            tint: [255; 4],
            tex_coord: UvParams::default(),
            uv_manual: false,
            uv_coords: Vec::new(),
            uv_axis: Vec3::ZERO,
            color_corners: Vec::new(),
        }
    }
    pub fn with_verts(&self, verts: Vec<u32>) -> Face {
        Face {
            verts,
            uv_manual: false,
            uv_coords: Vec::new(),
            color_corners: Vec::new(),
            ..self.clone()
        }
    }
    pub fn corner_colors(&self) -> Option<&[[u8; 4]]> {
        (self.color_corners.len() == self.verts.len()).then_some(self.color_corners.as_slice())
    }
    pub fn corner_color(&self, ci: usize) -> [u8; 4] {
        self.corner_colors()
            .and_then(|c| c.get(ci).copied())
            .unwrap_or(self.tint)
    }
    pub fn uv_normal(&self, n: Vec3) -> Vec3 {
        if self.uv_axis == Vec3::ZERO {
            n
        } else {
            self.uv_axis.unitized()
        }
    }
    pub fn by_hand_tex_coords(&self) -> Option<&[Vec2]> {
        (self.uv_manual && self.uv_coords.len() == self.verts.len())
            .then_some(self.uv_coords.as_slice())
    }
    pub fn flip_order(&mut self) {
        self.verts.reverse();
        if self.uv_coords.len() == self.verts.len() {
            self.uv_coords.reverse();
        }
        if self.color_corners.len() == self.verts.len() {
            self.color_corners.reverse();
        }
    }
    pub fn loop_edges(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        let n = self.verts.len();
        (0..n).map(move |i| (self.verts[i], self.verts[(i + 1) % n]))
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BMesh {
    pub positions: Vec<Vec3>,
    pub facets: Vec<Face>,
}
pub fn newell(points: &[Vec3]) -> Vec3 {
    let mut n = Vec3::ZERO;
    let k = points.len();
    for i in 0..k {
        let a = points[i];
        let b = points[(i + 1) % k];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    n
}
impl BMesh {
    pub fn add_vert(&mut self, p: Vec3) -> u32 {
        self.positions.push(p);
        (self.positions.len() - 1) as u32
    }
    pub fn add_face(&mut self, verts: Vec<u32>) -> usize {
        self.facets.push(Face::new(verts));
        self.facets.len() - 1
    }
    pub fn add_face_facing(&mut self, mut verts: Vec<u32>, outward: Vec3) -> usize {
        let pts: Vec<Vec3> = verts.iter().map(|v| self.positions[*v as usize]).collect();
        if newell(&pts).innerprod(outward) < 0.0 {
            verts.reverse();
        }
        self.add_face(verts)
    }
    pub fn face_points(&self, f: usize) -> Vec<Vec3> {
        self.facets[f]
            .verts
            .iter()
            .map(|v| self.positions[*v as usize])
            .collect()
    }
    pub fn face_normal(&self, f: usize) -> Vec3 {
        newell(&self.face_points(f)).unitized()
    }
    pub fn face_area(&self, f: usize) -> f32 {
        newell(&self.face_points(f)).magnitude() * 0.5
    }
    pub fn face_center(&self, f: usize) -> Vec3 {
        let p = self.face_points(f);
        p.iter().fold(Vec3::ZERO, |a, b| a + *b) / p.len().max(1) as f32
    }
    pub fn ridges(&self) -> Vec<Edge> {
        let mut s = BTreeSet::new();
        for f in &self.facets {
            for (a, b) in f.loop_edges() {
                s.insert(edge(a, b));
            }
        }
        s.into_iter().collect()
    }
    pub fn edge_faces(&self) -> HashMap<Edge, Vec<usize>> {
        let mut m: HashMap<Edge, Vec<usize>> = HashMap::new();
        for (fi, f) in self.facets.iter().enumerate() {
            for (a, b) in f.loop_edges() {
                m.entry(edge(a, b)).or_default().push(fi);
            }
        }
        m
    }
    pub fn vert_faces(&self) -> Vec<Vec<usize>> {
        let mut m = vec![Vec::new(); self.positions.len()];
        for (fi, f) in self.facets.iter().enumerate() {
            for v in &f.verts {
                if let Some(l) = m.get_mut(*v as usize) {
                    l.push(fi);
                }
            }
        }
        m
    }
    pub fn extent(&self) -> Option<(Vec3, Vec3)> {
        let first = *self.positions.first()?;
        Some(self.positions.iter().fold((first, first), |(lo, hi), p| {
            (
                v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
                v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
            )
        }))
    }
    pub fn sanity_check(&self) -> Result<(), String> {
        let n = self.positions.len() as u32;
        let mut directed = HashMap::new();
        for (fi, f) in self.facets.iter().enumerate() {
            if f.verts.len() < 3 {
                return Err(format!("face {fi} has {} verts", f.verts.len()));
            }
            let set: BTreeSet<u32> = f.verts.iter().copied().collect();
            if set.len() != f.verts.len() {
                return Err(format!("face {fi} repeats a vertex"));
            }
            if let Some(v) = f.verts.iter().find(|v| **v >= n) {
                return Err(format!("face {fi} references vertex {v} (of {n})"));
            }
            for (a, b) in f.loop_edges() {
                if let Some(other) = directed.insert((a, b), fi) {
                    return Err(format!(
                        "edge {a}->{b} used in the same direction by faces {other} and {fi}"
                    ));
                }
            }
        }
        for (e, fs) in self.edge_faces() {
            if fs.len() > 2 {
                return Err(format!("edge {e:?} is shared by {} faces", fs.len()));
            }
        }
        Ok(())
    }
    pub fn is_closed(&self) -> bool {
        let mut directed = BTreeSet::new();
        for f in &self.facets {
            for e in f.loop_edges() {
                directed.insert(e);
            }
        }
        !self.facets.is_empty() && directed.iter().all(|(a, b)| directed.contains(&(*b, *a)))
    }
    pub fn compact(&mut self) -> Vec<Option<u32>> {
        let mut used = vec![false; self.positions.len()];
        for f in &self.facets {
            for v in &f.verts {
                used[*v as usize] = true;
            }
        }
        let mut map = vec![None; self.positions.len()];
        let mut pos = Vec::new();
        for (i, p) in self.positions.iter().enumerate() {
            if used[i] {
                map[i] = Some(pos.len() as u32);
                pos.push(*p);
            }
        }
        self.positions = pos;
        for f in &mut self.facets {
            for v in &mut f.verts {
                *v = map[*v as usize].expect("every vertex of a face is marked used");
            }
        }
        map
    }
    pub fn append_tail(&mut self, other: &BMesh) {
        let o = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        for f in &other.facets {
            self.facets
                .push(f.with_verts(f.verts.iter().map(|v| v + o).collect()));
        }
    }
    pub fn displace(&mut self, d: Vec3) {
        for p in &mut self.positions {
            *p = *p + d;
        }
    }
    pub fn total_area(&self) -> f32 {
        (0..self.facets.len()).map(|f| self.face_area(f)).sum()
    }
    pub fn volume(&self) -> f32 {
        let mut v = 0.0;
        for f in 0..self.facets.len() {
            let p = self.face_points(f);
            for i in 1..p.len().saturating_sub(1) {
                v += p[0].innerprod(p[i].cross(p[i + 1]));
            }
        }
        v / 6.0
    }
    pub fn verts_of_faces(&self, faces: &BTreeSet<usize>) -> BTreeSet<u32> {
        faces
            .iter()
            .filter_map(|f| self.facets.get(*f))
            .flat_map(|f| f.verts.iter().copied())
            .collect()
    }
    pub fn verts_of_edges(&self, edges: &BTreeSet<Edge>) -> BTreeSet<u32> {
        edges.iter().flat_map(|(a, b)| [*a, *b]).collect()
    }
    pub fn edges_of_faces(&self, faces: &BTreeSet<usize>) -> BTreeSet<Edge> {
        faces
            .iter()
            .filter_map(|f| self.facets.get(*f))
            .flat_map(|f| f.loop_edges().map(|(a, b)| edge(a, b)).collect::<Vec<_>>())
            .collect()
    }
    pub fn edges_within(&self, verts: &BTreeSet<u32>) -> BTreeSet<Edge> {
        self.ridges()
            .into_iter()
            .filter(|(a, b)| verts.contains(a) && verts.contains(b))
            .collect()
    }
    pub fn faces_within(&self, verts: &BTreeSet<u32>) -> BTreeSet<usize> {
        (0..self.facets.len())
            .filter(|f| self.facets[*f].verts.iter().all(|v| verts.contains(v)))
            .collect()
    }
    pub fn expand_facets(&self, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
        let ef = self.edge_faces();
        let mut out = faces.clone();
        for e in self.edges_of_faces(faces) {
            if let Some(fs) = ef.get(&e) {
                out.extend(fs.iter().copied());
            }
        }
        out
    }
    pub fn linked_faces(&self, seed: usize) -> BTreeSet<usize> {
        let mut s = BTreeSet::from([seed]);
        loop {
            let g = self.expand_facets(&s);
            if g.len() == s.len() {
                return s;
            }
            s = g;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn quad() -> BMesh {
        let mut m = BMesh::default();
        for p in [
            v3(0.0, 0.0, 0.0),
            v3(1.0, 0.0, 0.0),
            v3(1.0, 0.0, -1.0),
            v3(0.0, 0.0, -1.0),
        ] {
            m.add_vert(p);
        }
        m.add_face(vec![0, 1, 2, 3]);
        m
    }
    #[test]
    fn newell_points_out_for_ccw() {
        let m = quad();
        let n = m.face_normal(0);
        assert!((n.y - 1.0).abs() < 1e-5, "{n:?}");
        assert!((m.face_area(0) - 1.0).abs() < 1e-5);
    }
    #[test]
    fn compact_drops_unused() {
        let mut m = quad();
        m.add_vert(v3(5.0, 5.0, 5.0));
        m.positions.insert(0, v3(9.0, 9.0, 9.0));
        for v in &mut m.facets[0].verts {
            *v += 1;
        }
        let map = m.compact();
        assert_eq!(m.positions.len(), 4);
        assert_eq!(map[0], None);
        assert_eq!(m.facets[0].verts, vec![0, 1, 2, 3]);
        m.sanity_check().unwrap();
    }
    #[test]
    fn validate_rejects_bad_faces() {
        let mut m = quad();
        m.facets.push(Face::new(vec![0, 1]));
        assert!(m.sanity_check().is_err());
        let mut m = quad();
        m.facets.push(Face::new(vec![0, 1, 2]));
        assert!(
            m.sanity_check().is_err(),
            "0->1 twice in the same direction"
        );
    }
}
