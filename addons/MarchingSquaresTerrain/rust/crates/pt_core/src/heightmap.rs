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
pub fn fill_sea_floor(
    src: &[Option<f32>],
    w: usize,
    h: usize,
    texel: f32,
    falloff: f32,
    fallback: f32,
) -> Vec<f32> {
    SeaFloor::new(src, w, h, texel, falloff).flood(src, fallback)
}

/// The height-independent part of `fill_sea_floor`: for every texel its nearest terrain texel
/// and how far below it the sea floor lies. It only depends on which texels hold terrain, so
/// sculpting (heights change, the land mask does not) re-fills the heightmap with one pass
/// instead of the distance transform and a noise lookup per sea texel.
#[derive(Clone, Debug, Default)]
pub struct SeaFloor {
    w: usize,
    h: usize,
    /// Texel holds terrain.
    land: Vec<bool>,
    /// Nearest terrain texel (`u32::MAX`: no terrain at all).
    near: Vec<u32>,
    /// Depth below the nearest terrain height (0 on terrain).
    drop: Vec<f32>,
    texel: f32,
    falloff: f32,
}

impl SeaFloor {
    pub fn new(src: &[Option<f32>], w: usize, h: usize, texel: f32, falloff: f32) -> SeaFloor {
        const INF: f32 = f32::MAX;
        const NONE: u32 = u32::MAX;
        let n = w * h;
        let mut dist = vec![INF; n];
        let mut near = vec![NONE; n];
        let mut land = vec![false; n];
        for i in 0..n.min(src.len()) {
            if src[i].is_some() {
                dist[i] = 0.0;
                near[i] = i as u32;
                land[i] = true;
            }
        }
        // Two-pass 8-neighbour chamfer distance, carrying the nearest terrain texel.
        let d1 = 1.0;
        let d2 = std::f32::consts::SQRT_2;
        let fwd: [(isize, isize, f32); 4] = [(-1, 0, d1), (-1, -1, d2), (0, -1, d1), (1, -1, d2)];
        let bwd: [(isize, isize, f32); 4] = [(1, 0, d1), (1, 1, d2), (0, 1, d1), (-1, 1, d2)];
        let relax = |x: usize,
                     y: usize,
                     offs: &[(isize, isize, f32); 4],
                     dist: &mut [f32],
                     near: &mut [u32]| {
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
        let mut drop = vec![0.0; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if dist[i] > 0.0 && dist[i] < INF {
                    let d = dist[i] * texel;
                    let wobble =
                        1.0 + 0.4505 * fbm(v2(x as f32, y as f32) * (texel / 9.0), 0x5ea, 3);
                    drop[i] = d * falloff * wobble.max(0.2005);
                }
            }
        }
        SeaFloor {
            w,
            h,
            land,
            near,
            drop,
            texel,
            falloff,
        }
    }

    /// Still valid for `src` (same layout and land mask)?
    pub fn fits(&self, src: &[Option<f32>], w: usize, h: usize, texel: f32, falloff: f32) -> bool {
        self.w == w
            && self.h == h
            && self.texel == texel
            && self.falloff == falloff
            && src.len() == self.land.len()
            && src.iter().zip(&self.land).all(|(v, l)| v.is_some() == *l)
    }

    /// The filled heightmap of `src` (which must match, see `matches`).
    pub fn flood(&self, src: &[Option<f32>], fallback: f32) -> Vec<f32> {
        (0..self.w * self.h)
            .map(|i| match self.near[i] {
                u32::MAX => fallback,
                j => src.get(j as usize).copied().flatten().unwrap_or(fallback) - self.drop[i],
            })
            .collect()
    }
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
    fn cached_sea_floor_follows_height_edits() {
        let (w, h) = (24, 10);
        let mut src = vec![None; w * h];
        for y in 2..8 {
            for x in 3..9 {
                src[y * w + x] = Some(x as f32 * 0.1 - y as f32 * 0.2);
            }
        }
        let sea = SeaFloor::new(&src, w, h, 0.5, 0.6);
        src[4 * w + 8] = Some(3.0);
        src[2 * w + 3] = Some(-2.0);
        assert!(sea.fits(&src, w, h, 0.5, 0.6));
        assert_eq!(
            sea.flood(&src, -1.0e4),
            fill_sea_floor(&src, w, h, 0.5, 0.6, -1.0e4)
        );
        src[0] = Some(0.0);
        assert!(!sea.fits(&src, w, h, 0.5, 0.6), "land mask changed");
    }

    #[test]
    fn empty_grid_uses_fallback() {
        let out = fill_sea_floor(&[None; 4], 2, 2, 1.0, 0.5, -7.0);
        assert!(out.iter().all(|v| *v == -7.0));
    }
}
