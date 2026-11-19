// Demo x-ray silhouette, pass 1 of 2 (material_overlay of the knight's meshes, set up by
// MstDemoPlayer): marks the pixels where the knight is *visible* in the stencil buffer
// (reference 1) without changing the colour. Pass 2 (xray_silhouette.gdshader, next_pass)
// then fills the rest of the knight's shape - the parts hidden behind terrain.
// Same vertex snap as the pixel object shader, so the depths match the knight exactly.
shader_type spatial;
render_mode unshaded, cull_back, depth_draw_never, shadows_disabled, specular_disabled;
stencil_mode write, compare_always, 1;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

void vertex() {
	PT_SNAP_VERTEX
}

void fragment() {
	// Metadata / id cameras: no silhouette there (outlines come from the real geometry only).
	if (PT_IS_META || PT_IS_ID) {
		discard;
	}
	ALBEDO = vec3(0.0);
	ALPHA = 0.0;
}
