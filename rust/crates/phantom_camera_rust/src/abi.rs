pub const PCAM_MANAGER_NODE_NAME: &str = "PhantomCameraManager";
#[expect(
    dead_code,
    reason = "group name a scene may use to reach every camera of the addon"
)]
pub const PCAM_GROUP_NAME: &str = "phantom_camera_group";
#[expect(
    dead_code,
    reason = "group name a scene may use to reach every camera host of the addon"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum LookAtMode {
    None = 0,
    Mimic = 1,
    Simple = 2,
    Group = 3,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum InactiveUpdateMode {
    Always = 0,
    Never = 1,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FollowLockAxis2D {
    None = 0,
    X = 1,
    Y = 2,
    Xy = 3,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum KeepAspect {
    KeepWidth = 0,
    KeepHeight = 1,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ProjectionType {
    Perspective = 0,
    Orthogonal = 1,
    Frustum = 2,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
#[expect(
    clippy::enum_variant_names,
    reason = "variant names are the easing labels shown in the inspector"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum EaseType {
    EaseIn = 0,
    EaseOut = 1,
    EaseInOut = 2,
    EaseOutIn = 3,
}
#[expect(
    dead_code,
    reason = "ordinals of an exported enum whose values scenes store as integers"
)]
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
pub const TWEEN_DEFAULT_DURATION: f64 = 1.0;
pub const TWEEN_DEFAULT_EASE: i64 = EaseType::EaseInOut as i64;
pub const TWEEN_DEFAULT_TRANSITION: i64 = TransitionType::Linear as i64;
pub const CAMERA_3D_DEFAULT_CULL_MASK: i64 = 1_048_575;
pub const LIMIT_LEFT_DEFAULT: i64 = -10_000_000;
pub const LIMIT_TOP_DEFAULT: i64 = -10_000_000;
pub const LIMIT_RIGHT_DEFAULT: i64 = 10_000_000;
pub const LIMIT_BOTTOM_DEFAULT: i64 = 10_000_000;
