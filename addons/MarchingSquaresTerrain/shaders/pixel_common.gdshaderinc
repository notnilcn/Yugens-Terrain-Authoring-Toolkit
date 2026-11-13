// Marching Squares Terrain - shared spatial-shader helpers.
//
// Include this in any spatial shader that should take part in the pixel pipeline:
//
//   #include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
//   ...
//   void fragment() {
//       ... your ALBEDO etc ...
//       PT_META_OUTPUT(NORMAL);   // last line: writes the metadata pass
//   }
//
// The PixelRenderer draws the scene twice at low resolution: once normally (colour) and once
// through a camera that only sees the metadata layer. On that camera the macro replaces the
// output with packed (pixel size, outline id, normal, depth) used for outlines and per-object
// pixel sizes. The global uniforms below are registered by the extension at startup, so no
// project settings are required.

#include "res://addons/MarchingSquaresTerrain/shaders/pixel_noise.gdshaderinc"

// Per-object flags (set by PixelObject / MarchingSquaresTerrain through instance shader parameters),
// layout in pt_core::meta::object_flags: bits 0-1 pixel size multiplier - 1, bits 2-4 outline
// palette id & 7, bit 5 outline enabled, bit 6 snap object origin to the texel grid,
// bits 8-15 full outline id (0-255), bit 17 palette off, bit 18 dither off.
// Shaders drawn by many instances (terrain chunks, grass) `#define PT_FLAGS_MATERIAL` before the
// include and get the flags as a material uniform: every instance uniform user reserves 16 slots of
// the 4096-entry instance buffer, which hundreds of hex chunks would exhaust.
#ifdef PT_FLAGS_MATERIAL
uniform int pt_flags = 32;
#else
instance uniform int pt_flags = 32;
#endif

// Cull mask of the id camera (meta layer + bit 23, pt_core::meta::ID_CAMERA_BIT).
global uniform uint pt_id_mask;

// The metadata camera sees *only* the meta layer; editor and game cameras see more, so they
// always get normal shading even though pixelized meshes are on the meta layer too.
#define PT_IS_META (CAMERA_VISIBLE_LAYERS == pt_meta_mask)
// The optional id pass (wide outline ids, per-object palette/dither) uses its own exact mask.
#define PT_IS_ID (CAMERA_VISIBLE_LAYERS == pt_id_mask)

// Packs the metadata for the current fragment (see pt_core::meta for the contract).
vec3 pt_meta_color(vec3 view_normal, float view_depth, int flags) {
	vec3 n = normalize(view_normal);
	n.z = max(n.z, 0.0);
	vec2 o = n.xy / (abs(n.x) + abs(n.y) + n.z + 1e-6);
	vec2 h = vec2(o.x + o.y, o.x - o.y);          // hemi-octahedral -> [-1, 1]^2
	uvec2 q = uvec2(clamp(floor((h * 0.5 + 0.5) * 9.0), 0.0, 8.0));
	uint normal_id = q.y * 9u + q.x;
	uint depth_id = uint(clamp(view_depth / max(pt_meta_depth_range, 0.001), 0.0, 1.0) * 1364.0 + 0.5);
	uint f = uint(flags & 63);
	uint v = (f * 81u + normal_id) * 1365u + depth_id;
	uint d0 = v / 36864u;
	uint d1 = (v / 192u) % 192u;
	uint d2 = v % 192u;
	return vec3(float(d0 + 64u), float(d1 + 64u), float(d2 + 64u)) / 255.0;
}

// Packs the id-pass payload (pt_core::meta::pack_id): N = aux * 256 + outline id, aux bit 0 =
// palette off, bit 1 = dither off; same base-192 digits offset by 64 as the meta pass.
vec3 pt_id_color(int flags) {
	uint hi = uint((flags >> 8) & 255);
	uint id = hi != 0u ? hi : uint((flags >> 2) & 7);
	uint aux = uint((flags >> 17) & 3);
	uint v = aux * 256u + id;
	uint d0 = v / 36864u;
	uint d1 = (v / 192u) % 192u;
	uint d2 = v % 192u;
	return vec3(float(d0 + 64u), float(d1 + 64u), float(d2 + 64u)) / 255.0;
}

// Replaces the fragment output with metadata when drawn by the metadata (or id) camera. Lights
// do not reach those cameras' output (ALBEDO = 0, specular disabled) and their environment has
// no ambient/fog, so EMISSION passes through.
#define PT_META_OUTPUT(view_normal) if (PT_IS_META) { ALBEDO = vec3(0.0); EMISSION = pt_meta_color(view_normal, -VERTEX.z, pt_flags); } else if (PT_IS_ID) { ALBEDO = vec3(0.0); EMISSION = pt_id_color(pt_flags); }

// Snaps the object's origin to the low-res texel lattice in view space (removes sub-texel
// swimming on moving objects). Call from vertex(): PT_SNAP_VERTEX
#define PT_SNAP_VERTEX if ((pt_flags & 64) != 0 && pt_texel_world > 0.0) { vec3 pt_o = (VIEW_MATRIX * vec4(MODEL_MATRIX[3].xyz, 1.0)).xyz; vec2 pt_d = round(pt_o.xy / pt_texel_world) * pt_texel_world - pt_o.xy; vec3 pt_w = mat3(INV_VIEW_MATRIX) * vec3(pt_d, 0.0); VERTEX += (inverse(MODEL_MATRIX) * vec4(pt_w, 0.0)).xyz; }

// ---- Cel lighting (published by PixelDayNight; all off by default) -------------------------
// pt_light = (ramp enabled, light wrap [-1, 1], rim width [0, 1], rim back-light bias [0, 1]).
// pt_light_ramp: 1D ramp (x = lit amount 0..1 after wrap, shadows and cloud shadows) that
// replaces the even toon bands for directional lights; its colour tints each band.
// pt_rim = (rgb colour already scaled by strength and sun energy, enabled).
global uniform vec4 pt_light;
global uniform sampler2D pt_light_ramp;
global uniform vec4 pt_rim;

// Nearest texel of the light ramp (crisp bands whatever the texture filter).
vec3 pt_ramp(float x) {
	int w = textureSize(pt_light_ramp, 0).x;
	return texelFetch(pt_light_ramp, ivec2(clamp(int(x * float(w)), 0, w - 1), 0), 0).rgb;
}

// Hard-edged rim on silhouettes (normal at a grazing angle to the view), stronger when the
// sun is behind the surface (back-lit), scaled by how much of the sun reaches it.
vec3 pt_rim_light(vec3 n, vec3 v, vec3 l, float lit) {
	float fres = 1.0 - clamp(dot(n, v), 0.0, 1.0);
	float edge = step(1.0 - pt_light.z, fres);
	float back = clamp(0.5 - 0.5 * dot(v, l), 0.0, 1.0);
	return pt_rim.rgb * edge * mix(1.0, back, pt_light.w) * mix(0.4, 1.0, lit);
}

// Banded (toon) diffuse used by every pixel shader's light().
// shade_floor lifts the darkest band; cloud multiplies directional light. With pt_light.x
// set, directional lights read their bands (and tint) from pt_light_ramp instead; light wrap
// (pt_light.y) shifts the terminator; pt_rim adds a coloured rim light.
#define PT_TOON_LIGHT(bands, shade_floor, cloud) { float pt_wr = clamp(pt_light.y, -0.99, 1.0); float pt_ndl = clamp((dot(NORMAL, LIGHT) + pt_wr) / (1.0 + pt_wr), 0.0, 1.0) * ATTENUATION; if (LIGHT_IS_DIRECTIONAL) { pt_ndl *= cloud; } float pt_b = max(float(bands), 1.0); float pt_q = floor(pt_ndl * pt_b + 0.5) / pt_b; vec3 pt_v = vec3(LIGHT_IS_DIRECTIONAL ? mix(shade_floor, 1.0, pt_q) : pt_q); if (LIGHT_IS_DIRECTIONAL && pt_light.x > 0.5) { pt_v = pt_ramp(pt_ndl); } DIFFUSE_LIGHT += LIGHT_COLOR / PI * pt_v; if (LIGHT_IS_DIRECTIONAL && pt_rim.a > 0.5) { DIFFUSE_LIGHT += pt_rim_light(NORMAL, VIEW, LIGHT, pt_ndl); } }
