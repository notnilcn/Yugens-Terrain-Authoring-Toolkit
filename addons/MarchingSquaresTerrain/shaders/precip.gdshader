// Marching Squares Terrain - PixelWeather rain streaks / snowflakes (one MultiMesh of quads).
// The MultiMeshInstance sits at the centre of a box that follows the camera; every instance has
// a random seed in INSTANCE_CUSTOM. Particles are anchored in *world* space and wrap inside the
// box, so moving the camera never drags the rain along. Each particle is drawn as an exact block
// of low-res texels (rain: 1 x `texels` stair-stepped streak slanted with the wind), centred on
// the camera's texel lattice so it never shimmers. Drops stop at the terrain (pt_terrain_height)
// and the water (pt_water_a) with a 3-texel splash.
// Snow (mode 1) is snapped in screen space instead: the flake centre is projected and rounded to
// a pixel of the (low-res) viewport, then drawn as a 1 x 1, 2 x 2 or 3 x 3 plus-shaped block of
// whole pixels, so flakes stay crisp under ortho and perspective cameras alike. Flakes fall
// slowly, drift with the fall wind and pt_wind (pixel_wind.gdshaderinc) and sway; PixelWeather
// sizes the box with the camera view so the on-screen density does not grow when zooming out.
//
// Metadata: opts out explicitly. The node is kept off the metadata layer (colour pass only), so
// precipitation gets no outline and never hides terrain edges.
shader_type spatial;
render_mode unshaded, cull_disabled, shadows_disabled, depth_draw_opaque;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_wind.gdshaderinc"

global uniform sampler2D pt_terrain_height;
global uniform vec4 pt_terrain_rect;
global uniform vec4 pt_water_a;

/// 0 = rain, 1 = snow.
uniform int mode = 0;
/// Half-size of the box.
uniform vec3 area = vec3(26.0, 12.0, 26.0);
uniform vec4 color : source_color = vec4(0.64, 0.72, 0.84, 1.0);
uniform float fall_speed = 16.0;
/// Horizontal drift (m/s).
uniform vec2 wind = vec2(0.0);
/// Rain: streak length; snow: flake size (low-res texels).
uniform int texels = 4;
/// World size of a texel when no PixelRenderer publishes one.
uniform float fallback_texel = 0.08;
/// Snow: fraction of flakes drawn `texels` wide (the rest are single pixels).
uniform float big_flakes = 0.3;
/// Snow: sideways sway of each flake (m).
uniform float sway = 0.5;
/// Snow: how much the global wind (pt_wind) pushes the flakes (m/s per wind strength unit).
uniform float wind_follow = 0.35;
/// Snow: camera distance to the box centre, for the depth tint.
uniform float focus_depth = 20.0;

varying float splash;
varying float depth_tint;
varying flat float flake;

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

// Snaps a view-space coordinate so a block `n` texels wide lands exactly on texels.
float snap_axis(float v, float tex, int n) {
	return (n % 2 == 1) ? (floor(v / tex) + 0.5) * tex : round(v / tex) * tex;
}

// Snowflake: returns the clip position of this quad corner (outside the clip volume = hidden).
vec4 snow_vertex(vec3 vtx, vec4 custom, vec3 c, mat4 view, mat4 proj, vec2 vp, float t, out float tint, out float fl) {
	tint = 0.0;
	fl = 1.0;
	vec3 size = area * 2.0;
	vec3 seed = custom.xyz * size;
	float ph = custom.w * 6.2831;
	float r2 = fract(custom.w * 13.37);
	float r3 = fract(custom.w * 71.31);
	// Per-flake fall speed spread; light flakes drift more with the wind.
	float spd = fall_speed * (0.7 + 0.6 * r3);
	vec2 wv = wind + pt_wind.xy * pt_wind.z * wind_follow * (1.3 - 0.6 * r3);
	vec3 p = seed + vec3(wv.x, -spd, wv.y) * t;
	p.x += sin(t * (0.6 + 0.5 * r2) + ph) * sway;
	p.z += cos(t * (0.5 + 0.4 * r3) + ph * 1.7) * sway * 0.7;
	// World-anchored wrap inside the box.
	p = c - area + mod(p - (c - area), size);
	// Gusts shove the flakes sideways together (pixel_wind's travelling gust field).
	vec2 wd = length(pt_wind.xy) > 1e-3 ? normalize(pt_wind.xy) : vec2(1.0, 0.0);
	p.xz += wd * pt_wind_gust(p.xz, t) * sway * 1.5;
	vec4 clip = proj * (view * vec4(p, 1.0));
	if (p.y < ground_at(p.xz) || clip.w <= 0.0) {
		return vec4(4.0, 4.0, 4.0, 1.0);
	}
	int s = r2 < big_flakes ? max(texels, 1) : 1;
	fl = float(s);
	// Without a PixelRenderer the viewport is full-res: scale the blocks up.
	float w = float(s) * (pt_texel_world > 0.0 ? 1.0 : 3.0);
	vec2 sp = (clip.xy / clip.w * 0.5 + 0.5) * vp;
	vec2 centre = (int(w) % 2 == 1) ? floor(sp) + 0.5 : floor(sp + 0.5);
	vec2 corner = centre + vtx.xy * w;
	clip.xy = (corner / vp * 2.0 - 1.0) * clip.w;
	tint = clamp((clip.w - focus_depth) / max(area.x, 1.0), -1.0, 1.0);
	return clip;
}

// Rain drop (Particles rain style): view-space snapped streak, splash for a moment on impact.
vec4 rain_vertex(vec3 vtx, vec4 custom, vec3 c, mat4 view, mat4 proj, float t, out float spl) {
	vec3 size = area * 2.0;
	vec3 seed = custom.xyz * size;
	vec3 drift = vec3(wind.x, -fall_speed, wind.y) * t;
	// World-anchored wrap inside the box.
	vec3 p = c - area + mod(seed + drift - (c - area), size);
	float ground = ground_at(p.xz);
	float hide = 0.0;
	spl = 0.0;
	if (p.y < ground) {
		// Under the ground: a splash for a moment right after hitting, then hidden.
		float since = (ground - p.y) / max(fall_speed, 0.01);
		if (since < 0.07) {
			spl = 1.0;
			p.y = ground + 0.02;
		} else {
			hide = 1.0;
		}
	}

	bool ortho = proj[3][3] != 0.0;
	vec3 v = (view * vec4(p, 1.0)).xyz;
	float tex = pt_texel_world > 0.0 ? pt_texel_world : fallback_texel * (ortho ? 1.0 : max(-v.z, 0.1) / 20.0);
	int w = 1;
	int h = texels;
	float shear = 0.0;
	if (spl > 0.5) {
		w = 3;
		h = 1;
	} else {
		vec3 vel = mat3(view) * vec3(wind.x, -fall_speed, wind.y);
		shear = clamp(vel.x / min(vel.y, -0.01), -1.5, 1.5);
	}
	v.x = snap_axis(v.x, tex, w);
	v.y = snap_axis(v.y, tex, h);
	vec2 q = vtx.xy * vec2(float(w), float(h)) * tex;
	q.x += shear * q.y;
	v.xy += q * (1.0 - hide);
	v.z += hide * 1e4; // behind the camera: culled
	return proj * vec4(v, 1.0);
}

void vertex() {
	vec3 c = MODEL_MATRIX[3].xyz;
	splash = 0.0;
	depth_tint = 0.0;
	flake = 1.0;
	if (mode == 1) {
		POSITION = snow_vertex(VERTEX, INSTANCE_CUSTOM, c, VIEW_MATRIX, PROJECTION_MATRIX, VIEWPORT_SIZE, TIME, depth_tint, flake);
	} else {
		POSITION = rain_vertex(VERTEX, INSTANCE_CUSTOM, c, VIEW_MATRIX, PROJECTION_MATRIX, TIME, splash);
	}
}

void fragment() {
	vec3 col = color.rgb * mix(1.0, 0.35, pt_night);
	if (mode == 1) {
		// 3-wide flakes are plus-shaped (corners cut).
		if (flake > 2.5) {
			vec2 q = abs(UV - 0.5);
			if (q.x > 0.17 && q.y > 0.17) {
				discard;
			}
		}
		// Flakes behind the focus are a touch dimmer and bluer: depth without blur.
		col *= mix(vec3(1.0), vec3(0.8, 0.85, 0.95), max(depth_tint, 0.0));
	}
	if (splash > 0.5) {
		col = mix(col, vec3(1.0), 0.3);
	}
	ALBEDO = col;
}
