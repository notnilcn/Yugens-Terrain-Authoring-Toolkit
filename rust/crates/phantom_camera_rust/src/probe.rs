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
