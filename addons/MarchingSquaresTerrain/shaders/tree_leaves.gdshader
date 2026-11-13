// Marching Squares Terrain - camera-facing leaf-cluster card (tree foliage).
// Port of Pixelizer3D's tree_leaves.gdshader onto pixel_common (metadata, toon light, cloud
// shadows). Unlike grass the card follows the camera in pitch too (classic 3D pixel-art foliage).
// Use a unit QuadMesh (or MultiMesh of them) under a PixelObject; the node origin is the card
// centre. The whole card sways in world space (`wind_strength` = fraction of the card height)
// on a stepped clock. Lighting is pinned to the card centre so a swaying card shades as one
// unit. Without a texture a procedural leafy blob is drawn. Alpha scissor only.
shader_type spatial;
render_mode world_vertex_coords, cull_disabled, specular_disabled, depth_draw_opaque;
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Fog of war on non-terrain surfaces (pt_fog_surface).
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

group_uniforms Albedo;
uniform sampler2D albedo_texture : source_color, filter_nearest, repeat_disable;
uniform bool use_albedo_texture = false;
uniform vec4 tint : source_color = vec4(0.36, 0.6, 0.28, 1.0);
/// Leaf highlight colour of the procedural blob (top-lit dither).
uniform vec4 light_tint : source_color = vec4(0.55, 0.78, 0.36, 1.0);
uniform float alpha_scissor_threshold : hint_range(0.0, 1.0) = 0.5;
/// Size of the procedural leaf clumps (cells across the card).
uniform float leaf_cells = 7.0;

group_uniforms Atlas;
uniform vec2 atlas_cells = vec2(1.0, 1.0);
uniform int variation_count = 1;
uniform bool animate_frames = false;
uniform float animation_fps = 4.0;
uniform int animation_frames = 4;

group_uniforms Wind;
uniform vec2 wind_direction = vec2(1.0, 0.2);
uniform float wind_strength = 0.06;
uniform float wind_speed = 0.9;
uniform float wind_turbulence = 0.5;
uniform float wind_fps = 6.0;
/// Vertical bob as a fraction of the lateral sway.
uniform float wind_bob = 0.25;

group_uniforms Shading;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.4;
group_uniforms;

varying vec2 f_uv;
varying vec2 f_local;
varying vec3 f_world;
varying vec3 f_center_view;
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
	vec3 local = (inverse(MODEL_MATRIX) * vec4(VERTEX, 1.0)).xyz;
	float ly = abs(local.y) > abs(local.z) ? local.y : -local.z;
	vec3 right = INV_VIEW_MATRIX[0].xyz;
	vec3 up = INV_VIEW_MATRIX[1].xyz;
	vec3 p = origin + right * (local.x * card_scale.x) + up * (ly * card_scale.y);
	vec2 dir = normalize(wind_direction + vec2(0.0001));
	float phase = clock * wind_speed + seed * 6.2831853;
	float gust = sin(phase) + wind_turbulence * sin(phase * 2.7 + 1.7);
	float sway = gust * wind_strength * card_scale.y;
	p += vec3(dir.x, 0.0, dir.y) * sway;
	p.y += sway * wind_bob * sin(phase * 1.3 + 0.7);
	VERTEX = p;
	// Facing the camera, tilted up: leaves read as lit from above.
	NORMAL = normalize(INV_VIEW_MATRIX[2].xyz + vec3(0.0, 0.8, 0.0));
	f_world = origin;
	f_seed = seed;
	f_local = vec2(local.x + 0.5, ly + 0.5);
	f_center_view = (VIEW_MATRIX * vec4(origin, 1.0)).xyz;

	int cell = int(floor(seed * float(max(variation_count, 1))));
	if (animate_frames) {
		int frames = max(1, animation_frames);
		cell = (int(floor(seed * float(frames))) + int(floor(TIME * max(animation_fps, 0.001)))) % frames;
	}
	int columns = max(1, int(atlas_cells.x));
	f_uv = (UV + vec2(float(cell % columns), float(cell / columns))) / max(atlas_cells, vec2(1.0));
}

float pt_leaf_hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

void fragment() {
	LIGHT_VERTEX = f_center_view;
	vec3 c = tint.rgb;
	float a;
	if (use_albedo_texture) {
		vec4 t = texture(albedo_texture, f_uv);
		c *= t.rgb;
		a = t.a * tint.a;
	} else {
		// Round blob with a ragged, clumpy edge and a lit upper half.
		vec2 q = f_local * 2.0 - 1.0;
		vec2 cellp = floor(f_local * leaf_cells);
		float jag = pt_leaf_hash(cellp + f_seed * 17.0);
		float r = length(q);
		a = step(r, 0.78 + 0.22 * jag);
		float lit = step(0.15 - 0.35 * jag, q.y);
		c = mix(tint.rgb, light_tint.rgb, lit * step(0.4, jag));
	}
	ALPHA = a;
	ALPHA_SCISSOR_THRESHOLD = max(alpha_scissor_threshold, 0.01);
	ALBEDO = c;
	{ vec4 pt_fs = pt_fog_surface(ALBEDO, world_pos, mst_pixel_lattice(world_pos, INV_VIEW_MATRIX, FRAGCOORD.xy)); ALBEDO = pt_fs.rgb; EMISSION *= pt_fs.a; } // [fog of war]
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, pt_cloud_shadow(f_world))
}
