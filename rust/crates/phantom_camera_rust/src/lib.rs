// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/**
//
//! `phantom_camera_rust` — a Rust GDExtension port of the Phantom Camera addon
//! (`client/addons/phantom_camera/`, v0.11.0.3) for Godot 4.7.
//!
//! Ported from: `addons/phantom_camera/scripts/**` (Marcus Skov, MIT — the
//! upstream `LICENSE` ships beside this addon). The original GDScript addon is
//! kept in the port's source repository as the read-only parity oracle; this
//! crate is the shipped implementation.
//!
//! Class names, exported property names, signal signatures and enum values are
//! the scene ABI (`*.tscn` files name classes and store properties as strings),
//! so they are ported verbatim and never change shape. `abi.rs` holds the frozen
//! enum/constant table; `PORT-PLAN.md` beside the `.gdextension` holds the
//! contract every module in this crate follows.

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

/// Every Godot class name this library registers.
///
/// Kept as an explicit list so the parity harness can assert the full class
/// surface without guessing, and so a forgotten `mod` shows up as a mismatched
/// count rather than a runtime "unknown class" parse error in a scene.
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
