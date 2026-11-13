//! Lightmap UV2 bookkeeping. Port of Unibuilder's UV2 staleness logic
//! (`nodes/ub_mesh_instance.gd`: `_uv2_is_current`, `UV2_SCALE_EPS`, `make_baked`'s texel
//! handling, MIT).
//!
//! The unwrap itself is Godot's `ArrayMesh.lightmap_unwrap(transform, texel_size)` (xatlas), as
//! in Unibuilder; it runs on bake / export in `mst::builder`. A mesh unwrapped at one texel size
//! and node scale is stamped with both, so it is only unwrapped again when they change.

use crate::math::Vec3;

/// Relative tolerance of the stamped node scale.
pub const UV2_SCALE_EPS: f32 = 1e-3;
/// Default lightmap texel size in world units (Unibuilder's `lightmap_texel` default).
pub const DEFAULT_TEXEL: f32 = 0.2;

/// What an unwrap request did (Unibuilder `UV2Result`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uv2Result {
    /// Texel size 0 (off) or nothing to unwrap.
    Skipped,
    /// The mesh already carries a UV2 for this texel and scale.
    Current,
    Rebuilt,
    Failed,
}

/// Stamp recorded on an unwrapped mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uv2Stamp {
    pub texel: f32,
    pub scale: Vec3,
}

fn approx(a: f32, b: f32) -> bool {
    // Godot `is_equal_approx`.
    a == b || (a - b).abs() < (1e-5 * a.abs()).max(1e-5)
}

/// True when a mesh with UV2 `stamp` is still valid for `texel` at node scale `scale`.
pub fn uv2_is_current(stamp: Option<Uv2Stamp>, has_uv2: bool, texel: f32, scale: Vec3) -> bool {
    let Some(s) = stamp.filter(|_| has_uv2) else { return false };
    if !approx(s.texel, texel) {
        return false;
    }
    [(scale.x, s.scale.x), (scale.y, s.scale.y), (scale.z, s.scale.z)].iter().all(|(now, was)| (now - was).abs() <= UV2_SCALE_EPS * now.abs().max(1.0))
}

/// What an unwrap request at `texel` should do (`Rebuilt` = run the unwrap).
pub fn plan_unwrap(texel: f32, surfaces: usize, current: bool, force: bool) -> Uv2Result {
    if texel <= 0.0 || surfaces == 0 {
        Uv2Result::Skipped
    } else if current && !force {
        Uv2Result::Current
    } else {
        Uv2Result::Rebuilt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::v3;

    #[test]
    fn stamp_must_match_texel_and_scale() {
        let s = Some(Uv2Stamp { texel: 0.2, scale: v3(1.0, 2.0, 1.0) });
        assert!(uv2_is_current(s, true, 0.2, v3(1.0, 2.0, 1.0)));
        assert!(uv2_is_current(s, true, 0.2, v3(1.0005, 2.0, 1.0)), "within the scale tolerance");
        assert!(!uv2_is_current(s, true, 0.25, v3(1.0, 2.0, 1.0)));
        assert!(!uv2_is_current(s, true, 0.2, v3(1.0, 2.5, 1.0)));
        assert!(!uv2_is_current(s, false, 0.2, v3(1.0, 2.0, 1.0)), "no UV2 array");
        assert!(!uv2_is_current(None, true, 0.2, v3(1.0, 2.0, 1.0)));
    }

    #[test]
    fn plan() {
        assert_eq!(plan_unwrap(0.0, 3, false, true), Uv2Result::Skipped);
        assert_eq!(plan_unwrap(0.2, 0, false, true), Uv2Result::Skipped);
        assert_eq!(plan_unwrap(0.2, 2, true, false), Uv2Result::Current);
        assert_eq!(plan_unwrap(0.2, 2, true, true), Uv2Result::Rebuilt);
        assert_eq!(plan_unwrap(0.2, 2, false, false), Uv2Result::Rebuilt);
    }
}
