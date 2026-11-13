// Marching Squares Terrain - raised water pools and waterfalls (PixelWater "Waterfalls", built by
// pt_core::waterfall). One opaque mesh; COLOR.r * 4 picks the part:
//   0 pool surface   UV = world xz
//   1 fall sheet     UV = (distance along the edge, world y), UV2 = (0 lip .. 1 base, drop)
//   2 base foam      UV = world xz, UV2.x = 0 at the fall .. 1 at the outer rim (into water)
//   3 spray puff     UV = (phase, size), UV2 = billboard corner; expanded here, facing the camera
//   4 landing basin  like 2, on dry ground: a small splash pool with a churning foam rim
// Fall streaks after catlikecoding's Hex Map river shader (via the MIT hex_map_godot port,
// water.gdshaderinc compute_river: two noise layers scrolling down at slightly different speeds,
// multiplied), here in pixel-art form: per-column streaks quantised to three colours, a glassy lip
// and churning foam towards the base. Spray fades with an ordered dither (no transparency in the
// pixel pipeline). Foam and spray take the sun's hue at near full brightness instead of the
// toon N.L shading, so they stay white-ish (warm at dusk) rather than turning grey.
shader_type spatial;
render_mode specular_disabled, cull_disabled;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Fog of war on non-terrain surfaces (pt_fog_surface).
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

group_uniforms Colour;
uniform vec4 water_color : source_color = vec4(0.17, 0.43, 0.58, 1.0);
uniform vec4 deep_color : source_color = vec4(0.10, 0.28, 0.44, 1.0);
uniform vec4 streak_color : source_color = vec4(0.44, 0.72, 0.82, 1.0);
uniform vec4 foam_color : source_color = vec4(0.86, 0.94, 0.95, 1.0);
group_uniforms;

group_uniforms Fall;
/// Speed of the streaks running down a fall (m/s).
uniform float fall_speed = 3.2;
/// Streak columns per metre of fall width.
uniform float streak_density = 4.0;
/// Length of the streaks (higher = longer).
uniform float streak_length = 0.7;
/// Share of the fall (from the base up) that churns into foam.
uniform float base_foam : hint_range(0.0, 1.0) = 0.22;
group_uniforms;

group_uniforms Foam_And_Spray;
uniform float foam_speed = 0.8;
uniform float spray_rate = 0.7;
/// How high the spray puffs rise, in puff sizes.
uniform float spray_rise = 0.7;
/// Brightness of foam and spray relative to full sunlight (day / night).
uniform float foam_light : hint_range(0.0, 1.5) = 0.95;
uniform float foam_night_light : hint_range(0.0, 1.5) = 0.3;
group_uniforms;

group_uniforms Pool;
uniform float ripple_scale = 1.4;
uniform float ripple_speed = 0.35;
group_uniforms;

group_uniforms Lighting;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.6;
group_uniforms;

varying flat float part;
varying vec3 world_pos;
varying float spray_t;
varying vec2 corner;
varying float cloud;
varying float foamy;

float bayer4(vec2 p) {
	ivec2 q = ivec2(mod(p, 4.0));
	int idx = q.x + q.y * 4;
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[idx]) + 0.5) / 16.0;
}

void vertex() {
	part = round(COLOR.r * 4.0);
	spray_t = 0.0;
	corner = UV2;
	if (part > 2.5 && part < 3.5) {
		// Spray: a puff rising from the base, growing and drifting, then respawning.
		float phase = UV.x;
		float size = UV.y;
		float t = fract(TIME * spray_rate + phase);
		spray_t = t;
		mat3 to_model = mat3(inverse(MODEL_MATRIX));
		vec3 right = normalize(to_model * INV_VIEW_MATRIX[0].xyz);
		vec3 up = normalize(to_model * INV_VIEW_MATRIX[1].xyz);
		vec3 drift = vec3(sin(phase * 37.0), 0.0, cos(phase * 23.0)) * 0.25 * t;
		// Billows up fast, then hangs and thins out (no tall column next to the fall).
		float rise = 1.0 - (1.0 - t) * (1.0 - t);
		vec3 c = VERTEX + vec3(0.0, rise * size * spray_rise, 0.0) + drift;
		float s = size * (0.6 + 0.6 * rise);
		VERTEX = c + (right * UV2.x + up * UV2.y) * s * 0.5;
		NORMAL = normalize(to_model * INV_VIEW_MATRIX[2].xyz);
	}
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec3 col = water_color.rgb;
	foamy = 0.0;
	vec3 meta_n = (VIEW_MATRIX * vec4(0.0, 1.0, 0.0, 0.0)).xyz;
	if (part < 0.5) {
		// Pool: calm, banded ripples drifting downstream-ish.
		float n = pt_vnoise(world_pos.xz * ripple_scale + vec2(TIME * ripple_speed, TIME * ripple_speed * 0.6));
		float n2 = pt_vnoise(world_pos.xz * ripple_scale * 2.1 - vec2(TIME * ripple_speed * 0.8, 0.0));
		float r = n * 0.65 + n2 * 0.35;
		col = mix(water_color.rgb, deep_color.rgb, 0.35);
		if (r > 0.7) {
			col = streak_color.rgb;
		} else if (r < 0.3) {
			col = deep_color.rgb;
		}
	} else if (part < 1.5) {
		// Fall sheet: pixel streak columns scrolling down (compute_river, two layers).
		float colw = 1.0 / max(streak_density, 0.1);
		float ci = floor(UV.x / colw);
		float h = pt_hash(vec2(ci, 3.7));
		float y = UV.y + TIME * fall_speed * (0.85 + 0.3 * h);
		float n1 = pt_vnoise(vec2(ci * 1.37, y / max(streak_length, 0.05)));
		float n2 = pt_vnoise(vec2(UV.x * 0.9 + TIME * 0.05, (UV.y + TIME * fall_speed * 0.92) * 0.45));
		float river = n1 * (0.55 + n2 * 0.9);
		float lip = corner.x;
		// Churn towards the base.
		river += smoothstep(1.0 - base_foam, 1.0, lip) * 0.6;
		// Columns alternate between two blues; dashes of light water and foam run down them.
		col = mix(water_color.rgb, deep_color.rgb, step(0.55, h) * 0.6);
		if (river > 0.74) {
			col = foam_color.rgb;
			foamy = 1.0;
		} else if (river > 0.5) {
			col = streak_color.rgb;
		}
		// Glassy highlight on the lip where the water tips over.
		if (lip < 0.06 && corner.y > 0.0) {
			col = streak_color.rgb;
		}
		meta_n = NORMAL;
	} else if (part < 2.5) {
		// Base foam: churning, fraying out towards the rim.
		float f = pt_vnoise(world_pos.xz * 2.4 + vec2(TIME * foam_speed, -TIME * foam_speed * 0.7)) * 0.6
				+ pt_vnoise(world_pos.xz * 1.1 - vec2(TIME * foam_speed * 0.5)) * 0.4;
		float edge = corner.x;
		if (f < 0.25 + edge * 0.5) {
			discard;
		}
		col = f > 0.5 + edge * 0.3 ? foam_color.rgb : streak_color.rgb;
		foamy = f > 0.5 + edge * 0.3 ? 1.0 : 0.0;
	} else if (part > 3.5) {
		// Landing basin: a splash pool on the ground, churned white where the fall hits it and
		// along a frayed foam rim.
		float edge = corner.x;
		float f = pt_vnoise(world_pos.xz * 2.6 + vec2(TIME * foam_speed, -TIME * foam_speed * 0.7)) * 0.6
				+ pt_vnoise(world_pos.xz * 1.2 - vec2(TIME * foam_speed * 0.5)) * 0.4;
		float ring = smoothstep(0.62, 0.9, edge);
		if (edge > 0.8 && f < 0.3 + (edge - 0.8) * 2.5) {
			discard;
		}
		float churn = (1.0 - smoothstep(0.0, 0.45, edge)) * 0.55 + ring * 0.6;
		float v = f + churn;
		if (v > 0.95) {
			col = foam_color.rgb;
			foamy = 1.0;
		} else if (v > 0.72) {
			col = streak_color.rgb;
		} else {
			col = f > 0.55 ? water_color.rgb : mix(water_color.rgb, deep_color.rgb, 0.5);
		}
	} else {
		// Spray: soft round puffs, dithered denser in the middle, thinning out as they age.
		float d = length(corner);
		float a = (1.0 - spray_t * spray_t) * (1.0 - d * d) * 1.6;
		if (d > 1.0 || bayer4(FRAGCOORD.xy) > a) {
			discard;
		}
		col = foam_color.rgb;
		foamy = 1.0;
	}
	ALBEDO = col;
	cloud = pt_cloud_shadow(world_pos);
	{ vec4 pt_fs = pt_fog_surface(ALBEDO, world_pos, mst_pixel_lattice(world_pos, INV_VIEW_MATRIX, FRAGCOORD.xy)); ALBEDO = pt_fs.rgb; EMISSION *= pt_fs.a; } // [fog of war]
	PT_META_OUTPUT(meta_n)
}

void light() {
	if (foamy > 0.5) {
		// Foam / spray: lit whatever its angle (it scatters light), but only as bright as the
		// light really is, so it dims with the scene at sunset / night like the water instead
		// of glowing. The hue is softened towards white so a low red sun does not turn it pink.
		if (LIGHT_IS_DIRECTIONAL) {
			float pk = max(max(LIGHT_COLOR.r, LIGHT_COLOR.g), max(LIGHT_COLOR.b, 1e-3));
			float k = pk / PI * mix(0.75, 1.0, cloud) * mix(foam_light, foam_night_light, pt_night);
			DIFFUSE_LIGHT += mix(LIGHT_COLOR / pk, vec3(1.0), 0.4) * k;
		} else {
			DIFFUSE_LIGHT += LIGHT_COLOR / PI * ATTENUATION;
		}
	} else {
		PT_TOON_LIGHT(bands, shade_floor, cloud)
	}
}
