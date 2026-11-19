// Marching Squares Terrain - terrain-only overlays: biome auto-texturing rules, grid lines,
// brush cursor, cell data (fog / tint / highlight) and debug views. Include after
// pixel_common.gdshaderinc. Every `mst_*` uniform is driven by the MarchingSquaresTerrain node
// through the RenderingServer (not saved in the material): edit the node / texture preset.
// CPU references: pt_core::autotex (rules), pt_core::overlay (cell math).

#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

group_uniforms Driven_By_Terrain_Node;
uniform vec4 mst_grid = vec4(0.0, 32.0, 32.0, 2.0);          // grid code, chunk stride x, y, ring radius
uniform vec2 mst_cell_size = vec2(2.0, 2.0);
uniform vec4 mst_grid_line = vec4(0.0, 0.08, 0.0, 0.0);       // enabled, width (world), fade distance (0 = none), chunk lines
uniform vec4 mst_grid_color : source_color = vec4(1.0, 1.0, 1.0, 0.35);
uniform vec4 mst_chunk_color : source_color = vec4(1.0, 0.85, 0.3, 0.6);
uniform vec4 mst_cursor = vec4(0.0, 0.0, 1.0, -1.0);          // centre x, z (local), radius, shape (-1 = off)
uniform vec4 mst_cursor_color : source_color = vec4(0.3, 1.0, 0.4, 1.0);
uniform vec4 mst_cursor_params = vec4(0.12, 0.25, 1.0, 0.0);  // rim width (world), fill alpha, falloff, 0
uniform int mst_fog = 0;                                       // 0 off, 1 = tint / highlight, 2 = + fog of war
uniform vec4 mst_fog_a : source_color = vec4(0.05, 0.06, 0.09, 1.0);
uniform vec4 mst_fog_b : source_color = vec4(0.09, 0.10, 0.14, 1.0);
uniform vec4 mst_fog_hidden = vec4(0.55, 0.25, 0.25, 0.0);    // dim, saturation, hatch, 0
uniform vec4 mst_highlight = vec4(0.18, 4.0, 0.0, 0.0);       // outline width (world), pulse speed
uniform int mst_debug_view = 0;   // 0 off, 1 slope, 2 contours, 3 slot ids, 4 chunk borders, 5 cell ids, 6 cell data
uniform float mst_contour_step = 1.0;
// Auto rules (pt_core::autotex::AutoRules); slots < 0 = rule off.
uniform int mst_auto_enabled = 0;
uniform int mst_auto_mask = 1;
uniform vec4 mst_auto_noise = vec4(0.35, 1.0, 0.25, 0.08);     // noise scale, filler slot, filler amount, filler scale
uniform vec4 mst_auto_rock = vec4(5.0, 38.0, 6.0, 6.0);        // slot, slope deg, noise deg, filler slot
uniform vec4 mst_auto_lip = vec4(4.0, 0.35, 0.0, 0.0);         // slot, ridge threshold
uniform vec4 mst_auto_shore = vec4(3.0, 0.0, 0.8, 0.4);        // slot, water level, height, noise
uniform vec4 mst_auto_shore2 = vec4(8.0, 0.0, 0.0, 0.0);       // filler slot
uniform vec4 mst_auto_snow = vec4(7.0, 14.0, 2.0, 0.06);       // slot, height, noise, noise scale
uniform vec4 mst_auto_snow2 = vec4(42.0, 12.0, 1.5, 0.0);      // max slope, slope fade, blend height
group_uniforms;

int mst_grid_code() {
	return int(mst_grid.x + 0.5);
}

// ---------------------------------------------------------------- auto rules

int mst_rule_slot(float v) {
	int s = int(floor(v + 0.5));
	return (s >= 0 && s < 15) ? s : -1;
}

int mst_filler(int s, vec2 xz, float salt) {
	if (s < 0 || mst_auto_noise.z <= 0.0) {
		return -1;
	}
	return pt_vnoise(xz * mst_auto_noise.w + vec2(salt, salt * 0.7)) < mst_auto_noise.z ? s : -1;
}

// Slot chosen by the rules at terrain-local p, or -1 (pt_core::autotex::AutoRules::eval).
int mst_auto_slot(vec3 p, float ny, float ridge) {
	vec2 xz = p.xz;
	float slope = degrees(acos(clamp(ny, -1.0, 1.0)));
	float j = pt_vnoise(xz * mst_auto_noise.x) - 0.5;
	int outs = mst_filler(mst_rule_slot(mst_auto_noise.y), xz, 31.7);
	int rock_slot = mst_rule_slot(mst_auto_rock.x);
	bool rock = rock_slot >= 0 && slope > mst_auto_rock.y + j * 2.0 * mst_auto_rock.z;
	int shore = mst_rule_slot(mst_auto_shore.x);
	if (shore >= 0 && !rock && p.y < mst_auto_shore.y + mst_auto_shore.z + j * 2.0 * mst_auto_shore.w) {
		int f = mst_filler(mst_rule_slot(mst_auto_shore2.x), xz, 53.1);
		outs = f >= 0 ? f : shore;
	}
	if (rock) {
		int f = mst_filler(mst_rule_slot(mst_auto_rock.w), xz, 71.3);
		outs = f >= 0 ? f : rock_slot;
	}
	int lip = mst_rule_slot(mst_auto_lip.x);
	if (lip >= 0 && ridge > mst_auto_lip.y + j * 0.3) {
		outs = lip;
	}
	int snow = mst_rule_slot(mst_auto_snow.x);
	if (snow >= 0) {
		float line = mst_auto_snow.y + (pt_fbm(xz * mst_auto_snow.w) - 0.5) * 2.0 * mst_auto_snow.z;
		if (p.y > line) {
			float cov = 1.0 - smoothstep(mst_auto_snow2.x - mst_auto_snow2.y, mst_auto_snow2.x, slope);
			cov *= clamp((p.y - line) / max(mst_auto_snow2.z, 0.01), 0.0, 1.0);
			if (cov > pt_vnoise(xz * 1.7 + vec2(5.3, 2.1))) {
				outs = snow;
			}
		}
	}
	return outs;
}

// Replaces the slots of `m` that are in the auto mask (manual paint with other slots wins).
ivec3 mst_auto_slots(ivec3 m, vec3 p, float ny, float ridge) {
	if (mst_auto_enabled == 0) {
		return m;
	}
	int a = mst_auto_slot(p, ny, ridge);
	if (a < 0) {
		return m;
	}
	ivec3 r = m;
	if (m.x < 16 && ((mst_auto_mask >> m.x) & 1) != 0) { r.x = a; }
	if (m.y < 16 && ((mst_auto_mask >> m.y) & 1) != 0) { r.y = a; }
	if (m.z < 16 && ((mst_auto_mask >> m.z) & 1) != 0) { r.z = a; }
	return r;
}

// ---------------------------------------------------------------- cursor

// Normalized brush distance (<= 1 inside), pt_core::brush::BrushShape::distance.
float mst_brush_dist(vec2 d, int shape) {
	float h = 0.8660254;
	if (shape == 1) {
		return max(abs(d.x), abs(d.y));
	}
	if (shape == 2) {
		return max(abs(dot(d, vec2(0.8660254, 0.5))), max(abs(d.y), abs(dot(d, vec2(-0.8660254, 0.5))))) / h;
	}
	if (shape == 3) {
		return max(abs(d.x), max(abs(dot(d, vec2(0.5, 0.8660254))), abs(dot(d, vec2(-0.5, 0.8660254))))) / h;
	}
	if (shape == 4) {
		return max(d.y, max(dot(d, vec2(-0.8660254, -0.5)), dot(d, vec2(0.8660254, -0.5)))) / 0.5;
	}
	if (shape == 5) {
		return max(-d.y, max(dot(d, vec2(0.8660254, 0.5)), dot(d, vec2(-0.8660254, 0.5)))) / 0.5;
	}
	return length(d);
}

// Brush cursor coverage: x = bright rim, y = dark outline (both sides of the rim, so the ring
// reads on light and dark ground), z = banded fill weight. Widths are in screen pixels (at
// least 2 px rim + 1 px outlines), so the ring stays visible from far away.
vec3 mst_cursor_mask(vec2 lp) {
	int shape = int(floor(mst_cursor.w + 0.5));
	float r = max(mst_cursor.z, 0.01);
	float d = mst_brush_dist((lp - mst_cursor.xy) / r, shape);
	float fw = max(fwidth(d), 1e-5);
	if (shape < 0) {
		return vec3(0.0);
	}
	float px = (1.0 - d) / fw;
	float rim_px = max(2.0, (mst_cursor_params.x / r) / fw);
	if (px < -1.5) {
		return vec3(0.0);
	}
	if (px < 0.0 || (px >= rim_px && px < rim_px + 1.0)) {
		return vec3(0.0, 1.0, 0.0);
	}
	if (px < rim_px) {
		return vec3(1.0, 0.0, 0.0);
	}
	// Banded falloff fill (4 steps) so the footprint reads as pixel art.
	float w = mst_cursor_params.z > 0.5 ? floor((1.0 - d) * 4.0 + 1.0) / 4.0 : 1.0;
	return vec3(0.0, 0.0, w);
}

vec3 mst_apply_cursor_mask(vec3 col, vec3 m) {
	col = mix(col, vec3(0.03, 0.03, 0.04), m.y);
	col = mix(col, mst_cursor_color.rgb, m.x);
	return mix(col, mst_cursor_color.rgb, clamp(mst_cursor_params.y * m.z * mst_cursor_color.a, 0.0, 1.0));
}

// Emission that keeps the rim bright in shadow / at night (the albedo alone is lit).
vec3 mst_cursor_glow(vec3 m) {
	return mst_cursor_color.rgb * (m.x * 0.55 + m.z * mst_cursor_params.y * 0.25);
}

vec3 mst_apply_cursor(vec3 col, vec2 lp) {
	return mst_apply_cursor_mask(col, mst_cursor_mask(lp));
}

// ---------------------------------------------------------------- grid lines

float mst_line(float dist, float width) {
	return dist < width ? 1.0 : 0.0;
}

// 1 on a chunk border (a neighbour `o` away belongs to another chunk).
float mst_chunk_border(vec2 lp, float o) {
	int g = mst_grid_code();
	ivec2 st = ivec2(mst_grid.yz + 0.5);
	int rad = int(mst_grid.w + 0.5);
	ivec2 c = mst_chunk_of(mst_cell_of(lp, g, mst_cell_size), g, st, rad);
	vec2 offs[4] = vec2[4](vec2(o, 0.0), vec2(-o, 0.0), vec2(0.0, o), vec2(0.0, -o));
	for (int i = 0; i < 4; i++) {
		if (mst_chunk_of(mst_cell_of(lp + offs[i], g, mst_cell_size), g, st, rad) != c) {
			return 1.0;
		}
	}
	return 0.0;
}

vec3 mst_apply_grid(vec3 col, vec2 lp, float view_dist, bool wall) {
	if (mst_grid_line.x < 0.5 || wall) {
		return col;
	}
	float a = mst_grid_color.a;
	if (mst_grid_line.z > 0.0) {
		a *= 1.0 - smoothstep(mst_grid_line.z * 0.7, mst_grid_line.z, view_dist);
	}
	float w = mst_grid_line.y;
	float on = mst_line(mst_edge_dist(lp, mst_grid_code(), mst_cell_size), w);
	col = mix(col, mst_grid_color.rgb, on * a);
	if (mst_grid_line.w > 0.5) {
		col = mix(col, mst_chunk_color.rgb, mst_chunk_border(lp, w * 1.5) * mst_chunk_color.a);
	}
	return col;
}

// ---------------------------------------------------------------- cell data (fog, tint)

vec3 mst_apply_cells(vec3 col, vec3 world, vec2 lp, vec2 px, float time, bool wall) {
	if (mst_fog == 0 || !mst_cell_data_ready()) {
		return col;
	}
	int g = mst_grid_code();
	ivec2 c = mst_cell_of(lp, g, mst_cell_size);
	vec4 tint = mst_cell_tint(c);
	col = mix(col, tint.rgb, tint.a);
	vec4 st = mst_cell_state(c);
	if (st.z > 0.0 && !wall && mst_edge_dist(lp, g, mst_cell_size) < mst_highlight.x) {
		float pulse = mst_highlight.y > 0.0 ? 0.75 + 0.25 * sin(time * mst_highlight.y) : 1.0;
		vec3 hc = tint.a > 0.0 ? tint.rgb : vec3(1.0, 0.95, 0.6);
		col = mix(col, hc, st.z * pulse);
	}
	if (mst_fog < 2) {
		return col;
	}
	return mst_apply_fog(col, mst_fog_at(lp), px, mst_fog_a.rgb, mst_fog_b.rgb, mst_fog_hidden);
}

// ---------------------------------------------------------------- debug views

vec3 mst_heat(float t) {
	t = clamp(t, 0.0, 1.0);
	return clamp(vec3(1.5 * t - 0.25, 1.0 - abs(t - 0.5) * 2.0 + 0.3 * t, 1.0 - 1.8 * t), 0.0, 1.0);
}

vec3 mst_slot_debug_color(int s) {
	vec3 pal[16] = vec3[16](
		vec3(0.30, 0.75, 0.25), vec3(0.65, 0.85, 0.30), vec3(0.15, 0.45, 0.20), vec3(0.95, 0.85, 0.50),
		vec3(0.55, 0.38, 0.22), vec3(0.60, 0.55, 0.50), vec3(0.30, 0.28, 0.30), vec3(0.97, 0.97, 1.00),
		vec3(0.62, 0.62, 0.70), vec3(0.85, 0.45, 0.25), vec3(0.25, 0.60, 0.45), vec3(0.85, 0.70, 0.45),
		vec3(0.50, 0.50, 0.85), vec3(0.20, 0.20, 0.25), vec3(0.50, 0.80, 0.95), vec3(1.00, 0.00, 1.00));
	return pal[clamp(s, 0, 15)];
}

// Returns the debug colour (or col when the view is off).
vec3 mst_debug_color(vec3 col, vec3 lpos, vec3 n, int slot, bool wall) {
	int v = mst_debug_view;
	if (v == 0) {
		return col;
	}
	vec2 lp = lpos.xz;
	int g = mst_grid_code();
	float lw = max(mst_grid_line.y, 0.05);
	if (v == 1) {
		float slope = degrees(acos(clamp(n.y, -1.0, 1.0)));
		return mst_heat(floor(slope / 10.0) / 9.0);
	}
	if (v == 2) {
		float st = max(mst_contour_step, 0.01);
		float band = floor(lpos.y / st);
		vec3 base = mst_heat(fract(band / 12.0)) * (0.75 + 0.25 * mod(band, 2.0));
		float f = mst_fract_dist(lpos.y / st) * st;
		float major = mod(band, 5.0) < 0.5 ? 1.0 : 0.0;
		return f < 0.06 * (1.0 + major) && !wall ? vec3(0.05) : base;
	}
	if (v == 3) {
		return mst_slot_debug_color(slot) * (wall ? 0.7 : 1.0);
	}
	if (v == 4) {
		ivec2 ch = mst_chunk_of(mst_cell_of(lp, g, mst_cell_size), g, ivec2(mst_grid.yz + 0.5), int(mst_grid.w + 0.5));
		vec3 cc = vec3(mst_hash_cell(ch, 1.0), mst_hash_cell(ch, 2.0), mst_hash_cell(ch, 3.0)) * 0.6 + 0.3;
		vec3 c = mix(col, cc, 0.55);
		return wall ? c * 0.8 : mix(c, vec3(1.0, 0.2, 0.1), mst_chunk_border(lp, lw * 2.0));
	}
	if (v == 5) {
		ivec2 c = mst_cell_of(lp, g, mst_cell_size);
		vec3 cc = vec3(mst_hash_cell(c, 4.0), mst_hash_cell(c, 5.0), mst_hash_cell(c, 6.0)) * 0.7 + 0.2;
		return wall ? cc * 0.6 : mix(cc, vec3(0.05), mst_line(mst_edge_dist(lp, g, mst_cell_size), lw));
	}
	if (v == 6) {
		if (!mst_cell_data_ready()) {
			return vec3(0.5, 0.0, 0.5);
		}
		ivec2 c = mst_cell_of(lp, g, mst_cell_size);
		vec4 st = mst_cell_state(c);
		vec4 tn = mst_cell_tint(c);
		return mix(vec3(st.x, st.y, st.z), tn.rgb, tn.a * 0.5);
	}
	return col;
}
