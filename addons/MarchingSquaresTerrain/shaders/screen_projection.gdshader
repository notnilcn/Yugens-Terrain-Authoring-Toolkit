// Marching Squares Terrain - screen-space texture projection.
// Port of Pixelizer3D's screen_projection.gdshader onto pixel_common. The texture is painted
// onto the low-res screen grid (SCREEN_UV * VIEWPORT_SIZE = texels), so projected texels never
// swim on moving meshes; the mesh supplies shape, depth, outlines and lighting.
// `texture_size` is the tile period in low-res texels; `lock_to_object` makes the pattern
// follow the object's projected origin instead of being glued to the screen.
shader_type spatial;
render_mode specular_disabled, cull_back;
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

uniform sampler2D projection_texture : source_color, filter_nearest, repeat_enable;
uniform bool use_projection_texture = false;
/// Tile period in low-res texels.
uniform vec2 texture_size = vec2(8.0, 8.0);
uniform vec4 tint : source_color = vec4(1.0);
/// Second colour of the procedural checker used without a texture.
uniform vec4 alt_tint : source_color = vec4(0.55, 0.55, 0.62, 1.0);
uniform bool lock_to_object = true;
/// Screen-space rotation of the projection grid (radians).
uniform float projection_angle = 0.0;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.3;

varying vec3 world_pos;
varying vec2 f_origin_ndc;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	vec4 clip = PROJECTION_MATRIX * VIEW_MATRIX * MODEL_MATRIX * vec4(0.0, 0.0, 0.0, 1.0);
	f_origin_ndc = clip.xy / max(abs(clip.w), 0.0001) * sign(clip.w);
	PT_SNAP_VERTEX
}

void fragment() {
	vec2 px = SCREEN_UV * VIEWPORT_SIZE;
	if (lock_to_object) {
		px -= (f_origin_ndc * vec2(0.5, -0.5) + 0.5) * VIEWPORT_SIZE;
	}
	if (abs(projection_angle) > 0.0001) {
		float s = sin(projection_angle);
		float c = cos(projection_angle);
		px = mat2(vec2(c, s), vec2(-s, c)) * px;
	}
	vec2 uv = px / max(texture_size, vec2(1.0));
	vec3 col;
	if (use_projection_texture) {
		col = texture(projection_texture, uv).rgb * tint.rgb;
	} else {
		vec2 q = floor(uv);
		col = mod(q.x + q.y, 2.0) < 0.5 ? tint.rgb : alt_tint.rgb;
	}
	ALBEDO = col;
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, pt_cloud_shadow(world_pos))
}
