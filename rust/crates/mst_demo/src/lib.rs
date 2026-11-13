//! Demo-only gameplay classes for the Marching Squares Terrain demo scenes (`demo/`).
//!
//! This is a separate GDExtension library (`demo/mst_demo.gdextension`, built into `demo/bin/`):
//! the shipped addon gains no gameplay class, and neither the addon nor Phantom Camera is linked —
//! both are reached through dynamic method calls, so the demo degrades gracefully without them.

use godot::init::{ExtensionLibrary, gdextension};

mod anim;
mod player;

struct MstDemoExtension;

#[gdextension(entry_symbol = mst_demo_init)]
unsafe impl ExtensionLibrary for MstDemoExtension {}
