use super::shapes::ShapeKind;
use pt_geom::math::*;
pub const MISS_DISTANCE: f32 = 10.0;
const TOUCH_SLACK: f32 = 1e-4;
pub fn default_part_size(kind: ShapeKind) -> Vec3 {
    match kind {
        ShapeKind::Box | ShapeKind::Wedge => v3(4.0, 1.0, 2.0),
        ShapeKind::CornerWedge => v3(2.0, 2.0, 2.0),
        ShapeKind::Cylinder => v3(2.0, 4.0, 2.0),
        _ => v3(4.0, 4.0, 4.0),
    }
}
pub fn boxes_overlap(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    a.0.x < b.1.x - TOUCH_SLACK
        && b.0.x < a.1.x - TOUCH_SLACK
        && a.0.y < b.1.y - TOUCH_SLACK
        && b.0.y < a.1.y - TOUCH_SLACK
        && a.0.z < b.1.z - TOUCH_SLACK
        && b.0.z < a.1.z - TOUCH_SLACK
}
pub fn lift_until_clear(part: (Vec3, Vec3), obstacles: &[(Vec3, Vec3)]) -> f32 {
    let mut lift = 0.0f32;
    for _ in 0..=obstacles.len() {
        let up = v3(0.0, lift, 0.0);
        let moved = (part.0 + up, part.1 + up);
        let top = obstacles
            .iter()
            .filter(|o| boxes_overlap(moved, **o))
            .map(|o| o.1.y)
            .fold(None, |m: Option<f32>, y| Some(m.map_or(y, |m| m.max(y))));
        match top {
            Some(y) => lift += y - moved.0.y,
            None => break,
        }
    }
    lift
}
pub fn snap_to_step(v: f32, step: f32) -> f32 {
    if step > 1e-6 {
        (v / step).round() * step
    } else {
        v
    }
}
pub fn place_part(
    ray_origin: Vec3,
    ray_dir: Vec3,
    hit: Option<Vec3>,
    extent: (Vec3, Vec3),
    obstacles: &[(Vec3, Vec3)],
    snap_step: Option<f32>,
) -> Vec3 {
    let mut at = hit.unwrap_or(ray_origin + ray_dir.unitized() * MISS_DISTANCE);
    if let Some(step) = snap_step {
        at.x = snap_to_step(at.x, step);
        at.z = snap_to_step(at.z, step);
    }
    at.y += lift_until_clear((extent.0 + at, extent.1 + at), obstacles);
    at
}
#[cfg(test)]
mod tests {
    use super::*;
    const BLOCK: (Vec3, Vec3) = (
        Vec3 {
            x: -2.0,
            y: 0.0,
            z: -1.0,
        },
        Vec3 {
            x: 2.0,
            y: 1.0,
            z: 1.0,
        },
    );
    #[test]
    fn a_miss_lands_ten_metres_ahead() {
        let at = place_part(v3(1.0, 5.0, 0.0), v3(0.0, 0.0, -2.0), None, BLOCK, &[], None);
        assert!((at - v3(1.0, 5.0, -10.0)).magnitude() < 1e-5, "{at:?}");
    }
    #[test]
    fn a_hit_on_the_ground_seats_the_base_there() {
        let at = place_part(
            v3(0.0, 10.0, 10.0),
            v3(0.0, -1.0, -1.0),
            Some(v3(0.3, 0.0, 0.6)),
            BLOCK,
            &[],
            Some(0.5),
        );
        assert!((at - v3(0.5, 0.0, 0.5)).magnitude() < 1e-5, "{at:?}");
    }
    #[test]
    fn a_hit_on_the_side_of_a_part_stacks_on_top() {
        let part = (v3(-2.0, 0.0, -1.0), v3(2.0, 1.0, 1.0));
        let at = place_part(
            v3(0.0, 0.5, 10.0),
            v3(0.0, 0.0, -1.0),
            Some(v3(0.0, 0.5, 1.0)),
            BLOCK,
            &[part],
            None,
        );
        assert!((at - v3(0.0, 1.0, 1.0)).magnitude() < 1e-5, "{at:?}");
    }
    #[test]
    fn inserting_twice_builds_a_stack() {
        let mut parts = Vec::new();
        for i in 0..3 {
            let at = place_part(
                v3(0.0, 10.0, 0.0),
                v3(0.0, -1.0, 0.0),
                Some(v3(0.0, if parts.is_empty() { 0.0 } else { 1.0 }, 0.0)),
                BLOCK,
                &parts,
                None,
            );
            assert!((at.y - i as f32).abs() < 1e-5, "{i}: {at:?}");
            parts.push((BLOCK.0 + at, BLOCK.1 + at));
        }
    }
    fn cube(lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
        (lo, hi)
    }
    #[test]
    fn studio_block_is_four_by_one_by_two() {
        assert_eq!(default_part_size(ShapeKind::Box), v3(4.0, 1.0, 2.0));
    }
    #[test]
    fn a_free_spot_is_not_lifted() {
        let part = cube(v3(0.0, 0.0, 0.0), v3(4.0, 1.0, 2.0));
        let far = cube(v3(10.0, 0.0, 0.0), v3(12.0, 5.0, 2.0));
        assert_eq!(lift_until_clear(part, &[far]), 0.0);
    }
    #[test]
    fn touching_counts_as_clear() {
        let part = cube(v3(0.0, 1.0, 0.0), v3(4.0, 2.0, 2.0));
        let below = cube(v3(0.0, 0.0, 0.0), v3(4.0, 1.0, 2.0));
        let beside = cube(v3(4.0, 1.0, 0.0), v3(6.0, 2.0, 2.0));
        assert_eq!(lift_until_clear(part, &[below, beside]), 0.0);
    }
    #[test]
    fn parts_stack_on_a_tower() {
        let part = cube(v3(0.0, 0.5, 0.0), v3(4.0, 1.5, 2.0));
        let tower = [
            cube(v3(0.0, 0.0, 0.0), v3(4.0, 1.0, 2.0)),
            cube(v3(1.0, 1.0, 0.5), v3(3.0, 3.0, 1.5)),
            cube(v3(-1.0, 3.0, -1.0), v3(5.0, 3.5, 3.0)),
        ];
        let lift = lift_until_clear(part, &tower);
        assert!((part.0.y + lift - 3.5).abs() < 1e-5, "{lift}");
    }
    #[test]
    fn lifting_always_stops() {
        let part = cube(v3(0.0, 0.0, 0.0), v3(1.0, 1.0, 1.0));
        let pile: Vec<_> = (0..50)
            .map(|i| cube(v3(0.0, i as f32, 0.0), v3(1.0, i as f32 + 1.0, 1.0)))
            .collect();
        assert!((lift_until_clear(part, &pile) - 50.0).abs() < 1e-4);
    }
}
