// Marching Squares Terrain - terrain surface. Vertex contract (see pt_core::mesh):
//   UV      ridge/ledge band factors
//   CUSTOM0 (mat_a, mat_b, mat_c, ridge_ledge_wall_slot)   texture slots, flat per cell
//   CUSTOM1 (weight_a, weight_b, is_wall, grass)
shader_type spatial;
render_mode specular_disabled, cull_back;

#define PT_FLAGS_MATERIAL
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Grid / cursor / fog of war / auto rules / debug views (mst_* uniforms, driven by the node).
#include "res://addons/MarchingSquaresTerrain/shaders/mst_terrain_overlay.gdshaderinc"
// [PixelWeather/PixelWater hook] wetness, snow, caustics (pixel_weather.gdshaderinc)
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_weather.gdshaderinc"
// [PixelFireSystem hook] burnt ground, embers, wet patches (pixel_fire.gdshaderinc)
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_fire.gdshaderinc"

group_uniforms Textures;
uniform sampler2DArray textures : source_color, filter_nearest_mipmap, repeat_enable;
uniform vec4 tints[16];
uniform float scales[16];
/// World units covered by one repeat of a texture at scale 1.
uniform float texture_world_size = 8.0;
group_uniforms;

group_uniforms Blending;
/// 0 = smooth, 1/2 = hard edges.
uniform int blend_mode = 0;
uniform float blend_sharpness : hint_range(0.0, 10.0) = 3.0;
uniform float blend_noise_scale = 0.5;
uniform float blend_noise_strength : hint_range(0.0, 1.0) = 0.0;
/// Smoothed blends become an ordered dither on the ground texture's own texel grid (each
/// texel shows one slot), so blend edges stay crisp pixel art instead of a soft cross-fade.
uniform bool blend_dither = true;
group_uniforms;

group_uniforms Ridge_Ledge;
/// Wall texture continues onto the floor along cliff tops (ridge) and cliff feet (ledge).
uniform bool use_ridge_texture = true;
uniform bool use_ledge_texture = true;
uniform float ridge_threshold : hint_range(0.0, 1.0) = 0.6;
uniform float ledge_threshold : hint_range(0.0, 1.0) = 0.6;
uniform float rl_noise_scale = 1.3;
uniform float rl_noise_strength : hint_range(0.0, 2.0) = 0.5;
group_uniforms;

group_uniforms Auto_Texturing;
/// Floors whose normal.y is below this take the cliff texture (Minor Deity-style rocky
/// slopes). 0 disables; ~0.8 rocks anything steeper than ~37 degrees.
uniform float slope_rock : hint_range(0.0, 1.0) = 0.0;
uniform bool shore_enabled = false;
uniform float shore_level = 0.0;
uniform float shore_band = 0.6;
uniform int shore_slot = 3;
group_uniforms;

group_uniforms Shading;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.3;
uniform float wall_shade : hint_range(0.0, 1.0) = 0.88;
/// Rock strata on walls/cliff faces: darkening of every other layer (0 = off).
uniform float wall_strata : hint_range(0.0, 1.0) = 0.14;
/// Height of one rock layer in world units.
uniform float wall_strata_height = 0.7;
/// Waviness of the strata boundaries (0 = straight, stacked-slab layers, e.g. for HexPrism).
uniform float wall_strata_noise : hint_range(0.0, 2.0) = 0.7;
group_uniforms;

varying vec3 local_pos;
varying vec3 local_normal;
varying flat vec4 mats;
varying vec4 weights;
varying vec3 world_pos;
varying float cloud;

float blend_bayer4(vec2 p) {
	ivec2 q = ivec2(mod(p, 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

vec4 slot_color(int slot, vec2 uv) {
	if (slot >= 15) {
		return vec4(0.0);
	}
	return texture(textures, vec3(uv * scales[slot], float(slot))) * tints[slot];
}

vec4 triplanar(int slot, vec3 p, vec3 n) {
	vec2 w = abs(n.xz) + 0.001;
	w /= (w.x + w.y);
	vec4 cx = slot_color(slot, p.zy / texture_world_size);
	vec4 cz = slot_color(slot, p.xy / texture_world_size);
	return cx * w.x + cz * w.y;
}

void vertex() {
	local_pos = VERTEX;
	local_normal = NORMAL;
	mats = CUSTOM0;
	weights = CUSTOM1;
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	bool wall = weights.z > 0.5;
	// Flat face normal (derivatives, outside any branch): walls give the metadata pass their
	// faces, so creases sit on real corners even though wall lighting normals are smoothed.
	vec3 face_n = normalize(cross(dFdx(VERTEX), dFdy(VERTEX)));
	face_n = face_n.z < 0.0 ? -face_n : face_n;
	ivec3 m = ivec3(round(mats.xyz));
	vec3 w = vec3(weights.x, weights.y, max(1.0 - weights.x - weights.y, 0.0));
	w = max(w, vec3(0.0));
	if (blend_noise_strength > 0.0) {
		float n = pt_vnoise(world_pos.xz * blend_noise_scale) - 0.5;
		w = clamp(w + vec3(n, -n * 0.5, -n * 0.5) * blend_noise_strength, 0.0, 1.0);
	}
	if (blend_mode == 0) {
		w = pow(w, vec3(1.0 + blend_sharpness));
		if (blend_dither) {
			w /= max(w.x + w.y + w.z, 1e-4);
			// World size of one texel of the dominant slot's texture.
			float tex_w = float(textureSize(textures, 0).x);
			float texel = clamp(texture_world_size / (tex_w * max(scales[clamp(m.x, 0, 15)], 1e-3)), 0.02, 0.5);
			vec2 cell = wall ? floor(vec2(local_pos.x + local_pos.z, local_pos.y) / texel) : floor(local_pos.xz / texel);
			float d = blend_bayer4(cell);
			w = w.x > d ? vec3(1.0, 0.0, 0.0) : (w.x + w.y > d ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, 1.0));
		}
	} else {
		w = (w.x >= w.y && w.x >= w.z) ? vec3(1.0, 0.0, 0.0) : (w.y >= w.z ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, 1.0));
	}
	w /= max(w.x + w.y + w.z, 1e-4);

	float void_w = (m.x == 15 ? w.x : 0.0) + (m.y == 15 ? w.y : 0.0) + (m.z == 15 ? w.z : 0.0);
	if (void_w > 0.5) {
		discard;
	}

	vec3 n = normalize(local_normal);
	if (!wall) {
		m = mst_auto_slots(m, local_pos, n.y, UV.y);
	}
	vec4 col;
	if (wall) {
		col = triplanar(m.x, local_pos, n) * w.x + triplanar(m.y, local_pos, n) * w.y + triplanar(m.z, local_pos, n) * w.z;
		col.rgb *= wall_shade;
		// Horizontal rock layers with noisy boundaries: reads as stone, hides the triplanar seams.
		// Low-detail walls of the distant LOD ring (CUSTOM1.z = 0.6) stay plain.
		float strata = local_pos.y / max(wall_strata_height, 0.05) + (pt_vnoise(world_pos.xz * 1.7 + world_pos.y * 0.3) - 0.5) * wall_strata_noise;
		col.rgb *= 1.0 - wall_strata * step(0.5, fract(strata)) * step(0.7, weights.z);
	} else {
		vec2 uv = local_pos.xz / texture_world_size;
		if (mst_auto_enabled != 0 && n.y < 0.8) {
			// Auto-rock on steep floors: project from the side so texels do not stretch.
			vec3 tn = normalize(vec3(n.x, 0.0, n.z) + vec3(0.001));
			col = triplanar(m.x, local_pos, tn) * w.x + triplanar(m.y, local_pos, tn) * w.y + triplanar(m.z, local_pos, tn) * w.z;
		} else {
			col = slot_color(m.x, uv) * w.x + slot_color(m.y, uv) * w.y + slot_color(m.z, uv) * w.z;
		}
		int rl = int(round(mats.w));
		float jitter = (pt_vnoise(world_pos.xz * rl_noise_scale) - 0.5) * rl_noise_strength;
		bool ledge = use_ledge_texture && UV.x > 0.0 && UV.x >= ledge_threshold + jitter;
		bool ridge = use_ridge_texture && UV.y > 0.0 && UV.y >= ridge_threshold + jitter;
		bool steep = n.y < slope_rock + jitter * 0.08;
		if (ledge || ridge || steep) {
			col = triplanar(rl, local_pos, normalize(vec3(n.x, 0.0, n.z) + vec3(0.001)));
			col = mix(col, slot_color(rl, uv), step(0.98, n.y));
			col.rgb *= wall_shade;
		}
		if (shore_enabled && world_pos.y < shore_level + shore_band + jitter * 0.3) {
			col = slot_color(shore_slot, uv);
		}
	}
	vec3 rgb = col.rgb;
	rgb = pt_water_caustics(pt_weather_surface(rgb, world_pos, n, wall, TIME), world_pos, TIME); // [PixelWeather/PixelWater hook]
	rgb = pt_fire_surface(rgb, world_pos, wall, TIME); EMISSION = wall ? vec3(0.0) : pt_fire_glow(world_pos, TIME); // [PixelFireSystem hook]
	int dom = (w.x >= w.y && w.x >= w.z) ? m.x : (w.y >= w.z ? m.y : m.z);
	rgb = mst_apply_cells(rgb, world_pos, local_pos.xz, mst_pixel_lattice(world_pos, INV_VIEW_MATRIX, FRAGCOORD.xy), TIME, wall);
	rgb = mst_apply_grid(rgb, local_pos.xz, -VERTEX.z, wall);
	vec3 cursor_m = mst_cursor_mask(local_pos.xz);
	rgb = mst_apply_cursor_mask(rgb, cursor_m);
	EMISSION += mst_cursor_glow(cursor_m);
	rgb = mst_debug_color(rgb, local_pos, n, dom, wall);
	ALBEDO = rgb;
	cloud = pt_cloud_shadow(world_pos);
	vec3 meta_n = wall ? face_n : NORMAL;
	PT_META_OUTPUT(meta_n)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, cloud)
}
