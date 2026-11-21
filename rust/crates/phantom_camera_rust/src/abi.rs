#![allow(dead_code)]
pub const PCAM_MANAGER_NODE_NAME: &str = "PhantomCameraManager";
pub const PCAM_GROUP_NAME: &str = "phantom_camera_group";
pub const PCAM_HOST_GROUP_NAME: &str = "phantom_camera_host_group";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcamKind {
    TwoD,
    ThreeD,
}
impl PcamKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TwoD => "2d",
            Self::ThreeD => "3d",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "2d" => Some(Self::TwoD),
            "3d" => Some(Self::ThreeD),
            _ => None,
        }
    }
}
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum LookAtMode {
    None = 0,
    Mimic = 1,
    Simple = 2,
    Group = 3,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum InactiveUpdateMode {
    Always = 0,
    Never = 1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowLockAxis2D {
    None = 0,
    X = 1,
    Y = 2,
    Xy = 3,
}
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowTargetPhysicsClass {
    CharacterBody = 0,
    RigidBody = 1,
    Other = 2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum InterpolationMode {
    Auto = 0,
    Idle = 1,
    Physics = 2,
    Manual = 3,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum KeepAspect {
    KeepWidth = 0,
    KeepHeight = 1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ProjectionType {
    Perspective = 0,
    Orthogonal = 1,
    Frustum = 2,
}
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
#[allow(clippy::enum_variant_names)]
pub enum EaseType {
    EaseIn = 0,
    EaseOut = 1,
    EaseInOut = 2,
    EaseOutIn = 3,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum TweenDirectorMatch {
    PhantomCamera = 0,
    TweenResource = 1,
    Any = 2,
}
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
pub fn layers_overlap(mask_a: i64, mask_b: i64) -> bool {
    mask_a & mask_b != 0
}
pub const TWEEN_DEFAULT_DURATION: f64 = 1.0;
pub const TWEEN_DEFAULT_EASE: i64 = EaseType::EaseInOut as i64;
pub const TWEEN_DEFAULT_TRANSITION: i64 = TransitionType::Linear as i64;
pub const CAMERA_3D_DEFAULT_CULL_MASK: i64 = 1_048_575;
pub const LIMIT_LEFT_DEFAULT: i64 = -10_000_000;
pub const LIMIT_TOP_DEFAULT: i64 = -10_000_000;
pub const LIMIT_RIGHT_DEFAULT: i64 = 10_000_000;
pub const LIMIT_BOTTOM_DEFAULT: i64 = 10_000_000;
pub const LIMIT_SIDES_DEFAULT: (i64, i64, i64, i64) = (
    LIMIT_LEFT_DEFAULT,
    LIMIT_TOP_DEFAULT,
    LIMIT_RIGHT_DEFAULT,
    LIMIT_BOTTOM_DEFAULT,
);
