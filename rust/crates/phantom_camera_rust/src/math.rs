// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/**
//
//! Tween/easing math for the Phantom Camera port.
//!
//! Two independent pieces:
//!
//! * [`smooth_damp`] — the oracle's own exponential smoother, ported verbatim from
//!   `phantom_camera_2d.gd:1044-1065` (byte-identical body at
//!   `phantom_camera_3d.gd:1294-1315`). Used by the follow/rotation/lookahead
//!   damping paths.
//! * [`interpolate_value_f64`] and friends — Godot's `Tween.interpolate_value`,
//!   which the oracle calls for every tweened value
//!   (`phantom_camera_host.gd:1221-1229`) and for the 3D quaternion slerp
//!   (`:1009-1018`).
//!
//! The interpolators call the engine's own `Tween::interpolate_value` rather than
//! reimplementing the 11×4 Penner easing table, so the ported tween math is
//! identical to the oracle by construction. That matters because the
//! `TransitionType`/`EaseType` integers are Godot's own ordinals and are passed
//! straight through (`tween_resource.gd:11-32`).

use godot::classes::Tween;
use godot::classes::tween::{EaseType, TransitionType};
use godot::prelude::*;

/// The oracle's `_smooth_damp` (`phantom_camera_2d.gd:1044-1065`).
///
/// Returns `(output, new_velocity)`; the caller stores `new_velocity` back into
/// its per-axis velocity field, exactly as the oracle's `set_velocity` callable
/// does. Argument order follows the oracle: `target` is the *desired* value,
/// `current` is the existing one.
///
/// Oracle body:
/// ```text
/// damping_time = maxf(0.0001, damping_time)
/// omega = 2.0 / damping_time
/// x = omega * delta
/// exp = 1 / (1 + x + 0.48x² + 0.235x³)
/// diff = clampf(self_axis - target_axis, -INF*damping_time, INF*damping_time)
/// target_axis = self_axis - diff
/// temp = (current_velocity + omega*diff) * delta
/// set_velocity((current_velocity - omega*temp) * exp)
/// output = target_axis + (diff + temp) * exp
/// if (target_axis_cached - self_axis > 0) == (output > target_axis_cached):
///     output = target_axis_cached
///     set_velocity((output - target_axis_cached) / delta)
/// ```
///
/// The oracle's `INF * damping_time` clamp only bites for non-finite input, so it
/// is implemented as a finiteness guard rather than a real clamp.
///
/// `// parity:` a `delta` of `0` would make the oracle's snap branch divide by
/// zero; this port returns the inputs unchanged instead. Unreachable in practice
/// (Godot never calls `_process` with a zero delta), and it keeps the no-panic
/// invariant.
pub fn smooth_damp(
    target: f64,
    current: f64,
    current_velocity: f64,
    damping_time: f64,
    delta: f64,
) -> (f64, f64) {
    // `maxf(0.0001, damping_time)`.
    let damping_time = damping_time.max(0.0001);

    // Not strictly positive, or not finite: return the inputs unchanged. `!(delta
    // > 0.0)` held for NaN and `-0.0` too, and `is_finite()` covers the NaN case
    // here, so this is the same predicate written without a negated comparison.
    if !delta.is_finite() || delta <= 0.0 {
        return (current, current_velocity);
    }

    let omega = 2.0 / damping_time;
    let x = omega * delta;
    let exponential = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);

    // `diff = self_axis - target_axis`, clamped to `±(INF * damping_time)`.
    let mut diff = current - target;
    if !diff.is_finite() {
        let max_change = f64::INFINITY * damping_time;
        diff = diff.clamp(-max_change, max_change);
    }

    // `target_axis = self_axis - diff` (== `target` when `diff` was not clamped).
    let effective_target = current - diff;

    let temp = (current_velocity + omega * diff) * delta;
    let mut new_velocity = (current_velocity - omega * temp) * exponential;
    let mut output = effective_target + (diff + temp) * exponential;

    // Overshoot clamp: when the sampler crossed the target, snap and zero the
    // velocity (the oracle's `(output - target_axis) / delta` with `output ==
    // target_axis`).
    if (target - current > 0.0) == (output > target) {
        output = target;
        new_velocity = 0.0;
    }

    (output, new_velocity)
}

/// Map the oracle's `PhantomCameraTween.TransitionType` ordinal onto Godot's enum.
///
/// The ordinals are identical by design (`tween_resource.gd:11-23`); an
/// out-of-range value falls back to `LINEAR`, matching Godot's own `default:`
/// branch.
fn transition_of(ord: i64) -> TransitionType {
    i32::try_from(ord)
        .ok()
        .and_then(TransitionType::try_from_ord)
        .unwrap_or(TransitionType::LINEAR)
}

/// Map the oracle's `PhantomCameraTween.EaseType` ordinal onto Godot's enum.
fn ease_of(ord: i64) -> EaseType {
    i32::try_from(ord)
        .ok()
        .and_then(EaseType::try_from_ord)
        .unwrap_or(EaseType::IN_OUT)
}

/// `Tween.interpolate_value` on two `Variant`s, with the oracle's instant case
/// (`duration == 0`, see `phantom_camera_host.gd:694`) short-circuited first.
fn interpolate_variant(
    from: &Variant,
    delta: &Variant,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Option<Variant> {
    // Same predicate as `smooth_damp`'s guard: not strictly positive, or not
    // finite (which also covers the NaN that `!(duration > 0.0)` used to catch).
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

/// Godot `Tween.interpolate_value` for a scalar.
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

/// Godot `Tween.interpolate_value` for a `Vector2`.
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

/// Godot `Tween.interpolate_value` for a `Vector3`.
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

/// `Tween.interpolate_value` for the `Camera2D.zoom` pair, matching the oracle's
/// `_tween_interpolate_value(_camera_zoom, active.zoom)` call
/// (`phantom_camera_host.gd:998-1001`), which passes `to - from` as the delta.
// parity: `phantom_camera_host.gd:998-1001` is the zoom interpolation leg of the
// oracle's tween pump. The live host currently writes `camera_zoom` from
// `interpolate_value_v2` directly, so nothing calls this yet; it stays as public
// API surface for the parity harness.
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

/// Godot `Tween.interpolate_value` for a `Quaternion`.
///
/// The oracle passes a `Quaternion` delta to `Tween.interpolate_value`
/// (`phantom_camera_host.gd:1009-1018`), which slerps; this forwards the same
/// container so the semantics match.
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

    /// A settled value is a fixed point of the smoother.
    #[test]
    fn settled_value_is_stable() {
        let (output, velocity) = smooth_damp(10.0, 10.0, 0.0, 0.1, 1.0 / 60.0);
        assert!((output - 10.0).abs() < 1e-12, "output {output}");
        assert!(velocity.abs() < 1e-12, "velocity {velocity}");
    }

    /// It converges toward the target instead of oscillating forever.
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

    /// Overshoot snaps exactly onto the target and zeroes the velocity, matching
    /// the oracle's final `if` branch.
    #[test]
    fn snap_lands_on_target() {
        let (output, velocity) = smooth_damp(1.0, 0.0, 25_000.0, 0.0001, 1.0);
        assert!((output - 1.0).abs() < 1e-12, "output {output}");
        assert!(velocity.abs() < 1e-12, "velocity {velocity}");
    }

    /// `damping_time` is floored at 1e-4 and never divides by zero.
    #[test]
    fn damping_time_is_floored() {
        let (output, velocity) = smooth_damp(5.0, 0.0, 0.0, 0.0, 1.0 / 60.0);
        assert!(output.is_finite(), "output {output}");
        assert!(velocity.is_finite(), "velocity {velocity}");
    }

    /// A zero delta returns the inputs unchanged (documented deviation from the
    /// oracle's division by zero).
    #[test]
    fn zero_delta_is_inert() {
        let (output, velocity) = smooth_damp(5.0, 1.0, 0.5, 0.1, 0.0);
        assert!((output - 1.0).abs() < 1e-12);
        assert!((velocity - 0.5).abs() < 1e-12);
    }
}
