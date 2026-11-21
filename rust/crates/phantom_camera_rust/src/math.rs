use godot::classes::Tween;
use godot::classes::tween::{EaseType, TransitionType};
use godot::prelude::*;
pub fn smooth_damp(
    target: f64,
    current: f64,
    current_velocity: f64,
    damping_time: f64,
    delta: f64,
) -> (f64, f64) {
    let damping_time = damping_time.max(0.0005);
    if !delta.is_finite() || delta <= 0.0 {
        return (current, current_velocity);
    }
    let omega = 2.0 / damping_time;
    let x = omega * delta;
    let exponential = 1.0 / (1.0 + x + 0.4805 * x * x + 0.2355 * x * x * x);
    let mut diff = current - target;
    if !diff.is_finite() {
        let max_change = f64::INFINITY * damping_time;
        diff = diff.clamp(-max_change, max_change);
    }
    let effective_target = current - diff;
    let temp = (current_velocity + omega * diff) * delta;
    let mut new_velocity = (current_velocity - omega * temp) * exponential;
    let mut output = effective_target + (diff + temp) * exponential;
    if (target - current > 0.0) == (output > target) {
        output = target;
        new_velocity = 0.0;
    }
    (output, new_velocity)
}
fn transition_of(ord: i64) -> TransitionType {
    i32::try_from(ord)
        .ok()
        .and_then(TransitionType::try_from_ord)
        .unwrap_or(TransitionType::LINEAR)
}
fn ease_of(ord: i64) -> EaseType {
    i32::try_from(ord)
        .ok()
        .and_then(EaseType::try_from_ord)
        .unwrap_or(EaseType::IN_OUT)
}
fn interpolate_variant(
    from: &Variant,
    delta: &Variant,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Option<Variant> {
    if !duration.is_finite() || duration <= 0.0 {
        return None;
    }
    Some(Tween::interpolate_value(
        from,
        delta,
        elapsed,
        duration,
        transition_of(transition),
        ease_of(ease),
    ))
}
pub fn interpolate_value_f64(
    from: f64,
    delta: f64,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> f64 {
    interpolate_variant(
        &Variant::from(from),
        &Variant::from(delta),
        elapsed,
        duration,
        transition,
        ease,
    )
    .and_then(|value| value.try_to::<f64>().ok())
    .unwrap_or(from + delta)
}
pub fn interpolate_value_v2(
    from: Vector2,
    delta: Vector2,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Vector2 {
    interpolate_variant(
        &from.to_variant(),
        &delta.to_variant(),
        elapsed,
        duration,
        transition,
        ease,
    )
    .and_then(|value| value.try_to::<Vector2>().ok())
    .unwrap_or(from + delta)
}
pub fn interpolate_value_v3(
    from: Vector3,
    delta: Vector3,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Vector3 {
    interpolate_variant(
        &from.to_variant(),
        &delta.to_variant(),
        elapsed,
        duration,
        transition,
        ease,
    )
    .and_then(|value| value.try_to::<Vector3>().ok())
    .unwrap_or(from + delta)
}
#[allow(dead_code)]
pub fn interpolate_value_zoom(
    from: Vector2,
    to: Vector2,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Vector2 {
    interpolate_value_v2(from, to - from, elapsed, duration, transition, ease)
}
pub fn interpolate_value_quat(
    from: Quaternion,
    delta: Quaternion,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Quaternion {
    interpolate_variant(
        &from.to_variant(),
        &delta.to_variant(),
        elapsed,
        duration,
        transition,
        ease,
    )
    .and_then(|value| value.try_to::<Quaternion>().ok())
    .unwrap_or(from)
}
#[cfg(test)]
mod tests {
    use super::smooth_damp;
    #[test]
    fn settled_value_is_stable() {
        let (output, velocity) = smooth_damp(10.0, 10.0, 0.0, 0.1, 1.0 / 60.0);
        assert!((output - 10.0).abs() < 1e-12, "output {output}");
        assert!(velocity.abs() < 1e-12, "velocity {velocity}");
    }
    #[test]
    fn converges_toward_target() {
        let mut current = 0.0_f64;
        let mut velocity = 0.0_f64;
        for _ in 0..600 {
            let (next, v) = smooth_damp(100.0, current, velocity, 0.5, 1.0 / 60.0);
            current = next;
            velocity = v;
        }
        assert!((current - 100.0).abs() < 1.0, "current {current}");
        assert!(velocity.abs() < 10.0, "velocity {velocity}");
    }
    #[test]
    fn snap_lands_on_target() {
        let (output, velocity) = smooth_damp(1.0, 0.0, 25_000.0, 0.0001, 1.0);
        assert!((output - 1.0).abs() < 1e-12, "output {output}");
        assert!(velocity.abs() < 1e-12, "velocity {velocity}");
    }
    #[test]
    fn damping_time_is_floored() {
        let (output, velocity) = smooth_damp(5.0, 0.0, 0.0, 0.0, 1.0 / 60.0);
        assert!(output.is_finite(), "output {output}");
        assert!(velocity.is_finite(), "velocity {velocity}");
    }
    #[test]
    fn zero_delta_is_inert() {
        let (output, velocity) = smooth_damp(5.0, 1.0, 0.5, 0.1, 0.0);
        assert!((output - 1.0).abs() < 1e-12);
        assert!((velocity - 0.5).abs() < 1e-12);
    }
}
