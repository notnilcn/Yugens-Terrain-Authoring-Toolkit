//! The values the Part section of the inspector shows and the meshes its edits produce,
//! worked out from plain mesh and recipe values so they are testable without the editor.
use pt_modeler::{BMesh, ShapeSpec, shapes};
use pt_core::math::Vec3;
pub const MIN_SIZE: f32 = 0.05;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartField {
    SizeX = 0,
    SizeY,
    SizeZ,
    PositionX,
    PositionY,
    PositionZ,
    OrientationX,
    OrientationY,
    OrientationZ,
    Transparency,
}
impl PartField {
    pub const ALL: [PartField; 10] = [
        PartField::SizeX,
        PartField::SizeY,
        PartField::SizeZ,
        PartField::PositionX,
        PartField::PositionY,
        PartField::PositionZ,
        PartField::OrientationX,
        PartField::OrientationY,
        PartField::OrientationZ,
        PartField::Transparency,
    ];
    pub fn from_i64(v: i64) -> Option<PartField> {
        Self::ALL.into_iter().find(|f| *f as i64 == v)
    }
    pub fn axis(self) -> usize {
        match self {
            PartField::SizeX | PartField::PositionX | PartField::OrientationX => 0,
            PartField::SizeY | PartField::PositionY | PartField::OrientationY => 1,
            _ => 2,
        }
    }
}
pub fn with_axis(v: [f32; 3], axis: usize, value: f32) -> [f32; 3] {
    let mut out = v;
    if axis < 3 {
        out[axis] = value;
    }
    out
}
pub fn centre(lo: Vec3, hi: Vec3) -> Vec3 {
    (lo + hi) * 0.5
}
pub fn extent_size(lo: Vec3, hi: Vec3) -> [f32; 3] {
    [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z]
}
pub fn regenerable(spec: &ShapeSpec, cage: &BMesh) -> bool {
    if spec.sort.is_grid_shape() || spec.sort.is_polyline() {
        return false;
    }
    let g = shapes::produce(spec, None);
    if g.positions.is_empty()
        || g.positions.len() != cage.positions.len()
        || g.facets.len() != cage.facets.len()
    {
        return false;
    }
    let offset = cage.positions[0] - g.positions[0];
    g.positions
        .iter()
        .zip(&cage.positions)
        .all(|(a, b)| (*a + offset - *b).magnitude() < 1e-3)
}
pub fn resized(spec: &ShapeSpec, cage: &BMesh, size: [f32; 3]) -> Option<(ShapeSpec, BMesh)> {
    if !regenerable(spec, cage) {
        return None;
    }
    let (lo, hi) = cage.extent()?;
    let mut next = *spec;
    next.size = Vec3 {
        x: size[0].max(MIN_SIZE),
        y: size[1].max(MIN_SIZE),
        z: size[2].max(MIN_SIZE),
    };
    let mut g = shapes::produce(&next, None);
    let (nlo, nhi) = g.extent()?;
    g.displace(centre(lo, hi) - centre(nlo, nhi));
    if g.facets.len() == cage.facets.len() {
        for (f, old) in g.facets.iter_mut().zip(&cage.facets) {
            f.stuff = old.stuff;
            f.tint = old.tint;
            f.tex_coord = old.tex_coord;
        }
    } else if let Some(old) = cage.facets.first() {
        for f in &mut g.facets {
            f.stuff = old.stuff;
            f.tint = old.tint;
        }
    }
    Some((next, g))
}
#[cfg(test)]
mod tests {
    use super::*;
    use pt_modeler::ShapeKind;
    use pt_core::math::v3;
    fn boxed(size: Vec3) -> (ShapeSpec, BMesh) {
        let spec = ShapeSpec {
            sort: ShapeKind::Box,
            size,
            ..ShapeSpec::default()
        };
        let mut m = shapes::produce(&spec, None);
        m.displace(v3(3.0, 1.0, -2.0));
        for f in &mut m.facets {
            f.stuff = 2;
            f.tint = [10, 20, 30, 255];
        }
        (spec, m)
    }
    #[test]
    fn a_fresh_box_is_regenerable_and_an_edited_one_is_not() {
        let (spec, mut m) = boxed(v3(4.0, 1.0, 2.0));
        assert!(regenerable(&spec, &m));
        m.positions[0].y += 0.5;
        assert!(!regenerable(&spec, &m));
    }
    #[test]
    fn resizing_keeps_the_centre_and_the_looks() {
        let (spec, m) = boxed(v3(4.0, 1.0, 2.0));
        let (lo, hi) = m.extent().unwrap();
        let (next, g) = resized(&spec, &m, [6.0, 1.0, 2.0]).unwrap();
        assert_eq!(next.size, v3(6.0, 1.0, 2.0));
        let (nlo, nhi) = g.extent().unwrap();
        assert!((centre(lo, hi) - centre(nlo, nhi)).magnitude() < 1e-4);
        assert!((extent_size(nlo, nhi)[0] - 6.0).abs() < 1e-4);
        assert!(g.facets.iter().all(|f| f.stuff == 2 && f.tint == [10, 20, 30, 255]));
        assert!(regenerable(&next, &g));
    }
    #[test]
    fn sizes_are_clamped_to_a_minimum() {
        let (spec, m) = boxed(v3(1.0, 1.0, 1.0));
        let (next, _) = resized(&spec, &m, [0.0, -3.0, 1.0]).unwrap();
        assert_eq!(next.size, v3(MIN_SIZE, MIN_SIZE, 1.0));
    }
    #[test]
    fn grid_shapes_are_not_resized_here() {
        let spec = ShapeSpec {
            sort: ShapeKind::ALL.into_iter().find(|k| k.is_grid_shape()).unwrap(),
            ..ShapeSpec::default()
        };
        assert!(resized(&spec, &BMesh::default(), [1.0; 3]).is_none());
    }
    #[test]
    fn fields_resolve_and_name_their_axis() {
        for f in PartField::ALL {
            assert_eq!(PartField::from_i64(f as i64), Some(f));
        }
        assert_eq!(PartField::SizeY.axis(), 1);
        assert_eq!(PartField::OrientationZ.axis(), 2);
        assert_eq!(with_axis([1.0, 2.0, 3.0], 1, 9.0), [1.0, 9.0, 3.0]);
    }
}
