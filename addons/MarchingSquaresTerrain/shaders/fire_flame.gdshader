// Marching Squares Terrain - PixelFireSystem flames (MultiMesh of camera-facing quads).
// INSTANCE_CUSTOM = (age 0..1, seed, kind, heat); kind 0 = flame tongue, 1 = ember spark.
// A flame is a lumpy teardrop (wide base, licking tip) in three hard bands: a pale yellow core,
// orange body and a red rim, shrinking and reddening with age and eroding from the rim with an
// ordered dither, so overlapping tongues merge into the blobby pixel-art fire of the reference.
// Emissive (lights barely touch it), opaque, writes the metadata pass like PixelSmoke puffs.
shader_type spatial;
render_mode specular_disabled, cull_disabled, shadows_disabled;

#define PT_FLAGS_MATERIAL
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

uniform vec4 core_color : source_color = vec4(1.0, 0.93, 0.55, 1.0);
uniform vec4 flame_color : source_color = vec4(1.0, 0.62, 0.16, 1.0);
uniform vec4 rim_color : source_color = vec4(0.86, 0.2, 0.12, 1.0);
uniform vec4 ember_color : source_color = vec4(1.0, 0.45, 0.15, 1.0);
/// Rim bumps of a tongue.
uniform float lumps = 5.0;
uniform float lumpiness : hint_range(0.0, 0.5) = 0.16;
/// How much narrower the tip is than the base.
uniform float taper : hint_range(0.0, 2.0) = 1.1;
uniform float churn_speed = 3.0;

varying vec4 custom;

float bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

void vertex() {
	custom = INSTANCE_CUSTOM;
	vec3 centre = MODEL_MATRIX[3].xyz;
	float s = length(MODEL_MATRIX[0].xyz);
	if ((pt_flags & 64) != 0 && pt_texel_world > 0.0) {
		vec3 o = (VIEW_MATRIX * vec4(centre, 1.0)).xyz;
		o.xy = round(o.xy / pt_texel_world) * pt_texel_world;
		centre = (INV_VIEW_MATRIX * vec4(o, 1.0)).xyz;
	}
	// Camera-facing, but kept upright (world up projected on the view plane) so tongues lick
	// upwards from steep top-down views too.
	vec3 right = INV_VIEW_MATRIX[0].xyz;
	vec3 up = INV_VIEW_MATRIX[1].xyz;
	mat4 bb = mat4(vec4(right * s, 0.0), vec4(up * s, 0.0), INV_VIEW_MATRIX[2] * s, vec4(centre, 1.0));
	MODELVIEW_MATRIX = VIEW_MATRIX * bb;
}

void fragment() {
	float age = custom.x;
	float seed = custom.y;
	int kind = int(custom.z + 0.5);
	float heat = custom.w;
	vec2 p = vec2(UV.x * 2.0 - 1.0, 1.0 - UV.y * 2.0);
	vec3 col;
	if (kind == 1) {
		// Ember: a tiny square spark that blinks out.
		if (max(abs(p.x), abs(p.y)) > 0.7 || bayer4(FRAGCOORD.xy) < age * age) {
			discard;
		}
		col = mix(core_color.rgb, ember_color.rgb, step(0.4, age));
	} else {
		// Teardrop: base centred a bit below the quad centre, narrowing towards the tip.
		vec2 q = vec2(p.x, p.y + 0.25);
		float up = max(q.y, 0.0);
		float sway = sin(pt_time * churn_speed * 1.7 + seed * 31.0 + up * 3.0) * 0.18 * up;
		q.x = (q.x - sway) * (1.0 + up * taper);
		q.y *= q.y > 0.0 ? 0.62 : 1.25;
		float a = atan(q.y, q.x);
		float ph = seed * 40.0 + pt_time * churn_speed * (seed > 0.5 ? 1.0 : -1.0);
		float rim = 1.0 - lumpiness * (0.5 + 0.5 * sin(a * lumps + ph)) * (0.6 + 0.4 * sin(a * (lumps - 2.0) - ph * 1.3));
		float r = length(q) / max(rim, 0.05);
		float fade = smoothstep(0.45, 1.0, age);
		float density = (1.0 - r * r) * 3.0 * (1.0 - fade);
		if (r >= 1.0 || density < bayer4(FRAGCOORD.xy)) {
			discard;
		}
		// Bands move outwards-in as the tongue ages: young hot tongues have a big yellow core.
		float hot = clamp(heat * 1.1 - age * 0.7, 0.0, 1.0);
		float core_r = 0.25 + 0.35 * hot;
		float body_r = 0.55 + 0.3 * hot;
		col = r < core_r ? core_color.rgb : (r < body_r ? flame_color.rgb : rim_color.rgb);
	}
	ALBEDO = col * 0.05;
	EMISSION = col;
	PT_META_OUTPUT(vec3(0.0, 0.0, 1.0))
}
