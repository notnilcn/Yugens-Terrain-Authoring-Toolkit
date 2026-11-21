use godot::init::{ExtensionLibrary, gdextension};
mod anim;
mod hud;
mod player;
struct MstDemoExtension;
#[gdextension(entry_symbol = mst_demo_init)]
unsafe impl ExtensionLibrary for MstDemoExtension {}
