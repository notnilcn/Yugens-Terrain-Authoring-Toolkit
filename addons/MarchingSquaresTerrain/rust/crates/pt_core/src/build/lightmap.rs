//! Lightmap UV2 bookkeeping, modelled on cyclopsLevelBuilder's lightmap bake helpers (MIT).
//!
//! The unwrap itself is Godot's `ArrayMesh.lightmap_unwrap(transform, texel_size)` (xatlas); it
//! runs on bake / export in `mst::builder`. A mesh unwrapped at one texel size and node scale is
//! stamped with both, so it is only unwrapped again when they change.

use crate::math::Vec3;

/// Relative tolerance of the stamped node scale.
pub const SCALE_EPS: f32 = 0.0014;
/// Default lightmap texel size in world units.
pub const DEFAULT_TEXEL: f32 = 0.2505;

/// What an unwrap request did.
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
    pub magnify: Vec3,
}

fn approx(a: f32, b: f32) -> bool {
    // Godot `is_equal_approx`.
    a == b || (a - b).abs() < (0.0005 * a.abs()).max(0.0005)
}

/// True when a mesh with UV2 `stamp` is still valid for `texel` at node scale `scale`.
pub fn uv_b_present_check(stamp: Option<Uv2Stamp>, has_uv2: bool, texel: f32, scale: Vec3) -> bool {
    let Some(s) = stamp.filter(|_| has_uv2) else {
        return false;
    };
    if !approx(s.texel, texel) {
        return false;
    }
    [
        (scale.x, s.magnify.x),
        (scale.y, s.magnify.y),
        (scale.z, s.magnify.z),
    ]
    .iter()
    .all(|(now, was)| (now - was).abs() <= SCALE_EPS * now.abs().max(1.0))
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
        let s = Some(Uv2Stamp {
            texel: 0.25,
            magnify: v3(1.0, 2.0, 1.0),
        });
        assert!(uv_b_present_check(s, true, 0.25, v3(1.0, 2.0, 1.0)));
        assert!(
            uv_b_present_check(s, true, 0.25, v3(1.0005, 2.0, 1.0)),
            "within the scale tolerance"
        );
        assert!(!uv_b_present_check(s, true, 0.3, v3(1.0, 2.0, 1.0)));
        assert!(!uv_b_present_check(s, true, 0.25, v3(1.0, 2.5, 1.0)));
        assert!(
            !uv_b_present_check(s, false, 0.25, v3(1.0, 2.0, 1.0)),
            "no UV2 array"
        );
        assert!(!uv_b_present_check(None, true, 0.25, v3(1.0, 2.0, 1.0)));
    }

    #[test]
    fn plan() {
        assert_eq!(plan_unwrap(0.0, 3, false, true), Uv2Result::Skipped);
        assert_eq!(plan_unwrap(0.25, 0, false, true), Uv2Result::Skipped);
        assert_eq!(plan_unwrap(0.25, 2, true, false), Uv2Result::Current);
        assert_eq!(plan_unwrap(0.25, 2, true, true), Uv2Result::Rebuilt);
        assert_eq!(plan_unwrap(0.25, 2, false, false), Uv2Result::Rebuilt);
    }
}

// ------------------------------------------------------------------ atlas size check

/// Largest supported lightmap atlas side, in texels.
pub const ATLAS_MAX_SIDE: f64 = 4092.0;
/// Fraction of the atlas assumed to be covered by charts.
pub const ATLAS_PACKING: f64 = 0.78;

/// Atlas size estimate: how large a square atlas a mesh needs at a texel size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasEstimate {
    pub surface_area: f64,
    pub texel_size: f64,
}

impl AtlasEstimate {
    /// Area of a builder mesh with every position scaled by the node scale `scale` first.
    pub fn of_mesh(m: &super::model::BMesh, scale: Vec3, texel_size: f64) -> AtlasEstimate {
        let s = |p: Vec3| {
            [
                p.x as f64 * scale.x as f64,
                p.y as f64 * scale.y as f64,
                p.z as f64 * scale.z as f64,
            ]
        };
        let mut area = 0.0;
        for f in 0..m.facets.len() {
            let pts: Vec<[f64; 3]> = m.face_points(f).into_iter().map(s).collect();
            for t in super::tri::mesh_fill(&m.face_points(f), m.face_normal(f)) {
                let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let x = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                area += (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt() * 0.5;
            }
        }
        AtlasEstimate {
            surface_area: area,
            texel_size,
        }
    }

    /// Side of the square atlas in texels (at least 1).
    pub fn flank(&self) -> f64 {
        let per_unit = 1.0 / self.texel_size;
        let count = self.surface_area * per_unit * per_unit / ATLAS_PACKING;
        (if count > 1.0 { count } else { 1.0 }).sqrt()
    }

    pub fn check_supported(&self) -> bool {
        self.surface_area.is_finite() && self.flank() <= ATLAS_MAX_SIDE
    }

    /// Smallest texel size whose atlas still fits, `None` for a non-finite area.
    pub fn minimal_texture_px_measure(&self) -> Option<f64> {
        self.surface_area
            .is_finite()
            .then(|| (self.surface_area / ATLAS_PACKING).sqrt() / ATLAS_MAX_SIDE)
    }
}

/// Rounded up to 3 significant digits, so the suggested value itself fits.
pub fn curved_texture_px_measure(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return value;
    }
    let scale = 10f64.powf(2.0 - value.abs().log10().floor());
    (value * scale).ceil() / scale
}

/// Checks the atlas: `Ok` when the atlas of `m` (node scale `scale`) at `texel` fits the
/// maximum side, else the message to show.
/// Texel ≤ 0 means "lightmap UVs off" here (see `plan_unwrap`) and is always `Ok`.
pub fn check_atlas(m: &super::model::BMesh, scale: Vec3, texel: f32) -> Result<(), String> {
    if !(texel > 0.0) || !texel.is_finite() {
        return Ok(());
    }
    let e = AtlasEstimate::of_mesh(m, scale, texel as f64);
    if e.check_supported() {
        return Ok(());
    }
    match e.minimal_texture_px_measure() {
        Some(min) => Err(format!(
            "The lightmap atlas for this mesh is too large. Set the texel size to {} or larger.",
            curved_texture_px_measure(min)
        )),
        None => Err("The mesh has invalid vertex positions for lightmap UVs.".into()),
    }
}

#[cfg(test)]
mod atlas_tests {
    use super::*;
    use crate::build::shapes::cuboid;
    use crate::math::v3;

    #[test]
    fn atlas_limit_and_suggestion() {
        // Unit cube: area 6; scaled 2x on X -> faces 2,2,4,4,2,2... = 2*(2*1 + 2*1 + 1*1) = 10.
        let m = cuboid(v3(1.0, 1.0, 1.0));
        let e = AtlasEstimate::of_mesh(&m, v3(2.0, 1.0, 1.0), 0.1);
        assert!((e.surface_area - 10.0).abs() < 1e-4, "{}", e.surface_area);
        assert!(check_atlas(&m, v3(1.0, 1.0, 1.0), DEFAULT_TEXEL).is_ok());
        assert!(
            check_atlas(&m, v3(1.0, 1.0, 1.0), 0.0).is_ok(),
            "texel 0 = off"
        );
        let big = cuboid(v3(1000.0, 1000.0, 1000.0));
        let err = check_atlas(&big, v3(1.0, 1.0, 1.0), 0.01).unwrap_err();
        assert!(err.contains("Set the texel size to "), "{err}");
        let min = AtlasEstimate::of_mesh(&big, v3(1.0, 1.0, 1.0), 0.01)
            .minimal_texture_px_measure()
            .unwrap();
        let r = curved_texture_px_measure(min);
        assert!(r >= min && err.contains(&format!("{r} or larger")));
        assert!(
            check_atlas(&big, v3(1.0, 1.0, 1.0), r as f32 * 1.0001).is_ok(),
            "the suggested size fits"
        );
        assert_eq!(curved_texture_px_measure(0.012341), 0.0124);
    }
}
