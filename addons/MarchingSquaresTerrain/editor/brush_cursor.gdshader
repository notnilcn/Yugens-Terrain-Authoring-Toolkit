// Editor brush cursor (MarchingSquaresTerrain plugin). Drawn on a mesh draped over the terrain;
// UV is the offset from the brush centre in radii. Shapes match pt_core::brush::BrushShape::distance.
// After MarchingSquaresTerrain's *_brush_radius_visual shaders (MIT): translucent fill with a falloff
// gradient (0.6 at the centre) or a flat 0.25 fill without falloff.
shader_type spatial;
render_mode unshaded, depth_test_disabled, cull_disabled, shadows_disabled, blend_mix;

uniform int shape = 0; // Round, Square, Hexagon, Hexagon30, Triangle, Triangle180
uniform bool falloff_visible = true;
uniform float falloff_ease = -1.6;
uniform vec4 tint : source_color = vec4(1.0);
uniform float edge_px = 1.25;

float brush_dist(vec2 d) {
	const float H = 0.8660254;
	if (shape == 1) return max(abs(d.x), abs(d.y));
	if (shape == 2 || shape == 3) {
		float a0 = shape == 2 ? radians(30.0) : 0.0;
		float m = 0.0;
		for (int i = 0; i < 3; i++) {
			float a = a0 + radians(60.0) * float(i);
			m = max(m, abs(dot(d, vec2(cos(a), sin(a)))));
		}
		return m / H;
	}
	if (shape == 4 || shape == 5) {
		float a0 = shape == 4 ? radians(90.0) : radians(270.0);
		float m = -1e9;
		for (int i = 0; i < 3; i++) {
			float a = a0 + radians(120.0) * float(i);
			m = max(m, dot(d, vec2(cos(a), sin(a))));
		}
		return m / 0.5;
	}
	return length(d);
}

// Godot's ease().
float ease_fn(float x, float c) {
	x = clamp(x, 0.0, 1.0);
	if (c > 0.0) {
		return c < 1.0 ? 1.0 - pow(1.0 - x, 1.0 / c) : pow(x, c);
	} else if (c < 0.0) {
		return x < 0.5 ? pow(x * 2.0, -c) * 0.5 : (1.0 - pow(1.0 - (x - 0.5) * 2.0, -c)) * 0.5 + 0.5;
	}
	return 0.0;
}

void fragment() {
	float h = brush_dist(UV);
	float w = fwidth(h) * edge_px;
	float inside = 1.0 - smoothstep(1.0 - w, 1.0, h);
	float a = falloff_visible ? (1.0 - ease_fn(h, falloff_ease)) * 0.6 : 0.25;
	// Two-tone rim (white inside, dark outside) so the brush stays readable on snow and grass.
	float rim_light = (1.0 - smoothstep(0.0, w, abs(h - 1.0 + 2.0 * w))) * 0.9;
	float rim_dark = (1.0 - smoothstep(0.0, w, abs(h - 1.0 + 0.5 * w))) * 0.75;
	vec3 col = mix(tint.rgb, vec3(0.08, 0.1, 0.16), rim_dark / max(max(a * inside, max(rim_light, rim_dark)), 1e-4));
	ALBEDO = col;
	ALPHA = max(max(a * inside, rim_light), rim_dark) * tint.a;
}
