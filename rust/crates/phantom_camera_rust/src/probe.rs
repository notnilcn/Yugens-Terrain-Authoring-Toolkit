// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
//
//! `PcamRustProbe` — a no-op `Node` that proves the second GDExtension library
//! is loaded and its classes are registered.
//!
//! The parity harness instantiates it and asserts the reported version, so a
//! missing/stale `phantom_camera_rust` library shows up as a single failed
//! assertion instead of a wall of "unknown class" scene errors.
//!
//! Not part of the upstream addon.

use godot::prelude::*;

/// Version of the `phantom-camera-rust` crate, taken from `Cargo.toml`.
pub const PORTS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the upstream Phantom Camera addon this crate ports.
pub const UPSTREAM_VERSION: &str = "0.11.0.3";

#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PcamRustProbe {
    base: Base<Node>,
}

#[godot_api]
impl INode for PcamRustProbe {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }

    fn ready(&mut self) {
        // `log` has one global sink per final artifact, so a second cdylib's
        // records never reach `kg_gdext`'s bridge (installed on its own copy of
        // the crate). Everything in this addon prints through Godot directly.
        godot_print!(
            "[phantom_camera_rust] loaded: port {PORTS_VERSION}, upstream {UPSTREAM_VERSION}, {} classes",
            crate::REGISTERED_CLASSES.len()
        );
    }
}

#[godot_api]
impl PcamRustProbe {
    /// Port crate version (`0.1.0`).
    #[func]
    fn get_ports_version(&self) -> GString {
        GString::from(PORTS_VERSION)
    }

    /// Upstream Phantom Camera addon version this crate ports (`0.11.0.3`).
    #[func]
    fn get_upstream_version(&self) -> GString {
        GString::from(UPSTREAM_VERSION)
    }

    /// Names of every class this library registers, for harness assertions.
    #[func]
    fn get_registered_classes(&self) -> PackedStringArray {
        let mut names = PackedStringArray::new();
        for name in crate::registered_class_names() {
            names.push(*name);
        }
        names
    }
}
