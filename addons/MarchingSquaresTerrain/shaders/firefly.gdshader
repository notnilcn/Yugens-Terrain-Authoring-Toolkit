// Marching Squares Terrain - fireflies (MultiMesh quads). INSTANCE_CUSTOM.x = random phase.
shader_type spatial;
render_mode unshaded, blend_add, depth_draw_never, cull_disabled, shadows_disabled;

// Shared effect clock (clock.rs), used instead of TIME so the editor can idle.
global uniform float pt_time;

global uniform float pt_night;

uniform vec4 color : source_color = vec4(1.0, 0.9, 0.45, 1.0);
uniform float size = 0.12;
uniform float wander = 0.6;
uniform float blink_speed = 1.3;
uniform float energy = 2.0;
/// 1 = only visible at night (driven by PixelDayNight), 0 = always.
uniform float night_only : hint_range(0.0, 1.0) = 1.0;
/// Smallest sprite on screen, in pixels (the glowing core is about half of it).
uniform float min_pixels = 2.5;

varying float glow;

void vertex() {
	float r = INSTANCE_CUSTOM.x * 6.2831;
	vec3 base = MODEL_MATRIX[3].xyz;
	vec3 off = vec3(sin(pt_time * 0.7 + r), sin(pt_time * 0.9 + r * 1.7) * 0.5, cos(pt_time * 0.6 + r * 2.3)) * wander;
	float blink = smoothstep(0.2, 0.9, sin(pt_time * blink_speed + r * 3.0) * 0.5 + 0.5);
	glow = blink * mix(1.0, pt_night, night_only);
	// Screen-space sprite (like precip.gdshader's flakes): `size` metres wide, but never less
	// than `min_pixels`, centred on a pixel centre. A 0.12 m sprite is well under one pixel of
	// the pixel renderer's low-res image at a 30 m camera, so the world-space quad fell between
	// pixel centres and the swarm vanished.
	vec4 clip = PROJECTION_MATRIX * (VIEW_MATRIX * vec4(base + off, 1.0));
	if (clip.w <= 0.0) {
		POSITION = vec4(4.0, 4.0, 4.0, 1.0);
	} else {
		vec2 vp = max(VIEWPORT_SIZE, vec2(1.0));
		float w = max(size * PROJECTION_MATRIX[1][1] * 0.5 * vp.y / clip.w, min_pixels);
		vec2 sp = (clip.xy / clip.w * 0.5 + 0.5) * vp;
		vec2 corner = floor(sp) + 0.5 + VERTEX.xy * w;
		clip.xy = (corner / vp * 2.0 - 1.0) * clip.w;
		POSITION = clip;
	}
}

void fragment() {
	float d = length(UV - 0.5) * 2.0;
	float core = step(d, 0.45);
	float halo = (1.0 - smoothstep(0.45, 1.0, d)) * 0.35;
	vec3 c = color.rgb * (core + halo) * glow * energy;
	// Keep the tint's hue at high energy: scale down instead of clipping each channel (energy 3
	// clipped the yellow towards white, which read as pale blue at night through the palette).
	ALBEDO = c / max(1.0, max(c.r, max(c.g, c.b)));
}
