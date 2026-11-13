// Marching Squares Terrain - fireflies (MultiMesh quads). INSTANCE_CUSTOM.x = random phase.
shader_type spatial;
render_mode unshaded, blend_add, depth_draw_never, cull_disabled, shadows_disabled;

global uniform float pt_night;

uniform vec4 color : source_color = vec4(1.0, 0.9, 0.45, 1.0);
uniform float size = 0.12;
uniform float wander = 0.6;
uniform float blink_speed = 1.3;
uniform float energy = 2.0;
/// 1 = only visible at night (driven by PixelDayNight), 0 = always.
uniform float night_only : hint_range(0.0, 1.0) = 1.0;

varying float glow;

void vertex() {
	float r = INSTANCE_CUSTOM.x * 6.2831;
	vec3 base = MODEL_MATRIX[3].xyz;
	vec3 off = vec3(sin(TIME * 0.7 + r), sin(TIME * 0.9 + r * 1.7) * 0.5, cos(TIME * 0.6 + r * 2.3)) * wander;
	float blink = smoothstep(0.2, 0.9, sin(TIME * blink_speed + r * 3.0) * 0.5 + 0.5);
	glow = blink * mix(1.0, pt_night, night_only);
	mat4 bb = mat4(INV_VIEW_MATRIX[0], INV_VIEW_MATRIX[1], INV_VIEW_MATRIX[2], vec4(base + off, 1.0));
	MODELVIEW_MATRIX = VIEW_MATRIX * bb * mat4(vec4(size, 0, 0, 0), vec4(0, size, 0, 0), vec4(0, 0, size, 0), vec4(0, 0, 0, 1));
}

void fragment() {
	float d = length(UV - 0.5) * 2.0;
	float core = step(d, 0.45);
	float halo = (1.0 - smoothstep(0.45, 1.0, d)) * 0.35;
	ALBEDO = color.rgb * (core + halo) * glow * energy;
}
