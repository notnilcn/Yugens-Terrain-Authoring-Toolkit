// Marching Squares Terrain - low-resolution resolve pass (canvas_item, drawn into a SubViewport).
// Inputs are the low-res colour and metadata renders of the scene. Per output texel:
//   1. per-object pixel sizes: replicate world-anchored k x k blocks (k = 1..4),
//   2. outlines (depth silhouettes = exterior palette row, outline-id changes = interior row,
//      hover/selection ids = full silhouette) and creases (normal edges, lightened),
//   3. god rays (Stable3D light sheets) through the shared cloud field,
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
/// World-lattice coordinates of output texel (0, 0) (mod 24), for stable blocks/dither.
uniform ivec2 lattice = ivec2(0);
uniform int max_pixel_size = 1;
uniform bool flip_source_y = false;
/// PixelRenderer.mode Low Res: no metadata passes (every texel decodes as background).
uniform bool low_res_mode = false;

group_uniforms Outlines;
uniform bool outlines = true;
uniform bool outline_background = true;
uniform float depth_threshold = 0.35;
uniform float normal_threshold = 0.3;
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
uniform sampler2D outline_palette : filter_nearest;
/// PixelOutlinePalette.multiply_colors: rows modulate the scene (`col * rgb * 2`, so 0.5 is
/// neutral) at the row's alpha instead of replacing it.
uniform bool outline_multiply = false;
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
// Port of Stable3D's StablePixelArtGodRays.ush (MIT, see CREDITS.md): stacked, world-anchored
// light sheets that contain the sun beam axis and face the camera; each sheet the view ray
// crosses adds the sunlight let through the cloud gaps at that point. Distances are metres
// (Stable3D's centimetre defaults / 100). PixelRenderer drives every uniform.
uniform bool rays_enabled = false;
/// RayColor (GodRayUseSunColor off) or the sun hue (on).
uniform vec4 ray_color : source_color = vec4(1.0, 0.984, 0.266, 1.0);
/// RayStrength x Global Brightness of the Stable3D default material instance (0.01 x 2),
/// times the LDR calibration (x4, renderer.rs LDR_RAY_CALIBRATION) and PixelRenderer.ray_strength.
uniform float ray_strength = 0.08;
/// Most sheets one view ray crosses (Stable3D: 24).
uniform int ray_samples = 24;
/// MaxRayLength (m).
uniform float ray_max_distance = 300.0;
/// Optional extra quantisation of the stack (<= 1 off; Stable3D has none).
uniform int ray_bands = 1;
uniform float ray_sheet_spacing = 2.23;
uniform float ray_sheet_quantum = 2.660933;
uniform float ray_sheet_max_stack = 3.0;
uniform float ray_sheet_distance_falloff = 0.0;
uniform float ray_sheet_surface_fade = 327.88;
uniform float ray_sheet_cloud_softness = 0.001;
uniform float ray_gap_erosion = 0.4;
uniform float ray_cloud_shadow_strength = 0.9;
uniform float ray_near_fade_start = 5.9;
uniform float ray_near_fade = 8.59;
uniform float ray_near_fade_power = 1.35;
uniform float ray_depth_cutoff = 283.39;
uniform float ray_sun_follow = 1.0;
/// Cloud threshold of the sheets' cloud mask (Stable3D's day-sequence curve, or
/// 1 - PixelSky coverage so the gaps match the ground shadows).
uniform float ray_cloud_threshold = 0.5;
/// LDR eye adaptation (not in Stable3D, whose HDR add is pulled back by UE's auto exposure):
/// the stack is scaled by min(1, ray_adaptation / potential stack of the ray). 0 = off.
uniform float ray_adaptation = 0.0;
uniform vec3 cam_origin;
uniform vec3 cam_right = vec3(1.0, 0.0, 0.0);
uniform vec3 cam_up = vec3(0.0, 1.0, 0.0);
uniform vec3 cam_forward = vec3(0.0, 0.0, -1.0);
uniform bool cam_ortho = true;
group_uniforms;

/// 0 final, 1 normals, 2 depth, 3 pixel size, 4 outline flags + ids, 5 raw colour,
/// 6 outline cause (red silhouette, blue id edge, yellow crease, green suppressed fold),
/// 7 god rays only.
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
			uint aux = iv / 256u;
			m.oid = int(iv % 256u);
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
	for (int k = 2; k <= max_pixel_size; k++) {
		ivec2 l = ivec2(s.x - margin.x + lattice.x, lattice.y - (s.y - margin.y));
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
	vec4 oc = texelFetch(outline_palette, ivec2(clamp(oid, 0, 255), row), 0);
	if (oc.a > 0.0) {
		return mix(col, outline_multiply ? col * oc.rgb * 2.0 : oc.rgb, oc.a);
	}
	return col * (1.0 - outline_darken * (row == 0 ? 1.0 : 0.5));
}

// World-space normal of a decoded view-space normal (camera basis uniforms).
vec3 world_normal(vec3 n) {
	return normalize(normalize(cam_right) * n.x + normalize(cam_up) * n.y - normalize(cam_forward) * n.z);
}

// Screen-space depth slope (m per texel step along off) that a surface with view normal n has
// (ortho texel size; 0 in perspective).
float texel_slope(vec3 n, ivec2 off) {
	vec2 sl = abs(n.xy) / max(n.z, 0.15) * pt_texel_world;
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
		&& b.depth - a.depth - 1.5 * texel_slope(a.n, off) > depth_threshold;
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

// Outline decision for output source texel s (whose resolved metadata is m0):
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
		// predicts (ortho texel size; 0 in perspective) so they don't self-outline.
		float step_edge = dd - 1.5 * texel_slope(m0.n, offs[i]);
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
			// towards the neighbour lights up.
			float nd = dot(m0.n - mn.n, vec3(1.0));
			float ni = smoothstep(-0.01, 0.01, nd);
			float di = clamp(sign(dd * 0.25 + 0.0025), 0.0, 1.0);
			// Convex folds only: the neighbour's point must not lie in front of this texel's
			// tangent plane. A cliff foot meeting the lower floor is concave, but under a tilted
			// ortho camera the floor texel there is still a little nearer than the wall texel,
			// which drew a second highlight along every wall foot (square terrain: "double
			// fences"). View-space step: screen offset in texels (source y runs down) and depth.
			vec3 dp = vec3(vec2(float(offs[i].x), -float(offs[i].y)) * pt_texel_world, -dd);
			float cv = step(dot(dp, m0.n), 0.1 * pt_texel_world);
			crease += (1.0 - dot(m0.n, mn.n)) * ni * di * cv;
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
	if (crease > normal_threshold * 0.1) {
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

// Stable3D god rays (StablePixelArtGodRays.ush, light-sheet part): returns the sheet stack
// min(stack * quantum, max_stack) for the view ray through source texel s that hits a surface
// at view depth `depth` (1e6 = background). The cloud field is the shared one (pt_fbm with
// PixelSky's scale and offset, projected along the sun onto the deck) so the shafts pour
// through the same gaps as the ground shadows; Stable3D's own value-noise FBM is not used.
// Ortho cameras start each ray on the near plane (Stable3D uses the camera origin for both).
float rays(ivec2 s, float depth) {
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
	float full_len = depth < 1e5 ? depth * dir_len : ray_max_distance;
	float ray_len = ray_max_distance > 0.0 ? min(full_len, ray_max_distance) : full_len;

	vec3 sun = normalize(pt_sun_dir);
	float safe_y = abs(sun.y) < 1e-4 ? (sun.y >= 0.0 ? 1e-4 : -1e-4) : sun.y;
	float inv_sun_y = 1.0 / max(safe_y, 0.08);
	float follow = clamp(ray_sun_follow, 0.0, 1.0);
	float inv_erode = 1.0 / max(1e-4, 1.0 - ray_gap_erosion);

	// The sheet planes contain the effective beam axis while facing the camera as closely
	// as possible.
	vec3 beam = normalize(vec3(sun.x * inv_sun_y * follow, 1.0, sun.z * inv_sun_y * follow));
	vec3 fwd = normalize(cam_forward);
	vec3 sheet_n = fwd - beam * dot(fwd, beam);
	float sheet_n_len = length(sheet_n);
	sheet_n = sheet_n_len > 1e-4 ? sheet_n / sheet_n_len : normalize(vec3(sun.z, 0.0, -sun.x));

	float spacing = max(ray_sheet_spacing, 0.01);
	float s0 = dot(sheet_n, cam_pos);
	float ndr = dot(sheet_n, ray_dir);
	float stack = 0.0;
	float potential = 0.0;
	if (abs(ndr) > 1e-5) {
		float inv_ndr = 1.0 / ndr;
		float k_dir = ndr > 0.0 ? 1.0 : -1.0;
		float k0 = ndr > 0.0 ? ceil(s0 / spacing) : floor(s0 / spacing);
		for (int i = 0; i < 48; i++) {
			if (i >= ray_samples) {
				break;
			}
			float k = k0 + k_dir * float(i);
			float tk = (k * spacing - s0) * inv_ndr;
			if (tk > ray_len) {
				break;
			}
			vec3 sp = cam_pos + ray_dir * tk;
			vec2 q = sp.xz + sun.xz * ((pt_cloud_b.z - sp.y) * inv_sun_y * follow);
			float cloud = pt_fbm(q * pt_cloud_a.z + pt_cloud_b.xy);
			float mask = smoothstep(ray_cloud_threshold - ray_sheet_cloud_softness, ray_cloud_threshold + ray_sheet_cloud_softness, cloud);
			float sun_vis = clamp(1.0 - mask * ray_cloud_shadow_strength, 0.0, 1.0);
			sun_vis = clamp((sun_vis - ray_gap_erosion) * inv_erode, 0.0, 1.0);

			float fade = clamp((tk - ray_near_fade_start) / max(ray_near_fade, 1e-4), 0.0, 1.0);
			fade = pow(fade, max(ray_near_fade_power, 1e-4));
			if (ray_depth_cutoff > 0.0) {
				float feather = max(1.0, ray_depth_cutoff * 0.05);
				fade *= clamp((ray_depth_cutoff - tk) / feather, 0.0, 1.0);
			}
			if (ray_sheet_surface_fade > 0.0) {
				fade *= clamp((full_len - tk) / ray_sheet_surface_fade, 0.0, 1.0);
			}
			fade *= exp(-max(ray_sheet_distance_falloff, 0.0) * tk);
			stack += sun_vis * fade;
			potential += fade;
		}
	}
	float m = min(stack * ray_sheet_quantum, max(ray_sheet_max_stack, 0.0));
	if (ray_bands > 1 && ray_sheet_max_stack > 0.0) {
		float b = float(ray_bands);
		m = floor(m / ray_sheet_max_stack * b + 0.5) / b * ray_sheet_max_stack;
	}
	// Side-on views cross many sheets and sit at the cap everywhere (a flat veil in LDR):
	// scale the stack back by how many sheets the ray could gather (eye adaptation).
	if (ray_adaptation > 0.0) {
		m *= min(1.0, ray_adaptation / max(potential * ray_sheet_quantum, 1e-4));
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
		COLOR = vec4(m0.obj ? vec3(float(m0.size) / 4.0, 0.2, 1.0 - float(m0.size) / 4.0) : vec3(0.0), 1.0);
	} else if (debug_view == 4) {
		COLOR = vec4(m0.obj ? id_color(m0.oid) * (m0.outline ? 1.0 : 0.45) : vec3(0.0), 1.0);
	} else if (debug_view == 5) {
		COLOR = vec4(col, 1.0);
	} else if (debug_view == 7) {
		COLOR = vec4(vec3(rays(c0, m0.obj ? m0.depth : 1e6) / max(ray_sheet_max_stack, 1e-3)), 1.0);
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
			// Stable3D: outputColor += rayTint * (godrayMask * RayStrength), added in linear
			// light like UE's scene colour (the colour texture here holds sRGB values).
			float r = rays(c0, m0.obj ? m0.depth : 1e6) * ray_strength * (1.0 - pt_night) * smoothstep(0.5, 1.0, pt_sun_light); // rays fade with the sun through sunset / blue hour (PixelDayNight)
			// UE's filmic tonemapper rolls the HDR sum off towards white; this LDR buffer
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
