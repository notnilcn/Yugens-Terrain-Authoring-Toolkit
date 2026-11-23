// Demo x-ray silhouette, write pass: marks the pixels where the knight is visible in the
// stencil buffer without changing the colour, so the read pass can fill the parts hidden behind
// terrain. The same vertex snap as the rest of the pixel pipeline, so the depths line up with the
// knight exactly.
shader_type spatial;
render_mode unshaded, cull_back, depth_draw_never, shadows_disabled, specular_disabled;
stencil_mode write, compare_always, 1;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

void vertex() {
	PT_SNAP_VERTEX
}

void fragment() {
	if (PT_IS_META || PT_IS_ID) {
		discard;
	}
	ALBEDO = vec3(0.0);
	ALPHA = 0.0;
}
