// Marching Squares Terrain - upright (Y-axis) billboard grass / foliage card.
// Port of Pixelizer3D's grass_billboard.gdshader onto pixel_common (metadata, toon light,
// cloud shadows). Use it on a unit QuadMesh/PlaneMesh (MultiMesh friendly) under a PixelObject.
//
// Cards face the camera around Y only; the node origin is the blade base. Wind is world space,
// scaled by the card height and bent towards the tip, on a stepped clock (`wind_fps`) for
// low-frame-rate pixel animation. `atlas_cells` picks a per-instance cell (hash of the world
// origin + INSTANCE_ID); `animate_frames` cycles cells. Without a texture a procedural
// three-blade tuft is drawn. Alpha scissor only (never transparent).
shader_type spatial;
render_mode world_vertex_coords, cull_disabled, specular_disabled, depth_draw_opaque;
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

group_uniforms Albedo;
uniform sampler2D albedo_texture : source_color, filter_nearest, repeat_disable;
uniform bool use_albedo_texture = false;
uniform vec4 tint : source_color = vec4(0.45, 0.7, 0.3, 1.0);
/// Darker colour towards the blade base (procedural tuft and textures alike).
uniform vec4 base_tint : source_color = vec4(0.22, 0.42, 0.2, 1.0);
uniform float alpha_scissor_threshold : hint_range(0.0, 1.0) = 0.5;

group_uniforms Atlas;
uniform vec2 atlas_cells = vec2(1.0, 1.0);
uniform int variation_count = 1;
uniform bool animate_frames = false;
uniform float animation_fps = 6.0;
uniform int animation_frames = 4;

group_uniforms Wind;
uniform vec2 wind_direction = vec2(1.0, 0.25);
/// Sway amplitude as a fraction of the card height.
uniform float wind_strength = 0.12;
uniform float wind_speed = 1.3;
uniform float wind_turbulence = 0.35;
/// Quantise the wind clock to this frame rate (0 = continuous).
uniform float wind_fps = 8.0;

group_uniforms Shading;
uniform int bands : hint_range(1, 8) = 2;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.45;
group_uniforms;

varying vec2 f_uv;
varying vec2 f_local;
varying vec3 f_world;
varying float f_seed;

float pt_foliage_hash(vec3 origin, float instance_id) {
	vec2 cell = fract(origin.xz * 0.125 + instance_id * 0.6180339887);
	cell = fract(cell * vec2(91.7, 47.3) + vec2(0.37, 0.11));
	return fract(cell.x * cell.y * 197.0);
}

void vertex() {
	vec3 origin = MODEL_MATRIX[3].xyz;
	float seed = pt_foliage_hash(origin, float(INSTANCE_ID));
	float clock = wind_fps > 0.0 ? floor(TIME * wind_fps) / wind_fps : TIME;
	vec3 card_scale = vec3(length(MODEL_MATRIX[0].xyz), length(MODEL_MATRIX[1].xyz), length(MODEL_MATRIX[2].xyz));
	// world_vertex_coords: VERTEX arrives in world space; recover the unit-card local position.
	vec3 local = (inverse(MODEL_MATRIX) * vec4(VERTEX, 1.0)).xyz;
	vec3 right = vec3(INV_VIEW_MATRIX[0].x, 0.0, INV_VIEW_MATRIX[0].z);
	right = length(right) < 1e-4 ? vec3(1.0, 0.0, 0.0) : normalize(right);
	vec3 back = normalize(cross(right, vec3(0.0, 1.0, 0.0)));
	// PlaneMesh cards lie in XZ: treat their Z as the card's height axis.
	float ly = abs(local.y) > abs(local.z) ? local.y : -local.z;
	float bend = clamp(ly + 0.5, 0.0, 1.0);
	vec3 p = origin + right * (local.x * card_scale.x) + vec3(0.0, (ly + 0.5) * card_scale.y, 0.0);
	vec2 dir = normalize(wind_direction + vec2(0.0001));
	float phase = clock * wind_speed + seed * 6.2831853;
	float gust = sin(phase) + wind_turbulence * sin(phase * 2.7 + 1.7);
	p += vec3(dir.x, 0.0, dir.y) * gust * wind_strength * bend * bend * card_scale.y;
	VERTEX = p;
	NORMAL = vec3(0.0, 1.0, 0.0);
	f_world = origin;
	f_seed = seed;
	f_local = vec2(local.x + 0.5, bend);

	int cell = int(floor(seed * float(max(variation_count, 1))));
	if (animate_frames) {
		int frames = max(1, animation_frames);
		cell = (int(floor(seed * float(frames))) + int(floor(TIME * max(animation_fps, 0.001)))) % frames;
	}
	int columns = max(1, int(atlas_cells.x));
	f_uv = (UV + vec2(float(cell % columns), float(cell / columns))) / max(atlas_cells, vec2(1.0));
}

// Three tapered blades in the unit card (x 0..1, y 0 base .. 1 tip).
float pt_tuft(vec2 q, float seed) {
	float a = 0.0;
	for (int i = 0; i < 3; i++) {
		float fi = float(i);
		float cx = 0.22 + 0.28 * fi + (fract(seed * (7.0 + fi * 3.0)) - 0.5) * 0.12;
		float h = 0.65 + 0.35 * fract(seed * (13.0 + fi * 5.0));
		float lean = (fi - 1.0) * 0.18;
		float x = cx + lean * q.y;
		float w = 0.11 * (1.0 - q.y / h);
		a = max(a, step(abs(q.x - x), w) * step(q.y, h));
	}
	return a;
}

void fragment() {
	vec4 c = tint;
	float a;
	if (use_albedo_texture) {
		vec4 t = texture(albedo_texture, f_uv);
		c.rgb *= t.rgb;
		a = t.a * tint.a;
	} else {
		a = pt_tuft(f_local, f_seed);
	}
	c.rgb = mix(base_tint.rgb * (use_albedo_texture ? c.rgb / max(tint.rgb, vec3(0.001)) : vec3(1.0)), c.rgb, smoothstep(0.0, 0.6, f_local.y));
	ALPHA = a;
	ALPHA_SCISSOR_THRESHOLD = max(alpha_scissor_threshold, 0.01);
	ALBEDO = c.rgb;
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, pt_cloud_shadow(f_world))
}
