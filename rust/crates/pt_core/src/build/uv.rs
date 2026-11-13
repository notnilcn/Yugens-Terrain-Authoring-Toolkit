//! Automatic planar UVs: every face is projected along its dominant axis so that textures stay
//! upright on walls and tile at a fixed world density (`units_per_repeat`). With a 16 px
//! texture and one repeat per metre this matches a 16 texels-per-metre pixel-art density, and
//! world-space projection keeps texels aligned across neighbouring objects.

use super::model::UvParams;
use crate::math::*;

/// Dominant-axis projection frame `(u_axis, v_axis)` for a face normal. `u` runs to the
/// viewer's right and `v` downwards (Godot UV convention) when looking at the face.
pub fn projection_frame(n: Vec3) -> (Vec3, Vec3) {
    let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
    if ay >= ax && ay >= az {
        // Floors / ceilings: u = +x, v = +z (top-down view with -z "up" on screen).
        (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, if n.y >= 0.0 { 1.0 } else { -1.0 }))
    } else if ax >= az {
        (v3(0.0, 0.0, -n.x.signum()), v3(0.0, -1.0, 0.0))
    } else {
        (v3(n.z.signum(), 0.0, 0.0), v3(0.0, -1.0, 0.0))
    }
}

/// UV of point `p` (already in the projection space: world or local) on a face with normal `n`.
pub fn auto_uv(p: Vec3, n: Vec3, params: &UvParams, units_per_repeat: f32) -> Vec2 {
    let (u, v) = projection_frame(n);
    let k = 1.0 / units_per_repeat.max(1e-6);
    let raw = v2(p.dot(u) * k, p.dot(v) * k);
    let r = raw.rotated(params.rotation.to_radians());
    let s = v2(if params.scale.x.abs() < 1e-6 { 1.0 } else { params.scale.x }, if params.scale.y.abs() < 1e-6 { 1.0 } else { params.scale.y });
    v2(r.x / s.x, r.y / s.y) + params.offset
}

/// Rounds a UV offset to whole texels of a `texture_px`-wide texture (avoids half-texel seams).
pub fn snap_offset(o: Vec2, texture_px: f32) -> Vec2 {
    let t = texture_px.max(1.0);
    v2((o.x * t).round() / t, (o.y * t).round() / t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_is_one_repeat_per_unit() {
        let p = UvParams::default();
        let a = auto_uv(v3(0.0, 0.0, 0.0), Vec3::UP, &p, 1.0);
        let b = auto_uv(v3(1.0, 0.0, 0.0), Vec3::UP, &p, 1.0);
        assert!(((b - a).length() - 1.0).abs() < 1e-5);
        let c = auto_uv(v3(0.0, 2.0, 0.0), v3(0.0, 0.0, 1.0), &p, 2.0);
        assert!((c.y + 1.0).abs() < 1e-5, "walls: v goes down as y goes up");
    }

    #[test]
    fn wall_u_runs_right() {
        // Looking at a +Z face from +Z, right is +X; at a +X face, right is -Z.
        let p = UvParams::default();
        assert!(auto_uv(v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), &p, 1.0).x > 0.0);
        assert!(auto_uv(v3(0.0, 0.0, -1.0), v3(1.0, 0.0, 0.0), &p, 1.0).x > 0.0);
    }

    #[test]
    fn offset_snaps_to_texels() {
        let o = snap_offset(v2(0.03, 0.5), 16.0);
        assert!((o.x - 0.0625).abs() < 1e-6 || (o.x - 0.0).abs() < 1e-6);
        assert!((o.y - 0.5).abs() < 1e-6);
    }
}
