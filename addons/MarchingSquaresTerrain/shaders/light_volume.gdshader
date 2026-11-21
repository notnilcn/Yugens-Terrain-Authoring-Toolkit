// Marching Squares Terrain - PixelLightVolume: a pixel-art pool of lamplight plus a small
// source glow.
//
// The view-ray glow follows robert-k's `godrays/godray.gdshader` (MIT; see CREDITS.md): the
// opaque surface under each pixel is rebuilt from the depth buffer and lit by a radial falloff
// (the "pool"), while an orb is placed where the view ray passes closest to the light (the
// "glow") and is cut away by geometry in front of it. Both are quantised into a few brightness
// steps; the step edges are an ordered (Bayer 4x4) dither in the low-res colour pass, so the
// light reads as pixel-art lamplight with no hard disc edge. Blending is additive, so the
// ground colour shows through.
//
// Geometry: a 1x1 QuadMesh expanded into a camera-facing square covering the pool ellipsoid
// around the node origin, pulled that far towards the camera so every surface inside the light
// sphere is behind it (depth test on, no depth write).
shader_type spatial;
render_mode unshaded, blend_add, depth_draw_never, cull_disabled, shadows_disabled, fog_disabled;

// Shared effect clock (clock.rs), used instead of TIME so the editor can idle.
global uniform float pt_time;

uniform sampler2D depth_tex : hint_depth_texture, repeat_disable, filter_nearest;

uniform vec4 color : source_color = vec4(1.0, 0.75, 0.4, 1.0);
/// Pool radius (world units).
uniform float radius = 3.0;
/// Pool brightness at the centre.
uniform float pool_energy = 0.35;
/// Falloff exponent of the pool.
uniform float falloff = 1.3;
/// Vertical distance counts this much (< 1 spreads a raised lamp's pool over the ground).
uniform float vertical_scale = 0.7;
uniform int glow_steps : hint_range(1, 8) = 4;
/// Width of the dithered transition between brightness steps (0 = hard steps, 1 = dither the
/// whole step).
uniform float dither_width : hint_range(0.0, 1.0) = 0.45;
/// Source orb radius (world units, 0 = no glow) and brightness.
uniform float glow_radius = 0.55;
uniform float glow_energy = 0.62;
uniform float glow_falloff_pow = 1.55;
uniform float flicker = 0.15;
uniform float night_boost = 1.0;

global uniform float pt_night;

// View-space position of the expanded disc (VERTEX in fragment() would still be the 1x1 quad).
varying vec3 disc_view;

float glow_bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

// Quantises v (0..1) into `steps` levels whose edges are ordered-dithered over `width`.
float band_falloff(float v, float steps, float width, float threshold) {
	float x = clamp(v, 0.0, 1.0) * steps;
	float cell = floor(x);
	float f = x - cell;
	float t = width > 0.001 ? clamp((f - (1.0 - width)) / width, 0.0, 1.0) : step(1.0, f);
	return (cell + step(threshold, t)) / steps;
}

void vertex() {
	vec3 centre = (MODELVIEW_MATRIX * vec4(0.0, 0.0, 0.0, 1.0)).xyz;
	// The pool is an ellipsoid stretched vertically by 1 / vertical_scale.
	float r = max(radius / clamp(vertical_scale, 0.25, 1.0), glow_radius);
	vec3 v = centre + vec3(VERTEX.xy * 2.0 * r, r);
	v.z = min(v.z, -0.05);
	disc_view = v;
	POSITION = PROJECTION_MATRIX * vec4(v, 1.0);
}

void fragment() {
	float depth = textureLod(depth_tex, SCREEN_UV, 0.0).r;
#if CURRENT_RENDERER == RENDERER_COMPATIBILITY
	vec3 ndc = vec3(SCREEN_UV, depth) * 2.0 - 1.0;
#else
	vec3 ndc = vec3(SCREEN_UV * 2.0 - 1.0, depth);
#endif
	vec4 sv = INV_PROJECTION_MATRIX * vec4(ndc, 1.0);
	sv.xyz /= sv.w;
	vec3 scene_pos = (INV_VIEW_MATRIX * vec4(sv.xyz, 1.0)).xyz;
	vec3 frag = (INV_VIEW_MATRIX * vec4(disc_view, 1.0)).xyz;
	vec3 lp = NODE_POSITION_WORLD;
	float dither = glow_bayer4(FRAGCOORD.xy);
	float steps = float(max(glow_steps, 1));

	// Pool: radial falloff on the reconstructed surface.
	vec3 dv = scene_pos - lp;
	dv.y *= vertical_scale;
	float nd = clamp(length(dv) / max(radius, 1e-4), 0.0, 1.0);
	float pool = pow(1.0 - nd, max(falloff, 1e-3));
	pool = band_falloff(pool, steps, dither_width, dither);

	// Glow: the light's perpendicular distance from the view ray, cut by anything nearer.
	bool ortho = PROJECTION_MATRIX[3][3] > 0.5;
	vec3 ray_dir = ortho ? -INV_VIEW_MATRIX[2].xyz : normalize(frag - INV_VIEW_MATRIX[3].xyz);
	vec3 to_light = lp - frag;
	float glow_t = dot(to_light, ray_dir);
	float glow_nd = length(to_light - ray_dir * glow_t) / max(glow_radius, 1e-4);
	float glow = glow_radius > 0.0 ? pow(clamp(1.0 - glow_nd, 0.0, 1.0), max(glow_falloff_pow, 1e-3)) : 0.0;
	float scene_t = dot(scene_pos - frag, ray_dir);
	glow *= step(0.0, scene_t - glow_t);
	glow = band_falloff(glow, steps, dither_width, dither);

	float fl = 1.0 - flicker * (0.5 + 0.5 * sin(pt_time * 13.0) * sin(pt_time * 7.3 + 1.7));
	float vis = mix(1.0, mix(0.35, 1.0, pt_night), night_boost);
	ALBEDO = color.rgb * (pool * pool_energy + glow * glow_energy) * fl * vis;
}
