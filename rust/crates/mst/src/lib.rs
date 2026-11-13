//! Marching Squares Terrain — a single, dependency-free Godot 4.7 GDExtension that combines a
//! marching-squares terrain authoring toolkit (square / triangle / hex / hex-ring grids)
//! with a 3D pixel-art renderer built for the Compatibility (OpenGL 3.3) renderer.
//!
//! Engine-free logic (meshing, brushes, palettes, metadata packing) lives in `pt_core`;
//! this crate turns it into nodes, resources and an editor plugin.

use godot::init::{ExtensionLibrary, InitStage, gdextension};

mod builder;
mod debug;
mod editor;
mod effects;
pub mod globals;
mod render;
mod terrain;
mod util;

struct MarchingSquaresTerrainExtension;

#[gdextension(entry_symbol = marching_squares_terrain_init)]
unsafe impl ExtensionLibrary for MarchingSquaresTerrainExtension {
    fn on_stage_init(stage: InitStage) {
        if stage == InitStage::MainLoop {
            // Shaders reference these as `global uniform`s; registering them before any
            // scene loads means users never have to touch Project Settings.
            globals::ensure();
        }
    }
}
