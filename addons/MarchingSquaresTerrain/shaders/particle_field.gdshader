// Marching Squares Terrain - prerendered particle flipbook layers (PixelParticleField, PixelWeather
// rain). "Particle effects without particles": a small sheet of 1-texel particles that tiles in
// space and loops in time (pt_core::flipbook) is drawn on a few stacked, *world-anchored* sheets
// instead of simulating particles. One mesh holds `layers` unit quads (VERTEX.z = layer index);
// the vertex shader places them:
//   mode 0 Ground:  horizontal sheets at world heights base_y + k * spacing that fall at
//                   fall_speed and wrap inside [base_y, base_y + layers * spacing]; sheet
//                   coordinates are world XZ / texel_size. Fully world-fixed: camera
//                   translation, orbit and zoom (ortho or perspective) never move a particle.
//   mode 1 Upright: vertical sheets facing the camera's yaw at world offsets k * spacing along
//                   the horizontal view direction (like the Stable3D god-ray sheets); sheet
//                   coordinates are (world position . right, -world y) / texel_size, so a
//                   streak in the sheet is a vertical streak in the world (rain). Translation,
//                   pitch and zoom are world-stable; yaw re-slices the sheets.
// A perspective camera sees each sheet with its own parallax and size (a 3D layer stack), an
// ortho camera sees them scroll together; a flat screen-space scroll is not used.
//
// Each sheet gets its own texel offset and frame offset (hash of its world index). Sheet texels
// smaller than a screen texel are found by sampling the texel footprint (max of up to 3 x 3
// taps), so particles never drop out when zoomed out. Particles are hard pixels: intensity x
// masks is compared with a per-texel threshold (dithered fade, no partial alpha). Masks:
//   - sunbeams: the Stable3D god-ray cloud mask at the particle (pt_rays_* from PixelRenderer;
//     the ground cloud shadow when god rays are off) and, for lit particles, the sun's shadow
//     map in light(), so dust shows in the light shafts and vanishes in shade;
//   - `amount` (weather intensity, set by the node), time of day (pt_night), a height band
//     above the terrain (pt_terrain_height) for ground-hugging motes (never over water), and an
//     optional disc around the camera's ground focus (`focus_radius`);
//   - beam = 1 means "only in god-ray shafts": with the god rays off nothing shows;
//   - zoomed out (sheet texels smaller than screen texels) the threshold is scaled by the
//     footprint, so the number of motes per screen texel stays the same instead of saturating.
// Additive and colour-pass only (off the metadata layer): no outlines, no depth writes.
shader_type spatial;
render_mode blend_add, depth_draw_never, cull_disabled, shadows_disabled, specular_disabled, ambient_light_disabled, fog_disabled;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_wind.gdshaderinc"

global uniform sampler2D pt_terrain_height;
global uniform vec4 pt_terrain_rect;
global uniform vec4 pt_water_a;
// God-ray cloud mask published by PixelRenderer: (enabled, cloud threshold, sheet softness,
// gap erosion) and (cloud shadow strength, sun follow, 0, 0).
global uniform vec4 pt_rays_a;
global uniform vec4 pt_rays_b;

uniform sampler2D sheet : filter_nearest, repeat_disable;
uniform ivec2 sheet_grid = ivec2(8, 8);
uniform int tile = 128;
uniform int frames = 64;
uniform float fps = 12.0;

/// 0 = Ground (horizontal, falling), 1 = Upright (vertical, camera yaw).
uniform int mode = 0;
uniform int layers = 6;
uniform float spacing = 1.0;
/// World size of one sheet texel (m).
uniform float texel_size = 0.1;
/// Half-size of each sheet around the focus (m).
uniform float extent = 40.0;
/// Ground: bottom of the falling stack. Upright: bottom of the sheets.
uniform float base_y = 0.0;
/// Upright: sheet height (m).
uniform float height = 20.0;
/// Where the camera looks (sheets are centred here; snapped by the node).
uniform vec3 focus = vec3(0.0);
/// Ground: sheet fall speed (m/s, negative rises). Upright: extra downward scroll (m/s).
uniform float fall_speed = 0.0;
/// Horizontal drift (m/s, world XZ).
uniform vec2 wind = vec2(0.0);
/// Upright: 1 / fall speed (s/m); slants the streaks along the wind on the sheet.
uniform float slant = 0.0;
/// Ground: how far the travelling gusts of the global wind (pt_wind) push particles (m).
uniform float gust = 0.0;
/// Ground: particles only live up to this far above the terrain (0 = anywhere).
uniform float band = 0.0;

uniform vec4 color : source_color = vec4(1.0, 0.95, 0.8, 1.0);
/// Second colour, picked per particle by its random tone.
uniform vec4 color_b : source_color = vec4(1.0, 0.85, 0.6, 1.0);
uniform float brightness = 1.0;
/// Weather / density multiplier 0..1 (drops whole particles).
uniform float amount = 1.0;
/// 0 = self-lit glow, 1 = lit by the sun only (dust).
uniform float lit = 1.0;
/// 0..1: how much the god-ray cloud mask gates the particles (1 = only in the shafts; below 1,
/// `1 - beam` of them also show in plain sunlight outside the cloud shadows).
uniform float beam = 0.8;
/// Extra brightness inside a god-ray shaft.
uniform float shaft_boost = 1.0;
/// Brightness left in shade (lit particles).
uniform float shade = 0.08;
/// 1 = fade out at night, -1 = only at night, 0 = always.
uniform float night = 1.0;
/// Particles only within this horizontal distance of `fade_center` (0 = everywhere).
uniform float focus_radius = 0.0;
uniform vec3 fade_center = vec3(0.0);

varying flat float sheet_id;
varying float range_fade;
varying vec3 sheet_right;

float terrain_at(vec2 xz) {
	float g = -1e6;
	if (pt_terrain_rect.z > 0.0 && pt_terrain_rect.w > 0.0) {
		vec2 uv = (xz - pt_terrain_rect.xy) / pt_terrain_rect.zw;
		if (uv.x >= 0.0 && uv.y >= 0.0 && uv.x <= 1.0 && uv.y <= 1.0) {
			ivec2 size = textureSize(pt_terrain_height, 0);
			g = texelFetch(pt_terrain_height, clamp(ivec2(uv * vec2(size - ivec2(1)) + 0.5), ivec2(0), size - ivec2(1)), 0).r;
		}
	}
	return g;
}

float ground_at(vec2 xz) {
	float g = terrain_at(xz);
	if (pt_water_a.x > 0.5) {
		g = max(g, pt_water_a.y);
	}
	return g;
}

// Sunlight let through the cloud gaps at p: the per-sheet term of the Stable3D god rays
// (pixel_resolve.gdshader rays()), so particles light up exactly in the shafts.
float sun_through_clouds(vec3 p) {
	if (pt_cloud_c.x < 0.5) {
		return 1.0;
	}
	if (pt_rays_a.x < 0.5) {
		return pt_cloud_shadow(p);
	}
	vec3 sun = normalize(pt_sun_dir);
	float inv_sun_y = 1.0 / max(sun.y, 0.08);
	float follow = clamp(pt_rays_b.y, 0.0, 1.0);
	vec2 q = p.xz + sun.xz * ((pt_cloud_b.z - p.y) * inv_sun_y * follow);
	float cloud = pt_fbm(q * pt_cloud_a.z + pt_cloud_b.xy);
	float mask = smoothstep(pt_rays_a.y - pt_rays_a.z, pt_rays_a.y + pt_rays_a.z, cloud);
	float vis = clamp(1.0 - mask * pt_rays_b.x, 0.0, 1.0);
	return clamp((vis - pt_rays_a.w) / max(1e-4, 1.0 - pt_rays_a.w), 0.0, 1.0);
}

vec4 tap(vec2 t, int f) {
	ivec2 i = ivec2(floor(t));
	i = ((i % tile) + tile) % tile;
	ivec2 cell = ivec2(f % sheet_grid.x, f / sheet_grid.x);
	return texelFetch(sheet, cell * tile + i, 0);
}

void vertex() {
	float i = VERTEX.z;
	float n_layers = float(max(layers, 1));
	vec3 p;
	range_fade = 1.0;
	sheet_right = vec3(1.0, 0.0, 0.0);
	if (mode == 0) {
		float range = n_layers * spacing;
		float y_rel = i * spacing - fall_speed * pt_time;
		float w = floor(y_rel / range);
		float y = y_rel - w * range;
		sheet_id = i - w * n_layers;
		// Sheets fade in at the top of the stack and out at the bottom (where they wrap).
		range_fade = clamp(min(y, range - y) / max(spacing, 1e-3), 0.0, 1.0);
		p = vec3(focus.x + VERTEX.x * extent, base_y + y, focus.z + VERTEX.y * extent);
		NORMAL = vec3(0.0, 1.0, 0.0);
	} else {
		vec3 fwd = -INV_VIEW_MATRIX[2].xyz;
		vec2 h = fwd.xz;
		if (length(h) < 0.05) {
			h = INV_VIEW_MATRIX[1].xz;
		}
		vec3 n = normalize(vec3(h.x, 0.0, h.y));
		vec3 r = vec3(-n.z, 0.0, n.x);
		float k = floor(dot(focus, n) / spacing) - floor(n_layers * 0.5) + i;
		sheet_id = k;
		float off = k * spacing - dot(focus, n);
		// The outermost sheets fade, so shifting the stack with the focus never pops one.
		range_fade = clamp((n_layers * 0.5 * spacing - abs(off + 0.5 * spacing)) / spacing, 0.0, 1.0);
		vec3 c = focus + n * off;
		p = vec3(c.x, 0.0, c.z) + r * (VERTEX.x * extent);
		p.y = base_y + (VERTEX.y * 0.5 + 0.5) * height;
		sheet_right = r;
		NORMAL = -n;
	}
	VERTEX = p;
}

void fragment() {
	vec3 wp = (INV_VIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
	float id = sheet_id;
	vec2 uv;
	float v_stretch = 1.0;
	if (mode == 0) {
		vec2 push = gust > 0.0 ? pt_wind.xy * pt_wind_gust(wp.xz, pt_time) * gust * pt_wind.z : vec2(0.0);
		uv = (wp.xz - wind * pt_time - push) / texel_size;
	} else {
		float u = dot(wp, sheet_right);
		float wr = dot(wind, sheet_right.xz);
		// A pitched camera sees a vertical sheet shortened by cos(pitch): stretch the sheet's
		// texels vertically so a streak keeps its length in screen texels (and slow the
		// flipbook by the same factor so the world fall speed stays the same).
		v_stretch = 1.0 / clamp(length((-INV_VIEW_MATRIX[2].xyz).xz), 0.3, 1.0);
		uv = vec2((u - wr * pt_time + slant * wr * wp.y) / texel_size, (-wp.y - fall_speed * pt_time) / (texel_size * v_stretch));
	}
	uv += vec2(pt_hash(vec2(id, 1.7)), pt_hash(vec2(3.1, id))) * float(tile);
	int f = int(mod(floor(pt_time * fps / v_stretch) + floor(pt_hash(vec2(id * 0.37, 9.2)) * float(frames)), float(max(frames, 1))));

	// Footprint taps: a sheet texel smaller than the screen texel must not be skipped.
	vec2 fw = max(fwidth(uv), vec2(1e-4));
	ivec2 n = ivec2(clamp(ceil(fw), vec2(1.0), vec2(3.0)));
	vec4 best = vec4(0.0);
	vec2 at = uv;
	for (int y = 0; y < 3; y++) {
		if (y >= n.y) {
			break;
		}
		for (int x = 0; x < 3; x++) {
			if (x >= n.x) {
				break;
			}
			vec2 o = (vec2(float(x), float(y)) + 0.5) / vec2(n) - 0.5;
			vec2 t = uv + o * fw;
			vec4 s = tap(t, f);
			if (s.r > best.r) {
				best = s;
				at = t;
			}
		}
	}
	// Magnified sheets (texel larger than a screen texel): a particle stays one screen texel
	// wide, drawn on the screen texel holding its centre; streaks keep their length.
	vec2 c = abs(fract(at) - 0.5);
	if (fw.x < 1.0 && c.x > 0.5 * fw.x + 1e-3) {
		discard;
	}
	if (fw.y < 1.0 && abs(best.b * 255.0 - 1.0) > 0.5 && c.y > 0.5 * fw.y + 1e-3) {
		discard;
	}
	float a = best.r * range_fade;
	// Density: whole particles drop out (by their random tone), never single streak texels.
	if (a <= 0.0 || pt_hash(vec2(best.g * 255.0 + 0.5, id * 0.173)) >= amount) {
		discard;
	}
	if (fw.x * fw.y > 1.0) {
		// Zoomed out: the taps above found the brightest of n x n sheet texels; keep one
		// screen texel's worth of particles.
		a /= fw.x * fw.y;
	}
	if (focus_radius > 0.0) {
		float d = length(wp.xz - fade_center.xz);
		a *= clamp((focus_radius - d) / (focus_radius * 0.45), 0.0, 1.0);
	}
	if (band > 0.0) {
		float t = terrain_at(wp.xz);
		// Ground-hugging particles never hang over the sea or a pool.
		if (pt_water_a.x > 0.5 && pt_water_a.y > t - 0.05) {
			discard;
		}
		float h = wp.y - max(t, pt_water_a.x > 0.5 ? pt_water_a.y : -1e6);
		if (h < -0.05) {
			discard;
		}
		a *= clamp((band - h) / max(band * 0.35, 0.05), 0.0, 1.0);
	}
	// In a shaft: full density and a glow boost; elsewhere `1 - beam` of the particles where the
	// ground is not under a cloud shadow.
	float shaft = 0.0;
	if (beam > 0.0) {
		if (beam > 0.999 && pt_rays_a.x < 0.5) {
			discard;
		}
		shaft = sun_through_clouds(wp);
		a *= max(shaft, (1.0 - beam) * pt_cloud_shadow(wp));
	}
	a *= night >= 0.0 ? mix(1.0, 1.0 - pt_night, night) : mix(1.0, pt_night, -night);
	// Hard pixels: a world-texel threshold instead of partial alpha (dithered fade).
	if (a < pt_hash(floor(at) + vec2(id * 7.13, id * 1.31)) * 0.98 + 0.02) {
		discard;
	}
	vec3 col = mix(color.rgb, color_b.rgb, step(0.5, best.g)) * brightness * (1.0 + shaft * shaft_boost);
	ALBEDO = col * lit;
	EMISSION = col * ((1.0 - lit) * mix(1.0, 0.35, pt_night) + lit * shade);
}

void light() {
	if (LIGHT_IS_DIRECTIONAL) {
		DIFFUSE_LIGHT += LIGHT_COLOR / PI * ATTENUATION;
	}
}
