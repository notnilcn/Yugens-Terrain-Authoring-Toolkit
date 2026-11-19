// Demo x-ray silhouette, pass 2 of 2 (next_pass of xray_mask.gdshader): draws the knight's
// shape with no depth test wherever pass 1 did not mark it visible, i.e. exactly the parts a
// cliff hides. Flat palette colour, solid rim and a 50% checker inside; the checker is anchored
// to the knight's texel-snapped origin, so it does not crawl while the knight walks. Drawn in
// the low-res colour pass only (the meta / id passes discard it), so it is pixel-crisp and the
// resolve never outlines it. `amount` is driven by MstDemoPlayer (0 unless a real occluder hides
// a meaningful part of the body; the stencil alone would also flag grass / ground at the feet)
// and fades the silhouette in / out through the same ordered dither.
shader_type spatial;
render_mode unshaded, cull_back, depth_draw_never, depth_test_disabled, shadows_disabled, specular_disabled;
stencil_mode read, compare_not_equal, 1;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

uniform vec4 silhouette_color : source_color = vec4(0.27, 0.56, 1.0, 1.0);
uniform vec4 rim_color : source_color = vec4(0.80, 0.93, 1.0, 1.0);
/// 1 - N.V above which a fragment counts as rim (solid).
uniform float rim_threshold : hint_range(0.0, 1.0) = 0.55;
/// Fraction of interior pixels drawn (ordered 2x2 dither).
uniform float fill : hint_range(0.0, 1.0) = 0.5;
/// Overall strength: 0 hides the silhouette, 1 shows it fully.
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
	// 2x2 Bayer thresholds 0, 2, 3, 1 (/4).
	int b = (p.x & 1) * 2 + (p.y & 1);
	float t = (b == 0 ? 0.0 : b == 1 ? 3.0 : b == 2 ? 2.0 : 1.0) / 4.0 + 0.125;
	if (t > (edge ? amount : fill * amount)) {
		discard;
	}
	ALBEDO = edge ? rim_color.rgb : silhouette_color.rgb;
	// Writing ALPHA puts the pass in the transparent list: after the opaque terrain and pass 1.
	ALPHA = 1.0;
}
