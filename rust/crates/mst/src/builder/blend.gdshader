// Marching Squares Terrain - Pixel Builder blend material: two layers blended by the painted
// vertex alpha (0 = layer A, 1 = layer B), optionally height-aware.
// Port of Unibuilder's materials/ub_blend.gdshader (MIT, Leafshade Consulting Ltd), adapted to
// the pixel pipeline: toon light, metadata output, nearest filtering; the PBR channels
// (normal / roughness / metallic / AO maps, parallax) are dropped since the pipeline shades flat.
shader_type spatial;
render_mode specular_disabled, cull_back;
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"

group_uniforms Blend;
/// 0 blends by the painted alpha only; 1 lets the layers' height maps decide where they meet.
uniform float height_blend : hint_range(0.0, 1.0) = 0.0;
/// Width of the height-blend transition.
uniform float blend_sharpness : hint_range(0.001, 1.0) = 0.1;
/// Swaps the layers without repainting.
uniform bool invert_mask = false;
/// Multiplies the result by the vertex color (RGB).
uniform bool tint_by_vertex_color = true;
/// Pixel-art mask: quantize the blend factor to this many steps (0 = smooth).
uniform int mask_steps : hint_range(0, 8) = 0;

group_uniforms LayerA;
uniform sampler2D texture_a : source_color, filter_nearest_mipmap, repeat_enable;
uniform vec4 albedo_a : source_color = vec4(1.0);
uniform sampler2D height_map_a : hint_default_white, filter_nearest_mipmap, repeat_enable;
uniform float height_offset_a : hint_range(-1.0, 1.0) = 0.0;
uniform sampler2D emission_map_a : source_color, filter_nearest, repeat_enable;
uniform vec4 emission_a : source_color = vec4(0.0, 0.0, 0.0, 1.0);
uniform float emission_energy_a : hint_range(0.0, 16.0) = 1.0;
uniform vec2 uv_scale_a = vec2(1.0, 1.0);
uniform vec2 uv_offset_a = vec2(0.0, 0.0);

group_uniforms LayerB;
uniform sampler2D texture_b : source_color, filter_nearest_mipmap, repeat_enable;
uniform vec4 albedo_b : source_color = vec4(1.0);
uniform sampler2D height_map_b : hint_default_white, filter_nearest_mipmap, repeat_enable;
uniform float height_offset_b : hint_range(-1.0, 1.0) = 0.0;
uniform sampler2D emission_map_b : source_color, filter_nearest, repeat_enable;
uniform vec4 emission_b : source_color = vec4(0.0, 0.0, 0.0, 1.0);
uniform float emission_energy_b : hint_range(0.0, 16.0) = 1.0;
uniform vec2 uv_scale_b = vec2(1.0, 1.0);
uniform vec2 uv_offset_b = vec2(0.0, 0.0);

group_uniforms Toon;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.3;
group_uniforms;

varying vec3 world_pos;
varying float cloud;

float blended_factor(float t, float ha, float hb) {
	if (height_blend <= 0.0) {
		return t;
	}
	float top = max(ha + (1.0 - t), hb + t) - blend_sharpness;
	float wa = max(ha + (1.0 - t) - top, 0.0);
	float wb = max(hb + t - top, 0.0);
	float height_t = wb / max(wa + wb, 0.00001);
	return mix(t, height_t, height_blend);
}

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	PT_SNAP_VERTEX
}

void fragment() {
	float paint = invert_mask ? 1.0 - COLOR.a : COLOR.a;
	vec2 uv_a = UV * uv_scale_a + uv_offset_a;
	vec2 uv_b = UV * uv_scale_b + uv_offset_b;
	float ha = texture(height_map_a, uv_a).r + height_offset_a;
	float hb = texture(height_map_b, uv_b).r + height_offset_b;
	float t = blended_factor(paint, ha, hb);
	if (mask_steps > 0) {
		t = clamp(floor(t * float(mask_steps) + 0.5) / float(mask_steps), 0.0, 1.0);
	}
	vec4 base_a = texture(texture_a, uv_a) * albedo_a;
	vec4 base_b = texture(texture_b, uv_b) * albedo_b;
	vec3 albedo = mix(base_a.rgb, base_b.rgb, t);
	if (tint_by_vertex_color) {
		albedo *= COLOR.rgb;
	}
	ALBEDO = albedo;
	EMISSION = mix(texture(emission_map_a, uv_a).rgb * emission_a.rgb * emission_energy_a,
			texture(emission_map_b, uv_b).rgb * emission_b.rgb * emission_energy_b, t);
	cloud = pt_cloud_shadow(world_pos);
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, cloud)
}
