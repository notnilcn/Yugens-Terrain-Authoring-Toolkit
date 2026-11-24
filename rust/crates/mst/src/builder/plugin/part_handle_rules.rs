//! The rules behind the whole-part handles, as plain functions over plain values, so how a part grows,
//! turns, snaps, stops at a neighbour or keeps its pivot can be tested without a running editor.
use godot::builtin::{Basis, Transform3D, Vector2, Vector3};
use pt_modeler::BMesh;
use pt_core::math::Vec3 as PVec3;
pub(super) const MIN_EXTENT: f32 = 0.05;
pub(super) type WorldBox = (Vector3, Vector3);
pub(super) fn grown_factors(world_extent: Vector3, deltas: Vector3, symmetric: bool) -> Vector3 {
    let k = if symmetric { 2.0 } else { 1.0 };
    let one = |e: f32, d: f32| {
        if e < 1e-6 || d == 0.0 {
            1.0
        } else {
            (e + d * k).max(MIN_EXTENT) / e
        }
    };
    Vector3::new(
        one(world_extent.x, deltas.x),
        one(world_extent.y, deltas.y),
        one(world_extent.z, deltas.z),
    )
}
pub(super) fn anchor_point(lo: Vector3, hi: Vector3, sides: [i8; 3], symmetric: bool) -> Vector3 {
    let pick = |l: f32, h: f32, s: i8| match s {
        _ if symmetric => (l + h) * 0.5,
        1 => l,
        -1 => h,
        _ => (l + h) * 0.5,
    };
    Vector3::new(
        pick(lo.x, hi.x, sides[0]),
        pick(lo.y, hi.y, sides[1]),
        pick(lo.z, hi.z, sides[2]),
    )
}
pub(super) fn node_scaled(xf: Transform3D, factors: Vector3, anchor: Vector3) -> Transform3D {
    let b = Basis::from_cols(
        xf.basis.col_a() * factors.x,
        xf.basis.col_b() * factors.y,
        xf.basis.col_c() * factors.z,
    );
    Transform3D::new(b, xf.origin + xf.basis * anchor - b * anchor)
}
pub(super) fn regenerated_origin(
    xf: Transform3D,
    anchor_before: Vector3,
    anchor_after: Vector3,
) -> Transform3D {
    Transform3D::new(
        xf.basis,
        xf.origin + xf.basis * (anchor_before - anchor_after),
    )
}
pub(super) fn axis_lengths(b: Basis) -> Vector3 {
    Vector3::new(b.col_a().length(), b.col_b().length(), b.col_c().length())
}
pub(super) fn face_point(lo: Vector3, hi: Vector3, axis: usize, side: i8) -> Vector3 {
    let mut l = (lo + hi) * 0.5;
    let v = if side > 0 { hi } else { lo };
    match axis {
        0 => l.x = v.x,
        1 => l.y = v.y,
        _ => l.z = v.z,
    }
    l
}
pub(super) fn corner_point(lo: Vector3, hi: Vector3, sides: [i8; 3]) -> Vector3 {
    Vector3::new(
        if sides[0] > 0 { hi.x } else { lo.x },
        if sides[1] > 0 { hi.y } else { lo.y },
        if sides[2] > 0 { hi.z } else { lo.z },
    )
}
pub(super) fn corner_sides(i: usize) -> [i8; 3] {
    [
        if i & 1 == 0 { -1 } else { 1 },
        if i & 2 == 0 { -1 } else { 1 },
        if i & 4 == 0 { -1 } else { 1 },
    ]
}
pub(super) fn box_corners(xf: Transform3D, lo: Vector3, hi: Vector3) -> [Vector3; 8] {
    std::array::from_fn(|i| xf * corner_point(lo, hi, corner_sides(i)))
}
pub(super) fn world_box(xf: Transform3D, lo: Vector3, hi: Vector3) -> WorldBox {
    box_corners(xf, lo, hi).iter().fold(
        (
            Vector3::splat(f32::INFINITY),
            Vector3::splat(f32::NEG_INFINITY),
        ),
        |(a, b), c| (a.coord_min(*c), b.coord_max(*c)),
    )
}
pub(super) fn half_extent_along(xf: Transform3D, lo: Vector3, hi: Vector3, dir: Vector3) -> f32 {
    let h = (hi - lo) * 0.5;
    dir.dot(xf.basis.col_a()).abs() * h.x
        + dir.dot(xf.basis.col_b()).abs() * h.y
        + dir.dot(xf.basis.col_c()).abs() * h.z
}
pub(super) fn snapped(v: f32, step: Option<f32>) -> f32 {
    match step {
        Some(s) if s > 1e-6 => (v / s).round() * s,
        _ => v,
    }
}
pub(super) fn snapped_to_grid_along(at: f32, raw: f32, step: f32) -> f32 {
    snapped(at + raw, Some(step)) - at
}
pub(super) fn move_step(raw: f32, box_low_along: f32, step: Option<f32>, use_boxes: bool) -> f32 {
    match step {
        Some(s) if use_boxes && s > 1e-6 => snapped_to_grid_along(box_low_along, raw, s),
        _ => snapped(raw, step),
    }
}
pub(super) fn wrap_angle(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}
pub(super) fn snapped_angle(raw: f32, step_deg: f32) -> f32 {
    if step_deg > 1e-3 {
        snapped(raw, Some(step_deg.to_radians()))
    } else {
        raw
    }
}
pub(super) fn turned_about(
    xf: Transform3D,
    axis: Vector3,
    angle: f32,
    pivot: Vector3,
) -> Transform3D {
    let r = Basis::from_axis_angle(axis, angle);
    Transform3D::new(r * xf.basis, pivot + r * (xf.origin - pivot))
}
const GRAZING: f32 = 0.25;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DragTurn {
    Spin,
    Tilt,
}
pub(super) fn drag_turn_key(moving: bool, letter: Option<char>, modified: bool) -> Option<DragTurn> {
    if !moving || modified {
        return None;
    }
    match letter {
        Some('r') => Some(DragTurn::Spin),
        Some('t') => Some(DragTurn::Tilt),
        _ => None,
    }
}
pub(super) fn quarter_turn_axis(turn: DragTurn, camera_right: Vector3) -> Vector3 {
    match turn {
        DragTurn::Spin => Vector3::UP,
        DragTurn::Tilt if camera_right.x.abs() >= camera_right.z.abs() => {
            Vector3::RIGHT * camera_right.x.signum()
        }
        DragTurn::Tilt => Vector3::BACK * camera_right.z.signum(),
    }
}
pub(super) fn quarter_turned(turn: Basis, axis: Vector3) -> Basis {
    let b = Basis::from_axis_angle(axis, std::f32::consts::FRAC_PI_2) * turn;
    let r = |v: Vector3| Vector3::new(v.x.round(), v.y.round(), v.z.round());
    Basis::from_cols(r(b.col_a()), r(b.col_b()), r(b.col_c()))
}
pub(super) fn dragged(start: Transform3D, centre: Vector3, turn: Basis, delta: Vector3) -> Transform3D {
    Transform3D::new(
        turn * start.basis,
        centre + turn * (start.origin - centre) + delta,
    )
}
pub(super) fn ground_drag_normal(ray_dir: Vector3, toward_camera: Vector3) -> Vector3 {
    if ray_dir.y.abs() >= GRAZING {
        return Vector3::UP;
    }
    let flat = Vector3::new(toward_camera.x, 0.0, toward_camera.z).normalized_or_zero();
    if flat == Vector3::ZERO {
        Vector3::UP
    } else {
        flat
    }
}
pub(super) fn pivot_compensation(old: Transform3D, new: Transform3D) -> Transform3D {
    new.affine_inverse() * old
}
const OVERLAP_EPS: f32 = 1e-3;
const SWEEP_STEP: f32 = 0.02;
const MAX_SWEEP_STEPS: usize = 4096;
pub(super) fn overlaps(a: WorldBox, b: WorldBox) -> bool {
    a.0.x < b.1.x - OVERLAP_EPS
        && b.0.x < a.1.x - OVERLAP_EPS
        && a.0.y < b.1.y - OVERLAP_EPS
        && b.0.y < a.1.y - OVERLAP_EPS
        && a.0.z < b.1.z - OVERLAP_EPS
        && b.0.z < a.1.z - OVERLAP_EPS
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PointerAfterCancel {
    Motion,
    LeftRelease,
    Other,
}
pub(super) fn swallow_after_cancel(swallowing: bool, event: PointerAfterCancel) -> (bool, bool) {
    match (swallowing, event) {
        (false, _) => (false, false),
        (true, PointerAfterCancel::Motion) => (true, true),
        (true, PointerAfterCancel::LeftRelease) => (true, false),
        (true, PointerAfterCancel::Other) => (false, false),
    }
}
pub(super) fn snap_axes_for(drag_axis: Option<Vector3>) -> [bool; 3] {
    match drag_axis {
        None => [true, false, true],
        Some(a) => {
            let a = a.normalized_or_zero();
            [a.x.abs() > AXIS_ALIGNED, a.y.abs() > AXIS_ALIGNED, a.z.abs() > AXIS_ALIGNED]
        }
    }
}
pub(super) fn snap_to_neighbours(
    start: WorldBox,
    neighbours: &[WorldBox],
    stepped: Vector3,
    unstepped: Vector3,
    free_axes: [bool; 3],
    reach: f32,
) -> Vector3 {
    let arr = |v: Vector3| [v.x, v.y, v.z];
    let lo = arr(start.0 + unstepped);
    let hi = arr(start.1 + unstepped);
    let raw = arr(unstepped);
    let boxes: Vec<([f32; 3], [f32; 3])> = neighbours.iter().map(|n| (arr(n.0), arr(n.1))).collect();
    let mut out = arr(stepped);
    for i in 0..3 {
        if !free_axes[i] {
            continue;
        }
        let side_by_side = |n: &&([f32; 3], [f32; 3])| {
            (0..3).filter(|j| *j != i).all(|j| {
                lo[j] <= n.1[j] + OVERLAP_EPS && n.0[j] <= hi[j] + OVERLAP_EPS
            })
        };
        let best = boxes
            .iter()
            .filter(|n| side_by_side(n))
            .flat_map(|n| [n.0[i] - hi[i], n.1[i] - lo[i], n.0[i] - lo[i], n.1[i] - hi[i]])
            .filter(|c| c.abs() <= reach)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()));
        if let Some(c) = best {
            out[i] = raw[i] + c;
        }
    }
    Vector3::new(out[0], out[1], out[2])
}
pub(super) fn collision_stop(
    start: WorldBox,
    obstacles: &[WorldBox],
    last_clear: Vector3,
    wanted: Vector3,
) -> Vector3 {
    let shifted = |d: Vector3| (start.0 + d, start.1 + d);
    let fresh: Vec<&WorldBox> = obstacles.iter().filter(|o| !overlaps(start, **o)).collect();
    let blocked = |d: Vector3| fresh.iter().any(|o| overlaps(shifted(d), **o));
    let n = ((wanted - last_clear).length() / SWEEP_STEP)
        .ceil()
        .clamp(1.0, MAX_SWEEP_STEPS as f32) as usize;
    let mut ok = last_clear;
    for k in 1..=n {
        let c = last_clear + (wanted - last_clear) * (k as f32 / n as f32);
        if blocked(c) {
            return ok;
        }
        ok = c;
    }
    wanted
}
const RECIPE_MATCH: f32 = 1e-3;
const AXIS_ALIGNED: f32 = 0.999;
pub(super) fn recipe_offset(cage: &BMesh, generated: &BMesh) -> Option<PVec3> {
    if generated.positions.is_empty()
        || generated.positions.len() != cage.positions.len()
        || generated.facets.len() != cage.facets.len()
    {
        return None;
    }
    let offset = cage.positions[0] - generated.positions[0];
    let same_places = generated
        .positions
        .iter()
        .zip(&cage.positions)
        .all(|(g, c)| (*g + offset - *c).magnitude() < RECIPE_MATCH);
    let same_faces = generated.facets.iter().zip(&cage.facets).all(|(g, c)| {
        g.verts == c.verts
            && g.glide == c.glide
            && g.uv_manual == c.uv_manual
            && g.uv_coords == c.uv_coords
            && g.color_corners == c.color_corners
    });
    (same_places && same_faces).then_some(offset)
}
pub(super) fn is_user_lock(has_edit_lock: bool, has_plugin_lock: bool) -> bool {
    has_edit_lock && !has_plugin_lock
}
pub(super) fn lock_wanted(user_locks: &[bool]) -> bool {
    !user_locks.iter().all(|l| *l)
}
pub(super) fn ray_box(o: Vector3, d: Vector3, lo: Vector3, hi: Vector3) -> Option<f32> {
    let mut t0 = 0.0f32;
    let mut t1 = f32::INFINITY;
    for (oa, da, l, h) in [
        (o.x, d.x, lo.x, hi.x),
        (o.y, d.y, lo.y, hi.y),
        (o.z, d.z, lo.z, hi.z),
    ] {
        if da.abs() < 1e-9 {
            if oa < l || oa > h {
                return None;
            }
            continue;
        }
        let (a, b) = ((l - oa) / da, (h - oa) / da);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0 <= t1).then_some(t0)
}
pub(super) fn segment_distance(p: Vector2, a: Vector2, b: Vector2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_squared();
    let q = if l2 < 1e-12 {
        a
    } else {
        a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
    };
    q.distance_to(p)
}
pub(super) fn polyline_distance(p: Vector2, pts: &[Vector2]) -> f32 {
    match pts {
        [] => f32::INFINITY,
        [one] => one.distance_to(p),
        _ => pts
            .windows(2)
            .map(|w| segment_distance(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min),
    }
}
pub(super) fn nearest_within<T: Copy>(cands: &[(T, u8, f32)], reach: f32) -> Option<T> {
    cands
        .iter()
        .filter(|c| c.2 <= reach)
        .min_by(|a, b| {
            (a.1, a.2)
                .partial_cmp(&(b.1, b.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| c.0)
}
#[cfg(test)]
mod tests {
    #[test]
    fn bounding_box_snap_lands_the_low_edge_on_the_grid() {
        use super::move_step;
        assert!((move_step(1.1, 2.3, Some(1.0), false) - 1.0).abs() < 1e-5);
        assert!((move_step(1.1, 2.3, Some(1.0), true) - 0.7).abs() < 1e-5);
        assert!((move_step(1.1, 2.3, None, true) - 1.1).abs() < 1e-5);
        assert!((move_step(-0.2, 2.3, Some(1.0), true) + 0.3).abs() < 1e-5);
    }
    use super::*;
    fn near(a: Vector3, b: Vector3) -> bool {
        (a - b).length() < 1e-4
    }
    #[test]
    fn one_sided_scale_keeps_the_opposite_face() {
        let (lo, hi) = (Vector3::new(-1.0, 0.0, -0.5), Vector3::new(1.0, 1.0, 0.5));
        let xf = Transform3D::new(Basis::IDENTITY, Vector3::new(3.0, 0.0, 0.0));
        let f = grown_factors(hi - lo, Vector3::new(2.0, 0.0, 0.0), false);
        assert!(near(f, Vector3::new(2.0, 1.0, 1.0)));
        let out = node_scaled(xf, f, anchor_point(lo, hi, [1, 0, 0], false));
        assert!(near(out * lo, xf * lo), "the -X face stays");
        assert!(
            near(out * hi, Vector3::new(6.0, 1.0, 0.5)),
            "the +X face moves by 2"
        );
    }
    #[test]
    fn negative_side_keeps_the_positive_face() {
        let (lo, hi) = (Vector3::new(-1.0, 0.0, -1.0), Vector3::new(1.0, 2.0, 1.0));
        let f = grown_factors(hi - lo, Vector3::new(0.0, 0.0, 1.0), false);
        let out = node_scaled(
            Transform3D::IDENTITY,
            f,
            anchor_point(lo, hi, [0, 0, -1], false),
        );
        assert!(near(out * hi, hi));
        assert!(near(out * lo, Vector3::new(-1.0, 0.0, -2.0)));
    }
    #[test]
    fn symmetric_keeps_the_centre() {
        let (lo, hi) = (Vector3::new(-1.0, 0.0, -1.0), Vector3::new(1.0, 2.0, 1.0));
        let f = grown_factors(hi - lo, Vector3::new(1.0, 0.0, 0.0), true);
        assert!(near(f, Vector3::new(2.0, 1.0, 1.0)));
        let out = node_scaled(
            Transform3D::IDENTITY,
            f,
            anchor_point(lo, hi, [1, 0, 0], true),
        );
        assert!(near(out * ((lo + hi) * 0.5), (lo + hi) * 0.5));
        assert!(near(out * hi, Vector3::new(2.0, 2.0, 1.0)));
    }
    #[test]
    fn regenerated_mesh_keeps_the_anchor_in_world() {
        let xf = Transform3D::new(
            Basis::from_axis_angle(Vector3::UP, 0.7),
            Vector3::new(1.0, 2.0, 3.0),
        );
        let (before, after) = (Vector3::new(-1.0, 0.5, 0.0), Vector3::new(-3.0, 0.5, 0.0));
        assert!(near(
            regenerated_origin(xf, before, after) * after,
            xf * before
        ));
    }
    #[test]
    fn shrinking_stops_at_the_minimum() {
        let f = grown_factors(Vector3::ONE, Vector3::new(-5.0, 0.0, 0.0), false);
        assert!((f.x - MIN_EXTENT).abs() < 1e-6);
    }
    #[test]
    fn corners_and_faces_follow_the_sides() {
        let (lo, hi) = (Vector3::new(-1.0, 0.0, -2.0), Vector3::new(1.0, 3.0, 2.0));
        assert!(near(face_point(lo, hi, 1, 1), Vector3::new(0.0, 3.0, 0.0)));
        assert!(near(
            face_point(lo, hi, 2, -1),
            Vector3::new(0.0, 1.5, -2.0)
        ));
        assert!(near(corner_point(lo, hi, corner_sides(7)), hi));
        assert!(near(corner_point(lo, hi, corner_sides(0)), lo));
        let (a, b) = world_box(Transform3D::IDENTITY, lo, hi);
        assert!(near(a, lo) && near(b, hi));
    }
    #[test]
    fn snapping_rounds_to_the_step() {
        assert_eq!(snapped(1.4, Some(1.0)), 1.0);
        assert_eq!(snapped(1.6, Some(1.0)), 2.0);
        assert_eq!(snapped(1.6, None), 1.6);
        assert!((snapped_to_grid_along(0.3, 1.0, 1.0) - 0.7).abs() < 1e-6);
        assert!((snapped_angle(0.5, 45.0) - 45f32.to_radians()).abs() < 1e-6);
        assert_eq!(snapped_angle(0.5, 0.0), 0.5);
        assert!((wrap_angle(3.5 * std::f32::consts::PI) + 0.5 * std::f32::consts::PI).abs() < 1e-5);
    }
    #[test]
    fn turning_keeps_the_pivot() {
        let xf = Transform3D::new(Basis::IDENTITY, Vector3::new(2.0, 0.0, 0.0));
        let out = turned_about(xf, Vector3::UP, std::f32::consts::FRAC_PI_2, Vector3::ZERO);
        assert!(near(out.origin, Vector3::new(0.0, 0.0, -2.0)));
    }
    #[test]
    fn r_and_t_turn_only_a_plain_key_during_a_move() {
        assert_eq!(drag_turn_key(true, Some('r'), false), Some(DragTurn::Spin));
        assert_eq!(drag_turn_key(true, Some('t'), false), Some(DragTurn::Tilt));
        assert_eq!(drag_turn_key(true, Some('r'), true), None);
        assert_eq!(drag_turn_key(false, Some('r'), false), None);
        assert_eq!(drag_turn_key(true, Some('e'), false), None);
        assert_eq!(drag_turn_key(true, None, false), None);
    }
    #[test]
    fn tilt_uses_the_world_axis_nearest_the_camera_right() {
        let spin = quarter_turn_axis(DragTurn::Spin, Vector3::new(0.3, 0.0, 0.9));
        assert_eq!(spin, Vector3::UP);
        let facing_z = quarter_turn_axis(DragTurn::Tilt, Vector3::new(0.9, 0.1, 0.3));
        assert_eq!(facing_z, Vector3::RIGHT);
        let facing_x = quarter_turn_axis(DragTurn::Tilt, Vector3::new(0.2, 0.0, -0.98));
        assert_eq!(facing_x, -Vector3::BACK);
    }
    #[test]
    fn four_quarter_turns_come_back_exactly() {
        let mut b = Basis::IDENTITY;
        for _ in 0..4 {
            b = quarter_turned(b, Vector3::UP);
        }
        assert_eq!(b, Basis::IDENTITY);
        let once = quarter_turned(Basis::IDENTITY, Vector3::UP);
        assert_eq!(once * Vector3::RIGHT, Vector3::new(0.0, 0.0, -1.0));
        let tilted = quarter_turned(once, Vector3::RIGHT);
        assert_eq!(tilted * Vector3::UP, Vector3::new(0.0, 0.0, 1.0));
    }
    #[test]
    fn a_turned_drag_spins_about_the_box_centre_then_moves() {
        let start = Transform3D::new(Basis::IDENTITY, Vector3::new(2.0, 0.0, 0.0));
        let turn = quarter_turned(Basis::IDENTITY, Vector3::UP);
        let out = dragged(start, Vector3::new(1.0, 0.0, 0.0), turn, Vector3::new(0.0, 3.0, 0.0));
        assert!(near(out.origin, Vector3::new(1.0, 3.0, -1.0)));
        assert_eq!(out.basis, turn);
        let still = dragged(start, Vector3::ZERO, Basis::IDENTITY, Vector3::ZERO);
        assert_eq!(still, start);
    }
    #[test]
    fn pivot_compensation_keeps_the_geometry() {
        let old = Transform3D::new(Basis::IDENTITY, Vector3::new(1.0, 0.0, 0.0));
        let new = Transform3D::new(
            Basis::from_axis_angle(Vector3::UP, 0.4),
            Vector3::new(2.0, 1.0, 0.0),
        );
        let m = pivot_compensation(old, new);
        let p = Vector3::new(0.3, 0.7, -0.2);
        assert!(near(new * (m * p), old * p));
    }
    #[test]
    fn collision_stops_before_the_first_overlap() {
        let start = (Vector3::ZERO, Vector3::ONE);
        let wall = (Vector3::new(3.0, 0.0, 0.0), Vector3::new(4.0, 1.0, 1.0));
        let d = collision_stop(start, &[wall], Vector3::ZERO, Vector3::new(5.0, 0.0, 0.0));
        assert!(d.x <= 2.0 + 1e-4 && d.x > 1.8, "{d:?}");
        let free = collision_stop(start, &[wall], Vector3::ZERO, Vector3::new(0.0, 0.0, 5.0));
        assert!(near(free, Vector3::new(0.0, 0.0, 5.0)));
        let inside = (Vector3::new(0.5, 0.0, 0.0), Vector3::new(1.5, 1.0, 1.0));
        let out = collision_stop(start, &[inside], Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
        assert!(
            near(out, Vector3::new(1.0, 0.0, 0.0)),
            "an overlap present at the start never blocks"
        );
    }
    #[test]
    fn ground_drag_avoids_grazing_planes() {
        let steep = Vector3::new(0.0, -0.8, -0.6).normalized();
        assert_eq!(ground_drag_normal(steep, Vector3::BACK), Vector3::UP);
        let grazing = Vector3::new(0.0, -0.05, -1.0).normalized();
        let n = ground_drag_normal(grazing, Vector3::new(0.0, 0.2, 1.0));
        assert!(n.y.abs() < 1e-6 && n.z > 0.99, "{n:?}");
    }
    #[test]
    fn recipe_regenerates_only_an_untouched_cage() {
        let g = pt_modeler::shapes::cuboid(pt_core::math::v3(4.0, 1.0, 2.0));
        let mut moved = g.clone();
        moved.displace(pt_core::math::v3(0.0, -0.5, 0.0));
        let off = recipe_offset(&moved, &g).expect("a pivot move keeps the recipe");
        assert!((off.y + 0.5).abs() < 1e-6);
        let mut painted = g.clone();
        painted.facets[0].stuff = 3;
        painted.facets[0].tint = [9, 9, 9, 255];
        assert!(
            recipe_offset(&painted, &g).is_some(),
            "material and tint are carried over"
        );
        let mut corners = g.clone();
        let n = corners.facets[2].verts.len();
        corners.facets[2].color_corners = vec![[255, 0, 0, 255]; n];
        assert!(
            recipe_offset(&corners, &g).is_none(),
            "vertex colours would be lost"
        );
        let mut smooth = g.clone();
        smooth.facets[1].glide ^= 1;
        assert!(
            recipe_offset(&smooth, &g).is_none(),
            "smoothing would be lost"
        );
        let mut uv = g.clone();
        uv.facets[0].uv_manual = true;
        assert!(recipe_offset(&uv, &g).is_none(), "manual UVs would be lost");
        let mut bent = g.clone();
        bent.positions[0].x += 0.3;
        assert!(
            recipe_offset(&bent, &g).is_none(),
            "moved vertices would be lost"
        );
    }
    #[test]
    fn lock_rules() {
        assert!(is_user_lock(true, false));
        assert!(
            !is_user_lock(true, true),
            "the builder's own lock is not a user lock"
        );
        assert!(!is_user_lock(false, false));
        assert!(lock_wanted(&[true, false]));
        assert!(!lock_wanted(&[true, true]));
    }
    #[test]
    fn rays_and_picking() {
        let t = ray_box(
            Vector3::new(-5.0, 0.5, 0.5),
            Vector3::RIGHT,
            Vector3::ZERO,
            Vector3::ONE,
        );
        assert_eq!(t, Some(5.0));
        assert_eq!(
            ray_box(
                Vector3::new(-5.0, 5.0, 0.5),
                Vector3::RIGHT,
                Vector3::ZERO,
                Vector3::ONE
            ),
            None
        );
        let p = Vector2::new(5.0, 1.0);
        assert!(
            (polyline_distance(p, &[Vector2::ZERO, Vector2::new(10.0, 0.0)]) - 1.0).abs() < 1e-6
        );
        let cands = [("ring", 2u8, 1.0f32), ("arrow", 1, 6.0), ("ball", 0, 20.0)];
        assert_eq!(nearest_within(&cands, 10.0), Some("arrow"));
        assert_eq!(nearest_within(&cands, 0.5), None);
    }
    #[test]
    fn snap_to_parts_lands_the_moved_face_on_a_neighbour_within_reach() {
        let start = (Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
        let wall = (Vector3::new(3.2, 0.0, 0.0), Vector3::new(5.0, 1.0, 1.0));
        let x = snap_axes_for(Some(Vector3::RIGHT));
        assert_eq!(x, [true, false, false]);
        let two = Vector3::new(2.0, 0.0, 0.0);
        let d = snap_to_neighbours(start, &[wall], two, two, x, 0.5);
        assert!(near(d, Vector3::new(2.2, 0.0, 0.0)), "high face meets the wall: {d:?}");
        let one = Vector3::new(1.0, 0.0, 0.0);
        let far = snap_to_neighbours(start, &[wall], one, one, x, 0.5);
        assert!(near(far, one), "out of reach keeps the step");
        let gridded = snap_to_neighbours(start, &[wall], Vector3::new(3.0, 0.0, 0.0), Vector3::new(2.6, 0.0, 0.0), x, 0.5);
        assert!(near(gridded, Vector3::new(2.2, 0.0, 0.0)), "near a face the part wins over the grid: {gridded:?}");
        let off = snap_to_neighbours(start, &[wall], Vector3::new(1.0, 0.0, 0.0), Vector3::new(1.4, 0.0, 0.0), x, 0.5);
        assert!(near(off, one), "away from a face the grid step stays: {off:?}");
        let aside = (Vector3::new(3.2, 5.0, 0.0), Vector3::new(4.0, 6.0, 1.0));
        let d = snap_to_neighbours(start, &[aside], two, two, x, 0.5);
        assert!(near(d, Vector3::new(2.0, 0.0, 0.0)), "a part above the path is not a neighbour");
        let tilted = snap_axes_for(Some(Vector3::new(1.0, 0.0, 1.0)));
        assert_eq!(tilted, [false, false, false], "an off-axis arrow stays on its axis");
        assert_eq!(snap_axes_for(None), [true, false, true], "ground drag snaps across the ground");
    }
    #[test]
    fn a_cancelled_drag_keeps_the_pointer_until_the_button_comes_up() {
        use PointerAfterCancel::*;
        assert_eq!(swallow_after_cancel(true, Motion), (true, true));
        assert_eq!(swallow_after_cancel(true, LeftRelease), (true, false), "the release ends it");
        assert_eq!(swallow_after_cancel(true, Other), (false, false));
        assert_eq!(swallow_after_cancel(false, Motion), (false, false));
    }
}
