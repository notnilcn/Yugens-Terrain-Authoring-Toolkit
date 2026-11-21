// Marching Squares Terrain - GPU-instanced HexPrism columns (hex_edge_instanced).
// GLSL port of pt_core::prism::place: keep the two in sync (pt_core tests compare the CPU
// version with the meshed HexPrism surface). The terrain node inserts this include and a call
// at the top of the terrain shader's vertex() (a runtime variant of the material's shader), so
// template vertices (CUSTOM1.z < -0.5) are moved to their place from the cell map and every
// other mesh is untouched; fragment() and light() stay the terrain shader's own.
//
// Column map (pt_core::prism::build_column_map, RGBA float, 6 texels per cell at
// (x * 6 + k, y) for cell mst_prism_origin + (x, y)), computed on the CPU per column:
//   T0 (height, floor slot, wall slot + 16 * grass, bevel), T1 drop[0..4],
//   T2 (drop[4], drop[5], neighbour floors 4 bits per edge (15 = none), 0),
//   T3..T5 the 12 side feet (corner i, its edges) at 2 * i + (edge == first ? 0 : 1).
// Template vertex: CUSTOM0 = (role, corner, edge, ring), CUSTOM1 = (normal edge, kind,
// -1 - normal kind, 0), UV2 = (blend edge 0, blend edge 1). INSTANCE_CUSTOM = (q, r, low, groove rings of the template).

uniform sampler2D mst_prism_map : filter_nearest, repeat_disable;
uniform ivec2 mst_prism_origin = ivec2(0);
/// Flat-to-flat cell width (cell_size.x).
uniform float mst_prism_width = 2.0;
uniform float mst_prism_level = 0.5;
/// Bevel / groove size as a fraction of the apothem.
uniform float mst_prism_bevel = 0.14;
/// Odd-r offset hex grid (Hex) instead of axial hex rings.
uniform bool mst_prism_offset = false;

struct MstPrismV {
	vec3 pos;
	vec3 nrm;
	vec2 uv;
	vec4 c0;
	vec4 c1;
};

vec4 mst_prism_col(ivec2 c, int k) {
	ivec2 t = c - mst_prism_origin;
	return texelFetch(mst_prism_map, ivec2(t.x * 6 + k, t.y), 0);
}

vec2 mst_prism_center(ivec2 c) {
	float w = mst_prism_width;
	if (mst_prism_offset) {
		return vec2(w * (float(c.x) + 0.5 * float(c.y & 1)), w * 0.8660254 * float(c.y));
	}
	return vec2(w * (float(c.x) + 0.5 * float(c.y)), w * 0.8660254 * float(c.y));
}

vec2 mst_prism_corner_dir(int i) {
	float a = radians(-30.0 + 60.0 * float(i));
	return vec2(cos(a), sin(a));
}

vec2 mst_prism_out_dir(int e) {
	float a = radians(60.0 * float(e));
	return vec2(cos(a), sin(a));
}

// The two edges meeting at corner i (mesher order).
ivec2 mst_prism_corner_edges(int i) {
	return i == 0 ? ivec2(0, 5) : ivec2(i - 1, i);
}

// Corner i inset by d1 / d2 along the inward normals of its two edges.
vec2 mst_prism_inset(int i, float d1, float d2, float r) {
	vec2 c = mst_prism_corner_dir(i) * r;
	if (d1 == 0.0 && d2 == 0.0) {
		return c;
	}
	ivec2 ce = mst_prism_corner_edges(i);
	vec2 n1 = -mst_prism_out_dir(ce.x);
	vec2 n2 = -mst_prism_out_dir(ce.y);
	float det = n1.x * n2.y - n1.y * n2.x;
	return c + vec2((d1 * n2.y - d2 * n1.y) / det, (n1.x * d2 - n2.x * d1) / det);
}

MstPrismV mst_prism_vertex(vec4 inst, vec4 tc0, vec4 tc1, vec2 tuv2) {
	MstPrismV o;
	ivec2 c = ivec2(round(inst.xy));
	bool low = inst.z > 0.5;
	float lvl = max(abs(mst_prism_level), 1e-3);
	float a = mst_prism_width * 0.5;
	float r = mst_prism_width / sqrt(3.0);
	vec4 t0 = mst_prism_col(c, 0);
	vec4 t1 = mst_prism_col(c, 1);
	vec4 t2 = mst_prism_col(c, 2);
	float h = t0.x;
	float own_floor = t0.y;
	float aw = max(round(t0.z), 0.0);
	float wall_slot = mod(aw, 16.0);
	float grass = (aw >= 16.0 && !low) ? 1.0 : 0.0;
	float b = low ? 0.0 : t0.w;
	int packed = int(round(t2.z));
	float drop[6];
	float nbf[6];
	bool bev[6];
	drop[0] = t1.x; drop[1] = t1.y; drop[2] = t1.z; drop[3] = t1.w; drop[4] = t2.x; drop[5] = t2.y;
	for (int n = 0; n < 6; n++) {
		nbf[n] = float((packed >> (4 * n)) & 15);
		bev[n] = drop[n] > 1e-4 && !low;
	}
	float gb = low ? 0.0 : min(min(mst_prism_bevel * a, lvl * 0.4), a * 0.48);
	float dd[6];
	int kmax[6];
	int kmin[6];
	for (int n = 0; n < 6; n++) {
		dd[n] = bev[n] ? b : 0.0;
		if (drop[n] <= 1e-4 || low) {
			kmax[n] = 0;
			kmin[n] = 1;
		} else {
			kmax[n] = int(floor((h - dd[n] - 1.5 * gb + 1e-4) / lvl));
			kmin[n] = int(ceil((h - drop[n] + gb - 1e-4) / lvl));
		}
	}

	int role = int(round(tc0.x));
	int i = int(round(tc0.y));
	int e = int(round(tc0.z));
	int ring = int(round(tc0.w));
	int ne = int(round(tc1.x));
	int kind = int(round(tc1.y));
	int nk = int(round(-1.0 - tc1.z));
	ivec2 ids = ivec2(round(tuv2));
	vec2 corner = mst_prism_corner_dir(i) * r;
	ivec2 ci = mst_prism_corner_edges(i);
	int eo = ci.x == e ? ci.y : ci.x;
	// Groove rings of this chunk's template (INSTANCE_CUSTOM.w).
	int grooves = int(round(inst.w));

	// Groove `ring` of side e.
	int k = kmax[e] - ring;
	bool groove = ring < grooves && k >= kmin[e];
	float gy = float(k) * lvl;
	// Foot of side e at corner i (precomputed; only foot vertices and collapsed rings).
	bool need_foot = role == 9 || (role >= 5 && role <= 8 && !groove);
	float foot = h;
	if (need_foot) {
		int fi = 2 * i + (ci.x == e ? 0 : 1);
		vec4 ft = mst_prism_col(c, 3 + fi / 4);
		int fk = fi % 4;
		foot = fk == 0 ? ft.x : (fk == 1 ? ft.y : (fk == 2 ? ft.z : ft.w));
	}
	// Groove inner point at this corner: meets the other side's groove, or ends on its line.
	vec2 inner = corner;
	bool cap = false;
	if (groove) {
		bool other = k <= kmax[eo] && k >= kmin[eo] && kmax[eo] - k < grooves;
		if (other) {
			inner = mst_prism_inset(i, gb, gb, r);
		} else {
			int fc = eo == i ? (i + 1) % 6 : (i + 5) % 6;
			vec2 u = normalize(mst_prism_corner_dir(fc) - mst_prism_corner_dir(i));
			inner = corner + u * (gb / max(dot(u, -mst_prism_out_dir(e)), 0.2));
			cap = true;
		}
	}

	vec3 p = vec3(0.0, h, 0.0);
	if (role == 1) {
		p = vec3(mst_prism_inset(i, dd[ci.x], dd[ci.y], r) * 0.4, h).xzy;
	} else if (role == 2) {
		p = vec3(mst_prism_inset(i, dd[ci.x], dd[ci.y], r), h).xzy;
	} else if (role == 3) {
		int j2 = (e + 1) % 6;
		ivec2 cj = mst_prism_corner_edges(j2);
		ivec2 cei = mst_prism_corner_edges(e);
		vec2 m = (mst_prism_inset(e, dd[cei.x], dd[cei.y], r) + mst_prism_inset(j2, dd[cj.x], dd[cj.y], r)) * 0.5;
		p = vec3(m.x, h, m.y);
	} else if (role == 4) {
		p = vec3(corner.x, h - dd[e], corner.y);
	} else if (role == 9) {
		p = vec3(corner.x, foot, corner.y);
	} else if (role >= 5 && role <= 8) {
		if (!groove) {
			p = vec3(corner.x, foot, corner.y);
		} else if (role == 5) {
			p = vec3(corner.x, gy, corner.y);
		} else if (role == 7) {
			p = vec3(corner.x, gy - gb, corner.y);
		} else if (role == 6 || cap) {
			p = vec3(inner.x, gy, inner.y);
		} else {
			p = vec3(corner.x, gy, corner.y);
		}
	}
	vec2 cc = mst_prism_center(c);
	o.pos = p + vec3(cc.x, 0.0, cc.y);
	vec2 od = mst_prism_out_dir(ne);
	o.nrm = nk == 0 ? vec3(0.0, 1.0, 0.0) : (nk == 1 ? vec3(0.0, -1.0, 0.0) : (nk == 2 ? vec3(od.x, 0.0, od.y) : normalize(vec3(od.x, 1.0, od.y))));

	if (kind == 0) {
		// Floor: blend weights over (own, ids.x, ids.y) and the ledge band (UV.x).
		vec3 wv = vec3(1.0, 0.0, 0.0);
		int wa = -1;
		int wb = -1;
		float wa_w = 0.0;
		float wb_w = 0.0;
		if (!low) {
			if (role == 2) {
				bool b1 = bev[ci.x];
				bool b2 = bev[ci.y];
				if (!b1 && !b2) {
					wv.x = 1.0 / 3.0; wa = ci.x; wa_w = 1.0 / 3.0; wb = ci.y; wb_w = 1.0 / 3.0;
				} else if (b1 && !b2) {
					wv.x = 0.5; wa = ci.y; wa_w = 0.5;
				} else if (!b1 && b2) {
					wv.x = 0.5; wa = ci.x; wa_w = 0.5;
				}
			} else if (role == 3 && !bev[e]) {
				wv.x = 0.5; wa = e; wa_w = 0.5;
			}
		}
		vec3 wt = vec3(wv.x, 0.0, 0.0);
		if (wa >= 0) {
			if (wa == ids.x) { wt.y += wa_w; } else if (wa == ids.y) { wt.z += wa_w; } else { wt.x += wa_w; }
		}
		if (wb >= 0) {
			if (wb == ids.x) { wt.y += wb_w; } else if (wb == ids.y) { wt.z += wb_w; } else { wt.x += wb_w; }
		}
		float s0 = own_floor;
		float s1 = own_floor;
		float s2 = own_floor;
		if (!low) {
			if (ids.x >= 0 && nbf[ids.x] < 14.5 && abs(drop[ids.x]) <= 1e-4) { s1 = nbf[ids.x]; }
			if (ids.y >= 0 && nbf[ids.y] < 14.5 && abs(drop[ids.y]) <= 1e-4) { s2 = nbf[ids.y]; }
		}
		bool l1 = nbf[ci.x] < 14.5 && drop[ci.x] < -1e-4;
		bool l2 = nbf[ci.y] < 14.5 && drop[ci.y] < -1e-4;
		bool le = nbf[e] < 14.5 && drop[e] < -1e-4;
		float band = (l1 || l2) ? 1.0 : 0.0;
		float ux = role == 2 ? band : (role == 1 ? band * 0.35 : (role == 3 ? (le ? 1.0 : 0.0) : 0.0));
		o.uv = vec2(low ? 0.0 : ux, 0.0);
		o.c0 = vec4(own_floor, s1, s2, wall_slot);
		o.c1 = vec4(wt.x, wt.y, 0.0, grass);
	} else {
		o.uv = vec2(1.0);
		o.c0 = vec4(wall_slot);
		o.c1 = vec4(1.0, 0.0, low ? 0.6 : (kind == 2 ? 0.75 : 1.0), 0.0);
	}
	return o;
}
