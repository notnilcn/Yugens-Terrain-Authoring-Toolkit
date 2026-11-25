// Marching Squares Terrain - low-resolution resolve pass (canvas_item, drawn into a SubViewport).
// Inputs are the low-res colour and meta renders of the scene. Per output texel:
//   1. per-object pixel sizes: replicate world-anchored k x k blocks (k = the sizes in
//      `block_sizes`, any k >= 2; the id pass carries sizes above 4),
//   2. outlines (depth silhouettes = exterior palette row, outline-id changes = interior row,
//      hover/selection ids = full silhouette) and creases (normal edges, lightened),
//   3. sun shafts (world-anchored light sheets) through the shared cloud field,
//   4. ordered dither (world-anchored) + palette LUT + simple grading.
shader_type canvas_item;
render_mode unshaded, blend_disabled;

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"

uniform sampler2D color_tex : filter_nearest;
uniform sampler2D meta_tex : filter_nearest;
/// Optional id pass (pt_core::meta::pack_id): full outline id + per-object palette/dither.
uniform sampler2D id_tex : filter_nearest;
uniform bool use_id_tex = false;
uniform ivec2 source_size = ivec2(1);
uniform ivec2 out_size = ivec2(1);
uniform ivec2 margin = ivec2(4);
/// World-lattice coordinates of output texel (0, 0) mod 24 (the Bayer sizes), for stable dither.
uniform ivec2 lattice = ivec2(0);
/// Per-object pixel sizes >= 2 in use (PixelRenderer collects them), ascending; only these
/// block sizes are tested per texel, so any size costs one decode.
uniform int block_sizes[16]; // render::object::MAX_BLOCK_SIZES
/// World-lattice coordinates of output texel (0, 0) mod each block size (same order), so every
/// size stays world-anchored however many sizes are in use.
uniform int block_phase_x[16];
uniform int block_phase_y[16];
uniform int block_size_count = 0;
uniform bool flip_source_y = false;
/// PixelRenderer.mode Low Res: no meta passes (every texel decodes as background).
uniform bool low_res_mode = false;

group_uniforms Outlines;
uniform bool outlines = true;
uniform bool outline_background = true;
uniform float depth_threshold = 0.3505;
/// Crease angle as 1 - cos(angle) between the normals of two touching texels of one object
/// (0.3 ~ 46 deg, 0.5 = 60 deg); only sharper ridges are highlighted.
uniform float normal_threshold = 0.5005;
uniform float outline_darken : hint_range(0.0, 1.0) = 0.55;
uniform float highlight : hint_range(0.0, 1.0) = 0.25;
/// Outline-id changes between touching surfaces draw the interior row.
uniform bool id_edges = true;
/// Wall folds: a depth step between two texels of the same object with (near) identical,
/// horizontal world normals is a fold of one continuous wall (e.g. a hex cliff corner whose
/// middle face is seen edge-on), not a silhouette, while the step is below this (world m).
/// 0 disables the test.
uniform float fold_limit = 4.0;
/// Edge-on walls (straight vertical depth steps onto lower ground of the same object) draw as
/// a ledge (lip highlight / interior row) instead of a silhouette.
uniform bool ledge_wall = true;
/// 256 x 2 (row 0 exterior/silhouette, row 1 interior). Alpha = opacity; alpha 0 = darken.
uniform sampler2D outline_rows : filter_nearest;
/// PixelOutlineColors.modulate_rows: rows modulate the scene (`col * rgb * 1.9980`, so about 0.5005 is
/// neutral) at the row's alpha instead of replacing it.
uniform bool modulate_rows = false;
group_uniforms;

group_uniforms Color;
uniform bool palette_enabled = false;
uniform sampler2D palette_lut : filter_nearest;
uniform float palette_mix : hint_range(0.0, 1.0) = 1.0;
uniform bool dither_enabled = true;
uniform float dither_spread = 0.035;
uniform int dither_size = 4;
uniform float saturation = 1.0;
uniform float contrast = 1.0;
uniform float brightness = 0.0;
group_uniforms;

group_uniforms God_Rays;
// Sun shafts after robert-k's god rays (godot-3d-pixel-art, MIT, see CREDITS.md): stacked,
// world-anchored light sheets containing the sun beam axis that face the camera; each sheet the
// view ray crosses adds the sunlight let through the cloud gaps at that point. Distances are
// metres. PixelRenderer drives every uniform.
uniform bool rays_enabled = false;
/// `ray_color` (or the sun's hue with `sun_color`).
uniform vec4 ray_color : source_color = vec4(1.0, 0.9845, 0.2665, 1.0);
/// Alpha of one lit sheet: SHEET_ALPHA x LDR_ALPHA_CALIBRATION x PixelRenderer.ray_alpha.
uniform float ray_alpha = 0.0805;
/// `count`: most sheets one view ray crosses.
uniform int ray_count = 26;
/// `depth` (m): how far a view ray gathers sheets.
uniform float ray_depth = 305.0;
/// No sheets beyond this distance (m; 0 = off).
uniform float ray_reach = 283.3905;
/// Optional extra quantisation of the stack (<= 1 off).
uniform int sheet_bands = 1;
uniform float sheet_gap = 2.2305;
uniform float sheet_energy = 2.6605;
uniform float sheet_stack_max = 3.0005;
uniform float sheet_falloff = 0.0;
uniform float sheet_depth_fade = 327.8805;
uniform float cloud_softness = 0.0015;
uniform float cloud_occlusion = 0.9005;
uniform float gap_erode = 0.4005;
/// `start_depth` (m) and the fade-in range after it (`depth_fade`, `depth_fade_power`).
uniform float start_depth = 5.9005;
uniform float depth_fade = 8.5905;
uniform float depth_fade_power = 1.3505;
/// 1 = the beams follow the sun direction, 0 = vertical.
uniform float sun_follow = 0.9975;
/// Cloud threshold of the sheets' cloud mask (the day curve, or 1 - PixelSky coverage so the
/// gaps match the ground shadows).
uniform float cloud_threshold = 0.5005;
/// LDR eye adaptation: the stack is scaled by min(1, eye_adaptation / potential stack of the
/// ray). 0 = off.
uniform float eye_adaptation = 0.0;
uniform vec3 cam_origin;
uniform vec3 cam_right = vec3(1.0, 0.0, 0.0);
uniform vec3 cam_up = vec3(0.0, 1.0, 0.0);
uniform vec3 cam_forward = vec3(0.0, 0.0, -1.0);
uniform bool cam_ortho = true;
group_uniforms;

/// 0 final, 1 normals, 2 depth, 3 pixel size, 4 outline flags + ids, 5 raw colour,
/// 6 outline cause (red silhouette, blue id edge, yellow crease, green suppressed fold),
/// 7 sun shafts only.
uniform int debug_view = 0;

const float BAYER4[16] = float[](0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
const float BAYER8[64] = float[](
	0.0, 32.0, 8.0, 40.0, 2.0, 34.0, 10.0, 42.0, 48.0, 16.0, 56.0, 24.0, 50.0, 18.0, 58.0, 26.0,
	12.0, 44.0, 4.0, 36.0, 14.0, 46.0, 6.0, 38.0, 60.0, 28.0, 52.0, 20.0, 62.0, 30.0, 54.0, 22.0,
	3.0, 35.0, 11.0, 43.0, 1.0, 33.0, 9.0, 41.0, 51.0, 19.0, 59.0, 27.0, 49.0, 17.0, 57.0, 25.0,
	15.0, 47.0, 7.0, 39.0, 13.0, 45.0, 5.0, 37.0, 63.0, 31.0, 55.0, 23.0, 61.0, 29.0, 53.0, 21.0);

struct Meta {
	bool obj;
	int size;
	int oid;
	bool outline;
	vec3 n;
	float depth;
	bool pal;
	bool dith;
};

int pmod(int a, int b) {
	return ((a % b) + b) % b;
}

vec2 src_uv(ivec2 s) {
	vec2 uv = (vec2(s) + 0.5) / vec2(source_size);
	if (flip_source_y) {
		uv.y = 1.0 - uv.y;
	}
	return uv;
}

Meta decode(ivec2 s) {
	Meta m;
	vec3 c = textureLod(meta_tex, src_uv(s), 0.0).rgb;
	ivec3 b = ivec3(round(c * 255.0)) - ivec3(64);
	if (low_res_mode || b.x < 0 || b.y < 0 || b.z < 0) {
		m.obj = false;
		m.size = 1;
		m.oid = 0;
		m.outline = false;
		m.n = vec3(0.0, 0.0, 1.0);
		m.depth = 1e6;
		m.pal = true;
		m.dith = true;
		return m;
	}
	uint v = (uint(b.x) * 192u + uint(b.y)) * 192u + uint(b.z);
	uint depth_id = v % 1365u;
	uint rest = v / 1365u;
	uint nid = rest % 81u;
	uint flags = rest / 81u;
	m.obj = true;
	m.size = int(flags & 3u) + 1;
	m.oid = int((flags >> 2u) & 7u);
	m.outline = (flags & 32u) != 0u;
	vec2 h = (vec2(float(nid % 9u), float(nid / 9u)) + 0.5) / 9.0 * 2.0 - 1.0;
	vec2 o = vec2(h.x + h.y, h.x - h.y) * 0.5;
	m.n = normalize(vec3(o, max(1.0 - abs(o.x) - abs(o.y), 0.0)));
	m.depth = float(depth_id) / 1364.0 * pt_meta_depth_range;
	m.pal = true;
	m.dith = true;
	if (use_id_tex) {
		ivec3 bi = ivec3(round(textureLod(id_tex, src_uv(s), 0.0).rgb * 255.0)) - ivec3(64);
		if (bi.x >= 0 && bi.y >= 0 && bi.z >= 0) {
			uint iv = (uint(bi.x) * 192u + uint(bi.y)) * 192u + uint(bi.z);
			uint aux = (iv / 256u) % 4u;
			m.oid = int(iv % 256u);
			m.size = int(iv / 1024u) + 1;
			m.pal = (aux & 1u) == 0u;
			m.dith = (aux & 2u) == 0u;
		}
	}
	return m;
}

// Picks the source texel that output texel s shows, honouring per-object pixel sizes:
// a k-sized object is drawn as world-anchored k x k blocks sampled at the block anchor.
ivec2 resolve(ivec2 s, out Meta best) {
	Meta own = decode(s);
	best = own;
	ivec2 chosen = s;
	for (int i = 0; i < 16; i++) {
		if (i >= block_size_count) {
			break;
		}
		int k = block_sizes[i];
		ivec2 l = ivec2(s.x - margin.x + block_phase_x[i], block_phase_y[i] - (s.y - margin.y));
		ivec2 a = ivec2(s.x - pmod(l.x, k), s.y + pmod(l.y, k));
		if (a == s) {
			continue;
		}
		Meta ma = decode(a);
		if (ma.obj && ma.size == k && ((own.obj && own.size == k) || ma.depth < best.depth)) {
			best = ma;
			chosen = a;
		}
	}
	return chosen;
}

// Outline colour for palette row (0 exterior, 1 interior) of an outline id.
vec3 outline_tint(vec3 col, int oid, int row) {
	vec4 oc = texelFetch(outline_rows, ivec2(clamp(oid, 0, 255), row), 0);
	if (oc.a > 0.0) {
		return mix(col, modulate_rows ? col * oc.rgb * 1.9980 : oc.rgb, oc.a);
	}
	return col * (1.0 - outline_darken * (row == 0 ? 1.0 : 0.5));
}

// World-space normal of a decoded view-space normal (camera basis uniforms).
vec3 world_normal(vec3 n) {
	return normalize(normalize(cam_right) * n.x + normalize(cam_up) * n.y - normalize(cam_forward) * n.z);
}

// Screen-space depth slope (m per texel step along off) that a surface with view normal n at
// view depth `depth` has. In ortho a texel is pt_texel_world wide everywhere; in perspective it
// widens with depth (2 tan(fov / 2) / source height per metre). Perspective applies it to
// floors only (world normal pointing up), which recede by many texel widths per texel at a
// grazing angle, with a looser normal clamp; walls keep no allowance there, so a wall standing
// just in front of another still outlines. Without it every row of distant ground stepped past
// the depth threshold and drew as a silhouette (dashed horizontal lines and dark speckled
// grass towards the horizon).
float texel_slope(vec3 n, ivec2 off, float depth) {
	float texel = pt_texel_world;
	float facing = max(n.z, 0.15);
	if (!cam_ortho) {
		if (world_normal(n).y <= 0.7) {
			return 0.0;
		}
		texel = depth * 2.0 * length(cam_up) / float(max(source_size.y, 1));
		facing = max(n.z, 0.03);
	}
	vec2 sl = abs(n.xy) / facing * texel;
	return dot(sl, abs(vec2(off)));
}

// True when the depth step from m0 to its farther neighbour mn is a fold of one continuous
// wall: same object (outline id + pixel size), parallel faces, both vertical.
bool is_fold(Meta m0, Meta mn, float dd) {
	if (fold_limit <= 0.0 || !mn.obj || mn.oid != m0.oid || mn.size != m0.size || dd >= fold_limit) {
		return false;
	}
	if (dot(m0.n, mn.n) < 0.97) {
		return false;
	}
	return abs(world_normal(m0.n).y) < 0.35 && abs(world_normal(mn.n).y) < 0.35;
}

// One row of an edge-on wall: texel r is in front of its neighbour r + off (same outline id
// and pixel size) by more than the depth threshold.
bool ledge_step(ivec2 r, ivec2 off, int oid, int size) {
	Meta a;
	Meta b;
	resolve(r, a);
	resolve(r + off, b);
	return a.obj && b.obj && a.oid == oid && b.oid == oid && a.size == size && b.size == size
		&& b.depth - a.depth - 1.5 * texel_slope(a.n, off, a.depth) > depth_threshold;
}

// Ledges: a vertical wall seen exactly edge-on (square terrain under an axis-aligned camera)
// has no width on screen, so the plateau edge or wall corner in front of it meets the lower
// ground directly and the depth step runs as a straight vertical line (>= 3 texels). That
// line flips between nothing and a 1-texel face as the camera turns, so it is drawn as a
// ledge (top lip highlight / interior row) instead of a hard silhouette. Back-facing cliff
// tops (slanted steps) keep the silhouette.
bool edge_on_ledge(ivec2 s, Meta m0, Meta mn, ivec2 off) {
	if (!ledge_wall || off.y != 0 || !mn.obj || mn.oid != m0.oid || mn.size != m0.size) {
		return false;
	}
	bool up = ledge_step(s + ivec2(0, -1), off, m0.oid, m0.size);
	bool dn = ledge_step(s + ivec2(0, 1), off, m0.oid, m0.size);
	if (up && dn) {
		return true;
	}
	if (up) {
		return ledge_step(s + ivec2(0, -2), off, m0.oid, m0.size);
	}
	if (dn) {
		return ledge_step(s + ivec2(0, 2), off, m0.oid, m0.size);
	}
	return false;
}

// 1 when the edge between this texel (m0) and its neighbour mn at screen offset `off` is
// convex (a ridge), else 0. Only ridges get the crease highlight; valleys (inner corners of a
// helmet, a cliff foot meeting the floor) stay clean.
// Across a ridge the normal turns towards the step direction (the neighbour's normal leans
// further along the view-space offset than this texel's); across a valley it turns back. A
// cliff foot meeting the floor is a valley (wall then floor going down the screen: the normal
// turns up, against the step), so square terrain draws no second line along every wall foot.
// The test uses normals only: the previous tangent-plane test needed the depth step between two
// texels, but the meta depth quantum (pt_meta_depth_range / 1364, ~0.15 m at the default range)
// is coarser than a texel (a few cm), so neighbours mostly decode at equal depth and every
// edge of a helmet or rock passed as convex, valleys included.
float convex_step(Meta m0, Meta mn, ivec2 off) {
	vec2 d = vec2(float(off.x), -float(off.y)); // source y runs down, view y up
	return step(0.02, dot(mn.n.xy - m0.n.xy, d));
}

// Whether a crease towards the neighbour at `off` (normal nn, strength c = 1 - cos) is drawn.
// The matching ids rule: the texel on this side (s - off) must belong to the
// same object (and surface: no depth step) too, so a crease never runs as a 1-texel sliver along a silhouette or id edge.
// Curved low-poly surfaces are skipped: when the normal keeps turning the same way two texels
// back (narrow facets of a rounded mesh) the change is spread out instead of concentrated on
// one edge (a crease between two flat faces has the same normal there).
bool crease_drawn(ivec2 s, Meta m0, ivec2 off, vec3 nn, float c) {
	Meta mb;
	resolve(s - off, mb);
	if (!mb.obj || mb.oid != m0.oid || mb.size != m0.size || abs(mb.depth - m0.depth) >= depth_threshold) {
		return false;
	}
	Meta mo;
	resolve(s - 2 * off, mo);
	if (!mo.obj || mo.oid != m0.oid || mo.size != m0.size) {
		return true;
	}
	// The far texel is turned away from the crease neighbour's normal by a comparable amount:
	// the surface bends steadily through here.
	return !(1.0 - dot(m0.n, mo.n) > 0.5 * c && dot(mo.n, nn) < dot(m0.n, nn));
}

// Outline decision for output source texel s (whose resolved meta is m0):
// 3 silhouette (exterior row), 2 id edge (interior row), 1 crease highlight, 0 none,
// -1 a depth step that the fold / crease rules suppressed (debug view only).
int outline_kind(ivec2 s, Meta m0) {
	// Hover / selection ids (254, 255) outline the whole silhouette, even with outlines off.
	bool hl = m0.obj && m0.oid >= 254;
	if (!m0.obj || !((outlines && m0.outline) || hl)) {
		return 0;
	}
	ivec2 offs[4] = ivec2[](ivec2(1, 0), ivec2(-1, 0), ivec2(0, 1), ivec2(0, -1));
	float depth_edge = 0.0;
	float crease = 0.0;
	vec3 crease_n = m0.n;
	ivec2 crease_off = ivec2(1, 0);
	bool id_edge = false;
	bool hl_edge = false;
	bool folded = false;
	bool ledge = false;
	for (int i = 0; i < 4; i++) {
		Meta mn;
		resolve(s + offs[i], mn);
		if (hl && (!mn.obj || mn.oid != m0.oid)) {
			hl_edge = true;
		}
		if (!mn.obj && !outline_background) {
			continue;
		}
		float dd = mn.depth - m0.depth;
		// Surfaces seen at a grazing angle (cliff walls edge-on) change depth quickly
		// between texels of the same face: allow the slope the texel's own normal
		// predicts so they don't self-outline.
		float step_edge = dd - 1.5 * texel_slope(m0.n, offs[i], m0.depth);
		if (step_edge > depth_threshold && is_fold(m0, mn, dd)) {
			step_edge = 0.0;
			folded = true;
		}
		if (step_edge > depth_threshold && edge_on_ledge(s, m0, mn, offs[i])) {
			step_edge = 0.0;
			ledge = true;
		}
		depth_edge = max(depth_edge, step_edge);
		if (mn.obj && abs(dd) < depth_threshold) {
			// Touching surfaces with different outline ids: the nearer side draws the
			// interior row (ties broken by id so only one side draws).
			if (id_edges && mn.oid != m0.oid && (dd > 0.0 || (dd == 0.0 && m0.oid > mn.oid))) {
				id_edge = true;
			}
			// t3ssel8r-style convex crease: only the side whose normal leans
			// towards the neighbour lights up. Only within one object (different ids are id
			// edges) and per neighbour, not summed: a fixed threshold on the
			// largest normal change, so the many shallow facet edges of low-poly curved
			// surfaces (helmets, rocks, trunks) stay clean and only sharp creases light up.
			float same = mn.oid == m0.oid && mn.size == m0.size ? 1.0 : 0.0;
			float nd = dot(m0.n - mn.n, vec3(1.0));
			float ni = smoothstep(-0.01, 0.01, nd);
			float di = clamp(sign(dd * 0.25 + 0.0025), 0.0, 1.0);
			float c = (1.0 - dot(m0.n, mn.n)) * ni * di * same * convex_step(m0, mn, offs[i]);
			if (c > crease) {
				crease = c;
				crease_n = mn.n;
				crease_off = offs[i];
			}
		}
	}
	if (hl_edge || depth_edge > depth_threshold) {
		return 3;
	}
	if (id_edge || (ledge && world_normal(m0.n).y <= 0.6)) {
		return 2;
	}
	if (ledge) {
		return 1;
	}
	if (crease > normal_threshold && crease_drawn(s, m0, crease_off, crease_n, crease)) {
		return 1;
	}
	return folded ? -1 : 0;
}

vec3 id_color(int oid) {
	float h = fract(float(oid) * 0.618034);
	return clamp(abs(fract(vec3(h) + vec3(0.0, 0.6667, 0.3333)) * 6.0 - 3.0) - 1.0, 0.0, 1.0) * 0.8 + 0.2;
}

vec3 fetch_color(ivec2 s) {
	return textureLod(color_tex, src_uv(s), 0.0).rgb;
}

vec3 lut(vec3 c) {
	vec3 q = floor(clamp(c, 0.0, 1.0) * 31.0 + 0.5);
	vec2 uv = vec2((q.b * 32.0 + q.r + 0.5) / 1024.0, (q.g + 0.5) / 32.0);
	return textureLod(palette_lut, uv, 0.0).rgb;
}

// Sun shafts (the light-sheet part of robert-k's god rays): returns the sheet stack
// min(stack * sheet_energy, sheet_stack_max) for the view ray through source texel s that hits a
// surface at view depth `depth` (1e6 = background). The cloud field is the shared one (pt_fbm
// with PixelSky's scale and offset, projected along the sun onto the deck) so the shafts pour
// through the same gaps as the ground shadows.
// Ortho cameras start each ray on the near plane, perspective ones at the camera origin.
float sun_shafts(ivec2 s, float depth) {
	if (pt_cloud_c.x < 0.5) {
		return 0.0;
	}
	vec2 ndc = vec2((float(s.x) + 0.5) / float(source_size.x) * 2.0 - 1.0, 1.0 - (float(s.y) + 0.5) / float(source_size.y) * 2.0);
	vec3 cam_pos = cam_origin;
	vec3 dir = cam_forward;
	if (cam_ortho) {
		cam_pos += cam_right * ndc.x + cam_up * ndc.y;
	} else {
		dir = cam_forward + cam_right * ndc.x + cam_up * ndc.y;
	}
	float dir_len = length(dir);
	vec3 ray_dir = dir / max(dir_len, 1e-4);
	float full_len = depth < 1e5 ? depth * dir_len : ray_depth;
	float ray_len = ray_depth > 0.0 ? min(full_len, ray_depth) : full_len;

	vec3 sun = normalize(pt_sun_dir);
	float safe_y = abs(sun.y) < 1e-4 ? (sun.y >= 0.0 ? 1e-4 : -1e-4) : sun.y;
	float inv_sun_y = 1.0 / max(safe_y, 0.08);
	float follow = clamp(sun_follow, 0.0, 1.0);
	float inv_erode = 1.0 / max(1e-4, 1.0 - gap_erode);

	// The sheet planes contain the effective beam axis while facing the camera as closely
	// as possible.
	vec3 beam_dir = normalize(vec3(sun.x * inv_sun_y * follow, 1.0, sun.z * inv_sun_y * follow));
	vec3 fwd = normalize(cam_forward);
	vec3 sheet_n = fwd - beam_dir * dot(fwd, beam_dir);
	float sheet_n_len = length(sheet_n);
	sheet_n = sheet_n_len > 1e-4 ? sheet_n / sheet_n_len : normalize(vec3(sun.z, 0.0, -sun.x));

	float gap = max(sheet_gap, 0.01);
	float plane_d = dot(sheet_n, cam_pos);
	float dir_dot = dot(sheet_n, ray_dir);
	float stack = 0.0;
	float all_lit = 0.0;
	if (abs(dir_dot) > 1e-5) {
		float inv_dir_dot = 1.0 / dir_dot;
		float k_step = dir_dot > 0.0 ? 1.0 : -1.0;
		float k_start = dir_dot > 0.0 ? ceil(plane_d / gap) : floor(plane_d / gap);
		for (int i = 0; i < 52; i++) {
			if (i >= ray_count) {
				break;
			}
			float plane = k_start + k_step * float(i);
			float t_ray = (plane * gap - plane_d) * inv_dir_dot;
			if (t_ray > ray_len) {
				break;
			}
			vec3 sp = cam_pos + ray_dir * t_ray;
			vec2 q = sp.xz + sun.xz * ((pt_cloud_b.z - sp.y) * inv_sun_y * follow);
			float cloud = pt_fbm(q * pt_cloud_a.z + pt_cloud_b.xy);
			float cloud_mask = smoothstep(cloud_threshold - cloud_softness, cloud_threshold + cloud_softness, cloud);
			float sun_vis = clamp(1.0 - cloud_mask * cloud_occlusion, 0.0, 1.0);
			sun_vis = clamp((sun_vis - gap_erode) * inv_erode, 0.0, 1.0);

			float fade = clamp((t_ray - start_depth) / max(depth_fade, 1e-4), 0.0, 1.0);
			fade = pow(fade, max(depth_fade_power, 1e-4));
			if (ray_reach > 0.0) {
				float feather = max(1.0, ray_reach * 0.05);
				fade *= clamp((ray_reach - t_ray) / feather, 0.0, 1.0);
			}
			if (sheet_depth_fade > 0.0) {
				fade *= clamp((full_len - t_ray) / sheet_depth_fade, 0.0, 1.0);
			}
			fade *= exp(-max(sheet_falloff, 0.0) * t_ray);
			stack += sun_vis * fade;
			all_lit += fade;
		}
	}
	float m = min(stack * sheet_energy, max(sheet_stack_max, 0.0));
	if (sheet_bands > 1 && sheet_stack_max > 0.0) {
		float b = float(sheet_bands);
		m = floor(m / sheet_stack_max * b + 0.5) / b * sheet_stack_max;
	}
	// Side-on views cross many sheets and sit at the cap everywhere (a flat veil in LDR):
	// scale the stack back by how many sheets the ray could gather (eye adaptation).
	if (eye_adaptation > 0.0) {
		m *= min(1.0, eye_adaptation / max(all_lit * sheet_energy, 1e-4));
	}
	return m;
}

void fragment() {
	ivec2 p = ivec2(floor(UV * vec2(out_size)));
	ivec2 s = p + margin;
	Meta m0;
	ivec2 c0 = resolve(s, m0);
	vec3 col = fetch_color(c0);
	ivec2 l = ivec2(p.x + lattice.x, lattice.y - p.y);

	if (debug_view == 1) {
		COLOR = vec4(m0.n * 0.5 + 0.5, 1.0);
	} else if (debug_view == 2) {
		COLOR = vec4(vec3(m0.obj ? 1.0 - m0.depth / max(pt_meta_depth_range, 0.001) : 0.0), 1.0);
	} else if (debug_view == 3) {
		float sz = clamp(log2(float(m0.size)) / 5.0, 0.0, 1.0); // 1 blue .. 32+ red
		COLOR = vec4(m0.obj ? vec3(sz, 0.2, 1.0 - sz) : vec3(0.0), 1.0);
	} else if (debug_view == 4) {
		COLOR = vec4(m0.obj ? id_color(m0.oid) * (m0.outline ? 1.0 : 0.45) : vec3(0.0), 1.0);
	} else if (debug_view == 5) {
		COLOR = vec4(col, 1.0);
	} else if (debug_view == 7) {
		COLOR = vec4(vec3(sun_shafts(c0, m0.obj ? m0.depth : 1e6) / max(sheet_stack_max, 1e-3)), 1.0);
	} else if (debug_view == 6) {
		int kind = outline_kind(s, m0);
		vec3 base = m0.obj ? vec3(0.25) + 0.1 * m0.n : vec3(0.0);
		COLOR = vec4(kind == 3 ? vec3(1.0, 0.15, 0.1) : kind == 2 ? vec3(0.1, 0.4, 1.0) : kind == 1 ? vec3(0.9, 0.9, 0.2) : kind == -1 ? vec3(0.1, 0.9, 0.2) : base, 1.0);
	} else {
		int kind = outline_kind(s, m0);
		if (kind == 3) {
			col = outline_tint(col, m0.oid, 0);
		} else if (kind == 2) {
			col = outline_tint(col, m0.oid, 1);
		} else if (kind == 1) {
			col = col * (1.0 + highlight);
		}

		float jitter;
		if (dither_size == 8) {
			jitter = BAYER8[pmod(l.y, 8) * 8 + pmod(l.x, 8)] / 64.0;
		} else {
			jitter = BAYER4[pmod(l.y, 4) * 4 + pmod(l.x, 4)] / 16.0;
		}

		if (rays_enabled) {
			// One sheet adds ray_color * ray_alpha where the cloud gaps let the sun through, in
			// linear light (the colour texture here holds sRGB values).
			float r = sun_shafts(c0, m0.obj ? m0.depth : 1e6) * ray_alpha * (1.0 - pt_night) * smoothstep(0.5, 1.0, pt_sun_light); // shafts fade with the sun through sunset / blue hour (PixelDayNight)
			// A filmic tonemapper rolls a bright HDR sum off towards white; this LDR buffer
			// has no headroom, so the add is a linear-light screen (a + b - ab): the same
			// lift in the shadows, no clipped washout on bright ground.
			vec3 lin = clamp(pow(max(col, vec3(0.0)), vec3(2.2)), 0.0, 1.0);
			vec3 add = clamp(ray_color.rgb * r, 0.0, 1.0);
			col = pow(lin + add * (1.0 - lin), vec3(1.0 / 2.2));
		}

		float lum = dot(col, vec3(0.299, 0.587, 0.114));
		col = mix(vec3(lum), col, saturation);
		col = (col - 0.5) * contrast + 0.5 + brightness;

		if (dither_enabled && m0.dith) {
			col += (jitter - 0.5) * dither_spread;
		}
		if (palette_enabled && m0.pal) {
			col = mix(col, lut(col), palette_mix);
		}
		COLOR = vec4(clamp(col, 0.0, 1.0), 1.0);
	}
}
