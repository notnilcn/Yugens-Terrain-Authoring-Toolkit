use godot::prelude::*;
mod abi;
mod dev;
mod emitters;
mod host;
mod manager;
mod math;
mod pcam2d;
mod pcam3d;
mod probe;
mod resources;
mod tween_director;
pub use emitters::{PhantomCameraNoiseEmitter2D, PhantomCameraNoiseEmitter3D};
pub use host::PhantomCameraHost;
pub use manager::PhantomCameraManager;
pub use pcam2d::PhantomCamera2D;
pub use pcam3d::PhantomCamera3D;
pub use probe::PcamRustProbe;
pub use resources::{
    Camera3DResource, PhantomCameraNoise2D, PhantomCameraNoise3D, PhantomCameraTween,
    TweenDirectorResource,
};
pub use tween_director::PhantomCameraTweenDirector;
pub const REGISTERED_CLASSES: &[&str] = &[
    "PhantomCamera2D",
    "PhantomCamera3D",
    "PhantomCameraHost",
    "PhantomCameraManager",
    "PhantomCameraNoise2D",
    "PhantomCameraNoise3D",
    "PhantomCameraNoiseEmitter2D",
    "PhantomCameraNoiseEmitter3D",
    "PhantomCameraTween",
    "PhantomCameraTweenDirector",
    "TweenDirectorResource",
    "Camera3DResource",
    "PcamRustProbe",
];
pub(crate) fn registered_class_names() -> &'static [&'static str] {
    REGISTERED_CLASSES
}
struct PhantomCameraExtension;
#[gdextension]
unsafe impl ExtensionLibrary for PhantomCameraExtension {}
