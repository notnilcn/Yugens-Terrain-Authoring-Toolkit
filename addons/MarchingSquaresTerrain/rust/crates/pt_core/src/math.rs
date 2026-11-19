//! Tiny vector types so the core crate stays dependency-free and engine-free.

use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IVec2 {
    pub x: i32,
    pub y: i32,
}

pub const fn v2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}
pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}
pub const fn iv2(x: i32, y: i32) -> IVec2 {
    IVec2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = v2(0.0, 0.0);
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }
    pub fn distance(self, o: Vec2) -> f32 {
        (self - o).length()
    }
    pub fn normalized(self) -> Vec2 {
        let l = self.length();
        if l > 1e-12 { self / l } else { Vec2::ZERO }
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
    pub fn rotated(self, angle: f32) -> Vec2 {
        let (s, c) = angle.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);
    pub const UP: Vec3 = v3(0.0, 1.0, 0.0);
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        if l > 1e-12 { self / l } else { Vec3::UP }
    }
    pub fn xz(self) -> Vec2 {
        v2(self.x, self.z)
    }
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
}

macro_rules! impl_ops {
    ($t:ident { $($f:ident),* }, $s:ty) => {
        impl Add for $t { type Output = $t; fn add(self, o: $t) -> $t { $t { $($f: self.$f + o.$f),* } } }
        impl Sub for $t { type Output = $t; fn sub(self, o: $t) -> $t { $t { $($f: self.$f - o.$f),* } } }
        impl Mul<$s> for $t { type Output = $t; fn mul(self, s: $s) -> $t { $t { $($f: self.$f * s),* } } }
        impl Div<$s> for $t { type Output = $t; fn div(self, s: $s) -> $t { $t { $($f: self.$f / s),* } } }
        impl Neg for $t { type Output = $t; fn neg(self) -> $t { $t { $($f: -self.$f),* } } }
    };
}
impl_ops!(Vec2 { x, y }, f32);
impl_ops!(Vec3 { x, y, z }, f32);
impl_ops!(IVec2 { x, y }, i32);

impl Mul<IVec2> for IVec2 {
    type Output = IVec2;
    fn mul(self, o: IVec2) -> IVec2 {
        iv2(self.x * o.x, self.y * o.y)
    }
}

/// Floor division (rounds towards negative infinity), matching GDScript `floori(a / b)`.
pub fn div_floor(a: i32, b: i32) -> i32 {
    a.div_euclid(b) - if b < 0 && a.rem_euclid(b) != 0 { 1 } else { 0 }
}

/// Positive modulo, matching GDScript `posmod`.
pub fn posmod(a: i32, b: i32) -> i32 {
    a.rem_euclid(b)
}

/// Godot's `ease()` curve, used by the bridge tool and brush falloff.
pub fn ease(x: f32, c: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if c > 0.0 {
        if c < 1.0 { 1.0 - (1.0 - x).powf(1.0 / c) } else { x.powf(c) }
    } else if c < 0.0 {
        if x < 0.5 {
            (x * 2.0).powf(-c) * 0.5
        } else {
            (1.0 - (1.0 - (x - 0.5) * 2.0).powf(-c)) * 0.5 + 0.5
        }
    } else {
        0.0
    }
}

/// Deterministic integer hash (SplitMix-style) for scatter and noise.
pub fn hash_u32(mut x: u64) -> u32 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)) as u32
}

/// Hash of a cell and a salt to a float in [0, 1).
pub fn hash01(c: IVec2, salt: u32) -> f32 {
    let k = ((c.x as u32 as u64) << 32) ^ (c.y as u32 as u64) ^ ((salt as u64) << 17).rotate_left(23);
    (hash_u32(k) >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth 2D value noise in [-1, 1], used for procedural initial heights.
pub fn value_noise(p: Vec2, seed: u32) -> f32 {
    let i = iv2(p.x.floor() as i32, p.y.floor() as i32);
    let f = v2(p.x - i.x as f32, p.y - i.y as f32);
    let u = v2(f.x * f.x * (3.0 - 2.0 * f.x), f.y * f.y * (3.0 - 2.0 * f.y));
    let a = hash01(i, seed);
    let b = hash01(i + iv2(1, 0), seed);
    let c = hash01(i + iv2(0, 1), seed);
    let d = hash01(i + iv2(1, 1), seed);
    let ab = a + (b - a) * u.x;
    let cd = c + (d - c) * u.x;
    (ab + (cd - ab) * u.y) * 2.0 - 1.0
}

/// Fractal value noise (fbm) in roughly [-1, 1].
pub fn fbm(p: Vec2, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves.max(1) {
        sum += value_noise(p * freq, seed.wrapping_add(o * 1013)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_div_and_posmod_match_gdscript() {
        assert_eq!(div_floor(-1, 2), -1);
        assert_eq!(div_floor(-2, 2), -1);
        assert_eq!(div_floor(3, 2), 1);
        assert_eq!(posmod(-1, 4), 3);
    }

    #[test]
    fn ease_endpoints() {
        for c in [-2.0, 0.5, 1.0, 2.0] {
            assert!((ease(0.0, c)).abs() < 1e-5);
            assert!((ease(1.0, c) - 1.0).abs() < 1e-5);
        }
    }
}
