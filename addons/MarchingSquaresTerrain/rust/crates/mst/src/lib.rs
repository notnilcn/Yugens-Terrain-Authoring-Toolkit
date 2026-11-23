use godot::init::{ExtensionLibrary, InitStage, gdextension};
mod builder;
mod camera_projection;
mod clock;
mod debug;
mod editor;
mod editor_detection;
mod effects;
mod features;
pub mod globals;
mod math_conversions;
mod instance_buffer;
mod mesh_arrays;
mod internal_children;
mod valid_handles;
mod node_setters;
mod object_flags;
mod prof;
mod property_hints;
mod render;
mod shader_materials;
mod shader_param_cache;
mod terrain;
struct MarchingSquaresTerrainExtension;
#[gdextension(entry_symbol = marching_squares_terrain_init)]
unsafe impl ExtensionLibrary for MarchingSquaresTerrainExtension {
    fn on_stage_init(stage: InitStage) {
        if stage == InitStage::MainLoop {
            globals::ensure();
            features::register_settings();
        }
    }
    fn on_stage_deinit(stage: InitStage) {
        if stage == InitStage::MainLoop {
            globals::clear_cache();
            shader_param_cache::clear_names();
        }
    }
}
