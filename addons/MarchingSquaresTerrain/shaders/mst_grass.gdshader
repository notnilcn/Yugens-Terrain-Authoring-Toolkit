// Marching Squares Terrain - grass tufts (MultiMesh of quads), after MST's mst_grass.gdshader.
// INSTANCE_CUSTOM = (slot, rand, 0, 0). Each slot has its own leaf sprite (a layer of
// `grass_sprites`, white leaves on alpha) and grass colour; the colour is matched to the ground
// texture under the blade root (MST "texture albedo matching"), with a darker root, lighter tips
// and per-blade variation. Wind sway adds gust fronts; PixelFireSystem state (pixel_fire.gdshaderinc)
// burns, shrinks, wets and regrows the blades. Pixels outside the sprite are discarded (opaque pipeline, depth write),
// so tufts keep their leafy silhouette in the pixel renderer. Lighting is flat per blade.
shader_type spatial;
render_mode specular_disabled, cull_disabled, depth_draw_opaque;

#define PT_FLAGS_MATERIAL
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
// Fog of war (cell data): no blades on unexplored cells, dimmed on explored-but-hidden ones.
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"
/// Driven by the terrain node.
uniform int mst_fog = 0;
uniform vec4 mst_fog_hidden = vec4(0.55, 0.25, 0.25, 0.0);
/// Terrain debug view active: blades are hidden.
uniform int mst_debug_view = 0;
// [PixelWeather/PixelWater hook] wetness, snow, caustics (pixel_weather.gdshaderinc)
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_weather.gdshaderinc"
// [PixelFireSystem hook] burning / burnt / wet / regrowing blades (pixel_fire.gdshaderinc)
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_fire.gdshaderinc"

group_uniforms Sprites;
/// One layer per texture slot (bound by the terrain node). Unbound = transparent (no squares).
uniform sampler2DArray grass_sprites : source_color, filter_nearest, hint_default_transparent;
uniform float alpha_cutoff : hint_range(0.01, 0.99) = 0.5;
group_uniforms;

group_uniforms Colour;
uniform sampler2DArray ground_textures : source_color, filter_nearest_mipmap, repeat_enable;
uniform vec4 ground_tints[16];
uniform float ground_scales[16];
uniform float texture_world_size = 8.0;
/// Per-slot grass colour (MarchingSquaresTextureSlot.grass_tint).
uniform vec4 grass_tints[16];
/// 1 = grass colour x ground colour under the root (MST default), 0 = flat grass colour.
uniform float ground_match : hint_range(0.0, 1.0) = 1.0;
uniform float tip_lighten : hint_range(0.0, 1.0) = 0.3;
uniform float root_darken : hint_range(0.0, 1.0) = 0.3;
/// Random per-blade brightness spread, so neighbouring tufts separate.
uniform float blade_variation : hint_range(0.0, 0.5) = 0.18;
group_uniforms;

group_uniforms Shape;
/// 0 = face the camera fully (best for top-down), 1 = rotate around Y only (MST).
uniform int billboard_mode = 0;
group_uniforms;

group_uniforms Wind;
uniform bool animate_active = true;
/// Scrolling noise texture (MST wind_noise_texture); procedural noise when unbound.
uniform sampler2D wind_texture : hint_default_black, repeat_enable;
uniform bool use_wind_texture = false;
uniform float wind_strength = 0.2;
uniform float wind_speed = 0.14;
uniform float wind_scale = 0.02;
uniform vec2 wind_direction = vec2(1.0, 1.0);
/// > 0 quantizes the wind animation to this many frames per second (stepped animation).
uniform float animation_fps = 0.0;
/// Gusts: bands of stronger wind rolling across the field along the wind direction.
uniform float gust_strength : hint_range(0.0, 2.0) = 0.5;
/// Distance between gust fronts (m).
uniform float gust_spacing = 9.0;
/// Speed of the gust fronts (m/s).
uniform float gust_speed = 3.0;
/// Blades over a fire flutter in its updraft.
uniform float fire_flutter : hint_range(0.0, 1.0) = 0.35;
group_uniforms;

group_uniforms Shading;
uniform int bands : hint_range(1, 8) = 3;
uniform float shade_floor : hint_range(0.0, 1.0) = 0.35;
group_uniforms;

varying vec3 base_color;
varying vec3 root_world;
varying float cloud;
varying flat float slot_layer;
varying flat vec2 fog;
varying flat vec4 fire;
varying flat float blade_seed;
varying flat float snow_cover; // [PixelWeather snow]

void vertex() {
	root_world = MODEL_MATRIX[3].xyz;
	int slot = clamp(int(round(INSTANCE_CUSTOM.x)), 0, 15);
	slot_layer = float(slot);
	fog = (mst_fog >= 2 && mst_cell_data_ready()) ? mst_fog_at(mst_cell_local(root_world)) : vec2(1.0);
	float r = INSTANCE_CUSTOM.y;
	blade_seed = r;
	fire = pt_fire_at(root_world);
	snow_cover = pt_snow_cover(root_world, 1.0); // [PixelWeather snow]
	vec2 uv = root_world.xz / texture_world_size * ground_scales[slot];
	vec3 ground = (textureLod(ground_textures, vec3(uv, float(slot)), 0.0) * ground_tints[slot]).rgb;
	vec3 tint = grass_tints[slot].rgb;
	base_color = mix(tint, ground * tint, ground_match) * (1.0 + (r - 0.5) * 2.0 * blade_variation);

	float top = 1.0 - UV.y;
	vec3 sway = vec3(0.0);
	if (animate_active) {
		float t = pt_time;
		if (animation_fps > 0.0) {
			t = floor(t * animation_fps) / animation_fps;
		}
		vec2 wd = normalize(wind_direction + vec2(1e-4));
		float gust;
		if (use_wind_texture) {
			vec2 wuv = root_world.xz * wind_scale + wd * t * wind_speed;
			gust = (textureLod(wind_texture, wuv, 0.0).r - 0.35) * 1.6;
		} else {
			gust = (pt_vnoise(root_world.xz * wind_scale * 6.0 + wd * t * wind_speed * 4.0) - 0.35) * 1.6;
		}
		gust += sin(t * 2.0 + r * 6.283) * 0.12;
		// Gust fronts (Wind Animations reference): a crest sweeps across, bending the blades
		// further, then they spring back.
		float front = fract(dot(root_world.xz, wd) / max(gust_spacing, 0.5) - t * gust_speed / max(gust_spacing, 0.5));
		float crest = smoothstep(0.0, 0.25, front) * (1.0 - smoothstep(0.25, 0.6, front));
		gust += crest * gust_strength * (0.7 + 0.6 * pt_vnoise(root_world.xz * 0.15 + 3.0));
		sway = vec3(wd.x, 0.0, wd.y) * gust * wind_strength * top;
		// Updraft flutter over burning ground.
		float fl = fire.x * fire_flutter;
		sway += vec3(sin(t * 23.0 + r * 40.0), 0.0, cos(t * 19.0 + r * 31.0)) * fl * 0.12 * top;
	}

	mat4 bb;
	if (billboard_mode == 1) {
		vec3 right = normalize(cross(vec3(0.0, 1.0, 0.0), INV_VIEW_MATRIX[2].xyz));
		vec3 fwd = normalize(cross(INV_VIEW_MATRIX[0].xyz, vec3(0.0, 1.0, 0.0)));
		bb = mat4(vec4(right, 0.0), vec4(0.0, 1.0, 0.0, 0.0), vec4(fwd, 0.0), vec4(root_world + sway, 1.0));
	} else {
		bb = mat4(INV_VIEW_MATRIX[0], INV_VIEW_MATRIX[1], INV_VIEW_MATRIX[2], vec4(root_world + sway, 1.0));
	}
	float scale = length(MODEL_MATRIX[0].xyz) * (0.85 + r * 0.3) * pt_fire_grass_scale(fire) * pt_snow_grass_scale(snow_cover);
	// Mirror half of the tufts so repeated sprites read less uniform.
	float flip = r > 0.5 ? -1.0 : 1.0;
	MODELVIEW_MATRIX = VIEW_MATRIX * bb * mat4(vec4(scale * flip, 0.0, 0.0, 0.0), vec4(0.0, scale, 0.0, 0.0), vec4(0.0, 0.0, scale, 0.0), vec4(0.0, 0.0, 0.0, 1.0));
	MODELVIEW_NORMAL_MATRIX = mat3(VIEW_MATRIX);
	NORMAL = vec3(0.0, 1.0, 0.0);
}

void fragment() {
	float lod_keep = pt_lod_keep(root_world);
	if (lod_keep < 1.0 && lod_keep <= pt_bayer4(FRAGCOORD.xy)) {
		discard; // distant LOD fade
	}
	vec4 sprite = texture(grass_sprites, vec3(UV, slot_layer));
	if (sprite.a < alpha_cutoff) {
		discard;
	}
	if (fog.y < 0.5 || mst_debug_view != 0) {
		discard;
	}
	float top = 1.0 - UV.y;
	ALBEDO = base_color * sprite.rgb * mix(1.0 - root_darken, 1.0 + tip_lighten, top);
	ALBEDO = pt_weather_grass_snow(ALBEDO, top, snow_cover); // [PixelWeather hook]
	ALBEDO = pt_fire_grass(ALBEDO, fire, top, blade_seed, pt_time); // [PixelFireSystem hook]
	EMISSION = pt_fire_grass_glow(ALBEDO, fire);
	if (fog.x < 0.5) {
		float l = dot(ALBEDO, vec3(0.299, 0.587, 0.114));
		ALBEDO = mix(vec3(l), ALBEDO, mst_fog_hidden.y) * mst_fog_hidden.x;
	}
	NORMAL = (VIEW_MATRIX * vec4(0.0, 1.0, 0.0, 0.0)).xyz;
	cloud = pt_cloud_shadow(root_world);
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(bands, shade_floor, cloud)
}
