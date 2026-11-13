//! Published height texture helpers (`pt_terrain_height`, read by the water shader).

use crate::math::{fbm, v2};

/// Fills texels without terrain (`None`) with a sea floor that falls away from the nearest
/// terrain texel: `nearest_height - distance * falloff`, the distance (world units) wobbled by
/// noise so the drop-off contour does not trace the chunk outline. Without this the water saw
/// "no terrain" as instantly deep, and the shallow sea floor of the outermost chunks ended in a
/// hard straight-edged (rhombus-looking, under a 45° camera) band.
///
/// `w × h` row-major grid, `texel` = world size of one texel. Returns heights for every texel
/// (`fallback` when the grid holds no terrain at all).
pub fn fill_sea_floor(src: &[Option<f32>], w: usize, h: usize, texel: f32, falloff: f32, fallback: f32) -> Vec<f32> {
    const INF: f32 = f32::MAX;
    let n = w * h;
    let mut dist = vec![INF; n];
    let mut near = vec![fallback; n];
    for i in 0..n.min(src.len()) {
        if let Some(v) = src[i] {
            dist[i] = 0.0;
            near[i] = v;
        }
    }
    // Two-pass 8-neighbour chamfer distance, carrying the nearest terrain height.
    let d1 = 1.0;
    let d2 = std::f32::consts::SQRT_2;
    let fwd: [(isize, isize, f32); 4] = [(-1, 0, d1), (-1, -1, d2), (0, -1, d1), (1, -1, d2)];
    let bwd: [(isize, isize, f32); 4] = [(1, 0, d1), (1, 1, d2), (0, 1, d1), (-1, 1, d2)];
    let relax = |x: usize, y: usize, offs: &[(isize, isize, f32); 4], dist: &mut [f32], near: &mut [f32]| {
        let i = y * w + x;
        for &(dx, dy, c) in offs {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if dist[j] < INF && dist[j] + c < dist[i] {
                dist[i] = dist[j] + c;
                near[i] = near[j];
            }
        }
    };
    for y in 0..h {
        for x in 0..w {
            relax(x, y, &fwd, &mut dist, &mut near);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            relax(x, y, &bwd, &mut dist, &mut near);
        }
    }
    let mut out = Vec::with_capacity(n);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if dist[i] == 0.0 {
                out.push(near[i]);
            } else if dist[i] == INF {
                out.push(fallback);
            } else {
                let d = dist[i] * texel;
                let wobble = 1.0 + 0.45 * fbm(v2(x as f32, y as f32) * (texel / 9.0), 0x5ea, 3);
                out.push(near[i] - d * falloff * wobble.max(0.2));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_floor_falls_away_from_terrain() {
        let (w, h) = (32, 8);
        let mut src = vec![None; w * h];
        for y in 0..h {
            for x in 0..8 {
                src[y * w + x] = Some(-0.5);
            }
        }
        let out = fill_sea_floor(&src, w, h, 1.0, 0.5, -100.0);
        assert_eq!(out[3 * w + 4], -0.5, "terrain kept");
        // Continuous next to the terrain, much deeper further out.
        assert!(out[3 * w + 8] > -1.5);
        assert!(out[3 * w + 31] < -4.0);
    }

    #[test]
    fn empty_grid_uses_fallback() {
        let out = fill_sea_floor(&[None; 4], 2, 2, 1.0, 0.5, -7.0);
        assert!(out.iter().all(|v| *v == -7.0));
    }
}
