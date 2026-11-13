// Marching Squares Terrain - PixelBirds (MultiMesh of tiny body + wing meshes).
// INSTANCE_CUSTOM = (flap phase, perched, gliding, 0). UV.x = wing weight (0 root / body,
// 0.5 elbow, 1 tip; tips take tip_color), UV.y = extra tip-colour weight.
//
// Flying birds are drawn like a pixel-art sprite: the mesh is laid out in the camera plane
// (wingspan along the screen horizontal, the body's length foreshortened vertically), so every
// bird reads as the classic "-v-" / "V" silhouette whatever its heading or the camera pitch.
// Two flap frames: wings raised into a "V" / spread level with drooped tips; glides hold the
// wings flat. Perched birds fold their wings into a small upright body.
// The sprite is never smaller than `min_texels` nor larger than `max_texels` (wingspan in
// low-res texels; PixelBirds passes the renderer's texel size).
// Takes part in the pixel pipeline: texel-snapped origin (PT_SNAP_VERTEX) and metadata output.
shader_type spatial;
render_mode specular_disabled, cull_disabled;

#define PT_FLAGS_MATERIAL
#include "res://addons/MarchingSquaresTerrain/shaders/pixel_common.gdshaderinc"
#include "res://addons/MarchingSquaresTerrain/shaders/mst_cells.gdshaderinc"

uniform vec4 color : source_color = vec4(0.9, 0.88, 0.82, 1.0);
/// Wing tip / tail colour.
uniform vec4 tip_color : source_color = vec4(0.36, 0.34, 0.4, 1.0);
/// Wing beats per second.
uniform float flap_speed = 4.0;
/// Smallest / largest on-screen wingspan in low-res texels (0 = no limit).
uniform float min_texels = 9.0;
uniform float max_texels = 18.0;
/// Low-res texel size in world units at distance d from the camera = x + y * d (set by
/// PixelBirds from the PixelRenderer each frame; 0 = no renderer, true size).
uniform vec2 texel_size = vec2(0.0);
/// Wingspan (m) of the instances (PixelBirds.size).
uniform float span = 0.8;

varying vec3 world_pos;
varying float cloud;
varying float tip;

void vertex() {
	float w = UV.x;
	tip = max(w, UV.y);
	vec3 v = VERTEX;
	if (INSTANCE_CUSTOM.y > 0.5) {
		// Perched: wings folded against the body.
		if (w > 0.01) {
			v.x = sign(v.x) * mix(0.08, 0.05, w);
			v.z = v.z * 0.4 - 0.04 - w * 0.12;
		}
	} else if (INSTANCE_CUSTOM.z > 0.5) {
		// Glide: flat wings, slight dihedral.
		v.y += w * 0.06;
	} else {
		// Two frames: raised "V" (narrower) / spread with drooped tips.
		float up = step(0.5, fract(pt_time * flap_speed + INSTANCE_CUSTOM.x));
		v.y += mix(-0.16 * w * w, 0.34 * w, up);
		v.x *= mix(1.0, 0.78, up * w);
	}
	vec3 cam_r = normalize(INV_VIEW_MATRIX[0].xyz);
	vec3 cam_u = normalize(INV_VIEW_MATRIX[1].xyz);
	vec3 cam_b = normalize(INV_VIEW_MATRIX[2].xyz);
	// Sprite plane: flying = front view (x along the camera's right, height = y plus the
	// foreshortened body length); perched = side profile facing the heading's screen side.
	vec2 sp = vec2(v.x, v.y + v.z * (w > 0.01 ? 0.6 : 0.45));
	if (INSTANCE_CUSTOM.y > 0.5) {
		float face = dot(MODEL_MATRIX[2].xyz, cam_r) < 0.0 ? -1.0 : 1.0;
		sp = vec2(v.z * face * 1.1, v.y * 2.6 + 0.12 + (w > 0.01 ? 0.05 : 0.0));
	}
	float k = 1.0;
	if (texel_size.x + texel_size.y > 0.0) {
		float dist = length(MODEL_MATRIX[3].xyz - CAMERA_POSITION_WORLD);
		float px = span / max(texel_size.x + texel_size.y * dist, 1e-5);
		if (min_texels > 0.0 && px < min_texels) {
			k = min_texels / max(px, 1e-3);
		}
		if (max_texels > 0.0 && px > max_texels) {
			k = max_texels / px;
		}
	}
	mat3 inv_model = inverse(mat3(MODEL_MATRIX));
	VERTEX = inv_model * ((cam_r * sp.x + cam_u * sp.y) * span * k);
	// Lit like a surface facing the camera and the sky.
	NORMAL = normalize(inv_model * normalize(cam_b + vec3(0.0, 0.8, 0.0)));
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	PT_SNAP_VERTEX
}

void fragment() {
	if (pt_fog_hides(world_pos, FRAGCOORD.xy)) {
		discard; // fog of war: no birds over cells the player cannot see
	}
	ALBEDO = mix(color.rgb, tip_color.rgb, smoothstep(0.6, 0.85, tip));
	cloud = pt_cloud_shadow(world_pos);
	PT_META_OUTPUT(NORMAL)
}

void light() {
	PT_TOON_LIGHT(2, 0.7, cloud)
}
