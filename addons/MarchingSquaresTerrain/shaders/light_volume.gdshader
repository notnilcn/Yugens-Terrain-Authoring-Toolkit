// Marching Squares Terrain - PixelLightVolume: a pixel-art pool of lamplight plus a small source halo.
//
// Ported from Stable3D PixelArt Orthographic's `StablePixelArtStencilPointLight` (Lorenzo Santa
// Cruz, MIT; see CREDITS.md): the opaque surface under each pixel is rebuilt from the depth
// buffer and lit by a radial falloff (the "pool"), and a view-facing orb is evaluated from the
// view ray's closest approach to the light (the "halo"), hidden by geometry in front of it.
// Both are quantised into a few bands; instead of Stable3D's smoothstep between bands the band
// edges are an ordered (Bayer 4x4) dither in the low-res colour pass, so the glow reads as
// pixel-art lamplight with no hard disc edge. Blending is additive, so the ground colour shows
// through.
//
// Geometry: a 1x1 QuadMesh expanded into a camera-facing square covering the pool ellipsoid
// around the node origin, pulled that far towards the camera so every surface inside the light sphere is behind
// it (depth test on, no depth write).
shader_type spatial;
render_mode unshaded, blend_add, depth_draw_never, cull_disabled, shadows_disabled, fog_disabled;

uniform sampler2D depth_tex : hint_depth_texture, repeat_disable, filter_nearest;

uniform vec4 color : source_color = vec4(1.0, 0.75, 0.4, 1.0);
/// Pool radius (world units).
uniform float radius = 3.0;
/// Pool brightness at the centre.
uniform float intensity = 0.35;
/// Falloff exponent of the pool.
uniform float falloff = 1.3;
/// Vertical distance counts this much (< 1 spreads a raised lamp's pool over the ground).
uniform float vertical_scale = 0.7;
uniform int bands : hint_range(1, 8) = 4;
/// Width of the dithered transition between bands (0 = hard steps, 1 = dither the whole band).
uniform float dither_width : hint_range(0.0, 1.0) = 0.45;
/// Source orb radius (world units, 0 = no halo) and brightness.
uniform float halo_size = 0.5;
uniform float halo_intensity = 0.6;
uniform float halo_falloff = 1.5;
uniform float flicker = 0.15;
uniform float night_boost = 1.0;

global uniform float pt_night;

// View-space position of the expanded disc (VERTEX in fragment() would still be the 1x1 quad).
varying vec3 disc_view;

float lv_bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

// Quantises v (0..1) into `levels` steps whose edges are ordered-dithered over `width`.
float lv_band(float v, float levels, float width, float threshold) {
	float x = clamp(v, 0.0, 1.0) * levels;
	float cell = floor(x);
	float f = x - cell;
	float t = width > 0.001 ? clamp((f - (1.0 - width)) / width, 0.0, 1.0) : step(1.0, f);
	return (cell + step(threshold, t)) / levels;
}

void vertex() {
	vec3 centre = (MODELVIEW_MATRIX * vec4(0.0, 0.0, 0.0, 1.0)).xyz;
	// The pool is an ellipsoid stretched vertically by 1 / vertical_scale.
	float r = max(radius / clamp(vertical_scale, 0.25, 1.0), halo_size);
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
	vec3 scene = (INV_VIEW_MATRIX * vec4(sv.xyz, 1.0)).xyz;
	vec3 frag = (INV_VIEW_MATRIX * vec4(disc_view, 1.0)).xyz;
	vec3 lp = NODE_POSITION_WORLD;
	float th = lv_bayer4(FRAGCOORD.xy);
	float levels = float(max(bands, 1));

	// Pool: radial falloff on the reconstructed surface.
	vec3 dv = scene - lp;
	dv.y *= vertical_scale;
	float nd = clamp(length(dv) / max(radius, 1e-4), 0.0, 1.0);
	float pool = pow(1.0 - nd, max(falloff, 1e-3));
	pool = lv_band(pool, levels, dither_width, th);

	// Halo: closest approach of the view ray to the light centre.
	bool ortho = PROJECTION_MATRIX[3][3] > 0.5;
	vec3 ray = ortho ? -INV_VIEW_MATRIX[2].xyz : normalize(frag - INV_VIEW_MATRIX[3].xyz);
	float t_centre = dot(lp - frag, ray);
	float hd = distance(lp, frag + ray * t_centre) / max(halo_size, 1e-4);
	float halo = halo_size > 0.0 ? pow(clamp(1.0 - hd, 0.0, 1.0), max(halo_falloff, 1e-3)) : 0.0;
	halo *= step(t_centre, dot(scene - frag, ray));
	halo = lv_band(halo, levels, dither_width, th);

	float fl = 1.0 - flicker * (0.5 + 0.5 * sin(TIME * 13.0) * sin(TIME * 7.3 + 1.7));
	float vis = mix(1.0, mix(0.35, 1.0, pt_night), night_boost);
	ALBEDO = color.rgb * (pool * intensity + halo * halo_intensity) * fl * vis;
}
