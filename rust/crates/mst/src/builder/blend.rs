//! `PixelBuilderBlendMaterial` — a material that blends two layers by the painted vertex alpha.
//! Port of Unibuilder's `materials/ub_blend_material.gd` (`UBBlendMaterial`, MIT) on top of the
//! pixel-pipeline version of `ub_blend.gdshader` (`blend.gdshader` next to this file).
//!
//! The shader includes `pixel_common.gdshaderinc`, so `PixelObject` keeps it as authored (toon
//! light, outlines, metadata) instead of converting it like a `StandardMaterial3D`.

use godot::classes::base_material_3d::{Feature, TextureParam};
use godot::classes::{BaseMaterial3D, IShaderMaterial, Shader, ShaderMaterial};
use godot::prelude::*;

pub const SHADER_CODE: &str = include_str!("blend.gdshader");

/// Uniform base names carried per layer; each exists as `<name>_a` and `<name>_b`.
pub const LAYER_UNIFORMS: [&str; 9] = ["texture", "albedo", "height_map", "height_offset", "emission_map", "emission", "emission_energy", "uv_scale", "uv_offset"];

pub fn layer_suffix(layer: i64) -> &'static str {
    if layer == 1 { "_b" } else { "_a" }
}

/// A fresh shader with the blend code.
pub fn blend_shader() -> Gd<Shader> {
    let mut sh = Shader::new_gd();
    sh.set_code(SHADER_CODE);
    sh
}

#[derive(GodotClass)]
#[class(tool, base = ShaderMaterial)]
pub struct PixelBuilderBlendMaterial {
    base: Base<ShaderMaterial>,
}

#[godot_api]
impl IShaderMaterial for PixelBuilderBlendMaterial {
    fn init(base: Base<ShaderMaterial>) -> Self {
        // The shader is not set here: ClassDB reads default values from a fresh instance and
        // warns about a Resource default. Loaded materials get their saved shader; `create()`
        // sets it at once; one made from the inspector gets it a frame later.
        base.to_init_gd().call_deferred("ensure_shader", &[]);
        PixelBuilderBlendMaterial { base }
    }

    // Required by the Material virtuals; ShaderMaterial's own (C++) implementation answers.
    fn get_shader_rid(&self) -> Rid {
        self.base().get_shader().map(|s| s.get_rid()).unwrap_or(Rid::Invalid)
    }

    fn get_shader_mode(&self) -> godot::classes::shader::Mode {
        godot::classes::shader::Mode::SPATIAL
    }
}

#[godot_api]
impl PixelBuilderBlendMaterial {
    /// Layer A (0) or B (1).
    #[constant]
    const LAYER_A: i64 = 0;
    #[constant]
    const LAYER_B: i64 = 1;

    /// A new blend material with its shader.
    #[func]
    pub fn create() -> Gd<PixelBuilderBlendMaterial> {
        let mut m = PixelBuilderBlendMaterial::new_gd();
        m.bind_mut().ensure_shader();
        m
    }

    /// Gives the material the blend shader if it has none.
    #[func]
    pub fn ensure_shader(&mut self) {
        if self.base().get_shader().is_none() {
            let sh = blend_shader();
            self.base_mut().set_shader(&sh);
        }
    }

    /// Current values of one layer's uniforms, keyed by base name.
    #[func]
    pub fn layer_values(&self, layer: i64) -> VarDictionary {
        let suffix = layer_suffix(layer);
        let mut out = VarDictionary::new();
        for name in LAYER_UNIFORMS {
            out.set(name, &self.base().get_shader_parameter(&format!("{name}{suffix}")));
        }
        out
    }

    /// Writes a layer snapshot back; missing keys are left alone (a partial dictionary patches).
    #[func]
    pub fn apply_layer(&mut self, layer: i64, values: VarDictionary) {
        let suffix = layer_suffix(layer);
        for (k, v) in values.iter_shared() {
            let name = k.to_string();
            self.base_mut().set_shader_parameter(&format!("{name}{suffix}"), &v);
        }
    }

    /// Maps a StandardMaterial3D / ORMMaterial3D onto one layer's uniforms.
    #[func]
    pub fn layer_from_standard(src: Gd<BaseMaterial3D>) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("texture", &src.get_texture(TextureParam::ALBEDO).to_variant());
        d.set("albedo", &src.get_albedo().to_variant());
        let em = src.get_feature(Feature::EMISSION);
        d.set("emission_map", &if em { src.get_texture(TextureParam::EMISSION).to_variant() } else { Variant::nil() });
        d.set("emission", &if em { src.get_emission() } else { Color::from_rgba(0.0, 0.0, 0.0, 1.0) }.to_variant());
        d.set("emission_energy", &src.get_emission_energy_multiplier().to_variant());
        d.set("height_map", &if src.get_feature(Feature::HEIGHT_MAPPING) { src.get_texture(TextureParam::HEIGHTMAP).to_variant() } else { Variant::nil() });
        let s = src.get_uv1_scale();
        let o = src.get_uv1_offset();
        d.set("uv_scale", &Vector2::new(s.x, s.y).to_variant());
        d.set("uv_offset", &Vector2::new(o.x, o.y).to_variant());
        d
    }

    /// Imports a standard material into one layer.
    #[func]
    pub fn import_layer(&mut self, layer: i64, src: Gd<BaseMaterial3D>) {
        let v = Self::layer_from_standard(src);
        self.apply_layer(layer, v);
    }
}
