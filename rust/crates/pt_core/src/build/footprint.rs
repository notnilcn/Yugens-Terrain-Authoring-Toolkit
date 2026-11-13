//! Floor footprints of a builder mesh: the polygons it stands on, used by the terrain to keep
//! grass (and small-hex decorations) from growing through a building's floor.

use super::model::BMesh;
use crate::math::*;

/// Loops (local positions) of the downward-facing faces whose lowest point lies within `band`
/// of the mesh's lowest point: the slab / plinth / rubble bottoms resting on the ground.
pub fn floor_footprints(m: &BMesh, band: f32) -> Vec<Vec<Vec3>> {
    let Some((lo, _)) = m.bounds() else { return Vec::new() };
    (0..m.faces.len())
        .filter(|f| m.face_normal(*f).y < -0.7)
        .map(|f| m.face_points(f))
        .filter(|pts| pts.len() >= 3 && pts.iter().map(|p| p.y).fold(f32::MAX, f32::min) <= lo.y + band)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;

    #[test]
    fn box_bottom_is_its_footprint_and_a_raised_box_is_not() {
        let mut m = cuboid(v3(2.0, 1.0, 2.0));
        let mut top = cuboid(v3(0.5, 0.5, 0.5));
        top.translate(v3(0.0, 3.0, 0.0));
        m.append(&top);
        let f = floor_footprints(&m, 0.3);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].len(), 4);
        let area: f32 = newell_area(&f[0]);
        assert!((area - 4.0).abs() < 1e-4);
    }

    fn newell_area(p: &[Vec3]) -> f32 {
        super::super::model::newell(p).length() * 0.5
    }
}
