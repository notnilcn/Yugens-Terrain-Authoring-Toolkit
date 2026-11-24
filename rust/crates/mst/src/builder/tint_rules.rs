//! How a part's own colour and see-through setting show through the material on its faces,
//! kept free of engine types so the rules are testable without a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotLook {
    Standard { takes_vertex_colour: bool },
    Custom,
}
pub fn needs_tintable_copy(look: SlotLook) -> bool {
    matches!(look, SlotLook::Standard { takes_vertex_colour: false })
}
pub fn fades_through_material(rendering_method: &str) -> bool {
    rendering_method == "gl_compatibility"
}
pub fn faded_alpha(look: SlotLook, base_alpha: f32, transparency: f32) -> Option<f32> {
    let t = transparency.clamp(0.0, 1.0);
    (matches!(look, SlotLook::Standard { .. }) && t > 0.0).then(|| base_alpha * (1.0 - t))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn see_through_parts_fade_standard_materials_only_where_the_renderer_cannot() {
        let std = SlotLook::Standard { takes_vertex_colour: true };
        assert!(fades_through_material("gl_compatibility"));
        assert!(!fades_through_material("forward_plus"), "the engine fades the instance itself");
        assert_eq!(faded_alpha(std, 1.0, 0.0), None, "opaque parts keep the shared material");
        assert_eq!(faded_alpha(std, 1.0, 0.5), Some(0.5));
        assert_eq!(faded_alpha(std, 0.8, 0.25), Some(0.6));
        assert_eq!(faded_alpha(std, 1.0, 2.0), Some(0.0));
        assert_eq!(faded_alpha(SlotLook::Custom, 1.0, 0.5), None, "custom shaders are left alone");
    }
    #[test]
    fn only_standard_materials_that_ignore_the_colour_get_a_copy() {
        assert!(needs_tintable_copy(SlotLook::Standard { takes_vertex_colour: false }));
        assert!(!needs_tintable_copy(SlotLook::Standard { takes_vertex_colour: true }));
        assert!(!needs_tintable_copy(SlotLook::Custom));
    }
}
