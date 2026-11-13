// Marching Squares Terrain - analytic grid-cell math and the per-cell data texture (fog of war,
// tint, highlight). Spatial-only (uses no stage built-ins itself; callers pass them in).
// CPU references: pt_core::overlay (cell math) and pt_core::celldata (texture layout), pinned
// by cargo tests. Cell coordinates are the terrain's sample coordinates: axial (q, r) for hex
// rings, odd-r offset for hex, (2i + o, j) for triangles, floor(p / cell_size) for square.
//
// Globals (registered in globals.rs, published by the MarchingSquaresTerrain whose fog is on):
global uniform sampler2D pt_cell_data;   // RGBA8, texel (2i, j) = (visible, explored, highlight, 1), (2i+1, j) = tint
global uniform vec4 pt_cell_rect;        // origin x, origin y, width, height (cells); width 0 = no data
global uniform vec4 pt_cell_grid;        // grid code (0 sq, 1 tri, 2 hex, 3 hex rings), cell size x, cell size y, transition soft width
global uniform vec4 pt_cell_xform;       // terrain origin x, z, 1 / scale x, 1 / scale z
global uniform vec4 pt_cell_fog;         // mode (2 = fog of war on), hidden dim, hidden saturation, hatch
global uniform vec4 pt_cell_fog_color;   // unexplored fog colour

// floor(a / b) for b > 0, exact for the small integers used here.
int mst_fdiv(int a, int b) {
	return int(floor((float(a) + 0.5) / float(b)));
}

ivec2 mst_axial_round(float q, float r) {
	float s = -q - r;
	float rq = floor(q + 0.5);
	float rr = floor(r + 0.5);
	float rs = floor(s + 0.5);
	float dq = abs(rq - q);
	float dr = abs(rr - r);
	float ds = abs(rs - s);
	if (dq > dr && dq > ds) {
		rq = -rr - rs;
	} else if (dr > ds) {
		rr = -rq - rs;
	}
	return ivec2(int(rq), int(rr));
}

// Cell containing terrain-local XZ point p (pt_core::overlay::shader_cell).
ivec2 mst_cell_of(vec2 p, int grid, vec2 cs) {
	cs = max(cs, vec2(0.01));
	if (grid == 0) {
		return ivec2(floor(p / cs));
	}
	if (grid == 1) {
		float h = cs.x * 0.8660254;
		float beta = p.y / h;
		float alpha = p.x / cs.x - beta * 0.5;
		float i = floor(alpha);
		float j = floor(beta);
		int o = (alpha - i) + (beta - j) < 1.0 ? 0 : 1;
		return ivec2(2 * int(i) + o, int(j));
	}
	float radius = cs.x / 1.7320508;
	ivec2 a = mst_axial_round((p.x * 1.7320508 / 3.0 - p.y / 3.0) / radius, p.y * 2.0 / 3.0 / radius);
	if (grid == 2) {
		return ivec2(a.x + mst_fdiv(a.y, 2), a.y);
	}
	return a;
}

// Centre of a cell (square: the cell middle).
vec2 mst_cell_center(ivec2 c, int grid, vec2 cs) {
	if (grid == 0) {
		return (vec2(c) + 0.5) * cs;
	}
	if (grid == 1) {
		int i = mst_fdiv(c.x, 2);
		float s = cs.x;
		float h = s * 0.8660254;
		vec2 l00 = vec2(float(i) * s + float(c.y) * s * 0.5, float(c.y) * h);
		vec2 ex = vec2(s, 0.0);
		vec2 ey = vec2(s * 0.5, h);
		if (c.x - 2 * i == 0) {
			return l00 + (ex + ey) / 3.0;
		}
		return l00 + (ex + ey) * (2.0 / 3.0);
	}
	vec2 sp = vec2(cs.x, cs.x * 0.8660254);
	if (grid == 2) {
		return vec2(sp.x * (float(c.x) + 0.5 * float(c.y - 2 * mst_fdiv(c.y, 2))), sp.y * float(c.y));
	}
	return vec2(sp.x * (float(c.x) + 0.5 * float(c.y)), sp.y * float(c.y));
}

float mst_fract_dist(float x) {
	float f = fract(x);
	return min(f, 1.0 - f);
}

// World distance from p to the nearest edge of its cell (pt_core::overlay::edge_distance).
float mst_edge_dist(vec2 p, int grid, vec2 cs) {
	cs = max(cs, vec2(0.01));
	if (grid == 0) {
		return min(mst_fract_dist(p.x / cs.x) * cs.x, mst_fract_dist(p.y / cs.y) * cs.y);
	}
	if (grid == 1) {
		float h = cs.x * 0.8660254;
		float beta = p.y / h;
		float alpha = p.x / cs.x - beta * 0.5;
		return min(min(mst_fract_dist(alpha), mst_fract_dist(beta)), mst_fract_dist(alpha + beta)) * h;
	}
	vec2 v = p - mst_cell_center(mst_cell_of(p, grid, cs), grid, cs);
	float m = max(abs(v.x), max(abs(dot(v, vec2(0.5, 0.8660254))), abs(dot(v, vec2(-0.5, 0.8660254)))));
	return cs.x * 0.5 - m;
}

// Chunk owning a cell (hex rings: hexmod to_lower_res).
ivec2 mst_chunk_of(ivec2 c, int grid, ivec2 stride, int radius) {
	if (grid == 3) {
		int r = max(radius, 1);
		int q = c.x;
		int rr = c.y;
		int s = -q - rr;
		int area = 3 * r * (r + 1) + 1;
		int shift = 3 * r + 2;
		int a = mst_fdiv(rr + shift * q, area);
		int b = mst_fdiv(s + shift * rr, area);
		int cc = mst_fdiv(q + shift * s, area);
		return ivec2(mst_fdiv(1 + a - b, 3), mst_fdiv(1 + b - cc, 3));
	}
	return ivec2(mst_fdiv(c.x, max(stride.x, 1)), mst_fdiv(c.y, max(stride.y, 1)));
}

float mst_hash_cell(ivec2 c, float salt) {
	return pt_hash(vec2(c) * 0.731 + vec2(salt, salt * 1.37));
}

// ---------------------------------------------------------------- cell data

bool mst_cell_data_ready() {
	return pt_cell_rect.z > 0.5 && pt_cell_rect.w > 0.5;
}

ivec2 mst_cell_texel(ivec2 c) {
	return c - ivec2(pt_cell_rect.xy);
}

bool mst_cell_inside(ivec2 l) {
	return l.x >= 0 && l.y >= 0 && l.x < int(pt_cell_rect.z) && l.y < int(pt_cell_rect.w);
}

// (visible, explored, highlight, 1); outside the data rectangle = unexplored.
vec4 mst_cell_state(ivec2 c) {
	ivec2 l = mst_cell_texel(c);
	if (!mst_cell_inside(l)) {
		return vec4(0.0, 0.0, 0.0, 1.0);
	}
	return texelFetch(pt_cell_data, ivec2(l.x * 2, l.y), 0);
}

vec4 mst_cell_tint(ivec2 c) {
	ivec2 l = mst_cell_texel(c);
	if (!mst_cell_inside(l)) {
		return vec4(0.0);
	}
	return texelFetch(pt_cell_data, ivec2(l.x * 2 + 1, l.y), 0);
}

// World XZ -> terrain-local XZ of the terrain owning the cell data.
vec2 mst_cell_local(vec3 world) {
	return (world.xz - pt_cell_xform.xy) * pt_cell_xform.zw;
}

// (visible, explored) at a terrain-local point, softened across cell borders by averaging
// four taps `pt_cell_grid.w` apart (the dither in mst_apply_fog turns that into pixels).
vec2 mst_fog_at(vec2 lp) {
	int grid = int(pt_cell_grid.x + 0.5);
	vec2 cs = pt_cell_grid.yz;
	float o = pt_cell_grid.w;
	vec2 s = mst_cell_state(mst_cell_of(lp, grid, cs)).xy * 2.0;
	if (o > 0.0) {
		s += mst_cell_state(mst_cell_of(lp + vec2(o, 0.0), grid, cs)).xy;
		s += mst_cell_state(mst_cell_of(lp - vec2(o, 0.0), grid, cs)).xy;
		s += mst_cell_state(mst_cell_of(lp + vec2(0.0, o), grid, cs)).xy;
		s += mst_cell_state(mst_cell_of(lp - vec2(0.0, o), grid, cs)).xy;
		return s / 6.0;
	}
	return s * 0.5;
}

// 4x4 Bayer threshold in [0, 1) for an integer pixel coordinate.
float mst_bayer4(vec2 px) {
	ivec2 p = ivec2(mod(floor(px), 4.0));
	int i = p.x + p.y * 4;
	int m[16] = int[16](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
	return (float(m[i]) + 0.5) / 16.0;
}

// World-anchored low-res pixel lattice (camera axes / texel size), so fog dither and hatching
// stay on the pixel grid without crawling. Falls back to screen pixels without a renderer.
vec2 mst_pixel_lattice(vec3 world, mat4 inv_view, vec2 frag) {
	if (pt_texel_world <= 0.0) {
		return floor(frag);
	}
	return floor(vec2(dot(world, inv_view[0].xyz), -dot(world, inv_view[1].xyz)) / pt_texel_world);
}

// Fog parameters (fog colour a, fog colour b, (hidden dim, hidden saturation, hatch, 0)).
// Unexplored -> dithered fog; explored but not visible -> desaturated, darker, hatched.
vec3 mst_apply_fog(vec3 col, vec2 ve, vec2 px, vec3 fog_a, vec3 fog_b, vec4 hidden) {
	float l = dot(col, vec3(0.299, 0.587, 0.114));
	vec3 dim = mix(vec3(l), col, hidden.y) * hidden.x;
	float hatch = mod(px.x + px.y, 4.0) < 1.0 ? hidden.z : 0.0;
	dim *= 1.0 - hatch;
	float t = mst_bayer4(px);
	vec3 c = ve.x > t ? col : dim;
	vec3 fog = mod(px.x + px.y, 2.0) < 1.0 ? fog_a : fog_b;
	return ve.y > t ? c : fog;
}

// Fog of war for moving effects (birds, smoke): true where the cell under `world` (straight
// down) is not currently visible, with a dithered edge, so they never give away what happens
// in hidden or remembered-only cells.
bool pt_fog_hides(vec3 world, vec2 px) {
	if (pt_cell_fog.x < 1.5 || !mst_cell_data_ready()) {
		return false;
	}
	return mst_fog_at(mst_cell_local(world)).x <= mst_bayer4(px);
}

// Fog of war for surfaces other than the terrain (water, waterfalls, props, PixelObjects) at a
// world point, from the terrain's published cell data. Returns (colour, emission scale).
vec4 pt_fog_surface(vec3 col, vec3 world, vec2 px) {
	if (pt_cell_fog.x < 1.5 || !mst_cell_data_ready()) {
		return vec4(col, 1.0);
	}
	vec2 ve = mst_fog_at(mst_cell_local(world));
	vec3 fa = pt_cell_fog_color.rgb;
	vec3 c = mst_apply_fog(col, ve, px, fa, fa * 1.25 + vec3(0.02), vec4(pt_cell_fog.yzw, 0.0));
	return vec4(c, ve.x);
}
