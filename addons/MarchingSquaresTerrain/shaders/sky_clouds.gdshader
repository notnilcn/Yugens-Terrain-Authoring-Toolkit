// Marching Squares Terrain - visible cloud deck. Uses the same field as the cloud shadows, so shadows
// sit exactly under the clouds. Colour pass only (not part of the metadata layer).
shader_type spatial;
render_mode unshaded, cull_disabled, depth_draw_never, shadows_disabled;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"

uniform vec4 cloud_color : source_color = vec4(1.0, 0.98, 0.95, 1.0);
uniform vec4 shade_color : source_color = vec4(0.72, 0.74, 0.85, 1.0);
uniform float opacity : hint_range(0.0, 1.0) = 0.85;
uniform int alpha_bands : hint_range(1, 8) = 3;

varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	float d = pt_cloud_density(world_pos.xz);
	vec3 l = normalize(pt_sun_dir);
	float d2 = pt_cloud_density(world_pos.xz + l.xz * 2.0);
	float shade = clamp(d2 - d * 0.5, 0.0, 1.0);
	ALBEDO = mix(cloud_color.rgb, shade_color.rgb, step(0.5, shade));
	// Darken with the night factor published by PixelDayNight.
	ALBEDO *= mix(1.0, 0.25, pt_night);
	float b = float(alpha_bands);
	ALPHA = floor(d * opacity * b + 0.5) / b;
}
