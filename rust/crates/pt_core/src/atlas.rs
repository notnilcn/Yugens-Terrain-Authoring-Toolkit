//! CPU texture-atlas baker — the original's `MarchingSquaresGeometryBaker` without its GPU pass.
//!
//! Every triangle gets half of a `res × res` atlas cell (two triangles per cell, like the
//! original), the triangle's UVs are inset by 2 texels to avoid bleeding, and each texel is
//! shaded by a caller closure from the triangle's barycentric coordinates (texels just outside
//! the triangle are extrapolated, which pads the edges). Used to export chunks as textured GLB
//! meshes that do not need the terrain shader.

use crate::math::*;

pub struct Atlas {
    /// Square atlas edge in pixels (power of two).
    pub size: u32,
    /// RGBA8, row-major.
    pub rgba: Vec<u8>,
    /// Three UVs per input triangle (0..1 atlas space).
    pub uvs: Vec<Vec2>,
}

/// `(atlas size, cells per row)` for `tris` triangles at `res` pixels per cell, or `None`
/// when it would exceed `max_size`.
pub fn layout(tris: usize, res: u32, max_size: u32) -> Option<(u32, u32)> {
    let res = res.max(4);
    let mut size = res.next_power_of_two();
    loop {
        let per_row = size / res;
        if (per_row as usize) * (per_row as usize) * 2 >= tris.max(1) {
            return Some((size, per_row));
        }
        size *= 2;
        if size > max_size {
            return None;
        }
    }
}

/// Bakes `tris` triangles. `shade(tri, [w0, w1, w2])` returns linear 0..1 RGBA.
pub fn bake(tris: usize, res: u32, max_size: u32, mut shade: impl FnMut(usize, [f32; 3]) -> [f32; 4]) -> Option<Atlas> {
    let res = res.max(4);
    let (size, per_row) = layout(tris, res, max_size)?;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let mut uvs = Vec::with_capacity(tris * 3);
    let inset = (res as f32 - 4.0) / res as f32;
    for t in 0..tris {
        let cell = t / 2;
        let (col, row) = ((cell as u32 % per_row) as f32, (cell as u32 / per_row) as f32);
        let even = t % 2 == 0;
        // Cell-local corners of the triangle (0..1).
        let corners = if even { [v2(0.0, 0.0), v2(1.0, 0.0), v2(0.0, 1.0)] } else { [v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)] };
        let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
        for c in corners {
            let p = centre + (c - centre) * inset;
            uvs.push(v2((col + p.x) / per_row as f32, (row + p.y) / per_row as f32));
        }
        let (x0, y0) = (col as u32 * res, row as u32 * res);
        for py in 0..res {
            for px in 0..res {
                let u = (px as f32 + 0.5) / res as f32;
                let v = (py as f32 + 0.5) / res as f32;
                if (u + v < 1.0) != even {
                    continue;
                }
                let w = if even {
                    [1.0 - u - v, u, v]
                } else {
                    let tt = 1.0 - u;
                    let s = v - tt;
                    [1.0 - s - tt, s, tt]
                };
                let c = shade(t, w);
                let i = (((y0 + py) * size + x0 + px) * 4) as usize;
                for k in 0..4 {
                    rgba[i + k] = (c[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
    }
    Some(Atlas { size, rgba, uvs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_grows_to_fit() {
        assert_eq!(layout(2, 16, 4096), Some((16, 1)));
        assert_eq!(layout(3, 16, 4096), Some((32, 2)));
        assert_eq!(layout(9000, 16, 4096), Some((2048, 128)));
        assert_eq!(layout(1_000_000, 32, 4096), None);
    }

    #[test]
    fn texels_get_their_triangle_and_uvs_stay_inside_cells() {
        // Triangle 0 red, triangle 1 green; barycentrics must sum to 1.
        let a = bake(2, 8, 64, |t, w| {
            assert!((w[0] + w[1] + w[2] - 1.0).abs() < 1e-5);
            if t == 0 { [1.0, 0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0, 1.0] }
        })
        .unwrap();
        assert_eq!(a.size, 8);
        let px = |x: u32, y: u32| &a.rgba[((y * 8 + x) * 4) as usize..((y * 8 + x) * 4 + 4) as usize];
        assert_eq!(px(1, 1), &[255, 0, 0, 255]);
        assert_eq!(px(6, 6), &[0, 255, 0, 255]);
        assert_eq!(a.uvs.len(), 6);
        assert!(a.uvs.iter().all(|u| u.x > 0.0 && u.x < 1.0 && u.y > 0.0 && u.y < 1.0));
    }

    #[test]
    fn barycentrics_reproduce_corners() {
        // Shade with the barycentric weight of corner 1: highest near that corner.
        let a = bake(2, 16, 64, |_, w| [w[1], 0.0, 0.0, 1.0]).unwrap();
        let r = |x: u32, y: u32| a.rgba[((y * 16 + x) * 4) as usize];
        assert!(r(14, 0) > 200, "even triangle corner 1 = (1, 0)");
        assert!(r(0, 0) < 30);
        assert!(r(15, 15) > 200, "odd triangle corner 1 = (1, 1)");
    }
}
