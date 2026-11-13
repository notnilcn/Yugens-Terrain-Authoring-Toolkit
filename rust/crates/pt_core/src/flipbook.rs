//! Prerendered particle flipbooks ("particle effects without particles").
//!
//! A small animated texture of 1-pixel particles that **tiles in space** and **loops in time**,
//! baked once and then drawn as a few world-anchored layers instead of simulating particles
//! (`PixelParticleField`, `PixelWeather` rain). The bake follows the compositing recipe of the
//! reference video exactly:
//!
//! - particles are emitted uniformly over one tile at a constant rate (particle `j` of `count`
//!   is born at frame `j * frames / count`), live `lifetime` frames, move at a constant velocity
//!   (plus an optional sideways sway) and fade in / out; they are hard 1-pixel shapes, no AA;
//! - **tiling**: the emitter is centred on the tile origin and the whole layer is cloned four
//!   times, offset to the four tile corners, so a particle leaving one edge re-enters at the
//!   opposite one;
//! - **looping**: the layer is duplicated with a time offset of `frames`, so particles still
//!   alive at the end of the loop are already alive at its start.
//!
//! Both are exact (identical to wrapping positions and ages modulo the tile / loop, which the
//! tests check) as long as a particle never travels more than half a tile, which [`bake`]
//! enforces by shortening lifetimes.
//!
//! The sheet packs the frames row-major into `cols x rows` tiles (64 frames of 128² = 8 x 8 =
//! 1024²). Texel channels: R = particle intensity (0 = empty), G = a per-particle random tone
//! (colour variation in the shader), B = particle kind (0 dot, 1 streak, 2 big, 3 leaf), A = 255.

/// Hard pixel shape of one particle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// One texel.
    Dot,
    /// A line of `n` texels trailing behind the motion (rain).
    Streak(u8),
    /// One texel, or with probability `big_chance` a 2 x 2 block (snow).
    Flake,
    /// Two texels that flip between horizontal and vertical every few frames (tumbling leaf).
    Leaf,
}

/// Bake parameters. Velocities and sway are in texels per frame / texels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Tile width = height in texels.
    pub size: u32,
    /// Frames in the loop.
    pub frames: u32,
    /// Particles emitted per loop (constant rate).
    pub count: u32,
    /// Particle lifetime in frames (shortened to keep a particle within half a tile).
    pub lifetime: u32,
    /// 0..1: lifetimes vary down to `lifetime * (1 - jitter)`.
    pub lifetime_jitter: f32,
    /// Base velocity (x right, y down) in texels per frame.
    pub velocity: [f32; 2],
    /// Random velocity added per particle, as a fraction of `|velocity|` plus `drift`.
    pub velocity_jitter: f32,
    /// Random velocity in any direction (texels / frame), for slowly wandering motes.
    pub drift: f32,
    /// Sideways sway amplitude (texels) and period (frames).
    pub sway: f32,
    pub sway_period: f32,
    /// Fade-in / fade-out lengths in frames (0 = pops).
    pub fade_in: u32,
    pub fade_out: u32,
    /// 0..1: random per-frame brightness flicker (embers).
    pub flicker: f32,
    /// Flake: chance of a 2 x 2 flake.
    pub big_chance: f32,
    pub shape: Shape,
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Preset::DustMotes.params()
    }
}

/// Built-in looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Slow, wandering motes that fade in and out (dust in sunbeams).
    DustMotes,
    /// Fast vertical streaks (rain).
    RainStreaks,
    /// Drifting, swaying flakes, some 2 x 2.
    Snow,
    /// Rising, flickering sparks.
    Embers,
    /// Tumbling, swaying two-texel leaves / pollen.
    Leaves,
}

impl Preset {
    pub const ALL: [Preset; 5] = [Preset::DustMotes, Preset::RainStreaks, Preset::Snow, Preset::Embers, Preset::Leaves];

    pub fn from_index(i: i64) -> Preset {
        Preset::ALL[(i.max(0) as usize).min(Preset::ALL.len() - 1)]
    }

    pub fn params(self) -> Params {
        let base = Params {
            size: 128,
            frames: 64,
            count: 96,
            lifetime: 48,
            lifetime_jitter: 0.3,
            velocity: [0.0, 0.0],
            velocity_jitter: 0.0,
            drift: 0.0,
            sway: 0.0,
            sway_period: 32.0,
            fade_in: 12,
            fade_out: 16,
            flicker: 0.0,
            big_chance: 0.0,
            shape: Shape::Dot,
            seed: 0x0D57,
        };
        match self {
            Preset::DustMotes => Params { count: 150, velocity: [0.05, -0.03], drift: 0.12, sway: 0.8, sway_period: 40.0, ..base },
            Preset::RainStreaks => Params {
                count: 220,
                lifetime: 8,
                lifetime_jitter: 0.25,
                velocity: [0.0, 7.0],
                velocity_jitter: 0.08,
                fade_in: 0,
                fade_out: 2,
                shape: Shape::Streak(5),
                seed: 0x0A11,
                ..base
            },
            Preset::Snow => Params {
                count: 110,
                lifetime: 64,
                lifetime_jitter: 0.2,
                velocity: [0.1, 0.25],
                drift: 0.12,
                sway: 1.5,
                sway_period: 32.0,
                fade_in: 8,
                fade_out: 8,
                big_chance: 0.25,
                shape: Shape::Flake,
                seed: 0x5110,
                ..base
            },
            Preset::Embers => Params {
                count: 56,
                lifetime: 24,
                lifetime_jitter: 0.4,
                velocity: [0.0, -1.2],
                velocity_jitter: 0.4,
                drift: 0.2,
                sway: 1.0,
                sway_period: 16.0,
                fade_in: 2,
                fade_out: 10,
                flicker: 0.5,
                seed: 0xE3BE,
                ..base
            },
            Preset::Leaves => Params {
                count: 36,
                lifetime: 64,
                lifetime_jitter: 0.1,
                velocity: [0.35, 0.45],
                velocity_jitter: 0.3,
                drift: 0.05,
                sway: 3.0,
                sway_period: 64.0 / 3.0,
                fade_in: 8,
                fade_out: 8,
                shape: Shape::Leaf,
                seed: 0x1EAF,
                ..base
            },
        }
    }
}

/// A baked sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Flipbook {
    pub size: u32,
    pub frames: u32,
    pub cols: u32,
    pub rows: u32,
    /// RGBA8, `cols * size` x `rows * size`, row-major.
    pub rgba: Vec<u8>,
}

impl Flipbook {
    pub fn width(&self) -> u32 {
        self.cols * self.size
    }
    pub fn height(&self) -> u32 {
        self.rows * self.size
    }

    /// Texel (x, y) of frame `f` (all in range).
    pub fn texel(&self, f: u32, x: u32, y: u32) -> [u8; 4] {
        let (fx, fy) = (f % self.cols, f / self.cols);
        let (sx, sy) = (fx * self.size + x, fy * self.size + y);
        let i = ((sy * self.width() + sx) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    /// Intensity (R) of every texel of frame `f`.
    pub fn frame_intensity(&self, f: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((self.size * self.size) as usize);
        for y in 0..self.size {
            for x in 0..self.size {
                out.push(self.texel(f, x, y)[0]);
            }
        }
        out
    }
}

/// Sheet grid for `frames` tiles: as square as possible, columns first (64 -> 8 x 8).
pub fn sheet_grid(frames: u32) -> (u32, u32) {
    let frames = frames.max(1);
    let cols = (frames as f64).sqrt().ceil() as u32;
    (cols, frames.div_ceil(cols))
}

/// SplitMix64: deterministic per-seed randoms.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
}

fn hash3(a: u32, b: u32, c: u64) -> f32 {
    let mut r = Rng(c ^ ((a as u64) << 32) ^ (b as u64).wrapping_mul(0x9E37_79B9));
    r.f()
}

/// One emitted particle, fully determined by the seed.
#[derive(Clone, Copy, Debug)]
struct Particle {
    born: u32,
    life: u32,
    /// Start position relative to the emitter centre, in [-size/2, size/2)².
    p0: [f32; 2],
    v: [f32; 2],
    sway_phase: f32,
    tone: u8,
    big: bool,
    id: u32,
}

fn particles(p: &Params) -> Vec<Particle> {
    let size = p.size.max(1) as f32;
    let frames = p.frames.max(1);
    let mut rng = Rng(p.seed ^ 0xF11B_B00C);
    let half = size * 0.5;
    let reach = shape_reach(p.shape) as f32 + p.sway.abs() + 1.0;
    (0..p.count)
        .map(|j| {
            let born = (j as u64 * frames as u64 / p.count.max(1) as u64) as u32;
            let p0 = [rng.f() * size - half, rng.f() * size - half];
            let speed = (p.velocity[0].powi(2) + p.velocity[1].powi(2)).sqrt();
            let jit = p.velocity_jitter * speed;
            let a = rng.f() * std::f32::consts::TAU;
            let d = rng.f() * p.drift;
            let v = [
                p.velocity[0] * (1.0 + rng.signed() * p.velocity_jitter) + rng.signed() * jit * 0.25 + a.cos() * d,
                p.velocity[1] * (1.0 + rng.signed() * p.velocity_jitter) + a.sin() * d,
            ];
            let mut life = ((p.lifetime as f32) * (1.0 - p.lifetime_jitter.clamp(0.0, 1.0) * rng.f())).round().max(1.0) as u32;
            life = life.min(frames);
            // Exactness of the corner clones: never travel more than half a tile.
            let vlen = (v[0] * v[0] + v[1] * v[1]).sqrt();
            if vlen > 1e-6 {
                let max_age = ((half - reach).max(0.0) / vlen).floor() as u32 + 1;
                life = life.min(max_age.max(1));
            }
            Particle {
                born,
                life,
                p0,
                v,
                sway_phase: rng.f() * std::f32::consts::TAU,
                tone: (rng.f() * 255.0) as u8,
                big: rng.f() < p.big_chance,
                id: j,
            }
        })
        .collect()
}

/// Largest pixel offset a shape adds around its anchor.
fn shape_reach(s: Shape) -> u32 {
    match s {
        Shape::Dot => 0,
        Shape::Streak(n) => n.max(1) as u32,
        Shape::Flake => 1,
        Shape::Leaf => 1,
    }
}

/// Pixel offsets (and kind) of a particle at `age`.
fn stamp(p: &Params, q: &Particle, age: u32) -> (Vec<[i32; 2]>, u8) {
    match p.shape {
        Shape::Dot => (vec![[0, 0]], 0),
        Shape::Streak(n) => {
            // Trail opposite the motion, one texel per step along the dominant axis.
            let (vx, vy) = (q.v[0], q.v[1]);
            let m = vx.abs().max(vy.abs()).max(1e-6);
            let (dx, dy) = (-vx / m, -vy / m);
            ((0..n.max(1) as i32).map(|k| [(dx * k as f32).round() as i32, (dy * k as f32).round() as i32]).collect(), 1)
        }
        Shape::Flake => {
            if q.big {
                (vec![[0, 0], [1, 0], [0, 1], [1, 1]], 2)
            } else {
                (vec![[0, 0]], 0)
            }
        }
        Shape::Leaf => {
            let flip = ((age / 4) + q.id) % 2 == 0;
            (if flip { vec![[0, 0], [1, 0]] } else { vec![[0, 0], [0, 1]] }, 3)
        }
    }
}

/// Position (relative to the emitter centre) and intensity 0..1 of a particle at `age`.
fn state(p: &Params, q: &Particle, age: u32) -> ([f32; 2], f32) {
    let a = age as f32;
    let mut pos = [q.p0[0] + q.v[0] * a, q.p0[1] + q.v[1] * a];
    if p.sway != 0.0 && p.sway_period > 0.0 {
        // Sideways to the motion (x when there is none).
        let (vx, vy) = (q.v[0], q.v[1]);
        let l = (vx * vx + vy * vy).sqrt();
        let (sx, sy) = if l > 1e-4 { (-vy / l, vx / l) } else { (1.0, 0.0) };
        let s = (a / p.sway_period * std::f32::consts::TAU + q.sway_phase).sin() - q.sway_phase.sin();
        pos[0] += sx * s * p.sway;
        pos[1] += sy * s * p.sway;
    }
    let fin = if p.fade_in == 0 { 1.0 } else { ((age + 1) as f32 / p.fade_in as f32).min(1.0) };
    let fout = if p.fade_out == 0 { 1.0 } else { ((q.life - age) as f32 / p.fade_out as f32).min(1.0) };
    let mut i = fin.min(fout);
    if p.flicker > 0.0 {
        i *= 1.0 - p.flicker * hash3(q.id, age, p.seed);
    }
    (pos, i.clamp(0.0, 1.0))
}

fn put(rgba: &mut [u8], width: u32, x: u32, y: u32, value: u8, tone: u8, kind: u8) {
    let i = ((y * width + x) * 4) as usize;
    // Max blend; ties broken by (tone, kind) so the result never depends on drawing order.
    if (value, tone, kind) > (rgba[i], rgba[i + 1], rgba[i + 2]) {
        rgba[i] = value;
        rgba[i + 1] = tone;
        rgba[i + 2] = kind;
    }
    rgba[i + 3] = 255;
}

/// Bakes the tiling, looping sheet by the corner-clone / time-offset-duplicate method.
pub fn bake(p: &Params) -> Flipbook {
    let size = p.size.max(1);
    let frames = p.frames.max(1);
    let (cols, rows) = sheet_grid(frames);
    let width = cols * size;
    let mut rgba = vec![0u8; (width * rows * size * 4) as usize];
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let s = size as f32;
    // The emitter is centred on the tile origin; the four clones sit on the four corners.
    let corners = [[0.0, 0.0], [s, 0.0], [0.0, s], [s, s]];
    for q in particles(p) {
        // Layer + its duplicate one loop later (time offset = frames).
        for shift in [0i64, frames as i64] {
            for f in 0..frames {
                let age = f as i64 + shift - q.born as i64;
                if age < 0 || age >= q.life as i64 {
                    continue;
                }
                let age = age as u32;
                let (pos, inten) = state(p, &q, age);
                let v = (inten * 255.0).round() as u8;
                if v == 0 {
                    continue;
                }
                let (offs, kind) = stamp(p, &q, age);
                let (fx, fy) = (f % cols, f / cols);
                for c in corners {
                    for o in &offs {
                        let x = (pos[0] + c[0]).floor() as i64 + o[0] as i64;
                        let y = (pos[1] + c[1]).floor() as i64 + o[1] as i64;
                        if x < 0 || y < 0 || x >= size as i64 || y >= size as i64 {
                            continue;
                        }
                        put(&mut rgba, width, fx * size + x as u32, fy * size + y as u32, v, q.tone, kind);
                    }
                }
            }
        }
    }
    Flipbook { size, frames, cols, rows, rgba }
}

/// Bakes a built-in preset with an optional seed override.
pub fn bake_preset(preset: Preset, seed: Option<u64>) -> Flipbook {
    let mut p = preset.params();
    if let Some(s) = seed {
        p.seed = s;
    }
    bake(&p)
}

/// Reference bake that wraps positions and ages modulo the tile / loop (tests only).
#[cfg(test)]
fn bake_wrapped(p: &Params) -> Flipbook {
    let size = p.size.max(1);
    let frames = p.frames.max(1);
    let (cols, rows) = sheet_grid(frames);
    let width = cols * size;
    let mut rgba = vec![0u8; (width * rows * size * 4) as usize];
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    for q in particles(p) {
        for f in 0..frames {
            let age = (f as i64 - q.born as i64).rem_euclid(frames as i64) as u32;
            if age >= q.life {
                continue;
            }
            let (pos, inten) = state(p, &q, age);
            let v = (inten * 255.0).round() as u8;
            if v == 0 {
                continue;
            }
            let (offs, kind) = stamp(p, &q, age);
            let (fx, fy) = (f % cols, f / cols);
            for o in &offs {
                let x = (pos[0].floor() as i64 + o[0] as i64).rem_euclid(size as i64) as u32;
                let y = (pos[1].floor() as i64 + o[1] as i64).rem_euclid(size as i64) as u32;
                put(&mut rgba, width, fx * size + x, fy * size + y, v, q.tone, kind);
            }
        }
    }
    Flipbook { size, frames, cols, rows, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(preset: Preset) -> Params {
        Params { size: 32, frames: 16, count: preset.params().count / 4 + 4, ..preset.params() }
    }

    #[test]
    fn grid_layout() {
        assert_eq!(sheet_grid(64), (8, 8));
        assert_eq!(sheet_grid(16), (4, 4));
        assert_eq!(sheet_grid(10), (4, 3));
        let b = bake(&Preset::DustMotes.params());
        assert_eq!((b.width(), b.height()), (1024, 1024));
        assert_eq!(b.rgba.len(), 1024 * 1024 * 4);
    }

    #[test]
    fn clones_and_duplicate_equal_wrapping() {
        // Corner clones + one-loop duplicate == wrapping positions / ages modulo tile / loop,
        // i.e. the sheet tiles seamlessly in space and loops seamlessly in time.
        for preset in Preset::ALL {
            for p in [preset.params(), small(preset)] {
                assert_eq!(bake(&p), bake_wrapped(&p), "{preset:?} {}", p.size);
            }
        }
    }

    #[test]
    fn deterministic_and_seeded() {
        let a = bake_preset(Preset::Snow, None);
        assert_eq!(a, bake_preset(Preset::Snow, None));
        assert_ne!(a, bake_preset(Preset::Snow, Some(7)));
    }

    #[test]
    fn constant_rate() {
        // Uniform emission: the number of live particles barely changes over the loop.
        let p = Preset::DustMotes.params();
        let qs = particles(&p);
        let alive: Vec<usize> = (0..p.frames)
            .map(|f| qs.iter().filter(|q| ((f as i64 - q.born as i64).rem_euclid(p.frames as i64) as u32) < q.life).count())
            .collect();
        let (lo, hi) = (*alive.iter().min().unwrap(), *alive.iter().max().unwrap());
        assert!(lo > 0 && hi - lo <= hi / 3 + 3, "{alive:?}");
    }

    #[test]
    fn particles_are_hard_pixels() {
        let b = bake(&small(Preset::RainStreaks));
        let lit = b.rgba.chunks_exact(4).filter(|t| t[0] > 0).count();
        assert!(lit > 0);
        // Streaks: every lit texel is a streak pixel.
        assert!(b.rgba.chunks_exact(4).filter(|t| t[0] > 0).all(|t| t[2] == 1));
        // Travel limit: no particle moves more than half a tile.
        let p = Preset::RainStreaks.params();
        for q in particles(&p) {
            let d = (q.v[0].hypot(q.v[1])) * (q.life - 1) as f32;
            assert!(d + 5.0 <= p.size as f32 * 0.5 + 1.0, "{d}");
        }
    }

    #[test]
    fn fades_in_and_out() {
        let p = Params { count: 1, lifetime: 20, lifetime_jitter: 0.0, fade_in: 4, fade_out: 4, flicker: 0.0, ..Preset::DustMotes.params() };
        let q = particles(&p)[0];
        let (_, first) = state(&p, &q, 0);
        let (_, mid) = state(&p, &q, 10);
        let (_, last) = state(&p, &q, q.life - 1);
        assert!(first < mid && last < mid && (mid - 1.0).abs() < 1e-6);
    }
}
