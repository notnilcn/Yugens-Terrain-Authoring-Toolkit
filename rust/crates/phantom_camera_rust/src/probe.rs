//! A load check that says out loud which addon build a running project actually loaded.
//!
//! Intent: when a scene behaves as though an older build were installed, someone needs to see
//! from inside the running game which build answered, which upstream version it claims and
//! what it registered. Placing one of these in a scene prints that once and exposes it for a
//! script to read.
//!
//! Responsibility: it owns reporting the build identity it was compiled with and the class
//! list the addon registered. It does not configure anything, does not drive a camera, and
//! does not check that the identity is the expected one.
//!
//! Why a node: the report has to come from inside the running project rather than from a file
//! on disk, and being a node lets it announce itself as soon as the scene it is placed in
//! becomes ready.
//!
//! Why a new type: no built-in node reports this build's identity or the addon's registered
//! class list, and a build check that lived in the addon's own registration code could not be
//! pointed at one scene.
//!
//! Not exported: nothing beyond the node itself; the versions are compiled in and the class
//! list is whatever the addon registered.

use godot::prelude::*;
pub const PORTS_VERSION: &str = env!("CARGO_PKG_VERSION");
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
        godot_print!(
            "[phantom_camera_rust] loaded: port {PORTS_VERSION}, upstream {UPSTREAM_VERSION}, {} classes",
            crate::REGISTERED_CLASSES.len()
        );
    }
}
#[godot_api]
impl PcamRustProbe {
    #[func]
    fn get_ports_version(&self) -> GString {
        GString::from(PORTS_VERSION)
    }
    #[func]
    fn get_upstream_version(&self) -> GString {
        GString::from(UPSTREAM_VERSION)
    }
    #[func]
    fn get_registered_classes(&self) -> PackedStringArray {
        let mut names = PackedStringArray::new();
        for name in crate::registered_class_names() {
            names.push(*name);
        }
        names
    }
}
