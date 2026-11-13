// Marching Squares Terrain - PixelSmoke puffs (MultiMesh of camera-facing quads).
// INSTANCE_CUSTOM = (age 0..1, seed, kind, 0); kind 0 = smoke, 1 = flame, 2 = dust burst.
// Each quad is a rounded, slightly lumpy pixel puff: a fake sphere normal lets the scene lights
// shade it in toon bands (lit cap, shaded underside), the colour steps from soot to pale smoke
// with age, and the puff erodes from its rim inwards with an ordered dither (no alpha blending,
// so the pipeline stays opaque and the puff still writes the metadata pass). Flames are emissive.
shader_type spatial;
render_mode specular_disabled, cull_disabled;

#define PT_FLAGS_MATERIAL
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

uniform vec4 smoke_color : source_color = vec4(0.8, 0.8, 0.82, 1.0);
/// Colour of young smoke (soot) right above the source.
uniform vec4 soot_color : source_color = vec4(0.42, 0.41, 0.44, 1.0);
uniform vec4 flame_color : source_color = vec4(1.0, 0.55, 0.15, 1.0);
uniform vec4 flame_core : source_color = vec4(1.0, 0.88, 0.4, 1.0);
uniform vec4 dust_color : source_color = vec4(0.72, 0.64, 0.52, 1.0);
/// Age (0..1) at which a puff starts to dissolve.
uniform float dissolve_start : hint_range(0.0, 1.0) = 0.2;
/// Bumps around the rim of a puff (0 = perfect discs).
uniform float lumps = 6.0;
/// Rim bump depth.
uniform float lumpiness : hint_range(0.0, 0.5) = 0.12;
/// Brightness of the puff's shaded underside and rim.
uniform float rim_shade : hint_range(0.0, 1.0) = 0.8;
/// How fast the rim bumps churn.
uniform float churn_speed = 0.8;

varying vec3 world_pos;
varying float cloud;
varying vec4 custom;

float bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

void vertex() {
	custom = INSTANCE_CUSTOM;
	// Camera-facing billboard of the instance's (uniform) scale, centred on the instance.
	vec3 centre = MODEL_MATRIX[3].xyz;
	float s = length(MODEL_MATRIX[0].xyz);
	if ((pt_flags & 64) != 0 && pt_texel_world > 0.0) {
		vec3 o = (VIEW_MATRIX * vec4(centre, 1.0)).xyz;
		o.xy = round(o.xy / pt_texel_world) * pt_texel_world;
		centre = (INV_VIEW_MATRIX * vec4(o, 1.0)).xyz;
	}
	mat4 bb = mat4(INV_VIEW_MATRIX[0] * s, INV_VIEW_MATRIX[1] * s, INV_VIEW_MATRIX[2] * s, vec4(centre, 1.0));
	MODELVIEW_MATRIX = VIEW_MATRIX * bb;
	world_pos = centre;
}

void fragment() {
	float age = custom.x;
	float seed = custom.y;
	int kind = int(custom.z + 0.5);
	vec2 p = vec2(UV.x * 2.0 - 1.0, 1.0 - UV.y * 2.0);
	// Lumpy rim: the radius wobbles with angle, churning slowly per puff.
	float a = atan(p.y, p.x);
	float ph = seed * 40.0 + pt_time * churn_speed * (seed > 0.5 ? 1.0 : -1.0);
	float rim = 1.0 - lumpiness * (0.5 + 0.5 * sin(a * lumps + ph)) * (0.6 + 0.4 * sin(a * (lumps - 2.0) - ph * 1.3));
	float r = length(p) / max(rim, 0.05);
	// Dome density: thick in the middle, thin at the rim, thinning with age. The Bayer threshold
	// erodes it from the rim inwards, so the outline stays rounded and only the rim is dithered.
	// Smoke thins as it rises: older puffs keep only a sparse dithered core, so the top of the
	// plume frays into the sky instead of ending in a solid blob.
	float fade = smoothstep(dissolve_start, 1.0, age);
	float thick = kind == 0 ? mix(2.2, 0.0, pow(fade, 0.7)) : 3.0 * (1.0 - fade) * (1.0 - fade);
	float density = (1.0 - r * r) * thick;
	if (r >= 1.0 || density < bayer4(FRAGCOORD.xy)) {
		discard;
	}
	if (pt_fog_hides(world_pos, FRAGCOORD.xy)) {
		discard; // fog of war: no smoke over cells the player cannot see
	}
	vec3 n = normalize(vec3(p, sqrt(max(1.0 - min(r * r, 1.0), 0.0)) + 0.35));
	NORMAL = n;
	vec3 col;
	if (kind == 1) {
		col = mix(flame_core.rgb, flame_color.rgb, max(step(0.35, age), step(0.6, r)));
		ALBEDO = col * 0.3;
		EMISSION = col * 1.2;
	} else if (kind == 2) {
		ALBEDO = dust_color.rgb;
	} else {
		// Soot near the source, then a couple of palette steps towards pale smoke.
		ALBEDO = mix(soot_color.rgb, smoke_color.rgb, step(0.08, age) * 0.5 + step(0.22, age) * 0.5);
	}
	if (kind != 1) {
		// Form shading so overlapping puffs stay readable: a shaded crescent on the underside
		// and a slightly darker rim.
		// Young puffs only: the thinning plume above reads as one soft mass, not stacked balls.
		float under = step(0.35, -p.y * 0.85 + p.x * 0.35 + (r - 0.55));
		float edge = step(0.82, r);
		ALBEDO *= mix(1.0, rim_shade, max(under, edge * 0.6) * (1.0 - step(0.3, age)));
	}
	cloud = pt_cloud_shadow(world_pos);
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(2, 0.65, cloud)
}
