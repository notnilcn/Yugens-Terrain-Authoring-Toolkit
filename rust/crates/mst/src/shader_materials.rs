use godot::classes::{Shader, ShaderMaterial};
use godot::prelude::*;
pub const SHADER_DIR: &str = "res://addons/MarchingSquaresTerrain/shaders/";
pub fn load_shader(name: &str) -> Option<Gd<Shader>> {
    try_load::<Shader>(&format!("{SHADER_DIR}{name}")).ok()
}
pub fn shader_material(name: &str) -> Gd<ShaderMaterial> {
    let mut m = ShaderMaterial::new_gd();
    if let Some(s) = load_shader(name) {
        m.set_shader(&s);
    } else {
        godot_error!("Marching Squares Terrain: missing shader {SHADER_DIR}{name}");
    }
    m
}
