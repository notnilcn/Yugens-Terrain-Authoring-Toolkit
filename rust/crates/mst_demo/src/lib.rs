use godot::init::{ExtensionLibrary, gdextension};
mod anim;
mod hud;
mod orbit_camera;
mod player;
struct MstDemoExtension;
#[gdextension(entry_symbol = mst_demo_init)]
unsafe impl ExtensionLibrary for MstDemoExtension {}
