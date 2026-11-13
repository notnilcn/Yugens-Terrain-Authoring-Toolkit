// Marching Squares Terrain - PixelWeather rain splashes: pixel ripple rings on the ground and the
// water (one MultiMesh of flat quads, INSTANCE_CUSTOM = random seed). Every instance replays a
// splash `rate` times a second at a new world-anchored spot inside a box around the camera
// focus (the box wraps like the precipitation, so moving the camera never drags splashes).
// A splash sits on the terrain height (pt_terrain_height) or the water level (pt_water_a),
// whichever is higher, snapped to a world lattice of `texel` metres, and draws:
//   a bright 1-texel impact, then a 1-texel ring that grows
//   to `radius` texels and dissolves (dithered) - flat on the ground, so the camera's pitch
//   squashes it into the ellipse of the reference rain shader.
// Colour pass only (off the metadata layer).
shader_type spatial;
render_mode unshaded, cull_disabled, shadows_disabled, depth_draw_opaque, fog_disabled;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"

global uniform sampler2D pt_terrain_height;
global uniform vec4 pt_terrain_rect;
global uniform vec4 pt_water_a;

/// Half-size of the box (x, z used).
uniform vec3 area = vec3(26.0, 12.0, 26.0);
/// Splashes per second per instance.
uniform float rate = 2.2;
/// World size of a splash texel (m).
uniform float texel = 0.08;
/// Largest ring radius (texels).
uniform int radius = 3;
uniform vec4 color : source_color = vec4(0.72, 0.8, 0.9, 1.0);
/// 0..1: fraction of instances that splash (rain intensity).
uniform float amount = 1.0;

varying float life;
varying vec2 centre;
varying float cycle;
varying float gone;

float ground_at(vec2 xz) {
	float g = -1e6;
	if (pt_terrain_rect.z > 0.0 && pt_terrain_rect.w > 0.0) {
		vec2 uv = (xz - pt_terrain_rect.xy) / pt_terrain_rect.zw;
		if (uv.x >= 0.0 && uv.y >= 0.0 && uv.x <= 1.0 && uv.y <= 1.0) {
			ivec2 size = textureSize(pt_terrain_height, 0);
			g = texelFetch(pt_terrain_height, clamp(ivec2(uv * vec2(size - ivec2(1)) + 0.5), ivec2(0), size - ivec2(1)), 0).r;
		}
	}
	if (pt_water_a.x > 0.5) {
		g = max(g, pt_water_a.y);
	}
	return g;
}

void vertex() {
	vec3 c = MODEL_MATRIX[3].xyz;
	vec4 seed = INSTANCE_CUSTOM;
	float t = pt_time * rate + seed.w * 7.0;
	cycle = floor(t);
	life = fract(t);
	vec2 size = area.xz * 2.0;
	vec2 rnd = vec2(pt_hash(seed.xy * 91.7 + cycle * 1.37), pt_hash(seed.yz * 57.3 + cycle * 3.11));
	// World-anchored spot, wrapped into the box around the focus.
	vec2 lo = c.xz - area.xz;
	vec2 xz = lo + mod(rnd * size - lo, size);
	xz = (floor(xz / texel) + 0.5) * texel;
	centre = xz;
	float g = ground_at(xz);
	gone = (g < -1e5 || seed.x > amount) ? 1.0 : 0.0;
	float r = float(radius) + 1.5;
	vec3 p = vec3(xz.x + VERTEX.x * r * texel * 2.0, g + 0.03, xz.y + VERTEX.y * r * texel * 2.0);
	// The node is a translation only (instances are identity).
	VERTEX = p - c;
	if (gone > 0.5) {
		VERTEX = vec3(0.0);
	}
}

void fragment() {
	if (gone > 0.5) {
		discard;
	}
	vec3 wp = (INV_VIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
	vec2 q = floor((wp.xz - centre) / texel + 0.5);
	float d = length(q);
	bool on = false;
	float bright = 0.0;
	if (life < 0.12) {
		on = d < 0.5;
		bright = 0.45;
	} else if (life < 0.75) {
		float r = mix(1.0, float(radius), (life - 0.12) / 0.63);
		on = abs(d - r) < 0.55;
		// The ring dissolves pixel by pixel as it spreads.
		on = on && pt_hash(q + cycle * 3.7) > (life - 0.3) / 0.5;
		bright = 0.2;
	}
	if (!on) {
		discard;
	}
	ALBEDO = mix(color.rgb, vec3(1.0), bright) * mix(1.0, 0.4, pt_night);
}
