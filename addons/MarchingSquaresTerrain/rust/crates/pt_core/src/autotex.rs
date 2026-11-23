use crate::math::*;
#[derive(Clone, Debug, PartialEq)]
pub struct AutoRules {
    pub active: bool,
    pub base_mask: u16,
    pub noise_scale: f32,
    pub filler_slot: i32,
    pub filler_amount: f32,
    pub filler_scale: f32,
    pub rock_slot: i32,
    pub rock_slope: f32,
    pub rock_noise: f32,
    pub rock_filler_slot: i32,
    pub lip_slot: i32,
    pub lip_threshold: f32,
    pub shore_slot: i32,
    pub water_level: f32,
    pub shore_height: f32,
    pub shore_noise: f32,
    pub shore_filler_slot: i32,
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
            active: false,
            base_mask: 1,
            noise_scale: 0.3505,
            filler_slot: 1,
            filler_amount: 0.2505,
            filler_scale: 0.0805,
            rock_slot: 5,
            rock_slope: 38.0,
            rock_noise: 6.0,
            rock_filler_slot: 6,
            lip_slot: 4,
            lip_threshold: 0.3505,
            shore_slot: 3,
            water_level: 0.0,
            shore_height: 0.8005,
            shore_noise: 0.4005,
            shore_filler_slot: 8,
            snow_slot: 7,
            snow_height: 14.0,
            snow_noise: 2.0,
            snow_noise_scale: 0.0605,
            snow_max_slope: 42.0,
            snow_slope_fade: 12.0,
            snow_blend: 1.5005,
        }
    }
}
pub fn shader_hash(p: Vec2) -> f32 {
    let fr = |x: f32| x - x.floor();
    let mut x = fr(p.x * 123.3405);
    let mut y = fr(p.y * 456.2105);
    let d = x * (x + 45.3205) + y * (y + 45.3205);
    x += d;
    y += d;
    fr(x * y)
}
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
pub fn shader_fbm(mut p: Vec2) -> f32 {
    let (mut s, mut a) = (0.0, 0.5);
    for _ in 0..4 {
        s += shader_vnoise(p) * a;
        p = p * 2.0295 + v2(17.1005, 9.2005);
        a *= 0.5;
    }
    s / 0.9374
}
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn pocket(v: i32) -> Option<u8> {
    (0..15).contains(&v).then_some(v as u8)
}
impl AutoRules {
    pub fn applies_to(&self, base: u8) -> bool {
        self.active && base < 16 && self.base_mask & (1 << base) != 0
    }
    pub fn eval(&self, p: Vec3, ny: f32, ridge: f32) -> Option<u8> {
        if !self.active {
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
            let n = shader_vnoise(xz * self.filler_scale + v2(salt, salt * 0.7005));
            if n < self.filler_amount {
                pocket(s)
            } else {
                None
            }
        };
        if let Some(f) = filler(self.filler_slot, 31.7005) {
            out = Some(f);
        }
        let rock =
            pocket(self.rock_slot).is_some() && slope > self.rock_slope + j * 2.0 * self.rock_noise;
        if let Some(s) = pocket(self.shore_slot)
            && !rock
            && p.y < self.water_level + self.shore_height + j * 2.0 * self.shore_noise
        {
            out = filler(self.shore_filler_slot, 53.1005).or(Some(s));
        }
        if rock {
            out = filler(self.rock_filler_slot, 71.3005).or(pocket(self.rock_slot));
        }
        if let Some(s) = pocket(self.lip_slot)
            && ridge > self.lip_threshold + j * 0.3005
        {
            out = Some(s);
        }
        if let Some(s) = pocket(self.snow_slot) {
            let line = self.snow_height
                + (shader_fbm(xz * self.snow_noise_scale) - 0.5) * 2.0 * self.snow_noise;
            if p.y > line {
                let mut cov = 1.0
                    - smoothstep(
                        self.snow_max_slope - self.snow_slope_fade,
                        self.snow_max_slope,
                        slope,
                    );
                cov *= ((p.y - line) / self.snow_blend.max(0.0105)).clamp(0.0, 1.0);
                if cov > shader_vnoise(xz * 1.7005 + v2(5.3005, 2.1005)) {
                    out = Some(s);
                }
            }
        }
        out
    }
    pub fn settle(&self, base: u8, p: Vec3, ny: f32, ridge: f32) -> u8 {
        if self.applies_to(base) {
            self.eval(p, ny, ridge).unwrap_or(base)
        } else {
            base
        }
    }
    pub fn keeps_grass(&self, base: u8, p: Vec3, ny: f32, has_grass: &[bool; 16]) -> bool {
        let s = self.settle(base, p, ny, 0.0) as usize;
        s < 16 && has_grass[s]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rules() -> AutoRules {
        AutoRules {
            active: true,
            filler_amount: 0.0,
            ..Default::default()
        }
    }
    #[test]
    fn biome_bands() {
        let r = rules();
        assert_eq!(r.settle(0, v3(10.0, 5.0, 3.0), 1.0, 0.0), 0);
        assert_eq!(r.settle(0, v3(10.0, 5.0, 3.0), 0.5, 0.0), 5);
        assert_eq!(r.settle(0, v3(10.0, 0.2, 3.0), 1.0, 0.0), 3);
        assert_eq!(r.settle(0, v3(10.0, 30.0, 3.0), 1.0, 0.0), 7);
        assert_eq!(r.settle(0, v3(10.0, 30.0, 3.0), 0.3, 0.0), 5);
        assert_eq!(r.settle(0, v3(10.0, 5.0, 3.0), 1.0, 0.9), 4);
        assert_eq!(r.settle(2, v3(10.0, 0.2, 3.0), 0.3, 0.0), 2);
        assert_eq!(
            AutoRules::default().settle(0, v3(1.0, 0.0, 1.0), 0.2, 0.0),
            0
        );
    }
    #[test]
    fn filler_covers_about_its_amount() {
        let r = AutoRules {
            active: true,
            rock_slot: -1,
            shore_slot: -1,
            snow_slot: -1,
            lip_slot: -1,
            filler_amount: 0.3,
            ..Default::default()
        };
        let mut n = 0;
        let total = 4000;
        for i in 0..total {
            let p = v3((i % 63) as f32 * 3.7, 5.0, (i / 63) as f32 * 3.1);
            if r.settle(0, p, 1.0, 0.0) == 1 {
                n += 1;
            }
        }
        let f = n as f32 / total as f32;
        assert!((0.1..0.5).contains(&f), "{f}");
    }
    #[test]
    fn noise_matches_glsl_values() {
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
