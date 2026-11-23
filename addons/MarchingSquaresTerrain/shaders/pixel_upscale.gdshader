// Marching Squares Terrain - final upscale of the resolved low-res image to the screen. The camera's
// sub-texel remainder is applied here, so pixels stay locked to the world while the view
// scrolls. `crisp_edges` clamps each sample to its texel (t3ssel8r-style pixel-perfect upscale, as
// in 3d-pixel-art-in-godot's `pixel_art_upscale.gdshader`, CC0) so non-integer scales show no seams.
shader_type canvas_item;
render_mode unshaded, blend_disabled;

uniform sampler2D source : filter_linear;
uniform vec2 source_size = vec2(1.0);
uniform vec2 view_texels = vec2(1.0);
uniform vec2 shift = vec2(0.0);
/// Texel coordinate of the camera centre in the source (its centre).
uniform vec2 center = vec2(0.5);
uniform bool crisp_edges = true;

void fragment() {
	vec2 q = (UV - 0.5) * view_texels + center + shift;
	if (crisp_edges) {
		vec2 seam = floor(q + 0.5);
		vec2 d = max(fwidth(q), vec2(1e-5));
		q = seam + clamp((q - seam) / d, -0.5, 0.5);
	} else {
		q = floor(q) + 0.5;
	}
	COLOR = vec4(textureLod(source, q / source_size, 0.0).rgb, 1.0);
}
