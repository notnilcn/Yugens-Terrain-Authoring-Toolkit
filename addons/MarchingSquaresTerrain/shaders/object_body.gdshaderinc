// Marching Squares Terrain - body of the generic object shader (StandardMaterial3D conversion target).
// Included by object.gdshader (cull back) and object_double.gdshader (cull disabled).

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Fog of war on non-terrain surfaces (pt_fog_surface).
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

uniform vec4 albedo_color : source_color = vec4(1.0);
uniform sampler2D albedo_texture : source_color, filter_nearest_mipmap, repeat_enable;
uniform bool use_albedo_texture = false;
uniform bool use_vertex_color = false;
uniform vec3 uv1_scale = vec3(1.0);
uniform vec3 uv1_offset = vec3(0.0);
uniform bool use_alpha_scissor = false;
uniform float alpha_scissor_threshold : hint_range(0.0, 1.0) = 0.5;
uniform vec4 emission_color : source_color = vec4(0.0, 0.0, 0.0, 1.0);
uniform float emission_energy = 1.0;
uniform sampler2D emission_texture : source_color, filter_nearest;
uniform bool use_emission_texture = false;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.3;
/// Rim highlight on silhouettes (0 = off).
uniform float rim_strength : hint_range(0.0, 1.0) = 0.0;

varying vec3 world_pos;
varying float cloud;

void vertex() {
	UV = UV * uv1_scale.xy + uv1_offset.xy;
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	PT_SNAP_VERTEX
}

void fragment() {
	vec4 c = albedo_color;
	if (use_albedo_texture) {
		c *= texture(albedo_texture, UV);
	}
	if (use_vertex_color) {
		c *= COLOR;
	}
	if (use_alpha_scissor) {
		ALPHA = c.a;
		ALPHA_SCISSOR_THRESHOLD = alpha_scissor_threshold;
	}
	ALBEDO = c.rgb;
	vec3 e = emission_color.rgb * emission_energy;
	if (use_emission_texture) {
		e *= texture(emission_texture, UV).rgb;
	}
	if (rim_strength > 0.0) {
		float rim = 1.0 - clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0);
		e += c.rgb * step(0.65, rim) * rim_strength;
	}
	EMISSION = e;
	cloud = pt_cloud_shadow(world_pos);
	{ vec4 pt_fs = pt_fog_surface(ALBEDO, world_pos, mst_pixel_lattice(world_pos, INV_VIEW_MATRIX, FRAGCOORD.xy)); ALBEDO = pt_fs.rgb; EMISSION *= pt_fs.a; } // [fog of war]
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, cloud)
}
