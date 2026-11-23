use godot::prelude::*;
mod abi;
mod dev;
mod host;
mod manager;
mod math;
mod noise_emitter_2d;
mod noise_emitter_3d;
mod noise_trauma;
mod pcam2d;
mod pcam3d;
mod probe;
mod resources;
mod tween_director;
pub use host::PhantomCameraHost;
pub use manager::PhantomCameraManager;
pub use noise_emitter_2d::PhantomCameraNoiseEmitter2D;
pub use noise_emitter_3d::PhantomCameraNoiseEmitter3D;
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
