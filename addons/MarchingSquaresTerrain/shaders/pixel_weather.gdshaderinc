// Marching Squares Terrain - weather and water hooks for ground shaders (terrain, grass).
// Include after pixel_common.gdshaderinc. The globals are registered by the extension
// (globals.rs) and driven by PixelWeather (pt_wet, pt_snow) and PixelWater (pt_water_*).
// Everything is banded/stepped so it stays pixel-art; all functions are no-ops when the
// globals are at their defaults (no weather, no water).

global uniform float pt_wet;     // 0 = dry, 1 = soaked (darker ground, puddles, drop sparkles)
global uniform float pt_snow;    // 0 = none, 1 = everything but cliffs is white
global uniform vec4 pt_water_a;  // enabled, water level (world y), 0, 0
global uniform vec4 pt_water_b;  // caustics strength, scale, bands, reach above the water line

const vec3 PT_CAUSTIC_COLOR = vec3(0.6, 0.88, 0.9);

// Web of bright caustic lines: where two drifting noise layers cross the same value.
float pt_caustic(vec2 xz, float t, float scale) {
	vec2 p = xz * scale;
	float a = pt_vnoise(p + vec2(t * 0.35, t * 0.21));
	float b = pt_vnoise(p * 1.37 + vec2(7.3 - t * 0.27, 3.1 + t * 0.31));
	return clamp(1.0 - abs(a - b) * 5.0, 0.0, 1.0);
}

// Banded caustics for ground near the water: on the lake bed and in the swash band just above
// the water line (light reflected off the swell), fading with distance from the surface.
vec3 pt_water_caustics(vec3 albedo, vec3 wp, float t) {
	if (pt_water_a.x < 0.5 || pt_water_b.x <= 0.0) {
		return albedo;
	}
	float h = wp.y - pt_water_a.y;
	float reach = max(pt_water_b.w, 0.01);
	if (h > reach || h < -8.0) {
		return albedo;
	}
	float fade = h > 0.0 ? 1.0 - h / reach : clamp(1.0 + h / 8.0, 0.0, 1.0);
	float bands = max(pt_water_b.z, 1.0);
	float c = floor(pt_caustic(wp.xz, t, pt_water_b.y) * fade * bands) / bands;
	return albedo + PT_CAUSTIC_COLOR * c * pt_water_b.x;
}

// ---- Snow (PixelWeather Snow page) -------------------------------------------------------
// pt_snow is the accumulated cover 0..1: it grows while snow falls and melts back when it stops.
// The cover is drawn in three flat tones (deep snow, thin snow, bare) with an ordered-dither
// fringe on a world lattice, so edges stay crisp pixel art and never crawl when the camera moves.
// Flat ground fills first, slopes stay thinner, cliffs and the shore line stay bare.
const vec3 PT_SNOW_COLOR = vec3(0.8, 0.84, 0.92);
const vec3 PT_SNOW_SHADE = vec3(0.64, 0.7, 0.84);
/// World size of one dither cell (about one ground-texture texel).
const float PT_SNOW_TEXEL = 0.125;

float pt_snow_bayer(vec2 c) {
	ivec2 i = ivec2(mod(c, 4.0));
	int idx = i.x + i.y * 4;
	// 4x4 Bayer matrix, row-major.
	int b[16] = int[16](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
	return (float(b[idx]) + 0.5) / 16.0;
}

// Snow cover tier at a world position with world normal y `ny`: 1 = deep snow, 0.6 = thin snow,
// 0 = bare. Grass and props call it with their root / surface position so they agree with the
// ground under them.
float pt_snow_cover(vec3 wp, float ny) {
	if (pt_snow < 0.002 || ny < 0.5) {
		return 0.0;
	}
	float up = smoothstep(0.5, 0.94, ny);
	float c = pt_snow * (0.2 + 0.8 * up) * 1.12;
	if (pt_water_a.x > 0.5) {
		c *= smoothstep(0.05, 0.7, wp.y - pt_water_a.y);
	}
	// Drifts: broad patches, smaller lumps and a fine edge noise.
	float m = pt_vnoise(wp.xz * 0.16) * 0.5 + pt_vnoise(wp.xz * 0.85 + 5.0) * 0.33 + pt_vnoise(wp.xz * 3.1 + 9.0) * 0.17;
	// Spread the (centre-heavy) noise so the covered fraction follows pt_snow roughly linearly.
	float d = c - clamp((m - 0.5) * 1.7 + 0.5, 0.0, 1.0) * 0.96;
	const float band = 0.1;
	if (d >= band) {
		return 1.0;
	}
	if (d >= band * 0.5) {
		return 0.6;
	}
	if (d <= 0.0) {
		return 0.0;
	}
	return d / (band * 0.5) > pt_snow_bayer(floor(wp.xz / PT_SNOW_TEXEL)) ? 0.6 : 0.0;
}

// Ground snow: deep snow in two banded tones (soft drift shading) with rare sparkles, a thin
// cool layer at the fringe.
vec3 pt_snow_surface(vec3 albedo, vec3 wp, vec3 n, float t) {
	float sc = pt_snow_cover(wp, n.y);
	if (sc <= 0.0) {
		return albedo;
	}
	vec2 cell = floor(wp.xz / PT_SNOW_TEXEL);
	if (sc < 0.99) {
		return mix(albedo, PT_SNOW_SHADE, 0.72);
	}
	float drift = pt_vnoise(wp.xz * 0.55 + 21.0);
	vec3 s = drift > 0.68 ? mix(PT_SNOW_COLOR, PT_SNOW_SHADE, 0.45) : PT_SNOW_COLOR;
	// Sparse cool specks give the flat white a texel texture (world-anchored).
	if (pt_hash(cell * 1.7 + 3.1) < 0.1) {
		s = mix(s, PT_SNOW_SHADE, 0.5);
	}
	if (pt_hash(cell + floor(t * 0.7) * vec2(5.3, 1.9)) > 0.996) {
		s = vec3(1.0);
	}
	return s;
}

// Snow on props (rock / bush / tree tops): upward faces take the same cover as the ground.
// `wn` is the world normal.
vec3 pt_snow_cap(vec3 albedo, vec3 wp, vec3 wn) {
	if (pt_snow < 0.002 || wn.y < 0.25) {
		return albedo;
	}
	// Props sit higher than the ground and cones (pines) are steep: let them whiten earlier.
	float sc = pt_snow_cover(wp, min(1.0, wn.y + 0.3));
	if (sc <= 0.0) {
		return albedo;
	}
	// Steep faces (pine cones, rock sides) only catch a dithered dusting so the prop keeps its
	// colour; flat-ish tops take full caps.
	float steep = 1.0 - smoothstep(0.55, 0.75, wn.y);
	vec3 cell = floor(wp / (PT_SNOW_TEXEL * 0.75));
	if (steep > 0.0 && pt_snow_bayer(cell.xz + cell.y * vec2(2.0, 3.0)) < steep * (1.1 - 0.5 * pt_snow)) {
		return albedo;
	}
	return sc >= 0.99 ? PT_SNOW_COLOR : mix(albedo, PT_SNOW_SHADE, 0.7);
}

// Grass blades: `cover` is pt_snow_cover() at the root. Under deep snow the blade is a snow
// lump (pt_snow_grass_scale() also shrinks it), under thin snow the tips carry a cap, elsewhere
// the tips frost with the overall cover.
vec3 pt_weather_grass_snow(vec3 albedo, float top, float cover) {
	albedo *= mix(1.0, 0.75, pt_wet);
	if (pt_snow < 0.002) {
		return albedo;
	}
	if (cover >= 0.99) {
		return top > 0.35 ? PT_SNOW_COLOR : PT_SNOW_SHADE;
	}
	if (cover > 0.0 && top > 0.55) {
		return PT_SNOW_COLOR;
	}
	float frost = step(1.0 - pt_snow * 0.4, top) * step(0.05, pt_snow);
	return mix(albedo, PT_SNOW_SHADE, frost * 0.85);
}

// Blade height factor: blades sink into deep snow (gone once the cover is thick).
float pt_snow_grass_scale(float cover) {
	return 1.0 - step(0.99, cover) * smoothstep(0.25, 0.7, pt_snow);
}

// Ground wetness and snow. `n` is the world normal, `wall` excludes cliff faces from snow.
vec3 pt_weather_surface(vec3 albedo, vec3 wp, vec3 n, bool wall, float t) {
	if (pt_wet > 0.001) {
		albedo *= mix(1.0, 0.66, pt_wet);
		albedo *= mix(vec3(1.0), vec3(0.9, 0.96, 1.06), pt_wet);
		float flat_ground = step(0.85, n.y) * (wall ? 0.0 : 1.0);
		float pud = pt_vnoise(wp.xz * 0.45 + 11.0) * 0.7 + pt_vnoise(wp.xz * 1.9) * 0.3;
		if (flat_ground > 0.5 && pud > 1.0 - 0.3 * pt_wet) {
			albedo = mix(albedo, vec3(0.3, 0.36, 0.44), 0.45);
			// Rain drops hitting the puddle: single bright specks on a world lattice.
			float drop = pt_hash(floor(wp.xz * 7.0) + floor(t * 9.0) * vec2(3.1, 7.7));
			if (drop > 1.0 - 0.025 * pt_wet) {
				albedo = vec3(0.75, 0.85, 0.95);
			}
		}
	}
	if (!wall) {
		albedo = pt_snow_surface(albedo, wp, n, t);
	}
	return albedo;
}

// Grass: darker when wet, frosted tips (top = 0 root .. 1 tip) under snow. Ground shaders that
// know the blade root use pt_weather_grass_snow() with pt_snow_cover() of the root instead.
vec3 pt_weather_grass(vec3 albedo, float top) {
	return pt_weather_grass_snow(albedo, top, 0.0);
}

// Rain intensity 0..1 (PixelWeather; 0 without weather). The flipbook rain sheets and the
// splash rings (rain_splash.gdshader) are drawn by PixelWeather itself; ground shaders can add
// pt_rain_splash() for extra splash rings baked into wet floors.
global uniform float pt_rain;

// Pixel splash rings on flat ground while it rains: a world lattice of 8-texel cells, each
// replaying a 1-texel ring (radius 0..2.5 texels of `texel` m) at its own phase. Returns albedo.
vec3 pt_rain_splash(vec3 albedo, vec3 wp, vec3 n, float t, float texel) {
	if (pt_rain < 0.01 || n.y < 0.85) {
		return albedo;
	}
	float cell = texel * 8.0;
	vec2 id = floor(wp.xz / cell);
	float ph = pt_hash(id * 1.31);
	float life = fract(t * 1.7 + ph);
	float cyc = floor(t * 1.7 + ph);
	if (pt_hash(id + cyc * 0.71) > pt_rain * 0.6) {
		return albedo;
	}
	vec2 c = (id + 0.25 + 0.5 * vec2(pt_hash(id + cyc), pt_hash(id.yx + cyc * 1.9))) * cell;
	vec2 q = floor((wp.xz - c) / texel + 0.5);
	if (abs(length(q) - life * 2.5) < 0.55 && life < 0.8) {
		return mix(albedo, vec3(0.78, 0.86, 0.95), 0.6);
	}
	return albedo;
}
