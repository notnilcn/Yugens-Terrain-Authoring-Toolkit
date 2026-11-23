// Demo x-ray silhouette, read pass: draws the knight's shape with no depth test wherever the
// write pass did not mark it visible in the stencil buffer, so exactly the parts an occluder
// hides show through. A flat palette colour, a solid rim and a dithered interior, drawn only in
// the low-res colour pass, so it stays pixel-crisp and the resolve never outlines it. Its strength
// is driven from outside, because the stencil alone would also flag the ground at the knight's
// feet.
shader_type spatial;
render_mode unshaded, cull_back, depth_draw_never, depth_test_disabled, shadows_disabled, specular_disabled;
stencil_mode read, compare_not_equal, 1;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

uniform vec4 silhouette_color : source_color = vec4(0.27, 0.56, 1.0, 1.0);
uniform vec4 rim_color : source_color = vec4(0.80, 0.93, 1.0, 1.0);
uniform float rim_threshold : hint_range(0.0, 1.0) = 0.55;
uniform float fill : hint_range(0.0, 1.0) = 0.5;
uniform float amount : hint_range(0.0, 1.0) = 1.0;

varying vec4 origin_clip;

void vertex() {
	PT_SNAP_VERTEX
	origin_clip = PROJECTION_MATRIX * VIEW_MATRIX * vec4(MODEL_MATRIX[3].xyz, 1.0);
}

void fragment() {
	if (PT_IS_META || PT_IS_ID || amount <= 0.0) {
		discard;
	}
	vec2 o = (origin_clip.xy / origin_clip.w * 0.5 + 0.5) * VIEWPORT_SIZE;
	ivec2 p = ivec2(floor(FRAGCOORD.xy - floor(o)));
	float rim = 1.0 - clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0);
	bool edge = rim > rim_threshold;
	int b = (p.x & 1) * 2 + (p.y & 1);
	float t = (b == 0 ? 0.0 : b == 1 ? 3.0 : b == 2 ? 2.0 : 1.0) / 4.0 + 0.125;
	if (t > (edge ? amount : fill * amount)) {
		discard;
	}
	ALBEDO = edge ? rim_color.rgb : silhouette_color.rgb;
	ALPHA = 1.0;
}
