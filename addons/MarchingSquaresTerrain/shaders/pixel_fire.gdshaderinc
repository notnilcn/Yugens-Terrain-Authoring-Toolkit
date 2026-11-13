// Marching Squares Terrain - PixelFireSystem hooks for ground shaders (terrain, grass).
// Include after pixel_common.gdshaderinc. The globals are registered by the extension
// (globals.rs) and driven by PixelFireSystem (effects/fire.rs, pt_core::fire):
//   pt_fire_tex  RGBA8, one texel per fire cell: R = heat (burning), G = char (burnt),
//                B = wetness, A = grass growth (1 = full height / no grass)
//   pt_fire_rect (world x of the texture's min corner, world z, 1 / width in m, 1 / depth in m)
//   pt_fire_a    (enabled, texel size in m, width in texels, height in texels)
// Everything is banded so it stays pixel-art; no-ops while no fire system is active.

global uniform sampler2D pt_fire_tex;
global uniform vec4 pt_fire_rect;
global uniform vec4 pt_fire_a;

const vec3 PT_CHAR_COLOR = vec3(0.1, 0.09, 0.09);
const vec3 PT_ASH_COLOR = vec3(0.36, 0.34, 0.34);
const vec3 PT_EMBER_COLOR = vec3(1.0, 0.42, 0.12);
const vec3 PT_EMBER_HOT = vec3(1.0, 0.85, 0.4);

bool pt_fire_on() {
	return pt_fire_a.x > 0.5;
}

vec4 pt_fire_fetch(ivec2 t) {
	ivec2 size = ivec2(pt_fire_a.zw);
	return texelFetch(pt_fire_tex, clamp(t, ivec2(0), size - 1), 0);
}

// Fire state at a world position, bilinear between texels (no state = (0, 0, 0, 1)).
vec4 pt_fire_at(vec3 wp) {
	if (!pt_fire_on()) {
		return vec4(0.0, 0.0, 0.0, 1.0);
	}
	vec2 uv = (wp.xz - pt_fire_rect.xy) * pt_fire_rect.zw;
	if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
		return vec4(0.0, 0.0, 0.0, 1.0);
	}
	vec2 f = uv * pt_fire_a.zw - 0.5;
	ivec2 i = ivec2(floor(f));
	vec2 t = fract(f);
	vec4 a = mix(pt_fire_fetch(i), pt_fire_fetch(i + ivec2(1, 0)), t.x);
	vec4 b = mix(pt_fire_fetch(i + ivec2(0, 1)), pt_fire_fetch(i + ivec2(1, 1)), t.x);
	return mix(a, b, t.y);
}

// Burnt ground, glowing embers under the flames and darker wet patches. `t` = TIME.
vec3 pt_fire_surface(vec3 albedo, vec3 wp, bool wall, float t) {
	if (!pt_fire_on() || wall) {
		return albedo;
	}
	vec4 f = pt_fire_at(wp);
	// Ragged edges: a noise threshold instead of a blend, in two char steps.
	float n = pt_vnoise(wp.xz * 2.3) * 0.6 + pt_vnoise(wp.xz * 7.1) * 0.4;
	float burnt = f.g;
	if (burnt > 0.12 + n * 0.25) {
		albedo = mix(albedo, PT_CHAR_COLOR, burnt > 0.45 + n * 0.3 ? 0.82 : 0.5);
		// Pale ash flecks on fully burnt ground.
		if (pt_hash(floor(wp.xz * 9.0)) > 1.1 - burnt * 0.13) {
			albedo = PT_ASH_COLOR;
		}
	}
	if (f.b > 0.3 + n * 0.2) {
		albedo *= vec3(0.8, 0.86, 0.9);
	}
	if (f.r > 0.04) {
		// Embers: flickering speckles, denser where it burns hotter.
		float e = pt_hash(floor(wp.xz * 8.0) + floor(t * 7.0) * vec2(1.7, 3.3));
		if (e > 1.0 - f.r * 0.22) {
			albedo = e > 1.0 - f.r * 0.07 ? PT_EMBER_HOT : PT_EMBER_COLOR;
		} else {
			albedo = mix(albedo, PT_CHAR_COLOR, step(0.2, f.r) * 0.6);
		}
	}
	return albedo;
}

// Emission for glowing embers (colour pass; add to EMISSION before PT_META_OUTPUT).
vec3 pt_fire_glow(vec3 wp, float t) {
	if (!pt_fire_on()) {
		return vec3(0.0);
	}
	vec4 f = pt_fire_at(wp);
	if (f.r <= 0.04) {
		return vec3(0.0);
	}
	float e = pt_hash(floor(wp.xz * 8.0) + floor(t * 7.0) * vec2(1.7, 3.3));
	return e > 1.0 - f.r * 0.22 ? PT_EMBER_COLOR * 0.8 : PT_EMBER_COLOR * 0.12 * step(0.2, f.r);
}

// Grass blades. `f` = pt_fire_at(blade root), `top` = 0 root .. 1 tip, `seed` = per-blade random.
// Burning blades blacken from the root with glowing tips, burnt blades are charred stubs that
// green up again as they regrow, wet blades are a deeper, fresher green.
vec3 pt_fire_grass(vec3 albedo, vec4 f, float top, float seed, float t) {
	if (!pt_fire_on()) {
		return albedo;
	}
	if (f.b > 0.25) {
		float l = dot(albedo, vec3(0.299, 0.587, 0.114));
		albedo = mix(albedo, vec3(l * 0.55, l * 1.12, l * 0.62), min(f.b, 1.0) * 0.45);
	}
	float burnt = max(f.g * (1.0 - f.a * 0.6), f.r * 0.8);
	if (burnt > 0.15 + seed * 0.3) {
		albedo = mix(albedo, vec3(0.12, 0.15, 0.1) * (0.6 + 1.1 * top), burnt > 0.5 ? 0.92 : 0.6);
	}
	if (f.r > 0.05) {
		float flick = pt_hash(vec2(seed * 97.0, floor(t * 8.0)));
		if (top > 1.0 - f.r * (0.4 + 0.6 * flick)) {
			albedo = top > 1.0 - f.r * 0.25 ? PT_EMBER_HOT : PT_EMBER_COLOR;
		}
	}
	return albedo;
}

// Glow of burning blade tips (colour pass).
vec3 pt_fire_grass_glow(vec3 albedo, vec4 f) {
	return f.r > 0.05 && albedo.r > 0.9 ? albedo * 0.7 : vec3(0.0);
}

// Height factor of a blade: burning grass shrinks to stubble and grows back with the fuel.
float pt_fire_grass_scale(vec4 f) {
	return mix(0.6, 1.0, clamp(f.a, 0.0, 1.0));
}
