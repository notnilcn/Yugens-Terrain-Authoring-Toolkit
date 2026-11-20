// Marching Squares Terrain - PixelSunShafts: shadow-driven light shafts.
//
// Port of robert-k's godot-3d-pixel-art `godrays/godray.gdshader` (MIT, see CREDITS.md): a few
// large sheets that contain the sun direction are drawn in the ray colour; in light() the
// directional light's shadow attenuation becomes the sheet's alpha, so every part of a sheet
// the sun reaches glows faintly and every part in a shadow stays clear. Stacked along the
// view, the sheets read as volumetric shafts between pillars, trunks and canopies.
//
// Compatibility renderer: shadowed lights are drawn in their own additive pass, after a base
// pass that never runs light() for them, so robert-k's trick (EMISSION plus an ALPHA written in
// light()) leaves the sheets invisible there. The sheets are blended additively instead and the
// shaft term is written through DIFFUSE_LIGHT (ALBEDO = ray color, no ambient, no emission),
// which adds ray_color * alpha where lit (tested in Compatibility).
//
// Added here on top of the port: the alpha is quantised into `shaft_steps` with an ordered
// dither on the step edges (pixel-art steps instead of a smooth ramp), and it fades with the
// shared `pt_sun_light` / `pt_night` globals so the shafts go out at night, and with the height
// above the node so the sheets do not haze the open sky.
shader_type spatial;
render_mode skip_vertex_transform, blend_add, depth_draw_never, cull_disabled, specular_disabled, ambient_light_disabled, fog_disabled;

uniform vec3 ray_color : source_color = vec3(1.0, 0.915, 0.27);
/// Distance from the camera over which the sheets fade in (no shaft right in front of the lens).
uniform float start_depth = 14.0;
/// Opacity of one lit sheet.
uniform float alpha = 0.01;
/// Brightness steps of the shaft opacity (0 = smooth, as robert-k's shader).
uniform int shaft_steps : hint_range(0, 8) = 3;
/// Width of the dithered transition between brightness steps (0 = hard steps).
uniform float dither_width : hint_range(0.0, 1.0) = 0.5;
/// Fraction of the sheet's half size over which it fades out at its rim (hides the sheet edges).
uniform float edge_fade : hint_range(0.0, 1.0) = 0.35;
/// Height of the direction to the sun (set by PixelSunShafts): shafts are strongest with a high sun.
uniform float sun_height = 1.0;
/// World height the stack stands on (set by PixelSunShafts) and the height above it where the
/// sheets have faded out (0 = no fade): keeps the open sky above the scene free of haze.
uniform float base_height = 0.0;
uniform float fade_height = 12.0;
/// Fade the sheets out this close (metres) in front of the surface behind them, so a sheet
/// cutting into the ground leaves no hard line (0 = off).
uniform float surface_fade = 1.5;
/// Distance from the camera where the sun's shadow map ends (set by PixelSunShafts from the
/// light's `directional_shadow_max_distance`; 0 = off). Past it ATTENUATION is 1 everywhere.
/// The sheets fade out where they, or the surface behind them, lie beyond about half of it:
/// in front of the open sky (or far terrain) every unshadowed sheet glows, so the sky turns
/// into a flat warm haze and the only shape left is the dark shadow of a tree or wall cutting
/// through it (a dark wedge instead of shafts). Shafts then only show against the scene.
uniform float shadow_reach = 0.0;

uniform sampler2D depth_tex : hint_depth_texture, repeat_disable, filter_nearest;

global uniform float pt_sun_light;
global uniform float pt_night;

varying vec3 view_pos;
varying float world_y;

float ss_bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

void vertex() {
	world_y = (MODEL_MATRIX * vec4(VERTEX, 1.0)).y;
	VERTEX = (MODELVIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
	view_pos = VERTEX;
}

void fragment() {
	ALBEDO = ray_color;
}

void light() {
	if (LIGHT_IS_DIRECTIONAL) {
		// View-space distance (from a vertex varying: fragment-written varyings read as 0 in
		// Compatibility's light pass).
		float depth_fade = clamp(length(view_pos) / max(start_depth, 0.001), 0.0, 1.0);
		// Rim fade from the UV (a per-vertex value would be 0 on every corner of the quad).
		vec2 r = abs(UV * 2.0 - 1.0);
		float edge_mask = 1.0 - smoothstep(1.0 - edge_fade, 1.0, max(r.x, r.y));
		float a = alpha * ATTENUATION * depth_fade * edge_mask * clamp(sun_height, 0.0, 1.0);
		if (fade_height > 0.0) {
			a *= 1.0 - smoothstep(fade_height * 0.4, fade_height, world_y - base_height);
		}
		if (surface_fade > 0.0 || shadow_reach > 0.0) {
			vec2 suv = FRAGCOORD.xy / VIEWPORT_SIZE;
			float depth = textureLod(depth_tex, suv, 0.0).r;
#if CURRENT_RENDERER == RENDERER_COMPATIBILITY
			vec3 ndc = vec3(suv, depth) * 2.0 - 1.0;
#else
			vec3 ndc = vec3(suv * 2.0 - 1.0, depth);
#endif
			vec4 sv = INV_PROJECTION_MATRIX * vec4(ndc, 1.0);
			float scene_z = -sv.z / sv.w;
			if (surface_fade > 0.0) {
				a *= clamp((scene_z - (-view_pos.z)) / surface_fade, 0.0, 1.0);
			}
			if (shadow_reach > 0.0) {
				// Godot fades the shadow out over the last 20% of its range: be gone well before.
				float reach = max(length(view_pos), scene_z);
				a *= 1.0 - smoothstep(shadow_reach * 0.35, shadow_reach * 0.6, reach);
			}
		}
		a *= clamp(pt_sun_light, 0.0, 1.0) * (1.0 - clamp(pt_night, 0.0, 1.0));
		if (shaft_steps > 0) {
			// Quantise relative to the full-strength sheet so the step count is stable.
			float v = clamp(a / max(alpha, 1e-5), 0.0, 1.0) * float(shaft_steps);
			float cell = floor(v);
			float f = v - cell;
			float t = dither_width > 0.001 ? clamp((f - (1.0 - dither_width)) / dither_width, 0.0, 1.0) : step(1.0, f);
			a = alpha * (cell + step(ss_bayer4(FRAGCOORD.xy), t)) / float(shaft_steps);
		}
		DIFFUSE_LIGHT += vec3(a);
	}
}
