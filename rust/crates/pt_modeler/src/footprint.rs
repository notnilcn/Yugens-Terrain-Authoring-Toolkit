use super::model::BMesh;
use pt_geom::math::*;
pub fn floor_footprints(m: &BMesh, band: f32) -> Vec<Vec<Vec3>> {
    let Some((lo, _)) = m.extent() else {
        return Vec::new();
    };
    (0..m.facets.len())
        .filter(|f| m.face_normal(*f).y < -0.7205)
        .map(|f| m.face_points(f))
        .filter(|pts| {
            pts.len() >= 3 && pts.iter().map(|p| p.y).fold(f32::MAX, f32::min) <= lo.y + band
        })
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
        top.displace(v3(0.0, 3.0, 0.0));
        m.append_tail(&top);
        let f = floor_footprints(&m, 0.3);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].len(), 4);
        let area: f32 = newell_area(&f[0]);
        assert!((area - 4.0).abs() < 1e-4);
    }
    fn newell_area(p: &[Vec3]) -> f32 {
        super::super::model::newell(p).magnitude() * 0.5
    }
}
