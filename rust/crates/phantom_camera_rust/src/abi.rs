// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/phantom_camera/phantom_camera_constants.gd
//
#![allow(dead_code)]
//! Frozen enum/constant table for the Phantom Camera port.
//!
//! Ported from `addons/phantom_camera/scripts/phantom_camera/phantom_camera_constants.gd`,
//! the `enum` blocks of `phantom_camera_2d.gd:69-99`, `phantom_camera_3d.gd:78-118`,
//! `phantom_camera_host.gd:43-48`, `camera_3d_resource.gd:11-20`,
//! `tween_resource.gd:11-32` and `tween_director_resource.gd:15-19`.
//!
//! **These integer values are ABI.** Exported enum properties are stored in
//! `.tscn` files as plain `int`, so the numbers here must equal the oracle's
//! forever. Only the *Rust* names are free; everything exposed to Godot is an
//! `i64` whose values come from this module.

/// Manager node/singleton name (`phantom_camera_constants.gd:11`).
///
/// Used both as the `Engine` singleton key and as the `/root` child name, exactly
/// as the oracle does.
pub const PCAM_MANAGER_NODE_NAME: &str = "PhantomCameraManager";

/// `phantom_camera_constants.gd:26` — declared but never registered as a group by
/// the oracle (dead constant); kept for parity of the public constant surface.
pub const PCAM_GROUP_NAME: &str = "phantom_camera_group";

/// `phantom_camera_constants.gd:27` — see [`PCAM_GROUP_NAME`].
pub const PCAM_HOST_GROUP_NAME: &str = "phantom_camera_host_group";

/// Which camera family an instance drives.
///
/// The oracle distinguishes 2D/3D through `get_class()`/`is_class()` overrides,
/// which gdext cannot reproduce (`phantom_camera_2d.gd:1961-1966`,
/// `phantom_camera_3d.gd:2470-2476`, relied on by `phantom_camera_manager.gd:136`).
/// Every PCam class therefore exposes `pcam_kind()` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcamKind {
    TwoD,
    ThreeD,
}

impl PcamKind {
    /// The string returned by `pcam_kind()`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TwoD => "2d",
            Self::ThreeD => "3d",
        }
    }

    /// Parse a `pcam_kind()` result.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "2d" => Some(Self::TwoD),
            "3d" => Some(Self::ThreeD),
            _ => None,
        }
    }
}

/// `phantom_camera_2d.gd:69-76` — note there is no `THIRD_PERSON` in 2D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowMode2D {
    None = 0,
    Glued = 1,
    Simple = 2,
    Group = 3,
    Path = 4,
    Framed = 5,
}

/// `phantom_camera_3d.gd:78-86` — 2D's five modes plus `THIRD_PERSON = 6`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowMode3D {
    None = 0,
    Glued = 1,
    Simple = 2,
    Group = 3,
    Path = 4,
    Framed = 5,
    ThirdPerson = 6,
}

/// `phantom_camera_3d.gd:89-94`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum LookAtMode {
    None = 0,
    Mimic = 1,
    Simple = 2,
    Group = 3,
}

/// `phantom_camera_2d.gd:82-84`, `phantom_camera_3d.gd:97-99`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum InactiveUpdateMode {
    Always = 0,
    Never = 1,
}

/// `phantom_camera_2d.gd:88-93`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowLockAxis2D {
    None = 0,
    X = 1,
    Y = 2,
    Xy = 3,
}

/// `phantom_camera_3d.gd:102-110`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowLockAxis3D {
    None = 0,
    X = 1,
    Y = 2,
    Z = 3,
    Xy = 4,
    Xz = 5,
    Yz = 6,
    Xyz = 7,
}

/// `phantom_camera_2d.gd:95-99` — internal discriminator for the follow target's
/// physics class; never exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowTargetPhysicsClass {
    CharacterBody = 0,
    RigidBody = 1,
    Other = 2,
}

/// `phantom_camera_host.gd:43-48`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum InterpolationMode {
    Auto = 0,
    Idle = 1,
    Physics = 2,
    Manual = 3,
}

/// `camera_3d_resource.gd:11-14`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum KeepAspect {
    KeepWidth = 0,
    KeepHeight = 1,
}

/// `camera_3d_resource.gd:16-20`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ProjectionType {
    Perspective = 0,
    Orthogonal = 1,
    Frustum = 2,
}

/// `tween_resource.gd:11-23` — identical ordering to Godot's own
/// `Tween.TransitionType`, because the integers are handed straight to
/// `Tween.interpolate_value` (`phantom_camera_host.gd:1221-1229`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum TransitionType {
    Linear = 0,
    Sine = 1,
    Quint = 2,
    Quart = 3,
    Quad = 4,
    Expo = 5,
    Elastic = 6,
    Cubic = 7,
    Circ = 8,
    Bounce = 9,
    Back = 10,
}

/// `tween_resource.gd:25-32` — Godot's `Tween.EaseType` ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
// The shared `Ease` prefix mirrors Godot's own `Tween.EASE_IN`/`EASE_OUT`/… names
// (`tween_resource.gd:25-32`) and the ordinals are the ABI; renaming the variants
// would only widen the gap to the oracle, so the lint is waived instead.
#[allow(clippy::enum_variant_names)]
pub enum EaseType {
    EaseIn = 0,
    EaseOut = 1,
    EaseInOut = 2,
    EaseOutIn = 3,
}

/// `tween_director_resource.gd:15-19`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum TweenDirectorMatch {
    PhantomCamera = 0,
    TweenResource = 1,
    Any = 2,
}

/// Set or clear one render layer bit, matching `phantom_camera_host.gd:1317-1329`
/// (`_set_layer`): layers are 1-based, `1..=20`, out of range is ignored.
pub fn set_layer(current_layers: i64, layer_number: i64, value: bool) -> i64 {
    if !(1..=20).contains(&layer_number) {
        return current_layers;
    }
    let bit = 1_i64 << (layer_number - 1);
    if value {
        current_layers | bit
    } else {
        current_layers & !bit
    }
}

/// `mask_a & mask_b != 0`, i.e. the oracle's `_pcam_is_in_host_layer`
/// (`phantom_camera_host.gd:364-366`).
pub fn layers_overlap(mask_a: i64, mask_b: i64) -> bool {
    mask_a & mask_b != 0
}

/// `tween_resource.gd:14` default. The host treats `0.0` as an instant transition
/// (`phantom_camera_host.gd:694`).
pub const TWEEN_DEFAULT_DURATION: f64 = 1.0;

/// `tween_resource.gd:29` default — `EASE_IN_OUT`.
pub const TWEEN_DEFAULT_EASE: i64 = EaseType::EaseInOut as i64;

/// `tween_resource.gd:20` default — `LINEAR`.
pub const TWEEN_DEFAULT_TRANSITION: i64 = TransitionType::Linear as i64;

/// `camera_3d_resource.gd:31` default — every 3D render layer.
pub const CAMERA_3D_DEFAULT_CULL_MASK: i64 = 1_048_575;

/// 2D limit default (`phantom_camera_2d.gd:445`).
pub const LIMIT_LEFT_DEFAULT: i64 = -10_000_000;
/// See [`LIMIT_LEFT_DEFAULT`] (`phantom_camera_2d.gd:450`).
pub const LIMIT_TOP_DEFAULT: i64 = -10_000_000;
/// See [`LIMIT_LEFT_DEFAULT`] (`phantom_camera_2d.gd:455`).
pub const LIMIT_RIGHT_DEFAULT: i64 = 10_000_000;
/// See [`LIMIT_LEFT_DEFAULT`] (`phantom_camera_2d.gd:460`).
pub const LIMIT_BOTTOM_DEFAULT: i64 = 10_000_000;

/// `phantom_camera_2d.gd:552` — the four limit sides as `(left, top, right, bottom)`.
pub const LIMIT_SIDES_DEFAULT: (i64, i64, i64, i64) = (
    LIMIT_LEFT_DEFAULT,
    LIMIT_TOP_DEFAULT,
    LIMIT_RIGHT_DEFAULT,
    LIMIT_BOTTOM_DEFAULT,
);
