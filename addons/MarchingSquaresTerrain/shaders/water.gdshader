// Marching Squares Terrain - opaque banded water. Depth comes from the terrain height texture that
// MarchingSquaresTerrain publishes (pt_terrain_height / pt_terrain_rect), so shorelines, depth bands
// and foam need no depth buffer (works in the Compatibility renderer).
//
// Look (after the ideas in docs/port/03-refs-features.md §1.3 / #8): a banded
// Gerstner swell, a fresnel "grazing" tint, a sun-side warm / away-side cool sky reflection band,
// shallow turquoise, banded caustics in the shallows (the same pattern PixelWater publishes to the
// terrain through pt_water_b), shoreline foam that surges in pulses with a pause between waves, a
// dithered rim between depth bands and rain-drop rings while PixelWeather makes the ground wet.
// Everything is quantised, so it stays pixel art. The metadata pass sees a flat normal (no
// outlines inside the water), lighting sees the wave normal.
shader_type spatial;
render_mode specular_disabled, cull_back;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Fog of war on non-terrain surfaces (pt_fog_surface).
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_weather.gdshaderinc"

global uniform sampler2D pt_terrain_height;
global uniform vec4 pt_terrain_rect;

group_uniforms Colour;
uniform vec4 shallow_color : source_color = vec4(0.36, 0.72, 0.74, 1.0);
uniform vec4 deep_color : source_color = vec4(0.10, 0.27, 0.42, 1.0);
/// Turquoise of very shallow water and of sun-lit wave crests.
uniform vec4 turquoise_color : source_color = vec4(0.36, 0.7, 0.66, 1.0);
/// Depth (m) over which the shallow turquoise fades out.
uniform float turquoise_depth : hint_range(0.0, 8.0) = 0.6;
/// Tint at grazing angles (fresnel), darker than the body colour.
uniform vec4 grazing_color : source_color = vec4(0.07, 0.2, 0.27, 1.0);
uniform float grazing_strength : hint_range(0.0, 1.0) = 0.55;
/// Depth (m) at which the water reaches deep_color.
uniform float depth_scale = 4.0;
uniform int depth_bands : hint_range(1, 8) = 4;
/// Ordered dither between depth bands (0 = hard steps).
uniform float rim_dither : hint_range(0.0, 1.0) = 0.6;
group_uniforms;

group_uniforms Reflection;
/// Sky reflected on the side of the waves facing the sun.
uniform vec4 sky_toward_sun : source_color = vec4(1.0, 0.84, 0.6, 1.0);
/// Sky reflected on the side facing away from the sun.
uniform vec4 sky_away_from_sun : source_color = vec4(0.36, 0.48, 0.74, 1.0);
uniform float reflection_strength : hint_range(0.0, 1.0) = 0.3;
uniform int reflection_bands : hint_range(1, 6) = 2;
/// Sun glints on wave facets (0 = off).
uniform float sparkle = 0.6;
group_uniforms;

group_uniforms Swell;
/// Gerstner swell amplitude (m). Needs some subdivisions on the PixelWater plane.
uniform float swell_height : hint_range(0.0, 2.0) = 0.12;
/// Longest wavelength (m); two shorter waves follow.
uniform float swell_length = 12.0;
uniform float swell_steepness : hint_range(0.0, 1.0) = 0.45;
uniform float swell_speed = 1.0;
/// Main wave direction (XZ).
uniform vec2 swell_direction = vec2(1.0, 0.35);
/// Small chop on top of the swell (legacy waves).
uniform float wave_height = 0.06;
uniform float wave_speed = 1.1;
uniform float wave_scale = 0.7;
group_uniforms;

group_uniforms Foam;
uniform vec4 foam_color : source_color = vec4(0.93, 0.97, 0.95, 1.0);
/// Width of the shore foam band (m, measured from the water line).
uniform float foam_width = 0.7;
uniform float foam_speed = 0.6;
/// Shore surges per second.
uniform float foam_pulse_speed = 0.35;
/// Fraction of each surge cycle with no surge (the pause between waves).
uniform float foam_dead_time : hint_range(0.0, 0.95) = 0.45;
/// How far a surge pushes the foam out, in foam widths.
uniform float foam_surge = 1.5;
group_uniforms;

group_uniforms Lighting;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.55;
group_uniforms;

varying vec3 world_pos;
varying float cloud;
varying float crest;

float terrain_height(vec2 xz) {
	if (pt_terrain_rect.z <= 0.0 || pt_terrain_rect.w <= 0.0) {
		return -1e6;
	}
	vec2 uv = (xz - pt_terrain_rect.xy) / pt_terrain_rect.zw;
	if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
		return -1e6;
	}
	ivec2 size = textureSize(pt_terrain_height, 0);
	vec2 f = uv * vec2(size - ivec2(1));
	ivec2 i = ivec2(floor(f));
	vec2 fr = fract(f);
	ivec2 m = size - ivec2(1);
	float a = texelFetch(pt_terrain_height, clamp(i, ivec2(0), m), 0).r;
	float b = texelFetch(pt_terrain_height, clamp(i + ivec2(1, 0), ivec2(0), m), 0).r;
	float c = texelFetch(pt_terrain_height, clamp(i + ivec2(0, 1), ivec2(0), m), 0).r;
	float d = texelFetch(pt_terrain_height, clamp(i + ivec2(1, 1), ivec2(0), m), 0).r;
	return mix(mix(a, b, fr.x), mix(c, d, fr.x), fr.y);
}

// Sum of three Gerstner waves at rest position xz: returns displacement (xyz) and writes the
// surface normal. `amp` scales the amplitude (damped in the shallows).
vec3 gerstner(vec2 xz, float t, float amp, out vec3 normal) {
	vec2 d0 = normalize(swell_direction + vec2(1e-4));
	vec2 dirs[3] = { d0, normalize(vec2(d0.x * 0.8 - d0.y * 0.6, d0.x * 0.6 + d0.y * 0.8)), normalize(vec2(d0.x * 0.9 + d0.y * 0.45, -d0.x * 0.45 + d0.y * 0.9)) };
	float lens[3] = { 1.0, 0.61, 0.37 };
	float amps[3] = { 1.0, 0.55, 0.3 };
	vec3 disp = vec3(0.0);
	vec3 n = vec3(0.0, 1.0, 0.0);
	for (int i = 0; i < 3; i++) {
		float k = 6.2831 / max(swell_length * lens[i], 0.1);
		float c = sqrt(9.8 / k) * swell_speed;
		float a = swell_height * amps[i] * amp;
		float q = swell_steepness / (k * max(swell_height * 3.0, 1e-3));
		q = min(q, 1.0);
		float f = k * (dot(dirs[i], xz) - c * t);
		float cf = cos(f);
		float sf = sin(f);
		disp += vec3(q * a * dirs[i].x * cf, a * sf, q * a * dirs[i].y * cf);
		n.x -= dirs[i].x * k * a * cf;
		n.z -= dirs[i].y * k * a * cf;
		n.y -= q * k * a * sf;
	}
	normal = normalize(n);
	return disp;
}

float bayer4(vec2 p) {
	ivec2 q = ivec2(mod(p, 4.0));
	int idx = q.x + q.y * 4;
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[idx]) + 0.5) / 16.0;
}

void vertex() {
	vec3 wp = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	float ground = terrain_height(wp.xz);
	float depth = ground < -1e5 ? depth_scale : max(wp.y - ground, 0.0);
	// The swell dies down in the shallows so it does not slosh through the shore.
	float damp = clamp(depth / 3.0, 0.0, 1.0);
	vec3 n;
	vec3 g = gerstner(wp.xz, TIME, damp, n);
	float chop = sin(wp.x * wave_scale + TIME * wave_speed) + cos(wp.z * wave_scale * 1.31 + TIME * wave_speed * 0.83);
	g.y += chop * wave_height * 0.5;
	crest = g.y / max(swell_height * 1.6 * damp, 1e-3);
	VERTEX += (inverse(MODEL_MATRIX) * vec4(g, 0.0)).xyz;
	world_pos = wp + g;
}

void fragment() {
	float ground = terrain_height(world_pos.xz);
	float depth = ground < -1e5 ? depth_scale : max(world_pos.y - ground, 0.0);
	vec3 wn;
	gerstner(world_pos.xz, TIME, clamp(depth / 3.0, 0.0, 1.0), wn);

	// Depth bands with an ordered-dither rim between them.
	float nb = float(depth_bands);
	float t = clamp(depth / depth_scale, 0.0, 1.0);
	t = clamp(floor(t * nb + 0.5 + (bayer4(FRAGCOORD.xy) - 0.5) * rim_dither) / nb, 0.0, 1.0);
	vec3 col = mix(shallow_color.rgb, deep_color.rgb, t);

	// Shallow turquoise + sun-lit crests.
	float tq = 1.0 - clamp(depth / max(turquoise_depth, 0.01), 0.0, 1.0);
	tq = floor(tq * 2.0 + 0.5) / 2.0;
	float lit_crest = step(0.55, crest) * step(0.0, dot(wn.xz, -normalize(pt_sun_dir.xz + vec2(1e-4)))) * (1.0 - pt_night);
	col = mix(col, turquoise_color.rgb, max(tq * 0.6, lit_crest * 0.4));

	// Banded caustics in the shallows (same field and settings as on the terrain).
	if (pt_water_a.x > 0.5 && pt_water_b.x > 0.0) {
		float fade = 1.0 - clamp(depth / 2.5, 0.0, 1.0);
		float cb = max(pt_water_b.z, 1.0);
		float c = floor(pt_caustic(world_pos.xz, TIME, pt_water_b.y) * fade * cb) / cb;
		col += PT_CAUSTIC_COLOR * c * pt_water_b.x * 0.35;
	}

	// Fresnel grazing tint and the sun-side / away-side sky reflection band.
	vec3 v = normalize(CAMERA_POSITION_WORLD - world_pos);
	float fres = 1.0 - clamp(dot(wn, v), 0.0, 1.0);
	fres = floor(fres * 4.0) / 4.0;
	col = mix(col, grazing_color.rgb, grazing_strength * step(0.5, fres));
	vec3 r = reflect(-v, wn);
	vec2 sun_xz = normalize(pt_sun_dir.xz + vec2(1e-4));
	float side = dot(normalize(r.xz + vec2(1e-4)), sun_xz) * 0.5 + 0.5;
	float rb = float(reflection_bands);
	side = floor(side * rb + 0.5) / rb;
	vec3 sky = mix(sky_away_from_sun.rgb, sky_toward_sun.rgb * (1.0 - pt_night * 0.7), side);
	col = mix(col, sky, reflection_strength * (0.4 + fres));
	if (sparkle > 0.0 && pt_night < 0.5 && depth > 0.3 && dot(r, normalize(pt_sun_dir)) > 1.0 - 0.012 * sparkle) {
		col = mix(col, foam_color.rgb, 0.8);
	}

	// Shore foam: a noisy band that surges out in pulses, then rests.
	float n = pt_vnoise(world_pos.xz * 1.7 + vec2(TIME * foam_speed, -TIME * foam_speed * 0.7));
	float phase = fract(TIME * foam_pulse_speed + pt_vnoise(world_pos.xz * 0.05) * 0.5);
	float live = 1.0 - foam_dead_time;
	float surge = phase < live ? sin(3.14159 * phase / live) : 0.0;
	// Distance to the shore line ~ depth / ground slope, so the foam stays a band of constant
	// width even over very flat shallows (where depth alone would foam the whole flat).
	vec2 e = vec2(0.5, 0.0);
	float hx0 = terrain_height(world_pos.xz - e.xy);
	float hx1 = terrain_height(world_pos.xz + e.xy);
	float hz0 = terrain_height(world_pos.xz - e.yx);
	float hz1 = terrain_height(world_pos.xz + e.yx);
	float slope = min(min(hx0, hx1), min(hz0, hz1)) < -1e5 ? 0.0 : length(vec2(hx1 - hx0, hz1 - hz0));
	float shore = depth / max(slope, 0.04);
	float reach = foam_width * (0.55 + n * 0.9);
	if (shore < reach) {
		col = foam_color.rgb;
	} else if (shore < reach + surge * foam_surge * foam_width && n > 0.35 && bayer4(FRAGCOORD.xy) < surge) {
		col = mix(col, foam_color.rgb, 0.85);
	}

	// Rain-drop rings while it rains (pt_wet from PixelWeather).
	if (pt_wet > 0.01) {
		vec2 g = world_pos.xz * 1.4;
		vec2 cell = floor(g);
		float h = pt_hash(cell);
		float age = fract(TIME * 1.6 + h * 7.0);
		vec2 centre = cell + 0.25 + 0.5 * vec2(pt_hash(cell + 3.1), pt_hash(cell + 7.7));
		float dist = length(g - centre);
		if (h < pt_wet * 0.8 && abs(dist - age * 0.45) < 0.05) {
			col = mix(col, foam_color.rgb, 0.45 * (1.0 - age));
		}
	}

	ALBEDO = col;
	NORMAL = (VIEW_MATRIX * vec4(wn, 0.0)).xyz;
	cloud = pt_cloud_shadow(world_pos);
	vec3 flat_n = (VIEW_MATRIX * vec4(0.0, 1.0, 0.0, 0.0)).xyz;
	{ vec4 pt_fs = pt_fog_surface(ALBEDO, world_pos, mst_pixel_lattice(world_pos, INV_VIEW_MATRIX, FRAGCOORD.xy)); ALBEDO = pt_fs.rgb; EMISSION *= pt_fs.a; } // [fog of war]
	PT_META_OUTPUT(flat_n)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, cloud)
}
