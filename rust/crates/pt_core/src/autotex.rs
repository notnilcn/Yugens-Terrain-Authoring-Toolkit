//! Biome auto-texturing rules: unpainted ground gets a filler variant by noise, sand near the
//! water line, rock on steep slopes, a lip texture on cliff tops and snow above a noisy snow
//! line that thins out on steep ground.
//!
//! Not a port: the rule set is modelled on Terrain3D's auto-shader and on the *slot list* of
//! Minor Deity's `Custom/TerrainUber` (`_Tex_*` uniforms, filler variants). Minor Deity's
//! actual rules live in that shader's recovered pixel stage
//! (`minor-deity-dump/shaders-recovered/Custom_TerrainUber/p0_ps_0.hlsl`, the block after
//! `_NoSnowBelow01`): shore bands at normalised elevation above sea 0.005 / 0.03 / 0.04
//! (fine → coarse → grassy, ±0.03 noise), moisture bands 0.1 / 0.3 / 0.7 / 0.95 (ground dry →
//! grass dry → mid → wet → swamp, ±0.3 noise) and a cliff threshold that rises with elevation.
//! Porting them needs a per-cell moisture field and a matching change in
//! `mst_terrain_overlay.gdshaderinc`, so it is left for a later pass.
//!
//! The terrain shader (`mst_terrain_overlay.gdshaderinc`, `mst_auto_slot`) evaluates the same
//! rules per pixel; this module is the CPU reference (used to keep grass off auto-rock/sand/
//! snow, and by tests/tools). Heights are terrain-local. Only slots in `base_mask` are
//! replaced, so anything painted with another slot wins.

use crate::math::*;

#[derive(Clone, Debug, PartialEq)]
pub struct AutoRules {
    pub enabled: bool,
    /// Slots the rules may replace (bit per slot). Default: slot 0 (the default floor).
    pub base_mask: u16,
    /// World scale of the jitter noise that roughens every threshold.
    pub noise_scale: f32,
    /// Filler variant of the base slot, mixed in by low-frequency noise (-1 = off).
    pub filler_slot: i32,
    /// Fraction of the ground covered by fillers (also used for the rock/shore fillers).
    pub filler_amount: f32,
    pub filler_scale: f32,
    /// Rock on slopes steeper than `rock_slope` degrees (± `rock_noise`).
    pub rock_slot: i32,
    pub rock_slope: f32,
    pub rock_noise: f32,
    pub rock_filler_slot: i32,
    /// Cliff-top lip: ridge band factor above `lip_threshold` (0..1, the mesh's UV.y).
    pub lip_slot: i32,
    pub lip_threshold: f32,
    /// Shore: below `water_level + shore_height` (± `shore_noise`), flatter than rock.
    pub shore_slot: i32,
    pub water_level: f32,
    pub shore_height: f32,
    pub shore_noise: f32,
    pub shore_filler_slot: i32,
    /// Snow above `snow_height` ± `snow_noise` (fbm at `snow_noise_scale`), fading out between
    /// `snow_max_slope - snow_slope_fade` and `snow_max_slope` degrees and over `snow_blend`
    /// height units above the line.
    pub snow_slot: i32,
    pub snow_height: f32,
    pub snow_noise: f32,
    pub snow_noise_scale: f32,
    pub snow_max_slope: f32,
    pub snow_slope_fade: f32,
    pub snow_blend: f32,
}

impl Default for AutoRules {
    fn default() -> Self {
        AutoRules {
            enabled: false,
            base_mask: 1,
            noise_scale: 0.35,
            filler_slot: 1,
            filler_amount: 0.25,
            filler_scale: 0.08,
            rock_slot: 5,
            rock_slope: 38.0,
            rock_noise: 6.0,
            rock_filler_slot: 6,
            lip_slot: 4,
            lip_threshold: 0.35,
            shore_slot: 3,
            water_level: 0.0,
            shore_height: 0.8,
            shore_noise: 0.4,
            shore_filler_slot: 8,
            snow_slot: 7,
            snow_height: 14.0,
            snow_noise: 2.0,
            snow_noise_scale: 0.06,
            snow_max_slope: 42.0,
            snow_slope_fade: 12.0,
            snow_blend: 1.5,
        }
    }
}

/// GLSL `pt_hash` (pixel_noise.gdshaderinc), in f32.
pub fn shader_hash(p: Vec2) -> f32 {
    let fr = |x: f32| x - x.floor();
    let mut x = fr(p.x * 123.34);
    let mut y = fr(p.y * 456.21);
    let d = x * (x + 45.32) + y * (y + 45.32);
    x += d;
    y += d;
    fr(x * y)
}

/// GLSL `pt_vnoise`: smooth value noise in [0, 1].
pub fn shader_vnoise(p: Vec2) -> f32 {
    let i = v2(p.x.floor(), p.y.floor());
    let f = p - i;
    let u = v2(f.x * f.x * (3.0 - 2.0 * f.x), f.y * f.y * (3.0 - 2.0 * f.y));
    let a = shader_hash(i);
    let b = shader_hash(i + v2(1.0, 0.0));
    let c = shader_hash(i + v2(0.0, 1.0));
    let d = shader_hash(i + v2(1.0, 1.0));
    let ab = a + (b - a) * u.x;
    let cd = c + (d - c) * u.x;
    ab + (cd - ab) * u.y
}

/// GLSL `pt_fbm`: 4 octaves, [0, 1].
pub fn shader_fbm(mut p: Vec2) -> f32 {
    let (mut s, mut a) = (0.0, 0.5);
    for _ in 0..4 {
        s += shader_vnoise(p) * a;
        p = p * 2.03 + v2(17.1, 9.2);
        a *= 0.5;
    }
    s / 0.9375
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn slot(v: i32) -> Option<u8> {
    (0..15).contains(&v).then_some(v as u8)
}

impl AutoRules {
    pub fn applies_to(&self, base: u8) -> bool {
        self.enabled && base < 16 && self.base_mask & (1 << base) != 0
    }

    /// Slot the rules put at terrain-local `p` (normal y `ny`, ridge band `ridge`), or `None`
    /// to keep the base slot. Mirrors `mst_auto_slot` in the shader.
    pub fn eval(&self, p: Vec3, ny: f32, ridge: f32) -> Option<u8> {
        if !self.enabled {
            return None;
        }
        let xz = p.xz();
        let slope = ny.clamp(-1.0, 1.0).acos().to_degrees();
        let j = shader_vnoise(xz * self.noise_scale) - 0.5;
        let mut out = None;
        let filler = |s: i32, salt: f32| -> Option<u8> {
            if self.filler_amount <= 0.0 {
                return None;
            }
            let n = shader_vnoise(xz * self.filler_scale + v2(salt, salt * 0.7));
            if n < self.filler_amount { slot(s) } else { None }
        };
        if let Some(f) = filler(self.filler_slot, 31.7) {
            out = Some(f);
        }
        let rock = slot(self.rock_slot).is_some() && slope > self.rock_slope + j * 2.0 * self.rock_noise;
        if let Some(s) = slot(self.shore_slot)
            && !rock
            && p.y < self.water_level + self.shore_height + j * 2.0 * self.shore_noise
        {
            out = filler(self.shore_filler_slot, 53.1).or(Some(s));
        }
        if rock {
            out = filler(self.rock_filler_slot, 71.3).or(slot(self.rock_slot));
        }
        if let Some(s) = slot(self.lip_slot)
            && ridge > self.lip_threshold + j * 0.3
        {
            out = Some(s);
        }
        if let Some(s) = slot(self.snow_slot) {
            let line = self.snow_height + (shader_fbm(xz * self.snow_noise_scale) - 0.5) * 2.0 * self.snow_noise;
            if p.y > line {
                let mut cov = 1.0 - smoothstep(self.snow_max_slope - self.snow_slope_fade, self.snow_max_slope, slope);
                cov *= ((p.y - line) / self.snow_blend.max(0.01)).clamp(0.0, 1.0);
                if cov > shader_vnoise(xz * 1.7 + v2(5.3, 2.1)) {
                    out = Some(s);
                }
            }
        }
        out
    }

    /// Effective slot for a base slot (manual paint outside `base_mask` wins).
    pub fn resolve(&self, base: u8, p: Vec3, ny: f32, ridge: f32) -> u8 {
        if self.applies_to(base) { self.eval(p, ny, ridge).unwrap_or(base) } else { base }
    }

    /// Grass may stay on a blade at `p` of base slot `base` (normal y `ny`) when the rules
    /// keep a grass slot there.
    pub fn keeps_grass(&self, base: u8, p: Vec3, ny: f32, has_grass: &[bool; 16]) -> bool {
        let s = self.resolve(base, p, ny, 0.0) as usize;
        s < 16 && has_grass[s]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> AutoRules {
        AutoRules { enabled: true, filler_amount: 0.0, ..Default::default() }
    }

    #[test]
    fn biome_bands() {
        let r = rules();
        // Flat mid-height grass stays.
        assert_eq!(r.resolve(0, v3(10.0, 5.0, 3.0), 1.0, 0.0), 0);
        // Steep -> rock.
        assert_eq!(r.resolve(0, v3(10.0, 5.0, 3.0), 0.5, 0.0), 5);
        // Shore band.
        assert_eq!(r.resolve(0, v3(10.0, 0.2, 3.0), 1.0, 0.0), 3);
        // Snow on flat peaks, rock on steep peaks.
        assert_eq!(r.resolve(0, v3(10.0, 30.0, 3.0), 1.0, 0.0), 7);
        assert_eq!(r.resolve(0, v3(10.0, 30.0, 3.0), 0.3, 0.0), 5);
        // Cliff lip.
        assert_eq!(r.resolve(0, v3(10.0, 5.0, 3.0), 1.0, 0.9), 4);
        // Manual paint (slot 2 not in the mask) wins.
        assert_eq!(r.resolve(2, v3(10.0, 0.2, 3.0), 0.3, 0.0), 2);
        // Disabled = identity.
        assert_eq!(AutoRules::default().resolve(0, v3(1.0, 0.0, 1.0), 0.2, 0.0), 0);
    }

    #[test]
    fn filler_covers_about_its_amount() {
        let r = AutoRules { enabled: true, rock_slot: -1, shore_slot: -1, snow_slot: -1, lip_slot: -1, filler_amount: 0.3, ..Default::default() };
        let mut n = 0;
        let total = 4000;
        for i in 0..total {
            let p = v3((i % 63) as f32 * 3.7, 5.0, (i / 63) as f32 * 3.1);
            if r.resolve(0, p, 1.0, 0.0) == 1 {
                n += 1;
            }
        }
        let f = n as f32 / total as f32;
        assert!((0.1..0.5).contains(&f), "{f}");
    }

    #[test]
    fn noise_matches_glsl_reference_values() {
        // Values of the GLSL pt_hash for a few lattice points (float math, same formula).
        let h = shader_hash(v2(0.0, 0.0));
        assert_eq!(h, 0.0);
        for p in [v2(1.0, 0.0), v2(3.0, -2.0), v2(17.5, 4.25)] {
            let v = shader_hash(p);
            assert!((0.0..1.0).contains(&v));
        }
        let n = shader_vnoise(v2(2.5, 3.5));
        assert!((0.0..=1.0).contains(&n));
    }
}
