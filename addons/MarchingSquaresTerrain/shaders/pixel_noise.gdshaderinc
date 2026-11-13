// Marching Squares Terrain - global uniforms, value noise and the shared cloud field.
// Safe to include from spatial *and* canvas_item shaders (no stage built-ins used).
// The global uniforms are registered at runtime by the extension (globals.rs).

global uniform uint pt_meta_mask;
global uniform float pt_meta_depth_range;
global uniform float pt_texel_world;
global uniform vec3 pt_sun_dir;
global uniform vec4 pt_cloud_a;      // coverage, softness, scale, shadow strength
global uniform vec4 pt_cloud_b;      // offset.x, offset.y, cloud height, bands (0 = smooth)
global uniform vec4 pt_cloud_c;      // enabled, 0, 0, 0
global uniform float pt_night;
global uniform float pt_sun_light;  // sun light 0..1 vs daylight (PixelDayNight; 1 without one)
global uniform vec4 pt_lod_fade;    // terrain LOD: focus.xz, fade end distance, band (0 = off)
global uniform float pt_sun_low;    // 0 sun high .. 1 sun at / below the horizon (PixelDayNight; 0 without one)
// Shared effect clock (clock.rs), used instead of TIME: seconds at runtime, frozen in the editor
// unless effects animate there. A shader that reads TIME makes the editor redraw every frame.
global uniform float pt_time;

// 4x4 ordered dither threshold (0..1) of a pixel coordinate.
float pt_bayer4(vec2 p) {
	ivec2 q = ivec2(mod(floor(p), 4.0));
	int m[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
	return (float(m[q.x + q.y * 4]) + 0.5) / 16.0;
}

// Distant-LOD fade (MarchingSquaresTerrain `lod_distance`): 1 = fully shown, 0 = gone. Props
// and grass dither out over the band before their chunk switches to the low-detail ring, so
// they do not pop at the ring edge.
float pt_lod_keep(vec3 world_pos) {
	if (pt_lod_fade.w <= 0.0) {
		return 1.0;
	}
	return clamp((pt_lod_fade.z - length(world_pos.xz - pt_lod_fade.xy)) / pt_lod_fade.w, 0.0, 1.0);
}

float pt_hash(vec2 p) {
	p = fract(p * vec2(123.34, 456.21));
	p += dot(p, p + 45.32);
	return fract(p.x * p.y);
}

float pt_vnoise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	float a = pt_hash(i);
	float b = pt_hash(i + vec2(1.0, 0.0));
	float c = pt_hash(i + vec2(0.0, 1.0));
	float d = pt_hash(i + vec2(1.0, 1.0));
	return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float pt_fbm(vec2 p) {
	float s = 0.0;
	float a = 0.5;
	for (int i = 0; i < 4; i++) {
		s += pt_vnoise(p) * a;
		p = p * 2.03 + vec2(17.1, 9.2);
		a *= 0.5;
	}
	return s / 0.9375;
}

// Cloud density (0 = clear sky, 1 = full cloud) at a world XZ position on the cloud plane.
float pt_cloud_density(vec2 world_xz) {
	vec2 p = world_xz * pt_cloud_a.z + pt_cloud_b.xy;
	float n = pt_fbm(p);
	float d = smoothstep(1.0 - pt_cloud_a.x - pt_cloud_a.y, 1.0 - pt_cloud_a.x + pt_cloud_a.y, n);
	if (pt_cloud_b.w > 0.5) {
		d = floor(d * pt_cloud_b.w + 0.5) / pt_cloud_b.w;
	}
	return d;
}

// Sun-projected cloud shadow multiplier (1 = lit) for a world position.
float pt_cloud_shadow(vec3 world_pos) {
	if (pt_cloud_c.x < 0.5) {
		return 1.0;
	}
	if (world_pos.y >= pt_cloud_b.z) {
		return 1.0;
	}
	vec3 l = normalize(pt_sun_dir);
	float t = (pt_cloud_b.z - world_pos.y) / max(l.y, 0.08);
	vec3 p = world_pos + l * t;
	// A low sun is diffuse: cloud shadows are softer through golden hour and dusk.
	return 1.0 - pt_cloud_density(p.xz) * pt_cloud_a.w * (1.0 - 0.6 * pt_sun_low);
}

