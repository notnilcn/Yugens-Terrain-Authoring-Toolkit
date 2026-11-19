//! Palette baking to a colour-cube image. A palette is a list of sRGB colours; the bake maps
//! every quantized RGB input to its nearest palette colour, by default in OKLab. The result is a
//! `N*N` wide, `N` tall image (`N` = [`LUT_SIZE`]) where pixel `(b * N + r, g)` holds the mapping
//! for input `(r, g, b) / (N - 1)`, so a shader can look a graded colour up with one texture
//! fetch — the nearest-colour idea behind the edge/outline shaders of `3DPixelArt_Tutorial` and
//! `3d-pixel-art-in-godot`, with the cube/chart split following Terrain3D's generated colour-map
//! textures.

/// Levels per colour axis: the image is a `LUT_SIZE` cube, `LUT_SIZE * LUT_SIZE` samples wide.
pub const LUT_SIZE: usize = 32;

/// Width of a colour-cube image in this addon's layout (`r` fastest, then `b`).
pub const LUT_WIDTH: usize = LUT_SIZE * LUT_SIZE;

/// Side of a full colour-cube chart: [`LUT_SIZE`] bands of [`LUT_SIZE`] rows, [`LUT_WIDTH`] wide,
/// pixel `(b * N + r, g + N * band)`. Band 0 is the undithered cube.
pub const CHART_SIZE: usize = LUT_WIDTH;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// sRGB (0..1) to OKLab.
pub fn oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// sRGB (0..1) to CIE Lab (CIE76, D65).
pub fn cie_lab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    let f = |t: f32| if t > 0.008_856 { t.cbrt() } else { t / 0.128_418 + 0.137_931 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// sRGB (0..1) to HSV (each 0..1).
pub fn hsv(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 1e-6 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, if max <= 1e-6 { 0.0 } else { d / max }, max]
}

/// Colour-match metric of the bake: perceptual (OKLab) unless another distance is picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorQuantize {
    /// OKLab distance (the default).
    Perceptual = 0,
    /// Plain RGB channel distance.
    Channel = 1,
    /// Hue / saturation / value distance, hue treated as circular.
    Hue = 2,
    /// [`ColorQuantize::Hue`] with the per-axis weights of [`QuantizeOptions::weights`].
    HueWeighted = 3,
    /// CIE Lab (CIE76) distance.
    Lab = 4,
    /// Nearest luma (Rec.709), the input mapped through an optional brightness curve.
    Value = 5,
}

impl ColorQuantize {
    pub fn from_index(v: i32) -> ColorQuantize {
        match v {
            1 => ColorQuantize::Channel,
            2 => ColorQuantize::Hue,
            3 => ColorQuantize::HueWeighted,
            4 => ColorQuantize::Lab,
            5 => ColorQuantize::Value,
            _ => ColorQuantize::Perceptual,
        }
    }
}

/// Bake options. `value_curve` (256 samples over 0..1) remaps the input luma for
/// [`ColorQuantize::Value`].
#[derive(Clone, Debug)]
pub struct QuantizeOptions {
    pub quantize: ColorQuantize,
    pub weights: [f32; 3],
    pub value_curve: Option<Vec<f32>>,
}

impl Default for QuantizeOptions {
    fn default() -> Self {
        QuantizeOptions { quantize: ColorQuantize::Perceptual, weights: [1.0, 1.0, 1.0], value_curve: None }
    }
}

fn luma(c: [f32; 3]) -> f32 {
    c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
}

fn color_features(c: [f32; 3], o: &QuantizeOptions, input: bool) -> [f32; 3] {
    match o.quantize {
        ColorQuantize::Perceptual => oklab(c),
        ColorQuantize::Channel => c,
        ColorQuantize::Lab => cie_lab(c),
        ColorQuantize::Hue | ColorQuantize::HueWeighted => hsv(c),
        ColorQuantize::Value => {
            let mut v = luma(c).clamp(0.0, 1.0);
            if input && let Some(curve) = &o.value_curve && !curve.is_empty() {
                v = curve[((v * (curve.len() - 1) as f32).round() as usize).min(curve.len() - 1)];
            }
            [v, 0.0, 0.0]
        }
    }
}

fn distance(a: [f32; 3], b: [f32; 3], o: &QuantizeOptions) -> f32 {
    match o.quantize {
        ColorQuantize::Hue | ColorQuantize::HueWeighted => {
            // Hue is circular.
            let dh = (a[0] - b[0]).abs();
            let dh = dh.min(1.0 - dh);
            let w = if o.quantize == ColorQuantize::HueWeighted { o.weights } else { [1.0; 3] };
            w[0] * dh * dh + w[1] * (a[1] - b[1]).powi(2) + w[2] * (a[2] - b[2]).powi(2)
        }
        _ => (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2),
    }
}

/// Bakes an RGBA8 colour cube in OKLab. Returns `(width, height, bytes)`.
pub fn bake_lut(palette: &[[f32; 3]]) -> (usize, usize, Vec<u8>) {
    bake_lut_with(palette, &QuantizeOptions::default())
}

/// Bakes an RGBA8 colour cube with any metric. Returns `(width, height, bytes)`.
pub fn bake_lut_with(palette: &[[f32; 3]], opts: &QuantizeOptions) -> (usize, usize, Vec<u8>) {
    let n = LUT_SIZE;
    let (w, h) = (n * n, n);
    let mut bytes = vec![0u8; w * h * 4];
    let feats: Vec<[f32; 3]> = palette.iter().map(|c| color_features(*c, opts, false)).collect();
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let rgb = [r as f32 / (n - 1) as f32, g as f32 / (n - 1) as f32, b as f32 / (n - 1) as f32];
                let out = if feats.is_empty() {
                    rgb
                } else {
                    let f = color_features(rgb, opts, true);
                    let mut best = (0, f32::MAX);
                    for (i, p) in feats.iter().enumerate() {
                        let d = distance(f, *p, opts);
                        if d < best.1 {
                            best = (i, d);
                        }
                    }
                    palette[best.0]
                };
                let o = ((g * w) + b * n + r) * 4;
                bytes[o] = (out[0].clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes[o + 1] = (out[1].clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes[o + 2] = (out[2].clamp(0.0, 1.0) * 255.0).round() as u8;
                bytes[o + 3] = 255;
            }
        }
    }
    (w, h, bytes)
}

/// Converts an RGBA8 colour-cube image into this addon's [`LUT_WIDTH`] x [`LUT_SIZE`] layout.
/// Accepts the native layout (returned unchanged) and a full chart ([`CHART_SIZE`] square, band 0
/// used since the resolve pass dithers itself).
pub fn import_color_cube(w: usize, h: usize, rgba: &[u8]) -> Option<Vec<u8>> {
    if rgba.len() < w * h * 4 {
        return None;
    }
    if w == LUT_WIDTH && h == LUT_SIZE {
        return Some(rgba[..w * h * 4].to_vec());
    }
    if w == CHART_SIZE && h == CHART_SIZE {
        let n = LUT_SIZE;
        let mut out = vec![0u8; LUT_WIDTH * n * 4];
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let (x, y) = (b * n + r, g);
                    let i = (y * CHART_SIZE + x) * 4;
                    let o = (g * LUT_WIDTH + b * n + r) * 4;
                    out[o..o + 3].copy_from_slice(&rgba[i..i + 3]);
                    out[o + 3] = 255;
                }
            }
        }
        return Some(out);
    }
    None
}

/// A warm, earthy 32-colour default palette authored for this addon.
pub const DEFAULT_PALETTE: [u32; 32] = [
    0x1a1423, 0x2d2238, 0x3f3150, 0x5b4a6b, 0x7d6f8a, 0xa79db0, 0xd8d0d4, 0xf6efe4, //
    0x2a3b2f, 0x34503a, 0x45683f, 0x5e8445, 0x7fa04c, 0xa8bd5e, 0x1f3a4a, 0x2a5a6e, //
    0x3c7f8f, 0x63a7a8, 0x9dd0c4, 0x4a2c25, 0x6e4232, 0x8f5b3e, 0xb3784c, 0xd09c62, //
    0xe8c690, 0x7a3b3b, 0xa64b3c, 0xd0703f, 0xeca35a, 0xf6d36b, 0x5c5a4a, 0x8a8670,
];

pub fn hex_to_rgb(c: u32) -> [f32; 3] {
    [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_maps_palette_colours_to_themselves() {
        let pal: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 0.0]];
        let (w, _, bytes) = bake_lut(&pal);
        let n = LUT_SIZE;
        let px = |r: usize, g: usize, b: usize| {
            let o = ((g * w) + b * n + r) * 4;
            [bytes[o], bytes[o + 1], bytes[o + 2]]
        };
        assert_eq!(px(0, 0, 0), [0, 0, 0]);
        assert_eq!(px(n - 1, n - 1, n - 1), [255, 255, 255]);
        assert_eq!(px(n - 1, 0, 0), [255, 0, 0]);
        assert_eq!(px(n - 1, 2, 1), [255, 0, 0]);
    }

    #[test]
    fn every_quantize_mode_maps_palette_colours_to_themselves() {
        let pal: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let n = LUT_SIZE;
        for m in 0..6 {
            let opts = QuantizeOptions { quantize: ColorQuantize::from_index(m), ..Default::default() };
            let (w, _, bytes) = bake_lut_with(&pal, &opts);
            let px = |r: usize, g: usize, b: usize| {
                let o = ((g * w) + b * n + r) * 4;
                [bytes[o], bytes[o + 1], bytes[o + 2]]
            };
            assert_eq!(px(0, 0, 0), [0, 0, 0], "metric {m}");
            assert_eq!(px(n - 1, n - 1, n - 1), [255, 255, 255], "metric {m}");
            if opts.quantize != ColorQuantize::Value {
                assert_eq!(px(n - 1, 0, 0), [255, 0, 0], "metric {m}");
                assert_eq!(px(0, 0, n - 1), [0, 0, 255], "metric {m}");
            }
        }
    }

    #[test]
    fn import_color_cube_chart() {
        // A chart stores the input colour itself: x = b * 32 + r, y = g + 32 * band.
        let n = LUT_SIZE;
        let mut src = vec![0u8; CHART_SIZE * CHART_SIZE * 4];
        for y in 0..CHART_SIZE {
            for x in 0..CHART_SIZE {
                let (r, b, g) = (x % n, x / n, y % n);
                let level = |v: usize| (v * 255 / (n - 1)) as u8;
                let i = (y * CHART_SIZE + x) * 4;
                src[i..i + 4].copy_from_slice(&[level(r), level(g), level(b), 255]);
            }
        }
        let out = import_color_cube(CHART_SIZE, CHART_SIZE, &src).unwrap();
        assert_eq!(out.len(), LUT_WIDTH * LUT_SIZE * 4);
        let o = ((n - 1) * LUT_WIDTH + (n - 1) * n + 0) * 4;
        assert_eq!(&out[o..o + 3], &[0, 255, 255]);
        let (w, h, native) = bake_lut(&[[0.5, 0.5, 0.5]]);
        assert_eq!(import_color_cube(w, h, &native).unwrap(), native);
        assert!(import_color_cube(10, 10, &[0; 400]).is_none());
    }
}
