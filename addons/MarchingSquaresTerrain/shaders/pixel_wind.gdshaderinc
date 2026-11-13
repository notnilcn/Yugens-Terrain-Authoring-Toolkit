// Marching Squares Terrain - shared wind for grass / foliage / props (include after
// pixel_noise.gdshaderinc). `pt_wind` is published by PixelWeather every frame:
//   xy = horizontal wind direction (world x, z, unit), z = strength (m/s: ~1 breeze, storms 3-4),
//   w = gustiness 0..1. Without a PixelWeather it keeps its default breeze (globals.rs).
// After the "Wind Animations" reference: gusts are bands of stronger wind that travel across the
// world along the wind, so neighbouring plants bend together and a gust reads as a wave; the
// sway is quantised into a few poses so the animation stays pixel-art (no sliding sub-texels).
//
// How grass (mst_grass.gdshader) should consume it:
//   #include "res://addons/MarchingSquaresTerrain/shaders/pixel_wind.gdshaderinc"
//   vec3 wp = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;               // vertex world position
//   VERTEX += inverse(mat3(MODEL_MATRIX)) * pt_wind_offset(wp, top, TIME); // top: 0 root .. 1 tip
//   ALBEDO *= 1.0 + pt_wind_gust(wp.xz, TIME) * 0.12;               // optional gust shimmer
// Trees / props: pt_wind_offset(wp, height01 * 0.3, TIME) on canopy vertices.
global uniform vec4 pt_wind;

// Travelling gust field 0..1 at a world XZ position.
float pt_wind_gust(vec2 xz, float t) {
	vec2 dir = length(pt_wind.xy) > 1e-3 ? normalize(pt_wind.xy) : vec2(1.0, 0.0);
	float speed = 2.0 + pt_wind.z * 2.5;
	// The along-wind coordinate scrolls with the wind; across-wind noise breaks the bands up.
	float along = dot(xz, dir) - t * speed;
	float across = dot(xz, vec2(-dir.y, dir.x));
	float g = pt_vnoise(vec2(along * 0.08, across * 0.05)) * 0.7 + pt_vnoise(vec2(along * 0.21, across * 0.13 + 4.0)) * 0.3;
	return smoothstep(1.0 - pt_wind.w, 1.0, g + pt_wind.w * 0.35);
}

// Bend offset (world m) for a vertex at height factor `top` (0 = rooted, 1 = tip): steady lean +
// travelling gust + a small flutter, quantised to 1/24 m poses.
vec3 pt_wind_offset(vec3 wp, float top, float t) {
	vec2 dir = length(pt_wind.xy) > 1e-3 ? normalize(pt_wind.xy) : vec2(1.0, 0.0);
	float gust = pt_wind_gust(wp.xz, t);
	float flutter = sin(t * (2.0 + pt_wind.z) + dot(wp.xz, vec2(1.7, 2.3))) * 0.25;
	float bend = (0.15 + gust * 0.85 + flutter * (0.3 + gust)) * pt_wind.z * 0.08 * top * top;
	bend = floor(bend * 24.0 + 0.5) / 24.0;
	return vec3(dir.x * bend, -abs(bend) * 0.3, dir.y * bend);
}
