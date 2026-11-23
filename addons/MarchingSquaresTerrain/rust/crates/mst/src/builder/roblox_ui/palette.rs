//! The honeycomb colour palette as plain values: where each swatch sits, what colour it
//! holds, which swatch a point lands on, and the pixels of the whole palette, so the popup
//! can show it as one picture and stay free of per-swatch controls.
const SQRT3: f32 = 1.732_050_8;
pub const RINGS: i32 = 6;
pub const GREYS: usize = 10;
const FILL: f32 = 0.88;
const MARK_INNER: f32 = 0.7;
const RIM: [u8; 4] = [27, 27, 27, 255];
const MARK: [u8; 4] = [255, 204, 51, 255];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    pub centre: (f32, f32),
    pub rgb: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Honeycomb {
    pub radius: f32,
    pub width: usize,
    pub height: usize,
    pub swatches: Vec<Swatch>,
    hexes: usize,
    grid: Vec<Option<usize>>,
    origin: (f32, f32),
    grey_y: f32,
    grey_x0: f32,
    grey_step: f32,
}
pub fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
fn ring_of(q: i32, r: i32) -> i32 {
    q.abs().max(r.abs()).max((q + r).abs())
}
pub fn hex_colour(x: f32, y: f32, ring: i32) -> [f32; 3] {
    if ring == 0 {
        return [1.0, 1.0, 1.0];
    }
    let t = ring as f32 / RINGS as f32;
    let hue = y.atan2(x) / std::f32::consts::TAU;
    hsv(hue, t.powf(0.85), 1.0 - 0.45 * t * t)
}
pub fn grey(i: usize) -> [f32; 3] {
    let t = i as f32 / (GREYS - 1) as f32;
    let g = 0.07 + 0.93 * t;
    [g, g, g]
}
fn inside(dx: f32, dy: f32, s: f32) -> bool {
    let (dx, dy) = (dx.abs(), dy.abs());
    dx <= SQRT3 * 0.5 * s && dy <= s - dx / SQRT3
}
impl Honeycomb {
    pub fn new(radius: f32) -> Honeycomb {
        let s = radius.max(2.0);
        let half_w = SQRT3 * s * (RINGS as f32 + 0.5);
        let half_h = 1.5 * s * RINGS as f32 + s;
        let origin = (half_w + 1.0, half_h + 1.0);
        let mut swatches = Vec::new();
        let mut grid = Vec::new();
        for r in -RINGS..=RINGS {
            for q in -RINGS..=RINGS {
                let ring = ring_of(q, r);
                if ring > RINGS {
                    grid.push(None);
                    continue;
                }
                grid.push(Some(swatches.len()));
                let x = SQRT3 * s * (q as f32 + r as f32 * 0.5);
                let y = 1.5 * s * r as f32;
                swatches.push(Swatch {
                    centre: (origin.0 + x, origin.1 + y),
                    rgb: hex_colour(x, y, ring),
                });
            }
        }
        let hexes = swatches.len();
        let grey_step = SQRT3 * s * 1.15;
        let grey_x0 = origin.0 - grey_step * (GREYS as f32 - 1.0) * 0.5;
        let grey_y = origin.1 + half_h + 0.8 * s + s;
        for i in 0..GREYS {
            swatches.push(Swatch {
                centre: (grey_x0 + grey_step * i as f32, grey_y),
                rgb: grey(i),
            });
        }
        Honeycomb {
            radius: s,
            width: (2.0 * origin.0).ceil() as usize,
            height: (grey_y + s + 2.0).ceil() as usize,
            swatches,
            hexes,
            grid,
            origin,
            grey_y,
            grey_x0,
            grey_step,
        }
    }
    pub fn swatch_at(&self, x: f32, y: f32) -> Option<usize> {
        let s = self.radius;
        let (px, py) = (x - self.origin.0, y - self.origin.1);
        let fq = (SQRT3 / 3.0 * px - py / 3.0) / s;
        let fr = (2.0 / 3.0 * py) / s;
        let fs = -fq - fr;
        let (mut q, mut r, rs) = (fq.round(), fr.round(), fs.round());
        let (dq, dr, ds) = ((q - fq).abs(), (r - fr).abs(), (rs - fs).abs());
        if dq > dr && dq > ds {
            q = -r - rs;
        } else if dr > ds {
            r = -q - rs;
        }
        let (q, r) = (q as i32, r as i32);
        if ring_of(q, r) <= RINGS {
            let side = (2 * RINGS + 1) as usize;
            let i = self.grid[(r + RINGS) as usize * side + (q + RINGS) as usize]?;
            return inside(x - self.swatches[i].centre.0, y - self.swatches[i].centre.1, s)
                .then_some(i);
        }
        let g = ((x - self.grey_x0) / self.grey_step).round();
        if g < 0.0 || g >= GREYS as f32 {
            return None;
        }
        let i = self.hexes + g as usize;
        let c = self.swatches[i].centre;
        inside(x - c.0, y - c.1, s).then_some(i)
    }
    pub fn nearest_colour(&self, rgb: [f32; 3]) -> usize {
        let d = |w: &Swatch| {
            (0..3)
                .map(|k| (w.rgb[k] - rgb[k]) * (w.rgb[k] - rgb[k]))
                .sum::<f32>()
        };
        self.swatches
            .iter()
            .enumerate()
            .min_by(|a, b| d(a.1).total_cmp(&d(b.1)))
            .map(|x| x.0)
            .unwrap_or(0)
    }
    pub fn rasterize(&self, marked: Option<usize>) -> Vec<u8> {
        let mut px = vec![0u8; self.width * self.height * 4];
        let s = self.radius;
        for y in 0..self.height {
            for x in 0..self.width {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let Some(i) = self.swatch_at(fx, fy) else {
                    continue;
                };
                let c = self.swatches[i].centre;
                let (dx, dy) = (fx - c.0, fy - c.1);
                let colour = if marked == Some(i) && !inside(dx, dy, s * MARK_INNER) {
                    MARK
                } else if inside(dx, dy, s * FILL) {
                    let rgb = self.swatches[i].rgb;
                    [
                        (rgb[0] * 255.0).round() as u8,
                        (rgb[1] * 255.0).round() as u8,
                        (rgb[2] * 255.0).round() as u8,
                        255,
                    ]
                } else {
                    RIM
                };
                let o = (y * self.width + x) * 4;
                px[o..o + 4].copy_from_slice(&colour);
            }
        }
        px
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honeycomb_has_every_ring_and_the_greys() {
        let h = Honeycomb::new(12.0);
        let rings = RINGS as usize;
        assert_eq!(h.hexes, 1 + 3 * rings * (rings + 1));
        assert_eq!(h.swatches.len(), h.hexes + GREYS);
    }
    #[test]
    fn every_swatch_centre_hits_itself() {
        let h = Honeycomb::new(12.0);
        for (i, w) in h.swatches.iter().enumerate() {
            assert_eq!(h.swatch_at(w.centre.0, w.centre.1), Some(i), "swatch {i}");
        }
    }
    #[test]
    fn swatches_fit_inside_the_picture() {
        let h = Honeycomb::new(13.5);
        let s = h.radius;
        for w in &h.swatches {
            let half_w = SQRT3 * 0.5 * s;
            assert!(w.centre.0 - half_w >= 0.0 && w.centre.0 + half_w <= h.width as f32);
            assert!(w.centre.1 - s >= 0.0 && w.centre.1 + s <= h.height as f32);
        }
    }
    #[test]
    fn corners_and_gaps_hit_nothing() {
        let h = Honeycomb::new(12.0);
        assert_eq!(h.swatch_at(0.5, 0.5), None);
        assert_eq!(h.swatch_at(h.width as f32 - 0.5, 0.5), None);
    }
    #[test]
    fn centre_is_white_and_the_rim_is_saturated() {
        let h = Honeycomb::new(12.0);
        let centre = h.swatch_at(h.width as f32 * 0.5, h.origin.1).unwrap();
        assert_eq!(h.swatches[centre].rgb, [1.0, 1.0, 1.0]);
        let right = h
            .swatch_at(h.origin.0 + SQRT3 * h.radius * RINGS as f32, h.origin.1)
            .unwrap();
        let [r, g, b] = h.swatches[right].rgb;
        assert!(r > 0.5 && g < 0.1 && b < 0.1, "{r} {g} {b}");
    }
    #[test]
    fn greys_run_from_dark_to_white() {
        assert!(grey(0)[0] < 0.1);
        assert_eq!(grey(GREYS - 1), [1.0, 1.0, 1.0]);
        for i in 1..GREYS {
            assert!(grey(i)[0] > grey(i - 1)[0]);
        }
    }
    #[test]
    fn nearest_colour_finds_an_exact_swatch() {
        let h = Honeycomb::new(12.0);
        for i in [0, 40, h.hexes + 3] {
            assert_eq!(h.nearest_colour(h.swatches[i].rgb), i);
        }
    }
    #[test]
    fn rasterize_marks_only_the_marked_swatch() {
        let h = Honeycomb::new(10.0);
        let plain = h.rasterize(None);
        let marked = h.rasterize(Some(0));
        assert_eq!(plain.len(), h.width * h.height * 4);
        let yellow = |p: &[u8]| p.chunks(4).filter(|c| *c == MARK).count();
        assert_eq!(yellow(&plain), 0);
        assert!(yellow(&marked) > 0);
    }
    #[test]
    fn hsv_primaries() {
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4);
        assert!(close(hsv(0.0, 1.0, 1.0), [1.0, 0.0, 0.0]));
        assert!(close(hsv(1.0 / 3.0, 1.0, 1.0), [0.0, 1.0, 0.0]));
        assert!(close(hsv(0.5, 0.0, 0.5), [0.5, 0.5, 0.5]));
    }
}
