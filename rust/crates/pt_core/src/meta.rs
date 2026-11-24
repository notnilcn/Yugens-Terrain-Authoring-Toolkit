pub const DIGIT_BASE: u32 = 192;
pub const DIGIT_OFFSET: u32 = 64;
pub const FLAG_LEVELS: u32 = 64;
pub const NORMAL_GRID: u32 = 9;
pub const NORMAL_LEVELS: u32 = NORMAL_GRID * NORMAL_GRID;
pub const DEPTH_LEVELS: u32 = 1365;
pub const DIGIT_CODES: u32 = DIGIT_BASE * DIGIT_BASE * DIGIT_BASE;
pub const MIN_EXACT_CHANNEL: u32 = 56;
const _: () = assert!(FLAG_LEVELS * NORMAL_LEVELS * DEPTH_LEVELS <= DIGIT_CODES);
const _: () = assert!(DIGIT_OFFSET + DIGIT_BASE - 1 <= 255);
const _: () = assert!(DIGIT_OFFSET >= MIN_EXACT_CHANNEL);
pub const OUTLINE_IDS: u32 = 256;
pub const MAX_USER_ID: u32 = 253;
pub const HOVER_ID: u32 = 254;
pub const SELECT_ID: u32 = 255;
pub const ID_AUX_LEVELS: u32 = 4;
pub const MAX_PIXEL_SIZE: u32 = 4096;
pub const META_PIXEL_SIZES: u32 = 4;
pub const FLAG_SIZE_SHIFT: i32 = 19;
const _: () = assert!(MAX_PIXEL_SIZE * OUTLINE_IDS * ID_AUX_LEVELS <= DIGIT_CODES);
const _: () = assert!(FLAG_SIZE_SHIFT + 12 <= 31 && MAX_PIXEL_SIZE == 1 << 12);
pub const ID_CAMERA_BIT: u32 = 23;
pub const FLAG_OUTLINE: i32 = 1 << 5;
pub const FLAG_SNAP: i32 = 1 << 6;
pub const FLAG_NO_PALETTE: i32 = 1 << 17;
pub const FLAG_NO_DITHER: i32 = 1 << 18;
pub fn object_flags(
    pixel_size: i32,
    outline: bool,
    outline_id: i32,
    snap: bool,
    palette: bool,
    dither: bool,
) -> i32 {
    let id = outline_id.clamp(0, OUTLINE_IDS as i32 - 1);
    let ps = pixel_size.clamp(1, MAX_PIXEL_SIZE as i32);
    (ps.min(META_PIXEL_SIZES as i32) - 1)
        | ((ps - 1) << FLAG_SIZE_SHIFT)
        | ((id & 7) << 2)
        | if outline { FLAG_OUTLINE } else { 0 }
        | if snap { FLAG_SNAP } else { 0 }
        | (id << 8)
        | if palette { 0 } else { FLAG_NO_PALETTE }
        | if dither { 0 } else { FLAG_NO_DITHER }
}
pub fn with_outline_id(flags: i32, outline_id: u32) -> i32 {
    let id = outline_id.min(OUTLINE_IDS - 1) as i32;
    (flags & !(7 << 2) & !(255 << 8)) | ((id & 7) << 2) | (id << 8) | FLAG_OUTLINE
}
pub fn block_sizes(in_use: impl IntoIterator<Item = (i32, u64)>, max_large: usize) -> Vec<i32> {
    let mut newest: Vec<(i32, u64)> = Vec::new();
    for (k, seq) in in_use {
        if k <= META_PIXEL_SIZES as i32 {
            continue;
        }
        match newest.iter_mut().find(|(s, _)| *s == k) {
            Some(e) => e.1 = e.1.max(seq),
            None => newest.push((k, seq)),
        }
    }
    newest.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    newest.truncate(max_large);
    let mut v: Vec<i32> = (2..=META_PIXEL_SIZES as i32).collect();
    v.extend(newest.iter().map(|(k, _)| *k));
    v.sort_unstable();
    v
}
pub fn pixel_size_from_flags(flags: i32) -> u32 {
    let hi = ((flags >> FLAG_SIZE_SHIFT) & (MAX_PIXEL_SIZE as i32 - 1)) as u32;
    if hi != 0 {
        hi + 1
    } else {
        (flags & 3) as u32 + 1
    }
}
pub fn id_meta_from_flags(flags: i32) -> IdMeta {
    let hi = ((flags >> 8) & 255) as u32;
    let lo = ((flags >> 2) & 7) as u32;
    IdMeta {
        outline_id: if hi != 0 { hi } else { lo },
        palette: flags & FLAG_NO_PALETTE == 0,
        dither: flags & FLAG_NO_DITHER == 0,
        pixel_size: pixel_size_from_flags(flags),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    pub pixel_size: u32,
    pub outline_id: u32,
    pub outline: bool,
    pub upright: u32,
    pub depth: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdMeta {
    pub outline_id: u32,
    pub palette: bool,
    pub dither: bool,
    pub pixel_size: u32,
}
fn digits(n: u32) -> [u8; 3] {
    debug_assert!(n < DIGIT_CODES);
    let d0 = n / (DIGIT_BASE * DIGIT_BASE);
    let d1 = (n / DIGIT_BASE) % DIGIT_BASE;
    let d2 = n % DIGIT_BASE;
    [
        (d0 + DIGIT_OFFSET) as u8,
        (d1 + DIGIT_OFFSET) as u8,
        (d2 + DIGIT_OFFSET) as u8,
    ]
}
fn number(b: [u8; 3]) -> Option<u32> {
    if b.iter().any(|&x| (x as u32) < DIGIT_OFFSET) {
        return None;
    }
    let d = b.map(|x| x as u32 - DIGIT_OFFSET);
    Some((d[0] * DIGIT_BASE + d[1]) * DIGIT_BASE + d[2])
}
pub fn pack(m: Meta) -> [u8; 3] {
    let flags = (m.pixel_size.clamp(1, META_PIXEL_SIZES) - 1)
        | (m.outline_id.min(7) << 2)
        | ((m.outline as u32) << 5);
    digits(
        (flags * NORMAL_LEVELS + m.upright.min(NORMAL_LEVELS - 1)) * DEPTH_LEVELS
            + m.depth.min(DEPTH_LEVELS - 1),
    )
}
pub fn unpack(b: [u8; 3]) -> Option<Meta> {
    let n = number(b)?;
    let depth = n % DEPTH_LEVELS;
    let rest = n / DEPTH_LEVELS;
    let normal = rest % NORMAL_LEVELS;
    let flags = rest / NORMAL_LEVELS;
    Some(Meta {
        pixel_size: (flags & 3) + 1,
        outline_id: (flags >> 2) & 7,
        outline: flags & 32 != 0,
        upright: normal,
        depth,
    })
}
pub fn pack_id(m: IdMeta) -> [u8; 3] {
    let aux = (!m.palette as u32) | ((!m.dither as u32) << 1);
    let size = m.pixel_size.clamp(1, MAX_PIXEL_SIZE) - 1;
    digits((size * ID_AUX_LEVELS + aux) * OUTLINE_IDS + m.outline_id.min(OUTLINE_IDS - 1))
}
pub fn unpack_id(b: [u8; 3]) -> Option<IdMeta> {
    let n = number(b)?;
    let rest = n / OUTLINE_IDS;
    let (size, aux) = (rest / ID_AUX_LEVELS, rest % ID_AUX_LEVELS);
    if size >= MAX_PIXEL_SIZE {
        return None;
    }
    Some(IdMeta {
        outline_id: n % OUTLINE_IDS,
        palette: aux & 1 == 0,
        dither: aux & 2 == 0,
        pixel_size: size + 1,
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn block_sizes_keep_every_size_in_use() {
        assert_eq!(block_sizes([(1, 0), (3, 1)], 13), vec![2, 3, 4]);
        assert_eq!(
            block_sizes([(8, 0), (6, 1), (40, 37)], 13),
            vec![2, 3, 4, 6, 8, 40]
        );
        assert_eq!(
            block_sizes([(12, 0), (12, 5), (9, 2)], 13),
            vec![2, 3, 4, 9, 12]
        );
        assert_eq!(
            block_sizes([(12, 0), (12, 5), (9, 2), (7, 3)], 2),
            vec![2, 3, 4, 7, 12]
        );
        assert_eq!(
            block_sizes([(4096, 0), (1000, 1)], 13),
            vec![2, 3, 4, 1000, 4096]
        );
    }
    use super::*;
    #[test]
    fn roundtrip_extremes() {
        for ps in 1..=4 {
            for id in [0, 3, 7] {
                for o in [false, true] {
                    for (nrm, dep) in [(0, 0), (80, 1364), (40, 700)] {
                        let m = Meta {
                            pixel_size: ps,
                            outline_id: id,
                            outline: o,
                            upright: nrm,
                            depth: dep,
                        };
                        assert_eq!(unpack(pack(m)), Some(m));
                    }
                }
            }
        }
        assert_eq!(unpack([0, 0, 0]), None);
    }
    #[test]
    fn digit_budget() {
        assert!(FLAG_LEVELS * NORMAL_LEVELS * DEPTH_LEVELS <= DIGIT_CODES);
        assert!(MAX_PIXEL_SIZE * OUTLINE_IDS * ID_AUX_LEVELS <= DIGIT_CODES);
        let max_meta = pack(Meta {
            pixel_size: 4,
            outline_id: 7,
            outline: true,
            upright: NORMAL_LEVELS - 1,
            depth: DEPTH_LEVELS - 1,
        });
        let max_id = pack_id(IdMeta {
            outline_id: SELECT_ID,
            palette: false,
            dither: false,
            pixel_size: MAX_PIXEL_SIZE,
        });
        let min_id = pack_id(IdMeta {
            outline_id: 0,
            palette: true,
            dither: true,
            pixel_size: 1,
        });
        let min_meta = pack(Meta {
            pixel_size: 1,
            outline_id: 0,
            outline: false,
            upright: 0,
            depth: 0,
        });
        for b in max_meta
            .iter()
            .chain(&max_id)
            .chain(&min_id)
            .chain(&min_meta)
        {
            assert!(*b as u32 >= DIGIT_OFFSET && *b as u32 >= MIN_EXACT_CHANNEL);
        }
        assert!(MAX_USER_ID + 1 >= 64, "at least 64 user outline ids");
        assert!(HOVER_ID > MAX_USER_ID && SELECT_ID > HOVER_ID && SELECT_ID < OUTLINE_IDS);
        assert!(ID_CAMERA_BIT >= 20 && ID_CAMERA_BIT < 24);
    }
    #[test]
    fn id_roundtrip_all() {
        for ps in [1, 2, 4, 5, 16, 24, 255, 1000, MAX_PIXEL_SIZE] {
            for id in 0..OUTLINE_IDS {
                for (p, d) in [(true, true), (false, true), (true, false), (false, false)] {
                    let m = IdMeta {
                        outline_id: id,
                        palette: p,
                        dither: d,
                        pixel_size: ps,
                    };
                    let b = pack_id(m);
                    assert!(b.iter().all(|&x| x as u32 >= DIGIT_OFFSET));
                    assert_eq!(unpack_id(b), Some(m));
                }
            }
        }
        assert_eq!(unpack_id([10, 200, 200]), None);
        assert_eq!(unpack_id([255, 255, 255]), None);
    }
    #[test]
    fn flags_layout() {
        for id in 0..OUTLINE_IDS as i32 {
            for (p, d) in [(true, true), (false, false), (true, false)] {
                let f = object_flags(3, true, id, true, p, d);
                assert_eq!(f & 3, 2);
                assert_eq!((f >> 2) & 7, id & 7);
                assert_ne!(f & FLAG_OUTLINE, 0);
                assert_ne!(f & FLAG_SNAP, 0);
                assert_eq!(
                    id_meta_from_flags(f),
                    IdMeta {
                        outline_id: id as u32,
                        palette: p,
                        dither: d,
                        pixel_size: 3
                    }
                );
                let m = unpack(pack(Meta {
                    pixel_size: 3,
                    outline_id: (f as u32 >> 2) & 7,
                    outline: true,
                    upright: 0,
                    depth: 0,
                }))
                .unwrap();
                assert_eq!(m.outline_id, id as u32 & 7);
            }
        }
        assert_eq!(
            id_meta_from_flags(32),
            IdMeta {
                outline_id: 0,
                palette: true,
                dither: true,
                pixel_size: 1
            }
        );
        assert_eq!(pixel_size_from_flags(32 | 2), 3);
        assert_eq!(id_meta_from_flags(32 | (5 << 2)).outline_id, 5);
        let h = with_outline_id(object_flags(2, false, 9, false, false, true), HOVER_ID);
        assert_eq!(
            id_meta_from_flags(h),
            IdMeta {
                outline_id: HOVER_ID,
                palette: false,
                dither: true,
                pixel_size: 2
            }
        );
        assert_ne!(h & FLAG_OUTLINE, 0);
        assert_eq!(h & 3, 1);
    }
    #[test]
    fn large_pixel_sizes() {
        for ps in [1, 3, 4, 5, 8, 16, 24, 255, MAX_PIXEL_SIZE as i32, 100_000] {
            let f = with_outline_id(object_flags(ps, true, 17, false, true, false), SELECT_ID);
            let want = ps.clamp(1, MAX_PIXEL_SIZE as i32) as u32;
            assert!(f >= 0, "pt_flags stays a positive int");
            assert_eq!(pixel_size_from_flags(f), want);
            assert_eq!(
                (f & 3) as u32 + 1,
                want.min(META_PIXEL_SIZES),
                "meta pass gets min(size, 4)"
            );
            let m = id_meta_from_flags(f);
            assert_eq!(unpack_id(pack_id(m)), Some(m));
            assert_eq!(m.pixel_size, want);
        }
        assert_eq!(
            pixel_size_from_flags(object_flags(0, false, 0, false, true, true)),
            1
        );
    }
}
